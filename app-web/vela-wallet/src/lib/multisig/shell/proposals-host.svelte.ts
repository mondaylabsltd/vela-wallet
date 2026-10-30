/**
 * The app-wide host of the core's proposals machine (`multisig_proposals`,
 * `MultisigProposalsCore`) — one machine, over the one shell (`handler.ts`,
 * the app's pipelines from `live.ts`). Kept apart from the accounts host
 * (`machines.svelte.ts`) so it joins it as one more machine: the same boot
 * after `loadCore()`, the same facts, the same notice routing.
 *
 * It boots after `loadCore()`, feeds the machine the app's facts (this
 * device's Vela accounts as `MultisigLocalAccount`, its networks, the
 * service in use, every multisig on this device), drives the reads the
 * machine paces (`others_tick` every {@link OTHERS_EVERY_MS} while a page is
 * visible, and when it is shown again) and routes the `Notice`s it sends:
 * a multisig's record to the store, executed rows to the activity, a mailbox
 * that took something to whoever shows that multisig. It decides nothing.
 *
 * What it drives now: what waits on every multisig NOT shown (`presence`,
 * `others_waiting`). The shown multisig's list and its actions are still
 * `core/proposals.svelte.ts`' — the machine is not told to follow one, so it
 * reads, keeps and writes nothing of it (one owner per stored key).
 *
 * perf-3: the transport its rooms would run on is `lazyPeerRooms` — the
 * PeerConnect client is fetched on the first room, never by a signed-in page.
 */
import { loadCore, MultisigProposalsCore } from '$lib/core/client';
import type { JsonWasmCore } from '$lib/core/json-shell';
import { SERVICE_ENDPOINTS_CHANGED, STORAGE_KEYS } from '$lib/onboarding/core/storage';
import { getAllNetworksSync, subscribeNetworks } from '$lib/services/networks';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { genesisServiceURL } from '../core/service';
import { proposals } from '../core/proposals.svelte';
import { multisig, type MultisigAccount } from '../core/store.svelte';
import type { Finding } from '../core/refusal';
import type { MultisigNotice } from '../generated/MultisigNotice';
import type { MultisigProposalsEvent } from '../generated/MultisigProposalsEvent';
import type { MultisigProposalsView } from '../generated/MultisigProposalsView';
import type { MultisigLocalAccount } from '../generated/MultisigLocalAccount';
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
	/** Every multisig on this device, and the one shown (a test's); the store's by default. */
	multisigs?: () => { accounts: MultisigAccount[]; shown: string | null };
	/** Where an executed row goes (a test's); the transaction store by default. */
	saveRows?: (rows: LocalTransaction[]) => Promise<void>;
}

const chainIds = (networks: { chainId: number }[]) => networks.map((n) => n.chainId);

const same = (a: string | null | undefined, b: string | null | undefined) =>
	!!a && !!b && a.toLowerCase() === b.toLowerCase();

class ProposalsHost {
	/** The machine, once booted. */
	machine = $state.raw<ProposalsMachine | null>(null);
	/** A mailbox took something for this multisig: whoever shows it reads it again. */
	onmailbox: ((safe: string) => void) | null = null;
	/** Executed rows were written to the activity: whoever shows it refreshes. */
	onrecorded: (() => void) | null = null;
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
			known: []
		});
		this.#others();
		const stop = $effect.root(() => {
			$effect(() => this.#others());
		});
		this.#stops.push(stop);
		if (!options.facts) this.#follow();
	}

	/**
	 * Every multisig on this device but the one shown (`proposals.followed`):
	 * what waits on each is the machine's to read.
	 */
	#others(): void {
		const { accounts, shown } = this.#options.multisigs?.() ?? {
			accounts: multisig.accounts,
			shown: proposals.followed
		};
		const others = accounts.filter((a) => !same(a.address, shown));
		this.send({ type: 'accounts', accounts: $state.snapshot(others) as never });
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
					case 'recorded_name':
						multisig.setRecordedName(address, update.name);
						return;
					case 'name_members':
						multisig.nameMembers(address, update.members as never, { typed: update.typed });
						return;
				}
				return;
			}
			case 'activity_rows': {
				if (notice.rows.length === 0) return;
				const save =
					this.#options.saveRows ??
					(async (rows: LocalTransaction[]) =>
						(await import('$lib/services/records')).saveTransactions(rows));
				await save(notice.rows as LocalTransaction[]);
				this.onrecorded?.();
				return;
			}
			case 'mailbox_took':
				this.onmailbox?.(notice.safe);
				return;
			case 'activated':
				// The accounts machine's (`machines.svelte.ts`): the store activates it.
				return;
			case 'open_proposal':
			case 'outside_sign':
				// Sent once the machine follows the shown multisig and opens its
				// links (not yet: `core/linked.svelte.ts`, `external/signing.svelte.ts`).
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
	}
}

export const proposalsHost = new ProposalsHost();
