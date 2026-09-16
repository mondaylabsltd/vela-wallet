/**
 * The third column's live details (2026-09-05): a tapped asset row opens ITS
 * token, a tapped activity row opens ITS transaction — never the fixture's
 * BNB and USDT, which is what both columns showed until now.
 */
import { describe, expect, it } from 'vitest';
import type { BalanceToken } from '$lib/core/generated/BalanceToken';
import type { BalanceView } from '$lib/core/generated/BalanceView';
import type { FeedItem } from '$lib/core/generated/FeedItem';
import type { FeedTxRecord } from '$lib/core/generated/FeedTxRecord';
import type { FeedTxStatus } from '$lib/core/generated/FeedTxStatus';
import type { FeedView } from '$lib/core/generated/FeedView';
import { resolveWalletFlowMessages, resolveWalletMessages } from '$lib/i18n/engine.server';
import { buildDesktopFlowState, buildFlowState } from '$lib/flows/fixtures';
import { buildDesktopState } from './fixtures';
import { balanceTokenId, withLiveWalletDesktop } from './live';
import {
	feedItemAt,
	feedItemStatus,
	findFeedItem,
	liveTxDetail,
	shownTxDetailStateDesktop,
	shownTxDetailStateMobile,
	withLiveTxDetailDesktop,
	withLiveTxDetailMobile
} from './live-detail';

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
	it('a folded split row lists its recipients under the facts (spec 038 #D2)', () => {
		const alice = '0x' + 'cd'.repeat(20);
		const bob = '0x' + 'ef'.repeat(20);
		const detail = liveTxDetail(
			item('s', {
				direction: 'out',
				counterparty: null,
				value: '0.5',
				batch: {
					kind: 'split',
					count: 2,
					total_usd: 1500,
					transfers: [
						{
							to: alice,
							to_name: 'Alice',
							value: '0.2',
							symbol: 'ETH',
							decimals: 18,
							usd_value: 600,
							logo_urls: null
						},
						{
							to: bob,
							to_name: null,
							value: '0.3',
							symbol: 'ETH',
							decimals: 18,
							usd_value: 900,
							logo_urls: null
						}
					],
					ids: ['s1', 's2'],
					from: '0x' + 'a1'.repeat(20),
					chain_id: 1,
					timestamp: 1_700_000_000,
					status: 'confirmed',
					tx_hash: '0x' + 'c3'.repeat(32),
					user_op_hash: '0xop',
					symbol: 'ETH',
					logo_urls: null,
					to: null,
					to_name: null
				}
			}),
			ctx
		);
		expect(detail.breakdownTitle).toContain('2');
		expect(detail.breakdown?.map((row) => row.label)).toEqual([
			'Alice',
			expect.stringMatching(/^0xef/)
		]);
		expect(detail.breakdown?.[0].identiconSvg).toBeTruthy();
		// No single "To" fact: the row has no one counterparty.
		expect(detail.facts.some((fact) => fact.lead?.kind === 'identicon')).toBe(false);
	});

	const ctx = {
		m: fm,
		wm: m,
		currency: USD,
		hidden: false,
		status: 'confirmed' as const,
		identicon: IDENTICON
	};

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

/**
 * Issue 211: a send that paid its gas in a coin the account did not hold was
 * listed as "Confirmed" in the Transaction Details panel while nothing had
 * landed on chain and no balance had moved. The panel stamped the confirmed
 * chip on EVERY row; the record's own lifecycle was on the wire the whole
 * time, and the desktop shell has been reading it since it was wired.
 */
describe('the status a transaction detail reports (issue 211)', () => {
	const ctx = {
		m: fm,
		wm: m,
		currency: USD,
		hidden: false,
		status: 'confirmed' as const,
		identicon: IDENTICON
	};

	function record(id: string, status: FeedTxStatus): FeedTxRecord {
		return {
			id,
			user_op_hash: '0xop',
			tx_hash: '',
			from: '0x' + 'a1'.repeat(20),
			to: '0x' + 'b1'.repeat(20),
			to_name: null,
			value: '1.25',
			symbol: 'ETH',
			decimals: 18,
			logo_urls: null,
			chain_id: 1,
			timestamp: 1_700_000_000,
			day_start_ms: 0,
			status,
			kind: 'send',
			usd: null
		};
	}

	it('a submitted send that has not landed reads Pending, not Confirmed', () => {
		const feed: FeedView = { ...FEED, transactions: [record('a', 'pending')] };
		const detail = liveTxDetail(item('a'), { ...ctx, status: feedItemStatus(feed, item('a')) });
		expect(detail.status.text).toBe(fm['componentsTx.detail.statusPending']);
		expect(detail.status.tone).toBe('info');
	});

	it('a definite refusal reads Failed', () => {
		const feed: FeedView = { ...FEED, transactions: [record('a', 'failed')] };
		expect(feedItemStatus(feed, item('a'))).toBe('failed');
		const detail = liveTxDetail(item('a'), { ...ctx, status: 'failed' });
		expect(detail.status.text).toBe(fm['componentsTx.detail.statusFailed']);
		expect(detail.status.tone).toBe('error');
	});

	it('a settled one still reads Confirmed', () => {
		const feed: FeedView = { ...FEED, transactions: [record('a', 'confirmed')] };
		expect(feedItemStatus(feed, item('a'))).toBe('confirmed');
		expect(liveTxDetail(item('a'), ctx).status.text).toBe(
			fm['componentsTx.receipt.statusConfirmed']
		);
	});

	it("a folded batch row answers with the group's status, not a record id", () => {
		// The repro was a split to two recipients: the row's id is the shared
		// user_op_hash, so no record matches it and the lookup alone would
		// report the settled reading forever.
		const split = item('0xop', {
			direction: 'out',
			counterparty: null,
			batch: {
				kind: 'split',
				count: 2,
				total_usd: 0,
				transfers: [],
				ids: ['s1', 's2'],
				from: '0x' + 'a1'.repeat(20),
				chain_id: 137,
				timestamp: 1_700_000_000,
				status: 'pending',
				tx_hash: '',
				user_op_hash: '0xop',
				symbol: 'pUSD',
				logo_urls: null,
				to: null,
				to_name: null
			}
		});
		const feed: FeedView = {
			...FEED,
			transactions: [record('s1', 'pending'), record('s2', 'pending')]
		};
		expect(feedItemStatus(feed, split)).toBe('pending');
	});

	it('a row with no local record keeps the settled reading', () => {
		// An incoming transfer the chain already carries has no lifecycle of
		// its own to report.
		expect(feedItemStatus(FEED, item('a'))).toBe('confirmed');
		expect(feedItemStatus(FEED, undefined)).toBe('confirmed');
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

/**
 * Issue #213: a Transaction Details panel drew the mocks' "+120 USDT received
 * from 0x9F3c…21aE" for an account whose Activity list was empty. The panel is
 * prerendered WITH that transaction in it, and the live layer only replaced it
 * when the selection resolved — so switching accounts under an open detail (or
 * anything else that takes the record away) left the drawn one on screen.
 */
describe('a transaction detail with no record behind it (issue #213)', () => {
	const drawnDesktop = buildDesktopFlowState('da2', fm, IDENTICON);
	const drawnMobile = buildFlowState('a2', fm, IDENTICON);
	const ctx = {
		m: fm,
		wm: m,
		currency: USD,
		hidden: false,
		status: 'confirmed' as const,
		identicon: IDENTICON
	};

	it('the drawn states really do carry the mocks transaction', () => {
		// Guards the premise: if the fixture ever stops holding a transaction,
		// the rest of this block is testing nothing.
		if (drawnDesktop.body.kind !== 'tx-detail' || drawnMobile.sheet?.kind !== 'tx-detail') {
			throw new Error('a2 / da2 no longer draw a transaction');
		}
		expect(drawnDesktop.body.model.amount).toBe('+120 USDT');
		expect(drawnMobile.sheet.model.amount).toBe('+120 USDT');
	});

	it('is never the state the wallet shows: the list it came from is', () => {
		expect(shownTxDetailStateDesktop('da2', undefined)).toBe('da1');
		expect(shownTxDetailStateMobile('a2', undefined)).toBe('a1');
	});

	it('stays the transaction screen while the record resolves', () => {
		const detail = liveTxDetail(item('a'), ctx);
		expect(shownTxDetailStateDesktop('da2', detail)).toBe('da2');
		expect(shownTxDetailStateMobile('a2', detail)).toBe('a2');
	});

	it('leaves every other state alone', () => {
		expect(shownTxDetailStateDesktop('dsd3', undefined)).toBe('dsd3');
		expect(shownTxDetailStateDesktop(undefined, undefined)).toBeUndefined();
		expect(shownTxDetailStateMobile('r1', undefined)).toBe('r1');
		expect(shownTxDetailStateMobile(undefined, undefined)).toBeUndefined();
	});

	it("the phone's sheet closes rather than standing the drawn one in", () => {
		expect(withLiveTxDetailMobile(drawnMobile, undefined).sheet).toBeUndefined();
	});

	it('the live record replaces the drawn one on both layouts', () => {
		const detail = liveTxDetail(item('a'), ctx);
		const desktop = withLiveTxDetailDesktop(drawnDesktop, detail);
		const mobile = withLiveTxDetailMobile(drawnMobile, detail);
		if (desktop.body.kind !== 'tx-detail' || mobile.sheet?.kind !== 'tx-detail') {
			throw new Error('the transaction screen lost its transaction');
		}
		expect(desktop.body.model.amount).toBe('+1.25 ETH');
		expect(mobile.sheet.model.amount).toBe('+1.25 ETH');
	});
});
