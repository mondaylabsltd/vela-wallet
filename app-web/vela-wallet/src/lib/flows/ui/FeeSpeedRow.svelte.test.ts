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

/** A frame for layout and the size observer to catch up. */
const settle = async () => {
	await tick();
	await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
};

/** The gas price a tier's row is showing — `undefined` when it shows none. */
const gasOf = (option: HTMLElement) => option.querySelector('.gas-value')?.textContent?.trim();

/**
 * One option of the picker, laid out whole (078 round 2): line 1 is the name
 * and the money, line 2 the description and the gas bid — the reason under
 * the name, the bid under the money — the tick in its own column. Nothing
 * spills, the money and the bid are each one unbroken line, and the name is
 * read whole.
 */
const twoLines = (option: HTMLElement) => {
	const box = (selector: string) =>
		(option.querySelector(selector) as HTMLElement).getBoundingClientRect();
	const name = box('.name');
	const values = box('.values');
	const detail = box('.detail');
	const gas = box('.gas');
	const gasValue = option.querySelector('.gas-value') as HTMLElement;
	const line = parseFloat(getComputedStyle(gasValue).lineHeight) || gasValue.offsetHeight;
	const label = option.textContent ?? '';
	// Line 1: the money beside the name, on one line.
	expect(values.top, label).toBeLessThan(name.bottom);
	expect(values.height, label).toBeLessThan(values.width);
	// Line 2: under line 1, the reason starting where the name starts…
	expect(detail.top, label).toBeGreaterThanOrEqual(name.bottom - 1);
	expect(Math.abs(detail.left - name.left), label).toBeLessThanOrEqual(1);
	// …and the bid under the money, whole, ending on the money's right edge.
	expect(gas.top, label).toBeGreaterThanOrEqual(values.bottom - 1);
	expect(gasValue.getBoundingClientRect().height, label).toBeLessThan(line * 1.5);
	expect(
		Math.abs(gasValue.getBoundingClientRect().right - values.right),
		label
	).toBeLessThanOrEqual(1);
	// The reason never runs under the bid.
	if (gas.top < detail.bottom) expect(detail.right, label).toBeLessThanOrEqual(gas.left + 1);
	// Nothing spills, and the name is whole.
	expect(option.scrollWidth).toBeLessThanOrEqual(Math.ceil(option.getBoundingClientRect().width));
	const nameEl = option.querySelector('.name') as HTMLElement;
	expect(nameEl.scrollWidth, nameEl.textContent ?? '').toBeLessThanOrEqual(nameEl.clientWidth);
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

	// 078 round 2, at the narrowest phone: two lines, not three. The reason is
	// line 2 under the name, the bid shares that line under the money, and
	// there is no longer an empty half-line under every name.
	it('puts the description and the gas bid on ONE second line at 320px', async () => {
		const { options } = await drawn({ open: true });
		for (const option of options) {
			twoLines(option);
			const detail = (option.querySelector('.detail') as HTMLElement).getBoundingClientRect();
			const gas = (option.querySelector('.gas') as HTMLElement).getBoundingClientRect();
			// The bid sits on the reason's FIRST line — the same line, not a third.
			expect(gas.top, option.textContent ?? '').toBeLessThan(detail.top + detail.height / 2);
		}
	});

	// The design rule: accent is for moving money and submitting. The speed in
	// force is marked in TEXT colour and weight, the tick included, and the
	// tick keeps its own column, level with line 1.
	it('marks the speed in force in text colour, never accent, the tick level with line 1', async () => {
		const { options } = await drawn({ open: true });
		const probe = (color: string) => {
			const el = document.createElement('span');
			el.style.color = color;
			document.body.appendChild(el);
			const value = getComputedStyle(el).color;
			el.remove();
			return value;
		};
		const fg = probe('var(--color-fg-base)');
		const accent = probe('var(--color-accent-base)');
		const selected = options[0];
		const name = selected.querySelector('.name') as HTMLElement;
		const tick = selected.querySelector('.tick') as HTMLElement;
		expect(getComputedStyle(name).color).toBe(fg);
		expect(getComputedStyle(tick).color).toBe(fg);
		expect(getComputedStyle(name).color).not.toBe(accent);
		expect(Number(getComputedStyle(name).fontWeight)).toBeGreaterThanOrEqual(600);
		const [n, t] = [name.getBoundingClientRect(), tick.getBoundingClientRect()];
		expect(Math.abs((t.top + t.bottom) / 2 - (n.top + n.bottom) / 2)).toBeLessThanOrEqual(2);
		// Unselected names are the quieter colour.
		expect(getComputedStyle(options[1].querySelector('.name') as HTMLElement).color).not.toBe(fg);
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
			const values = option.querySelector('.values') as HTMLElement;
			const gas = option.querySelector('.gas') as HTMLElement;
			const value = option.querySelector('.value') as HTMLElement;
			expect(gas.getBoundingClientRect().top).toBeGreaterThanOrEqual(
				values.getBoundingClientRect().bottom - 1
			);
			// Ending on the money's right edge, as the fee's own footnote.
			expect(
				Math.abs(
					(option.querySelector('.gas-value') as HTMLElement).getBoundingClientRect().right -
						values.getBoundingClientRect().right
				)
			).toBeLessThanOrEqual(1);
			const size = (el: HTMLElement) => parseFloat(getComputedStyle(el).fontSize);
			expect(size(gas)).toBeLessThan(size(value));
			expect(getComputedStyle(gas).color).not.toBe(getComputedStyle(value).color);
			// The figure in the numeric face with tabular digits; the word in the UI face.
			const figure = option.querySelector('.gas-value') as HTMLElement;
			expect(getComputedStyle(figure).fontVariantNumeric).toContain('tabular-nums');
			expect(getComputedStyle(figure).fontFamily).not.toBe(getComputedStyle(gas).fontFamily);
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
	 * visibly jumped. Since 078 round 2 the bid shares line 2 with the
	 * description, so a blank bid would also hand the description room to
	 * re-wrap into: the slot is held at the width it last had.
	 */
	it('holds the bid’s room while a tier re-measures, so nothing jumps', async () => {
		const screen = render(FeeSpeedRow, { props: { speed: { ...SPEED, open: true } } });
		screen.container.style.width = '320px';
		await settle();
		const options = () => [...screen.container.querySelectorAll<HTMLElement>('button.option')];
		const heights = options().map((o) => o.getBoundingClientRect().height);
		await screen.rerender({
			speed: {
				...SPEED,
				open: true,
				options: SPEED.options.map((option) => ({
					...option,
					value: '…',
					valueFiat: undefined,
					gasPrice: undefined
				}))
			}
		});
		await settle();
		expect(options().map((o) => o.getBoundingClientRect().height)).toEqual(heights);
		// Held open, but saying nothing: no label naming a figure that is not there.
		for (const option of options()) {
			expect(option.querySelector('.gas')).not.toBeNull();
			expect(option.querySelector('.gas-label')).toBeNull();
			const reserve = option.querySelector('.gas-reserve') as HTMLElement;
			expect(getComputedStyle(reserve).visibility).toBe('hidden');
			expect(reserve.getAttribute('aria-hidden')).toBe('true');
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
		for (const option of options) twoLines(option);
	});

	/**
	 * Issue 685 — the gas price as a RANGE, at the narrowest phone, with the
	 * longest words the corpus has. The widest range the formatter can write,
	 * produced BY the formatter rather than typed: bids just under 0.001 gwei
	 * whose caps cross it, so the whole set goes to gwei and a collision
	 * widens it to five digits — `0.00099999 ~ 0.0029999 gwei`, 27 characters
	 * (a cap is at most 3 × its bid, so no gwei-sized set writes longer).
	 * Under the longest label and beside the longest tier names it stays on
	 * ONE line under the money, the money does not wrap, the description is
	 * never run under it, and nothing spills out of the 320px — nor out of
	 * the 272px an option really gets on a 320px phone (measured in the send
	 * form, once the page's gutters are taken). No row wraps its money under
	 * its name, and the NAME, the thing being chosen, is not elided: while the
	 * range sat in the money's column it took its width from the name, and at
	 * 272px "Стандартно" dropped its fee to a second line on that row alone.
	 */
	it('fits the widest range on one line at 320px and 272px, beside or above the description', async () => {
		const names = ['Быстро', 'Стандартно', 'Медленно'];
		// The core's own strings for the widest set it can draw — five digits on
		// both ends of every row (pinned in `app_fee_speed.rs`,
		// `the_widest_ranges_read_at_five_digits`).
		const ranges = [
			'0.00099999 ~ 0.0029999 gwei',
			'0.00099995 ~ 0.0019998 gwei',
			'0.00099991 ~ 0.0014998 gwei'
		];
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
			for (const option of options) twoLines(option);
		}
	});

	/**
	 * Ranges that re-measure and land again change no option's height — the
	 * second landing is on the room the first one held.
	 */
	it('does not jump when three ranges land again on the held line', async () => {
		const ranges = ['0.02021 ~ 0.06041 gwei', '0.02013 ~ 0.04023 gwei', '0.02011 ~ 0.03016 gwei'];
		const settled = {
			...SPEED,
			open: true,
			options: SPEED.options.map((option, i) => ({ ...option, gasPrice: ranges[i] }))
		};
		const screen = render(FeeSpeedRow, { props: { speed: settled } });
		screen.container.style.width = '320px';
		await settle();
		const options = () => [...screen.container.querySelectorAll<HTMLElement>('button.option')];
		const heights = options().map((o) => o.getBoundingClientRect().height);
		const measuring = {
			...settled,
			options: settled.options.map((option) => ({
				...option,
				value: '…',
				valueFiat: undefined,
				gasPrice: undefined
			}))
		};
		await screen.rerender({ speed: measuring });
		await settle();
		expect(options().map((o) => o.getBoundingClientRect().height)).toEqual(heights);
		await screen.rerender({ speed: settled });
		await settle();
		expect(options().map((o) => o.getBoundingClientRect().height)).toEqual(heights);
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
