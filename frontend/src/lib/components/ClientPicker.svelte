<script lang="ts">
	// Pick an existing client by name or phone, or create one on the spot.
	import { api, errorMessage } from '$lib/api';
	import { debounce } from '$lib/debounce';
	import { t } from '$lib/i18n/index.svelte';
	import type { Client } from '$lib/types';

	let {
		selected = $bindable(null),
		label
	}: { selected?: Client | null; label: string } = $props();

	let query = $state('');
	let results = $state<Client[]>([]);
	let creating = $state(false);
	let newName = $state('');
	let newPhone = $state('');
	let error = $state('');
	let busy = $state(false);

	const labelId = $props.id();
	let seq = 0;

	async function run(q: string) {
		const mine = ++seq;
		try {
			const rows = await api<Client[]>('GET', `/clients?q=${encodeURIComponent(q)}`);
			if (mine !== seq) return; // a newer search already started
			results = rows;
			error = '';
		} catch (e) {
			if (mine === seq) error = errorMessage(e);
		}
	}
	const search = debounce(run, 250);

	function onInput() {
		if (!query.trim()) {
			// Cleared field: drop pending and in-flight searches.
			search.cancel();
			seq++;
			results = [];
			error = '';
			return;
		}
		search(query);
	}

	function startCreate() {
		// Whatever was typed is most likely the name (or a phone number).
		const typed = query.trim();
		const isPhone = /^[\d\s+()-]{5,}$/.test(typed);
		newName = isPhone ? '' : typed;
		newPhone = isPhone ? typed : '';
		creating = true;
		error = '';
	}

	async function create() {
		busy = true;
		error = '';
		try {
			selected = await api<Client>('POST', '/clients', { name: newName, phone: newPhone });
			creating = false;
			query = '';
			results = [];
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
			<span><strong>{selected.name}</strong>{#if selected.phone}<br /><span class="hint">{selected.phone}</span>{/if}</span>
			<button type="button" class="btn" onclick={() => (selected = null)}>{t('clients.change')}</button>
		</div>
	{:else if creating}
		<div class="new">
			<label class="field">
				{t('clients.name')}
				<input type="text" bind:value={newName} maxlength="120" autocomplete="off" />
			</label>
			<label class="field">
				{t('clients.phone')}
				<input type="tel" bind:value={newPhone} maxlength="32" autocomplete="off" />
			</label>
			<div class="row">
				<button type="button" class="btn" onclick={() => (creating = false)}>{t('common.cancel')}</button>
				<button type="button" class="btn primary" disabled={busy || !newName.trim()} onclick={create}
					>{t('clients.create')}</button
				>
			</div>
		</div>
	{:else}
		<input
			type="search"
			bind:value={query}
			oninput={onInput}
			aria-labelledby={labelId}
			placeholder={t('clients.searchPlaceholder')}
			autocomplete="off"
		/>
		{#if results.length}
			<ul class="results">
				{#each results as c (c.id)}
					<li>
						<button type="button" onclick={() => (selected = c)}>
							<strong>{c.name}</strong>
							{#if c.phone}<span class="hint">{c.phone}</span>{/if}
						</button>
					</li>
				{/each}
			</ul>
		{/if}
		<button type="button" class="btn block add" onclick={startCreate}>{t('clients.addNew')}</button>
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
