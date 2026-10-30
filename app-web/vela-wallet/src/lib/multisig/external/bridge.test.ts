/**
 * The connect page does not pile up sessions (ms-4): once a dApp's new
 * session is approved, its older sessions that are not connected — rooms
 * that dApp gave up — are ended, not left as more Disconnect rows.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

class FakeConnection extends EventTarget {
	state: 'offline' | 'waiting' | 'connected' | 'closed';
	closed = false;
	room = { toString: () => 'record' };
	relay = Object.assign(new EventTarget(), {
		status: { kind: 'up' } as { kind: string },
		retried: 0,
		retry() {
			this.retried += 1;
		}
	});
	constructor(state: 'offline' | 'connected') {
		super();
		this.state = state;
	}
	close() {
		this.closed = true;
	}
}

/** Which saved session connects (by record), and each connection made. */
const connectedRecords = new Set<string>();
const connections = new Map<string, FakeConnection>();
vi.mock('$lib/vendor/peerconnect/index.js', () => ({
	Room: {
		parse: (record: string) => ({ record }),
		join: (invite: string) => {
			// PeerConnect's own refusal of an invite that does not read.
			if (invite.includes('DAMAGED')) throw new Error('invalid invite');
			return { toString: () => 'joined' };
		}
	}
}));
vi.mock('./relay', () => ({
	connectRoom: (room: { record?: string; toString(): string }) => {
		const record = room.record ?? room.toString();
		const c = new FakeConnection(connectedRecords.has(record) ? 'connected' : 'offline');
		connections.set(record, c);
		return c;
	}
}));
type Connect = (
	params: { name: string },
	offered: string[],
	origin: string | undefined,
	cancelled: AbortSignal
) => Promise<string[]>;
type Approve = (
	request: { id?: string; method: string; params?: unknown[] },
	cancelled?: AbortSignal
) => Promise<boolean>;
type Perform = (
	method: string,
	params: unknown,
	context: { request: { id: string } }
) => Promise<unknown>;
const prompts = new Map<FakeConnection, Connect>();
const approvals = new Map<FakeConnection, Approve>();
const performs = new Map<FakeConnection, Perform>();
vi.mock('$lib/walletpair/wallet', () => ({
	emptyStore: () => ({ approved: [], chains: {}, records: new Map(), forwarded: new Set() }),
	WalletError: class extends Error {
		constructor(
			readonly code: number,
			message: string
		) {
			super(message);
		}
	},
	ReferenceWallet: class {
		constructor(
			connection: FakeConnection,
			options: {
				prompts: { connect: Connect; approve: Approve };
				signers: { perform: Perform }[];
			}
		) {
			prompts.set(connection, options.prompts.connect);
			let n = 0;
			// Every request has an id, as WalletPair's do.
			approvals.set(connection, (request, cancelled) =>
				options.prompts.approve(
					{ id: `req-${++n}`, ...request } as never,
					cancelled ?? new AbortController().signal
				)
			);
			performs.set(connection, (method, params, context) =>
				options.signers[0].perform(method, params, context)
			);
		}
	}
}));
/** The SafeTx a typed-data request asks for, as the page reads it (the core's check, stood in for). */
const SAFE_TX = vi.hoisted(() => ({
	chainId: 5042002,
	safe: '0x00000000000000000000000000000000000000cc',
	tx: {}
}));
vi.mock('./typed', () => ({
	readSafeTxRequest: (typed: unknown) => (typed === 'safe-tx' ? SAFE_TX : null),
	ethSignForm: (sig: string) =>
		`${sig.slice(0, -2)}${(parseInt(sig.slice(-2), 16) + 4).toString(16)}`
}));
vi.mock('../core/kernel', () => ({ safeTxHash: () => `0x${'5a'.repeat(32)}` }));
/** What the wallet answers a forwarded request with (the injected signer, stood in for). */
const wallet = vi.hoisted(() => ({ perform: async (): Promise<unknown> => '0xsig' }));
vi.mock('./injected', () => ({
	injectedSigner: (provider: unknown, accounts: string[], chain: string) => ({
		provider,
		accounts,
		chain,
		network: { namespace: 'eip155' },
		perform: () => wallet.perform()
	}),
	currentChain: async () => 'eip155:100'
}));

const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => void store.set(k, v),
	removeItem: (k: string) => void store.delete(k)
});

const { Bridge, DamagedInviteError } = await import('./bridge');

const VELA = { name: 'Vela', origin: 'https://wallet.getvela.app' };
const session = (id: string, record: string, dapp = VELA) => ({
	id,
	record,
	created: 1,
	dapp,
	store: { approved: ['eip155'], chains: {}, records: [], forwarded: [] }
});

beforeEach(() => {
	store.clear();
	connectedRecords.clear();
	connections.clear();
	prompts.clear();
	approvals.clear();
});

describe("the connect page's sessions", () => {
	it('end the rooms a dApp gave up once it pairs again, and keep the others', async () => {
		store.set(
			'vela.connect.sessions.v1',
			JSON.stringify([
				session('old-1', 'r-old-1'),
				session('old-2', 'r-old-2'),
				// The same dApp, connected right now: another device, kept.
				session('live', 'r-live'),
				session('other', 'r-other', { name: 'Other dApp', origin: 'https://other.example' })
			])
		);
		connectedRecords.add('r-live');
		const provider = { request: async () => ['0x00000000000000000000000000000000000000aa'] };
		const bridge = new Bridge(provider as never, [], 'eip155:100');
		bridge.resumeAll();
		expect(bridge.sessions).toHaveLength(4);

		const fresh = bridge.join('wss://relay#invite');
		const connect = prompts.get(connections.get('joined')!)!;
		const answer = connect(
			{ name: VELA.name },
			['eip155'],
			VELA.origin,
			new AbortController().signal
		);
		bridge.prompt?.answer(true);
		expect(await answer).toEqual(['eip155']);

		expect(bridge.sessions.map((s) => s.id).sort()).toEqual(['live', 'other', fresh].sort());
		expect(connections.get('r-old-1')?.closed).toBe(true);
		expect(connections.get('r-old-2')?.closed).toBe(true);
		expect(connections.get('r-live')?.closed).toBe(false);
	});

	it('keep every session when the new one is declined', async () => {
		store.set('vela.connect.sessions.v1', JSON.stringify([session('old-1', 'r-old-1')]));
		const provider = { request: async () => ['0x00000000000000000000000000000000000000aa'] };
		const bridge = new Bridge(provider as never, [], 'eip155:100');
		bridge.resumeAll();
		bridge.join('wss://relay#invite');
		const connect = prompts.get(connections.get('joined')!)!;
		const answer = connect(
			{ name: VELA.name },
			['eip155'],
			VELA.origin,
			new AbortController().signal
		);
		bridge.prompt?.answer(false);
		expect(await answer).toEqual([]);
		expect(bridge.sessions).toHaveLength(2);
	});
});

/**
 * ms-14: a room opened from a link that names its members signs as one of
 * them only — a request as anyone else is refused before the wallet is asked.
 */
describe('a damaged invite (round 6)', () => {
	it("is said as the link's: DamagedInviteError, and nothing is saved or started", () => {
		store.set('vela.connect.sessions.v1', JSON.stringify([session('old-1', 'r-old-1')]));
		const provider = { request: async () => ['0x00000000000000000000000000000000000000aa'] };
		const bridge = new Bridge(provider as never, [], 'eip155:100');
		bridge.resumeAll();
		const before = store.get('vela.connect.sessions.v1');
		expect(() => bridge.join('wss://relay#DAMAGED')).toThrow(DamagedInviteError);
		expect(bridge.sessions).toHaveLength(1);
		expect(store.get('vela.connect.sessions.v1')).toBe(before);
		expect(connections.has('joined')).toBe(false);
	});
});

describe('a session for the members its link named (ms-14)', () => {
	const MEMBER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';
	const STRANGER = '0x3c44cdddb6a900fa2b585dd299e03d12fa4293bc';

	it('refuses to sign or send as anyone but them', async () => {
		const provider = { request: async () => [MEMBER] };
		const bridge = new Bridge(provider as never, [MEMBER], 'eip155:100');
		bridge.join('wss://relay#invite', [MEMBER.toUpperCase().replace('0X', '0x')]);
		const approve = approvals.get(connections.get('joined')!)!;
		expect(await approve({ method: 'eth_signTypedData_v4', params: [STRANGER, '{}'] })).toBe(false);
		expect(bridge.activity[0]).toMatchObject({
			method: 'eth_signTypedData_v4',
			outcome: 'rejected'
		});
		expect(await approve({ method: 'eth_sendTransaction', params: [{ from: STRANGER }] })).toBe(
			false
		);
		const asked = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, '{}'] });
		// r4C-7: held for the owner's tap, shown first.
		await vi.waitFor(() => expect(bridge.request).toBeDefined());
		bridge.request!.answer(true);
		expect(await asked).toBe(true);
		// Asking the wallet to switch networks names no account.
		expect(await approve({ method: 'wallet_switchEthereumChain', params: [{}] })).toBe(true);
	});

	it('an older link named no one: the wallet is asked as before', async () => {
		const provider = { request: async () => [STRANGER] };
		const bridge = new Bridge(provider as never, [STRANGER], 'eip155:100');
		bridge.join('wss://relay#invite');
		const approve = approvals.get(connections.get('joined')!)!;
		const asked = approve({ method: 'eth_signTypedData_v4', params: [STRANGER, '{}'] });
		await vi.waitFor(() => expect(bridge.request).toBeDefined());
		bridge.request!.answer(true);
		expect(await asked).toBe(true);
	});
});

/**
 * r4C-7: Vela's request waits here, read and shown, until the owner taps;
 * r4C-12: each ends with its outcome — signed, declined here or in the
 * wallet, failed with the wallet's words, withdrawn by Vela; r4C-16: the
 * hash itself for a wallet without typed data, on a tap; r4C-1: the page's
 * own way onto the relay.
 */
describe('a request from Vela, on this page', () => {
	const MEMBER = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';

	function open(request: () => Promise<unknown> = async () => [MEMBER]) {
		const provider = { request: vi.fn(request) };
		const bridge = new Bridge(provider as never, [MEMBER], 'eip155:5042002', 'MetaMask');
		bridge.join('wss://relay#invite', [MEMBER]);
		const connection = connections.get('joined')!;
		return {
			bridge,
			provider,
			connection,
			approve: approvals.get(connection)!,
			perform: performs.get(connection)!
		};
	}

	it('is read and held for the tap; the wallet is asked only then', async () => {
		const { bridge, approve } = open();
		const asked = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] });
		await vi.waitFor(() => expect(bridge.request?.entry.safeTx).toEqual(SAFE_TX));
		expect(bridge.request!.entry.stage).toBe('review');
		bridge.request!.answer(true);
		expect(await asked).toBe(true);
		expect(bridge.activity[0]).toMatchObject({ stage: 'wallet', account: MEMBER });
		expect(bridge.activity[0].walletAt).toBeTypeOf('number');
	});

	it('declined here, or withdrawn by Vela while shown: said as each', async () => {
		const { bridge, approve } = open();
		const declined = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] });
		await vi.waitFor(() => expect(bridge.request).toBeDefined());
		bridge.request!.answer(false);
		expect(await declined).toBe(false);
		expect(bridge.activity[0]).toMatchObject({ stage: 'answered', outcome: 'rejected' });

		const cancel = new AbortController();
		const withdrawn = approve(
			{ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] },
			cancel.signal
		);
		await vi.waitFor(() => expect(bridge.request).toBeDefined());
		cancel.abort();
		expect(await withdrawn).toBe(false);
		expect(bridge.activity[0]).toMatchObject({ outcome: 'withdrawn' });
		expect(bridge.request).toBeUndefined();
	});

	it('r4C-15: Vela asking again for the same transaction withdraws what it asked before', async () => {
		const { bridge, approve } = open();
		// Asked before Vela went offline: nobody waits on it once Vela asks again.
		const stale = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] });
		await vi.waitFor(() => expect(bridge.request).toBeDefined());
		const first = bridge.request!.entry;
		const again = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] });
		expect(await stale).toBe(false);
		expect(first).toMatchObject({ stage: 'answered', outcome: 'withdrawn' });
		// The one shown is the new ask, never the stale one first.
		await vi.waitFor(() => expect(bridge.request?.entry).not.toBe(first));
		bridge.request!.answer(true);
		expect(await again).toBe(true);
	});

	it('withdrawn once in the wallet (walletpair_cancel): said, whatever the wallet answers', async () => {
		const { bridge, approve, perform, connection } = open();
		const asked = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] });
		await vi.waitFor(() => expect(bridge.request).toBeDefined());
		const id = bridge.request!.entry.id;
		bridge.request!.answer(true);
		await asked;
		const cancel = { id: 'c1', method: 'walletpair_cancel', params: [id] };
		connection.dispatchEvent(
			new MessageEvent('message', { data: new TextEncoder().encode(JSON.stringify(cancel)) })
		);
		expect(bridge.activity[0].outcome).toBe('withdrawn');
		await perform('eth_signTypedData_v4', [], { request: { id } });
		expect(bridge.activity[0]).toMatchObject({ stage: 'answered', outcome: 'withdrawn' });
	});

	it('ends with the outcome: signed, declined in the wallet, or failed in its words', async () => {
		const { bridge, approve, perform } = open();
		const run = async (answer: () => Promise<unknown>) => {
			wallet.perform = answer;
			const asked = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] });
			await vi.waitFor(() => expect(bridge.request).toBeDefined());
			const id = bridge.request!.entry.id;
			bridge.request!.answer(true);
			await asked;
			await perform('eth_signTypedData_v4', [], { request: { id } }).catch(() => {});
			return bridge.activity[0];
		};
		expect(await run(async () => '0xsig')).toMatchObject({
			outcome: 'done',
			result: '0xsig',
			form: 'typedData'
		});
		expect(
			await run(async () => {
				throw Object.assign(new Error('User rejected the request.'), { code: 4001 });
			})
		).toMatchObject({ outcome: 'rejected', problem: { kind: 'declined' } });
		expect(
			await run(async () => {
				throw Object.assign(new Error('{"message":"Ledger: \\"0x6985\\" denied"}'), {
					code: -32603
				});
			})
		).toMatchObject({
			outcome: 'failed',
			problem: { kind: 'unknown', reason: 'Ledger: "0x6985" denied' }
		});
		wallet.perform = async () => '0xsig';
	});

	it('r4C-16: no typed data in the wallet — the hash, in the eth_sign form, on a tap', async () => {
		const sig = `0x${'11'.repeat(64)}1b`;
		const { bridge, approve, perform, provider } = open(async () => sig);
		wallet.perform = async () => {
			throw Object.assign(new Error('Method not supported'), { code: 4200 });
		};
		const asked = approve({ method: 'eth_signTypedData_v4', params: [MEMBER, 'safe-tx'] });
		await vi.waitFor(() => expect(bridge.request).toBeDefined());
		const id = bridge.request!.entry.id;
		bridge.request!.answer(true);
		await asked;
		const answered = perform('eth_signTypedData_v4', [], { request: { id } });
		await vi.waitFor(() => expect(bridge.request?.entry.stage).toBe('fallback'));
		bridge.request!.answer(true);
		expect(await answered).toBe(`0x${'11'.repeat(64)}1f`);
		expect(provider.request).toHaveBeenLastCalledWith({
			method: 'personal_sign',
			params: [`0x${'5a'.repeat(32)}`, MEMBER]
		});
		expect(bridge.activity[0]).toMatchObject({ outcome: 'done', form: 'ethSign' });
		wallet.perform = async () => '0xsig';
	});

	it("r4C-1: says the page's own relay trouble, the worst of its rooms; Retry reaches them", () => {
		const { bridge, connection } = open();
		expect(bridge.relay).toEqual({ kind: 'up' });
		connection.relay.status = { kind: 'failed', why: 'refused', detail: 'HTTP 4403' } as never;
		expect(bridge.relay).toMatchObject({ kind: 'failed', why: 'refused' });
		bridge.retryRelay();
		expect(connection.relay.retried).toBe(1);
	});
});
