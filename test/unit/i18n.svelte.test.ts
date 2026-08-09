import { afterEach, describe, expect, it } from 'vitest';

import { i18n } from '../../src/lib/i18n/i18n.svelte';
import { en } from '../../src/lib/i18n/en';
import { registry, FALLBACK_LOCALE, type Locale } from '../../src/lib/i18n/registry';

const registeredLocales = Object.keys(registry) as Locale[];

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
		expect(i18n.locale).toBe('en');
	});

	it('resolves "system" to English when navigator.language is the empty string', () => {
		stubNavigatorLanguage('');
		i18n.setLanguage('system');
		expect(i18n.locale).toBe('en');
	});

	it('an explicit choice overrides whatever the system locale would have resolved to', () => {
		for (const code of registeredLocales) {
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
		if (other === undefined) return;
		i18n.setLanguage(other);
		expect(i18n.t.nav.settings).not.toBe(first);
	});
});

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

	expect(typeof a, `${path}: expected a string leaf in en`).toBe('string');
	expect(typeof b, `${path}: expected a string leaf in the other dictionary`).toBe('string');
}

describe('registered-dictionary structural parity (runtime check complementing the compile-time one)', () => {
	for (const [code, entry] of Object.entries(registry)) {
		if (code === 'en') continue;

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
});
