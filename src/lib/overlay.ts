import type { en } from './i18n/en';
import type { OverlayLook, PaletteCommand } from './types';
import { configState } from './state/config.svelte.js';

type Dictionary = typeof en;
type ReasonKey = Exclude<keyof Dictionary['reasons'], 'unknown'>;

const REASON_KEYS: Record<string, ReasonKey> = {
	cancelled: 'cancelled',
	'focus-changed': 'focusChanged',
	'focus-restore-failed': 'focusRestoreFailed',
	'capture-timeout': 'captureTimeout',
	'modifiers-stuck': 'modifiersStuck',
	'modifiers-stuck-before-paste': 'modifiersStuckBeforePaste',
	'backend-error': 'backendError',
	'no-sentinel': 'noSentinel',
	'empty-result': 'emptyResult',
	'clipboard-write-failed': 'clipboardWriteFailed',
	'paste-failed': 'pasteFailed',
	'no-last-result': 'noLastResult',
	'palette-focus-failed': 'paletteFocusFailed'
};

export function knownReasonSentence(reason: string, t: Dictionary): string | null {
	const key = REASON_KEYS[reason];
	return key ? t.reasons[key] : null;
}

export function reasonSentence(reason: string, t: Dictionary): string {
	return knownReasonSentence(reason, t) ?? t.reasons.unknown(reason);
}

export function stoppedTitle(reason: string, t: Dictionary): string {
	switch (reason) {
		case 'cancelled':
			return t.hud.cancelled;
		case 'no-last-result':
			return t.hud.nothingToPaste;
		case 'palette-focus-failed':
			return t.hud.paletteClosed;
		default:
			return t.hud.nothingPasted;
	}
}

export function applyOverlayLook(look: OverlayLook): void {
	configState.config.settings.theme = look.theme;
	configState.config.settings.language = look.language;
	configState.loading = false;
}

export type PaletteRow =
	| { kind: 'command'; command: PaletteCommand }
	| { kind: 'instruction'; text: string };

function fold(text: string): string {
	return text
		.normalize('NFD')
		.replace(/[̀-ͯ]/g, '')
		.toLowerCase();
}

export function commandMatches(command: PaletteCommand, query: string): boolean {
	const name = fold(command.name);
	return fold(query)
		.split(/\s+/)
		.filter((token) => token.length > 0)
		.every((token) => name.includes(token));
}

export function paletteRows(
	commands: PaletteCommand[],
	query: string,
	canInstruct: boolean
): PaletteRow[] {
	const text = query.trim();
	const matched = text ? commands.filter((c) => commandMatches(c, text)) : commands;
	const folded = fold(text);
	const ranked = [...matched].sort(
		(a, b) =>
			Number(!fold(a.name).startsWith(folded)) - Number(!fold(b.name).startsWith(folded))
	);
	const rows: PaletteRow[] = ranked.map((command) => ({ kind: 'command', command }));
	if (text && canInstruct) rows.push({ kind: 'instruction', text });
	return rows;
}

export function nextIndex(current: number, delta: number, length: number): number {
	if (length === 0) return 0;
	return (((current + delta) % length) + length) % length;
}
