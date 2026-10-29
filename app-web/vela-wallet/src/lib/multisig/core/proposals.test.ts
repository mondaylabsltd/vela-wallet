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
vi.mock('./chain', () => ({
	hasCode: async () => true,
	// Read through the wallet's own RPC: on Arbitrum the Safe is deployed, at nonce 7.
	readMembers: async (chainId: number) =>
		chainId === UNREAD
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
	verifyApproval: () => true,
	verifyEcdsaApproval: () => ({}),
	sameAddress: (a?: string | null, b?: string | null) =>
		!!a && !!b && a.toLowerCase() === b.toLowerCase()
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
vi.mock('$lib/onboarding/core/storage', () => ({ loadAccounts: () => [] }));
vi.mock('./records', () => ({ recordRemoteExecutions: async () => false }));
vi.mock('./execute', () => ({ submitExecution: async () => ({}) }));
vi.mock('../external/signing.svelte', () => ({ externalSigning: { approve: async () => ({}) } }));
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
	proposals,
	proposeTransfers,
	proposeCalls,
	draftCalls,
	flushOutbox,
	MailboxUnreadableError,
	OUTBOX_PATIENCE_MS,
	outbox,
	readProposal,
	SLOW_REFRESH_MS
} = await import('./proposals.svelte');
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
	proposals.follow(null, []);
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
});
