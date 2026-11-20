import type { en } from './en';

export const it: typeof en = {
	nav: {
		commands: 'Comandi',
		backends: 'Backend',
		history: 'Cronologia',
		settings: 'Impostazioni',
		working: 'In corso…',
		version: (v: string) => `Versione ${v}`,
		quit: 'Esci da Clipboard AI'
	},

	common: {
		cancel: 'Annulla',
		delete: 'Elimina',
		saving: 'Salvataggio…',
		back: 'Indietro'
	},

	app: {
		loading: 'Caricamento…',
		loadErrorTitle: 'Impossibile leggere la configurazione salvata',
		loadErrorPreservedBefore: 'Il file originale è stato conservato invariato in',
		loadErrorPreservedAfter:
			'Correggilo manualmente e riavvia l’app per ripristinare i backend e i prompt esistenti.',
		loadErrorNotPreserved:
			'Clipboard AI non è riuscita a conservare nemmeno il file originale — controlla il registro.',
		loadErrorFallbackNotice:
			'Quella che vedi qui sotto è una configurazione vuota, non le tue impostazioni salvate. Qualsiasi modifica apportata qui viene salvata subito — ma questo crea soltanto un nuovo config.json e non toccherà la copia conservata sopra.',
		errorLoadTitle: 'Impossibile caricare la configurazione',
		errorSaveTitle: 'Impossibile salvare la configurazione',
		problemsTitle: 'Alcune impostazioni non sono state applicate'
	},

	commands: {
		title: 'Comandi',
		groupTitle: 'Seleziona del testo ovunque, poi premi una scorciatoia',
		groupHint: 'Sostituisce la selezione',
		count: (n: number) => `${n} ${n === 1 ? 'configurato' : 'configurati'}`,
		addCommand: 'Aggiungi comando',
		columnName: 'Nome',
		columnHotkey: 'Hotkey',
		columnBackend: 'Backend',
		columnActions: 'Azioni',
		empty: 'Nessun comando configurato.',
		unknownBackend: 'Backend sconosciuto',
		editAria: (name: string) => `Modifica ${name}`,
		deleteAria: (name: string) => `Elimina ${name}`,
		deleteTitle: (name: string) => `Eliminare «${name}»?`,
		deleteDescription:
			'Rimuove definitivamente il comando e il relativo hotkey. Questa azione non può essere annullata.',
		form: {
			addTitle: 'Aggiungi comando',
			editTitle: 'Modifica comando',
			addDescription: 'Configura un nuovo comando.',
			editDescription: 'Aggiorna backend, hotkey e prompt di questo comando.',
			nameLabel: 'Nome',
			namePlaceholder: 'Correggi la grammatica',
			backendLabel: 'Backend',
			backendPlaceholder: 'Seleziona un backend',
			noBackendsHint: 'Ancora nessun backend — aggiungine prima uno nella scheda «Backend».',
			hotkeyLabel: 'Hotkey',
			promptLabel: 'Prompt di sistema',
			promptPlaceholder: 'Correggi grammatica e ortografia. Mantieni il significato.',
			promptHttpHint:
				'Il backend selezionato è un semplice endpoint HTTP. Il prompt non viene inviato con la richiesta.',
			promptSentinelHint:
				'L’app chiede automaticamente al modello di restituire solo il testo trasformato e nient’altro, quindi non serve indicarlo nel prompt.',
			formattingLabel: 'Formattazione',
			formattingOptionAuto: 'Mantieni la formattazione della selezione',
			formattingOptionRich: 'Restituisci sempre testo formattato',
			formattingOptionPlain: 'Solo testo semplice',
			formattingHintAuto:
				'Grassetto, elenchi, link e immagini nella selezione vengono conservati. Una selezione di testo semplice resta semplice.',
			formattingHintRich:
				'Al modello viene chiesto di rispondere con titoli, elenchi ed enfasi anche se la selezione è testo semplice. Per prompt che migliorano la formattazione.',
			formattingHintPlain:
				'Viene inviato e incollato solo il testo. Formattazione e immagini vengono perse.',
			saveError: 'Impossibile salvare il comando',
			saveFailedFallback: 'Salvataggio non riuscito.',
			addSubmit: 'Aggiungi comando',
			saveSubmit: 'Salva modifiche',
			deleteSubmit: 'Elimina comando'
		}
	},

	backends: {
		title: 'Backend',
		groupTitle: 'Dove i comandi inviano il testo',
		groupHint: 'Ogni comando ne usa uno',
		count: (n: number) => `${n} ${n === 1 ? 'configurato' : 'configurati'}`,
		addBackend: 'Aggiungi backend',
		columnName: 'Nome',
		columnKind: 'Tipo',
		columnModel: 'Modello',
		columnActions: 'Azioni',
		empty: 'Nessun backend configurato.',
		editAria: (name: string) => `Modifica ${name}`,
		deleteAria: (name: string) => `Elimina ${name}`,
		deleteTitle: (name: string) => `Eliminare «${name}»?`,
		deleteDescription: 'Rimuove definitivamente il backend. Questa azione non può essere annullata.',
		blockedDeleteTitle: (name: string) => `Impossibile eliminare «${name}»`,
		blockedDeleteDescription: (usedBy: string) =>
			`Ancora utilizzato da: ${usedBy}. Assegna prima a questi comandi un backend diverso, poi eliminalo.`,
		usedBy: (names: string) => `Usato da ${names}`,
		unused: 'Non usato da nessun comando',
		form: {
			addTitle: 'Aggiungi backend',
			editTitle: 'Modifica backend',
			addDescription: 'Configura un nuovo backend AI.',
			editDescription: 'Aggiorna i dettagli di connessione di questo backend.',
			nameLabel: 'Nome',
			namePlaceholder: 'Ollama locale',
			kindLabel: 'Tipo',
			baseUrlLabel: 'URL di base',
			modelLabel: 'Modello',
			modelPlaceholderOpenAiCompatible: 'qwen2.5:32b-instruct',
			modelPlaceholderOther: 'claude-opus-5',
			modelPlaceholderClaudeCli: 'haiku',
			modelHintClaudeCli:
				'Facoltativo. Un modello più piccolo come haiku risponde più rapidamente; vuoto usa il valore predefinito della CLI.',
			effortLabel: 'Effort',
			effortOptionDefault: 'Predefinito',
			effortOptionLow: 'Basso',
			effortOptionMedium: 'Medio',
			effortOptionHigh: 'Alto',
			effortOptionXhigh: 'Molto alto',
			effortOptionMax: 'Massimo',
			effortHint:
				'Un effort più basso risponde più rapidamente ed è sufficiente per testi brevi. Predefinito lascia decidere alla CLI.',
			httpUrlHint:
				'Il testo selezionato viene inviato a questo URL come corpo della richiesta POST. Il prompt del comando non viene inviato.',
			expectResponseLabel: 'Attendi una risposta',
			expectResponseHint:
				'Attivo incolla il corpo della risposta sulla selezione. Disattivo si limita a inviare.',
			apiKeyLabel: 'Chiave API',
			apiKeyOptional: '(opzionale)',
			showApiKey: 'Mostra chiave API',
			hideApiKey: 'Nascondi chiave API',
			timeoutLabel: 'Timeout (ms)',
			timeoutHint: (defaultMs: string) =>
				`Un modello locale a freddo può richiedere fino a circa 16.000 ms alla prima richiesta della giornata — il valore predefinito di ${defaultMs} ms evita un timeout falso in quel caso.`,
			timeoutTooLow: (minMs: number) => `Il timeout deve essere di almeno ${minMs} ms.`,
			saveError: 'Impossibile salvare il backend',
			saveFailedFallback: 'Salvataggio non riuscito.',
			addSubmit: 'Aggiungi backend',
			saveSubmit: 'Salva modifiche',
			deleteSubmit: 'Elimina backend'
		}
	},

	hotkeyRecorder: {
		prompt: 'Premi una combinazione…',
		placeholder: 'Clicca per impostare un hotkey',
		hintModifier: 'Tieni premuto un modificatore (Ctrl, Alt, Maiusc o Win) e premi un tasto.',
		recordingStatus: 'Registrazione hotkey in corso. Premi una combinazione di tasti, oppure Esc per annullare.',
		ariaHotkey: (value: string) => `Hotkey: ${value}. Clicca per modificare.`,
		ariaEmpty: 'Nessun hotkey impostato. Clicca per registrarne uno.'
	},

	history: {
		title: 'Cronologia',
		hint: (limit: number) => `ultime ${limit} esecuzioni`,
		clear: 'Svuota',
		columnTime: 'Ora',
		columnCommand: 'Comando',
		columnBackend: 'Backend',
		columnOutcome: 'Esito',
		columnDuration: 'Durata',
		statusSuccess: 'Riuscito',
		statusFailed: 'Fallito',
		statusAborted: 'Interrotto',
		statusCancelled: 'Annullato',
		instructionTag: 'Istruzione',
		emptyTitle: 'Ancora nessuna esecuzione',
		emptyDescription: 'Attiva un comando con il suo hotkey — il risultato comparirà qui.',
		loadErrorTitle: 'Impossibile caricare la cronologia',
		copyErrorTitle: 'Impossibile copiare negli appunti',
		expandAria: (name: string) => `Espandi i dettagli di ${name}`,
		collapseAria: (name: string) => `Comprimi i dettagli di ${name}`,
		abortedPrefix: 'Interrotto:',
		failedPrefix: 'Fallito:',
		inputLabel: 'Ingresso',
		resultLabel: 'Risultato',
		noInputCaptured: 'nulla è stato acquisito per questa esecuzione',
		noResultPasted: 'nulla è stato incollato per questa esecuzione',
		copy: 'Copia',
		copied: 'Copiato',
		copyResult: 'Copia risultato',
		searchPlaceholder: 'Cerca nella cronologia',
		noMatches: 'Nessuna esecuzione corrisponde alla ricerca.',
		changesLabel: 'Cosa è cambiato',
		durationDetail: (total: string, backend: string) =>
			`${total} in totale, di cui ${backend} in attesa del backend`,
		clearTitle: 'Svuotare la cronologia?',
		clearDescription: (count: number) =>
			`Rimuove definitivamente tutte le ${count} esecuzioni registrate, incluso qualsiasi testo di input e risultato ancora conservato qui per il recupero. Questa azione non può essere annullata.`
	},

	reasons: {
		cancelled: 'Non è stato incollato nulla.',
		focusChanged: 'Hai cambiato finestra prima che arrivasse la risposta.',
		focusRestoreFailed: 'Impossibile tornare alla tua finestra.',
		captureTimeout: 'Nessun testo era selezionato, oppure non è stato possibile copiarlo.',
		modifiersStuck: 'Un tasto modificatore era ancora premuto.',
		modifiersStuckBeforePaste: 'Un tasto modificatore era premuto al momento di incollare.',
		backendError: 'Il backend ha segnalato un errore. I dettagli sono nella cronologia.',
		noSentinel: 'La risposta è arrivata in un formato inatteso.',
		emptyResult: 'La risposta era vuota.',
		clipboardWriteFailed: 'Gli appunti erano occupati, quindi non è stato incollato nulla.',
		pasteFailed: 'Non è stato possibile inviare la combinazione per incollare.',
		noLastResult: 'Esegui prima un comando, poi potrai incollare di nuovo il suo risultato.',
		paletteFocusFailed: 'Impossibile aprire la tavolozza. Riprova con la scorciatoia.',
		unknown: (code: string) => `Interrotto: ${code}.`
	},

	hud: {
		escHint: 'per annullare',
		done: 'Fatto',
		wordsChanged: (n: number) => (n === 1 ? '1 parola modificata' : `${n} parole modificate`),
		noChanges: 'Niente da modificare',
		sent: 'Inviato',
		pastedAgain: 'Incollato di nuovo',
		undoHint: 'per annullare la modifica',
		nothingPasted: 'Non è stato incollato nulla',
		cancelled: 'Annullato',
		nothingToPaste: 'Ancora niente da incollare',
		paletteClosed: 'Tavolozza chiusa'
	},

	palette: {
		placeholder: 'Cerca comandi o scrivi un’istruzione',
		groupCommands: 'Comandi',
		runInstruction: (text: string) => `Esegui «${text}»`,
		instructionHint: 'Istruzione una tantum',
		noBackend: 'Aggiungi un backend IA per eseguire istruzioni',
		noMatches: 'Nessun comando corrisponde. Premi Invio per eseguirlo come istruzione.',
		empty: 'Ancora nessun comando. Scrivi invece un’istruzione.',
		footerRun: 'esegui',
		footerBackend: 'backend',
		footerClose: 'chiudi',
		listLabel: 'Comandi e istruzioni'
	},

	settings: {
		title: 'Impostazioni',
		groupAppearance: 'Aspetto',
		groupStartup: 'Avvio',
		themeLabel: 'Tema',
		themeDescription: 'Si applica a questa finestra. Segue Windows per impostazione predefinita.',
		themeSystem: 'Sistema',
		themeLight: 'Chiaro',
		themeDark: 'Scuro',
		languageLabel: 'Lingua',
		languageDescription: 'Lingua dell’interfaccia per questa finestra.',
		languageSystem: 'Sistema',
		autostartLabel: 'Avvia con Windows',
		autostartDescription: 'Avvia Clipboard AI ridotto a icona nell’area di notifica all’accesso.',
		autostartDevNoteBefore:
			'Rifiutato nelle build di sviluppo — registrerebbe l’eseguibile di debug al posto dell’app installata all’accesso; prova questa funzione con una build installata (',
		autostartDevNoteAfter: ').',
		groupRunning: 'Mentre un comando è in esecuzione',
		hudLabel: 'Stato vicino al cursore',
		hudDescription:
			'Mostra quale comando è in esecuzione, permette di annullare con Esc e spiega perché non è stato incollato nulla.',
		busyCursorLabel: 'Puntatore di attesa',
		busyCursorDescription: 'Mostra il puntatore di attesa mentre il backend lavora.',
		groupShortcuts: 'Scorciatoie disponibili ovunque',
		paletteLabel: 'Tavolozza dei comandi',
		paletteDescription:
			'Scegli un comando per il testo selezionato oppure scrivi un’istruzione una tantum.',
		pasteLastLabel: 'Incolla di nuovo l’ultimo risultato',
		pasteLastDescription:
			'Incolla il risultato più recente al cursore, anche dal menu dell’area di notifica. Disattivato di default perché Office usa Ctrl+Alt+V per Incolla speciale.',
		pasteLastToggle: 'Usa una scorciatoia per incollare di nuovo l’ultimo risultato',
		saveErrorTitle: 'Impossibile salvare questa modifica',
		saveFailedFallback: 'Impossibile salvare la modifica.'
	}
};
