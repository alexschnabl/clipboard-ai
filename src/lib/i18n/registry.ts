
import { en } from './en';
import { de } from './de';
import { fr } from './fr';
import { es } from './es';
import { it } from './it';

interface LanguageEntry {
	nativeName: string;
	dictionary: typeof en;
}

export const registry = {
	en: { nativeName: 'English', dictionary: en },
	de: { nativeName: 'Deutsch', dictionary: de },
	fr: { nativeName: 'Français', dictionary: fr },
	es: { nativeName: 'Español', dictionary: es },
	it: { nativeName: 'Italiano', dictionary: it }
} satisfies Record<string, LanguageEntry>;

export type Locale = keyof typeof registry;

export type LanguageChoice = 'system' | Locale;

export const FALLBACK_LOCALE: Locale = 'en';
