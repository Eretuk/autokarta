// Money, quantities and vehicle labels. Amounts travel as integers (minor
// units, thousandths); only this module converts them to and from text.

import { formatNumber, i18n, t } from './i18n/index.svelte';
import type { PartUnit } from './types';

/** Number of minor units in the currency (2 for PLN, 0 for JPY). */
export function currencyDigits(currency: string): number {
	try {
		return new Intl.NumberFormat('en', { style: 'currency', currency }).resolvedOptions()
			.maximumFractionDigits ?? 2;
	} catch {
		return 2;
	}
}

export function formatMoney(minor: number, currency: string): string {
	const digits = currencyDigits(currency);
	return new Intl.NumberFormat(i18n.locale, { style: 'currency', currency }).format(
		minor / 10 ** digits
	);
}

/** Plain amount for an input field: "120.33" -> "120,33" in pl/ru. */
export function moneyInput(minor: number | undefined | null, currency: string): string {
	if (minor === undefined || minor === null) return '';
	const digits = currencyDigits(currency);
	return formatNumber(minor / 10 ** digits, {
		minimumFractionDigits: minor % 10 ** digits === 0 ? 0 : digits,
		maximumFractionDigits: digits,
		useGrouping: false
	});
}

/**
 * Parses a decimal typed by a person: "12", "12,5", "12.50", "1 200,00".
 * Returns an integer scaled by 10^digits, or null if it is not a number.
 * Works on strings to avoid float rounding (0.1 + 0.2).
 */
export function parseDecimal(raw: string, digits: number): number | null {
	const v = raw.replace(/[\s ']/g, '').replace(',', '.');
	const m = /^(\d+)(?:\.(\d*))?$/.exec(v);
	if (!m) return null;
	const frac = m[2] ?? '';
	if (frac.length > digits) return null;
	const n = Number(m[1] + frac.padEnd(digits, '0'));
	return Number.isSafeInteger(n) ? n : null;
}

export function parseMoney(raw: string, currency: string): number | null {
	return parseDecimal(raw, currencyDigits(currency));
}

export function parseQuantity(raw: string): number | null {
	const n = parseDecimal(raw, 3);
	return n === null || n === 0 ? null : n;
}

export function formatQuantity(milli: number): string {
	return formatNumber(milli / 1000, { maximumFractionDigits: 3 });
}

/** "3.5 l", "2 pcs": a stock quantity with its unit. */
export function formatStock(milli: number, unit: PartUnit): string {
	return `${formatQuantity(milli)} ${t(`units.${unit}`)}`;
}

export function quantityInput(milli: number): string {
	return formatNumber(milli / 1000, { maximumFractionDigits: 3, useGrouping: false });
}

export function formatDistance(value: number, unit: string): string {
	return `${formatNumber(value)} ${unit}`;
}

interface VehicleLike {
	make: string | null;
	model: string | null;
	year?: number | null;
}

/** "Toyota Yaris 2008", or empty when nothing is known. */
export function vehicleName(v: VehicleLike): string {
	return [v.make, v.model, v.year].filter(Boolean).join(' ');
}

interface Identified {
	plate: string | null;
	vin: string | null;
	frame_number: string | null;
}

/** The identifier people recognize first: plate, else VIN, else frame. */
export function vehicleIdent(v: Identified): string {
	return v.plate ?? v.vin ?? v.frame_number ?? '';
}

/**
 * The 17-character VIN when the text looks like one (spaces and dashes
 * ignored, an "I" import-marker prefix dropped), otherwise null.
 */
export function asVin(text: string): string | null {
	let v = text.replace(/[\s-]/g, '').toUpperCase();
	if (v.length === 18 && v.startsWith('I')) v = v.slice(1);
	return /^[A-HJ-NPR-Z0-9]{17}$/.test(v) ? v : null;
}

/**
 * Which identifier field a search text most likely belongs to, so "Add
 * vehicle" after an empty search can prefill it. Text that fits none of
 * them (a scanned part barcode, an owner's name) prefills nothing rather
 * than landing in the wrong field.
 */
export function guessIdentifier(
	text: string
): { field: 'vin' | 'frame_number' | 'plate'; value: string } | null {
	const vin = asVin(text);
	if (vin) return { field: 'vin', value: vin };
	const v = text.trim().toUpperCase();
	// Japanese frame number: model code, dash, serial ("NHW10-0030520").
	if (/^[A-Z0-9]{2,10}-\d{4,8}$/.test(v.replace(/\s/g, ''))) {
		return { field: 'frame_number', value: v.replace(/\s/g, '') };
	}
	// Plates are short and mix letters with digits; long digit-only codes
	// are barcodes.
	const compact = v.replace(/[\s-]/g, '');
	if (/^[\p{L}\d]{2,10}$/u.test(compact) && /\p{L}/u.test(compact) && /\d/.test(compact)) {
		return { field: 'plate', value: v.replace(/\s+/g, ' ') };
	}
	return null;
}

/** Today in the device's time zone as YYYY-MM-DD, for date inputs. Not
 * `toISOString()`: that is the UTC date, a day off around midnight. */
export function localToday(): string {
	const d = new Date();
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
}
