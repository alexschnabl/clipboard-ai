// lib/i18n/en.ts — canonical UI-text dictionary for Clipboard AI's settings
// window. English is authoritative: every other dictionary (`de.ts`, and
// whatever's registered in registry.ts) is typed against `typeof en`, so a
// missing key, an extra key, or a function with the wrong parameter type in
// one of them is a compile error (verified by temporarily deleting a German
// key and confirming `npm run check` fails — see v3-i18n-report.md and
// v3-i18n-scale-report.md).
//
// Deliberately *not* `as const`: that would narrow every string leaf to its
// literal English value's type (e.g. `"Cancel"` rather than `string`), and
// `de.ts` assigning `"Abbrechen"` to a property typed `"Cancel"` would then
// itself be a type error. Leaves stay plain `string` — or a function
// returning `string`, for the handful of entries that need an interpolated
// value — so every dictionary only has to agree with this one on *shape*,
// not content.
//
// Grouped by the screen/component that owns the text, mirroring
// src/routes/+page.svelte and src/lib/components/*.svelte, plus `history`
// and `settings` for the sections .superpowers/mockups/mockup-a-compact.html
// adds. Error text that originates in Rust (config.rs's `ConfigError`,
// hotkey-registration failures, ...) is deliberately out of scope — see the
// task brief — and stays English/untranslated wherever it surfaces; nothing
// in this dictionary attempts to cover it.

export const en = {
	nav: {
		commands: 'Commands',
		backends: 'Backends',
		history: 'History',
		settings: 'Settings'
	},

	// Shared across more than one dialog verbatim (e.g. both delete
	// confirmations use the same "Cancel"/"Delete" pair) — kept out of
	// `commands`/`backends` so there is exactly one place to translate them.
	common: {
		cancel: 'Cancel',
		delete: 'Delete',
		saving: 'Saving…'
	},

	// src/routes/+page.svelte: page chrome, load-error banner, save/load
	// error banners, and the "problems" banner.
	app: {
		title: 'Clipboard AI',
		subtitle: 'Configure backends and commands.',
		loading: 'Loading…',
		loadErrorTitle: 'Your saved configuration could not be read',
		// The full sentence is split around the `<code>{preservedPath}</code>`
		// element +page.svelte renders inline — before/after halves rather
		// than one string with a placeholder, so the path can stay real
		// markup instead of being interpolated as plain text.
		loadErrorPreservedBefore: 'The original file was kept, unmodified, at',
		loadErrorPreservedAfter:
			'Fix it by hand and restart the app to recover your existing backends and prompts.',
		loadErrorNotPreserved:
			'Clipboard AI could not preserve the original file either — check the log.',
		loadErrorFallbackNotice:
			'What you see below is a blank starting point, not your saved settings. Any change you make here saves immediately — that only creates a fresh config.json and will not touch the preserved copy above.',
		errorLoadTitle: 'Couldn’t load config',
		errorSaveTitle: 'Couldn’t save config',
		problemsTitle: 'Some settings couldn’t be applied'
	},

	// src/lib/components/CommandList.svelte + CommandForm.svelte
	commands: {
		title: 'Commands',
		// Toolbar count next to the title (e.g. "3 configured") -- distinct
		// from `empty`, which replaces the whole table when the count is zero.
		count: (n: number) => `${n} configured`,
		addCommand: 'Add command',
		columnName: 'Name',
		columnHotkey: 'Hotkey',
		columnBackend: 'Backend',
		columnActions: 'Actions',
		empty: 'No commands configured.',
		// CommandList's backend-name lookup falls back to this when a
		// command's `backendId` doesn't match any configured backend -- should
		// only be reachable via a stale/edited config.json, since deleting a
		// backend that's still in use is refused (see `blockedDeleteTitle`).
		unknownBackend: 'Unknown backend',
		editAria: (name: string) => `Edit ${name}`,
		deleteAria: (name: string) => `Delete ${name}`,
		deleteTitle: (name: string) => `Delete “${name}”?`,
		deleteDescription: 'This removes the command and its hotkey permanently. This can’t be undone.',
		form: {
			addTitle: 'Add command',
			editTitle: 'Edit command',
			addDescription: 'Configure a new command.',
			editDescription: 'Update this command’s backend, hotkey and prompt.',
			nameLabel: 'Name',
			namePlaceholder: 'Fix grammar',
			backendLabel: 'Backend',
			backendPlaceholder: 'Select a backend',
			noBackendsHint: 'No backends yet — add one on the Backends tab first.',
			hotkeyLabel: 'Hotkey',
			promptLabel: 'System prompt',
			promptPlaceholder: 'Fix grammar and spelling. Keep the meaning.',
			saveError: 'Couldn’t save command',
			saveFailedFallback: 'Save failed.',
			addSubmit: 'Add command',
			saveSubmit: 'Save changes'
		}
	},

	// src/lib/components/BackendList.svelte + BackendForm.svelte
	backends: {
		title: 'Backends',
		// Toolbar count next to the title (e.g. "3 configured") -- distinct
		// from `empty`, which replaces the whole table when the count is zero.
		count: (n: number) => `${n} configured`,
		addBackend: 'Add backend',
		columnName: 'Name',
		columnKind: 'Kind',
		columnModel: 'Model',
		columnActions: 'Actions',
		empty: 'No backends configured.',
		editAria: (name: string) => `Edit ${name}`,
		deleteAria: (name: string) => `Delete ${name}`,
		deleteTitle: (name: string) => `Delete “${name}”?`,
		deleteDescription: 'This removes the backend permanently. This can’t be undone.',
		blockedDeleteTitle: (name: string) => `Can’t delete “${name}”`,
		blockedDeleteDescription: (usedBy: string) =>
			`Still used by: ${usedBy}. Point those commands at a different backend first, then delete it.`,
		// Mirrors `KIND_LABELS` in src/lib/types.ts (owned by another agent) —
		// kept as a parallel translated copy rather than importing that map,
		// since this file must not depend on anything outside `lib/i18n`.
		kindLabel: {
			openaiCompatible: 'OpenAI-compatible',
			anthropicApi: 'Anthropic API',
			claudeCli: 'Claude CLI'
		},
		form: {
			addTitle: 'Add backend',
			editTitle: 'Edit backend',
			addDescription: 'Configure a new AI backend.',
			editDescription: 'Update this backend’s connection details.',
			nameLabel: 'Name',
			namePlaceholder: 'Local Ollama',
			kindLabel: 'Kind',
			baseUrlLabel: 'Base URL',
			baseUrlPlaceholder: 'http://localhost:11434/v1',
			modelLabel: 'Model',
			modelPlaceholderOpenAiCompatible: 'qwen2.5:32b-instruct',
			modelPlaceholderOther: 'claude-opus-5',
			apiKeyLabel: 'API key',
			apiKeyOptional: '(optional)',
			showApiKey: 'Show API key',
			hideApiKey: 'Hide API key',
			timeoutLabel: 'Timeout (ms)',
			// `defaultMs` is passed in already formatted (the caller's
			// `DEFAULT_TIMEOUT_MS.toLocaleString()`) so this dictionary never
			// has to make its own locale-formatting decision.
			timeoutHint: (defaultMs: string) =>
				`A cold local model can take ~16,000 ms on its first request of the day — the ${defaultMs} ms default avoids a false timeout then.`,
			timeoutTooLow: (minMs: number) => `Timeout must be at least ${minMs} ms.`,
			saveError: 'Couldn’t save backend',
			saveFailedFallback: 'Save failed.',
			addSubmit: 'Add backend',
			saveSubmit: 'Save changes'
		}
	},

	// src/lib/components/HotkeyRecorder.svelte
	hotkeyRecorder: {
		prompt: 'Press a combination…',
		placeholder: 'Click to set a hotkey',
		hintModifier: 'Hold a modifier (Ctrl, Alt, Shift or Win) and press a key.',
		recordingStatus: 'Recording hotkey. Press a key combination, or Escape to cancel.',
		ariaHotkey: (value: string) => `Hotkey: ${value}. Click to change.`,
		ariaEmpty: 'No hotkey set. Click to record one.'
	},

	// New section (mockup-a-compact.html): run history.
	history: {
		title: 'History',
		// `limit` is the retention cap (100 — see `history::MAX_ENTRIES` in
		// src-tauri/src/history.rs), passed in rather than hard-coded so this
		// stays correct if that constant ever changes.
		hint: (limit: number) => `last ${limit} runs`,
		clear: 'Clear',
		columnTime: 'Time',
		columnCommand: 'Command',
		columnBackend: 'Backend',
		columnOutcome: 'Outcome',
		columnDuration: 'Duration',
		statusSuccess: 'Success',
		statusFailed: 'Failed',
		statusAborted: 'Aborted',
		emptyTitle: 'No runs yet',
		emptyDescription: 'Trigger a command with its hotkey and the result will show up here.'
	},

	// New section (mockup-a-compact.html): theme / language / autostart.
	settings: {
		title: 'Settings',
		themeLabel: 'Theme',
		themeDescription: 'Applies to this window. Follows Windows by default.',
		themeSystem: 'System',
		themeLight: 'Light',
		themeDark: 'Dark',
		languageLabel: 'Language',
		languageDescription: 'Interface language for this window.',
		languageSystem: 'System',
		// Language *names* (English shown as "English", French as
		// "Français", ...) are not dictionary entries: they live once each
		// in registry.ts's `nativeName`, not once per *other* language in
		// every dictionary here -- see that file's header comment for why.
		autostartLabel: 'Start with Windows',
		autostartDescription: 'Launch Clipboard AI minimized to the tray on sign-in.'
	}
};
