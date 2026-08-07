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
