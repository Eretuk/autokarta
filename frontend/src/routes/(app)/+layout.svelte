<script lang="ts">
	import { page } from '$app/state';
	import Icon, { type IconName } from '$lib/components/Icon.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';
	import { t, type MessageKey } from '$lib/i18n/index.svelte';
	import { session } from '$lib/session.svelte';

	let { children } = $props();

	interface NavItem {
		href: string;
		label: MessageKey;
		icon: IconName;
		ownerOnly?: boolean;
	}

	const items: NavItem[] = [
		{ href: '/', label: 'nav.home', icon: 'home' },
		{ href: '/scanner', label: 'nav.scanner', icon: 'scan' },
		{ href: '/staff', label: 'nav.staff', icon: 'staff', ownerOnly: true },
		{ href: '/account', label: 'nav.account', icon: 'account' }
	];

	const visible = $derived(items.filter((i) => !i.ownerOnly || session.user?.role === 'owner'));

	function isActive(href: string) {
		const path = page.url.pathname;
		return href === '/' ? path === '/' : path === href || path.startsWith(href + '/');
	}
</script>

<div class="shell">
	<header class="top">
		<a href="/" class="home-link" aria-label={t('nav.home')}><Wordmark /></a>
		<nav class="nav" aria-label={t('nav.main')}>
			{#each visible as item (item.href)}
				<a href={item.href} aria-current={isActive(item.href) ? 'page' : undefined}>
					<Icon name={item.icon} />
					<span>{t(item.label)}</span>
				</a>
			{/each}
		</nav>
	</header>

	<main>
		{@render children()}
	</main>
</div>

<style>
	.shell {
		min-height: 100dvh;
	}
	.top {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 16px;
		padding: calc(10px + env(safe-area-inset-top)) var(--gutter) 10px;
		border-bottom: 1px solid var(--line);
		background: var(--paper);
	}
	.home-link {
		text-decoration: none;
		display: inline-flex;
	}
	main {
		max-width: 760px;
		margin: 0 auto;
		padding: 20px var(--gutter) calc(var(--nav-h) + 32px + env(safe-area-inset-bottom));
	}

	/* Phones: tab bar at the bottom, within thumb reach. */
	.nav {
		position: fixed;
		left: 0;
		right: 0;
		bottom: 0;
		z-index: 10;
		display: flex;
		background: var(--surface);
		border-top: 1px solid var(--line-strong);
		padding-bottom: env(safe-area-inset-bottom);
	}
	.nav a {
		flex: 1 1 0;
		min-width: 0;
		min-height: var(--nav-h);
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		gap: 2px;
		color: var(--ink-muted);
		text-decoration: none;
		font-size: 0.8rem;
		font-weight: 600;
		border-top: 3px solid transparent;
	}
	.nav a span {
		max-width: 100%;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		padding: 0 4px;
	}
	.nav a[aria-current='page'] {
		color: var(--accent);
		border-top-color: var(--accent);
	}

	/* Wide screens: the same links sit in the header. */
	@media (min-width: 900px) {
		.nav {
			position: static;
			background: none;
			border: none;
			padding: 0;
			gap: 4px;
		}
		.nav a {
			flex: none;
			flex-direction: row;
			min-height: var(--tap);
			padding: 0 14px;
			gap: 8px;
			font-size: 1rem;
			border-top: none;
			border-radius: var(--radius);
		}
		.nav a[aria-current='page'] {
			background: var(--accent-soft);
			color: var(--ink);
		}
		main {
			padding-bottom: 48px;
		}
	}
</style>
