<script lang="ts">
	import { tick } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import Plate from '$lib/components/Plate.svelte';
	import Sheet from '$lib/components/Sheet.svelte';
	import {
		formatDistance,
		formatMoney,
		formatQuantity,
		moneyInput,
		parseMoney,
		parseQuantity,
		quantityInput,
		vehicleName
	} from '$lib/format';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import { parseReading, withLowerConfirm } from '$lib/odometer';
	import { session } from '$lib/session.svelte';
	import { business } from '$lib/settings.svelte';
	import type { WorkOrderDetail, WorkOrderItem, WorkOrderStatus } from '$lib/types';

	let { data } = $props();
	// Follows navigation to another order; local updates overwrite it.
	let wo = $derived<WorkOrderDetail>(data.order);

	const isOwner = $derived(session.user?.role === 'owner');
	const isOpen = $derived(wo.status === 'open');
	const unit = $derived(t(`units.${wo.vehicle.odometer_unit}`));
	const currency = $derived(wo.totals?.currency ?? business.currency);
	const base = $derived(`/work-orders/${wo.id}`);

	let error = $state('');

	// ----- quick add -----
	let desc = $state('');
	let qty = $state('1');
	let price = $state('');
	let addBusy = $state(false);
	let descInput = $state<HTMLInputElement>();

	async function addItem(e: SubmitEvent) {
		e.preventDefault();
		const quantity = parseQuantity(qty);
		if (quantity === null) {
			error = t('errors.invalid_quantity');
			return;
		}
		let unitPrice: number | undefined;
		if (isOwner && price.trim()) {
			const p = parseMoney(price, currency);
			if (p === null) {
				error = t('errors.invalid_price');
				return;
			}
			unitPrice = p;
		}
		addBusy = true;
		error = '';
		try {
			wo = await api<WorkOrderDetail>('POST', `${base}/items`, {
				description: desc,
				quantity_milli: quantity,
				unit_price_minor: unitPrice
			});
			desc = '';
			qty = '1';
			price = '';
			await tick();
			descInput?.focus();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			addBusy = false;
		}
	}

	// ----- edit item -----
	let itemOpen = $state(false);
	let item = $state<WorkOrderItem | null>(null);
	let itemDesc = $state('');
	let itemQty = $state('');
	let itemPrice = $state('');
	let itemError = $state('');
	let itemBusy = $state(false);

	function openItem(i: WorkOrderItem) {
		if (!isOpen) return;
		item = i;
		itemDesc = i.description;
		itemQty = quantityInput(i.quantity_milli);
		itemPrice = moneyInput(i.unit_price_minor, currency);
		itemError = '';
		itemOpen = true;
	}

	async function saveItem(e: SubmitEvent) {
		e.preventDefault();
		if (!item) return;
		const quantity = parseQuantity(itemQty);
		if (quantity === null) {
			itemError = t('errors.invalid_quantity');
			return;
		}
		const body: Record<string, unknown> = { description: itemDesc, quantity_milli: quantity };
		if (isOwner) {
			if (itemPrice.trim()) {
				const p = parseMoney(itemPrice, currency);
				if (p === null) {
					itemError = t('errors.invalid_price');
					return;
				}
				body.unit_price_minor = p;
			} else {
				body.unit_price_minor = null;
			}
		}
		itemBusy = true;
		itemError = '';
		try {
			wo = await api<WorkOrderDetail>('PATCH', `${base}/items/${item.id}`, body);
			itemOpen = false;
		} catch (err) {
			itemError = errorMessage(err);
		} finally {
			itemBusy = false;
		}
	}

	async function deleteItem() {
		if (!item || !confirm(t('workOrders.confirmDeleteItem'))) return;
		itemBusy = true;
		try {
			wo = await api<WorkOrderDetail>('DELETE', `${base}/items/${item.id}`);
			itemOpen = false;
		} catch (err) {
			itemError = errorMessage(err);
		} finally {
			itemBusy = false;
		}
	}

	// ----- details -----
	let detailsOpen = $state(false);
	let dOdometer = $state('');
	let dComplaint = $state('');
	let dNotes = $state('');
	let dError = $state('');
	let dBusy = $state(false);

	function openDetails() {
		dOdometer = wo.odometer?.toString() ?? '';
		dComplaint = wo.complaint ?? '';
		dNotes = wo.notes ?? '';
		dError = '';
		detailsOpen = true;
	}

	async function saveDetails(e: SubmitEvent) {
		e.preventDefault();
		const reading = parseReading(dOdometer);
		if (Number.isNaN(reading)) {
			dError = t('errors.invalid_odometer');
			return;
		}
		dBusy = true;
		dError = '';
		try {
			const updated = await withLowerConfirm((confirm_lower) =>
				api<WorkOrderDetail>('PATCH', base, {
					complaint: dComplaint,
					notes: dNotes,
					odometer: reading ?? undefined,
					confirm_lower
				})
			);
			if (updated) {
				wo = updated;
				detailsOpen = false;
			}
		} catch (err) {
			dError = errorMessage(err);
		} finally {
			dBusy = false;
		}
	}

	// ----- status -----
	let statusBusy = $state(false);
	async function setStatus(status: WorkOrderStatus) {
		if (status === 'cancelled' && !confirm(t('workOrders.confirmCancel'))) return;
		statusBusy = true;
		error = '';
		try {
			wo = await api<WorkOrderDetail>('PATCH', base, { status });
		} catch (err) {
			error = errorMessage(err);
		} finally {
			statusBusy = false;
		}
	}
</script>

<svelte:head><title>{t('workOrders.number', { number: wo.number })} · Autokarta</title></svelte:head>

<header class="head">
	<h1>{t('workOrders.number', { number: wo.number })}</h1>
	<span class="status {wo.status}">{t(`workOrders.statuses.${wo.status}`)}</span>
</header>
<p class="hint meta">
	{formatDate(wo.opened_on)}{#if wo.created_by_name}{' · ' + wo.created_by_name}{/if}
</p>

<a class="vehicle card" href="/vehicles/{wo.vehicle.id}">
	<Plate plate={wo.vehicle.plate} fallback={wo.vehicle.vin ?? wo.vehicle.frame_number} />
	<span class="vname">{vehicleName(wo.vehicle) || t('vehicles.unnamed')}</span>
	{#if wo.client}
		<span class="hint client">{wo.client.name}{#if wo.client.phone}{' · ' + wo.client.phone}{/if}</span>
	{/if}
</a>

<section class="card">
	<dl class="facts">
		<dt>{t('vehicles.odometer')}</dt>
		<dd>{wo.odometer !== null ? formatDistance(wo.odometer, unit) : t('workOrders.notRecorded')}</dd>
		<dt>{t('workOrders.complaint')}</dt>
		<dd class="pre">{wo.complaint ?? t('workOrders.notRecorded')}</dd>
		{#if wo.notes}
			<dt>{t('workOrders.notes')}</dt>
			<dd class="pre">{wo.notes}</dd>
		{/if}
	</dl>
	<div class="actions">
		<button class="btn" type="button" onclick={openDetails}>{t('workOrders.editDetails')}</button>
	</div>
</section>

<div class="section-head"><h2>{t('workOrders.items')}</h2></div>

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

{#if wo.items.length === 0}
	<p class="hint">{t('workOrders.noItems')}</p>
{:else}
	<ul class="rows">
		{#each wo.items as i (i.id)}
			<li>
				<button class="row-link" type="button" onclick={() => openItem(i)} disabled={!isOpen}>
					<span class="row-main">
						<span class="desc">{i.description}</span>
						{#if i.quantity_milli !== 1000}
							<span class="hint">
								× {formatQuantity(i.quantity_milli)}{#if isOwner && i.unit_price_minor !== undefined}{' · ' +
										formatMoney(i.unit_price_minor, currency)}{/if}
							</span>
						{/if}
					</span>
					{#if isOwner}
						<span class="row-side">
							{#if i.line_total_minor !== undefined}
								<strong>{formatMoney(i.line_total_minor, currency)}</strong>
							{:else}
								<span class="badge muted">{t('workOrders.noPrice')}</span>
							{/if}
						</span>
					{/if}
				</button>
			</li>
		{/each}
	</ul>
{/if}

{#if isOwner && wo.totals}
	<div class="total">
		<span>{t('workOrders.total')}</span>
		<strong>{formatMoney(wo.totals.total_minor, currency)}</strong>
	</div>
	{#if wo.totals.unpriced_items > 0}
		<p class="hint right">{t('workOrders.unpriced', { count: wo.totals.unpriced_items })}</p>
	{/if}
{/if}

{#if isOpen}
	<form class="add card" onsubmit={addItem}>
		<label class="field">
			{t('workOrders.addWork')}
			<input
				bind:this={descInput}
				type="text"
				bind:value={desc}
				maxlength="500"
				required
				autocomplete="off"
				enterkeyhint="done"
				placeholder={t('workOrders.addWorkPlaceholder')}
			/>
		</label>
		<div class="two-cols">
			<label class="field">
				{t('workOrders.quantity')}
				<input type="text" inputmode="decimal" bind:value={qty} maxlength="12" autocomplete="off" />
			</label>
			{#if isOwner}
				<label class="field">
					{t('workOrders.unitPrice', { currency })}
					<input type="text" inputmode="decimal" bind:value={price} maxlength="16" autocomplete="off" />
				</label>
			{/if}
		</div>
		<button class="btn primary block" type="submit" disabled={addBusy || !desc.trim()}>{t('workOrders.addItem')}</button>
	</form>

	<div class="actions status-actions">
		<button class="btn primary" type="button" disabled={statusBusy} onclick={() => setStatus('done')}
			>{t('workOrders.markDone')}</button
		>
		<button class="btn danger" type="button" disabled={statusBusy} onclick={() => setStatus('cancelled')}
			>{t('workOrders.cancel')}</button
		>
	</div>
{:else}
	<p class="hint">{t('workOrders.closedHint')}</p>
	<div class="actions">
		<button class="btn" type="button" disabled={statusBusy} onclick={() => setStatus('open')}>{t('workOrders.reopen')}</button>
	</div>
{/if}

<Sheet bind:open={itemOpen} title={t('workOrders.editItem')}>
	<form onsubmit={saveItem}>
		{#if itemError}<div class="notice error" role="alert">{itemError}</div>{/if}
		<label class="field">
			{t('workOrders.description')}
			<input type="text" bind:value={itemDesc} maxlength="500" required autocomplete="off" />
		</label>
		<div class="two-cols">
			<label class="field">
				{t('workOrders.quantity')}
				<input type="text" inputmode="decimal" bind:value={itemQty} maxlength="12" autocomplete="off" />
			</label>
			{#if isOwner}
				<label class="field">
					{t('workOrders.unitPrice', { currency })}
					<input type="text" inputmode="decimal" bind:value={itemPrice} maxlength="16" autocomplete="off" />
				</label>
			{/if}
		</div>
		<button class="btn primary block" type="submit" disabled={itemBusy}>{t('common.saveChanges')}</button>
		<button class="btn danger block gap" type="button" disabled={itemBusy} onclick={deleteItem}
			>{t('workOrders.deleteItem')}</button
		>
	</form>
</Sheet>

<Sheet bind:open={detailsOpen} title={t('workOrders.editDetails')}>
	<form onsubmit={saveDetails}>
		{#if dError}<div class="notice error" role="alert">{dError}</div>{/if}
		<label class="field">
			{t('workOrders.odometerNow', { unit })}
			<input type="text" inputmode="numeric" bind:value={dOdometer} maxlength="9" autocomplete="off" />
		</label>
		<label class="field">
			{t('workOrders.complaint')}
			<textarea bind:value={dComplaint} maxlength="2000"></textarea>
		</label>
		<label class="field">
			{t('workOrders.notes')}
			<span class="hint">{t('workOrders.notesHint')}</span>
			<textarea bind:value={dNotes} maxlength="4000"></textarea>
		</label>
		<button class="btn primary block" type="submit" disabled={dBusy}>{t('common.saveChanges')}</button>
	</form>
</Sheet>

<style>
	.head {
		display: flex;
		align-items: center;
		gap: 12px;
		flex-wrap: wrap;
	}
	.head h1 {
		margin: 0;
	}
	.meta {
		margin: 4px 0 14px;
	}
	.vehicle {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 6px 12px;
		color: inherit;
		text-decoration: none;
	}
	.vehicle:hover {
		border-color: var(--accent);
	}
	.vname {
		font-weight: 600;
	}
	.client {
		flex-basis: 100%;
	}
	.facts {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 6px 14px;
		margin: 0 0 8px;
	}
	.facts dt {
		color: var(--ink-muted);
	}
	.facts dd {
		margin: 0;
		min-width: 0;
	}
	.pre {
		white-space: pre-wrap;
		overflow-wrap: anywhere;
	}
	.desc {
		font-weight: 500;
		overflow-wrap: anywhere;
	}
	.row-link:disabled {
		cursor: default;
	}
	.total {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		padding: 0 var(--gutter);
		font-size: 1.2rem;
	}
	.right {
		text-align: right;
		padding: 0 var(--gutter);
	}
	.add {
		margin-top: 16px;
	}
	.status-actions {
		margin-bottom: 16px;
	}
	.gap {
		margin-top: 10px;
	}
</style>
