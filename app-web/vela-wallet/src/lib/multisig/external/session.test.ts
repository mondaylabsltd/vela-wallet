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
		parse: () => ({ invite: undefined }),
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
