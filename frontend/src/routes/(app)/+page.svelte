<script lang="ts">
	import Icon from '$lib/components/Icon.svelte';
	import { t } from '$lib/i18n/index.svelte';
	import { session } from '$lib/session.svelte';
</script>

<svelte:head><title>Autokarta</title></svelte:head>

<h1>{t('home.greeting', { name: session.user?.display_name ?? '' })}</h1>

<section class="card soon">
	<h2>{t('home.soonTitle')}</h2>
	<p>{t('home.soonBody')}</p>
</section>

<div class="tiles">
	<a class="tile" href="/scanner"><Icon name="scan" /><span>{t('home.tryScanner')}</span></a>
	{#if session.user?.role === 'owner'}
		<a class="tile" href="/staff"><Icon name="staff" /><span>{t('home.manageStaff')}</span></a>
	{/if}
</div>

<style>
	.soon {
		border-left: 5px solid var(--accent);
	}
	.soon p {
		margin: 0;
	}
	.tiles {
		display: grid;
		grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
		gap: 12px;
	}
	.tile {
		display: flex;
		align-items: center;
		gap: 12px;
		min-height: 72px;
		padding: 12px 16px;
		background: var(--surface);
		border: 2px solid var(--line-strong);
		border-radius: var(--radius);
		color: var(--ink);
		text-decoration: none;
		font-weight: 600;
	}
	.tile:hover {
		border-color: var(--accent);
	}
</style>
