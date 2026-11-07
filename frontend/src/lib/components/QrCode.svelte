<script lang="ts">
	// QR code drawn as one SVG path: sharp at any print size, no canvas.
	import { encode } from 'uqr';

	let { text, label }: { text: string; label?: string } = $props();

	// Medium error correction survives a scratched or oily label; a two-module
	// quiet zone is enough on a white sticker.
	const qr = $derived(encode(text, { ecc: 'M', border: 2 }));
	const path = $derived.by(() => {
		let d = '';
		qr.data.forEach((row, y) =>
			row.forEach((dark, x) => {
				if (dark) d += `M${x} ${y}h1v1h-1z`;
			})
		);
		return d;
	});
</script>

<svg
	viewBox="0 0 {qr.size} {qr.size}"
	shape-rendering="crispEdges"
	role={label ? 'img' : undefined}
	aria-label={label}
	aria-hidden={label ? undefined : 'true'}
>
	<rect width={qr.size} height={qr.size} fill="#fff" />
	<path d={path} fill="#000" />
</svg>

<style>
	svg {
		display: block;
		width: 100%;
		height: auto;
	}
</style>
