/**
 * r16-21: a batch row's full address ("Run by 0x9641…102e2") broke wherever
 * the line ran out — "…C7C8 / 102e2", "…ab3169 / 2D". As on the desktop
 * (r15-10), a full address may break only between its two halves (22 + 20),
 * never inside one: at every width each half is one line.
 */
import '$lib/tokens/tokens.css';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import KeyValueRows from './KeyValueRows.svelte';

const FULL = '0x9641d764fc13c8B624c04430C7356C1C7C8102e2';

/** The lines `node`'s text is laid on: one per distinct top. */
function lines(node: Node): number {
	const range = document.createRange();
	range.selectNodeContents(node);
	const tops = new Set([...range.getClientRects()].map((r) => Math.round(r.top)));
	return tops.size;
}

describe('KeyValueRows', () => {
	for (const width of [220, 260, 300, 340, 400]) {
		it(`breaks a full address only between its halves at ${width}px`, () => {
			const host = document.createElement('div');
			host.style.width = `${width}px`;
			document.body.appendChild(host);
			const screen = render(KeyValueRows, {
				target: host,
				props: { rows: [{ label: 'Run by', value: FULL, mono: true }] }
			});
			const dd = screen.container.querySelector('dd')!;
			expect(dd.textContent).toBe(FULL);
			const halves = [...dd.querySelectorAll('.address-half')];
			expect(halves.map((h) => h.textContent)).toEqual([
				'0x9641d764fc13c8B624c0',
				'4430C7356C1C7C8102e2'
			]);
			for (const half of halves) expect(lines(half)).toBe(1);
			// Two lines at most: the halves, never a digit alone.
			expect(lines(dd)).toBeLessThanOrEqual(2);
			host.remove();
		});
	}

	it('draws a value that is not an address as it was', () => {
		const screen = render(KeyValueRows, {
			props: { rows: [{ label: 'Network', value: 'Arc Testnet' }] }
		});
		const dd = screen.container.querySelector('dd')!;
		expect(dd.textContent).toBe('Arc Testnet');
		expect(dd.querySelector('.address-half')).toBeNull();
	});
});
