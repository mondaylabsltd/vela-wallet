/**
 * §3.5: a multisig on this device checked again, on the network a sheet is
 * about (`checkOn`, the proposal sheet's and the signing gate's read): the
 * chain's state where it is deployed, the core's counterfactual rule where
 * it is not. A verdict is kept for the session and handed to the accounts
 * machine (its `findings_on`); "cannot check yet" is no verdict.
 *
 * Every stored multisig's own re-check — once a session, on each network it
 * comes to be used on, a network that does not answer asked again — is the
 * core accounts machine's `recheck` (Rust: `app_multisig_accounts.rs`,
 * `rechecked_once_a_session_and_again_where_unanswered`,
 * `a_module_added_since_is_found_and_one_gone_is_not_said`,
 * `a_network_it_comes_to_be_used_on_is_checked_then`).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const MODULE = '0x9813461E88aDD60D0eC0943B74A83368CA2E36D1';
const GNOSIS = 100;
const BASE = 8453;

const seams = vi.hoisted(() => ({
	deployed: [] as number[],
	down: [] as number[],
	risks: {} as Record<number, unknown[]>,
	verdict: {} as Record<number, unknown>,
	reads: [] as string[],
	written: [] as [string, number, unknown[]][]
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
vi.mock('./store.svelte', () => ({
	multisig: {
		accounts: [],
		find: (address: string) => (address === SAFE ? account : undefined),
		setFindingsOn: (address: string, chainId: number, found: unknown[]) =>
			void seams.written.push([address, chainId, found])
	}
}));

const account = {
	address: SAFE,
	genesis: { factory: '', singleton: '', saltNonce: '0x1', initializer: '0x' },
	owners: [],
	threshold: 1,
	recorded: true,
	chainIds: [GNOSIS, BASE],
	addedMs: 0
};

const { checkOn, checkedOn, forgetChecks } = await import('./recheck');

beforeEach(() => {
	forgetChecks();
	seams.deployed = [];
	seams.down = [];
	seams.risks = {};
	seams.verdict = {};
	seams.reads = [];
	seams.written = [];
});

describe('a multisig checked again on one network (§3.5)', () => {
	it('deployed with a module: the risk, once a session, handed to the accounts machine', async () => {
		seams.deployed = [GNOSIS];
		seams.risks[GNOSIS] = [{ kind: 'module', address: MODULE }];
		const risk = { from: 'chain', chainId: GNOSIS, risk: { kind: 'module', address: MODULE } };
		expect(await checkOn(account, GNOSIS)).toEqual([risk]);
		expect(await checkOn(account, GNOSIS)).toEqual([risk]);
		expect(seams.reads).toEqual([`code:${GNOSIS}`, `deployed:${GNOSIS}`]);
		expect(seams.written).toEqual([[SAFE, GNOSIS, [risk]]]);
		expect(checkedOn(SAFE, GNOSIS)).toBe(true);
	});

	it('a network that does not answer changes nothing, and is asked again', async () => {
		seams.down = [GNOSIS];
		expect(await checkOn(account, GNOSIS)).toBeNull();
		expect(seams.written).toEqual([]);
		seams.down = [];
		seams.deployed = [GNOSIS];
		expect(await checkOn(account, GNOSIS)).toEqual([]);
		expect(seams.written).toEqual([[SAFE, GNOSIS, []]]);
	});

	it("the core's 'cannot check yet' is not a verdict: nothing kept, asked again", async () => {
		seams.verdict[BASE] = { kind: 'unknown', unread: ['0x1'] };
		expect(await checkOn(account, BASE)).toBeNull();
		expect(await checkOn(account, BASE)).toBeNull();
		expect(seams.reads.filter((r) => r === `counterfactual:${BASE}`)).toHaveLength(2);
		expect(seams.written).toEqual([]);
	});
});
