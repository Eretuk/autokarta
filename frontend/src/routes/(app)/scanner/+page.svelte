<script lang="ts">
	// Camera scanner test page: checks that barcode and QR reading works on
	// this device before it is used for parts stock.
	import { onDestroy } from 'svelte';
	import { formatDate, t, type MessageKey } from '$lib/i18n/index.svelte';
	import { createScanner, formatLabel, type Detection, type Scanner } from '$lib/scanner';

	interface Hit extends Detection {
		id: number;
		at: Date;
	}

	let nextId = 0;

	let video: HTMLVideoElement;
	let stream: MediaStream | null = null;
	let scanner: Scanner | null = null;
	let timer: ReturnType<typeof setTimeout> | undefined;

	let status = $state<'idle' | 'starting' | 'running'>('idle');
	let error = $state<MessageKey | null>(null);
	let engine = $state<Scanner['engine'] | null>(null);
	let hits = $state<Hit[]>([]);
	let torchAvailable = $state(false);
	let torchOn = $state(false);

	async function start() {
		error = null;
		if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
			error = 'scanner.errors.insecure';
			return;
		}
		status = 'starting';
		try {
			stream = await navigator.mediaDevices.getUserMedia({
				video: { facingMode: { ideal: 'environment' }, width: { ideal: 1920 }, height: { ideal: 1080 } },
				audio: false
			});
		} catch (e) {
			const name = e instanceof DOMException ? e.name : '';
			error =
				name === 'NotAllowedError'
					? 'scanner.errors.denied'
					: name === 'NotFoundError' || name === 'OverconstrainedError'
						? 'scanner.errors.noCamera'
						: 'scanner.errors.generic';
			status = 'idle';
			return;
		}

		try {
			video.srcObject = stream;
			await video.play();
			scanner ??= await createScanner();
			engine = scanner.engine;
		} catch {
			stop();
			error = 'scanner.errors.engine';
			return;
		}

		const track = stream.getVideoTracks()[0];
		const caps = (track.getCapabilities?.() ?? {}) as MediaTrackCapabilities & { torch?: boolean };
		torchAvailable = caps.torch === true;
		torchOn = false;

		status = 'running';
		loop();
	}

	async function loop() {
		if (status !== 'running' || !scanner) return;
		try {
			const found = await scanner.detect(video);
			for (const d of found) record(d);
		} catch {
			// A single bad frame is not worth reporting; keep scanning.
		}
		// zxing runs on the main thread, so give the UI some breathing room.
		timer = setTimeout(loop, scanner.engine === 'native' ? 120 : 250);
	}

	function record(d: Detection) {
		const last = hits[0];
		// The same code stays in view for many frames; only log it once.
		if (last && last.text === d.text && last.format === d.format && Date.now() - last.at.getTime() < 3000) {
			last.at = new Date();
			return;
		}
		hits = [{ ...d, id: nextId++, at: new Date() }, ...hits].slice(0, 20);
		navigator.vibrate?.(60);
	}

	async function toggleTorch() {
		const track = stream?.getVideoTracks()[0];
		if (!track) return;
		try {
			await track.applyConstraints({ advanced: [{ torch: !torchOn } as MediaTrackConstraintSet] });
			torchOn = !torchOn;
		} catch {
			torchAvailable = false;
		}
	}

	function stop() {
		clearTimeout(timer);
		stream?.getTracks().forEach((tr) => tr.stop());
		stream = null;
		if (video) video.srcObject = null;
		status = 'idle';
		torchOn = false;
	}

	onDestroy(stop);
</script>

<svelte:head><title>{t('scanner.title')} · Autokarta</title></svelte:head>

<h1>{t('scanner.title')}</h1>
<p class="hint">{t('scanner.intro')}</p>

{#if error}<div class="notice error" role="alert">{t(error)}</div>{/if}

<div class="viewport" class:live={status === 'running'}>
	<!-- svelte-ignore a11y_media_has_caption -->
	<video bind:this={video} playsinline muted></video>
	{#if status === 'running'}<div class="frame" aria-hidden="true"></div>{/if}
	{#if status !== 'running'}
		<div class="placeholder">
			<button class="btn primary" type="button" onclick={start} disabled={status === 'starting'}>
				{status === 'starting' ? t('scanner.starting') : t('scanner.start')}
			</button>
		</div>
	{/if}
</div>

{#if status === 'running'}
	<div class="controls">
		<span class="hint">
			{engine === 'native' ? t('scanner.engineNative') : t('scanner.engineZxing')}
		</span>
		<span class="buttons">
			{#if torchAvailable}
				<button class="btn" type="button" aria-pressed={torchOn} onclick={toggleTorch}>
					{torchOn ? t('scanner.torchOff') : t('scanner.torchOn')}
				</button>
			{/if}
			<button class="btn" type="button" onclick={stop}>{t('scanner.stop')}</button>
		</span>
	</div>
{/if}

<section class="results" aria-live="polite">
	<h2>{t('scanner.results')}</h2>
	{#if hits.length === 0}
		<p class="hint">{t('scanner.empty')}</p>
	{:else}
		<ol>
			{#each hits as hit (hit.id)}
				<li>
					<code>{hit.text}</code>
					<span class="hint">
						{formatLabel(hit.format)} · {formatDate(hit.at, { timeStyle: 'medium' })}
					</span>
				</li>
			{/each}
		</ol>
	{/if}
</section>

<style>
	.viewport {
		position: relative;
		aspect-ratio: 4 / 3;
		max-height: 60dvh;
		width: 100%;
		background: #0d110f;
		border-radius: var(--radius);
		overflow: hidden;
	}
	video {
		width: 100%;
		height: 100%;
		object-fit: cover;
		display: block;
	}
	.viewport:not(.live) video {
		visibility: hidden;
	}
	.placeholder {
		position: absolute;
		inset: 0;
		display: grid;
		place-items: center;
	}
	.frame {
		position: absolute;
		inset: 22% 10%;
		border: 3px solid rgb(255 255 255 / 0.85);
		border-radius: 8px;
		box-shadow: 0 0 0 100vmax rgb(0 0 0 / 0.25);
	}
	.controls {
		display: flex;
		align-items: center;
		justify-content: space-between;
		flex-wrap: wrap;
		gap: 10px;
		margin: 12px 0;
	}
	.buttons {
		display: flex;
		gap: 8px;
	}
	.results {
		margin-top: 20px;
	}
	ol {
		list-style: none;
		margin: 0;
		padding: 0;
	}
	li {
		display: grid;
		gap: 2px;
		padding: 10px 0;
		border-bottom: 1px solid var(--line);
	}
	code {
		font-size: 1.1rem;
		font-weight: 600;
		overflow-wrap: anywhere;
	}
</style>
