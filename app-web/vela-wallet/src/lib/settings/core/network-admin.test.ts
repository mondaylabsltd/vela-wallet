/**
 * The resident `network_admin` session, over the real core.
 *
 * - Spec 072: a screen event raised before the stores are read waits for
 *   them — the wide layout's nav is on screen the moment the page is, and an
 *   opening answered over an empty ledger seeded nothing.
 * - Owner decision 2026-10-29: RPC providers are gone. A wallet that saved
 *   keys keeps the RPC each network was using: its first load writes them as
 *   per-network RPCs, then deletes the keys — and a write that fails keeps
 *   them, for the next load. The keys here are obviously fake.
 */

import { beforeEach, describe, expect, it, vi } from 'vitest';

const kv = new Map<string, string>();
/** The next `vela.networkConfig` write fails (quota, a sealed store). */
const failing = { networkConfig: false };
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => {
		if (failing.networkConfig && key === 'vela.networkConfig') throw new Error('quota');
		kv.set(key, value);
	}),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
vi.mock('$lib/onboarding/core/storage', () => ({
	loadServiceEndpoints: vi.fn(() => ({})),
	saveServiceEndpoints: vi.fn()
}));
vi.mock('$lib/services/chain-registry', () => ({
	loadSearchIndex: vi.fn(async () => []),
	fetchRawChainData: vi.fn(async () => null)
}));

const FAKE_KEY = 'fake-alchemy-key';
const ALCHEMY_ETH = `https://eth-mainnet.g.alchemy.com/v2/${FAKE_KEY}`;

// A loaded full run (both projects at once, `pnpm test:unit`) overran the
// default 5 s here: a first test pays for compiling its module graph (the
// real core, where one loads), not its own time (the house pattern,
// bridge.test.ts).
vi.setConfig({ testTimeout: 30_000 });

beforeEach(() => {
	kv.clear();
	failing.networkConfig = false;
	vi.resetModules();
	// The cards' probes ask endpoints; none of them answers here.
	vi.stubGlobal(
		'fetch',
		vi.fn(() => Promise.reject(new Error('offline')))
	);
});

/** A fresh resident — the module is one session per page — over a fresh core. */
async function resident() {
	await import('$lib/i18n/wasm-init.server');
	return (await import('./network-admin.svelte')).networkAdmin;
}

const storedConfigs = () =>
	JSON.parse(kv.get('vela.networkConfig') ?? '[]') as { chainId: number; rpcURL?: string }[];

describe('an event raised before the ledger is read', () => {
	it('waits for it, and is answered over what was read', async () => {
		kv.set(
			'vela.networkConfig',
			JSON.stringify([{ chainId: 1, rpcURL: 'https://my-node.example' }])
		);
		const networkAdmin = await resident();
		networkAdmin.dispatch({ type: 'override_expanded', chain_id: 1 });
		await networkAdmin.boot();
		await vi.waitFor(() => expect(networkAdmin.view.loaded).toBe(true));
		const row = networkAdmin.view.networks.find((r) => r.chain_id === 1);
		// The card opened over the saved RPC, not the shipped one.
		expect(row?.rpc_url).toBe('https://my-node.example');
		expect(row?.rpc_health).not.toBeNull();
	});
});

describe('saved RPC provider keys, retired', () => {
	it('become the RPC of every network they served, then are deleted', async () => {
		kv.set('vela.rpcProviders', JSON.stringify({ alchemy: FAKE_KEY }));
		// Ethereum has a node of its own: it never asked the provider first.
		kv.set(
			'vela.networkConfig',
			JSON.stringify([{ chainId: 1, rpcURL: 'https://my-node.example' }])
		);
		const networkAdmin = await resident();
		await networkAdmin.boot();
		await vi.waitFor(() => expect(kv.has('vela.rpcProviders')).toBe(false));

		const configs = storedConfigs();
		expect(configs.find((c) => c.chainId === 1)?.rpcURL).toBe('https://my-node.example');
		expect(configs.find((c) => c.chainId === 8453)?.rpcURL).toBe(
			`https://base-mainnet.g.alchemy.com/v2/${FAKE_KEY}`
		);
		const base = networkAdmin.view.networks.find((r) => r.chain_id === 8453);
		expect(base?.rpc_url).toBe(`https://base-mainnet.g.alchemy.com/v2/${FAKE_KEY}`);
		// §41 (P2-8): its RPC still, but a third party's keyed endpoint is not
		// the person's own node — "Only use my node" may not trust it alone.
		expect(base?.own_rpc).toBe(false);
	});

	it('stay when the write fails, and the next load tries again', async () => {
		kv.set('vela.rpcProviders', JSON.stringify({ alchemy: FAKE_KEY }));
		failing.networkConfig = true;
		const first = await resident();
		await first.boot();
		await vi.waitFor(() => expect(first.view.loaded).toBe(true));
		// Nothing written, nothing deleted, nothing shown that is not stored.
		await new Promise((resolve) => setTimeout(resolve, 50));
		expect(kv.has('vela.rpcProviders')).toBe(true);
		expect(kv.has('vela.networkConfig')).toBe(false);
		expect(first.view.networks.find((r) => r.chain_id === 1)?.own_rpc).toBe(false);

		failing.networkConfig = false;
		vi.resetModules();
		const next = await resident();
		await next.boot();
		await vi.waitFor(() => expect(kv.has('vela.rpcProviders')).toBe(false));
		expect(storedConfigs().find((c) => c.chainId === 1)?.rpcURL).toBe(ALCHEMY_ETH);
	});
});

describe('the Vela networks shown on the person’s word (owner decision 2026-10-29)', () => {
	it('lists the core’s rows in the web table’s order, and shows one again in one step', async () => {
		const { CHAINS } = await import('$lib/services/chains');
		const { spaceNetworksSync } = await import('$lib/services/networks');
		const networkAdmin = await resident();
		await networkAdmin.boot();
		await vi.waitFor(() => expect(networkAdmin.view.loaded).toBe(true));
		// This device had no record: settled on the twenty, before the ledger read it.
		const four = CHAINS.filter((c) => c.shownByDefault === false).map((c) => c.chainId);
		expect(networkAdmin.view.hidden_vela_chain_ids).toEqual(four);
		const builtins = networkAdmin.view.networks.filter((row) => !row.is_custom);
		// The single source of order: the core's rows and the web table agree.
		expect(builtins.map((row) => row.chain_id)).toEqual(
			CHAINS.filter((c) => c.shownByDefault !== false).map((c) => c.chainId)
		);
		expect(spaceNetworksSync().map((n) => n.chainId)).toEqual(
			builtins.filter((row) => !row.is_testnet).map((row) => row.chain_id)
		);

		// Settings → Add network → Plume: a Vela network, nothing to check.
		networkAdmin.dispatch({ type: 'chain_selected', chain_id: 98866, keep_custom_rpc: false });
		await vi.waitFor(() => expect(networkAdmin.view.wizard.vela_network).toBe(true));
		expect(networkAdmin.view.wizard.can_add).toBe(true);
		networkAdmin.dispatch({ type: 'add_confirmed', now_iso: '2026-10-29T12:00:00.000Z' });
		await vi.waitFor(() =>
			expect(JSON.parse(kv.get('vela.shownNetworks') as string)).toEqual({
				version: 1,
				shown: [98866]
			})
		);
		await vi.waitFor(() => expect(spaceNetworksSync().at(-1)?.chainId).toBe(98866));
		expect(kv.get('vela.customNetworks')).toBeUndefined();
		const plume = networkAdmin.view.networks.find((row) => row.chain_id === 98866);
		expect(plume?.optional).toBe(true);

		// …and out again: the list loses it, nothing else is touched.
		networkAdmin.dispatch({ type: 'delete_confirmed', id: 'plume' });
		await vi.waitFor(() =>
			expect(spaceNetworksSync().some((n) => n.chainId === 98866)).toBe(false)
		);
		expect(JSON.parse(kv.get('vela.shownNetworks') as string).shown).toEqual([]);
	});
});
