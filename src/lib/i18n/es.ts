// lib/i18n/es.ts — Spanish (Spain) UI text. Typed against `typeof en` (not
// independently declared): TypeScript rejects this file at compile time if
// a key is missing, a key exists here that `en.ts` doesn't have, or a
// function's parameter types don't match.
// Do not add `as const` or a standalone type annotation other than the one
// below; either would weaken or break that guarantee.
//
// Terminology, kept consistent everywhere in this file (not swapped for a
// synonym anywhere, per the task brief):
//   - "command" -> comando / comandos
//   - "backend" -> backend (kept as the English loanword — universal in
//     Spanish technical usage; the same choice de.ts makes for German, and
//     every backend *kind* label is already an English technical term too).
//   - "hotkey"  -> hotkey (kept as a loanword rather than "combinación de
//     teclas" or "atajo de teclado", specifically so the Commands table's
//     ~108px hotkey column doesn't overflow — see de.ts's note on the same
//     column).
//   - "prompt"  -> prompt (kept as the loanword — standard in current
//     Spanish AI/dev usage and shorter than "instrucción").
//   - "timeout" -> tiempo de espera (translated, not loaned — it isn't a
//     table-column value like hotkey, only a form label/sentence, so there
//     was room to use the term Spanish Windows software actually uses;
//     flagged for a layout check anyway in the review report since the
//     mockup groups it with other narrow-column concerns).
//
// `history.statusAborted` -> "Cancelado", deliberately not the cognate
// "Abortado": in Spanish that word reads harsh/clinical (closer to "aborted"
// in the medical sense) and would make a deliberate, correct safety decline
// sound like a crash. "Cancelado" reads as calm and intentional — the same
// register as a cancelled download — which is what distinguishes it from
// `statusFailed` -> "Fallido" (an actual error).
//
// Register: impersonal/formal ("usted") for sentences that address the
// user, infinitives for buttons and short in-control microcopy ("Guardar",
// "Cancelar", "Añadir"), matching typical Windows desktop software sold in
// Spain. Peninsular Spanish throughout — notably "pulsar"/"pulse" for key
// presses, where Latin American Spanish would more commonly use
// "presionar"/"presione".
// Quotation marks use Spanish angular quotes ( «…» ), not the German „…“
// or English “…” pairs the other two dictionaries use.

import type { en } from './en';

export const es: typeof en = {
	nav: {
		commands: 'Comandos',
		backends: 'Backends',
		history: 'Historial',
		settings: 'Ajustes'
	},

	common: {
		cancel: 'Cancelar',
		delete: 'Eliminar',
		saving: 'Guardando…'
	},

	app: {
		title: 'Clipboard AI',
		subtitle: 'Configurar backends y comandos.',
		loading: 'Cargando…',
		loadErrorTitle: 'No se pudo leer la configuración guardada',
		loadErrorPreservedBefore: 'El archivo original se conservó sin modificar en',
		loadErrorPreservedAfter:
			'Corríjalo a mano y reinicie la aplicación para recuperar sus backends y prompts existentes.',
		loadErrorNotPreserved:
			'Clipboard AI tampoco pudo conservar el archivo original — consulte el registro.',
		loadErrorFallbackNotice:
			'Lo que ve a continuación es un punto de partida en blanco, no su configuración guardada. Cualquier cambio que haga aquí se guarda de inmediato — esto solo crea un nuevo config.json y no afectará a la copia conservada arriba.',
		errorLoadTitle: 'No se pudo cargar la configuración',
		errorSaveTitle: 'No se pudo guardar la configuración',
		problemsTitle: 'Algunos ajustes no se pudieron aplicar'
	},

	commands: {
		title: 'Comandos',
		addCommand: 'Añadir comando',
		columnName: 'Nombre',
		columnHotkey: 'Hotkey',
		columnBackend: 'Backend',
		columnActions: 'Acciones',
		empty: 'No hay comandos configurados.',
		editAria: (name: string) => `Editar ${name}`,
		deleteAria: (name: string) => `Eliminar ${name}`,
		deleteTitle: (name: string) => `¿Eliminar «${name}»?`,
		deleteDescription: 'Esto elimina el comando y su hotkey de forma permanente. Esta acción no se puede deshacer.',
		form: {
			addTitle: 'Añadir comando',
			editTitle: 'Editar comando',
			addDescription: 'Configurar un nuevo comando.',
			editDescription: 'Actualizar el backend, el hotkey y el prompt de este comando.',
			nameLabel: 'Nombre',
			namePlaceholder: 'Corregir gramática',
			backendLabel: 'Backend',
			backendPlaceholder: 'Seleccionar un backend',
			noBackendsHint: 'Aún no hay backends — añada uno primero en la pestaña «Backends».',
			hotkeyLabel: 'Hotkey',
			promptLabel: 'Prompt del sistema',
			promptPlaceholder: 'Corregir la gramática y la ortografía. Mantener el significado.',
			saveError: 'No se pudo guardar el comando',
			saveFailedFallback: 'No se pudo guardar.',
			addSubmit: 'Añadir comando',
			saveSubmit: 'Guardar cambios'
		}
	},

	backends: {
		title: 'Backends',
		addBackend: 'Añadir backend',
		columnName: 'Nombre',
		columnKind: 'Tipo',
		columnModel: 'Modelo',
		columnActions: 'Acciones',
		empty: 'No hay backends configurados.',
		editAria: (name: string) => `Editar ${name}`,
		deleteAria: (name: string) => `Eliminar ${name}`,
		deleteTitle: (name: string) => `¿Eliminar «${name}»?`,
		deleteDescription: 'Esto elimina el backend de forma permanente. Esta acción no se puede deshacer.',
		blockedDeleteTitle: (name: string) => `No se puede eliminar «${name}»`,
		blockedDeleteDescription: (usedBy: string) =>
			`Aún lo utilizan: ${usedBy}. Asigne primero otro backend a esos comandos antes de eliminarlo.`,
		// Mirrors `KIND_LABELS` in src/lib/types.ts (owned by another agent) —
		// kept as a parallel translated copy rather than importing that map,
		// since this file must not depend on anything outside `lib/i18n`.
		kindLabel: {
			openaiCompatible: 'Compatible con OpenAI',
			anthropicApi: 'Anthropic API',
			claudeCli: 'Claude CLI'
		},
		form: {
			addTitle: 'Añadir backend',
			editTitle: 'Editar backend',
			addDescription: 'Configurar un nuevo backend de IA.',
			editDescription: 'Actualizar los datos de conexión de este backend.',
			nameLabel: 'Nombre',
			namePlaceholder: 'Ollama local',
			kindLabel: 'Tipo',
			baseUrlLabel: 'URL base',
			baseUrlPlaceholder: 'http://localhost:11434/v1',
			modelLabel: 'Modelo',
			modelPlaceholderOpenAiCompatible: 'qwen2.5:32b-instruct',
			modelPlaceholderOther: 'claude-opus-5',
			apiKeyLabel: 'Clave de API',
			apiKeyOptional: '(opcional)',
			showApiKey: 'Mostrar clave de API',
			hideApiKey: 'Ocultar clave de API',
			timeoutLabel: 'Tiempo de espera (ms)',
			// `defaultMs` is passed in already formatted (the caller's
			// `DEFAULT_TIMEOUT_MS.toLocaleString()`) so this dictionary never
			// has to make its own locale-formatting decision.
			timeoutHint: (defaultMs: string) =>
				`Un modelo local en frío puede tardar unos 16.000 ms en su primera solicitud del día — el valor predeterminado de ${defaultMs} ms evita entonces un falso tiempo de espera.`,
			timeoutTooLow: (minMs: number) => `El tiempo de espera debe ser de al menos ${minMs} ms.`,
			saveError: 'No se pudo guardar el backend',
			saveFailedFallback: 'No se pudo guardar.',
			addSubmit: 'Añadir backend',
			saveSubmit: 'Guardar cambios'
		}
	},

	hotkeyRecorder: {
		prompt: 'Pulsar una combinación…',
		placeholder: 'Clic para configurar un hotkey',
		hintModifier: 'Mantenga pulsada una tecla modificadora (Ctrl, Alt, Mayús o Win) y pulse una tecla.',
		recordingStatus: 'Grabando hotkey. Pulse una combinación de teclas, o Escape para cancelar.',
		ariaHotkey: (value: string) => `Hotkey: ${value}. Clic para cambiar.`,
		ariaEmpty: 'No hay ningún hotkey configurado. Clic para grabarlo.'
	},

	// New section (mockup-a-compact.html): run history.
	history: {
		title: 'Historial',
		// `limit` is the retention cap (100 — see `history::MAX_ENTRIES` in
		// src-tauri/src/history.rs), passed in rather than hard-coded so this
		// stays correct if that constant ever changes.
		hint: (limit: number) => `últimas ${limit} ejecuciones`,
		clear: 'Borrar',
		columnTime: 'Hora',
		columnCommand: 'Comando',
		columnBackend: 'Backend',
		columnOutcome: 'Resultado',
		columnDuration: 'Duración',
		statusSuccess: 'Correcto',
		statusFailed: 'Fallido',
		statusAborted: 'Cancelado',
		emptyTitle: 'Aún no hay ejecuciones',
		emptyDescription: 'Ejecute un comando con su hotkey y el resultado aparecerá aquí.'
	},

	// New section (mockup-a-compact.html): theme / language / autostart.
	settings: {
		title: 'Ajustes',
		themeLabel: 'Tema',
		themeDescription: 'Se aplica a esta ventana. Sigue la configuración de Windows de forma predeterminada.',
		themeSystem: 'Sistema',
		themeLight: 'Claro',
		themeDark: 'Oscuro',
		languageLabel: 'Idioma',
		languageDescription: 'Idioma de la interfaz para esta ventana.',
		languageSystem: 'Sistema',
		autostartLabel: 'Iniciar con Windows',
		autostartDescription: 'Inicia Clipboard AI minimizado en el área de notificación al iniciar sesión.'
	}
};
