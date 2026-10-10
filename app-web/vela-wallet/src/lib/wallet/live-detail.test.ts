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
import { buildSigningRecord } from '$lib/services/dapp-history';
import { dappRequestDisplay } from '$lib/core/kernels';
import type { TxTechnicalRow } from '$lib/flows/model';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { feedItemsThroughCore } from './core/feed-through-core';
import { withFigureMaskable } from './testing/figure-maskable';
import { dappActivityRecords, feedDapp } from './dapp-activity-fixtures';
import { buildDesktopState } from './fixtures';
import { balanceTokenId, withLiveWalletDesktop } from './live';
import {
	feedItemAt,
	findFeedItem,
	liveTxDetail,
	shownTxDetailStateDesktop,
	shownTxDetailStateMobile,
	storedRequestJson,
	withLiveTxDetailDesktop,
	withLiveTxDetailMobile
} from './live-detail';

const m = resolveWalletMessages('en');
const fm = resolveWalletFlowMessages('en');
const USD = { code: 'USD', rate: 1, committed: true, pending: null };
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
	return withFigureMaskable({
		id,
		direction: 'in',
		counterparty: '0x' + 'b1'.repeat(20),
		alias: null,
		value: '1.25',
		symbol: 'ETH',
		decimals: 18,
		usd_value: 2500,
		priced: true,
		chain_id: 1,
		timestamp: 1_700_000_000,
		day_start_ms: 0,
		tx_hash: '0x' + 'c3'.repeat(32),
		batch: null,
		kind: (partial.direction ?? 'in') === 'in' ? 'receive' : 'send',
		status: 'confirmed',
		site: null,
		counterparty_role: 'recipient',
		subtitle: [],
		...partial
	});
}

const FEED: FeedView = {
	transactions: [],
	new_item_id: null,
	toast: null,
	history_empty_key: 'history.emptyTitle',
	home_empty_key: 'home.emptyNoActivity',
	hidden: false,
	contact_rows: [],
	// The detail is opened from History's list (`rows`); the home's cut plays
	// no part in naming a tap.
	home_rows: [],
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
		unreachable_networks: [],
		unreachable_key: null,
		internal_chain_ids: [],
		internal_key: null,
		checking_key: null,
		live_key: null,
		empty_key: null,
		holdings_loading: false,
		cached_total_usd: 1000,
		switcher: { open: false, loading: false, balances: [], hidden: false }
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

	// The network fact names a NETWORK, so it wears the network's own logo —
	// as the confirm page's does. It was the chain's letters only, and on the
	// shells that took the coin's rule, Ethereum's logo beside "Base".
	it('ETH sent on Base: the network fact wears Base, not Ethereum and not letters', () => {
		const detail = liveTxDetail(item('b', { direction: 'out', chain_id: 8453 }), ctx);
		const network = detail.facts.find((f) => f.label === fm['componentsTx.detail.labelChain']);
		expect(network?.value).toBe('Base');
		expect(network?.lead).toEqual({
			kind: 'token',
			mark: expect.objectContaining({
				ticker: 'ETH',
				logoUrls: ['https://ethereum-data.getvela.app/chainlogos/eip155-8453.png'],
				badgeHidden: true
			})
		});
	});

	// A sweep's record names each coin's logos but not its contract: the
	// chain's own coin still gets the core's logo after them; a token it cannot
	// place keeps what it named — never a logo guessed from its ticker.
	it('a sweep lists its coins with the logos the record named, the own coin’s added', () => {
		const named = 'https://named.example/usdc.png';
		const coin = (symbol: string, logo_urls: string[] | null) => ({
			to: '0x' + 'cd'.repeat(20),
			to_name: null,
			value: '1',
			symbol,
			decimals: 18,
			usd_value: 1,
			logo_urls
		});
		const detail = liveTxDetail(
			item('w', {
				direction: 'out',
				counterparty: null,
				chain_id: 8453,
				batch: {
					kind: 'multi_select',
					count: 3,
					total_usd: 3,
					transfers: [coin('ETH', null), coin('USDC', [named]), coin('DEGEN', null)],
					ids: ['w1', 'w2', 'w3'],
					from: '0x' + 'a1'.repeat(20),
					chain_id: 8453,
					timestamp: 1_700_000_000,
					status: 'confirmed',
					tx_hash: '0x' + 'c3'.repeat(32),
					user_op_hash: '0xop',
					symbol: null,
					logo_urls: null,
					to: '0x' + 'cd'.repeat(20),
					to_name: null
				}
			}),
			ctx
		);
		expect(detail.breakdown?.map((row) => [row.label, row.lead?.logoUrls])).toEqual([
			['ETH', ['https://ethereum-data.getvela.app/chainlogos/eip155-1.png']],
			['USDC', [named]],
			['DEGEN', undefined]
		]);
	});

	// A sweep — several coins, one payee, one operation — has no one figure and
	// no one coin: the core sends neither. Its detail read a lone "−" over the
	// list, under "Sent " with nothing after it.
	it('through the core: a sweep says how many coins left and lists each, never a lone minus', async () => {
		const ACCOUNT = '0x' + 'a1'.repeat(20);
		const PAYEE = '0x' + 'cd'.repeat(20);
		const NOW_S = 1_700_000_000;
		const line = (
			n: number,
			symbol: string,
			value: string,
			usd: string,
			decimals = 18
		): LocalTransaction => ({
			id: `sweep-${n}`,
			userOpHash: '0x' + 'ab'.repeat(32),
			txHash: '0x' + 'c3'.repeat(32),
			from: ACCOUNT,
			to: PAYEE,
			value,
			symbol,
			decimals,
			chainId: 8453,
			timestamp: NOW_S - 60,
			status: 'confirmed',
			type: 'send',
			usd
		});
		const items = await feedItemsThroughCore(
			[line(1, 'ETH', '0.001', '$3.00'), line(2, 'USDC', '5', '$5.00', 6)],
			ACCOUNT,
			NOW_S * 1000
		);
		expect(items).toHaveLength(1);
		const sweep = items[0];
		expect(sweep.batch?.kind).toBe('multi_select');
		expect([sweep.value, sweep.symbol]).toEqual([null, '']);

		const detail = liveTxDetail(sweep, ctx);
		expect(detail.title).toBe(fm['componentsTx.detail.sent']);
		expect(detail.amount).toBe('2 assets');
		expect(detail.fiat).toBe('≈ $8.00');
		expect(detail.breakdown?.map((row) => [row.label, row.value])).toEqual([
			['ETH', '0.001 ETH'],
			['USDC', '5 USDC']
		]);
		// Privacy hides the count with the money, as the Activity row does. A
		// sweep's hero names no one coin, so its mask stands alone…
		const hidden = liveTxDetail(sweep, { ...ctx, hidden: true });
		expect(hidden.amount).toBe('••••');
		// …and each coin it swept is masked too, keeping its unit. These were
		// drawn in full under the masked hero.
		expect(hidden.breakdown?.map((row) => [row.label, row.value])).toEqual([
			['ETH', '•••• ETH'],
			['USDC', '•••• USDC']
		]);
		expect(hidden.fiat).toBe('••••');
		// A single send still reads as one figure and its coin.
		expect(liveTxDetail(item('b', { direction: 'out' }), ctx)).toMatchObject({
			title: 'Sent ETH',
			amount: '−1.25 ETH'
		});
	}, 30_000);

	// PR 3 item 12: a hidden transfer's detail read "•••• xDAI" on iOS and a
	// bare "••••" on Android and here. One rule now: the amount is masked and
	// its unit is kept — what kind of money, never how much.
	it('masks the money while privacy hides it — and keeps the coin', () => {
		const detail = liveTxDetail(item('a'), { ...ctx, hidden: true });
		expect(detail.amount).toBe('•••• ETH');
		expect(detail.amount).not.toContain('1.25');
		// No sign either: which way it went is the title's to say.
		expect(detail.amount).not.toMatch(/[+−-]/);
		// A fiat worth has no unit apart from its figure: the bare mask.
		expect(detail.fiat).toBe('••••');
		const sent = liveTxDetail(item('b', { direction: 'out', symbol: 'xDAI' }), {
			...ctx,
			hidden: true
		});
		expect(sent.amount).toBe('•••• xDAI');
		// Shown, nothing changes.
		expect(liveTxDetail(item('a'), ctx).amount).toBe('+1.25 ETH');
	});

	it('a hidden split masks every recipient’s share, each with its coin', async () => {
		const BOB = '0x' + 'b0'.repeat(20);
		const CAROL = '0x' + 'ca'.repeat(20);
		const part = (n: number, to: string, value: string): LocalTransaction => ({
			id: `split-${n}`,
			userOpHash: '0x' + 'cd'.repeat(32),
			txHash: '0x' + 'e5'.repeat(32),
			from: ACCOUNT,
			to,
			value,
			symbol: 'USDC',
			decimals: 6,
			chainId: 8453,
			timestamp: NOW_S - 60,
			status: 'confirmed',
			type: 'send',
			usd: `$${value}`
		});
		const items = await feedItemsThroughCore(
			[part(1, BOB, '37.5'), part(2, CAROL, '12.25')],
			ACCOUNT,
			NOW_S * 1000
		);
		expect(items).toHaveLength(1);
		const split = items[0];
		expect(split.batch?.kind).toBe('split');

		const shown = liveTxDetail(split, ctx);
		expect(shown.breakdown?.map((row) => row.value)).toEqual(['37.5 USDC', '12.25 USDC']);

		const hidden = liveTxDetail(split, { ...ctx, hidden: true });
		expect(hidden.breakdown?.map((row) => row.value)).toEqual(['•••• USDC', '•••• USDC']);
		// Who was paid is not money: the recipients are still named.
		expect(hidden.breakdown?.map((row) => row.address)).toEqual([BOB, CAROL]);
		// Nothing on the whole detail says how much.
		const text = JSON.stringify(hidden);
		for (const figure of ['37.5', '12.25', '49.75']) expect(text).not.toContain(figure);
	}, 30_000);

	// 083 H2, spec 093: a dApp's call opens to what it did and where — the
	// core's facts in its order, labelled here — and a call that moved no
	// coin has no figure: no lone "−", no "≈ $0.00".
	it("a dApp's call draws the core's facts, and no figure it never moved", () => {
		const router = '0x' + '3f'.repeat(20);
		const call = item('d', {
			direction: 'out',
			kind: 'dapp_tx',
			value: null,
			symbol: '',
			decimals: null,
			usd_value: 0,
			tx_hash: null,
			status: 'pending',
			dapp: feedDapp({
				site: 'app.uniswap.org',
				intent: 'Swap',
				intent_term: 'intentSwap',
				place: 'Uniswap',
				facts: [
					{ type: 'site', site: 'app.uniswap.org' },
					{ type: 'network', chain_id: 1 },
					{ type: 'contract', address: router, name: 'Uniswap Universal Router' },
					{ type: 'date', timestamp: 1_700_000_000 }
				],
				technical: [
					{ type: 'operation', operation: { type: 'contract_interaction' } },
					{ type: 'content', content: 'call_data' },
					{ type: 'user_op_hash', hash: '0x' + 'e1'.repeat(32) }
				]
			})
		});
		const detail = liveTxDetail(call, ctx);
		expect(detail.title).toBe('Swap on Uniswap');
		expect(detail.status?.text).toBe(fm['componentsTx.detail.statusPending']);
		expect(detail.note).toBeUndefined();
		expect(detail.facts.map((f) => [f.label, f.value])).toEqual([
			[fm['connect.detail.labelApp'], 'app.uniswap.org'],
			[fm['componentsTx.detail.labelChain'], 'Ethereum'],
			// A noun for a record of something done (083 F3 review).
			[fm['tokenDetail.labelContract'], 'Uniswap Universal Router'],
			[fm['componentsTx.detail.labelDate'], expect.any(String)]
		]);
		expect(detail.facts[2]).toMatchObject({ copyValue: router, mono: false });
		expect(detail.amount).toBe('');
		expect(detail.fiat).toBe('');
		expect(liveTxDetail(call, { ...ctx, hidden: true }).amount).toBe('');
		expect(detail.explorerUrl).toBeUndefined();
		const technical = detail.technical!;
		expect(technical.title).toBe(fm['componentsUi.signing.advancedToggle']);
		expect(technical.rows.map(rowLabel)).toEqual([
			fm['componentsTx.detail.labelOperation'],
			fm['connect.detail.contentCallData'],
			fm['componentsTx.receipt.userOpHash']
		]);
		expect(technical.rows[0]).toMatchObject({
			fact: { value: fm['componentsTx.detail.opContractInteraction'] }
		});
		expect(technical.rows[2]).toMatchObject({ fact: { copyValue: '0x' + 'e1'.repeat(32) } });
	});

	// Spec 097 N7: unknown is not zero.
	it('an amount with no known price has no fiat figure — never "≈ $0.00"', () => {
		const sent = liveTxDetail(item('p', { direction: 'out', usd_value: 0, priced: false }), ctx);
		expect(sent.amount).toBe('−1.25 ETH');
		expect(sent.fiat).toBe('');
		const order = item('o', {
			direction: 'out',
			kind: 'dapp_tx',
			value: '0.003',
			symbol: 'BNB',
			usd_value: 0,
			priced: false,
			dapp: feedDapp({ intent: 'Create order', place: '1inch' })
		});
		const detail = liveTxDetail(order, ctx);
		expect(detail.title).toBe('Create order on 1inch');
		expect(detail.amount).toBe('−0.003 BNB');
		expect(detail.fiat).toBe('');
		expect(liveTxDetail(item('q'), ctx).fiat).toBe('≈ $2,500.00');
	});

	// Spec 097 N5: a borrow moved nothing out; what it brought in is its figure.
	it('a row with nothing out leads with what came back', () => {
		const borrow = item('b', {
			direction: 'out',
			kind: 'dapp_tx',
			value: null,
			symbol: '',
			decimals: null,
			usd_value: 0,
			priced: false,
			dapp: feedDapp({
				intent: 'Borrow',
				intent_term: 'intentBorrow',
				place: 'Aave',
				received: {
					direction: 'in',
					verified: true,
					symbol: 'USDC',
					value: '0.3',
					decimals: 18,
					exact: true
				}
			})
		});
		const detail = liveTxDetail(borrow, ctx);
		expect(detail.amount).toBe('+0.3 USDC');
		expect(detail.positive).toBe(true);
		expect(detail.received).toBeUndefined();
		expect(detail.fiat).toBe('');
	});

	// Spec 097 N4: a failed operation says why, under its chip.
	it('a failed dApp row says why it failed', () => {
		const failed = (failure: 'reverted' | 'refused' | 'not_sent') =>
			liveTxDetail(
				item('f', {
					direction: 'out',
					kind: 'dapp_tx',
					status: 'failed',
					value: null,
					tx_hash: null,
					dapp: feedDapp({ intent: 'Withdraw', intent_term: 'intentWithdraw', failure })
				}),
				ctx
			);
		const refused = failed('refused');
		expect(refused.status?.text).toBe(fm['componentsTx.detail.statusFailed']);
		expect(refused.note).toBe(fm['componentsUi.signing.refused']);
		expect(refused.note).toBe('The network refused it — nothing was sent.');
		expect(failed('reverted').note).toBe(fm['componentsTx.receipt.failedHint']);
		expect(failed('not_sent').note).toBe(fm['send.txErrorGeneric']);
	});

	it('the stored request is read by record id when asked — and only then', () => {
		const asked: string[] = [];
		const detail = liveTxDetail(
			item('dapp-7-tx', {
				direction: 'out',
				kind: 'dapp_tx',
				dapp: feedDapp({ technical: [{ type: 'content', content: 'call_data' }] })
			}),
			{
				...ctx,
				storedRequest: (id) => {
					asked.push(id);
					return id === 'dapp-7-tx' ? '[{"to":"0x1"}]' : '';
				}
			}
		);
		// Building the detail asks nothing.
		expect(asked).toEqual([]);
		const content = detail.technical!.rows[0];
		if (content.kind !== 'content') throw new Error('no content row');
		expect(content.missing).toBe(fm['connect.detail.contentMissing']);
		// Shown as the core words it (`dappRequestDisplay`): call data, pretty.
		expect(content.read()).toBe(dappRequestDisplay('call_data', '[{"to":"0x1"}]'));
		expect(content.read()).toBe('[\n  {\n    "to": "0x1"\n  }\n]');
		expect(asked).toEqual(['dapp-7-tx', 'dapp-7-tx']);
		// No reader (or no record): none to show.
		const bare = liveTxDetail(
			item('x', { dapp: feedDapp({ technical: [{ type: 'content', content: 'message' }] }) }),
			ctx
		).technical!.rows[0];
		expect(bare.kind === 'content' && bare.read()).toBeNull();
	});

	it("a stored request is its kept params' JSON, unformatted; an empty or absent one is ''", () => {
		const tx = dappActivityRecords(ACCOUNT, NOW_S)[2];
		expect(storedRequestJson(tx)).toBe(JSON.stringify(tx.signedRequest!.params));
		expect(storedRequestJson({ ...tx, signedRequest: { method: 'x', params: [] } })).toBe('');
		expect(storedRequestJson({ ...tx, signedRequest: undefined })).toBe('');
		expect(storedRequestJson(undefined)).toBe('');
	});

	// Spec 093: what Technical details shows is the core's reading of the
	// kept request, asked with the content word and the params' JSON.
	it('the fixtures read as the core shows them: a pretty document, a message as its text', () => {
		const [signIn, permit] = dappActivityRecords(ACCOUNT, NOW_S);
		const read = (tx: typeof permit, content: 'typed_data' | 'message') => {
			const row = liveTxDetail(
				item(tx.id, { dapp: feedDapp({ technical: [{ type: 'content', content }] }) }),
				{ ...ctx, storedRequest: () => storedRequestJson(tx) }
			).technical!.rows[0];
			if (row.kind !== 'content') throw new Error('no content row');
			return row.read();
		};
		const typed = read(permit, 'typed_data')!;
		expect(typed).toBe(dappRequestDisplay('typed_data', storedRequestJson(permit)));
		expect(typed).toContain('"primaryType": "PermitSingle"');
		expect(typed).not.toContain('\\"');
		const message = read(signIn, 'message')!;
		expect(message.startsWith('app.uniswap.org wants you to sign in')).toBe(true);
	});

	// Spec 093: the core keeps `""` when the request's shape alone is past the
	// cut — "not recorded", never a drawn "[]".
	it('a request the core kept nothing of reads "not recorded", not "[]"', () => {
		const kept = buildSigningRecord({
			method: 'wallet_sendCalls',
			params: [{ calls: [] }],
			storedRequest: '',
			requestTruncated: true,
			result: '',
			from: ACCOUNT,
			chainId: 1,
			dappOrigin: 'https://app.example',
			nowMs: NOW_S * 1000
		});
		expect(storedRequestJson(kept)).toBe('');
		expect(dappRequestDisplay('call_data', '')).toBeNull();
		const content = liveTxDetail(
			item(kept.id, { dapp: feedDapp({ technical: [{ type: 'content', content: 'call_data' }] }) }),
			{ ...ctx, storedRequest: () => storedRequestJson(kept) }
		).technical!.rows[0];
		if (content.kind !== 'content') throw new Error('no content row');
		expect(content.read()).toBeNull();
		expect(content.missing).toBe(fm['connect.detail.contentMissing']);
	});

	// Spec 093 / 082 RJ16: who got the money is a recipient — "To", by the
	// row's name for them — never "the contract" a payment was called on.
	it('through the core: a dApp’s plain send names its recipient, by name', async () => {
		const ALICE = '0x' + 'a1'.repeat(20);
		const [send] = await feedItemsThroughCore(
			[
				{
					id: 'dapp-1789999000000-tx',
					userOpHash: '0x' + 'e2'.repeat(32),
					txHash: '0x' + 'f2'.repeat(32),
					from: ACCOUNT,
					to: ALICE,
					toName: 'Alice',
					value: '0x2386f26fc10000',
					symbol: 'ETH',
					decimals: 18,
					chainId: 1,
					timestamp: NOW_S - 30,
					status: 'confirmed',
					type: 'dapp_tx',
					dappOrigin: 'https://pay.example',
					dappUrl: 'https://pay.example',
					intent: 'Send',
					signedRequest: { method: 'eth_sendTransaction', params: [{ to: ALICE, value: '0x1' }] },
					dappSummary: { action: 'call', calls: 1, contract: ALICE }
				}
			],
			ACCOUNT,
			NOW_S * 1000
		);
		const facts = liveTxDetail(send, ctx).facts;
		const to = facts.find((f) => f.label === fm['componentsTx.detail.to']);
		expect(to).toMatchObject({ value: 'Alice', copyValue: ALICE, mono: false });
		expect(to?.lead?.kind).toBe('identicon');
		// One or the other: no contract fact beside the recipient.
		expect(facts.map((f) => f.label)).not.toContain(fm['tokenDetail.labelContract']);
	}, 30_000);

	// Spec 093: the fixture permit and swap, described by the REAL core.
	it('through the core: a permit is off-chain, states its cap in red and never expires', async () => {
		const [, permit] = await feedItemsThroughCore(
			dappActivityRecords(ACCOUNT, NOW_S),
			ACCOUNT,
			NOW_S * 1000 + 1000
		);
		const detail = liveTxDetail(permit, { ...ctx, storedRequest: () => '["0xabc"]' });
		expect(detail.title).toBe('Spending permit on Uniswap');
		expect(detail.status).toBeUndefined();
		expect(detail.note).toBe(fm['connect.detail.offChainNote']);
		expect(detail.amount).toBe(`${m.activity.unlimited} USDC`);
		expect(detail.danger).toBe(true);
		expect(detail.facts.map((f) => f.label)).toEqual([
			fm['connect.detail.labelApp'],
			fm['componentsTx.detail.labelChain'],
			fm['componentsUi.signing.labelSpender'],
			fm['componentsUi.signingApprove.spendingCap'],
			fm['componentsUi.signingApprove.expiresLabel'],
			fm['componentsTx.detail.labelDate']
		]);
		expect(detail.facts[3]).toMatchObject({
			value: `${m.activity.unlimited} USDC`,
			tone: 'danger'
		});
		expect(detail.facts[4].value).toBe(fm['componentsUi.signingApprove.noExpiry']);
		expect(detail.explorerUrl).toBeUndefined();
		expect(detail.technical!.rows.map(rowLabel)).toEqual([
			fm['componentsTx.detail.labelOperation'],
			fm['connect.detail.contentTypedData'],
			fm['componentsUi.signing.typeLabel']
		]);
		expect(detail.technical!.rows[0]).toMatchObject({
			fact: { value: fm['componentsTx.detail.opTypedDataSignature'] }
		});
		expect(detail.technical!.rows[2]).toMatchObject({ fact: { value: 'PermitSingle' } });
	}, 30_000);

	it('through the core: a swap shows what left, what came back, and its hashes', async () => {
		const [, , swap] = await feedItemsThroughCore(
			dappActivityRecords(ACCOUNT, NOW_S),
			ACCOUNT,
			NOW_S * 1000 + 1000
		);
		const detail = liveTxDetail(swap, ctx);
		expect(detail.title).toBe('Swap on Uniswap');
		expect(detail.status?.text).toBe(fm['componentsTx.receipt.statusConfirmed']);
		expect(detail.amount).toBe('≈ \u2212100 USDC');
		expect(detail.received).toBe('≈ +0.03 ETH');
		// The sheet's lines, one row each, the first under the title.
		const at = detail.facts.findIndex(
			(f) => f.label === fm['componentsUi.signing.balanceChangesTitle']
		);
		const changes = detail.facts.slice(at, at + 2);
		expect(changes.map((f) => f.label)).toEqual([
			fm['componentsUi.signing.balanceChangesTitle'],
			''
		]);
		expect(changes.map((f) => [f.value, f.tone])).toEqual([
			['≈ \u2212100 USDC', undefined],
			['≈ +0.03 ETH', 'success']
		]);
		expect(detail.explorerUrl).toContain('0x' + 'f1'.repeat(32));
		expect(detail.technical!.rows.map(rowLabel)).toEqual([
			fm['componentsTx.detail.labelOperation'],
			fm['connect.detail.contentCallData'],
			fm['componentsTx.detail.labelHash'],
			fm['componentsTx.receipt.userOpHash']
		]);
	}, 30_000);
});

const ACCOUNT = '0xD400866e00B055B20752a826CD5C89b811de130b';
const NOW_S = 1_790_000_000;

function rowLabel(row: TxTechnicalRow): string {
	return row.kind === 'fact' ? row.fact.label : row.label;
}

/**
 * PR 3 — an unverified token is a direction, never a figure.
 *
 * Android's signing sheet printed "Unverified token
 * +5,000,000,000,000,000,000,000.00": the simulation's raw delta, a number
 * the site being signed for chose. The web's sheet draws no balance rows
 * (spec 082 RG6), so the one place this shell prints an unverified judgment
 * is here — a dApp transaction's detail in Activity, from the lines its
 * record stored. The judgment no longer holds the figure; a record written
 * before still does, and the core reads it for its sign and drops it.
 */
describe('an unverified token in a record’s balance changes is a sign and a name, never a figure (PR 3)', () => {
	const ctx = { m: fm, wm: m, currency: USD, hidden: false, identicon: IDENTICON };
	const LURE = '5000000000000000000000';
	const OUT = '-7000000000000000000000';
	const LABEL = fm['componentsUi.signing.balanceUnverifiedToken'];
	const [, , swap] = dappActivityRecords(ACCOUNT, NOW_S);
	const record = (balanceChanges: unknown): LocalTransaction => ({
		...swap,
		balanceChanges: balanceChanges as LocalTransaction['balanceChanges']
	});
	/** The detail of the one record, through the real core. */
	async function detailOf(balanceChanges: unknown) {
		const [row] = await feedItemsThroughCore(
			[record(balanceChanges)],
			ACCOUNT,
			NOW_S * 1000 + 1000
		);
		return liveTxDetail(row, ctx);
	}
	/** The balance-change rows of a detail: the titled one, and the untitled ones after it. */
	function changeRows(detail: ReturnType<typeof liveTxDetail>) {
		const at = detail.facts.findIndex(
			(f) => f.label === fm['componentsUi.signing.balanceChangesTitle']
		);
		if (at === -1) return [];
		const rest = detail.facts.slice(at + 1);
		const end = rest.findIndex((f) => f.label !== '');
		return [detail.facts[at], ...rest.slice(0, end === -1 ? rest.length : end)];
	}
	/** Every string the detail would draw, wherever it sits in the model. */
	function everyString(value: unknown, out: string[] = []): string[] {
		if (typeof value === 'string') out.push(value);
		else if (Array.isArray(value)) value.forEach((entry) => everyString(entry, out));
		else if (value !== null && typeof value === 'object') {
			Object.values(value).forEach((entry) => everyString(entry, out));
		}
		return out;
	}
	/** The figure in any way a formatter writes it: "5000", "5,000", "5 000", "5.000". */
	const figure = (lead: string) => new RegExp(`${lead}[\\s,.\u00a0\u202f']?000`);

	it('the label is the corpus’s, and the search for the figure finds one when it is there', () => {
		expect(LABEL).toBe('Unverified token');
		for (const written of ['+5000000000000000000000', '+5,000,000.00', '5\u202f000', '5.000,00']) {
			expect(figure('5').test(written), written).toBe(true);
		}
		expect(figure('5').test(`+ ${LABEL}`)).toBe(false);
	});

	it.each([
		[
			'a record stored before (the figure still in it)',
			[
				{ type: 'erc20_unverified', token: '0x' + 'ba'.repeat(20), delta: LURE },
				{ type: 'erc20_unverified', token: '0x' + 'cd'.repeat(20), delta: OUT }
			]
		],
		[
			'a record stored since (a direction, no figure)',
			[
				{ type: 'erc20_unverified', token: '0x' + 'ba'.repeat(20), direction: 'in' },
				{ type: 'erc20_unverified', token: '0x' + 'cd'.repeat(20), direction: 'out' }
			]
		]
	])(
		'%s: "+" and "−" beside the label, and no digit run of the figure anywhere',
		async (_, lines) => {
			const detail = await detailOf(lines);
			expect(changeRows(detail).map((f) => [f.value, f.tone])).toEqual([
				[`+ ${LABEL}`, 'success'],
				[`\u2212 ${LABEL}`, undefined]
			]);
			// Not in the rows, and not in the hero, the technical section or any
			// other string of the page.
			const drawn = everyString(detail);
			expect(drawn.filter((text) => figure('5').test(text) || figure('7').test(text))).toEqual([]);
			expect(drawn.some((text) => text.includes(LABEL))).toBe(true);
		},
		30_000
	);

	it('a zero is never drawn, and a line with no direction costs nothing else its place', async () => {
		const detail = await detailOf([
			{ type: 'erc20_unverified', token: '0x' + 'ba'.repeat(20), direction: 'still' },
			{ type: 'erc20_unverified', token: '0x' + 'ba'.repeat(20), delta: '0' },
			{ type: 'erc20_unverified', token: '0x' + 'cd'.repeat(20), delta: LURE },
			{ type: 'erc20_unverified', token: null, direction: 'unreadable' }
		]);
		expect(changeRows(detail).map((f) => f.value)).toEqual([`+ ${LABEL}`]);
		expect(everyString(detail).filter((text) => figure('5').test(text))).toEqual([]);
	}, 30_000);
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
		expect(detail.status?.text).toBe(fm['componentsTx.detail.statusPending']);
		expect(detail.status?.tone).toBe('info');
	});

	it('a definite refusal reads Failed', () => {
		const detail = liveTxDetail(item('a', { status: 'failed' }), ctx);
		expect(detail.status?.text).toBe(fm['componentsTx.detail.statusFailed']);
		expect(detail.status?.tone).toBe('error');
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
		expect(detail.status?.text).toBe(fm['componentsUi.signing.intentUnknown']);
		expect(detail.facts.map((fact) => fact.label)).not.toContain(
			fm['componentsTx.detail.labelHash']
		);
		expect(JSON.stringify(detail.facts)).not.toContain('dapp-1790500000796-tx');
		expect(detail.explorerUrl).toBeUndefined();
		expect(detail.deleteLabel).toBe(fm['history.deleteRecord']);
		expect(detail.deleteQuiet).toBe(true);
	});

	it('a settled one still reads Confirmed', () => {
		expect(liveTxDetail(item('a'), ctx).status?.text).toBe(
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
		expect(liveTxDetail(split, ctx).status?.text).toBe(fm['componentsTx.detail.statusPending']);
	});

	it('a dApp transaction keeps its status chip; a dApp signature has none, and says so (spec 093)', () => {
		const tx = item('d', {
			direction: 'out',
			kind: 'dapp_tx',
			status: 'pending',
			site: '127.0.0.1:8137',
			value: null,
			symbol: '',
			tx_hash: null,
			dapp: feedDapp({ site: '127.0.0.1:8137', place: '127.0.0.1:8137' })
		});
		const detail = liveTxDetail(tx, ctx);
		expect(detail.title).toBe(`${m.activity.intents.intentContractCall} on 127.0.0.1:8137`);
		expect(detail.status?.text).toBe(fm['componentsTx.detail.statusPending']);
		expect(detail.explorerUrl).toBeUndefined();
		const signature = liveTxDetail(
			item('m', {
				direction: 'out',
				kind: 'sign_message',
				value: null,
				symbol: '',
				tx_hash: null,
				dapp: feedDapp({ action: 'message', off_chain: true, intent_term: 'messageIntent' })
			}),
			ctx
		);
		expect(signature.title).toBe(m.activity.intents.messageIntent);
		expect(signature.status).toBeUndefined();
		expect(signature.note).toBe(fm['connect.detail.offChainNote']);
		expect(signature.deleteLabel).toBe(fm['history.deleteRecord']);
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
