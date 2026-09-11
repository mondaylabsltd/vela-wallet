/**
 * Token-layer gates (research.md D6):
 *  1. Drift gate — committed tokens.css/tokens.ts byte-equal a fresh
 *     regeneration from docs/design-tokens.json.
 *  2. Literal audit — no hard-coded visual values outside the token layer
 *     (docs/design-system.md rule 1), with the documented whitelist.
 */
import { readFileSync, readdirSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import { describe, expect, it } from 'vitest';
import { generateCss, generateTs } from '../../../scripts/gen-tokens.mjs';
import { BREAKPOINT_CONTACTS_OVERLAY, BREAKPOINT_DESKTOP } from './tokens';

const APP_ROOT = join(import.meta.dirname, '..', '..', '..');

describe('drift gate', () => {
	it('tokens.css matches a fresh regeneration', () => {
		const committed = readFileSync(join(APP_ROOT, 'src/lib/tokens/tokens.css'), 'utf8');
		expect(committed).toBe(generateCss());
	});

	it('tokens.ts matches a fresh regeneration', () => {
		const committed = readFileSync(join(APP_ROOT, 'src/lib/tokens/tokens.ts'), 'utf8');
		expect(committed).toBe(generateTs());
	});

	it('light and dark define the same color paths (generator asserts, we re-check the emission)', () => {
		const css = generateCss();
		const darkVars = [...css.matchAll(/^\t(--color-[\w-]+):/gm)].map((m) => m[1]);
		const lightBlock = css.split('@media (prefers-color-scheme: light)')[1] ?? '';
		// Web additions declared once in :root, mode-independent by construction:
		// onAccent is a constant, the rail pair are color-mix over tokens that
		// already flip per mode.
		const MODE_INDEPENDENT = new Set([
			'--color-onAccent',
			'--color-rail-ordinal',
			'--color-rail-ordinalSoft'
		]);
		for (const name of darkVars) {
			if (MODE_INDEPENDENT.has(name)) continue;
			expect(lightBlock, name).toContain(`${name}:`);
		}
	});
});

describe('literal audit — product UI references tokens, never raw values', () => {
	/**
	 * BrandMark carries asset colors verbatim from the design SVGs. DemoPage is
	 * the other kind of exception: it draws a STAND-IN WEB PAGE inside the
	 * explore browser (spec 022), and a website's palette and type scale are
	 * not ours to express in our tokens — the day it becomes a real WebView,
	 * those values leave with it.
	 */
	const LITERAL_WHITELIST = new Set([
		'BrandMark.svelte',
		'AppIcon.svelte',
		// spec 028 Phase 9 (T488): the same brand asset as data, and the receive
		// share IMAGE — a render product composed as an SVG string with the app's
		// faces embedded (`@font-face` needs `font-family:`). Its surfaces still
		// read the live tokens at save time; only the asset's fills are values.
		'brand-mark.ts',
		'share-image.ts',
		'DemoPage.svelte',
		'chains.ts',
		// spec 026: the parallel-space badge is deliberately NOT product chrome.
		// Its violet exists to look foreign — a design token would make a test
		// wallet look native, which is the exact confusion the badge prevents.
		'ParallelSpaceBadge.svelte'
	]);

	const collect = (dir: string): string[] =>
		readdirSync(dir).flatMap((name) => {
			const path = join(dir, name);
			if (statSync(path).isDirectory()) return collect(path);
			return name.endsWith('.svelte') || name.endsWith('.css') || name.endsWith('.ts')
				? [path]
				: [];
		});

	const sources = [
		...collect(join(APP_ROOT, 'src/lib/ui')),
		// spec 018 T018: the contacts feature layer is audited too.
		...collect(join(APP_ROOT, 'src/lib/contacts')),
		// spec 021: and the wallet-flow layer, on the same terms.
		...collect(join(APP_ROOT, 'src/lib/flows')),
		// spec 022: so are explore and signing. Their `fixtures.ts` is exempt for
		// the same reason the wallet's is — a site's brand colour and a token's
		// chain colour are CONTENT, and a design token cannot name them.
		...collect(join(APP_ROOT, 'src/lib/explore')),
		...collect(join(APP_ROOT, 'src/lib/signing')),
		// spec 024 T010: the live-wiring layers. `core` and `services` hold no
		// visuals at all, which is exactly why they are audited — a colour
		// appearing there would be a category error, not a taste question.
		// settings and session were an unlisted gap since 023/019.
		...collect(join(APP_ROOT, 'src/lib/core')),
		...collect(join(APP_ROOT, 'src/lib/services')),
		...collect(join(APP_ROOT, 'src/lib/settings')),
		...collect(join(APP_ROOT, 'src/lib/session')),
		// spec 026 T203: the money layer and the dev harness. `dev` is audited
		// too — a fixture is allowed to look alien, but only on purpose and only
		// where it is written down (see the whitelist).
		...collect(join(APP_ROOT, 'src/lib/dev')),
		// spec 027 T303: the packaged-extension layer. It holds no visuals today
		// — which is the point of auditing it, since a colour appearing there
		// would be a category error. The extension's own page scripts are plain
		// `.js` and fall outside this collector entirely; that is acceptable only
		// while they draw nothing (recorded in results.md).
		...collect(join(APP_ROOT, 'src/lib/extension')),
		// spec 027 T303/T320: the dApp layer. The request window is the one place
		// a stranger's site gets to put words on this wallet's screen, so a colour
		// literal drifting in here would be exactly the wrong kind of surprise.
		...collect(join(APP_ROOT, 'src/lib/dapp')),
		...collect(join(APP_ROOT, 'src/routes')),
		join(APP_ROOT, 'src/app.css')
	].filter(
		(path) =>
			!path.includes('/tokens/') && !path.endsWith('.test.ts') && !path.endsWith('/fixtures.ts')
	);

	it('audits a non-trivial file set', () => {
		expect(sources.length).toBeGreaterThanOrEqual(10);
	});

	it('no hex colors outside the whitelist', () => {
		for (const path of sources) {
			if (LITERAL_WHITELIST.has(path.split('/').at(-1)!)) continue;
			const text = readFileSync(path, 'utf8');
			expect(text.match(/#[0-9a-fA-F]{3,8}\b/g), relative(APP_ROOT, path)).toBeNull();
		}
	});

	/**
	 * The only px literals allowed anywhere in product UI are the two responsive
	 * breakpoints, and only inside `@media` — a media query cannot read a custom
	 * property, so the value has to be spelled out. Both are generated tokens
	 * (`--breakpoint-desktop`, `--breakpoint-contactsOverlay`) and this gate
	 * pins the literals to those exports so the two can never drift apart.
	 */
	const BREAKPOINT_LITERALS = [`${BREAKPOINT_DESKTOP}px`, `${BREAKPOINT_CONTACTS_OVERLAY}px`];

	it('the only px literals are the breakpoints, and they equal the token exports', () => {
		for (const path of sources) {
			if (LITERAL_WHITELIST.has(path.split('/').at(-1)!)) continue;
			const text = readFileSync(path, 'utf8');
			const pxLiterals = text.match(/\b\d+(?:\.\d+)?px\b/g) ?? [];
			const offenders = pxLiterals.filter((v) => !BREAKPOINT_LITERALS.includes(v));
			expect(offenders, relative(APP_ROOT, path)).toEqual([]);
			// outside comments, breakpoint literals may appear only in media queries
			for (const line of text.split('\n')) {
				const hit = BREAKPOINT_LITERALS.some((v) => line.includes(v));
				if (hit && !/\/\*|\*\//.test(line)) {
					expect(line, relative(APP_ROOT, path)).toMatch(/@media/);
				}
			}
		}
	});

	it('no box-shadow or font-family literals (vars only)', () => {
		for (const path of sources) {
			if (LITERAL_WHITELIST.has(path.split('/').at(-1)!)) continue;
			// examine whole declarations — they may span lines (prettier wraps values)
			const declarations = readFileSync(path, 'utf8').split(';');
			for (const decl of declarations) {
				if (/box-shadow:/.test(decl)) expect(decl, path).toMatch(/var\(--/);
				if (/font-family:/.test(decl)) expect(decl, path).toMatch(/var\(--font/);
			}
		}
	});
});
