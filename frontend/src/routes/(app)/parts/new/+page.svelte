<script lang="ts">
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, errorMessage, safeNext } from '$lib/api';
	import PartFields, { emptyPartForm, partBody, partForm, type PartForm } from '$lib/components/PartFields.svelte';
	import ScanButton from '$lib/components/ScanButton.svelte';
	import { parseDecimal, parseMoney } from '$lib/format';
	import { t } from '$lib/i18n/index.svelte';
	import { session } from '$lib/session.svelte';
	import { business } from '$lib/settings.svelte';
	import type { PartDetail } from '$lib/types';

	let { data } = $props();

	const params = page.url.searchParams;
	// Where to go after saving, e.g. back to the work order the part is for.
	// Only paths inside the app, never another site.
	const rawBack = params.get('back') ?? '';
	const back = safeNext(rawBack) === rawBack ? rawBack : '';

	let form = $state<PartForm>(initialForm());

	function initialForm(): PartForm {
		const condition = params.get('condition') === 'used' ? 'used' : 'new';
		const src = data.source;
		if (src) {
			// A copy in another condition: same catalogue data, own price,
			// stock and barcodes (the box barcode belongs to the new one).
			return {
				...partForm(src, business.currency),
				condition,
				min_stock: '0',
				price: ''
			};
		}
		// Coming from a search that found nothing: a text with digits is most
		// likely a part number, anything else a name.
		const typed = (params.get('q') ?? '').trim();
		return {
			...emptyPartForm(),
			condition,
			// A delivery line passes what the invoice says explicitly.
			name: params.get('name') ?? (/\d/.test(typed) ? '' : typed),
			part_number: params.get('part_number') ?? (/\d/.test(typed) ? typed : '')
		};
	}
	let barcode = $state(params.get('barcode') ?? '');
	let stockNow = $state('');
	let stockCost = $state('');
	let error = $state('');
	let busy = $state(false);

	const isOwner = $derived(session.user?.role === 'owner');

	function withCreated(path: string, id: string): string {
		const url = new URL(path, location.origin);
		url.searchParams.set('created', id);
		return url.pathname + url.search;
	}

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		const parsed = partBody(form, business.currency, isOwner);
		if ('error' in parsed) {
			error = t(parsed.error);
			return;
		}
		const stock = stockNow.trim() ? parseDecimal(stockNow, 3) : null;
		if (stockNow.trim() && stock === null) {
			error = t('errors.invalid_quantity');
			return;
		}
		let cost: number | undefined;
		if (isOwner && stockCost.trim()) {
			const c = parseMoney(stockCost, business.currency);
			if (c === null) {
				error = t('errors.invalid_price');
				return;
			}
			cost = c;
		}
		busy = true;
		try {
			const p = await api<PartDetail>('POST', '/parts', {
				...parsed.body,
				barcodes: barcode.trim() ? [barcode.trim()] : [],
				stock_milli: stock,
				unit_cost_minor: cost
			});
			// The page we came from learns which part was made (a delivery
			// line attaches it, stock count opens it).
			await goto(back ? withCreated(back, p.id) : `/parts/${p.id}`, { replaceState: true });
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>{t('parts.addTitle')} · Autokarta</title></svelte:head>

<h1>{t('parts.addTitle')}</h1>

<form onsubmit={submit}>
	<PartFields bind:form currency={business.currency} showPrice={isOwner} />

	<label class="field" for="barcode">{t('parts.barcode')}</label>
	<p class="hint barcode-hint">{t('parts.barcodeHint')}</p>
	<div class="with-action field-gap">
		<input
			id="barcode"
			type="text"
			class="mono"
			bind:value={barcode}
			maxlength="128"
			autocomplete="off"
			spellcheck="false"
		/>
		<ScanButton onScan={(d) => (barcode = d.text.trim())} />
	</div>

	<div class:two-cols={isOwner}>
		<label class="field">
			{t('parts.stockNow', { unit: t(`units.${form.unit}`) })}
			<input type="text" inputmode="decimal" bind:value={stockNow} maxlength="12" autocomplete="off" />
		</label>
		{#if isOwner}
			<label class="field">
				{t('parts.netCost', { currency: business.currency })}
				<input type="text" inputmode="decimal" bind:value={stockCost} maxlength="16" autocomplete="off" />
			</label>
		{/if}
	</div>

	{#if error}<div class="notice error" role="alert">{error}</div>{/if}

	<button class="btn primary block" type="submit" disabled={busy || !form.name.trim()}>{t('parts.addSubmit')}</button>
</form>

<style>
	label[for='barcode'] {
		margin-bottom: 2px;
	}
	.barcode-hint {
		margin: 0 0 6px;
	}
	.field-gap {
		margin-bottom: 14px;
	}
</style>
