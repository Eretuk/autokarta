import { loadApi } from '$lib/api';
import type { PartDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	part: await loadApi<PartDetail>(`/parts/${params.id}`)
});
