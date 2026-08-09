export type BackendKind = 'openai-compatible' | 'anthropic-api' | 'claude-cli';

/** Shared between `BackendList.svelte` (table column) and `BackendForm.svelte`
 *  (Select trigger) so a new `BackendKind` can't update one screen's label and
 *  silently leave the other showing `undefined`. */
export const KIND_LABELS: Record<BackendKind, string> = {
	'openai-compatible': 'OpenAI-compatible',
	'anthropic-api': 'Anthropic API',
	'claude-cli': 'Claude CLI'
};

/** Wire values match `config::Effort`'s `#[serde(rename_all = "lowercase")]`
 *  exactly (see config.rs) -- `default` means "pass no `--effort` flag,
 *  let the CLI decide", a real user choice distinct from this field's own
 *  wire/schema default of `low` (see that enum's doc comment for why). */
export type Effort = 'default' | 'low' | 'medium' | 'high' | 'xhigh' | 'max';

export interface BackendConfig {
	id: string;
	name: string;
	kind: BackendKind;
	baseUrl: string | null;
	model: string | null;
	apiKey: string | null;
	timeoutMs: number;
	/** `claude-cli`-only; ignored by the two HTTP kinds. */
	effort: Effort;
	/** `claude-cli`-only; ignored by the two HTTP kinds. */
	sessionPersistence: boolean;
}

export interface CommandConfig {
	id: string;
	name: string;
	backendId: string;
	hotkey: string;
	systemPrompt: string;
}

/** Wire values match `config::Theme`'s `#[serde(rename_all = "lowercase")]`
 *  exactly (see config.rs) -- `system` means "follow the OS", detection of
 *  which is this frontend's job, not something the enum itself encodes. */
export type Theme = 'system' | 'light' | 'dark';

/** Wire values match `config::Language`'s `#[serde(rename_all =
 *  "lowercase")]` exactly -- `system` means "detect the OS locale, fall
 *  back to English", same division of labour as `Theme` above. */
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

/** Mirrors `history::Outcome` (src-tauri/src/history.rs) exactly: internally
 *  tagged on `status`, so a switch/if-chain on `outcome.status` narrows
 *  `reason`/`error` for free with no separate discriminant field to keep in
 *  sync by hand. */
export type HistoryOutcome =
	| { status: 'succeeded' }
	| { status: 'aborted'; reason: string }
	| { status: 'failed'; error: string };

/** Mirrors `history::HistoryEntry` (src-tauri/src/history.rs) field-for-field.
 *  `totalCycleMs`/`inputText`/`resultText` are `| null`, not `?:` --
 *  serde_json serialises a Rust `None` as JSON `null` rather than omitting
 *  the key (none of the three `#[serde(default)]` attributes there carry
 *  `skip_serializing_if`), so every entry always carries all eight keys. */
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
