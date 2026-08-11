export type BackendKind = 'openai-compatible' | 'anthropic-api' | 'claude-cli' | 'http';

export const KIND_LABELS: Record<BackendKind, string> = {
	'openai-compatible': 'OpenAI-compatible',
	'anthropic-api': 'Anthropic API',
	'claude-cli': 'Claude CLI',
	http: 'HTTP'
};

export type Effort = 'default' | 'low' | 'medium' | 'high' | 'xhigh' | 'max';

export interface BackendConfig {
	id: string;
	name: string;
	kind: BackendKind;
	baseUrl: string | null;
	model: string | null;
	apiKey: string | null;
	timeoutMs: number;
	effort: Effort;
	expectsResponse: boolean;
}

export interface CommandConfig {
	id: string;
	name: string;
	backendId: string;
	hotkey: string;
	systemPrompt: string;
}

export type Theme = 'system' | 'light' | 'dark';

export type Language = 'system' | 'en' | 'de' | 'fr' | 'es' | 'it';

export interface Settings {
	theme: Theme;
	language: Language;
	autostart: boolean;
}

export interface Config {
	version: number;
	backends: BackendConfig[];
	commands: CommandConfig[];
	settings: Settings;
}

export interface Problem {
	commandId: string | null;
	message: string;
}

export interface SaveResult {
	ok: boolean;
	error: string | null;
	problems: Problem[];
}

export interface ConfigLoadError {
	reason: string;
	preservedPath: string | null;
}

export interface GetConfigResult {
	config: Config;
	loadError: ConfigLoadError | null;
	startupProblems: Problem[];
}

export type HistoryOutcome =
	| { status: 'succeeded' }
	| { status: 'aborted'; reason: string }
	| { status: 'failed'; error: string };

export interface HistoryEntry {
	timestampMs: number;
	commandId: string;
	commandName: string;
	backendName: string;
	outcome: HistoryOutcome;
	backendMs: number;
	totalCycleMs: number | null;
	inputText: string | null;
	resultText: string | null;
}
