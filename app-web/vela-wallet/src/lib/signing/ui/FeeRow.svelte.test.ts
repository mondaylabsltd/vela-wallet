/**
 * PR 3 final note F12, in a real browser: the signing sheet's fee row holds
 * its line count while the fee's worth is on its way.
 *
 * The fee was one string. "0.0015 ETH · ≈…" fitted beside "Network fee" at
 * 320 px; "0.0015 ETH · ≈₫112,500.00" did not, the label wrapped, the row grew
 * a line, and the bottom-anchored sheet moved 18 px under the person. The row
 * draws the send form's two pieces now — the coin, and the money under it
 * while the display currency has not committed — and keeps that layout.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import '../../../app.css';
import type { FeeModel } from '../model';
import FeeRow from './FeeRow.svelte';

type Onchain = Extract<FeeModel, { kind: 'onchain' }>;

const BASE: Onchain = {
	kind: 'onchain',
	label: 'Network fee',
	value: '0.0015 ETH',
	tappable: false,
	refreshLabel: 'Refresh fee',
	refreshing: false,
	chevron: false
};

const ESTIMATING: Onchain = { ...BASE, value: 'Estimating…', valueFiatWithheld: true };
const WITHHELD: Onchain = { ...BASE, valueFiat: '≈ …', valueFiatWithheld: true };
const LONG: Onchain = { ...BASE, valueFiat: '≈ ₫112,500.00' };
const SHORT: Onchain = { ...BASE, valueFiat: '≈ $4.50' };

/** The sheet's content column at 320 px (24 px gutters), and around it. */
const WIDTHS = ['272px', '240px', '342px'] as const;

function mount(width: string, fee: Onchain, scale = '1') {
	const host = document.createElement('div');
	host.style.width = width;
	host.style.setProperty('--text-scale', scale);
	document.body.appendChild(host);
	const screen = render(FeeRow, { target: host, props: { fee } });
	const row = () => host.querySelector('.row') as HTMLElement;
	const box = (el: Element) => {
		const r = el.getBoundingClientRect();
		return [r.left, r.top, r.width, r.height].map((n) => Math.round(n * 10) / 10);
	};
	return {
		screen,
		host,
		row,
		// The label's cell takes the room the coin leaves, so its WIDTH is
		// not a place: where its words start, and how tall they stand, is.
		measure: () => ({
			card: box(host.querySelector('.line') as HTMLElement),
			label: box(row().querySelector('.label') as HTMLElement).filter((_, i) => i !== 2),
			coin: box(row().querySelectorAll('.value')[0])
		}),
		values: () => [...row().querySelectorAll<HTMLElement>('.value')],
		done: () => {
			screen.unmount();
			host.remove();
		}
	};
}

const oneLine = (el: HTMLElement) => {
	const probe = el.cloneNode(false) as HTMLElement;
	probe.textContent = 'X';
	probe.style.position = 'absolute';
	probe.style.whiteSpace = 'nowrap';
	el.parentElement?.appendChild(probe);
	const height = probe.getBoundingClientRect().height;
	probe.remove();
	return height;
};

describe('the signing fee row — the worth landing moves nothing (F12)', () => {
	it('withheld, the money stands under the coin; a long figure and a short one land in that line', async () => {
		for (const width of WIDTHS) {
			for (const scale of ['1', '1.35']) {
				const at = `${width} × ${scale}`;
				const view = mount(width, WITHHELD, scale);
				const before = view.measure();
				const [coin, money] = view.values().map((v) => v.getBoundingClientRect());
				expect(money.top, at).toBeGreaterThanOrEqual(coin.bottom - 0.5);
				// Both at the row's end.
				expect(Math.abs(money.right - coin.right), at).toBeLessThanOrEqual(0.5);
				expect(getComputedStyle(view.values()[1]).textAlign, at).toBe('end');
				const moneyTop = money.top;

				await view.screen.rerender({ fee: LONG });
				await tick();
				expect(view.measure(), `${at}: a long figure`).toEqual(before);
				expect(view.values()[1].textContent, at).toBe('≈ ₫112,500.00');
				expect(view.values()[1].getBoundingClientRect().top, at).toBe(moneyTop);
				// Whole, inside the row.
				expect(view.row().scrollWidth, at).toBeLessThanOrEqual(view.row().clientWidth);

				// …and it does not fold back up for a short one.
				await view.screen.rerender({ fee: SHORT });
				await tick();
				expect(view.measure(), `${at}: a short figure`).toEqual(before);
				view.done();
			}
		}
	});

	it('from "Estimating…" on: the fee landing, then its worth, on a line kept from the first frame', async () => {
		for (const width of WIDTHS) {
			const view = mount(width, ESTIMATING);
			const card = () => view.measure().card;
			const label = () => view.measure().label;
			const before = { card: card(), label: label() };
			// The money's line is there, empty, and says nothing to a reader.
			const kept = view.row().querySelector('.kept') as HTMLElement;
			expect(kept).not.toBeNull();
			expect(kept.getAttribute('aria-hidden')).toBe('true');
			expect(kept.textContent?.trim()).toBe('');

			await view.screen.rerender({ fee: WITHHELD });
			await tick();
			expect({ card: card(), label: label() }, `${width}: the fee lands`).toEqual(before);
			expect(view.row().querySelector('.kept'), width).toBeNull();

			await view.screen.rerender({ fee: LONG });
			await tick();
			expect({ card: card(), label: label() }, `${width}: its worth lands`).toEqual(before);
			view.done();
		}
	});

	it('the label is never broken to make room — the fee goes under it whole', async () => {
		// The display currency committed long before: nothing was withheld, and
		// a long figure meets a narrow row.
		const view = mount('240px', LONG);
		const label = view.row().querySelector('.label') as HTMLElement;
		expect(label.getBoundingClientRect().height).toBeLessThanOrEqual(oneLine(label) + 0.5);
		for (const value of view.values()) {
			expect(value.getBoundingClientRect().height).toBeLessThanOrEqual(oneLine(value) + 0.5);
			expect(value.getBoundingClientRect().right).toBeLessThanOrEqual(
				view.row().getBoundingClientRect().right + 0.5
			);
		}
		expect(view.row().scrollWidth).toBeLessThanOrEqual(view.row().clientWidth);
		view.done();
	});

	it('never withheld and room to spare: one line, the coin and its money side by side', async () => {
		const view = mount('342px', SHORT);
		const [coin, money] = view.values().map((v) => v.getBoundingClientRect());
		expect(Math.abs(money.top - coin.top)).toBeLessThanOrEqual(0.5);
		expect(money.left).toBeGreaterThanOrEqual(coin.right);
		// No "·": "≈" already joins a coin to its money.
		expect(view.row().textContent).not.toContain('·');
		expect(view.row().querySelector('.kept')).toBeNull();
		view.done();
	});

	it('with coins to choose from: the chevron beside the coin, and the money ends where the coin ends', async () => {
		const view = mount('272px', { ...WITHHELD, tappable: true, chevron: true });
		const textRight = (el: Element) => {
			const range = document.createRange();
			range.selectNodeContents(el);
			return range.getBoundingClientRect().right;
		};
		const [coin, money] = view.values();
		const chevron = view.row().querySelector('.chevron') as HTMLElement;
		// The chevron is on the coin's line, after it…
		expect(chevron.getBoundingClientRect().left).toBeGreaterThanOrEqual(textRight(coin));
		expect(chevron.getBoundingClientRect().top).toBeLessThan(coin.getBoundingClientRect().bottom);
		// …and the money under the coin ends at the coin's edge, not under the chevron.
		expect(Math.abs(textRight(money) - textRight(coin))).toBeLessThanOrEqual(0.5);
		const before = view.measure();
		await view.screen.rerender({ fee: { ...LONG, tappable: true, chevron: true } });
		await tick();
		expect(view.measure()).toEqual(before);
		expect(Math.abs(textRight(view.values()[1]) - textRight(view.values()[0]))).toBeLessThanOrEqual(
			0.5
		);
		view.done();
	});

	it('a fee nothing can price is the coin alone', async () => {
		const view = mount('272px', BASE);
		expect(view.values().map((v) => v.textContent?.trim())).toEqual(['0.0015 ETH']);
		view.done();
	});
});
