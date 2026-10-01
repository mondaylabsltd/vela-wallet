/**
 * The signing amount fits the 360 px side panel (spec 082 T215, G60, SC-008).
 *
 * EX-W3: a USDC transfer of 10^30 drew `-1,000,000,000,000,0…` running off
 * the panel's right edge — the one number a person is asked to check before
 * sliding was cut. A long amount now steps its size down to a floor and
 * wraps at its digit groups; it never runs past the sheet's padding, and
 * nothing of it is dropped. An everyday amount is drawn as before.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import AmountHero from './AmountHero.svelte';
import type { AmountLine } from '../model';

/** The side panel's width, less the sheet's own padding either side. */
const PANEL_PX = 360;
const PAD_PX = 24;

let host: HTMLDivElement | null = null;

afterEach(() => {
	host?.remove();
	host = null;
});

async function drawn(line: AmountLine, extra: { compact?: boolean; card?: boolean } = {}) {
	host = document.createElement('div');
	host.style.cssText = `width:${PANEL_PX}px;padding:0 ${PAD_PX}px;box-sizing:border-box;`;
	document.body.appendChild(host);
	render(AmountHero, { target: host, props: { line, ...extra } });
	await tick();
	await document.fonts.ready;
	const number = host.querySelector<HTMLElement>('.number')!;
	const value = host.querySelector<HTMLElement>('.value')!;
	const box = host.getBoundingClientRect();
	return {
		number,
		value,
		right: box.right - PAD_PX,
		left: box.left + PAD_PX,
		fontPx: parseFloat(getComputedStyle(number).fontSize),
		/** How many lines the figure's text is laid out on (a flex item is one box). */
		lines: () => {
			const range = document.createRange();
			range.selectNodeContents(number);
			const rects = [...range.getClientRects()].filter((r) => r.width > 0);
			return new Set(rects.map((r) => Math.round(r.top))).size;
		}
	};
}

const HUGE: AmountLine = {
	sign: '−',
	value: '1,000,000,000,000,000,000,000,000,000,000',
	symbol: 'USDC',
	tone: 'danger',
	fiat: '≈ $1,000,000,000,000,000,000,000,000,000,000.00'
};

describe('a long amount at 360 px (G60)', () => {
	it('never runs past the sheet’s padding, and nothing of it is dropped', async () => {
		const view = await drawn(HUGE);
		for (const rect of view.number.getClientRects()) {
			expect(rect.right).toBeLessThanOrEqual(view.right + 0.5);
			expect(rect.left).toBeGreaterThanOrEqual(view.left - 0.5);
		}
		expect(view.value.scrollWidth).toBeLessThanOrEqual(view.value.clientWidth + 1);
		expect(view.number.textContent).toBe(`${HUGE.sign}${HUGE.value}`);
	});

	it('steps down to the floor size and wraps at its digit groups', async () => {
		const view = await drawn(HUGE);
		expect(view.fontPx).toBeLessThan(32);
		expect(view.lines()).toBeGreaterThan(1);
		// Each line ends at a group: the break opportunities follow the separators.
		const breaks = [...view.number.querySelectorAll('wbr')].map(
			(wbr) => wbr.previousSibling?.textContent?.slice(-1) ?? ''
		);
		expect(breaks.length).toBeGreaterThan(0);
		for (const last of breaks) expect(last).toBe(',');
	});

	it('fits compact (swap) and card lines too', async () => {
		for (const extra of [{ compact: true }, { card: true }]) {
			const view = await drawn(HUGE, extra);
			for (const rect of view.number.getClientRects()) {
				expect(rect.right).toBeLessThanOrEqual(view.right + 0.5);
			}
			host?.remove();
		}
	});

	it('an everyday amount keeps the full hero size, on one line', async () => {
		const view = await drawn({ sign: '−', value: '0.001', symbol: 'xDAI', tone: 'neutral' });
		expect(view.fontPx).toBe(32);
		expect(view.lines()).toBe(1);
	});
});
