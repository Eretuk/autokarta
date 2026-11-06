import { redirect } from '@sveltejs/kit';
import { loadSession } from '$lib/session.svelte';
import { loadBusinessSettings } from '$lib/settings.svelte';
import type { LayoutLoad } from './$types';

// Every page in this group needs a signed-in staff member.
export const load: LayoutLoad = async ({ url }) => {
	const user = await loadSession();
	if (!user) {
		const next = url.pathname + url.search;
		redirect(307, next === '/' ? '/login' : `/login?next=${encodeURIComponent(next)}`);
	}
	await loadBusinessSettings();
	return { user };
};
