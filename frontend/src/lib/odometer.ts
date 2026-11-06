import { ApiError } from './api';
import { t } from './i18n/index.svelte';

/**
 * Runs a request that saves an odometer reading. If the server refuses a
 * reading lower than the previous one, asks the person and retries with
 * `confirm_lower`. Returns null when they decline.
 */
export async function withLowerConfirm<T>(send: (confirmLower: boolean) => Promise<T>): Promise<T | null> {
	try {
		return await send(false);
	} catch (e) {
		if (e instanceof ApiError && e.code === 'odometer_lower_than_previous') {
			if (!confirm(t('odometer.confirmLower'))) return null;
			return await send(true);
		}
		throw e;
	}
}

/** "123 456" -> 123456; null for empty; NaN for garbage. */
export function parseReading(raw: string): number | null {
	const v = raw.replace(/[\s .,']/g, '');
	if (!v) return null;
	return /^\d+$/.test(v) ? Number(v) : NaN;
}
