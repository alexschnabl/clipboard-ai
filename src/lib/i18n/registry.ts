// lib/i18n/registry.ts — the single source of truth for which languages
// Clipboard AI ships. Both the store (i18n.svelte.ts, for detection and
// dictionary lookup) and the settings UI (SettingsPanel.svelte, for the
// language picker's option list) read from this map instead of each
// hardcoding its own copy — that duplication (a Rust enum, the store's
// detection logic, and the Settings control all independently listing
// "en"/"de") is exactly what made adding a language touch four places
// before this file existed.
//
// Adding a language:
//   1. Write `src/lib/i18n/xx.ts`, typed `export const xx: typeof en = {...}`
//      (the compiler lists every key still missing).
//   2. Add one entry to `registry` below: `xx: { nativeName: '...', dictionary: xx }`.
//   3. Add the matching variant to `Language` in src-tauri/src/config.rs
//      (see that enum's doc comment for why it stays a plain enum) and to
//      `Language` in src/lib/types.ts (the wire-contract mirror of it).
// Nothing else in this module, `i18n.svelte.ts`, or `SettingsPanel.svelte`
// needs to change.

import { en } from './en';
import { de } from './de';

/** `nativeName` is the language's own name for itself (e.g. "Français", not
 *  "French"), shown in the picker regardless of which locale is currently
 *  active — the standard language-picker convention. It lives here, once
 *  per language, rather than as a translatable string inside every
 *  dictionary: a `languageFrench`-style key in `en.ts` *and* `de.ts` *and*
 *  every future dictionary would be an entry per *other* language, an O(n²)
 *  growth the dictionaries themselves must never have to carry. */
interface LanguageEntry {
	nativeName: string;
	dictionary: typeof en;
}

// `satisfies`, not a `: Record<...>` annotation: an annotation would widen
// each dictionary's type to the annotation's `typeof en` and silently erase
// which key is `en` vs `de` from the type of `registry` itself, which is
// exactly what `Locale = keyof typeof registry` below depends on to stay
// precise. `satisfies` checks the shape without doing that widening.
export const registry = {
	en: { nativeName: 'English', dictionary: en },
	de: { nativeName: 'Deutsch', dictionary: de }
} satisfies Record<string, LanguageEntry>;

/** Every code this build ships a dictionary for. Derived from `registry`'s
 *  own keys — never hand-declared — so this type and the registry can't
 *  drift apart the way a parallel `type Locale = 'en' | 'de'` could. */
export type Locale = keyof typeof registry;

/** Mirrors `Settings.language` in src-tauri/src/config.rs (the persisted
 *  *choice*, serialized as the same lowercase strings this union uses, via
 *  `#[serde(rename_all = "lowercase")]`) — distinct from `Locale` above,
 *  which is always the already-resolved language actually shown. `'system'`
 *  means "detect the OS locale, fall back to English". */
export type LanguageChoice = 'system' | Locale;

/** English is canonical (see en.ts's own header comment) and therefore
 *  always present in `registry`, so it's the one locale that OS-locale
 *  detection can fall back to unconditionally when nothing else matches. */
export const FALLBACK_LOCALE: Locale = 'en';
