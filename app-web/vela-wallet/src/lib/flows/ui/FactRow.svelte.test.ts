/**
 * The label-value row, in a browser — issue 423.
 *
 * On the Xiaomi the confirm's To row drew its avatar on its own, a wide gap
 * away from the name, level with neither line; the line under the name was
 * set in mono. The From row above it puts the avatar right beside the name.
 * A model test cannot see any of that — it is all layout — so the row is
 * drawn here and measured: the face beside the NAME on one line, at the From
 * row's distance, and the line under it in the body face.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { FactRowModel } from '../model';
import FactRow from './FactRow.svelte';

const SVG =
	'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 10 10"><circle cx="5" cy="5" r="5"/></svg>';
const PAYEE = '0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c';

const FROM: FactRowModel = {
	label: 'From',
	value: 'double check',
	lead: { kind: 'identicon', svg: SVG, address: '0x88cCA0EeDbF2C4426110bbFc998F048689266894' }
};

/** The issue's row: the registry's name for the address, whose word it is and the short address under it. */
const TO: FactRowModel = {
	label: 'To',
	value: 'Wallet',
	lead: { kind: 'identicon', svg: SVG, address: PAYEE },
	detail: 'Vela User · 0x14fB…eA5c'
};

const drawn = async (fact: FactRowModel) => {
	const screen = render(FactRow, { props: { fact } });
	// A phone's card: 390 less the screen's and the card's gutters.
	screen.container.style.width = '326px';
	await tick();
	await new Promise((resolve) => requestAnimationFrame(() => requestAnimationFrame(resolve)));
	const el = (selector: string) => screen.container.querySelector(selector) as HTMLElement;
	return {
		face: el('.mark').getBoundingClientRect(),
		value: el('.value'),
		detail: screen.container.querySelector('.detail') as HTMLElement | null
	};
};

describe('FactRow', () => {
	it("draws a payee's face beside the NAME, as the From row draws its own", async () => {
		const from = await drawn(FROM);
		const to = await drawn(TO);
		const name = to.value.getBoundingClientRect();

		// One line: the face is level with the name, not between two lines.
		const middle = (to.face.top + to.face.bottom) / 2;
		expect(middle).toBeGreaterThanOrEqual(name.top);
		expect(middle).toBeLessThanOrEqual(name.bottom);
		// Right beside it — at exactly the From row's distance, not across the
		// width of the longer line under it.
		const fromGap = from.value.getBoundingClientRect().left - from.face.right;
		expect(name.left - to.face.right).toBeCloseTo(fromGap, 0);
		expect(name.left).toBeGreaterThan(to.face.right);
	});

	it('puts whose word the name is and the short address under it, whole, in the body face', async () => {
		const to = await drawn(TO);
		const detail = to.detail as HTMLElement;
		expect(detail.textContent).toBe('Vela User · 0x14fB…eA5c');
		const name = to.value.getBoundingClientRect();
		const line = detail.getBoundingClientRect();
		expect(line.top).toBeGreaterThanOrEqual(name.bottom - 1);
		// Both lines end on the row's right edge.
		expect(Math.abs(line.right - name.right)).toBeLessThanOrEqual(1);
		// Never cut.
		expect(detail.scrollWidth).toBeLessThanOrEqual(Math.ceil(line.width));
		// The body face — the name's — never the mono one.
		const mono = getComputedStyle(document.documentElement).getPropertyValue('--font-mono').trim();
		expect(mono).not.toBe('');
		expect(getComputedStyle(detail).fontFamily).toBe(getComputedStyle(to.value).fontFamily);
		expect(getComputedStyle(detail).fontFamily).not.toBe(mono);
	});

	it('cuts a long name before it moves the line under it or the face off the row', async () => {
		const to = await drawn({ ...TO, value: 'W'.repeat(64) });
		const detail = (to.detail as HTMLElement).getBoundingClientRect();
		const name = to.value.getBoundingClientRect();
		expect(to.value.scrollWidth).toBeGreaterThan(to.value.clientWidth);
		expect(to.face.width).toBeGreaterThan(0);
		expect(name.left).toBeGreaterThan(to.face.right);
		expect(Math.abs(detail.right - name.right)).toBeLessThanOrEqual(1);
	});
});

/**
 * The send confirm's and the transaction detail's Network row drew its chain
 * mark at the inline token size (26, 28 with its ring) beside From and To
 * faces of 20. The row's own rule is that both arts shrink to its one box —
 * Android draws both at 20 too.
 */
describe('FactRow — the network mark is its identicons’ size', () => {
	const NETWORK: FactRowModel = {
		label: 'Network',
		value: 'Base',
		lead: { kind: 'token', mark: { ticker: 'ETH', badgeColor: 'transparent', badgeHidden: true } }
	};

	const box = async (fact: FactRowModel, selector: string) => {
		const screen = render(FactRow, { props: { fact } });
		await tick();
		return screen.container.querySelector(selector) as HTMLElement;
	};

	it('draws the chain mark in the box the From face is drawn in', async () => {
		const face = (await box(FROM, '.identicon')).getBoundingClientRect();
		const token = await box(NETWORK, '.token');
		const mark = token.getBoundingClientRect();
		// The row's art box: the --icon-lg token, read from the page, not a number here.
		const probe = document.createElement('span');
		probe.style.width = 'var(--icon-lg)';
		probe.style.display = 'block';
		document.body.appendChild(probe);
		const artBox = probe.getBoundingClientRect().width;
		probe.remove();
		expect(artBox).toBeGreaterThan(0);
		expect(face.width).toBe(artBox);
		expect(mark.width).toBe(face.width);
		expect(mark.height).toBe(face.height);
		// The ticker drawn under a logo still fits the smaller circle.
		const glyph = token.querySelector<HTMLElement>('.glyph')!;
		expect(glyph.getBoundingClientRect().width).toBeLessThanOrEqual(mark.width);
	});

	it('sits level with the network’s name, beside it as the face sits beside a name', async () => {
		const token = await box(NETWORK, '.token');
		const row = token.closest('.fact') as HTMLElement;
		const name = row.querySelector('.value')!.getBoundingClientRect();
		const mark = token.getBoundingClientRect();
		const middle = (mark.top + mark.bottom) / 2;
		expect(middle).toBeGreaterThanOrEqual(name.top);
		expect(middle).toBeLessThanOrEqual(name.bottom);
		expect(name.left).toBeGreaterThan(mark.right);
	});
});
