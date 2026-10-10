/**
 * An Explore row and an Explore tile are tappable over their WHOLE area — the
 * padding, and the gaps between the mark and the words (PR 3 final note F9).
 *
 * iOS's tile took taps only on its mark and its words: the plain button style
 * hit-tests what is drawn, and the shape that covers the rest was outside it.
 * On the web each is one `<button>`, whose box is its target. This holds that
 * true by pressing where a person's thumb actually lands — the corners, the
 * edges, the gap beside the mark — and seeing each press open the site.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import '../../../app.css';
import type { SiteModel } from '../model';
import SiteRow from './SiteRow.svelte';
import SiteTile from './SiteTile.svelte';

const SITE: SiteModel = {
	id: 'uniswap',
	name: 'Uniswap',
	host: 'app.uniswap.org',
	letter: 'U',
	tint: 'var(--color-accent-base)',
	subtitle: 'app.uniswap.org',
	meta: 'now'
};

/** Points across a box: its corners and edges (1 px in), its middle, and a grid between. */
function probes(box: DOMRect): [number, number][] {
	const xs = [box.left + 1, box.left + box.width * 0.25, box.left + box.width / 2];
	xs.push(box.left + box.width * 0.75, box.right - 1);
	const ys = [box.top + 1, box.top + box.height * 0.25, box.top + box.height / 2];
	ys.push(box.top + box.height * 0.75, box.bottom - 1);
	return xs.flatMap((x) => ys.map((y): [number, number] => [x, y]));
}

/** Press at a point as the browser would: on whatever is drawn there. */
function pressAt(x: number, y: number): Element | null {
	const hit = document.elementFromPoint(x, y);
	hit?.dispatchEvent(new MouseEvent('click', { bubbles: true, clientX: x, clientY: y }));
	return hit;
}

function host(width: string): HTMLElement {
	const el = document.createElement('div');
	el.style.cssText = `position:absolute;inset-inline-start:0;top:0;width:${width};`;
	document.body.appendChild(el);
	return el;
}

describe('an Explore row is one target, edge to edge', () => {
	it('every press inside its box opens the site — padding and gaps included', () => {
		const onopen = vi.fn();
		const target = host('320px');
		render(SiteRow, { target, props: { site: SITE, onopen } });
		const row = target.querySelector('button.row') as HTMLButtonElement;
		const box = row.getBoundingClientRect();
		// The row is as wide as its list, and taller than its words: there IS
		// padding to miss.
		expect(box.width).toBeCloseTo(target.getBoundingClientRect().width, 0);
		const words = (row.querySelector('.name') as HTMLElement).getBoundingClientRect();
		expect(box.height).toBeGreaterThan(words.height * 2);

		const points = probes(box);
		// The gap between the mark and the words, at the words' own height.
		const mark = row.firstElementChild!.getBoundingClientRect();
		points.push([(mark.right + words.left) / 2, box.top + box.height / 2]);
		for (const [x, y] of points) {
			const hit = pressAt(x, y);
			expect(hit && row.contains(hit), `(${x.toFixed(0)}, ${y.toFixed(0)})`).toBe(true);
		}
		expect(onopen).toHaveBeenCalledTimes(points.length);
		expect(onopen).toHaveBeenLastCalledWith('uniswap');
		target.remove();
	});
});

describe('an Explore tile is one target, edge to edge', () => {
	it('every press inside its cell opens the site — beside the mark, and between the mark and the name', () => {
		const onopen = vi.fn();
		// A quarter of a phone's row, as the favourites grid lays it out.
		const target = host('84px');
		render(SiteTile, { target, props: { tile: { kind: 'site', site: SITE }, onopen } });
		const tile = target.querySelector('button.tile') as HTMLButtonElement;
		const box = tile.getBoundingClientRect();
		const mark = tile.firstElementChild!.getBoundingClientRect();
		const name = (tile.querySelector('.label') as HTMLElement).getBoundingClientRect();
		// The tile fills its cell, and the mark does not: there is room beside
		// the mark, and a gap under it, to miss.
		expect(box.width).toBeCloseTo(target.getBoundingClientRect().width, 0);
		expect(box.width).toBeGreaterThan(mark.width + 8);
		expect(name.top).toBeGreaterThan(mark.bottom);

		const points = probes(box);
		points.push([box.left + 2, mark.top + mark.height / 2]); // beside the mark
		points.push([box.right - 2, mark.top + mark.height / 2]);
		points.push([box.left + box.width / 2, (mark.bottom + name.top) / 2]); // the gap
		points.push([box.left + 2, name.top + name.height / 2]); // beside the name
		for (const [x, y] of points) {
			const hit = pressAt(x, y);
			expect(hit && tile.contains(hit), `(${x.toFixed(0)}, ${y.toFixed(0)})`).toBe(true);
		}
		expect(onopen).toHaveBeenCalledTimes(points.length);
		expect(onopen).toHaveBeenLastCalledWith('uniswap');
		target.remove();
	});

	it('the "add" tile too', () => {
		const onopen = vi.fn();
		const target = host('84px');
		render(SiteTile, { target, props: { tile: { kind: 'add', label: 'Add' }, onopen } });
		const tile = target.querySelector('button.tile') as HTMLButtonElement;
		const points = probes(tile.getBoundingClientRect());
		for (const [x, y] of points) pressAt(x, y);
		expect(onopen).toHaveBeenCalledTimes(points.length);
		expect(onopen).toHaveBeenLastCalledWith('add');
		target.remove();
	});
});
