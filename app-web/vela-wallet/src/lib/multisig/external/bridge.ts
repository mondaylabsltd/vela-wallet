// The connect page's sessions (ported from sub3's bridge): each is a PeerConnect room joined from Vela's link or
// code, with WalletPair's reference Wallet on top, all signing through the one injected wallet. Rooms and Wallet
// records are kept in localStorage, so a session survives the page reloading (WalletPair core §2.4, §6;
// PeerConnect §3.4) and Vela can ask the same wallet again without a new code.
//
// r4C-7/r4D-2: a request to sign or send waits here for the owner's tap — the page reads the SafeTx first (the
// core checks it is one), shows the review, and only then asks the wallet. r4C-12: every request ends with its
// outcome (signed, declined here or in the wallet, failed with the wallet's reason, withdrawn by Vela). r4C-16: a
// wallet that cannot sign typed data is offered Safe's `eth_sign` form (`personal_sign` over the safeTxHash), on
// the owner's tap. r4C-15: a live session whose Vela side goes away mid-request is said.

import { Room } from '$lib/vendor/peerconnect/index.js';
import type { Eip1193Provider } from './eip6963';
import { chainOf } from '$lib/walletpair/networks/evm';
import {
	ErrorCode,
	parseRequest,
	type ConnectParams,
	type Request,
	type Response
} from '$lib/walletpair/protocol';
import {
	ReferenceWallet,
	WalletError,
	emptyStore,
	type Signer,
	type WalletStore
} from '$lib/walletpair/wallet';
import { connectRoom, type RelayConnection, type RelayStatus } from './relay';
import { injectedSigner, currentChain, type InjectedSigner } from './injected';
import { isExpected, readConnectHash } from './connect-link';
import { readSafeTxRequest, ethSignForm, type SafeTxRequest } from './typed';
import { safeTxHash } from '../core/kernel';
import { walletProblem, type WalletProblem } from './errors';

const STORAGE_KEY = 'vela.connect.sessions.v1';

interface Saved {
	id: string;
	record: string;
	created: number;
	dapp?: { name: string; origin?: string; icon?: string };
	/**
	 * ms-14: the members Vela asked for in the link that opened this room
	 * (lowercase). A request to sign as anyone else is refused here.
	 */
	members?: string[];
	store: {
		approved: string[];
		chains: Record<string, string>;
		records: [string, Response | null][];
		forwarded: string[];
	};
}

export interface SessionView {
	id: string;
	state: string;
	dapp?: { name: string; origin?: string; icon?: string };
	approved: boolean;
	/** ms-12: the person declined its connection here; Vela was told so. */
	declined: boolean;
	created: number;
	/** When `state` last changed (ms since epoch): "Vela's side isn't connected" after a while. */
	stateSince: number;
}

/** Where a request stands on this page. */
export type Stage =
	/** Shown here, waiting for the owner's tap (Sign with the wallet, or Decline). */
	| 'review'
	/** The wallet cannot sign typed data: the `eth_sign` form is offered, on a tap. */
	| 'fallback'
	/** Handed to the wallet, which is asking the owner. */
	| 'wallet'
	/** Answered (see `outcome`). */
	| 'answered';

export interface ActivityEntry {
	/** The request's id (WalletPair's). */
	id: string;
	at: number;
	session: string;
	origin?: string;
	method: string;
	/** The account it signs or sends as, lowercase. */
	account: string | null;
	/** `eth_signTypedData_v4`: the SafeTx it asks for, as the core read it; `null`: not one. */
	safeTx?: SafeTxRequest | null;
	/** `eth_sendTransaction`: the transaction, as asked. */
	send?: { to: string; data: string; value: string };
	stage: Stage;
	/** When it was handed to the wallet. */
	walletAt?: number;
	/**
	 * `done`: answered with a result; `rejected`: declined (here, or in the
	 * wallet — `problem` says which), or refused here; `failed`: the wallet
	 * failed; `withdrawn`: Vela took it back.
	 */
	outcome?: 'done' | 'rejected' | 'failed' | 'withdrawn';
	problem?: WalletProblem;
	/** A signature's form, as produced. */
	form?: 'typedData' | 'ethSign';
	/** The answer (a signature or a transaction hash). */
	result?: string;
}

export interface ConnectPrompt {
	session: string;
	params: ConnectParams;
	origin?: string;
	answer(approved: boolean): void;
}

/** A request waiting for the owner's tap. */
export interface RequestPrompt {
	entry: ActivityEntry;
	/** `true`: hand it to the wallet (in the `eth_sign` form at the `fallback` stage). */
	answer(go: boolean): void;
}

function load(): Saved[] {
	try {
		return JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '[]');
	} catch {
		return [];
	}
}

function persist(sessions: Saved[]): void {
	try {
		localStorage.setItem(STORAGE_KEY, JSON.stringify(sessions));
	} catch {
		// Storage unavailable (private mode): the session lasts as long as the page.
	}
}

const RANK: Record<RelayStatus['kind'], number> = { up: 0, connecting: 1, retrying: 2, failed: 3 };

/** Events: `change` (sessions or their states), `activity`, `prompt` (a connection or request waits for the user). */
export class Bridge extends EventTarget {
	readonly signer: InjectedSigner;
	readonly activity: ActivityEntry[] = [];
	prompt: ConnectPrompt | undefined;
	/** The request waiting for the owner's tap, if any. */
	request: RequestPrompt | undefined;

	#saved: Saved[] = load();
	#live = new Map<
		string,
		{ connection: RelayConnection; wallet: ReferenceWallet; store: WalletStore; since: number }
	>();

	/** Sessions whose connection the person declined on this page (ms-12). */
	#declined = new Set<string>();
	/** The wallet's own name (its EIP-6963 announcement): what Vela calls it (ms-13). */
	readonly walletName: string;
	/** Asks waiting for a tap, oldest first: one is shown at a time. */
	#queue: { entry: ActivityEntry; resolve(go: boolean): void }[] = [];

	constructor(provider: Eip1193Provider, accounts: string[], chain: string, walletName = '') {
		super();
		this.walletName = walletName;
		this.signer = injectedSigner(provider, accounts, chain);
		provider.on?.('accountsChanged', (list) => this.#accountsChanged(list as string[]));
		provider.on?.('chainChanged', (id) => this.#chainChanged(chainOf(id as string)));
	}

	get sessions(): SessionView[] {
		return this.#saved.map((s) => {
			const live = this.#live.get(s.id);
			return {
				id: s.id,
				state: live?.connection.state ?? 'offline',
				dapp: s.dapp,
				approved: s.store.approved.length > 0,
				declined: this.#declined.has(s.id),
				created: s.created,
				stateSince: live?.since ?? s.created
			};
		});
	}

	/** r4C-1: this page's own way onto the relay — the worst of its sessions'. */
	get relay(): RelayStatus {
		let worst: RelayStatus = { kind: 'up' };
		for (const { connection } of this.#live.values()) {
			const status = connection.relay.status;
			if (RANK[status.kind] > RANK[worst.kind]) worst = status;
		}
		return worst;
	}

	/** Try the relay again now, for every session. */
	retryRelay(): void {
		for (const { connection } of this.#live.values()) connection.relay.retry();
	}

	/** Reconnects every saved session. */
	resumeAll(): void {
		for (const saved of this.#saved) this.#start(saved);
	}

	/**
	 * Joins the room of a scanned invite, for `members` (the link's; empty
	 * when it named none). Returns the session id.
	 */
	join(invite: string, members: string[] = []): string {
		const room = Room.join(invite);
		// The joiner keeps its record before the first handshake (PeerConnect §3.4).
		const saved: Saved = {
			id: crypto.randomUUID(),
			record: room.toString(),
			created: Date.now(),
			members: members.map((a) => a.toLowerCase()),
			store: { approved: [], chains: {}, records: [], forwarded: [] }
		};
		this.#saved.push(saved);
		persist(this.#saved);
		this.#start(saved);
		return saved.id;
	}

	/** Ends a session for both sides (WalletPair core §2.4). */
	end(id: string): void {
		const live = this.#live.get(id);
		if (live) live.connection.close();
		else this.#forget(id);
	}

	#start(saved: Saved): void {
		if (this.#live.has(saved.id)) return;
		const connection = connectRoom(Room.parse(saved.record));
		const store = emptyStore();
		store.approved = saved.store.approved;
		store.chains = saved.store.chains;
		store.records = new Map(saved.store.records);
		store.forwarded = new Set(saved.store.forwarded);

		const save = () => {
			saved.record = connection.room.toString();
			saved.store = {
				approved: [...store.approved],
				chains: { ...store.chains },
				records: [...store.records],
				forwarded: [...store.forwarded]
			};
			persist(this.#saved);
			this.#changed();
		};

		const wallet = new ReferenceWallet(connection, {
			// The wallet the owner signs with, as it names itself — never this
			// page's own name, which Vela would then call the wallet (ms-13).
			name: this.walletName,
			signers: [this.#tracked()],
			store,
			save,
			prompts: {
				connect: (params, offered, origin, cancelled) =>
					this.#askConnect(saved, params, offered, origin, cancelled),
				approve: (request: Request, cancelled: AbortSignal) =>
					this.#askRequest(saved, request, cancelled)
			}
		});
		const live = { connection, wallet, store, since: Date.now() };
		this.#live.set(saved.id, live);

		connection.addEventListener('paired', save);
		connection.addEventListener('state', () => {
			live.since = Date.now();
			if (connection.state === 'closed') this.#forget(saved.id);
			this.#changed();
		});
		connection.relay.addEventListener('change', () => this.#changed());
		// r4C-12: Vela withdrawing a request (`walletpair_cancel`) is said, even
		// once the wallet has it — the wallet's own prompt stays until dismissed.
		connection.addEventListener('message', (event) => {
			const inbound = parseRequest((event as MessageEvent<Uint8Array>).data);
			if (inbound.kind !== 'request' || inbound.request.method !== 'walletpair_cancel') return;
			const params = inbound.request.params;
			const id = Array.isArray(params) ? params[0] : undefined;
			const entry = this.activity.find((a) => a.id === id && a.session === saved.id);
			if (entry && entry.stage !== 'answered') {
				entry.outcome = 'withdrawn';
				if (entry.stage === 'wallet') this.#notify();
			}
		});
		save();
	}

	/**
	 * The injected signer, with each request's outcome recorded (r4C-12) and
	 * the `eth_sign` form offered when the wallet cannot sign typed data.
	 */
	#tracked(): Signer {
		const signer = this.signer;
		return {
			network: signer.network,
			get chain() {
				return signer.chain;
			},
			get accounts() {
				return signer.accounts;
			},
			perform: async (method, params, context) => {
				const entry = this.activity.find((a) => a.id === context.request.id);
				try {
					let result;
					try {
						result = await signer.perform(method, params, context);
					} catch (error) {
						const unsupported =
							method === 'eth_signTypedData_v4' &&
							walletProblem(error).kind === 'unsupported' &&
							!!entry?.safeTx;
						if (!unsupported || !entry) throw error;
						result = await this.#ethSignForm(entry);
					}
					if (entry) {
						entry.stage = 'answered';
						entry.outcome ??= 'done';
						if (typeof result === 'string') entry.result = result;
						if (method === 'eth_signTypedData_v4') entry.form ??= 'typedData';
						this.#notify();
					}
					return result;
				} catch (error) {
					if (entry) {
						const problem = walletProblem(error);
						entry.stage = 'answered';
						entry.problem = problem;
						entry.outcome ??= problem.kind === 'declined' ? 'rejected' : 'failed';
						this.#notify();
					}
					throw error;
				}
			}
		};
	}

	/**
	 * r4C-16: the wallet cannot sign typed data. The owner may sign the
	 * safeTxHash itself instead (Safe's `eth_sign` form, `v + 4`) — on a tap,
	 * the hash shown to match; the core checks the answer on Vela's side.
	 */
	async #ethSignForm(entry: ActivityEntry): Promise<string> {
		entry.stage = 'fallback';
		const go = await this.#wait(entry);
		if (!go) throw new WalletError(ErrorCode.UserRejected, 'Declined on the connect page');
		entry.stage = 'wallet';
		entry.walletAt = Date.now();
		this.#notify();
		const hash = safeTxHash(entry.safeTx!.chainId, entry.safeTx!.safe, entry.safeTx!.tx);
		let answer: unknown;
		try {
			answer = await this.signer.provider.request({
				method: 'personal_sign',
				params: [hash, entry.account]
			});
		} catch (error) {
			const e = error as { code?: unknown; message?: unknown };
			throw new WalletError(
				typeof e?.code === 'number' ? e.code : ErrorCode.Internal,
				typeof e?.message === 'string' ? e.message : 'The wallet failed'
			);
		}
		if (typeof answer !== 'string') throw new WalletError(ErrorCode.Internal, 'No signature');
		entry.form = 'ethSign';
		return ethSignForm(answer);
	}

	/**
	 * A request to sign or send: refused here when it is not as a member the
	 * link named (ms-14); else shown and held until the owner's tap (r4C-7) —
	 * or Vela withdraws it. A network switch goes straight to the wallet,
	 * which asks itself.
	 */
	async #askRequest(saved: Saved, request: Request, cancelled: AbortSignal): Promise<boolean> {
		const as = signingAccount(request);
		const refused = as !== null && !isExpected(saved.members ?? [], as);
		const params = Array.isArray(request.params) ? request.params : [];
		const entry: ActivityEntry = {
			id: request.id,
			at: Date.now(),
			session: saved.id,
			origin: request.origin,
			method: request.method,
			account: as,
			stage: 'review'
		};
		if (request.method === 'eth_signTypedData_v4') entry.safeTx = readSafeTxRequest(params[1]);
		if (request.method === 'eth_sendTransaction') {
			const tx = (params[0] ?? {}) as { to?: unknown; data?: unknown; value?: unknown };
			entry.send = {
				to: String(tx.to ?? ''),
				data: String(tx.data ?? '0x'),
				value: String(tx.value ?? '0x0')
			};
		}
		this.#log(entry);
		if (refused) {
			entry.stage = 'answered';
			entry.outcome = 'rejected';
			this.#notify();
			return false;
		}
		if (request.method === 'wallet_switchEthereumChain') {
			entry.stage = 'wallet';
			entry.walletAt = Date.now();
			return true;
		}
		// r4C-15: Vela asking again for the same transaction (back after going
		// offline, the sheet opened again) replaces what it asked before, which
		// nobody waits on any more: that one is withdrawn, never shown first.
		const again = hashOf(entry.safeTx);
		if (again) {
			for (const held of [...this.#queue]) {
				if (held.entry.session !== saved.id || hashOf(held.entry.safeTx) !== again) continue;
				held.entry.outcome = 'withdrawn';
				this.#settle(held.entry, false);
			}
		}
		const onCancel = () => {
			entry.outcome = 'withdrawn';
			this.#settle(entry, false);
		};
		cancelled.addEventListener('abort', onCancel, { once: true });
		const go = await this.#wait(entry);
		cancelled.removeEventListener('abort', onCancel);
		if (!go) {
			entry.stage = 'answered';
			entry.outcome ??= 'rejected';
			this.#notify();
			return false;
		}
		entry.stage = 'wallet';
		entry.walletAt = Date.now();
		this.#notify();
		return true;
	}

	/** Hold `entry` for the owner's tap; one is shown at a time. */
	#wait(entry: ActivityEntry): Promise<boolean> {
		return new Promise<boolean>((resolve) => {
			this.#queue.push({ entry, resolve });
			this.#show();
		});
	}

	#settle(entry: ActivityEntry, go: boolean): void {
		const i = this.#queue.findIndex((q) => q.entry === entry);
		if (i < 0) return;
		const [held] = this.#queue.splice(i, 1);
		held.resolve(go);
		this.#show();
	}

	#show(): void {
		const next = this.#queue[0];
		this.request = next
			? { entry: next.entry, answer: (go) => this.#settle(next.entry, go) }
			: undefined;
		this.dispatchEvent(new Event('prompt'));
		this.#notify();
	}

	async #askConnect(
		saved: Saved,
		params: ConnectParams,
		offered: string[],
		origin: string | undefined,
		cancelled: AbortSignal
	): Promise<string[]> {
		saved.dapp = { name: params.name, origin, icon: params.icon };
		persist(this.#saved);
		const approved = await new Promise<boolean>((resolve) => {
			this.prompt = { session: saved.id, params, origin, answer: resolve };
			cancelled.addEventListener('abort', () => resolve(false), { once: true });
			this.dispatchEvent(new Event('prompt'));
		});
		this.prompt = undefined;
		this.dispatchEvent(new Event('prompt'));
		if (!approved) {
			// ms-12: said on this page as declined, not "Waiting for Vela…".
			this.#declined.add(saved.id);
			this.#changed();
			return [];
		}
		this.#declined.delete(saved.id);
		// The wallet app's own connection prompt, if it has not connected this page yet.
		try {
			const accounts = (await this.signer.provider.request({
				method: 'eth_requestAccounts'
			})) as string[];
			this.signer.accounts.splice(
				0,
				this.signer.accounts.length,
				...accounts.map((a) => a.toLowerCase())
			);
			this.signer.chain = await currentChain(this.signer.provider);
			for (const live of this.#live.values()) live.store.chains.eip155 = this.signer.chain;
		} catch {
			return [];
		}
		if (!this.signer.accounts.length) return [];
		this.#replaced(saved);
		return offered;
	}

	/**
	 * `current` was just approved: an older session of the same dApp that is
	 * not connected now is one it gave up — a Vela that pairs again opens a
	 * new room only when it no longer holds the old one — and is ended rather
	 * than left as one more Disconnect row for a room nobody will open.
	 */
	#replaced(current: Saved): void {
		const dapp = current.dapp;
		if (!dapp) return;
		for (const other of [...this.#saved]) {
			if (other.id === current.id || !other.dapp) continue;
			if (other.dapp.name !== dapp.name || other.dapp.origin !== dapp.origin) continue;
			const live = this.#live.get(other.id);
			if (live?.connection.state === 'connected') continue;
			// Its peer is told if it comes back while this page is open; the
			// page keeps no record of it.
			try {
				live?.connection.close();
			} catch {
				// already closing
			}
			this.#forget(other.id);
		}
	}

	#accountsChanged(list: string[]): void {
		this.signer.accounts.splice(
			0,
			this.signer.accounts.length,
			...list.map((a) => a.toLowerCase())
		);
		for (const { wallet } of this.#live.values()) {
			// No account left: the wallet app disconnected this page; the sessions stay, unapproved (core §4.2).
			if (this.signer.accounts.length === 0) wallet.revoke();
			else wallet.refresh();
		}
		this.#changed();
	}

	#chainChanged(chain: string): void {
		this.signer.chain = chain;
		for (const { wallet } of this.#live.values()) wallet.setChain('eip155', chain);
		this.#changed();
	}

	#forget(id: string): void {
		this.#live.delete(id);
		this.#saved = this.#saved.filter((s) => s.id !== id);
		persist(this.#saved);
		this.#changed();
	}

	#log(entry: ActivityEntry): void {
		this.activity.unshift(entry);
		this.activity.length = Math.min(this.activity.length, 50);
		this.#notify();
	}

	#notify(): void {
		this.dispatchEvent(new Event('activity'));
	}

	#changed(): void {
		this.dispatchEvent(new Event('change'));
	}
}

/** A request's safeTxHash, lowercase; `null` when it is not a SafeTx the core read. */
function hashOf(request: SafeTxRequest | null | undefined): string | null {
	if (!request) return null;
	try {
		return safeTxHash(request.chainId, request.safe, request.tx).toLowerCase();
	} catch {
		return null;
	}
}

/** The PeerConnect invite carried in the bridge URL's fragment (`connect-link.ts`). */
export function inviteFromHash(hash: string): string | null {
	return readConnectHash(hash).invite;
}

/** The account a request signs or sends as, lowercase; `null` for one that names none. */
export function signingAccount(request: Pick<Request, 'method' | 'params'>): string | null {
	const params = Array.isArray(request.params) ? request.params : [];
	const first = params[0] as unknown;
	const account =
		request.method === 'eth_sendTransaction'
			? (first as { from?: unknown } | undefined)?.from
			: request.method === 'eth_signTypedData_v4' || request.method === 'eth_sign'
				? first
				: request.method === 'personal_sign'
					? params[1]
					: null;
	return typeof account === 'string' ? account.toLowerCase() : null;
}
