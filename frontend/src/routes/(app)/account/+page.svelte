<script lang="ts">
	import { goto } from '$app/navigation';
	import { api, errorMessage } from '$lib/api';
	import PasswordInput from '$lib/components/PasswordInput.svelte';
	import { i18n, LOCALE_NAMES, LOCALES, setLocale, t, type Locale } from '$lib/i18n/index.svelte';
	import { logout, session, setUser, type User } from '$lib/session.svelte';
	import { setTheme, theme, THEMES } from '$lib/theme.svelte';

	let displayName = $state(session.user?.display_name ?? '');
	let profileBusy = $state(false);
	let profileMsg = $state<{ ok: boolean; text: string } | null>(null);

	async function saveProfile(e: SubmitEvent) {
		e.preventDefault();
		profileBusy = true;
		profileMsg = null;
		try {
			setUser(await api<User>('PATCH', '/auth/me', { display_name: displayName }));
			profileMsg = { ok: true, text: t('account.profileSaved') };
		} catch (err) {
			profileMsg = { ok: false, text: errorMessage(err) };
		} finally {
			profileBusy = false;
		}
	}

	let langError = $state('');
	async function chooseLanguage(locale: Locale) {
		const previous = i18n.locale;
		setLocale(locale); // instant feedback; saved to the profile below
		langError = '';
		try {
			setUser(await api<User>('PATCH', '/auth/me', { locale }));
		} catch (err) {
			setLocale(previous);
			langError = errorMessage(err);
		}
	}

	let pw = $state({ current: '', next: '', repeat: '' });
	let pwBusy = $state(false);
	let pwMsg = $state<{ ok: boolean; text: string } | null>(null);

	async function changePassword(e: SubmitEvent) {
		e.preventDefault();
		pwMsg = null;
		if (pw.next !== pw.repeat) {
			pwMsg = { ok: false, text: t('errors.passwords_do_not_match') };
			return;
		}
		pwBusy = true;
		try {
			await api('POST', '/auth/password', { current_password: pw.current, new_password: pw.next });
			pw = { current: '', next: '', repeat: '' };
			pwMsg = { ok: true, text: t('account.passwordChanged') };
		} catch (err) {
			pwMsg = { ok: false, text: errorMessage(err) };
		} finally {
			pwBusy = false;
		}
	}

	async function signOut() {
		try {
			await logout();
		} finally {
			await goto('/login', { replaceState: true });
		}
	}
</script>

<svelte:head><title>{t('account.title')} · Autokarta</title></svelte:head>

<h1>{t('account.title')}</h1>

<section class="card">
	<h2>{t('account.profile')}</h2>
	<p class="hint">
		{t('account.signedInAs', { username: session.user?.username ?? '' })} ·
		{session.user ? t(`roles.${session.user.role}`) : ''}
	</p>
	<form onsubmit={saveProfile}>
		{#if profileMsg}
			<div class="notice" class:ok={profileMsg.ok} class:error={!profileMsg.ok} role="status">
				{profileMsg.text}
			</div>
		{/if}
		<label class="field">
			{t('staff.displayName')}
			<input type="text" bind:value={displayName} required maxlength="80" />
		</label>
		<button class="btn primary" type="submit" disabled={profileBusy}>{t('common.saveChanges')}</button>
	</form>
</section>

<section class="card">
	<h2>{t('account.language')}</h2>
	{#if langError}<div class="notice error" role="alert">{langError}</div>{/if}
	<div class="segmented">
		{#each LOCALES as loc (loc)}
			<button
				type="button"
				class="btn"
				lang={loc}
				aria-pressed={i18n.locale === loc}
				onclick={() => chooseLanguage(loc)}>{LOCALE_NAMES[loc]}</button
			>
		{/each}
	</div>
</section>

<section class="card">
	<h2>{t('account.theme')}</h2>
	<p class="hint">{t('account.themeHint')}</p>
	<div class="segmented">
		{#each THEMES as th (th)}
			<button type="button" class="btn" aria-pressed={theme.pref === th} onclick={() => setTheme(th)}
				>{t(`account.themes.${th}`)}</button
			>
		{/each}
	</div>
</section>

<section class="card">
	<h2>{t('account.changePassword')}</h2>
	<form onsubmit={changePassword}>
		{#if pwMsg}
			<div class="notice" class:ok={pwMsg.ok} class:error={!pwMsg.ok} role="status">{pwMsg.text}</div>
		{/if}
		<label class="field" for="pw-current">{t('account.currentPassword')}</label>
		<PasswordInput id="pw-current" bind:value={pw.current} />
		<label class="field spaced" for="pw-new">
			{t('account.newPassword')}
			<span class="hint">{t('account.passwordRule')}</span>
		</label>
		<PasswordInput id="pw-new" bind:value={pw.next} autocomplete="new-password" />
		<label class="field spaced" for="pw-repeat">{t('account.repeatPassword')}</label>
		<PasswordInput id="pw-repeat" bind:value={pw.repeat} autocomplete="new-password" />
		<p class="hint spaced">{t('account.passwordSignsOut')}</p>
		<button class="btn primary" type="submit" disabled={pwBusy}>{t('account.changePasswordSubmit')}</button>
	</form>
</section>

<button class="btn danger block" type="button" onclick={signOut}>{t('account.signOut')}</button>

<style>
	label.field {
		margin-bottom: 6px;
	}
	.spaced {
		margin-top: 14px;
	}
</style>
