<script lang="ts">
	// The business itself: legal details (printed on work orders) and the
	// settings that apply to everyone. Owners only.
	import { api, errorMessage } from '$lib/api';
	import { t } from '$lib/i18n/index.svelte';
	import { session } from '$lib/session.svelte';
	import { business, missingOrgFields, setBusinessSettings, type RequiredOrgField } from '$lib/settings.svelte';
	import type { BusinessSettings, DistanceUnit, Organization } from '$lib/types';

	const UNITS: DistanceUnit[] = ['km', 'mi'];
	const isOwner = $derived(session.user?.role === 'owner');
	const missing = $derived(missingOrgFields());

	type OrgForm = Record<keyof Organization, string>;
	function orgForm(o: Organization): OrgForm {
		return {
			company_name: o.company_name ?? '',
			tax_id: o.tax_id ?? '',
			regon: o.regon ?? '',
			legal_address: o.legal_address ?? '',
			service_address: o.service_address ?? '',
			phone: o.phone ?? '',
			email: o.email ?? '',
			contact_person: o.contact_person ?? '',
			bank_account: o.bank_account ?? ''
		};
	}

	let org = $state<OrgForm>(orgForm(business.organization));
	let orgBusy = $state(false);
	let orgMsg = $state<{ ok: boolean; text: string } | null>(null);

	function isRequired(f: keyof Organization): boolean {
		return (['company_name', 'tax_id', 'legal_address', 'contact_person', 'phone'] as string[]).includes(f);
	}

	async function saveOrg(e: SubmitEvent) {
		e.preventDefault();
		orgBusy = true;
		orgMsg = null;
		const body: Record<string, string | null> = {};
		for (const [k, v] of Object.entries(org)) body[k] = v.trim() || null;
		try {
			const s = await api<BusinessSettings>('PATCH', '/settings', { organization: body });
			setBusinessSettings(s);
			org = orgForm(s.organization);
			orgMsg = { ok: true, text: t('settings.saved') };
		} catch (err) {
			orgMsg = { ok: false, text: errorMessage(err) };
		} finally {
			orgBusy = false;
		}
	}

	let biz = $state({
		currency: business.currency,
		distance_unit: business.distance_unit,
		vat: String(business.default_vat_percent),
		markup: business.markup_percent === null || business.markup_percent === undefined ? '' : String(business.markup_percent)
	});
	let bizBusy = $state(false);
	let bizMsg = $state<{ ok: boolean; text: string } | null>(null);

	async function saveBusiness(e: SubmitEvent) {
		e.preventDefault();
		const currency = biz.currency.trim().toUpperCase();
		const markupText = biz.markup.trim().replace('%', '').trim();
		if (markupText && (!/^\d{1,4}$/.test(markupText) || Number(markupText) > 1000)) {
			bizMsg = { ok: false, text: t('errors.invalid_markup') };
			return;
		}
		const vatText = biz.vat.trim().replace('%', '').trim();
		if (!/^\d{1,3}$/.test(vatText) || Number(vatText) > 100) {
			bizMsg = { ok: false, text: t('errors.invalid_vat') };
			return;
		}
		// Amounts are stored without a currency: a change relabels every
		// existing price instead of converting it.
		if (
			currency !== business.currency &&
			!confirm(t('settings.confirmCurrency', { from: business.currency, to: currency }))
		) {
			return;
		}
		bizBusy = true;
		bizMsg = null;
		try {
			setBusinessSettings(
				await api<BusinessSettings>('PATCH', '/settings', {
					currency,
					distance_unit: biz.distance_unit,
					default_vat_percent: Number(vatText),
					markup_percent: markupText ? Number(markupText) : null
				})
			);
			bizMsg = { ok: true, text: t('settings.saved') };
		} catch (err) {
			bizMsg = { ok: false, text: errorMessage(err) };
		} finally {
			bizBusy = false;
		}
	}

	const fieldLabel = (f: RequiredOrgField) => t(`organization.fields.${f}`);
</script>

<svelte:head><title>{t('organization.title')} · Autokarta</title></svelte:head>

<h1>{t('organization.title')}</h1>

{#if !isOwner}
	<div class="notice error" role="alert">{t('errors.forbidden')}</div>
{:else}
	<section class="card">
		<h2>{t('organization.details')}</h2>
		<p class="hint">{t('organization.detailsHint')}</p>
		{#if missing.length > 0}
			<div class="notice" role="status">
				{t('organization.missing', { fields: missing.map(fieldLabel).join(', ') })}
			</div>
		{/if}
		<form onsubmit={saveOrg}>
			{#if orgMsg}
				<div class="notice" class:ok={orgMsg.ok} class:error={!orgMsg.ok} role="status">{orgMsg.text}</div>
			{/if}
			<label class="field">
				{t('organization.fields.company_name')}{isRequired('company_name') ? ' *' : ''}
				<input type="text" bind:value={org.company_name} maxlength="160" autocomplete="organization" />
			</label>
			<div class="two-cols">
				<label class="field">
					{t('organization.fields.tax_id')} *
					<input type="text" class="mono" bind:value={org.tax_id} maxlength="30" autocomplete="off" spellcheck="false" />
				</label>
				<label class="field">
					{t('organization.fields.regon')}
					<input
						type="text"
						class="mono"
						inputmode="numeric"
						bind:value={org.regon}
						maxlength="20"
						autocomplete="off"
						spellcheck="false"
					/>
				</label>
			</div>
			<label class="field">
				{t('organization.fields.legal_address')} *
				<textarea bind:value={org.legal_address} maxlength="300" rows="3" autocomplete="street-address"></textarea>
			</label>
			<label class="field">
				{t('organization.fields.service_address')}
				<span class="hint">{t('organization.serviceAddressHint')}</span>
				<textarea bind:value={org.service_address} maxlength="300" rows="3"></textarea>
			</label>
			<label class="field">
				{t('organization.fields.contact_person')} *
				<span class="hint">{t('organization.contactHint')}</span>
				<input type="text" bind:value={org.contact_person} maxlength="120" autocomplete="name" />
			</label>
			<div class="two-cols">
				<label class="field">
					{t('organization.fields.phone')} *
					<input type="tel" bind:value={org.phone} maxlength="30" autocomplete="tel" />
				</label>
				<label class="field">
					{t('organization.fields.email')}
					<input type="email" bind:value={org.email} maxlength="254" autocomplete="email" />
				</label>
			</div>
			<label class="field">
				{t('organization.fields.bank_account')}
				<input type="text" class="mono" bind:value={org.bank_account} maxlength="50" autocomplete="off" spellcheck="false" />
			</label>
			<p class="hint">{t('organization.requiredHint')}</p>
			<button class="btn primary" type="submit" disabled={orgBusy}>{t('common.saveChanges')}</button>
		</form>
	</section>

	<section class="card">
		<h2>{t('settings.title')}</h2>
		<p class="hint">{t('settings.hint')}</p>
		<form onsubmit={saveBusiness}>
			{#if bizMsg}
				<div class="notice" class:ok={bizMsg.ok} class:error={!bizMsg.ok} role="status">{bizMsg.text}</div>
			{/if}
			<label class="field">
				{t('settings.currency')}
				<span class="hint">{t('settings.currencyHint')}</span>
				<input type="text" bind:value={biz.currency} maxlength="3" autocapitalize="characters" autocomplete="off" required />
			</label>
			<span class="label" id="distance-unit-label">{t('settings.distanceUnit')}</span>
			<p class="hint">{t('settings.distanceUnitHint')}</p>
			<div class="segmented spaced-b" role="group" aria-labelledby="distance-unit-label">
				{#each UNITS as u (u)}
					<button type="button" class="btn" aria-pressed={biz.distance_unit === u} onclick={() => (biz.distance_unit = u)}
						>{t(`units.${u}`)}</button
					>
				{/each}
			</div>
			<div class="two-cols">
				<label class="field">
					{t('settings.defaultVat')}
					<span class="hint">{t('settings.defaultVatHint')}</span>
					<input type="text" inputmode="numeric" bind:value={biz.vat} maxlength="4" autocomplete="off" required />
				</label>
				<label class="field">
					{t('settings.markup')}
					<span class="hint">{t('settings.markupHint')}</span>
					<input type="text" inputmode="numeric" bind:value={biz.markup} maxlength="5" autocomplete="off" />
				</label>
			</div>
			<button class="btn primary" type="submit" disabled={bizBusy}>{t('common.saveChanges')}</button>
		</form>
	</section>
{/if}

<style>
	.label {
		display: block;
		font-weight: 500;
	}
	.spaced-b {
		margin-bottom: 14px;
	}
	textarea {
		resize: vertical;
	}
</style>
