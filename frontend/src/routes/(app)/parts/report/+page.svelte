<script lang="ts">
	// Stock report for the owner and the accountant: per month what was
	// bought and what went into work orders, and what is on the shelves on
	// a given day. CSV files are built here so headers are translated.
	import { onMount } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import { csvMoney, csvQuantity, downloadCsv } from '$lib/csv';
	import { formatMoney, formatStock, localToday } from '$lib/format';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import type { MonthsReport, StockReport } from '$lib/types';

	const tz = Intl.DateTimeFormat().resolvedOptions().timeZone;
	const thisYear = new Date().getFullYear();

	let year = $state(thisYear);
	let months = $state<MonthsReport | null>(null);
	let mError = $state('');
	let mLoading = $state(false);

	let date = $state(localToday());
	let stock = $state<StockReport | null>(null);
	let sError = $state('');
	let sLoading = $state(false);

	async function loadMonths() {
		mLoading = true;
		mError = '';
		try {
			months = await api<MonthsReport>('GET', `/reports/months?year=${year}&tz=${encodeURIComponent(tz)}`);
		} catch (e) {
			mError = errorMessage(e);
		} finally {
			mLoading = false;
		}
	}

	async function loadStock() {
		sLoading = true;
		sError = '';
		try {
			stock = await api<StockReport>(
				'GET',
				`/reports/stock?date=${encodeURIComponent(date)}&tz=${encodeURIComponent(tz)}`
			);
		} catch (e) {
			sError = errorMessage(e);
		} finally {
			sLoading = false;
		}
	}

	function setYear(y: number) {
		year = y;
		loadMonths();
	}

	function monthName(first: string): string {
		return formatDate(`${first}T12:00:00`, { month: 'long', year: 'numeric' });
	}

	const totals = $derived.by(() => {
		const rows = months?.months ?? [];
		return {
			purchased: rows.reduce((s, m) => s + m.purchased_net_minor, 0),
			usedCost: rows.reduce((s, m) => s + m.used_cost_minor, 0),
			usedSale: rows.reduce((s, m) => s + m.used_sale_minor, 0)
		};
	});

	function exportMonths() {
		if (!months) return;
		const c = months.currency;
		downloadCsv(
			`autokarta-${months.year}-${t('report.fileMonths')}.csv`,
			[
				t('report.month'),
				t('report.purchased', { currency: c }),
				t('report.usedCost', { currency: c }),
				t('report.usedSale', { currency: c }),
				t('report.margin', { currency: c }),
				t('report.stockEnd', { currency: c }),
				t('report.csvUnpricedReceipts'),
				t('report.csvUnpricedLines'),
				t('report.csvUnvaluedParts'),
				t('report.csvNegativeParts')
			],
			months.months.map((m) => [
				m.month.slice(0, 7),
				csvMoney(m.purchased_net_minor, c),
				csvMoney(m.used_cost_minor, c),
				csvMoney(m.used_sale_minor, c),
				csvMoney(m.used_sale_minor - m.used_cost_minor, c),
				csvMoney(m.stock_end.value_minor, c),
				m.purchased_unpriced,
				m.used_unpriced,
				m.stock_end.unvalued_parts,
				m.stock_end.negative_parts
			])
		);
	}

	function exportStock() {
		if (!stock) return;
		const c = stock.currency;
		downloadCsv(
			`autokarta-${t('report.fileStock')}-${stock.date}.csv`,
			[
				t('parts.partNumber'),
				t('parts.brand'),
				t('parts.name'),
				t('parts.condition'),
				t('report.unit'),
				t('parts.location'),
				t('report.quantity'),
				t('report.unitCost', { currency: c }),
				t('report.value', { currency: c })
			],
			stock.rows.map((r) => [
				r.part_number,
				r.brand,
				r.name,
				t(`parts.conditions.${r.condition}`),
				t(`units.${r.unit}`),
				r.location,
				csvQuantity(r.quantity_milli),
				csvMoney(r.unit_cost_minor, c),
				csvMoney(r.value_minor, c)
			])
		);
	}

	onMount(() => {
		loadMonths();
		loadStock();
	});
</script>

<svelte:head><title>{t('report.title')} · Autokarta</title></svelte:head>

<p class="crumbs"><a href="/parts">← {t('parts.title')}</a></p>
<h1>{t('report.title')}</h1>
<p class="hint intro">{t('report.intro')}</p>

<section class="card">
	<div class="head-row">
		<h2>{t('report.monthsTitle')}</h2>
		<div class="years" role="group" aria-label={t('report.year')}>
			<button class="btn" type="button" aria-label={t('report.prevYear')} onclick={() => setYear(year - 1)}>←</button>
			<strong>{year}</strong>
			<button
				class="btn"
				type="button"
				aria-label={t('report.nextYear')}
				disabled={year >= thisYear}
				onclick={() => setYear(year + 1)}>→</button
			>
		</div>
	</div>
	{#if mError}<div class="notice error" role="alert">{mError}</div>{/if}
	{#if mLoading && !months}
		<p class="hint">{t('common.loading')}</p>
	{:else if months}
		{#if months.months.length === 0}
			<p class="hint">{t('report.noMonths')}</p>
		{:else}
			{@const c = months.currency}
			<ul class="months" class:stale={mLoading}>
				{#each [...months.months].reverse() as m (m.month)}
					{@const margin = m.used_sale_minor - m.used_cost_minor}
					<li>
						<h3>{monthName(m.month)}</h3>
						<dl>
							<dt>{t('report.purchasedShort')}</dt>
							<dd>{formatMoney(m.purchased_net_minor, c)}</dd>
							<dt>{t('report.usedCostShort')}</dt>
							<dd>{formatMoney(m.used_cost_minor, c)}</dd>
							<dt>{t('report.usedSaleShort')}</dt>
							<dd>{formatMoney(m.used_sale_minor, c)}</dd>
							<dt>{t('report.marginShort')}</dt>
							<dd class:danger={margin < 0}>{formatMoney(margin, c)}</dd>
							<dt>{t('report.stockEndShort')}</dt>
							<dd><strong>{formatMoney(m.stock_end.value_minor, c)}</strong></dd>
						</dl>
						{#if m.purchased_unpriced || m.used_unpriced || m.used_unvalued || m.stock_end.unvalued_parts || m.stock_end.negative_parts}
							<ul class="warnings">
								{#if m.purchased_unpriced}<li>{t('report.warnUnpricedReceipts', { count: m.purchased_unpriced })}</li>{/if}
								{#if m.used_unpriced}<li>{t('report.warnUnpricedLines', { count: m.used_unpriced })}</li>{/if}
								{#if m.used_unvalued}<li>{t('report.warnUnvaluedLines', { count: m.used_unvalued })}</li>{/if}
								{#if m.stock_end.unvalued_parts}<li>
										{t('report.warnUnvaluedParts', { count: m.stock_end.unvalued_parts })}
									</li>{/if}
								{#if m.stock_end.negative_parts}<li>
										{t('report.warnNegativeParts', { count: m.stock_end.negative_parts })}
									</li>{/if}
							</ul>
						{/if}
					</li>
				{/each}
			</ul>
			<dl class="year-total">
				<dt>{t('report.yearPurchased', { year: months.year })}</dt>
				<dd>{formatMoney(totals.purchased, c)}</dd>
				<dt>{t('report.yearUsedCost', { year: months.year })}</dt>
				<dd>{formatMoney(totals.usedCost, c)}</dd>
				<dt>{t('report.yearUsedSale', { year: months.year })}</dt>
				<dd>{formatMoney(totals.usedSale, c)}</dd>
			</dl>
			<button class="btn block" type="button" onclick={exportMonths}>{t('report.exportCsv')}</button>
		{/if}
	{/if}
</section>

<section class="card">
	<h2>{t('report.stockTitle')}</h2>
	<form
		class="with-action date-row"
		onsubmit={(e) => {
			e.preventDefault();
			loadStock();
		}}
	>
		<label class="visually-hidden" for="stock-date">{t('report.stockDate')}</label>
		<input id="stock-date" type="date" bind:value={date} max={localToday()} required />
		<button class="btn" type="submit" disabled={sLoading}>{t('report.show')}</button>
	</form>
	{#if sError}<div class="notice error" role="alert">{sError}</div>{/if}
	{#if stock}
		{@const c = stock.currency}
		<p class="total">
			{t('report.stockOn', { date: formatDate(`${stock.date}T12:00:00`) })}
			<strong>{formatMoney(stock.total.value_minor, c)}</strong>
		</p>
		<p class="hint">{t('report.stockParts', { count: stock.rows.length })}</p>
		{#if stock.total.unvalued_parts || stock.total.negative_parts}
			<ul class="warnings">
				{#if stock.total.unvalued_parts}<li>{t('report.warnUnvaluedParts', { count: stock.total.unvalued_parts })}</li>{/if}
				{#if stock.total.negative_parts}<li>{t('report.warnNegativeParts', { count: stock.total.negative_parts })}</li>{/if}
			</ul>
		{/if}
		{#if stock.rows.length > 0}
			<button class="btn block spaced" type="button" onclick={exportStock}>{t('report.exportCsv')}</button>
			<ul class="rows" class:stale={sLoading}>
				{#each stock.rows as r (r.id)}
					<li>
						<a class="row-link" href="/parts/{r.id}">
							<span class="row-main">
								<span class="name"
									>{r.name}{#if r.condition === 'used'}
										<span class="badge accent">{t('parts.conditions.used')}</span>{/if}</span
								>
								{#if r.brand || r.part_number}
									<span class="hint"
										>{r.brand ?? ''}{#if r.brand && r.part_number}{' · '}{/if}<span class="mono">{r.part_number ?? ''}</span
										></span
									>
								{/if}
								<span class="hint"
									>{formatStock(r.quantity_milli, r.unit)}{#if r.unit_cost_minor !== null}{' × ' +
											formatMoney(r.unit_cost_minor, c)}{/if}</span
								>
							</span>
							<span class="row-side">
								{#if r.value_minor !== null}
									<strong>{formatMoney(r.value_minor, c)}</strong>
								{:else}
									<span class="badge danger">{t('report.noCost')}</span>
								{/if}
							</span>
						</a>
					</li>
				{/each}
			</ul>
		{/if}
	{:else if sLoading}
		<p class="hint">{t('common.loading')}</p>
	{/if}
</section>

<section class="card">
	<h2>{t('report.howTitle')}</h2>
	<ul class="how">
		<li>{t('report.howNet')}</li>
		<li>{t('report.howValue')}</li>
		<li>{t('report.howSale')}</li>
		<li>{t('report.howCount')}</li>
	</ul>
</section>

<style>
	.crumbs {
		margin: 0 0 6px;
	}
	.intro {
		margin-top: 0;
	}
	.head-row {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
		margin-bottom: 8px;
	}
	.head-row h2 {
		margin: 0;
	}
	.years {
		display: flex;
		align-items: center;
		gap: 10px;
	}
	.years .btn {
		min-width: var(--tap);
		padding: 8px;
	}
	.months {
		list-style: none;
		padding: 0;
		margin: 0 0 14px;
	}
	.months > li {
		padding: 12px 0;
		border-bottom: 1px solid var(--line);
	}
	.months h3 {
		margin: 0 0 6px;
		text-transform: capitalize;
	}
	dl {
		display: grid;
		grid-template-columns: 1fr auto;
		gap: 4px 14px;
		margin: 0;
	}
	dt {
		color: var(--ink-muted);
	}
	dd {
		margin: 0;
		text-align: right;
		font-variant-numeric: tabular-nums;
	}
	.year-total {
		margin-bottom: 14px;
	}
	.warnings {
		margin: 8px 0 0;
		padding-left: 1.2rem;
		color: var(--danger);
		font-size: 0.92rem;
	}
	.danger {
		color: var(--danger);
	}
	.date-row {
		margin-bottom: 12px;
	}
	.total {
		display: flex;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
		margin: 0 0 4px;
	}
	.total strong {
		font-size: 1.3rem;
	}
	.name {
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.spaced {
		margin: 10px 0 12px;
	}
	.stale {
		opacity: 0.6;
	}
	.how {
		margin: 0;
		padding-left: 1.2rem;
		display: grid;
		gap: 6px;
	}
</style>
