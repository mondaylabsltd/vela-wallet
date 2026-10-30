/**
 * The app-wide host of the core's proposals machine (`multisig_proposals`,
 * `MultisigProposalsCore`) — one machine, over the one shell (`handler.ts`,
 * the app's pipelines from `live.ts`). Kept apart from the accounts host
 * (`machines.svelte.ts`) so it joins it as one more machine: the same boot
 * after `loadCore()`, the same facts, the same notice routing.
 *
 * It boots after `loadCore()`, feeds the machine the app's facts (this
 * device's Vela accounts as `MultisigLocalAccount`, its networks, the
 * service in use, the addresses the person knows, every multisig on this
 * device), drives the reads the machine paces (`others_tick` every
 * {@link OTHERS_EVERY_MS} while a page is visible, and when it is shown
 * again) and routes the `Notice`s it sends: a multisig's record to the
 * store, executed rows to the activity, a mailbox that took something to
 * whoever shows that multisig. It decides nothing.
 *
 * It drives the shown multisig's list (`follow`; `core/proposals.svelte.ts`
 * is its facade: the poll at the view's `pace_ms`, `watch`, `online`,
 * `refresh`) and every action on it — propose (`draft`, its review, then
 * `confirm_draft`), approve, preview, execute, send again — and what waits on
 * every multisig on this device (`presence`, `others_waiting`). The machine
 * owns the stored keys it reads (`vela.multisigOutbox`, `…Executing`,
 * `…Waiting`, the executions, the member used last).
 *
 * perf-3: the transport its rooms would run on is `lazyPeerRooms` — the
 * PeerConnect client is fetched on the first room, never by a signed-in page.
 */
import { loadCore, MultisigProposalsCore } from '$lib/core/client';
import type { JsonWasmCore } from '$lib/core/json-shell';
import { contactsBook } from '$lib/contacts/core/contacts-book.svelte';
import { SERVICE_ENDPOINTS_CHANGED, STORAGE_KEYS } from '$lib/onboarding/core/storage';
import { getAllNetworksSync, subscribeNetworks } from '$lib/services/networks';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { session } from '$lib/session/core/session.svelte';
import { genesisServiceURL } from '../core/service';
import { multisig, type MultisigAccount } from '../core/store.svelte';
import type { Finding } from '../core/refusal';
import type { MultisigActionState } from '../generated/MultisigActionState';
import type { MultisigNotice } from '../generated/MultisigNotice';
import type { MultisigProposalKind } from '../generated/MultisigProposalKind';
import type { MultisigProposalsEvent } from '../generated/MultisigProposalsEvent';
import type { MultisigProposalsView } from '../generated/MultisigProposalsView';
import type { MultisigLocalAccount } from '../generated/MultisigLocalAccount';
import type { OutsideSessionRequest } from '../generated/OutsideSessionRequest';
import { openOutsideSession } from '../external/signing.svelte';
import type { MultisigShellDeps } from './handler';
import { liveDeps } from './live';
import { localAccounts } from './local-accounts';
import { MultisigMachine } from './machine.svelte';
import { lazyPeerRooms } from './peer-lazy';

/** How often the multisigs not shown are read again while a page is up (the core's own pace). */
export const OTHERS_EVERY_MS = 60_000;

type ProposalsMachine = MultisigMachine<MultisigProposalsView, MultisigProposalsEvent>;

export interface ProposalsHostOptions {
	/** The shell's pipelines (a test's simulated world); the app's by default. */
	deps?: MultisigShellDeps;
	/** The wasm, loaded (a test's `initSync`); `loadCore` by default. */
	load?: () => Promise<void>;
	/** The proposals core (a test's); a fresh `MultisigProposalsCore` by default. */
	core?: () => JsonWasmCore;
	/** The facts the machine starts from (a test's); the app's by default. */
	facts?: () => { serviceUrl: string; accounts: MultisigLocalAccount[]; networks: number[] };
	/** Every multisig on this device (a test's); the store's by default. */
	multisigs?: () => { accounts: MultisigAccount[] };
	/** Where an executed row goes (a test's); the transaction store by default. */
	saveRows?: (rows: LocalTransaction[]) => Promise<void>;
}

const chainIds = (networks: { chainId: number }[]) => networks.map((n) => n.chainId);

const same = (a: string | null | undefined, b: string | null | undefined) =>
	!!a && !!b && a.toLowerCase() === b.toLowerCase();

/** A proposal the machine was asked to make, waiting on the person's review (§3.6). */
export interface DraftAsk {
	address: string;
	chainId: number;
	/** What it is (a transfer, a rename, a member change, a cancellation). */
	kind: MultisigProposalKind['kind'];
}

/** The machine's word on a proposal that did not go ({@link ProposalsHost.propose}). */
export class ProposeFailedError extends Error {
	constructor(
		readonly state: MultisigActionState,
		readonly chainId: number
	) {
		super(`not proposed: ${state.kind}`);
		this.name = 'ProposeFailedError';
	}
}

/**
 * The addresses the person knows (r4a-18) — contacts, this device's own
 * accounts and multisigs — for the review's look-alikes and first sends; the
 * machine adds the members itself, by this device's names.
 */
function knownAddresses(): unknown[] {
	const out: { address: string; label: string; kind?: 'own' }[] = [];
	for (const c of contactsBook.view?.contacts ?? []) {
		out.push({ address: c.address, label: c.name ?? c.resolved_name ?? c.address });
	}
	for (const row of session.view.accounts ?? []) {
		out.push({ address: row.account.address, label: row.account.name, kind: 'own' });
	}
	for (const a of multisig.accounts) {
		out.push({ address: a.address, label: a.genesis.name ?? a.label ?? a.address, kind: 'own' });
	}
	return out;
}

/**
 * The machine's executed rows are facts (the proposal, the transaction that
 * ran it, when): titled and worded as Activity rows by the app's own
 * `executionRows` — one per transfer, else one titled as its proposal. A
 * row already a transaction's is kept as it is.
 */
async function activityRows(rows: unknown[]): Promise<LocalTransaction[]> {
	const out: LocalTransaction[] = [];
	for (const raw of rows) {
		const row = raw as {
			from?: string;
			chainId?: number;
			safeTxHash?: string;
			nonce?: string;
			action?: unknown;
			txHash?: string;
			userOpHash?: string;
			timestamp?: number;
		};
		if (!row.safeTxHash || row.chainId === undefined || !row.from || !row.txHash) {
			out.push(raw as LocalTransaction);
			continue;
		}
		const { executionRows } = await import('../core/records');
		const item = {
			chainId: row.chainId,
			safeTxHash: row.safeTxHash,
			nonce: BigInt(row.nonce ?? '0'),
			review: { action: row.action ?? null }
		};
		out.push(
			...(await executionRows(
				row.from,
				item as never,
				row.txHash,
				row.userOpHash ?? '',
				row.timestamp ?? Math.floor(Date.now() / 1000)
			))
		);
	}
	return out;
}

/** The multisig's record as the machine reads it (a snapshot: no proxy crosses). */
const record = (account: MultisigAccount) => $state.snapshot(account) as never;

class ProposalsHost {
	/** The machine, once booted. */
	machine = $state.raw<ProposalsMachine | null>(null);
	/** A mailbox took something for this multisig (the machine reads its list again itself). */
	onmailbox: ((safe: string) => void) | null = null;
	/** Executed rows were written to the activity: whoever shows it refreshes. */
	onrecorded: (() => void) | null = null;
	/** The proposal drafted for the person's review before its passkey (§3.6), or `null`. */
	reviewing = $state.raw<DraftAsk | null>(null);
	#booted: Promise<void> | null = null;
	#options: ProposalsHostOptions = {};
	#stops: (() => void)[] = [];

	/** The machine's view, or `null` before it booted. */
	get view(): MultisigProposalsView | null {
		return this.machine?.view ?? null;
	}

	/** Idempotent: the first caller boots, every later one shares it. */
	boot(options: ProposalsHostOptions = {}): Promise<void> {
		this.#booted ??= this.#start(options);
		return this.#booted;
	}

	#facts() {
		return (
			this.#options.facts?.() ?? {
				serviceUrl: genesisServiceURL(),
				accounts: localAccounts(),
				networks: chainIds(getAllNetworksSync())
			}
		);
	}

	async #start(options: ProposalsHostOptions): Promise<void> {
		this.#options = options;
		await (options.load ?? loadCore)();
		const deps: MultisigShellDeps = {
			...(options.deps ?? liveDeps({ peer: lazyPeerRooms(() => {}), notice: () => {} })),
			notice: (notice) => this.route(notice)
		};
		const machine: ProposalsMachine = new MultisigMachine(
			options.core?.() ?? new MultisigProposalsCore(),
			deps,
			'proposals'
		);
		this.machine = machine;
		const facts = this.#facts();
		machine.dispatch({
			type: 'start',
			service_url: facts.serviceUrl,
			accounts: facts.accounts,
			networks: facts.networks,
			known: options.facts ? [] : knownAddresses()
		});
		this.#others();
		const stop = $effect.root(() => {
			$effect(() => this.#others());
			if (!options.facts) $effect(() => this.send({ type: 'known', known: knownAddresses() }));
		});
		this.#stops.push(stop);
		if (!options.facts) this.#follow();
	}

	/** Every multisig on this device: what waits on each is the machine's to read. */
	#others(): void {
		const { accounts } = this.#options.multisigs?.() ?? { accounts: multisig.accounts };
		this.send({ type: 'accounts', accounts: $state.snapshot(accounts) as never });
	}

	/** The app's facts as they move: another tab, a network added, another service. */
	#follow(): void {
		if (typeof window === 'undefined') return;
		const onstorage = (event: StorageEvent) => {
			if (event.key === null || event.key === STORAGE_KEYS.accounts) this.refreshLocal();
			if (event.key === null || event.key === STORAGE_KEYS.serviceEndpoints) this.#service();
		};
		const onservice = () => this.#service();
		window.addEventListener('storage', onstorage);
		window.addEventListener(SERVICE_ENDPOINTS_CHANGED, onservice);
		const stopNetworks = subscribeNetworks(() => {
			this.send({ type: 'networks', networks: this.#facts().networks });
		});
		this.#stops.push(
			() => window.removeEventListener('storage', onstorage),
			() => window.removeEventListener(SERVICE_ENDPOINTS_CHANGED, onservice),
			stopNetworks
		);
	}

	#service(): void {
		this.send({ type: 'service', url: this.#facts().serviceUrl });
	}

	/** An event for the machine (dropped before it booted: `boot` starts it from the facts). */
	send(event: MultisigProposalsEvent): void {
		this.machine?.dispatch(event);
	}

	/** Follow `account`'s list (or none): the machine reads it now, then on every tick. */
	async follow(account: MultisigAccount | null): Promise<void> {
		await this.boot();
		this.send({ type: 'follow', account: account ? record(account) : null });
	}

	/** The followed multisig's record changed (a network added, a name typed). */
	account(account: MultisigAccount): void {
		if (same(this.view?.followed, account.address)) {
			this.send({ type: 'account', account: record(account) });
		}
	}

	/**
	 * The first view `pick` finds something in (the one now, or one the
	 * machine commits later), or `null` past `ms`.
	 */
	until<T>(
		pick: (view: MultisigProposalsView) => T | null | undefined,
		ms = 600_000
	): Promise<T | null> {
		const machine = this.machine;
		if (!machine) return Promise.resolve(null);
		const now = pick(machine.view);
		if (now !== null && now !== undefined) return Promise.resolve(now);
		return new Promise((resolve) => {
			const timer = setTimeout(() => {
				stop();
				resolve(null);
			}, ms);
			const stop = machine.listen((view) => {
				const found = pick(view);
				if (found === null || found === undefined) return;
				clearTimeout(timer);
				stop();
				resolve(found);
			});
		});
	}

	/** Where action `key` (`propose`, `approve:<hash>`, `execute:<hash>`) stands once it stops working. */
	settled(key: string): Promise<MultisigActionState | null> {
		return this.until((view) => {
			const state = view.actions[key];
			return state && state.kind !== 'working' ? state : null;
		});
	}

	/**
	 * Propose `proposal` on `chainId` for `account`, at the next free number:
	 * drawn with its review first (`draft`), then — `review`: once the person
	 * confirmed it on the review-first sheet ({@link reviewing}); else at once,
	 * as `as` (a cancellation asked from its sheet) — signed by a member here
	 * (`confirm_draft`), or handed to an outside member's wallet. Resolves its
	 * safeTxHash; throws {@link ProposeFailedError} with the machine's word on
	 * why it did not go (withdrawn: `cancelled`).
	 */
	async propose(
		account: MultisigAccount,
		chainId: number,
		proposal: MultisigProposalKind,
		{
			review = true,
			as = null,
			signal
		}: { review?: boolean; as?: string | null; signal?: AbortSignal } = {}
	): Promise<{ safeTxHash: string }> {
		await this.boot();
		const fail = (state: MultisigActionState | null): never => {
			throw new ProposeFailedError(state ?? { kind: 'cancelled' }, chainId);
		};
		if (signal?.aborted) fail({ kind: 'cancelled' });
		if (!same(this.view?.followed, account.address)) {
			this.send({ type: 'follow', account: record(account) });
		}
		const withdraw = () => this.withdrawDraft();
		signal?.addEventListener('abort', withdraw, { once: true });
		try {
			this.send({ type: 'draft', chain_id: chainId, proposal });
			// Drawn (its draft up), or why not (its state).
			const drafted = await this.until((view) => {
				if (view.draft) return 'drawn' as const;
				const state = view.actions.propose;
				return state && state.kind !== 'working' ? state : null;
			});
			if (signal?.aborted) fail({ kind: 'cancelled' });
			if (drafted !== 'drawn') fail(drafted);
			if (review) {
				const ask: DraftAsk = { address: account.address, chainId, kind: proposal.kind };
				this.reviewing = ask;
				// The person's word on its review: `confirm_draft`, or `withdraw` (the sheet's).
				const asked = await this.until((view) =>
					view.actions.propose?.kind === 'working' || !view.draft
						? (view.actions.propose ?? { kind: 'cancelled' })
						: null
				);
				// Confirmed: the review closes; its passkey (or outside wallet) is next.
				if (this.reviewing === ask) this.reviewing = null;
				if (asked?.kind !== 'working' && asked?.kind !== 'done') fail(asked);
			} else {
				this.send({ type: 'confirm_draft', as_member: as });
			}
			// No member here: an outside member's wallet proposes it — its answer settles it.
			const done = await this.until((view) => {
				const state = view.actions.propose;
				return state && state.kind !== 'working' && state.kind !== 'outsideAsked' ? state : null;
			});
			if (done?.kind !== 'done') {
				// Not proposed: its draft goes too (nothing more is asked or sent).
				if (this.view?.draft) this.send({ type: 'withdraw' });
				fail(done);
			}
			return { safeTxHash: (done as Extract<MultisigActionState, { kind: 'done' }>).safe_tx_hash };
		} finally {
			signal?.removeEventListener('abort', withdraw);
			if (this.reviewing?.address === account.address) this.reviewing = null;
		}
	}

	/** The person's word on the review-first sheet: propose it, as `as` (else the machine's pick). */
	confirmDraft(as: string | null): void {
		this.send({ type: 'confirm_draft', as_member: as });
	}

	/** The person withdrew the proposal on its review: nothing is signed or sent. */
	withdrawDraft(): void {
		this.reviewing = null;
		if (this.view?.draft || this.view?.actions.propose?.kind === 'working') {
			this.send({ type: 'withdraw' });
		}
	}

	/** This device's Vela accounts, read again (one was added, or signed in). */
	refreshLocal(): void {
		this.send({ type: 'local_accounts', accounts: this.#facts().accounts });
	}

	/** Read every multisig not shown again (one round at a time: the machine's). */
	async othersTick(): Promise<void> {
		await this.boot();
		this.send({ type: 'others_tick' });
	}

	/**
	 * Read the others now and every {@link OTHERS_EVERY_MS} while the page is
	 * shown, and when it is shown again. Returns the stop.
	 */
	followOthers(): () => void {
		const run = () => {
			if (document.visibilityState === 'visible') void this.othersTick();
		};
		run();
		const timer = setInterval(run, OTHERS_EVERY_MS);
		document.addEventListener('visibilitychange', run);
		let stopped = false;
		return () => {
			if (stopped) return;
			stopped = true;
			clearInterval(timer);
			document.removeEventListener('visibilitychange', run);
		};
	}

	/** Where the machine's notice goes. */
	async route(notice: MultisigNotice): Promise<void> {
		switch (notice.kind) {
			case 'account': {
				const { address, update } = notice;
				switch (update.kind) {
					case 'use_chain':
						multisig.useChain(address, update.chain_id);
						return;
					case 'findings_on':
						multisig.setFindingsOn(address, update.chain_id, update.findings as Finding[]);
						return;
					default:
						// The accounts machine applies it (`machines.svelte.ts` routes the same).
						multisig.send({ type: 'apply', address, update });
						return;
				}
			}
			case 'activity_rows': {
				if (notice.rows.length === 0) return;
				const save =
					this.#options.saveRows ??
					(async (rows: LocalTransaction[]) =>
						(await import('$lib/services/records')).saveTransactions(rows));
				await save(await activityRows(notice.rows));
				this.onrecorded?.();
				return;
			}
			case 'mailbox_took':
				this.onmailbox?.(notice.safe);
				// What was signed here reached it: the shown list is read again.
				if (same(this.view?.followed, notice.safe)) this.send({ type: 'refresh', fresh: true });
				return;
			case 'activated':
				// The accounts machine's (`machines.svelte.ts`): the store activates it.
				return;
			case 'outside_sign': {
				// An outside member's wallet signs it; its answer goes back to the machine.
				const request = notice.request as OutsideSessionRequest;
				void openOutsideSession(request).then((outcome) =>
					this.send({
						type: 'outside_answered',
						chain_id: request.chainId,
						safe_tx_hash: request.safeTxHash,
						outcome
					})
				);
				return;
			}
			case 'open_proposal':
				// A link's proposal (the accounts machine's): `MultisigHost` opens links.
				return;
		}
	}

	/** Tests: forget the machine, as a new document would. */
	resetForTests(): void {
		for (const stop of this.#stops.splice(0)) stop();
		this.machine?.dispose();
		this.machine = null;
		this.#booted = null;
		this.#options = {};
		this.onmailbox = null;
		this.onrecorded = null;
		this.reviewing = null;
	}
}

export const proposalsHost = new ProposalsHost();
