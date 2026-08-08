// lib/i18n/it.ts — Italian UI text. Typed against `typeof en` (not
// independently declared): TypeScript rejects this file at compile time if
// a key is missing, a key exists here that `en.ts` doesn't have, or a
// function's parameter types don't match — that check is the entire reason
// this project chose a hand-rolled dictionary over a message-file toolchain.
// Do not add `as const` or a standalone type annotation other than the one
// below; either would weaken or break that guarantee.
//
// Terminology, kept consistent everywhere in this file (not swapped for a
// synonym anywhere, per the task brief):
//   - "command"  -> comando / comandi (the saved hotkey+prompt pair; never
//     confused with a shell command, which never appears in this UI).
//   - "backend"  -> backend (kept as the English loanword, invariant for
//     plural like all such loanwords in Italian — "i backend" — there is no
//     shorter native alternative in common Italian technical use).
//   - "hotkey"   -> hotkey (kept as a loanword rather than "combinazione di
//     tasti" or "scorciatoia", specifically so the Commands table's narrow
//     hotkey column doesn't overflow; "hotkey" is also standard in Italian
//     technical software, not a stretch loanword).
//   - "prompt"   -> prompt (kept as the loanword — the term Italian AI
//     tooling actually uses; "istruzione"/"richiesta" would be unusual).
//   - "timeout"  -> timeout (kept as the loanword — standard, unremarkable
//     Italian IT usage; "tempo massimo" appears nowhere in real software).
//
// Register: informal-neutral. Buttons and menu-style actions use the short
// tu-imperative ("Salva", "Annulla", "Elimina", "Aggiungi", "Mostra",
// "Nascondi") — this is the form Italian Windows dialogs and menus actually
// use (File > Salva, Modifica > Elimina, ...), not the longer infinitive
// ("Salvare"). Short instructional captions (e.g. form addDescription/
// editDescription, hints) use the same imperative register for consistency.
// Sentences that state a consequence rather than an instruction
// (delete-confirmation bodies) stay declarative instead, mirroring how
// en.ts phrases those same sentences with an explicit subject ("This
// removes…").
//
// Dynamic values are quoted with Italian guillemets («…»), the quotation
// mark real Microsoft Italian localizations use for interpolated names —
// not the English “…” pair. Confirmation-dialog titles that ask a yes/no
// question ("Delete “X”?") use the infinitive-as-question form standard in
// Italian Windows dialogs ("Eliminare «X»?", cf. "Uscire senza salvare?"),
// which is distinct from — and not in conflict with — the button
// imperative above.
//
// statusFailed vs statusAborted (history): "Fallito" (failed) is reserved
// for a genuine error. "Interrotto" (aborted) deliberately reads as a
// neutral, controlled stop — never as an error or a crash — because an
// aborted run is the app correctly declining to paste, not a malfunction.

import type { en } from './en';

export const it: typeof en = {
	nav: {
		commands: 'Comandi',
		backends: 'Backend',
		history: 'Cronologia',
		settings: 'Impostazioni'
	},

	common: {
		cancel: 'Annulla',
		delete: 'Elimina',
		saving: 'Salvataggio…'
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
			saveError: 'Impossibile salvare il comando',
			saveFailedFallback: 'Salvataggio non riuscito.',
			addSubmit: 'Aggiungi comando',
			saveSubmit: 'Salva modifiche'
		}
	},

	backends: {
		title: 'Backend',
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
			apiKeyLabel: 'Chiave API',
			apiKeyOptional: '(opzionale)',
			showApiKey: 'Mostra chiave API',
			hideApiKey: 'Nascondi chiave API',
			timeoutLabel: 'Timeout (ms)',
			// `defaultMs` is passed in already formatted (the caller's
			// `DEFAULT_TIMEOUT_MS.toLocaleString()`) so this dictionary never
			// has to make its own locale-formatting decision.
			timeoutHint: (defaultMs: string) =>
				`Un modello locale a freddo può richiedere fino a circa 16.000 ms alla prima richiesta della giornata — il valore predefinito di ${defaultMs} ms evita un timeout falso in quel caso.`,
			timeoutTooLow: (minMs: number) => `Il timeout deve essere di almeno ${minMs} ms.`,
			saveError: 'Impossibile salvare il backend',
			saveFailedFallback: 'Salvataggio non riuscito.',
			addSubmit: 'Aggiungi backend',
			saveSubmit: 'Salva modifiche'
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

	// New section (mockup-a-compact.html): run history.
	history: {
		title: 'Cronologia',
		// `limit` is the retention cap (100 — see `history::MAX_ENTRIES` in
		// src-tauri/src/history.rs), passed in rather than hard-coded so this
		// stays correct if that constant ever changes.
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
		// Short tu-imperative, matching this file's button register
		// ("Salva", "Annulla", "Elimina", ...).
		copy: 'Copia',
		copied: 'Copiato',
		copyResult: 'Copia risultato',
		// Infinitive-as-question, same form the header comment documents for
		// `deleteTitle` ("Eliminare «X»?").
		clearTitle: 'Svuotare la cronologia?',
		clearDescription: (count: number) =>
			`Rimuove definitivamente tutte le ${count} esecuzioni registrate, incluso qualsiasi testo di input e risultato ancora conservato qui per il recupero. Questa azione non può essere annullata.`
	},

	// New section (mockup-a-compact.html): theme / language / autostart.
	settings: {
		title: 'Impostazioni',
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
