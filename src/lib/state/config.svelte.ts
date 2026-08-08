// lib/state/config.svelte.ts — app config state: load via Tauri, plus
// in-memory backend/command edits that persist immediately on mutation.

import { invoke } from '@tauri-apps/api/core';
import type {
	BackendConfig,
	CommandConfig,
	Config,
	ConfigLoadError,
	GetConfigResult,
	HistoryEntry,
	Problem,
	SaveResult,
	Settings
} from '$lib/types';

function emptyConfig(): Config {
	return {
		version: 2,
		backends: [],
		commands: [],
		settings: { theme: 'system', language: 'system', autostart: false }
	};
}

/** `error` and its source always moved together as two separate fields --
 *  every write site set both or neither, which is an invariant a plain pair
 *  of `$state` fields can't enforce. One nullable value instead: `source`
 *  lets `+page.svelte` title its Alert correctly instead of always assuming
 *  a save failed -- this is set from both `load()` and `commit()` below, and
 *  a poisoned-mutex or IPC-level rejection on the very first `get_config` is
 *  not a save. */
export interface ConfigError {
	message: string;
	source: 'load' | 'save';
}

export class ConfigState {
	// ── State ─────────────────────────────────────────────────────────
	config = $state<Config>(emptyConfig());
	loading = $state(true);
	saving = $state(false);
	error = $state<ConfigError | null>(null);
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
			// R10: a hotkey that failed to register at launch is otherwise
			// invisible until an unrelated Save re-populates `problems` --
			// see `startupProblems` on `GetConfigResult`. Surfaced through the
			// same `problems` channel `save()` already uses below, so
			// `CommandList`'s existing per-command `commandId` attribution
			// picks it up with no changes there.
			this.problems = result.startupProblems ?? [];
			this.error = null;
			this.revision++;
		} catch (e) {
			this.error = { message: String(e), source: 'load' };
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
			this.error = result.error ? { message: result.error, source: 'save' } : null;
			this.problems = result.problems;
			return result.ok;
		} catch (e) {
			this.error = { message: String(e), source: 'save' };
			this.problems = [];
			return false;
		} finally {
			this.saving = false;
		}
	}

	/** Chained tail of in-flight commits, or `null` once every queued one has
	 *  settled. Two overlapping commits (e.g. SettingsPanel's theme toggle
	 *  immediately followed by its language select, both before the first
	 *  save's IPC round trip resolves) each used to snapshot and roll back
	 *  independently -- if the *earlier* one's save then failed, its
	 *  rollback reassigned `this.config` to a snapshot taken *before* the
	 *  later commit's change too, silently discarding it even if that
	 *  second save had already succeeded and reached disk (review3-frontend
	 *  F1). Routing every commit's snapshot+mutate+save(+rollback) through
	 *  this chain, one at a time in call order, closes that: a commit's
	 *  snapshot is only ever taken once every earlier commit has fully
	 *  settled -- succeeded, or already rolled back -- so its own rollback
	 *  can only ever undo its own change, never a later one still standing. */
	private commitChain: Promise<boolean> | null = null;

	/** The shape every mutation below shares: apply `mutate` to the live
	 *  `$state` proxy (so the list updates immediately), then persist it.
	 *  Rust's `config::validate` (duplicate hotkey, duplicate id, unknown
	 *  backend, a timeout below `MIN_TIMEOUT_MS`) is the authority a
	 *  rejection can come back from, and by the time it does, `mutate` has
	 *  already run -- so a rejected save rolls back to the pre-mutation
	 *  snapshot rather than leaving the list showing a change that never
	 *  reached disk. `$state.snapshot` takes that snapshot as a plain,
	 *  non-proxied clone; mutating the live proxy afterwards can't alias it.
	 *
	 *  Not declared `async` itself -- it needs to tell, synchronously,
	 *  whether another commit is already in flight *before* deciding
	 *  whether to run immediately or queue behind it. An `await` on an
	 *  already-resolved promise still defers to a microtask, which would
	 *  delay `save()`'s synchronous `this.saving = true` by a tick for the
	 *  common (single, non-overlapping) case and break callers that check
	 *  `saving` right after calling a commit method without awaiting it. */
	private commit(mutate: (config: Config) => void): Promise<boolean> {
		const run = async (): Promise<boolean> => {
			const snapshot = $state.snapshot(this.config) as Config;
			mutate(this.config);
			const ok = await this.save();
			if (!ok) this.config = snapshot;
			return ok;
		};

		const previous = this.commitChain;
		const chained = previous ? previous.then(run, run) : run();
		this.commitChain = chained;
		// Fire-and-forget bookkeeping, deliberately not part of the chain
		// returned to the caller: clears the tail pointer once this link
		// settles, but only if nothing queued after it already took over
		// (guards against a stale reference; two-callback `.then` form so
		// this itself never produces an unhandled rejection).
		const clearIfCurrent = (): void => {
			if (this.commitChain === chained) this.commitChain = null;
		};
		void chained.then(clearIfCurrent, clearIfCurrent);

		return chained;
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

	/** Theme, language and autostart all persist through the exact same
	 *  commit-and-rollback path as a backend/command edit -- there is no
	 *  separate settings save button, and no reason for this one corner of
	 *  the config to durably diverge from the rest of it if `save_config`
	 *  rejects the write. Callers (SettingsPanel) pass the *whole* `Settings`
	 *  object rather than one field, spreading their own change over
	 *  `config.settings` first -- matching `updateBackend`/`updateCommand`'s
	 *  shape of "hand over the intended next value", not a partial patch. */
	updateSettings(settings: Settings): Promise<boolean> {
		return this.commit((config) => {
			config.settings = settings;
		});
	}

	// ── History ───────────────────────────────────────────────────────
	// A separate concern from `config` above in every way that matters here:
	// it lives in its own `history.json` (see history.rs's module doc
	// comment for why), it is never mutated through `commit`'s
	// snapshot/rollback machinery -- there is nothing to roll back to, a
	// clear is just as final on a retry as on the first attempt -- and a
	// failure to load or clear it has no bearing on whether `config` above
	// is usable. Kept on this same class only because it is, like `config`,
	// a single piece of state the settings window needs loaded once and
	// shared across whatever reads it -- not because the two are related.
	history = $state<HistoryEntry[]>([]);
	historyLoading = $state(true);
	/** IPC-level failure only (a rejected `invoke`) -- `get_history`/
	 *  `clear_history` have no validated-vs-rejected split the way
	 *  `save_config` does, they either return data or the call itself
	 *  throws. Cleared at the start of every fresh attempt so a stale
	 *  failure from a previous load doesn't linger next to a successful
	 *  retry's data. */
	historyError = $state<string | null>(null);

	async loadHistory(): Promise<void> {
		this.historyLoading = true;
		this.historyError = null;
		try {
			this.history = await invoke<HistoryEntry[]>('get_history');
		} catch (e) {
			// `get_history`/`clear_history` are being wired in parallel with
			// this panel (see the task brief) -- until that lands, `invoke`
			// rejects with a "command not found"-shaped error. Surfacing it
			// through `historyError` rather than letting it throw is what
			// keeps that gap non-fatal for the rest of the settings window.
			this.historyError = String(e);
		} finally {
			this.historyLoading = false;
		}
	}

	async clearHistory(): Promise<boolean> {
		this.historyError = null;
		try {
			await invoke('clear_history');
			this.history = [];
			return true;
		} catch (e) {
			this.historyError = String(e);
			return false;
		}
	}
}

export const configState = new ConfigState();
