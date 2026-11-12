import { loadApi } from '$lib/api';
import type { DeliveryDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	delivery: await loadApi<DeliveryDetail>(`/deliveries/${params.id}`)
});
