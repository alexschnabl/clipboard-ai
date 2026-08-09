export const MODIFIER_KEYS: ReadonlySet<string> = new Set([
	'Control',
	'Alt',
	'Shift',
	'Meta',
	'AltGraph'
]);

export function hotkeyFromEvent(event: KeyboardEvent): string | null {
	if (MODIFIER_KEYS.has(event.key)) return null;

	const parts: string[] = [];
	if (event.ctrlKey) parts.push('Ctrl');
	if (event.altKey) parts.push('Alt');
	if (event.shiftKey) parts.push('Shift');
	if (event.metaKey) parts.push('Super');

	if (parts.length === 0) return null;

	const key = keyFromCode(event.code);
	if (key === null) return null;

	parts.push(key);
	return parts.join('+');
}

function keyFromCode(code: string): string | null {
	if (/^Key[A-Z]$/.test(code)) return code.slice(3);
	if (/^Digit[0-9]$/.test(code)) return code.slice(5);
	if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;

	if (IDENTITY_CODES.has(code)) return code;
	return ALIASED_CODES[code] ?? null;
}

const IDENTITY_CODES: ReadonlySet<string> = new Set([
	'Comma',
	'Period',
	'Slash',
	'Semicolon',
	'Quote',
	'BracketLeft',
	'BracketRight',
	'Backslash',
	'Minus',
	'Equal',
	'Backquote',
	'Space',
	'Enter',
	'Tab',
	'Backspace',
	'Delete',
	'Insert',
	'Home',
	'End',
	'PageUp',
	'PageDown',
	'Numpad0',
	'Numpad1',
	'Numpad2',
	'Numpad3',
	'Numpad4',
	'Numpad5',
	'Numpad6',
	'Numpad7',
	'Numpad8',
	'Numpad9',
	'NumpadAdd',
	'NumpadSubtract',
	'NumpadMultiply',
	'NumpadDivide',
	'NumpadDecimal',
	'NumpadEnter',
	'NumpadEqual',
	'CapsLock',
	'NumLock',
	'ScrollLock',
	'PrintScreen',
	'Pause'
]);

const ALIASED_CODES: Record<string, string> = {
	ArrowUp: 'Up',
	ArrowDown: 'Down',
	ArrowLeft: 'Left',
	ArrowRight: 'Right'
};
