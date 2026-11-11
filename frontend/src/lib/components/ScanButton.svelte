<script lang="ts">
	// Icon button next to a field: opens the camera, returns the first code.
	import { t } from '$lib/i18n/index.svelte';
	import type { Detection } from '$lib/scanner';
	import CameraScanner from './CameraScanner.svelte';
	import Icon from './Icon.svelte';
	import Sheet from './Sheet.svelte';

	let { onScan, title }: { onScan: (d: Detection) => void; title?: string } = $props();
	let open = $state(false);
	// One code per opening: a box often carries several (EAN plus a lot
	// code) and the scanner reports every code found in a frame.
	let done = false;
</script>

<button type="button" class="scan" aria-label={t('scanner.scanCode')} onclick={() => {
		done = false;
		open = true;
	}}>
	<Icon name="scan" />
</button>

<Sheet bind:open title={title ?? t('scanner.scanCode')}>
	<CameraScanner
		autostart
		onDetect={(d) => {
			if (done) return;
			done = true;
			open = false;
			onScan(d);
		}}
	/>
</Sheet>

<style>
	.scan {
		flex: none;
		width: var(--tap);
		min-height: var(--tap);
		display: grid;
		place-items: center;
		color: var(--ink);
		background: var(--surface);
		border: 2px solid var(--line-strong);
		border-radius: var(--radius);
		cursor: pointer;
	}
</style>
