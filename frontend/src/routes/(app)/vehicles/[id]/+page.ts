import { loadApi } from '$lib/api';
import type { VehicleDetail } from '$lib/types';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ params }) => ({
	vehicle: await loadApi<VehicleDetail>(`/vehicles/${params.id}`)
});
