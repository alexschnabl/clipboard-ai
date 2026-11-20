import type { en } from './en';

export const fr: typeof en = {
	nav: {
		commands: 'Commandes',
		backends: 'Backends',
		history: 'Historique',
		vocabulary: 'Vocabulaire',
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
		},
		tryIt: {
			title: 'Essayer',
			hint: 'Rien n’est collé',
			inputLabel: 'Votre texte',
			inputPlaceholder: 'Tapez ou collez un exemple, comme le texte que vous sélectionneriez.',
			outputLabel: 'Ce qui serait collé',
			run: 'Lancer',
			shortcutOr: 'ou',
			stop: 'Arrêter',
			asking: (backend: string) => `${backend} répond…`,
			idle: 'Lancez un exemple pour voir le résultat ici. L’exemple est envoyé en texte brut.',
			stale: 'Modifié depuis cet essai',
			noChanges: 'Aucun changement. Une vraie exécution recollerait le même texte.',
			appLabel: 'Comme dans',
			appAnywhere: 'toute autre app',
			defaultModel: 'modèle par défaut',
			ruleApplied: (app: string) => `règle pour ${app}`,
			sendOnly:
				'Ce backend envoie seulement le texte et ne colle rien en retour, il n’y a donc rien à prévisualiser. L’onglet Historique montre chaque exécution.',
			httpNote: 'Lancer envoie l’exemple au point de terminaison, comme une vraie exécution.',
			errors: {
				timeout:
					'Le backend n’a pas répondu dans son délai. Un modèle lent ou à froid peut demander un délai plus long.',
				unreachable:
					'Impossible de joindre le backend. Vérifiez qu’il fonctionne et que son adresse est correcte.',
				misconfigured: 'Le backend n’est pas entièrement configuré.',
				badResponse: 'Le backend a répondu par une erreur.',
				noSentinel:
					'Le modèle a répondu, mais pas sous la forme attendue. Réessayez ou simplifiez le prompt.',
				emptyResult: 'Le modèle n’a rien renvoyé.',
				nothingToPreview: 'Ce backend ne colle rien en retour, il n’y a donc rien à prévisualiser.',
				unknownBackend: 'Choisissez d’abord un backend.',
				emptyInput: 'Tapez d’abord un exemple.',
				notAnAiBackend: 'Ce backend ne peut pas écrire de prompts. Choisissez un backend d’IA.',
				internal: 'Un problème est survenu dans l’app.'
			}
		},
		writer: {
			openClaude: 'Écrire avec Claude',
			openOther: 'Écrire avec l’IA',
			title: 'Écrire le prompt pour moi',
			description:
				'Dites simplement ce que la commande doit faire. Le résultat remplace le prompt, et vous pouvez le modifier ou l’annuler.',
			label: 'Que doit faire la commande ?',
			placeholder: 'Rendre mes e-mails plus chaleureux, mais les garder courts.',
			reviseHint:
				'Votre prompt actuel est envoyé aussi, vous pouvez donc demander une modification, comme « rends-le plus court ».',
			backendLabel: 'Écrit par',
			submit: 'Écrire le prompt',
			working: 'Écriture…',
			done: (backend: string) => `Écrit par ${backend}.`,
			undo: 'Annuler',
			errorTitle: 'Impossible d’écrire le prompt'
		},
		rules: {
			title: 'Dans certaines apps',
			hint: 'Utilisé à la place là-bas',
			add: 'Ajouter une règle pour une app',
			adds: (text: string) => `Ajoute : ${text}`,
			uses: (backend: string) => `Utilise ${backend}`,
			nothing: 'Ne change encore rien',
			savedWithCommand: 'Les règles sont enregistrées avec la commande.',
			addTitle: 'Ajouter une règle pour une app',
			editTitle: (app: string) => `Règle pour ${app}`,
			description: 'Quand vous lancez cette commande dans cette app, ces réglages sont utilisés à la place.',
			appLabel: 'App',
			search: 'Rechercher dans les apps ouvertes',
			loading: 'Recherche des apps ouvertes…',
			none: 'Aucune app ouverte trouvée. Saisissez plutôt le nom du programme.',
			noMatch: 'Aucune app ouverte ne correspond.',
			manual: 'Saisir le nom du programme',
			pickFromList: 'Choisir parmi les apps ouvertes',
			refresh: 'Actualiser',
			change: 'Changer',
			exeLabel: 'Fichier du programme',
			exePlaceholder: 'OUTLOOK.EXE',
			exeHint: 'Le nom du .exe, tel que le Gestionnaire des tâches l’affiche sous Détails.',
			nameLabel: 'Affiché comme',
			namePlaceholder: 'Outlook',
			promptLabel: 'Ajouter au prompt',
			promptPlaceholder: 'Reste formel et vouvoie le lecteur.',
			promptHint: 'Ajouté après le prompt de la commande, dans cette app uniquement.',
			backendLabel: 'Backend',
			formattingLabel: 'Mise en forme',
			same: 'Comme la commande',
			duplicate: 'Cette commande a déjà une règle pour cette app.',
			empty: 'Changez au moins une chose pour cette app.',
			pickApp: 'Choisissez d’abord une app.',
			submitAdd: 'Ajouter la règle',
			submitSave: 'Enregistrer la règle',
			delete: 'Supprimer la règle',
			editAria: (app: string) => `Modifier la règle pour ${app}`
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
			`Encore utilisé par : ${usedBy}. Faites d’abord pointer ces commandes et leurs règles par app vers un autre backend, puis supprimez-le.`,
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

	vocabulary: {
		title: 'Vocabulaire',
		keepTitle: 'Mots que chaque commande garde tels quels',
		keepHint: 'Noms, produits, lieux',
		keepPlaceholder: 'Ajouter un mot ou un nom',
		keepAdd: 'Ajouter',
		keepRemove: (word: string) => `Retirer ${word}`,
		keepEmpty: 'Aucun mot pour l’instant. Ajoutez les noms que le modèle a tendance à « corriger ».',
		replaceTitle: 'Toujours remplacer',
		replaceHint: 'Avant que le texte n’atteigne le modèle',
		replaceFrom: 'Remplacer',
		replaceTo: 'Par',
		replaceFromPlaceholder: 'sus tech',
		replaceToPlaceholder: 'Sus-Tech',
		replaceAdd: 'Ajouter un remplacement',
		replaceSave: 'Enregistrer',
		replaceDeleted: 'supprimé',
		replaceEditAria: (from: string) => `Modifier le remplacement de ${from}`,
		replaceRemove: (from: string) => `Retirer le remplacement de ${from}`,
		note: 'La liste de mots est ajoutée aux instructions de chaque commande, car un modèle qui « corrige » un nom qu’il ne connaît pas est le mauvais résultat le plus courant. Les remplacements portent sur des mots entiers, quelle que soit la casse, et modifient le texte sélectionné avant qu’un backend ne le voie.',
		saveErrorTitle: 'Impossible d’enregistrer cette modification',
		saveFailedFallback: 'Impossible d’enregistrer la modification.'
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
		saveErrorTitle: 'Impossible d’enregistrer cette modification',
		saveFailedFallback: 'Impossible d’enregistrer la modification.'
	}
};
