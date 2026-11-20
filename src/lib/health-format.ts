import type { en } from '$lib/i18n/en';
import type { Health } from '$lib/types';
import type { HealthResult } from '$lib/state/health.svelte';

export type Tone = 'ok' | 'warn' | 'error' | 'busy' | 'idle';

type HealthStrings = (typeof en)['health'];

export function formatSeconds(ms: number, locale?: string): string {
	const seconds = ms / 1000;
	const digits = seconds < 10 ? 1 : 0;
	const n = new Intl.NumberFormat(locale, {
		minimumFractionDigits: digits,
		maximumFractionDigits: digits
	}).format(seconds);
	return `${n} s`;
}

export function cliVersion(raw: string): string {
	const match = raw.match(/\d+(\.\d+)+/);
	return match ? match[0] : raw.trim();
}

export function healthTone(result: HealthResult | undefined, checking = false): Tone {
	if (checking) return 'busy';
	if (!result) return 'idle';
	if (!result.health) return 'error';
	switch (result.health.status) {
		case 'ok':
		case 'cold':
		case 'reachable':
		case 'cliReady':
			return 'ok';
		case 'slow':
		case 'cliUnknownAuth':
			return 'warn';
		default:
			return 'error';
	}
}

function misconfigured(detail: string, t: HealthStrings): string {
	if (/baseUrl is required|no URL set|url is required/i.test(detail)) return t.needsUrl;
	if (/model is required/i.test(detail)) return t.needsModel;
	if (/API key/i.test(detail)) return t.keyUnreadable;
	return t.misconfigured;
}

export function healthSentence(health: Health, t: HealthStrings, locale?: string): string {
	switch (health.status) {
		case 'ok':
			return t.ok(formatSeconds(health.ms, locale));
		case 'cold':
			return t.cold(formatSeconds(health.firstMs, locale), formatSeconds(health.warmMs, locale));
		case 'slow':
			return t.slow(formatSeconds(health.ms, locale));
		case 'reachable':
			return t.reachable;
		case 'cliReady':
			return t.cliReady(cliVersion(health.version));
		case 'cliSignedOut':
			return t.cliSignedOut;
		case 'cliUnknownAuth':
			return t.cliUnknownAuth(cliVersion(health.version));
		case 'cliMissing':
			return t.cliMissing;
		case 'keyMissing':
			return t.keyMissing;
		case 'keyRejected':
			return t.keyRejected;
		case 'modelNotFound':
			return t.modelNotFound;
		case 'notReachable':
			return t.notReachable;
		case 'timeout':
			return t.timeout(health.secs);
		case 'misconfigured':
			return misconfigured(health.detail, t);
		case 'badResponse':
			return t.badResponse;
	}
}

export function healthDetail(result: HealthResult | undefined): string | undefined {
	if (!result) return undefined;
	if (result.error) return result.error;
	const h = result.health;
	if (h && 'detail' in h) return h.detail;
	return undefined;
}

export function resultSentence(
	result: HealthResult | undefined,
	checking: boolean,
	t: HealthStrings,
	locale?: string
): string {
	if (checking) return t.checking;
	if (!result) return t.notChecked;
	if (!result.health) return t.checkFailed;
	return healthSentence(result.health, t, locale);
}
