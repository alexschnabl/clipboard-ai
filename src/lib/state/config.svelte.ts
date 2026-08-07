// lib/state/config.svelte.ts — app config state: load via Tauri, plus
// in-memory backend/command edits that persist immediately on mutation.

import { invoke } from '@tauri-apps/api/core';
import type {
	BackendConfig,
	CommandConfig,
	Config,
	ConfigLoadError,
	GetConfigResult,
	Problem,
	SaveResult
} from '$lib/types';

function emptyConfig(): Config {
	return { version: 2, backends: [], commands: [] };
}

export class ConfigState {
	// ── State ─────────────────────────────────────────────────────────
	config = $state<Config>(emptyConfig());
	loading = $state(true);
	saving = $state(false);
	error = $state<string | null>(null);
	/** Which operation produced `error` above, so `+page.svelte` can title its
	 *  Alert correctly instead of always assuming a save failed -- `error` is
	 *  set from both `load()` and `commit()` below, and a poisoned-mutex or
	 *  IPC-level rejection on the very first `get_config` is not a save. */
	errorSource = $state<'load' | 'save' | null>(null);
	problems = $state<Problem[]>([]);
	/** Set by `load()` whenever the backend's real config.json failed to
	 *  read at startup. Non-null means `config` above is the empty startup
	 *  fallback, not the user's actual saved configuration -- the settings
	 *  window must say so loudly rather than look like a fresh install
	 *  (see review finding C2). Saving is still allowed while this is set:
	 *  the original file has already been preserved on disk by the
	 *  backend, so a fresh save here cannot lose anything. */
	loadError = $state<ConfigLoadError | null>(null);
	/** Bumped whenever the settings window is hidden (the Rust-emitted
	 *  `settings://window-hidden` event -- see `+page.svelte`) and on every
	 *  `load()`. The window is only ever hidden, never destroyed, so a
	 *  `HotkeyRecorder` mid-recording or an open `BackendForm`/`CommandForm`
	 *  dialog would otherwise survive a hide/reshow cycle with stale state
	 *  (review findings: hotkey-lifecycle #1, components C1). `+page.svelte`
	 *  keys the tab bodies on this counter so a change remounts them --
	 *  dropping any open dialog, re-seeding every form, and unmounting the
	 *  recorder so its `onDestroy` resumes hotkeys. */
	revision = $state(0);

	// ── Public API ────────────────────────────────────────────────────
	async load(): Promise<void> {
		this.loading = true;
		try {
			const result = await invoke<GetConfigResult>('get_config');
			this.config = result.config;
			this.loadError = result.loadError;
			this.error = null;
			this.errorSource = null;
			this.revision++;
		} catch (e) {
			this.error = String(e);
			this.errorSource = 'load';
		} finally {
			this.loading = false;
		}
	}

	/** Writes `this.config` to disk via `save_config`. Private: every caller
	 *  outside this file goes through `commit` below instead -- there is no
	 *  page-level Save button anymore, every edit persists on its own. */
	private async save(): Promise<boolean> {
		this.saving = true;
		try {
			const result = await invoke<SaveResult>('save_config', { config: this.config });
			this.error = result.error;
			this.errorSource = result.error ? 'save' : null;
			this.problems = result.problems;
			return result.ok;
		} catch (e) {
			this.error = String(e);
			this.errorSource = 'save';
			this.problems = [];
			return false;
		} finally {
			this.saving = false;
		}
	}

	/** The shape every mutation below shares: apply `mutate` to the live
	 *  `$state` proxy (so the list updates immediately), then persist it.
	 *  Rust's `config::validate` (duplicate hotkey, duplicate id, unknown
	 *  backend, a timeout below `MIN_TIMEOUT_MS`) is the authority a
	 *  rejection can come back from, and by the time it does, `mutate` has
	 *  already run -- so a rejected save rolls back to the pre-mutation
	 *  snapshot rather than leaving the list showing a change that never
	 *  reached disk. `$state.snapshot` takes that snapshot as a plain,
	 *  non-proxied clone; mutating the live proxy afterwards can't alias it.
	 */
	private async commit(mutate: (config: Config) => void): Promise<boolean> {
		const snapshot = $state.snapshot(this.config) as Config;
		mutate(this.config);
		const ok = await this.save();
		if (!ok) this.config = snapshot;
		return ok;
	}

	commandsUsingBackend(id: string): CommandConfig[] {
		return this.config.commands.filter((c) => c.backendId === id);
	}

	addBackend(backend: BackendConfig): Promise<boolean> {
		return this.commit((config) => config.backends.push(backend));
	}

	updateBackend(backend: BackendConfig): Promise<boolean> {
		return this.commit((config) => {
			const i = config.backends.findIndex((b) => b.id === backend.id);
			if (i >= 0) config.backends[i] = backend;
		});
	}

	/** Refuses while any command still references it -- the caller shows the
	 *  reason. That check runs *before* `commit`, not inside it: it's a
	 *  local, always-cheap guard, and routing it through a doomed round trip
	 *  to Rust just so `config::validate` rejects it too would turn a plain
	 *  refusal into a failed save. */
	async deleteBackend(id: string): Promise<{ ok: boolean; usedBy: string[] }> {
		const usedBy = this.commandsUsingBackend(id).map((c) => c.name);
		if (usedBy.length > 0) return { ok: false, usedBy };
		const ok = await this.commit((config) => {
			config.backends = config.backends.filter((b) => b.id !== id);
		});
		return { ok, usedBy: [] };
	}

	addCommand(command: CommandConfig): Promise<boolean> {
		return this.commit((config) => config.commands.push(command));
	}

	updateCommand(command: CommandConfig): Promise<boolean> {
		if (!this.config.commands.some((c) => c.id === command.id)) return Promise.resolve(false);
		return this.commit((config) => {
			const i = config.commands.findIndex((c) => c.id === command.id);
			if (i >= 0) config.commands[i] = command;
		});
	}

	deleteCommand(id: string): Promise<boolean> {
		return this.commit((config) => {
			config.commands = config.commands.filter((c) => c.id !== id);
		});
	}
}

export const configState = new ConfigState();
