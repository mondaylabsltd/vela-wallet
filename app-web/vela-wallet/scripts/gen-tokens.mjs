#!/usr/bin/env node
/**
 * Design-token pipeline (spec 006-web-onboarding, contracts/tokens.md).
 *
 * Source  <-  docs/design-tokens.json        Penpot DTCG export, THE value authority
 * Output  ->  src/lib/tokens/tokens.css      :root dark base + light overrides
 * Output  ->  src/lib/tokens/tokens.ts       constants components/tests need in JS
 *
 * Both outputs are COMMITTED; `--check` fails when they drift from the export,
 * and the vitest drift gate re-runs the pure generators for the same guarantee.
 *
 * Web additions (tokens the export lacks) live in WEB_ADDITIONS below with the
 * docs/design-system.md rule that licenses each; nothing else may invent a value.
 */

import { readFileSync, writeFileSync, mkdirSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP_ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const SOURCE = join(APP_ROOT, '..', '..', 'docs', 'design-tokens.json');
const OUT_CSS = join(APP_ROOT, 'src', 'lib', 'tokens', 'tokens.css');
const OUT_TS = join(APP_ROOT, 'src', 'lib', 'tokens', 'tokens.ts');

/** docs/design-system.md §Layout names these; the DTCG export does not carry them. */
const WEB_ADDITIONS = [
	['size-control-sm', '36px', 'sizing.control.sm per docs/design-system.md'],
	['size-control-md', '44px', 'sizing.control.md per docs/design-system.md'],
	['size-control-lg', '52px', 'sizing.control.lg per docs/design-system.md'],
	['breakpoint-desktop', '1280px', 'feature 006 responsive contract'],
	[
		'breakpoint-contactsOverlay',
		'1120px',
		'spec 018 desktop SPEC sheet: below this width the third column overlays instead of squeezing the list'
	],
	['motion-panel-in', '240ms', 'spec 018 FR-011: third-column open'],
	['motion-panel-out', '200ms', 'spec 018 FR-011: third-column close'],
	['motion-crossfade', '150ms', 'spec 018 FR-011: content swap + reduced-motion degrade'],
	['motion-hover', '120ms', 'spec 018 FR-011: desktop row hover raise'],
	['motion-bubble-in', '120ms', 'spec 018 FR-011: index-rail letter bubble fade-in'],
	['motion-bubble-out', '80ms', 'spec 018 FR-011: index-rail letter bubble fade-out'],
	[
		'size-identiconHero',
		'64px',
		'spec 018: contact-detail hero avatar, measured 64 in C2 (mobile)'
	],
	['size-identiconDetail', '48px', 'spec 018: contact-detail avatar, measured 48 in DC2 (desktop)'],
	[
		'size-identiconViewer',
		'160px',
		"spec 019: the identicon viewer's artwork — big enough to read as a picture rather than an avatar, and to fit beside a wrapped address on the narrowest phone"
	],
	[
		'text-hero',
		'46px',
		'spec 019: the v2 Welcome headline. The DTCG type scale tops out at 40px (text-5xl), and the onboarding design specifies 46/38 — declared once here rather than sprinkled as literals'
	],
	['text-heroCompact', '38px', 'spec 019: the v2 Welcome headline below the desktop breakpoint'],
	[
		'text-heroTight',
		'31px',
		"spec 019 (2026-08-25): the third rung of the hero ladder, for locales whose headline is too wide for the second. 46/38/31 steps by ~0.82 each; measured at the shipped font the widest authored line runs 6.9em (zh) to 15.0em (id), and 31px is what fits ru's 10.88em in a 390pt phone's 342px column. 390 is the contract, not the floor: narrower frames are allowed to wrap (founder direction 2026-08-26)"
	],
	[
		'layout-onboardingRail',
		'320px',
		'spec 019: the onboarding rail, at and above the desktop breakpoint only. Below it the page keeps its single column — a rail is an answer to spare width, and a phone has none'
	],
	[
		'layout-onboardingColumn',
		'520px',
		'spec 019: the screen column beside the rail. Content ends where it ends there; the mobile layout anchors its CTA to the bottom of the viewport, which is a phone pattern and stays on phones'
	],
	[
		'layout-frameMax',
		'1920px',
		'spec 038 T078: the widest the signed-in frame and the onboarding frame grow before centring on the page ground. The mocks were drawn at 1440 and every column in them is left-anchored; on a 4 K display that left three quarters of the screen empty. 1920 keeps the composition the mocks drew and lets it sit in the middle of anything wider'
	],
	[
		'layout-onboardingFrameGutter',
		'72px',
		"spec 019: the desktop flow frame's horizontal breathing room — the column plus twice this is the frame's max width"
	],
	[
		'text-stepOrdinal',
		'104px',
		'spec 019: the rail step ordinal, set as display TYPE rather than drawn as a stepper widget'
	],
	[
		'layout-railRuleW',
		'28px',
		"spec 019: the accent rule under the rail's tagline — a mark, not a divider, so it has a width of its own"
	],
	[
		'layout-railDetailMeasure',
		'214px',
		"spec 019: the step detail's measure inside the rail — wraps at a readable line, not at the rail's edge"
	],
	[
		'space-railOrdinalGap',
		'6px',
		'spec 019: between the rail ordinal and its /03 — tighter than the space scale steps, per the design'
	],
	[
		'space-railNameGap',
		'22px',
		'spec 019: between the rail ordinal block and the step name, per the design'
	],
	[
		'layout-welcomeCtaMin',
		'176px',
		"spec 019: the desktop welcome CTAs' minimum width — side-by-side buttons sized to their labels, not to the column"
	],
	['text-stepTotal', '20px', 'spec 019: the /03 beside the ordinal, and the step name under it'],
	[
		'text-railTagline',
		'26px',
		'spec 019: the rail line outside the journey. 26, not the design 30 — the string cannot carry the design hard break (Android and iOS render the same key on one line), and 26 fits the CJK taglines in the rail measure whole'
	],
	[
		'color-rail-ordinal',
		'color-mix(in srgb, var(--color-fg-base) 14%, var(--color-bg-sunken))',
		'spec 019: the ordinal is a WATERMARK on the rail surface. Mixed rather than declared per mode because WEB_ADDITIONS land in :root only — and a mix off the two tokens that already flip is the more honest statement anyway: one step off the background, whichever background that is'
	],
	[
		'color-rail-ordinalSoft',
		'color-mix(in srgb, var(--color-fg-base) 22%, var(--color-bg-sunken))',
		'spec 019: the /03, a step further up from the background than the ordinal'
	],
	[
		'layout-settingsNavW',
		'216px',
		"spec 023 desktop SPEC: the settings second-level nav column, measured 216 in DST1–DST8 (the app sidebar's 240 is a different column and keeps its own width)"
	],
	[
		'layout-settingsControlW',
		'280px',
		'spec 023: the desktop panel\u2019s right-hand control column — measured 280 across DST2/DST3, so a dropdown, a segmented control and a slider all end on the same line'
	],
	[
		'layout-rowMeasure',
		'560px',
		'issue 195: the widest a label\u2194value row grows on the desktop (an asset and its amount, a setting and its control). Rows are two-ended, so every pixel of column width past this lands as dead space BETWEEN the two things the eye has to connect \u2014 at the 800 content column a token sat 610px from its balance. 560 is the settings control column (280) doubled: the label gets as much room as the control and no more. NOT multiplied by --text-scale: larger text fills the row from both ends, and the gap is the thing being bounded'
	],
	[
		'layout-settingsDialogW',
		'520px',
		'spec 023: the centred desktop dialog (DST4b / DSR1), measured 520 — wide enough for a URL in the mono face without becoming a second page'
	],
	[
		'layout-promptCard',
		'440px',
		'spec 019: the centred prompt card past the desktop breakpoint (PromptSheet, SignOutSheet, IdenticonViewer). Spec 019 recorded declaring it and never did, so every prompt card read `max-width: var(--layout-promptCard)` as invalid and spanned the whole window (founder-found 2026-09-05). 440 holds a 42-character address in the mono face on one line with the card padding, and stays a card rather than a page'
	],
	[
		'layout-flowColumn',
		'440px',
		'spec 019: the v2 onboarding flow column. The design centres every step in one column of this width at every viewport; the Welcome hero is the only wider one'
	],
	[
		'layout-welcomeColumn',
		'620px',
		'spec 019: the v2 Welcome column, wider than the flow it starts'
	],
	[
		'text-introTitle',
		'28px',
		'spec 020: the intro slide headline. Measured 28 on the boards, and the DTCG scale steps 26 -> 32 straight past it'
	],
	[
		'text-introBody',
		'16px',
		"spec 020: the intro slide's supporting paragraph. Measured 16 — a step above the 15 the Welcome subtitle uses, because this copy is the slide's whole point rather than a line under a headline"
	],
	[
		'size-introArtW',
		'160px',
		'spec 020: the illustration box. One size for all three slides, so the artwork does not resize as the carousel pages'
	],
	['size-introArtH', '128px', 'spec 020: the illustration box, see size-introArtW'],
	['layout-introGapTitle', '64px', 'spec 020: headline to illustration, measured off the boards'],
	['layout-introGapBody', '72px', 'spec 020: illustration to body copy, measured off the boards'],
	[
		'layout-introBodyMeasure',
		'320px',
		"spec 020: the body's line length. Narrower than the content column — the boards wrap this copy well inside the 24px screen padding"
	],
	['size-introDot', '8px', 'spec 020: one page dot; the gap between two is the same 8'],
	[
		'size-qrCard',
		'344px',
		'spec 021: the receive QR card, measured 344x344 in R2. Fixed, NOT fluid — the SPEC sheet pins it at 1.35x text scale too, because a QR that shrinks with its caption stops scanning'
	],
	[
		'size-statusHero',
		'88px',
		'spec 021: the send-receipt status circle, measured 88 in SD4a/SD4c. One size for all four outcomes so the disc does not resize as the transaction moves through them'
	],
	[
		'size-chainBadge',
		'40px',
		'spec 021: the network-row chain badge, measured 40 in R1. Larger than the 32 token icon because this row IS the network, not a token that happens to be on one'
	],
	[
		'layout-shareCardW',
		'480px',
		'spec 021: the receive share card (R4) — a render product saved to the photo library, so its geometry is fixed rather than responsive'
	],
	['layout-shareCardH', '700px', 'spec 021: the receive share card, see layout-shareCardW'],
	// spec 022 (explore + signing), every value MEASURED off design/explore
	['size-siteTile', '56px', 'spec 022: favourites tile avatar, measured 56 in E2 (x33–88)'],
	['size-siteRow', '40px', 'spec 022: site-row avatar, measured 40 in E2 recent rows'],
	['size-searchField', '48px', 'spec 022: start-page search box, measured 48 in E2 (y116–163)'],
	['size-addressPill', '40px', 'spec 022: browsing address pill, measured 40 in E4'],
	['size-browserBar', '56px', 'spec 022: browsing toolbar row, measured 56 in E4'],
	['size-signingAvatar', '36px', 'spec 022: dApp avatar in the signing header, measured 36 in CS1'],
	['size-networkChip', '26px', 'spec 022: the network chip beside it, measured 26 in CS1'],
	['size-slideTrack', '56px', 'spec 022: slide-to-confirm track, measured 342x56 in CS1'],
	[
		'size-slideKnob',
		'48px',
		'spec 022: its knob; the 4px inset either side is what makes the travel W-56'
	],
	[
		'size-tabCount',
		'26px',
		'spec 022: the boxed tab count — the header chip in E2 and the toolbar box in E4 are the same square'
	],
	['size-desktopTabStrip', '36px', 'spec 022: desktop tab strip height, measured 36 in DE3'],
	['size-desktopTab', '32px', 'spec 022: one desktop tab inside it'],
	['size-desktopToolbar', '56px', 'spec 022: desktop browser toolbar, measured 56 in DE3'],
	['size-desktopControl', '32px', 'spec 022: its icon buttons and address field'],
	[
		'icon-tab',
		'28px',
		'078 round 2 (founder, 2026-09-26): the phone tab bar is icons only — the labels truncated in es/pt/de/it — so its glyph steps up from icon-xl (26) to 28, the size all four shells draw'
	],
	[
		'layout-tabBarHeight',
		'56px',
		'078 round 2: the icon-only phone tab bar, without the safe area — the bar lost its label line, so it is no longer the 86 dock the export names'
	],
	['layout-desktopTabW', '200px', 'spec 022: one desktop tab’s width, measured in DE3'],
	['layout-contactsRailW', '216px', 'spec 018 research D9: desktop group-rail width (DC1)'],
	['layout-contactsMenuW', '216px', 'spec 018 research D9: dropdown/context menu width (M1/M2)'],
	[
		'color-onAccent',
		'#FFFFFF',
		'CTA label on accent.base, white in BOTH modes per mocks (fg.inverse flips)'
	],
	['opacity-hover', '0.92', 'pointer hover feedback; no export token exists for hover']
];

/** Composite stacks: export families + docs/design-system.md CJK/system fallbacks. */
const FONT_UI = "'Plus Jakarta Sans', 'Noto Sans SC', system-ui, sans-serif";
const FONT_MONO = "'IBM Plex Mono', ui-monospace, SFMono-Regular, monospace";

export const BREAKPOINT_DESKTOP = 1280;

/** Spec 018: the desktop third column overlays the list below this width. */
export const BREAKPOINT_CONTACTS_OVERLAY = 1120;

// px-typed DTCG categories; everything else resolves via path rules below.
const PX_TYPES = new Set([
	'spacing',
	'sizing',
	'borderRadius',
	'fontSizes',
	'borderWidth',
	'letterSpacing'
]);

/** `number`-typed tokens that are durations and therefore emit as ms. */
const MS_PREFIXES = ['motion.duration.', 'motion.sheet.', 'motion.entrance.'];

function flatten(setObj, prefix = '') {
	const out = [];
	for (const [key, value] of Object.entries(setObj)) {
		if (key.startsWith('$')) continue;
		const path = prefix ? `${prefix}.${key}` : key;
		if (value && typeof value === 'object' && '$value' in value) {
			out.push({ path, type: value.$type, value: value.$value });
		} else if (value && typeof value === 'object') {
			out.push(...flatten(value, path));
		}
	}
	return out;
}

function cssValue({ path, type, value }) {
	if (type === 'color') return String(value);
	if (type === 'fontFamilies')
		return Array.isArray(value) ? `'${value.join("', '")}'` : `'${value}'`;
	if (type === 'shadow') {
		// "0 1 3 0 rgba(...)" -> "0 1px 3px 0 rgba(...)"
		const m = String(value).match(/^(-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (-?[\d.]+) (.+)$/);
		if (!m) throw new Error(`unparseable shadow: ${path} = ${value}`);
		const px = (n) => (Number(n) === 0 ? '0' : `${n}px`);
		return `${px(m[1])} ${px(m[2])} ${px(m[3])} ${px(m[4])} ${m[5]}`;
	}
	if (PX_TYPES.has(type)) return Number(value) === 0 ? '0' : `${value}px`;
	if (type === 'number' && MS_PREFIXES.some((p) => path.startsWith(p))) return `${value}ms`;
	return String(value); // fontWeights, opacity, remaining numbers
}

const varName = (path) => `--${path.replaceAll('.', '-')}`;

function loadTokens(sourcePath = SOURCE) {
	const doc = JSON.parse(readFileSync(sourcePath, 'utf8'));
	for (const set of ['core', 'color-light', 'color-dark']) {
		if (!doc[set]) throw new Error(`token source missing set "${set}"`);
	}
	const core = flatten(doc.core);
	const light = flatten(doc['color-light']);
	const dark = flatten(doc['color-dark']);
	const lightPaths = light.map((t) => t.path).join('\n');
	const darkPaths = dark.map((t) => t.path).join('\n');
	if (lightPaths !== darkPaths) {
		throw new Error('color-light and color-dark define different token paths');
	}
	return { core, light, dark };
}

const HEADER = `/* GENERATED by scripts/gen-tokens.mjs — do not edit. Source: docs/design-tokens.json */`;

export function generateCss(tokens = loadTokens()) {
	const { core, light, dark } = tokens;
	const decl = (t) => `\t${varName(t.path)}: ${cssValue(t)};`;
	const declsFor = (list) => list.map(decl).join('\n');
	const additions = WEB_ADDITIONS.map(
		([name, value, why]) => `\t--${name}: ${value}; /* web addition: ${why} */`
	).join('\n');

	return `${HEADER}

/* core set (mode-independent) + dark mode as base: the "default" design (W1)
   is dark, and browsers without prefers-color-scheme support get dark. */
:root {
${declsFor(core)}
\t--font-ui: ${FONT_UI}; /* web addition: docs/design-system.md CJK fallback */
\t--font-mono: ${FONT_MONO}; /* web addition: mono fallbacks */
${additions}
${declsFor(dark)}
}

/* mode-light via system preference; a future in-app theme setting flips
   data-theme instead and must win over the media query. */
@media (prefers-color-scheme: light) {
\t:root:not([data-theme='dark']) {
${declsFor(light).replaceAll('\t', '\t\t')}
\t}
}

:root[data-theme='light'] {
${declsFor(light)}
}

:root[data-theme='dark'] {
${declsFor(dark)}
}
`;
}

export function generateTs(tokens = loadTokens()) {
	const { light, dark } = tokens;
	const table = (list) => list.map((t) => `\t'${t.path}': '${String(t.value)}'`).join(',\n');
	return `${HEADER.replace('/*', '//').replace(' */', '')}

export const BREAKPOINT_DESKTOP = ${BREAKPOINT_DESKTOP};

/** Spec 018: below this width the desktop third column overlays the list. */
export const BREAKPOINT_CONTACTS_OVERLAY = ${BREAKPOINT_CONTACTS_OVERLAY};

export const CONTROL = { sm: 36, md: 44, lg: 52 } as const;

export const FONT_UI = ${JSON.stringify(FONT_UI)};

export const FONT_MONO = ${JSON.stringify(FONT_MONO)};

export const MOTION = { fast: 150, base: 250, slow: 400 } as const;

/** Web addition: CTA label color on accent surfaces, both modes (see tokens.css). */
export const ON_ACCENT = '#FFFFFF';

/** Raw per-mode color tables (path -> value) for the contrast gate. */
export const COLORS = {
\tlight: {
${table(light).replaceAll('\t', '\t\t')}
\t},
\tdark: {
${table(dark).replaceAll('\t', '\t\t')}
\t}
} as const;
`;
}

function main() {
	const check = process.argv.includes('--check');
	const css = generateCss();
	const ts = generateTs();
	if (check) {
		const readOr = (p) => {
			try {
				return readFileSync(p, 'utf8');
			} catch {
				return '';
			}
		};
		const drift = [readOr(OUT_CSS) !== css && OUT_CSS, readOr(OUT_TS) !== ts && OUT_TS].filter(
			Boolean
		);
		if (drift.length) {
			console.error(
				`tokens drift from docs/design-tokens.json — run \`pnpm gen:tokens\`:\n  ${drift.join('\n  ')}`
			);
			process.exit(1);
		}
		console.log('tokens in sync');
		return;
	}
	mkdirSync(dirname(OUT_CSS), { recursive: true });
	writeFileSync(OUT_CSS, css);
	writeFileSync(OUT_TS, ts);
	console.log(`wrote ${OUT_CSS}\nwrote ${OUT_TS}`);
}

if (process.argv[1] && fileURLToPath(import.meta.url) === process.argv[1]) {
	main();
}
