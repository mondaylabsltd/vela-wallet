/**
 * The site row's second line, in a browser (spec 099 navigation).
 *
 * A resume row names an open tab by its host, and a host too long for the
 * row must be cut from its START — the pill's rule: the end of a host is
 * the registrable domain, the part that decides who you are talking to.
 * `app.uniswap.org.evil.xyz` cut at its end reads `app.uniswap.or…`, which
 * is Uniswap to anybody glancing at it. Only layout can show which end was
 * cut, so this measures where the characters land.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { SiteModel } from '../model';
import SiteRow from './SiteRow.svelte';

const NARROW = 180;

/** A host built to read as Uniswap when cut at its end. */
const PHISH = 'app.uniswap.org.evil.xyz';

function site(host: string): SiteModel {
	return {
		id: 't1',
		name: 'Uniswap Interface',
		host,
		letter: 'U',
		tint: 'var(--color-accent-base)',
		subtitle: host
	};
}

async function drawRow(host: string, hostLine: boolean) {
	const screen = render(SiteRow, { props: { site: site(host), hostLine } });
	screen.container.style.width = `${NARROW}px`;
	await tick();
	const line = screen.container.querySelector('.sub') as HTMLElement;
	const text = (line.querySelector('bdi') ?? line).firstChild as Text;
	return { line, text };
}

/** Where one character of `text` sits on screen. */
function charBox(text: Text, at: number): DOMRect {
	const range = document.createRange();
	range.setStart(text, at);
	range.setEnd(text, at + 1);
	return range.getBoundingClientRect();
}

describe('SiteRow host line', () => {
	it('cuts a long host from its start, so its registrable domain shows', async () => {
		const { line, text } = await drawRow(PHISH, true);
		const box = line.getBoundingClientRect();
		expect(line.scrollWidth).toBeGreaterThan(line.clientWidth);
		const last = charBox(text, PHISH.length - 1);
		const first = charBox(text, 0);
		// The end of the host is inside the line, at its right edge…
		expect(last.right).toBeLessThanOrEqual(box.right + 0.5);
		expect(last.left).toBeGreaterThanOrEqual(box.left);
		// …and its start is what was cut away.
		expect(first.right).toBeLessThan(box.left);
		expect(getComputedStyle(line).textOverflow).toBe('ellipsis');
	});

	it('keeps a host that fits where any second line sits, at the row start', async () => {
		const { line, text } = await drawRow('app.aave.com', true);
		const box = line.getBoundingClientRect();
		expect(line.scrollWidth).toBeLessThanOrEqual(line.clientWidth);
		expect(charBox(text, 0).left).toBeCloseTo(box.left, 0);
		expect(line.textContent).toBe('app.aave.com');
	});

	it('keeps a port after its host', async () => {
		const { line } = await drawRow('localhost:8080', true);
		const text = line.querySelector('bdi')?.firstChild as Text;
		expect(charBox(text, 0).left).toBeLessThan(charBox(text, 'localhost:8080'.length - 1).left);
	});

	it('leaves any other second line cut at its end', async () => {
		const { line, text } = await drawRow(PHISH, false);
		const box = line.getBoundingClientRect();
		expect(line.querySelector('bdi')).toBeNull();
		expect(charBox(text, 0).left).toBeCloseTo(box.left, 0);
		expect(charBox(text, PHISH.length - 1).left).toBeGreaterThan(box.right);
	});
});
