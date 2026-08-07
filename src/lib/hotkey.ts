/** Keys that are modifiers themselves — pressing one is never a complete combination. */
export const MODIFIER_KEYS: ReadonlySet<string> = new Set([
	'Control',
	'Alt',
	'Shift',
	'Meta',
	'AltGraph'
]);

/**
 * Convert a browser keydown into the shortcut spelling Rust expects, or `null`
 * when the event is not (yet) a usable combination.
 *
 * `null` means "do not register this" — never fall back to a guess. A spelling
 * the plugin does not accept produces a hotkey that silently does nothing.
 */
export function hotkeyFromEvent(event: KeyboardEvent): string | null {
	if (MODIFIER_KEYS.has(event.key)) return null;

	const parts: string[] = [];
	if (event.ctrlKey) parts.push('Ctrl');
	if (event.altKey) parts.push('Alt');
	if (event.shiftKey) parts.push('Shift');
	if (event.metaKey) parts.push('Super');

	// A bare key would swallow that key system-wide.
	if (parts.length === 0) return null;

	const key = keyFromCode(event.code);
	if (key === null) return null;

	parts.push(key);
	return parts.join('+');
}

/**
 * Derive the key name from `event.code` rather than `event.key`.
 *
 * `event.key` is the *produced character*, so Ctrl+Shift+1 reports `!` and a
 * non-US layout reports whatever that layout produces. `code` is the physical
 * key and matches what the OS-level shortcut registration sees.
 *
 * Every spelling returned here was checked against the actual parser rather
 * than guessed: `global-hotkey` 0.8.0 `src/hotkey.rs` `parse_key()` (reached
 * via `tauri-plugin-global-shortcut` 2.3.2's `Shortcut::from_str`, since
 * `Shortcut` is a type alias for `global_hotkey::hotkey::HotKey`). That
 * function upper-cases its input, so a DOM `code` that matches an arm
 * case-insensitively can be passed straight through.
 */
function keyFromCode(code: string): string | null {
	if (/^Key[A-Z]$/.test(code)) return code.slice(3);
	if (/^Digit[0-9]$/.test(code)) return code.slice(5);
	if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;

	if (IDENTITY_CODES.has(code)) return code;
	return ALIASED_CODES[code] ?? null;
}

/**
 * Codes whose DOM name is already one of `parse_key`'s match arms, so they
 * pass through unchanged.
 *
 * Media keys (`AudioVolumeMute`, `MediaPlayPause`, …) are also accepted by the
 * parser but deliberately left out: they are pressed without modifiers, and
 * `hotkeyFromEvent` requires one, so they could never reach here anyway.
 */
const IDENTITY_CODES: ReadonlySet<string> = new Set([
	// Punctuation
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
	// Editing and navigation
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
	// Numeric keypad — a common choice for global hotkeys precisely because
	// these keys rarely collide with an application's own shortcuts.
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
	// Locks and system keys
	'CapsLock',
	'NumLock',
	'ScrollLock',
	'PrintScreen',
	'Pause'
]);

/**
 * Codes the parser accepts only under a different name. The DOM says
 * `ArrowUp`; `parse_key` matches `"ARROWUP"` or the alias `"UP"`, and Rust's
 * `HotKey::into_string()` prints the short form — so the short form is what
 * round-trips.
 */
const ALIASED_CODES: Record<string, string> = {
	ArrowUp: 'Up',
	ArrowDown: 'Down',
	ArrowLeft: 'Left',
	ArrowRight: 'Right'
};
