import { api } from '$lib/api';
import type { PartDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	part: await api<PartDetail>('GET', `/parts/${params.id}`)
});
