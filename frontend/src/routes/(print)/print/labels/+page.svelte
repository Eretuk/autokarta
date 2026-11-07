<script lang="ts">
	// QR labels on an A4 sticker sheet: 3 columns x 8 rows of 70 x 37 mm.
	// "Skip" leaves the first labels of a partly used sheet empty.
	import QrCode from '$lib/components/QrCode.svelte';
	import { t } from '$lib/i18n/index.svelte';
	import { ownCode } from '$lib/labels';
	import type { PartDetail } from '$lib/types';

	const PER_SHEET = 24;

	let { data } = $props();
	const part: PartDetail = $derived(data.part);
	const code = $derived(ownCode(part.barcodes));

	let copies = $state(1);
	let skip = $state(0);

	const clamp = (n: number, min: number, max: number) =>
		Number.isFinite(n) ? Math.min(max, Math.max(min, Math.trunc(n))) : min;
	const nCopies = $derived(clamp(copies, 1, 4 * PER_SHEET));
	const nSkip = $derived(clamp(skip, 0, PER_SHEET - 1));

	/** One entry per label position: false = left empty, true = printed. */
	const sheets = $derived.by(() => {
		const cells = [...Array(nSkip).fill(false), ...Array(nCopies).fill(true)];
		const out: boolean[][] = [];
		for (let i = 0; i < cells.length; i += PER_SHEET) out.push(cells.slice(i, i + PER_SHEET));
		return out;
	});
</script>

<svelte:head>
	<title>{t('labels.title')} · {part.name}</title>
	<!-- In the head, not in <style>: removed when leaving the page. Label
	     sheets are cut edge to edge, so the page has no margins. -->
	<style>
		@page {
			size: A4;
			margin: 0;
		}
	</style>
</svelte:head>

<section class="options">
	<h1>{t('labels.title')}</h1>
	{#if code}
		<div class="two-cols">
			<label class="field">
				{t('labels.copies')}
				<input type="number" min="1" max={4 * PER_SHEET} bind:value={copies} />
			</label>
			<label class="field">
				{t('labels.skip')}
				<input type="number" min="0" max={PER_SHEET - 1} bind:value={skip} />
			</label>
		</div>
		<p class="hint">{t('labels.skipHint')}</p>
		<p class="hint">{t('labels.printHint')}</p>
	{:else}
		<div class="notice error" role="alert">{t('labels.noCode')}</div>
	{/if}
</section>

{#if code}
	{#each sheets as cells, s (s)}
		<div class="sheet">
			{#each cells as printed, i (i)}
				<div class="label" class:empty={!printed}>
					{#if printed}
						<div class="qr"><QrCode text={code} /></div>
						<div class="text">
							<div class="name">{part.name}</div>
							{#if part.condition === 'used'}<div class="used">{t('parts.conditions.used')}</div>{/if}
							{#if part.brand || part.part_number}
								<div class="ident">
									{part.brand ?? ''}{#if part.brand && part.part_number}{' · '}{/if}{part.part_number ?? ''}
								</div>
							{/if}
							<div class="bottom">
								{#if part.location}<span class="location">{part.location}</span>{/if}
								<span class="code">{code}</span>
							</div>
						</div>
					{/if}
				</div>
			{/each}
		</div>
	{/each}
{/if}

<style>
	.options {
		max-width: 560px;
		margin: 0 auto 16px;
	}
	.options h1 {
		margin-top: 0;
	}
	.sheet {
		box-sizing: border-box;
		width: 210mm;
		height: 297mm;
		margin: 0 auto 16px;
		display: grid;
		grid-template-columns: repeat(3, 70mm);
		grid-auto-rows: 37mm;
		align-content: start;
		background: #fff;
		color: #000;
		box-shadow: 0 1px 6px rgb(0 0 0 / 0.25);
		overflow: hidden;
	}
	.label {
		box-sizing: border-box;
		/* Inner margin: printers cannot reach the very edge of the paper. */
		padding: 3.5mm;
		display: grid;
		grid-template-columns: 25mm 1fr;
		gap: 2.5mm;
		align-items: center;
		outline: 1px dashed #ccc;
		overflow: hidden;
	}
	.qr {
		width: 25mm;
	}
	.text {
		display: flex;
		flex-direction: column;
		height: 100%;
		min-width: 0;
		font-family: var(--font-body);
		line-height: 1.15;
	}
	.name {
		font-weight: 600;
		font-size: 9.5pt;
		display: -webkit-box;
		-webkit-line-clamp: 3;
		line-clamp: 3;
		-webkit-box-orient: vertical;
		overflow: hidden;
		overflow-wrap: anywhere;
	}
	.used {
		align-self: flex-start;
		margin-top: 0.8mm;
		padding: 0 1.5mm;
		border: 0.4mm solid #000;
		font-weight: 700;
		font-size: 8pt;
		text-transform: uppercase;
	}
	.ident {
		margin-top: 1mm;
		font-size: 7.5pt;
		overflow-wrap: anywhere;
	}
	.bottom {
		margin-top: auto;
		display: flex;
		justify-content: space-between;
		align-items: baseline;
		gap: 2mm;
	}
	.location {
		font-family: var(--font-head);
		font-weight: 700;
		font-size: 13pt;
	}
	.code {
		font-size: 6.5pt;
		color: #333;
		margin-left: auto;
	}
	@media print {
		.options {
			display: none;
		}
		.sheet {
			margin: 0;
			box-shadow: none;
			break-after: page;
		}
		.sheet:last-child {
			break-after: auto;
		}
		.label {
			outline: none;
		}
	}
</style>
