// The connect page's sessions (ported from sub3's bridge): each is a PeerConnect room joined from Vela's link or
// code, with WalletPair's reference Wallet on top, all signing through the one injected wallet. Rooms and Wallet
// records are kept in localStorage, so a session survives the page reloading (WalletPair core §2.4, §6;
// PeerConnect §3.4) and Vela can ask the same wallet again without a new code.

import { Room } from '$lib/vendor/peerconnect/index.js';
import type { Eip1193Provider } from './eip6963';
import { chainOf } from '$lib/walletpair/networks/evm';
import type { ConnectParams, Request, Response } from '$lib/walletpair/protocol';
import { ReferenceWallet, emptyStore, type WalletStore } from '$lib/walletpair/wallet';
import { connectRoom } from './relay';
import { injectedSigner, currentChain, type InjectedSigner } from './injected';
import { isExpected, readConnectHash } from './connect-link';

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
}

export interface ActivityEntry {
	at: number;
	session: string;
	origin?: string;
	method: string;
	outcome?: 'done' | 'rejected' | 'failed';
}

export interface ConnectPrompt {
	session: string;
	params: ConnectParams;
	origin?: string;
	answer(approved: boolean): void;
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

/** Events: `change` (sessions or their states), `activity`, `prompt` (a connection waits for the user). */
export class Bridge extends EventTarget {
	readonly signer: InjectedSigner;
	readonly activity: ActivityEntry[] = [];
	prompt: ConnectPrompt | undefined;

	#saved: Saved[] = load();
	#live = new Map<
		string,
		{ connection: ReturnType<typeof connectRoom>; wallet: ReferenceWallet; store: WalletStore }
	>();

	/** Sessions whose connection the person declined on this page (ms-12). */
	#declined = new Set<string>();
	/** The wallet's own name (its EIP-6963 announcement): what Vela calls it (ms-13). */
	readonly walletName: string;

	constructor(provider: Eip1193Provider, accounts: string[], chain: string, walletName = '') {
		super();
		this.walletName = walletName;
		this.signer = injectedSigner(provider, accounts, chain);
		provider.on?.('accountsChanged', (list) => this.#accountsChanged(list as string[]));
		provider.on?.('chainChanged', (id) => this.#chainChanged(chainOf(id as string)));
	}

	get sessions(): SessionView[] {
		return this.#saved.map((s) => ({
			id: s.id,
			state: this.#live.get(s.id)?.connection.state ?? 'offline',
			dapp: s.dapp,
			approved: s.store.approved.length > 0,
			declined: this.#declined.has(s.id),
			created: s.created
		}));
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
			signers: [this.signer],
			store,
			save,
			prompts: {
				connect: (params, offered, origin, cancelled) =>
					this.#askConnect(saved, params, offered, origin, cancelled),
				approve: async (request: Request) => {
					// ms-14: a signature or a transaction as someone the link did
					// not name is refused here, before the wallet is asked.
					const as = signingAccount(request);
					const refused = as !== null && !isExpected(saved.members ?? [], as);
					// The wallet app asks the user itself; the bridge shows what is happening.
					this.#log({
						at: Date.now(),
						session: saved.id,
						origin: request.origin,
						method: request.method,
						...(refused ? { outcome: 'rejected' as const } : {})
					});
					return !refused;
				}
			}
		});
		this.#live.set(saved.id, { connection, wallet, store });

		connection.addEventListener('paired', save);
		connection.addEventListener('state', () => {
			if (connection.state === 'closed') this.#forget(saved.id);
			this.#changed();
		});
		save();
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
		this.dispatchEvent(new Event('activity'));
	}

	#changed(): void {
		this.dispatchEvent(new Event('change'));
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
