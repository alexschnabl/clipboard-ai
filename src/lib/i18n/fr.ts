// lib/i18n/fr.ts — French UI text. Typed against `typeof en` (not
// independently declared): TypeScript rejects this file at compile time if
// a key is missing, a key exists here that `en.ts` doesn't have, or a
// function's parameter types don't match.
// Do not add `as const` or a standalone type annotation other than the one
// below; either would weaken or break that guarantee.
//
// Terminology, kept consistent everywhere in this file (not swapped for a
// synonym anywhere, per the task brief):
//   - "command" -> commande / commandes
//   - "backend" -> backend (kept as the English loanword — standard French
//     IT usage, same choice de.ts/es.ts make for German/Spanish; every
//     backend *kind* label is already an English/mixed technical term too).
//   - "hotkey"  -> raccourci (short for "raccourci clavier", kept to one
//     word specifically so the Commands table's ~108px hotkey column in
//     .superpowers/mockups/mockup-a-compact.html doesn't overflow — see
//     de.ts's note on the same column).
//   - "prompt"  -> prompt (kept as the loanword — standard in current
//     French AI/dev usage; "invite", the official Microsoft French term,
//     reads stilted here and isn't how French AI tooling actually talks).
//   - "timeout" -> délai (translated, not loaned — shorter than the official
//     "délai d'expiration" and unambiguous next to "(ms)"; short enough that
//     the loanword wasn't needed even in the narrower spots).
//
// Register: vouvoiement ("vous") for sentences that address the user
// directly (error-recovery text, inline hints); infinitives for buttons,
// menu actions, and short dialog-subtitle captions ("Configurer une
// nouvelle commande."), matching typical French Windows desktop software —
// this mirrors exactly how de.ts splits the same two registers.
// Quotation marks use French angular quotes ( « … » ), not the German „…“
// or English “…” pairs the other dictionaries use, with a space before
// ; : ! ? and the closing » per French typographic convention. Curly
// apostrophes (’) throughout, matching en.ts/de.ts.

import type { en } from './en';

export const fr: typeof en = {
	nav: {
		commands: 'Commandes',
		backends: 'Backends',
		history: 'Historique',
		settings: 'Paramètres'
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

	// src/lib/components/CommandList.svelte + CommandForm.svelte
	commands: {
		title: 'Commandes',
		// "commande" is feminine, hence "configurée(s)" -- agrees with
		// `backends.count` below using the masculine "configuré(s)" instead.
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
			saveError: 'Impossible d’enregistrer la commande',
			saveFailedFallback: 'Échec de l’enregistrement.',
			addSubmit: 'Ajouter la commande',
			saveSubmit: 'Enregistrer les modifications'
		}
	},

	// src/lib/components/BackendList.svelte + BackendForm.svelte
	backends: {
		title: 'Backends',
		// "backend" is masculine, hence "configuré(s)" -- see `commands.count`
		// above for the feminine counterpart.
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
			apiKeyLabel: 'Clé API',
			apiKeyOptional: '(facultatif)',
			showApiKey: 'Afficher la clé API',
			hideApiKey: 'Masquer la clé API',
			timeoutLabel: 'Délai (ms)',
			// `defaultMs` is passed in already formatted (the caller's
			// `DEFAULT_TIMEOUT_MS.toLocaleString()`) so this dictionary never
			// has to make its own locale-formatting decision.
			timeoutHint: (defaultMs: string) =>
				`Un modèle local à froid peut nécessiter environ 16 000 ms pour sa première requête de la journée — la valeur par défaut de ${defaultMs} ms évite alors un dépassement de délai injustifié.`,
			timeoutTooLow: (minMs: number) => `Le délai doit être d’au moins ${minMs} ms.`,
			saveError: 'Impossible d’enregistrer le backend',
			saveFailedFallback: 'Échec de l’enregistrement.',
			addSubmit: 'Ajouter le backend',
			saveSubmit: 'Enregistrer les modifications'
		}
	},

	// src/lib/components/HotkeyRecorder.svelte
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

	// New section (mockup-a-compact.html): run history.
	history: {
		title: 'Historique',
		// `limit` is the retention cap (100 — see `history::MAX_ENTRIES` in
		// src-tauri/src/history.rs), passed in rather than hard-coded so this
		// stays correct if that constant ever changes.
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
		// French typography wants a space before ":" -- baked into the
		// prefix itself so `{prefix} {value}` in the component still comes
		// out with exactly one space either side, same as every other
		// dictionary's `{prefix} {value}` template.
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

	// New section (mockup-a-compact.html): theme / language / autostart.
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
		// Language *names* (English shown as "English", French as
		// "Français", ...) are not dictionary entries: they live once each
		// in registry.ts's `nativeName`, not once per *other* language in
		// every dictionary here -- see that file's header comment for why.
		autostartLabel: 'Démarrer avec Windows',
		autostartDescription: 'Lance Clipboard AI réduit dans la zone de notification à l’ouverture de session.',
		autostartDevNoteBefore:
			'Refusé dans les builds de développement — cela enregistrerait l’exécutable de débogage à la place de l’application installée au démarrage de la session ; testez plutôt cela avec un build installé (',
		autostartDevNoteAfter: ').',
		saveErrorTitle: 'Impossible d’enregistrer cette modification',
		saveFailedFallback: 'Impossible d’enregistrer la modification.'
	}
};
