/**
 * The proposals machine (`multisig_proposals`, `MultisigProposalsCore`) as
 * the screens ask it: its view, its events, and the waits they make on it
 * (`propose`, `until`, `settled`). It is booted, fed the app's facts and has
 * its notices routed by the ONE host of the multisig machines
 * (`machines.svelte.ts`) — {@link ProposalsHost.boot} is that host's boot;
 * the notices that are the proposals' (executed rows to the activity, a
 * mailbox that took something, an outside member's wallet) are worded here
 * ({@link ProposalsHost.route}), the host calling it. It decides nothing.
 *
 * It drives the shown multisig's list (`follow`; `core/proposals.svelte.ts`
 * is its facade: the poll at the view's `pace_ms`, `watch`, `online`,
 * `refresh`) and every action on it — propose (`draft`, its review, then
 * `confirm_draft`), approve, preview, execute, send again — what waits on
 * every multisig on this device (`presence`, `others_waiting`), a dApp's
 * request (`dapp`) and a link's proposal (`linked`). The machine owns the
 * stored keys it reads (`vela.multisigOutbox`, `…Executing`, `…Waiting`,
 * the executions, the member used last).
 */
import type { JsonWasmCore } from '$lib/core/json-shell';
import type { LocalTransaction } from '$lib/services/transactions-model';
import type { Executed } from '../core/records';
import type { MultisigAccount } from '../core/store.svelte';
import type { MultisigActionState } from '../generated/MultisigActionState';
import type { MultisigNotice } from '../generated/MultisigNotice';
import type { MultisigProposalKind } from '../generated/MultisigProposalKind';
import type { MultisigProposalsEvent } from '../generated/MultisigProposalsEvent';
import type { MultisigProposalsView } from '../generated/MultisigProposalsView';
import type { OutsideSessionRequest } from '../generated/OutsideSessionRequest';
import { openOutsideSession } from '../external/signing.svelte';
import type { MultisigShellDeps } from './handler';
import type { MultisigMachine } from './machine.svelte';

/** How often the multisigs not shown are read again while a page is up (the core's own pace). */
export const OTHERS_EVERY_MS = 60_000;

type ProposalsMachine = MultisigMachine<MultisigProposalsView, MultisigProposalsEvent>;

/** What a test hands the one host for the proposals machine (`machines.boot`). */
export interface ProposalsHostOptions {
	/** The shell's pipelines (a test's simulated world); the app's by default. */
	deps?: MultisigShellDeps;
	/** The wasm, loaded (a test's `initSync`); `loadCore` by default. */
	load?: () => Promise<void>;
	/** The proposals core (a test's); a fresh `MultisigProposalsCore` by default. */
	core?: () => JsonWasmCore;
	/** The facts the machines start from (a test's); the app's by default. */
	facts?: () => {
		serviceUrl: string;
		accounts: import('../generated/MultisigLocalAccount').MultisigLocalAccount[];
		networks: number[];
		space?: number[];
	};
	/** Every multisig on this device (a test's); the store's by default. */
	multisigs?: () => { accounts: MultisigAccount[] };
	/** Where an executed row goes (a test's); the transaction store by default. */
	saveRows?: (rows: LocalTransaction[]) => Promise<void>;
}

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
 * The machine's executed rows: which rows each writes is its plan (`rows`,
 * the core's `execution_rows`), worded as Activity rows by `executionRows`
 * (a token's symbol, the proposal's title). A row already a transaction's
 * is kept as it is.
 */
async function activityRows(rows: unknown[]): Promise<LocalTransaction[]> {
	const out: LocalTransaction[] = [];
	for (const raw of rows) {
		const row = raw as { rows?: unknown };
		if (!Array.isArray(row.rows)) {
			out.push(raw as LocalTransaction);
			continue;
		}
		const { executionRows } = await import('../core/records');
		out.push(...(await executionRows(raw as Executed)));
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
	/** Where an executed row goes (a test's); the transaction store by default. */
	#saveRows: ProposalsHostOptions['saveRows'] = undefined;
	/** The one host's reset (tests), handed over with the machine. */
	#reset: (() => void) | null = null;
	/** The machine's view, or `null` before it booted. */
	get view(): MultisigProposalsView | null {
		return this.machine?.view ?? null;
	}

	/** The one host's boot (idempotent): every machine, this one among them. */
	boot(options: ProposalsHostOptions = {}): Promise<void> {
		return import('./machines.svelte').then(({ machines }) => machines.boot(options));
	}

	/** The one host hands over the machine it booted (and how it is reset, for tests). */
	attach(
		machine: ProposalsMachine | null,
		saveRows: ProposalsHostOptions['saveRows'],
		reset: () => void
	): void {
		this.machine = machine;
		this.#saveRows = saveRows;
		this.#reset = reset;
	}

	/** An event for the machine (dropped before it booted: `boot` starts it from the facts). */
	send(event: MultisigProposalsEvent): void {
		this.machine?.dispatch(event);
	}

	/** Follow `account`'s list (or none): the machine reads it now, then on every tick. */
	async follow(account: MultisigAccount | null): Promise<void> {
		// Following none boots nothing: there is nothing to stop following.
		if (!account && !this.machine) return;
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

	/**
	 * core-ms-2: propose the draft and, its proposer's approval making it
	 * ready (`draft.executeNow`), execute it in the same press — the
	 * machine's `propose_execute` (`execute:<hash>` working from the moment
	 * it is proposed).
	 */
	proposeExecute(as: string | null): void {
		this.send({ type: 'propose_execute', as_member: as });
	}

	/** The person withdrew the proposal on its review: nothing is signed or sent. */
	withdrawDraft(): void {
		this.reviewing = null;
		if (this.view?.draft || this.view?.actions.propose?.kind === 'working') {
			this.send({ type: 'withdraw' });
		}
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

	/** Where the proposals machine's notices go (the one host's router calls this). */
	async route(notice: MultisigNotice): Promise<void> {
		switch (notice.kind) {
			case 'account':
				// A multisig's record: the one host's to route (`machines.route`).
				return;
			case 'activity_rows': {
				if (notice.rows.length === 0) return;
				const save =
					this.#saveRows ??
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
				// The accounts machine's: the store activates it.
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
				// A link's proposal: the one host's (`machines.onproposal`).
				return;
		}
	}

	/** Tests: forget the machines, as a new document would (the one host's). */
	resetForTests(): void {
		this.#reset?.();
	}

	/** The one host forgets the machine. */
	detach(): void {
		this.machine = null;
		this.#saveRows = undefined;
		this.onmailbox = null;
		this.onrecorded = null;
		this.reviewing = null;
	}
}

export const proposalsHost = new ProposalsHost();
