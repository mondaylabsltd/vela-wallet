/**
 * An outside owner's wallet stays paired (ms-4): signing ends by leaving the
 * room, and leaving is not ending — the pairing is kept, so the next
 * signature reopens the page instead of asking for a new code (and the
 * connect page does not gather one room per signature).
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';

const SAVED_KEY = 'vela.multisig.externalWallets.v1';
const OWNER = '0x00000000000000000000000000000000000000aa';

class FakeConnection extends EventTarget {
	state: 'offline' | 'waiting' | 'connected' | 'closed' = 'offline';
	reason: 'left' | 'closed' | 'closed-by-peer' | undefined;
	room = { paired: true, invite: undefined as string | undefined, toString: () => 'record-1' };
	relay = Object.assign(new EventTarget(), {
		status: { kind: 'connecting' } as { kind: string },
		retried: 0,
		retry() {
			this.retried += 1;
		}
	});
	send() {
		return 0;
	}
	leave() {
		this.#close('left');
	}
	close() {
		this.#close('closed');
	}
	closedByPeer() {
		this.#close('closed-by-peer');
	}
	#close(reason: 'left' | 'closed' | 'closed-by-peer') {
		this.state = 'closed';
		this.reason = reason;
		this.dispatchEvent(new Event('state'));
	}
}

let connection: FakeConnection;
vi.mock('$lib/vendor/peerconnect/index.js', () => ({
	Room: {
		// A room just created still has its invite; one paired before has none.
		parse: (record: string) => ({ invite: record === 'fresh' ? 'wss://relay#x' : undefined }),
		create: () => ({ invite: 'wss://relay#x', toString: () => 'fresh' })
	}
}));
vi.mock('./relay', () => ({
	connectRoom: () => (connection = new FakeConnection()),
	relayUrl: () => 'wss://relay'
}));

const store = new Map<string, string>();
vi.stubGlobal('localStorage', {
	getItem: (k: string) => store.get(k) ?? null,
	setItem: (k: string, v: string) => void store.set(k, v),
	removeItem: (k: string) => void store.delete(k)
});

const { ExternalWallet } = await import('./session.svelte');

const saved = () => JSON.parse(store.get(SAVED_KEY) ?? '[]') as { id: string }[];

beforeEach(() => {
	store.clear();
	store.set(
		SAVED_KEY,
		JSON.stringify([{ id: 'p1', record: 'record-1', accounts: [OWNER], savedMs: 1 }])
	);
});

describe("an outside owner's paired wallet", () => {
	it('is the one asked again, and stays paired after the sheet leaves the room', () => {
		const wallet = ExternalWallet.for([OWNER], 'https://wallet.getvela.app/en/connect');
		// The page only needs reopening: no new code.
		expect(wallet.resumed).toBe(true);
		wallet.leave();
		expect(connection.reason).toBe('left');
		expect(saved().map((s) => s.id)).toEqual(['p1']);
		// The next signature finds it again.
		expect(ExternalWallet.for([OWNER], 'https://wallet.getvela.app/en/connect').resumed).toBe(true);
	});

	it('is forgotten once the pairing is ended, from either side', () => {
		ExternalWallet.for([OWNER], 'https://wallet.getvela.app/en/connect');
		connection.closedByPeer();
		expect(saved()).toEqual([]);

		store.set(
			SAVED_KEY,
			JSON.stringify([{ id: 'p2', record: 'record-1', accounts: [OWNER], savedMs: 1 }])
		);
		ExternalWallet.for([OWNER], 'https://wallet.getvela.app/en/connect').end();
		expect(saved()).toEqual([]);
	});
});

/** ms-14: the link names the members who may sign, in its fragment with the invite. */
describe("the connect page's link", () => {
	it('a new room: the invite and the members, after #, nothing in the query', () => {
		store.clear();
		const wallet = ExternalWallet.for([OWNER], 'https://wallet.getvela.app/en/connect');
		expect(wallet.resumed).toBe(false);
		const url = new URL(wallet.link);
		expect(url.search).toBe('');
		expect(url.hash).toBe(`#${encodeURIComponent('wss://relay#x')}&for=${OWNER}`);
	});

	it('a room paired before: the members alone', () => {
		const wallet = ExternalWallet.for([OWNER], 'https://wallet.getvela.app/en/connect');
		expect(wallet.resumed).toBe(true);
		expect(wallet.link).toBe(`https://wallet.getvela.app/en/connect#for=${OWNER}`);
	});
});

/**
 * r4C-6: the pairing is the person's to change: another wallet is a new room
 * (a fresh code), whatever this device paired before, and Unpair ends it.
 * r4C-4: the sheet sees this side's relay state; r4C-2: the link names the
 * transaction, so the page can approve it with nobody here.
 */
describe('pairing management and what the sheet is told', () => {
	const PAGE = 'https://wallet.getvela.app/en/connect';
	const TX = { chainId: 5042002, multisig: OWNER, safeTxHash: `0x${'ab'.repeat(32)}` };

	it('"Use another wallet": a fresh room with its invite, the saved pairing kept', () => {
		const known = ExternalWallet.for([OWNER], PAGE);
		expect(known.resumed).toBe(true);
		expect(known.pairedAccount).toBe(OWNER);
		known.leave();
		const other = ExternalWallet.fresh([OWNER], PAGE, TX);
		expect(other.resumed).toBe(false);
		expect(other.link).toContain(encodeURIComponent('wss://relay#x'));
		expect(other.link).toContain(`&ms=5042002.${OWNER}.0x${'ab'.repeat(32)}`);
		expect(saved().map((s) => s.id)).toEqual(['p1']);
	});

	it('Unpair forgets it: the next request is a new room', () => {
		ExternalWallet.for([OWNER], PAGE).end();
		expect(saved()).toEqual([]);
		expect(ExternalWallet.for([OWNER], PAGE).resumed).toBe(false);
	});

	it("follows this side's relay state, and Retry reaches it", () => {
		const wallet = ExternalWallet.for([OWNER], PAGE);
		expect(wallet.relay).toEqual({ kind: 'connecting' });
		connection.relay.status = { kind: 'failed', why: 'refused', detail: 'HTTP 4403' } as never;
		connection.relay.dispatchEvent(new Event('change'));
		expect(wallet.relay).toMatchObject({ kind: 'failed', why: 'refused' });
		wallet.retryRelay();
		expect(connection.relay.retried).toBe(1);
	});

	it('times each phase from when it began', () => {
		const wallet = ExternalWallet.for([OWNER], PAGE);
		const before = wallet.since;
		wallet.to('confirming');
		expect(wallet.phase).toBe('confirming');
		expect(wallet.since).toBeGreaterThanOrEqual(before);
		const at = wallet.since;
		wallet.to('confirming');
		expect(wallet.since).toBe(at);
	});
});
