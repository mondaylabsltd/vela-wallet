/**
 * PR 3 — the scan hands the core no clock.
 *
 * `block_timestamp` used to carry `now_ms: Date.now()` beside the block's
 * time, and the core stamped a transfer whose block it could not read with
 * it: three receipts of 2026-09-29 stood under "Today" on 2026-10-10. The
 * answer is now the block's own time or `null` and nothing else; an unread
 * block's transfers are withheld by the core until a later poll reads it.
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

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

import { executeTokenTrustOperation, tokenTrustOperationFailure } from './token-trust-executor';
import type { TrustEffect } from './token-trust-types';

const ME = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
/** 2026-09-29 08:00:00 UTC — the block's own time. */
const BLOCK_TIME = 1_790_668_800;
/** 2026-10-10 08:00:00 UTC — the clock when the scan ran. */
const NOW_MS = 1_791_619_200_000;
const READ: TrustEffect = {
	id: 1,
	operation: { type: 'rpc_get_block_by_number', address: ME, chain_id: 100, block: '0x1092' }
};

beforeEach(() => {
	pool.calls = [];
	vi.useFakeTimers({ toFake: ['Date'] });
	vi.setSystemTime(NOW_MS);
});
afterEach(() => {
	vi.useRealTimers();
	vi.restoreAllMocks();
});

describe('block_timestamp is the block’s time or null — and never carries a clock (PR 3)', () => {
	it('a block that was read: its own seconds, and no `now_ms`', async () => {
		pool.answer = () => ({
			result: { number: '0x1092', timestamp: '0x' + BLOCK_TIME.toString(16) }
		});
		const clock = vi.spyOn(Date, 'now');
		const answer = await executeTokenTrustOperation(READ);
		// The whole answer: nothing beside the block's time.
		expect(answer).toEqual({
			type: 'block_timestamp',
			address: ME,
			chain_id: 100,
			block_number: 0x1092,
			timestamp_sec: BLOCK_TIME
		});
		expect(answer).not.toHaveProperty('now_ms');
		expect(pool.calls).toEqual([
			{ method: 'eth_getBlockByNumber', params: ['0x1092', false], chainId: 100 }
		]);
		expect(clock).not.toHaveBeenCalled();
	});

	it('a block that could not be read — an error, no block, no time in it: null, and still no clock', async () => {
		const clock = vi.spyOn(Date, 'now');
		for (const reply of [
			{ error: { code: -32000, message: 'header not found' } },
			{ result: null },
			{ result: { number: '0x1092' } },
			{ result: { number: '0x1092', timestamp: 1_790_668_800 } }
		]) {
			pool.answer = () => reply;
			const answer = await executeTokenTrustOperation(READ);
			expect(answer, JSON.stringify(reply)).toEqual({
				type: 'block_timestamp',
				address: ME,
				chain_id: 100,
				block_number: 0x1092,
				timestamp_sec: null
			});
			expect(JSON.stringify(answer)).not.toContain(String(NOW_MS / 1000));
		}
		expect(clock).not.toHaveBeenCalled();
	});

	it('a read that threw answers the same through the failure twin: null, no `now_ms`', () => {
		const clock = vi.spyOn(Date, 'now');
		const answer = tokenTrustOperationFailure(READ, new Error('every endpoint failed'));
		expect(answer).toEqual({
			type: 'block_timestamp',
			address: ME,
			chain_id: 100,
			block_number: 0x1092,
			timestamp_sec: null
		});
		expect(answer).not.toHaveProperty('now_ms');
		expect(clock).not.toHaveBeenCalled();
	});
});
