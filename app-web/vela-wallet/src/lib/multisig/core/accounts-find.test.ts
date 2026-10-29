/**
 * ms-21: a multisig can be read by its address without being kept — a
 * link's transaction on a device with none of its members adds nothing and
 * switches nothing. Keeping it is a tap of its own, and does not show it;
 * the add sheet's Add still adds and shows.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OWNERS = ['0x00000000000000000000000000000000000000aa'];

const seams = vi.hoisted(() => ({
	deployed: true,
	recordedOn: undefined as number[] | undefined,
	table: null as { chains: Set<number> | null; homeChainId: number | null } | null
}));
const store = vi.hoisted(() => ({
	rows: [] as { address: string; shared?: boolean }[],
	active: null as string | null,
	calls: [] as string[]
}));
vi.mock('./store.svelte', () => ({
	multisig: {
		get accounts() {
			return store.rows;
		},
		find: (a: string) => store.rows.find((r) => r.address.toLowerCase() === a.toLowerCase()),
		add: (address: string) => {
			store.calls.push('add');
			const row = { address };
			store.rows.push(row);
			return row;
		},
		markShared: (address: string) => {
			store.calls.push('markShared');
			const row = store.rows.find((r) => r.address === address);
			if (row) row.shared = true;
		},
		activate: (address: string | null) => {
			store.calls.push('activate');
			store.active = address;
		}
	}
}));
vi.mock('./chain', () => ({ hasCode: async () => seams.deployed }));
vi.mock('./safe-service', () => ({}));
vi.mock('./kernel', () => ({
	genesisAddress: () => SAFE,
	genesisMembers: () => ({ owners: OWNERS, threshold: 1 }),
	sameAddress: (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase()
}));
vi.mock('./owners', () => ({}));
vi.mock('./service', () => ({
	ServiceUnavailableError: class extends Error {},
	fetchGenesis: async () => ({
		found: true,
		pending: false,
		genesis: { owners: OWNERS },
		recordedOn: seams.recordedOn
	}),
	serviceTable: async () => seams.table
}));
// The wallet's networks: Gnosis, Arc Testnet and Sepolia; chain 999 is none of them.
vi.mock('$lib/services/networks', () => ({
	networkForChainId: (id: number) =>
		[100, 5042002, 11155111].includes(id) ? { chainId: id } : null
}));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: async () => ({ result: '0x0' }) }));

const { addMultisig, findMultisig, keepMultisig, recordHomeChain, REGISTRY_HOME_CHAIN_ID } =
	await import('./accounts');

beforeEach(() => {
	seams.deployed = true;
	seams.recordedOn = undefined;
	seams.table = null;
	store.rows = [];
	store.active = null;
	store.calls = [];
});

describe('reading a multisig by its address (ms-21)', () => {
	it('finds and checks it, and keeps nothing', async () => {
		const found = await findMultisig(SAFE, [100]);
		expect(found).toMatchObject({
			ok: true,
			account: { address: SAFE, owners: OWNERS, threshold: 1, recorded: true, chainIds: [100] }
		});
		expect(store.calls).toEqual([]);
		expect(store.rows).toEqual([]);
		expect(store.active).toBeNull();
	});

	it('kept on a tap: in the list, not shown', async () => {
		const found = await findMultisig(SAFE, [100]);
		if (!found.ok) throw new Error('not found');
		keepMultisig(found.account);
		expect(store.rows.map((r) => r.address)).toEqual([SAFE]);
		expect(store.calls).toEqual(['add', 'markShared']);
		expect(store.active).toBeNull();
	});

	it('the add sheet’s Add still adds it and shows it', async () => {
		const added = await addMultisig(SAFE, [100]);
		expect(added.ok).toBe(true);
		expect(store.calls).toEqual(['add', 'markShared', 'activate']);
		expect(store.active).toBe(SAFE);
	});
});

/**
 * Where a multisig added by address is used comes from what says so — the
 * chains it is deployed on, then the ones its record is on — never the first
 * network of the space shown; when nothing says, the person is asked.
 */
describe('the networks of a multisig added by address', () => {
	const SPACE = [11155111, 5042002];

	it('an undeployed one is used where its record is, not on the first network shown', async () => {
		seams.deployed = false;
		seams.recordedOn = [5042002];
		const found = await findMultisig(SAFE, SPACE);
		expect(found).toMatchObject({ ok: true, account: { chainIds: [5042002] } });
	});

	it('a deployed one is used where it is deployed, then where it is recorded', async () => {
		seams.recordedOn = [100];
		const found = await findMultisig(SAFE, SPACE);
		expect(found).toMatchObject({ ok: true, account: { chainIds: [11155111, 5042002, 100] } });
	});

	it('when nothing says (a record on its way, or on a network this wallet lacks), asks', async () => {
		seams.deployed = false;
		for (const recordedOn of [[], undefined, [999]]) {
			seams.recordedOn = recordedOn;
			expect(await findMultisig(SAFE, SPACE)).toEqual({ ok: false, reason: 'chooseNetwork' });
		}
		// Nothing is added while the question is open.
		expect(await addMultisig(SAFE, SPACE)).toEqual({ ok: false, reason: 'chooseNetwork' });
		expect(store.rows).toEqual([]);
		// The person's pick is where it is used.
		const added = await addMultisig(SAFE, SPACE, 5042002);
		expect(added.ok).toBe(true);
		expect(store.active).toBe(SAFE);
		const picked = await findMultisig(SAFE, SPACE, 5042002);
		expect(picked).toMatchObject({ ok: true, account: { chainIds: [5042002] } });
	});
});

/** A record on a chain the service does not serve goes to the service's own home chain. */
describe('where a record falls back to', () => {
	it("is the service's home chain, as it says", async () => {
		seams.table = { chains: new Set([5042002]), homeChainId: 5042002 };
		expect(await recordHomeChain()).toBe(5042002);
	});

	it("is the default service's (Gnosis) only when the service says nothing", async () => {
		seams.table = { chains: null, homeChainId: null };
		expect(await recordHomeChain()).toBe(REGISTRY_HOME_CHAIN_ID);
		seams.table = null;
		expect(await recordHomeChain()).toBe(REGISTRY_HOME_CHAIN_ID);
	});
});
