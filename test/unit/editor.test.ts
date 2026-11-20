import { describe, expect, it } from 'vitest';
import {
	addKeepWords,
	canPreview,
	defaultWriterBackend,
	displayExe,
	editorErrorKey,
	hasRuleFor,
	normalizeExe,
	ruleIsEmpty,
	toEditorError,
	upsertReplacement
} from '../../src/lib/editor';
import type { AppRule, BackendConfig, BackendKind } from '../../src/lib/types';

function backend(id: string, kind: BackendKind, expectsResponse = true): BackendConfig {
	return {
		id,
		name: id,
		kind,
		baseUrl: null,
		model: null,
		apiKey: null,
		timeoutMs: 120_000,
		effort: 'low',
		expectsResponse
	};
}

function rule(exe: string, overrides: Partial<AppRule> = {}): AppRule {
	return { exe, name: '', promptAddition: '', backendId: null, formatting: null, ...overrides };
}

describe('defaultWriterBackend', () => {
	const ollama = backend('ollama', 'openai-compatible');
	const cli = backend('cli', 'claude-cli');
	const api = backend('api', 'anthropic-api');
	const hook = backend('hook', 'http');

	it('keeps the command backend when it is already a Claude backend', () => {
		expect(defaultWriterBackend([cli, api], 'api')).toBe('api');
	});

	it('prefers any Claude backend over the command backend', () => {
		expect(defaultWriterBackend([ollama, cli], 'ollama')).toBe('cli');
	});

	it('falls back to the command backend when there is no Claude backend', () => {
		expect(defaultWriterBackend([hook, ollama], 'ollama')).toBe('ollama');
	});

	it('never picks an HTTP backend', () => {
		expect(defaultWriterBackend([hook, ollama], 'hook')).toBe('ollama');
		expect(defaultWriterBackend([hook], 'hook')).toBeNull();
	});
});

describe('canPreview', () => {
	it('previews AI backends and HTTP backends that answer, but not send-only ones', () => {
		expect(canPreview(backend('a', 'claude-cli'))).toBe(true);
		expect(canPreview(backend('b', 'http', true))).toBe(true);
		expect(canPreview(backend('c', 'http', false))).toBe(false);
		expect(canPreview(undefined)).toBe(false);
	});
});

describe('exe names', () => {
	it('normalizes to a lowercase file name and keeps the display spelling', () => {
		expect(normalizeExe(' OUTLOOK.EXE ')).toBe('outlook.exe');
		expect(normalizeExe('"C:\\Program Files\\Microsoft Office\\OUTLOOK.EXE"')).toBe('outlook.exe');
		expect(displayExe('C:\\Apps\\Code.exe')).toBe('Code.exe');
	});

	it('finds an existing rule for the same app, ignoring the one being edited', () => {
		const rules = [rule('OUTLOOK.EXE'), rule('Code.exe')];
		expect(hasRuleFor(rules, 'outlook.exe')).toBe(true);
		expect(hasRuleFor(rules, 'outlook.exe', 0)).toBe(false);
		expect(hasRuleFor(rules, 'notepad.exe')).toBe(false);
	});

	it('treats a rule that overrides nothing as empty', () => {
		expect(ruleIsEmpty(rule('a.exe', { promptAddition: '   ' }))).toBe(true);
		expect(ruleIsEmpty(rule('a.exe', { formatting: 'plain' }))).toBe(false);
		expect(ruleIsEmpty(rule('a.exe', { backendId: 'b' }))).toBe(false);
	});
});

describe('addKeepWords', () => {
	it('adds trimmed words, one per line, without duplicates', () => {
		expect(addKeepWords(['Tauri'], '  Villach \nTauri\n\nSus-Tech\r\n')).toEqual([
			'Tauri',
			'Villach',
			'Sus-Tech'
		]);
	});

	it('ignores blank input', () => {
		expect(addKeepWords(['a'], '   ')).toEqual(['a']);
	});
});

describe('upsertReplacement', () => {
	const list = [
		{ from: 'teh', to: 'the' },
		{ from: 'sus tech', to: 'Sus-Tech' },
		{ from: 'iphone', to: 'iPhone' }
	];

	it('appends a new pair, trimmed', () => {
		expect(upsertReplacement(list, { from: ' adn ', to: ' and ' }, null)).toEqual([
			...list,
			{ from: 'adn', to: 'and' }
		]);
	});

	it('replaces an existing pair in place', () => {
		expect(upsertReplacement(list, { from: 'sus tech', to: 'SUS Tech' }, 1)).toEqual([
			list[0],
			{ from: 'sus tech', to: 'SUS Tech' },
			list[2]
		]);
	});

	it('drops another pair with the same source instead of keeping a duplicate', () => {
		expect(upsertReplacement(list, { from: 'TEH', to: 'The' }, null)).toEqual([
			list[1],
			list[2],
			{ from: 'TEH', to: 'The' }
		]);
		expect(upsertReplacement(list, { from: 'Teh', to: 'the' }, 2)).toEqual([
			list[1],
			{ from: 'Teh', to: 'the' }
		]);
	});

	it('ignores a pair without a source', () => {
		expect(upsertReplacement(list, { from: '  ', to: 'x' }, null)).toBe(list);
	});
});

describe('editor errors', () => {
	it('maps backend reasons to message keys and unknown ones to internal', () => {
		expect(editorErrorKey('no-sentinel')).toBe('noSentinel');
		expect(editorErrorKey('bad-response')).toBe('badResponse');
		expect(editorErrorKey('timeout')).toBe('timeout');
		expect(editorErrorKey('something-new')).toBe('internal');
	});

	it('normalizes thrown values', () => {
		expect(toEditorError({ reason: 'timeout', detail: null })).toEqual({
			reason: 'timeout',
			detail: null
		});
		expect(toEditorError({ reason: 'unreachable', detail: 'refused' }).detail).toBe('refused');
		expect(toEditorError('boom')).toEqual({ reason: 'internal', detail: 'boom' });
	});
});
