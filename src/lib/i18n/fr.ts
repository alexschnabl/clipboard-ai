import type { en } from './en';

export const fr: typeof en = {
	nav: {
		commands: 'Commandes',
		backends: 'Backends',
		history: 'Historique',
		settings: 'Paramètres',
		working: 'En cours…'
	},

	common: {
		cancel: 'Annuler',
		delete: 'Supprimer',
		saving: 'Enregistrement…'
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
			saveError: 'Impossible d’enregistrer la commande',
			saveFailedFallback: 'Échec de l’enregistrement.',
			addSubmit: 'Ajouter la commande',
			saveSubmit: 'Enregistrer les modifications'
		}
	},

	backends: {
		title: 'Backends',
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
			saveSubmit: 'Enregistrer les modifications'
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
		clearTitle: 'Effacer l’historique ?',
		clearDescription: (count: number) =>
			`Supprime définitivement les ${count} exécutions enregistrées, y compris tout texte d’entrée et de résultat encore conservé ici pour récupération. Cette action est irréversible.`
	},

	settings: {
		title: 'Paramètres',
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
