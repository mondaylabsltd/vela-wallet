/**
 * The balance breakdown draws a heading with the rows it is about, and not
 * without them (PR 3 final note F20), in a real browser.
 *
 * "Networks still updating — These networks couldn't be reached, so your
 * cached balance is shown until they recover." headed an EMPTY list: the sheet
 * a person opens from "Some tokens couldn't be priced." told a healthy wallet
 * that networks could not be reached and its balance was cached.
 */
import { describe, expect, it, vi } from 'vitest';
import { render } from 'vitest-browser-svelte';
import '$lib/tokens/tokens.css';
import type { BalanceDetailModel } from '../model';
import BalanceDetailBody from './BalanceDetailBody.svelte';

const mark = (letter: string) => ({ letter, color: 'var(--color-fg-subtle)' });

const PENDING = 'Networks still updating';
const NOTE =
	"These networks couldn't be reached, so your cached balance is shown until they recover.";
const DONE = 'Updated';
const UNPRICED = "Some tokens couldn't be priced.";

const panel = (parts: Partial<BalanceDetailModel>): BalanceDetailModel => ({
	title: 'Balance breakdown',
	summary: 'Total $4,500.00',
	sectionPending: PENDING,
	pendingNote: NOTE,
	pending: [],
	sectionDone: DONE,
	done: [],
	sectionUnpriced: UNPRICED,
	unpriced: [],
	...parts
});

const ETHEREUM_DONE = { id: '1', mark: mark('E'), name: 'Ethereum', amount: '$4,500.00' };
const TEMPO_OUT = {
	id: '4217',
	mark: mark('T'),
	name: 'Tempo',
	status: 'Token list unavailable',
	tone: 'error' as const,
	action: 'Retry now'
};
const BASE_LIMITED = {
	id: '8453',
	mark: mark('B'),
	name: 'Base',
	status: 'Rate-limited · retrying automatically',
	tone: 'neutral' as const
};

describe('the balance breakdown — a heading stands over its rows, or not at all', () => {
	it('every network answered: nothing says a network could not be reached', async () => {
		const screen = render(BalanceDetailBody, {
			props: {
				panel: panel({
					done: [ETHEREUM_DONE],
					unpriced: [{ id: '1:0xabc', mark: mark('E'), name: 'PEPE', detail: 'Ethereum · 1,200' }]
				})
			}
		});
		const text = screen.container.textContent ?? '';
		expect(text).not.toContain(PENDING);
		expect(text).not.toContain(NOTE);
		// What the sheet was opened for is there, under its own name.
		expect(text).toContain(UNPRICED);
		expect(text).toContain('PEPE');
		expect(text).toContain(DONE);
		expect(text).toContain('Ethereum');
		expect(screen.container.querySelectorAll('ul')).toHaveLength(2);
	});

	it('a network out of reach: the heading, its note and the row — with the core’s short status', async () => {
		const onretry = vi.fn();
		const screen = render(BalanceDetailBody, {
			props: {
				panel: panel({ pending: [BASE_LIMITED, TEMPO_OUT], done: [ETHEREUM_DONE] }),
				onretry
			}
		});
		const text = screen.container.textContent ?? '';
		expect(text).toContain(PENDING);
		expect(text).toContain(NOTE);
		expect(text).toContain('Token list unavailable');
		expect(text).toContain('Rate-limited · retrying automatically');
		// A rate limit lifts on its own: no button. The other row has one.
		const buttons = screen.container.querySelectorAll('button');
		expect(buttons).toHaveLength(1);
		buttons[0].click();
		expect(onretry).toHaveBeenCalledWith('4217');
	});

	it('nothing held anywhere yet: no "Updated" heading over nothing either', async () => {
		const screen = render(BalanceDetailBody, { props: { panel: panel({}) } });
		expect(screen.container.querySelectorAll('.section')).toHaveLength(0);
		expect(screen.container.querySelectorAll('ul')).toHaveLength(0);
		expect(screen.container.textContent).toContain('Total $4,500.00');
	});

	it('a short status sits on one line beside its network at 320 px', async () => {
		const host = document.createElement('div');
		host.style.width = '288px';
		document.body.appendChild(host);
		const screen = render(BalanceDetailBody, {
			target: host,
			props: { panel: panel({ pending: [TEMPO_OUT] }) }
		});
		const status = screen.container.querySelector('.status.error') as HTMLElement;
		const lineHeight = parseFloat(getComputedStyle(status).lineHeight);
		const height = status.getBoundingClientRect().height;
		// One line (a face with `normal` line-height reports NaN: compare to the size).
		const oneLine = Number.isFinite(lineHeight)
			? lineHeight
			: parseFloat(getComputedStyle(status).fontSize) * 1.5;
		expect(height).toBeLessThanOrEqual(oneLine + 1);
		screen.unmount();
		host.remove();
	});
});
