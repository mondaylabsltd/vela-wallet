/**
 * §3.5: a multisig already on this device — one added before the
 * counterfactual rule, or one that has gained a module since — is checked
 * again, once a session and on each network it comes to be used on, as
 * adding one is: deployed there, the chain's state; not deployed there, the
 * core's counterfactual rule. What is found is kept on it (never silent);
 * a network that does not answer is asked again, and changes nothing.
 * The chain's readings are stood in for (`authority.test.ts` reads them).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';
const GNOSIS = 100;
const BASE = 8453;
/** A network this device does not have. */
const ELSEWHERE = 99999;

const seams = vi.hoisted(() => ({
	/** Where it has code; a chain in `down` does not answer. */
	deployed: [] as number[],
	down: [] as number[],
	/** A deployed Safe's check, per chain (default: only its members act). */
	risks: {} as Record<number, unknown[]>,
	/** The core's counterfactual verdict, per chain (default: verified). */
	verdict: {} as Record<number, unknown>,
	reads: [] as string[]
}));

const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
});

vi.mock('$lib/services/networks', () => ({
	networkForChainId: (id: number) => (id === ELSEWHERE ? null : { chainId: id })
}));
vi.mock('./chain', () => ({
	hasCode: async (chainId: number) => {
		seams.reads.push(`code:${chainId}`);
		if (seams.down.includes(chainId)) throw new Error('down');
		return seams.deployed.includes(chainId);
	}
}));
vi.mock('./authority', () => ({
	deployedOn: async (chainId: number) => {
		seams.reads.push(`deployed:${chainId}`);
		return { unread: false, risks: seams.risks[chainId] ?? [], needCode: [] };
	},
	counterfactualOn: async (chainId: number) => {
		seams.reads.push(`counterfactual:${chainId}`);
		return seams.verdict[chainId] ?? { kind: 'verified', owners: [], threshold: 1 };
	}
}));

const { multisig } = await import('./store.svelte');
const { checkOn, forgetChecks, recheckStored } = await import('./recheck');
const { signingGate } = await import('./gate.svelte');

/** A row as a build before the rule kept it: no findings. */
function stored(chainIds: number[]) {
	multisig.accounts = [
		{
			address: SAFE,
			genesis: { factory: '', singleton: '', saltNonce: '0x1', initializer: '0x' },
			owners: [],
			threshold: 1,
			recorded: true,
			chainIds,
			addedMs: 0
		}
	];
}

const kept = () => multisig.find(SAFE)?.findings;

beforeEach(() => {
	forgetChecks();
	storage.clear();
	seams.deployed = [];
	seams.down = [];
	seams.risks = {};
	seams.verdict = {};
	seams.reads = [];
});

describe('a multisig on this device, checked again (§3.5)', () => {
	it('deployed with a module since: marked high risk, kept on it and written', async () => {
		stored([GNOSIS]);
		seams.deployed = [GNOSIS];
		seams.risks[GNOSIS] = [{ kind: 'module', address: MODULE }];
		await recheckStored();
		const risk = { from: 'chain', chainId: GNOSIS, risk: { kind: 'module', address: MODULE } };
		expect(kept()).toEqual([risk]);
		expect(JSON.parse(storage.get('vela.multisigAccounts')!)[0].findings).toEqual([risk]);
		expect(signingGate(multisig.find(SAFE)!, GNOSIS)).toMatchObject({ kind: 'risk' });
	});

	it('not deployed, its genesis refused by the core: read-only there, the other network as it was', async () => {
		stored([GNOSIS, BASE]);
		seams.deployed = [GNOSIS];
		seams.verdict[BASE] = { kind: 'refused', reasons: [{ kind: 'module', address: MODULE }] };
		await recheckStored();
		expect(seams.reads).toEqual(
			expect.arrayContaining([`deployed:${GNOSIS}`, `counterfactual:${BASE}`])
		);
		expect(kept()).toEqual([
			{ from: 'genesis', chainId: BASE, reason: { kind: 'module', address: MODULE } }
		]);
		expect(signingGate(multisig.find(SAFE)!, BASE)).toMatchObject({ kind: 'refused' });
		expect(signingGate(multisig.find(SAFE)!, GNOSIS)).toEqual({ kind: 'open' });
	});

	it('once a session — and a network it comes to be used on is checked then', async () => {
		stored([GNOSIS]);
		seams.deployed = [GNOSIS];
		await recheckStored();
		await recheckStored();
		expect(seams.reads).toEqual([`code:${GNOSIS}`, `deployed:${GNOSIS}`]);
		multisig.accounts = [{ ...multisig.accounts[0], chainIds: [GNOSIS, BASE] }];
		await recheckStored();
		expect(seams.reads).toEqual([
			`code:${GNOSIS}`,
			`deployed:${GNOSIS}`,
			`code:${BASE}`,
			`counterfactual:${BASE}`
		]);
		expect(kept() ?? []).toEqual([]);
	});

	it('a network that does not answer changes nothing, and is asked again', async () => {
		stored([GNOSIS]);
		seams.deployed = [GNOSIS];
		seams.down = [GNOSIS];
		await recheckStored();
		expect(kept()).toBeUndefined();
		expect(await checkOn(multisig.find(SAFE)!, GNOSIS)).toBeNull();
		seams.down = [];
		seams.risks[GNOSIS] = [{ kind: 'guard', address: MODULE }];
		await recheckStored();
		expect(kept()).toEqual([
			{ from: 'chain', chainId: GNOSIS, risk: { kind: 'guard', address: MODULE } }
		]);
	});

	it("the core's 'cannot check yet' is not a verdict: nothing kept, asked again", async () => {
		stored([BASE]);
		seams.verdict[BASE] = { kind: 'unknown', unread: ['0x1'] };
		expect(await checkOn(multisig.find(SAFE)!, BASE)).toBeNull();
		expect(await checkOn(multisig.find(SAFE)!, BASE)).toBeNull();
		expect(seams.reads.filter((r) => r === `counterfactual:${BASE}`)).toHaveLength(2);
	});

	it('a network this device does not have is not read', async () => {
		stored([ELSEWHERE]);
		await recheckStored();
		expect(seams.reads).toEqual([]);
	});

	it('the chain is the authority: a risk gone from it is no longer said', async () => {
		stored([GNOSIS]);
		multisig.setFindings(SAFE, [
			{ from: 'chain', chainId: GNOSIS, risk: { kind: 'module', address: MODULE } }
		]);
		seams.deployed = [GNOSIS];
		await recheckStored();
		expect(kept()).toEqual([]);
	});
});
