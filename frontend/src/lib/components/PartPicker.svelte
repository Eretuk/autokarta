<script lang="ts">
	// Find a part by name, part number or barcode. A scan that matches one
	// barcode picks the part at once: the box in hand is the part.
	import { api, errorMessage } from '$lib/api';
	import { debounce } from '$lib/debounce';
	import { formatStock } from '$lib/format';
	import { t } from '$lib/i18n/index.svelte';
	import type { PartSummary } from '$lib/types';
	import ScanButton from './ScanButton.svelte';

	let { onPick, back }: { onPick: (p: PartSummary) => void; back: string } = $props();

	let query = $state('');
	/** The current query came from the camera: an unknown one is a barcode. */
	let scannedQuery = $state(false);
	let results = $state<PartSummary[]>([]);
	let searched = $state(false);
	let error = $state('');
	let seq = 0;

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
				onPick(rows[0]);
				return;
			}
			results = rows.slice(0, 20);
			searched = true;
			error = '';
		} catch (e) {
			if (mine === seq) error = errorMessage(e);
		}
	}
	const search = debounce((q: string) => run(q, false), 250);

	const newPartHref = $derived.by(() => {
		const params = new URLSearchParams();
		if (query.trim()) params.set(scannedQuery ? 'barcode' : 'q', query.trim());
		params.set('back', back);
		return `/parts/new?${params}`;
	});
</script>

<div class="with-action">
	<label class="visually-hidden" for="part-picker">{t('parts.searchLabel')}</label>
	<input
		id="part-picker"
		type="search"
		bind:value={query}
		oninput={() => {
			scannedQuery = false;
			search(query);
		}}
		placeholder={t('parts.searchPlaceholder')}
		autocomplete="off"
		spellcheck="false"
		enterkeyhint="search"
	/>
	<ScanButton
		onScan={(d) => {
			search.cancel();
			query = d.text.trim();
			scannedQuery = true;
			run(query, true);
		}}
	/>
</div>

{#if error}<div class="notice error" role="alert">{error}</div>{/if}

{#if results.length > 0}
	<ul class="rows picks">
		{#each results as p (p.id)}
			<li>
				<button class="row-link" type="button" onclick={() => onPick(p)}>
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
						<span class:danger={p.stock_milli <= 0}>{formatStock(p.stock_milli, p.unit)}</span>
						{#if p.location}<span class="hint">{p.location}</span>{/if}
					</span>
				</button>
			</li>
		{/each}
	</ul>
{:else if searched}
	<p class="hint">{t('parts.noResults')}</p>
{/if}

{#if searched}
	<p class="new-part">
		{t('parts.notInCatalogue')} <a href={newPartHref}>{t('parts.add')}</a>
	</p>
{/if}

<style>
	.picks {
		margin-top: 12px;
	}
	.name {
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.danger {
		color: var(--danger);
	}
	.new-part {
		margin-top: 8px;
	}
</style>
