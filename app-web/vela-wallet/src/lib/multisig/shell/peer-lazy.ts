/**
 * perf-3: PeerConnect (its wasm inlined, compiled the moment it is imported)
 * stays off every signed-in page until a room is opened. This transport
 * loads `peer.ts` on the first `open` and hands every call to it; nothing
 * before that reaches the vendored client (`e2e/budgets.e2e.ts` pins it).
 */
import type { PeerTransport } from './handler';
import type { PeerEvent, PeerRooms } from './peer';

export function lazyPeerRooms(tell: (event: PeerEvent) => void): PeerRooms {
	let rooms: Promise<PeerRooms> | null = null;
	const load = () => (rooms ??= import('./peer').then((m) => m.peerRooms(tell)));
	let loaded: PeerRooms | null = null;
	const transport: PeerTransport = {
		open: async (input) => (loaded = await load()).open(input),
		send: async (room, message) => (await load()).send(room, message),
		receive: async (room, signal) => (await load()).receive(room, signal),
		close: async (room, cancel, end) => (await load()).close(room, cancel, end)
	};
	return {
		...transport,
		retry: (room) => loaded?.retry(room),
		leaveAll: () => loaded?.leaveAll()
	};
}
