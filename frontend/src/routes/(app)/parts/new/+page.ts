import { loadApi } from '$lib/api';
import type { PartDetail } from '$lib/types';
import type { PageLoad } from './$types';

// "Add a used one" opens this form filled in from an existing part.
export const load: PageLoad = async ({ url }) => {
	const from = url.searchParams.get('from');
	return {
		source: from ? await loadApi<PartDetail>(`/parts/${encodeURIComponent(from)}`) : null
	};
};
