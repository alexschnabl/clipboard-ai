use std::collections::{hash_map::Entry, HashMap};
use std::sync::Arc;

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::{backend, config, plan, recover_lock, AppState, CommandEntry, Problem};

struct Registration {
    shortcut: Shortcut,
    command_id: String,
    command_name: String,
    hotkey: String,
}

fn build_command_map(cfg: &config::Config) -> (HashMap<Shortcut, CommandEntry>, Vec<Problem>) {
    let mut map: HashMap<Shortcut, CommandEntry> = HashMap::new();
    let mut problems = Vec::new();
    let vocabulary = Arc::new(cfg.vocabulary.clone());
    for command in &cfg.commands {
        if cfg.backend(&command.backend_id).is_none() {
            problems.push(Problem {
                command_id: Some(command.id.clone()),
                message: format!("{}: unknown backend", command.name),
            });
            continue;
        }
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
        match plan::build(command, cfg, vocabulary.clone(), &backend::build) {
            Ok((plan, rule_problems)) => {
                for message in rule_problems {
                    problems.push(Problem {
                        command_id: Some(command.id.clone()),
                        message: format!("{}: {message}", command.name),
                    });
                }
                let entry = CommandEntry {
                    id: command.id.clone(),
                    name: command.name.clone(),
                    hotkey: command.hotkey.clone(),
                    plan: Arc::new(plan),
                };
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
            expects_response: true,
        }
    }

    fn command(id: &str, name: &str, hotkey: &str) -> CommandConfig {
        CommandConfig {
            id: id.into(),
            name: name.into(),
            backend_id: "b".into(),
            hotkey: hotkey.into(),
            system_prompt: "x".into(),
            formatting: config::Formatting::Auto,
            app_rules: Vec::new(),
        }
    }

    #[test]
    fn colliding_hotkeys_that_differ_only_in_modifier_spelling_are_reported_not_silently_dropped() {
        let cfg = Config {
            version: 1,
            backends: vec![backend_cfg()],
            commands: vec![
                command("one", "First", "Ctrl+Alt+J"),
                command("two", "Second", "Control+Alt+J"),
            ],
            settings: config::Settings::default(),
            vocabulary: config::Vocabulary::default(),
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
                formatting: config::Formatting::Auto,
                app_rules: Vec::new(),
            }],
            settings: config::Settings::default(),
            vocabulary: config::Vocabulary::default(),
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
            vocabulary: config::Vocabulary::default(),
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
            vocabulary: config::Vocabulary::default(),
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
            vocabulary: config::Vocabulary::default(),
        };
        let (map, problems) = build_command_map(&cfg);
        assert_eq!(map.len(), 2);
        assert!(problems.is_empty(), "expected no problems, got: {problems:?}");
    }
}

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

pub const PALETTE_PROBLEM_ID: &str = "palette";
pub const PASTE_LAST_PROBLEM_ID: &str = "paste-last";
const PALETTE_NAME: &str = "Command palette";
const PASTE_LAST_NAME: &str = "Paste last result";

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GlobalShortcuts {
    pub palette: Option<Shortcut>,
    pub paste_last: Option<Shortcut>,
}

fn claim_global(
    hotkey: &str,
    id: &str,
    name: &str,
    commands: &HashMap<Shortcut, CommandEntry>,
    taken: &[(Shortcut, &str)],
    problems: &mut Vec<Problem>,
) -> Option<Registration> {
    let trimmed = hotkey.trim();
    if trimmed.is_empty() {
        return None;
    }
    let shortcut = match trimmed.parse::<Shortcut>() {
        Ok(s) => s,
        Err(e) => {
            problems.push(Problem {
                command_id: Some(id.to_string()),
                message: format!("{name}: unparseable hotkey {trimmed:?} ({e})"),
            });
            return None;
        }
    };
    let owner = commands
        .get(&shortcut)
        .map(|entry| entry.name.as_str())
        .or_else(|| taken.iter().find(|(s, _)| *s == shortcut).map(|(_, owner)| *owner));
    if let Some(owner) = owner {
        problems.push(Problem {
            command_id: Some(id.to_string()),
            message: format!(
                "{name}: hotkey {trimmed:?} is already used by \"{owner}\"; pick another one"
            ),
        });
        return None;
    }
    Some(Registration {
        shortcut,
        command_id: id.to_string(),
        command_name: name.to_string(),
        hotkey: trimmed.to_string(),
    })
}

fn build_global_shortcuts(
    settings: &config::Settings,
    commands: &HashMap<Shortcut, CommandEntry>,
) -> (GlobalShortcuts, Vec<Registration>, Vec<Problem>) {
    let mut problems = Vec::new();
    let mut registrations = Vec::new();
    let mut globals = GlobalShortcuts::default();

    if let Some(reg) = claim_global(
        &settings.palette_hotkey,
        PALETTE_PROBLEM_ID,
        PALETTE_NAME,
        commands,
        &[],
        &mut problems,
    ) {
        globals.palette = Some(reg.shortcut);
        registrations.push(reg);
    }

    if settings.paste_last_enabled {
        let taken: Vec<(Shortcut, &str)> =
            globals.palette.iter().map(|s| (*s, PALETTE_NAME)).collect();
        if let Some(reg) = claim_global(
            &settings.paste_last_hotkey,
            PASTE_LAST_PROBLEM_ID,
            PASTE_LAST_NAME,
            commands,
            &taken,
            &mut problems,
        ) {
            globals.paste_last = Some(reg.shortcut);
            registrations.push(reg);
        }
    }

    (globals, registrations, problems)
}

#[cfg(test)]
mod global_shortcut_tests {
    use super::*;
    use crate::config::{BackendConfig, CommandConfig, Config, Effort, Settings, KIND_CLAUDE_CLI};

    fn commands_with(hotkey: &str) -> HashMap<Shortcut, CommandEntry> {
        let cfg = Config {
            version: 2,
            backends: vec![BackendConfig {
                id: "b".into(),
                name: "Backend".into(),
                kind: KIND_CLAUDE_CLI.into(),
                base_url: None,
                model: None,
                api_key: None,
                timeout_ms: 120_000,
                effort: Effort::Low,
                expects_response: true,
            }],
            commands: vec![CommandConfig {
                id: "fix".into(),
                name: "Fix grammar".into(),
                backend_id: "b".into(),
                hotkey: hotkey.into(),
                system_prompt: "x".into(),
                formatting: config::Formatting::Auto,
                app_rules: Vec::new(),
            }],
            settings: Settings::default(),
            vocabulary: config::Vocabulary::default(),
        };
        build_command_map(&cfg).0
    }

    #[test]
    fn the_defaults_claim_the_palette_and_leave_paste_again_off() {
        let (globals, regs, problems) =
            build_global_shortcuts(&Settings::default(), &commands_with("Ctrl+Alt+J"));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(globals.palette, Some("Ctrl+Alt+Space".parse().unwrap()));
        assert_eq!(globals.paste_last, None);
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].command_id, PALETTE_PROBLEM_ID);
    }

    #[test]
    fn paste_again_registers_once_turned_on() {
        let settings = Settings { paste_last_enabled: true, ..Settings::default() };
        let (globals, regs, problems) = build_global_shortcuts(&settings, &commands_with("Ctrl+Alt+J"));
        assert!(problems.is_empty(), "{problems:?}");
        assert_eq!(globals.paste_last, Some("Ctrl+Alt+V".parse().unwrap()));
        assert_eq!(regs.len(), 2);
    }

    #[test]
    fn a_command_already_on_the_palette_hotkey_keeps_it_and_the_clash_is_reported() {
        let (globals, regs, problems) =
            build_global_shortcuts(&Settings::default(), &commands_with("Ctrl+Alt+Space"));
        assert_eq!(globals.palette, None, "the existing command keeps working as before");
        assert!(regs.is_empty());
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].command_id.as_deref(), Some(PALETTE_PROBLEM_ID));
        assert!(problems[0].message.contains("Fix grammar"), "{}", problems[0].message);
    }

    #[test]
    fn a_clash_is_found_even_when_the_spelling_differs() {
        let (globals, _, problems) =
            build_global_shortcuts(&Settings::default(), &commands_with("Control+Alt+Space"));
        assert_eq!(globals.palette, None);
        assert_eq!(problems.len(), 1);
    }

    #[test]
    fn paste_again_on_the_palette_hotkey_is_refused_in_favour_of_the_palette() {
        let settings = Settings {
            paste_last_enabled: true,
            paste_last_hotkey: "Ctrl+Alt+Space".into(),
            ..Settings::default()
        };
        let (globals, _, problems) = build_global_shortcuts(&settings, &commands_with("Ctrl+Alt+J"));
        assert!(globals.palette.is_some());
        assert_eq!(globals.paste_last, None);
        assert_eq!(problems[0].command_id.as_deref(), Some(PASTE_LAST_PROBLEM_ID));
        assert!(problems[0].message.contains(PALETTE_NAME));
    }

    #[test]
    fn an_unparseable_or_empty_palette_hotkey_is_handled_without_panicking() {
        let settings = Settings { palette_hotkey: "Ctrl+Alt+Nope".into(), ..Settings::default() };
        let (globals, _, problems) = build_global_shortcuts(&settings, &HashMap::new());
        assert_eq!(globals.palette, None);
        assert!(problems[0].message.contains("unparseable"));

        let settings = Settings { palette_hotkey: "  ".into(), ..Settings::default() };
        let (globals, regs, problems) = build_global_shortcuts(&settings, &HashMap::new());
        assert_eq!(globals.palette, None);
        assert!(regs.is_empty() && problems.is_empty(), "an empty hotkey just means no shortcut");
    }

    #[test]
    fn a_failed_palette_registration_is_reported_like_any_other_hotkey() {
        let (_, regs, _) = build_global_shortcuts(&Settings::default(), &HashMap::new());
        let problems = register_hotkeys_with(&regs, |_| Err("taken by another app"));
        assert_eq!(problems[0].command_id.as_deref(), Some(PALETTE_PROBLEM_ID));
        assert!(problems[0].message.contains("Command palette"));
        assert!(problems[0].message.contains("already in use"));
    }
}

pub(crate) fn apply_config(
    app: &tauri::AppHandle,
    state: &AppState,
    config: config::Config,
) -> Vec<Problem> {
    let mut problems = Vec::new();
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
    problems.extend(crate::localapi::apply_for_app(app, config.settings.local_api));
    let mut registrations: Vec<Registration> = map
        .iter()
        .map(|(s, e)| Registration {
            shortcut: *s,
            command_id: e.id.clone(),
            command_name: e.name.clone(),
            hotkey: e.hotkey.clone(),
        })
        .collect();
    let (globals, global_registrations, global_problems) =
        build_global_shortcuts(&config.settings, &map);
    problems.extend(global_problems);
    registrations.extend(global_registrations);

    *recover_lock(state.commands.lock(), "commands", Some(&state.config_dir)) = map;
    *recover_lock(state.shortcuts.lock(), "shortcuts", Some(&state.config_dir)) = globals;
    *recover_lock(state.config.lock(), "config", Some(&state.config_dir)) = config;

    problems.extend(register_hotkeys(app, &registrations));
    crate::runner::reregister_escape_if_running(app, state);
    problems
}
