import { api, ApiError } from './api';
import { isLocale, setLocale, type Locale } from './i18n/index.svelte';

export type Role = 'owner' | 'mechanic';

export interface User {
	id: string;
	username: string;
	display_name: string;
	role: Role;
	locale: Locale;
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

export async function logout() {
	try {
		await api('POST', '/auth/logout');
	} finally {
		session.user = null;
	}
}
