<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, ApiError, errorMessage } from '$lib/api';
	import ClientPicker from '$lib/components/ClientPicker.svelte';
	import VehicleFields, { vehicleBody, type VehicleForm } from '$lib/components/VehicleFields.svelte';
	import { asVin } from '$lib/format';
	import { parseReading } from '$lib/odometer';
	import { t } from '$lib/i18n/index.svelte';
	import { business } from '$lib/settings.svelte';
	import type { Client, VehicleDetail } from '$lib/types';

	// Coming from a search that found nothing: reuse what was typed.
	const typed = (page.url.searchParams.get('q') ?? '').trim();
	const typedVin = asVin(typed);

	let form = $state<VehicleForm>({
		plate: typedVin ? '' : typed,
		vin: typedVin ?? '',
		frame_number: '',
		make: '',
		model: '',
		year: '',
		engine: '',
		note: '',
		role: 'customer',
		odometer_unit: business.distance_unit
	});
	let owner = $state<Client | null>(null);
	let odometer = $state('');
	let error = $state('');
	let duplicateQuery = $state('');
	let busy = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		duplicateQuery = '';
		const reading = parseReading(odometer);
		if (Number.isNaN(reading)) {
			error = t('errors.invalid_odometer');
			return;
		}
		busy = true;
		try {
			const v = await api<VehicleDetail>('POST', '/vehicles', {
				...vehicleBody(form),
				owner_id: owner?.id ?? null,
				odometer: reading
			});
			await goto(`/vehicles/${v.id}`, { replaceState: true });
		} catch (err) {
			error = errorMessage(err);
			if (err instanceof ApiError && (err.code === 'vin_taken' || err.code === 'frame_number_taken')) {
				duplicateQuery = err.code === 'vin_taken' ? form.vin : form.frame_number;
			}
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>{t('vehicles.addTitle')} · Autokarta</title></svelte:head>

<h1>{t('vehicles.addTitle')}</h1>

<form onsubmit={submit}>
	<VehicleFields bind:form />

	<fieldset>
		<legend>{t('vehicles.ownerAndMileage')}</legend>
		<ClientPicker bind:selected={owner} label={t('vehicles.owner')} />
		<label class="field">
			{t('vehicles.currentOdometer', { unit: t(`units.${form.odometer_unit}`) })}
			<input type="text" inputmode="numeric" bind:value={odometer} maxlength="9" autocomplete="off" />
		</label>
	</fieldset>

	{#if error}
		<div class="notice error" role="alert">
			{error}
			{#if duplicateQuery}
				<br /><a href="/?q={encodeURIComponent(duplicateQuery)}">{t('vehicles.openExisting')}</a>
			{/if}
		</div>
	{/if}

	<button class="btn primary block" type="submit" disabled={busy}>{t('vehicles.addSubmit')}</button>
</form>

<style>
	fieldset {
		border: none;
		margin: 0 0 8px;
		padding: 0;
	}
	legend {
		font-family: var(--font-head);
		font-weight: 700;
		font-size: 1.25rem;
		margin-bottom: 8px;
		padding: 0;
	}
</style>
