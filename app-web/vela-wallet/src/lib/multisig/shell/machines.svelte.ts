/**
 * The app-wide host of the core's long-lived multisig machines — one each,
 * over the one shell (`handler.ts`, the app's pipelines from `live.ts`).
 *
 * It boots after `loadCore()`, feeds the machines the app's facts (this
 * device's Vela accounts as `MultisigLocalAccount`, its networks and the
 * space's, the service in use), and routes the `Notice`s one machine sends
 * to another. It decides nothing: what a fact means is the machines'.
 *
 * The accounts machine (`multisig_accounts`) is here now, handed to the
 * store's facade (`core/store.svelte.ts`) once it has read the store. The
 * proposals and members machines plug in the same way: one
 * {@link MultisigMachine} each, booted beside it, their notices routed by
 * {@link MultisigMachines.route}.
 *
 * perf-3: the transport a machine's rooms run on is `lazyPeerRooms` — the
 * PeerConnect client is fetched on the first room, never by a signed-in page.
 */
import { loadCore, MultisigAccountsCore } from '$lib/core/client';
import type { JsonWasmCore } from '$lib/core/json-shell';
import { SERVICE_ENDPOINTS_CHANGED, STORAGE_KEYS } from '$lib/onboarding/core/storage';
import { getAllNetworksSync, spaceNetworksSync, subscribeNetworks } from '$lib/services/networks';
import { genesisServiceURL } from '../core/service';
import { ACTIVE_MULTISIG_KEY, MULTISIG_ACCOUNTS_KEY, multisig } from '../core/store.svelte';
import type { MultisigAccountsEvent } from '../generated/MultisigAccountsEvent';
import type { MultisigAccountsView } from '../generated/MultisigAccountsView';
import type { MultisigLocalAccount } from '../generated/MultisigLocalAccount';
import type { MultisigNotice } from '../generated/MultisigNotice';
import type { MultisigShellDeps } from './handler';
import { liveDeps } from './live';
import { localAccounts } from './local-accounts';
import { MultisigMachine } from './machine.svelte';
import { lazyPeerRooms } from './peer-lazy';

/** How often a record that has not landed is asked about again while a page is open. */
export const RECORD_RETRY_MS = 60_000;

/** The proofs a creation keeps until its record lands (another tab may change them). */
const PENDING_PROOFS_KEY = 'vela.multisigPendingProofs';

const chainIds = (networks: { chainId: number }[]) => networks.map((n) => n.chainId);

export interface BootOptions {
	/** The shell's pipelines (a test's simulated world); the app's by default. */
	deps?: MultisigShellDeps;
	/** The wasm, loaded (a test's `initSync`); `loadCore` by default. */
	load?: () => Promise<void>;
	/** The accounts core (a test's); a fresh `MultisigAccountsCore` by default. */
	accountsCore?: () => JsonWasmCore;
	/** The facts the machines start from (a test's); the app's by default. */
	facts?: () => {
		serviceUrl: string;
		accounts: MultisigLocalAccount[];
		networks: number[];
		space: number[];
	};
}

class MultisigMachines {
	/** The accounts machine, once booted. */
	accounts = $state.raw<MultisigMachine<MultisigAccountsView, MultisigAccountsEvent> | null>(null);
	#booted: Promise<void> | null = null;
	#facts: NonNullable<BootOptions['facts']> = () => ({
		serviceUrl: genesisServiceURL(),
		accounts: localAccounts(),
		networks: chainIds(getAllNetworksSync()),
		space: chainIds(spaceNetworksSync())
	});
	#stops: (() => void)[] = [];
	#checkedRecords = false;

	/** Idempotent: the first caller boots, every later one shares it. */
	boot(options: BootOptions = {}): Promise<void> {
		this.#booted ??= this.#start(options);
		return this.#booted;
	}

	async #start(options: BootOptions): Promise<void> {
		await (options.load ?? loadCore)();
		if (options.facts) this.#facts = options.facts;
		const deps: MultisigShellDeps = {
			...(options.deps ?? liveDeps({ peer: lazyPeerRooms(() => {}), notice: () => {} })),
			notice: (notice) => this.route(notice)
		};
		const accounts = new MultisigMachine<MultisigAccountsView, MultisigAccountsEvent>(
			options.accountsCore?.() ?? new MultisigAccountsCore(),
			deps,
			'accounts'
		);
		this.accounts = accounts;
		const facts = this.#facts();
		accounts.dispatch({
			type: 'start',
			service_url: facts.serviceUrl,
			accounts: facts.accounts,
			networks: facts.networks
		});
		accounts.dispatch({ type: 'space', chain_ids: facts.space });
		await new Promise<void>((resolve) => {
			if (accounts.view.loaded) return resolve();
			const stop = accounts.listen((view) => {
				if (!view.loaded) return;
				stop();
				resolve();
			});
		});
		multisig.attach(accounts);
		if (!options.facts) this.#follow();
	}

	/** The app's facts as they move: another tab, a network added, another service. */
	#follow(): void {
		if (typeof window === 'undefined') return;
		const onstorage = (event: StorageEvent) => {
			const key = event.key;
			if (
				key === null ||
				key === MULTISIG_ACCOUNTS_KEY ||
				key === ACTIVE_MULTISIG_KEY ||
				key === PENDING_PROOFS_KEY
			) {
				this.send({ type: 'reload' });
			}
			if (key === null || key === STORAGE_KEYS.accounts) this.refreshLocal();
			if (key === null || key === STORAGE_KEYS.serviceEndpoints) this.#service();
		};
		const onservice = () => this.#service();
		window.addEventListener('storage', onstorage);
		window.addEventListener(SERVICE_ENDPOINTS_CHANGED, onservice);
		const stopNetworks = subscribeNetworks(() => {
			const facts = this.#facts();
			this.send({ type: 'networks', networks: facts.networks });
			this.send({ type: 'space', chain_ids: facts.space });
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

	/** An event for the accounts machine (held by the facade until it is loaded). */
	send(event: MultisigAccountsEvent): void {
		multisig.send(event);
	}

	/** This device's Vela accounts, read again (one was added, or signed in). */
	refreshLocal(): void {
		this.send({ type: 'local_accounts', accounts: this.#facts().accounts });
	}

	/**
	 * Keep asking about every multisig whose record has not landed — now,
	 * when the browser comes back online, when the page is shown again, and
	 * every {@link RECORD_RETRY_MS} — while a page that shows multisigs is
	 * up; and, once a session, what each shared one's record is now (its
	 * name; whether the service still has it). One retry at a time is the
	 * machine's (`tick`). Returns the stop.
	 */
	followRecords(): () => void {
		const tick = () => this.send({ type: 'tick' });
		const shown = () => {
			if (document.visibilityState === 'visible') tick();
		};
		tick();
		if (!this.#checkedRecords) {
			this.#checkedRecords = true;
			this.send({ type: 'check_records' });
		}
		const timer = setInterval(tick, RECORD_RETRY_MS);
		window.addEventListener('online', tick);
		window.addEventListener('focus', tick);
		document.addEventListener('visibilitychange', shown);
		return () => {
			clearInterval(timer);
			window.removeEventListener('online', tick);
			window.removeEventListener('focus', tick);
			document.removeEventListener('visibilitychange', shown);
		};
	}

	/** Where a machine's notice goes. */
	route(notice: MultisigNotice): void {
		switch (notice.kind) {
			case 'account':
				this.send({ type: 'apply', address: notice.address, update: notice.update });
				return;
			case 'activity_rows':
				// TODO(W8 proposals surface): the executed rows go to the activity once
				// the proposals machine drives the web (`recordExecution` writes them now).
				return;
			case 'mailbox_took':
				// TODO(W8 proposals surface): the shown multisig's list is read again
				// by the proposals machine; `proposals.svelte.ts` reads its own now.
				return;
			case 'activated':
				// TODO(W8 proposals surface): the proposals machine follows the multisig
				// shown; `MultisigHost` follows `multisig.active` until then.
				return;
		}
	}

	/** Tests: forget the machines, as a new document would. */
	resetForTests(): void {
		for (const stop of this.#stops.splice(0)) stop();
		this.accounts?.dispose();
		this.accounts = null;
		this.#booted = null;
		this.#checkedRecords = false;
		multisig.attach(null);
	}
}

export const machines = new MultisigMachines();
