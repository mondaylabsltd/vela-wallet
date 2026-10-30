/**
 * Tests: the core's accounts machine (the real wasm), attached to the store's
 * facade over an in-memory store and a world where nothing else answers —
 * what a screen's test seeds instead of writing `multisig.accounts`. The
 * wasm must be initialized first (`$lib/i18n/wasm-init.server` in a server
 * test, `loadCore()` in a browser one).
 */
import { MultisigAccountsCore } from '$lib/core/client';
import { multisig } from '../core/store.svelte';
import type { MultisigAccountsEvent } from '../generated/MultisigAccountsEvent';
import type { MultisigAccountsView } from '../generated/MultisigAccountsView';
import type { MultisigLocalAccount } from '../generated/MultisigLocalAccount';
import type { MultisigShellDeps } from './handler';
import { MultisigMachine } from './machine.svelte';

export function memoryStorage(seed: Record<string, string> = {}) {
	const map = new Map(Object.entries(seed));
	return {
		map,
		getItem: (k: string) => map.get(k) ?? null,
		setItem: (k: string, v: string) => void map.set(k, v),
		removeItem: (k: string) => void map.delete(k)
	};
}

/** A shell whose service and chains never answer, over `storage`. */
export function offlineDeps(
	storage: ReturnType<typeof memoryStorage>,
	over: Partial<MultisigShellDeps> = {}
): MultisigShellDeps {
	return {
		fetch: (async () => {
			throw new TypeError('offline');
		}) as unknown as typeof fetch,
		rpc: async () => {
			throw new Error('offline');
		},
		storage: () => storage,
		now: () => Date.now(),
		random: (n) => new Uint8Array(n).fill(7),
		passkey: async () => ({ type: 'passkey_cancelled' }),
		askRisk: async () => false,
		sendCalls: async () => ({ type: 'calls_cancelled' }),
		quoteCalls: async () => ({ type: 'quote_failed', detail: 'offline' }),
		simulate: async () => null,
		recipientFacts: async () => ({ isContract: null, firstTime: null }),
		userOpStatus: async () => null,
		peer: {
			open: async () => {
				throw new Error('no rooms here');
			},
			send: async () => {},
			receive: async () => ({ closed: 'closed' }),
			close: async () => {}
		},
		wallet: async () => {
			throw { code: 4900, message: 'no wallet' };
		},
		notice: () => {},
		...over
	};
}

export type AccountsMachineForTests = MultisigMachine<MultisigAccountsView, MultisigAccountsEvent>;

/**
 * The machine, started over `seed` (the stored keys) and attached to the
 * facade once it has read them.
 */
export async function startAccounts({
	seed = {},
	deps = {},
	networks = [100],
	accounts = []
}: {
	seed?: Record<string, string>;
	deps?: Partial<MultisigShellDeps>;
	networks?: number[];
	accounts?: MultisigLocalAccount[];
} = {}): Promise<{ machine: AccountsMachineForTests; storage: ReturnType<typeof memoryStorage> }> {
	const storage = memoryStorage(seed);
	const machine: AccountsMachineForTests = new MultisigMachine(
		new MultisigAccountsCore(),
		offlineDeps(storage, deps),
		'accounts'
	);
	machine.dispatch({ type: 'start', service_url: 'http://service.test', accounts, networks });
	await new Promise<void>((resolve) => {
		if (machine.view.loaded) return resolve();
		const stop = machine.listen((view) => {
			if (!view.loaded) return;
			stop();
			resolve();
		});
	});
	multisig.attach(machine);
	return { machine, storage };
}

/** The facade back to no machine, and the machine gone. */
export function stopAccounts(machine: AccountsMachineForTests | null | undefined): void {
	multisig.attach(null);
	machine?.dispose();
}

/** Settles the machine's own work (its store writes answer on the next turns). */
export async function settled(): Promise<void> {
	for (let i = 0; i < 5; i += 1) await new Promise((r) => setTimeout(r, 0));
}
