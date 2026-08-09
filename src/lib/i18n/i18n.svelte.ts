
import { registry, FALLBACK_LOCALE, type Locale, type LanguageChoice } from './registry';

export type { Locale, LanguageChoice };

function detectSystemLocale(): Locale {
	const lang = typeof navigator === 'undefined' ? undefined : navigator.language;
	if (typeof lang !== 'string') return FALLBACK_LOCALE;
	const lower = lang.toLowerCase();
	const primary = lower.split('-')[0] ?? lower;
	return primary in registry ? (primary as Locale) : FALLBACK_LOCALE;
}

function resolveLocale(choice: LanguageChoice): Locale {
	return choice === 'system' ? detectSystemLocale() : choice;
}

let languageChoice = $state<LanguageChoice>('system');

export const i18n = {
	get locale(): Locale {
		return resolveLocale(languageChoice);
	},
	get t() {
		return registry[resolveLocale(languageChoice)].dictionary;
	},
	setLanguage(choice: LanguageChoice): void {
		languageChoice = choice;
	}
};
