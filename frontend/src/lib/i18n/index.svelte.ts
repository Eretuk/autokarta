// Small typed i18n layer. English (en.json) is the source of truth: its
// structure defines the valid keys, other languages may miss keys and then
// fall back to English.
//
// A message is either a string with {placeholders}, or a plural object
// selected with Intl.PluralRules ({ one, few, many, other }), because Polish
// and Russian have more plural forms than English.

import en from './en.json';
import pl from './pl.json';
import ru from './ru.json';

export const LOCALES = ['en', 'pl', 'ru'] as const;
export type Locale = (typeof LOCALES)[number];

/** Language names are always shown in their own language. */
export const LOCALE_NAMES: Record<Locale, string> = {
	en: 'English',
	pl: 'Polski',
	ru: 'Русский'
};

type PluralForms = Partial<Record<Intl.LDMLPluralRule, string>> & { other: string };
type Messages = typeof en;
type Tree = { [key: string]: string | PluralForms | Tree };

type KeyPaths<T, P extends string = ''> = {
	[K in keyof T & string]: T[K] extends string
		? `${P}${K}`
		: T[K] extends { other: string }
			? `${P}${K}`
			: KeyPaths<T[K], `${P}${K}.`>;
}[keyof T & string];

export type MessageKey = KeyPaths<Messages>;
export type Params = Record<string, string | number>;

const catalogs: Record<Locale, Tree> = { en, pl, ru };

const STORAGE_KEY = 'autokarta.locale';

export const i18n = $state<{ locale: Locale }>({ locale: detectLocale() });

export function isLocale(v: unknown): v is Locale {
	return typeof v === 'string' && (LOCALES as readonly string[]).includes(v);
}

function detectLocale(): Locale {
	try {
		const saved = localStorage.getItem(STORAGE_KEY);
		if (isLocale(saved)) return saved;
	} catch {
		// Storage can be unavailable (private mode); fall through.
	}
	if (typeof navigator !== 'undefined') {
		for (const lang of navigator.languages ?? []) {
			const base = lang.toLowerCase().split('-')[0];
			if (isLocale(base)) return base;
		}
	}
	return 'en';
}

export function setLocale(locale: Locale) {
	i18n.locale = locale;
	if (typeof document !== 'undefined') document.documentElement.lang = locale;
	try {
		localStorage.setItem(STORAGE_KEY, locale);
	} catch {
		// Not critical: the profile locale is applied again after login.
	}
}

function lookup(tree: Tree, key: string): string | PluralForms | undefined {
	let node: string | PluralForms | Tree | undefined = tree;
	for (const part of key.split('.')) {
		if (node === undefined || typeof node === 'string') return undefined;
		node = (node as Tree)[part];
	}
	if (typeof node === 'string') return node;
	if (node && typeof node === 'object' && typeof node.other === 'string') return node as PluralForms;
	return undefined;
}

const pluralRules = new Map<Locale, Intl.PluralRules>();
function plural(locale: Locale, count: number): Intl.LDMLPluralRule {
	let rules = pluralRules.get(locale);
	if (!rules) {
		rules = new Intl.PluralRules(locale);
		pluralRules.set(locale, rules);
	}
	return rules.select(count);
}

/** Translate a key. `{count}` selects the plural form and is number-formatted. */
export function t(key: MessageKey, params?: Params): string {
	const locale = i18n.locale;
	let locUsed: Locale = locale;
	let msg = lookup(catalogs[locale], key);
	if (msg === undefined) {
		locUsed = 'en';
		msg = lookup(catalogs.en, key);
	}
	if (msg === undefined) return key;

	if (typeof msg !== 'string') {
		const count = Number(params?.count ?? 0);
		msg = msg[plural(locUsed, count)] ?? msg.other;
	}
	if (!params) return msg;
	return msg.replace(/\{(\w+)\}/g, (whole, name: string) => {
		const v = params[name];
		if (v === undefined) return whole;
		return typeof v === 'number' ? formatNumber(v) : v;
	});
}

/** True if a key exists in English, used for server error codes. */
export function hasKey(key: string): key is MessageKey {
	return lookup(catalogs.en, key) !== undefined;
}

export function formatNumber(n: number, options?: Intl.NumberFormatOptions): string {
	return new Intl.NumberFormat(i18n.locale, options).format(n);
}

export function formatDate(value: string | Date, options?: Intl.DateTimeFormatOptions): string {
	const date = typeof value === 'string' ? new Date(value) : value;
	return new Intl.DateTimeFormat(i18n.locale, options ?? { dateStyle: 'medium' }).format(date);
}
