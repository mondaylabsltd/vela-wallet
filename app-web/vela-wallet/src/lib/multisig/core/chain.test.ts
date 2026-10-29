/**
 * Where a multisig can live: a chain with Safe's own code at the Safe v1.4.1
 * contracts its genesis names (factory, singleton, fallback handler) — read
 * here with the wallet's own RPC, judged by the core against each contract's
 * code hash. The registry plays no part.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const HANDLER = '0xfd0732dc9e303f09fcef3a7388ad10a83459ec99';
/** Safe's own runtime code, as Gnosis holds it (the core's vectors). */
const code = (name: string) =>
	readFileSync(`../../rust/crates/vela-core/tests/vectors/safe-code/${name}.hex`, 'utf8').trim();
const SAFE_CODE: Record<string, string> = {
	'0x4e1dcf7ad4e460cfd30791ccc4f9c8a4f820ec67': code('safe-proxy-factory-1.4.1'),
	'0x29fcb43b46531bca003ddc8fcb67ffe91900c762': code('safe-l2-1.4.1'),
	[HANDLER]: code('compatibility-fallback-handler-1.4.1')
};

const seams = vi.hoisted(() => ({
	/** Per chain: what `eth_getCode` answers for the handler, or `throw`. */
	handler: {} as Record<number, string | 'throw'>,
	asked: [] as number[]
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (_method: string, params: [string], chainId: number) => {
		seams.asked.push(chainId);
		const at = params[0].toLowerCase();
		if (at !== HANDLER) return { result: SAFE_CODE[at] ?? '0x' };
		const answer = seams.handler[chainId] ?? SAFE_CODE[HANDLER];
		if (answer === 'throw') throw new Error('no answer');
		return { result: answer };
	}
}));

let safeContractsOn: typeof import('./chain').safeContractsOn;
let hasContractCode: typeof import('./chain').hasContractCode;

beforeAll(async () => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	({ safeContractsOn, hasContractCode } = await import('./chain'));
});

/**
 * Who signs how is read on this (r4-9): an EIP-7702 delegation is a person's
 * own wallet with smart-account features — its key signs — not a smart
 * account that approves only in Vela. The onchain suite's outside owner is
 * one (Gnosis's anvil account #1 is delegated).
 */
describe('a smart account, or a wallet', () => {
	it('an EIP-7702 delegation is a wallet; other code is a smart account; none is neither', async () => {
		const DELEGATED = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';
		const CONTRACT = '0x00000000000000000000000000000000000000c0';
		SAFE_CODE[DELEGATED] = '0xef01002e623468c560072e91d7a94a1dcb2eefbdbe69cb';
		SAFE_CODE[CONTRACT] = '0x6080604052';
		expect(await hasContractCode(100, DELEGATED)).toBe(false);
		expect(await hasContractCode(100, CONTRACT)).toBe(true);
		expect(await hasContractCode(100, '0x00000000000000000000000000000000000000e0')).toBe(false);
	});
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

	it('missing too: code at its address that is not Safe’s', async () => {
		seams.handler[998] = '0x6080604052';
		expect(await safeContractsOn(998)).toEqual({
			kind: 'missing',
			names: ['Compatibility Fallback Handler']
		});
	});

	it('unknown when the chain does not answer, and asked again next time', async () => {
		seams.handler[8453] = 'throw';
		expect(await safeContractsOn(8453)).toEqual({ kind: 'unknown' });
		seams.handler[8453] = SAFE_CODE[HANDLER];
		expect(await safeContractsOn(8453)).toEqual({ kind: 'present' });
		// A verdict is kept: asked once a session.
		seams.asked = [];
		expect(await safeContractsOn(8453)).toEqual({ kind: 'present' });
		expect(seams.asked).toEqual([]);
	});
});
