/**
 * WCAG AA contrast gate (spec SC-004), computed directly from the token source
 * so a palette change fails here before any screenshot exists.
 *
 * Thresholds per pair are the WCAG 2.x AA minima for how the page actually
 * uses the pair:
 *  - 4.5:1 for normal-size body/label text
 *  - 3:1  for non-text UI (pager dots) and for fg.subtle, used only for the
 *    card index ('01'), whose meaning is duplicated by card order and the
 *    pager — decorative-adjacent, but held to the UI-component minimum.
 *  - Neutral primary button (vis-6, owner 2026-10): every main action that
 *    moves no value is fg.inverse on fg.base, held to 4.5:1; its focus ring
 *    is fg.base around a bg.base gap, held to the 3:1 non-text minimum.
 *  - Value CTA label (the one accent button, Button `value`):
 *    **documented sub-AA exception.** White on accent is ~3.6:1;
 *    at 17px/600 the label does NOT qualify as WCAG large-scale text
 *    (≥24px regular or ≥18.66px bold), so strict SC 1.4.3 wants 4.5:1. The
 *    RN app ships the same brand treatment; options (darken accent for CTA /
 *    enlarge+embolden label / accept) are a founder decision recorded in the
 *    delivery report. The 3:1 floor here only guards against regressing
 *    below even the large-text minimum.
 */
import { describe, expect, it } from 'vitest';
import { COLORS, ON_ACCENT } from './tokens';

function channel(v: number): number {
	const s = v / 255;
	return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4;
}

function luminance(hex: string): number {
	const m = hex.match(/^#([0-9a-fA-F]{6})$/);
	if (!m) throw new Error(`not a 6-digit hex color: ${hex}`);
	const n = parseInt(m[1], 16);
	return (
		0.2126 * channel((n >> 16) & 0xff) +
		0.7152 * channel((n >> 8) & 0xff) +
		0.0722 * channel(n & 0xff)
	);
}

function ratio(a: string, b: string): number {
	const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

/** [foreground, background, minimum, usage] */
const PAIRS: Array<[string, string, number, string]> = [
	['color.fg.base', 'color.bg.base', 4.5, 'headings/body on page background'],
	['color.fg.base', 'color.bg.raised', 4.5, 'card titles on raised cards'],
	['color.fg.muted', 'color.bg.base', 4.5, 'tagline + quiet link on page background'],
	['color.fg.muted', 'color.bg.raised', 4.5, 'card descriptions + secondary CTA label'],
	['color.fg.subtle', 'color.bg.raised', 3, 'card index number (see header note)'],
	['color.fg.subtle', 'color.bg.base', 3, 'inactive pager dots (non-text UI)'],
	['color.fg.base', 'color.bg.base', 3, 'active pager dot, checks, selected marks, focus ring'],
	['color.fg.base', 'color.bg.raised', 3, 'checks and selected marks on raised sheets'],
	['color.fg.inverse', 'color.fg.base', 4.5, 'neutral primary button label on its fill'],
	['color.accent.base', 'color.bg.base', 3, 'value button fill against page background'],
	['color.accent.base', 'color.bg.raised', 3, 'value button fill against a raised sheet']
];

describe('WCAG AA over the exact token values, both modes', () => {
	for (const mode of ['light', 'dark'] as const) {
		const table: Record<string, string> = COLORS[mode];
		for (const [fg, bg, min, usage] of PAIRS) {
			it(`${mode}: ${fg} on ${bg} ≥ ${min} (${usage})`, () => {
				expect(ratio(table[fg], table[bg])).toBeGreaterThanOrEqual(min);
			});
		}

		it(`${mode}: value CTA label on accent ≥ 3 (documented sub-AA exception, see header)`, () => {
			expect(ratio(ON_ACCENT, table['color.accent.base'])).toBeGreaterThanOrEqual(3);
		});
	}
});
