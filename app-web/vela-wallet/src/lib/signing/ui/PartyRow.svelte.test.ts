/**
 * The whole address, inside the sheet, at the side panel's width (spec 089).
 *
 * Forty hex digits have no break opportunity: at 360 px the recipient's full
 * address ran past the sheet's padding and its last characters — the ones a
 * person compares against address poisoning — were clipped by the panel.
 */
import { tick } from 'svelte';
import { afterEach, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import PartyRow from './PartyRow.svelte';

const PANEL_PX = 360;
const PAD_PX = 24;
const ADDRESS = '0x4D4d4D4d4d4D4D4d4D4D4D4d4d4d4d4D4D4d4d4D';

let host: HTMLDivElement | null = null;

afterEach(() => {
	host?.remove();
	host = null;
});

describe('the party row at the side panel’s width (089)', () => {
	for (const scale of [1, 1.35]) {
		it(`draws every character of the address inside the sheet (text ×${scale})`, async () => {
			host = document.createElement('div');
			host.style.cssText = `width:${PANEL_PX}px;padding:0 ${PAD_PX}px;box-sizing:border-box;--text-scale:${scale};`;
			document.body.appendChild(host);
			render(PartyRow, {
				target: host,
				props: { label: 'Recipient', name: '0x4D4d4D…4d4d4D', address: ADDRESS }
			});
			await tick();
			await document.fonts.ready;
			const right = host.getBoundingClientRect().right - PAD_PX;
			const address = host.querySelector<HTMLElement>('.address')!;
			expect(address.textContent).toBe(ADDRESS);
			const range = document.createRange();
			range.selectNodeContents(address);
			for (const rect of range.getClientRects())
				expect(rect.right).toBeLessThanOrEqual(right + 0.5);
		});
	}
});
