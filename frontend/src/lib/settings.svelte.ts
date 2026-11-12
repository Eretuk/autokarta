import { api } from './api';
import type { BusinessSettings } from './types';

export const business = $state<BusinessSettings>({
	currency: 'PLN',
	distance_unit: 'km',
	default_vat_percent: 23,
	organization: {
		company_name: null,
		tax_id: null,
		regon: null,
		legal_address: null,
		service_address: null,
		phone: null,
		email: null,
		contact_person: null,
		bank_account: null
	}
});

/** What the organization page asks for before the details count as filled in. */
export const REQUIRED_ORG_FIELDS = ['company_name', 'tax_id', 'legal_address', 'contact_person', 'phone'] as const;
export type RequiredOrgField = (typeof REQUIRED_ORG_FIELDS)[number];

export function missingOrgFields(): RequiredOrgField[] {
	return REQUIRED_ORG_FIELDS.filter((f) => !business.organization[f]?.trim());
}

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
