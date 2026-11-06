import { api } from '$lib/api';
import type { WorkOrderDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	order: await api<WorkOrderDetail>('GET', `/work-orders/${params.id}`)
});
