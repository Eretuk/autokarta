<script lang="ts">
	import { page } from '$app/state';
	import Icon, { type IconName } from '$lib/components/Icon.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';
	import { t, type MessageKey } from '$lib/i18n/index.svelte';
	import { appUpdate, applyUpdate } from '$lib/pwa.svelte';
	import { refreshSession, session } from '$lib/session.svelte';
	import { appVersion, loadVersion } from '$lib/version.svelte';
	import { onMount } from 'svelte';

	let { children } = $props();

	interface NavItem {
		href: string;
		label: MessageKey;
		icon: IconName;
		ownerOnly?: boolean;
		/** Not in the phone tab bar, which is full: reached from the account page there. */
		wideOnly?: boolean;
	}

	const items: NavItem[] = [
		{ href: '/', label: 'nav.vehicles', icon: 'car' },
		{ href: '/work-orders', label: 'nav.orders', icon: 'orders' },
		{ href: '/parts', label: 'nav.parts', icon: 'box' },
		{ href: '/staff', label: 'nav.staff', icon: 'staff', ownerOnly: true },
		{ href: '/organization', label: 'nav.organization', icon: 'org', ownerOnly: true, wideOnly: true },
		{ href: '/account', label: 'nav.account', icon: 'account' }
	];

	const visible = $derived(items.filter((i) => !i.ownerOnly || session.user?.role === 'owner'));

	// Owners get a dot on the account tab while a newer release is out.
	const updateDot = $derived(session.user?.role === 'owner' && appVersion.info?.update_available === true);

	onMount(loadVersion);

	function isActive(href: string) {
		const path = page.url.pathname;
		if (href === '/') return path === '/' || path.startsWith('/vehicles');
		return path === href || path.startsWith(href + '/');
	}
</script>

<!-- A role change by the owner or a deactivation takes effect when the app
     comes back to the foreground, without a manual reload. -->
<svelte:window
	onfocus={() => {
		refreshSession();
		loadVersion();
	}}
/>
<svelte:document
	onvisibilitychange={() => {
		if (document.visibilityState === 'visible') {
			refreshSession();
			loadVersion();
		}
	}}
/>

<div class="shell">
	<header class="top">
		<a href="/" class="home-link" aria-label={t('nav.home')}><Wordmark /></a>
		<nav class="nav" aria-label={t('nav.main')}>
			{#each visible as item (item.href)}
				<a
					href={item.href}
					class:wide-only={item.wideOnly}
					class:phone-parent={item.href === '/account' && isActive('/organization')}
					aria-current={isActive(item.href) ? 'page' : undefined}
				>
					<span class="icon">
						<Icon name={item.icon} />
						{#if item.href === '/account' && updateDot}
							<span class="dot" title={t('nav.updateAvailable')}></span>
							<span class="visually-hidden">{t('nav.updateAvailable')}</span>
						{/if}
					</span>
					<span>{t(item.label)}</span>
				</a>
			{/each}
		</nav>
	</header>

	{#if appUpdate.available}
		<div class="update" role="status">
			<span>{t('common.updateAvailable')}</span>
			<button class="btn primary" type="button" onclick={applyUpdate}>{t('common.reload')}</button>
		</div>
	{/if}

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
	.update {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 8px var(--gutter);
		background: var(--accent-soft);
		border-bottom: 1px solid var(--line);
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
	.icon {
		position: relative;
		display: inline-flex;
	}
	.dot {
		position: absolute;
		top: -2px;
		right: -4px;
		width: 10px;
		height: 10px;
		border-radius: 50%;
		background: var(--danger);
		border: 2px solid var(--surface);
	}
	.nav a[aria-current='page'],
	.nav a.phone-parent {
		color: var(--accent);
		border-top-color: var(--accent);
	}
	.nav a.wide-only {
		display: none;
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
		.nav a,
		.nav a.wide-only {
			display: flex;
			flex: none;
			flex-direction: row;
			min-height: var(--tap);
			padding: 0 14px;
			gap: 8px;
			font-size: 1rem;
			border-top: none;
			border-radius: var(--radius);
		}
		/* The header shows the organization link itself. */
		.nav a.phone-parent:not([aria-current]) {
			color: var(--ink-muted);
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
