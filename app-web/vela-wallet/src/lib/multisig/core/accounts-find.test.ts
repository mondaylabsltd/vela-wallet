/**
 * ms-21: a multisig can be read by its address without being kept — a
 * link's transaction on a device with none of its members adds nothing and
 * switches nothing. Keeping it is a tap of its own, and does not show it;
 * the add sheet's Add still adds and shows.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OWNERS = ['0x00000000000000000000000000000000000000aa'];

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
vi.mock('./chain', () => ({ hasCode: async () => true }));
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
	fetchGenesis: async () => ({ found: true, pending: false, genesis: { owners: OWNERS } })
}));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: async () => ({ result: '0x0' }) }));

const { addMultisig, findMultisig, keepMultisig } = await import('./accounts');

beforeEach(() => {
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
