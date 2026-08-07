// lib/state/config.svelte.ts — app config state: load/save via Tauri, plus in-memory backend edits

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
	dirty = $state(false);
	error = $state<string | null>(null);
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
			this.dirty = false;
			this.error = null;
			this.revision++;
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
			this.problems = [];
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

	addCommand(command: CommandConfig): void {
		this.config.commands.push(command);
		this.touch();
	}

	updateCommand(command: CommandConfig): void {
		const i = this.config.commands.findIndex((c) => c.id === command.id);
		if (i < 0) return;
		this.config.commands[i] = command;
		this.touch();
	}

	deleteCommand(id: string): void {
		this.config.commands = this.config.commands.filter((c) => c.id !== id);
		this.touch();
	}
}

export const configState = new ConfigState();
