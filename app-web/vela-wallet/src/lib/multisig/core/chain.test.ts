/**
 * Where a multisig can live: a chain with code at the Safe v1.4.1 contracts
 * its genesis names (factory, singleton, fallback handler) — read here with
 * the wallet's own RPC, judged by the core. The registry plays no part.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const HANDLER = '0xfd0732dc9e303f09fcef3a7388ad10a83459ec99';

const seams = vi.hoisted(() => ({
	/** Per chain: what `eth_getCode` answers for the handler, or `throw`. */
	handler: {} as Record<number, string | 'throw'>,
	asked: [] as number[]
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (_method: string, params: [string], chainId: number) => {
		seams.asked.push(chainId);
		if (params[0].toLowerCase() !== HANDLER) return { result: '0x6080' };
		const code = seams.handler[chainId] ?? '0x6080';
		if (code === 'throw') throw new Error('no answer');
		return { result: code };
	}
}));

let safeContractsOn: typeof import('./chain').safeContractsOn;

beforeAll(async () => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	({ safeContractsOn } = await import('./chain'));
});

describe('the Safe contracts on a chain', () => {
	it('present: every one has code', async () => {
		expect(await safeContractsOn(42161)).toEqual({ kind: 'present' });
	});

	it('missing: one has none, named', async () => {
		seams.handler[999] = '0x';
		expect(await safeContractsOn(999)).toEqual({
			kind: 'missing',
			names: ['Compatibility Fallback Handler']
		});
	});

	it('unknown when the chain does not answer, and asked again next time', async () => {
		seams.handler[8453] = 'throw';
		expect(await safeContractsOn(8453)).toEqual({ kind: 'unknown' });
		seams.handler[8453] = '0x6080';
		expect(await safeContractsOn(8453)).toEqual({ kind: 'present' });
		// A verdict is kept: asked once a session.
		seams.asked = [];
		expect(await safeContractsOn(8453)).toEqual({ kind: 'present' });
		expect(seams.asked).toEqual([]);
	});
});
