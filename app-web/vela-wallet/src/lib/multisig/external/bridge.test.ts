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
		join: () => ({ toString: () => 'joined' })
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
type Approve = (request: { method: string; params?: unknown[] }) => Promise<boolean>;
const prompts = new Map<FakeConnection, Connect>();
const approvals = new Map<FakeConnection, Approve>();
vi.mock('$lib/walletpair/wallet', () => ({
	emptyStore: () => ({ approved: [], chains: {}, records: new Map(), forwarded: new Set() }),
	ReferenceWallet: class {
		constructor(
			connection: FakeConnection,
			options: { prompts: { connect: Connect; approve: Approve } }
		) {
			prompts.set(connection, options.prompts.connect);
			approvals.set(connection, options.prompts.approve);
		}
	}
}));
vi.mock('./injected', () => ({
	injectedSigner: (provider: unknown, accounts: string[], chain: string) => ({
		provider,
		accounts,
		chain
	}),
	currentChain: async () => 'eip155:100'
}));

const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => void store.set(k, v),
	removeItem: (k: string) => void store.delete(k)
});

const { Bridge } = await import('./bridge');

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
		expect(await approve({ method: 'eth_signTypedData_v4', params: [MEMBER, '{}'] })).toBe(true);
		// Asking the wallet to switch networks names no account.
		expect(await approve({ method: 'wallet_switchEthereumChain', params: [{}] })).toBe(true);
	});

	it('an older link named no one: the wallet is asked as before', async () => {
		const provider = { request: async () => [STRANGER] };
		const bridge = new Bridge(provider as never, [STRANGER], 'eip155:100');
		bridge.join('wss://relay#invite');
		const approve = approvals.get(connections.get('joined')!)!;
		expect(await approve({ method: 'eth_signTypedData_v4', params: [STRANGER, '{}'] })).toBe(true);
	});
});
