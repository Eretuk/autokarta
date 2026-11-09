<script lang="ts">
	import { onMount } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import PasswordInput from '$lib/components/PasswordInput.svelte';
	import Sheet from '$lib/components/Sheet.svelte';
	import { formatDate, i18n, LOCALE_NAMES, LOCALES, t, type Locale, type MessageKey } from '$lib/i18n/index.svelte';
	import { session, type Role } from '$lib/session.svelte';

	interface StaffMember {
		id: string;
		username: string;
		display_name: string;
		role: Role;
		locale: Locale;
		is_active: boolean;
		created_at: string;
	}

	const ROLES: Role[] = ['mechanic', 'owner'];

	let staff = $state<StaffMember[]>([]);
	let loading = $state(true);
	let loadError = $state('');
	let flash = $state('');

	const isOwner = $derived(session.user?.role === 'owner');
	const activeCount = $derived(staff.filter((s) => s.is_active).length);

	async function refresh() {
		try {
			staff = await api<StaffMember[]>('GET', '/users');
			loadError = '';
		} catch (e) {
			loadError = errorMessage(e);
		} finally {
			loading = false;
		}
	}

	onMount(() => {
		if (isOwner) refresh();
	});

	// ----- add -----
	let addOpen = $state(false);
	let addBusy = $state(false);
	let addError = $state('');
	let form = $state({
		display_name: '',
		username: '',
		role: 'mechanic' as Role,
		locale: i18n.locale as Locale,
		password: ''
	});

	function openAdd() {
		form = { display_name: '', username: '', role: 'mechanic', locale: i18n.locale, password: '' };
		addError = '';
		addOpen = true;
	}

	async function submitAdd(e: SubmitEvent) {
		e.preventDefault();
		addBusy = true;
		addError = '';
		try {
			const created = await api<StaffMember>('POST', '/users', form);
			addOpen = false;
			flash = t('staff.added', { name: created.display_name });
			await refresh();
		} catch (err) {
			addError = errorMessage(err);
		} finally {
			addBusy = false;
		}
	}

	// ----- edit -----
	let editing = $state<StaffMember | null>(null);
	let editOpen = $state(false);
	let edit = $state({ display_name: '', role: 'mechanic' as Role });
	let editBusy = $state(false);
	let editError = $state('');
	let newPassword = $state('');
	let pwBusy = $state(false);
	let pwError = $state('');
	let pwDone = $state(false);

	const isSelf = $derived(editing?.id === session.user?.id);

	function openEdit(m: StaffMember) {
		editing = m;
		edit = { display_name: m.display_name, role: m.role };
		editError = '';
		newPassword = '';
		pwError = '';
		pwDone = false;
		editOpen = true;
	}

	async function patch(body: Record<string, unknown>, message: MessageKey) {
		if (!editing) return;
		editBusy = true;
		editError = '';
		try {
			const updated = await api<StaffMember>('PATCH', `/users/${editing.id}`, body);
			editOpen = false;
			flash = t(message, { name: updated.display_name });
			await refresh();
		} catch (err) {
			editError = errorMessage(err);
		} finally {
			editBusy = false;
		}
	}

	function submitEdit(e: SubmitEvent) {
		e.preventDefault();
		patch({ display_name: edit.display_name, role: isSelf ? undefined : edit.role }, 'staff.saved');
	}

	async function submitPassword(e: SubmitEvent) {
		e.preventDefault();
		if (!editing) return;
		pwBusy = true;
		pwError = '';
		pwDone = false;
		try {
			await api('POST', `/users/${editing.id}/password`, { password: newPassword });
			newPassword = '';
			pwDone = true;
		} catch (err) {
			pwError = errorMessage(err);
		} finally {
			pwBusy = false;
		}
	}
</script>

<svelte:head><title>{t('staff.title')} · Autokarta</title></svelte:head>

{#if !isOwner}
	<h1>{t('staff.title')}</h1>
	<div class="notice error">{t('errors.forbidden')}</div>
{:else}
	<div class="head">
		<div>
			<h1>{t('staff.title')}</h1>
			{#if !loading && !loadError}
				<p class="hint">{t('staff.activeCount', { count: activeCount })}</p>
			{/if}
		</div>
		<button class="btn primary" type="button" onclick={openAdd}>{t('staff.add')}</button>
	</div>

	{#if flash}<div class="notice ok" role="status">{flash}</div>{/if}
	{#if loadError}<div class="notice error" role="alert">{loadError}</div>{/if}

	{#if loading}
		<p class="hint">{t('common.loading')}</p>
	{:else}
		<ul class="list">
			{#each staff as m (m.id)}
				<li>
					<button type="button" class="row" class:inactive={!m.is_active} onclick={() => openEdit(m)}>
						<span class="who">
							<span class="name">{m.display_name}</span>
							<span class="hint">{m.username}</span>
						</span>
						<span class="badges">
							<span class="badge" class:accent={m.role === 'owner'}>{t(`roles.${m.role}`)}</span>
							{#if !m.is_active}<span class="badge muted">{t('staff.inactive')}</span>{/if}
						</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
{/if}

<Sheet bind:open={addOpen} title={t('staff.addTitle')}>
	<form onsubmit={submitAdd}>
		{#if addError}<div class="notice error" role="alert">{addError}</div>{/if}
		<label class="field">
			{t('staff.displayName')}
			<input type="text" bind:value={form.display_name} required maxlength="80" autocomplete="off" />
		</label>
		<label class="field">
			{t('staff.username')}
			<span class="hint">{t('staff.usernameHint')}</span>
			<input
				type="text"
				bind:value={form.username}
				required
				minlength="3"
				maxlength="32"
				pattern="[A-Za-z0-9._\-]+"
				autocomplete="off"
				autocapitalize="off"
				autocorrect="off"
				spellcheck="false"
			/>
		</label>
		<fieldset>
			<legend>{t('staff.role')}</legend>
			<div class="segmented">
				{#each ROLES as r (r)}
					<button type="button" class="btn" aria-pressed={form.role === r} onclick={() => (form.role = r)}
						>{t(`roles.${r}`)}</button
					>
				{/each}
			</div>
			<p class="hint">{t(`staff.roleHint.${form.role}`)}</p>
		</fieldset>
		<label class="field">
			{t('account.language')}
			<select bind:value={form.locale}>
				{#each LOCALES as loc (loc)}<option value={loc}>{LOCALE_NAMES[loc]}</option>{/each}
			</select>
		</label>
		<label class="field" for="new-staff-password">
			{t('staff.initialPassword')}
			<span class="hint">{t('staff.initialPasswordHint')}</span>
		</label>
		<PasswordInput id="new-staff-password" bind:value={form.password} autocomplete="new-password" />
		<button class="btn primary block gap" type="submit" disabled={addBusy}>{t('staff.addSubmit')}</button>
	</form>
</Sheet>

<Sheet bind:open={editOpen} title={editing?.display_name ?? ''}>
	{#if editing}
		<p class="hint">
			{editing.username} · {t('staff.since', { date: formatDate(editing.created_at) })}
		</p>
		<form onsubmit={submitEdit}>
			{#if editError}<div class="notice error" role="alert">{editError}</div>{/if}
			<label class="field">
				{t('staff.displayName')}
				<input type="text" bind:value={edit.display_name} required maxlength="80" />
			</label>
			<fieldset disabled={isSelf}>
				<legend>{t('staff.role')}</legend>
				<div class="segmented">
					{#each ROLES as r (r)}
						<button type="button" class="btn" aria-pressed={edit.role === r} onclick={() => (edit.role = r)}
							>{t(`roles.${r}`)}</button
						>
					{/each}
				</div>
				{#if isSelf}<p class="hint">{t('staff.selfHint')}</p>{/if}
			</fieldset>
			<button class="btn primary block" type="submit" disabled={editBusy}>{t('common.saveChanges')}</button>
		</form>

		<!-- Your own password: account page (asks for the current one). -->
		{#if !isSelf}
			<form class="section" onsubmit={submitPassword}>
				<h3>{t('staff.resetPassword')}</h3>
				<p class="hint">{t('staff.resetPasswordHint')}</p>
				{#if pwError}<div class="notice error" role="alert">{pwError}</div>{/if}
				{#if pwDone}<div class="notice ok" role="status">{t('staff.passwordSet')}</div>{/if}
				<label class="visually-hidden" for="reset-password">{t('account.newPassword')}</label>
				<PasswordInput id="reset-password" bind:value={newPassword} autocomplete="new-password" />
				<button class="btn block gap" type="submit" disabled={pwBusy}>{t('staff.setPassword')}</button>
			</form>
		{/if}

		{#if !isSelf}
			<div class="section">
				{#if editing.is_active}
					<p class="hint">{t('staff.deactivateHint')}</p>
					<button
						class="btn danger block"
						type="button"
						disabled={editBusy}
						onclick={() => patch({ is_active: false }, 'staff.deactivated')}>{t('staff.deactivate')}</button
					>
				{:else}
					<button
						class="btn block"
						type="button"
						disabled={editBusy}
						onclick={() => patch({ is_active: true }, 'staff.activated')}>{t('staff.activate')}</button
					>
				{/if}
			</div>
		{/if}
	{/if}
</Sheet>

<style>
	.head {
		display: flex;
		align-items: flex-start;
		justify-content: space-between;
		gap: 12px;
		flex-wrap: wrap;
		margin-bottom: 12px;
	}
	.head h1 {
		margin-bottom: 2px;
	}
	.list {
		list-style: none;
		margin: 0;
		padding: 0;
		border: 1px solid var(--line);
		border-radius: var(--radius);
		background: var(--surface);
		overflow: hidden;
	}
	.list li + li {
		border-top: 1px solid var(--line);
	}
	.row {
		width: 100%;
		min-height: 64px;
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 12px;
		padding: 10px var(--gutter);
		font: inherit;
		color: inherit;
		text-align: left;
		background: none;
		border: none;
		cursor: pointer;
	}
	.row:hover {
		background: var(--surface-2);
	}
	.row.inactive .name {
		color: var(--ink-muted);
		text-decoration: line-through;
	}
	.who {
		display: grid;
		min-width: 0;
	}
	.name {
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.badges {
		display: flex;
		flex-wrap: wrap;
		justify-content: flex-end;
		gap: 6px;
	}
	fieldset {
		border: none;
		margin: 0 0 14px;
		padding: 0;
	}
	legend {
		font-weight: 500;
		margin-bottom: 6px;
		padding: 0;
	}
	fieldset .hint {
		margin: 6px 0 0;
	}
	.section {
		margin-top: 24px;
		padding-top: 18px;
		border-top: 1px solid var(--line);
	}
	.gap {
		margin-top: 14px;
	}
</style>
