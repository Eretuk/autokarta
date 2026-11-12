<script lang="ts">
	// Paper is always white: printable pages ignore the dark theme. The
	// toolbar is for the screen only.
	import { goto } from '$app/navigation';
	import { t } from '$lib/i18n/index.svelte';

	let { children } = $props();

	// Opened in a new tab or straight from a link there is nothing to go
	// back to: go to the app instead.
	function back() {
		if (history.length > 1) history.back();
		else goto('/');
	}
</script>

<div class="toolbar">
	<button class="btn" type="button" onclick={back}>{t('print.back')}</button>
	<button class="btn primary" type="button" onclick={() => window.print()}>{t('print.print')}</button>
</div>

<div class="paper-area">
	{@render children()}
</div>

<style>
	:global(body) {
		background: var(--surface-2);
	}
	.toolbar {
		position: sticky;
		top: 0;
		z-index: 1;
		display: flex;
		justify-content: space-between;
		gap: 12px;
		padding: calc(8px + env(safe-area-inset-top)) var(--gutter) 8px;
		background: var(--paper);
		border-bottom: 1px solid var(--line);
	}
	.paper-area {
		overflow-x: auto;
		padding: 16px var(--gutter) 32px;
		color-scheme: light;
	}
	@media print {
		.toolbar {
			display: none;
		}
		.paper-area {
			overflow: visible;
			padding: 0;
		}
		:global(body) {
			background: #fff;
		}
	}
</style>
