import { api } from '$lib/api';
import type { VehicleDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	vehicle: await api<VehicleDetail>('GET', `/vehicles/${params.id}`)
});
