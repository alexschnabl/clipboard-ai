export type BackendKind = 'openai-compatible' | 'anthropic-api' | 'claude-cli';

/** Shared between `BackendList.svelte` (table column) and `BackendForm.svelte`
 *  (Select trigger) so a new `BackendKind` can't update one screen's label and
 *  silently leave the other showing `undefined`. */
export const KIND_LABELS: Record<BackendKind, string> = {
	'openai-compatible': 'OpenAI-compatible',
	'anthropic-api': 'Anthropic API',
	'claude-cli': 'Claude CLI'
};

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

/** A problem surfaced by the backend: something in the config didn't apply
 *  cleanly. `commandId` is set whenever it's attributable to one command --
 *  see `CommandList.svelte`'s `problemsFor`, which filters on it directly
 *  instead of matching against `message`'s wording. */
export interface Problem {
	commandId: string | null;
	message: string;
}

export interface SaveResult {
	ok: boolean;
	error: string | null;
	problems: Problem[];
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
	/** `Problem`s produced at startup (a hotkey the OS refused to register, an
	 *  unparseable hotkey, ...) -- see `GetConfigResult` in
	 *  src-tauri/src/commands.rs. Without loading this into `configState`, a
	 *  hotkey that failed to register at launch was invisible in the UI until
	 *  an unrelated Save happened to call `apply_config` again. */
	startupProblems: Problem[];
}
