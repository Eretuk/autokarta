// Own QR labels for parts without a usable barcode. The label carries a
// short random code that is stored as one of the part's barcodes, so
// scanning it goes through the normal barcode search.

/** Crockford base32: no I, L, O, U, nothing to misread when typed by hand. */
const ALPHABET = '0123456789ABCDEFGHJKMNPQRSTVWXYZ';
export const OWN_CODE_PREFIX = 'AK-';

/** "AK-7F3K9QZ2": 40 random bits, plenty for one workshop's shelves. */
export function newOwnCode(): string {
	const bytes = crypto.getRandomValues(new Uint8Array(8));
	// 256 is a multiple of 32, so every character is equally likely.
	return OWN_CODE_PREFIX + Array.from(bytes, (b) => ALPHABET[b % 32]).join('');
}

export function ownCode(barcodes: string[]): string | null {
	return barcodes.find((b) => b.startsWith(OWN_CODE_PREFIX)) ?? null;
}
