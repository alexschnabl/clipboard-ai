import type { en } from './en';

export const it: typeof en = {
	nav: {
		commands: 'Comandi',
		backends: 'Backend',
		history: 'Cronologia',
		settings: 'Impostazioni',
		working: 'In corso…',
		version: (v: string) => `Versione ${v}`,
		needsAttention: 'Richiede attenzione',
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
		problems: {
			hotkeyTaken: 'Un’altra app usa già questa scorciatoia. Scegline un’altra.',
			hotkeyClash: (other: string) => `Stessa scorciatoia di «${other}», si avvia solo quello.`,
			hotkeyInvalid: 'Questa scorciatoia non è riconosciuta. Registrala di nuovo.',
			backendMissing: 'Il suo backend non esiste più. Scegline un altro.'
		},
		onboarding: {
			title: 'Per iniziare',
			intro: 'Clipboard AI ha bisogno di un backend che risponda. Poi seleziona del testo ovunque e premi una scorciatoia.',
			hide: 'Nascondi',
			hideAria: 'Nascondi questa guida per sempre',
			step1Title: 'Configura un backend',
			step1Text:
				'Un modello locale con Ollama, il tuo abbonamento Claude con la CLI claude, oppure una chiave API.',
			checkBackends: 'Verifica i backend',
			openBackends: 'Apri Backend',
			checking: 'Verifica dei backend…',
			works: (name: string) => `${name} funziona.`,
			noneWorking: 'Nessuno dei tuoi backend ha ancora risposto.',
			step2Title: (name: string) => `Prova «${name}»`,
			step2Text: 'Seleziona una frase in qualsiasi app e premi',
			step2NoCommand: 'Aggiungi un comando con una scorciatoia e un prompt.',
			backendSilent: (backend: string) => `Il suo backend, ${backend}, non ha risposto.`,
			useBackend: (name: string) => `Usa ${name}`,
			step2Done: 'Ha funzionato. È tutto pronto.'
		},
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
		check: 'Verifica',
		checkAll: 'Verifica tutti',
		checkAria: (name: string) => `Verifica ${name}`,
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
			apiKeyStoredPlaceholder: 'Salvata in Gestione credenziali di Windows',
			apiKeyHint: 'Conservata in Gestione credenziali di Windows, non in config.json.',
			apiKeyInline:
				'Ancora in config.json, perché Gestione credenziali di Windows l’ha rifiutata. Il salvataggio riprova.',
			apiKeyRemove: 'Rimuovi chiave',
			apiKeyWillBeRemoved: 'La chiave viene rimossa al salvataggio.',
			apiKeyUndo: 'Annulla',
			apiKeyRevealFailed: 'Impossibile leggere la chiave da Gestione credenziali di Windows.',
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

	health: {
		checking: 'Verifica…',
		notChecked: 'Non ancora verificato',
		checkFailed: 'La verifica stessa non è riuscita',
		ok: (time: string) => `Risposta in ${time}`,
		cold: (first: string, warm: string) =>
			`Il modello era freddo. Prima risposta in ${first}, la successiva in ${warm}`,
		slow: (time: string) => `Funziona, ma è lento. ${time}`,
		reachable: 'Raggiungibile. Non gli è stato inviato nulla',
		cliReady: (version: string) => `Accesso effettuato, CLI ${version}`,
		cliSignedOut: 'Accesso non effettuato. Avvia claude in un terminale per accedere',
		cliUnknownAuth: (version: string) => `CLI ${version} trovata, accesso non confermato`,
		cliMissing: 'CLI di Claude non trovata. Installala ed effettua l’accesso',
		keyMissing: 'Nessuna chiave API',
		keyRejected: 'Chiave rifiutata',
		keyUnreadable: 'Impossibile leggere la chiave API',
		modelNotFound: 'Modello o URL non trovati',
		notReachable: 'Non raggiungibile',
		timeout: (secs: number) => `Nessuna risposta entro ${secs} s`,
		needsUrl: 'Nessun URL',
		needsModel: 'Nessun modello',
		misconfigured: 'Configurazione incompleta',
		badResponse: 'Risposta inattesa'
	},

	update: {
		promptTitle: (version: string) => `Clipboard AI ${version} è disponibile`,
		promptText: 'Installala ora, oppure più tardi dalle impostazioni.',
		install: 'Installa e riavvia',
		later: 'Più tardi',
		downloading: 'Download in corso…',
		downloadingPercent: (percent: number) => `Download in corso… ${percent}%`
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

	settings: {
		title: 'Impostazioni',
		groupAppearance: 'Aspetto',
		groupStartup: 'Avvio',
		groupAbout: 'Informazioni',
		versionLabel: (version: string) => `Clipboard AI ${version}`,
		updateDescription: 'Controlla GitHub una volta al giorno. Non si installa nulla finché non fai clic.',
		updateCheck: 'Cerca aggiornamenti',
		updateChecking: 'Verifica…',
		updateUpToDate: 'Hai l’ultima versione.',
		updateAvailable: (version: string) => `La versione ${version} è disponibile.`,
		updateInstallFailed: 'Impossibile installare l’aggiornamento. Riprova più tardi.',
		updateCheckFailed: 'Impossibile cercare aggiornamenti. Riprova più tardi.',
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
		saveErrorTitle: 'Impossibile salvare questa modifica',
		saveFailedFallback: 'Impossibile salvare la modifica.'
	}
};
