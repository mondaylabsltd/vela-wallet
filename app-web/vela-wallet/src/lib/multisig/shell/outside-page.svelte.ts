/**
 * Vela's `/connect` page, run by the core's page machine
 * (`multisig_outside::page::OutsidePage`, wasm `MultisigOutsidePageCore`):
 * the transaction a link names — read, verified and counted by the core,
 * approved, executed or rejected through the injected wallet, what was
 * signed kept and sent again — and the page's side of live WalletPair
 * sessions with Vela. The machine decides; this holds its view for the page
 * to draw, and hands it the page's I/O: the RPC pool, `localStorage`, the
 * PeerConnect rooms (`peer-lazy.ts`), the injected wallet (EIP-1193, its own
 * `accountsChanged` / `chainChanged` dispatched as the `wallet` event), and
 * the §3.5 risk question, asked on the page itself.
 *
 * Also the core's reading of the connect page's link (`multisig_link`): what
 * its fragment holds, and whether an account is one of its members.
 */
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { genesisServiceURL } from '../core/service';
import type { ProposalLink } from '../core/link';
import type { OutsidePageEvent } from '../generated/OutsidePageEvent';
import type { OutsidePageView } from '../generated/OutsidePageView';
import type { MultisigOperation } from '../generated/MultisigOperation';
import type { JsonWasmCore } from '$lib/core/json-shell';
import type { MultisigShellDeps, PeerTransport } from './handler';
import { lazyPeerRooms } from './peer-lazy';
import type { PeerEvent, PeerRooms } from './peer';
import { MultisigMachine } from './machine.svelte';

type Op<T extends MultisigOperation['type']> = Extract<MultisigOperation, { type: T }>;

/** The injected wallet (EIP-1193), as the page holds it. */
export interface PageWallet {
	request(args: { method: string; params?: unknown }): Promise<unknown>;
	on?(event: string, listener: (...args: unknown[]) => void): void;
	removeListener?(event: string, listener: (...args: unknown[]) => void): void;
}

/** §3.5: the risk the core found, put to the person on the page; answered once. */
export interface RiskAsk {
	chainId: number;
	findings: unknown[];
	answer(signs: boolean): void;
}

/** What a connect page's fragment holds, as the core reads it (`readConnectHash`). */
export interface ConnectHash {
	invite: string | null;
	members: string[];
	tx: ProposalLink | null;
	damaged: boolean;
}

function linkCall<T>(method: string, args: Record<string, unknown>): T {
	return JSON.parse(wasm.multisigLinkCall(method, JSON.stringify(args))) as T;
}

/** The core's reading of the page's fragment (callable once the core is loaded). */
export function readConnectHash(hash: string): ConnectHash {
	return linkCall<ConnectHash>('readConnectHash', { hash });
}

/** The page's link with `members` and `tx` (and the invite, when one is given). */
export function connectLink(
	page: string,
	invite: string | null,
	members: string[],
	tx: ProposalLink | null = null
): string {
	return linkCall<string>('connectLink', { page, invite, members, tx });
}

/** Whether `account` is one of `members` (any account, when the link named none). */
export function isExpected(members: string[], account: string): boolean {
	return linkCall<boolean>('isExpected', { members, account });
}

/** `eth_chainId`'s answer as a number, or `null` for one that does not read. */
export function chainIdOf(answer: unknown): number | null {
	if (typeof answer !== 'string' && typeof answer !== 'number') return null;
	const n = Number(answer);
	return Number.isSafeInteger(n) && n > 0 ? n : null;
}

/**
 * The page's dependencies: no passkey, no send pipeline — the page signs and
 * sends through the owner's own wallet (`wallet_request`).
 */
export function pageDeps(input: {
	peer: PeerTransport;
	wallet(): PageWallet | null;
	askRisk(op: Op<'ask_risk'>): Promise<boolean>;
}): MultisigShellDeps {
	const unsupported = (what: string) =>
		Promise.reject(new Error(`${what}: not on the connect page`));
	return {
		fetch: (url, init) => fetch(url, { ...init, cache: 'no-store' }),
		rpc: (chainId, method, params, person) =>
			poolRpcCall(method, (Array.isArray(params) ? params : [params]) as unknown[], chainId, 0, {
				person: person ?? false
			}),
		// R22: a guard's read, two independent nodes (the core's envelope).
		rpcCorroborated: async (chainId, method, params, person) =>
			(await import('$lib/services/corroborated-read')).corroboratedRpc(
				chainId,
				method,
				params,
				person ?? false
			),
		storage: () => {
			try {
				return typeof localStorage === 'undefined' ? null : localStorage;
			} catch {
				return null;
			}
		},
		now: () => Date.now(),
		random: (n) => crypto.getRandomValues(new Uint8Array(n)),
		passkey: () => unsupported('passkey'),
		askRisk: input.askRisk,
		sendCalls: () => unsupported('send'),
		quoteCalls: () => unsupported('quote'),
		// The signing sheet's own simulation of the review's calls, loaded on the first ask.
		simulate: async (op) => {
			const { rpcSimulateReply } = await import('$lib/services/sim/sim-engine-rpc');
			return rpcSimulateReply(
				op.from,
				op.calls as never,
				op.chain_id,
				op.state_overrides ?? undefined
			).catch(() => null);
		},
		// A recipient as the chain says it; this browser holds no history of the owner's.
		recipientFacts: async (op) => {
			const code = await poolRpcCall('eth_getCode', [op.address, 'latest'], op.chain_id).catch(
				() => null
			);
			const raw = code?.result;
			return {
				isContract: typeof raw === 'string' ? raw !== '0x' && raw !== '0x0' : null,
				firstTime: null
			};
		},
		userOpStatus: async () => null,
		peer: input.peer,
		wallet: async (method, params) => {
			const wallet = input.wallet();
			if (!wallet) throw { code: 4900, message: 'no wallet connected' };
			return wallet.request({ method, params: params as never });
		},
		notice: () => {}
	};
}

/** The connect page's machine, with the page's I/O. */
export class OutsidePageHost {
	/** §3.5: a risk the core asks about before a signature, waiting for the person. */
	risk = $state<RiskAsk | null>(null);
	readonly #machine: MultisigMachine<OutsidePageView, OutsidePageEvent>;
	readonly #rooms: PeerRooms;
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- the rooms open, for Retry; never drawn
	readonly #handles = new Set<string>();
	#wallet: PageWallet | null = null;
	#unsubscribe: (() => void) | null = null;

	constructor(core: JsonWasmCore = new wasm.MultisigOutsidePageCore()) {
		this.#rooms = lazyPeerRooms((event: PeerEvent) => this.#machine.dispatch(event));
		const rooms = this.#rooms;
		const handles = this.#handles;
		const peer: PeerTransport = {
			open: async (opening) => {
				const opened = await rooms.open(opening);
				handles.add(opened.room);
				return opened;
			},
			send: (room, message) => rooms.send(room, message),
			receive: (room, signal) => rooms.receive(room, signal),
			close: async (room, cancel, end) => {
				handles.delete(room);
				await rooms.close(room, cancel, end);
			}
		};
		const deps = pageDeps({
			peer,
			wallet: () => this.#wallet,
			askRisk: (op) =>
				new Promise<boolean>((resolve) => {
					this.risk?.answer(false);
					this.risk = {
						chainId: op.chain_id,
						findings: op.findings as unknown[],
						answer: (signs) => {
							this.risk = null;
							resolve(signs);
						}
					};
				})
		});
		this.#machine = new MultisigMachine<OutsidePageView, OutsidePageEvent>(core, deps, 'connect');
		this.#machine.dispatch({ type: 'start', service_url: genesisServiceURL(), wallet_name: '' });
	}

	get view(): OutsidePageView {
		return this.#machine.view;
	}

	dispatch(event: OutsidePageEvent): void {
		this.#machine.dispatch(event);
	}

	/**
	 * The wallet picked (`name`: its own, EIP-6963): its accounts and network
	 * now, its own events from then on, and the sessions kept here resumed.
	 * A wallet that will not say its accounts has none connected yet: the
	 * page offers to connect it (`connect_wallet`), its refusal said then.
	 */
	async useWallet(wallet: PageWallet, name: string): Promise<void> {
		const said = await wallet.request({ method: 'eth_accounts' }).catch(() => []);
		const accounts = (Array.isArray(said) ? said : []).map(String);
		const chain = chainIdOf(await wallet.request({ method: 'eth_chainId' }).catch(() => null));
		this.#unsubscribe?.();
		this.#wallet = wallet;
		let chainNow = chain;
		let accountsNow = accounts.map((a) => a.toLowerCase());
		const onAccounts = (list: unknown) => {
			accountsNow = (Array.isArray(list) ? list : []).map((a) => String(a).toLowerCase());
			this.#machine.dispatch({ type: 'wallet', accounts: accountsNow, chain_id: chainNow });
		};
		const onChain = (id: unknown) => {
			chainNow = chainIdOf(id) ?? chainNow;
			this.#machine.dispatch({ type: 'wallet', accounts: accountsNow, chain_id: chainNow });
		};
		wallet.on?.('accountsChanged', onAccounts);
		wallet.on?.('chainChanged', onChain);
		this.#unsubscribe = () => {
			wallet.removeListener?.('accountsChanged', onAccounts);
			wallet.removeListener?.('chainChanged', onChain);
		};
		this.#machine.dispatch({ type: 'start', service_url: genesisServiceURL(), wallet_name: name });
		this.#machine.dispatch({ type: 'wallet', accounts: accountsNow, chain_id: chain });
		this.#machine.dispatch({ type: 'resume_all' });
	}

	/** Try the relay again now, for every room (the person's Retry). */
	retryRelay(): void {
		for (const room of this.#handles) this.#rooms.retry(room);
	}

	/** The page went away: its rooms left (the pairings kept), its wallet let go. */
	dispose(): void {
		this.risk?.answer(false);
		this.#unsubscribe?.();
		this.#rooms.leaveAll();
		this.#machine.dispose();
	}
}
