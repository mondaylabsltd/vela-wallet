/**
 * What is still read and proposed here, not by the proposals machine: a
 * dApp's request from a multisig (`DappRequestHost`, moved onto the machine
 * next) — its calls drafted and proposed as one transaction — and the one
 * reading of a stored proposal (`readProposal`, `markCancellations`) the
 * dApp path and a link's preview use. The shown multisig's list, its
 * actions, the outbox, the poll, the kept list and the gate are the
 * machine's, and its suites drive them (rust/crates/vela-core/tests/
 * app_multisig_proposals.rs, app_multisig_presence.rs).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OWNER = '0x00000000000000000000000000000000000000aa';
const TO = '0x000000000000000000000000000000000000dEaD';

const errors = vi.hoisted(() => {
	class ServiceUnavailableError extends Error {}
	class ServiceBusyError extends ServiceUnavailableError {
		constructor(readonly waitMs: number | null) {
			super('busy');
		}
	}
	return { ServiceUnavailableError, ServiceBusyError };
});

const mailbox = {
	/**
	 * The cross-chain list (`GET /api/proposals/{safe}`): `false`, a service
	 * without it (an older build, which answers 404) — every chain is asked
	 * on its own; `true`, every proposal in one read.
	 */
	all: false,
	allReads: 0,
	/** A busy service (429) for writes: how long it says to wait; `null`: not busy. */
	busyMs: null as number | null,
	/** The genesis owner a Safe swapped out on Gnosis: approvals by it no longer count. */
	down: false,
	slowMs: 0,
	proposals: [] as unknown[],
	postDown: false,
	reads: 0,
	probes: 0,
	url: 'https://safe-genesis.getvela.app'
};

// ms-19: the outbox lives in localStorage, which the server project lacks.
const storage = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
});
const signed: string[] = [];
const submitted: unknown[] = [];

/** A chain the service does not read (not in its health table): Arbitrum. */
const UNREAD = 42161;
/** A network the multisig is not deployed on (its genesis's members there): Base. */
const UNDEPLOYED = 8453;
/** r4b-3: networks whose RPC does not answer. */
const rpc = vi.hoisted(() => ({ down: [] as number[], memberReads: 0 }));
vi.mock('./chain', () => ({
	hasCode: async () => true,
	// Read through the wallet's own RPC: on Arbitrum the Safe is deployed, at nonce 7.
	readMembers: async (chainId: number) =>
		(rpc.memberReads += 1) && rpc.down.includes(chainId)
			? Promise.reject(new Error(`chain ${chainId} unavailable`))
			: chainId === UNREAD
				? { owners: [OWNER], threshold: 1, nonce: 7n, deployed: true }
				: chainId === UNDEPLOYED
					? { owners: [OWNER], threshold: 1, nonce: 0n, deployed: false }
					: { owners: [OWNER], threshold: 2, nonce: 3n, deployed: true }
}));
vi.mock('./service', () => ({
	ServiceUnavailableError: errors.ServiceUnavailableError,
	ServiceBusyError: errors.ServiceBusyError,
	fetchAllProposals: async () => {
		mailbox.allReads += 1;
		if (!mailbox.all) return null;
		if (mailbox.down) throw new Error('mailbox down');
		return { proposals: mailbox.proposals, chains: [] };
	},
	genesisServiceURL: () => mailbox.url,
	probeService: async () => {
		mailbox.probes += 1;
		return !mailbox.down;
	},
	fetchProposals: async (chainId: number) => {
		mailbox.reads += 1;
		if (mailbox.slowMs) await new Promise((r) => setTimeout(r, mailbox.slowMs));
		if (mailbox.down) throw new Error('mailbox down');
		// Arbitrum's own proposals there; every other network gets the rest.
		const list = (mailbox.proposals as { chainId: number }[]).filter(
			(p) => (p.chainId === UNREAD) === (chainId === UNREAD)
		);
		// What the service says on a chain it does not read: the genesis's, at nonce 0.
		const members =
			chainId === UNREAD
				? {
						deployed: false,
						owners: ['0x00000000000000000000000000000000000000ee'],
						threshold: '2',
						nonce: '0x0'
					}
				: {};
		return { members, proposals: list };
	},
	submitApproval: async (req: { safeTx: unknown }) => {
		if (mailbox.busyMs !== null) throw new errors.ServiceBusyError(mailbox.busyMs);
		if (mailbox.postDown) throw new errors.ServiceUnavailableError('mailbox down');
		submitted.push(req.safeTx);
		return { ok: true, status: 'created', proposal: {} };
	}
}));
vi.mock('./kernel', () => ({
	safeTxHash: (_c: number, _s: string, tx: { nonce: string; to: string }) =>
		`0x${tx.nonce.slice(2).padStart(2, '0')}${'00'.repeat(31)}`,
	reviewTx: (_c: number, _s: string, tx: { to: string; value: string; data: string }) => ({
		safeTxHash: '0x',
		nonce: '0',
		action:
			tx.data === '0x7ea1a8e0'
				? { kind: 'rename', name: 'Team treasury' }
				: { kind: 'transfer', token: null, to: tx.to, amount: BigInt(tx.value).toString() },
		refusal: null
	}),
	// The core's rule (`multisig_networks` / `missing_networks`), as it is.
	multisigNetworks: (e: {
		deployed?: number[];
		proposals?: number[];
		kept?: number[];
		have?: number[];
	}) =>
		[...new Set([...(e.deployed ?? []), ...(e.proposals ?? []), ...(e.kept ?? [])])].filter(
			(id) => !e.have || e.have.includes(id)
		),
	missingNetworks: (e: { proposals?: number[]; have?: number[] }) =>
		e.have ? [...new Set(e.proposals ?? [])].filter((id) => !e.have!.includes(id)) : [],
	transfersTx: (t: { to: string; amount: string }[], nonce: string) => ({
		to: t[0].to,
		value: `0x${BigInt(t[0].amount).toString(16)}`,
		data: '0x',
		operation: 0,
		safeTxGas: '0x0',
		baseGas: '0x0',
		gasPrice: '0x0',
		gasToken: '0x0000000000000000000000000000000000000000',
		refundReceiver: '0x0000000000000000000000000000000000000000',
		nonce
	}),
	callsTx: (calls: { to: string; value: string; data: string }[], nonce: string) => ({
		to: calls.length > 1 ? '0x9641d764fc13c8B624c04430C7356C1C7C8102e2' : calls[0].to,
		value: calls.length > 1 ? '0x0' : calls[0].value,
		data: calls.length > 1 ? '0x8d80ff0a' : calls[0].data,
		operation: calls.length > 1 ? 1 : 0,
		safeTxGas: '0x0',
		baseGas: '0x0',
		gasPrice: '0x0',
		gasToken: '0x0000000000000000000000000000000000000000',
		refundReceiver: '0x0000000000000000000000000000000000000000',
		nonce
	}),
	cancelTx: () => ({}),
	namedGenesis: (g: unknown) => g,
	isEcdsaApproval: () => false,
	isOnChainApproval: (a: object) => 'onChain' in a,
	verifyApproval: () => true,
	verifyEcdsaApproval: () => ({}),
	sameAddress: (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase()
}));
/** r4C-16: owners the chain says approved (`approvedHashes`), by safeTxHash. */
const chainApproved = vi.hoisted(() => new Map<string, string[]>());
vi.mock('../external/onchain', () => ({
	onChainApprovals: async (_c: number, _s: string, hash: string, owners: string[]) =>
		(chainApproved.get(hash.toLowerCase()) ?? []).filter((o) =>
			owners.some((x) => x.toLowerCase() === o.toLowerCase())
		)
}));
vi.mock('./owners', () => ({
	localOwners: (owners: string[]) =>
		owners
			.filter((o) => o.toLowerCase() === OWNER)
			.map((address) => ({ address, name: 'Mine', account: { address } })),
	signApproval: async (_account: unknown, _chain: number, hash: string) => {
		signed.push(hash);
		return { owner: OWNER };
	}
}));
/** 2026-10-29: an executor whose account approves on the trusted page (`approval_route`). */
const pageRoute = vi.hoisted(() => ({ accounts: [] as { address: string }[] }));
vi.mock('$lib/onboarding/core/storage', () => ({ loadAccounts: () => pageRoute.accounts }));
vi.mock('$lib/core/kernels', async (original) => ({
	...(await original<typeof import('$lib/core/kernels')>()),
	multisigApprovalRoute: () => ({ type: 'page' })
}));
vi.mock('./records', () => ({
	recordRemoteExecutions: async () => ({ wrote: false, unreadable: [] })
}));
const sentExecutions = vi.hoisted(() => ({ count: 0 }));
vi.mock('./execute', () => ({
	submitExecution: async () => {
		sentExecutions.count += 1;
		return {};
	},
	// The look's own stoppers are `execute.test.ts`'s; here, a look that is
	// not a priced one stops, by its kind.
	throwIfStopped: (preview: { kind: string }) => {
		if (preview.kind !== 'priced') throw new Error(`stopped: ${preview.kind}`);
	}
}));
vi.mock('../external/signing.svelte', () => ({ externalSigning: { approve: async () => ({}) } }));
/**
 * §3.5: whether this device signs for it on a network — the gate's own rules
 * are `gate.test.ts`'s; here, only that every signature passes it first.
 * `refuse`: the gate throws (a refused network, or the risk declined).
 */
const gate = vi.hoisted(() => ({ asked: [] as number[], refuse: null as Error | null }));
vi.mock('./gate.svelte', () => ({
	authorizeSigning: async (_account: unknown, chainId: number) => {
		gate.asked.push(chainId);
		if (gate.refuse) throw gate.refuse;
	}
}));
const account = {
	address: SAFE,
	genesis: { factory: '', singleton: '', saltNonce: '0x1', initializer: '0x' },
	owners: [OWNER],
	threshold: 2,
	recorded: true,
	chainIds: [100],
	addedMs: 0
};
const used: number[] = [];
vi.mock('./store.svelte', () => ({
	multisig: { find: () => account, useChain: (_a: string, id: number) => used.push(id) }
}));

const { proposeCalls, draftCalls, readProposal, markCancellations } =
	await import('./proposals.svelte');
const { serviceOutage } = await import('./service-state.svelte');

function stored(nonce: number, chainId = 100) {
	const tx = {
		to: TO,
		value: '0x1',
		data: '0x',
		operation: 0,
		safeTxGas: '0x0',
		baseGas: '0x0',
		gasPrice: '0x0',
		gasToken: '0x0000000000000000000000000000000000000000',
		refundReceiver: '0x0000000000000000000000000000000000000000',
		nonce: `0x${nonce.toString(16)}`
	};
	return {
		chainId,
		safe: SAFE,
		safeTxHash: `0x${nonce.toString(16).padStart(2, '0')}${'00'.repeat(31)}`,
		safeTx: tx,
		proposer: OWNER,
		approvals: [],
		createdMs: 1,
		updatedMs: 1
	};
}

beforeEach(() => {
	mailbox.all = false;
	mailbox.allReads = 0;
	mailbox.busyMs = null;
	used.length = 0;
	mailbox.down = false;
	mailbox.slowMs = 0;
	mailbox.proposals = [];
	mailbox.postDown = false;
	mailbox.reads = 0;
	mailbox.probes = 0;
	mailbox.url = 'https://safe-genesis.getvela.app';
	serviceOutage.reset();
	storage.clear();
	signed.length = 0;
	submitted.length = 0;
	gate.asked.length = 0;
	gate.refuse = null;
	rpc.down = [];
});

describe("a dApp's calls, proposed", () => {
	it('several are ONE transaction (a MultiSendCallOnly batch), at the next number', async () => {
		mailbox.proposals = [stored(3)];
		const calls = [
			{ to: TO, value: '0x1', data: '0x' },
			{ to: OWNER, value: '0x0', data: '0x1234' }
		];
		const draft = await draftCalls(SAFE, 100, calls);
		expect(draft.item.tx.operation).toBe(1);
		expect(draft.item.nonce).toBe(4n);
		const { nonce } = await proposeCalls(SAFE, 100, calls);
		expect(nonce).toBe('0x4');
		expect(submitted).toHaveLength(1);
		expect(signed).toHaveLength(1);
	});
});

describe('who counts', () => {
	const STRANGER = '0x00000000000000000000000000000000000000ee';

	it("only the network's current owners: any other valid approval is shown, never counted", () => {
		const p = {
			...stored(3),
			approvals: [
				{ owner: STRANGER, createdMs: 1 },
				{ owner: OWNER, createdMs: 2 }
			]
		};
		const members = { owners: [OWNER], threshold: 2, nonce: 3n, deployed: true };
		const item = readProposal(account as never, members, p as never)!;
		expect(item.approvals.map((a) => [a.owner, a.counts])).toEqual([
			[STRANGER, false],
			[OWNER, true]
		]);
		expect(item.approved).toBe(1);
		expect(item.status).toBe('pending');
		// The stranger an owner now (swapped in on this network): it counts.
		const swapped = readProposal(
			account as never,
			{ ...members, owners: [OWNER, STRANGER] },
			p as never
		)!;
		expect(swapped.approved).toBe(2);
		expect(swapped.status).toBe('ready');
	});

	it('a rename where it is not deployed cannot be executed there: a rename never deploys', () => {
		const p = stored(0, UNDEPLOYED);
		p.safeTx.data = '0x7ea1a8e0';
		p.approvals = [{ owner: OWNER, createdMs: 1 }] as never;
		const undeployed = { owners: [OWNER], threshold: 1, nonce: 0n, deployed: false };
		const item = readProposal(account as never, undeployed, p as never)!;
		expect(item.status).toBe('ready');
		expect(item.renameUndeployed).toBe(true);
		expect(item.canExecute).toEqual([]);
		const deployed = readProposal(account as never, { ...undeployed, deployed: true }, p as never)!;
		expect(deployed.renameUndeployed).toBe(false);
		expect(deployed.canExecute).toHaveLength(1);
	});

	it('r4C-16: an owner that approved on chain (approveHash) counts — read from the chain, never the mailbox', async () => {
		const OTHER = '0x00000000000000000000000000000000000000bb';
		const p = { ...stored(3), approvals: [{ owner: OWNER, createdMs: 1 }] };
		const members = { owners: [OWNER, OTHER], threshold: 2, nonce: 3n, deployed: true };
		// The mailbox claiming an on-chain approval is not believed.
		const claimed = readProposal(account as never, members, {
			...p,
			approvals: [...p.approvals, { owner: OTHER, onChain: true, createdMs: 2 }]
		} as never)!;
		expect(claimed.approved).toBe(1);
		// The chain's word is.
		const item = readProposal(account as never, members, p as never, [OTHER])!;
		expect(item.approved).toBe(2);
		expect(item.status).toBe('ready');
		expect(item.approvals.find((a) => a.owner === OTHER)?.approval).toEqual({
			owner: OTHER,
			onChain: true
		});
		// A stranger's on-chain approval is shown, never counted.
		const stranger = readProposal(account as never, members, p as never, [
			'0x00000000000000000000000000000000000000ee'
		])!;
		expect(stranger.approved).toBe(1);
	});

	it('r4C-16: readChain asks the chain about the members the mailbox has nothing from', async () => {
		const hash = (stored(3) as { safeTxHash: string }).safeTxHash.toLowerCase();
		chainApproved.set(hash, [OWNER]);
		const { readChain } = await import('./proposals.svelte');
		// The mailbox has OWNER's approval: the chain is not asked about it.
		mailbox.proposals = [{ ...stored(3), approvals: [{ owner: OWNER, createdMs: 1 }] }];
		let read = await readChain(account as never, 100);
		expect(read.items[0].approvals.map((a) => 'onChain' in a.approval)).toEqual([false]);
		// It has nothing from OWNER, who approved on chain: that counts.
		mailbox.proposals = [{ ...stored(3), approvals: [] }];
		read = await readChain(account as never, 100);
		expect(read.items[0].approvals).toMatchObject([
			{ owner: OWNER, counts: true, approval: { owner: OWNER, onChain: true } }
		]);
		expect(read.items[0].approved).toBe(1);
		chainApproved.clear();
	});
});

/**
 * r4b-11: the waiting list is kept per multisig across reloads — shown at
 * once on the next page, said as the last known, and kept while the mailbox
 * fails or is slow; never a source of a new proposal's number.
 */

describe('proposals at one number (r4b-6)', () => {
	it('each open one knows the others — only one of them can run', () => {
		const at = (hash: string, nonce: bigint, status = 'pending', kind = 'transfer') =>
			({
				safeTxHash: hash,
				nonce,
				status,
				review: { action: { kind } },
				cancelledBy: null,
				sameNonce: []
			}) as never;
		const items = [at('0xa', 5n), at('0xb', 5n), at('0xc', 6n), at('0xd', 5n, 'done')];
		markCancellations(SAFE, items);
		const by = Object.fromEntries(
			(items as { safeTxHash: string; sameNonce: string[] }[]).map((i) => [
				i.safeTxHash,
				i.sameNonce
			])
		);
		expect(by).toEqual({ '0xa': ['0xb'], '0xb': ['0xa'], '0xc': [], '0xd': [] });
	});
});
