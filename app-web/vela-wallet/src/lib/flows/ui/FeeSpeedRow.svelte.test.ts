/**
 * The speed control (spec 068), in a browser.
 *
 * A `.svelte.test.ts` because everything worth pinning here is rendering: what
 * is on screen before anybody touches it, what a tap emits, and whether the
 * figures beside the names are actually drawn. A node test of the view-model
 * cannot see any of it — which is the trap this repo keeps falling into: a
 * model test stays green over a component that never renders the options.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import { gasPriceRangeTexts } from '../gas-price';
import type { FeeSpeedModel } from '../model';
import FeeSpeedRow from './FeeSpeedRow.svelte';

const SPEED: FeeSpeedModel = {
	label: 'Speed',
	value: 'Fast',
	open: false,
	onceNote: 'For this transaction only',
	gasPriceLabel: 'Gas Bid',
	gasPriceLine: true,
	options: [
		{
			id: 'fast',
			label: 'Fast',
			detail: 'First to confirm, even when the network is busy',
			value: '0.0021 ETH',
			valueFiat: '≈ $6.30',
			gasPrice: '300 gwei',
			selected: true
		},
		{
			id: 'standard',
			label: 'Standard',
			detail: 'Balanced for everyday transfers',
			value: '0.0013 ETH',
			valueFiat: '≈ $3.90',
			gasPrice: '282 gwei',
			selected: false
		},
		{
			id: 'slow',
			label: 'Slow',
			detail: 'Lowest fee, if you can wait',
			value: '0.001 ETH',
			valueFiat: '≈ $3.00',
			gasPrice: '270 gwei',
			selected: false
		}
	]
};

const drawn = async (patch: Partial<typeof SPEED> = {}, onselect?: (id: string) => void) => {
	const screen = render(FeeSpeedRow, { props: { speed: { ...SPEED, ...patch }, onselect } });
	screen.container.style.width = '320px';
	await tick();
	return {
		summary: screen.container.querySelector('button.summary') as HTMLButtonElement,
		options: [...screen.container.querySelectorAll<HTMLButtonElement>('button.option')],
		once: screen.container.querySelector('.once') as HTMLElement | null
	};
};

/** The gas price a tier's row is showing — `undefined` when it shows none. */
const gasOf = (option: HTMLElement) => option.querySelector('.gas-value')?.textContent?.trim();

/**
 * One option of the picker, laid out whole: the range on one line, nothing
 * spilling, the money unwrapped, the description below it, the name unelided.
 */
const fitsOneLine = (option: HTMLElement) => {
	const name = option.querySelector('.name') as HTMLElement;
	const amounts = option.querySelector('.amounts') as HTMLElement;
	const gas = option.querySelector('.gas') as HTMLElement;
	const value = option.querySelector('.gas-value') as HTMLElement;
	const detail = option.querySelector('.detail') as HTMLElement;
	const line = parseFloat(getComputedStyle(value).lineHeight) || value.offsetHeight;
	// The money is on the name's line — the row did not wrap it under.
	expect(amounts.getBoundingClientRect().top).toBeLessThan(name.getBoundingClientRect().bottom);
	// One line: the range is not broken across two.
	expect(value.getBoundingClientRect().height, option.textContent ?? '').toBeLessThan(line * 1.5);
	expect(option.scrollWidth).toBeLessThanOrEqual(Math.ceil(option.getBoundingClientRect().width));
	expect(amounts.getBoundingClientRect().height).toBeLessThan(
		amounts.getBoundingClientRect().width
	);
	expect(detail.getBoundingClientRect().top).toBeGreaterThanOrEqual(
		gas.getBoundingClientRect().bottom
	);
	expect(detail.getBoundingClientRect().width).toBeGreaterThan(name.getBoundingClientRect().width);
	// And the name is still there to be read, whole — not elided.
	expect(name.getBoundingClientRect().width).toBeGreaterThan(0);
	expect(name.scrollWidth, name.textContent ?? '').toBeLessThanOrEqual(name.clientWidth);
};

describe('FeeSpeedRow', () => {
	// The whole point of "folded by default": most sends need no decision about
	// speed, and a picker sitting open asks everybody to make one.
	it('draws one line and nothing else until it is opened', async () => {
		const { summary, options, once } = await drawn();
		expect(summary.getAttribute('aria-expanded')).toBe('false');
		expect(options).toHaveLength(0);
		expect(once).toBeNull();
		// Folded, it still says which speed is in force — the answer somebody
		// is scanning for, beside the word that is only the question.
		expect(summary.textContent).toContain('Speed');
		expect(summary.textContent).toContain('Fast');
	});

	it('shows every option WITH its own fee once open, so the trade is visible', async () => {
		const { summary, options, once } = await drawn({ open: true });
		expect(summary.getAttribute('aria-expanded')).toBe('true');
		expect(options.map((o) => o.querySelector('.name')?.textContent)).toEqual([
			'Fast',
			'Standard',
			'Slow'
		]);
		for (const [i, option] of options.entries()) {
			const values = [...option.querySelectorAll('.value')].map((v) => v.textContent);
			expect(values, SPEED.options[i].id).toEqual([
				SPEED.options[i].value,
				SPEED.options[i].valueFiat
			]);
		}
		// Said BEFORE the options: a person is owed the "just this one" promise
		// before the tap, not after it.
		expect(once?.textContent).toBe('For this transaction only');
		const order = (once as HTMLElement).compareDocumentPosition(options[0]);
		expect(order & Node.DOCUMENT_POSITION_FOLLOWING).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
	});

	// The owner's ruling, drawn: the NAME is the speed, and the advantage that
	// used to be smuggled into it ("Economy" under a "Speed" heading) is a line
	// of its own underneath. A model test cannot see this — it stays green over
	// a component that never renders the line.
	it('shows every option WITH the line that says what that speed buys', async () => {
		const { options } = await drawn({ open: true });
		expect(options.map((o) => o.querySelector('.detail')?.textContent)).toEqual([
			'First to confirm, even when the network is busy',
			'Balanced for everyday transfers',
			'Lowest fee, if you can wait'
		]);
	});

	// At the narrowest phone, which is where a two-line option either works or
	// does not: the description sits BELOW the name, gets the option's whole
	// width (it is under the fee as well, not squeezed beside it), and nothing
	// spills out of the 320px the sheet has.
	it('puts the description on its own line, full width, at 320px', async () => {
		const { options } = await drawn({ open: true });
		for (const option of options) {
			const name = option.querySelector('.name') as HTMLElement;
			const detail = option.querySelector('.detail') as HTMLElement;
			const [nameBox, detailBox, optionBox] = [
				name.getBoundingClientRect(),
				detail.getBoundingClientRect(),
				option.getBoundingClientRect()
			];
			expect(detailBox.top, option.textContent ?? '').toBeGreaterThanOrEqual(nameBox.bottom);
			expect(detailBox.width).toBeGreaterThan(nameBox.width);
			expect(option.scrollWidth).toBeLessThanOrEqual(Math.ceil(optionBox.width));
		}
	});

	it('marks the one in force, and only that one', async () => {
		const { options } = await drawn({ open: true });
		expect(options.map((o) => o.getAttribute('aria-pressed'))).toEqual(['true', 'false', 'false']);
	});

	it('emits the WIRE name of the tier that was tapped', async () => {
		const picked: string[] = [];
		const { options } = await drawn({ open: true }, (id) => picked.push(id));
		options[2].click();
		await tick();
		// `slow`, not "Slow": the name on the wire is the one the relay
		// validates, and a label is a translation.
		expect(picked).toEqual(['slow']);
	});

	/**
	 * Issue 684 — what each speed actually buys, beside its name.
	 *
	 * On seven networks the three fees are identical, because `fee_policy`
	 * clamps every tier to the $0.01 floor. Three equal figures and nothing
	 * else read as a broken picker; this is the row that tells them apart. A
	 * model test cannot see whether it is actually drawn — which is the trap
	 * this file exists for.
	 */
	it('draws the effective gas price for every option that has one', async () => {
		const { options } = await drawn({ open: true });
		expect(options.map(gasOf)).toEqual(['300 gwei', '282 gwei', '270 gwei']);
	});

	it('draws nothing at all where there is no honest number', async () => {
		const { options } = await drawn({
			open: true,
			gasPriceLine: false,
			options: SPEED.options.map((option) => ({ ...option, gasPrice: undefined }))
		});
		expect(options.map((o) => o.querySelector('.gas'))).toEqual([null, null, null]);
		// And the row is still a complete, readable option.
		expect(options.map((o) => o.querySelector('.name')?.textContent)).toEqual([
			'Fast',
			'Standard',
			'Slow'
		]);
	});

	// Quiet, and subordinate: under the money rather than beside it, smaller
	// than it, and in the muted colour. The fee stays the primary figure.
	it('keeps the gas price under the fee and visually beneath it', async () => {
		const { options } = await drawn({ open: true });
		for (const option of options) {
			const amounts = option.querySelector('.amounts') as HTMLElement;
			const gas = option.querySelector('.gas') as HTMLElement;
			const value = option.querySelector('.value') as HTMLElement;
			expect(gas.getBoundingClientRect().top).toBeGreaterThanOrEqual(
				amounts.getBoundingClientRect().bottom
			);
			// Ending on the money's right edge, as the fee's own footnote.
			expect(
				Math.abs(
					(option.querySelector('.gas-value') as HTMLElement).getBoundingClientRect().right -
						amounts.getBoundingClientRect().right
				)
			).toBeLessThanOrEqual(1);
			const size = (el: HTMLElement) => parseFloat(getComputedStyle(el).fontSize);
			expect(size(gas)).toBeLessThan(size(value));
			expect(getComputedStyle(gas).color).not.toBe(getComputedStyle(value).color);
		}
	});

	// The reason it goes on its own line rather than joining the fee: at 320px
	// a fourth item on that line would wrap the money or squeeze the name, and
	// the description below must keep its own full-width line either way.
	it('does not crowd the row or reflow the description at 320px', async () => {
		const { options } = await drawn({ open: true });
		for (const option of options) {
			const name = option.querySelector('.name') as HTMLElement;
			const detail = option.querySelector('.detail') as HTMLElement;
			const amounts = option.querySelector('.amounts') as HTMLElement;
			const gas = option.querySelector('.gas') as HTMLElement;
			expect(detail.getBoundingClientRect().top).toBeGreaterThanOrEqual(
				gas.getBoundingClientRect().bottom
			);
			expect(detail.getBoundingClientRect().width).toBeGreaterThan(
				name.getBoundingClientRect().width
			);
			// The money never wrapped to make room for it.
			expect(amounts.getBoundingClientRect().height).toBeLessThan(
				amounts.getBoundingClientRect().width
			);
			expect(option.scrollWidth).toBeLessThanOrEqual(
				Math.ceil(option.getBoundingClientRect().width)
			);
		}
	});

	// Named in plain sight. Under a fee, in the same numeric face, a bare
	// "3,244 wei" reads as a second amount being charged — and on a
	// floor-clamped chain as one that disagrees with the fee about which tier
	// is dearer. The label is drawn, quiet, and ahead of the figure it names.
	it('names the figure where a person can see it', async () => {
		const { options } = await drawn({ open: true });
		for (const option of options) {
			const label = option.querySelector('.gas-label') as HTMLElement;
			const value = option.querySelector('.gas-value') as HTMLElement;
			expect(label.textContent).toBe('Gas Bid');
			expect(getComputedStyle(label).clipPath).toBe('none');
			expect(getComputedStyle(label).position).not.toBe('absolute');
			expect(label.getBoundingClientRect().width).toBeGreaterThan(0);
			expect(label.getBoundingClientRect().right).toBeLessThanOrEqual(
				value.getBoundingClientRect().left
			);
			// Quiet: the same muted colour as its figure, not the fee's.
			const fee = option.querySelector('.value') as HTMLElement;
			expect(getComputedStyle(label).color).not.toBe(getComputedStyle(fee).color);
		}
	});

	/**
	 * The option keeps ONE height while its tier re-measures. The line used
	 * to be unmounted whenever a tier had no settled figure, so on every open,
	 * tap and refresh two rows lost a line and grew it back — the picker
	 * visibly jumped. Held open empty, it cannot.
	 */
	it('holds the line open while the set is measuring, so nothing jumps', async () => {
		const settled = await drawn({ open: true });
		const heights = settled.options.map((o) => o.getBoundingClientRect().height);
		document.body.innerHTML = '';
		const measuring = await drawn({
			open: true,
			options: SPEED.options.map((option) => ({
				...option,
				value: '…',
				valueFiat: undefined,
				gasPrice: undefined
			}))
		});
		expect(measuring.options.map((o) => o.getBoundingClientRect().height)).toEqual(heights);
		// Held open, but saying nothing: no label naming a figure that is not there.
		for (const option of measuring.options) {
			expect(option.querySelector('.gas')).not.toBeNull();
			expect(option.querySelector('.gas-label')).toBeNull();
			expect(option.querySelector('.gas')?.textContent?.trim()).toBe('');
		}
	});

	/**
	 * The worst the corpus has, at the narrowest phone: the longest label
	 * (Italian "Offerta del gas"), the longest offered tier names (Russian) and
	 * a five-digit figure. The label is drawn; nothing spills and the money
	 * still does not wrap.
	 */
	it('fits its label at 320px with the longest words the corpus has', async () => {
		const names = ['Быстро', 'Стандартно', 'Медленно'];
		const { options } = await drawn({
			open: true,
			gasPriceLabel: 'Offerta del gas',
			options: SPEED.options.map((option, i) => ({
				...option,
				label: names[i],
				gasPrice: ['270.11 gwei', '270.14 gwei', '299.59 gwei'][i]
			}))
		});
		for (const option of options) {
			const amounts = option.querySelector('.amounts') as HTMLElement;
			const gas = option.querySelector('.gas') as HTMLElement;
			const detail = option.querySelector('.detail') as HTMLElement;
			expect(option.scrollWidth).toBeLessThanOrEqual(
				Math.ceil(option.getBoundingClientRect().width)
			);
			expect(gas.getBoundingClientRect().height).toBeLessThan(gas.getBoundingClientRect().width);
			expect(amounts.getBoundingClientRect().height).toBeLessThan(
				amounts.getBoundingClientRect().width
			);
			expect(detail.getBoundingClientRect().top).toBeGreaterThanOrEqual(
				gas.getBoundingClientRect().bottom
			);
		}
	});

	/**
	 * Issue 685 — the gas price as a RANGE, at the narrowest phone, with the
	 * longest words the corpus has. The widest range the formatter can write,
	 * produced BY the formatter rather than typed: bids just under 0.001 gwei
	 * whose caps cross it, so the whole set goes to gwei and a collision
	 * widens it to five digits — `0.00099999 ~ 0.0029999 gwei`, 27 characters
	 * (a cap is at most 3 × its bid, so no gwei-sized set writes longer).
	 * Under the longest label and beside the longest tier names it stays on
	 * ONE line, the money does not wrap, the description keeps its own
	 * full-width line below, and nothing spills out of the 320px — nor out of
	 * the 272px an option really gets on a 320px phone (measured in the send
	 * form, once the page's gutters are taken). No row wraps its money under
	 * its name, and the NAME, the thing being chosen, is not elided: while the
	 * range sat in the money's column it took its width from the name, and at
	 * 272px "Стандартно" dropped its fee to a second line on that row alone.
	 */
	it('fits the widest range on one line at 320px and 272px without reflowing the description', async () => {
		const names = ['Быстро', 'Стандартно', 'Медленно'];
		const ranges = gasPriceRangeTexts([
			{ low: 999_990n, high: 2_999_870n },
			{ low: 999_950n, high: 1_999_810n },
			{ low: 999_910n, high: 1_499_780n }
		]) as string[];
		expect(ranges[0]).toBe('0.00099999 ~ 0.0029999 gwei');
		const { options } = await drawn({
			open: true,
			gasPriceLabel: 'Offerta del gas',
			options: SPEED.options.map((option, i) => ({
				...option,
				label: names[i],
				gasPrice: ranges[i]
			}))
		});
		expect(options.map(gasOf)).toEqual(ranges);
		for (const width of ['320px', '272px']) {
			(options[0].closest('section')?.parentElement as HTMLElement).style.width = width;
			for (const option of options) fitsOneLine(option);
		}
	});

	/**
	 * Settling from "…" to three ranges changes no option's height — the
	 * range is on the line that was held open for it, so the picker does not
	 * jump when the set lands.
	 */
	it('does not jump when three ranges land on the held line', async () => {
		const ranges = ['0.02021 ~ 0.06041 gwei', '0.02013 ~ 0.04023 gwei', '0.02011 ~ 0.03016 gwei'];
		const settled = await drawn({
			open: true,
			options: SPEED.options.map((option, i) => ({ ...option, gasPrice: ranges[i] }))
		});
		const heights = settled.options.map((o) => o.getBoundingClientRect().height);
		document.body.innerHTML = '';
		const measuring = await drawn({
			open: true,
			options: SPEED.options.map((option) => ({
				...option,
				value: '…',
				valueFiat: undefined,
				gasPrice: undefined
			}))
		});
		expect(measuring.options.map((o) => o.getBoundingClientRect().height)).toEqual(heights);
	});

	// A tier with no figure yet must still be a row somebody can read and pick.
	it('draws a waiting option without dropping its name', async () => {
		const { options } = await drawn({
			open: true,
			options: [
				SPEED.options[0],
				{ ...SPEED.options[1], value: '…', valueFiat: '' },
				SPEED.options[2]
			]
		});
		expect(options[1].querySelector('.name')?.textContent).toBe('Standard');
		expect([...options[1].querySelectorAll('.value')].map((v) => v.textContent)).toEqual(['…']);
	});
});

/**
 * Issue 686 — what the control says about the NETWORK rather than the choice.
 *
 * Both lines are rendering, which is why they are pinned here: a view-model
 * test stays green over a component that never draws them.
 */
describe('FeeSpeedRow on a network where the choice buys nothing extra (issue 686)', () => {
	const FREE = 'Fast costs no more on this network, so this send uses Fast.';
	const SINGLE = 'This network has one speed.';
	const lines = (container: HTMLElement) => ({
		free: container.querySelector('.free')?.textContent ?? null,
		single: container.querySelector('.single')?.textContent ?? null
	});

	// The summary says "Fast" while Settings says the person's slower default;
	// the reason has to be right there, without opening anything.
	it('says why the speed in force is the fastest, folded', async () => {
		const screen = render(FeeSpeedRow, { props: { speed: { ...SPEED, freeNote: FREE } } });
		screen.container.style.width = '320px';
		await tick();
		const summary = screen.container.querySelector('button.summary') as HTMLElement;
		expect(summary.textContent).toContain('Fast');
		expect(lines(screen.container).free).toBe(FREE);
		// Under the word it explains, not somewhere after it.
		const note = screen.container.querySelector('.free') as HTMLElement;
		expect(note.getBoundingClientRect().top).toBeGreaterThanOrEqual(
			summary.getBoundingClientRect().bottom
		);
		expect(screen.container.querySelectorAll('button.option')).toHaveLength(0);
	});

	it('says nothing of the kind when nothing was upgraded', async () => {
		const screen = render(FeeSpeedRow, { props: { speed: SPEED } });
		await tick();
		expect(lines(screen.container)).toEqual({ free: null, single: null });
	});

	it('replaces the three options with one statement where there is one speed', async () => {
		const screen = render(FeeSpeedRow, {
			props: { speed: { ...SPEED, open: true, singleNote: SINGLE } }
		});
		screen.container.style.width = '320px';
		await tick();
		expect(lines(screen.container).single).toBe(SINGLE);
		expect(screen.container.querySelectorAll('button.option')).toHaveLength(0);
		expect(screen.container.querySelector('ul')).toBeNull();
		// No "for this transaction only" over a choice that is not there.
		expect(screen.container.querySelector('.once')).toBeNull();
		const single = screen.container.querySelector('.single') as HTMLElement;
		expect(single.scrollWidth).toBeLessThanOrEqual(Math.ceil(single.getBoundingClientRect().width));
	});

	// The longest wording the corpus has for each line (ru), in the 272 CSS
	// pixels the control really gets on a 320px phone: it wraps as a calm
	// paragraph and never spills sideways or pushes the summary around.
	it('wraps the longest wording at 272px without spilling', async () => {
		const RU_FREE =
			'В этой сети «Быстро» не стоит дороже, поэтому эта транзакция отправляется со скоростью «Быстро».';
		const RU_SINGLE = 'В этой сети только одна скорость.';
		for (const width of ['320px', '272px']) {
			const screen = render(FeeSpeedRow, {
				props: { speed: { ...SPEED, open: true, freeNote: RU_FREE, singleNote: RU_SINGLE } }
			});
			screen.container.style.width = width;
			await tick();
			for (const selector of ['.free', '.single', 'button.summary']) {
				const box = screen.container.querySelector(selector) as HTMLElement;
				expect(box.scrollWidth, `${selector} @ ${width}`).toBeLessThanOrEqual(
					Math.ceil(box.getBoundingClientRect().width)
				);
			}
			const free = screen.container.querySelector('.free') as HTMLElement;
			const summary = screen.container.querySelector('button.summary') as HTMLElement;
			expect(free.getBoundingClientRect().top).toBeGreaterThanOrEqual(
				summary.getBoundingClientRect().bottom
			);
			screen.unmount();
		}
	});

	it('keeps the statement folded away with the rest of the control', async () => {
		const screen = render(FeeSpeedRow, { props: { speed: { ...SPEED, singleNote: SINGLE } } });
		await tick();
		expect(lines(screen.container).single).toBeNull();
	});
});
