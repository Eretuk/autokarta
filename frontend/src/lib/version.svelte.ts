// The server's version and, for owners, whether a newer release is out
// (the server asks GitHub twice a day; see backend/src/updates.rs).

import { api } from './api';
import type { VersionInfo } from './types';

export const appVersion = $state<{ info: VersionInfo | null }>({ info: null });

const REFRESH_EVERY_MS = 60 * 60 * 1000;
let loadedAt = 0;

/** At most once an hour; failures keep what we have (the app works without it). */
export async function loadVersion() {
	if (Date.now() - loadedAt < REFRESH_EVERY_MS) return;
	loadedAt = Date.now();
	try {
		appVersion.info = await api<VersionInfo>('GET', '/version');
	} catch {
		loadedAt = 0;
	}
}
