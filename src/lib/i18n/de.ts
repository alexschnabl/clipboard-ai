// lib/i18n/de.ts — German UI text. Typed against `typeof en` (not
// independently declared): TypeScript rejects this file at compile time if
// a key is missing, a key exists here that `en.ts` doesn't have, or a
// function's parameter types don't match — that check is the entire reason
// this project chose a hand-rolled dictionary over a message-file toolchain.
// Do not add `as const` or a standalone type annotation other than the one
// below; either would weaken or break that guarantee.
//
// Terminology, kept consistent everywhere in this file (not swapped for a
// synonym anywhere, per the task brief):
//   - "command"  -> Befehl / Befehle
//   - "backend"  -> Backend (kept as the English loanword — there is no
//     shorter native alternative in common German technical use, and every
//     backend *kind* label is already an English technical term too).
//   - "hotkey"   -> Hotkey (kept as a loanword rather than "Tastenkombination"
//     specifically so the Commands table's ~108px hotkey column and the
//     Backends table's ~90px timeout-style columns in
//     .superpowers/mockups/mockup-a-compact.html don't overflow — see
//     v3-i18n-report.md for the full overflow note).
//
// Register: formal-neutral, imperative for buttons ("Speichern",
// "Abbrechen", "Hinzufügen"), matching typical Windows desktop software.
// Quotation marks use German typography ( „…“ ), not the English “…” pair
// en.ts uses.

import type { en } from './en';

export const de: typeof en = {
	nav: {
		commands: 'Befehle',
		backends: 'Backends',
		history: 'Verlauf',
		settings: 'Einstellungen'
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
