import type { AppRule, BackendConfig, EditorError, Replacement } from '$lib/types';

export function isAiBackend(backend: BackendConfig | undefined): boolean {
	return !!backend && backend.kind !== 'http';
}

export function isClaudeBackend(backend: BackendConfig | undefined): boolean {
	return !!backend && (backend.kind === 'claude-cli' || backend.kind === 'anthropic-api');
}

export function canPreview(backend: BackendConfig | undefined): boolean {
	return !!backend && (backend.kind !== 'http' || backend.expectsResponse);
}

export function defaultWriterBackend(
	backends: BackendConfig[],
	commandBackendId: string
): string | null {
	const command = backends.find((b) => b.id === commandBackendId);
	if (isClaudeBackend(command)) return commandBackendId;
	const claude = backends.find(isClaudeBackend);
	if (claude) return claude.id;
	if (isAiBackend(command)) return commandBackendId;
	return backends.find(isAiBackend)?.id ?? null;
}

export function normalizeExe(exe: string): string {
	const trimmed = exe.trim().replace(/^"+|"+$/g, '');
	const file = trimmed.split(/[\\/]/).pop() ?? trimmed;
	return file.trim().toLowerCase();
}

export function displayExe(exe: string): string {
	const trimmed = exe.trim().replace(/^"+|"+$/g, '');
	return (trimmed.split(/[\\/]/).pop() ?? trimmed).trim();
}

export function ruleIsEmpty(rule: AppRule): boolean {
	return !rule.promptAddition.trim() && rule.backendId === null && rule.formatting === null;
}

export function hasRuleFor(rules: AppRule[], exe: string, except?: number): boolean {
	const wanted = normalizeExe(exe);
	return rules.some((r, i) => i !== except && normalizeExe(r.exe) === wanted);
}

export function addKeepWords(existing: string[], input: string): string[] {
	const next = [...existing];
	for (const raw of input.split(/\r?\n/)) {
		const word = raw.trim();
		if (word && !next.includes(word)) next.push(word);
	}
	return next;
}

export function cleanReplacement(pair: Replacement): Replacement | null {
	const from = pair.from.trim();
	if (!from) return null;
	return { from, to: pair.to.trim() };
}

export function upsertReplacement(
	list: Replacement[],
	pair: Replacement,
	at: number | null
): Replacement[] {
	const clean = cleanReplacement(pair);
	if (!clean) return list;
	const key = clean.from.toLowerCase();
	const clashes = (r: Replacement): boolean => r.from.trim().toLowerCase() === key;
	const rest = list.filter((r, i) => i !== at && !clashes(r));
	if (at === null || at < 0 || at >= list.length) return [...rest, clean];
	const position = list.slice(0, at).filter((r) => !clashes(r)).length;
	rest.splice(position, 0, clean);
	return rest;
}

export function tryInputsKey(parts: unknown[]): string {
	return JSON.stringify(parts);
}

export function iconToDataUrl(icon: { size: number; rgba: string } | null): string | null {
	if (!icon || typeof document === 'undefined') return null;
	try {
		const bytes = Uint8ClampedArray.from(atob(icon.rgba), (c) => c.charCodeAt(0));
		if (bytes.length !== icon.size * icon.size * 4) return null;
		const canvas = document.createElement('canvas');
		canvas.width = icon.size;
		canvas.height = icon.size;
		const context = canvas.getContext('2d');
		if (!context) return null;
		context.putImageData(new ImageData(bytes, icon.size, icon.size), 0, 0);
		return canvas.toDataURL('image/png');
	} catch {
		return null;
	}
}

const ERROR_KEYS = {
	timeout: 'timeout',
	unreachable: 'unreachable',
	misconfigured: 'misconfigured',
	'bad-response': 'badResponse',
	'no-sentinel': 'noSentinel',
	'empty-result': 'emptyResult',
	'nothing-to-preview': 'nothingToPreview',
	'unknown-backend': 'unknownBackend',
	'empty-input': 'emptyInput',
	'not-an-ai-backend': 'notAnAiBackend'
} as const;

export type EditorErrorKey = (typeof ERROR_KEYS)[keyof typeof ERROR_KEYS] | 'internal';

export function editorErrorKey(reason: string): EditorErrorKey {
	return (ERROR_KEYS as Record<string, EditorErrorKey>)[reason] ?? 'internal';
}

export function toEditorError(e: unknown): EditorError {
	if (e && typeof e === 'object' && 'reason' in e && typeof e.reason === 'string') {
		const detail = 'detail' in e && typeof e.detail === 'string' ? e.detail : null;
		return { reason: e.reason, detail };
	}
	return { reason: 'internal', detail: String(e) };
}

export const trySamples = new Map<string, string>();

export function inertHtml(html: string): string {
	if (typeof DOMParser === 'undefined') return '';
	const doc = new DOMParser().parseFromString(`<!doctype html><body>${html}</body>`, 'text/html');
	for (const element of Array.from(doc.body.querySelectorAll('*'))) {
		for (const attribute of Array.from(element.attributes)) {
			const name = attribute.name.toLowerCase();
			if (name === 'class' || name === 'id' || name === 'target' || name.startsWith('on')) {
				element.removeAttribute(attribute.name);
			}
		}
		if (element.tagName === 'A') element.removeAttribute('href');
	}
	return doc.body.innerHTML;
}
