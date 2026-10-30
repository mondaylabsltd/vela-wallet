/**
 * The mailbox, when it cannot be read (ms-1, ms-2): nothing is proposed
 * with a number taken from a list that is not there, and what is shown says
 * it may be stale — the last transactions read stay, never "No activity yet".
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
/** A network this device does not have (a member's own). */
const ELSEWHERE = 99999;
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
/**
 * §3.6: the proposer's own review before its passkey (`review-first`'s own
 * rules are its test's); here it is confirmed, and each one asked is kept.
 * `decline`: the person declines it.
 */
const firstReview = vi.hoisted(() => ({
	asked: [] as { chainId: number; safeTxHash: string }[],
	decline: false
}));
vi.mock('./review-first.svelte', () => ({
	reviewFirst: async (
		_address: string,
		chainId: number,
		_chain: unknown,
		item: { safeTxHash: string }
	) => {
		firstReview.asked.push({ chainId, safeTxHash: item.safeTxHash });
		if (firstReview.decline) throw new DOMException('declined', 'AbortError');
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

const {
	approve,
	approveExternally,
	execute,
	proposals,
	proposeTransfers,
	proposeCalls,
	draftCalls,
	flushOutbox,
	MailboxUnreadableError,
	OUTBOX_PATIENCE_MS,
	outbox,
	readProposal,
	markCancellations,
	SLOW_REFRESH_MS,
	POLL_MS,
	OPEN_POLL_MS
} = await import('./proposals.svelte');
const { executions } = await import('./execution-watch.svelte');
const { serviceOutage, RETRY_STEPS_MS } = await import('./service-state.svelte');

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
	executions.resetForTests();
	proposals.follow(null, []);
});

describe('the proposer reviews it in full before its passkey (§3.6)', () => {
	it('a transfer from the send flow: its review at its number first; declined, nothing signed', async () => {
		firstReview.asked = [];
		const before = signed.length;
		await proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }]);
		expect(firstReview.asked).toHaveLength(1);
		expect(firstReview.asked[0].chainId).toBe(100);
		expect(signed).toHaveLength(before + 1);

		firstReview.decline = true;
		try {
			await expect(
				proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }])
			).rejects.toMatchObject({ name: 'AbortError' });
		} finally {
			firstReview.decline = false;
		}
		expect(firstReview.asked).toHaveLength(2);
		// The passkey was never asked.
		expect(signed).toHaveLength(before + 1);
	});

	it('a dApp’s calls were reviewed on their draft: not asked again', async () => {
		firstReview.asked = [];
		await proposeCalls(SAFE, 100, [{ to: TO, value: '0x0', data: '0x' }]);
		expect(firstReview.asked).toHaveLength(0);
	});
});

describe('every signature passes the gate first (§3.5)', () => {
	it('asks it for the network before the passkey, and signs nothing it stops', async () => {
		await proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }]);
		expect(gate.asked).toEqual([100]);
		expect(signed).toHaveLength(1);

		gate.refuse = new DOMException('declined', 'AbortError');
		await expect(
			proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }])
		).rejects.toMatchObject({ name: 'AbortError' });
		expect(gate.asked).toEqual([100, 100]);
		// Nothing signed, nothing sent: the passkey was never asked.
		expect(signed).toHaveLength(1);
		expect(submitted).toHaveLength(1);
	});

	it('approve, execute and an outside wallet’s approval: each asks it for the item’s network', async () => {
		const item = {
			chainId: 8453,
			safeTxHash: `0x${'11'.repeat(32)}`,
			tx: stored(1).safeTx,
			approvals: []
		};
		const owner = { address: OWNER, name: 'Mine', account: { address: OWNER } };
		gate.refuse = new Error('refused here');
		const multisigAccount = account as Parameters<typeof approve>[0];
		const proposal = item as unknown as Parameters<typeof approve>[1];
		const local = owner as unknown as Parameters<typeof approve>[2];
		await expect(approve(multisigAccount, proposal, local)).rejects.toThrow('refused here');
		await expect(
			execute(multisigAccount, proposal, local, {
				deployed: true,
				owners: [OWNER],
				threshold: 1,
				nonce: 0n
			})
		).rejects.toThrow('refused here');
		await expect(
			approveExternally(multisigAccount, 8453, stored(1).safeTx, [OWNER])
		).rejects.toThrow('refused here');
		expect(gate.asked).toEqual([8453, 8453, 8453]);
		expect(signed).toEqual([]);
		expect(sentExecutions.count).toBe(0);
		expect(submitted).toEqual([]);
	});

	const members = { deployed: true, owners: [OWNER], threshold: 1, nonce: 0n };
	const pending = () =>
		({
			chainId: 100,
			safeTxHash: `0x${'22'.repeat(32)}`,
			tx: stored(1).safeTx,
			approvals: [],
			canApprove: [{ address: OWNER }],
			review: { action: null }
		}) as unknown as Parameters<typeof approve>[1];
	const local = {
		address: OWNER,
		name: 'Mine',
		account: { address: OWNER }
	} as unknown as Parameters<typeof approve>[2];

	it('a look that already stops before the passkey (not checked in time) stops before the gate asks', async () => {
		const multisigAccount = account as Parameters<typeof approve>[0];
		await expect(
			execute(multisigAccount, pending(), local, members, { kind: 'unchecked' })
		).rejects.toThrow('stopped: unchecked');
		// No risk asked for an execution that was never going to be signed: the
		// approval a caller gives instead asks the gate once, as its own.
		expect(gate.asked).toEqual([]);
		expect(signed).toEqual([]);
		expect(sentExecutions.count).toBe(0);
	});

	it('r7-3: an executor that signs on its page goes there once — its operation, no approval first', async () => {
		pageRoute.accounts = [{ address: OWNER }];
		try {
			const multisigAccount = account as Parameters<typeof approve>[0];
			await execute(multisigAccount, pending(), local, members, {
				kind: 'priced'
			} as unknown as Parameters<typeof execute>[4]);
			expect(gate.asked).toEqual([100]);
			// Its `msg.sender` is its approval: nothing else is signed.
			expect(signed).toEqual([]);
			expect(sentExecutions.count).toBe(1);
		} finally {
			pageRoute.accounts = [];
		}
	});
});

describe('a mailbox that cannot be read', () => {
	it('proposes nothing — no passkey, no number taken from a missing list (ms-1)', async () => {
		mailbox.down = true;
		await expect(
			proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }])
		).rejects.toBeInstanceOf(MailboxUnreadableError);
		expect(signed).toEqual([]);
		expect(submitted).toEqual([]);
		await expect(
			draftCalls(SAFE, 100, [{ to: TO, value: '0x1', data: '0x' }])
		).rejects.toBeInstanceOf(MailboxUnreadableError);
	});

	it('takes the number after every open proposal once the list is read', async () => {
		mailbox.proposals = [stored(3), stored(4)];
		await proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }]);
		expect(submitted).toHaveLength(1);
		expect((submitted[0] as { nonce: string }).nonce).toBe('0x5');
		const draft = await draftCalls(SAFE, 100, [{ to: TO, value: '0x1', data: '0x' }]);
		expect(draft.item.nonce).toBe(5n);
		expect(draft.item.approvals).toEqual([]);
	});

	it('keeps the last transactions read and says the list may be stale (ms-2)', async () => {
		mailbox.proposals = [stored(3)];
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		expect(proposals.open).toHaveLength(1);
		expect(proposals.stale).toBe(false);

		mailbox.down = true;
		await proposals.refresh(account, [100]);
		expect(proposals.stale).toBe(true);
		// The waiting transaction is still shown, not dropped as if there were none.
		expect(proposals.open.map((i) => i.nonce)).toEqual([3n]);
		expect(proposals.chain(100)?.mailbox).toBe(false);

		mailbox.down = false;
		await proposals.refresh(account, [100]);
		expect(proposals.stale).toBe(false);
		expect(proposals.chain(100)?.mailbox).toBe(true);
	});

	it('says so while the mailbox is slow, before it has answered', async () => {
		vi.useFakeTimers();
		try {
			mailbox.slowMs = SLOW_REFRESH_MS + 5_000;
			proposals.follow(account, [100]);
			const run = proposals.refresh(account, [100]);
			await vi.advanceTimersByTimeAsync(SLOW_REFRESH_MS + 1);
			expect(proposals.stale).toBe(true);
			await vi.advanceTimersByTimeAsync(5_000);
			await run;
			expect(proposals.stale).toBe(false);
		} finally {
			proposals.follow(null, []);
			vi.useRealTimers();
		}
	});
});

describe('a multisig service that is away', () => {
	const chains = [100, 5042002, 1];

	it('asks no network until its next step, then one probe for all of them', async () => {
		mailbox.proposals = [stored(3)];
		proposals.follow(account, chains);
		await proposals.refresh(account, chains);
		expect(mailbox.reads).toBe(3);

		// Found away (by any request): the next rounds ask the mailbox nothing.
		serviceOutage.failed();
		mailbox.down = true;
		mailbox.reads = 0;
		await proposals.refresh(account, chains);
		expect(mailbox.reads).toBe(0);
		expect(mailbox.probes).toBe(0);
		// Said as the service away, over the last list — never "nothing waiting".
		expect(proposals.unreachable).toBe(true);
		expect(proposals.open.map((i) => i.nonce)).toEqual([3n, 3n, 3n]);

		// Its step came: one probe for the three networks, still away.
		serviceOutage.reset();
		serviceOutage.failed(Date.now() - RETRY_STEPS_MS[0]);
		await proposals.refresh(account, chains);
		expect(mailbox.probes).toBe(1);
		expect(mailbox.reads).toBe(0);
		expect(serviceOutage.blocked()).toBe(true);

		// The Retry: asked now, whatever the schedule — and it is back.
		mailbox.down = false;
		await proposals.retry();
		expect(mailbox.reads).toBe(3);
		expect(proposals.stale).toBe(false);
		expect(proposals.unreachable).toBe(false);
		proposals.follow(null, []);
	});

	it('a slow or partial read is still said as stale, not as the service away', async () => {
		mailbox.down = true;
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		// The mock's failure is not a network one: nothing marked the outage.
		expect(proposals.stale).toBe(true);
		expect(proposals.unreachable).toBe(false);
		proposals.follow(null, []);
	});

	it("forgets another service's list when the address changes", async () => {
		mailbox.proposals = [stored(3)];
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		expect(proposals.open).toHaveLength(1);
		mailbox.url = 'http://127.0.0.1:8788';
		mailbox.down = true;
		await proposals.refresh(account, [100]);
		expect(proposals.open).toEqual([]);
		expect(proposals.stale).toBe(true);
		proposals.follow(null, []);
	});
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

describe('a signed approval the mailbox has not taken (ms-19)', () => {
	it('is kept, and sent again when a page comes up', async () => {
		mailbox.postDown = true;
		// Signed, and not in the mailbox: proposed all the same — kept, and
		// shown as not sent yet, never "not made".
		const { safeTxHash } = await proposeTransfers(SAFE, 100, [
			{ to: TO, token: null, amount: '1' }
		]);
		expect(safeTxHash).toMatch(/^0x/);
		expect(signed).toHaveLength(1);
		expect(submitted).toEqual([]);
		expect([...storage.keys()]).toEqual(['vela.multisigOutbox']);

		// Still out of reach: kept for the next page.
		expect(await flushOutbox()).toEqual([]);
		expect(storage.size).toBe(1);

		mailbox.postDown = false;
		expect(await flushOutbox()).toEqual([SAFE]);
		expect(submitted).toHaveLength(1);
		expect(storage.size).toBe(0);
		// Once: nothing left to send.
		expect(await flushOutbox()).toEqual([]);
	});

	it('leaves nothing behind once the mailbox has it', async () => {
		await proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }]);
		expect(storage.size).toBe(0);
	});
});

/**
 * The mailbox takes proposals on any chain; the service reads only its own.
 * On a network it does not read, its word on members is the genesis's at
 * nonce 0 — the wallet never uses it: it reads the Safe's owners, threshold
 * and nonce itself, through its own RPC, and every network the multisig is
 * used on is polled alike.
 */
describe('proposals on a chain the service does not read', () => {
	it('are polled, and judged by the chain as this wallet reads it', async () => {
		mailbox.proposals = [stored(3), stored(6, UNREAD), stored(7, UNREAD), stored(8, UNREAD)];
		proposals.follow(account, [100, UNREAD]);
		await proposals.refresh(account, [100, UNREAD]);
		const arbitrum = proposals.chain(UNREAD);
		expect(arbitrum?.mailbox).toBe(true);
		// The chain's members, not the service's genesis-at-nonce-0.
		expect(arbitrum?.members).toMatchObject({ deployed: true, nonce: 7n, owners: [OWNER] });
		expect(arbitrum?.items.map((i) => [i.nonce, i.status])).toEqual([
			[6n, 'done'],
			[7n, 'pending'],
			[8n, 'queued']
		]);
		expect(proposals.chain(100)?.items.map((i) => i.nonce)).toEqual([3n]);
	});

	it('take the next number from the chain and the mailbox there', async () => {
		mailbox.proposals = [stored(7, UNREAD)];
		await proposeTransfers(SAFE, UNREAD, [{ to: TO, token: null, amount: '1' }]);
		expect((submitted[0] as { nonce: string }).nonce).toBe('0x8');
	});
});

/**
 * A write the service is over its limit for (`429`) is never a failure: the
 * signed approval is kept, the proposal shown from what was signed, and sent
 * again on the service's word for a while — then said as not sent, kept,
 * with Send again. Nothing is signed twice.
 */
describe('a busy service (429) on a write', () => {
	it('keeps what was signed, sends it again after the wait, and never signs twice', async () => {
		vi.useFakeTimers();
		try {
			mailbox.busyMs = 3_000;
			mailbox.proposals = [stored(3)];
			const { safeTxHash } = await proposeTransfers(SAFE, 100, [
				{ to: TO, token: null, amount: '1' }
			]);
			expect(signed).toHaveLength(1);
			expect(submitted).toEqual([]);
			const waiting = outbox.stateOf(100, SAFE, safeTxHash);
			expect(waiting?.why).toBe('busy');
			expect((waiting?.retryAt ?? 0) - Date.now()).toBe(3_000);
			// Shown from what was signed, as not sent yet: its number is taken.
			proposals.follow(account, [100]);
			await proposals.refresh(account, [100]);
			const shown = proposals.chain(100)?.items.find((i) => i.safeTxHash === safeTxHash);
			expect(shown?.unsent).toBe(true);
			expect(shown?.approvals[0]?.unsent).toBe(true);
			expect(shown?.nonce).toBe(4n);

			// Still busy at the first resend: waits again, as it says.
			await vi.advanceTimersByTimeAsync(3_000);
			expect(submitted).toEqual([]);
			mailbox.busyMs = null;
			await vi.advanceTimersByTimeAsync(3_000);
			expect(submitted).toHaveLength(1);
			expect(outbox.stateOf(100, SAFE, safeTxHash)).toBeNull();
			expect(storage.has('vela.multisigOutbox')).toBe(false);
			// Never signed again.
			expect(signed).toHaveLength(1);
		} finally {
			proposals.follow(null, []);
			vi.useRealTimers();
		}
	});

	it('after a while is said as not sent, kept, and sent on Send again', async () => {
		vi.useFakeTimers();
		try {
			mailbox.busyMs = 20_000;
			const { safeTxHash } = await proposeTransfers(SAFE, 100, [
				{ to: TO, token: null, amount: '1' }
			]);
			await vi.advanceTimersByTimeAsync(OUTBOX_PATIENCE_MS + 30_000);
			const left = outbox.stateOf(100, SAFE, safeTxHash);
			expect(left).toEqual({ why: 'busy', retryAt: null });
			expect(storage.has('vela.multisigOutbox')).toBe(true);
			const tries = submitted.length;
			await vi.advanceTimersByTimeAsync(60_000);
			// Not on its own any more: nothing goes until the person says.
			expect(submitted.length).toBe(tries);
			mailbox.busyMs = null;
			expect(await outbox.sendAgain({ chainId: 100, safe: SAFE, hash: safeTxHash })).toEqual([
				SAFE
			]);
			expect(submitted).toHaveLength(1);
			expect(outbox.stateOf(100, SAFE, safeTxHash)).toBeNull();
			expect(signed).toHaveLength(1);
		} finally {
			vi.useRealTimers();
		}
	});
});

/**
 * The cross-chain list: one read for every chain the mailbox holds proposals
 * on. A network this device has becomes one of the multisig's; one it lacks
 * is listed as missing — shown, never judged, never counted.
 */
describe('proposals found on every chain', () => {
	it('come in one read, reach networks the multisig was not used on, and list the ones missing', async () => {
		mailbox.all = true;
		mailbox.proposals = [
			stored(3),
			stored(0, UNDEPLOYED),
			stored(5, ELSEWHERE),
			stored(6, ELSEWHERE)
		];
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		expect(mailbox.allReads).toBe(1);
		// No read per network: the list had them all.
		expect(mailbox.reads).toBe(0);
		// Base, where it is not deployed, is one of its networks now.
		expect(used).toEqual([UNDEPLOYED]);
		expect(proposals.chain(UNDEPLOYED)?.items.map((i) => i.nonce)).toEqual([0n]);
		// The member's own network: missing, with how many wait there.
		expect(proposals.missing).toEqual([{ chainId: ELSEWHERE, count: 2 }]);
		// Nothing there is judged or counted: no chain state, no item, no turn.
		expect(proposals.chain(ELSEWHERE)).toBeUndefined();
		expect(proposals.open.some((i) => i.chainId === ELSEWHERE)).toBe(false);
		expect(proposals.open.map((i) => [i.chainId, i.nonce])).toEqual([
			[UNDEPLOYED, 0n],
			[100, 3n]
		]);
		proposals.follow(null, []);
	});

	it('once the network is added, its proposals review like any other', async () => {
		mailbox.all = true;
		mailbox.proposals = [stored(3, UNREAD)];
		// Arbitrum here stands for the network just added: this device has it now.
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		expect(proposals.missing).toEqual([]);
		expect(proposals.chain(UNREAD)?.items[0]?.status).toBe('done');
		proposals.follow(null, []);
	});

	/**
	 * r4D-18 (round 5): a network with nothing waiting is read every fifth
	 * poll (a minute), not every poll; anything waiting — or a read the page
	 * asks for — reads it at once.
	 */
	it('a quiet network’s chain is read once a minute; one with something waiting, every poll', async () => {
		mailbox.all = true;
		mailbox.proposals = [];
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		const reads = () => rpc.memberReads;
		const start = reads();
		for (let i = 0; i < 4; i += 1) await proposals.refresh(account, [100], { tick: true });
		expect(reads(), 'four quiet polls: the mailbox only').toBe(start);
		await proposals.refresh(account, [100], { tick: true });
		expect(reads(), 'the fifth reads the chain').toBe(start + 1);
		// Something to run (at the chain's nonce): every poll reads the chain.
		mailbox.proposals = [stored(3)];
		await proposals.refresh(account, [100], { tick: true });
		await proposals.refresh(account, [100], { tick: true });
		expect(reads()).toBe(start + 3);
		// A read the page asks for (focus, Retry) is never quiet.
		mailbox.proposals = [];
		await proposals.refresh(account, [100], { tick: true });
		await proposals.refresh(account, [100]);
		expect(reads()).toBe(start + 5);
		proposals.follow(null, []);
	});

	/**
	 * r5reg-14: an approval from another device was seen on an open sheet
	 * after 11.2 s (the 12 s poll). While a sheet on a waiting transaction is
	 * open the poll runs at OPEN_POLL_MS: what changed in the mailbox is seen
	 * within that — then the pace goes back.
	 */
	it('an open sheet’s transaction: a change in the mailbox is seen within 4 s, not 12', async () => {
		vi.useFakeTimers();
		try {
			mailbox.all = true;
			mailbox.proposals = [stored(3)];
			proposals.follow(account, [100]);
			await proposals.refresh(account, [100]);
			expect(proposals.pace).toBe(POLL_MS);
			const stop = proposals.watch();
			expect(proposals.pace).toBe(OPEN_POLL_MS);
			// Another member's proposal (as an approval would) lands in the mailbox now.
			mailbox.proposals = [stored(3), stored(4)];
			const start = Date.now();
			while (proposals.open.length < 2 && Date.now() - start < POLL_MS * 2)
				await vi.advanceTimersByTimeAsync(250);
			const seenAfter = Date.now() - start;
			expect(seenAfter).toBeLessThanOrEqual(OPEN_POLL_MS);
			// Released: the 12 s pace again.
			stop();
			expect(proposals.pace).toBe(POLL_MS);
			// Without a sheet holding it, the same change waits for the 12 s round.
			mailbox.proposals = [stored(3), stored(4), stored(5)];
			const again = Date.now();
			while (proposals.open.length < 3 && Date.now() - again < POLL_MS * 2)
				await vi.advanceTimersByTimeAsync(250);
			expect(Date.now() - again).toBeGreaterThan(OPEN_POLL_MS);
		} finally {
			proposals.follow(null, []);
			vi.useRealTimers();
		}
	});

	it('from a service without the list (404): asked network by network, as before', async () => {
		mailbox.all = false;
		mailbox.proposals = [stored(3)];
		proposals.follow(account, [100, 5042002]);
		await proposals.refresh(account, [100, 5042002]);
		expect(mailbox.allReads).toBe(1);
		expect(mailbox.reads).toBe(2);
		expect(proposals.missing).toEqual([]);
		expect(proposals.open).toHaveLength(2);
		proposals.follow(null, []);
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
describe('the list kept across reloads (r4b-11)', () => {
	it('a reload with a failing mailbox still shows the last list, as the last known', async () => {
		mailbox.proposals = [stored(3), stored(4)];
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		expect(proposals.open.map((i) => i.nonce)).toEqual([3n, 4n]);
		expect(proposals.cached).toBeNull();

		// The next page: nothing in memory, the mailbox failing.
		proposals.follow(null, []);
		mailbox.down = true;
		proposals.follow(account, [100]);
		// At once, before anything answered.
		expect(proposals.open.map((i) => i.nonce)).toEqual([3n, 4n]);
		expect(proposals.cached).not.toBeNull();
		expect(proposals.stale).toBe(true);
		await proposals.refresh(account, [100]);
		expect(proposals.open.map((i) => i.nonce)).toEqual([3n, 4n]);
		expect(proposals.cached).not.toBeNull();
		// …and no number is taken from it.
		await expect(
			proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }])
		).rejects.toBeInstanceOf(MailboxUnreadableError);

		// The mailbox back: read in full, no longer the last known.
		mailbox.down = false;
		await proposals.refresh(account, [100]);
		expect(proposals.cached).toBeNull();
		expect(proposals.stale).toBe(false);
	});

	it('another service’s list is not this one’s', async () => {
		mailbox.proposals = [stored(3)];
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		proposals.follow(null, []);
		mailbox.url = 'https://other.example';
		mailbox.down = true;
		proposals.follow(account, [100]);
		expect(proposals.open).toEqual([]);
		expect(proposals.cached).toBeNull();
	});
});

describe('a network that does not answer (r4b-3)', () => {
	it('is named as unreadable — the list is never "nothing waits"', async () => {
		mailbox.proposals = [stored(3)];
		proposals.follow(account, [100]);
		await proposals.refresh(account, [100]);
		rpc.down = [100];
		await proposals.refresh(account, [100]);
		expect(proposals.unreadable).toEqual([100]);
		expect(proposals.stale).toBe(true);
		expect(proposals.open.map((i) => i.nonce)).toEqual([3n]);
		rpc.down = [];
		await proposals.refresh(account, [100]);
		expect(proposals.unreadable).toEqual([]);
	});
});

describe('back to the tab, and offline (r4b-16, r4b-15)', () => {
	it('a tab shown again reads at once; offline is said, and back online reads', async () => {
		const listeners = new Map<string, () => void>();
		const target = {
			addEventListener: (type: string, fn: () => void) => void listeners.set(type, fn),
			removeEventListener: (type: string) => void listeners.delete(type)
		};
		const doc = { ...target, visibilityState: 'visible' };
		vi.stubGlobal('window', target);
		vi.stubGlobal('document', doc);
		vi.stubGlobal('navigator', { onLine: true });
		try {
			proposals.follow(account, [100]);
			await new Promise((r) => setTimeout(r, 0));
			const before = mailbox.reads;
			listeners.get('visibilitychange')?.();
			await new Promise((r) => setTimeout(r, 0));
			expect(mailbox.reads).toBeGreaterThan(before);
			listeners.get('offline')?.();
			expect(proposals.offline).toBe(true);
			listeners.get('online')?.();
			expect(proposals.offline).toBe(false);
			proposals.follow(null, []);
			expect(listeners.size).toBe(0);
		} finally {
			vi.unstubAllGlobals();
			vi.stubGlobal('localStorage', {
				getItem: (k: string) => storage.get(k) ?? null,
				setItem: (k: string, v: string) => void storage.set(k, v),
				removeItem: (k: string) => void storage.delete(k)
			});
		}
	});
});

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

describe('an execution in flight is not offered again (r4b-7)', () => {
	it('while this device’s execution may land, nobody here can execute it', () => {
		const members = { owners: [OWNER], threshold: 1, nonce: 3n, deployed: true };
		const p = { ...stored(3), approvals: [{ owner: OWNER, createdMs: 1 }] };
		expect(readProposal(account as never, members, p as never)!.executing).toBe(false);
		executions.start({
			chainId: 100,
			multisig: SAFE,
			safeTxHash: p.safeTxHash,
			nonce: '3',
			userOpHash: '0x01',
			by: OWNER,
			maybeSent: false,
			startedMs: Date.now(),
			createdMs: 1
		});
		const item = readProposal(account as never, members, p as never)!;
		expect(item.executing).toBe(true);
		expect(item.canExecute).toEqual([]);
	});
});

describe('a proposal withdrawn before it is made (r4b-5)', () => {
	it('asks no passkey and proposes nothing once the person dismissed it', async () => {
		mailbox.proposals = [stored(3)];
		mailbox.slowMs = 50;
		const stop = new AbortController();
		const proposing = proposeTransfers(
			SAFE,
			100,
			[{ to: TO, token: null, amount: '1' }],
			stop.signal
		);
		// Dismissed while the waiting list is still being read.
		stop.abort();
		await expect(proposing).rejects.toMatchObject({ name: 'AbortError' });
		expect(signed).toEqual([]);
		expect(submitted).toEqual([]);
	});
});
