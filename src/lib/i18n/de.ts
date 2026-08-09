import type { en } from './en';

export const de: typeof en = {
	nav: {
		commands: 'Befehle',
		backends: 'Backends',
		history: 'Verlauf',
		settings: 'Einstellungen',
		working: 'Arbeitet…'
	},

	common: {
		cancel: 'Abbrechen',
		delete: 'Löschen',
		saving: 'Wird gespeichert…'
	},

	app: {
		loading: 'Wird geladen…',
		loadErrorTitle: 'Ihre gespeicherte Konfiguration konnte nicht gelesen werden',
		loadErrorPreservedBefore: 'Die Originaldatei wurde unverändert beibehalten unter',
		loadErrorPreservedAfter:
			'Beheben Sie sie von Hand und starten Sie die App neu, um Ihre bestehenden Backends und Prompts wiederherzustellen.',
		loadErrorNotPreserved:
			'Clipboard AI konnte die Originaldatei ebenfalls nicht sichern — prüfen Sie das Protokoll.',
		loadErrorFallbackNotice:
			'Was Sie unten sehen, ist ein leerer Ausgangspunkt, nicht Ihre gespeicherten Einstellungen. Jede Änderung, die Sie hier vornehmen, wird sofort gespeichert — dabei wird lediglich eine neue config.json erstellt; die gesicherte Kopie oben bleibt unangetastet.',
		errorLoadTitle: 'Konfiguration konnte nicht geladen werden',
		errorSaveTitle: 'Konfiguration konnte nicht gespeichert werden',
		problemsTitle: 'Einige Einstellungen konnten nicht übernommen werden'
	},

	commands: {
		title: 'Befehle',
		count: (n: number) => `${n} konfiguriert`,
		addCommand: 'Befehl hinzufügen',
		columnName: 'Name',
		columnHotkey: 'Hotkey',
		columnBackend: 'Backend',
		columnActions: 'Aktionen',
		empty: 'Keine Befehle konfiguriert.',
		unknownBackend: 'Unbekanntes Backend',
		editAria: (name: string) => `${name} bearbeiten`,
		deleteAria: (name: string) => `${name} löschen`,
		deleteTitle: (name: string) => `„${name}“ löschen?`,
		deleteDescription:
			'Dies entfernt den Befehl und seinen Hotkey dauerhaft. Diese Aktion kann nicht rückgängig gemacht werden.',
		form: {
			addTitle: 'Befehl hinzufügen',
			editTitle: 'Befehl bearbeiten',
			addDescription: 'Neuen Befehl konfigurieren.',
			editDescription: 'Backend, Hotkey und Prompt dieses Befehls aktualisieren.',
			nameLabel: 'Name',
			namePlaceholder: 'Grammatik korrigieren',
			backendLabel: 'Backend',
			backendPlaceholder: 'Backend auswählen',
			noBackendsHint: 'Noch keine Backends — zuerst eines im Tab „Backends“ hinzufügen.',
			hotkeyLabel: 'Hotkey',
			promptLabel: 'Systemprompt',
			promptPlaceholder: 'Grammatik und Rechtschreibung korrigieren. Bedeutung beibehalten.',
			promptSentinelHint:
				'Die App weist das Modell automatisch an, nur den transformierten Text zurückzugeben und sonst nichts — das müssen Sie im Prompt nicht angeben.',
			saveError: 'Befehl konnte nicht gespeichert werden',
			saveFailedFallback: 'Speichern fehlgeschlagen.',
			addSubmit: 'Befehl hinzufügen',
			saveSubmit: 'Änderungen speichern'
		}
	},

	backends: {
		title: 'Backends',
		count: (n: number) => `${n} konfiguriert`,
		addBackend: 'Backend hinzufügen',
		columnName: 'Name',
		columnKind: 'Typ',
		columnModel: 'Modell',
		columnActions: 'Aktionen',
		empty: 'Keine Backends konfiguriert.',
		editAria: (name: string) => `${name} bearbeiten`,
		deleteAria: (name: string) => `${name} löschen`,
		deleteTitle: (name: string) => `„${name}“ löschen?`,
		deleteDescription:
			'Dies entfernt das Backend dauerhaft. Diese Aktion kann nicht rückgängig gemacht werden.',
		blockedDeleteTitle: (name: string) => `„${name}“ kann nicht gelöscht werden`,
		blockedDeleteDescription: (usedBy: string) =>
			`Wird noch verwendet von: ${usedBy}. Weisen Sie diesen Befehlen zuerst ein anderes Backend zu, bevor Sie es löschen.`,
		form: {
			addTitle: 'Backend hinzufügen',
			editTitle: 'Backend bearbeiten',
			addDescription: 'Neues KI-Backend konfigurieren.',
			editDescription: 'Verbindungsdetails dieses Backends aktualisieren.',
			nameLabel: 'Name',
			namePlaceholder: 'Lokales Ollama',
			kindLabel: 'Typ',
			baseUrlLabel: 'Basis-URL',
			modelLabel: 'Modell',
			modelPlaceholderOpenAiCompatible: 'qwen2.5:32b-instruct',
			modelPlaceholderOther: 'claude-opus-5',
			modelPlaceholderClaudeCli: 'haiku',
			modelHintClaudeCli:
				'Optional. Ein kleineres Modell wie haiku antwortet schneller; leer nutzt die Standardvorgabe der CLI.',
			effortLabel: 'Effort',
			effortOptionDefault: 'Standard',
			effortOptionLow: 'Niedrig',
			effortOptionMedium: 'Mittel',
			effortOptionHigh: 'Hoch',
			effortOptionXhigh: 'Sehr hoch',
			effortOptionMax: 'Maximal',
			effortHint:
				'Ein niedrigerer Effort antwortet schneller und reicht für kurze Texte aus. Standard überlässt die Wahl der CLI.',
			sessionPersistenceLabel: 'Sitzungsverlauf behalten',
			sessionPersistenceHint:
				'Aus hält Fire-and-Forget-Läufe aus dem eigenen Sitzungsverlauf der CLI heraus.',
			apiKeyLabel: 'API-Schlüssel',
			apiKeyOptional: '(optional)',
			showApiKey: 'API-Schlüssel anzeigen',
			hideApiKey: 'API-Schlüssel ausblenden',
			timeoutLabel: 'Timeout (ms)',
			timeoutHint: (defaultMs: string) =>
				`Ein kaltgestartetes lokales Modell kann bei der ersten Anfrage des Tages bis zu ca. 16.000 ms benötigen — der Standardwert von ${defaultMs} ms vermeidet dabei einen falschen Timeout.`,
			timeoutTooLow: (minMs: number) => `Timeout muss mindestens ${minMs} ms betragen.`,
			saveError: 'Backend konnte nicht gespeichert werden',
			saveFailedFallback: 'Speichern fehlgeschlagen.',
			addSubmit: 'Backend hinzufügen',
			saveSubmit: 'Änderungen speichern'
		}
	},

	hotkeyRecorder: {
		prompt: 'Kombination drücken…',
		placeholder: 'Klicken, um einen Hotkey festzulegen',
		hintModifier: 'Halten Sie eine Zusatztaste (Strg, Alt, Umschalt oder Win) gedrückt und drücken Sie eine Taste.',
		recordingStatus:
			'Hotkey wird aufgezeichnet. Drücken Sie die gewünschte Kombination oder Escape zum Abbrechen.',
		ariaHotkey: (value: string) => `Hotkey: ${value}. Zum Ändern klicken.`,
		ariaEmpty: 'Kein Hotkey festgelegt. Zum Aufzeichnen klicken.'
	},

	history: {
		title: 'Verlauf',
		hint: (limit: number) => `letzte ${limit} Ausführungen`,
		clear: 'Leeren',
		columnTime: 'Zeit',
		columnCommand: 'Befehl',
		columnBackend: 'Backend',
		columnOutcome: 'Ergebnis',
		columnDuration: 'Dauer',
		statusSuccess: 'Erfolgreich',
		statusFailed: 'Fehlgeschlagen',
		statusAborted: 'Abgebrochen',
		emptyTitle: 'Noch keine Ausführungen',
		emptyDescription: 'Lösen Sie einen Befehl über seinen Hotkey aus — das Ergebnis erscheint hier.',
		loadErrorTitle: 'Verlauf konnte nicht geladen werden',
		copyErrorTitle: 'Kopieren in die Zwischenablage fehlgeschlagen',
		expandAria: (name: string) => `Details zu ${name} einblenden`,
		collapseAria: (name: string) => `Details zu ${name} ausblenden`,
		abortedPrefix: 'Abgebrochen:',
		failedPrefix: 'Fehlgeschlagen:',
		inputLabel: 'Eingabe',
		resultLabel: 'Ergebnis',
		noInputCaptured: 'für diesen Lauf wurde nichts erfasst',
		noResultPasted: 'für diesen Lauf wurde nichts eingefügt',
		copy: 'Kopieren',
		copied: 'Kopiert',
		copyResult: 'Ergebnis kopieren',
		clearTitle: 'Verlauf leeren?',
		clearDescription: (count: number) =>
			`Löscht dauerhaft alle ${count} erfassten Ausführungen, einschließlich aller hier noch zur Wiederherstellung vorgehaltenen Eingabe- und Ergebnistexte. Diese Aktion kann nicht rückgängig gemacht werden.`
	},

	settings: {
		title: 'Einstellungen',
		themeLabel: 'Design',
		themeDescription: 'Gilt für dieses Fenster. Folgt standardmäßig den Windows-Einstellungen.',
		themeSystem: 'System',
		themeLight: 'Hell',
		themeDark: 'Dunkel',
		languageLabel: 'Sprache',
		languageDescription: 'Sprache der Benutzeroberfläche für dieses Fenster.',
		languageSystem: 'System',
		autostartLabel: 'Mit Windows starten',
		autostartDescription: 'Startet Clipboard AI bei der Anmeldung minimiert im Infobereich.',
		autostartDevNoteBefore:
			'In Entwicklungs-Builds abgelehnt — dies würde die Debug-Ausführungsdatei anstelle der installierten App bei der Anmeldung registrieren; testen Sie dies stattdessen mit einem installierten (',
		autostartDevNoteAfter: ') Build.',
		saveErrorTitle: 'Änderung konnte nicht gespeichert werden',
		saveFailedFallback: 'Die Änderung konnte nicht gespeichert werden.'
	}
};
