<script lang="ts">
	// Supplier deliveries (purchase invoices). Drafts first: they are the
	// ones still waiting for someone to finish and post them.
	import { goto } from '$app/navigation';
	import { onMount } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import { debounce } from '$lib/debounce';
	import Icon from '$lib/components/Icon.svelte';
	import { formatMoney } from '$lib/format';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import { business } from '$lib/settings.svelte';
	import type { DeliveryDetail, DeliveryListRow } from '$lib/types';

	const FILTERS = ['all', 'draft', 'posted'] as const;
	type Filter = (typeof FILTERS)[number];

	let filter = $state<Filter>('all');
	let query = $state('');
	let rows = $state<DeliveryListRow[]>([]);
	let loading = $state(true);
	let error = $state('');
	let busy = $state(false);
	let seq = 0;

	async function run() {
		const mine = ++seq;
		try {
			const r = await api<DeliveryListRow[]>(
				'GET',
				`/deliveries?status=${filter}&q=${encodeURIComponent(query.trim())}`
			);
			if (mine !== seq) return;
			rows = r;
			error = '';
		} catch (e) {
			if (mine === seq) error = errorMessage(e);
		} finally {
			if (mine === seq) loading = false;
		}
	}
	const search = debounce(run, 250);

	function setFilter(f: Filter) {
		search.cancel();
		filter = f;
		loading = true;
		run();
	}

	async function create() {
		busy = true;
		error = '';
		try {
			const d = await api<DeliveryDetail>('POST', '/deliveries', {});
			await goto(`/parts/deliveries/${d.id}`);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}

	onMount(run);
</script>

<svelte:head><title>{t('deliveries.title')} · Autokarta</title></svelte:head>

<p class="crumbs"><a href="/parts">← {t('parts.title')}</a></p>
<h1>{t('deliveries.title')}</h1>
<p class="hint intro">{t('deliveries.intro')}</p>

<button class="btn primary block add" type="button" disabled={busy} onclick={create}>
	<Icon name="plus" />{t('deliveries.new')}
</button>

<form class="search" role="search" onsubmit={(e) => e.preventDefault()}>
	<label class="visually-hidden" for="delivery-search">{t('deliveries.searchLabel')}</label>
	<input
		id="delivery-search"
		type="search"
		bind:value={query}
		oninput={() => {
			loading = true;
			search();
		}}
		placeholder={t('deliveries.searchPlaceholder')}
		autocomplete="off"
		enterkeyhint="search"
	/>
</form>

<div class="segmented filters" role="group" aria-label={t('parts.filter')}>
	{#each FILTERS as f (f)}
		<button type="button" class="btn" aria-pressed={filter === f} onclick={() => setFilter(f)}
			>{t(`deliveries.filters.${f}`)}</button
		>
	{/each}
</div>

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

{#if loading && rows.length === 0}
	<p class="hint">{t('common.loading')}</p>
{:else if rows.length === 0}
	<p class="hint">{query.trim() ? t('deliveries.noResults') : t('deliveries.empty')}</p>
{:else}
	<ul class="rows" class:stale={loading}>
		{#each rows as d (d.id)}
			<li>
				<a class="row-link" href="/parts/deliveries/{d.id}">
					<span class="row-main">
						<span class="name">
							{d.supplier_name ?? t('deliveries.noSupplier')}
							{#if d.status === 'draft'}<span class="badge accent">{t('deliveries.status.draft')}</span>{/if}
						</span>
						<span class="hint">
							{t('deliveries.number', { number: d.number })}{#if d.invoice_number}{' · '}<span class="mono"
									>{d.invoice_number}</span
								>{/if}{' · '}{formatDate(d.invoice_date)}
						</span>
						<span class="hint">
							{t('deliveries.lines', { count: d.item_count })}{#if d.file_count > 0}{' · ' +
									t('deliveries.fileCount', { count: d.file_count })}{/if}
						</span>
					</span>
					<span class="row-side">
						<strong>{formatMoney(d.total_net_minor, business.currency)}</strong>
						<span class="hint">{t('deliveries.grossShort', { value: formatMoney(d.total_gross_minor, business.currency) })}</span>
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
	.add,
	.search,
	.filters {
		margin-bottom: 12px;
	}
	.name {
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.stale {
		opacity: 0.6;
	}
</style>
