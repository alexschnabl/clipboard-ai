import { describe, expect, it } from 'vitest';
import { wordDiff } from '../../src/lib/diff';

function render(before: string, after: string): string {
	return (wordDiff(before, after) ?? [])
		.map((p) => (p.kind === 'same' ? p.text : p.kind === 'added' ? `[+${p.text}]` : `[-${p.text}]`))
		.join('');
}

describe('wordDiff', () => {
	it('marks a replaced word as removed then added', () => {
		expect(render('I will send it tomorow.', 'I will send it tomorrow.')).toBe(
			'I will send it [-tomorow.][+tomorrow.]'
		);
	});

	it('keeps the spacing around a removed word', () => {
		expect(render('send the the report', 'send the report')).toBe('send the [-the] report');
	});

	it('returns only unchanged parts for identical text', () => {
		expect(wordDiff('same text', 'same text')).toEqual([{ kind: 'same', text: 'same text' }]);
	});

	it('handles empty sides', () => {
		expect(render('', 'new words')).toBe('[+new] [+words]');
		expect(render('old', '')).toBe('[-old]');
	});

	it('gives up on very large inputs', () => {
		const big = Array.from({ length: 700 }, (_, i) => `w${i}`).join(' ');
		expect(wordDiff(big, big + ' end')).toBeNull();
	});
});
