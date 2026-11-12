<script lang="ts">
	// Pick a supplier by name or NIP, or create one on the spot.
	import { api, errorMessage } from '$lib/api';
	import { debounce } from '$lib/debounce';
	import { t } from '$lib/i18n/index.svelte';
	import type { Supplier } from '$lib/types';

	let {
		selected = $bindable(null),
		label,
		onChange
	}: { selected?: Supplier | null; label: string; onChange?: (s: Supplier | null) => void } = $props();

	let query = $state('');
	let results = $state<Supplier[]>([]);
	let creating = $state(false);
	let newName = $state('');
	let newTaxId = $state('');
	let error = $state('');
	let busy = $state(false);

	const labelId = $props.id();
	let seq = 0;

	async function run(q: string) {
		const mine = ++seq;
		try {
			const rows = await api<Supplier[]>('GET', `/suppliers?q=${encodeURIComponent(q)}`);
			if (mine !== seq) return;
			results = rows;
			error = '';
		} catch (e) {
			if (mine === seq) error = errorMessage(e);
		}
	}
	const search = debounce(run, 250);

	function onInput() {
		if (!query.trim()) {
			search.cancel();
			seq++;
			results = [];
			error = '';
			return;
		}
		search(query);
	}

	function choose(s: Supplier | null) {
		selected = s;
		onChange?.(s);
	}

	function startCreate() {
		// A typed number is most likely the NIP, anything else the name.
		const typed = query.trim();
		const isTaxId = /^[A-Za-z]{0,2}[\d\s.-]{8,}$/.test(typed);
		newName = isTaxId ? '' : typed;
		newTaxId = isTaxId ? typed : '';
		creating = true;
		error = '';
	}

	async function create() {
		busy = true;
		error = '';
		try {
			const s = await api<Supplier>('POST', '/suppliers', { name: newName, tax_id: newTaxId });
			creating = false;
			query = '';
			results = [];
			choose(s);
		} catch (e) {
			error = errorMessage(e);
		} finally {
			busy = false;
		}
	}
</script>

<div class="picker">
	<span class="label" id={labelId}>{label}</span>
	{#if error}<div class="notice error" role="alert">{error}</div>{/if}

	{#if selected}
		<div class="chosen">
			<span
				><strong>{selected.name}</strong>{#if selected.tax_id}<br /><span class="hint"
						>{t('deliveries.taxId')}: <span class="mono">{selected.tax_id}</span></span
					>{/if}</span
			>
			<button type="button" class="btn" onclick={() => choose(null)}>{t('deliveries.changeSupplier')}</button>
		</div>
	{:else if creating}
		<div class="new">
			<label class="field">
				{t('deliveries.supplierName')}
				<input type="text" bind:value={newName} maxlength="160" autocomplete="off" />
			</label>
			<label class="field">
				{t('deliveries.taxId')}
				<input type="text" bind:value={newTaxId} maxlength="30" autocomplete="off" inputmode="text" />
			</label>
			<div class="row">
				<button type="button" class="btn" onclick={() => (creating = false)}>{t('common.cancel')}</button>
				<button type="button" class="btn primary" disabled={busy || !newName.trim()} onclick={create}
					>{t('deliveries.createSupplier')}</button
				>
			</div>
		</div>
	{:else}
		<input
			type="search"
			bind:value={query}
			oninput={onInput}
			aria-labelledby={labelId}
			placeholder={t('deliveries.supplierSearch')}
			autocomplete="off"
		/>
		{#if results.length}
			<ul class="results">
				{#each results as s (s.id)}
					<li>
						<button type="button" onclick={() => choose(s)}>
							<strong>{s.name}</strong>
							{#if s.tax_id}<span class="hint mono">{s.tax_id}</span>{/if}
						</button>
					</li>
				{/each}
			</ul>
		{/if}
		<button type="button" class="btn block add" onclick={startCreate}>{t('deliveries.newSupplier')}</button>
	{/if}
</div>

<style>
	.picker {
		margin-bottom: 14px;
	}
	.label {
		display: block;
		font-weight: 500;
		margin-bottom: 6px;
	}
	.chosen {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 10px 12px;
		background: var(--surface);
		border: 2px solid var(--accent);
		border-radius: var(--radius);
	}
	.chosen strong {
		overflow-wrap: anywhere;
	}
	.results {
		list-style: none;
		margin: 6px 0 0;
		padding: 0;
		border: 1px solid var(--line);
		border-radius: var(--radius);
		background: var(--surface);
	}
	.results li + li {
		border-top: 1px solid var(--line);
	}
	.results button {
		width: 100%;
		min-height: var(--tap);
		display: flex;
		justify-content: space-between;
		align-items: center;
		gap: 8px;
		padding: 8px 12px;
		font: inherit;
		color: inherit;
		text-align: left;
		background: none;
		border: none;
		cursor: pointer;
	}
	.add {
		margin-top: 8px;
	}
	.new {
		padding: 12px;
		border: 1px dashed var(--line-strong);
		border-radius: var(--radius);
	}
	.row {
		display: flex;
		gap: 8px;
		justify-content: flex-end;
	}
</style>
