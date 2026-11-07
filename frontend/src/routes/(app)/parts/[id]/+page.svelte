<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, errorMessage } from '$lib/api';
	import PartFields, { partBody, partForm, type PartForm } from '$lib/components/PartFields.svelte';
	import ScanButton from '$lib/components/ScanButton.svelte';
	import Sheet from '$lib/components/Sheet.svelte';
	import { formatMoney, formatStock, parseDecimal, parseMoney, parseQuantity, quantityInput } from '$lib/format';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import { newOwnCode, ownCode } from '$lib/labels';
	import { session } from '$lib/session.svelte';
	import { business } from '$lib/settings.svelte';
	import type { PartDetail } from '$lib/types';

	let { data } = $props();
	// Follows navigation to another part; local updates overwrite it.
	let p = $derived<PartDetail>(data.part);

	const isOwner = $derived(session.user?.role === 'owner');
	const currency = $derived(business.currency);
	const unit = $derived(t(`units.${p.unit}`));
	const low = $derived(p.min_stock_milli > 0 && p.stock_milli < p.min_stock_milli);

	// ----- delivery -----
	let receiveOpen = $state(false);
	let rQty = $state('');
	let rCost = $state('');
	let rNote = $state('');
	let rError = $state('');
	let rBusy = $state(false);

	function openReceive() {
		rQty = '';
		rCost = '';
		rNote = '';
		rError = '';
		receiveOpen = true;
	}

	async function receive(e: SubmitEvent) {
		e.preventDefault();
		const qty = parseQuantity(rQty);
		if (qty === null) {
			rError = t('errors.invalid_quantity');
			return;
		}
		let cost: number | undefined;
		if (isOwner && rCost.trim()) {
			const c = parseMoney(rCost, currency);
			if (c === null) {
				rError = t('errors.invalid_price');
				return;
			}
			cost = c;
		}
		rBusy = true;
		rError = '';
		try {
			p = await api<PartDetail>('POST', `/parts/${p.id}/movements`, {
				kind: 'receipt',
				quantity_milli: qty,
				unit_cost_minor: cost,
				note: rNote
			});
			receiveOpen = false;
		} catch (err) {
			rError = errorMessage(err);
		} finally {
			rBusy = false;
		}
	}

	// ----- stock count -----
	let countOpen = $state(false);
	let cQty = $state('');
	let cError = $state('');
	let cBusy = $state(false);

	function openCount() {
		cQty = p.stock_milli >= 0 ? quantityInput(p.stock_milli) : '';
		cError = '';
		countOpen = true;
	}

	async function count(e: SubmitEvent) {
		e.preventDefault();
		const qty = parseDecimal(cQty, 3);
		if (qty === null) {
			cError = t('errors.invalid_quantity');
			return;
		}
		cBusy = true;
		cError = '';
		try {
			p = await api<PartDetail>('POST', `/parts/${p.id}/movements`, { kind: 'count', quantity_milli: qty });
			countOpen = false;
		} catch (err) {
			cError = errorMessage(err);
		} finally {
			cBusy = false;
		}
	}

	// ----- edit -----
	let editOpen = $state(false);
	let form = $state<PartForm | null>(null);
	let editError = $state('');
	let editBusy = $state(false);

	function openEdit() {
		form = partForm(p, currency);
		editError = '';
		editOpen = true;
	}

	async function saveEdit(e: SubmitEvent) {
		e.preventDefault();
		if (!form) return;
		const parsed = partBody(form, currency, isOwner);
		if ('error' in parsed) {
			editError = t(parsed.error);
			return;
		}
		editBusy = true;
		editError = '';
		try {
			p = await api<PartDetail>('PUT', `/parts/${p.id}`, parsed.body);
			editOpen = false;
		} catch (err) {
			editError = errorMessage(err);
		} finally {
			editBusy = false;
		}
	}

	async function deletePart() {
		if (!confirm(t('parts.confirmDelete'))) return;
		editBusy = true;
		editError = '';
		try {
			await api('DELETE', `/parts/${p.id}`);
			await goto('/parts', { replaceState: true });
		} catch (err) {
			editError = errorMessage(err);
		} finally {
			editBusy = false;
		}
	}

	// ----- barcodes -----
	let newBarcode = $state('');
	let bError = $state('');
	let bBusy = $state(false);

	async function addBarcode(code: string) {
		if (!code.trim()) return;
		bBusy = true;
		bError = '';
		try {
			p = await api<PartDetail>('POST', `/parts/${p.id}/barcodes`, { barcode: code });
			newBarcode = '';
		} catch (err) {
			bError = errorMessage(err);
		} finally {
			bBusy = false;
		}
	}

	// The first label of a part gets the part its own code; later labels
	// reuse it, so every printed label of the part scans the same.
	let labelBusy = $state(false);
	async function printLabel() {
		labelBusy = true;
		bError = '';
		try {
			if (!ownCode(p.barcodes)) {
				p = await api<PartDetail>('POST', `/parts/${p.id}/barcodes`, { barcode: newOwnCode() });
			}
			await goto(`/print/labels?part=${p.id}`);
		} catch (err) {
			bError = errorMessage(err);
		} finally {
			labelBusy = false;
		}
	}

	async function removeBarcode(code: string) {
		if (!confirm(t('parts.confirmRemoveBarcode', { code }))) return;
		bError = '';
		try {
			p = await api<PartDetail>('DELETE', `/parts/${p.id}/barcodes`, { barcode: code });
		} catch (err) {
			bError = errorMessage(err);
		}
	}
</script>

<svelte:head><title>{p.name} · Autokarta</title></svelte:head>

<header class="head">
	<h1>{p.name}</h1>
	{#if p.condition === 'used'}<span class="badge accent">{t('parts.conditions.used')}</span>{/if}
	{#if p.brand || p.part_number}
		<p class="hint ident">
			{p.brand ?? ''}{#if p.brand && p.part_number}{' · '}{/if}<span class="mono">{p.part_number ?? ''}</span>
		</p>
	{/if}
</header>

<section class="card stock-card">
	<div class="stock">
		<span class="hint">{t('parts.stock')}</span>
		<strong class="amount" class:danger={low || p.stock_milli < 0}>{formatStock(p.stock_milli, p.unit)}</strong>
		{#if low}<span class="badge danger">{t('parts.low')}</span>{/if}
	</div>
	<dl class="facts">
		{#if p.location}<dt>{t('parts.location')}</dt><dd>{p.location}</dd>{/if}
		{#if p.min_stock_milli > 0}<dt>{t('parts.minStock')}</dt><dd>{formatStock(p.min_stock_milli, p.unit)}</dd>{/if}
		{#if isOwner && p.sell_price_minor !== undefined && p.sell_price_minor !== null}
			<dt>{t('parts.sellPrice', { currency })}</dt>
			<dd>{formatMoney(p.sell_price_minor, currency)}</dd>
		{/if}
	</dl>
	{#if p.note}<p class="note">{p.note}</p>{/if}
	<button class="btn primary block" type="button" onclick={openReceive}>{t('parts.receive')}</button>
	<div class="actions">
		<button class="btn" type="button" onclick={openCount}>{t('parts.count')}</button>
		<button class="btn" type="button" onclick={openEdit}>{t('parts.edit')}</button>
		<button class="btn" type="button" disabled={labelBusy} onclick={printLabel}>{t('labels.print')}</button>
		{#if p.condition === 'new'}
			<a class="btn" href="/parts/new?from={p.id}&condition=used">{t('parts.addUsed')}</a>
		{/if}
	</div>
</section>

<section class="card">
	<h2>{t('parts.barcodes')}</h2>
	{#if bError}<div class="notice error" role="alert">{bError}</div>{/if}
	{#if p.barcodes.length === 0}
		<p class="hint">{t('parts.noBarcodes')}</p>
	{:else}
		<ul class="codes">
			{#each p.barcodes as code (code)}
				<li>
					<span class="mono">{code}</span>
					<button
						class="remove"
						type="button"
						aria-label={t('parts.removeBarcode', { code })}
						onclick={() => removeBarcode(code)}
					>
						<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
	<form
		class="with-action"
		onsubmit={(e) => {
			e.preventDefault();
			addBarcode(newBarcode);
		}}
	>
		<label class="visually-hidden" for="new-barcode">{t('parts.addBarcode')}</label>
		<input
			id="new-barcode"
			type="text"
			class="mono"
			bind:value={newBarcode}
			maxlength="128"
			autocomplete="off"
			spellcheck="false"
			enterkeyhint="done"
			placeholder={t('parts.addBarcode')}
		/>
		<ScanButton onScan={(d) => addBarcode(d.text)} />
	</form>
	{#if newBarcode.trim()}
		<button class="btn block gap" type="button" disabled={bBusy} onclick={() => addBarcode(newBarcode)}
			>{t('parts.addBarcode')}</button
		>
	{/if}
</section>

<div class="section-head"><h2>{t('parts.history')}</h2></div>
{#if p.movements.length === 0}
	<p class="hint">{t('parts.noHistory')}</p>
{:else}
	<ul class="rows">
		{#each p.movements as m (m.id)}
			<li class="movement">
				<span class="row-main">
					<span>
						<strong>{t(`parts.kinds.${m.kind}`)}</strong>
						{#if m.work_order_id && m.work_order_number !== null}
							· <a href="/work-orders/{m.work_order_id}">{t('workOrders.number', { number: m.work_order_number })}</a>
						{/if}
					</span>
					<span class="hint">
						{formatDate(m.created_at)}{#if m.created_by_name}{' · ' + m.created_by_name}{/if}{#if m.unit_cost_minor !== undefined}{' · ' +
								t('parts.costEach', { price: formatMoney(m.unit_cost_minor, currency) })}{/if}
					</span>
					{#if m.note}<span class="hint">{m.note}</span>{/if}
				</span>
				<span class="row-side">
					<strong class:danger={m.quantity_milli < 0}
						>{m.quantity_milli > 0 ? '+' : '−'}{formatStock(Math.abs(m.quantity_milli), p.unit)}</strong
					>
				</span>
			</li>
		{/each}
	</ul>
{/if}

<Sheet bind:open={receiveOpen} title={t('parts.receive')}>
	<form onsubmit={receive}>
		{#if rError}<div class="notice error" role="alert">{rError}</div>{/if}
		<label class="field">
			{t('parts.receiveQty', { unit })}
			<input type="text" inputmode="decimal" bind:value={rQty} maxlength="12" autocomplete="off" required />
		</label>
		{#if isOwner}
			<label class="field">
				{t('parts.unitCost', { currency })}
				<input type="text" inputmode="decimal" bind:value={rCost} maxlength="16" autocomplete="off" />
			</label>
		{/if}
		<label class="field">
			{t('parts.movementNote')}
			<input type="text" bind:value={rNote} maxlength="500" autocomplete="off" placeholder={t('parts.movementNotePlaceholder')} />
		</label>
		<button class="btn primary block" type="submit" disabled={rBusy}>{t('parts.receiveSubmit')}</button>
	</form>
</Sheet>

<Sheet bind:open={countOpen} title={t('parts.count')}>
	<form onsubmit={count}>
		{#if cError}<div class="notice error" role="alert">{cError}</div>{/if}
		<p class="hint">{t('parts.countHint')}</p>
		<label class="field">
			{t('parts.countQty', { unit })}
			<input type="text" inputmode="decimal" bind:value={cQty} maxlength="12" autocomplete="off" required />
		</label>
		<button class="btn primary block" type="submit" disabled={cBusy}>{t('parts.countSubmit')}</button>
	</form>
</Sheet>

<Sheet bind:open={editOpen} title={t('parts.edit')}>
	{#if form}
		<form onsubmit={saveEdit}>
			{#if editError}<div class="notice error" role="alert">{editError}</div>{/if}
			<PartFields bind:form currency={business.currency} showPrice={isOwner} />
			<button class="btn primary block" type="submit" disabled={editBusy || !form.name.trim()}
				>{t('common.saveChanges')}</button
			>
			{#if isOwner}
				<button class="btn danger block gap" type="button" disabled={editBusy} onclick={deletePart}
					>{t('parts.delete')}</button
				>
			{/if}
		</form>
	{/if}
</Sheet>

<style>
	.head h1 {
		margin: 0;
		overflow-wrap: anywhere;
	}
	.ident {
		margin: 4px 0 14px;
	}
	.stock {
		display: flex;
		flex-wrap: wrap;
		align-items: baseline;
		gap: 4px 12px;
		margin-bottom: 10px;
	}
	.stock .hint {
		flex-basis: 100%;
	}
	.amount {
		font-family: var(--font-head);
		font-size: 2.4rem;
		line-height: 1;
	}
	.danger {
		color: var(--danger);
	}
	.facts {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 6px 14px;
		margin: 0 0 12px;
	}
	.facts dt {
		color: var(--ink-muted);
	}
	.facts dd {
		margin: 0;
	}
	.note {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.codes {
		list-style: none;
		padding: 0;
		margin: 0 0 12px;
	}
	.codes li {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		border-bottom: 1px solid var(--line);
	}
	.codes .mono {
		overflow-wrap: anywhere;
	}
	.remove {
		width: var(--tap);
		height: var(--tap);
		flex: none;
		display: grid;
		place-items: center;
		background: transparent;
		border: none;
		color: var(--danger);
		cursor: pointer;
	}
	.remove svg {
		width: 22px;
		height: 22px;
		stroke: currentColor;
		stroke-width: 2.4;
		stroke-linecap: round;
	}
	.movement {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
		padding: 10px var(--gutter);
	}
	.gap {
		margin-top: 10px;
	}
</style>
