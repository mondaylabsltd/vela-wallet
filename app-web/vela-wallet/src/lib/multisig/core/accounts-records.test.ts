/**
 * Registry v4: what a multisig's record is now, looked at once a session.
 *
 * - Its name is the record's on chain (the multisig may have renamed itself),
 *   read with this wallet's own RPC and checked by the core — the service's
 *   word only where the chain cannot be read here.
 * - A multisig the service holds no record of (one recorded in an earlier
 *   registry, which v4's service no longer reads) is not shared — said, and
 *   shared again on an owner's passkey — never left looking shared.
 * - Every genesis the wallet hands the service is named: v4 records nothing
 *   without a name, and the service drops an unnamed one sent with a proposal.
 *
 * The core is the real one (wasm); the chain's answer is registry v4's own
 * `genesisOf` reply from a Gnosis fork (`tests/vectors/multisig-genesis-of.hex`).
 */
import { readFileSync } from 'node:fs';
import { beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const VECTORS = '../../rust/crates/vela-core/tests/vectors';
const SAFE = '0x4851114Fca55f17B1a5fC76BcE6c59107D003359';
const A = '0x1111111111111111111111111111111111111111';
const B = '0x2222222222222222222222222222222222222222';

interface Row {
	address: string;
	genesis: { name?: string; [k: string]: unknown };
	recorded: boolean;
	shared?: boolean;
	chainIds: number[];
	addedMs: number;
}

const seams = vi.hoisted(() => ({
	/** The service's `GET /api/genesis` answer. */
	answer: null as unknown,
	/** The registry's `genesisOf` reply on chain, or `null`: the chain does not answer. */
	chain: null as string | null,
	networks: [100] as number[],
	imports: [] as Record<string, unknown>[],
	importStatus: 'queued' as string,
	proofs: 0
}));
const store = vi.hoisted(() => ({ rows: [] as Row[], calls: [] as string[] }));

vi.mock('./store.svelte', () => ({
	multisig: {
		get accounts() {
			return store.rows;
		},
		find: (a: string) => store.rows.find((r) => r.address.toLowerCase() === a.toLowerCase()),
		markRecorded: (a: string) => {
			store.calls.push('markRecorded');
			const row = store.rows.find((r) => r.address === a);
			if (row) Object.assign(row, { recorded: true, shared: true });
		},
		markShared: (a: string) => {
			store.calls.push('markShared');
			const row = store.rows.find((r) => r.address === a);
			if (row) row.shared = true;
		},
		markUnshared: (a: string) => {
			store.calls.push('markUnshared');
			const row = store.rows.find((r) => r.address === a);
			if (row) Object.assign(row, { recorded: false, shared: false });
		},
		setRecordedName: (a: string, name: string) => {
			store.calls.push(`setRecordedName:${name}`);
			const row = store.rows.find((r) => r.address === a);
			if (row) row.genesis = { ...row.genesis, name };
		}
	}
}));
vi.mock('./chain', () => ({ hasCode: async () => false }));
vi.mock('./safe-service', () => ({}));
vi.mock('./owners', () => ({
	signOwnerProof: async (_owner: unknown, multisig: string) => {
		seams.proofs += 1;
		return { owner: A, multisig };
	}
}));
vi.mock('./service-state.svelte', () => ({ serviceOutage: { blocked: () => false } }));
vi.mock('./service', () => ({
	ServiceUnavailableError: class extends Error {},
	fetchGenesis: async () => seams.answer,
	fetchTask: async () => null,
	importGenesis: async (req: Record<string, unknown>) => {
		seams.imports.push(req);
		return { status: seams.importStatus };
	},
	serviceTable: async () => ({ chains: null, registryChains: null, homeChainId: 100 })
}));
vi.mock('$lib/services/networks', () => ({
	networkForChainId: (id: number) => (seams.networks.includes(id) ? { chainId: id } : null)
}));
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: async (_method: string, params: [{ to: string; data: string }]) => {
		if (seams.chain === null) throw new Error('no answer');
		// The registry is asked, and asked about this multisig.
		expect(params[0].to).toBe('0xF412388bF908E510066d3Be5c704bd9A327f945d');
		expect(params[0].data.toLowerCase()).toBe(
			`0xf6913a5d${SAFE.slice(2).toLowerCase().padStart(64, '0')}`
		);
		return { result: seams.chain };
	}
}));
vi.stubGlobal('localStorage', {
	getItem: () => null,
	setItem: () => {},
	removeItem: () => {}
});

let genesis: Row['genesis'];
const HOUR = 3_600_000;
let accounts: typeof import('./accounts');

beforeAll(async () => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	const { multisigGenesis } = await import('./kernel');
	genesis = multisigGenesis([A, B], 2, 'Team fund', '0x7').genesis as unknown as Row['genesis'];
});

// A session per test: `checkRecords` looks at each multisig once a session.
beforeEach(async () => {
	vi.resetModules();
	const { initSync: init } = await import('../../../../../../rust/pkg-web/vela_core.js');
	init({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	accounts = await import('./accounts');
});
const checkRecords = () => accounts.checkRecords();
const retryUnrecorded = () => accounts.retryUnrecorded();
const shareAgain = (...a: Parameters<typeof accounts.shareAgain>) => accounts.shareAgain(...a);

function row(over: Partial<Row> = {}): Row {
	return {
		address: SAFE,
		genesis: { ...genesis },
		recorded: true,
		shared: true,
		chainIds: [100],
		addedMs: Date.now() - HOUR,
		...over
	};
}

beforeEach(() => {
	store.calls = [];
	seams.imports = [];
	seams.importStatus = 'queued';
	seams.proofs = 0;
	seams.networks = [100];
	seams.chain = readFileSync(`${VECTORS}/multisig-genesis-of.hex`, 'utf8').trim();
	seams.answer = {
		found: true,
		recordedOn: [100],
		readFrom: 100,
		genesis: { ...genesis, name: 'Team fund' }
	};
});

const fresh = row;

describe('what a record is now (registry v4)', () => {
	it("takes its name from the chain's own record: a rename shows here", async () => {
		store.rows = [fresh({ genesis: { ...genesis, name: 'Old name' } })];
		// The service still serves the old name (its cache): the chain says otherwise.
		seams.answer = {
			found: true,
			recordedOn: [100],
			readFrom: 100,
			genesis: { ...genesis, name: 'Old name' }
		};
		await checkRecords();
		expect(store.calls).toEqual(['setRecordedName:Team fund']);
		expect(store.rows[0].genesis.name).toBe('Team fund');
	});

	it("takes the service's word only where the chain cannot be read here", async () => {
		store.rows = [fresh({ genesis: { ...genesis, name: 'Old name' } })];
		seams.networks = [];
		seams.answer = {
			found: true,
			recordedOn: [100],
			readFrom: 100,
			genesis: { ...genesis, name: 'From service' }
		};
		await checkRecords();
		expect(store.calls).toEqual(['setRecordedName:From service']);
	});

	it('never takes a name Vela would not show', async () => {
		store.rows = [fresh({ genesis: { ...genesis, name: 'Old name' } })];
		seams.networks = [];
		const reversed = `Team${String.fromCharCode(0x202e)}dnuf`;
		seams.answer = { found: true, recordedOn: [100], genesis: { ...genesis, name: reversed } };
		await checkRecords();
		expect(store.calls).toEqual([]);
	});

	it('nor one that comes with a genesis that is not this address', async () => {
		store.rows = [fresh({ genesis: { ...genesis, name: 'Old name' } })];
		seams.answer = {
			found: true,
			recordedOn: [100],
			genesis: { ...genesis, saltNonce: '0x8', name: 'Elsewhere' }
		};
		await checkRecords();
		expect(store.calls).toEqual([]);
	});

	it('is not shared when the service holds no record of it (an earlier registry)', async () => {
		store.rows = [fresh()];
		seams.answer = { found: false };
		await checkRecords();
		expect(store.calls).toEqual(['markUnshared']);
		expect(store.rows[0]).toMatchObject({ recorded: false, shared: false });
	});

	it('gives a record queued moments ago the time to land', async () => {
		store.rows = [fresh({ addedMs: Date.now() - 1000 })];
		seams.answer = { found: false };
		await checkRecords();
		expect(store.calls).toEqual([]);
	});
});

describe('sharing again', () => {
	it("asks once a session without a proof, with the record's name", async () => {
		store.rows = [
			fresh({ recorded: false, shared: false, genesis: { ...genesis, name: undefined } })
		];
		seams.answer = { found: false };
		seams.importStatus = 'resolved';
		await retryUnrecorded();
		await retryUnrecorded();
		expect(seams.imports).toHaveLength(1);
		const sent = seams.imports[0] as { genesis: { name: string }; ownerProof?: unknown };
		// Unnamed here: sent with the name the service would give it, never empty.
		expect(sent.genesis.name).toMatch(/^[A-Z][a-z]+ [A-Z][a-z]+$/);
		expect(sent.ownerProof).toBeUndefined();
		expect(store.calls).toEqual([]);
	});

	it("an owner's passkey shares it again: one proof, a named genesis, shared", async () => {
		const account = fresh({ recorded: false, shared: false });
		store.rows = [account];
		const owner = { address: A, name: 'One', account: { address: A } };
		expect(await shareAgain(account as never, owner as never)).toBe(true);
		expect(seams.proofs).toBe(1);
		expect(seams.imports).toHaveLength(1);
		expect(seams.imports[0]).toMatchObject({
			chainId: 100,
			safe: account.address,
			genesis: { name: 'Team fund' },
			ownerProof: { owner: A }
		});
		expect(store.calls).toEqual(['markShared']);
	});

	it('records on the index chain whatever network it is used on — one ask, no service chain needed', async () => {
		// Used on Arbitrum only, which the service does not read.
		const account = fresh({ recorded: false, shared: false, chainIds: [42161] });
		store.rows = [account];
		const owner = { address: A, name: 'One', account: { address: A } };
		await shareAgain(account as never, owner as never);
		expect(seams.imports).toEqual([expect.objectContaining({ chainId: 100 })]);
		seams.imports = [];
		seams.answer = { found: false };
		await retryUnrecorded();
		expect(seams.imports).toEqual([expect.objectContaining({ chainId: 100 })]);
	});

	it('is shared when the service keeps what the owner proved, though its sponsor will not pay now', async () => {
		const account = fresh({ recorded: false, shared: false });
		store.rows = [account];
		seams.importStatus = 'resolved';
		seams.answer = { found: true, pending: true, genesis };
		const owner = { address: A, name: 'One', account: { address: A } };
		expect(await shareAgain(account as never, owner as never)).toBe(true);
		expect(store.calls).toEqual(['markShared']);
		// Neither paid for nor kept: not shared, and the person is told.
		store.calls = [];
		seams.answer = { found: false };
		expect(await shareAgain(account as never, owner as never)).toBe(false);
		expect(store.calls).toEqual([]);
	});
});
