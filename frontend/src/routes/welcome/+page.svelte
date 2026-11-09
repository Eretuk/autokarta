<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, errorMessage } from '$lib/api';
	import PasswordInput from '$lib/components/PasswordInput.svelte';
	import Wordmark from '$lib/components/Wordmark.svelte';
	import { t } from '$lib/i18n/index.svelte';
	import { logout, session, setUser, takeLoginPassword, type User } from '$lib/session.svelte';

	let { data } = $props();

	// Typed a moment ago on the login page; asked again only after a reload.
	const remembered = takeLoginPassword();
	let current = $state(remembered ?? '');
	let next = $state('');
	let repeat = $state('');
	let error = $state('');
	let busy = $state(false);

	async function submit(e: SubmitEvent) {
		e.preventDefault();
		error = '';
		if (next !== repeat) {
			error = t('errors.passwords_do_not_match');
			return;
		}
		busy = true;
		try {
			await api('POST', '/auth/password', { current_password: current, new_password: next });
			setUser(await api<User>('GET', '/auth/me'));
			await goto(data.next, { replaceState: true });
		} catch (err) {
			error = errorMessage(err);
		} finally {
			busy = false;
		}
	}

	async function signOut() {
		await logout();
		await goto('/login', { replaceState: true });
	}
</script>

<svelte:head><title>{t('welcome.title')} · Autokarta</title></svelte:head>

<main>
	<div class="brand"><Wordmark size="lg" /></div>

	<form class="card" onsubmit={submit}>
		<h1>{t('welcome.title')}</h1>
		<p class="hint">{t('welcome.hint')}</p>
		{#if session.user}
			<p class="hint">{t('account.signedInAs', { username: session.user.username })}</p>
		{/if}
		{#if error}<div class="notice error" role="alert">{error}</div>{/if}

		{#if remembered === null}
			<label class="field" for="current">{t('welcome.current')}</label>
			<PasswordInput id="current" bind:value={current} />
		{/if}

		<label class="field" for="new">
			{t('account.newPassword')}
			<span class="hint">{t('account.passwordRule')}</span>
		</label>
		<PasswordInput id="new" bind:value={next} autocomplete="new-password" />
		<label class="field" for="repeat">{t('account.repeatPassword')}</label>
		<PasswordInput id="repeat" bind:value={repeat} autocomplete="new-password" />

		<button class="btn primary block submit" type="submit" disabled={busy}>{t('welcome.submit')}</button>
	</form>

	<div class="out">
		<button class="btn" type="button" onclick={signOut}>{t('account.signOut')}</button>
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
		margin: 14px 0 6px;
	}
	.submit {
		margin-top: 22px;
	}
	.out {
		display: flex;
		justify-content: center;
	}
</style>
