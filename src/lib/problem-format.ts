import type { en } from '$lib/i18n/en';

export type CommandProblem =
	| { kind: 'hotkeyTaken' }
	| { kind: 'hotkeyClash'; other: string }
	| { kind: 'hotkeyInvalid' }
	| { kind: 'backendMissing' }
	| { kind: 'other'; text: string };

export function classifyCommandProblem(message: string, commandName: string): CommandProblem {
	if (/\bis already in use\b/.test(message)) return { kind: 'hotkeyTaken' };
	const clash = message.match(/collides with "(.+?)"'s hotkey/);
	if (clash) return { kind: 'hotkeyClash', other: clash[1] ?? '' };
	if (/\bunparseable hotkey\b/.test(message)) return { kind: 'hotkeyInvalid' };
	if (/: unknown backend$/.test(message)) return { kind: 'backendMissing' };
	const prefix = `${commandName}: `;
	const text = message.startsWith(prefix) ? message.slice(prefix.length) : message;
	return { kind: 'other', text: text.charAt(0).toUpperCase() + text.slice(1) };
}

export function commandProblemSentence(
	problem: CommandProblem,
	t: (typeof en)['commands']['problems']
): string {
	switch (problem.kind) {
		case 'hotkeyTaken':
			return t.hotkeyTaken;
		case 'hotkeyClash':
			return t.hotkeyClash(problem.other);
		case 'hotkeyInvalid':
			return t.hotkeyInvalid;
		case 'backendMissing':
			return t.backendMissing;
		case 'other':
			return problem.text;
	}
}
