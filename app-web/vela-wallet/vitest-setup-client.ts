/**
 * The browser tests measure the faces people read in.
 *
 * The app's fonts and its global styles (the tokens, and `app.css` with the
 * body's line height) arrive through the root layout (`+layout.svelte`); a
 * component test mounts no
 * layout, so without this every measurement was taken in the runner's system
 * font — SF Pro on macOS, DejaVu and no CJK face at all on a Linux runner —
 * and a width or line-height pinned on one machine failed on the other.
 * Loaded here and awaited before any test, the same metrics hold everywhere.
 */
import '@fontsource/plus-jakarta-sans/400.css';
import '@fontsource/plus-jakarta-sans/500.css';
import '@fontsource/plus-jakarta-sans/600.css';
import '@fontsource/plus-jakarta-sans/700.css';
import '@fontsource/ibm-plex-mono/400.css';
import '@fontsource/ibm-plex-mono/500.css';
import './src/lib/tokens/tokens.css';
import './src/app.css';
import { beforeAll } from 'vitest';

// Every subset the suite draws in: Latin and Latin-extended (the European
// locales) and Vietnamese. The CJK face (`src/lib/fonts/cjk.css`) is NOT
// loaded: the app links it on the CJK locales' pages only, so everywhere else
// Cyrillic and CJK text is the system's face — here as in the app.
const LATIN = 'Aa0x×−—…éøßşğ ạờử';

beforeAll(async () => {
	await Promise.all([
		...['400', '500', '600', '700'].map((w) =>
			document.fonts.load(`${w} 16px "Plus Jakarta Sans"`, LATIN)
		),
		...['400', '500'].map((w) => document.fonts.load(`${w} 16px "IBM Plex Mono"`, LATIN))
	]);
	await document.fonts.ready;
});
