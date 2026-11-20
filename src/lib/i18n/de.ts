import type { en } from './en';

export const de: typeof en = {
	nav: {
		commands: 'Befehle',
		backends: 'Backends',
		history: 'Verlauf',
		settings: 'Einstellungen',
		working: 'Arbeitet…',
		version: (v: string) => `Version ${v}`,
		needsAttention: 'Braucht Aufmerksamkeit',
		quit: 'Clipboard AI beenden'
	},

	common: {
		cancel: 'Abbrechen',
		delete: 'Löschen',
		saving: 'Wird gespeichert…',
		back: 'Zurück'
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
		groupTitle: 'Text irgendwo markieren, dann Tastenkürzel drücken',
		groupHint: 'Ersetzt die Auswahl',
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
		problems: {
			hotkeyTaken: 'Eine andere App nutzt dieses Tastenkürzel schon. Wähle ein anderes.',
			hotkeyClash: (other: string) => `Gleiches Tastenkürzel wie „${other}“, nur das läuft.`,
			hotkeyInvalid: 'Dieses Tastenkürzel wird nicht erkannt. Nimm es neu auf.',
			backendMissing: 'Sein Backend gibt es nicht mehr. Wähle ein anderes.'
		},
		onboarding: {
			title: 'Erste Schritte',
			intro: 'Clipboard AI braucht ein Backend, das antwortet. Dann markierst du irgendwo Text und drückst ein Tastenkürzel.',
			hide: 'Ausblenden',
			hideAria: 'Diese Anleitung dauerhaft ausblenden',
			step1Title: 'Backend einrichten',
			step1Text:
				'Ein lokales Modell über Ollama, dein Claude-Abo über die claude CLI oder ein API-Schlüssel.',
			checkBackends: 'Backends prüfen',
			openBackends: 'Backends öffnen',
			checking: 'Backends werden geprüft…',
			works: (name: string) => `${name} funktioniert.`,
			noneWorking: 'Noch hat keines deiner Backends geantwortet.',
			step2Title: (name: string) => `„${name}“ ausprobieren`,
			step2Text: 'Markiere einen Satz in einer beliebigen App und drücke',
			step2NoCommand: 'Lege einen Befehl mit Tastenkürzel und Prompt an.',
			backendSilent: (backend: string) => `Sein Backend, ${backend}, hat nicht geantwortet.`,
			useBackend: (name: string) => `${name} verwenden`,
			step2Done: 'Hat geklappt. Alles bereit.'
		},
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
			promptHttpHint:
				'Das gewählte Backend ist ein einfacher HTTP-Endpunkt. Der Prompt wird nicht mitgesendet.',
			promptSentinelHint:
				'Die App weist das Modell automatisch an, nur den transformierten Text zurückzugeben und sonst nichts — das müssen Sie im Prompt nicht angeben.',
			formattingLabel: 'Formatierung',
			formattingOptionAuto: 'Formatierung der Auswahl beibehalten',
			formattingOptionRich: 'Immer formatierten Text zurückgeben',
			formattingOptionPlain: 'Nur reiner Text',
			formattingHintAuto:
				'Fett, Listen, Links und Bilder in der Auswahl bleiben erhalten. Eine reine Textauswahl bleibt reiner Text.',
			formattingHintRich:
				'Das Modell wird gebeten, auch bei reinem Text mit Überschriften, Listen und Hervorhebungen zu antworten. Für Prompts, die die Formatierung verbessern.',
			formattingHintPlain:
				'Nur der Text wird gesendet und eingefügt. Formatierung und Bilder gehen verloren.',
			saveError: 'Befehl konnte nicht gespeichert werden',
			saveFailedFallback: 'Speichern fehlgeschlagen.',
			addSubmit: 'Befehl hinzufügen',
			saveSubmit: 'Änderungen speichern',
			deleteSubmit: 'Befehl löschen'
		}
	},

	backends: {
		title: 'Backends',
		groupTitle: 'Wohin Befehle den Text schicken',
		groupHint: 'Jeder Befehl nutzt eines',
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
		usedBy: (names: string) => `Verwendet von ${names}`,
		unused: 'Von keinem Befehl verwendet',
		check: 'Prüfen',
		checkAll: 'Alle prüfen',
		checkAria: (name: string) => `${name} prüfen`,
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
			httpUrlHint:
				'Der markierte Text wird als POST-Body an diese URL gesendet. Der Prompt des Befehls wird nicht mitgesendet.',
			expectResponseLabel: 'Antwort erwarten',
			expectResponseHint:
				'Aktiviert ersetzt die Auswahl durch den Antwort-Body. Deaktiviert wird nur gesendet.',
			apiKeyLabel: 'API-Schlüssel',
			apiKeyOptional: '(optional)',
			showApiKey: 'API-Schlüssel anzeigen',
			hideApiKey: 'API-Schlüssel ausblenden',
			apiKeyStoredPlaceholder: 'In der Windows-Anmeldeinformationsverwaltung gespeichert',
			apiKeyHint: 'Liegt in der Windows-Anmeldeinformationsverwaltung, nicht in config.json.',
			apiKeyInline:
				'Noch in config.json, weil die Windows-Anmeldeinformationsverwaltung ihn abgelehnt hat. Beim Speichern wird es erneut versucht.',
			apiKeyRemove: 'Schlüssel entfernen',
			apiKeyWillBeRemoved: 'Der Schlüssel wird beim Speichern entfernt.',
			apiKeyUndo: 'Rückgängig',
			apiKeyRevealFailed: 'Der Schlüssel konnte nicht aus der Windows-Anmeldeinformationsverwaltung gelesen werden.',
			timeoutLabel: 'Timeout (ms)',
			timeoutHint: (defaultMs: string) =>
				`Ein kaltgestartetes lokales Modell kann bei der ersten Anfrage des Tages bis zu ca. 16.000 ms benötigen — der Standardwert von ${defaultMs} ms vermeidet dabei einen falschen Timeout.`,
			timeoutTooLow: (minMs: number) => `Timeout muss mindestens ${minMs} ms betragen.`,
			saveError: 'Backend konnte nicht gespeichert werden',
			saveFailedFallback: 'Speichern fehlgeschlagen.',
			addSubmit: 'Backend hinzufügen',
			saveSubmit: 'Änderungen speichern',
			deleteSubmit: 'Backend löschen'
		}
	},

	health: {
		checking: 'Wird geprüft…',
		notChecked: 'Noch nicht geprüft',
		checkFailed: 'Die Prüfung selbst ist fehlgeschlagen',
		ok: (time: string) => `Antwort in ${time}`,
		cold: (first: string, warm: string) =>
			`Modell war kalt. Erste Antwort nach ${first}, die nächste nach ${warm}`,
		slow: (time: string) => `Funktioniert, aber langsam. Dauerte ${time}`,
		reachable: 'Erreichbar. Es wurde nichts gesendet',
		cliReady: (version: string) => `Angemeldet, CLI ${version}`,
		cliSignedOut: 'Nicht angemeldet. Starte claude im Terminal und melde dich an',
		cliUnknownAuth: (version: string) => `CLI ${version} gefunden, Anmeldung nicht bestätigt`,
		cliMissing: 'Claude CLI nicht gefunden. Installiere sie und melde dich an',
		keyMissing: 'Kein API-Schlüssel gesetzt',
		keyRejected: 'Schlüssel abgelehnt',
		keyUnreadable: 'API-Schlüssel konnte nicht gelesen werden',
		modelNotFound: 'Modell oder URL nicht gefunden',
		notReachable: 'Nicht erreichbar',
		timeout: (secs: number) => `Keine Antwort innerhalb von ${secs} s`,
		needsUrl: 'Keine URL gesetzt',
		needsModel: 'Kein Modell gesetzt',
		misconfigured: 'Einrichtung unvollständig',
		badResponse: 'Unerwartete Antwort'
	},

	update: {
		promptTitle: (version: string) => `Clipboard AI ${version} ist verfügbar`,
		promptText: 'Jetzt installieren oder später in den Einstellungen.',
		install: 'Installieren und neu starten',
		later: 'Später',
		downloading: 'Wird geladen…',
		downloadingPercent: (percent: number) => `Wird geladen… ${percent} %`
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
		searchPlaceholder: 'Verlauf durchsuchen',
		noMatches: 'Keine Einträge passen zur Suche.',
		changesLabel: 'Was sich geändert hat',
		durationDetail: (total: string, backend: string) =>
			`${total} insgesamt, davon ${backend} Warten auf das Backend`,
		clearTitle: 'Verlauf leeren?',
		clearDescription: (count: number) =>
			`Löscht dauerhaft alle ${count} erfassten Ausführungen, einschließlich aller hier noch zur Wiederherstellung vorgehaltenen Eingabe- und Ergebnistexte. Diese Aktion kann nicht rückgängig gemacht werden.`
	},

	settings: {
		title: 'Einstellungen',
		groupAppearance: 'Darstellung',
		groupStartup: 'Start',
		groupAbout: 'Über',
		versionLabel: (version: string) => `Clipboard AI ${version}`,
		updateDescription: 'Prüft einmal am Tag auf GitHub. Installiert wird erst, wenn du klickst.',
		updateCheck: 'Nach Updates suchen',
		updateChecking: 'Wird geprüft…',
		updateUpToDate: 'Du hast die neueste Version.',
		updateAvailable: (version: string) => `Version ${version} ist verfügbar.`,
		updateInstallFailed: 'Das Update konnte nicht installiert werden. Versuch es später noch einmal.',
		updateCheckFailed: 'Suche nach Updates fehlgeschlagen. Versuch es später noch einmal.',
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
