/**
 * The sidebar's network filter — the phone app's `selectedChainId`
 * (`HoldingsList.tsx`, `useHomeController.ts`) on the desktop overlay.
 *
 * Three things worth pinning: the rows come from what is HELD (the board's
 * "8 · 1 · 3 · 1…" were a fixture wallet's counts, drawn on every live page
 * until now); a chosen chain narrows the holdings and the feed; and the hero
 * total never narrows — it is the whole wallet's, as it is on the phone.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildDesktopState } from './fixtures';
import { liveChainRows, withLiveWalletDesktop } from './live';

const m = resolveWalletMessages('en');
const IDENTICON = () => '<svg></svg>';
const USD = { code: 'USD', rate: 1, committed: true };

function token(chain_id: number, symbol: string): BalanceToken {
	return {
		chain_id,
		symbol,
		name: symbol,
		balance: '1',
		decimals: 18,
		token_address: null,
		price_usd: 100,
		spam: false
	};
}

function view(tokens: BalanceToken[]): BalanceView {
	return {
		address: '0x' + 'a1'.repeat(20),
		display_total_usd: 300,
		balance_unknown: false,
		balance_partial: false,
		unreachable: false,
		notice: null,
		hidden: false,
		refreshing: false,
		last_refreshed_at_ms: 0,
		tokens,
		unpriced_tokens: [],
		failed_chain_ids: [],
		rate_limited_chain_ids: [],
		banner_chain_ids: [],
		holdings_loading: false,
		cached_total_usd: 300,
		switcher: { open: false, loading: false, balances: [] }
	};
}

function item(id: string, chain_id: number): FeedItem {
	return {
		id,
		direction: 'in',
		counterparty: '0x' + 'b1'.repeat(20),
		alias: null,
		value: '1',
		symbol: 'ETH',
		decimals: 18,
		usd_value: 100,
		chain_id,
		timestamp: 1_700_000_000,
		day_start_ms: 0,
		tx_hash: '0xabc',
		batch: null
	};
}

const HELD = view([token(1, 'ETH'), token(1, 'USDT'), token(56, 'BNB')]);

const FEED: FeedView = {
	transactions: [],
	new_item_id: null,
	toast: null,
	rows: [
		{ type: 'header', id: 'day-1', day_start_ms: 1, timestamp: 1 },
		{ type: 'item', item: item('a', 1) },
		{ type: 'item', item: item('b', 56) },
		{ type: 'header', id: 'day-0', day_start_ms: 0, timestamp: 0 },
		{ type: 'item', item: item('c', 1) }
	]
};

describe('liveChainRows', () => {
	it('lists 全部 and then every chain with a holding, counted in tokens', () => {
		const rows = liveChainRows(HELD, 'All networks', null);
		expect(rows.map((row) => [row.name, row.count, row.chainId])).toEqual([
			['All networks', 3, null],
			['Ethereum', 2, 1],
			['BNB Chain', 1, 56]
		]);
		expect(rows.map((row) => row.selected)).toEqual([true, false, false]);
	});

	it('ticks the chosen chain', () => {
		const rows = liveChainRows(HELD, 'All networks', 56);
		expect(rows.map((row) => row.selected)).toEqual([false, false, true]);
	});

	it("is only 全部 while nothing is held yet — never the board's counts", () => {
		expect(liveChainRows(view([]), 'All networks', null)).toHaveLength(1);
	});
});

describe('the desktop overlay under a filter', () => {
	const base = buildDesktopState('d1', m, IDENTICON);

	it('narrows the holdings and the feed, and drops a day left empty', () => {
		const filtered = withLiveWalletDesktop(base, {
			balance: HELD,
			currency: USD,
			m,
			feed: FEED,
			chainFilter: 56
		});
		expect(filtered.assetRows.map((row) => row.ticker)).toEqual(['BNB']);
		// Two days in the feed, one item on BNB Chain: one day survives.
		expect(filtered.activityGroups).toHaveLength(1);
		expect(filtered.activityGroups[0].rows).toHaveLength(1);
		expect(filtered.sidebar.networks.find((row) => row.selected)?.chainId).toBe(56);
	});

	it('keeps the hero total whole — the filter is a lens, not a sub-wallet', () => {
		const whole = withLiveWalletDesktop(base, { balance: HELD, currency: USD, m, feed: FEED });
		const narrowed = withLiveWalletDesktop(base, {
			balance: HELD,
			currency: USD,
			m,
			feed: FEED,
			chainFilter: 1
		});
		expect(narrowed.balance).toEqual(whole.balance);
		expect(whole.assetRows).toHaveLength(3);
		expect(narrowed.assetRows).toHaveLength(2);
	});

	it('a chain filtered down to nothing reads as the empty state, not a blank list', () => {
		const nothing = withLiveWalletDesktop(base, {
			balance: HELD,
			currency: USD,
			m,
			feed: FEED,
			chainFilter: 137
		});
		expect(nothing.assetRows).toEqual([]);
		expect(nothing.assetsSection.mode).toBe('empty');
	});
});
