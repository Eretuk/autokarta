// Service worker updates. A new version waits until the user reloads: an
// automatic reload would throw away a form someone is filling in.

export const appUpdate = $state<{ available: boolean }>({ available: false });

let update: ((reload?: boolean) => Promise<void>) | null = null;

export async function registerServiceWorker() {
	const { registerSW } = await import('virtual:pwa-register');
	update = registerSW({
		immediate: true,
		onNeedRefresh() {
			appUpdate.available = true;
		}
	});
}

/** Activates the waiting version and reloads the page. */
export function applyUpdate() {
	update?.(true);
}
