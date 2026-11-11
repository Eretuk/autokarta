import { redirect } from '@sveltejs/kit';
import { pageError } from '$lib/api';
import { loadSession, welcomeUrl } from '$lib/session.svelte';
import { loadBusinessSettings } from '$lib/settings.svelte';
import type { LayoutLoad } from './$types';

// Every page in this group needs a signed-in staff member.
export const load: LayoutLoad = async ({ url }) => {
	const user = await loadSession().catch(pageError);
	if (!user) {
		const next = url.pathname + url.search;
		redirect(307, next === '/' ? '/login' : `/login?next=${encodeURIComponent(next)}`);
	}
	if (user.must_change_password) redirect(307, welcomeUrl(url.pathname + url.search));
	await loadBusinessSettings().catch(pageError);
	return { user };
};
