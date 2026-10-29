/**
 * The executor never deploys a genesis the core does not find Verified on
 * that chain (§3.5): checked right before the deploy call is built, on code
 * read then — with the shipped core and Safe's own code (the core's vectors).
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const VECTORS = '../../rust/crates/vela-core/tests/vectors';
const SAFE_CODE: Record<string, string> = {
	'0x4e1dcf7ad4e460cfd30791ccc4f9c8a4f820ec67': 'safe-proxy-factory-1.4.1',
	'0x29fcb43b46531bca003ddc8fcb67ffe91900c762': 'safe-l2-1.4.1',
	'0xfd0732dc9e303f09fcef3a7388ad10a83459ec99': 'compatibility-fallback-handler-1.4.1'
};
const EXECUTOR = '0x1111111111111111111111111111111111111111';

const seams = vi.hoisted(() => ({ codeDown: false, codeReads: 0 }));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (method: string, params: unknown[]) => {
		if (method !== 'eth_getCode') throw new Error(`unexpected ${method}`);
		seams.codeReads += 1;
		if (seams.codeDown) throw new Error('no answer');
		const name = SAFE_CODE[String(params[0]).toLowerCase()];
		return {
			result: name ? readFileSync(`${VECTORS}/safe-code/${name}.hex`, 'utf8').trim() : '0x'
		};
	}
}));
vi.mock('$lib/services/accounts', () => ({ findAccountByAddress: () => null }));
vi.mock('$lib/services/safe-transaction', () => ({}));
vi.mock('$lib/services/tx-reconciler', () => ({}));
vi.mock('$lib/signing/sign-challenge', () => ({}));
vi.mock('$lib/core/kernels', () => ({}));

let kernel: typeof import('./kernel');
let execute: typeof import('./execute');
let authority: typeof import('./authority');

beforeAll(async () => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	kernel = await import('./kernel');
	execute = await import('./execute');
	authority = await import('./authority');
});

beforeEach(() => {
	seams.codeDown = false;
	seams.codeReads = 0;
});

function input(multisig: string, genesis: import('./kernel').Genesis) {
	return {
		chainId: 100,
		multisig,
		genesis,
		members: { deployed: false, owners: [EXECUTOR], threshold: 1, nonce: 0n },
		tx: kernel.cancelTx(multisig, '0x0'),
		approvals: [],
		executor: { address: EXECUTOR } as never
	};
}

describe('deploying a multisig as it executes', () => {
	it('deploys a Verified genesis, on code read just then', async () => {
		const ms = kernel.multisigGenesis([EXECUTOR], 1, 'Team', '0x7');
		const calls = await execute.planCalls(input(ms.address, ms.genesis));
		expect(calls[0]).toEqual(kernel.deployCall(ms.genesis));
		expect(seams.codeReads).toBe(3);
		// Again: read again, never kept.
		await execute.planCalls(input(ms.address, ms.genesis));
		expect(seams.codeReads).toBe(6);
	});

	it('never deploys a hooked genesis: refused, with why, before any passkey', async () => {
		const file = JSON.parse(readFileSync(`${VECTORS}/multisig/hostile-geneses.json`, 'utf8'));
		const row = file.geneses.find((g: { name: string }) => g.name === 'HOOK_MODULE');
		const genesis = {
			factory: file.factory,
			singleton: file.singleton,
			saltNonce: row.saltNonce,
			initializer: row.initializer
		};
		const refused = await execute.planCalls(input(row.address, genesis)).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(authority.GenesisRefusedError);
		expect(refused).toMatchObject({
			chainId: 100,
			authority: { kind: 'refused', reasons: [{ kind: 'module', address: file.attacker }] }
		});
	});

	it('does not deploy on a chain that did not answer', async () => {
		seams.codeDown = true;
		const ms = kernel.multisigGenesis([EXECUTOR], 1, 'Team', '0x7');
		const refused = await execute.planCalls(input(ms.address, ms.genesis)).catch((e: unknown) => e);
		expect(refused).toBeInstanceOf(authority.GenesisRefusedError);
		expect(refused).toMatchObject({ authority: { kind: 'unknown' } });
	});

	it('the core itself refuses a deployment it was given no code for', () => {
		const ms = kernel.multisigGenesis([EXECUTOR], 1, 'Team', '0x7');
		expect(() =>
			kernel.executionCalls({
				multisig: ms.address,
				deployMultisig: ms.genesis,
				deployOwners: [],
				tx: kernel.cancelTx(ms.address, '0x0'),
				approvals: [],
				executor: EXECUTOR
			})
		).toThrow();
	});
});
