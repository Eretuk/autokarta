<script lang="ts">
	import { api, errorMessage } from '$lib/api';
	import Plate from '$lib/components/Plate.svelte';
	import { vehicleName } from '$lib/format';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import type { WorkOrderListRow } from '$lib/types';

	const FILTERS = ['open', 'done', 'all'] as const;
	type Filter = (typeof FILTERS)[number];

	let filter = $state<Filter>('open');
	let rows = $state<WorkOrderListRow[]>([]);
	let loading = $state(true);
	let error = $state('');

	let seq = 0;

	$effect(() => {
		const f = filter;
		// Only the latest request may touch the page: a slow answer for a
		// filter the user already left must not show its rows, error or end
		// the loading state of the current one.
		const mine = ++seq;
		loading = true;
		api<WorkOrderListRow[]>('GET', `/work-orders?status=${f}`)
			.then((r) => {
				if (mine !== seq) return;
				rows = r;
				error = '';
			})
			.catch((e) => {
				if (mine === seq) error = errorMessage(e);
			})
			.finally(() => {
				if (mine === seq) loading = false;
			});
	});
</script>

<svelte:head><title>{t('workOrders.title')} · Autokarta</title></svelte:head>

<h1>{t('workOrders.title')}</h1>

<div class="segmented filters" role="group" aria-label={t('workOrders.filter')}>
	{#each FILTERS as f (f)}
		<button type="button" class="btn" aria-pressed={filter === f} onclick={() => (filter = f)}
			>{t(`workOrders.filters.${f}`)}</button
		>
	{/each}
</div>

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

{#if loading && rows.length === 0}
	<p class="hint">{t('common.loading')}</p>
{:else if rows.length === 0}
	<p class="hint">{filter === 'open' ? t('workOrders.noneOpen') : t('workOrders.none')}</p>
	<p><a href="/">{t('workOrders.startHint')}</a></p>
{:else}
	<ul class="rows" class:stale={loading}>
		{#each rows as w (w.id)}
			<li>
				<a class="row-link" href="/work-orders/{w.id}">
					<span class="row-main">
						<span><strong>{t('workOrders.number', { number: w.number })}</strong> · {formatDate(w.opened_on)}</span>
						<span class="vehicle">
							<Plate plate={w.plate} fallback={w.vin ?? w.frame_number} />
							<span>{vehicleName(w)}</span>
						</span>
						{#if w.complaint}<span class="clip hint">{w.complaint}</span>{/if}
					</span>
					<span class="row-side">
						{#if filter !== 'open'}<span class="status {w.status}">{t(`workOrders.statuses.${w.status}`)}</span>{/if}
						<span class="hint">{t('workOrders.itemCount', { count: w.item_count })}</span>
						{#if w.client_name}<span class="hint">{w.client_name}</span>{/if}
					</span>
				</a>
			</li>
		{/each}
	</ul>
{/if}

<style>
	.filters {
		margin-bottom: 16px;
	}
	.vehicle {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		gap: 8px;
	}
	.clip {
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}
	.stale {
		opacity: 0.6;
	}
</style>
