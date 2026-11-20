import type { en } from './en';

export const fr: typeof en = {
	nav: {
		commands: 'Commandes',
		backends: 'Backends',
		history: 'Historique',
		settings: 'Paramètres',
		working: 'En cours…',
		version: (v: string) => `Version ${v}`,
		needsAttention: 'Demande votre attention',
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
		problems: {
			hotkeyTaken: 'Une autre application utilise déjà ce raccourci. Choisissez-en un autre.',
			hotkeyClash: (other: string) => `Même raccourci que « ${other} », seul celui-là s’exécute.`,
			hotkeyInvalid: 'Ce raccourci n’est pas reconnu. Enregistrez-le à nouveau.',
			backendMissing: 'Son backend n’existe plus. Choisissez-en un autre.'
		},
		onboarding: {
			title: 'Premiers pas',
			intro: 'Clipboard AI a besoin d’un backend qui répond. Ensuite, sélectionnez du texte n’importe où et appuyez sur un raccourci.',
			hide: 'Masquer',
			hideAria: 'Masquer ce guide définitivement',
			step1Title: 'Configurer un backend',
			step1Text:
				'Un modèle local via Ollama, votre abonnement Claude via la CLI claude, ou une clé API.',
			checkBackends: 'Vérifier les backends',
			openBackends: 'Ouvrir Backends',
			checking: 'Vérification des backends…',
			works: (name: string) => `${name} fonctionne.`,
			noneWorking: 'Aucun de vos backends n’a encore répondu.',
			step2Title: (name: string) => `Essayer « ${name} »`,
			step2Text: 'Sélectionnez une phrase dans n’importe quelle application et appuyez sur',
			step2NoCommand: 'Ajoutez une commande avec un raccourci et un prompt.',
			backendSilent: (backend: string) => `Son backend, ${backend}, n’a pas répondu.`,
			useBackend: (name: string) => `Utiliser ${name}`,
			step2Done: 'Ça a marché. Tout est prêt.'
		},
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
		check: 'Vérifier',
		checkAll: 'Tout vérifier',
		checkAria: (name: string) => `Vérifier ${name}`,
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
			apiKeyStoredPlaceholder: 'Enregistrée dans le Gestionnaire d’identification Windows',
			apiKeyHint: 'Conservée dans le Gestionnaire d’identification Windows, pas dans config.json.',
			apiKeyInline:
				'Toujours dans config.json, car le Gestionnaire d’identification Windows l’a refusée. L’enregistrement réessaie.',
			apiKeyRemove: 'Supprimer la clé',
			apiKeyWillBeRemoved: 'La clé sera supprimée à l’enregistrement.',
			apiKeyUndo: 'Annuler',
			apiKeyRevealFailed: 'Impossible de lire la clé dans le Gestionnaire d’identification Windows.',
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

	health: {
		checking: 'Vérification…',
		notChecked: 'Pas encore vérifié',
		checkFailed: 'La vérification elle-même a échoué',
		ok: (time: string) => `Réponse en ${time}`,
		cold: (first: string, warm: string) =>
			`Le modèle était froid. Première réponse en ${first}, la suivante en ${warm}`,
		slow: (time: string) => `Fonctionne, mais lentement. ${time}`,
		reachable: 'Joignable. Rien ne lui a été envoyé',
		cliReady: (version: string) => `Connecté, CLI ${version}`,
		cliSignedOut: 'Non connecté. Lancez claude dans un terminal pour vous connecter',
		cliUnknownAuth: (version: string) => `CLI ${version} trouvée, connexion non confirmée`,
		cliMissing: 'CLI Claude introuvable. Installez-la et connectez-vous',
		keyMissing: 'Aucune clé API définie',
		keyRejected: 'Clé refusée',
		keyUnreadable: 'Impossible de lire la clé API',
		modelNotFound: 'Modèle ou URL introuvable',
		notReachable: 'Injoignable',
		timeout: (secs: number) => `Pas de réponse en ${secs} s`,
		needsUrl: 'Aucune URL définie',
		needsModel: 'Aucun modèle défini',
		misconfigured: 'Configuration incomplète',
		badResponse: 'Réponse inattendue'
	},

	update: {
		promptTitle: (version: string) => `Clipboard AI ${version} est disponible`,
		promptText: 'Installez-la maintenant, ou plus tard depuis les paramètres.',
		install: 'Installer et redémarrer',
		later: 'Plus tard',
		downloading: 'Téléchargement…',
		downloadingPercent: (percent: number) => `Téléchargement… ${percent} %`
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

	settings: {
		title: 'Paramètres',
		groupAppearance: 'Apparence',
		groupStartup: 'Démarrage',
		groupAbout: 'À propos',
		versionLabel: (version: string) => `Clipboard AI ${version}`,
		updateDescription: 'Vérifie GitHub une fois par jour. Rien ne s’installe sans votre clic.',
		updateCheck: 'Rechercher des mises à jour',
		updateChecking: 'Vérification…',
		updateUpToDate: 'Vous avez la dernière version.',
		updateAvailable: (version: string) => `La version ${version} est disponible.`,
		updateInstallFailed: 'Impossible d’installer la mise à jour. Réessayez plus tard.',
		updateCheckFailed: 'Impossible de rechercher des mises à jour. Réessayez plus tard.',
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
		saveErrorTitle: 'Impossible d’enregistrer cette modification',
		saveFailedFallback: 'Impossible d’enregistrer la modification.'
	}
};
