import { api } from './api';
import type { BusinessSettings } from './types';

export const business = $state<BusinessSettings>({ currency: 'PLN', distance_unit: 'km' });

let loaded = false;

export async function loadBusinessSettings(force = false) {
	if (loaded && !force) return;
	Object.assign(business, await api<BusinessSettings>('GET', '/settings'));
	loaded = true;
}

export function setBusinessSettings(s: BusinessSettings) {
	Object.assign(business, s);
	loaded = true;
}
