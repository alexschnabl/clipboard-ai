# Clipboard AI v2 Settings UI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A settings window where backends and commands can be created, edited and deleted, with changes taking effect immediately and no restart.

**Architecture:** A SvelteKit frontend (adapter-static) in a window that is created hidden and hides rather than exits on close. Four Tauri commands bridge it to Rust, which keeps sole ownership of validation and of the live hotkey re-registration that v1 deferred.

**Tech Stack:** SvelteKit + Svelte 5 runes, shadcn-svelte, Tailwind v4, mode-watcher, vitest. Rust side unchanged except `config.rs`, a new `commands.rs`, and `lib.rs`.

## Global Constraints

From `docs/superpowers/specs/2026-08-07-clipboard-ai-v2-settings-design.md`. Every task's requirements implicitly include this section.

- **Windows only.** v2 scope: no in-flight cycle feedback, no multiple prompts per command, no secure key storage, no review-before-replace mode.
- Config schema **version 2**. Key `prompts` renamed to `commands`; `PromptConfig` renamed to `CommandConfig`. A v1 file is migrated on load; an unknown future version is **rejected**, and the app keeps running with an empty command set rather than dying.
- **Validation lives in Rust only.** Do not reimplement it in TypeScript.
- The settings window is created **hidden**; closing it **hides** rather than exits. Quit is tray-only.
- **Theme follows the OS.** `mode-watcher` with `<ModeWatcher />`; no in-app toggle.
- Hotkey recording must call `suspend_hotkeys` on entry and `resume_hotkeys` on **every** exit path — commit, cancel, blur, and window close. Leaving hotkeys suspended is the worst failure this feature can produce.
- A hotkey the OS refuses must be surfaced inline, never silently accepted.
- Match `E:\Code\M42Toolkit` conventions: state as singleton classes in `$lib/state/*.svelte.ts`, shadcn components under `$lib/components/shadcn`, base colour zinc, style `lyra`, icon library remixicon.
- The API key stays in `config.json`, masked in the UI with a reveal toggle.

## Environment

**cargo fails at the link step unless MSVC paths are loaded in the same invocation:**

```
cmd /c '"D:\Applications\Work\Visual Studio\18\VC\Auxiliary\Build\vcvars64.bat" >nul && cd /d E:\Code\clipboard-ai\src-tauri && cargo test'
```

The VS path is non-stock. **Never run `npm run tauri dev`** in a subagent — it is a long-running GUI process that will hang the session. Verify with `cargo build`, `cargo test`, `npm run build` and `npm run test`.

Current Rust suite: **71 tests passing.**

## File Structure

| File | Responsibility |
|---|---|
| `src-tauri/src/config.rs` | *(modify)* v2 schema, rename, v1→v2 migration |
| `src-tauri/src/commands.rs` | *(new)* the four `#[tauri::command]`s and `SaveResult` |
| `src-tauri/src/lib.rs` | *(modify)* live-apply state, window lifecycle, command registration |
| `src/routes/+layout.svelte` | Root layout — `ModeWatcher`, app.css |
| `src/routes/+layout.ts` | `export const prerender = true; export const ssr = false;` |
| `src/routes/+page.svelte` | The two tabs |
| `src/lib/types.ts` | TypeScript mirrors of the Rust config types |
| `src/lib/state/config.svelte.ts` | Config state singleton: load, mutate, dirty tracking, save |
| `src/lib/utils/hotkey.ts` | Browser `KeyboardEvent` → Rust shortcut string. Pure, tested. |
| `src/lib/components/BackendList.svelte` | Backend list + delete |
| `src/lib/components/BackendForm.svelte` | Backend add/edit, fields by kind |
| `src/lib/components/CommandList.svelte` | Command list + delete |
| `src/lib/components/CommandForm.svelte` | Command add/edit |
| `src/lib/components/HotkeyRecorder.svelte` | Press-to-record field |
| `test/unit/hotkey.test.ts` | vitest over the mapping |

`hotkey.ts` is deliberately separate from `HotkeyRecorder.svelte`: the mapping is the fiddly part and it is pure, so it gets tested without a DOM.

---

### Task 1: Config schema v2 and migration

**Files:**
- Modify: `src-tauri/src/config.rs`
- Modify: `src-tauri/src/lib.rs` (rename references)

**Interfaces:**
- Consumes: nothing
- Produces:
  - `config::CommandConfig { id, name, backend_id, hotkey, system_prompt }` (was `PromptConfig`)
  - `config::Config { version: u32, backends: Vec<BackendConfig>, commands: Vec<CommandConfig> }`
  - `config::CURRENT_VERSION: u32 = 2`
  - `config::parse(json: &str) -> Result<Config, ConfigError>` — migrates v1, rejects unknown versions
  - `ConfigError::UnsupportedVersion { version: u32 }`

Serde stays camelCase on the wire (`backendId`, `systemPrompt`).

- [ ] **Step 1: Write the failing tests**

Add to `config.rs`'s test module, keeping the existing seven:

```rust
    fn v1_json() -> &'static str {
        r#"{
          "version": 1,
          "backends": [
            {"id":"ollama","name":"Local","kind":"openai-compatible",
             "baseUrl":"http://localhost:11434/v1","model":"m","apiKey":null,"timeoutMs":120000}
          ],
          "prompts": [
            {"id":"fix","name":"Fix grammar","backendId":"ollama",
             "hotkey":"Ctrl+Alt+J","systemPrompt":"Fix grammar."}
          ]
        }"#
    }

    #[test]
    fn migrates_a_v1_file_by_reading_prompts_as_commands() {
        let cfg = parse(v1_json()).expect("v1 should migrate");
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.commands.len(), 1);
        assert_eq!(cfg.commands[0].id, "fix");
        assert_eq!(cfg.commands[0].hotkey, "Ctrl+Alt+J");
    }

    #[test]
    fn a_migrated_v1_config_is_still_validated() {
        let json = v1_json().replace(r#""backendId":"ollama""#, r#""backendId":"nope""#);
        assert!(matches!(parse(&json), Err(ConfigError::UnknownBackend { .. })));
    }

    #[test]
    fn rejects_a_version_from_the_future() {
        let json = valid_json().replace(r#""version": 1"#, r#""version": 99"#);
        assert!(matches!(parse(&json), Err(ConfigError::UnsupportedVersion { version: 99 })));
    }

    #[test]
    fn a_v2_file_round_trips_unchanged() {
        let cfg = parse(&serde_json::to_string(&default_config()).unwrap()).unwrap();
        assert_eq!(cfg.version, CURRENT_VERSION);
        assert_eq!(cfg.commands.len(), 1);
    }
```

Update `valid_json()` to `"version": 2` with a `"commands"` key, and rename `prompts` to `commands` throughout the existing tests. The existing rejection tests must keep failing for the reasons their names claim — verify, do not assume.

- [ ] **Step 2: Run the tests to verify they fail**

Run the vcvars-wrapped `cargo test config`.
Expected: compile errors — `CURRENT_VERSION`, `UnsupportedVersion` and `commands` do not exist.

- [ ] **Step 3: Write the implementation**

In `config.rs`:

```rust
pub const CURRENT_VERSION: u32 = 2;
```

Add to `ConfigError`:

```rust
    #[error("config version {version} is newer than this app understands")]
    UnsupportedVersion { version: u32 },
```

Rename `PromptConfig` to `CommandConfig` and `Config::prompts` to `Config::commands`. Then make `parse` version-aware. Deserialize into an untyped `Value` first so a v1 file's `prompts` key can be read before typing:

```rust
pub fn parse(json: &str) -> Result<Config, ConfigError> {
    let mut value: serde_json::Value = serde_json::from_str(json)?;
    let version = value.get("version").and_then(|v| v.as_u64()).unwrap_or(1) as u32;

    match version {
        1 => {
            // v1 called them prompts. Move the key, then type it as v2.
            if let Some(obj) = value.as_object_mut() {
                if let Some(prompts) = obj.remove("prompts") {
                    obj.insert("commands".to_string(), prompts);
                }
                obj.insert("version".to_string(), serde_json::json!(CURRENT_VERSION));
            }
        }
        v if v == CURRENT_VERSION => {}
        other => return Err(ConfigError::UnsupportedVersion { version: other }),
    }

    let config: Config = serde_json::from_value(value)?;
    validate(&config)?;
    Ok(config)
}
```

Update `default_config()` to emit `version: CURRENT_VERSION` and `commands`. Update `Config::backend` unchanged. Rename every `prompts` reference in `lib.rs` to `commands` — `build_prompt_map` becomes `build_command_map`, and its returned map is unchanged in shape.

`load_or_create` gains one behaviour: after a successful parse, if the file on disk was version 1, rewrite it as v2 so the migration happens once rather than on every start.

```rust
pub fn load_or_create(path: &Path) -> Result<Config, ConfigError> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, serde_json::to_string_pretty(&default_config())?)?;
    }
    let raw = fs::read_to_string(path)?;
    let config = parse(&raw)?;
    if !raw.contains("\"commands\"") {
        // Migrated from v1 — persist it so this happens once.
        fs::write(path, serde_json::to_string_pretty(&config)?)?;
    }
    Ok(config)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Expected: PASS, 11 config tests; whole suite 75.

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/config.rs src-tauri/src/lib.rs
git commit -m "feat: config schema v2 with prompts to commands migration"
```

---

### Task 2: Live apply

**Files:**
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `config::Config`, `backend::build`
- Produces:
  - `AppState.config: Mutex<Config>` — the in-memory source of truth
  - `apply_config(app: &tauri::AppHandle, state: &AppState, config: Config) -> Vec<String>` — unregisters, rebuilds, re-registers, returns per-command problems

v1 registers hotkeys once in `setup` and never again, which makes a settings UI a lie. This extracts that into a function both startup and `save_config` call.

- [ ] **Step 1: Write the implementation**

There are no unit tests for this task: it needs a live `AppHandle` for shortcut registration. Its two testable halves — `build_command_map` and config validation — are already covered. It is verified by hand in Task 10.

In `lib.rs`, extend `AppState`:

```rust
struct AppState {
    config: Mutex<config::Config>,
    commands: Mutex<HashMap<Shortcut, (String, Arc<dyn backend::Backend>)>>,
    ops: Arc<dyn osops::OsOps>,
    busy: AtomicBool,
    status_item: tauri::menu::MenuItem<tauri::Wry>,
    config_dir: PathBuf,
}
```

Then:

```rust
/// Unregister everything, rebuild from `config`, re-register. Returns per-command
/// problems that did not prevent the save (a hotkey the OS refused, a backend
/// that would not build).
///
/// A cycle already running is unaffected: it holds an Arc<dyn Backend> cloned
/// before it started, so replacing the map cannot reach it. A save during a
/// cycle therefore takes effect from the next press.
fn apply_config(
    app: &tauri::AppHandle,
    state: &AppState,
    config: config::Config,
) -> Vec<String> {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;

    let _ = app.global_shortcut().unregister_all();

    let (map, mut problems) = build_command_map(&config);
    let shortcuts: Vec<Shortcut> = map.keys().copied().collect();

    if let Ok(mut guard) = state.commands.lock() {
        *guard = map;
    }
    if let Ok(mut guard) = state.config.lock() {
        *guard = config;
    }

    for shortcut in shortcuts {
        if let Err(e) = app.global_shortcut().register(shortcut) {
            problems.push(format!("{shortcut}: could not register ({e})"));
        }
    }
    problems
}
```

`build_command_map` changes to key by `Shortcut` and to report a parse failure as a problem rather than silently skipping:

```rust
fn build_command_map(
    cfg: &config::Config,
) -> (HashMap<Shortcut, (String, Arc<dyn backend::Backend>)>, Vec<String>) {
    let mut map = HashMap::new();
    let mut problems = Vec::new();
    for command in &cfg.commands {
        let Ok(shortcut) = command.hotkey.parse::<Shortcut>() else {
            problems.push(format!("{}: unparseable hotkey {:?}", command.name, command.hotkey));
            continue;
        };
        let Some(backend_cfg) = cfg.backend(&command.backend_id) else {
            problems.push(format!("{}: unknown backend", command.name));
            continue;
        };
        match backend::build(backend_cfg) {
            Ok(b) => {
                if map.insert(shortcut, (command.system_prompt.clone(), b)).is_some() {
                    problems.push(format!("{}: hotkey collides with another command", command.name));
                }
            }
            Err(e) => problems.push(format!("{}: {e}", command.name)),
        }
    }
    (map, problems)
}
```

Rewrite `setup` to build `AppState` first and then call `apply_config` once, replacing the inline registration. The startup problems it returns feed the existing log and tray status wiring unchanged.

- [ ] **Step 2: Verify it builds and the suite still passes**

Expected: builds clean, 75 tests.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: extract apply_config for live hotkey re-registration"
```

---

### Task 3: Tauri commands

**Files:**
- Create: `src-tauri/src/commands.rs`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `AppState`, `apply_config`, `config`
- Produces:
  - `commands::SaveResult { ok: bool, error: Option<String>, problems: Vec<String> }`
  - `get_config`, `save_config`, `suspend_hotkeys`, `resume_hotkeys` as `#[tauri::command]`

- [ ] **Step 1: Write the implementation**

Create `src-tauri/src/commands.rs`:

```rust
use serde::Serialize;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_global_shortcut::GlobalShortcutExt;

use crate::{apply_config, config, AppState};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveResult {
    pub ok: bool,
    pub error: Option<String>,
    pub problems: Vec<String>,
}

#[tauri::command]
pub fn get_config(state: State<'_, AppState>) -> Result<config::Config, String> {
    state
        .config
        .lock()
        .map(|c| c.clone())
        .map_err(|_| "config lock poisoned".to_string())
}

#[tauri::command]
pub fn save_config(
    app: AppHandle,
    state: State<'_, AppState>,
    config: config::Config,
) -> SaveResult {
    // Validate through the same path the file loader uses, so the UI cannot
    // persist something a restart would then reject.
    let json = match serde_json::to_string_pretty(&config) {
        Ok(j) => j,
        Err(e) => return SaveResult { ok: false, error: Some(e.to_string()), problems: vec![] },
    };
    let validated = match config::parse(&json) {
        Ok(c) => c,
        Err(e) => return SaveResult { ok: false, error: Some(e.to_string()), problems: vec![] },
    };

    let path = config::config_path(&app);
    if let Err(e) = std::fs::write(&path, &json) {
        return SaveResult { ok: false, error: Some(e.to_string()), problems: vec![] };
    }

    let problems = apply_config(&app, state.inner(), validated);
    SaveResult { ok: true, error: None, problems }
}

#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle) -> Result<(), String> {
    app.global_shortcut().unregister_all().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn resume_hotkeys(app: AppHandle, state: State<'_, AppState>) -> Vec<String> {
    let config = match state.config.lock() {
        Ok(c) => c.clone(),
        Err(_) => return vec!["config lock poisoned".into()],
    };
    apply_config(&app, &state, config)
}
```

`Config` and its members need `Clone` and `Serialize` + `Deserialize` — they already derive all three from v1.

In `lib.rs` add `mod commands;` and register the handlers:

```rust
.invoke_handler(tauri::generate_handler![
    commands::get_config,
    commands::save_config,
    commands::suspend_hotkeys,
    commands::resume_hotkeys,
])
```

Note `save_config` serialises then re-parses rather than validating the struct directly. That is deliberate: it exercises the exact path a restart takes, so the UI cannot write a file that would later be rejected.

**On testing this task.** These four need a live `AppHandle` and `State`, so there are no unit tests here — the same boundary as v1's `lib.rs`. Two properties the spec asks for are guaranteed structurally instead, and are worth confirming by reading rather than asserting:

- *A rejected config leaves the in-memory config untouched* — every `return` on a validation or write failure happens before `apply_config` is reached, and `apply_config` is the only thing that mutates state.
- *Validate before persist* — `config::parse` runs before `fs::write`, so a rejected config never reaches disk either.

Say in your report that you checked both by tracing the function, and name any path that violates them.

- [ ] **Step 2: Verify it builds**

Expected: builds clean, 75 tests still pass.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/src/commands.rs src-tauri/src/lib.rs
git commit -m "feat: add Tauri commands for config read, save and hotkey suspension"
```

---

### Task 4: Window lifecycle

**Files:**
- Modify: `src-tauri/tauri.conf.json`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `AppState`
- Produces: a `settings` window, created hidden, shown from the tray, hidden on close

- [ ] **Step 1: Declare the window**

In `tauri.conf.json`, replace `"windows": []`:

```json
"windows": [
  {
    "label": "settings",
    "title": "Clipboard AI",
    "width": 900,
    "height": 640,
    "minWidth": 720,
    "minHeight": 520,
    "visible": false,
    "resizable": true,
    "center": true
  }
]
```

Leave `frontendDist` alone for now — Task 5 changes it when the SvelteKit build exists.

- [ ] **Step 2: Wire showing and hiding**

In `lib.rs`'s `setup`, add a helper and use it from both the tray menu and a left click:

```rust
fn show_settings(app: &tauri::AppHandle) {
    use tauri::Manager;
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
```

Add a `Settings` item as the first actionable entry in the tray menu (after the status line and separator) whose handler calls `show_settings`. Add a left-click handler on the tray icon that does the same — check `TrayIconEvent::Click` with `button: MouseButton::Left` and `button_state: MouseButtonState::Up`.

Then intercept the window's close so it hides instead of exiting:

```rust
if let Some(window) = app.get_webview_window("settings") {
    let handle = app.handle().clone();
    window.on_window_event(move |event| {
        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
            api.prevent_close();
            if let Some(w) = handle.get_webview_window("settings") {
                let _ = w.hide();
            }
            // Defensive: if the user closed the window mid-recording, the app
            // would otherwise be left with no registered shortcuts and no
            // visible reason. Restoring here makes that unrecoverable state
            // impossible to reach by closing the window.
            if let Some(state) = handle.try_state::<AppState>() {
                if let Ok(config) = state.config.lock().map(|c| c.clone()) {
                    let _ = apply_config(&handle, &state, config);
                }
            }
        }
    });
}
```

That defensive re-apply is not belt-and-braces. Hotkey recording suspends the app's shortcuts, and closing the window is the one exit path the frontend cannot reliably intercept.

- [ ] **Step 3: Verify it builds**

Expected: builds clean, 75 tests.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/tauri.conf.json src-tauri/src/lib.rs
git commit -m "feat: add hidden settings window with show-from-tray and hide-on-close"
```

---

### Task 5: SvelteKit scaffold

**Files:**
- Delete: `src/main.js`, `src/styles.css`, `src/index.html`, `src/assets/`
- Create: `svelte.config.js`, `vite.config.js`, `tsconfig.json`, `components.json`, `src/app.html`, `src/app.css`, `src/routes/+layout.svelte`, `src/routes/+layout.ts`, `src/routes/+page.svelte`
- Modify: `package.json`, `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: nothing
- Produces: a building SvelteKit app with shadcn-svelte, Tailwind v4 and mode-watcher

**Mirror `E:\Code\M42Toolkit`.** Read its `svelte.config.js`, `components.json`, `app.css` and `+layout.svelte` and follow the same shape — same aliases (`ui` → `$lib/components/shadcn`), same base colour (zinc), same style (`lyra`), same icon library (remixicon).

- [ ] **Step 1: Install dependencies**

```bash
npm i -D @sveltejs/kit @sveltejs/adapter-static @sveltejs/vite-plugin-svelte svelte svelte-check typescript vite vitest jsdom
npm i @tauri-apps/api mode-watcher tailwindcss @tailwindcss/vite tw-animate-css bits-ui clsx tailwind-merge class-variance-authority remixicon-svelte
```

Verify the resolved versions match M42Toolkit's majors (Svelte 5, Tailwind 4, bits-ui 2). Record them in your report.

- [ ] **Step 2: Configure SvelteKit for Tauri**

`svelte.config.js` — adapter-static with an SPA fallback, because Tauri has no Node server:

```js
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
export default {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({ fallback: "index.html" }),
    alias: { $lib: "./src/lib" },
  },
};
```

`vite.config.js` — fixed port so Tauri's `devUrl` matches, and `strictPort` so it fails loudly rather than silently moving:

```js
import { sveltekit } from "@sveltejs/kit/vite";
import tailwindcss from "@tailwindcss/vite";

export default {
  plugins: [tailwindcss(), sveltekit()],
  clearScreen: false,
  server: { port: 1420, strictPort: true, watch: { ignored: ["**/src-tauri/**"] } },
};
```

`package.json` scripts:

```json
"scripts": {
  "dev": "vite dev",
  "build": "vite build",
  "check": "svelte-kit sync && svelte-check --tsconfig ./tsconfig.json",
  "test": "vitest run",
  "tauri": "tauri"
}
```

`tauri.conf.json` build block:

```json
"build": {
  "frontendDist": "../build",
  "devUrl": "http://localhost:1420",
  "beforeDevCommand": "npm run dev",
  "beforeBuildCommand": "npm run build"
}
```

- [ ] **Step 3: Add shadcn-svelte**

Run the shadcn-svelte init and add the components this plan needs:

```bash
npx shadcn-svelte@latest init
npx shadcn-svelte@latest add button card dialog input label select separator switch table tabs textarea badge alert sonner
```

**Verify the CLI's actual flags and prompts rather than assuming** — it is interactive and its options change between versions. Configure it to match M42Toolkit's `components.json`: components alias `$lib/components`, ui alias `$lib/components/shadcn`, base colour zinc, style `lyra`, icon library remixicon, TypeScript on. If the CLI cannot be driven non-interactively, write `components.json` by hand from M42Toolkit's and add components individually. Record what you did.

- [ ] **Step 4: Root layout and a placeholder page**

`src/routes/+layout.ts`:

```ts
export const prerender = true;
export const ssr = false;
```

`src/routes/+layout.svelte`:

```svelte
<script lang="ts">
	import '../app.css';
	import { ModeWatcher } from 'mode-watcher';
	import type { Snippet } from 'svelte';

	let { children }: { children: Snippet } = $props();
</script>

<ModeWatcher />
<main class="min-h-screen bg-background text-foreground">
	{@render children()}
</main>
```

`src/routes/+page.svelte` — a placeholder for now, replaced in Task 7:

```svelte
<div class="p-8">
	<h1 class="text-2xl font-semibold">Clipboard AI</h1>
</div>
```

Delete the vanilla scaffold files.

- [ ] **Step 5: Verify it builds**

```bash
npm run build && npm run check
```

Expected: a `build/` directory containing `index.html`; `svelte-check` reports no errors.

Then verify the Rust side still builds with the new `frontendDist` — it will fail if `build/` is missing, which is why Step 5 runs the frontend build first.

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "feat: replace vanilla scaffold with SvelteKit, shadcn-svelte and Tailwind v4"
```

---

### Task 6: The hotkey mapping

**Files:**
- Create: `src/lib/utils/hotkey.ts`
- Test: `test/unit/hotkey.test.ts`
- Create: `vitest.config.ts`

**Interfaces:**
- Consumes: nothing
- Produces:
  - `hotkeyFromEvent(event: KeyboardEvent): string | null` — `null` means not a usable combination yet
  - `MODIFIER_KEYS: ReadonlySet<string>`

This is the fiddly part of the feature and it is pure logic, so it is tested without a DOM. The output must match the spelling `tauri-plugin-global-shortcut` accepts — verified in v1 as case-insensitive `Ctrl+Alt+J` form.

- [ ] **Step 1: Write the failing tests**

`vitest.config.ts`:

```ts
import { defineConfig } from 'vitest/config';

export default defineConfig({
	test: { environment: 'node', include: ['test/unit/**/*.test.ts'] },
});
```

`test/unit/hotkey.test.ts`:

```ts
import { describe, expect, it } from 'vitest';
import { hotkeyFromEvent } from '../../src/lib/utils/hotkey';

function ev(init: Partial<KeyboardEvent>): KeyboardEvent {
	return { ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...init } as KeyboardEvent;
}

describe('hotkeyFromEvent', () => {
	it('maps a single modifier plus a letter', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'j', code: 'KeyJ' }))).toBe('Ctrl+J');
	});

	it('normalises modifier order regardless of which was pressed first', () => {
		const a = hotkeyFromEvent(ev({ altKey: true, ctrlKey: true, key: 'j', code: 'KeyJ' }));
		const b = hotkeyFromEvent(ev({ ctrlKey: true, altKey: true, key: 'j', code: 'KeyJ' }));
		expect(a).toBe('Ctrl+Alt+J');
		expect(b).toBe('Ctrl+Alt+J');
	});

	it('uppercases the letter', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'j', code: 'KeyJ' }))).toBe('Ctrl+J');
	});

	it('includes Shift and Super in the fixed order', () => {
		const result = hotkeyFromEvent(
			ev({ ctrlKey: true, altKey: true, shiftKey: true, metaKey: true, key: 'k', code: 'KeyK' })
		);
		expect(result).toBe('Ctrl+Alt+Shift+Super+K');
	});

	it('returns null while only a modifier is held', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'Control', code: 'ControlLeft' }))).toBeNull();
		expect(hotkeyFromEvent(ev({ altKey: true, key: 'Alt', code: 'AltLeft' }))).toBeNull();
		expect(hotkeyFromEvent(ev({ shiftKey: true, key: 'Shift', code: 'ShiftLeft' }))).toBeNull();
		expect(hotkeyFromEvent(ev({ metaKey: true, key: 'Meta', code: 'MetaLeft' }))).toBeNull();
	});

	it('rejects a bare key with no modifier', () => {
		expect(hotkeyFromEvent(ev({ key: 'j', code: 'KeyJ' }))).toBeNull();
	});

	it('maps digits from the code, not the shifted character', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, shiftKey: true, key: '!', code: 'Digit1' })))
			.toBe('Ctrl+Shift+1');
	});

	it('maps function keys', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'F5', code: 'F5' }))).toBe('Ctrl+F5');
	});

	it('returns null for a key it cannot map rather than guessing', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'Unidentified', code: 'Lang1' }))).toBeNull();
	});
});
```

The digit test matters: `event.key` for `Ctrl+Shift+1` is `!`, which the plugin will not accept. Reading `code` avoids the whole shifted-character problem.

The last test matters more: an unmappable key must produce `null`, never a guess. Registering a guessed spelling is how a hotkey silently does nothing.

- [ ] **Step 2: Run the tests to verify they fail**

```bash
npm run test
```
Expected: FAIL — module not found.

- [ ] **Step 3: Write the implementation**

`src/lib/utils/hotkey.ts`:

```ts
/** Keys that are modifiers themselves — pressing one is never a complete combination. */
export const MODIFIER_KEYS: ReadonlySet<string> = new Set([
	'Control',
	'Alt',
	'Shift',
	'Meta',
	'AltGraph'
]);

/**
 * Convert a browser keydown into the shortcut spelling Rust expects, or `null`
 * when the event is not (yet) a usable combination.
 *
 * `null` means "do not register this" — never fall back to a guess. A spelling
 * the plugin does not accept produces a hotkey that silently does nothing.
 */
export function hotkeyFromEvent(event: KeyboardEvent): string | null {
	if (MODIFIER_KEYS.has(event.key)) return null;

	const parts: string[] = [];
	if (event.ctrlKey) parts.push('Ctrl');
	if (event.altKey) parts.push('Alt');
	if (event.shiftKey) parts.push('Shift');
	if (event.metaKey) parts.push('Super');

	// A bare key would swallow that key system-wide.
	if (parts.length === 0) return null;

	const key = keyFromCode(event.code);
	if (key === null) return null;

	parts.push(key);
	return parts.join('+');
}

/**
 * Derive the key name from `event.code` rather than `event.key`.
 *
 * `event.key` is the *produced character*, so Ctrl+Shift+1 reports `!` and a
 * non-US layout reports whatever that layout produces. `code` is the physical
 * key and matches what the OS-level shortcut registration sees.
 */
function keyFromCode(code: string): string | null {
	if (/^Key[A-Z]$/.test(code)) return code.slice(3);
	if (/^Digit[0-9]$/.test(code)) return code.slice(5);
	if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;

	const named: Record<string, string> = {
		Space: 'Space',
		Enter: 'Enter',
		Tab: 'Tab',
		Backspace: 'Backspace',
		Delete: 'Delete',
		Insert: 'Insert',
		Home: 'Home',
		End: 'End',
		PageUp: 'PageUp',
		PageDown: 'PageDown',
		ArrowUp: 'Up',
		ArrowDown: 'Down',
		ArrowLeft: 'Left',
		ArrowRight: 'Right',
		Comma: 'Comma',
		Period: 'Period',
		Slash: 'Slash',
		Semicolon: 'Semicolon',
		Quote: 'Quote',
		BracketLeft: 'BracketLeft',
		BracketRight: 'BracketRight',
		Backslash: 'Backslash',
		Minus: 'Minus',
		Equal: 'Equal',
		Backquote: 'Backquote'
	};
	return named[code] ?? null;
}
```

**Verify the punctuation and named-key spellings against the plugin** before trusting them: read `tauri-plugin-global-shortcut` 2.3.2 / `global-hotkey` 0.8.0 in `~/.cargo/registry` and check which strings its parser accepts. If a spelling in the table above is wrong, fix the table and add a test for it. Report what you checked and anything you corrected.

- [ ] **Step 4: Run the tests to verify they pass**

Expected: PASS, 9 tests.

- [ ] **Step 5: Commit**

```bash
git add src/lib/utils/hotkey.ts test/unit/hotkey.test.ts vitest.config.ts
git commit -m "feat: add browser-to-Rust hotkey mapping"
```

---

### Task 7: Config state and the Backends screen

**Files:**
- Create: `src/lib/types.ts`, `src/lib/state/config.svelte.ts`
- Create: `src/lib/components/BackendList.svelte`, `src/lib/components/BackendForm.svelte`
- Modify: `src/routes/+page.svelte`

**Interfaces:**
- Consumes: the Tauri commands from Task 3
- Produces:
  - `types.ts`: `BackendKind`, `BackendConfig`, `CommandConfig`, `Config`, `SaveResult`
  - `configState` singleton with `config`, `loading`, `dirty`, `problems`, `error`, and methods `load()`, `save()`, `addBackend()`, `updateBackend()`, `deleteBackend(id)`, `commandsUsingBackend(id)`

- [ ] **Step 1: Write the types and state**

`src/lib/types.ts`:

```ts
export type BackendKind = 'openai-compatible' | 'anthropic-api' | 'claude-cli';

export interface BackendConfig {
	id: string;
	name: string;
	kind: BackendKind;
	baseUrl: string | null;
	model: string | null;
	apiKey: string | null;
	timeoutMs: number;
}

export interface CommandConfig {
	id: string;
	name: string;
	backendId: string;
	hotkey: string;
	systemPrompt: string;
}

export interface Config {
	version: number;
	backends: BackendConfig[];
	commands: CommandConfig[];
}

export interface SaveResult {
	ok: boolean;
	error: string | null;
	problems: string[];
}
```

`src/lib/state/config.svelte.ts` — a singleton class, matching M42Toolkit's pattern:

```ts
import { invoke } from '@tauri-apps/api/core';
import type { BackendConfig, CommandConfig, Config, SaveResult } from '$lib/types';

function emptyConfig(): Config {
	return { version: 2, backends: [], commands: [] };
}

class ConfigState {
	config = $state<Config>(emptyConfig());
	loading = $state(true);
	saving = $state(false);
	dirty = $state(false);
	error = $state<string | null>(null);
	problems = $state<string[]>([]);

	async load(): Promise<void> {
		this.loading = true;
		try {
			this.config = await invoke<Config>('get_config');
			this.dirty = false;
			this.error = null;
		} catch (e) {
			this.error = String(e);
		} finally {
			this.loading = false;
		}
	}

	async save(): Promise<boolean> {
		this.saving = true;
		try {
			const result = await invoke<SaveResult>('save_config', { config: this.config });
			this.error = result.error;
			this.problems = result.problems;
			if (result.ok) this.dirty = false;
			return result.ok;
		} catch (e) {
			this.error = String(e);
			return false;
		} finally {
			this.saving = false;
		}
	}

	touch(): void {
		this.dirty = true;
	}

	commandsUsingBackend(id: string): CommandConfig[] {
		return this.config.commands.filter((c) => c.backendId === id);
	}

	addBackend(backend: BackendConfig): void {
		this.config.backends.push(backend);
		this.touch();
	}

	updateBackend(backend: BackendConfig): void {
		const i = this.config.backends.findIndex((b) => b.id === backend.id);
		if (i >= 0) this.config.backends[i] = backend;
		this.touch();
	}

	/** Refuses while any command still references it — the caller shows the reason. */
	deleteBackend(id: string): { ok: boolean; usedBy: string[] } {
		const usedBy = this.commandsUsingBackend(id).map((c) => c.name);
		if (usedBy.length > 0) return { ok: false, usedBy };
		this.config.backends = this.config.backends.filter((b) => b.id !== id);
		this.touch();
		return { ok: true, usedBy: [] };
	}
}

export const configState = new ConfigState();
```

- [ ] **Step 2: Build the Backends screen**

`BackendList.svelte` renders a shadcn `Table` of backends (name, kind, model) with Edit and Delete buttons per row and an Add button above. Delete calls `configState.deleteBackend` and, when refused, shows an `Alert` naming the commands that use it.

`BackendForm.svelte` is a shadcn `Dialog` with a `Select` for kind and fields that switch on it:

- `openai-compatible` — name, base URL, model, optional API key, timeout
- `anthropic-api` — name, model (placeholder `claude-opus-5`), API key, timeout
- `claude-cli` — name, timeout

The API key `Input` uses `type={revealed ? 'text' : 'password'}` with a toggle button.

New backends get an id from `crypto.randomUUID()`.

**Check each shadcn-svelte component's actual props and slot/snippet shape against the installed package** rather than assuming — Svelte 5 snippets changed these APIs, and M42Toolkit's `$lib/components/shadcn` is the reference for how they are used in practice.

- [ ] **Step 3: Verify**

```bash
npm run check && npm run build && npm run test
```
Expected: no type errors, a clean build, 9 tests still passing.

- [ ] **Step 4: Commit**

```bash
git add src/lib src/routes
git commit -m "feat: add config state and the Backends screen"
```

---

### Task 8: The Commands screen and hotkey recorder

**Files:**
- Create: `src/lib/components/HotkeyRecorder.svelte`, `CommandList.svelte`, `CommandForm.svelte`
- Modify: `src/routes/+page.svelte`, `src/lib/state/config.svelte.ts`

**Interfaces:**
- Consumes: `hotkeyFromEvent`, `configState`
- Produces: `addCommand`, `updateCommand`, `deleteCommand(id)` on `configState`; a `HotkeyRecorder` component with `value: string` and `onchange: (v: string) => void`

- [ ] **Step 1: Build the recorder**

`HotkeyRecorder.svelte`:

```svelte
<script lang="ts">
	import { invoke } from '@tauri-apps/api/core';
	import { hotkeyFromEvent } from '$lib/utils/hotkey';

	let { value, onchange }: { value: string; onchange: (v: string) => void } = $props();

	let recording = $state(false);
	let captured = $state<string | null>(null);
	let hint = $state<string | null>(null);

	async function start() {
		if (recording) return;
		recording = true;
		captured = null;
		hint = null;
		// The app's own shortcuts must be off, or recording a combination that is
		// already assigned fires a transform instead of capturing it.
		await invoke('suspend_hotkeys').catch((e) => (hint = String(e)));
	}

	async function stop(commit: boolean) {
		if (!recording) return;
		recording = false;
		if (commit && captured) onchange(captured);
		captured = null;
		// Every exit path must resume. Leaving the app with no shortcuts and no
		// visible reason is this feature's worst failure.
		await invoke('resume_hotkeys').catch((e) => (hint = String(e)));
	}

	function onkeydown(event: KeyboardEvent) {
		if (!recording) return;
		event.preventDefault();
		event.stopPropagation();
		if (event.key === 'Escape') {
			void stop(false);
			return;
		}
		const mapped = hotkeyFromEvent(event);
		if (mapped) {
			captured = mapped;
			hint = null;
			void stop(true);
		} else {
			hint = 'Hold a modifier (Ctrl, Alt, Shift or Win) and press a key.';
		}
	}
</script>

<svelte:window on:keydown={recording ? onkeydown : undefined} />

<button
	type="button"
	class="w-full rounded-md border px-3 py-2 text-left font-mono text-sm"
	class:ring-2={recording}
	onclick={start}
	onblur={() => stop(false)}
>
	{recording ? 'Press a combination…' : value || 'Click to set a hotkey'}
</button>
{#if hint}
	<p class="text-muted-foreground mt-1 text-xs">{hint}</p>
{/if}
```

Note `stop(false)` on blur rather than `stop(true)`: committing a half-captured combination on blur is how a user ends up with a hotkey they did not choose.

- [ ] **Step 2: Build the Commands screen**

`CommandList.svelte` — a `Table` of commands (name, hotkey shown in a `Badge`, backend name), with Edit, Delete and Add. Any per-command entry in `configState.problems` is shown inline on its row, so a hotkey the OS refused is visible next to the command it belongs to rather than in a general error area.

`CommandForm.svelte` — a `Dialog` with name, a `Select` of backends by name, the `HotkeyRecorder`, and a `Textarea` for the prompt. New commands get `crypto.randomUUID()`.

Add the matching `addCommand`, `updateCommand` and `deleteCommand` methods to `configState`, following the backend ones.

- [ ] **Step 3: Assemble the page**

`+page.svelte` — a shadcn `Tabs` with Backends and Commands, a header carrying the Save button, a dirty indicator, and an `Alert` for `configState.error`. Call `configState.load()` in `onMount`. Disable Save while `!dirty || saving`.

- [ ] **Step 4: Verify**

```bash
npm run check && npm run build && npm run test
```

- [ ] **Step 5: Commit**

```bash
git add src/lib src/routes
git commit -m "feat: add the Commands screen and hotkey recorder"
```

---

### Task 9: Unsaved-changes guard

**Files:**
- Modify: `src/routes/+page.svelte`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- Consumes: `configState.dirty`
- Produces: a confirmation before the window hides with pending edits

- [ ] **Step 1: Ask the frontend before hiding**

Keep the Rust close handler from Task 4, but let the frontend veto once. Rust emits, waits briefly, and hides anyway if nothing answers — a frontend that failed to load must never trap the window open.

In `lib.rs`, add a flag the frontend can set, and consult it in `CloseRequested`:

```rust
/// Set by the frontend whenever its dirty state changes. Rust cannot see
/// Svelte state, and asking across the bridge inside a close handler would
/// block the UI thread, so the frontend pushes it instead.
static DIRTY: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn set_dirty(dirty: bool) {
    DIRTY.store(dirty, Ordering::SeqCst);
}
```

Register `commands::set_dirty` alongside the others. Then in the `CloseRequested` handler, before hiding:

```rust
if DIRTY.load(Ordering::SeqCst) {
    // Let the frontend show its confirmation. It replies by calling
    // force_close_settings, which clears the flag and hides.
    let _ = handle.emit("settings://close-requested", ());
    return;
}
```

and add the escape hatch the frontend calls after the user chooses Discard or Save:

```rust
#[tauri::command]
pub fn force_close_settings(app: AppHandle) {
    DIRTY.store(false, Ordering::SeqCst);
    if let Some(window) = app.get_webview_window("settings") {
        let _ = window.hide();
    }
}
```

Keep the defensive `apply_config` from Task 4 on the hide path — it must still run whichever route closes the window, because recording may have left hotkeys suspended.

- [ ] **Step 2: Show the confirmation**

In `+page.svelte`, keep `set_dirty` in sync and handle the event:

```svelte
<script lang="ts">
	import { onMount } from 'svelte';
	import { invoke } from '@tauri-apps/api/core';
	import { listen } from '@tauri-apps/api/event';
	import { configState } from '$lib/state/config.svelte';

	let confirmingClose = $state(false);

	$effect(() => {
		void invoke('set_dirty', { dirty: configState.dirty });
	});

	onMount(() => {
		const unlisten = listen('settings://close-requested', () => {
			confirmingClose = true;
		});
		return () => {
			void unlisten.then((f) => f());
		};
	});

	async function discardAndClose() {
		confirmingClose = false;
		await configState.load(); // drop local edits
		await invoke('force_close_settings');
	}

	async function saveAndClose() {
		if (await configState.save()) {
			confirmingClose = false;
			await invoke('force_close_settings');
		}
	}
</script>
```

Render a shadcn `Dialog` bound to `confirmingClose` with Cancel, Discard and Save. Save must not close when `save()` returns false — the user needs to see why it was rejected.

- [ ] **Step 3: Verify it builds**

Expected: `cargo build` clean, `npm run build` clean, all tests pass.

- [ ] **Step 3: Commit**

```bash
git add -A
git commit -m "feat: warn on closing the settings window with unsaved changes"
```

---

### Task 10: Manual verification

**Files:**
- Create: `docs/superpowers/notes/2026-08-07-v2-verification.md`

This task is the human's.

- [ ] **Step 1: Start**

```powershell
cd E:\Code\clipboard-ai
npm run tauri dev
```

Expect: tray icon, no window. Tray menu now has a Settings item.

- [ ] **Step 2: The migration**

Before starting, confirm `%APPDATA%\com.clipboardai.app\config.json` still says `"version": 1` with a `prompts` key. After starting, confirm it says `"version": 2` with `commands`, that the existing command survived, and that its hotkey still works.

- [ ] **Step 3: Window lifecycle**

Open Settings from the tray, and by left-clicking the tray icon. Close it with the X — the app must stay running with its tray icon. Reopen; it should be the same window, not a second one.

- [ ] **Step 4: Backends CRUD**

Add an OpenAI-compatible backend, edit it, and delete it. Try deleting a backend a command uses — it must refuse and name the command. Check the API key field masks and reveals.

- [ ] **Step 5: Commands and live apply — the point of the release**

Add a command with a new hotkey, save, and **press the hotkey without restarting.** It must work. Then change its prompt, save, and confirm the new prompt takes effect on the next press.

- [ ] **Step 6: Hotkey recording**

Record a hotkey that is already assigned to another command — it must capture rather than firing a transform. Press Escape mid-recording and confirm the old value is kept. Start recording and close the window without finishing; then confirm your **other hotkeys still work** — this is the failure mode that would otherwise be invisible.

- [ ] **Step 7: A hotkey another app owns**

Set a command's hotkey to something Windows or another app already owns (`Ctrl+Alt+Delete` will not register). Save. It must show an inline problem on that command, not fail silently.

- [ ] **Step 8: Theme**

Switch Windows between light and dark while the window is open. The UI must follow without a restart.

- [ ] **Step 9: Write up and commit**

Record what passed, anything that behaved oddly, and the migration result.

---

## After this plan

v2 gives the app a real configuration surface. Still open, in rough priority order: in-flight cycle feedback (designed separately, concepts in `.superpowers/concepts/`), multiple prompts per command, the review-before-replace write-back mode, formatting preservation, and macOS/Linux.
