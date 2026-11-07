<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, errorMessage } from '$lib/api';
	import ClientPicker from '$lib/components/ClientPicker.svelte';
	import Icon from '$lib/components/Icon.svelte';
	import Plate from '$lib/components/Plate.svelte';
	import Sheet from '$lib/components/Sheet.svelte';
	import VehicleFields, { vehicleBody, type VehicleForm } from '$lib/components/VehicleFields.svelte';
	import { formatDistance, formatMoney, vehicleName } from '$lib/format';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import { parseReading, withLowerConfirm } from '$lib/odometer';
	import { session } from '$lib/session.svelte';
	import { business } from '$lib/settings.svelte';
	import type { Client, VehicleDetail, WorkOrderDetail } from '$lib/types';

	let { data } = $props();
	// Follows navigation to another vehicle; local updates overwrite it.
	let v = $derived<VehicleDetail>(data.vehicle);

	const isOwner = $derived(session.user?.role === 'owner');
	const lastReading = $derived(v.odometer[0] ?? null);
	const pastOwners = $derived(v.owners.filter((o) => o.ended_on !== null));
	const unit = $derived(t(`units.${v.odometer_unit}`));
	const today = () => new Date().toISOString().slice(0, 10);

	// ----- new work order -----
	let woOpen = $state(false);
	let woOdometer = $state('');
	let woComplaint = $state('');
	let woError = $state('');
	let woBusy = $state(false);

	function openWorkOrder() {
		woOdometer = '';
		woComplaint = '';
		woError = '';
		woOpen = true;
	}

	async function createWorkOrder(e: SubmitEvent) {
		e.preventDefault();
		const reading = parseReading(woOdometer);
		if (Number.isNaN(reading)) {
			woError = t('errors.invalid_odometer');
			return;
		}
		woBusy = true;
		woError = '';
		try {
			const wo = await withLowerConfirm((confirm_lower) =>
				api<WorkOrderDetail>('POST', '/work-orders', {
					vehicle_id: v.id,
					odometer: reading,
					complaint: woComplaint,
					confirm_lower
				})
			);
			if (wo) await goto(`/work-orders/${wo.id}`);
		} catch (err) {
			woError = errorMessage(err);
		} finally {
			woBusy = false;
		}
	}

	// ----- edit -----
	let editOpen = $state(false);
	let form = $state<VehicleForm | null>(null);
	let editError = $state('');
	let editBusy = $state(false);

	function openEdit() {
		form = {
			plate: v.plate ?? '',
			vin: v.vin ?? '',
			frame_number: v.frame_number ?? '',
			make: v.make ?? '',
			model: v.model ?? '',
			year: v.year?.toString() ?? '',
			engine: v.engine ?? '',
			note: v.note ?? '',
			role: v.role,
			odometer_unit: v.odometer_unit
		};
		editError = '';
		editOpen = true;
	}

	async function saveEdit(e: SubmitEvent) {
		e.preventDefault();
		if (!form) return;
		editBusy = true;
		editError = '';
		try {
			v = await api<VehicleDetail>('PUT', `/vehicles/${v.id}`, vehicleBody(form));
			editOpen = false;
		} catch (err) {
			editError = errorMessage(err);
		} finally {
			editBusy = false;
		}
	}

	// ----- owner -----
	let ownerOpen = $state(false);
	let newOwner = $state<Client | null>(null);
	let ownerSince = $state('');
	let ownerError = $state('');
	let ownerBusy = $state(false);

	function openOwner() {
		newOwner = null;
		ownerSince = today();
		ownerError = '';
		ownerOpen = true;
	}

	async function saveOwner(clientId: string | null) {
		ownerBusy = true;
		ownerError = '';
		try {
			v = await api<VehicleDetail>('PUT', `/vehicles/${v.id}/owner`, {
				client_id: clientId,
				since: ownerSince || undefined
			});
			ownerOpen = false;
		} catch (err) {
			ownerError = errorMessage(err);
		} finally {
			ownerBusy = false;
		}
	}

	// ----- client details -----
	let clientOpen = $state(false);
	let cName = $state('');
	let cPhone = $state('');
	let cNote = $state('');
	let cError = $state('');
	let cBusy = $state(false);

	function openClient() {
		if (!v.owner) return;
		cName = v.owner.name;
		cPhone = v.owner.phone ?? '';
		cNote = v.owner.note ?? '';
		cError = '';
		clientOpen = true;
	}

	async function saveClient(e: SubmitEvent) {
		e.preventDefault();
		if (!v.owner) return;
		cBusy = true;
		cError = '';
		try {
			await api<Client>('PUT', `/clients/${v.owner.id}`, { name: cName, phone: cPhone, note: cNote });
			// The name also appears in the owner history: reload the whole car.
			v = await api<VehicleDetail>('GET', `/vehicles/${v.id}`);
			clientOpen = false;
		} catch (err) {
			cError = errorMessage(err);
		} finally {
			cBusy = false;
		}
	}

	// ----- odometer -----
	let odoOpen = $state(false);
	let odoValue = $state('');
	let odoDate = $state('');
	let odoError = $state('');
	let odoBusy = $state(false);

	function openOdometer() {
		odoValue = '';
		odoDate = today();
		odoError = '';
		odoOpen = true;
	}

	async function saveOdometer(e: SubmitEvent) {
		e.preventDefault();
		const reading = parseReading(odoValue);
		if (reading === null || Number.isNaN(reading)) {
			odoError = t('errors.invalid_odometer');
			return;
		}
		odoBusy = true;
		odoError = '';
		try {
			const updated = await withLowerConfirm((confirm_lower) =>
				api<VehicleDetail>('POST', `/vehicles/${v.id}/odometer`, {
					reading,
					read_on: odoDate || undefined,
					confirm_lower
				})
			);
			if (updated) {
				v = updated;
				odoOpen = false;
			}
		} catch (err) {
			odoError = errorMessage(err);
		} finally {
			odoBusy = false;
		}
	}

	let historyError = $state('');
	async function deleteReading(id: string) {
		if (!confirm(t('odometer.confirmDelete'))) return;
		historyError = '';
		try {
			v = await api<VehicleDetail>('DELETE', `/vehicles/${v.id}/odometer/${id}`);
		} catch (err) {
			historyError = errorMessage(err);
		}
	}
</script>

<svelte:head><title>{v.plate ?? v.vin ?? v.frame_number} · Autokarta</title></svelte:head>

<header class="head">
	<Plate plate={v.plate} fallback={v.vin ?? v.frame_number} size="lg" />
	<h1>{vehicleName(v) || t('vehicles.unnamed')}</h1>
	{#if v.role !== 'customer'}<span class="badge accent">{t(`vehicles.roles.${v.role}`)}</span>{/if}
</header>

<button class="btn primary block big" type="button" onclick={openWorkOrder}>
	<Icon name="plus" />{t('workOrders.new')}
</button>

<section class="card">
	<dl class="facts">
		{#if v.vin}<dt>{t('vehicles.vin')}</dt><dd class="mono">{v.vin}</dd>{/if}
		{#if v.frame_number}<dt>{t('vehicles.frameNumber')}</dt><dd class="mono">{v.frame_number}</dd>{/if}
		{#if v.plate && (v.vin || v.frame_number)}<dt>{t('vehicles.plate')}</dt><dd>{v.plate}</dd>{/if}
		{#if v.engine}<dt>{t('vehicles.engine')}</dt><dd>{v.engine}</dd>{/if}
		<dt>{t('vehicles.odometer')}</dt>
		<dd>
			{#if lastReading}
				<strong>{formatDistance(lastReading.reading, unit)}</strong>
				<span class="hint">· {formatDate(lastReading.read_on)}</span>
			{:else}
				<span class="hint">{t('vehicles.noOdometer')}</span>
			{/if}
		</dd>
	</dl>
	{#if v.note}<p class="note">{v.note}</p>{/if}
	<div class="actions">
		<button class="btn" type="button" onclick={openOdometer}>{t('odometer.add')}</button>
		<button class="btn" type="button" onclick={openEdit}>{t('vehicles.edit')}</button>
	</div>
</section>

<section class="card">
	<h2>{t('vehicles.owner')}</h2>
	{#if v.owner}
		<p class="owner">
			<strong>{v.owner.name}</strong>
			{#if v.owner.phone}<a class="btn phone" href="tel:{v.owner.phone.replace(/[^\d+]/g, '')}">{v.owner.phone}</a>{/if}
		</p>
		{#if v.owner.note}<p class="hint">{v.owner.note}</p>{/if}
	{:else}
		<p class="hint">{t('vehicles.noOwner')}</p>
	{/if}
	<div class="actions">
		{#if v.owner}<button class="btn" type="button" onclick={openClient}>{t('clients.edit')}</button>{/if}
		<button class="btn" type="button" onclick={openOwner}>{v.owner ? t('vehicles.changeOwner') : t('vehicles.setOwner')}</button>
	</div>
	{#if pastOwners.length}
		<details>
			<summary>{t('vehicles.pastOwners', { count: pastOwners.length })}</summary>
			<ul class="plain">
				{#each pastOwners as o (o.client_id + o.started_on)}
					<li>
						{o.name}{#if o.phone}, {o.phone}{/if}
						<span class="hint">
							· {t('vehicles.ownedFromTo', { from: formatDate(o.started_on), to: formatDate(o.ended_on ?? '') })}
						</span>
					</li>
				{/each}
			</ul>
		</details>
	{/if}
</section>

<div class="section-head"><h2>{t('workOrders.title')}</h2></div>
{#if v.work_orders.length === 0}
	<p class="hint">{t('vehicles.noWorkOrders')}</p>
{:else}
	<ul class="rows">
		{#each v.work_orders as w (w.id)}
			<li>
				<a class="row-link" href="/work-orders/{w.id}">
					<span class="row-main">
						<span><strong>{t('workOrders.number', { number: w.number })}</strong> · {formatDate(w.opened_on)}</span>
						{#if w.complaint}<span class="clip">{w.complaint}</span>{/if}
						<span class="hint">
							{t('workOrders.itemCount', { count: w.item_count })}{#if w.odometer !== null}{' · ' +
									formatDistance(w.odometer, unit)}{/if}
						</span>
					</span>
					<span class="row-side">
						<span class="status {w.status}">{t(`workOrders.statuses.${w.status}`)}</span>
						{#if w.total_minor !== undefined && w.total_minor !== null}
							<span>{formatMoney(w.total_minor, business.currency)}</span>
						{/if}
					</span>
				</a>
			</li>
		{/each}
	</ul>
{/if}

{#if v.odometer.length > 1}
	<details class="history">
		<summary>{t('odometer.history')}</summary>
		{#if historyError}<div class="notice error" role="alert">{historyError}</div>{/if}
		<ul class="plain">
			{#each v.odometer as r (r.id)}
				<li>
					<span>
						<strong>{formatDistance(r.reading, unit)}</strong>
						<span class="hint">· {formatDate(r.read_on)}</span>
						{#if r.work_order_number}
							<a href="/work-orders/{r.work_order_id}">{t('workOrders.number', { number: r.work_order_number })}</a>
						{/if}
					</span>
					{#if isOwner && !r.work_order_id}
						<button class="link-danger" type="button" onclick={() => deleteReading(r.id)}>{t('common.delete')}</button>
					{/if}
				</li>
			{/each}
		</ul>
	</details>
{/if}

<Sheet bind:open={woOpen} title={t('workOrders.new')}>
	<form onsubmit={createWorkOrder}>
		{#if woError}<div class="notice error" role="alert">{woError}</div>{/if}
		<label class="field">
			{t('workOrders.odometerNow', { unit })}
			{#if lastReading}<span class="hint">{t('odometer.last', { value: formatDistance(lastReading.reading, unit) })}</span>{/if}
			<input type="text" inputmode="numeric" bind:value={woOdometer} maxlength="9" autocomplete="off" />
		</label>
		<label class="field">
			{t('workOrders.complaint')}
			<span class="hint">{t('workOrders.complaintHint')}</span>
			<textarea bind:value={woComplaint} maxlength="2000"></textarea>
		</label>
		<button class="btn primary block" type="submit" disabled={woBusy}>{t('workOrders.create')}</button>
	</form>
</Sheet>

<Sheet bind:open={clientOpen} title={t('clients.edit')}>
	<form onsubmit={saveClient}>
		{#if cError}<div class="notice error" role="alert">{cError}</div>{/if}
		<p class="hint">{t('clients.editHint')}</p>
		<label class="field">
			{t('clients.name')}
			<input type="text" bind:value={cName} maxlength="120" required autocomplete="off" />
		</label>
		<label class="field">
			{t('clients.phone')}
			<input type="tel" bind:value={cPhone} maxlength="32" autocomplete="off" />
		</label>
		<label class="field">
			{t('clients.note')}
			<textarea bind:value={cNote} maxlength="2000"></textarea>
		</label>
		<button class="btn primary block" type="submit" disabled={cBusy || !cName.trim()}>{t('common.saveChanges')}</button>
	</form>
</Sheet>

<Sheet bind:open={odoOpen} title={t('odometer.add')}>
	<form onsubmit={saveOdometer}>
		{#if odoError}<div class="notice error" role="alert">{odoError}</div>{/if}
		<label class="field">
			{t('odometer.reading', { unit })}
			{#if lastReading}<span class="hint">{t('odometer.last', { value: formatDistance(lastReading.reading, unit) })}</span>{/if}
			<input type="text" inputmode="numeric" bind:value={odoValue} maxlength="9" autocomplete="off" required />
		</label>
		<label class="field">
			{t('odometer.date')}
			<input type="date" bind:value={odoDate} max={today()} />
		</label>
		<button class="btn primary block" type="submit" disabled={odoBusy}>{t('odometer.save')}</button>
	</form>
</Sheet>

<Sheet bind:open={ownerOpen} title={v.owner ? t('vehicles.changeOwner') : t('vehicles.setOwner')}>
	{#if ownerError}<div class="notice error" role="alert">{ownerError}</div>{/if}
	{#if v.owner}<p class="hint">{t('vehicles.changeOwnerHint')}</p>{/if}
	<ClientPicker bind:selected={newOwner} label={t('vehicles.newOwner')} />
	<label class="field">
		{t('vehicles.ownerSince')}
		<input type="date" bind:value={ownerSince} max={today()} />
	</label>
	<button
		class="btn primary block"
		type="button"
		disabled={ownerBusy || !newOwner}
		onclick={() => saveOwner(newOwner?.id ?? null)}>{t('common.saveChanges')}</button
	>
	{#if v.owner}
		<button class="btn block gap" type="button" disabled={ownerBusy} onclick={() => saveOwner(null)}
			>{t('vehicles.removeOwner')}</button
		>
	{/if}
</Sheet>

<Sheet bind:open={editOpen} title={t('vehicles.edit')}>
	{#if form}
		<form onsubmit={saveEdit}>
			<VehicleFields bind:form />
			{#if editError}<div class="notice error" role="alert">{editError}</div>{/if}
			<button class="btn primary block" type="submit" disabled={editBusy}>{t('common.saveChanges')}</button>
		</form>
	{/if}
</Sheet>

<style>
	.head {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 10px 14px;
		margin-bottom: 16px;
	}
	.head h1 {
		margin: 0;
		font-size: 1.6rem;
	}
	.big {
		min-height: 56px;
		font-size: 1.1rem;
		margin-bottom: 16px;
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
	.note {
		white-space: pre-wrap;
		border-left: 3px solid var(--line-strong);
		padding-left: 10px;
	}
	.owner {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
		font-size: 1.1rem;
	}
	.phone {
		font-weight: 600;
	}
	details {
		margin-top: 12px;
	}
	summary {
		min-height: var(--tap);
		display: flex;
		align-items: center;
		cursor: pointer;
		font-weight: 600;
		color: var(--accent);
	}
	.history {
		margin-bottom: 16px;
	}
	.plain {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	.plain li {
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
		padding: 8px 0;
		border-bottom: 1px solid var(--line);
	}
	.clip {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.link-danger {
		min-height: var(--tap);
		padding: 0 8px;
		font: inherit;
		color: var(--danger);
		background: none;
		border: none;
		cursor: pointer;
	}
	.gap {
		margin-top: 10px;
	}
</style>
