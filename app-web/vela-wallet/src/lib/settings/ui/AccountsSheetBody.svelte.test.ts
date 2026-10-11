/**
 * The account switcher's rows keep their height whatever the balance's
 * length (0.8 — a figure landing moves nothing), in a real browser.
 *
 * While the display currency is on its way a row's balance is the pending
 * mark; then the figure lands. At 320 px a long one — "₫112,500,000.00" —
 * squeezed the name's column to a word's width: the name broke over two
 * lines, the address over two more, the wrapped words ran under the figure,
 * and the row grew 33 px, taking the bottom-anchored sheet's heading with it.
 */
import { tick } from 'svelte';
import { describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { AccountsSheetModel } from '../model';
import AccountsSheetBody from './AccountsSheetBody.svelte';

const sheet = (amount: string, summary: string): AccountsSheetModel => ({
	title: 'Accounts',
	summary,
	rows: [
		{
			name: 'Everyday wallet',
			addressDisplay: '0x0cE19C…084e2e',
			addressFull: '0x0cE19Cc09A0b561B1AB9ee3B88C93685F5084e2e',
			identiconSvg: '<svg viewBox="0 0 8 8"></svg>',
			amount,
			selected: true
		},
		{
			name: 'Savings',
			addressDisplay: '0x9F3cA7…8021aE',
			addressFull: '0x9F3cA71b04E82f5C55d9B21aE00734F8Dd8021aE',
			identiconSvg: '<svg viewBox="0 0 8 8"></svg>',
			amount,
			selected: false
		}
	],
	primary: 'Create New Account',
	secondary: 'Sign In with Existing'
});

const PENDING = sheet('…', '2 accounts · Total …');
const LANDED = sheet('₫112,500,000.00', '2 accounts · Total ₫225,000,000.00');

describe('the account switcher — a balance landing moves nothing', () => {
	for (const width of ['390px', '320px', '280px']) {
		it(`at ${width}: the rows are as tall with a long figure as with the pending mark, and nothing overlaps`, async () => {
			const host = document.createElement('div');
			host.style.width = width;
			document.body.appendChild(host);
			const screen = render(AccountsSheetBody, { target: host, props: { sheet: PENDING } });
			const rows = () => [...host.querySelectorAll<HTMLElement>('li.account')];
			const heights = () => rows().map((row) => row.getBoundingClientRect().height);
			const tops = () => rows().map((row) => row.getBoundingClientRect().top);
			const before = {
				heights: heights(),
				tops: tops(),
				body: host.getBoundingClientRect().height
			};

			await screen.rerender({ sheet: LANDED });
			await tick();
			expect(heights(), 'row heights').toEqual(before.heights);
			expect(tops(), 'row tops').toEqual(before.tops);
			expect(host.getBoundingClientRect().height, 'the sheet body').toBe(before.body);

			for (const row of rows()) {
				const name = row.querySelector('.name') as HTMLElement;
				const address = row.querySelector('.address') as HTMLElement;
				const amount = row.querySelector('.amount') as HTMLElement;
				// The figure is whole, on one line, inside the row…
				expect(amount.textContent).toBe('₫112,500,000.00');
				expect(amount.scrollWidth).toBeLessThanOrEqual(amount.clientWidth + 1);
				expect(amount.getBoundingClientRect().right).toBeLessThanOrEqual(
					host.getBoundingClientRect().right + 0.5
				);
				// …and the name and the address give way to it on ONE line each,
				// never running under it.
				for (const text of [name, address]) {
					const box = text.getBoundingClientRect();
					expect(box.right, text.className).toBeLessThanOrEqual(
						amount.getBoundingClientRect().left + 0.5
					);
					expect(box.height, text.className).toBeLessThan(
						parseFloat(getComputedStyle(text).fontSize) * 2
					);
				}
			}
			screen.unmount();
			host.remove();
		});
	}
});
