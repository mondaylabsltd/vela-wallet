/**
 * PR 3 — a receipt's time is its block's time, never "now".
 *
 * On the owner's iPhone three receipts of 0.001 xDAI, received on 2026-09-29,
 * stood under "Today" on 2026-10-10: the scan could not read their block's
 * time, the core stamped them with the clock the shell handed it, and the
 * shell stored that for good. The core no longer takes a clock, marks what it
 * stores (`timeVerified`), and repairs the records stored before the mark
 * through two operations this executor answers:
 *
 * - `read_receive_time` — the transaction's receipt, then its block, through
 *   the RPC pool: the block's seconds, or `null`. Never a clock.
 * - `write_receive_time` — that ONE stored record, rewritten behind the
 *   store's lock.
 *
 * What is under test is this shell's wiring: the store, the pool reads and
 * the executor's answers — and, at the end, all of it under the real core.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalTransaction } from '$lib/services/transactions-model';
import type { FeedEffect } from './feed-types';
import type { FeedView } from '$lib/core/generated/FeedView';

const kv = new Map<string, string>();
const storage = vi.hoisted(() => ({ refuse: false, writes: 0 }));
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => {
		if (storage.refuse) throw new Error('quota');
		storage.writes += 1;
		kv.set(key, value);
	}),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
vi.mock('$lib/services/activity', async (importOriginal) => {
	const original = await importOriginal<typeof import('$lib/services/activity')>();
	return { ...original, syncReceivedTransfers: vi.fn(async () => 0) };
});
vi.mock('$lib/services/recipient-identity', () => ({
	resolveRecipientIdentity: async () => null
}));

/** The RPC pool, stubbed: every call is recorded, and answered by `pool.answer`. */
const pool = vi.hoisted(() => ({
	calls: [] as { method: string; params: unknown[]; chainId: number }[],
	answer: (() => ({})) as (method: string, params: unknown[], chainId: number) => unknown
}));
vi.mock('$lib/services/rpc-pool', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/rpc-pool')>()),
	poolRpcCall: async (method: string, params: unknown[], chainId: number) => {
		pool.calls.push({ method, params, chainId });
		return pool.answer(method, params, chainId);
	}
}));

import { dayStartMs } from '$lib/services/activity';
import { createFeedExecutor, readReceiveTime, toFeedRecord } from './feed-executor';

const TX_KEY = 'vela.transactionHistory';
const ME = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
const OTHER = '0x' + 'b1'.repeat(20);
/** 2026-09-29 08:00:00 UTC — when the money really arrived. */
const BLOCK_TIME = 1_790_668_800;
/** 2026-10-10 08:00:00 UTC — the clock the old scan stamped it with. */
const NOW_S = 1_791_619_200;
const hex = (n: number) => '0x' + n.toString(16);

const receive = (txHash: string, over: Partial<LocalTransaction> = {}): LocalTransaction => ({
	id: `100-${txHash}-0`,
	userOpHash: '',
	txHash,
	from: OTHER,
	to: ME,
	value: '0.001',
	symbol: 'xDAI',
	decimals: 18,
	logoUrls: ['https://logos.example/xdai.png'],
	chainId: 100,
	// Wrong: the clock at the scan, not the block's time. And no mark.
	timestamp: NOW_S,
	status: 'confirmed',
	type: 'receive',
	usd: '$0.00',
	...over
});
const SENT: LocalTransaction = {
	id: '0xop-sent',
	userOpHash: '0xop-sent',
	txHash: '0xsent',
	from: ME,
	to: OTHER,
	value: '2',
	symbol: 'xDAI',
	decimals: 18,
	chainId: 100,
	// A send's time IS the clock's, when the person confirmed: it is not touched.
	timestamp: NOW_S - 60,
	status: 'confirmed',
	type: 'send'
};

const stored = (): LocalTransaction[] => JSON.parse(kv.get(TX_KEY) ?? '[]');
const effect = (operation: FeedEffect['operation']): FeedEffect => ({ id: 1, operation });
const signal = () => new AbortController().signal;
const executor = () => createFeedExecutor(() => [], { storeLoaded: () => {} });

/** A chain that knows each transaction's block, and each block's time. */
function chain(blocks: Record<string, { number: number; time: number }>) {
	pool.answer = (method, params) => {
		if (method === 'eth_getTransactionReceipt') {
			const block = blocks[params[0] as string];
			return { result: block ? { blockNumber: hex(block.number), status: '0x1' } : null };
		}
		if (method === 'eth_getBlockByNumber') {
			const block = Object.values(blocks).find((b) => hex(b.number) === params[0]);
			return { result: block ? { number: params[0], timestamp: hex(block.time) } : null };
		}
		return { error: { code: -32601, message: 'not stubbed' } };
	};
}

beforeEach(() => {
	kv.clear();
	storage.refuse = false;
	storage.writes = 0;
	pool.calls = [];
	pool.answer = () => ({ error: { code: -32601, message: 'not stubbed' } });
});
afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe('write_receive_time rewrites that one stored record (PR 3)', () => {
	it('the record takes the block’s time and the mark; every other field, and every other record, is as it was', async () => {
		const wrong = receive('0xaaa');
		const neighbour = receive('0xbbb');
		kv.set(TX_KEY, JSON.stringify([SENT, wrong, neighbour]));
		const before = stored();

		const answer = await executor().execute(
			effect({ type: 'write_receive_time', id: wrong.id, timestamp_sec: BLOCK_TIME }),
			signal()
		);
		expect(answer).toEqual({ type: 'receive_time_written', id: wrong.id, ok: true });

		const after = stored();
		// The same records, in the same places.
		expect(after.map((tx) => tx.id)).toEqual(before.map((tx) => tx.id));
		// The other two, byte for byte.
		expect(JSON.stringify(after[0])).toBe(JSON.stringify(before[0]));
		expect(JSON.stringify(after[2])).toBe(JSON.stringify(before[2]));
		// This one: the block's time and the mark…
		expect(after[1].timestamp).toBe(BLOCK_TIME);
		expect(after[1].timeVerified).toBe(true);
		// …and every other field byte-identical, with no field gone or added.
		const was = before[1] as unknown as Record<string, unknown>;
		const is = after[1] as unknown as Record<string, unknown>;
		expect(Object.keys(is).sort()).toEqual([...Object.keys(was), 'timeVerified'].sort());
		for (const field of Object.keys(was).filter((key) => key !== 'timestamp')) {
			expect(JSON.stringify(is[field]), field).toBe(JSON.stringify(was[field]));
		}
		expect(storage.writes).toBe(1);
	});

	it('the next read_tx_store hands the core the mark — and only on the record that has it', async () => {
		const wrong = receive('0xaaa');
		kv.set(TX_KEY, JSON.stringify([SENT, wrong, receive('0xbbb')]));
		const run = executor();
		await run.execute(
			effect({ type: 'write_receive_time', id: wrong.id, timestamp_sec: BLOCK_TIME }),
			signal()
		);
		const read = await run.execute(
			effect({ type: 'read_tx_store', address: ME, read_id: 4 }),
			signal()
		);
		if (read.type !== 'store_loaded') throw new Error(`answered ${read.type}`);
		const [sent, rewritten, unmarked] = read.records;
		expect(rewritten).toMatchObject({
			id: wrong.id,
			timestamp: BLOCK_TIME,
			time_verified: true,
			// Its day is the block's day now, in the device's zone.
			day_start_ms: dayStartMs(BLOCK_TIME)
		});
		// No mark stays no mark: that is what asks for the repair.
		expect(unmarked).not.toHaveProperty('time_verified');
		expect(sent).not.toHaveProperty('time_verified');
	});

	it('the mark is read as stored: false is false, and anything but a boolean is no mark', () => {
		expect(toFeedRecord(receive('0xaaa', { timeVerified: true }))?.time_verified).toBe(true);
		expect(toFeedRecord(receive('0xaaa', { timeVerified: false }))?.time_verified).toBe(false);
		for (const junk of ['true', 1, null, {}]) {
			expect(
				toFeedRecord(receive('0xaaa', { timeVerified: junk as never })),
				JSON.stringify(junk)
			).not.toHaveProperty('time_verified');
		}
	});

	it('an unknown id answers ok:false and writes nothing', async () => {
		kv.set(TX_KEY, JSON.stringify([SENT, receive('0xaaa')]));
		const before = kv.get(TX_KEY);
		const answer = await executor().execute(
			effect({ type: 'write_receive_time', id: '100-0xnobody-0', timestamp_sec: BLOCK_TIME }),
			signal()
		);
		expect(answer).toEqual({ type: 'receive_time_written', id: '100-0xnobody-0', ok: false });
		expect(kv.get(TX_KEY)).toBe(before);
		expect(storage.writes).toBe(0);
	});

	it('a store that refuses the write answers ok:false; so does a time that is not one', async () => {
		const wrong = receive('0xaaa');
		kv.set(TX_KEY, JSON.stringify([wrong]));
		const before = kv.get(TX_KEY);
		const run = executor();
		storage.refuse = true;
		expect(
			await run.execute(
				effect({ type: 'write_receive_time', id: wrong.id, timestamp_sec: BLOCK_TIME }),
				signal()
			)
		).toEqual({ type: 'receive_time_written', id: wrong.id, ok: false });
		storage.refuse = false;
		for (const odd of [Number.NaN, 0, -5, Number.POSITIVE_INFINITY]) {
			expect(
				await run.execute(
					effect({ type: 'write_receive_time', id: wrong.id, timestamp_sec: odd }),
					signal()
				),
				String(odd)
			).toEqual({ type: 'receive_time_written', id: wrong.id, ok: false });
		}
		expect(kv.get(TX_KEY)).toBe(before);
		// The failure twin leaves the core's round answered too.
		expect(
			run.toFailure(
				effect({ type: 'write_receive_time', id: wrong.id, timestamp_sec: BLOCK_TIME }),
				new Error('x')
			)
		).toEqual({ type: 'receive_time_written', id: wrong.id, ok: false });
	});

	it('queues behind the store’s other writers: a delete and a rewrite issued together both land', async () => {
		const wrong = receive('0xaaa');
		kv.set(TX_KEY, JSON.stringify([SENT, wrong, receive('0xbbb')]));
		const run = executor();
		const [written, deleted] = await Promise.all([
			run.execute(
				effect({ type: 'write_receive_time', id: wrong.id, timestamp_sec: BLOCK_TIME }),
				signal()
			),
			run.execute(effect({ type: 'delete_tx_record', id: SENT.id }), signal())
		]);
		expect(written).toMatchObject({ ok: true });
		expect(deleted).toEqual({ type: 'delete_committed', id: SENT.id });
		// Neither write lost the other's.
		expect(stored().map((tx) => [tx.id, tx.timestamp, tx.timeVerified])).toEqual([
			[wrong.id, BLOCK_TIME, true],
			['100-0xbbb-0', NOW_S, undefined]
		]);
	});
});

describe('read_receive_time asks the chain, through the pool, and never the clock (PR 3)', () => {
	const ask = (txHash = '0xaaa') =>
		executor().execute(
			effect({ type: 'read_receive_time', id: `100-${txHash}-0`, chain_id: 100, tx_hash: txHash }),
			signal()
		);

	it('receipt, then its block: the block’s own seconds', async () => {
		chain({ '0xaaa': { number: 4_242, time: BLOCK_TIME } });
		expect(await ask()).toEqual({
			type: 'receive_time_read',
			id: '100-0xaaa-0',
			timestamp_sec: BLOCK_TIME
		});
		// Two reads, on the record's chain, the second for the receipt's block.
		expect(pool.calls).toEqual([
			{ method: 'eth_getTransactionReceipt', params: ['0xaaa'], chainId: 100 },
			{ method: 'eth_getBlockByNumber', params: [hex(4_242), false], chainId: 100 }
		]);
	});

	it('no receipt: null, and the block is not asked for', async () => {
		chain({});
		expect(await ask()).toEqual({
			type: 'receive_time_read',
			id: '100-0xaaa-0',
			timestamp_sec: null
		});
		expect(pool.calls.map((call) => call.method)).toEqual(['eth_getTransactionReceipt']);
	});

	it('the block read fails — an error, no block, a pool that gave up: null each time, and asked once', async () => {
		const receipt = { result: { blockNumber: hex(4_242) } };
		const failures: (() => unknown)[] = [
			() => ({ error: { code: -32000, message: 'header not found' } }),
			() => ({ result: null }),
			() => {
				throw new Error('every endpoint failed');
			}
		];
		for (const blockRead of failures) {
			pool.calls = [];
			pool.answer = (method) => (method === 'eth_getTransactionReceipt' ? receipt : blockRead());
			expect(await ask()).toEqual({
				type: 'receive_time_read',
				id: '100-0xaaa-0',
				timestamp_sec: null
			});
			// No retry inside the operation: when to ask again is the core's.
			expect(pool.calls.map((call) => call.method)).toEqual([
				'eth_getTransactionReceipt',
				'eth_getBlockByNumber'
			]);
		}
	});

	it('the receipt read fails, or a hex does not read: null', async () => {
		const answers: ((method: string) => unknown)[] = [
			() => ({ error: { code: -32005, message: 'rate limited' } }),
			() => {
				throw new Error('every endpoint failed');
			},
			// A receipt with no block, or one that is not a hex quantity.
			() => ({ result: {} }),
			() => ({ result: { blockNumber: 4_242 } }),
			() => ({ result: { blockNumber: 'latest' } }),
			// A block whose time does not read.
			(method) =>
				method === 'eth_getTransactionReceipt'
					? { result: { blockNumber: hex(4_242) } }
					: { result: { timestamp: '0x' } },
			(method) =>
				method === 'eth_getTransactionReceipt'
					? { result: { blockNumber: hex(4_242) } }
					: { result: { timestamp: 1_790_668_800 } },
			(method) =>
				method === 'eth_getTransactionReceipt'
					? { result: { blockNumber: hex(4_242) } }
					: { result: {} }
		];
		for (const [i, answer] of answers.entries()) {
			pool.answer = answer;
			expect(await readReceiveTime(100, '0xaaa'), `case ${i}`).toBeNull();
		}
		// The failure twin: not read.
		expect(
			executor().toFailure(
				effect({ type: 'read_receive_time', id: 'r', chain_id: 100, tx_hash: '0xaaa' }),
				new Error('x')
			)
		).toEqual({ type: 'receive_time_read', id: 'r', timestamp_sec: null });
	});

	it('no answer carries a clock time: the clock is never read, whatever the chain says', async () => {
		vi.useFakeTimers({ toFake: ['Date'] });
		vi.setSystemTime(NOW_S * 1000);
		const clock = vi.spyOn(Date, 'now');
		const answers: unknown[] = [];

		chain({ '0xaaa': { number: 4_242, time: BLOCK_TIME } });
		answers.push(await ask());
		chain({});
		answers.push(await ask());
		pool.answer = (method) =>
			method === 'eth_getTransactionReceipt'
				? { result: { blockNumber: hex(4_242) } }
				: { error: { code: -32000, message: 'header not found' } };
		answers.push(await ask());
		pool.answer = () => {
			throw new Error('every endpoint failed');
		};
		answers.push(await ask());

		expect(answers.map((a) => (a as { timestamp_sec: number | null }).timestamp_sec)).toEqual([
			BLOCK_TIME,
			null,
			null,
			null
		]);
		// The clock stood at NOW_S the whole time: no answer is it, and nothing asked it.
		expect(answers.some((a) => JSON.stringify(a).includes(String(NOW_S)))).toBe(false);
		expect(clock).not.toHaveBeenCalled();
	});
});

/**
 * The whole loop, under the REAL core and this shell's own session: the feed
 * reads the store, the core asks for the three unmarked receipts' block
 * times, the executor reads them off the (stubbed) chain, the core asks for
 * the rewrites, the store is rewritten, and the core reads it once more — so
 * the rows stand under the day the money arrived. An operation this executor
 * did not answer would leave the core's round waiting for ever; this is the
 * test that would hang.
 */
describe('through the core: receipts stamped "today" end under their own day (PR 3)', () => {
	it('three stored receipts are re-timed in the store and re-dated in the feed; nothing else is touched', async () => {
		await import('$lib/i18n/wasm-init.server');
		const { createActivityFeedSession } = await import('./feed-session');
		const receipts = [receive('0xaaa'), receive('0xbbb'), receive('0xccc')];
		// Already marked: its time is not asked about again.
		const marked = receive('0xddd', { timestamp: BLOCK_TIME - 3_600, timeVerified: true });
		kv.set(TX_KEY, JSON.stringify([...receipts, SENT, marked]));
		const sentBefore = JSON.stringify(SENT);
		const markedBefore = JSON.stringify(marked);
		chain({
			'0xaaa': { number: 4_241, time: BLOCK_TIME },
			'0xbbb': { number: 4_242, time: BLOCK_TIME + 5 },
			'0xccc': { number: 4_243, time: BLOCK_TIME + 10 },
			'0xddd': { number: 4_000, time: 1 },
			'0xsent': { number: 4_100, time: 1 }
		});

		const views: FeedView[] = [];
		const faults: unknown[] = [];
		const session = createActivityFeedSession({
			onView: (view) => views.push(view),
			onError: (error) => faults.push(error),
			ownAccounts: () => [],
			records: { storeLoaded: () => {} }
		});
		try {
			session.start({ type: 'account_switched', address: ME });

			// The store: each receipt has its block's time and the mark.
			await vi.waitFor(
				() =>
					expect(
						stored()
							.filter((tx) => receipts.some((r) => r.id === tx.id))
							.map((tx) => [tx.txHash, tx.timestamp, tx.timeVerified])
					).toEqual([
						['0xaaa', BLOCK_TIME, true],
						['0xbbb', BLOCK_TIME + 5, true],
						['0xccc', BLOCK_TIME + 10, true]
					]),
				{ timeout: 20_000, interval: 20 }
			);
			// The feed: the three rows under the day the money arrived — the
			// core's own re-read after the round, nothing this test asked for.
			const itemsOf = (view: FeedView | undefined) =>
				(view?.rows ?? []).flatMap((row) => (row.type === 'item' ? [row.item] : []));
			await vi.waitFor(
				() => {
					const items = itemsOf(views.at(-1)).filter((item) =>
						receipts.some((r) => r.id === item.id)
					);
					expect(items.map((item) => item.timestamp).sort()).toEqual([
						BLOCK_TIME,
						BLOCK_TIME + 5,
						BLOCK_TIME + 10
					]);
				},
				{ timeout: 20_000, interval: 20 }
			);
			const last = views.at(-1)!;
			const headers = last.rows.flatMap((row) => (row.type === 'header' ? [row.day_start_ms] : []));
			// Two days on the feed: the send's (the clock's, rightly) and the
			// receipts' own — which is no longer the send's.
			expect(headers).toContain(dayStartMs(BLOCK_TIME));
			expect(headers).toContain(dayStartMs(SENT.timestamp));
			expect(dayStartMs(BLOCK_TIME)).not.toBe(dayStartMs(NOW_S));

			// Asked about the three unmarked receipts only: a receipt and a block
			// each, and never the send or the receipt already marked.
			const asked = pool.calls.filter((call) => call.method === 'eth_getTransactionReceipt');
			expect(asked.map((call) => call.params[0]).sort()).toEqual(['0xaaa', '0xbbb', '0xccc']);
			expect(pool.calls.filter((call) => call.method === 'eth_getBlockByNumber')).toHaveLength(3);
			expect(pool.calls.every((call) => call.chainId === 100)).toBe(true);
			// Nothing else in the store moved.
			const after = stored();
			expect(JSON.stringify(after.find((tx) => tx.id === SENT.id))).toBe(sentBefore);
			expect(JSON.stringify(after.find((tx) => tx.id === marked.id))).toBe(markedBefore);
			expect(after).toHaveLength(5);
			expect(faults).toEqual([]);
		} finally {
			session.dispose();
		}
		// The core's first load, inside the test: 5 s is not enough under a full parallel run.
	}, 60_000);
});
