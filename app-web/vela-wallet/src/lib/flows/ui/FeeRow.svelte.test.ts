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
		mark: row.querySelector('.label + *') as HTMLElement,
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
		// Narrower still, so the money HAS to give way — the case the row is for.
		const { column, row, label, values } = await drawn('220px');
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
		// The money dropped WHOLE onto its own line, right-aligned under the coin.
		const [coin, money] = values.map((v) => v.getBoundingClientRect());
		expect(money.top).toBeGreaterThanOrEqual(coin.bottom - 0.5);
		expect(Math.abs(money.right - coin.right)).toBeLessThanOrEqual(0.5);
	});

	it("never paints the coin's mark over the figure: the label gives way, elided on its line", async () => {
		// The row used to let the VALUE column shrink below its figure, which
		// then ran leftward under the mark — "0 [ETH] 01329 AVAX" — with
		// nothing overflowing the row, so no width check could see it. The
		// longest shipped labels at a 320px phone's column, and a worst case.
		const cases: [string, string, string][] = [
			['Commissione di rete', '272px', '1'],
			['ネットワーク手数料', '272px', '1'],
			['Commissione di rete', '240px', '1.3'],
			['Network fee', '220px', '1']
		];
		for (const [text, width, scale] of cases) {
			document.documentElement.style.setProperty('--text-scale', scale);
			const { column, row, label, mark, values } = await drawn(width, { ...FEE, label: text });
			const at = `${text} @ ${width} × ${scale}`;
			const labelBox = label.getBoundingClientRect();
			const markBox = mark.getBoundingClientRect();
			expect(markBox.width, at).toBeGreaterThan(0);
			expect(markBox.left, at).toBeGreaterThanOrEqual(labelBox.right - 0.5);
			for (const value of values) {
				const box = value.getBoundingClientRect();
				expect(box.left, at).toBeGreaterThanOrEqual(markBox.right - 0.5);
				expect(box.right, at).toBeLessThanOrEqual(column.right + 0.5);
				expect(box.height, at).toBeLessThanOrEqual(oneLine(value) + 0.5);
			}
			expect(labelBox.height, at).toBeLessThanOrEqual(oneLine(label) + 0.5);
			expect(row.scrollWidth, at).toBeLessThanOrEqual(row.clientWidth);
		}
		document.documentElement.style.removeProperty('--text-scale');
	});

	it('gives the money up before the label: a label is elided only once the money has dropped', async () => {
		// Every width on the way down: the label is never short of room while
		// the money is still beside the coin — and there IS a width where the
		// money has dropped and the label is still whole.
		let droppedWithLabelWhole = false;
		for (let width = 400; width >= 200; width -= 10) {
			const { label, values } = await drawn(`${width}px`);
			const [coin, money] = values.map((v) => v.getBoundingClientRect());
			const dropped = money.top >= coin.bottom - 0.5;
			const elided = label.scrollWidth > label.clientWidth;
			if (elided) expect(dropped, `${width}px`).toBe(true);
			if (dropped && !elided) droppedWithLabelWhole = true;
		}
		expect(droppedWithLabelWhole).toBe(true);
		// …and with room for everything, nothing is elided.
		const wide = await drawn('600px', { ...FEE, label: 'Commissione di rete' });
		expect(wide.label.scrollWidth).toBeLessThanOrEqual(wide.label.clientWidth);
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
