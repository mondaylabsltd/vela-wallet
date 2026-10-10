/**
 * The fee row in a tight column (issue 231): the reporter's screenshot had
 * "Network fee" wrapped onto two lines, because the row was one string and
 * broke wherever it ran out — which was inside its own label.
 *
 * A `.svelte.test.ts`: where a line breaks is the browser's to answer, and no
 * node test of the view-model can see it.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import FeeRow from './FeeRow.svelte';

const FEE = {
	label: 'Network fee',
	mark: { ticker: 'AVAX', badgeColor: 'var(--color-fg-muted)' },
	value: '0.001329 AVAX',
	valueFiat: '≈ $0.01',
	openLabel: 'Fee token',
	refreshLabel: 'Refresh fee'
};

/** How tall ONE line of this element's text is, from the element itself. */
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

const drawn = async (width: string, fee: typeof FEE = FEE, onrefresh?: () => void) => {
	const screen = render(FeeRow, { props: { fee, onrefresh } });
	screen.container.style.width = width;
	await tick();
	// `.open` is the coin opener — the part that used to BE the row. The ⟳
	// (spec 068) is its sibling, so the give-way order below is measured on
	// the element whose children actually compete for the width.
	const row = screen.container.querySelector('button.open') as HTMLElement;
	return {
		screen,
		column: screen.container.getBoundingClientRect(),
		row,
		refresh: screen.container.querySelector('button.refresh') as HTMLButtonElement,
		stale: screen.container.querySelector('.stale') as HTMLElement | null,
		label: row.querySelector('.label') as HTMLElement,
		// The coin's mark: the first piece of the coin's own line.
		mark: row.querySelector('.coin > *') as HTMLElement,
		amount: row.querySelector('.amount') as HTMLElement,
		values: [...row.querySelectorAll<HTMLElement>('.value')]
	};
};

describe('FeeRow', () => {
	it('keeps "Network fee" on one line in a 300px column, and in a tighter one', async () => {
		// 220px is the width that tells: in this harness's font the old row
		// (one string, a shrinkable label) still fitted at 300px and wrapped
		// its label to two lines — 32 high against 16 — at 220px.
		for (const width of ['300px', '220px']) {
			const { label } = await drawn(width);
			expect(label.getBoundingClientRect().height, width).toBeLessThanOrEqual(oneLine(label) + 0.5);
		}
	});

	it('breaks no figure: each half is one line, and both stay inside the column', async () => {
		// Narrow, so the label and the fee cannot share a line.
		const { column, row, label, values, amount } = await drawn('220px');
		// No "·": a dropped line that began with one read as a leftover.
		expect(values.map((v) => v.textContent?.trim())).toEqual(['0.001329 AVAX', '≈ $0.01']);
		expect(label.getBoundingClientRect().height).toBeLessThanOrEqual(oneLine(label) + 0.5);
		for (const value of values) {
			const box = value.getBoundingClientRect();
			expect(box.height).toBeLessThanOrEqual(oneLine(value) + 0.5);
			expect(box.right).toBeLessThanOrEqual(column.right + 0.5);
			expect(box.left).toBeGreaterThanOrEqual(column.left - 0.5);
		}
		expect(row.scrollWidth).toBeLessThanOrEqual(row.clientWidth);
		// The fee went under the label WHOLE, at the row's end.
		expect(amount.getBoundingClientRect().top).toBeGreaterThanOrEqual(
			label.getBoundingClientRect().bottom - 0.5
		);
	});

	// 078 round 3 (founder, German at the largest size): the label was broken
	// mid-word ("Netzwerkg / ebühr") and the value ended in "…". The label is
	// a whole: never elided, never broken inside a word; when it and the fee
	// cannot share a line, it keeps its own and the fee goes under it.
	it('keeps the label whole — never elided, never broken — the fee under it when they do not fit', async () => {
		const cases: [string, string, string][] = [
			['Netzwerkgebühr', '342px', '1.35'],
			['Netzwerkgebühr', '272px', '1.35'],
			['Commissione di rete', '272px', '1'],
			['ネットワーク手数料', '272px', '1'],
			['Commissione di rete', '240px', '1.3'],
			['Сетевая комиссия', '272px', '1.35'],
			['Network fee', '220px', '1']
		];
		for (const [text, width, scale] of cases) {
			document.documentElement.style.setProperty('--text-scale', scale);
			const { column, row, label, mark, values, amount } = await drawn(width, {
				...FEE,
				label: text
			});
			const at = `${text} @ ${width} × ${scale}`;
			// Whole: nothing clipped, and no word split across lines.
			expect(label.scrollWidth, at).toBeLessThanOrEqual(label.clientWidth + 0.5);
			expect(getComputedStyle(label).textOverflow, at).not.toBe('ellipsis');
			const words = text.split(' ');
			if (words.length === 1) {
				expect(label.getBoundingClientRect().height, at).toBeLessThanOrEqual(oneLine(label) + 0.5);
			}
			// The fee is beside the label, or wholly under it — never over it.
			const [labelBox, amountBox, markBox] = [label, amount, mark].map((el) =>
				el.getBoundingClientRect()
			);
			const beside = amountBox.left >= labelBox.right - 0.5;
			const under = amountBox.top >= labelBox.bottom - 0.5;
			expect(beside || under, at).toBe(true);
			// And the figures are never painted under the coin's mark.
			for (const value of values) {
				const box = value.getBoundingClientRect();
				expect(box.left, at).toBeGreaterThanOrEqual(markBox.right - 0.5);
				expect(box.right, at).toBeLessThanOrEqual(column.right + 0.5);
				expect(box.height, at).toBeLessThanOrEqual(oneLine(value) + 0.5);
			}
			expect(row.scrollWidth, at).toBeLessThanOrEqual(row.clientWidth);
		}
		document.documentElement.style.removeProperty('--text-scale');
	});

	it('shares one line while there is room, and gives the label its own only when there is not', async () => {
		// Every width on the way down: the label is never elided, and there IS a
		// width where the two share the line and one where the fee drops.
		let shared = false;
		let dropped = false;
		for (let width = 600; width >= 200; width -= 10) {
			const { label, amount } = await drawn(`${width}px`);
			expect(label.scrollWidth, `${width}px`).toBeLessThanOrEqual(label.clientWidth + 0.5);
			const under =
				amount.getBoundingClientRect().top >= label.getBoundingClientRect().bottom - 0.5;
			if (under) dropped = true;
			else shared = true;
		}
		expect(shared).toBe(true);
		expect(dropped).toBe(true);
	});

	/*
	 * 0.8 — withholding never moves the layout. Until the display currency
	 * commits the money half is "≈ …", which fits beside the coin on any
	 * line. Then the figure lands: "≈ ₫112,500.00" did not fit that line at a
	 * phone's width, the fee dropped under the label, and the form under the
	 * row moved down 18 px. While the money is withheld it takes a line of its
	 * own under the coin — the one layout whose line count does not depend on
	 * how long the figure turns out to be — and the row keeps it.
	 */
	it('with its money withheld, the figure landing moves nothing — long or short, at any width', async () => {
		// Down to a row too tight for the label and a long figure to share a
		// line, and at the largest text size: the money's line is its own, so
		// its length decides nothing (PR 3 final notes F12 and F13).
		for (const [width, scale] of [
			['342px', '1'],
			['272px', '1'],
			['500px', '1'],
			['220px', '1'],
			['272px', '1.35']
		] as const) {
			document.documentElement.style.setProperty('--text-scale', scale);
			const withheld = { ...FEE, value: '0.0015 ETH', valueFiat: '≈ …', valueFiatWithheld: true };
			const { screen, row } = await drawn(width, withheld as typeof FEE);
			const box = (selector: string, nth = 0) => {
				const r = row.querySelectorAll<HTMLElement>(selector)[nth].getBoundingClientRect();
				return [r.left, r.top, r.width, r.height].map((n) => Math.round(n * 10) / 10);
			};
			const card = () => (row.parentElement as HTMLElement).getBoundingClientRect().height;
			// The coin's mark (PR 3 final note F13): it slid 19 px left when the
			// figure landed, because it stood before a column as wide as its
			// longer line. It is one piece with the coin now.
			const mark = () => box('.coin > *');
			const before = {
				card: card(),
				label: box('.label'),
				coin: box('.value', 0),
				mark: mark()
			};
			// Beside the coin, on the coin's own line: the two share a middle.
			expect(before.mark[0] + before.mark[2], width).toBeLessThanOrEqual(before.coin[0] + 0.5);
			const middle = (b: number[]) => b[1] + b[3] / 2;
			expect(Math.abs(middle(before.mark) - middle(before.coin)), width).toBeLessThanOrEqual(1);
			// The money stands under the coin, on a line of its own.
			const moneyTop = box('.value', 1)[1];
			expect(moneyTop, width).toBeGreaterThanOrEqual(before.coin[1] + before.coin[3] - 0.5);

			// A long figure lands…
			await screen.rerender({ fee: { ...FEE, value: '0.0015 ETH', valueFiat: '≈ ₫112,500.00' } });
			await tick();
			expect(card(), `${width}: long figure`).toBe(before.card);
			expect(box('.label'), `${width}: label`).toEqual(before.label);
			expect(box('.value', 0), `${width}: the coin`).toEqual(before.coin);
			expect(mark(), `${width}: the coin's mark`).toEqual(before.mark);
			expect(box('.value', 1)[1], `${width}: the money's line`).toBe(moneyTop);
			expect(row.querySelectorAll('.value')[1].textContent).toBe('≈ ₫112,500.00');
			expect(row.scrollWidth, width).toBeLessThanOrEqual(row.clientWidth);

			// …or a short one: the row does not fold back up under the person.
			await screen.rerender({ fee: { ...FEE, value: '0.0015 ETH', valueFiat: '≈ $4.50' } });
			await tick();
			expect(card(), `${width}: short figure`).toBe(before.card);
			expect(box('.label'), `${width}: label`).toEqual(before.label);
			expect(box('.value', 0), `${width}: the coin`).toEqual(before.coin);
			expect(mark(), `${width}: the coin's mark, short figure`).toEqual(before.mark);
			screen.unmount();
		}
		document.documentElement.style.removeProperty('--text-scale');
	});

	it('a row whose money was never withheld is drawn as it always was: side by side while both fit', async () => {
		const { values } = await drawn('500px', { ...FEE, value: '0.0015 ETH', valueFiat: '≈ $4.50' });
		const [coin, money] = values.map((v) => v.getBoundingClientRect());
		expect(Math.abs(money.top - coin.top)).toBeLessThanOrEqual(0.5);
	});

	it('is one line when there is room, and draws no separator with no money to follow', async () => {
		const wide = await drawn('600px');
		const [coin, money] = wide.values.map((v) => v.getBoundingClientRect());
		expect(Math.abs(money.top - coin.top)).toBeLessThanOrEqual(0.5);

		const bare = await drawn('300px', { ...FEE, valueFiat: undefined as unknown as string });
		expect(bare.values.map((v) => v.textContent?.trim())).toEqual(['0.001329 AVAX']);
	});

	// --- spec 068 --------------------------------------------------------
	//
	// The refresh has to be a control of its OWN. The row was a single button
	// wrapping everything, and a refresh inside it would have been a button
	// inside a button: invalid markup, and a tap whose meaning depended on
	// which pixel it landed on.
	it('offers a refresh beside the fee, named, and not nested inside the coin opener', async () => {
		let asked = 0;
		const { row, refresh } = await drawn('300px', FEE, () => (asked += 1));
		expect(refresh.getAttribute('aria-label')).toBe('Refresh fee');
		expect(row.contains(refresh)).toBe(false);
		expect(refresh.disabled).toBe(false);
		refresh.click();
		await tick();
		expect(asked).toBe(1);
	});

	// A fee row with no live session behind it (the gallery) must not offer a
	// control that does nothing.
	// 078 round 2: the refresh control is the card's own height — same top and
	// bottom edges — at every text size, including a fee dropped to two lines
	// at the largest one, where it used to float mid-card.
	it('makes the refresh control exactly the card’s height, at every text size', async () => {
		for (const scale of ['0.82', '1', '1.35']) {
			for (const width of ['360px', '220px']) {
				const { screen, refresh } = await drawn(width, FEE, () => {});
				screen.container.style.setProperty('--text-scale', scale);
				await tick();
				const card = (refresh.parentElement as HTMLElement).getBoundingClientRect();
				const box = refresh.getBoundingClientRect();
				const label = `${scale} @ ${width}`;
				expect(Math.abs(box.top - card.top), label).toBeLessThanOrEqual(0.5);
				expect(Math.abs(box.bottom - card.bottom), label).toBeLessThanOrEqual(0.5);
				// …with the glyph still at its vertical centre.
				const glyph = (refresh.querySelector('svg') as SVGElement).getBoundingClientRect();
				expect(
					Math.abs((glyph.top + glyph.bottom) / 2 - (card.top + card.bottom) / 2),
					label
				).toBeLessThanOrEqual(1);
				screen.unmount();
			}
		}
	});

	// 078 round 3: what is SEEN of the refresh is a small round icon button —
	// a fixed circle at the card's vertical centre — while its tap area is the
	// card's height. Nothing visible stretches with a two-line card.
	it('draws the refresh as a fixed circle at the centre, whatever the card’s height', async () => {
		let circle: number | null = null;
		for (const [width, scale] of [
			['360px', '1'],
			['220px', '1.35']
		] as const) {
			const { screen, refresh } = await drawn(width, { ...FEE, label: 'Netzwerkgebühr' }, () => {});
			screen.container.style.setProperty('--text-scale', scale);
			await tick();
			const turn = refresh.querySelector('.turn') as HTMLElement;
			const [t, card] = [turn, refresh.parentElement as HTMLElement].map((el) =>
				el.getBoundingClientRect()
			);
			expect(Math.abs(t.width - t.height), width).toBeLessThanOrEqual(0.5);
			if (circle === null) circle = t.width;
			else expect(t.width, 'the same circle at every size').toBe(circle);
			expect(getComputedStyle(turn).borderRadius).not.toBe('0px');
			expect(
				Math.abs((t.top + t.bottom) / 2 - (card.top + card.bottom) / 2),
				width
			).toBeLessThanOrEqual(1);
			// The button around it paints nothing of its own.
			expect(getComputedStyle(refresh).backgroundColor).toBe('rgba(0, 0, 0, 0)');
			expect(refresh.hasAttribute('data-focus-inner')).toBe(true);
			screen.unmount();
		}
	});

	it('has no live refresh when nothing is listening', async () => {
		const { refresh } = await drawn('300px');
		expect(refresh.disabled).toBe(true);
	});

	it('says a measurement is out, and refuses a second tap while it is', async () => {
		let asked = 0;
		const { refresh } = await drawn(
			'300px',
			{ ...FEE, refreshing: true } as typeof FEE,
			() => (asked += 1)
		);
		expect(refresh.getAttribute('aria-busy')).toBe('true');
		expect(refresh.disabled).toBe(true);
		refresh.click();
		await tick();
		expect(asked).toBe(0);
	});

	// The turn must be ON the glyph. It was first applied to the BUTTON, whose
	// padding is deliberately lopsided (none at the start), so its geometric
	// centre sits away from the icon and `rotate(360deg)` swung the icon around
	// that point — it orbited instead of spinning, and the owner read it as the
	// control drifting about. A square, icon-tight box is the property that
	// makes a rotation stay put, so that is what this measures rather than the
	// class name.
	it('turns the glyph on its own centre, not the button around its lopsided one', async () => {
		const { screen, refresh } = await drawn(
			'300px',
			{ ...FEE, refreshing: true } as typeof FEE,
			() => {}
		);
		const turning = screen.container.querySelector('.spinning') as HTMLElement;
		expect(turning).not.toBeNull();
		expect(turning.tagName).not.toBe('BUTTON');

		const box = turning.getBoundingClientRect();
		expect(box.width).toBeGreaterThan(0);
		expect(Math.abs(box.width - box.height)).toBeLessThanOrEqual(1);

		// And it really is the icon's own box, not a stretched wrapper: the
		// button is wider than the glyph because of that start-side padding.
		expect(refresh.getBoundingClientRect().width).toBeGreaterThan(box.width);
	});

	// `FeeView.stale` finally has a consumer. It has to read as OLD, not
	// BROKEN: the danger colour belongs to a refusal, and a quote past its
	// 30s TTL is neither a refusal nor a fault.
	// The note's line is always in the layout; only its ink is toggled. Below
	// this row sit the speed control and Continue, so a note that appeared
	// would shove Continue down a line just as somebody reached for it.
	it('keeps the note’s line reserved, so nothing below shifts when it speaks', async () => {
		const quiet = await drawn('300px');
		const loud = await drawn('300px', {
			...FEE,
			staleNote: 'Measured a while ago'
		} as typeof FEE);

		expect(quiet.stale).not.toBeNull();
		expect(getComputedStyle(quiet.stale as HTMLElement).visibility).toBe('hidden');
		expect((quiet.stale as HTMLElement).getAttribute('aria-hidden')).toBe('true');
		expect((loud.stale as HTMLElement).getAttribute('aria-hidden')).toBeNull();

		// The whole block is the same height either way — that is the point.
		const height = (el: HTMLElement) => el.getBoundingClientRect().height;
		const block = (s: (typeof quiet)['screen']) =>
			height(s.container.querySelector('.fee') as HTMLElement);
		expect(block(quiet.screen)).toBeGreaterThan(0);
		expect(Math.abs(block(quiet.screen) - block(loud.screen))).toBeLessThanOrEqual(1);
	});

	it('shows an old quote calmly, in the muted tone and never the danger one', async () => {
		const { stale } = await drawn('300px', {
			...FEE,
			staleNote: 'Measured a while ago'
		} as typeof FEE);
		expect(stale?.textContent?.trim()).toBe('Measured a while ago');
		const colour = getComputedStyle(stale as HTMLElement).color;
		const token = (name: string) => {
			const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
			expect(value, name).not.toBe('');
			const probe = document.createElement('span');
			probe.style.color = value;
			document.body.appendChild(probe);
			const resolved = getComputedStyle(probe).color;
			probe.remove();
			return resolved;
		};

		// Two ways to fail this line, and it must fail both. Shouting turns an
		// old-but-working number into an error nobody will trust again; and the
		// faintest tone on a row of figures is the same as saying nothing — it
		// is what the owner read straight past.
		//
		// The token is `--color-error-base`. This assertion used to name
		// `--color-danger-base`, which does not exist: the lookup returned '',
		// nothing checked that, and "never the danger one" was comparing a real
		// colour against an empty string — true no matter what the tone was.
		// `token()` now fails on an empty value, so a renamed token cannot
		// quietly retire the check again.
		expect(colour).not.toBe(token('--color-error-base'));
		expect(colour).not.toBe(token('--color-warning-base'));
		expect(colour).not.toBe(token('--color-fg-subtle'));
		// Resolved through a token, so a theme change cannot make it shout.
		expect(colour).toBe(token('--color-fg-muted'));

		// Found by its shape before it is read.
		expect((stale as HTMLElement).querySelector('svg')).not.toBeNull();
	});
});

/**
 * PR 2 polish: a failed fee's row does exactly what its figure says — the
 * core's `FeeFailureView.tap`, as the builder puts it (`FeeRowModel.tap`).
 */
describe('FeeRow — the tap does what the figure says (PR 2 polish)', () => {
	const rowWith = async (tap: 'open' | 'retry' | 'none', value: string, refreshing = false) => {
		const calls: string[] = [];
		const screen = render(FeeRow, {
			props: {
				fee: { ...FEE, value, valueFiat: undefined, tap, refreshing },
				onopen: () => calls.push('open'),
				onrefresh: () => calls.push('refresh')
			}
		});
		await tick();
		return { screen, calls };
	};

	it('"Tap to retry" asks again — no chevron, no list; while a re-ask is out, nothing', async () => {
		const { screen, calls } = await rowWith('retry', 'Tap to retry');
		const row = screen.container.querySelector('button.open') as HTMLButtonElement;
		expect(row.getAttribute('aria-label')).toBe('Refresh fee');
		expect(row.querySelector('.amount > .chevron')).toBeNull();
		row.click();
		await tick();
		expect(calls).toEqual(['refresh']);
		screen.unmount();
		const busy = await rowWith('retry', '—', true);
		(busy.screen.container.querySelector('button.open') as HTMLButtonElement).click();
		await tick();
		expect(busy.calls).toEqual([]);
	});

	it('"Pay with another coin" opens the coins, behind its chevron', async () => {
		const { screen, calls } = await rowWith('open', 'Pay with another coin');
		const row = screen.container.querySelector('button.open') as HTMLButtonElement;
		expect(row.getAttribute('aria-label')).toBe('Fee token');
		// The coin's mark, and the chevron after the figure.
		expect(row.querySelector('.amount > .chevron:last-child > svg')).not.toBeNull();
		row.click();
		await tick();
		expect(calls).toEqual(['open']);
	});

	it('the dash with nothing behind it is no control — stated, no chevron, no press', async () => {
		const { screen, calls } = await rowWith('none', '—');
		expect(screen.container.querySelector('button.open')).toBeNull();
		const stated = screen.container.querySelector('[data-testid="fee-row-stated"]') as HTMLElement;
		expect(stated).not.toBeNull();
		expect(stated.querySelector('.amount > .chevron')).toBeNull();
		expect(getComputedStyle(stated).cursor).toBe('default');
		stated.click();
		await tick();
		expect(calls).toEqual([]);
	});
});
