<script lang="ts">
	// Camera preview that reports barcodes / QR codes as they are seen.
	// The same code staying in view is reported once, not on every frame.
	import { onDestroy, onMount } from 'svelte';
	import { t, type MessageKey } from '$lib/i18n/index.svelte';
	import { createScanner, type Detection, type Scanner } from '$lib/scanner';

	let {
		onDetect,
		autostart = false,
		showEngine = false
	}: {
		onDetect: (d: Detection) => void;
		autostart?: boolean;
		showEngine?: boolean;
	} = $props();

	let video: HTMLVideoElement;
	let stream: MediaStream | null = null;
	let scanner: Scanner | null = null;
	let timer: ReturnType<typeof setTimeout> | undefined;
	let last: { key: string; at: number } | null = null;
	// Bumped by every start() and stop(). getUserMedia can resolve after the
	// sheet was closed (e.g. the permission prompt was still open); a stale
	// start must release its camera instead of leaving it on.
	let attempt = 0;

	let status = $state<'idle' | 'starting' | 'running'>('idle');
	let error = $state<MessageKey | null>(null);
	let engine = $state<Scanner['engine'] | null>(null);
	let torchAvailable = $state(false);
	let torchOn = $state(false);

	export async function start() {
		error = null;
		if (!window.isSecureContext || !navigator.mediaDevices?.getUserMedia) {
			error = 'scanner.errors.insecure';
			return;
		}
		stop();
		const mine = ++attempt;
		status = 'starting';
		let s: MediaStream;
		try {
			s = await navigator.mediaDevices.getUserMedia({
				video: { facingMode: { ideal: 'environment' }, width: { ideal: 1920 }, height: { ideal: 1080 } },
				audio: false
			});
		} catch (e) {
			if (mine !== attempt) return;
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
		if (mine !== attempt) {
			s.getTracks().forEach((tr) => tr.stop());
			return;
		}
		stream = s;

		try {
			video.srcObject = s;
			await video.play();
			scanner ??= await createScanner();
			engine = scanner.engine;
		} catch {
			if (mine !== attempt) return;
			stop();
			error = 'scanner.errors.engine';
			return;
		}
		if (mine !== attempt) return;

		const track = s.getVideoTracks()[0];
		const caps = (track.getCapabilities?.() ?? {}) as MediaTrackCapabilities & { torch?: boolean };
		torchAvailable = caps.torch === true;
		torchOn = false;

		status = 'running';
		loop();
	}

	async function loop() {
		if (status !== 'running' || !scanner) return;
		try {
			for (const d of await scanner.detect(video)) {
				const key = d.format + ':' + d.text;
				const now = Date.now();
				const repeat = last?.key === key && now - last.at < 3000;
				last = { key, at: now };
				if (!repeat) {
					navigator.vibrate?.(60);
					onDetect(d);
				}
			}
		} catch {
			// A single bad frame is not worth reporting; keep scanning.
		}
		// zxing runs on the main thread, so give the UI some breathing room.
		if (status === 'running') timer = setTimeout(loop, scanner.engine === 'native' ? 120 : 250);
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

	export function stop() {
		attempt++;
		clearTimeout(timer);
		stream?.getTracks().forEach((tr) => tr.stop());
		stream = null;
		if (video) video.srcObject = null;
		status = 'idle';
		torchOn = false;
	}

	onMount(() => {
		if (autostart) start();
	});
	onDestroy(stop);
</script>

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
			{#if showEngine}{engine === 'native' ? t('scanner.engineNative') : t('scanner.engineZxing')}{/if}
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
</style>
