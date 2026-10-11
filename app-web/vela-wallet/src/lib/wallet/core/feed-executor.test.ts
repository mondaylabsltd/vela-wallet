/**
 * The web activity_feed executor (spec 025 Phase 4): stored records reach the
 * core in its vocabulary (coerced, never rejected), the raw rows reach the
 * sink, own accounts alias locally, and the failure twin answers every op.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalTransaction } from '$lib/services/transactions-model';
import type { FeedEffect } from './feed-types';
import type { FeedView } from '$lib/core/generated/FeedView';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
vi.mock('$lib/services/activity', async (importOriginal) => {
	const original = await importOriginal<typeof import('$lib/services/activity')>();
	return { ...original, syncReceivedTransfers: vi.fn(async () => 2) };
});

const waterfall = vi.fn(async (addr: string) =>
	addr === '0x' + 'b1'.repeat(20) ? { name: 'vitalik.eth', source: 'ENS' } : null
);
vi.mock('$lib/services/recipient-identity', () => ({
	resolveRecipientIdentity: (addr: string) => waterfall(addr)
}));

import {
	createFeedExecutor,
	firstCallData,
	storedJudgments,
	storedSettlement,
	storedSummary,
	toFeedRecord
} from './feed-executor';

const effect = (operation: FeedEffect['operation']): FeedEffect => ({ id: 1, operation });
const ME = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
const OTHER = '0x' + 'b1'.repeat(20);

const RECEIVED: LocalTransaction = {
	id: '1-0xabc-0',
	userOpHash: '',
	txHash: '0xabc',
	from: OTHER,
	to: ME,
	value: '1.5',
	symbol: 'ETH',
	decimals: 18,
	chainId: 1,
	timestamp: 1_700_000_000,
	status: 'confirmed',
	type: 'receive',
	usd: '$4,500.00'
};

beforeEach(() => kv.clear());

describe('toFeedRecord', () => {
	it('speaks the core vocabulary, day-keyed in the device timezone', () => {
		const record = toFeedRecord(RECEIVED);
		expect(record).toMatchObject({
			id: '1-0xabc-0',
			from: OTHER,
			to: ME,
			value: '1.5',
			symbol: 'ETH',
			chain_id: 1,
			kind: 'receive',
			status: 'confirmed',
			usd: '$4,500.00'
		});
		expect(record?.day_start_ms).toBe(
			new Date(new Date(1_700_000_000 * 1000).setHours(0, 0, 0, 0)).getTime()
		);
	});
	it('a legacy untyped row is kind null (read as send by the core); junk coerces', () => {
		const legacy = toFeedRecord({ ...RECEIVED, type: undefined, decimals: NaN, timestamp: NaN });
		expect(legacy).toMatchObject({ kind: null, decimals: 0, timestamp: 0 });
		expect(toFeedRecord({ ...RECEIVED, type: 'nonsense' as never })).toBeNull();
	});
	it("a dApp's transaction hands the core its origin and intent — never the dApp's own name (083 H2)", () => {
		const dapp: LocalTransaction = {
			...RECEIVED,
			from: ME,
			to: OTHER,
			type: 'dapp_tx',
			value: '0x2386f26fc10000',
			dappOrigin: 'app.uniswap.org',
			dappUrl: 'https://evil.example',
			intent: 'Swap'
		};
		expect(toFeedRecord(dapp)).toMatchObject({
			kind: 'dapp_tx',
			value: '0x2386f26fc10000',
			dapp_url: 'https://evil.example',
			intent: 'Swap'
		});
		// A record from before `dappUrl`: no site rather than the name.
		const older = toFeedRecord({ ...dapp, dappUrl: undefined, intent: undefined });
		expect(older).toMatchObject({ dapp_url: null, intent: null });
		// Every other kind is unchanged on the wire.
		expect(toFeedRecord(RECEIVED)).not.toHaveProperty('dapp_url');
		expect(toFeedRecord({ ...RECEIVED, dappOrigin: 'x' })).not.toHaveProperty('intent');
	});
	it('a figure a page sent as a JSON number still reads as that figure (083 H2)', () => {
		const numeric = { ...RECEIVED, type: 'dapp_tx', value: 10_000_000_000_000_000 };
		expect(toFeedRecord(numeric as unknown as LocalTransaction)?.value).toBe('10000000000000000');
		// Past 2^53 and past 1e21 it is still written out digit for digit.
		const large = { ...numeric, value: 2 ** 70 };
		expect(toFeedRecord(large as unknown as LocalTransaction)?.value).toBe(
			'1180591620717411303424'
		);
		// Not a whole, non-negative number: no figure.
		for (const odd of [-1, 0.5, Number.NaN, Number.POSITIVE_INFINITY]) {
			expect(toFeedRecord({ ...numeric, value: odd } as unknown as LocalTransaction)?.value).toBe(
				''
			);
		}
	});
});

describe('the store and the scan', () => {
	it('read_tx_store hands the raw rows to the sink and the core its records, read_id echoed', async () => {
		kv.set('vela.transactionHistory', JSON.stringify([RECEIVED]));
		const raw: LocalTransaction[][] = [];
		const executor = createFeedExecutor(() => [], { storeLoaded: (records) => raw.push(records) });
		const result = await executor.execute(
			effect({ type: 'read_tx_store', address: ME, read_id: 7 }),
			new AbortController().signal
		);
		expect(raw[0]).toEqual([RECEIVED]);
		expect(result).toMatchObject({ type: 'store_loaded', read_id: 7 });
		if (result.type === 'store_loaded') expect(result.records[0].id).toBe('1-0xabc-0');
	});

	it('scan_incoming_transfers answers the sync count', async () => {
		const executor = createFeedExecutor(() => [], { storeLoaded: () => {} });
		expect(
			await executor.execute(
				effect({ type: 'scan_incoming_transfers', address: ME }),
				new AbortController().signal
			)
		).toEqual({ type: 'sync_completed', new_count: 2 });
	});

	it('delete_tx_record removes the row from the store', async () => {
		kv.set('vela.transactionHistory', JSON.stringify([RECEIVED]));
		const executor = createFeedExecutor(() => [], { storeLoaded: () => {} });
		expect(
			await executor.execute(
				effect({ type: 'delete_tx_record', id: '1-0xabc-0' }),
				new AbortController().signal
			)
		).toEqual({ type: 'delete_committed', id: '1-0xabc-0' });
		expect(JSON.parse(kv.get('vela.transactionHistory')!)).toEqual([]);
	});
});

describe('identity', () => {
	it('an own account aliases locally with no network; anyone else asks the waterfall', async () => {
		const executor = createFeedExecutor(() => [{ address: ME, name: 'Me' }], {
			storeLoaded: () => {}
		});
		const signal = new AbortController().signal;
		expect(
			await executor.execute(effect({ type: 'resolve_recipient_identity', addr: ME }), signal)
		).toEqual({ type: 'alias_resolved', addr: ME, name: 'Me' });
		expect(waterfall).not.toHaveBeenCalled();
		expect(
			await executor.execute(effect({ type: 'resolve_recipient_identity', addr: OTHER }), signal)
		).toEqual({ type: 'alias_resolved', addr: OTHER, name: 'vitalik.eth' });
		const nobody = '0x' + 'c2'.repeat(20);
		expect(
			await executor.execute(effect({ type: 'resolve_recipient_identity', addr: nobody }), signal)
		).toEqual({ type: 'alias_resolved', addr: nobody, name: null });
	});
});

describe('the failure twin', () => {
	it('answers every operation without deciding', () => {
		const executor = createFeedExecutor(() => [], { storeLoaded: () => {} });
		expect(
			executor.toFailure(
				effect({ type: 'read_tx_store', address: ME, read_id: 3 }),
				new Error('io')
			)
		).toMatchObject({ type: 'store_loaded', records: [], read_id: 3 });
		expect(
			executor.toFailure(effect({ type: 'scan_incoming_transfers', address: ME }), new Error('net'))
		).toEqual({ type: 'sync_completed', new_count: 0 });
		expect(
			executor.toFailure(effect({ type: 'timer', ms: 1, generation: 4 }), new Error('x'))
		).toEqual({ type: 'toast_expired', generation: 4 });
	});
});

/**
 * Spec 082 RG1–RG4: a dApp's transaction is a row of its own, from the stored
 * record — its site from `dappOrigin`, its status the record's — and a write
 * shows within one poke (`reconcile_completed`), not the next 10–30 s tick.
 */
describe('a dApp transaction in Activity', () => {
	const LOCAL_HASH = '0x' + 'ab'.repeat(32);
	const DAPP: LocalTransaction = {
		id: `dapp-${LOCAL_HASH}`,
		userOpHash: LOCAL_HASH,
		txHash: '',
		from: ME,
		to: OTHER,
		value: '0x0',
		symbol: 'xDAI',
		decimals: 18,
		chainId: 100,
		timestamp: 1_700_000_000,
		status: 'pending',
		type: 'dapp_tx',
		dappOrigin: 'http://127.0.0.1:8137',
		dappUrl: 'http://127.0.0.1:8137',
		maybeSent: true
	};

	// The site is read from `dappUrl`, the origin the request came from —
	// never `dappOrigin`, which may hold the dApp's own name (083 H2 review).
	it('maps the stored site for the core', () => {
		expect(toFeedRecord(DAPP)).toMatchObject({
			kind: 'dapp_tx',
			status: 'pending',
			dapp_url: 'http://127.0.0.1:8137'
		});
		expect(toFeedRecord({ ...DAPP, dappUrl: undefined })?.dapp_url).toBeNull();
		expect(toFeedRecord(RECEIVED)?.dapp_url).toBeUndefined();
	});

	it('a may-have-been-sent op is a Pending row under its local hash, one poke after the write', async () => {
		await import('$lib/i18n/wasm-init.server');
		const { ActivityFeedCore } = await import('$lib/core/client');
		const core = new ActivityFeedCore();
		type Result = {
			view: FeedView;
			effects: { id: number; operation: { type: string; read_id?: number } }[];
		};
		const dispatch = (event: unknown) => JSON.parse(core.dispatch(JSON.stringify(event))) as Result;
		const resolve = (id: number, result: unknown) =>
			JSON.parse(core.resolve_effect(BigInt(id), JSON.stringify(result))) as Result;
		const readOf = (result: Result) =>
			result.effects.find((e) => e.operation.type === 'read_tx_store');
		const answer = (effect: NonNullable<ReturnType<typeof readOf>>, rows: LocalTransaction[]) =>
			resolve(effect.id, {
				type: 'store_loaded',
				records: rows.map(toFeedRecord).filter((r) => r !== null),
				now_ms: 1_700_000_100_000,
				read_id: effect.operation.read_id
			});
		try {
			const first = readOf(dispatch({ type: 'account_switched', address: ME }));
			expect(first).toBeDefined();
			const empty = answer(first!, []);
			expect(empty.view.rows.filter((r) => r.type === 'item')).toHaveLength(0);

			// The record is written; the executor pokes the feed once.
			const poke = readOf(dispatch({ type: 'reconcile_completed', resolved_count: 1 }));
			expect(poke).toBeDefined();
			const after = answer(poke!, [DAPP]);
			const items = after.view.rows.flatMap((r) => (r.type === 'item' ? [r.item] : []));
			expect(items).toHaveLength(1);
			expect(items[0]).toMatchObject({
				id: DAPP.id,
				kind: 'dapp_tx',
				status: 'pending',
				site: '127.0.0.1:8137'
			});
		} finally {
			core.free();
		}
		// The core's first load, inside the test: 5 s is not enough under a full parallel run.
	}, 30_000);
});

/**
 * Spec 082 RJ16 (T233, G52): a dApp record's detail names who got the money.
 * The stored request's call data reaches the core, which decodes a plain token
 * transfer's real recipient and calls any other call's `to` the contract.
 */
describe('the call data behind a dApp record (RJ16)', () => {
	const ME_ = '0x' + '11'.repeat(20);
	const USDC = '0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83';
	const RECIPIENT = '76875e38fc6bc2dedcaed807ce00782db5c0d141';
	const TRANSFER =
		'0xa9059cbb' + RECIPIENT.padStart(64, '0') + (10n ** 30n).toString(16).padStart(64, '0');
	const record = (signedRequest: LocalTransaction['signedRequest']): LocalTransaction => ({
		id: 'dapp-1-tx',
		userOpHash: '0x' + 'ab'.repeat(32),
		txHash: '',
		from: ME_,
		to: USDC,
		value: '0',
		symbol: 'xDAI',
		decimals: 18,
		chainId: 100,
		timestamp: 1_700_000_000,
		status: 'failed',
		type: 'dapp_tx',
		dappOrigin: 'http://127.0.0.1:8137',
		signedRequest
	});

	it('eth_sendTransaction: its own data; wallet_sendCalls: the first leg’s', () => {
		expect(
			toFeedRecord(
				record({ method: 'eth_sendTransaction', params: [{ to: USDC, data: TRANSFER }] })
			)?.call_data
		).toBe(TRANSFER);
		expect(
			firstCallData({
				method: 'wallet_sendCalls',
				params: [
					{
						calls: [
							{ to: USDC, data: TRANSFER },
							{ to: USDC, data: '0x01' }
						]
					}
				]
			})
		).toBe(TRANSFER);
	});

	it('none for a plain send, an unreadable request, or any other kind', () => {
		expect(
			firstCallData({ method: 'eth_sendTransaction', params: [{ to: USDC, value: '0x1' }] })
		).toBeNull();
		expect(firstCallData({ method: 'eth_sendTransaction', params: [{ data: '0x' }] })).toBeNull();
		expect(
			firstCallData({ method: 'eth_sendTransaction', params: [{ data: 'nothex' }] })
		).toBeNull();
		expect(firstCallData(undefined)).toBeNull();
		expect(toFeedRecord({ ...record(undefined), type: 'send' })?.call_data).toBeNull();
	});

	it('through the core: the DX-W3 record names the transfer’s recipient, and no explorer', async () => {
		await import('$lib/i18n/wasm-init.server');
		const { ActivityFeedCore } = await import('$lib/core/client');
		const core = new ActivityFeedCore();
		type Result = {
			view: FeedView;
			effects: { id: number; operation: { type: string; read_id?: number } }[];
		};
		const dispatch = (event: unknown) => JSON.parse(core.dispatch(JSON.stringify(event))) as Result;
		const resolve = (id: number, result: unknown) =>
			JSON.parse(core.resolve_effect(BigInt(id), JSON.stringify(result))) as Result;
		try {
			const first = dispatch({ type: 'account_switched', address: ME_ }).effects.find(
				(e) => e.operation.type === 'read_tx_store'
			)!;
			const stored = record({
				method: 'eth_sendTransaction',
				params: [{ to: USDC, data: TRANSFER }]
			});
			const after = resolve(first.id, {
				type: 'store_loaded',
				records: [toFeedRecord(stored)],
				now_ms: 1_700_000_100_000,
				read_id: first.operation.read_id
			});
			const items = after.view.rows.flatMap((r) => (r.type === 'item' ? [r.item] : []));
			expect(items).toHaveLength(1);
			expect(items[0].counterparty?.toLowerCase()).toBe(`0x${RECIPIENT}`);
			expect(items[0].counterparty_role).toBe('recipient');
			expect(items[0].tx_hash).toBeNull();
		} finally {
			core.free();
		}
	}, 30_000);
});

/**
 * Spec 093: a dApp's signatures are rows too, so the core hears the same about
 * them as about its transactions — the origin, the recorded intent, the
 * summary and the sheet's balance changes, each as stored. A summary or a
 * list of changes that is not the core's own shape is dropped whole (the row
 * then reads by its kind), never repaired, and never faults the feed.
 */
describe('a dApp record’s summary and balance changes (spec 093)', () => {
	const SUMMARY = {
		action: 'permit',
		calls: 0,
		contract: '0x000000000022d473030f116ddee9f6b43ac78ba3',
		spender: '0x3fc91a3afd70395cd496c647d5a6cc9d4b2b7fad',
		token: '0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48',
		symbol: 'USDC',
		decimals: 6,
		unlimited: true,
		primary_type: 'PermitSingle'
	} as const;
	const SIGNATURE: LocalTransaction = {
		id: 'dapp-1-typed',
		userOpHash: '',
		txHash: '',
		from: ME,
		to: '',
		value: '0',
		symbol: '',
		decimals: 0,
		chainId: 1,
		timestamp: 1_700_000_000,
		status: 'confirmed',
		type: 'sign_typed_data',
		dappOrigin: 'Uniswap Interface',
		dappUrl: 'https://app.uniswap.org',
		intent: 'Permit',
		dappSummary: { ...SUMMARY }
	};

	it('a signature hands the core its origin, intent and summary — never the dApp’s name', () => {
		expect(toFeedRecord(SIGNATURE)).toMatchObject({
			kind: 'sign_typed_data',
			dapp_url: 'https://app.uniswap.org',
			intent: 'Permit',
			summary: SUMMARY,
			balance_changes: null,
			call_data: null
		});
		const message = toFeedRecord({
			...SIGNATURE,
			type: 'sign_message',
			dappUrl: undefined,
			dappSummary: { action: 'sign_in', calls: 0, signin_domain: 'app.uniswap.org' }
		});
		expect(message).toMatchObject({
			kind: 'sign_message',
			dapp_url: null,
			summary: { action: 'sign_in', calls: 0, signin_domain: 'app.uniswap.org' }
		});
		// A record from before 093: no summary, read by its kind.
		expect(toFeedRecord({ ...SIGNATURE, dappSummary: undefined })?.summary).toBeNull();
		// Transfers say none of it.
		expect(toFeedRecord(RECEIVED)).not.toHaveProperty('summary');
	});

	it('a summary of any other shape is dropped whole, never repaired', () => {
		expect(storedSummary(SUMMARY)).toEqual(SUMMARY);
		for (const bad of [
			null,
			'permit',
			[SUMMARY],
			{ ...SUMMARY, action: 'steal' },
			{ ...SUMMARY, calls: -1 },
			{ ...SUMMARY, calls: 1.5 },
			{ ...SUMMARY, decimals: 2 ** 40 },
			{ ...SUMMARY, unlimited: 'yes' },
			{ ...SUMMARY, spender: 42 },
			{ ...SUMMARY, expires_at: Number.NaN }
		]) {
			expect(storedSummary(bad), JSON.stringify(bad)).toBeNull();
		}
		// Absent optional fields are absent, not invented.
		expect(storedSummary({ action: 'message' })).toEqual({ action: 'message', calls: 0 });
	});

	// PR 3: the shape of an unverified token's line changed (a `delta` then, a
	// `direction` now), and this side's own decoder asked every line for a
	// `delta` — it would have dropped the lines of every record the newer
	// build writes. There is one reader now, the core's, and it reads both.
	it('balance changes go to the core as stored, in either shape', () => {
		const lines = [
			{ type: 'native', delta: '-1' },
			{
				type: 'erc20_trusted',
				token: '0xa0b8',
				delta: '5',
				symbol: 'USDC',
				decimals: 6,
				in_trusted_set: true
			},
			{ type: 'erc20_unverified', token: null, direction: 'in' }
		];
		// As written before PR 3: the unverified line still holds its figure.
		const older = [...lines.slice(0, 2), { type: 'erc20_unverified', token: null, delta: '7' }];
		for (const stored of [lines, older]) {
			expect(storedJudgments(stored)).toBe(stored);
			expect(
				toFeedRecord({
					...SIGNATURE,
					type: 'dapp_tx',
					balanceChanges: stored as LocalTransaction['balanceChanges']
				})?.balance_changes
			).toBe(stored);
		}
		// No list is no list.
		for (const none of [[], 'x', null, undefined, { 0: lines[0] }]) {
			expect(storedJudgments(none), JSON.stringify(none)).toBeNull();
		}
	});

	it('through the core: both shapes make their lines, and a list it cannot read costs the lines, never the row', async () => {
		const { feedItemsThroughCore } = await import('./feed-through-core');
		const record = (id: string, balanceChanges: unknown): LocalTransaction => ({
			...SIGNATURE,
			id,
			type: 'dapp_tx',
			txHash: '0x' + 'f1'.repeat(32),
			dappSummary: { action: 'call', calls: 1 },
			balanceChanges: balanceChanges as LocalTransaction['balanceChanges']
		});
		const LURE = '5000000000000000000000';
		const items = await feedItemsThroughCore(
			[
				record('dapp-1-tx', [{ type: 'erc20_unverified', token: '0xbad', delta: LURE }]),
				record('dapp-2-tx', [{ type: 'erc20_unverified', token: '0xbad', direction: 'out' }]),
				// Not a judgment at all, and one line of a kind nobody knows.
				record('dapp-3-tx', [{ type: 'native', delta: 1 }]),
				record('dapp-4-tx', [
					{ type: 'native', delta: '-1' },
					{ type: 'nft', delta: '1' }
				])
			],
			ME,
			1_700_000_100_000
		);
		const changes = Object.fromEntries(items.map((it) => [it.id, it.dapp?.changes ?? null]));
		// The older record keeps its line — as a direction, its figure gone.
		expect(changes['dapp-1-tx']).toEqual([
			{ direction: 'in', verified: false, symbol: '', value: null, decimals: null }
		]);
		expect(changes['dapp-2-tx']).toEqual([
			{ direction: 'out', verified: false, symbol: '', value: null, decimals: null }
		]);
		expect(JSON.stringify(items)).not.toContain('5000');
		// All four records are rows; the two unreadable lists say nothing.
		expect(Object.keys(changes).sort()).toEqual([
			'dapp-1-tx',
			'dapp-2-tx',
			'dapp-3-tx',
			'dapp-4-tx'
		]);
		expect(changes['dapp-3-tx'] ?? []).toEqual([]);
		expect(changes['dapp-4-tx'] ?? []).toEqual([]);
	}, 30_000);

	// Spec 097: the summary carries what the sheet's reading named and the fee
	// the wallet added; the record carries how its operation ended.
	it('the reading, the fee and the settlement pass as written; any bad part drops it whole', () => {
		const summary = {
			...SUMMARY,
			action: 'call',
			contract_name: 'NativeOrderFactory',
			owner: '1inch',
			fee_token: '0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d',
			fee_amount: '20000',
			tokens: [
				{ address: '0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d', symbol: 'USDC', decimals: 18 }
			]
		};
		expect(storedSummary(summary)).toEqual(summary);
		expect(storedSummary({ ...summary, tokens: [{ address: '0x1', symbol: 'X' }] })).toBeNull();
		expect(storedSummary({ ...summary, owner: 7 })).toBeNull();

		const settlement = {
			moved: [
				{ token: '0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d', delta: '-1160000000000000000' },
				{ token: null, delta: '1499036349071560' }
			]
		};
		expect(storedSettlement(settlement)).toEqual(settlement);
		expect(storedSettlement({ failure: 'refused' })).toEqual({ failure: 'refused' });
		for (const bad of [
			'x',
			{ moved: 'x' },
			{ moved: [{ token: 1, delta: '1' }] },
			{ moved: [{ token: null, delta: 1 }] },
			{ failure: 'exploded' }
		]) {
			expect(storedSettlement(bad), JSON.stringify(bad)).toBeNull();
		}
		const tx: LocalTransaction = {
			...SIGNATURE,
			id: 'dapp-1-tx',
			type: 'dapp_tx',
			dappSummary: summary as LocalTransaction['dappSummary'],
			settlement
		};
		expect(toFeedRecord(tx)).toMatchObject({ summary, settlement });
		expect(toFeedRecord({ ...tx, settlement: undefined })?.settlement).toBeNull();
		// A send's record carries none of a dApp's.
		expect(toFeedRecord({ ...tx, type: 'send' })).not.toHaveProperty('settlement');
	});

	it('through the core: a malformed summary still makes a row — by its kind', async () => {
		const { feedItemsThroughCore } = await import('./feed-through-core');
		const items = await feedItemsThroughCore(
			[
				{ ...SIGNATURE, dappSummary: { action: 'steal' } as never },
				{ ...SIGNATURE, id: 'dapp-2-typed', timestamp: 1_700_000_001 }
			],
			ME,
			1_700_000_100_000
		);
		expect(items.map((it) => [it.id, it.dapp?.intent_term])).toEqual([
			['dapp-2-typed', 'permitIntent'],
			['dapp-1-typed', 'typedDataIntent']
		]);
	}, 30_000);
});
