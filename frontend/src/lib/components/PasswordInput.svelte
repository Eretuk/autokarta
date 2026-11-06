<script lang="ts">
	import { t } from '$lib/i18n/index.svelte';

	let {
		value = $bindable(''),
		id,
		autocomplete = 'current-password',
		required = true
	}: {
		value?: string;
		id: string;
		autocomplete?: 'current-password' | 'new-password';
		required?: boolean;
	} = $props();

	let shown = $state(false);
</script>

<span class="wrap">
	<input
		{id}
		type={shown ? 'text' : 'password'}
		bind:value
		{autocomplete}
		{required}
		autocapitalize="off"
		autocorrect="off"
		spellcheck="false"
	/>
	<button
		type="button"
		class="toggle"
		aria-pressed={shown}
		aria-controls={id}
		onclick={() => (shown = !shown)}
	>
		{shown ? t('common.hide') : t('common.show')}
	</button>
</span>

<style>
	.wrap {
		position: relative;
		display: block;
	}
	input {
		padding-right: 5.5rem;
	}
	.toggle {
		position: absolute;
		top: 2px;
		right: 2px;
		bottom: 2px;
		min-width: var(--tap);
		padding: 0 12px;
		font: inherit;
		font-size: 0.92rem;
		font-weight: 600;
		color: var(--accent);
		background: transparent;
		border: none;
		border-radius: var(--radius);
		cursor: pointer;
	}
</style>
