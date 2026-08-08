// test/unit/i18n.svelte.test.ts — locale resolution (system detection,
// explicit override) plus a runtime structural-parity check between `en`
// and `de` that complements the compile-time one (`de: typeof en` in
// de.ts). The compile-time check alone would not catch a mismatch smuggled
// in via an `any`/type-assertion escape hatch somewhere down the line; this
// walks the actual runtime objects and can't be bypassed that way.
//
// Named `*.svelte.test.ts`, not plain `*.test.ts`, for the same reason
// config.svelte.test.ts is: the module under test
// (`src/lib/i18n/i18n.svelte.ts`) uses runes ($state), which only compile
// through the Svelte plugin registered in vitest.config.ts, and that
// plugin's module filter matches on the `.svelte.` infix in the filename
// (see `DEFAULT_SVELTE_MODULE_INFIX` in @sveltejs/vite-plugin-svelte).
import { afterEach, describe, expect, it } from 'vitest';

import { i18n } from '../../src/lib/i18n/i18n.svelte';
import { en } from '../../src/lib/i18n/en';
import { de } from '../../src/lib/i18n/de';

/** Replaces `navigator` for the duration of one test with a plain object
 *  exposing only `.language` -- simpler and more reliable than trying to
 *  redefine the single property on jsdom's real `Navigator` instance, whose
 *  `language` getter is not guaranteed configurable. Every test that calls
 *  this restores the original via `afterEach` below. */
function stubNavigatorLanguage(language: string | undefined): void {
	Object.defineProperty(globalThis, 'navigator', {
		value: language === undefined ? {} : { language },
		configurable: true
	});
}

const realNavigator = globalThis.navigator;

afterEach(() => {
	Object.defineProperty(globalThis, 'navigator', {
		value: realNavigator,
		configurable: true
	});
	i18n.setLanguage('system');
});

describe('i18n locale resolution', () => {
	it('resolves "system" to German for a de-* navigator locale', () => {
		stubNavigatorLanguage('de-DE');
		i18n.setLanguage('system');
		expect(i18n.locale).toBe('de');
	});

	it('resolves "system" to German for regional German variants too', () => {
		for (const tag of ['de', 'de-AT', 'de-CH', 'DE-DE']) {
			stubNavigatorLanguage(tag);
			i18n.setLanguage('system');
			expect(i18n.locale, `expected ${tag} to resolve to German`).toBe('de');
		}
	});

	it('resolves "system" to English for a non-German navigator locale', () => {
		for (const tag of ['en-US', 'fr-FR', 'ja-JP', 'en']) {
			stubNavigatorLanguage(tag);
			i18n.setLanguage('system');
			expect(i18n.locale, `expected ${tag} to resolve to English`).toBe('en');
		}
	});

	it('resolves "system" to English when navigator.language is absent', () => {
		stubNavigatorLanguage(undefined);
		i18n.setLanguage('system');
		expect(i18n.locale).toBe('en');
	});

	it('an explicit "de" choice overrides a non-German system locale', () => {
		stubNavigatorLanguage('en-US');
		i18n.setLanguage('de');
		expect(i18n.locale).toBe('de');
	});

	it('an explicit "en" choice overrides a German system locale', () => {
		stubNavigatorLanguage('de-DE');
		i18n.setLanguage('en');
		expect(i18n.locale).toBe('en');
	});

	it('t exposes the dictionary matching the resolved locale', () => {
		stubNavigatorLanguage('en-US');
		i18n.setLanguage('de');
		expect(i18n.t.commands.addCommand).toBe(de.commands.addCommand);

		i18n.setLanguage('en');
		expect(i18n.t.commands.addCommand).toBe(en.commands.addCommand);
	});

	it('switching languages is reflected immediately on the next read', () => {
		i18n.setLanguage('en');
		expect(i18n.t.nav.settings).toBe('Settings');
		i18n.setLanguage('de');
		expect(i18n.t.nav.settings).toBe('Einstellungen');
	});
});

/** Recursively walks two values built from the same dictionary shape and
 *  fails with a path pointing at the first divergence, rather than a bare
 *  boolean -- this is the test's whole point: pinpoint exactly which key a
 *  future edit broke. Functions are compared by arity only (the
 *  compile-time `typeof en` check already pins their parameter *types*;
 *  this runtime pass only needs to confirm both sides still agree that a
 *  leaf is a function, and with how many arguments). */
function assertSameShape(a: unknown, b: unknown, path: string): void {
	if (typeof a === 'function' || typeof b === 'function') {
		expect(typeof a, `${path}: expected a function on both sides`).toBe('function');
		expect(typeof b, `${path}: expected a function on both sides`).toBe('function');
		expect(
			(a as (...args: unknown[]) => unknown).length,
			`${path}: en and de disagree on parameter count`
		).toBe((b as (...args: unknown[]) => unknown).length);
		return;
	}

	if (typeof a === 'object' && a !== null) {
		expect(typeof b, `${path}: en has an object, de does not`).toBe('object');
		expect(b, `${path}: en has an object, de does not`).not.toBeNull();

		const aKeys = Object.keys(a as Record<string, unknown>).sort();
		const bKeys = Object.keys(b as Record<string, unknown>).sort();
		expect(bKeys, `${path}: de is missing or has extra keys compared to en`).toEqual(aKeys);

		for (const key of aKeys) {
			assertSameShape(
				(a as Record<string, unknown>)[key],
				(b as Record<string, unknown>)[key],
				path ? `${path}.${key}` : key
			);
		}
		return;
	}

	// Every leaf that isn't a function or a nested object must be a plain
	// string (this dictionary has no numbers, booleans or arrays) -- on
	// both sides, regardless of what the two languages' text actually says.
	expect(typeof a, `${path}: expected a string leaf in en`).toBe('string');
	expect(typeof b, `${path}: expected a string leaf in de`).toBe('string');
}

describe('en/de structural parity (runtime check complementing the compile-time one)', () => {
	it('de has exactly the same nested key structure as en', () => {
		assertSameShape(en, de, '');
	});

	it('every function leaf takes the same number of parameters in both languages', () => {
		expect(de.commands.editAria.length).toBe(en.commands.editAria.length);
		expect(de.backends.blockedDeleteDescription.length).toBe(
			en.backends.blockedDeleteDescription.length
		);
		expect(de.history.hint.length).toBe(en.history.hint.length);
	});
});
