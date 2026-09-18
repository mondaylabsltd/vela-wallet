/**
 * The inner calls' own gas floor (spec 062, found on Base): the bundler's
 * `callGasLimit` for an UNDEPLOYED Safe is 21k + calldata, because the sender
 * has no code and the estimate call "succeeds" trivially. A real contract call
 * as the first operation on a fresh chain was accepted and never bundled.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it, vi } from 'vitest';

const answers = new Map<string, string | null>();
const asked: { from: string; to: string; value: string; data: string }[] = [];
vi.mock('./rpc-adapter', () => ({
	rpcCall: vi.fn(async (_method: string, params: [Record<string, string>]) => {
		asked.push(params[0] as (typeof asked)[number]);
		const hex = answers.get(params[0].to);
		return hex === null || hex === undefined
			? { jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'down' } }
			: { jsonrpc: '2.0', id: 1, result: hex };
	})
}));

import { innerCallsGasFloor } from './safe-transaction';

const SAFE = '0x' + '88'.repeat(20);
const REGISTRY = '0x' + '94'.repeat(20);
const call = (to: string, data: Uint8Array, value = '0') => ({ to, value, data });
const contractCall = new Uint8Array(4324).fill(0xcd);
const transfer = new Uint8Array(0);

describe('innerCallsGasFloor', () => {
	it('measures each real contract call from the Safe itself and pads the frames around it', async () => {
		asked.length = 0;
		answers.set(REGISTRY, '0x' + (4_308_125).toString(16));
		const floor = await innerCallsGasFloor(1, SAFE, [call(REGISTRY, contractCall)]);
		// 4,308,125 × 1.25 + 60k for the frame + 50k per call — the figure the
		// live Base run raised the bundler's 265,786 to.
		expect(floor).toBe(5_495_156n);
		expect(asked).toEqual([
			{ from: SAFE, to: REGISTRY, value: '0x0', data: '0x' + 'cd'.repeat(4324) }
		]);
	});

	it('plain transfers are not measured — the defaults cover them', async () => {
		asked.length = 0;
		expect(
			await innerCallsGasFloor(1, SAFE, [call('0x' + '11'.repeat(20), transfer, '1000')])
		).toBeNull();
		expect(asked).toEqual([]);
	});

	it('a call nobody could estimate gives no floor, never a guess', async () => {
		answers.set(REGISTRY, null);
		expect(await innerCallsGasFloor(1, SAFE, [call(REGISTRY, contractCall)])).toBeNull();
	});
});
