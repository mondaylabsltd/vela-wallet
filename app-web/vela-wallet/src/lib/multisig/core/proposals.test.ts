/**
 * The mailbox, when it cannot be read (ms-1, ms-2): nothing is proposed
 * with a number taken from a list that is not there, and what is shown says
 * it may be stale — the last transactions read stay, never "No activity yet".
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OWNER = '0x00000000000000000000000000000000000000aa';
const TO = '0x000000000000000000000000000000000000dEaD';

const mailbox = {
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
vi.mock('./chain', () => ({
	hasCode: async () => true,
	// Read through the wallet's own RPC: on Arbitrum the Safe is deployed, at nonce 7.
	readMembers: async (chainId: number) =>
		chainId === UNREAD
			? { owners: [OWNER], threshold: 1, nonce: 7n, deployed: true }
			: { owners: [OWNER], threshold: 2, nonce: 3n, deployed: true }
}));
vi.mock('./service', () => ({
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
		if (mailbox.postDown) throw new Error('mailbox down');
		submitted.push(req.safeTx);
		return { ok: true, status: 'created', proposal: {} };
	}
}));
vi.mock('./kernel', () => ({
	safeTxHash: (_c: number, _s: string, tx: { nonce: string; to: string }) =>
		`0x${tx.nonce.slice(2).padStart(2, '0')}${'00'.repeat(31)}`,
	reviewTx: (_c: number, _s: string, tx: { to: string; value: string }) => ({
		safeTxHash: '0x',
		nonce: '0',
		action: { kind: 'transfer', token: null, to: tx.to, amount: BigInt(tx.value).toString() },
		refusal: null
	}),
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
vi.mock('./store.svelte', () => ({
	multisig: { find: () => account, useChain: () => {} }
}));

const {
	proposals,
	proposeTransfers,
	proposeCalls,
	draftCalls,
	flushOutbox,
	MailboxUnreadableError,
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
		await expect(
			proposeTransfers(SAFE, 100, [{ to: TO, token: null, amount: '1' }])
		).rejects.toThrow();
		// Signed, not in the mailbox: kept, not lost with the page.
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
