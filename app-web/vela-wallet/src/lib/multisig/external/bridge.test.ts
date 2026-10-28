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
const prompts = new Map<FakeConnection, Connect>();
vi.mock('$lib/walletpair/wallet', () => ({
	emptyStore: () => ({ approved: [], chains: {}, records: new Map(), forwarded: new Set() }),
	ReferenceWallet: class {
		constructor(connection: FakeConnection, options: { prompts: { connect: Connect } }) {
			prompts.set(connection, options.prompts.connect);
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
