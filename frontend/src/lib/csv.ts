// CSV for the accountant, built in the browser so headers are translated and
// numbers use the UI language's decimal separator. Semicolon-separated with
// a UTF-8 BOM: what Excel in Poland and Russia opens correctly by double-click.

import { formatNumber } from './i18n/index.svelte';
import { currencyDigits } from './format';

export type Cell = string | number | null | undefined;

function quote(cell: Cell): string {
	const v = cell === null || cell === undefined ? '' : String(cell);
	return /[";\r\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

/** A plain decimal without grouping, e.g. "1234,50" in pl/ru. */
export function csvDecimal(value: number, digits: number): string {
	return formatNumber(value, {
		minimumFractionDigits: digits,
		maximumFractionDigits: digits,
		useGrouping: false
	});
}

export function csvMoney(minor: number | null | undefined, currency: string): string {
	if (minor === null || minor === undefined) return '';
	const digits = currencyDigits(currency);
	return csvDecimal(minor / 10 ** digits, digits);
}

export function csvQuantity(milli: number): string {
	return formatNumber(milli / 1000, { maximumFractionDigits: 3, useGrouping: false });
}

export function downloadCsv(fileName: string, header: string[], rows: Cell[][]) {
	const text = [header, ...rows].map((r) => r.map(quote).join(';')).join('\r\n');
	const blob = new Blob(['﻿', text, '\r\n'], { type: 'text/csv;charset=utf-8' });
	const url = URL.createObjectURL(blob);
	const a = document.createElement('a');
	a.href = url;
	a.download = fileName;
	document.body.append(a);
	a.click();
	a.remove();
	setTimeout(() => URL.revokeObjectURL(url), 1000);
}
