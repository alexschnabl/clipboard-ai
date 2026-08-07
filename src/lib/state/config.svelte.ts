// lib/state/config.svelte.ts — app config state: load/save via Tauri, plus in-memory backend edits

import { invoke } from '@tauri-apps/api/core';
import type { BackendConfig, CommandConfig, Config, SaveResult } from '$lib/types';

function emptyConfig(): Config {
	return { version: 2, backends: [], commands: [] };
}

class ConfigState {
	// ── State ─────────────────────────────────────────────────────────
	config = $state<Config>(emptyConfig());
	loading = $state(true);
	saving = $state(false);
	dirty = $state(false);
	error = $state<string | null>(null);
	problems = $state<string[]>([]);

	// ── Public API ────────────────────────────────────────────────────
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
