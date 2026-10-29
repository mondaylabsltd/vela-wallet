/**
 * r4a-13: creating a multisig keeps it on this device first — the moment
 * the passkey answers, the account and the creator's proof are stored, before
 * the service is asked — so a reload, a closed tab or a service that is down
 * never loses it; the record is asked for again by the loop. r4a-12: a
 * service that is away is no failure of the creation; one that refuses is
 * said, with the multisig still kept. r4a-14: one member is a multisig too
 * (1 of 1), as the core's rule allows.
 *
 * The core is the real one (wasm).
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';

interface Row {
	address: string;
	recorded: boolean;
	shared?: boolean;
	chainIds: number[];
	members?: Record<string, unknown>;
	firstRun?: boolean;
}

const seams = vi.hoisted(() => ({
	/** What the service's record call does. */
	record: 'recorded' as 'recorded' | 'down' | 'refused' | 'hang',
	release: null as (() => void) | null
}));
const store = vi.hoisted(() => ({ rows: [] as Row[], calls: [] as string[] }));
const kept = vi.hoisted(() => new Map<string, string>());

vi.mock('./store.svelte', () => ({
	multisig: {
		get accounts() {
			return store.rows;
		},
		find: (a: string) => store.rows.find((r) => r.address.toLowerCase() === a.toLowerCase()),
		add: (address: string, _genesis: unknown, recorded: boolean, chainIds: number[]) => {
			store.calls.push('add');
			const row = { address, recorded, chainIds };
			store.rows.push(row);
			return row;
		},
		nameMembers: (a: string, members: Record<string, unknown>) => {
			store.calls.push('nameMembers');
			const row = store.rows.find((r) => r.address === a);
			if (row) row.members = members;
		},
		setFirstRun: (a: string, on: boolean) => {
			const row = store.rows.find((r) => r.address === a);
			if (row) row.firstRun = on;
		},
		activate: () => store.calls.push('activate'),
		markRecorded: (a: string) => {
			store.calls.push('markRecorded');
			const row = store.rows.find((r) => r.address === a);
			if (row) Object.assign(row, { recorded: true, shared: true });
		},
		markShared: (a: string) => {
			store.calls.push('markShared');
			const row = store.rows.find((r) => r.address === a);
			if (row) row.shared = true;
		}
	}
}));
vi.mock('./chain', () => ({ hasCode: async () => false }));
vi.mock('./safe-service', () => ({}));
vi.mock('./owners', () => ({
	signOwnerProof: async (_owner: unknown, multisig: string) => {
		store.calls.push('passkey');
		return { owner: A, multisig };
	}
}));
vi.mock('./service-state.svelte', () => ({ serviceOutage: { blocked: () => false } }));
vi.mock('./service', () => {
	class ServiceUnavailableError extends Error {}
	return {
		ServiceUnavailableError,
		fetchGenesis: async () => ({ found: false }),
		fetchTask: async () => null,
		importGenesis: async () => {
			store.calls.push('record');
			if (seams.record === 'down') throw new ServiceUnavailableError('down');
			if (seams.record === 'refused') throw new Error('proof refused');
			if (seams.record === 'hang') {
				await new Promise<void>((r) => (seams.release = r));
				throw new ServiceUnavailableError('gone');
			}
			return { status: 'recorded' };
		},
		serviceTable: async () => ({ chains: null, registryChains: null, homeChainId: 100 })
	};
});
vi.mock('$lib/services/networks', () => ({ networkForChainId: () => ({ chainId: 100 }) }));
vi.mock('$lib/services/rpc-pool', () => ({ poolRpcCall: async () => ({ result: '0x' }) }));
vi.stubGlobal('localStorage', {
	getItem: (k: string) => kept.get(k) ?? null,
	setItem: (k: string, v: string) => void kept.set(k, v),
	removeItem: (k: string) => void kept.delete(k)
});

let accounts: typeof import('./accounts');

beforeAll(async () => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	accounts = await import('./accounts');
});

beforeEach(() => {
	store.rows = [];
	store.calls = [];
	kept.clear();
	seams.record = 'recorded';
	seams.release = null;
});

const creator = { address: A, name: 'Alice', account: { address: A } } as never;
const choice = (owners = [A, B], threshold = 2) => ({
	owners,
	threshold,
	chainId: 5042002,
	creator,
	name: 'Team fund',
	members: { [B]: { name: 'Bob', kind: 'wallet' as const } }
});
const proofs = () => JSON.parse(kept.get('vela.multisigPendingProofs') ?? '{}') as object;

describe('creating keeps the multisig here first (r4a-13)', () => {
	it('stores the account and the proof the moment the passkey answers, before the service', async () => {
		seams.record = 'hang';
		const saved: string[] = [];
		const done = accounts.createMultisig(choice(), '0x7', (a) => saved.push(a.address));
		// The service has not answered: the multisig is already kept, the
		// proof with it — a reload now loses nothing.
		await vi.waitFor(() => expect(seams.release).not.toBeNull());
		expect(store.calls.slice(0, 4)).toEqual(['passkey', 'add', 'nameMembers', 'activate']);
		expect(store.rows).toHaveLength(1);
		expect(saved).toEqual([store.rows[0].address]);
		expect(Object.keys(proofs())).toEqual([store.rows[0].address.toLowerCase()]);
		expect(accounts.recordWaiting(store.rows[0].address)).toBe(true);
		// What the creator said of the members is kept with it (r4D-5).
		expect(store.rows[0].members).toEqual({ [B]: { name: 'Bob', kind: 'wallet' } });
		expect(store.rows[0].firstRun).toBe(true);
		seams.release?.();
		const outcome = await done;
		expect(outcome.refused).toBeNull();
		expect(outcome.account.recorded).toBe(false);
	});

	it('a service that is down is no failure: kept, not shared, its proof waiting for the loop', async () => {
		seams.record = 'down';
		const outcome = await accounts.createMultisig(choice(), '0x7');
		expect(outcome.refused).toBeNull();
		expect(outcome.account.recorded).toBe(false);
		expect(outcome.account.shared).toBeUndefined();
		expect(accounts.recordWaiting(outcome.account.address)).toBe(true);
	});

	it('recorded: shared, and its proof no longer kept', async () => {
		const outcome = await accounts.createMultisig(choice(), '0x7');
		expect(store.calls).toContain('markRecorded');
		expect(outcome.account.recorded).toBe(true);
		expect(accounts.recordWaiting(outcome.account.address)).toBe(false);
	});

	it('a refusal is said, and the multisig stays kept here', async () => {
		seams.record = 'refused';
		const outcome = await accounts.createMultisig(choice(), '0x7');
		expect(outcome.refused).toBe('proof refused');
		expect(store.rows).toHaveLength(1);
	});
});

describe('one member is a multisig too (r4a-14)', () => {
	it('the core makes a 1 of 1 genesis, and it is created', async () => {
		const outcome = await accounts.createMultisig({ ...choice([A], 1), members: {} }, '0x7');
		expect(outcome.account.address).toMatch(/^0x[0-9a-fA-F]{40}$/);
		const { genesisMembers } = await import('./kernel');
		const { multisigGenesis } = await import('./kernel');
		const { genesis } = multisigGenesis([A], 1, 'Solo', '0x7');
		expect(genesisMembers(genesis)).toEqual({ owners: [A], threshold: 1 });
	});
});
