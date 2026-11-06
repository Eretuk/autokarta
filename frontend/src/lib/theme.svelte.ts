// Theme preference is per device (a dark workshop phone vs. an office PC),
// so it lives in localStorage, not in the user profile.

export const THEMES = ['system', 'light', 'dark'] as const;
export type ThemePref = (typeof THEMES)[number];

const STORAGE_KEY = 'autokarta.theme';

function load(): ThemePref {
	try {
		const v = localStorage.getItem(STORAGE_KEY);
		if (v === 'light' || v === 'dark') return v;
	} catch {
		// ignore
	}
	return 'system';
}

export const theme = $state<{ pref: ThemePref }>({ pref: load() });

export function setTheme(pref: ThemePref) {
	theme.pref = pref;
	const root = document.documentElement;
	if (pref === 'system') delete root.dataset.theme;
	else root.dataset.theme = pref;
	try {
		if (pref === 'system') localStorage.removeItem(STORAGE_KEY);
		else localStorage.setItem(STORAGE_KEY, pref);
	} catch {
		// ignore
	}
}
