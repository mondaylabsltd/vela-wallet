/**
 * Build-time identicon rendering (spec 015, research.md D1).
 *
 * Same seam as `engine.server.ts`: the wasm module runs in Node during
 * prerender only, so the deployed Worker stays wasm-free. Web renders the
 * identicon as an inline `<svg>` — the same bytes the other platforms
 * rasterize through vela-core's `identicon-raster` feature.
 *
 * Seeds go through `identiconNormalizeSeed` (never a local `toLowerCase()` —
 * spec 003's drift rule), and invalid seeds fall back to the shared
 * placeholder artwork instead of throwing into the page build.
 */
import {
	identiconNormalizeSeed,
	identiconSvgCircular
} from '../../../../../rust/pkg-web/vela_core.js';
import '#lib/i18n/wasm-init.server.ts';
// One artwork for "unrenderable", shared with the runtime renderer
// (`identicon.ts`) through a module that imports neither of them.
import { IDENTICON_PLACEHOLDER_SVG } from './identicon-placeholder';

export { IDENTICON_PLACEHOLDER_SVG };

/** Circular identicon SVG for a raw seed; placeholder when unrenderable. */
export function identiconSvgFor(rawSeed: string): string {
	const seed = identiconNormalizeSeed(rawSeed);
	if (seed.length === 0) return IDENTICON_PLACEHOLDER_SVG;
	try {
		return identiconSvgCircular(seed);
	} catch {
		return IDENTICON_PLACEHOLDER_SVG;
	}
}
