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

/** Set once at startup if the real config.json failed to load -- see
 *  `get_config` in commands.rs. Its presence means the config the settings
 *  window is showing is the empty startup fallback, not the user's real,
 *  saved configuration -- see `configState.loadError`. */
export interface ConfigLoadError {
	reason: string;
	preservedPath: string | null;
}

export interface GetConfigResult {
	config: Config;
	loadError: ConfigLoadError | null;
}
