/**
 * The multisig accounts this browser has added, and which one (if any) is the
 * account the wallet is showing — a facade over the core's accounts machine
 * (`vela_core::app::multisig_accounts`, the same one the apps drive), kept
 * synchronous for the app's callers.
 *
 * Kept under keys of their own, never in `vela.accounts`: an older build reads
 * that list with a parser that requires a passkey, and its migration re-derives
 * every address from the keys — a multisig row there would be dropped or
 * rewritten (docs/multisig-vault-design.md §3.1). The Vela accounts stay the
 * session core's; a multisig is an overlay on top of it: when one is active,
 * the wallet shows its address, and the session's own active account is
 * untouched underneath.
 *
 * The machine is the only writer of `vela.multisigAccounts` and
 * `vela.activeMultisig`. Until its view says `loaded` (the wasm is fetched
 * after the first paint), what is shown is a display-only snapshot of the
 * stored JSON, so the first paint does not change; a change asked for then
 * waits for the machine, and is never written here.
 */
import { untrack } from 'svelte';
import type { Finding } from './refusal';
import { sameAddress, type Genesis } from './kernel';
import type { MultisigAccountsView } from '../generated/MultisigAccountsView';
import type { MultisigAccountsEvent } from '../generated/MultisigAccountsEvent';
import type { MultisigAccountRow } from '../generated/MultisigAccountRow';
import type { MultisigAddOutcome } from '../generated/MultisigAddOutcome';

export const MULTISIG_ACCOUNTS_KEY = 'vela.multisigAccounts';
export const ACTIVE_MULTISIG_KEY = 'vela.activeMultisig';

/** One stored multisig, in the JSON the machine keeps (`MultisigAccount`). */
export interface MultisigAccount {
	address: string;
	genesis: Genesis;
	/** The founding members (the genesis's), in their order. */
	owners: string[];
	threshold: number;
	/** Whether the registry is known to hold the genesis (else it is retried). */
	recorded: boolean;
	/**
	 * Whether the genesis service can hand the genesis to another device — the
	 * registry holds it, or the service keeps it until the record lands.
	 */
	shared?: boolean;
	/** The networks it is used on: where its proposals are looked for. */
	chainIds: number[];
	/**
	 * The person's own name for it on this device — an imported Safe's name
	 * from the Safe{Wallet} address book. Never part of the Safe; a name in
	 * its genesis comes first.
	 */
	label?: string;
	/**
	 * What this device knows of its members (r4D-5), by lowercase address: a
	 * name and a kind, typed by its creator or carried by an invite. Labels
	 * only — who may act is the genesis's and each chain's, never this.
	 */
	members?: Record<string, MemberInfo>;
	/**
	 * r4D-8: created on this device, its first steps (invite, fund, what the
	 * first transfer costs) not dismissed yet.
	 */
	firstRun?: boolean;
	addedMs: number;
	/**
	 * What else can act for it, as the chain showed when the person added it
	 * anyway (§3.5: a module, a guard, a handler or singleton that is not
	 * Safe's) — kept to be shown wherever it is, never hidden. The machine's
	 * re-check keeps each network's current: a risk found there since, or its
	 * genesis refused on a network it is not deployed on (read-only there).
	 */
	findings?: Finding[];
}

/** A member's label and kind, as one device knows it (never authority). */
export interface MemberInfo {
	name?: string;
	/**
	 * A Vela account (its key a passkey), an outside wallet, or (r7) a smart
	 * contract account that approves from its own app.
	 */
	kind?: 'passkey' | 'wallet' | 'contract';
	/**
	 * r5reg-16: `name` is what the person typed on this device (Settings →
	 * Add member), not a link's label — shown for that member over any other.
	 */
	typed?: true;
}

/**
 * The accounts machine as the facade drives it: its view, its events, and a
 * word each time the view moves (`shell/machine.svelte.ts`; a test's stand-in).
 */
export interface AccountsMachine {
	readonly view: MultisigAccountsView;
	dispatch(event: MultisigAccountsEvent): void;
	listen(onview: (view: MultisigAccountsView) => void): () => void;
}

/**
 * What a lookup by address came to (the machine's `MultisigAddOutcome`), in
 * the words the sheets say it in: the multisig read, or why it is not.
 */
export type FoundOutcome =
	| { ok: true; account: MultisigAccount }
	| {
			ok: false;
			reason:
				'notFound' | 'unavailable' | 'unsupported' | 'notMultisig' | 'chooseNetwork' | 'unchecked';
	  }
	| { ok: false; reason: 'refused'; findings: Finding[] }
	| { ok: false; reason: 'unverified'; chainId: number }
	/** r14-5: `chainId`'s nodes were rate-limiting the lookup's reads. */
	| { ok: false; reason: 'busy'; chainId: number }
	| { ok: false; reason: 'highRisk'; findings: Finding[]; account: MultisigAccount | null };

export function foundOutcome(
	outcome: MultisigAddOutcome | null,
	account: MultisigAccount | null
): FoundOutcome {
	if (!outcome) return { ok: false, reason: 'unavailable' };
	switch (outcome.kind) {
		case 'added':
			return account ? { ok: true, account } : { ok: false, reason: 'notFound' };
		case 'refused':
			return { ok: false, reason: 'refused', findings: outcome.findings as Finding[] };
		case 'highRisk':
			return { ok: false, reason: 'highRisk', findings: outcome.findings as Finding[], account };
		case 'unverified':
			return { ok: false, reason: 'unverified', chainId: outcome.chain_id };
		case 'busy':
			return { ok: false, reason: 'busy', chainId: outcome.chain_id };
		default:
			return { ok: false, reason: outcome.kind };
	}
}

/** How long a kept multisig is waited for before nothing was kept. */
const KEEP_MS = 5_000;

/** The stored list, read for the first paint only (the machine reads it again). */
function snapshot(): { accounts: MultisigAccount[]; active: string | null } {
	try {
		const raw = localStorage.getItem(MULTISIG_ACCOUNTS_KEY);
		const rows = raw ? (JSON.parse(raw) as unknown) : [];
		const accounts = Array.isArray(rows)
			? rows
					.filter(
						(r): r is MultisigAccount =>
							!!r &&
							typeof r === 'object' &&
							typeof (r as MultisigAccount).address === 'string' &&
							!!(r as MultisigAccount).genesis
					)
					.map((r) => ({ ...r, chainIds: Array.isArray(r.chainIds) ? r.chainIds : [] }))
			: [];
		return { accounts, active: localStorage.getItem(ACTIVE_MULTISIG_KEY) || null };
	} catch {
		return { accounts: [], active: null };
	}
}

class MultisigStore {
	#machine = $state.raw<AccountsMachine | null>(null);
	#snapshot = $state.raw<{ accounts: MultisigAccount[]; active: string | null }>({
		accounts: [],
		active: null
	});
	/** Changes asked for before the machine was loaded: sent once it is. */
	#queue: MultisigAccountsEvent[] = [];
	#booted = false;

	/** The machine's view, once it has read the store; `null` before. */
	get view(): MultisigAccountsView | null {
		return this.#machine?.view ?? null;
	}

	/** The last view's accounts, so one view reads as one list (not reactive itself). */
	#read: { view: MultisigAccountsView; accounts: MultisigAccount[] } | null = null;

	/** Every multisig on this device, in the order they were added. */
	get accounts(): MultisigAccount[] {
		const view = this.view;
		if (!view) return this.#snapshot.accounts;
		if (this.#read?.view !== view) {
			// A multisig the view did not change stays the same object: every view
			// is a fresh parse, and a sheet holding one must not see it replaced
			// (and redraw) each time the machine says anything.
			const was = new Map((this.#read?.accounts ?? []).map((a) => [JSON.stringify(a), a] as const));
			this.#read = {
				view,
				accounts: view.accounts.map((row) => {
					const account = row.account as MultisigAccount;
					return was.get(JSON.stringify(account)) ?? account;
				})
			};
		}
		return this.#read.accounts;
	}

	/**
	 * §31: every multisig address on this device — the machine's list once it
	 * is loaded, else the stored one (read now when nothing has booted the
	 * store yet). What the signing machine refuses a site's request to act
	 * for. Reactive once booted.
	 */
	addresses(): string[] {
		const accounts =
			this.view || this.#booted || typeof window === 'undefined'
				? this.accounts
				: snapshot().accounts;
		return accounts.map((a) => a.address);
	}

	/** The machine's row for `address` (its name, sharing, flags), once it is loaded. */
	row(address: string): MultisigAccountRow | null {
		return (
			this.view?.accounts.find((r) =>
				sameAddress((r.account as MultisigAccount).address, address)
			) ?? null
		);
	}

	/**
	 * Whether another device can find it by its address (the machine's
	 * `row.shared`); `null` until the machine has read the store — nothing is
	 * said of it then.
	 */
	shared(address: string): boolean | null {
		return this.row(address)?.shared ?? null;
	}

	/**
	 * §3.5: who can act for `address` on `chainId`, read now unless this
	 * session already has (or is reading) its verdict — also a network it is
	 * not used on yet. A network that does not answer is no verdict: asked
	 * again on the next call. The machine's `check_on`; read the answer with
	 * {@link verdictOn}.
	 */
	checkOn(address: string, chainId: number): void {
		// The sheet's own ask (r17-10: a person's Try again would say `person`).
		this.send({ type: 'check_on', address, chain_id: chainId, person: false });
	}

	/**
	 * This session's verdict for `address` on `chainId` (the machine's row):
	 * `checked` — the findings are current (`[]`: only its members act for
	 * it); `checking` — being read now; neither — not read, or the network did
	 * not answer (no verdict). Reactive.
	 */
	verdictOn(
		address: string,
		chainId: number
	): { checked: boolean; checking: boolean; findings: Finding[] } {
		const row = this.row(address);
		const findings = ((row?.account as MultisigAccount | undefined)?.findings ?? []).filter(
			(f) => f.chainId === chainId
		);
		return {
			checked: row?.checked_on.includes(chainId) ?? false,
			checking: row?.checking_on.includes(chainId) ?? false,
			findings
		};
	}

	/** The active multisig, or `null` when the wallet shows a Vela account. */
	get active(): MultisigAccount | null {
		const view = this.view;
		const address = view ? view.active : this.#snapshot.active;
		return address ? (this.find(address) ?? null) : null;
	}

	/**
	 * The stored list for the first paint, and the machines' host (fetched on
	 * its own: it carries the app's pipelines) — which attaches the machine
	 * here once it has read the store.
	 */
	boot(): void {
		if (this.#booted || typeof window === 'undefined') return;
		this.#booted = true;
		this.#snapshot = snapshot();
		void import('../shell/machines.svelte')
			.then((host) => host.machines.boot())
			.catch((error: unknown) =>
				console.error('[multisig] the accounts machine did not start:', error)
			);
	}

	/** The host hands over the machine once its view is `loaded`: what waited goes now. */
	attach(machine: AccountsMachine | null): void {
		this.#machine = machine;
		if (!machine) return;
		const waiting = this.#queue;
		this.#queue = [];
		for (const event of waiting) machine.dispatch(event);
	}

	/** One event to the machine (held until it is loaded). */
	send(event: MultisigAccountsEvent): void {
		// Sent from an effect, it is no dependency of it: what the machine
		// answers never runs the effect that asked again.
		untrack(() => {
			const machine = this.#machine;
			if (machine) machine.dispatch(event);
			else this.#queue.push(event);
		});
	}

	/**
	 * The first view (from now on) `read` answers something for — what a
	 * sheet waits on after its event (an outcome, a kept account).
	 */
	until<T>(read: (view: MultisigAccountsView) => T | undefined | null): Promise<T> {
		return new Promise((resolve) => {
			let stop: (() => void) | null = null;
			const check = (view: MultisigAccountsView | null) => {
				if (!view) return false;
				const got = read(view);
				if (got === undefined || got === null) return false;
				stop?.();
				resolve(got);
				return true;
			};
			const attachAndWatch = () => {
				const machine = this.#machine;
				if (!machine) return void setTimeout(attachAndWatch, 50);
				if (check(machine.view)) return;
				stop = machine.listen((view) => void check(view));
			};
			attachAndWatch();
		});
	}

	/** `event`, then the first view `read` answers something for. */
	request<T>(
		event: MultisigAccountsEvent,
		read: (view: MultisigAccountsView) => T | undefined | null
	): Promise<T> {
		this.send(event);
		return this.until(read);
	}

	/**
	 * Keep what the last `find` read (the person's tap): in the list,
	 * not shown. Resolves with it as kept — `undefined` if nothing was kept.
	 */
	keepFound(address: string, acceptRisks = false): Promise<MultisigAccount | undefined> {
		const kept = this.request({ type: 'keep_found', accept_risks: acceptRisks }, (view) =>
			view.accounts.some((r) => sameAddress((r.account as MultisigAccount).address, address))
				? this.find(address)
				: undefined
		);
		const late = new Promise<undefined>((resolve) => setTimeout(() => resolve(undefined), KEEP_MS));
		return Promise.race([kept, late]);
	}

	find(address: string): MultisigAccount | undefined {
		return this.accounts.find((a) => sameAddress(a.address, address));
	}

	/**
	 * r4D-8: its invite was handed on from this device (the link copied or
	 * shared, the code shown) — its first steps say so (`row.invite_shared`).
	 */
	inviteShared(address: string): void {
		this.send({ type: 'invite_shared', address });
	}

	/** r4D-8: show (or, `false`, stop showing) its first steps on the home. */
	setFirstRun(address: string, on: boolean): void {
		this.send({ type: 'set_first_run', address, on });
	}

	/** Note that the multisig is used on `chainId` (its proposals are looked for there). */
	useChain(address: string, chainId: number): void {
		this.send({ type: 'apply', address, update: { kind: 'use_chain', chain_id: chainId } });
	}

	/**
	 * What the chain says now about `chainId` (the re-check, §3.5): that
	 * network's findings are replaced by `found`, every other network's kept.
	 */
	setFindingsOn(address: string, chainId: number, found: Finding[]): void {
		this.send({
			type: 'apply',
			address,
			update: { kind: 'findings_on', chain_id: chainId, findings: found }
		});
	}

	remove(address: string): void {
		this.send({ type: 'remove', address });
	}

	/** Show this multisig (or, `null`, the session's own Vela account). */
	activate(address: string | null): void {
		this.send({ type: 'activate', address });
	}
}

export const multisig = new MultisigStore();
