/**
 * WCAG AA contrast gate (spec SC-004), computed directly from the token source
 * so a palette change fails here before any screenshot exists.
 *
 * Thresholds per pair are the WCAG 2.x AA minima for how the page actually
 * uses the pair, at the size it is drawn. Large text (≥24px regular, or
 * ≥18.66px bold) is the only text held to 3:1; every label in the app is
 * smaller than that — a button's 17px/600 label included — so text pairs are
 * held to 4.5:1:
 *  - 4.5:1 for body/label text, button labels (17px) and small text (10–13px)
 *  - 3:1  for non-text UI (pager dots, icons, focus rings) and for fg.subtle,
 *    which is NOT a text colour: it draws marks whose meaning something else
 *    carries (the card index '01', a chevron, a ✕). Text that carries
 *    information — an address, a section heading, a label, a note — is
 *    fg.muted or stronger (r4D-11; the source scan at the end holds the
 *    screens that were fixed for it).
 *  - Neutral primary button (vis-6, owner 2026-10): every main action that
 *    moves no value is fg.inverse on fg.base, held to 4.5:1; its focus ring
 *    is fg.base around a bg.base gap, held to the 3:1 non-text minimum.
 *  - The value button (Button `value`) and the destructive one (`danger`):
 *    their white label on the web's fill tokens, color-accent-fill and
 *    color-error-fill (scripts/gen-tokens.mjs WEB_MODE_COLORS), 4.5:1 at rest
 *    AND under the pointer's hover dim. Until r4D-11 the value label sat on
 *    accent.base at 3.6:1 as a documented exception, and the dark danger
 *    label on error.base at 2.8:1 with no exception at all.
 */
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
import { describe, expect, it } from 'vitest';
import { COLORS, ON_ACCENT, WEB_COLORS } from './tokens';

function channel(v: number): number {
	const s = v / 255;
	return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

function rgb(hex: string): [number, number, number] {
	const m = hex.match(/^#([0-9a-fA-F]{6})$/);
	if (!m) throw new Error(`not a 6-digit hex color: ${hex}`);
	const n = parseInt(m[1], 16);
	return [(n >> 16) & 0xff, (n >> 8) & 0xff, n & 0xff];
}

function luminance(hex: string): number {
	const [r, g, b] = rgb(hex);
	return 0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);
}

function ratio(a: string, b: string): number {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

/** `fill` drawn at `alpha` over `ground`: what a hover dim (opacity) shows. */
function over(fill: string, ground: string, alpha: number): string {
	const [f, g] = [rgb(fill), rgb(ground)];
	const mix = f.map((c, i) => Math.round(alpha * c + (1 - alpha) * g[i]));
	return `#${mix.map((c) => c.toString(16).padStart(2, '0')).join('')}`;
}

/** The pointer hover dim on a button (`--opacity-hover`, tokens.css). */
const HOVER = 0.92;

/** [foreground, background, minimum, usage] */
const PAIRS: Array<[string, string, number, string]> = [
	['color.fg.base', 'color.bg.base', 4.5, 'headings/body on page background'],
	['color.fg.base', 'color.bg.raised', 4.5, 'card titles on raised cards'],
	['color.fg.muted', 'color.bg.base', 4.5, 'tagline + quiet link on page background'],
	['color.fg.muted', 'color.bg.raised', 4.5, 'card descriptions + secondary CTA label'],
	// r4D-11: the 10–11px text that carries information — a header address,
	// an Activity day heading, a sheet's section heading, a fact's label, the
	// custom-service note — on every surface it is drawn on.
	['color.fg.muted', 'color.bg.sunken', 4.5, 'addresses + labels on a sunken row (10–11px)'],
	// r4D-11 (round 6): the review's source note, 11px on the phone's sheet
	// (bg.base, #fafaf8 — it read 3.38:1 in fg.subtle) and the desktop dialog.
	['color.fg.muted', 'color.bg.base', 4.5, 'a review’s source note on the sheet (11px)'],
	['color.fg.muted', 'color.bg.raised', 4.5, 'a review’s source note in the dialog (11px)'],
	['color.fg.subtle', 'color.bg.raised', 3, 'card index number (see header note)'],
	['color.fg.subtle', 'color.bg.base', 3, 'inactive pager dots (non-text UI)'],
	['color.fg.base', 'color.bg.base', 3, 'active pager dot, checks, selected marks, focus ring'],
	['color.fg.base', 'color.bg.raised', 3, 'checks and selected marks on raised sheets'],
	['color.fg.muted', 'color.bg.base', 3, 'a text field’s focus edge (app.css [data-field])'],
	['color.fg.muted', 'color.bg.raised', 3, 'a text field’s focus edge in a sheet'],
	['color.fg.inverse', 'color.fg.base', 4.5, 'neutral primary button label on its fill'],
	// Technical details' raw data (`RawData`), drawn on the sunken panel: its
	// values and hidden-character markers, its keys and notes, and its
	// punctuation, whose meaning the indented layout also carries.
	['color.fg.base', 'color.bg.sunken', 4.5, 'raw data values and markers on the sunken panel'],
	['color.fg.muted', 'color.bg.sunken', 4.5, 'raw data keys and notes on the sunken panel'],
	['color.fg.subtle', 'color.bg.sunken', 3, 'raw data punctuation (see header note)'],
	['color.accent.base', 'color.bg.base', 3, 'accent marks (rail rule) on the page background']
];

type Fill = 'color-accent-fill' | 'color-error-fill';

/** [foreground, web fill token, usage]: a white label on a filled button, 17px/600. */
const LABELS_ON_FILLS: Array<[string, Fill, string]> = [
	[ON_ACCENT, 'color-accent-fill', 'value button: Confirm & Send, Approve, Execute (17px/600)'],
	[ON_ACCENT, 'color-error-fill', 'danger button: Remove, Reject, Sign out (17px/600)']
];

/** [background, usage]: success said in small text (color-success-text). */
const SUCCESS_TEXT: Array<[string, string]> = [
	['color.bg.base', 'an incoming amount, “Approved” on the page (11–15px)'],
	['color.bg.raised', '“Approved” in a sheet (11px)'],
	['color.bg.sunken', 'a fact’s success value on a sunken row'],
	['color.success.soft', '“Sent” on its soft panel']
];

describe('WCAG AA over the exact token values, both modes', () => {
	for (const mode of ['light', 'dark'] as const) {
		const table: Record<string, string> = COLORS[mode];
		const web: Record<string, string> = WEB_COLORS[mode];
		for (const [fg, bg, min, usage] of PAIRS) {
			it(`${mode}: ${fg} on ${bg} ≥ ${min} (${usage})`, () => {
				expect(ratio(table[fg], table[bg])).toBeGreaterThanOrEqual(min);
			});
		}

		for (const [fg, fill, usage] of LABELS_ON_FILLS) {
			it(`${mode}: white on ${fill} ≥ 4.5 at rest and hovered (${usage})`, () => {
				expect(ratio(fg, web[fill])).toBeGreaterThanOrEqual(4.5);
				for (const ground of ['color.bg.base', 'color.bg.raised']) {
					expect(ratio(fg, over(web[fill], table[ground], HOVER))).toBeGreaterThanOrEqual(4.5);
				}
			});
			it(`${mode}: ${fill} stands off the page ≥ 3 (non-text: the button's edge)`, () => {
				expect(ratio(web[fill], table['color.bg.base'])).toBeGreaterThanOrEqual(3);
			});
		}

		for (const [bg, usage] of SUCCESS_TEXT) {
			it(`${mode}: color-success-text on ${bg} ≥ 4.5 (${usage})`, () => {
				expect(ratio(web['color-success-text'], table[bg])).toBeGreaterThanOrEqual(4.5);
			});
		}
	}

	it('the accent fill is the accent’s own hue, one step darker (the brand, not a new colour)', () => {
		const hue = (hex: string) => {
			const [r, g, b] = rgb(hex).map((c) => c / 255);
			const max = Math.max(r, g, b);
			const d = max - Math.min(r, g, b);
			const h = max === r ? ((g - b) / d) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
			return (h * 60 + 360) % 360;
		};
		for (const mode of ['light', 'dark'] as const) {
			const fill = WEB_COLORS[mode]['color-accent-fill'];
			const accent = COLORS[mode]['color.accent.base'];
			expect(Math.abs(hue(fill) - hue(accent))).toBeLessThan(3);
			expect(luminance(fill)).toBeLessThan(luminance(accent));
		}
	});
});

/**
 * r4D-11: the screens the round-4 audit measured draw no text in fg.subtle.
 * A source scan, because a token pair cannot see which element wears it: in
 * these files fg.subtle may only colour a glyph (a ✕, a chevron, a dot) — a
 * rule that names one of those — never a text block.
 */
describe('information never drawn in fg.subtle on the multisig screens (r4D-11)', () => {
	const ROOT = join(__dirname, '..', '..');
	const FILES = [
		'lib/wallet/ui/WalletHeader.svelte',
		'lib/wallet/WalletHome.svelte',
		'lib/wallet/WalletDesktop.svelte',
		'lib/wallet/ui/Sidebar.svelte',
		'lib/flows/screens/SendConfirm.svelte',
		'lib/wallet/ui/ActivityRow.svelte',
		'lib/wallet/ui/BalanceDisplay.svelte',
		'lib/settings/ui/AccountRow.svelte',
		'lib/settings/ui/AccountsSheetBody.svelte',
		'lib/settings/ui/SectionLabel.svelte',
		'lib/flows/ui/FactRow.svelte',
		'lib/flows/ui/RecipientField.svelte',
		'lib/multisig/ui/AddMultisigSheet.svelte',
		'lib/multisig/ui/CreateMultisigSheet.svelte',
		'lib/multisig/ui/CustomServiceNote.svelte',
		'lib/multisig/ui/MultisigRows.svelte',
		'lib/multisig/ui/MultisigSettings.svelte',
		'lib/multisig/ui/ProposalSheet.svelte',
		// The round-4 merge: the other branches' new multisig surfaces, held to the same rule.
		'lib/flows/ui/ProvenanceNote.svelte',
		'lib/multisig/external/TxReview.svelte',
		'lib/multisig/ui/AuthorityNotice.svelte',
		'lib/multisig/ui/ExternalSignSheet.svelte',
		'lib/multisig/ui/InviteSheet.svelte',
		'lib/multisig/ui/MemberChanges.svelte',
		'lib/multisig/ui/MultisigHomeCard.svelte',
		'lib/multisig/ui/NetworkChoice.svelte',
		'lib/multisig/ui/RiskConfirmSheet.svelte',
		'routes/[locale]/connect/+page.svelte',
		// r4D-11 (round 6): the one review's source note ("Its code, … read
		// before and after") is the signing sheet's line, 11px on the sheet.
		'lib/signing/ui/SourceLine.svelte'
	];
	/** Rules that colour a glyph or a placeholder, not information. */
	const GLYPH =
		/(^|[\s.,>])(\.remove|\.chevron|\.chev|\.toggle|\.decimals|\.dot|\.live-dot|button)|::placeholder/;

	for (const file of FILES) {
		it(file, () => {
			const css = readFileSync(join(ROOT, file), 'utf8').split('<style>')[1] ?? '';
			const offenders: string[] = [];
			for (const [, selector, body] of css.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
				if (!/(^|[^-])color:\s*var\(--color-fg-subtle\)/.test(body)) continue;
				const sel = selector.replace(/\/\*[\s\S]*?\*\//g, '').trim();
				if (!GLYPH.test(sel)) offenders.push(sel);
			}
			expect(offenders).toEqual([]);
		});
	}
});
