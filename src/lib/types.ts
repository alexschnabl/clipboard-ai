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

export type Formatting = 'auto' | 'rich' | 'plain';

export interface AppRule {
	exe: string;
	name: string;
	promptAddition: string;
	backendId: string | null;
	formatting: Formatting | null;
}

export interface CommandConfig {
	id: string;
	name: string;
	backendId: string;
	hotkey: string;
	systemPrompt: string;
	formatting: Formatting;
	appRules?: AppRule[];
}

export interface Replacement {
	from: string;
	to: string;
}

export interface Vocabulary {
	keep: string[];
	replacements: Replacement[];
}

export type Theme = 'system' | 'light' | 'dark';

export type Language = 'system' | 'en' | 'de' | 'fr' | 'es' | 'it';

export interface Settings {
	theme: Theme;
	language: Language;
	autostart: boolean;
	onboardingDismissed?: boolean;
}

export interface Config {
	version: number;
	backends: BackendConfig[];
	commands: CommandConfig[];
	settings: Settings;
	vocabulary?: Vocabulary;
}

export interface Problem {
	commandId: string | null;
	message: string;
}

export interface SaveResult {
	ok: boolean;
	error: string | null;
	problems: Problem[];
	storedKeys?: string[];
	inlineKeys?: string[];
	isDefaultSetup?: boolean;
}

export type KeyChanges = Record<string, string | null>;

export interface ConfigLoadError {
	reason: string;
	preservedPath: string | null;
}

export interface GetConfigResult {
	config: Config;
	loadError: ConfigLoadError | null;
	startupProblems: Problem[];
	storedKeys?: string[];
	inlineKeys?: string[];
	isDefaultSetup?: boolean;
}

export type Health =
	| { status: 'ok'; ms: number }
	| { status: 'cold'; firstMs: number; warmMs: number }
	| { status: 'slow'; ms: number }
	| { status: 'reachable'; ms: number }
	| { status: 'cliReady'; version: string }
	| { status: 'cliSignedOut'; version: string }
	| { status: 'cliUnknownAuth'; version: string }
	| { status: 'cliMissing' }
	| { status: 'keyMissing' }
	| { status: 'keyRejected' }
	| { status: 'modelNotFound' }
	| { status: 'notReachable'; detail: string }
	| { status: 'timeout'; secs: number }
	| { status: 'misconfigured'; detail: string }
	| { status: 'badResponse'; detail: string };

export interface UpdateInfo {
	version: string;
	currentVersion: string;
	notes: string | null;
}

export interface UpdateProgress {
	downloaded: number;
	total: number | null;
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

export interface TryResult {
	text: string;
	html: string | null;
	backendMs: number;
	backendName: string;
	rule: string | null;
	warnings: string[];
}

export interface EditorError {
	reason: string;
	detail: string | null;
}

export interface AppIcon {
	size: number;
	rgba: string;
}

export interface RunningApp {
	exe: string;
	name: string;
	title: string;
	icon: AppIcon | null;
}
