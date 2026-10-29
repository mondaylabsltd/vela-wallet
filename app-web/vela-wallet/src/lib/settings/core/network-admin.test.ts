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
		expect(base?.own_rpc).toBe(true);
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
