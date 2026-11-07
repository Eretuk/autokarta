<script lang="ts" module>
	// A handful of stroke icons drawn for this app (no icon font, no CDN).
	export type IconName = 'home' | 'scan' | 'staff' | 'account' | 'car' | 'orders' | 'plus' | 'box';
</script>

<script lang="ts">
	let { name }: { name: IconName } = $props();
</script>

<svg viewBox="0 0 24 24" aria-hidden="true">
	{#if name === 'home'}
		<path d="M3 11l9-7 9 7" /><path d="M5 10v10h14V10" /><path d="M10 20v-6h4v6" />
	{:else if name === 'scan'}
		<path d="M3 8V4h4M17 4h4v4M21 16v4h-4M7 20H3v-4" /><path d="M7 8v8M10 8v8M13 8v8M16 8v8" />
	{:else if name === 'staff'}
		<circle cx="9" cy="8" r="3.2" /><path d="M3 20c0-3.3 2.7-6 6-6s6 2.7 6 6" />
		<path d="M16 5.2a3 3 0 010 5.6M18 14.4c1.8.9 3 2.9 3 5.6" />
	{:else if name === 'car'}
		<path d="M5 16V11l2-5h10l2 5v5" /><path d="M3 16h18v3H3z" /><path d="M5 11h14" />
		<circle cx="7.5" cy="16" r="0.5" /><circle cx="16.5" cy="16" r="0.5" />
	{:else if name === 'orders'}
		<rect x="5" y="4" width="14" height="17" rx="1.5" /><path d="M9 4V2.8h6V4" />
		<path d="M8.5 10h7M8.5 14h7M8.5 18h4" />
	{:else if name === 'box'}
		<path d="M3.5 7.5L12 3l8.5 4.5v9L12 21l-8.5-4.5z" /><path d="M3.5 7.5L12 12l8.5-4.5M12 12v9" />
	{:else if name === 'plus'}
		<path d="M12 5v14M5 12h14" />
	{:else if name === 'account'}
		<circle cx="12" cy="8" r="3.6" /><path d="M4.5 20.5c.8-3.8 3.9-6 7.5-6s6.7 2.2 7.5 6" />
	{/if}
</svg>

<style>
	svg {
		width: 26px;
		height: 26px;
		fill: none;
		stroke: currentColor;
		stroke-width: 2;
		stroke-linecap: round;
		stroke-linejoin: round;
	}
</style>
