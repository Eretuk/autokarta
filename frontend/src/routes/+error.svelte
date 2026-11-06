<script lang="ts">
	import { page } from '$app/state';
	import { t } from '$lib/i18n/index.svelte';
</script>

<main>
	<h1>{page.status === 404 ? t('notFound.title') : t('errors.unknown')}</h1>
	{#if page.status === 404}<p>{t('notFound.body')}</p>{/if}
	<a class="btn primary" href="/">{t('notFound.home')}</a>
</main>

<style>
	main {
		max-width: 560px;
		margin: 0 auto;
		padding: 48px var(--gutter);
	}
</style>
