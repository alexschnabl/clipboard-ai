
import { invoke } from '@tauri-apps/api/core';
import type {
	BackendConfig,
	CommandConfig,
	Config,
	ConfigLoadError,
	GetConfigResult,
	HistoryEntry,
	KeyChanges,
	Problem,
	SaveResult,
	Settings,
	Vocabulary
} from '$lib/types';
import { DEFAULT_SETTINGS } from '../types';

function emptyConfig(): Config {
	return {
		version: 2,
		backends: [],
		commands: [],
		settings: { ...DEFAULT_SETTINGS }
	};
}

export interface ConfigError {
	message: string;
	source: 'load' | 'save';
}

export class ConfigState {
	config = $state<Config>(emptyConfig());
	loading = $state(true);
	error = $state<ConfigError | null>(null);
	problems = $state<Problem[]>([]);
	loadError = $state<ConfigLoadError | null>(null);
	revision = $state(0);
	storedKeys = $state<string[]>([]);
	inlineKeys = $state<string[]>([]);
	isDefaultSetup = $state(false);

	async load(): Promise<void> {
		this.loading = true;
		try {
			const result = await invoke<GetConfigResult>('get_config');
			this.config = {
				...result.config,
				settings: { ...DEFAULT_SETTINGS, ...result.config.settings }
			};
			this.loadError = result.loadError;
			this.problems = result.startupProblems ?? [];
			this.storedKeys = result.storedKeys ?? [];
			this.inlineKeys = result.inlineKeys ?? [];
			this.isDefaultSetup = result.isDefaultSetup ?? false;
			this.error = null;
			this.revision++;
		} catch (e) {
			this.error = { message: String(e), source: 'load' };
		} finally {
			this.loading = false;
		}
	}

	private async save(keyChanges?: KeyChanges, removedBackends?: string[]): Promise<boolean> {
		try {
			const result = await invoke<SaveResult>('save_config', {
				config: this.config,
				...(keyChanges ? { keyChanges } : {}),
				...(removedBackends ? { removedBackends } : {})
			});
			this.error = result.error ? { message: result.error, source: 'save' } : null;
			this.problems = result.problems;
			if (result.storedKeys) this.storedKeys = result.storedKeys;
			if (result.inlineKeys) this.inlineKeys = result.inlineKeys;
			if (result.isDefaultSetup !== undefined) this.isDefaultSetup = result.isDefaultSetup;
			return result.ok;
		} catch (e) {
			this.error = { message: String(e), source: 'save' };
			this.problems = [];
			return false;
		}
	}

	private commitChain: Promise<boolean> | null = null;

	private commit(
		mutate: (config: Config) => void,
		keyChanges?: KeyChanges,
		removedBackends?: string[]
	): Promise<boolean> {
		const run = async (): Promise<boolean> => {
			const snapshot = $state.snapshot(this.config) as Config;
			mutate(this.config);
			const ok = await this.save(keyChanges, removedBackends);
			if (!ok) this.config = snapshot;
			return ok;
		};

		const previous = this.commitChain;
		const chained = previous ? previous.then(run, run) : run();
		this.commitChain = chained;
		const clearIfCurrent = (): void => {
			if (this.commitChain === chained) this.commitChain = null;
		};
		void chained.then(clearIfCurrent, clearIfCurrent);

		return chained;
	}

	commandsUsingBackend(id: string): CommandConfig[] {
		return this.config.commands.filter(
			(c) => c.backendId === id || (c.appRules ?? []).some((r) => r.backendId === id)
		);
	}

	hasStoredKey(id: string): boolean {
		return this.storedKeys.includes(id);
	}

	hasInlineKey(id: string): boolean {
		return this.inlineKeys.includes(id);
	}

	private keyChangesFor(id: string, key: string | null | undefined): KeyChanges | undefined {
		return key === undefined ? undefined : { [id]: key };
	}

	addBackend(backend: BackendConfig, apiKey?: string | null): Promise<boolean> {
		return this.commit(
			(config) => config.backends.push({ ...backend, apiKey: null }),
			this.keyChangesFor(backend.id, apiKey)
		);
	}

	updateBackend(backend: BackendConfig, apiKey?: string | null): Promise<boolean> {
		if (!this.config.backends.some((b) => b.id === backend.id)) return Promise.resolve(false);
		return this.commit((config) => {
			const i = config.backends.findIndex((b) => b.id === backend.id);
			if (i >= 0) config.backends[i] = { ...backend, apiKey: null };
		}, this.keyChangesFor(backend.id, apiKey));
	}

	async revealApiKey(id: string): Promise<string | null> {
		return invoke<string | null>('get_api_key', { backendId: id });
	}

	dismissOnboarding(): Promise<boolean> {
		return this.updateSettings({ onboardingDismissed: true });
	}

	async deleteBackend(id: string): Promise<{ ok: boolean; usedBy: string[] }> {
		const usedBy = this.commandsUsingBackend(id).map((c) => c.name);
		if (usedBy.length > 0) return { ok: false, usedBy };
		const ok = await this.commit(
			(config) => {
				config.backends = config.backends.filter((b) => b.id !== id);
			},
			undefined,
			[id]
		);
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

	updateSettings(patch: Partial<Settings>): Promise<boolean> {
		return this.commit((config) => {
			config.settings = { ...config.settings, ...patch };
		});
	}

	get vocabulary(): Vocabulary {
		return this.config.vocabulary ?? { keep: [], replacements: [] };
	}

	updateVocabulary(next: Vocabulary | ((current: Vocabulary) => Vocabulary)): Promise<boolean> {
		return this.commit((config) => {
			const current = config.vocabulary ?? { keep: [], replacements: [] };
			config.vocabulary = typeof next === 'function' ? next($state.snapshot(current) as Vocabulary) : next;
		});
	}

	history = $state<HistoryEntry[]>([]);
	historyLoading = $state(true);
	historyError = $state<string | null>(null);

	async loadHistory(): Promise<void> {
		this.historyLoading = true;
		this.historyError = null;
		try {
			this.history = await invoke<HistoryEntry[]>('get_history');
		} catch (e) {
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
