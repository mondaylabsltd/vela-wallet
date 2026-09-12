/**
 * The receive code — a code that is DATA, not decoration (spec 028 T410/T411).
 *
 * Spec 021 drew the receive card before anything could fill it, and
 * `qr-pattern.ts` said so honestly in its own header: *"It never encodes data…
 * a pattern that looked scannable but wasn't would be worse than one that
 * plainly isn't."* Then 025 graduated that card onto a live receive screen and
 * nobody replaced the generator, so an honest placeholder became a dishonest
 * product: a person shows the code to a friend and no money arrives.
 *
 * This is the generator that replaces it on every live surface. `qr-pattern.ts`
 * stays exactly where a placeholder is what is meant — the galleries are canon
 * and their screenshots are diffed, and a fixture that suddenly encoded a real
 * address would put real addresses in the gallery.
 *
 * The encoder is the `qrcode` package, which is what the Expo app ships
 * (spec 028 D44, corrected: the 554-line hand-rolled module in the Expo tree is
 * imported by nothing, so porting it would have been porting code that never
 * ran). `buildQrPath` is ported from `src/components/qr-path.ts` @ 28d25ae9.
 */
import QRCode from 'qrcode';

/** A code ready to draw: the module count, and one SVG path over its darks. */
export interface QrCode {
	/** Modules per side. A plain address is 29; a payment link can reach 49. */
	modules: number;
	/** One path in MODULE units — the viewBox is `0 0 modules modules`. */
	path: string;
}

/**
 * Encode `text` at error-correction level M, the level the Expo app uses and
 * the level the drawn card's geometry was chosen against.
 *
 * Throws only if the text cannot be encoded at all, which for an address or a
 * payment link means the caller built something malformed — a caller should
 * not paper over that with a placeholder, because a placeholder is exactly the
 * failure this module exists to end.
 */
export function encodeQr(text: string): QrCode {
	const code = QRCode.create(text, { errorCorrectionLevel: 'M' });
	const modules = code.modules.size;
	return { modules, path: buildQrPath(code.modules.data, modules) };
}

/**
 * One SVG path covering every dark module, with consecutive darks in a row
 * merged into a single `h` run.
 *
 * Ported from src/components/qr-path.ts @ 28d25ae9. Rendering each module as
 * its own rect leaves hairline white gridlines from pixel rounding, and a code
 * with gridlines photographs badly — which is the only way most people will
 * ever read it.
 */
export function buildQrPath(data: ArrayLike<number>, moduleCount: number): string {
	let d = '';
	for (let y = 0; y < moduleCount; y++) {
		for (let x = 0; x < moduleCount; x++) {
			if (data[y * moduleCount + x] !== 1) continue;
			let run = 1;
			while (x + run < moduleCount && data[y * moduleCount + x + run] === 1) run++;
			d += `M${x} ${y}h${run}v1h-${run}z`;
			x += run - 1;
		}
	}
	return d;
}
