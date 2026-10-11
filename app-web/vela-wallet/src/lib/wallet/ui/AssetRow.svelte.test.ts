/**
 * A hidden holding, as the row DRAWS it (device walk of 2026-10-11, item 3).
 *
 * `privacy-fixture.test.ts` holds every builder to the core's shared fixture;
 * a builder's output is a model, and the worth's mask is drawn by this
 * component, not carried in it. So the last step is checked where it happens:
 * each holding of the fixture's hidden balance, through the live builder and
 * this row, in a browser — and the text a person reads is the core's two
 * strings, "•••• xDAI" and "••••".
 */
import '$lib/tokens/tokens.css';
import { beforeAll, describe, expect, it } from 'vitest';
import { render } from 'vitest-browser-svelte';
import { loadCore } from '$lib/core/client';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { WalletMessages } from '../messages';
import { liveAssetRow } from '../live';
import fixture from '../../../../../../rust/crates/vela-core/tests/fixtures/privacy-hidden.json';
import AssetRow from './AssetRow.svelte';

const FIXTURE = fixture as unknown as {
	holdings_masked: { symbol: string; amount: string; worth: string }[];
	shown: { balance: BalanceView };
	hidden: { balance: BalanceView };
};
const USD = { code: 'USD', rate: 1, committed: true, pending: null };
/** The row reads one string of the page's copy: what an unpriced token says. */
const m = { balance: { noPrice: 'No price' } } as unknown as WalletMessages;

beforeAll(() => loadCore());

function drawn(balance: BalanceView): { amount: string; worth: string }[] {
	return balance.tokens.map((token) => {
		const screen = render(AssetRow, {
			props: { row: liveAssetRow(token, USD, m, balance.hidden) }
		});
		const read = (selector: string) =>
			screen.container.querySelector(selector)?.textContent?.trim() ?? '';
		const cells = { amount: read('.balance'), worth: read('.fiat') };
		screen.unmount();
		return cells;
	});
}

describe('a holdings row, hidden', () => {
	it('reads the core’s masked amount — the coin kept — and the bare mask for its worth', () => {
		expect(drawn(FIXTURE.hidden.balance)).toEqual(
			FIXTURE.holdings_masked.map(({ amount, worth }) => ({ amount, worth }))
		);
	});

	it('sets only the dots wide — the coin’s name reads as a word', () => {
		const balance = FIXTURE.hidden.balance;
		const screen = render(AssetRow, {
			props: { row: liveAssetRow(balance.tokens[0], USD, m, balance.hidden) }
		});
		const cell = screen.container.querySelector('.balance')!;
		const dots = cell.querySelector('.dots')!;
		expect(dots.textContent).toBe('••••');
		expect(parseFloat(getComputedStyle(dots).letterSpacing)).toBeGreaterThan(0);
		// The coin is the cell's own text, at the cell's own spacing.
		expect(getComputedStyle(cell).letterSpacing).toBe('normal');
		// One line, as the shown row is.
		const line = parseFloat(getComputedStyle(cell).lineHeight);
		if (Number.isFinite(line)) expect(cell.getBoundingClientRect().height).toBeLessThan(line * 1.5);
	});

	it('shown, draws the figures the masks stand for', () => {
		expect(drawn(FIXTURE.shown.balance)).toEqual([
			{ amount: '418.25', worth: '$418.25' },
			{ amount: '376.5432', worth: '$376.54' }
		]);
	});
});
