<script lang="ts" module>
	import type { DistanceUnit, VehicleRole } from '$lib/types';

	export interface VehicleForm {
		plate: string;
		vin: string;
		frame_number: string;
		make: string;
		model: string;
		year: string;
		engine: string;
		note: string;
		role: VehicleRole;
		odometer_unit: DistanceUnit;
	}

	/** JSON body for POST/PUT /vehicles. Empty strings mean "no value". */
	export function vehicleBody(f: VehicleForm) {
		const year = f.year.trim() ? Number(f.year.trim()) : null;
		return {
			plate: f.plate,
			vin: f.vin,
			frame_number: f.frame_number,
			make: f.make,
			model: f.model,
			year: Number.isFinite(year) ? year : -1,
			engine: f.engine,
			note: f.note,
			role: f.role,
			odometer_unit: f.odometer_unit
		};
	}
</script>

<script lang="ts">
	// Identification and description fields of a vehicle. At least one of
	// plate, VIN or frame number is required (checked by the server).
	import { vinFromScan } from '$lib/format';
	import { t } from '$lib/i18n/index.svelte';
	import ScanButton from './ScanButton.svelte';

	let { form = $bindable() }: { form: VehicleForm } = $props();

	const ROLES: VehicleRole[] = ['customer', 'fleet', 'for_sale'];
	const UNITS: DistanceUnit[] = ['km', 'mi'];
</script>

<fieldset>
	<legend>{t('vehicles.identification')}</legend>
	<p class="hint">{t('vehicles.identificationHint')}</p>

	<label class="field">
		{t('vehicles.plate')}
		<input type="text" bind:value={form.plate} maxlength="16" autocomplete="off" autocapitalize="characters" spellcheck="false" />
	</label>

	<label class="field" for="vin">{t('vehicles.vin')}</label>
	<div class="with-action field-gap">
		<input
			id="vin"
			type="text"
			class="mono"
			bind:value={form.vin}
			maxlength="20"
			autocomplete="off"
			autocapitalize="characters"
			spellcheck="false"
		/>
		<ScanButton title={t('vehicles.scanVin')} onScan={(d) => (form.vin = vinFromScan(d.text) ?? d.text)} />
	</div>

	<label class="field">
		{t('vehicles.frameNumber')}
		<span class="hint">{t('vehicles.frameNumberHint')}</span>
		<input
			type="text"
			class="mono"
			bind:value={form.frame_number}
			maxlength="24"
			autocomplete="off"
			autocapitalize="characters"
			spellcheck="false"
		/>
	</label>
</fieldset>

<fieldset>
	<legend>{t('vehicles.description')}</legend>
	<div class="two-cols">
		<label class="field">
			{t('vehicles.make')}
			<input type="text" bind:value={form.make} maxlength="60" autocomplete="off" />
		</label>
		<label class="field">
			{t('vehicles.model')}
			<input type="text" bind:value={form.model} maxlength="60" autocomplete="off" />
		</label>
		<label class="field">
			{t('vehicles.year')}
			<input type="text" inputmode="numeric" pattern="[0-9]*" bind:value={form.year} maxlength="4" autocomplete="off" />
		</label>
		<label class="field">
			{t('vehicles.engine')}
			<input type="text" bind:value={form.engine} maxlength="60" autocomplete="off" />
		</label>
	</div>

	<span class="label">{t('vehicles.role')}</span>
	<div class="segmented field-gap">
		{#each ROLES as r (r)}
			<button type="button" class="btn" aria-pressed={form.role === r} onclick={() => (form.role = r)}
				>{t(`vehicles.roles.${r}`)}</button
			>
		{/each}
	</div>

	<span class="label">{t('vehicles.odometerUnit')}</span>
	<div class="segmented field-gap">
		{#each UNITS as u (u)}
			<button type="button" class="btn" aria-pressed={form.odometer_unit === u} onclick={() => (form.odometer_unit = u)}
				>{t(`units.${u}`)}</button
			>
		{/each}
	</div>

	<label class="field">
		{t('vehicles.note')}
		<textarea bind:value={form.note} maxlength="2000"></textarea>
	</label>
</fieldset>

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
		margin-bottom: 4px;
		padding: 0;
	}
	label[for='vin'] {
		margin-bottom: 6px;
	}
	.field-gap {
		margin-bottom: 14px;
	}
	.label {
		display: block;
		font-weight: 500;
		margin-bottom: 6px;
	}
</style>
