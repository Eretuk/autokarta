<script lang="ts">
	// Stock count (inventory): walk the shelves, scan a box, type how many
	// are really there, save, scan the next one. The difference to what the
	// system thought is recorded as a count correction.
	import { page } from '$app/state';
	import { onMount, tick } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import { debounce } from '$lib/debounce';
	import ScanButton from '$lib/components/ScanButton.svelte';
	import { formatStock, moneyInput, parseDecimal, parseMoney, quantityInput } from '$lib/format';
	import { t } from '$lib/i18n/index.svelte';
	import { session } from '$lib/session.svelte';
	import { business } from '$lib/settings.svelte';
	import type { PartDetail, PartSummary } from '$lib/types';

	const isOwner = $derived(session.user?.role === 'owner');
	const currency = $derived(business.currency);

	let query = $state('');
	let results = $state<PartSummary[]>([]);
	let searched = $state(false);
	let unknownBarcode = $state('');
	let error = $state('');
	let seq = 0;

	let part = $state<PartDetail | null>(null);
	let qty = $state('');
	let cost = $state('');
	let busy = $state(false);
	let qtyInput = $state<HTMLInputElement>();
	let searchInput = $state<HTMLInputElement>();

	type Done = { id: string; name: string; before: number; after: number; unit: PartDetail['unit'] };
	/** Parts counted on this visit, newest first. Memory only. */
	let done = $state<Done[]>([]);
	let lastSaved = $state<Done | null>(null);

	async function run(q: string, scanned: boolean) {
		const mine = ++seq;
		if (!q.trim()) {
			results = [];
			searched = false;
			return;
		}
		try {
			const rows = await api<PartSummary[]>('GET', `/parts?q=${encodeURIComponent(q.trim())}`);
			if (mine !== seq) return;
			if (scanned && rows.length === 1 && rows[0].barcode_match) {
				await open(rows[0].id);
				return;
			}
			results = rows.slice(0, 20);
			searched = true;
			unknownBarcode = scanned && !rows.some((r) => r.barcode_match) ? q.trim() : '';
			error = '';
		} catch (e) {
			if (mine === seq) error = errorMessage(e);
		}
	}
	const search = debounce((q: string) => run(q, false), 250);

	function onScan(text: string) {
		search.cancel();
		query = text.trim();
		run(query, true);
	}

	async function open(id: string) {
		error = '';
		try {
			const p = await api<PartDetail>('GET', `/parts/${id}`);
			part = p;
			qty = p.stock_milli > 0 ? quantityInput(p.stock_milli) : '';
			cost = moneyInput(p.last_cost_minor, currency);
			query = '';
			results = [];
			searched = false;
			unknownBarcode = '';
			lastSaved = null;
			await tick();
			qtyInput?.focus();
			qtyInput?.select();
		} catch (e) {
			error = errorMessage(e);
		}
	}

	async function save(e: SubmitEvent) {
		e.preventDefault();
		if (!part) return;
		const counted = parseDecimal(qty, 3);
		if (counted === null) {
			error = t('errors.invalid_quantity');
			return;
		}
		let unitCost: number | undefined;
		if (isOwner && cost.trim()) {
			const c = parseMoney(cost, currency);
			if (c === null) {
				error = t('errors.invalid_price');
				return;
			}
			unitCost = c;
		}
		busy = true;
		error = '';
		try {
			const before = part.stock_milli;
			const p = await api<PartDetail>('POST', `/parts/${part.id}/movements`, {
				kind: 'count',
				quantity_milli: counted,
				unit_cost_minor: unitCost
			});
			const entry: Done = { id: p.id, name: p.name, before, after: p.stock_milli, unit: p.unit };
			done = [entry, ...done.filter((x) => x.id !== p.id)];
			lastSaved = entry;
			part = null;
			await tick();
			searchInput?.focus();
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	const newPartHref = $derived.by(() => {
		const params = new URLSearchParams();
		if (unknownBarcode) params.set('barcode', unknownBarcode);
		else if (query.trim()) params.set('q', query.trim());
		params.set('back', '/parts/count');
		return `/parts/new?${params}`;
	});

	// Back from "New part": the part was created with its stock already, so
	// it only goes on the list.
	onMount(async () => {
		const created = page.url.searchParams.get('created');
		if (!created) return;
		history.replaceState(history.state, '', '/parts/count');
		try {
			const p = await api<PartDetail>('GET', `/parts/${created}`);
			const entry: Done = { id: p.id, name: p.name, before: 0, after: p.stock_milli, unit: p.unit };
			done = [entry];
			lastSaved = entry;
		} catch {
			// The part exists; the list is only a convenience.
		}
	});
</script>

<svelte:head><title>{t('stockCount.title')} · Autokarta</title></svelte:head>

<p class="crumbs"><a href="/parts">← {t('parts.title')}</a></p>
<h1>{t('stockCount.title')}</h1>
<p class="hint intro">{t('stockCount.intro')}</p>

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

{#if lastSaved}
	<div class="notice ok" role="status">
		{t('stockCount.saved', { name: lastSaved.name, value: formatStock(lastSaved.after, lastSaved.unit) })}
	</div>
{/if}

{#if part}
	<form class="card counting" onsubmit={save}>
		<h2 class="pname">
			{part.name}
			{#if part.condition === 'used'}<span class="badge accent">{t('parts.conditions.used')}</span>{/if}
		</h2>
		{#if part.brand || part.part_number}
			<p class="hint ident">
				{part.brand ?? ''}{#if part.brand && part.part_number}{' · '}{/if}<span class="mono">{part.part_number ?? ''}</span>
			</p>
		{/if}
		<dl class="facts">
			{#if part.location}<dt>{t('parts.location')}</dt><dd><strong>{part.location}</strong></dd>{/if}
			<dt>{t('stockCount.expected')}</dt>
			<dd class:danger={part.stock_milli < 0}>{formatStock(part.stock_milli, part.unit)}</dd>
		</dl>
		<label class="field">
			{t('parts.countQty', { unit: t(`units.${part.unit}`) })}
			<input
				bind:this={qtyInput}
				class="big"
				type="text"
				inputmode="decimal"
				bind:value={qty}
				maxlength="12"
				autocomplete="off"
				enterkeyhint="done"
				required
			/>
		</label>
		{#if isOwner}
			<label class="field">
				{t('parts.netCost', { currency })}
				<span class="hint">{t('parts.countCostHint')}</span>
				<input type="text" inputmode="decimal" bind:value={cost} maxlength="16" autocomplete="off" />
			</label>
		{/if}
		<button class="btn primary block" type="submit" disabled={busy}>{t('stockCount.save')}</button>
		<button class="btn block gap" type="button" disabled={busy} onclick={() => (part = null)}>{t('common.cancel')}</button>
	</form>
{:else}
	<form class="search" role="search" onsubmit={(e) => e.preventDefault()}>
		<label class="visually-hidden" for="count-search">{t('parts.searchLabel')}</label>
		<div class="with-action">
			<input
				bind:this={searchInput}
				id="count-search"
				type="search"
				bind:value={query}
				oninput={() => {
					unknownBarcode = '';
					search(query);
				}}
				placeholder={t('parts.searchPlaceholder')}
				autocomplete="off"
				spellcheck="false"
				enterkeyhint="search"
			/>
			<ScanButton onScan={(d) => onScan(d.text)} />
		</div>
	</form>

	{#if unknownBarcode}
		<div class="notice" role="status">
			{t('parts.unknownBarcode')} <span class="mono">{unknownBarcode}</span>
		</div>
		<a class="btn primary block" href={newPartHref}>{t('parts.addWithBarcode')}</a>
	{/if}

	{#if results.length > 0}
		<ul class="rows picks">
			{#each results as p (p.id)}
				<li>
					<button class="row-link" type="button" onclick={() => open(p.id)}>
						<span class="row-main">
							<span class="name"
								>{p.name}{#if p.condition === 'used'}
									<span class="badge accent">{t('parts.conditions.used')}</span>{/if}</span
							>
							{#if p.brand || p.part_number}
								<span class="hint">
									{p.brand ?? ''}{#if p.brand && p.part_number}{' · '}{/if}<span class="mono">{p.part_number ?? ''}</span>
								</span>
							{/if}
						</span>
						<span class="row-side">
							<span>{formatStock(p.stock_milli, p.unit)}</span>
							{#if p.location}<span class="hint">{p.location}</span>{/if}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{:else if searched && !unknownBarcode}
		<p class="hint">{t('parts.noResults')}</p>
	{/if}
	{#if searched && !unknownBarcode}
		<p>{t('parts.notInCatalogue')} <a href={newPartHref}>{t('parts.add')}</a></p>
	{/if}
{/if}

{#if done.length > 0}
	<div class="section-head"><h2>{t('stockCount.done', { count: done.length })}</h2></div>
	<ul class="rows">
		{#each done as x (x.id)}
			{@const diff = x.after - x.before}
			<li>
				<a class="row-link" href="/parts/{x.id}">
					<span class="row-main"><span class="name">{x.name}</span></span>
					<span class="row-side">
						<strong>{formatStock(x.after, x.unit)}</strong>
						{#if diff !== 0}
							<span class="hint" class:danger={diff < 0}
								>{diff > 0 ? '+' : '−'}{formatStock(Math.abs(diff), x.unit)}</span
							>
						{:else}
							<span class="hint">{t('stockCount.noChange')}</span>
						{/if}
					</span>
				</a>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.crumbs {
		margin: 0 0 6px;
	}
	.intro {
		margin-top: 0;
	}
	.search {
		margin-bottom: 12px;
	}
	.search input {
		font-size: 1.15rem;
		min-height: 56px;
	}
	.picks {
		margin-top: 12px;
	}
	.name,
	.pname {
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.pname {
		margin: 0;
	}
	.ident {
		margin: 4px 0 10px;
	}
	.facts {
		display: grid;
		grid-template-columns: max-content 1fr;
		gap: 6px 14px;
		margin: 0 0 14px;
	}
	.facts dt {
		color: var(--ink-muted);
	}
	.facts dd {
		margin: 0;
	}
	.big {
		font-family: var(--font-head);
		font-size: 2rem;
		min-height: 64px;
	}
	.danger {
		color: var(--danger);
	}
	.gap {
		margin-top: 10px;
	}
</style>
