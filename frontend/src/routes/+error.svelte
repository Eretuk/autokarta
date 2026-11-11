<script lang="ts">
	import { page } from '$app/state';
	import { hasKey, t } from '$lib/i18n/index.svelte';

	// Page errors carry the API error code as their message (see `pageError`).
	const code = $derived(page.error?.message ?? '');
	const notFound = $derived(page.status === 404);
	const offline = $derived(code === 'network_error' || code === 'server_unavailable');
	const message = $derived.by(() => {
		if (notFound) return t('notFound.body');
		const key = `errors.${code}`;
		return code && hasKey(key) ? t(key) : t('errors.unknown');
	});
</script>

<main>
	<h1>{notFound ? t('notFound.title') : t('notFound.errorTitle')}</h1>
	<p>{message}</p>
	<div class="actions">
		{#if offline}
			<button class="btn primary" type="button" onclick={() => location.reload()}>{t('notFound.retry')}</button>
			<a class="btn" href="/">{t('notFound.home')}</a>
		{:else}
			<a class="btn primary" href="/">{t('notFound.home')}</a>
		{/if}
	</div>
</main>

<style>
	main {
		max-width: 560px;
		margin: 0 auto;
		padding: 48px var(--gutter);
	}
	.actions {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
	}
</style>
