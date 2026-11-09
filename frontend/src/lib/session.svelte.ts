import { api, ApiError } from './api';
import { isLocale, setLocale, type Locale } from './i18n/index.svelte';

export type Role = 'owner' | 'mechanic';

export interface User {
	id: string;
	username: string;
	display_name: string;
	role: Role;
	locale: Locale;
	/** Signed in with a password someone else knows; must set a new one first. */
	must_change_password: boolean;
}

export const session = $state<{ user: User | null }>({ user: null });

export function setUser(user: User | null) {
	session.user = user;
	if (user && isLocale(user.locale)) setLocale(user.locale);
}

/** Loads the current user once; returns null when not signed in. */
export async function loadSession(): Promise<User | null> {
	if (session.user) return session.user;
	try {
		setUser(await api<User>('GET', '/auth/me'));
	} catch (e) {
		if (e instanceof ApiError && e.status === 401) {
			session.user = null;
		} else {
			throw e;
		}
	}
	return session.user;
}

// The password typed on the login page, kept in memory only until /welcome
// uses it, so a forced password change does not ask for it a second time.
let loginPassword: string | null = null;
export function rememberLoginPassword(p: string) {
	loginPassword = p;
}
export function takeLoginPassword(): string | null {
	const p = loginPassword;
	loginPassword = null;
	return p;
}

/** Where a user who must change the password goes, keeping the target page. */
export function welcomeUrl(next: string): string {
	return next === '/' ? '/welcome' : `/welcome?next=${encodeURIComponent(next)}`;
}

export async function logout() {
	try {
		await api('POST', '/auth/logout');
	} finally {
		session.user = null;
	}
}
