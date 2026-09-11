/**
 * The third column's live details (2026-09-05): a tapped asset row opens ITS
 * token, a tapped activity row opens ITS transaction — never the fixture's
 * BNB and USDT, which is what both columns showed until now.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletFlowMessages, resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildDesktopState } from './fixtures';
import { balanceTokenId, withLiveWalletDesktop } from './live';
import { feedItemAt, findFeedItem, liveTxDetail } from './live-detail';

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const USD = { code: 'USD', rate: 1, committed: true };
const IDENTICON = () => '<svg></svg>';

function token(
	chain_id: number,
	symbol: string,
	token_address: string | null = null
): BalanceToken {
	return {
		chain_id,
		symbol,
		name: `${symbol} coin`,
		balance: '0.5',
		decimals: 18,
		token_address,
		price_usd: 2000,
		spam: false
	};
}

function item(id: string, partial: Partial<FeedItem> = {}): FeedItem {
	return {
		id,
		direction: 'in',
		counterparty: '0x' + 'b1'.repeat(20),
		alias: null,
		value: '1.25',
		symbol: 'ETH',
		decimals: 18,
		usd_value: 2500,
		chain_id: 1,
		timestamp: 1_700_000_000,
		day_start_ms: 0,
		tx_hash: '0x' + 'c3'.repeat(32),
		batch: null,
		...partial
	};
}

const FEED: FeedView = {
	transactions: [],
	new_item_id: null,
	toast: null,
	rows: [
		{ type: 'header', id: 'day-1', day_start_ms: 1, timestamp: 1 },
		{ type: 'item', item: item('a') },
		{ type: 'item', item: item('b', { chain_id: 42161, direction: 'out', alias: 'Alice' }) },
		{ type: 'header', id: 'day-0', day_start_ms: 0, timestamp: 0 },
		{ type: 'item', item: item('c', { symbol: 'USDT' }) }
	]
};

function view(tokens: BalanceToken[]): BalanceView {
	return {
		address: null,
		display_total_usd: 1000,
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
		cached_total_usd: 1000,
		switcher: { open: false, loading: false, balances: [] }
	};
}

describe('naming a feed item', () => {
	it('by id, from the row the tap carried', () => {
		expect(findFeedItem(FEED, 'b')?.alias).toBe('Alice');
		expect(findFeedItem(FEED, 'nope')).toBeUndefined();
		expect(findFeedItem(null, 'a')).toBeUndefined();
	});

	it('by (group, row), the way the history screen counts', () => {
		expect(feedItemAt(FEED, 0, 1)?.id).toBe('b');
		expect(feedItemAt(FEED, 1, 0)?.id).toBe('c');
		expect(feedItemAt(FEED, 1, 1)).toBeUndefined();
	});
});

describe('liveTxDetail', () => {
	const ctx = { m: fm, wm: m, currency: USD, hidden: false, identicon: IDENTICON };

	it('words the tapped transaction, not the fixture one', () => {
		const detail = liveTxDetail(
			item('b', { direction: 'out', alias: 'Alice', chain_id: 42161 }),
			ctx
		);
		expect(detail.title).toBe(fm['history.txLabelSent'].replace('{{symbol}}', 'ETH'));
		expect(detail.amount).toBe('−1.25 ETH');
		expect(detail.positive).toBe(false);
		expect(detail.facts.map((f) => f.label)).toEqual([
			fm['componentsTx.detail.to'],
			fm['componentsTx.detail.labelChain'],
			fm['componentsTx.detail.labelDate'],
			fm['componentsTx.detail.labelHash']
		]);
		// A named counterparty reads as its name, in the UI face; an address in mono.
		expect(detail.facts[0].value).toBe('Alice');
		expect(detail.facts[0].mono).toBe(false);
		expect(detail.facts[1].value).toBe('Arbitrum');
	});

	it('masks the money while privacy hides it', () => {
		const detail = liveTxDetail(item('a'), { ...ctx, hidden: true });
		expect(detail.amount).not.toContain('1.25');
		expect(detail.fiat).not.toContain('2');
	});
});

describe('the asset column', () => {
	const base = buildDesktopState('d1', m, IDENTICON);
	const held = [token(1, 'ETH'), token(1, 'USDT', '0x' + 'd4'.repeat(20))];

	it('opens on the tapped token with its own facts and transactions', () => {
		const usdt = held[1];
		const model = withLiveWalletDesktop(base, {
			balance: view(held),
			currency: USD,
			m,
			feed: FEED,
			selectedToken: usdt
		});
		expect(model.initialPanel).toBe('asset-detail');
		expect(model.panels.assetDetail.title).toBe('USDT');
		expect(model.panels.assetDetail.token.balance).toBe('0.5 USDT');
		expect(model.panels.assetDetail.facts.map((f) => f.value)).toEqual([
			'USDT coin',
			m.assetDetail.priceValue.replace('{{symbol}}', 'USDT').replace('{{value}}', '$2,000.00'),
			'0xd4d4d4…d4d4d4',
			'18'
		]);
		// Only USDT's own rows, on its own chain.
		expect(model.panels.assetDetail.rows).toHaveLength(1);
		expect(model.assetRows.map((row) => row.id)).toEqual(held.map(balanceTokenId));
	});

	it('stays closed, and drawn, when nothing is selected', () => {
		const model = withLiveWalletDesktop(base, {
			balance: view(held),
			currency: USD,
			m,
			feed: FEED
		});
		expect(model.initialPanel).toBe('none');
		expect(model.panels.assetDetail).toBe(base.panels.assetDetail);
	});
});
