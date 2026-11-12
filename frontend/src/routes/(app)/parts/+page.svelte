<script lang="ts">
	// Parts on the shelf. Scanning a box opens its part straight away; an
	// unknown barcode offers to add the part with that barcode.
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import { debounce } from '$lib/debounce';
	import Icon from '$lib/components/Icon.svelte';
	import ScanButton from '$lib/components/ScanButton.svelte';
	import { formatStock } from '$lib/format';
	import { t } from '$lib/i18n/index.svelte';
	import { session } from '$lib/session.svelte';
	import type { PartSummary } from '$lib/types';

	const FILTERS = ['all', 'low'] as const;
	type Filter = (typeof FILTERS)[number];

	let query = $state(page.url.searchParams.get('q') ?? '');
	let filter = $state<Filter>(page.url.searchParams.get('filter') === 'low' ? 'low' : 'all');
	let results = $state<PartSummary[]>([]);
	let loading = $state(true);
	let error = $state('');
	/** Barcode of the last scan that matched no part. */
	let unknownBarcode = $state('');
	let seq = 0;

	async function run(q: string, f: Filter, scanned = false) {
		const mine = ++seq;
		try {
			const rows = await api<PartSummary[]>(
				'GET',
				`/parts?q=${encodeURIComponent(q.trim())}&filter=${f}`
			);
			if (mine !== seq) return; // a newer search already started
			if (scanned && rows.length === 1 && rows[0].barcode_match) {
				await goto(`/parts/${rows[0].id}`);
				return;
			}
			results = rows;
			unknownBarcode = scanned && !rows.some((r) => r.barcode_match) ? q.trim() : '';
			error = '';
		} catch (e) {
			if (mine === seq) error = errorMessage(e);
		} finally {
			if (mine === seq) loading = false;
		}
	}
	const search = debounce((q: string) => run(q, filter), 250);

	function remember() {
		// Keep the search in the URL so "back" from a part restores it.
		const params = new URLSearchParams();
		if (query.trim()) params.set('q', query.trim());
		if (filter === 'low') params.set('filter', 'low');
		const qs = params.toString();
		history.replaceState(history.state, '', qs ? `/parts?${qs}` : '/parts');
	}

	function onInput() {
		loading = true;
		unknownBarcode = '';
		search(query);
		remember();
	}

	function setFilter(f: Filter) {
		search.cancel();
		filter = f;
		loading = true;
		remember();
		run(query, f);
	}

	function onScan(text: string) {
		search.cancel();
		query = text.trim();
		filter = 'all';
		loading = true;
		remember();
		run(query, 'all', true);
	}

	onMount(() => run(query, filter));

	const addHref = $derived(
		unknownBarcode
			? `/parts/new?barcode=${encodeURIComponent(unknownBarcode)}`
			: query.trim()
				? `/parts/new?q=${encodeURIComponent(query.trim())}`
				: '/parts/new'
	);
</script>

<svelte:head><title>{t('parts.title')} · Autokarta</title></svelte:head>

<h1 class="visually-hidden">{t('parts.title')}</h1>

<form class="search" role="search" onsubmit={(e) => e.preventDefault()}>
	<label class="visually-hidden" for="part-search">{t('parts.searchLabel')}</label>
	<div class="with-action">
		<input
			id="part-search"
			type="search"
			bind:value={query}
			oninput={onInput}
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
{/if}

<a class="btn primary block add" href={addHref}>
	<Icon name="plus" />{unknownBarcode ? t('parts.addWithBarcode') : t('parts.add')}
</a>

<nav class="actions tools" aria-label={t('parts.tools')}>
	<a class="btn" href="/parts/count">{t('stockCount.title')}</a>
	{#if session.user?.role === 'owner'}
		<a class="btn" href="/parts/deliveries">{t('deliveries.title')}</a>
		<a class="btn" href="/parts/report">{t('report.title')}</a>
	{/if}
</nav>

<div class="segmented filters" role="group" aria-label={t('parts.filter')}>
	{#each FILTERS as f (f)}
		<button type="button" class="btn" aria-pressed={filter === f} onclick={() => setFilter(f)}
			>{t(`parts.filters.${f}`)}</button
		>
	{/each}
</div>

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

{#if loading && results.length === 0}
	<p class="hint">{t('common.loading')}</p>
{:else if results.length === 0}
	<p class="hint">
		{query.trim() ? t('parts.noResults') : filter === 'low' ? t('parts.noLow') : t('parts.empty')}
	</p>
{:else}
	<ul class="rows" class:stale={loading}>
		{#each results as p (p.id)}
			{@const low = p.min_stock_milli > 0 && p.stock_milli < p.min_stock_milli}
			<li>
				<a class="row-link" href="/parts/{p.id}">
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
						<strong class:negative={p.stock_milli < 0} class:low>{formatStock(p.stock_milli, p.unit)}</strong>
						{#if low}<span class="badge danger">{t('parts.low')}</span>{/if}
						{#if p.location}<span class="hint">{p.location}</span>{/if}
					</span>
				</a>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.search {
		margin-bottom: 12px;
	}
	.search input {
		font-size: 1.15rem;
		min-height: 56px;
	}
	.add {
		margin-bottom: 8px;
	}
	.tools {
		margin: 0 0 16px;
	}
	.filters {
		margin-bottom: 12px;
	}
	.name {
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.low,
	.negative {
		color: var(--danger);
	}
	.stale {
		opacity: 0.6;
	}
</style>
