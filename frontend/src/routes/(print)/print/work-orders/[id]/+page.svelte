<script lang="ts">
	// A work order on paper. Prices only come from the API for owners, so a
	// mechanic's printout simply has no price columns.
	import { onMount, tick } from 'svelte';
	import { formatDistance, formatMoney, formatQuantity, formatStock, vehicleName } from '$lib/format';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import { business } from '$lib/settings.svelte';
	import type { WorkOrderDetail } from '$lib/types';

	let { data } = $props();
	const wo: WorkOrderDetail = $derived(data.order);
	const withPrices = $derived(wo.totals !== undefined);
	const currency = $derived(wo.totals?.currency ?? business.currency);
	const printedAt = new Date();
	const org = $derived(business.organization);
	const address = $derived(org.service_address || org.legal_address);
	/** Printed in groups of four, as on bank documents. */
	const bank = $derived(org.bank_account?.replace(/(.{4})(?=.)/g, '$1 ') ?? null);

	// Opened from the "Print" button: go straight to the print dialog.
	onMount(async () => {
		await tick();
		window.print();
	});
</script>

<svelte:head>
	<title>{t('workOrders.number', { number: wo.number })} · Autokarta</title>
	<!-- In the head, not in <style>: removed when leaving the page, so it never
	     leaks into another printable page with different margins. -->
	<style>
		@page {
			size: A4;
			margin: 15mm;
		}
	</style>
</svelte:head>

<article class="sheet">
	{#if org.company_name}
		<div class="org">
			<strong>{org.company_name}</strong>
			{#if address}<div class="pre">{address}</div>{/if}
			<div>
				{#if org.tax_id}{t('organization.fields.tax_id')} {org.tax_id}{/if}{#if org.tax_id && org.regon}{' · '}{/if}{#if org.regon}{t(
						'organization.fields.regon'
					)}
					{org.regon}{/if}
			</div>
			{#if org.phone || org.email}
				<div>{[org.phone, org.email].filter(Boolean).join(' · ')}</div>
			{/if}
			{#if bank}<div class="mono">{t('organization.fields.bank_account')}: {bank}</div>{/if}
		</div>
	{/if}
	<header class="top">
		<h1>{t('workOrders.number', { number: wo.number })}</h1>
		<div class="meta">
			<div>{formatDate(wo.opened_on)}</div>
			{#if wo.status !== 'open'}<div>{t(`workOrders.statuses.${wo.status}`)}</div>{/if}
		</div>
	</header>

	<dl class="facts">
		<dt>{t('print.vehicle')}</dt>
		<dd>
			<strong>{wo.vehicle.plate ?? ''}</strong>
			{vehicleName(wo.vehicle)}
			{#if wo.vehicle.vin}<span class="mono">· {t('vehicles.vin')} {wo.vehicle.vin}</span>{/if}
			{#if wo.vehicle.frame_number}<span class="mono">· {wo.vehicle.frame_number}</span>{/if}
		</dd>
		<dt>{t('vehicles.odometer')}</dt>
		<dd>{wo.odometer !== null ? formatDistance(wo.odometer, t(`units.${wo.vehicle.odometer_unit}`)) : '—'}</dd>
		{#if wo.complaint}
			<dt>{t('workOrders.complaint')}</dt>
			<dd class="pre">{wo.complaint}</dd>
		{/if}
	</dl>

	<table>
		<thead>
			<tr>
				<th class="num">#</th>
				<th>{t('print.description')}</th>
				<th class="right">{t('print.quantity')}</th>
				{#if withPrices}
					<th class="right">{t('print.price')}</th>
					<th class="right">{t('print.amount')}</th>
				{/if}
			</tr>
		</thead>
		<tbody>
			{#each wo.items as i, n (i.id)}
				<tr>
					<td class="num">{n + 1}</td>
					<td>
						{i.description}{#if i.condition === 'used'}{' '}<strong>({t('parts.conditions.used')})</strong>{/if}
						{#if i.part_number || i.brand}
							<div class="sub">{i.brand ?? ''}{#if i.brand && i.part_number}{' · '}{/if}<span class="mono">{i.part_number ?? ''}</span></div>
						{/if}
					</td>
					<td class="right nowrap">{i.unit ? formatStock(i.quantity_milli, i.unit) : formatQuantity(i.quantity_milli)}</td>
					{#if withPrices}
						<td class="right nowrap">{i.unit_price_minor !== undefined ? formatMoney(i.unit_price_minor, currency) : '—'}</td>
						<td class="right nowrap">{i.line_total_minor !== undefined ? formatMoney(i.line_total_minor, currency) : '—'}</td>
					{/if}
				</tr>
			{:else}
				<tr><td colspan={withPrices ? 5 : 3} class="empty">{t('workOrders.noItems')}</td></tr>
			{/each}
		</tbody>
		{#if withPrices && wo.totals}
			<tfoot>
				<tr>
					<td colspan="4" class="right">{t('workOrders.total')}</td>
					<td class="right nowrap"><strong>{formatMoney(wo.totals.total_minor, currency)}</strong></td>
				</tr>
			</tfoot>
		{/if}
	</table>

	<footer class="printed">
		{t('print.printedAt', { date: formatDate(printedAt, { dateStyle: 'short', timeStyle: 'short' }) })}
	</footer>
</article>

<style>
	.org {
		margin-bottom: 8mm;
		padding-bottom: 4mm;
		border-bottom: 1px solid #000;
		font-size: 10pt;
		line-height: 1.4;
	}
	.org strong {
		font-size: 12pt;
	}
	.sheet {
		box-sizing: border-box;
		width: 210mm;
		min-height: 297mm;
		margin: 0 auto;
		padding: 15mm;
		background: #fff;
		color: #000;
		font-size: 11pt;
		line-height: 1.35;
		box-shadow: 0 1px 6px rgb(0 0 0 / 0.25);
	}
	.top {
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 12px;
		border-bottom: 2px solid #000;
		padding-bottom: 4mm;
		margin-bottom: 5mm;
	}
	h1 {
		margin: 0;
		font-size: 20pt;
	}
	.meta {
		text-align: right;
	}
	.facts {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 2mm 6mm;
		margin: 0 0 6mm;
	}
	.facts dt {
		color: #444;
	}
	.facts dd {
		margin: 0;
	}
	.pre {
		white-space: pre-wrap;
	}
	.mono {
		font-family: ui-monospace, 'SF Mono', Menlo, Consolas, monospace;
		font-size: 0.95em;
	}
	table {
		width: 100%;
		border-collapse: collapse;
	}
	th,
	td {
		border-bottom: 1px solid #999;
		padding: 2mm 1.5mm;
		text-align: left;
		vertical-align: top;
	}
	th {
		border-bottom: 2px solid #000;
		font-weight: 600;
	}
	tfoot td {
		border-bottom: none;
		border-top: 2px solid #000;
		font-size: 13pt;
	}
	tr {
		break-inside: avoid;
	}
	.num {
		width: 8mm;
		color: #444;
	}
	.right {
		text-align: right;
	}
	.nowrap {
		white-space: nowrap;
	}
	.sub {
		color: #444;
		font-size: 9.5pt;
	}
	.empty {
		color: #444;
	}
	.printed {
		margin-top: 8mm;
		color: #666;
		font-size: 9pt;
	}
	@media print {
		.sheet {
			width: auto;
			min-height: 0;
			padding: 0;
			box-shadow: none;
		}
	}
</style>
