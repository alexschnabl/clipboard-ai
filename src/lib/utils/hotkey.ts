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
 * function upper-cases its input and accepts the exact identifiers below
 * (some, like the arrow keys, only under a short alias — `"UP"`, not
 * `"ARROWUP"`'s literal DOM name — which is why those four differ from the
 * `code` value).
 */
function keyFromCode(code: string): string | null {
	if (/^Key[A-Z]$/.test(code)) return code.slice(3);
	if (/^Digit[0-9]$/.test(code)) return code.slice(5);
	if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;

	const named: Record<string, string> = {
		Space: 'Space',
		Enter: 'Enter',
		Tab: 'Tab',
		Backspace: 'Backspace',
		Delete: 'Delete',
		Insert: 'Insert',
		Home: 'Home',
		End: 'End',
		PageUp: 'PageUp',
		PageDown: 'PageDown',
		ArrowUp: 'Up',
		ArrowDown: 'Down',
		ArrowLeft: 'Left',
		ArrowRight: 'Right',
		Comma: 'Comma',
		Period: 'Period',
		Slash: 'Slash',
		Semicolon: 'Semicolon',
		Quote: 'Quote',
		BracketLeft: 'BracketLeft',
		BracketRight: 'BracketRight',
		Backslash: 'Backslash',
		Minus: 'Minus',
		Equal: 'Equal',
		Backquote: 'Backquote'
	};
	return named[code] ?? null;
}
