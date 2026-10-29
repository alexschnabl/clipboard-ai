import type { en } from './en';

export const es: typeof en = {
	nav: {
		commands: 'Comandos',
		backends: 'Backends',
		history: 'Historial',
		settings: 'Ajustes',
		working: 'En curso…'
	},

	common: {
		cancel: 'Cancelar',
		delete: 'Eliminar',
		saving: 'Guardando…'
	},

	app: {
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
		count: (n: number) => `${n} configurado${n === 1 ? '' : 's'}`,
		addCommand: 'Añadir comando',
		columnName: 'Nombre',
		columnHotkey: 'Hotkey',
		columnBackend: 'Backend',
		columnActions: 'Acciones',
		empty: 'No hay comandos configurados.',
		unknownBackend: 'Backend desconocido',
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
			promptHttpHint:
				'El backend seleccionado es un punto de conexión HTTP simple. El prompt no se envía con la petición.',
			promptSentinelHint:
				'La aplicación pide automáticamente al modelo que devuelva solo el texto transformado y nada más, así que no hace falta indicarlo en el prompt.',
			formattingLabel: 'Formato',
			formattingOptionAuto: 'Conservar el formato de la selección',
			formattingOptionRich: 'Devolver siempre texto con formato',
			formattingOptionPlain: 'Solo texto sin formato',
			formattingHintAuto:
				'La negrita, las listas, los enlaces y las imágenes de la selección se conservan. Una selección de texto sin formato se mantiene sin formato.',
			formattingHintRich:
				'Se pide al modelo que responda con títulos, listas y énfasis aunque la selección sea texto sin formato. Para prompts que mejoran el formato.',
			formattingHintPlain:
				'Solo se envía y se pega el texto. El formato y las imágenes se pierden.',
			saveError: 'No se pudo guardar el comando',
			saveFailedFallback: 'No se pudo guardar.',
			addSubmit: 'Añadir comando',
			saveSubmit: 'Guardar cambios'
		}
	},

	backends: {
		title: 'Backends',
		count: (n: number) => `${n} configurado${n === 1 ? '' : 's'}`,
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
		form: {
			addTitle: 'Añadir backend',
			editTitle: 'Editar backend',
			addDescription: 'Configurar un nuevo backend de IA.',
			editDescription: 'Actualizar los datos de conexión de este backend.',
			nameLabel: 'Nombre',
			namePlaceholder: 'Ollama local',
			kindLabel: 'Tipo',
			baseUrlLabel: 'URL base',
			modelLabel: 'Modelo',
			modelPlaceholderOpenAiCompatible: 'qwen2.5:32b-instruct',
			modelPlaceholderOther: 'claude-opus-5',
			modelPlaceholderClaudeCli: 'haiku',
			modelHintClaudeCli:
				'Opcional. Un modelo más pequeño como haiku responde más rápido; en blanco usa el predeterminado de la CLI.',
			effortLabel: 'Esfuerzo',
			effortOptionDefault: 'Predeterminado',
			effortOptionLow: 'Bajo',
			effortOptionMedium: 'Medio',
			effortOptionHigh: 'Alto',
			effortOptionXhigh: 'Muy alto',
			effortOptionMax: 'Máximo',
			effortHint:
				'Un esfuerzo más bajo responde más rápido y basta para textos cortos. Predeterminado deja que la CLI decida.',
			httpUrlHint:
				'El texto seleccionado se envía a esta URL como cuerpo de la petición POST. El prompt del comando no se envía.',
			expectResponseLabel: 'Esperar una respuesta',
			expectResponseHint:
				'Activado pega el cuerpo de la respuesta sobre la selección. Desactivado solo envía.',
			apiKeyLabel: 'Clave de API',
			apiKeyOptional: '(opcional)',
			showApiKey: 'Mostrar clave de API',
			hideApiKey: 'Ocultar clave de API',
			timeoutLabel: 'Tiempo de espera (ms)',
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

	history: {
		title: 'Historial',
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
		emptyDescription: 'Ejecute un comando con su hotkey y el resultado aparecerá aquí.',
		loadErrorTitle: 'No se pudo cargar el historial',
		copyErrorTitle: 'No se pudo copiar al portapapeles',
		expandAria: (name: string) => `Expandir detalles de ${name}`,
		collapseAria: (name: string) => `Contraer detalles de ${name}`,
		abortedPrefix: 'Cancelado:',
		failedPrefix: 'Fallido:',
		inputLabel: 'Entrada',
		resultLabel: 'Resultado',
		noInputCaptured: 'no se capturó nada en esta ejecución',
		noResultPasted: 'no se pegó nada en esta ejecución',
		copy: 'Copiar',
		copied: 'Copiado',
		copyResult: 'Copiar resultado',
		clearTitle: '¿Borrar el historial?',
		clearDescription: (count: number) =>
			`Elimina permanentemente las ${count} ejecuciones registradas, incluido cualquier texto de entrada y resultado que aún se conserve aquí para su recuperación. Esta acción no se puede deshacer.`
	},

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
		autostartDescription: 'Inicia Clipboard AI minimizado en el área de notificación al iniciar sesión.',
		autostartDevNoteBefore:
			'Rechazado en compilaciones de desarrollo: registraría el ejecutable de depuración para iniciarse al iniciar sesión en lugar de la aplicación instalada; pruebe esto con una compilación instalada (',
		autostartDevNoteAfter: ').',
		saveErrorTitle: 'No se pudo guardar ese cambio',
		saveFailedFallback: 'No se pudo guardar el cambio.'
	}
};
