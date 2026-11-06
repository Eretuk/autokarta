// Barcode / QR detection from a camera <video>.
//
// Uses the browser's native BarcodeDetector where it exists (Chrome on
// Android) and falls back to zxing-wasm elsewhere. The zxing .wasm file is
// self-hosted: by default zxing-wasm would download it from a CDN.

export type Engine = 'native' | 'zxing';

export interface Detection {
	text: string;
	/** Normalized to BarcodeDetector names: ean_13, code_128, qr_code, ... */
	format: string;
}

export interface Scanner {
	engine: Engine;
	detect(video: HTMLVideoElement): Promise<Detection[]>;
}

const WANTED = [
	'ean_13',
	'ean_8',
	'upc_a',
	'upc_e',
	'code_128',
	'code_39',
	'code_93',
	'itf',
	'codabar',
	'qr_code',
	'data_matrix'
];

const ZXING_FORMATS = [
	'EAN13',
	'EAN8',
	'UPCA',
	'UPCE',
	'Code128',
	'Code39',
	'Code93',
	'ITF',
	'Codabar',
	'QRCode',
	'DataMatrix'
] as const;

// zxing-wasm format name -> BarcodeDetector name.
const ZXING_TO_NATIVE: Record<(typeof ZXING_FORMATS)[number], string> & Record<string, string> = {
	EAN13: 'ean_13',
	EAN8: 'ean_8',
	UPCA: 'upc_a',
	UPCE: 'upc_e',
	Code128: 'code_128',
	Code39: 'code_39',
	Code93: 'code_93',
	ITF: 'itf',
	Codabar: 'codabar',
	QRCode: 'qr_code',
	DataMatrix: 'data_matrix'
};

/** Display names of symbologies are technical identifiers, not translated. */
const LABELS: Record<string, string> = {
	ean_13: 'EAN-13',
	ean_8: 'EAN-8',
	upc_a: 'UPC-A',
	upc_e: 'UPC-E',
	code_128: 'Code 128',
	code_39: 'Code 39',
	code_93: 'Code 93',
	itf: 'ITF',
	codabar: 'Codabar',
	qr_code: 'QR',
	data_matrix: 'Data Matrix'
};

export function formatLabel(format: string): string {
	return LABELS[format] ?? format;
}

interface NativeBarcode {
	rawValue: string;
	format: string;
}
interface NativeDetector {
	detect(source: CanvasImageSource): Promise<NativeBarcode[]>;
}
interface NativeDetectorCtor {
	new (options?: { formats: string[] }): NativeDetector;
	getSupportedFormats(): Promise<string[]>;
}

async function createNative(): Promise<Scanner | null> {
	const Ctor = (globalThis as unknown as { BarcodeDetector?: NativeDetectorCtor }).BarcodeDetector;
	if (!Ctor) return null;
	try {
		const supported = await Ctor.getSupportedFormats();
		const formats = WANTED.filter((f) => supported.includes(f));
		if (formats.length === 0) return null;
		const detector = new Ctor({ formats });
		return {
			engine: 'native',
			detect: async (video) =>
				(await detector.detect(video)).map((b) => ({ text: b.rawValue, format: b.format }))
		};
	} catch {
		return null;
	}
}

async function createZxing(): Promise<Scanner> {
	const [{ prepareZXingModule, readBarcodes }, { default: wasmUrl }] = await Promise.all([
		import('zxing-wasm/reader'),
		import('zxing-wasm/reader/zxing_reader.wasm?url')
	]);
	await prepareZXingModule({
		overrides: {
			locateFile: (path: string, prefix: string) => (path.endsWith('.wasm') ? wasmUrl : prefix + path)
		},
		fireImmediately: true
	});

	const canvas = document.createElement('canvas');
	const ctx = canvas.getContext('2d', { willReadFrequently: true });
	if (!ctx) throw new Error('canvas_unavailable');
	// Full-resolution frames are slow to decode in wasm; 1280px is plenty.
	const MAX_WIDTH = 1280;

	return {
		engine: 'zxing',
		detect: async (video) => {
			const w = video.videoWidth;
			const h = video.videoHeight;
			if (!w || !h) return [];
			const scale = Math.min(1, MAX_WIDTH / w);
			canvas.width = Math.round(w * scale);
			canvas.height = Math.round(h * scale);
			ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
			const image = ctx.getImageData(0, 0, canvas.width, canvas.height);
			const results = await readBarcodes(image, {
				formats: [...ZXING_FORMATS],
				tryHarder: true,
				maxNumberOfSymbols: 4
			});
			return results
				.filter((r) => r.isValid)
				.map((r) => ({ text: r.text, format: ZXING_TO_NATIVE[r.format] ?? r.format }));
		}
	};
}

export async function createScanner(): Promise<Scanner> {
	return (await createNative()) ?? (await createZxing());
}
