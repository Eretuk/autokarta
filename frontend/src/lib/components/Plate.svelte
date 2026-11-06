<script lang="ts">
	// A vehicle's plate drawn as a small number plate, so it is recognised at
	// a glance in lists. Falls back to VIN / frame number in monospace.
	let {
		plate,
		fallback = null,
		size = 'md'
	}: { plate: string | null; fallback?: string | null; size?: 'md' | 'lg' } = $props();
</script>

{#if plate}
	<span class="plate {size}"><span class="strip" aria-hidden="true"></span><span class="txt">{plate}</span></span>
{:else if fallback}
	<span class="ident {size}">{fallback}</span>
{/if}

<style>
	.plate {
		display: inline-flex;
		align-items: stretch;
		background: var(--plate);
		color: var(--plate-ink);
		border: 2px solid var(--plate-ink);
		border-radius: 4px;
		overflow: hidden;
		white-space: nowrap;
		line-height: 1;
	}
	.strip {
		width: 7px;
		background: #1f4e9c;
	}
	.txt {
		padding: 4px 7px 3px;
		font-family: var(--font-head);
		font-weight: 700;
		font-size: 1.05rem;
		letter-spacing: 0.06em;
	}
	.lg .strip {
		width: 11px;
	}
	.lg .txt {
		font-size: 1.7rem;
		padding: 5px 12px 4px;
	}
	.ident {
		font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
		font-weight: 600;
		overflow-wrap: anywhere;
	}
	.ident.lg {
		font-size: 1.25rem;
	}
</style>
