import { describe, expect, it } from 'vitest';
import {
	commandMatches,
	knownReasonSentence,
	nextIndex,
	paletteRows,
	reasonSentence,
	stoppedTitle
} from '../../src/lib/overlay';
import { registry } from '../../src/lib/i18n/registry';
import { en } from '../../src/lib/i18n/en';
import type { PaletteCommand } from '../../src/lib/types';

const commands: [PaletteCommand, PaletteCommand, PaletteCommand, PaletteCommand] = [
	{ id: 'c1', name: 'Fix grammar', hotkey: 'Ctrl+Alt+J', backendName: 'Claude' },
	{ id: 'c2', name: 'Make it concise', hotkey: 'Ctrl+Alt+K', backendName: 'Claude' },
	{ id: 'c3', name: 'Übersetzen ins Englische', hotkey: '', backendName: 'Ollama' },
	{ id: 'c4', name: 'Translate to German', hotkey: 'Ctrl+Alt+G', backendName: 'Ollama' }
];

const CYCLE_REASONS = [
	'cancelled',
	'focus-changed',
	'focus-restore-failed',
	'capture-timeout',
	'modifiers-stuck',
	'modifiers-stuck-before-paste',
	'backend-error',
	'no-sentinel',
	'empty-result',
	'clipboard-write-failed',
	'paste-failed',
	'no-last-result',
	'palette-focus-failed'
];

describe('palette rows', () => {
	it('lists every command in config order when the search is empty', () => {
		const rows = paletteRows(commands, '', true);
		expect(rows.map((r) => (r.kind === 'command' ? r.command.id : r.kind))).toEqual([
			'c1',
			'c2',
			'c3',
			'c4'
		]);
	});

	it('offers the text as an instruction first when no command matches', () => {
		const rows = paletteRows(commands, 'make it friendlier', true);
		expect(rows).toEqual([{ kind: 'instruction', text: 'make it friendlier' }]);
	});

	it('puts matching commands first so Enter runs the command, with the instruction last', () => {
		const rows = paletteRows(commands, 'fix', true);
		expect(rows[0]).toMatchObject({ kind: 'command', command: { id: 'c1' } });
		expect(rows.at(-1)).toEqual({ kind: 'instruction', text: 'fix' });
	});

	it('ranks names that start with the query above names that merely contain it', () => {
		const rows = paletteRows(commands, 'ger', true);
		const ids = rows.flatMap((r) => (r.kind === 'command' ? [r.command.id] : []));
		expect(ids).toEqual(['c4']);
		const t = paletteRows(commands, 't', true).flatMap((r) =>
			r.kind === 'command' ? [r.command.id] : []
		);
		expect(t[0]).toBe('c4');
	});

	it('never offers an instruction without an AI backend to run it', () => {
		expect(paletteRows(commands, 'make it friendlier', false)).toEqual([]);
		expect(paletteRows(commands, 'fix', false).every((r) => r.kind === 'command')).toBe(true);
	});

	it('matches every word, ignoring case and accents', () => {
		expect(commandMatches(commands[2], 'ubersetzen engl')).toBe(true);
		expect(commandMatches(commands[1], 'MAKE concise')).toBe(true);
		expect(commandMatches(commands[1], 'make grammar')).toBe(false);
	});

	it('wraps arrow-key movement at both ends', () => {
		expect(nextIndex(3, 1, 4)).toBe(0);
		expect(nextIndex(0, -1, 4)).toBe(3);
		expect(nextIndex(0, 1, 0)).toBe(0);
	});
});

describe('stop reasons', () => {
	it('turns every reason the cycle records into a plain sentence in every language', () => {
		for (const [code, entry] of Object.entries(registry)) {
			for (const reason of CYCLE_REASONS) {
				const sentence = knownReasonSentence(reason, entry.dictionary);
				expect(sentence, `${code} ${reason}`).toBeTruthy();
				expect(sentence, `${code} ${reason}`).toMatch(/^\p{Lu}.*\.$/u);
			}
		}
	});

	it('falls back to the raw code for a reason it does not know', () => {
		expect(knownReasonSentence('from-the-future', en)).toBeNull();
		expect(reasonSentence('from-the-future', en)).toContain('from-the-future');
	});

	it('titles a cancel and a missing result differently from a failed paste', () => {
		expect(stoppedTitle('cancelled', en)).toBe(en.hud.cancelled);
		expect(stoppedTitle('no-last-result', en)).toBe(en.hud.nothingToPaste);
		expect(stoppedTitle('focus-changed', en)).toBe(en.hud.nothingPasted);
	});
});

describe('new overlay strings', () => {
	function strings(value: unknown): string[] {
		if (typeof value === 'string') return [value];
		if (typeof value === 'function') return [String(value(3)), String(value(1)), String(value('x'))];
		if (value && typeof value === 'object') return Object.values(value).flatMap(strings);
		return [];
	}

	it('use plain hyphens only, never en or em dashes', () => {
		for (const [code, entry] of Object.entries(registry)) {
			const dict = entry.dictionary;
			const s = dict.settings;
			const added = [
				dict.reasons,
				dict.hud,
				dict.palette,
				dict.history.statusCancelled,
				dict.history.instructionTag,
				s.groupRunning,
				s.hudLabel,
				s.hudDescription,
				s.busyCursorLabel,
				s.busyCursorDescription,
				s.groupShortcuts,
				s.paletteLabel,
				s.paletteDescription,
				s.pasteLastLabel,
				s.pasteLastDescription,
				s.pasteLastToggle
			];
			for (const text of added.flatMap(strings)) {
				expect(text, code).not.toMatch(/[–—]/);
			}
		}
	});

	it('pluralise the word count', () => {
		expect(en.hud.wordsChanged(1)).toBe('1 word changed');
		expect(en.hud.wordsChanged(4)).toBe('4 words changed');
	});
});
