/**
 * The app-wide host of the multisig machines, and the store's facade over
 * the accounts machine (the real wasm, an in-memory store):
 *
 * - the facade reads the stored list through the machine, changes go to it
 *   as events, and one asked for before the machine is up waits for it;
 * - a notice from another machine (`account`) reaches the accounts machine;
 * - a record that has not landed is asked about again at once, online, on
 *   focus, when the page is shown again and on an interval — the machine's
 *   `tick` (one retry at a time is the machine's: Rust,
 *   `created_while_the_service_is_down_is_kept_and_recorded_later`).
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MultisigAccountsEvent } from '../generated/MultisigAccountsEvent';
import {
	memoryStorage,
	offlineDeps,
	settled,
	startAccounts,
	stopAccounts
} from './accounts.testing';

const SAFE = '0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83';
const OTHER = '0x00000000000000000000000000000000000000ee';
const row = (address: string, recorded = true) => ({
	address,
	genesis: { factory: '0x01', singleton: '0x02', saltNonce: '0x1', initializer: '0x' },
	owners: ['0x00000000000000000000000000000000000000aa'],
	threshold: 1,
	recorded,
	chainIds: [100],
	addedMs: 1
});

// The host's follow: the page's own events, stood in for.
const page = new EventTarget();
const doc = Object.assign(new EventTarget(), { visibilityState: 'visible' });
vi.stubGlobal('window', page);
vi.stubGlobal('document', doc);

const { multisig } = await import('../core/store.svelte');
const { machines } = await import('./machines.svelte');

afterEach(() => {
	machines.resetForTests();
	vi.useRealTimers();
});

describe('the store’s facade over the accounts machine', () => {
	let machine: Awaited<ReturnType<typeof startAccounts>>['machine'] | null = null;
	beforeEach(() => {
		machine = null;
	});
	afterEach(() => stopAccounts(machine));

	it('reads the stored list through the machine; a change is the machine’s, written by it', async () => {
		const started = await startAccounts({
			seed: {
				'vela.multisigAccounts': JSON.stringify([row(SAFE), row(OTHER)]),
				'vela.activeMultisig': SAFE
			}
		});
		machine = started.machine;
		expect(multisig.accounts.map((a) => a.address)).toEqual([SAFE, OTHER]);
		expect(multisig.active?.address).toBe(SAFE);
		multisig.useChain(SAFE, 8453);
		expect(multisig.find(SAFE)?.chainIds).toEqual([100, 8453]);
		multisig.activate(OTHER);
		expect(multisig.active?.address).toBe(OTHER);
		multisig.setFirstRun(OTHER, true);
		multisig.remove(SAFE);
		await settled();
		const stored = JSON.parse(started.storage.map.get('vela.multisigAccounts')!);
		expect(stored.map((r: { address: string }) => r.address)).toEqual([OTHER]);
		expect(stored[0].firstRun).toBe(true);
		expect(started.storage.map.get('vela.activeMultisig')).toBe(OTHER);
		expect(multisig.row(OTHER)?.shared).toBe(true);
	});

	it('a change asked for before the machine is up waits for it, and is never lost', async () => {
		multisig.setLabel(SAFE, 'Ops');
		multisig.activate(SAFE);
		expect(multisig.active).toBeNull();
		const started = await startAccounts({
			seed: { 'vela.multisigAccounts': JSON.stringify([row(SAFE)]) }
		});
		machine = started.machine;
		expect(multisig.find(SAFE)?.label).toBe('Ops');
		expect(multisig.active?.address).toBe(SAFE);
	});
});

describe('the machines’ host', () => {
	async function boot(seed: Record<string, string>) {
		const storage = memoryStorage(seed);
		const seen: MultisigAccountsEvent['type'][] = [];
		await machines.boot({
			load: async () => {},
			deps: offlineDeps(storage),
			facts: () => ({
				serviceUrl: 'http://service.test',
				accounts: [],
				networks: [100],
				space: [100]
			})
		});
		const dispatch = machines.accounts!.dispatch.bind(machines.accounts!);
		machines.accounts!.dispatch = (event: MultisigAccountsEvent) => {
			seen.push(event.type);
			dispatch(event);
		};
		return { storage, seen };
	}

	it('routes another machine’s change to a stored multisig to the accounts machine', async () => {
		const { storage } = await boot({ 'vela.multisigAccounts': JSON.stringify([row(SAFE)]) });
		machines.route({
			kind: 'account',
			address: SAFE,
			update: { kind: 'use_chain', chain_id: 42161 }
		});
		await settled();
		expect(multisig.find(SAFE)?.chainIds).toEqual([100, 42161]);
		expect(JSON.parse(storage.map.get('vela.multisigAccounts')!)[0].chainIds).toEqual([100, 42161]);
	});

	it('asks again about a record that has not landed: at once, online, focus, shown, and on an interval', async () => {
		const { seen } = await boot({ 'vela.multisigAccounts': JSON.stringify([row(SAFE, false)]) });
		vi.useFakeTimers();
		const stop = machines.followRecords();
		const ticks = () => seen.filter((t) => t === 'tick').length;
		expect(ticks()).toBe(1);
		expect(seen).toContain('check_records');
		page.dispatchEvent(new Event('online'));
		page.dispatchEvent(new Event('focus'));
		doc.dispatchEvent(new Event('visibilitychange'));
		expect(ticks()).toBe(4);
		vi.advanceTimersByTime(60_000);
		expect(ticks()).toBe(5);
		stop();
		page.dispatchEvent(new Event('online'));
		vi.advanceTimersByTime(60_000);
		expect(ticks()).toBe(5);
		// Its records are looked at once a session, not once a page.
		machines.followRecords()();
		expect(seen.filter((t) => t === 'check_records')).toHaveLength(1);
	});

	/**
	 * ms-22 (a regression): a proposal link waiting for a network this device
	 * lacks is the accounts machine's record — a new document (a reload, the
	 * Add network wizard) still waits for it, and opens it once the network
	 * is here (Rust: `a_link_waiting_for_its_network_survives_a_reload`).
	 */
	it('keeps a link waiting for its network through a reload, and opens it once the network is here', async () => {
		const storage = memoryStorage({});
		const start = (networks: number[]) =>
			machines.boot({
				load: async () => {},
				deps: offlineDeps(storage),
				facts: () => ({
					serviceUrl: 'http://service.test',
					accounts: [],
					networks,
					space: networks
				})
			});
		await start([100]);
		multisig.send({
			type: 'open_link',
			hash: `#ms=8453.${SAFE}.0x${'ab'.repeat(32)}`,
			take_kept: false
		});
		await settled();
		expect(machines.accounts!.view.link?.kind).toBe('missingNetwork');
		expect(storage.map.has('vela.multisigPendingLink')).toBe(true);
		// A new document over the same store: still waiting, nothing said.
		machines.resetForTests();
		await start([100]);
		await settled();
		expect(machines.accounts!.view.link).toBeNull();
		expect(storage.map.has('vela.multisigPendingLink')).toBe(true);
		// The network is added: the link opens, and is waited for no more.
		multisig.send({ type: 'networks', networks: [100, 8453] });
		await vi.waitFor(() => expect(storage.map.has('vela.multisigPendingLink')).toBe(false));
		expect(machines.accounts!.view.link?.kind).not.toBe('missingNetwork');
	});
});
