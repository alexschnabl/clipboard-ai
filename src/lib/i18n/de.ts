import type { en } from './en';

export const de: typeof en = {
	nav: {
		commands: 'Befehle',
		backends: 'Backends',
		history: 'Verlauf',
		vocabulary: 'Vokabular',
		settings: 'Einstellungen',
		working: 'Arbeitet…',
		version: (v: string) => `Version ${v}`,
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
		},
		tryIt: {
			title: 'Ausprobieren',
			hint: 'Es wird nichts eingefügt',
			inputLabel: 'Ihr Text',
			inputPlaceholder: 'Tippen oder fügen Sie ein Beispiel ein, so wie Sie Text markieren würden.',
			outputLabel: 'Das würde eingefügt',
			run: 'Ausführen',
			shortcutOr: 'oder',
			stop: 'Stoppen',
			asking: (backend: string) => `${backend} wird gefragt…`,
			idle: 'Führen Sie ein Beispiel aus, um das Ergebnis hier zu sehen.',
			stale: 'Seit diesem Lauf geändert',
			noChanges: 'Keine Änderungen. Ein echter Lauf würde denselben Text wieder einfügen.',
			appLabel: 'Wie in',
			appAnywhere: 'jeder anderen App',
			defaultModel: 'Standardmodell',
			ruleApplied: (app: string) => `Regel für ${app}`,
			sendOnly:
				'Dieses Backend sendet den Text nur und fügt nichts zurück ein, daher gibt es keine Vorschau. Der Tab „Verlauf“ zeigt jeden Lauf.',
			httpNote: 'Ausführen sendet das Beispiel an den Endpunkt, wie ein echter Lauf.',
			errors: {
				timeout:
					'Das Backend hat nicht innerhalb seines Timeouts geantwortet. Ein langsames oder kaltes Modell braucht eventuell ein längeres Timeout.',
				unreachable:
					'Das Backend ist nicht erreichbar. Prüfen Sie, ob es läuft und die Adresse stimmt.',
				misconfigured: 'Das Backend ist nicht vollständig eingerichtet.',
				badResponse: 'Das Backend hat mit einem Fehler geantwortet.',
				noSentinel:
					'Das Modell hat geantwortet, aber nicht in der erwarteten Form. Versuchen Sie es erneut oder vereinfachen Sie den Prompt.',
				emptyResult: 'Das Modell hat nichts zurückgegeben.',
				nothingToPreview: 'Dieses Backend fügt nichts zurück ein, daher gibt es keine Vorschau.',
				unknownBackend: 'Wählen Sie zuerst ein Backend.',
				emptyInput: 'Geben Sie zuerst ein Beispiel ein.',
				notAnAiBackend: 'Dieses Backend kann keine Prompts schreiben. Wählen Sie ein KI-Backend.',
				internal: 'In der App ist etwas schiefgelaufen.'
			}
		},
		writer: {
			openClaude: 'Mit Claude schreiben',
			openOther: 'Mit KI schreiben',
			title: 'Prompt für mich schreiben',
			description:
				'Beschreiben Sie in einfachen Worten, was der Befehl tun soll. Das Ergebnis ersetzt den Prompt, und Sie können es bearbeiten oder rückgängig machen.',
			label: 'Was soll der Befehl tun?',
			placeholder: 'Meine E-Mails freundlicher klingen lassen, aber kurz halten.',
			reviseHint:
				'Ihr aktueller Prompt wird mitgeschickt, Sie können also auch um eine Änderung bitten, etwa „mach ihn kürzer“.',
			backendLabel: 'Geschrieben von',
			submit: 'Prompt schreiben',
			working: 'Wird geschrieben…',
			done: (backend: string) => `Geschrieben von ${backend}.`,
			undo: 'Rückgängig',
			errorTitle: 'Der Prompt konnte nicht geschrieben werden'
		},
		rules: {
			title: 'In bestimmten Apps',
			hint: 'Dort stattdessen verwendet',
			add: 'Regel für eine App hinzufügen',
			adds: (text: string) => `Ergänzt: ${text}`,
			uses: (backend: string) => `Nutzt ${backend}`,
			nothing: 'Ändert noch nichts',
			savedWithCommand: 'Regeln werden mit dem Befehl gespeichert.',
			addTitle: 'Regel für eine App hinzufügen',
			editTitle: (app: string) => `Regel für ${app}`,
			description: 'Wenn Sie diesen Befehl in dieser App ausführen, gelten stattdessen diese Einstellungen.',
			appLabel: 'App',
			search: 'Offene Apps durchsuchen',
			loading: 'Offene Apps werden gesucht…',
			none: 'Keine offenen Apps gefunden. Geben Sie stattdessen den Programmnamen ein.',
			noMatch: 'Keine offene App passt.',
			manual: 'Programmnamen eingeben',
			pickFromList: 'Aus offenen Apps wählen',
			refresh: 'Aktualisieren',
			change: 'Ändern',
			exeLabel: 'Programmdatei',
			exePlaceholder: 'OUTLOOK.EXE',
			exeHint: 'Der Name der .exe, wie ihn der Task-Manager unter „Details“ zeigt.',
			nameLabel: 'Angezeigt als',
			namePlaceholder: 'Outlook',
			promptLabel: 'Zum Prompt hinzufügen',
			promptPlaceholder: 'Bleib förmlich und sprich die Person mit Sie an.',
			promptHint: 'Wird nur in dieser App an den Prompt des Befehls angehängt.',
			backendLabel: 'Backend',
			formattingLabel: 'Formatierung',
			same: 'Wie beim Befehl',
			duplicate: 'Dieser Befehl hat bereits eine Regel für diese App.',
			empty: 'Ändern Sie mindestens eine Sache für diese App.',
			pickApp: 'Wählen Sie zuerst eine App.',
			submitAdd: 'Regel hinzufügen',
			submitSave: 'Regel speichern',
			delete: 'Regel löschen',
			editAria: (app: string) => `Regel für ${app} bearbeiten`
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

	vocabulary: {
		title: 'Vokabular',
		keepTitle: 'Wörter, die jeder Befehl unverändert lässt',
		keepHint: 'Namen, Produkte, Orte',
		keepPlaceholder: 'Wort oder Namen hinzufügen',
		keepAdd: 'Hinzufügen',
		keepRemove: (word: string) => `${word} entfernen`,
		keepEmpty: 'Noch keine Wörter. Fügen Sie Namen hinzu, die das Modell gern „korrigiert“.',
		replaceTitle: 'Immer ersetzen',
		replaceHint: 'Bevor der Text das Modell erreicht',
		replaceFrom: 'Ersetzen',
		replaceTo: 'Durch',
		replaceFromPlaceholder: 'sus tech',
		replaceToPlaceholder: 'Sus-Tech',
		replaceAdd: 'Ersetzung hinzufügen',
		replaceSave: 'Speichern',
		replaceDeleted: 'entfernt',
		replaceEditAria: (from: string) => `Ersetzung für ${from} bearbeiten`,
		replaceRemove: (from: string) => `Ersetzung für ${from} entfernen`,
		note: 'Die Wortliste kommt in die Anweisungen jedes Befehls, denn ein Modell, das einen unbekannten Namen „korrigiert“, ist das häufigste schlechte Ergebnis. Ersetzungen gelten für ganze Wörter in jeder Groß- und Kleinschreibung und ändern den markierten Text, bevor ein Backend ihn sieht.',
		saveErrorTitle: 'Die Änderung konnte nicht gespeichert werden',
		saveFailedFallback: 'Die Änderung konnte nicht gespeichert werden.'
	},

	settings: {
		title: 'Einstellungen',
		groupAppearance: 'Darstellung',
		groupStartup: 'Start',
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
