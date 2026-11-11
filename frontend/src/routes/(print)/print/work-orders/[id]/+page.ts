import { loadApi } from '$lib/api';
import type { WorkOrderDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	order: await loadApi<WorkOrderDetail>(`/work-orders/${params.id}`)
});
