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

import { en } from './en';
import { de } from './de';

/** The two languages Clipboard AI ships today. Adding a third (see
 *  v3-research.md): type `src/lib/i18n/fr.ts` against `typeof en` (the
 *  compiler lists everything missing), add `'fr'` here and to
 *  `dictionaries` below, and add it to the settings UI's language picker —
 *  nothing else in this module changes. */
export type Locale = 'en' | 'de';

/** Mirrors `Settings.language` in src-tauri/src/config.rs
 *  (`Language::System | ::En | ::De`, serialized as the lowercase strings
 *  `"system" | "en" | "de"`) -- the persisted *choice*, distinct from
 *  `Locale` above, which is always the already-resolved language actually
 *  shown. `'system'` means "detect the OS locale, fall back to English". */
export type LanguageChoice = 'system' | Locale;

const dictionaries = { en, de } satisfies Record<Locale, typeof en>;

/** WebView2 exposes the OS locale to the frontend only via
 *  `navigator.language` -- there is no other signal available in this Tauri
 *  context (see v3-research.md's i18n section). Guarded on both
 *  `navigator`'s and `.language`'s presence: this module can be evaluated
 *  outside a full browser environment (SvelteKit's prerender step, a test
 *  runner without `jsdom`), and a missing value must fall back to English,
 *  not throw. Any tag beginning `de` (`de`, `de-DE`, `de-AT`, `de-CH`, ...)
 *  resolves to German; anything else, including a missing value, resolves
 *  to English. */
function detectSystemLocale(): Locale {
	const lang = typeof navigator === 'undefined' ? undefined : navigator.language;
	return typeof lang === 'string' && lang.toLowerCase().startsWith('de') ? 'de' : 'en';
}

function resolveLocale(choice: LanguageChoice): Locale {
	return choice === 'system' ? detectSystemLocale() : choice;
}

let languageChoice = $state<LanguageChoice>('system');

export const i18n = {
	/** The resolved locale actually in use. Never `'system'` -- that choice
	 *  has already been resolved to `'en'` or `'de'` by the time it's read
	 *  here, via `detectSystemLocale` above. */
	get locale(): Locale {
		return resolveLocale(languageChoice);
	},
	/** The active dictionary. Reactive: reading this from a component
	 *  template or a `$derived` re-evaluates whenever `setLanguage` changes
	 *  `languageChoice`, because the `$state` read happens on every access,
	 *  same as `configState.config` in config.svelte.ts. */
	get t() {
		return dictionaries[resolveLocale(languageChoice)];
	},
	/** Drives the store. The caller owns persistence -- typically
	 *  `configState.load()`'s resolved `settings.language` on startup, and
	 *  the settings UI's language control after that -- this module never
	 *  reads or writes `configState` itself. */
	setLanguage(choice: LanguageChoice): void {
		languageChoice = choice;
	}
};
