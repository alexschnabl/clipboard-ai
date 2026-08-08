// lib/i18n/i18n.svelte.ts — the app's UI-text store. Rune-backed, matching
// this project's own convention for cross-component state (see
// src/lib/state/config.svelte.ts): a module-level `$state`, exposed through
// plain getters on a single exported object, reactive in any Svelte
// component that reads `i18n.t`/`i18n.locale` from a template or `$derived`.
//
// Consumption contract (depended on by other agents, do not rename):
//   import { i18n } from '$lib/i18n/i18n.svelte';
//   {i18n.t.commands.addCommand}
//
// `t` is a nested typed object, not a `t('some.key')` string lookup — see
// .superpowers/sdd/v3-research.md's i18n section for why a hand-rolled
// dictionary was chosen over Paraglide/svelte-i18n/typesafe-i18n. A typo in
// a consuming component (`i18n.t.commands.addCommnad`) is a compile error,
// with full autocomplete, because `t` really is `typeof en`/`typeof de`.
//
// Persistence of the *choice* belongs to `configState`
// (`settings.language` in config.json, owned by another agent) — this
// module never imports or writes `configState` itself. A caller drives this
// store via `setLanguage`, typically once after `configState.load()`
// resolves, and again whenever the user changes the setting.
//
// Which languages exist at all — codes, native display names, dictionaries
// — lives in `registry.ts`, not here: see that file for how to add one.

import { registry, FALLBACK_LOCALE, type Locale, type LanguageChoice } from './registry';

export type { Locale, LanguageChoice };

/** WebView2 exposes the OS locale to the frontend only via
 *  `navigator.language` -- there is no other signal available in this Tauri
 *  context (see v3-research.md's i18n section). Guarded on both
 *  `navigator`'s and `.language`'s presence: this module can be evaluated
 *  outside a full browser environment (SvelteKit's prerender step, a test
 *  runner without `jsdom`), and a missing value must fall back to English,
 *  not throw.
 *
 *  Matches on the primary subtag only (`navigator.language.split('-')[0]`),
 *  against `registry`'s own keys, so any registered language resolves for
 *  every regional variant (`fr-CA`, `fr-FR`, ... all resolve to `fr` once
 *  `fr` is registered) without this function needing to know the language
 *  list at all -- adding a language never requires touching this function. */
function detectSystemLocale(): Locale {
	const lang = typeof navigator === 'undefined' ? undefined : navigator.language;
	if (typeof lang !== 'string') return FALLBACK_LOCALE;
	const lower = lang.toLowerCase();
	// `String.split` always returns at least one element, even for '' -- the
	// `?? lower` only exists to satisfy `noUncheckedIndexedAccess`, it can
	// never actually be reached.
	const primary = lower.split('-')[0] ?? lower;
	return primary in registry ? (primary as Locale) : FALLBACK_LOCALE;
}

function resolveLocale(choice: LanguageChoice): Locale {
	return choice === 'system' ? detectSystemLocale() : choice;
}

let languageChoice = $state<LanguageChoice>('system');

export const i18n = {
	/** The resolved locale actually in use. Never `'system'` -- that choice
	 *  has already been resolved to a `Locale` by the time it's read here,
	 *  via `detectSystemLocale` above. */
	get locale(): Locale {
		return resolveLocale(languageChoice);
	},
	/** The active dictionary. Reactive: reading this from a component
	 *  template or a `$derived` re-evaluates whenever `setLanguage` changes
	 *  `languageChoice`, because the `$state` read happens on every access,
	 *  same as `configState.config` in config.svelte.ts. */
	get t() {
		return registry[resolveLocale(languageChoice)].dictionary;
	},
	/** Drives the store. The caller owns persistence -- typically
	 *  `configState.load()`'s resolved `settings.language` on startup, and
	 *  the settings UI's language control after that -- this module never
	 *  reads or writes `configState` itself. */
	setLanguage(choice: LanguageChoice): void {
		languageChoice = choice;
	}
};
