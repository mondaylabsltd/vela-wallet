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

const STAGES: ReceiptStage[] = ['submitting', 'submitted', 'confirmed', 'failed', 'not_sent'];

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

/**
 * PR 2 polish: "Not sent yet" — the relay turned the operation back because
 * the account's previous one still holds the nonce. Nothing failed, so the
 * disc is the waiting one: the clock, in the quiet colours, standing still —
 * never the exclamation, never the error's red.
 */
describe('StatusHero — not sent yet is no failure (PR 2 polish)', () => {
	const colourOf = (variable: string): string => {
		const probe = document.createElement('span');
		probe.style.color = `var(${variable})`;
		document.body.appendChild(probe);
		const colour = getComputedStyle(probe).color;
		probe.remove();
		return colour;
	};

	it('draws the clock, in the waiting colours, and does not breathe', () => {
		expect(drawn('not_sent')).toEqual(strokes(UTILITY_ICONS.clock));
		const screen = render(StatusHero, {
			props: { stage: 'not_sent', title: 'Not sent yet', captions: ['Try again once it’s done.'] }
		});
		const disc = screen.container.querySelector('.disc') as HTMLElement;
		expect(disc.classList.contains('failed')).toBe(false);
		const style = getComputedStyle(disc);
		expect(style.color).toBe(colourOf('--color-fg-muted'));
		expect(style.color).not.toBe(colourOf('--color-error-base'));
		expect(style.animationName).toBe('none');
		// Nothing is on its way: no ring.
		expect(disc.querySelector('.ring')).toBeNull();
	});
});
