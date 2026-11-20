use std::{collections::HashMap, path::Path};

use thiserror::Error;

use crate::config::{self, Config};

pub const TARGET_PREFIX: &str = "clipboard-ai:backend:";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecretError {
    #[error("Windows Credential Manager error: {0}")]
    Store(String),
    #[error("the key read back from Windows Credential Manager did not match what was written")]
    Mismatch,
}

pub trait SecretStore: Send + Sync {
    fn read(&self, target: &str) -> Result<Option<String>, SecretError>;
    fn write(&self, target: &str, secret: &str) -> Result<(), SecretError>;
    fn delete(&self, target: &str) -> Result<(), SecretError>;
}

pub fn target_for(backend_id: &str) -> String {
    format!("{TARGET_PREFIX}{backend_id}")
}

pub(crate) fn backup_target_for(backend_id: &str) -> String {
    format!("{TARGET_PREFIX}{backend_id}:previous")
}

pub fn api_key(backend_id: &str) -> Result<Option<String>, SecretError> {
    resolve_api_key(backend_id, None)
}

pub fn resolve_api_key(
    backend_id: &str,
    inline_key: Option<&str>,
) -> Result<Option<String>, SecretError> {
    resolve_with(&CredentialManager, backend_id, inline_key)
}

pub fn resolve_with(
    store: &dyn SecretStore,
    backend_id: &str,
    inline_key: Option<&str>,
) -> Result<Option<String>, SecretError> {
    if let Some(key) = inline_key.filter(|k| !k.trim().is_empty()) {
        return Ok(Some(key.to_string()));
    }
    Ok(store.read(&target_for(backend_id))?.filter(|k| !k.trim().is_empty()))
}

pub(crate) fn uses_api_key(kind: &str) -> bool {
    matches!(kind, config::KIND_OPENAI | config::KIND_ANTHROPIC | config::KIND_HTTP)
}

pub(crate) fn write_verified(
    store: &dyn SecretStore,
    target: &str,
    secret: &str,
) -> Result<(), SecretError> {
    store.write(target, secret)?;
    match store.read(target)? {
        Some(back) if back == secret => Ok(()),
        _ => Err(SecretError::Mismatch),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MigrationFailure {
    pub backend_id: String,
    pub backend_name: String,
    pub reason: String,
}

impl MigrationFailure {
    pub fn message(&self) -> String {
        format!(
            "Couldn't move the API key for \"{}\" into Windows Credential Manager ({}). \
             It stays in config.json for now, and the app tries again on the next start.",
            self.backend_name, self.reason
        )
    }
}

#[derive(Debug, Default)]
pub(crate) struct MigrationOutcome {
    pub migrated: Vec<String>,
    pub backed_up: Vec<String>,
    pub failures: Vec<MigrationFailure>,
}

fn migrate_one(
    store: &dyn SecretStore,
    backend_id: &str,
    key: &str,
) -> Result<bool, SecretError> {
    let target = target_for(backend_id);
    let existing = store.read(&target)?.filter(|v| !v.trim().is_empty());
    let backed_up = match existing {
        Some(existing) if existing != key => {
            write_verified(store, &backup_target_for(backend_id), &existing)?;
            true
        }
        _ => false,
    };
    write_verified(store, &target, key)?;
    Ok(backed_up)
}

pub(crate) fn migrate_inline_keys(config: &mut Config, store: &dyn SecretStore) -> MigrationOutcome {
    let mut outcome = MigrationOutcome::default();
    for backend in &mut config.backends {
        let Some(key) = backend.api_key.clone().filter(|k| !k.trim().is_empty()) else {
            continue;
        };
        match migrate_one(store, &backend.id, &key) {
            Ok(backed_up) => {
                backend.api_key = None;
                if backed_up {
                    outcome.backed_up.push(backend.id.clone());
                }
                outcome.migrated.push(backend.id.clone());
            }
            Err(e) => outcome.failures.push(MigrationFailure {
                backend_id: backend.id.clone(),
                backend_name: backend.name.clone(),
                reason: e.to_string(),
            }),
        }
    }
    outcome
}

pub(crate) fn migrate_config_file(
    path: &Path,
    config: &mut Config,
    store: &dyn SecretStore,
) -> Vec<MigrationFailure> {
    let MigrationOutcome { migrated, backed_up, failures } = migrate_inline_keys(config, store);
    let dir = path.parent();
    for id in &backed_up {
        let line = format!(
            "SECRETS replaced-a-different-stored-key backend={id} old-key-kept-as={}",
            backup_target_for(id)
        );
        eprintln!("{line}");
        if let Some(dir) = dir {
            crate::status::append_log(dir, &line);
        }
    }
    if !migrated.is_empty() {
        let line = match config::write_atomic(path, config) {
            Ok(()) => format!("SECRETS moved-keys-to-credential-manager count={}", migrated.len()),
            Err(e) => format!(
                "SECRETS keys-stored-but-config-rewrite-failed count={} reason={e}; \
                 the keys stay in config.json until the next successful save",
                migrated.len()
            ),
        };
        eprintln!("{line}");
        if let Some(dir) = dir {
            crate::status::append_log(dir, &line);
        }
    }
    for failure in &failures {
        let line = format!(
            "SECRETS key-migration-failed backend={} reason={}",
            failure.backend_id, failure.reason
        );
        eprintln!("{line}");
        if let Some(dir) = dir {
            crate::status::append_log(dir, &line);
        }
    }
    failures
}

pub(crate) fn stored_key_ids(config: &Config, store: &dyn SecretStore) -> Vec<String> {
    config
        .backends
        .iter()
        .filter(|b| uses_api_key(&b.kind))
        .filter(|b| matches!(resolve_with(store, &b.id, b.api_key.as_deref()), Ok(Some(_))))
        .map(|b| b.id.clone())
        .collect()
}

pub(crate) fn inline_key_ids(config: &Config) -> Vec<String> {
    config
        .backends
        .iter()
        .filter(|b| b.api_key.as_deref().is_some_and(|k| !k.trim().is_empty()))
        .map(|b| b.id.clone())
        .collect()
}

#[derive(Debug, Default)]
pub(crate) struct KeyApplication {
    pub warnings: Vec<String>,
    undo: Vec<(String, Option<String>)>,
}

impl KeyApplication {
    pub fn roll_back(&self, store: &dyn SecretStore) -> Vec<String> {
        let mut failed = Vec::new();
        for (target, previous) in self.undo.iter().rev() {
            let result = match previous {
                Some(value) => store.write(target, value),
                None => store.delete(target),
            };
            if let Err(e) = result {
                failed.push(format!("{target}: {e}"));
            }
        }
        failed
    }
}

pub(crate) fn apply_key_changes(
    config: &mut Config,
    previous: &Config,
    key_changes: &HashMap<String, Option<String>>,
    store: &dyn SecretStore,
) -> Result<KeyApplication, String> {
    let mut applied = KeyApplication::default();
    for backend in &mut config.backends {
        let change = match key_changes.get(&backend.id) {
            Some(change) => Some(change.clone()),
            None => match backend.api_key.take().filter(|k| !k.trim().is_empty()) {
                Some(inline) => Some(Some(inline)),
                None => {
                    let carried = previous
                        .backend(&backend.id)
                        .and_then(|p| p.api_key.clone())
                        .filter(|k| !k.trim().is_empty());
                    if let Some(carried) = carried {
                        let target = target_for(&backend.id);
                        match store.read(&target) {
                            Ok(before) if migrate_one(store, &backend.id, &carried).is_ok() => {
                                applied.undo.push((target, before));
                                backend.api_key = None;
                            }
                            _ => backend.api_key = Some(carried),
                        }
                    }
                    None
                }
            },
        };
        let Some(change) = change else { continue };
        let target = target_for(&backend.id);
        let before = match store.read(&target) {
            Ok(before) => before,
            Err(e) => {
                applied.roll_back(store);
                return Err(format!(
                    "Couldn't read the current API key for \"{}\" from Windows Credential Manager \
                     ({e}), so nothing was changed.",
                    backend.name
                ));
            }
        };
        match change.map(|k| k.trim().to_string()).filter(|k| !k.is_empty()) {
            Some(key) => match write_verified(store, &target, &key) {
                Ok(()) => {
                    applied.undo.push((target, before));
                    backend.api_key = None;
                }
                Err(e) => {
                    let _ = match &before {
                        Some(value) => store.write(&target, value),
                        None => store.delete(&target),
                    };
                    backend.api_key = Some(key);
                    applied.warnings.push(format!(
                        "Couldn't save the API key for \"{}\" in Windows Credential Manager ({e}). \
                         It was saved in config.json in plain text instead.",
                        backend.name
                    ));
                }
            },
            None => {
                if let Err(e) = store.delete(&target) {
                    applied.roll_back(store);
                    return Err(format!(
                        "Couldn't remove the API key for \"{}\" from Windows Credential Manager ({e}).",
                        backend.name
                    ));
                }
                applied.undo.push((target, before));
                backend.api_key = None;
            }
        }
    }
    Ok(applied)
}

pub(crate) fn delete_removed(
    previous: &Config,
    current: &Config,
    removed: &[String],
    store: &dyn SecretStore,
) -> Vec<String> {
    let mut warnings = Vec::new();
    for old in &previous.backends {
        if current.backend(&old.id).is_some() || !removed.contains(&old.id) {
            continue;
        }
        let _ = store.delete(&backup_target_for(&old.id));
        if let Err(e) = store.delete(&target_for(&old.id)) {
            warnings.push(format!(
                "Couldn't remove the API key of the deleted backend \"{}\" from Windows \
                 Credential Manager ({e}). Remove \"{}\" there by hand if you like.",
                old.name,
                target_for(&old.id)
            ));
        }
    }
    warnings
}

pub struct CredentialManager;

#[cfg(windows)]
mod windows_store {
    use windows::{
        core::{HSTRING, PCWSTR, PWSTR},
        Win32::{
            Foundation::ERROR_NOT_FOUND,
            Security::Credentials::{
                CredDeleteW, CredFree, CredReadW, CredWriteW, CREDENTIALW, CRED_FLAGS,
                CRED_PERSIST_LOCAL_MACHINE, CRED_TYPE_GENERIC,
            },
        },
    };

    use super::{CredentialManager, SecretError, SecretStore};

    const MAX_BLOB_BYTES: usize = 5 * 512;

    fn is_not_found(e: &windows::core::Error) -> bool {
        e.code() == ERROR_NOT_FOUND.to_hresult()
    }

    fn store_error(e: windows::core::Error) -> SecretError {
        SecretError::Store(format!("{} (0x{:08X})", e.message(), e.code().0))
    }

    impl SecretStore for CredentialManager {
        fn read(&self, target: &str) -> Result<Option<String>, SecretError> {
            let target = HSTRING::from(target);
            let mut raw: *mut CREDENTIALW = std::ptr::null_mut();
            match unsafe { CredReadW(&target, CRED_TYPE_GENERIC, None, &mut raw) } {
                Ok(()) => {}
                Err(e) if is_not_found(&e) => return Ok(None),
                Err(e) => return Err(store_error(e)),
            }
            let result = unsafe {
                let cred = &*raw;
                let len = cred.CredentialBlobSize as usize;
                if cred.CredentialBlob.is_null() || len == 0 {
                    Ok(Some(String::new()))
                } else if len % 2 != 0 {
                    Err(SecretError::Store("stored value is not UTF-16 text".into()))
                } else {
                    let bytes = std::slice::from_raw_parts(cred.CredentialBlob, len);
                    let units: Vec<u16> =
                        bytes.chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
                    String::from_utf16(&units)
                        .map(Some)
                        .map_err(|_| SecretError::Store("stored value is not UTF-16 text".into()))
                }
            };
            unsafe { CredFree(raw as *const _) };
            result
        }

        fn write(&self, target: &str, secret: &str) -> Result<(), SecretError> {
            let mut blob: Vec<u8> = secret.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
            if blob.len() > MAX_BLOB_BYTES {
                return Err(SecretError::Store(format!(
                    "the key is longer than the {} characters Credential Manager can hold",
                    MAX_BLOB_BYTES / 2
                )));
            }
            let mut target_w: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
            let mut user_w: Vec<u16> = "Clipboard AI".encode_utf16().chain(Some(0)).collect();
            let mut comment_w: Vec<u16> =
                "API key used by Clipboard AI".encode_utf16().chain(Some(0)).collect();
            let credential = CREDENTIALW {
                Flags: CRED_FLAGS(0),
                Type: CRED_TYPE_GENERIC,
                TargetName: PWSTR(target_w.as_mut_ptr()),
                Comment: PWSTR(comment_w.as_mut_ptr()),
                CredentialBlobSize: blob.len() as u32,
                CredentialBlob: blob.as_mut_ptr(),
                Persist: CRED_PERSIST_LOCAL_MACHINE,
                UserName: PWSTR(user_w.as_mut_ptr()),
                ..Default::default()
            };
            let result = unsafe { CredWriteW(&credential, 0) }.map_err(store_error);
            blob.iter_mut().for_each(|b| *b = 0);
            result
        }

        fn delete(&self, target: &str) -> Result<(), SecretError> {
            let target_w: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
            match unsafe { CredDeleteW(PCWSTR(target_w.as_ptr()), CRED_TYPE_GENERIC, None) } {
                Ok(()) => Ok(()),
                Err(e) if is_not_found(&e) => Ok(()),
                Err(e) => Err(store_error(e)),
            }
        }
    }
}

#[cfg(not(windows))]
impl SecretStore for CredentialManager {
    fn read(&self, _target: &str) -> Result<Option<String>, SecretError> {
        Err(SecretError::Store("only Windows Credential Manager is supported".into()))
    }
    fn write(&self, _target: &str, _secret: &str) -> Result<(), SecretError> {
        Err(SecretError::Store("only Windows Credential Manager is supported".into()))
    }
    fn delete(&self, _target: &str) -> Result<(), SecretError> {
        Err(SecretError::Store("only Windows Credential Manager is supported".into()))
    }
}

#[cfg(test)]
pub(crate) mod fake {
    use std::{collections::HashMap, sync::Mutex};

    use super::{SecretError, SecretStore};

    #[derive(Default)]
    pub struct FakeStore {
        pub entries: Mutex<HashMap<String, String>>,
        pub fail_writes: bool,
        pub fail_reads: bool,
        pub fail_deletes: bool,
        pub corrupt_reads: bool,
    }

    impl FakeStore {
        pub fn get(&self, target: &str) -> Option<String> {
            self.entries.lock().unwrap().get(target).cloned()
        }
        pub fn put(&self, target: &str, value: &str) {
            self.entries.lock().unwrap().insert(target.to_string(), value.to_string());
        }
    }

    impl SecretStore for FakeStore {
        fn read(&self, target: &str) -> Result<Option<String>, SecretError> {
            if self.fail_reads {
                return Err(SecretError::Store("read refused".into()));
            }
            let value = self.get(target);
            Ok(if self.corrupt_reads { value.map(|v| format!("{v}-garbled")) } else { value })
        }
        fn write(&self, target: &str, secret: &str) -> Result<(), SecretError> {
            if self.fail_writes {
                return Err(SecretError::Store("write refused".into()));
            }
            self.put(target, secret);
            Ok(())
        }
        fn delete(&self, target: &str) -> Result<(), SecretError> {
            if self.fail_deletes {
                return Err(SecretError::Store("delete refused".into()));
            }
            self.entries.lock().unwrap().remove(target);
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::fake::FakeStore;
    use super::*;
    use crate::config::{default_config, BackendConfig, Effort, KIND_ANTHROPIC, KIND_CLAUDE_CLI};

    fn keyed_backend(id: &str, key: Option<&str>) -> BackendConfig {
        BackendConfig {
            id: id.into(),
            name: format!("Backend {id}"),
            kind: KIND_ANTHROPIC.into(),
            base_url: None,
            model: None,
            api_key: key.map(Into::into),
            timeout_ms: 120_000,
            effort: Effort::Low,
            expects_response: true,
        }
    }

    fn config_with(backends: Vec<BackendConfig>) -> Config {
        let mut cfg = default_config();
        cfg.backends = backends;
        cfg.commands.clear();
        cfg
    }

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("cai-test-secrets-{}-{}", std::process::id(), tag));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn targets_are_namespaced_per_backend_id() {
        assert_eq!(target_for("abc"), "clipboard-ai:backend:abc");
    }

    #[test]
    fn an_inline_key_moves_into_the_store_and_leaves_the_file() {
        let dir = temp_dir("migrate-ok");
        let path = dir.join("config.json");
        let mut cfg = config_with(vec![keyed_backend("a", Some("sk-test-a")), keyed_backend("b", None)]);
        config::write_atomic(&path, &cfg).unwrap();
        let store = FakeStore::default();

        let failures = migrate_config_file(&path, &mut cfg, &store);

        assert!(failures.is_empty());
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-test-a"));
        assert!(cfg.backends[0].api_key.is_none());
        let on_disk = config::parse(&fs::read_to_string(&path).unwrap()).unwrap();
        assert!(on_disk.backends.iter().all(|b| b.api_key.is_none()));
        assert!(!fs::read_to_string(&path).unwrap().contains("sk-test-a"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_refused_write_keeps_the_key_in_the_file_and_reports_it() {
        let dir = temp_dir("migrate-refused");
        let path = dir.join("config.json");
        let mut cfg = config_with(vec![keyed_backend("a", Some("sk-test-a"))]);
        config::write_atomic(&path, &cfg).unwrap();
        let store = FakeStore { fail_writes: true, ..Default::default() };

        let failures = migrate_config_file(&path, &mut cfg, &store);

        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].backend_id, "a");
        assert!(failures[0].message().contains("Backend a"));
        assert!(!failures[0].message().contains("sk-test-a"), "never echo the key");
        assert_eq!(cfg.backends[0].api_key.as_deref(), Some("sk-test-a"));
        assert!(fs::read_to_string(&path).unwrap().contains("sk-test-a"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_read_back_mismatch_counts_as_a_failure_and_keeps_the_key() {
        let mut cfg = config_with(vec![keyed_backend("a", Some("sk-test-a"))]);
        let store = FakeStore { corrupt_reads: true, ..Default::default() };
        let outcome = migrate_inline_keys(&mut cfg, &store);
        assert!(outcome.migrated.is_empty());
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(cfg.backends[0].api_key.as_deref(), Some("sk-test-a"));
    }

    #[test]
    fn a_refused_read_back_keeps_the_key() {
        let mut cfg = config_with(vec![keyed_backend("a", Some("sk-test-a"))]);
        let store = FakeStore { fail_reads: true, ..Default::default() };
        let outcome = migrate_inline_keys(&mut cfg, &store);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(cfg.backends[0].api_key.as_deref(), Some("sk-test-a"));
    }

    #[test]
    fn one_failing_backend_does_not_block_the_others() {
        struct PickyStore(FakeStore);
        impl SecretStore for PickyStore {
            fn read(&self, t: &str) -> Result<Option<String>, SecretError> {
                self.0.read(t)
            }
            fn write(&self, t: &str, s: &str) -> Result<(), SecretError> {
                if t.ends_with(":bad") {
                    Err(SecretError::Store("nope".into()))
                } else {
                    self.0.write(t, s)
                }
            }
            fn delete(&self, t: &str) -> Result<(), SecretError> {
                self.0.delete(t)
            }
        }
        let mut cfg =
            config_with(vec![keyed_backend("bad", Some("k1")), keyed_backend("good", Some("k2"))]);
        let store = PickyStore(FakeStore::default());
        let outcome = migrate_inline_keys(&mut cfg, &store);
        assert_eq!(outcome.migrated, vec!["good".to_string()]);
        assert_eq!(outcome.failures.len(), 1);
        assert_eq!(cfg.backends[0].api_key.as_deref(), Some("k1"));
        assert!(cfg.backends[1].api_key.is_none());
    }

    #[test]
    fn a_failed_file_rewrite_still_leaves_every_key_recoverable() {
        let dir = temp_dir("migrate-readonly");
        let path = dir.join("config.json");
        let mut cfg = config_with(vec![keyed_backend("a", Some("sk-test-a"))]);
        config::write_atomic(&path, &cfg).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        fs::set_permissions(&path, perms).unwrap();
        let store = FakeStore::default();

        let failures = migrate_config_file(&path, &mut cfg, &store);

        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(false);
        let _ = fs::set_permissions(&path, perms);
        let on_disk = fs::read_to_string(&path).unwrap();
        let _ = fs::remove_dir_all(&dir);

        assert!(failures.is_empty());
        assert!(on_disk.contains("sk-test-a"), "the file keeps its copy when the rewrite fails");
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-test-a"));
        assert_eq!(resolve_with(&store, "a", None).unwrap().as_deref(), Some("sk-test-a"));
    }

    #[test]
    fn migration_is_idempotent() {
        let mut cfg = config_with(vec![keyed_backend("a", Some("sk-test-a"))]);
        let store = FakeStore::default();
        migrate_inline_keys(&mut cfg, &store);
        let outcome = migrate_inline_keys(&mut cfg, &store);
        assert!(outcome.migrated.is_empty() && outcome.failures.is_empty());
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-test-a"));
    }

    #[test]
    fn blank_inline_keys_are_left_alone() {
        let mut cfg = config_with(vec![keyed_backend("a", Some("   "))]);
        let store = FakeStore::default();
        let outcome = migrate_inline_keys(&mut cfg, &store);
        assert!(outcome.migrated.is_empty() && outcome.failures.is_empty());
        assert!(store.get(&target_for("a")).is_none());
    }

    #[test]
    fn an_old_config_file_with_plain_keys_still_loads_and_then_migrates() {
        let json = r#"{"version":2,"backends":[{"id":"x","name":"X","kind":"anthropic-api",
            "baseUrl":null,"model":null,"apiKey":"sk-legacy","timeoutMs":120000}],"commands":[]}"#;
        let mut cfg = config::parse(json).expect("old files must still load");
        assert_eq!(cfg.backends[0].api_key.as_deref(), Some("sk-legacy"));
        let store = FakeStore::default();
        migrate_inline_keys(&mut cfg, &store);
        assert_eq!(store.get(&target_for("x")).as_deref(), Some("sk-legacy"));
    }

    #[test]
    fn resolution_prefers_an_unmigrated_inline_key_then_the_store() {
        let store = FakeStore::default();
        store.put(&target_for("a"), "from-store");
        assert_eq!(resolve_with(&store, "a", Some("inline")).unwrap().as_deref(), Some("inline"));
        assert_eq!(resolve_with(&store, "a", Some(" ")).unwrap().as_deref(), Some("from-store"));
        assert_eq!(resolve_with(&store, "a", None).unwrap().as_deref(), Some("from-store"));
        assert_eq!(resolve_with(&store, "missing", None).unwrap(), None);
    }

    #[test]
    fn a_new_key_from_the_editor_goes_to_the_store_not_the_file() {
        let previous = config_with(vec![keyed_backend("a", None)]);
        let mut next = previous.clone();
        let store = FakeStore::default();
        let changes = HashMap::from([("a".to_string(), Some("sk-new".to_string()))]);
        let applied = apply_key_changes(&mut next, &previous, &changes, &store).unwrap();
        assert!(applied.warnings.is_empty());
        assert!(next.backends[0].api_key.is_none());
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-new"));
    }

    #[test]
    fn a_save_without_key_changes_keeps_an_unmigrated_inline_key_when_the_store_still_fails() {
        let previous = config_with(vec![keyed_backend("a", Some("sk-legacy"))]);
        let mut next = config_with(vec![keyed_backend("a", None)]);
        let store = FakeStore { fail_writes: true, ..Default::default() };
        let applied = apply_key_changes(&mut next, &previous, &HashMap::new(), &store).unwrap();
        assert_eq!(next.backends[0].api_key.as_deref(), Some("sk-legacy"));
        assert!(applied.warnings.is_empty(), "a carried key must not warn on every save");
    }

    #[test]
    fn a_save_moves_an_unmigrated_inline_key_once_the_store_works() {
        let previous = config_with(vec![keyed_backend("a", Some("sk-legacy"))]);
        let mut next = config_with(vec![keyed_backend("a", None)]);
        let store = FakeStore::default();
        let applied = apply_key_changes(&mut next, &previous, &HashMap::new(), &store).unwrap();
        assert!(next.backends[0].api_key.is_none());
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-legacy"));
        applied.roll_back(&store);
        assert!(store.get(&target_for("a")).is_none(), "roll back undoes the move too");
    }

    #[test]
    fn migration_never_destroys_a_different_stored_key() {
        let mut cfg = config_with(vec![keyed_backend("a", Some("sk-inline-old"))]);
        let store = FakeStore::default();
        store.put(&target_for("a"), "sk-stored-newer");
        let outcome = migrate_inline_keys(&mut cfg, &store);
        assert_eq!(outcome.backed_up, vec!["a".to_string()]);
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-inline-old"));
        assert_eq!(store.get(&backup_target_for("a")).as_deref(), Some("sk-stored-newer"));
    }

    #[test]
    fn an_unreadable_current_key_aborts_a_key_change_without_touching_anything() {
        let previous = config_with(vec![keyed_backend("a", None)]);
        let mut next = previous.clone();
        let store = FakeStore { fail_reads: true, ..Default::default() };
        store.put(&target_for("a"), "sk-old");
        let changes = HashMap::from([("a".to_string(), Some("sk-new".to_string()))]);
        assert!(apply_key_changes(&mut next, &previous, &changes, &store).is_err());
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-old"));
    }

    #[test]
    fn a_store_failure_on_save_falls_back_to_the_file_with_a_warning() {
        let previous = config_with(vec![keyed_backend("a", None)]);
        let mut next = previous.clone();
        let store = FakeStore { fail_writes: true, ..Default::default() };
        let changes = HashMap::from([("a".to_string(), Some("sk-new".to_string()))]);
        let applied = apply_key_changes(&mut next, &previous, &changes, &store).unwrap();
        assert_eq!(next.backends[0].api_key.as_deref(), Some("sk-new"));
        assert_eq!(applied.warnings.len(), 1);
        assert!(!applied.warnings[0].contains("sk-new"));
    }

    #[test]
    fn clearing_a_key_deletes_it_and_roll_back_restores_it() {
        let previous = config_with(vec![keyed_backend("a", None)]);
        let mut next = previous.clone();
        let store = FakeStore::default();
        store.put(&target_for("a"), "sk-old");
        let changes = HashMap::from([("a".to_string(), None)]);
        let applied = apply_key_changes(&mut next, &previous, &changes, &store).unwrap();
        assert!(store.get(&target_for("a")).is_none());
        applied.roll_back(&store);
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-old"));
    }

    #[test]
    fn roll_back_restores_a_replaced_key() {
        let previous = config_with(vec![keyed_backend("a", None)]);
        let mut next = previous.clone();
        let store = FakeStore::default();
        store.put(&target_for("a"), "sk-old");
        let changes = HashMap::from([("a".to_string(), Some("sk-new".to_string()))]);
        let applied = apply_key_changes(&mut next, &previous, &changes, &store).unwrap();
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-new"));
        applied.roll_back(&store);
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-old"));
    }

    #[test]
    fn a_refused_delete_fails_the_save_and_keeps_the_key() {
        let previous = config_with(vec![keyed_backend("a", None)]);
        let mut next = previous.clone();
        let store = FakeStore { fail_deletes: true, ..Default::default() };
        store.put(&target_for("a"), "sk-old");
        let changes = HashMap::from([("a".to_string(), None)]);
        assert!(apply_key_changes(&mut next, &previous, &changes, &store).is_err());
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("sk-old"));
    }

    #[test]
    fn deleting_a_backend_deletes_its_credential_only() {
        let previous = config_with(vec![keyed_backend("a", None), keyed_backend("b", None)]);
        let current = config_with(vec![keyed_backend("b", None)]);
        let store = FakeStore::default();
        store.put(&target_for("a"), "ka");
        store.put(&target_for("b"), "kb");
        let warnings = delete_removed(&previous, &current, &["a".to_string()], &store);
        assert!(warnings.is_empty());
        assert!(store.get(&target_for("a")).is_none());
        assert_eq!(store.get(&target_for("b")).as_deref(), Some("kb"));
    }

    #[test]
    fn a_backend_missing_from_a_stale_save_keeps_its_credential() {
        let previous = config_with(vec![keyed_backend("a", None), keyed_backend("b", None)]);
        let stale = config_with(vec![keyed_backend("b", None)]);
        let store = FakeStore::default();
        store.put(&target_for("a"), "ka");
        delete_removed(&previous, &stale, &[], &store);
        assert_eq!(store.get(&target_for("a")).as_deref(), Some("ka"));
    }

    #[test]
    fn stored_key_ids_lists_only_keyed_backends_with_a_key() {
        let mut cli = keyed_backend("cli", None);
        cli.kind = KIND_CLAUDE_CLI.into();
        let cfg = config_with(vec![
            keyed_backend("stored", None),
            keyed_backend("inline", Some("k")),
            keyed_backend("none", None),
            cli,
        ]);
        let store = FakeStore::default();
        store.put(&target_for("stored"), "k");
        store.put(&target_for("cli"), "k");
        assert_eq!(stored_key_ids(&cfg, &store), vec!["stored".to_string(), "inline".to_string()]);
    }

    #[cfg(windows)]
    #[test]
    fn the_real_credential_manager_round_trips_a_test_credential() {
        let target = format!("clipboard-ai-test:{}:round-trip", std::process::id());
        let store = CredentialManager;
        let _ = store.delete(&target);
        assert_eq!(store.read(&target).unwrap(), None);
        write_verified(&store, &target, "test-value-äö-🔑").unwrap();
        assert_eq!(store.read(&target).unwrap().as_deref(), Some("test-value-äö-🔑"));
        store.delete(&target).unwrap();
        assert_eq!(store.read(&target).unwrap(), None);
        store.delete(&target).expect("deleting a missing credential is not an error");
    }
}
