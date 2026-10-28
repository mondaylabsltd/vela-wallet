/**
 * An outside owner's wallet (MetaMask, Rabby, a wallet app…), reached the way
 * remote-inject does it: Vela opens an end-to-end encrypted PeerConnect room
 * and is WalletPair's DApp side; the owner opens Vela's `/connect` page in the
 * browser that holds the wallet (a link or a scanned code), and that page is
 * the Wallet side, forwarding to the injected wallet. Keys, prompts and
 * signing stay in the owner's wallet.
 *
 * Nothing that comes back is believed: a signature is checked by the core
 * (`verifyEcdsaApproval`: `ecrecover` over the safeTxHash is the owner) before
 * it is used, and the accounts the page reports only decide whom to ask.
 *
 * A paired room is kept on this device (its record, and the accounts it last
 * reported), so the same wallet signs again without a new code: the page
 * resumes its rooms when it is opened again.
 */
import { Room } from '$lib/vendor/peerconnect/index.js';
import { CallError, WalletPairClient } from '$lib/walletpair/client';
import { namespaceRules } from '$lib/walletpair/networks';
import { chainIdOf } from '$lib/walletpair/networks/evm';
import { ErrorCode, type Json } from '$lib/walletpair/protocol';
import { sameAddress } from '../core/kernel';
import { connectRoom, relayUrl } from './relay';

const SAVED_KEY = 'vela.multisig.externalWallets.v1';

interface Saved {
	id: string;
	record: string;
	/** Lowercase, as the page last reported them. */
	accounts: string[];
	wallet?: string;
	savedMs: number;
}

function readSaved(): Saved[] {
	try {
		const rows = JSON.parse(localStorage.getItem(SAVED_KEY) ?? '[]') as unknown;
		return Array.isArray(rows)
			? (rows as Saved[]).filter((r) => typeof r?.id === 'string' && typeof r.record === 'string')
			: [];
	} catch {
		return [];
	}
}

function writeSaved(rows: Saved[]): void {
	try {
		localStorage.setItem(SAVED_KEY, JSON.stringify(rows.slice(-8)));
	} catch {
		// private mode: the pairing lasts as long as the page
	}
}

/** Where the session stands, for the sheet to say. */
export type Phase =
	/** The room is open; the page has not joined it yet. */
	| 'waiting'
	/** The page is here; the owner is approving the connection in it. */
	| 'connecting'
	/** Connected, but none of its accounts is one that may sign. */
	| 'wrongAccount'
	/** Asking the wallet to switch to the transaction's network. */
	| 'switching'
	/** The wallet is asking the owner. */
	| 'confirming'
	| 'done'
	| 'failed';

export class WalletDeclinedError extends Error {
	constructor() {
		super('The wallet declined');
		this.name = 'WalletDeclinedError';
	}
}

export class ExternalWallet {
	phase = $state<Phase>('waiting');
	/** Lowercase accounts the page reports, once connected. */
	accounts = $state<string[]>([]);
	/** `eip155:<id>` of the wallet's active network. */
	chain = $state<string | null>(null);
	wallet = $state<string | null>(null);
	/** The page to open: with the invite while unpaired, plain once paired. */
	link = $state('');

	readonly #connection: ReturnType<typeof connectRoom>;
	readonly #client: WalletPairClient;
	readonly #saved: Saved;
	readonly #page: string;

	private constructor(saved: Saved, page: string, fresh: boolean) {
		this.#saved = saved;
		this.#page = page;
		const room = Room.parse(saved.record);
		this.#connection = connectRoom(room);
		this.#client = new WalletPairClient(this.#connection, namespaceRules);
		this.link = fresh && room.invite ? `${page}#${encodeURIComponent(room.invite)}` : page;
		this.accounts = saved.accounts;
		this.wallet = saved.wallet ?? null;
		this.#connection.addEventListener('paired', () => {
			// Paired: the invite is spent; the page resumes by its own record.
			this.link = page;
			this.#save();
		});
		this.#connection.addEventListener('state', () => {
			if (this.#connection.state === 'connected' && this.phase === 'waiting') {
				this.phase = 'connecting';
			}
			// Left from here keeps the pairing; ended by either side forgets it.
			if (this.#connection.state === 'closed' && this.#connection.reason !== 'left') {
				this.#forget();
			}
		});
		this.#client.addEventListener('state', () => this.#stateChanged());
		this.#client.addEventListener('ended', () => this.#forget());
	}

	/**
	 * A session that can sign for one of `owners`: one this device paired
	 * before with such an account, else a new room. `page` is Vela's
	 * `/connect` page for the person's language.
	 */
	static for(owners: string[], page: string): ExternalWallet {
		const known = readSaved()
			.reverse()
			.find((s) => s.accounts.some((a) => owners.some((o) => sameAddress(o, a))));
		if (known) return new ExternalWallet(known, page, false);
		const room = Room.create(relayUrl());
		const saved: Saved = {
			id: crypto.randomUUID(),
			record: room.toString(),
			accounts: [],
			savedMs: Date.now()
		};
		return new ExternalWallet(saved, page, true);
	}

	/** Whether this is a room paired before (the page only needs reopening). */
	get resumed(): boolean {
		return this.link === this.#page;
	}

	/** The accounts the wallet reports, among `owners`. */
	signers(owners: string[]): string[] {
		return this.accounts.filter((a) => owners.some((o) => sameAddress(o, a)));
	}

	/**
	 * Wait for the page and the owner's approval of the connection. A room
	 * paired before is approved already once the page is back.
	 */
	async ready(signal: AbortSignal): Promise<void> {
		if (this.#client.approved && this.#connection.state === 'connected') return;
		const origin = location.origin;
		const answer = this.#client.start(
			'walletpair_connect',
			[{ name: 'Vela', url: origin, namespaces: ['eip155'] }],
			origin
		);
		await this.#settle(answer, signal);
		this.#stateChanged();
	}

	/**
	 * The owner's EIP-712 signature of a SafeTx (`eth_signTypedData_v4`), by
	 * one of `owners` the wallet holds: the wallet is moved to the
	 * transaction's network first, since it signs only for its active one.
	 * Returns `{ owner, signature }` as the wallet answered — the caller
	 * checks it.
	 */
	async signTypedData(
		chainId: number,
		owners: string[],
		typedData: Record<string, unknown>,
		signal: AbortSignal
	): Promise<{ owner: string; signature: string }> {
		await this.ready(signal);
		const owner = await this.#ownerAmong(owners, signal);
		await this.#onChain(chainId, signal);
		this.phase = 'confirming';
		const signature = await this.#settle(
			this.#client.start(
				'eth_signTypedData_v4',
				[owner, JSON.stringify(typedData)],
				location.origin
			),
			signal
		);
		if (typeof signature !== 'string') throw new Error('The wallet answered no signature');
		return { owner, signature };
	}

	/** Send a transaction from `from` with the wallet (it pays the fee). Returns its hash. */
	async sendTransaction(
		chainId: number,
		tx: { from: string; to: string; data: string; value?: string },
		signal: AbortSignal
	): Promise<string> {
		await this.ready(signal);
		await this.#ownerAmong([tx.from], signal);
		await this.#onChain(chainId, signal);
		this.phase = 'confirming';
		const hash = await this.#settle(
			this.#client.start(
				'eth_sendTransaction',
				[{ from: tx.from.toLowerCase(), to: tx.to, data: tx.data, value: tx.value ?? '0x0' }],
				location.origin
			),
			signal
		);
		if (typeof hash !== 'string') throw new Error('The wallet answered no transaction');
		return hash;
	}

	/** Stop using the room on this side; the pairing stays for next time. */
	leave(): void {
		this.#connection.leave();
	}

	/** End the pairing for both sides and forget it. */
	end(): void {
		this.#client.close();
		this.#connection.close();
		this.#forget();
	}

	async #ownerAmong(owners: string[], signal: AbortSignal): Promise<string> {
		for (;;) {
			const found = this.signers(owners)[0];
			if (found) return found;
			this.phase = 'wrongAccount';
			// Wait for the wallet to switch accounts (a `state` event), or a cancel.
			await new Promise<void>((resolve, reject) => {
				const done = () => {
					this.#client.removeEventListener('state', done);
					signal.removeEventListener('abort', aborted);
					resolve();
				};
				const aborted = () => {
					this.#client.removeEventListener('state', done);
					reject(new DOMException('Cancelled', 'AbortError'));
				};
				this.#client.addEventListener('state', done);
				signal.addEventListener('abort', aborted, { once: true });
			});
		}
	}

	async #onChain(chainId: number, signal: AbortSignal): Promise<void> {
		if (this.chain === `eip155:${chainId}`) return;
		this.phase = 'switching';
		await this.#settle(
			this.#client.start(
				'wallet_switchEthereumChain',
				[{ chainId: chainIdOf(`eip155:${chainId}`) }],
				location.origin
			),
			signal
		);
		this.#stateChanged();
		if (this.chain !== `eip155:${chainId}`) throw new Error('The wallet stayed on another network');
	}

	/** A call's answer, withdrawn (`walletpair_cancel`) if the person cancels here. */
	async #settle(
		started: { id: string; result: Promise<Json> },
		signal: AbortSignal
	): Promise<Json> {
		const cancel = () => this.#client.cancel([started.id]);
		if (signal.aborted) cancel();
		signal.addEventListener('abort', cancel, { once: true });
		try {
			return await started.result;
		} catch (error) {
			if (error instanceof CallError && error.code === ErrorCode.UserRejected) {
				throw signal.aborted
					? new DOMException('Cancelled', 'AbortError')
					: new WalletDeclinedError();
			}
			throw error;
		} finally {
			signal.removeEventListener('abort', cancel);
		}
	}

	#stateChanged(): void {
		const evm = this.#client.state.eip155;
		this.accounts = evm?.accounts ?? [];
		this.chain = evm?.chain ?? null;
		this.wallet = this.#client.wallet?.name ?? this.wallet;
		if (this.#client.approved && (this.phase === 'waiting' || this.phase === 'connecting')) {
			this.phase = 'connecting';
		}
		this.#save();
	}

	#save(): void {
		this.#saved.record = this.#connection.room.toString();
		this.#saved.accounts = this.accounts;
		this.#saved.wallet = this.wallet ?? undefined;
		this.#saved.savedMs = Date.now();
		// Only a paired room is worth keeping: an unused invite is spent on close.
		if (!this.#connection.room.paired) return;
		const rows = readSaved().filter((s) => s.id !== this.#saved.id);
		writeSaved([...rows, this.#saved]);
	}

	#forget(): void {
		writeSaved(readSaved().filter((s) => s.id !== this.#saved.id));
	}
}
