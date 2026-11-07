<script lang="ts" module>
	import { moneyInput, parseDecimal, parseMoney, quantityInput } from '$lib/format';
	import type { MessageKey } from '$lib/i18n/index.svelte';
	import type { PartCondition, PartDetail, PartUnit } from '$lib/types';

	export interface PartForm {
		name: string;
		brand: string;
		part_number: string;
		condition: PartCondition;
		unit: PartUnit;
		location: string;
		min_stock: string;
		price: string;
		note: string;
	}

	export function emptyPartForm(): PartForm {
		return {
			name: '',
			brand: '',
			part_number: '',
			condition: 'new',
			unit: 'pcs',
			location: '',
			min_stock: '0',
			price: '',
			note: ''
		};
	}

	export function partForm(p: PartDetail, currency: string): PartForm {
		return {
			name: p.name,
			brand: p.brand ?? '',
			part_number: p.part_number ?? '',
			condition: p.condition,
			unit: p.unit,
			location: p.location ?? '',
			min_stock: quantityInput(p.min_stock_milli),
			price: moneyInput(p.sell_price_minor, currency),
			note: p.note ?? ''
		};
	}

	/**
	 * JSON body for POST/PUT /parts, or the message key of the first field
	 * that does not parse. The price is only sent by owners: for mechanics
	 * the server keeps whatever price the part has.
	 */
	export function partBody(
		f: PartForm,
		currency: string,
		withPrice: boolean
	): { body: Record<string, unknown> } | { error: MessageKey } {
		const min = f.min_stock.trim() ? parseDecimal(f.min_stock, 3) : 0;
		if (min === null) return { error: 'errors.invalid_quantity' };
		const body: Record<string, unknown> = {
			name: f.name,
			brand: f.brand,
			part_number: f.part_number,
			condition: f.condition,
			unit: f.unit,
			location: f.location,
			min_stock_milli: min,
			note: f.note
		};
		if (withPrice) {
			const price = f.price.trim() ? parseMoney(f.price, currency) : null;
			if (f.price.trim() && price === null) return { error: 'errors.invalid_price' };
			body.sell_price_minor = price;
		}
		return { body };
	}
</script>

<script lang="ts">
	// Catalogue fields of a part. Only the name is required: a box of bulbs
	// on the shelf does not need a part number to be counted.
	import { t } from '$lib/i18n/index.svelte';

	let {
		form = $bindable(),
		currency,
		showPrice
	}: { form: PartForm; currency: string; showPrice: boolean } = $props();

	const UNITS: PartUnit[] = ['pcs', 'l', 'kg', 'm'];
	const CONDITIONS: PartCondition[] = ['new', 'used'];
</script>

<label class="field">
	{t('parts.name')}
	<input
		type="text"
		bind:value={form.name}
		maxlength="160"
		required
		autocomplete="off"
		placeholder={t('parts.namePlaceholder')}
	/>
</label>

<span class="label">{t('parts.condition')}</span>
<div class="segmented field-gap">
	{#each CONDITIONS as c (c)}
		<button type="button" class="btn" aria-pressed={form.condition === c} onclick={() => (form.condition = c)}
			>{t(`parts.conditions.${c}`)}</button
		>
	{/each}
</div>

<div class="two-cols">
	<label class="field">
		{t('parts.brand')}
		<input type="text" bind:value={form.brand} maxlength="60" autocomplete="off" />
	</label>
	<label class="field">
		{t('parts.location')}
		<input type="text" bind:value={form.location} maxlength="60" autocomplete="off" autocapitalize="characters" />
	</label>
</div>

<label class="field">
	{t('parts.partNumber')}
	<span class="hint">{t('parts.partNumberHint')}</span>
	<input
		type="text"
		class="mono"
		bind:value={form.part_number}
		maxlength="40"
		autocomplete="off"
		autocapitalize="characters"
		spellcheck="false"
	/>
</label>

<span class="label">{t('parts.unit')}</span>
<div class="segmented field-gap">
	{#each UNITS as u (u)}
		<button type="button" class="btn" aria-pressed={form.unit === u} onclick={() => (form.unit = u)}
			>{t(`units.${u}`)}</button
		>
	{/each}
</div>

<label class="field">
	{t('parts.minStock')}
	<span class="hint">{t('parts.minStockHint')}</span>
	<input type="text" inputmode="decimal" bind:value={form.min_stock} maxlength="12" autocomplete="off" />
</label>

{#if showPrice}
	<label class="field">
		{t('parts.sellPrice', { currency })}
		<span class="hint">{t('parts.sellPriceHint')}</span>
		<input type="text" inputmode="decimal" bind:value={form.price} maxlength="16" autocomplete="off" />
	</label>
{/if}

<label class="field">
	{t('parts.note')}
	<textarea bind:value={form.note} maxlength="2000"></textarea>
</label>

<style>
	.label {
		display: block;
		font-weight: 500;
		margin-bottom: 6px;
	}
	.field-gap {
		margin-bottom: 14px;
	}
</style>
