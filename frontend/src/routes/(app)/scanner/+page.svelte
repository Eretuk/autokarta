<script lang="ts">
	// Camera scanner test page: checks that barcode and QR reading works on
	// this device. Linked from the account page.
	import CameraScanner from '$lib/components/CameraScanner.svelte';
	import { formatDate, t } from '$lib/i18n/index.svelte';
	import { formatLabel, type Detection } from '$lib/scanner';

	interface Hit extends Detection {
		id: number;
		at: Date;
	}

	let nextId = 0;
	let hits = $state<Hit[]>([]);
</script>

<svelte:head><title>{t('scanner.title')} · Autokarta</title></svelte:head>

<h1>{t('scanner.title')}</h1>
<p class="hint">{t('scanner.intro')}</p>

<CameraScanner showEngine onDetect={(d) => (hits = [{ ...d, id: nextId++, at: new Date() }, ...hits].slice(0, 20))} />

<section class="results" aria-live="polite">
	<h2>{t('scanner.results')}</h2>
	{#if hits.length === 0}
		<p class="hint">{t('scanner.empty')}</p>
	{:else}
		<ol>
			{#each hits as hit (hit.id)}
				<li>
					<code>{hit.text}</code>
					<span class="hint">
						{formatLabel(hit.format)} · {formatDate(hit.at, { timeStyle: 'medium' })}
					</span>
				</li>
			{/each}
		</ol>
	{/if}
</section>

<style>
	.results {
		margin-top: 20px;
	}
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li {
		display: grid;
		gap: 2px;
		padding: 10px 0;
		border-bottom: 1px solid var(--line);
	}
	code {
		font-size: 1.1rem;
		font-weight: 600;
		overflow-wrap: anywhere;
	}
</style>
