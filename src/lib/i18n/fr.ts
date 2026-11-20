import type { en } from './en';

export const fr: typeof en = {
	nav: {
		commands: 'Commandes',
		backends: 'Backends',
		history: 'Historique',
		settings: 'Paramètres',
		working: 'En cours…',
		version: (v: string) => `Version ${v}`,
		quit: 'Quitter Clipboard AI'
	},

	common: {
		cancel: 'Annuler',
		delete: 'Supprimer',
		saving: 'Enregistrement…',
		back: 'Retour'
	},

	app: {
		loading: 'Chargement…',
		loadErrorTitle: 'Votre configuration enregistrée n’a pas pu être lue',
		loadErrorPreservedBefore: 'Le fichier d’origine a été conservé, inchangé, à l’emplacement',
		loadErrorPreservedAfter:
			'Corrigez-le à la main puis redémarrez l’application pour récupérer vos backends et prompts existants.',
		loadErrorNotPreserved:
			'Clipboard AI n’a pas non plus pu conserver le fichier d’origine — consultez le journal.',
		loadErrorFallbackNotice:
			'Ce que vous voyez ci-dessous est un point de départ vierge, pas vos paramètres enregistrés. Toute modification que vous effectuez ici est enregistrée immédiatement — cela ne fait que créer un nouveau config.json et ne touchera pas la copie conservée ci-dessus.',
		errorLoadTitle: 'Impossible de charger la configuration',
		errorSaveTitle: 'Impossible d’enregistrer la configuration',
		problemsTitle: 'Certains paramètres n’ont pas pu être appliqués'
	},

	commands: {
		title: 'Commandes',
		groupTitle: 'Sélectionnez du texte n’importe où, puis appuyez sur un raccourci',
		groupHint: 'Remplace la sélection',
		count: (n: number) => `${n} configurée${n === 1 ? '' : 's'}`,
		addCommand: 'Ajouter une commande',
		columnName: 'Nom',
		columnHotkey: 'Raccourci',
		columnBackend: 'Backend',
		columnActions: 'Actions',
		empty: 'Aucune commande configurée.',
		unknownBackend: 'Backend inconnu',
		editAria: (name: string) => `Modifier ${name}`,
		deleteAria: (name: string) => `Supprimer ${name}`,
		deleteTitle: (name: string) => `Supprimer « ${name} » ?`,
		deleteDescription:
			'Cette action supprime définitivement la commande et son raccourci. Elle est irréversible.',
		form: {
			addTitle: 'Ajouter une commande',
			editTitle: 'Modifier la commande',
			addDescription: 'Configurer une nouvelle commande.',
			editDescription: 'Mettre à jour le backend, le raccourci et le prompt de cette commande.',
			nameLabel: 'Nom',
			namePlaceholder: 'Corriger la grammaire',
			backendLabel: 'Backend',
			backendPlaceholder: 'Sélectionner un backend',
			noBackendsHint: 'Aucun backend pour l’instant — ajoutez-en un d’abord dans l’onglet « Backends ».',
			hotkeyLabel: 'Raccourci',
			promptLabel: 'Prompt système',
			promptPlaceholder: 'Corriger la grammaire et l’orthographe. Conserver le sens.',
			promptHttpHint:
				'Le backend sélectionné est un simple point de terminaison HTTP. Le prompt n’est pas transmis avec la requête.',
			promptSentinelHint:
				'L’application demande automatiquement au modèle de ne renvoyer que le texte transformé, rien d’autre ; inutile de le préciser dans votre invite.',
			formattingLabel: 'Mise en forme',
			formattingOptionAuto: 'Conserver la mise en forme de la sélection',
			formattingOptionRich: 'Toujours renvoyer du texte mis en forme',
			formattingOptionPlain: 'Texte brut uniquement',
			formattingHintAuto:
				'Le gras, les listes, les liens et les images de la sélection sont conservés. Une sélection en texte brut reste en texte brut.',
			formattingHintRich:
				'Le modèle est invité à répondre avec des titres, des listes et des mises en évidence, même si la sélection est en texte brut. Pour les invites qui améliorent la mise en forme.',
			formattingHintPlain:
				'Seul le texte est envoyé et collé. La mise en forme et les images sont perdues.',
			saveError: 'Impossible d’enregistrer la commande',
			saveFailedFallback: 'Échec de l’enregistrement.',
			addSubmit: 'Ajouter la commande',
			saveSubmit: 'Enregistrer les modifications',
			deleteSubmit: 'Supprimer la commande'
		}
	},

	backends: {
		title: 'Backends',
		groupTitle: 'Où les commandes envoient le texte',
		groupHint: 'Chaque commande en utilise un',
		count: (n: number) => `${n} configuré${n === 1 ? '' : 's'}`,
		addBackend: 'Ajouter un backend',
		columnName: 'Nom',
		columnKind: 'Type',
		columnModel: 'Modèle',
		columnActions: 'Actions',
		empty: 'Aucun backend configuré.',
		editAria: (name: string) => `Modifier ${name}`,
		deleteAria: (name: string) => `Supprimer ${name}`,
		deleteTitle: (name: string) => `Supprimer « ${name} » ?`,
		deleteDescription: 'Cette action supprime définitivement le backend. Elle est irréversible.',
		blockedDeleteTitle: (name: string) => `Impossible de supprimer « ${name} »`,
		blockedDeleteDescription: (usedBy: string) =>
			`Encore utilisé par : ${usedBy}. Faites d’abord pointer ces commandes vers un autre backend, puis supprimez-le.`,
		usedBy: (names: string) => `Utilisé par ${names}`,
		unused: 'Utilisé par aucune commande',
		form: {
			addTitle: 'Ajouter un backend',
			editTitle: 'Modifier le backend',
			addDescription: 'Configurer un nouveau backend IA.',
			editDescription: 'Mettre à jour les informations de connexion de ce backend.',
			nameLabel: 'Nom',
			namePlaceholder: 'Ollama local',
			kindLabel: 'Type',
			baseUrlLabel: 'URL de base',
			modelLabel: 'Modèle',
			modelPlaceholderOpenAiCompatible: 'qwen2.5:32b-instruct',
			modelPlaceholderOther: 'claude-opus-5',
			modelPlaceholderClaudeCli: 'haiku',
			modelHintClaudeCli:
				'Facultatif. Un modèle plus léger comme haiku répond plus vite ; laissez vide pour le modèle par défaut de la CLI.',
			effortLabel: 'Effort',
			effortOptionDefault: 'Par défaut',
			effortOptionLow: 'Faible',
			effortOptionMedium: 'Moyen',
			effortOptionHigh: 'Élevé',
			effortOptionXhigh: 'Très élevé',
			effortOptionMax: 'Maximal',
			effortHint:
				'Un effort plus faible répond plus vite et suffit pour un texte court. Par défaut laisse la CLI décider.',
			httpUrlHint:
				'Le texte sélectionné est envoyé à cette URL comme corps de la requête POST. Le prompt de la commande n’est pas transmis.',
			expectResponseLabel: 'Attendre une réponse',
			expectResponseHint:
				'Activé colle le corps de la réponse sur la sélection. Désactivé se contente d’envoyer.',
			apiKeyLabel: 'Clé API',
			apiKeyOptional: '(facultatif)',
			showApiKey: 'Afficher la clé API',
			hideApiKey: 'Masquer la clé API',
			timeoutLabel: 'Délai (ms)',
			timeoutHint: (defaultMs: string) =>
				`Un modèle local à froid peut nécessiter environ 16 000 ms pour sa première requête de la journée — la valeur par défaut de ${defaultMs} ms évite alors un dépassement de délai injustifié.`,
			timeoutTooLow: (minMs: number) => `Le délai doit être d’au moins ${minMs} ms.`,
			saveError: 'Impossible d’enregistrer le backend',
			saveFailedFallback: 'Échec de l’enregistrement.',
			addSubmit: 'Ajouter le backend',
			saveSubmit: 'Enregistrer les modifications',
			deleteSubmit: 'Supprimer le backend'
		}
	},

	hotkeyRecorder: {
		prompt: 'Appuyez sur une combinaison…',
		placeholder: 'Cliquez pour définir un raccourci',
		hintModifier:
			'Maintenez une touche de modification (Ctrl, Alt, Maj ou Win) enfoncée et appuyez sur une touche.',
		recordingStatus:
			'Enregistrement du raccourci en cours. Appuyez sur une combinaison de touches, ou sur Échap pour annuler.',
		ariaHotkey: (value: string) => `Raccourci : ${value}. Cliquez pour modifier.`,
		ariaEmpty: 'Aucun raccourci défini. Cliquez pour en enregistrer un.'
	},

	history: {
		title: 'Historique',
		hint: (limit: number) => `${limit} dernières exécutions`,
		clear: 'Effacer',
		columnTime: 'Heure',
		columnCommand: 'Commande',
		columnBackend: 'Backend',
		columnOutcome: 'Résultat',
		columnDuration: 'Durée',
		statusSuccess: 'Réussi',
		statusFailed: 'Échoué',
		statusAborted: 'Annulé',
		statusCancelled: 'Annulé',
		instructionTag: 'Instruction',
		emptyTitle: 'Aucune exécution pour l’instant',
		emptyDescription: 'Déclenchez une commande avec son raccourci et le résultat s’affichera ici.',
		loadErrorTitle: 'Impossible de charger l’historique',
		copyErrorTitle: 'Impossible de copier dans le presse-papiers',
		expandAria: (name: string) => `Développer les détails de ${name}`,
		collapseAria: (name: string) => `Réduire les détails de ${name}`,
		abortedPrefix: 'Interrompu :',
		failedPrefix: 'Échoué :',
		inputLabel: 'Entrée',
		resultLabel: 'Résultat',
		noInputCaptured: 'rien n’a été capturé pour cette exécution',
		noResultPasted: 'rien n’a été collé pour cette exécution',
		copy: 'Copier',
		copied: 'Copié',
		copyResult: 'Copier le résultat',
		searchPlaceholder: 'Rechercher dans l’historique',
		noMatches: 'Aucune exécution ne correspond à votre recherche.',
		changesLabel: 'Ce qui a changé',
		durationDetail: (total: string, backend: string) =>
			`${total} au total, dont ${backend} d’attente du backend`,
		clearTitle: 'Effacer l’historique ?',
		clearDescription: (count: number) =>
			`Supprime définitivement les ${count} exécutions enregistrées, y compris tout texte d’entrée et de résultat encore conservé ici pour récupération. Cette action est irréversible.`
	},

	reasons: {
		cancelled: 'Rien n’a été collé.',
		focusChanged: 'Vous avez changé de fenêtre avant l’arrivée de la réponse.',
		focusRestoreFailed: 'Impossible de revenir à votre fenêtre.',
		captureTimeout: 'Aucun texte n’était sélectionné, ou il n’a pas pu être copié.',
		modifiersStuck: 'Une touche de modification était encore enfoncée.',
		modifiersStuckBeforePaste: 'Une touche de modification était enfoncée au moment de coller.',
		backendError: 'Le backend a signalé une erreur. Les détails sont dans l’historique.',
		noSentinel: 'La réponse est arrivée dans un format inattendu.',
		emptyResult: 'La réponse était vide.',
		clipboardWriteFailed: 'Le presse-papiers était occupé, rien n’a pu être collé.',
		pasteFailed: 'La frappe pour coller n’a pas pu être envoyée.',
		noLastResult: 'Lancez d’abord une commande, vous pourrez ensuite recoller son résultat.',
		paletteFocusFailed: 'La palette n’a pas pu s’ouvrir. Réessayez le raccourci.',
		unknown: (code: string) => `Arrêté : ${code}.`
	},

	hud: {
		escHint: 'pour annuler',
		done: 'Terminé',
		wordsChanged: (n: number) => (n === 1 ? '1 mot modifié' : `${n} mots modifiés`),
		noChanges: 'Rien à modifier',
		sent: 'Envoyé',
		pastedAgain: 'Collé à nouveau',
		undoHint: 'pour défaire',
		nothingPasted: 'Rien n’a été collé',
		cancelled: 'Annulé',
		nothingToPaste: 'Rien à coller pour l’instant',
		paletteClosed: 'Palette fermée'
	},

	palette: {
		placeholder: 'Chercher une commande ou saisir une instruction',
		groupCommands: 'Commandes',
		runInstruction: (text: string) => `Exécuter « ${text} »`,
		instructionHint: 'Instruction ponctuelle',
		noBackend: 'Ajoutez un backend IA pour exécuter des instructions',
		noMatches: 'Aucune commande ne correspond. Entrée l’exécute comme instruction.',
		empty: 'Aucune commande pour l’instant. Saisissez plutôt une instruction.',
		footerRun: 'exécuter',
		footerBackend: 'backend',
		footerClose: 'fermer',
		listLabel: 'Commandes et instructions'
	},

	settings: {
		title: 'Paramètres',
		groupAppearance: 'Apparence',
		groupStartup: 'Démarrage',
		themeLabel: 'Thème',
		themeDescription: 'S’applique à cette fenêtre. Suit Windows par défaut.',
		themeSystem: 'Système',
		themeLight: 'Clair',
		themeDark: 'Sombre',
		languageLabel: 'Langue',
		languageDescription: 'Langue de l’interface pour cette fenêtre.',
		languageSystem: 'Système',
		autostartLabel: 'Démarrer avec Windows',
		autostartDescription: 'Lance Clipboard AI réduit dans la zone de notification à l’ouverture de session.',
		autostartDevNoteBefore:
			'Refusé dans les builds de développement — cela enregistrerait l’exécutable de débogage à la place de l’application installée au démarrage de la session ; testez plutôt cela avec un build installé (',
		autostartDevNoteAfter: ').',
		groupRunning: 'Pendant l’exécution d’une commande',
		hudLabel: 'Statut près du curseur',
		hudDescription:
			'Indique la commande en cours, permet d’annuler avec Esc et explique pourquoi rien n’a été collé.',
		busyCursorLabel: 'Pointeur occupé',
		busyCursorDescription: 'Affiche le pointeur occupé pendant que le backend travaille.',
		groupShortcuts: 'Raccourcis disponibles partout',
		paletteLabel: 'Palette de commandes',
		paletteDescription:
			'Choisissez une commande pour le texte sélectionné, ou saisissez une instruction ponctuelle.',
		pasteLastLabel: 'Recoller le dernier résultat',
		pasteLastDescription:
			'Colle le résultat le plus récent au curseur, aussi depuis le menu de la zone de notification. Désactivé par défaut car Office utilise Ctrl+Alt+V pour le collage spécial.',
		pasteLastToggle: 'Utiliser un raccourci pour recoller le dernier résultat',
		saveErrorTitle: 'Impossible d’enregistrer cette modification',
		saveFailedFallback: 'Impossible d’enregistrer la modification.'
	}
};
