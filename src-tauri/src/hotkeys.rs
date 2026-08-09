//! Config -> live hotkeys: turning a saved [`config::Config`] into
//! OS-registered global shortcuts, and back out into [`Problem`]s the
//! settings window can show when part of that didn't take effect.
//!
//! This is the pure, already-unit-tested half of what `lib.rs` used to hold
//! in one 1135-line file -- everything here needs at most a live
//! `tauri::AppHandle` to reach the global-shortcut plugin, never the wider
//! app wiring (`AppState` construction, the tray menu, the hotkey-dispatch
//! closure). `run()`'s `.setup()` closure is deliberately *not* split the
//! same way -- see its own doc comments for the safety-critical call
//! ordering that would otherwise be scattered across files instead of kept
//! readable top-to-bottom in one place.

use std::collections::{hash_map::Entry, HashMap};

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::{backend, config, recover_lock, AppState, CommandEntry, Problem};

/// One command's hotkey, ready to hand to the OS. Carries the command's
/// identity so a registration failure can be attributed to it without
/// matching on message text.
struct Registration {
    shortcut: Shortcut,
    command_id: String,
    command_name: String,
    hotkey: String,
}

/// Build the hotkey -> command-entry map from config.
///
/// A command whose hotkey will not parse, or whose backend will not build, is
/// skipped with a message rather than aborting the whole load — one bad
/// entry must not take the other commands down with it.
///
/// The map is keyed by the parsed `Shortcut`, not the raw config string.
/// `config::validate` dedups hotkeys case-insensitively but stores the raw
/// spelling (e.g. `"Ctrl+Alt+J"`), while the global-shortcut plugin hands
/// the handler back a `Shortcut` whose `Display` renders lowercase and
/// spells the modifier out in full (`"control+alt+j"`) — those two strings
/// never match. Parsing once here and reusing the identical `Shortcut`
/// value both for registration and as the map key sidesteps textual
/// normalization entirely: the plugin always hands the handler back the
/// exact value that was registered, and `Shortcut` derives `Eq`/`Hash`, so
/// lookup is structural rather than string-based.
fn build_command_map(cfg: &config::Config) -> (HashMap<Shortcut, CommandEntry>, Vec<Problem>) {
    let mut map: HashMap<Shortcut, CommandEntry> = HashMap::new();
    let mut problems = Vec::new();
    for command in &cfg.commands {
        let Some(backend_cfg) = cfg.backend(&command.backend_id) else {
            problems.push(Problem {
                command_id: Some(command.id.clone()),
                message: format!("{}: unknown backend", command.name),
            });
            continue;
        };
        let shortcut = match command.hotkey.parse::<Shortcut>() {
            Ok(s) => s,
            Err(e) => {
                problems.push(Problem {
                    command_id: Some(command.id.clone()),
                    message: format!(
                        "{}: unparseable hotkey {:?} ({e})",
                        command.name, command.hotkey
                    ),
                });
                continue;
            }
        };
        match backend::build(backend_cfg) {
            Ok(backend) => {
                let entry = CommandEntry {
                    id: command.id.clone(),
                    name: command.name.clone(),
                    hotkey: command.hotkey.clone(),
                    system_prompt: command.system_prompt.clone(),
                    backend,
                };
                // `config::validate` dedups hotkeys case-insensitively but
                // stores the raw spelling, so two entries that spell the
                // same modifier differently (e.g. "Ctrl+Alt+J" and
                // "Control+Alt+J") both pass validation even though they
                // parse to the identical `Shortcut`. An unchecked insert
                // would silently drop one command's binding with no signal
                // anywhere; report it as a problem instead.
                //
                // First-wins, explicitly: this used to be a bare
                // `map.insert`, which is last-wins -- while the problem
                // message named the *earlier* command as the survivor. So
                // the message pointed the user at the dead binding: they
                // would press the hotkey, the later command would fire, and
                // the diagnostic would insist the earlier one was
                // registered. First-wins keeps earlier-listed commands
                // stable and makes the message true.
                match map.entry(shortcut) {
                    Entry::Occupied(existing) => {
                        problems.push(Problem {
                            command_id: Some(command.id.clone()),
                            message: format!(
                                "{}: hotkey {:?} collides with \"{}\"'s hotkey after parsing \
                                 ({shortcut}); only \"{}\" is registered",
                                command.name,
                                command.hotkey,
                                existing.get().name,
                                existing.get().name
                            ),
                        });
                    }
                    Entry::Vacant(slot) => {
                        slot.insert(entry);
                    }
                }
            }
            Err(e) => problems.push(Problem {
                command_id: Some(command.id.clone()),
                message: format!("{}: {e}", command.name),
            }),
        }
    }
    (map, problems)
}

#[cfg(test)]
mod build_command_map_tests {
    use super::*;
    use crate::config::{BackendConfig, CommandConfig, Config, Effort, KIND_CLAUDE_CLI};

    fn backend_cfg() -> BackendConfig {
        BackendConfig {
            id: "b".into(),
            name: "Backend".into(),
            kind: KIND_CLAUDE_CLI.into(),
            base_url: None,
            model: None,
            api_key: None,
            timeout_ms: 120_000,
            effort: Effort::Low,
            session_persistence: false,
        }
    }

    fn command(id: &str, name: &str, hotkey: &str) -> CommandConfig {
        CommandConfig {
            id: id.into(),
            name: name.into(),
            backend_id: "b".into(),
            hotkey: hotkey.into(),
            system_prompt: "x".into(),
        }
    }

    #[test]
    fn colliding_hotkeys_that_differ_only_in_modifier_spelling_are_reported_not_silently_dropped() {
        // Minor: config::validate dedups hotkeys case-insensitively but
        // stores the raw spelling, so "Ctrl+Alt+J" and "Control+Alt+J"
        // both pass validation even though `Shortcut::parse` normalizes
        // both to the identical value. Without an is_some() check on
        // insert, the second command's hotkey would silently replace the
        // first's in the map with no signal anywhere that it happened.
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![
                command("one", "First", "Ctrl+Alt+J"),
                command("two", "Second", "Control+Alt+J"),
            ],
            settings: config::Settings::default(),
        };
        let (map, problems) = build_command_map(&cfg);
        assert_eq!(
            map.len(),
            1,
            "both config entries parse to the same Shortcut, so only one hotkey can ever fire"
        );
        assert!(
            problems.iter().any(|p| p.message.contains("Second")),
            "the dropped command's collision must be reported as a problem, got: {problems:?}"
        );
        assert!(
            problems.iter().any(|p| p.command_id.as_deref() == Some("two")),
            "the collision problem must carry the dropped command's id, got: {problems:?}"
        );
        // The assertion this test was missing, and the reason a real bug
        // lived here undetected: which command actually SURVIVES. The old
        // code was last-wins (bare HashMap::insert) while the message named
        // the earlier command as the survivor -- so pressing the hotkey ran
        // "Second" while the diagnostic insisted "First" was registered,
        // sending anyone debugging it to the wrong command. First-wins is
        // now pinned: the survivor must be the earlier-listed command, the
        // same one the message names.
        let survivor = map.values().next().expect("one entry must remain");
        assert_eq!(
            survivor.name, "First",
            "the earlier-listed command must win the collision, matching the message"
        );
        assert!(
            problems.iter().any(|p| p.message.contains("only \"First\" is registered")),
            "the message must name the actual survivor, got: {problems:?}"
        );
    }

    #[test]
    fn a_command_naming_an_unknown_backend_is_skipped_with_a_problem() {
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![CommandConfig {
                id: "orphan".into(),
                name: "Orphan".into(),
                backend_id: "does-not-exist".into(),
                hotkey: "Ctrl+Alt+J".into(),
                system_prompt: "x".into(),
            }],
            settings: config::Settings::default(),
        };
        let (map, problems) = build_command_map(&cfg);
        assert!(map.is_empty(), "a command with no valid backend must not reach the map");
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some("orphan"));
        assert!(problems[0].message.contains("unknown backend"), "got: {}", problems[0].message);
    }

    #[test]
    fn a_command_with_an_unparseable_hotkey_is_skipped_with_a_problem() {
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![command("bad-hotkey", "Bad Hotkey", "")],
            settings: config::Settings::default(),
        };
        let (map, problems) = build_command_map(&cfg);
        assert!(map.is_empty());
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some("bad-hotkey"));
        assert!(problems[0].message.contains("unparseable"), "got: {}", problems[0].message);
    }

    #[test]
    fn one_bad_entry_does_not_take_down_the_entries_after_it() {
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![
                command("bad", "Bad", ""),
                command("good", "Good", "Ctrl+Alt+K"),
            ],
            settings: config::Settings::default(),
        };
        let (map, problems) = build_command_map(&cfg);
        assert_eq!(map.len(), 1, "the good entry after the bad one must still register");
        assert!(map.values().any(|e| e.name == "Good"));
        assert_eq!(problems.len(), 1, "only the bad entry should produce a problem");
    }

    #[test]
    fn non_colliding_hotkeys_register_cleanly_with_no_problems() {
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![command("one", "First", "Ctrl+Alt+J"), command("two", "Second", "Ctrl+Alt+K")],
            settings: config::Settings::default(),
        };
        let (map, problems) = build_command_map(&cfg);
        assert_eq!(map.len(), 2);
        assert!(problems.is_empty(), "expected no problems, got: {problems:?}");
    }
}

/// Attempts to register every `Registration` with the OS, reporting a
/// `Problem` attributed to that command's id for each one the OS refuses
/// (e.g. already owned by another application).
///
/// Thin wrapper around `register_hotkeys_with` -- the actual OS call is the
/// one part of this that needs a live `AppHandle` and so cannot run in a
/// unit test; `register_hotkeys_with` is the attribution logic pulled out
/// so it can be exercised with a fake, always-failing registration function
/// instead. See `register_hotkeys_tests` below.
fn register_hotkeys(app: &tauri::AppHandle, registrations: &[Registration]) -> Vec<Problem> {
    register_hotkeys_with(registrations, |shortcut| {
        app.global_shortcut().register(*shortcut)
    })
}

fn register_hotkeys_with<F, E>(registrations: &[Registration], mut register: F) -> Vec<Problem>
where
    F: FnMut(&Shortcut) -> Result<(), E>,
    E: std::fmt::Display,
{
    let mut problems = Vec::new();
    for reg in registrations {
        if let Err(e) = register(&reg.shortcut) {
            problems.push(Problem {
                command_id: Some(reg.command_id.clone()),
                // Leads with the command name and the config's own spelling,
                // matching every other problem message -- and specifically
                // not `Shortcut`'s `Display`, which would print a spelling
                // the user never typed (see `CommandEntry::hotkey`).
                message: format!(
                    "{}: hotkey {:?} is already in use ({e})",
                    reg.command_name, reg.hotkey
                ),
            });
        }
    }
    problems
}

#[cfg(test)]
mod register_hotkeys_tests {
    use super::*;

    // The headline fix: `register_hotkeys` used to take a bare `&[Shortcut]`
    // and format problems with `Shortcut`'s own `Display` impl, which
    // renders lowercase and in a fixed modifier order that never matches
    // the config's spelling -- so `CommandList.svelte`'s row-matching could
    // never succeed for this problem type. Attributing by `command_id`
    // instead sidesteps string matching entirely.
    fn registration(hotkey: &str, command_id: &str, command_name: &str) -> Registration {
        Registration {
            shortcut: hotkey.parse().unwrap(),
            command_id: command_id.to_string(),
            command_name: command_name.to_string(),
            hotkey: hotkey.to_string(),
        }
    }

    #[test]
    fn a_hotkey_that_fails_to_register_is_attributed_to_its_command_id() {
        let registrations = vec![registration("Ctrl+Alt+J", "cmd-1", "Fix grammar")];

        let problems =
            register_hotkeys_with(&registrations, |_| Err("already registered by another app"));

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some("cmd-1"));
        assert!(
            problems[0].message.contains("already in use"),
            "got: {}",
            problems[0].message
        );
    }

    // The message the user actually reads must use the config's spelling.
    // `Shortcut`'s `Display` would render this same hotkey as
    // "control+alt+KeyJ" -- lowercased, reordered, raw key code -- which is
    // a spelling that appears nowhere in their config.
    #[test]
    fn the_problem_message_uses_the_config_spelling_and_names_the_command() {
        let registrations = vec![registration("Ctrl+Alt+J", "cmd-1", "Fix grammar")];

        let problems = register_hotkeys_with(&registrations, |_| Err("in use"));

        let message = &problems[0].message;
        assert!(message.contains("Fix grammar"), "must name the command, got: {message}");
        assert!(message.contains("Ctrl+Alt+J"), "must use the config spelling, got: {message}");
        assert!(
            !message.contains("control+alt+KeyJ"),
            "must not leak the parsed Shortcut's Display form, got: {message}"
        );
    }

    #[test]
    fn a_hotkey_that_registers_successfully_produces_no_problem() {
        let registrations = vec![registration("Ctrl+Alt+K", "cmd-2", "Summarise")];

        let problems = register_hotkeys_with::<_, &str>(&registrations, |_| Ok(()));

        assert!(problems.is_empty(), "expected no problems, got: {problems:?}");
    }

    #[test]
    fn two_failing_registrations_in_one_batch_are_both_separately_attributed() {
        let registrations = vec![
            registration("Ctrl+Alt+J", "cmd-1", "Fix grammar"),
            registration("Ctrl+Alt+K", "cmd-2", "Summarise"),
        ];

        let problems =
            register_hotkeys_with(&registrations, |_| Err("already registered by another app"));

        assert_eq!(problems.len(), 2);
        assert_eq!(problems[0].command_id.as_deref(), Some("cmd-1"));
        assert_eq!(problems[1].command_id.as_deref(), Some("cmd-2"));
    }

    #[test]
    fn only_the_failing_shortcut_in_a_batch_is_attributed() {
        let bad: Shortcut = "Ctrl+Alt+J".parse().unwrap();
        let registrations = vec![
            registration("Ctrl+Alt+K", "cmd-ok", "Summarise"),
            registration("Ctrl+Alt+J", "cmd-bad", "Fix grammar"),
        ];

        let problems = register_hotkeys_with(&registrations, |s| {
            if *s == bad {
                Err("in use")
            } else {
                Ok(())
            }
        });

        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some("cmd-bad"));
    }
}

/// Unregisters every hotkey, rebuilds the command map from `config`, swaps
/// both `state.commands` and `state.config` to the new values, then
/// re-registers with the OS. Returns per-command problems that did not
/// prevent the apply (an unparseable hotkey, an unknown backend, a hotkey
/// collision -- see `build_command_map` -- or a hotkey the OS refused to
/// register, e.g. one already owned by another application), plus an
/// unattributed one (`command_id: None`) if the initial unregister itself
/// failed.
///
/// Called once at startup, in `lib.rs`'s `run`/`setup`, and later by a
/// settings-window save on every save -- that's why `config` arrives as an
/// owned, already-loaded value rather than a path this function reads for
/// itself. v1 registered hotkeys once in `setup` and never again, which
/// made a settings UI a lie; this is the fix.
///
/// A cycle already running is unaffected by this call. The hotkey-dispatch
/// closure (in `run`'s `setup`) clones the matching `CommandEntry` -- and,
/// inside it, the `Arc<dyn Backend>` -- out of `state.commands` *inside* the
/// spawned cycle thread, once, right at its start and before `cycle::run`
/// itself is ever invoked (R15: this used to say "before spawning the cycle
/// thread", which described a different mechanism than the code -- the
/// clone always happened on the spawned thread, not on the handler thread
/// beforehand; the substance below was correct even though that mechanism
/// claim wasn't). `cycle::run` then only ever touches that owned clone, and
/// never looks at the map again. So swapping `state.commands` here, even
/// while a cycle is mid-flight, cannot reach it: a save during a 16-second
/// cold-model cycle takes effect starting with the *next* press, not the
/// one already running.
pub(crate) fn apply_config(
    app: &tauri::AppHandle,
    state: &AppState,
    config: config::Config,
) -> Vec<Problem> {
    let mut problems = Vec::new();
    // `suspend_hotkeys` (commands.rs) propagates this same call's Result;
    // this one used to discard it. The plugin clears its own bookkeeping
    // via `mem::take` *before* the OS-level unregister loop, and that loop
    // aborts on the first OS-level failure -- so a partial failure here
    // leaves stale OS registrations that `register_hotkeys` below would
    // then see as ordinary "already in use" collisions, with nothing
    // distinguishing them from a genuinely external app holding the key.
    // Surfacing it here at least tells whoever reads `problems` that the
    // unregister step itself was the one that went wrong.
    if let Err(e) = app.global_shortcut().unregister_all() {
        problems.push(Problem {
            command_id: None,
            message: format!(
                "failed to clear previous hotkeys ({e}); a hotkey reported as \"already in use\" \
                 below may actually be this app's own stale registration"
            ),
        });
    }

    let (map, build_problems) = build_command_map(&config);
    problems.extend(build_problems);
    let registrations: Vec<Registration> = map
        .iter()
        .map(|(s, e)| Registration {
            shortcut: *s,
            command_id: e.id.clone(),
            command_name: e.name.clone(),
            hotkey: e.hotkey.clone(),
        })
        .collect();

    *recover_lock(state.commands.lock(), "commands", Some(&state.config_dir)) = map;
    *recover_lock(state.config.lock(), "config", Some(&state.config_dir)) = config;

    problems.extend(register_hotkeys(app, &registrations));
    problems
}
