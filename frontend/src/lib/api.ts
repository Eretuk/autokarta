// Thin JSON client for the backend. The backend returns stable error codes
// (`{"error": "username_taken"}`); they are translated in the UI via `errors.*`.

import { goto } from '$app/navigation';
import { hasKey, t } from './i18n/index.svelte';

export class ApiError extends Error {
	constructor(
		public status: number,
		public code: string,
		public retryAfterSecs?: number
	) {
		super(code);
	}
}

type Method = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';

/** Called when the session is gone (expired, revoked, account deactivated). */
let onUnauthorized: (() => void) | null = null;
export function setUnauthorizedHandler(fn: () => void) {
	onUnauthorized = fn;
}

export async function api<T>(method: Method, path: string, body?: unknown): Promise<T> {
	let res: Response;
	try {
		res = await fetch(`/api${path}`, {
			method,
			credentials: 'same-origin',
			headers: body === undefined ? undefined : { 'Content-Type': 'application/json' },
			body: body === undefined ? undefined : JSON.stringify(body)
		});
	} catch {
		throw new ApiError(0, 'network_error');
	}

	if (res.ok) {
		return (res.status === 204 ? undefined : await res.json()) as T;
	}

	let code = 'unknown';
	try {
		const data = await res.json();
		if (typeof data?.error === 'string') code = data.error;
	} catch {
		// Not JSON, e.g. a proxy error page.
		if (res.status >= 500) code = 'server_unavailable';
	}
	const retry = Number(res.headers.get('Retry-After'));
	const err = new ApiError(res.status, code, Number.isFinite(retry) && retry > 0 ? retry : undefined);

	// A failed login is also a 401, but it must not redirect anywhere.
	if (res.status === 401 && path !== '/auth/login' && path !== '/auth/me') {
		onUnauthorized?.();
	}
	throw err;
}

/** Human-readable, translated message for any error thrown by `api`. */
export function errorMessage(e: unknown): string {
	if (e instanceof ApiError) {
		if (e.code === 'too_many_attempts' && e.retryAfterSecs) {
			return t('errors.too_many_attempts_wait', { count: Math.ceil(e.retryAfterSecs / 60) });
		}
		const key = `errors.${e.code}`;
		if (hasKey(key)) return t(key);
	}
	return t('errors.unknown');
}

/** Only allow same-site relative paths, to avoid open redirects via ?next=. */
export function safeNext(next: string | null): string {
	if (next && next.startsWith('/') && !next.startsWith('//') && !next.startsWith('/\\')) return next;
	return '/';
}

export function redirectToLogin() {
	const here = location.pathname + location.search;
	goto(here === '/' ? '/login' : `/login?next=${encodeURIComponent(here)}`, { replaceState: true });
}
