import { describe, expect, it } from 'vitest';
import { hotkeyFromEvent } from '../../src/lib/utils/hotkey';

function ev(init: Partial<KeyboardEvent>): KeyboardEvent {
	return { ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...init } as KeyboardEvent;
}

describe('hotkeyFromEvent', () => {
	it('maps a single modifier plus a letter', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'j', code: 'KeyJ' }))).toBe('Ctrl+J');
	});

	it('normalises modifier order regardless of which was pressed first', () => {
		const a = hotkeyFromEvent(ev({ altKey: true, ctrlKey: true, key: 'j', code: 'KeyJ' }));
		const b = hotkeyFromEvent(ev({ ctrlKey: true, altKey: true, key: 'j', code: 'KeyJ' }));
		expect(a).toBe('Ctrl+Alt+J');
		expect(b).toBe('Ctrl+Alt+J');
	});

	it('uppercases the letter', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'j', code: 'KeyJ' }))).toBe('Ctrl+J');
	});

	it('includes Shift and Super in the fixed order', () => {
		const result = hotkeyFromEvent(
			ev({ ctrlKey: true, altKey: true, shiftKey: true, metaKey: true, key: 'k', code: 'KeyK' })
		);
		expect(result).toBe('Ctrl+Alt+Shift+Super+K');
	});

	it('returns null while only a modifier is held', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'Control', code: 'ControlLeft' }))).toBeNull();
		expect(hotkeyFromEvent(ev({ altKey: true, key: 'Alt', code: 'AltLeft' }))).toBeNull();
		expect(hotkeyFromEvent(ev({ shiftKey: true, key: 'Shift', code: 'ShiftLeft' }))).toBeNull();
		expect(hotkeyFromEvent(ev({ metaKey: true, key: 'Meta', code: 'MetaLeft' }))).toBeNull();
	});

	it('rejects a bare key with no modifier', () => {
		expect(hotkeyFromEvent(ev({ key: 'j', code: 'KeyJ' }))).toBeNull();
	});

	it('maps digits from the code, not the shifted character', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, shiftKey: true, key: '!', code: 'Digit1' }))).toBe(
			'Ctrl+Shift+1'
		);
	});

	it('maps function keys', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'F5', code: 'F5' }))).toBe('Ctrl+F5');
	});

	it('returns null for a key it cannot map rather than guessing', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'Unidentified', code: 'Lang1' }))).toBeNull();
	});
});

// Every mapping below is cross-checked against the actual parser, not guessed:
// `global-hotkey` 0.8.0 `src/hotkey.rs` `parse_key()` (the function
// `tauri-plugin-global-shortcut` 2.3.2 uses via `Shortcut::from_str`, since
// `Shortcut` is a type alias for `global_hotkey::hotkey::HotKey`). Each
// right-hand spelling below is one of the literal match arms `parse_key`
// accepts (case-insensitively).
describe('keyFromCode table verified against global-hotkey 0.8.0 parse_key()', () => {
	const namedKeyCases: Array<[code: string, expected: string]> = [
		['Comma', 'Comma'],
		['Period', 'Period'],
		['Slash', 'Slash'],
		['Semicolon', 'Semicolon'],
		['Quote', 'Quote'],
		['BracketLeft', 'BracketLeft'],
		['BracketRight', 'BracketRight'],
		['Backslash', 'Backslash'],
		['Minus', 'Minus'],
		['Equal', 'Equal'],
		['Backquote', 'Backquote'],
		['Space', 'Space'],
		['Enter', 'Enter'],
		['Tab', 'Tab'],
		['Backspace', 'Backspace'],
		['Delete', 'Delete'],
		['Insert', 'Insert'],
		['Home', 'Home'],
		['End', 'End'],
		['PageUp', 'PageUp'],
		['PageDown', 'PageDown'],
		// The DOM code is "ArrowUp"/"ArrowDown"/"ArrowLeft"/"ArrowRight", but
		// parse_key only recognises "ARROWUP" or its alias "UP" (etc.) --
		// both work, since Rust's HotKey::into_string() always prints the
		// short form and that must round-trip through the same parser.
		['ArrowUp', 'Up'],
		['ArrowDown', 'Down'],
		['ArrowLeft', 'Left'],
		['ArrowRight', 'Right']
	];

	it.each(namedKeyCases)('maps code %s to the accepted spelling %s', (code, expected) => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: code, code }))).toBe(`Ctrl+${expected}`);
	});

	it('accepts the full function-key range the parser supports (F1-F24)', () => {
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'F1', code: 'F1' }))).toBe('Ctrl+F1');
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'F24', code: 'F24' }))).toBe('Ctrl+F24');
	});

	it('rejects a function-key code outside the range the parser accepts', () => {
		// The parser's match arms stop at F24; a hypothetical F25 key must
		// not be guessed into a hotkey the plugin would then fail to parse.
		expect(hotkeyFromEvent(ev({ ctrlKey: true, key: 'F25', code: 'F25' }))).toBeNull();
	});
});
