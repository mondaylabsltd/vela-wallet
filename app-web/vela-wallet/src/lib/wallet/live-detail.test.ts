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
import { buildDesktopFlowState, buildFlowState } from '$lib/flows/fixtures';
import { buildDesktopState } from './fixtures';
import { balanceTokenId, withLiveWalletDesktop } from './live';
import {
	feedItemAt,
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
		kind: (partial.direction ?? 'in') === 'in' ? 'receive' : 'send',
		status: 'confirmed',
		site: null,
		counterparty_role: 'recipient',
		...partial
	};
}

const FEED: FeedView = {
	transactions: [],
	new_item_id: null,
	toast: null,
	history_empty_key: 'history.emptyTitle',
	home_empty_key: 'home.emptyNoActivity',
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

	// 083 H2: a dApp's transaction opens to what it did and where it came
	// from — the intent as the title rather than "Sent" and an empty coin, the
	// site as the first fact, then the contract — and a call that moved no
	// coin has no figure: no lone "−", no "≈ $0.00".
	it("a dApp's call names its site and intent, and no figure it never moved", () => {
		const call = item('d', {
			direction: 'out',
			value: null,
			symbol: '',
			decimals: null,
			usd_value: 0,
			tx_hash: null,
			status: 'pending',
			dapp: { site: 'app.uniswap.org', intent: 'Swap', intent_term: 'intentSwap' }
		});
		const detail = liveTxDetail(call, ctx);
		expect(detail.title).toBe(m.activity.intents.intentSwap);
		expect(detail.facts.map((f) => f.label).slice(0, 2)).toEqual([
			fm['connect.detail.labelApp'],
			fm['componentsTx.detail.to']
		]);
		expect(detail.facts[0].value).toBe('app.uniswap.org');
		expect(detail.amount).toBe('');
		expect(detail.fiat).toBe('');
		expect(liveTxDetail(call, { ...ctx, hidden: true }).amount).toBe('');

		const undecoded = liveTxDetail(
			item('e', { ...call, dapp: { site: null, intent: null, intent_term: null } }),
			ctx
		);
		expect(undecoded.title).toBe(m.activity.contractCall);
		expect(undecoded.facts[0].label).toBe(fm['componentsTx.detail.to']);
	});
});

/**
 * Issue 211: a send that paid its gas in a coin the account did not hold was
 * listed as "Confirmed" in the Transaction Details panel while nothing had
 * landed on chain and no balance had moved. The panel stamped the confirmed
 * chip on EVERY row; the record's own lifecycle was on the wire the whole
 * time, and the desktop shell has been reading it since it was wired.
 */
describe('the status a transaction detail reports (issue 211, spec 082 RG1)', () => {
	const ctx = {
		m: fm,
		wm: m,
		currency: USD,
		hidden: false,
		identicon: IDENTICON
	};

	it('a submitted send that has not landed reads Pending, not Confirmed', () => {
		const detail = liveTxDetail(item('a', { status: 'pending' }), ctx);
		expect(detail.status.text).toBe(fm['componentsTx.detail.statusPending']);
		expect(detail.status.tone).toBe('info');
	});

	it('a definite refusal reads Failed', () => {
		const detail = liveTxDetail(item('a', { status: 'failed' }), ctx);
		expect(detail.status.text).toBe(fm['componentsTx.detail.statusFailed']);
		expect(detail.status.tone).toBe('error');
	});

	it('a record nothing will settle reads Unknown, draws no hash and keeps a quiet delete (087 F04/F05)', () => {
		const detail = liveTxDetail(
			item('dapp-1790500000796-tx', {
				direction: 'out',
				kind: 'dapp_tx',
				status: 'unknown',
				tx_hash: null
			}),
			ctx
		);
		expect(detail.status).toEqual({ text: 'Unknown', tone: 'info' });
		expect(detail.status.text).toBe(fm['componentsUi.signing.intentUnknown']);
		expect(detail.facts.map((fact) => fact.label)).not.toContain(
			fm['componentsTx.detail.labelHash']
		);
		expect(JSON.stringify(detail.facts)).not.toContain('dapp-1790500000796-tx');
		expect(detail.explorerUrl).toBeUndefined();
		expect(detail.deleteLabel).toBe(fm['history.deleteRecord']);
		expect(detail.deleteQuiet).toBe(true);
	});

	it('a settled one still reads Confirmed', () => {
		expect(liveTxDetail(item('a'), ctx).status.text).toBe(
			fm['componentsTx.receipt.statusConfirmed']
		);
	});

	it('the status is the core’s, not a lookup: a folded batch carries its own', () => {
		// The row's id is the shared user_op_hash, which matches no record id;
		// the core puts the group's status on the item itself.
		const split = item('0xop', {
			direction: 'out',
			kind: 'send',
			status: 'pending',
			counterparty: null
		});
		expect(liveTxDetail(split, ctx).status.text).toBe(fm['componentsTx.detail.statusPending']);
	});

	it('a dApp transaction is titled as one and names the site that asked (RG2)', () => {
		const dapp = item('d', {
			direction: 'out',
			kind: 'dapp_tx',
			status: 'pending',
			site: '127.0.0.1:8137',
			value: null,
			symbol: '',
			tx_hash: null
		});
		const detail = liveTxDetail(dapp, ctx);
		expect(detail.title).toBe(fm['history.txLabelDappTx']);
		expect(detail.facts).toContainEqual({
			label: fm['componentsUi.signing.siweOrigin'],
			value: '127.0.0.1:8137'
		});
		expect(detail.status.text).toBe(fm['componentsTx.detail.statusPending']);
		expect(detail.explorerUrl).toBeUndefined();
	});

	it('a swap’s router is the contract it went to, not its recipient (RJ16, G52)', () => {
		const router = '0x' + '3f'.repeat(20);
		const swap = item('w', {
			direction: 'out',
			kind: 'dapp_tx',
			counterparty: router,
			counterparty_role: 'contract',
			tx_hash: '0x' + 'ab'.repeat(32)
		});
		const facts = liveTxDetail(swap, ctx).facts;
		expect(facts[0].label).toBe(fm['componentsUi.signing.interactingLabel']);
		expect(facts[0].label).not.toBe(fm['componentsTx.detail.to']);
		// A plain transfer's decoded recipient keeps "To".
		const transfer = item('t', {
			direction: 'out',
			kind: 'dapp_tx',
			counterparty: router,
			counterparty_role: 'recipient'
		});
		expect(liveTxDetail(transfer, ctx).facts[0].label).toBe(fm['componentsTx.detail.to']);
	});

	it('no transaction hash: no explorer control; a pending record’s delete is quiet (RJ16, RJ18)', () => {
		const pending = liveTxDetail(item('p', { status: 'pending', tx_hash: null }), ctx);
		expect(pending.explorerUrl).toBeUndefined();
		expect(pending.deleteQuiet).toBe(true);
		const failed = liveTxDetail(item('f', { status: 'failed', tx_hash: null }), ctx);
		expect(failed.explorerUrl).toBeUndefined();
		expect(failed.deleteQuiet).toBe(false);
		expect(liveTxDetail(item('c'), ctx).deleteQuiet).toBe(false);
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
