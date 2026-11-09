import { redirect } from '@sveltejs/kit';
import { loadSession, welcomeUrl } from '$lib/session.svelte';
import { loadBusinessSettings } from '$lib/settings.svelte';
import type { LayoutLoad } from './$types';

// Printable pages: same sign-in rule as the app, but no app chrome.
export const load: LayoutLoad = async ({ url }) => {
	const user = await loadSession();
	if (!user) redirect(307, `/login?next=${encodeURIComponent(url.pathname + url.search)}`);
	if (user.must_change_password) redirect(307, welcomeUrl(url.pathname + url.search));
	await loadBusinessSettings();
	return { user };
};
