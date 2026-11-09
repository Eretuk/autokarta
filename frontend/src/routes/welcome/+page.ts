import { redirect } from '@sveltejs/kit';
import { safeNext } from '$lib/api';
import { loadSession } from '$lib/session.svelte';
import type { PageLoad } from './$types';

// Only for a signed-in user who still has a password someone else knows.
export const load: PageLoad = async ({ url }) => {
	const next = safeNext(url.searchParams.get('next'));
	const user = await loadSession();
	if (!user) redirect(307, '/login');
	if (!user.must_change_password) redirect(307, next);
	return { next };
};
