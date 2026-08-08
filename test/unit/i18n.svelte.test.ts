// test/unit/i18n.svelte.test.ts — locale resolution (system detection,
// explicit override) plus a runtime structural-parity check between every
// registered dictionary and `en` that complements the compile-time one
// (`xx: typeof en` in each dictionary file). The compile-time check alone
// would not catch a mismatch smuggled in via an `any`/type-assertion escape
// hatch somewhere down the line; this walks the actual runtime objects and
// can't be bypassed that way.
//
// Deliberately written against `registry` generically -- looping over
// `Object.entries(registry)` -- rather than hard-coding `en`/`de`, so this
// file does not need editing the next time a language is added; only
// registry.ts does. See that file's header comment for the add-a-language
// recipe this test exists to keep honest.
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
import { registry, FALLBACK_LOCALE, type Locale } from '../../src/lib/i18n/registry';

const registeredLocales = Object.keys(registry) as Locale[];

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
	it('every registered locale resolves "system" from its own bare and regional tags', () => {
		for (const code of registeredLocales) {
			for (const tag of [code, `${code}-XX`, code.toUpperCase()]) {
				stubNavigatorLanguage(tag);
				i18n.setLanguage('system');
				expect(i18n.locale, `expected ${tag} to resolve to ${code}`).toBe(code);
			}
		}
	});

	it('resolves "system" to English for a navigator locale matching no registered language', () => {
		// Fabricated primary subtags guaranteed not to collide with whatever
		// is currently registered -- this must keep working no matter how
		// many real languages registry.ts grows to contain.
		const unregisteredTags = ['xx-XX', 'qq-QQ', 'zz'].filter((tag) => {
			const primary = (tag.split('-')[0] ?? tag).toLowerCase();
			return !(registeredLocales as string[]).includes(primary);
		});
		expect(unregisteredTags.length).toBeGreaterThan(0);

		for (const tag of unregisteredTags) {
			stubNavigatorLanguage(tag);
			i18n.setLanguage('system');
			expect(i18n.locale, `expected ${tag} to fall back to ${FALLBACK_LOCALE}`).toBe(
				FALLBACK_LOCALE
			);
		}
	});

	it('resolves "system" to English when navigator.language is absent', () => {
		stubNavigatorLanguage(undefined);
		i18n.setLanguage('system');
		expect(i18n.locale).toBe(FALLBACK_LOCALE);
	});

	it('an explicit choice overrides whatever the system locale would have resolved to', () => {
		for (const code of registeredLocales) {
			// Stub a system locale guaranteed to resolve to something other
			// than `code` itself, so the explicit choice is the only thing
			// that could produce `code` here. A registered sibling language
			// works; with only one language registered (`sibling` would equal
			// `code`, its only possible value), an unregistered tag -- which
			// always resolves to the fallback -- works just as well.
			const sibling = registeredLocales.find((other) => other !== code) ?? FALLBACK_LOCALE;
			const otherSystemTag = sibling === code ? 'zz-ZZ' : sibling;
			stubNavigatorLanguage(otherSystemTag);
			i18n.setLanguage(code);
			expect(i18n.locale, `explicit ${code} should override ${otherSystemTag}`).toBe(code);
		}
	});

	it('t exposes the dictionary matching the resolved locale, for every registered locale', () => {
		for (const [code, entry] of Object.entries(registry)) {
			i18n.setLanguage(code as Locale);
			expect(i18n.t.commands.addCommand).toBe(entry.dictionary.commands.addCommand);
		}
	});

	it('switching languages is reflected immediately on the next read', () => {
		i18n.setLanguage('en');
		const first = i18n.t.nav.settings;
		const other = registeredLocales.find((code) => code !== 'en');
		if (other === undefined) return; // only English registered -- nothing to switch to
		i18n.setLanguage(other);
		expect(i18n.t.nav.settings).not.toBe(first);
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
			`${path}: en and the other dictionary disagree on parameter count`
		).toBe((b as (...args: unknown[]) => unknown).length);
		return;
	}

	if (typeof a === 'object' && a !== null) {
		expect(typeof b, `${path}: en has an object, the other dictionary does not`).toBe('object');
		expect(b, `${path}: en has an object, the other dictionary does not`).not.toBeNull();

		const aKeys = Object.keys(a as Record<string, unknown>).sort();
		const bKeys = Object.keys(b as Record<string, unknown>).sort();
		expect(
			bKeys,
			`${path}: the other dictionary is missing or has extra keys compared to en`
		).toEqual(aKeys);

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
	expect(typeof b, `${path}: expected a string leaf in the other dictionary`).toBe('string');
}

describe('registered-dictionary structural parity (runtime check complementing the compile-time one)', () => {
	for (const [code, entry] of Object.entries(registry)) {
		if (code === 'en') continue; // en is the baseline everything else is compared against

		it(`${code} has exactly the same nested key structure as en`, () => {
			assertSameShape(en, entry.dictionary, '');
		});
	}

	it('every registered language has a non-empty native display name', () => {
		for (const [code, entry] of Object.entries(registry)) {
			expect(entry.nativeName.length, `${code}'s nativeName must not be empty`).toBeGreaterThan(
				0
			);
		}
	});

	it('every function leaf takes the same number of parameters in every registered dictionary', () => {
		for (const [code, entry] of Object.entries(registry)) {
			if (code === 'en') continue;
			expect(entry.dictionary.commands.editAria.length).toBe(en.commands.editAria.length);
			expect(entry.dictionary.backends.blockedDeleteDescription.length).toBe(
				en.backends.blockedDeleteDescription.length
			);
			expect(entry.dictionary.history.hint.length).toBe(en.history.hint.length);
		}
	});
});
