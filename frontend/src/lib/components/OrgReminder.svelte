<script lang="ts">
	// A quiet nudge for owners while the company details are incomplete.
	// It never blocks work, and mechanics never see it.
	import { t } from '$lib/i18n/index.svelte';
	import { session } from '$lib/session.svelte';
	import { missingOrgFields } from '$lib/settings.svelte';

	const missing = $derived(session.user?.role === 'owner' ? missingOrgFields() : []);
</script>

{#if missing.length > 0}
	<div class="notice reminder" role="status">
		<span>{t('organization.missing', { fields: missing.map((f) => t(`organization.fields.${f}`)).join(', ') })}</span>
		<a href="/organization">{t('organization.fillIn')}</a>
	</div>
{/if}

<style>
	.reminder {
		display: flex;
		flex-wrap: wrap;
		align-items: center;
		justify-content: space-between;
		gap: 8px 16px;
	}
	.reminder a {
		font-weight: 600;
		min-height: var(--tap);
		display: inline-flex;
		align-items: center;
	}
</style>
