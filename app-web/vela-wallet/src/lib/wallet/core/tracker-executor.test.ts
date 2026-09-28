/**
 * The web tx_tracker executor.
 *
 * Issue 188 / spec 082 RE8: the balance read belongs to `holdings_moved` —
 * the core asks for it after a confirmation and after a failed op that spent
 * gas, never for one that did not land — so a confirmation alone no longer
 * refreshes, and the auto-add still only fires on a receipt's own logs.
 *
 * Ruling 8 (T086): `find_op_event` runs `eth_getLogs` and `eth_blockNumber`
 * through the pool and answers what came back, as it came — a range limit is
 * the core's to recognise, never this file's.
 *
 * T182: a may-have-been-sent record is restored as one after a reload.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { TrackEffect } from './tracker-types';
import type { LocalTransaction } from '$lib/services/transactions-model';

const store = vi.hoisted(() => ({ txs: [] as unknown[] }));
const rpc = vi.hoisted(() => ({
	impl: null as null | ((method: string, params: unknown[], chainId: number) => Promise<unknown>),
	calls: [] as { method: string; params: unknown[]; chainId: number }[]
}));

vi.mock('$lib/services/records', () => ({
	loadTransactions: vi.fn(async () => store.txs),
	updateTransactions: vi.fn(async () => {})
}));
vi.mock('$lib/services/tx-reconciler', () => ({
	pollUserOpStatus: vi.fn(async () => null),
	requestUserOpReceipt: vi.fn(async () => ({ reachedBundler: false, resolution: null }))
}));
vi.mock('$lib/services/rpc-adapter', () => ({
	rpcCall: vi.fn(async (method: string, params: unknown[], chainId: number) => {
		rpc.calls.push({ method, params, chainId });
		return rpc.impl!(method, params, chainId);
	})
}));

import { createTxTrackerExecutor } from './tracker-executor';

const OP = '0x' + 'c1'.repeat(32);
const ENTRY_POINT = '0x0000000071727De22E5E9d8BAf0edAc6f37da032';
const TOPIC = '0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f';

function ports() {
	return { feedReconciled: vi.fn(), receiptLogsConfirmed: vi.fn(), confirmed: vi.fn() };
}

function findOp(from: number | null, to: number | null): TrackEffect {
	return {
		id: 1,
		operation: {
			type: 'find_op_event',
			chain_id: 100,
			entry_point: ENTRY_POINT,
			topic0: TOPIC,
			user_op_hash: OP,
			from_block: from,
			to_block: to
		}
	};
}

beforeEach(() => {
	rpc.calls = [];
	rpc.impl = null;
	store.txs = [];
});

describe('the balance read after an op of ours landed (RE8)', () => {
	it('notify_confirmed does not refresh by itself; auto-add only with logs', async () => {
		const p = ports();
		const executor = createTxTrackerExecutor(p);
		const effect: TrackEffect = {
			id: 1,
			operation: {
				type: 'notify_confirmed',
				user_op_hash: '0xABC',
				chain_id: 8453,
				tx_hash: '0xdef'
			}
		};
		await expect(executor.execute(effect)).resolves.toEqual({ type: 'notified' });
		expect(p.confirmed).not.toHaveBeenCalled();
		// No receipt was polled for this hash (a found event has none), so there
		// are no logs to admit tokens from — and no sender is guessed.
		expect(p.receiptLogsConfirmed).not.toHaveBeenCalled();
	});

	it('holdings_moved — and only it — asks for the forced balance read', async () => {
		const p = ports();
		const executor = createTxTrackerExecutor(p);
		await expect(
			executor.execute({ id: 2, operation: { type: 'holdings_moved', chain_id: 100 } })
		).resolves.toEqual({ type: 'notified' });
		expect(p.confirmed).toHaveBeenCalledTimes(1);
		expect(p.confirmed).toHaveBeenCalledWith(100);
	});
});

describe('find_op_event answers what the pool said (ruling 8)', () => {
	it('found: the log array goes back verbatim, with the head', async () => {
		const logs = [
			{ address: ENTRY_POINT, topics: [TOPIC, OP], data: '0x', transactionHash: '0xt' }
		];
		rpc.impl = async (method) =>
			method === 'eth_blockNumber'
				? { jsonrpc: '2.0', id: 1, result: '0x2e3a' }
				: { jsonrpc: '2.0', id: 1, result: logs };
		const answer = await createTxTrackerExecutor(ports()).execute(findOp(11_800, 11_834));
		expect(answer).toMatchObject({
			type: 'op_event',
			user_op_hash: OP,
			logs_json: JSON.stringify(logs),
			error_json: null,
			head_block: 0x2e3a
		});
		const getLogs = rpc.calls.find((c) => c.method === 'eth_getLogs')!;
		expect(getLogs.chainId).toBe(100);
		expect(getLogs.params).toEqual([
			{
				address: ENTRY_POINT,
				topics: [TOPIC, OP],
				fromBlock: '0x2e18',
				toBlock: '0x2e3a'
			}
		]);
	});

	it('a range error is handed over as the JSON error, not judged here', async () => {
		const error = {
			code: -32005,
			message: 'ranges over 10000 blocks are not supported on free plan'
		};
		rpc.impl = async (method) =>
			method === 'eth_blockNumber'
				? { jsonrpc: '2.0', id: 1, result: '0x10' }
				: { jsonrpc: '2.0', id: 1, error };
		const answer = await createTxTrackerExecutor(ports()).execute(findOp(1, 16));
		expect(answer).toMatchObject({
			logs_json: null,
			error_json: JSON.stringify(error),
			head_block: 16
		});
	});

	it('head only: no logs are read when the scan has no start yet', async () => {
		rpc.impl = async () => ({ jsonrpc: '2.0', id: 1, result: '0x64' });
		const answer = await createTxTrackerExecutor(ports()).execute(findOp(null, null));
		expect(answer).toMatchObject({ logs_json: null, error_json: null, head_block: 100 });
		expect(rpc.calls.map((c) => c.method)).toEqual(['eth_blockNumber']);
	});

	it('a pool that reached nobody is no answer at all', async () => {
		rpc.impl = async () => {
			throw new Error('All RPC endpoints failed for chain 100');
		};
		const answer = await createTxTrackerExecutor(ports()).execute(findOp(1, 2));
		expect(answer).toMatchObject({ logs_json: null, error_json: null, head_block: null });
	});
});

describe('a may-have-been-sent op survives a reload (T182)', () => {
	function row(over: Partial<LocalTransaction>): LocalTransaction {
		return {
			id: 'dapp-1-tx',
			userOpHash: OP,
			txHash: '',
			from: '0x' + 'a1'.repeat(20),
			to: '0x' + 'b1'.repeat(20),
			value: '0',
			symbol: 'xDAI',
			decimals: 18,
			chainId: 100,
			timestamp: 1_700_000_000,
			status: 'pending',
			type: 'dapp_tx',
			...over
		};
	}

	it('records_loaded carries maybe_sent and submit_block; an old row reads false / unknown', async () => {
		store.txs = [
			row({ id: 'dapp-1-tx', maybeSent: true, submitBlock: 48_478_700 }),
			row({ id: 'send-2', userOpHash: '0x' + 'd2'.repeat(32), type: 'send' })
		];
		const answer = await createTxTrackerExecutor(ports()).execute({
			id: 1,
			operation: { type: 'load_pending_txs' }
		});
		expect(answer).toMatchObject({
			type: 'records_loaded',
			records: [
				{
					record_id: 'dapp-1-tx',
					user_op_hash: OP,
					chain_id: 100,
					submitted_at_ms: 1_700_000_000_000,
					maybe_sent: true,
					submit_block: 48_478_700
				},
				{ record_id: 'send-2', maybe_sent: false, submit_block: null }
			]
		});
	});
});
