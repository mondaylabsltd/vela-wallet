/**
 * ms-21: a multisig can be read by its address without being kept — a
 * link's transaction on a device with none of its members adds nothing and
 * switches nothing. Keeping it is a tap of its own, and does not show it;
 * the add sheet's Add still adds and shows.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OWNERS = ['0x00000000000000000000000000000000000000aa'];

const seams = vi.hoisted(() => ({
	/** The chains it has code on; `null`: every one asked. */
	deployedOn: null as number[] | null,
	recordedOn: undefined as number[] | undefined,
	/** The chains the mailbox holds proposals of it for. */
	proposedOn: [] as number[],
	/** Chains whose mailbox read fails. */
	mailboxDown: [] as number[],
	mailboxReads: 0,
	/**
	 * The cross-chain list: `null`, a service without it (an older build); else
	 * the chains it holds proposals on — any chain, this device's or not.
	 */
	listedOn: null as number[] | null,
	listReads: 0,
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
		},
		useChain: (address: string, chainId: number) => store.calls.push(`useChain:${chainId}`)
	}
}));
vi.mock('./chain', () => ({
	hasCode: async (chainId: number) =>
		seams.deployedOn === null || seams.deployedOn.includes(chainId)
}));
vi.mock('./safe-service', () => ({}));
// The core's own rule for which networks it is on (`multisigNetworks`); the
// genesis is taken as deriving the address.
vi.mock('./kernel', async (importOriginal) => ({
	...(await importOriginal<typeof import('./kernel')>()),
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
	fetchProposals: async (chainId: number) => {
		seams.mailboxReads += 1;
		if (seams.mailboxDown.includes(chainId)) throw new Error('away');
		return {
			members: {},
			proposals: seams.proposedOn.includes(chainId) ? [{ chainId }] : []
		};
	},
	fetchAllProposals: async () => {
		seams.listReads += 1;
		if (seams.listedOn === null) return null;
		return {
			proposals: seams.listedOn.map((chainId) => ({ chainId })),
			chains: seams.listedOn.map((chainId) => ({ chainId, count: 1 }))
		};
	},
	servedChains: async () => seams.table?.chains ?? null,
	serviceTable: async () => seams.table
}));
// The wallet's networks: Gnosis, Arc Testnet and Sepolia; chain 999 is none of them.
vi.mock('$lib/services/networks', () => ({
	networkForChainId: (id: number) =>
		[100, 5042002, 11155111].includes(id) ? { chainId: id } : null
}));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: async () => ({ result: '0x0' }) }));

const {
	addMultisig,
	discoverChains,
	findMultisig,
	keepMultisig,
	recordHomeChain,
	REGISTRY_HOME_CHAIN_ID
} = await import('./accounts');

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

beforeEach(() => {
	seams.deployedOn = null;
	seams.recordedOn = undefined;
	seams.proposedOn = [];
	seams.mailboxDown = [];
	seams.mailboxReads = 0;
	seams.listedOn = null;
	seams.listReads = 0;
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
 * chains it is deployed on, those the mailbox holds proposals of it for, the
 * network a link or the person names — never the chain its record is on (the
 * registry is an index, not a home), nor the first network of the space
 * shown. When nothing says, the person is asked.
 */
describe('the networks of a multisig added by address', () => {
	const ARBITRUM = 42161;
	const SPACE = [11155111, 5042002, ARBITRUM];

	it('deployed on Arbitrum: Arbitrum — though its record is on Gnosis', async () => {
		seams.deployedOn = [ARBITRUM];
		seams.recordedOn = [100];
		const found = await findMultisig(SAFE, SPACE);
		expect(found).toMatchObject({ ok: true, account: { chainIds: [ARBITRUM] } });
	});

	it('undeployed, with proposals in the mailbox on a network: that network', async () => {
		seams.deployedOn = [];
		seams.recordedOn = [100];
		seams.proposedOn = [5042002];
		const found = await findMultisig(SAFE, SPACE);
		expect(found).toMatchObject({ ok: true, account: { chainIds: [5042002] } });
	});

	it('deployed and named: each once, in that order — the mailbox not asked per network', async () => {
		seams.deployedOn = [ARBITRUM];
		seams.proposedOn = [5042002, ARBITRUM];
		const found = await findMultisig(SAFE, SPACE, 11155111);
		expect(found).toMatchObject({ ok: true, account: { chainIds: [ARBITRUM, 11155111] } });
		expect(seams.mailboxReads).toBe(0);
	});

	it('the cross-chain list: one read, every network it names that this device has', async () => {
		seams.deployedOn = [];
		// A member's own network (99999) this device lacks: never one of its networks here.
		seams.listedOn = [99999, 5042002];
		const found = await findMultisig(SAFE, SPACE);
		expect(found).toMatchObject({ ok: true, account: { chainIds: [5042002] } });
		expect(seams.listReads).toBe(1);
		expect(seams.mailboxReads).toBe(0);
	});

	it('the cross-chain list naming only networks this device lacks: asks', async () => {
		seams.deployedOn = [];
		seams.listedOn = [99999];
		expect(await findMultisig(SAFE, SPACE)).toEqual({ ok: false, reason: 'chooseNetwork' });
		expect(seams.mailboxReads).toBe(0);
	});

	it('when nothing says (its record on the registry chain is not a home), asks', async () => {
		seams.deployedOn = [];
		// A mailbox that cannot be read on a network says nothing of it.
		seams.mailboxDown = [ARBITRUM];
		for (const recordedOn of [[100], [], undefined, [1, 100]]) {
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

/** Every record goes to the index chain: the service's own home chain. */
describe('where a record goes', () => {
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

/**
 * A multisig this device has: another member may have deployed it, or
 * proposed, on a network this device never used it on. Once a session the
 * wallet looks, and such a network becomes one of its networks here.
 */
describe('networks another member made it used on', () => {
	it('are found once a session where it is deployed, with no mailbox read per network', async () => {
		seams.deployedOn = [42161];
		seams.proposedOn = [8453];
		const account = { address: SAFE, chainIds: [100] };
		await discoverChains(account as never, [100, 42161, 8453, 10]);
		expect(store.calls).toEqual(['useChain:42161']);
		expect(seams.mailboxReads).toBe(0);
		store.calls = [];
		await discoverChains(account as never, [100, 42161, 8453, 10]);
		expect(store.calls).toEqual([]);
	});
});
