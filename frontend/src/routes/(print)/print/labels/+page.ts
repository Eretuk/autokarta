import { error } from '@sveltejs/kit';
import { api } from '$lib/api';
import type { PartDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ url }) => {
	const id = url.searchParams.get('part');
	if (!id) error(404, 'not_found');
	return { part: await api<PartDetail>('GET', `/parts/${encodeURIComponent(id)}`) };
};
