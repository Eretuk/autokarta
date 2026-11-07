<script lang="ts">
	// Modal built on <dialog>: a bottom sheet on phones, a centered panel on
	// wide screens. Native <dialog> gives focus trapping and Esc for free.
	import type { Snippet } from 'svelte';
	import { t } from '$lib/i18n/index.svelte';

	let {
		open = $bindable(false),
		title,
		children
	}: { open?: boolean; title: string; children: Snippet } = $props();

	let dialog: HTMLDialogElement;

	// Closing on a backdrop tap must not fire when a finger went down inside
	// the sheet and came up outside (selecting text in a field, the page
	// shifting as the keyboard opens): the browser reports that click on the
	// dialog itself. So both the press and the release must be outside.
	let pressedOutside = false;
	function isOutside(e: MouseEvent | PointerEvent) {
		if (e.target !== dialog) return false;
		const r = dialog.getBoundingClientRect();
		return e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom;
	}

	$effect(() => {
		if (open && !dialog.open) dialog.showModal();
		else if (!open && dialog.open) dialog.close();
	});
</script>

<dialog
	bind:this={dialog}
	onclose={() => (open = false)}
	onpointerdown={(e) => (pressedOutside = isOutside(e))}
	onclick={(e) => {
		if (pressedOutside && isOutside(e)) open = false;
		pressedOutside = false;
	}}
	aria-labelledby="sheet-title"
>
	<div class="inner">
		<header>
			<h2 id="sheet-title">{title}</h2>
			<button type="button" class="close" onclick={() => (open = false)} aria-label={t('common.close')}>
				<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6L6 18" /></svg>
			</button>
		</header>
		{#if open}
			{@render children()}
		{/if}
	</div>
</dialog>

<style>
	dialog {
		width: 100%;
		max-width: 560px;
		max-height: 92dvh;
		margin: auto auto 0;
		padding: 0;
		border: none;
		border-radius: 12px 12px 0 0;
		background: var(--paper);
		color: var(--ink);
	}
	dialog::backdrop {
		background: rgb(0 0 0 / 0.5);
	}
	@media (min-width: 640px) {
		dialog {
			margin: auto;
			border-radius: 10px;
		}
	}
	.inner {
		padding: 8px var(--gutter) calc(20px + env(safe-area-inset-bottom));
	}
	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		position: sticky;
		top: 0;
		background: var(--paper);
		padding: 8px 0;
		z-index: 1;
	}
	h2 {
		margin: 0;
	}
	.close {
		width: var(--tap);
		height: var(--tap);
		flex: none;
		display: grid;
		place-items: center;
		background: transparent;
		border: none;
		color: var(--ink);
		cursor: pointer;
		border-radius: var(--radius);
	}
	.close svg {
		width: 26px;
		height: 26px;
		stroke: currentColor;
		stroke-width: 2.4;
		stroke-linecap: round;
	}
</style>
