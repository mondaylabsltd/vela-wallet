/**
 * §3.5 for a Safe{Wallet} import: whichever genesis it comes with (the
 * service's, or one derived from Safe{Wallet}'s props), who controls it is
 * vetted before it is kept — a refused genesis is not imported, a deployed
 * Safe something besides its members can act for only on the person's tap.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const OWNER = '0x1111111111111111111111111111111111111111';
const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';

const seams = vi.hoisted(() => ({
	deployedOn: [] as number[],
	verdict: null as unknown,
	risks: [] as unknown[],
	genesis: null as unknown,
	calls: [] as string[]
}));
vi.mock('./store.svelte', () => ({
	multisig: {
		find: () => undefined,
		add: (address: string) => {
			seams.calls.push('add');
			return { address };
		},
		setLabel: () => {},
		markShared: () => {},
		setFindings: (_a: string, f: unknown[]) => seams.calls.push(`setFindings:${f.length}`)
	}
}));
vi.mock('./chain', () => ({ hasCode: async (id: number) => seams.deployedOn.includes(id) }));
vi.mock('./authority', () => ({
	counterfactualOn: async () => seams.verdict,
	deployedOn: async () => ({ risks: seams.risks, needCode: [], unread: false })
}));
vi.mock('./safe-service', () => ({ hasTransactionService: () => false }));
vi.mock('./owners', () => ({}));
vi.mock('./service-state.svelte', () => ({ serviceOutage: { blocked: () => false } }));
vi.mock('./service', () => ({
	ServiceUnavailableError: class extends Error {},
	fetchGenesis: async () => ({ found: true, pending: false, genesis: seams.genesis }),
	importGenesis: async () => ({ status: 'recorded' }),
	servedChains: async () => new Set([100]),
	serviceTable: async () => null
}));
vi.mock('$lib/services/networks', () => ({
	networkForChainId: (id: number) => (id === 100 ? { chainId: id } : null)
}));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: async () => ({ result: '0x' }) }));

let accounts: typeof import('./accounts');
let address = '';

beforeAll(async () => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	const kernel = await import('./kernel');
	const ms = kernel.multisigGenesis([OWNER], 1, 'Vault', '0x7');
	seams.genesis = ms.genesis;
	address = ms.address;
	accounts = await import('./accounts');
});

beforeEach(() => {
	seams.deployedOn = [];
	seams.verdict = { kind: 'verified', owners: [OWNER], threshold: 1 };
	seams.risks = [];
	seams.calls = [];
});

const exported = () => ({
	address,
	name: 'Vault',
	chains: [
		{ chainId: 100, deployed: false, owners: [OWNER], threshold: 1, props: null, version: null }
	]
});

describe('importing a Safe{Wallet} Safe', () => {
	it('a Verified genesis is imported', async () => {
		expect((await accounts.importExported(exported())).ok).toBe(true);
		expect(seams.calls).toEqual(['add']);
	});

	it('a refused genesis is not, and says why', async () => {
		seams.verdict = { kind: 'refused', reasons: [{ kind: 'setupCode', address: MODULE }] };
		expect(await accounts.importExported(exported(), { acceptRisks: true })).toEqual({
			ok: false,
			reason: 'refused',
			findings: [{ from: 'genesis', chainId: 100, reason: { kind: 'setupCode', address: MODULE } }]
		});
		expect(seams.calls).toEqual([]);
	});

	it('deployed with a module: only on the person’s tap, keeping what was found', async () => {
		seams.deployedOn = [100];
		seams.risks = [{ kind: 'module', address: MODULE }];
		expect(await accounts.importExported(exported())).toMatchObject({
			ok: false,
			reason: 'highRisk'
		});
		expect(seams.calls).toEqual([]);
		expect((await accounts.importExported(exported(), { acceptRisks: true })).ok).toBe(true);
		expect(seams.calls).toEqual(['add', 'setFindings:1']);
	});

	it('on no network this wallet can read: not imported unchecked', async () => {
		const elsewhere = { ...exported(), chains: [{ ...exported().chains[0], chainId: 999 }] };
		expect(await accounts.importExported(elsewhere)).toEqual({
			ok: false,
			reason: 'unverified',
			chainId: 999
		});
	});
});
