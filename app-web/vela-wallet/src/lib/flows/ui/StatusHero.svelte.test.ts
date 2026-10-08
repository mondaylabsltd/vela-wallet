/**
 * The receipt's disc, in a browser — issue 460.
 *
 * A failed send, a failed dApp receipt and the signing sheet's failed status
 * all draw this disc. It wore the ✕, which is the sheet's close: a red one in
 * a status disc reads as a dead close button. The other three apps draw '!'
 * (Android VelaIcons.Exclamation, iOS LucideGlyph.exclamation, desktop
 * Icon::Exclamation), and so does the web now — no stage draws the ✕.
 */
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { UTILITY_ICONS } from '$lib/wallet/icons';
import type { ReceiptStage } from '../model';
import StatusHero from './StatusHero.svelte';

const STAGES: ReceiptStage[] = ['submitting', 'submitted', 'confirmed', 'failed'];

const strokes = (def: (typeof UTILITY_ICONS)[keyof typeof UTILITY_ICONS]): string[] =>
	def.style === 'fill'
		? def.paths
		: def.elements.flatMap((el) => (el.tag === 'path' ? [el.d] : []));

const drawn = (stage: ReceiptStage): string[] => {
	const screen = render(StatusHero, {
		props: { stage, title: 'Title', captions: ['Caption'] }
	});
	const disc = screen.container.querySelector('.disc');
	expect(disc).not.toBeNull();
	return [...disc!.querySelectorAll('path')].map((path) => path.getAttribute('d') ?? '');
};

describe('StatusHero — the failure mark (issue 460)', () => {
	it('a failure wears the exclamation', () => {
		expect(drawn('failed')).toEqual(strokes(UTILITY_ICONS.exclamation));
	});

	it('no stage draws the close glyph', () => {
		const close = strokes(UTILITY_ICONS.x);
		for (const stage of STAGES) {
			const paths = drawn(stage);
			for (const d of close) expect(paths, stage).not.toContain(d);
		}
	});
});
