<script lang="ts">
	// Home = find a vehicle. This is the first thing done for every job, so
	// the search box and the scan button are right at the top.
	import { page } from '$app/state';
	import { onMount } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import { debounce } from '$lib/debounce';
	import Icon from '$lib/components/Icon.svelte';
	import OrgReminder from '$lib/components/OrgReminder.svelte';
	import Plate from '$lib/components/Plate.svelte';
	import ScanButton from '$lib/components/ScanButton.svelte';
	import { formatDistance, vehicleName } from '$lib/format';
	import { t } from '$lib/i18n/index.svelte';
	import type { VehicleSummary } from '$lib/types';

	let query = $state(page.url.searchParams.get('q') ?? '');
	let results = $state<VehicleSummary[]>([]);
	let loading = $state(true);
	let error = $state('');
	let seq = 0;

	async function run(q: string) {
		const mine = ++seq;
		try {
			const rows = await api<VehicleSummary[]>('GET', `/vehicles?q=${encodeURIComponent(q.trim())}`);
			if (mine !== seq) return; // a newer search already started
			results = rows;
			error = '';
		} catch (e) {
			if (mine === seq) error = errorMessage(e);
		} finally {
			if (mine === seq) loading = false;
		}
	}
	const search = debounce(run, 250);

	function onInput() {
		loading = true;
		search(query);
		// Keep the query in the URL so "back" from a vehicle restores it.
		const url = query.trim() ? `/?q=${encodeURIComponent(query.trim())}` : '/';
		history.replaceState(history.state, '', url);
	}

	onMount(() => run(query));

	const addHref = $derived(
		query.trim() ? `/vehicles/new?q=${encodeURIComponent(query.trim())}` : '/vehicles/new'
	);
</script>

<svelte:head><title>{t('vehicles.title')} · Autokarta</title></svelte:head>

<h1 class="visually-hidden">{t('vehicles.title')}</h1>

<OrgReminder />

<form class="search" role="search" onsubmit={(e) => e.preventDefault()}>
	<label class="visually-hidden" for="vehicle-search">{t('vehicles.searchLabel')}</label>
	<div class="with-action">
		<input
			id="vehicle-search"
			type="search"
			bind:value={query}
			oninput={onInput}
			placeholder={t('vehicles.searchPlaceholder')}
			autocomplete="off"
			autocapitalize="characters"
			spellcheck="false"
			enterkeyhint="search"
		/>
		<ScanButton
			onScan={(d) => {
				query = d.text.trim();
				onInput();
			}}
		/>
	</div>
</form>

<a class="btn primary block add" href={addHref}><Icon name="plus" />{t('vehicles.add')}</a>

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

<h2 class="list-title">{query.trim() ? t('vehicles.results') : t('vehicles.recent')}</h2>

{#if loading && results.length === 0}
	<p class="hint">{t('common.loading')}</p>
{:else if results.length === 0}
	<p class="hint">{query.trim() ? t('vehicles.noResults') : t('vehicles.empty')}</p>
{:else}
	<ul class="rows" class:stale={loading}>
		{#each results as v (v.id)}
			<li>
				<a class="row-link" href="/vehicles/{v.id}">
					<span class="row-main">
						<Plate plate={v.plate} fallback={v.vin ?? v.frame_number} />
						<span class="name">{vehicleName(v) || t('vehicles.unnamed')}</span>
						{#if v.owner_name}<span class="hint">{v.owner_name}</span>{/if}
					</span>
					<span class="row-side">
						{#if v.open_work_orders > 0}
							<span class="status open">{t('vehicles.inWork')}</span>
						{/if}
						{#if v.last_odometer !== null}
							<span class="hint">{formatDistance(v.last_odometer, v.odometer_unit)}</span>
						{/if}
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
	.list-title {
		font-size: 1.1rem;
		color: var(--ink-muted);
		margin: 22px 0 10px;
	}
	.name {
		font-weight: 600;
	}
	.stale {
		opacity: 0.6;
	}
</style>
