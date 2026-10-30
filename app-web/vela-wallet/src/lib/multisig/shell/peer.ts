/**
 * The PeerConnect transport behind the multisig machines' `peer_*`
 * operations: rooms (Noise end to end; the relay reads nothing), the relay's
 * token (`../external/relay.ts`), and nothing of WalletPair — what is sent
 * and what an arrival means are the core's (`multisig_outside`).
 *
 * A room is known to the core by a handle this transport hands out. What the
 * connection says of itself goes back to the machine that opened it as its
 * own events: `peer_state` (offline / waiting / connected / closed), `paired`
 * (the record that reopens it now) and, for Vela's side, `relay` (where
 * getting onto the relay stands).
 */
import { Room } from '$lib/vendor/peerconnect/index.js';
import { connectRoom, relayUrl, type RelayConnection, type RelayStatus } from '../external/relay';
import type { PeerTransport } from './handler';

/** What a connection tells the machine that opened it. */
export type PeerEvent =
	| { type: 'peer_state'; room: string; state: string }
	| { type: 'paired'; room: string; pairing: string }
	| { type: 'relay'; status: RelayStatus };

interface Open {
	connection: RelayConnection;
	inbox: unknown[];
	waiting: ((next: { message: unknown } | { closed: string }) => void) | null;
	closed: string | null;
}

const decoder = new TextDecoder();

/** A message as it arrived: JSON when it reads as JSON, else its text. */
function read(data: Uint8Array | string): unknown {
	const text = typeof data === 'string' ? data : decoder.decode(data);
	try {
		return JSON.parse(text);
	} catch {
		return text;
	}
}

export interface PeerRooms extends PeerTransport {
	/** Try the relay again now (a wait cut short, a stop undone — the person's Retry). */
	retry(room: string): void;
	/** Leave every room (the screen went away): pairings are kept. */
	leaveAll(): void;
}

/**
 * The transport for one machine; `tell` hands its events to that machine.
 * `connect` is the relay's connector (a test passes its own).
 */
export function peerRooms(
	tell: (event: PeerEvent) => void,
	connect: (room: Room) => RelayConnection = connectRoom
): PeerRooms {
	const rooms = new Map<string, Open>();
	let next = 0;

	function deliver(open: Open, item: { message: unknown } | { closed: string }) {
		const waiter = open.waiting;
		if (waiter) {
			open.waiting = null;
			waiter(item);
		} else if ('message' in item) {
			open.inbox.push(item.message);
		}
	}

	return {
		async open({ relay, invite, pairing }) {
			const room =
				pairing !== null
					? Room.parse(pairing)
					: invite !== null
						? Room.join(invite)
						: Room.create(relay ?? relayUrl());
			const fresh = pairing === null && invite === null;
			const handle = `room-${++next}`;
			const connection = connect(room);
			const open: Open = { connection, inbox: [], waiting: null, closed: null };
			rooms.set(handle, open);
			connection.addEventListener('message', (event) => {
				deliver(open, { message: read((event as MessageEvent<Uint8Array>).data) });
			});
			connection.addEventListener('paired', () => {
				tell({ type: 'paired', room: handle, pairing: connection.room.toString() });
			});
			connection.addEventListener('state', () => {
				tell({ type: 'peer_state', room: handle, state: connection.state });
				if (connection.state === 'closed') {
					open.closed = connection.reason ?? 'closed';
					deliver(open, { closed: open.closed });
				}
			});
			connection.relay.addEventListener('change', () => {
				tell({ type: 'relay', status: connection.relay.status });
			});
			return {
				room: handle,
				invite: fresh ? (room.invite ?? null) : null,
				pairing: room.toString()
			};
		},
		async send(handle, message) {
			const open = rooms.get(handle);
			if (!open || open.closed) return;
			open.connection.send(typeof message === 'string' ? message : JSON.stringify(message));
		},
		receive(handle, signal) {
			const open = rooms.get(handle);
			if (!open) return Promise.resolve({ closed: 'closed' });
			const queued = open.inbox.shift();
			if (queued !== undefined) return Promise.resolve({ message: queued });
			if (open.closed) return Promise.resolve({ closed: open.closed });
			return new Promise((resolve) => {
				open.waiting = resolve;
				signal.addEventListener('abort', () => resolve({ closed: 'left' }), { once: true });
			});
		},
		async close(handle, _cancel, end) {
			// `walletpair_cancel` is the core's message, sent before this (`cancel`).
			const open = rooms.get(handle);
			if (!open) return;
			if (end) open.connection.close();
			else open.connection.leave();
			open.closed = end ? 'closed' : 'left';
			deliver(open, { closed: open.closed });
			rooms.delete(handle);
		},
		retry(handle) {
			rooms.get(handle)?.connection.relay.retry();
		},
		leaveAll() {
			for (const [handle, open] of rooms) {
				open.connection.leave();
				deliver(open, { closed: 'left' });
				rooms.delete(handle);
			}
		}
	};
}
