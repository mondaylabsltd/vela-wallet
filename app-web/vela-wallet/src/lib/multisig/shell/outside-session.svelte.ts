/**
 * Vela's side of a live session with an outside owner's wallet, run by the
 * core's session machine (`multisig_outside::session::OutsideSession`, wasm
 * `MultisigOutsideSessionCore`): the room (or a pairing kept before),
 * `walletpair_connect`, the owner among the wallet's accounts, the network
 * switched, the SafeTx signed — and the answer verified by the core before
 * it is handed back — or the transaction sent. The machine decides; this
 * holds its view for `ExternalSignSheet` to draw, and hands it the
 * PeerConnect rooms (`peer-lazy.ts`: nothing of PeerConnect loads before a
 * room is opened, perf-3), `localStorage` and the mailbox.
 *
 * {@link openOutsideSession} (in `../external/signing.svelte.ts`) is the one
 * entry a caller uses: it shows the sheet and settles with how the ask ended.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { shareOrigin } from '../core/link';
import { genesisServiceURL } from '../core/service';
import type { JsonWasmCore } from '$lib/core/json-shell';
import type { OutsideSessionEvent } from '../generated/OutsideSessionEvent';
import type { OutsideSessionRequest } from '../generated/OutsideSessionRequest';
import type { OutsideSessionView } from '../generated/OutsideSessionView';
import type { PeerTransport } from './handler';
import { MultisigMachine } from './machine.svelte';
import { connectLink, pageDeps } from './outside-page.svelte';
import type { PeerEvent, PeerRooms } from './peer';
import { lazyPeerRooms } from './peer-lazy';

/** How often the mailbox is looked at for an approval made from the link alone (r4C-2). */
export const MAILBOX_EVERY_MS = 8_000;

/** Vela's `/connect` page for the person's language, at the wallet's web address. */
export function connectPage(locale: string): string {
	return typeof location === 'undefined' ? '' : `${shareOrigin(location.origin)}/${locale}/connect`;
}

/** One ask of an outside owner's wallet: the session machine with Vela's I/O. */
export class OutsideSessionHost {
	readonly #machine: MultisigMachine<OutsideSessionView, OutsideSessionEvent>;
	readonly #rooms: PeerRooms;
	readonly #page: string;
	#room: string | null = null;

	constructor(
		request: OutsideSessionRequest,
		page: string,
		core: JsonWasmCore = new wasm.MultisigOutsideSessionCore()
	) {
		this.#page = page;
		this.#rooms = lazyPeerRooms((event: PeerEvent) => this.#machine.dispatch(event));
		const rooms = this.#rooms;
		const peer: PeerTransport = {
			open: async (opening) => {
				const opened = await rooms.open(opening);
				this.#room = opened.room;
				return opened;
			},
			send: (room, message) => rooms.send(room, message),
			receive: (room, signal) => rooms.receive(room, signal),
			close: (room, cancel, end) => rooms.close(room, cancel, end)
		};
		const deps = pageDeps({ peer, wallet: () => null, askRisk: async () => false });
		this.#machine = new MultisigMachine<OutsideSessionView, OutsideSessionEvent>(
			core,
			deps,
			'outside session'
		);
		this.#machine.dispatch({
			type: 'start',
			request,
			service_url: genesisServiceURL(),
			relay: null,
			origin: typeof location === 'undefined' ? null : location.origin
		});
	}

	get view(): OutsideSessionView {
		return this.#machine.view;
	}

	/**
	 * The page to open: with the invite for a new room, none for one paired
	 * before — the members who may sign (ms-14) and the transaction (r4C-2),
	 * all in the fragment (the core's `connectLink`).
	 */
	get link(): string {
		const v = this.#machine.view;
		const tx = v.tx
			? { chainId: v.tx.chainId, multisig: v.tx.safe, safeTxHash: v.tx.safeTxHash }
			: null;
		return connectLink(this.#page, v.invite, v.members, tx);
	}

	dispatch(event: OutsideSessionEvent): void {
		this.#machine.dispatch(event);
	}

	/** Try the relay again now (the person's Retry). */
	retryRelay(): void {
		if (this.#room) this.#rooms.retry(this.#room);
	}

	/** The sheet went away: its room left (the pairing kept). */
	dispose(): void {
		this.#rooms.leaveAll();
		this.#machine.dispose();
	}
}
