use std::collections::{hash_map::Entry, HashMap};

use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};

use crate::{backend, config, recover_lock, AppState, CommandEntry, Problem};

struct Registration {
    shortcut: Shortcut,
    command_id: String,
    command_name: String,
    hotkey: String,
}

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
