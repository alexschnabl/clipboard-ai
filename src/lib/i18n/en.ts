export const en = {
	nav: {
		commands: 'Commands',
		backends: 'Backends',
		history: 'History',
		settings: 'Settings',
		working: 'Working…',
		version: (v: string) => `Version ${v}`,
		quit: 'Quit Clipboard AI'
	},

	common: {
		cancel: 'Cancel',
		delete: 'Delete',
		saving: 'Saving…',
		back: 'Back'
	},

	app: {
		loading: 'Loading…',
		loadErrorTitle: 'Your saved configuration could not be read',
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

	commands: {
		title: 'Commands',
		groupTitle: 'Select text anywhere, then press a hotkey',
		groupHint: 'Replaces the selection',
		count: (n: number) => `${n} configured`,
		addCommand: 'Add command',
		columnName: 'Name',
		columnHotkey: 'Hotkey',
		columnBackend: 'Backend',
		columnActions: 'Actions',
		empty: 'No commands configured.',
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
			promptHttpHint:
				'The selected backend is a plain HTTP endpoint. The prompt is not sent with the request.',
			promptSentinelHint:
				'The app automatically tells the model to return only the transformed text and nothing else, so you don’t need to say that in your prompt.',
			formattingLabel: 'Formatting',
			formattingOptionAuto: 'Keep the selection’s formatting',
			formattingOptionRich: 'Always return formatted text',
			formattingOptionPlain: 'Plain text only',
			formattingHintAuto:
				'Bold, lists, links and images in the selection survive the round trip. A plain-text selection stays plain.',
			formattingHintRich:
				'The model is asked to answer with headings, lists and emphasis even when the selection is plain. Use this for prompts that improve formatting.',
			formattingHintPlain: 'Only the text is sent and pasted. Formatting and images are dropped.',
			saveError: 'Couldn’t save command',
			saveFailedFallback: 'Save failed.',
			addSubmit: 'Add command',
			saveSubmit: 'Save changes',
			deleteSubmit: 'Delete command'
		}
	},

	backends: {
		title: 'Backends',
		groupTitle: 'Where commands send text',
		groupHint: 'Each command uses one',
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
		usedBy: (names: string) => `Used by ${names}`,
		unused: 'Not used by any command',
		form: {
			addTitle: 'Add backend',
			editTitle: 'Edit backend',
			addDescription: 'Configure a new AI backend.',
			editDescription: 'Update this backend’s connection details.',
			nameLabel: 'Name',
			namePlaceholder: 'Local Ollama',
			kindLabel: 'Kind',
			baseUrlLabel: 'Base URL',
			modelLabel: 'Model',
			modelPlaceholderOpenAiCompatible: 'qwen2.5:32b-instruct',
			modelPlaceholderOther: 'claude-opus-5',
			modelPlaceholderClaudeCli: 'haiku',
			modelHintClaudeCli:
				'Optional. A smaller model such as haiku answers faster; blank uses the CLI’s default.',
			effortLabel: 'Effort',
			effortOptionDefault: 'Default',
			effortOptionLow: 'Low',
			effortOptionMedium: 'Medium',
			effortOptionHigh: 'High',
			effortOptionXhigh: 'Very high',
			effortOptionMax: 'Maximum',
			effortHint:
				'Lower effort answers faster and is enough for short text. Default lets the CLI decide.',
			httpUrlHint:
				'The selected text is sent to this URL as the POST body. The command’s prompt is not sent.',
			expectResponseLabel: 'Expect a response',
			expectResponseHint: 'On pastes the response body over the selection. Off just sends.',
			apiKeyLabel: 'API key',
			apiKeyOptional: '(optional)',
			showApiKey: 'Show API key',
			hideApiKey: 'Hide API key',
			timeoutLabel: 'Timeout (ms)',
			timeoutHint: (defaultMs: string) =>
				`A cold local model can take ~16,000 ms on its first request of the day — the ${defaultMs} ms default avoids a false timeout then.`,
			timeoutTooLow: (minMs: number) => `Timeout must be at least ${minMs} ms.`,
			saveError: 'Couldn’t save backend',
			saveFailedFallback: 'Save failed.',
			addSubmit: 'Add backend',
			saveSubmit: 'Save changes',
			deleteSubmit: 'Delete backend'
		}
	},

	hotkeyRecorder: {
		prompt: 'Press a combination…',
		placeholder: 'Click to set a hotkey',
		hintModifier: 'Hold a modifier (Ctrl, Alt, Shift or Win) and press a key.',
		recordingStatus: 'Recording hotkey. Press a key combination, or Escape to cancel.',
		ariaHotkey: (value: string) => `Hotkey: ${value}. Click to change.`,
		ariaEmpty: 'No hotkey set. Click to record one.'
	},

	history: {
		title: 'History',
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
		statusCancelled: 'Cancelled',
		instructionTag: 'Instruction',
		emptyTitle: 'No runs yet',
		emptyDescription: 'Trigger a command with its hotkey and the result will show up here.',
		loadErrorTitle: 'Couldn’t load history',
		copyErrorTitle: 'Couldn’t copy to clipboard',
		expandAria: (name: string) => `Expand details for ${name}`,
		collapseAria: (name: string) => `Collapse details for ${name}`,
		abortedPrefix: 'Aborted:',
		failedPrefix: 'Failed:',
		inputLabel: 'Input',
		resultLabel: 'Result',
		noInputCaptured: 'nothing was captured for this run',
		noResultPasted: 'nothing was pasted for this run',
		copy: 'Copy',
		copied: 'Copied',
		copyResult: 'Copy result',
		searchPlaceholder: 'Search history',
		noMatches: 'No runs match your search.',
		changesLabel: 'What changed',
		durationDetail: (total: string, backend: string) =>
			`${total} in total, ${backend} of it waiting for the backend`,
		clearTitle: 'Clear history?',
		clearDescription: (count: number) =>
			`Permanently deletes all ${count} recorded runs, including any input and result text still held here for recovery. This can’t be undone.`
	},

	reasons: {
		cancelled: 'Nothing was pasted.',
		focusChanged: 'You switched windows before the answer arrived.',
		focusRestoreFailed: 'Could not switch back to your window.',
		captureTimeout: 'No text was selected, or it could not be copied.',
		modifiersStuck: 'A modifier key was still held down.',
		modifiersStuckBeforePaste: 'A modifier key was held down when it was time to paste.',
		backendError: 'The backend reported an error. History has the details.',
		noSentinel: 'The answer came back in an unexpected format.',
		emptyResult: 'The answer was empty.',
		clipboardWriteFailed: 'The clipboard was busy, so nothing could be pasted.',
		pasteFailed: 'The paste keystroke could not be sent.',
		noLastResult: 'Run a command first, then you can paste its result again.',
		paletteFocusFailed: 'The palette could not open. Try the hotkey again.',
		unknown: (code: string) => `Stopped: ${code}.`
	},

	hud: {
		escHint: 'to cancel',
		done: 'Done',
		wordsChanged: (n: number) => (n === 1 ? '1 word changed' : `${n} words changed`),
		noChanges: 'Nothing needed changing',
		sent: 'Sent',
		pastedAgain: 'Pasted again',
		undoHint: 'to undo',
		nothingPasted: 'Nothing pasted',
		cancelled: 'Cancelled',
		nothingToPaste: 'Nothing to paste yet',
		paletteClosed: 'Palette closed'
	},

	palette: {
		placeholder: 'Search commands or type an instruction',
		groupCommands: 'Commands',
		runInstruction: (text: string) => `Run "${text}"`,
		instructionHint: 'One-off instruction',
		noBackend: 'Add an AI backend to run instructions',
		noMatches: 'No command matches. Press Enter to run it as an instruction.',
		empty: 'No commands yet. Type an instruction instead.',
		footerRun: 'run',
		footerBackend: 'backend',
		footerClose: 'close',
		listLabel: 'Commands and instructions'
	},

	settings: {
		title: 'Settings',
		groupAppearance: 'Appearance',
		groupStartup: 'Startup',
		themeLabel: 'Theme',
		themeDescription: 'Applies to this window. Follows Windows by default.',
		themeSystem: 'System',
		themeLight: 'Light',
		themeDark: 'Dark',
		languageLabel: 'Language',
		languageDescription: 'Interface language for this window.',
		languageSystem: 'System',
		autostartLabel: 'Start with Windows',
		autostartDescription: 'Launch Clipboard AI minimized to the tray on sign-in.',
		autostartDevNoteBefore:
			'Refused in development builds — it would register the debug executable to run at login instead of your installed app; test this with an installed (',
		autostartDevNoteAfter: ') build instead.',
		groupRunning: 'While a command runs',
		hudLabel: 'Status near the cursor',
		hudDescription:
			'Shows which command is running, lets you cancel with Esc, and says why nothing was pasted.',
		busyCursorLabel: 'Busy pointer',
		busyCursorDescription: 'Shows the busy pointer while the backend works.',
		groupShortcuts: 'Shortcuts that work everywhere',
		paletteLabel: 'Command palette',
		paletteDescription: 'Pick any command for the selected text, or type a one-off instruction.',
		pasteLastLabel: 'Paste the last result again',
		pasteLastDescription:
			'Pastes the most recent result at the cursor. Also in the tray menu. Off by default because Office uses Ctrl+Alt+V for Paste Special.',
		pasteLastToggle: 'Use a hotkey to paste the last result again',
		saveErrorTitle: 'Couldn’t save that change',
		saveFailedFallback: 'Could not save the change.'
	}
};
