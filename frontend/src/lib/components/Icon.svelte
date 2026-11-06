<script lang="ts" module>
	// A handful of stroke icons drawn for this app (no icon font, no CDN).
	export type IconName = 'home' | 'scan' | 'staff' | 'account';
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
