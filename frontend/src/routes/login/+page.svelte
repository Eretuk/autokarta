<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { api, errorMessage, safeNext } from '$lib/api';
	import Wordmark from '$lib/components/Wordmark.svelte';
	import PasswordInput from '$lib/components/PasswordInput.svelte';
	import { i18n, LOCALE_NAMES, LOCALES, setLocale, t, type Locale } from '$lib/i18n/index.svelte';
	import { loadSession, setUser, type User } from '$lib/session.svelte';

	let username = $state('');
	let password = $state('');
	let error = $state('');
	let busy = $state(false);

	const next = $derived(safeNext(page.url.searchParams.get('next')));
	// Over plain HTTP (except localhost) the browser drops the Secure session
	// cookie: sign-in "succeeds" and then bounces back here. Say why instead.
	const insecure = typeof window !== 'undefined' && !window.isSecureContext;

	// Already signed in (e.g. opened /login from history): go straight in.
	onMount(() => {
		loadSession()
			.then((u) => u && goto(next, { replaceState: true }))
			.catch(() => {});
	});

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		busy = true;
		error = '';
		try {
			setUser(await api<User>('POST', '/auth/login', { username, password }));
			await goto(next, { replaceState: true });
		} catch (err) {
			error = errorMessage(err);
			password = '';
		} finally {
			busy = false;
		}
	}
</script>

<svelte:head><title>{t('login.title')} · Autokarta</title></svelte:head>

<main>
	<div class="brand"><Wordmark size="lg" /></div>

	<form class="card" onsubmit={submit}>
		<h1>{t('login.title')}</h1>
		{#if insecure}<div class="notice error" role="alert">{t('login.httpsRequired')}</div>{/if}
		{#if error}<div class="notice error" role="alert">{error}</div>{/if}

		<label class="field" for="username">
			{t('login.username')}
		</label>
		<input
			id="username"
			type="text"
			bind:value={username}
			autocomplete="username"
			autocapitalize="off"
			autocorrect="off"
			spellcheck="false"
			required
		/>

		<label class="field pw" for="password">{t('login.password')}</label>
		<PasswordInput id="password" bind:value={password} />

		<button class="btn primary block submit" type="submit" disabled={busy}>
			{busy ? t('login.signingIn') : t('login.submit')}
		</button>
		<p class="hint forgot">{t('login.forgot')}</p>
	</form>

	<div class="lang">
		<label class="visually-hidden" for="lang">{t('account.language')}</label>
		<select id="lang" value={i18n.locale} onchange={(e) => setLocale(e.currentTarget.value as Locale)}>
			{#each LOCALES as loc (loc)}
				<option value={loc} lang={loc}>{LOCALE_NAMES[loc]}</option>
			{/each}
		</select>
	</div>
</main>

<style>
	main {
		max-width: 420px;
		margin: 0 auto;
		padding: max(48px, env(safe-area-inset-top)) var(--gutter) 32px;
	}
	.brand {
		display: flex;
		justify-content: center;
		margin-bottom: 28px;
	}
	label.field {
		margin-bottom: 6px;
	}
	label.pw {
		margin-top: 14px;
	}
	.submit {
		margin-top: 22px;
	}
	.forgot {
		margin: 14px 0 0;
	}
	.lang {
		display: flex;
		justify-content: center;
	}
	.lang select {
		width: auto;
		min-width: 200px;
		text-align: center;
	}
</style>
