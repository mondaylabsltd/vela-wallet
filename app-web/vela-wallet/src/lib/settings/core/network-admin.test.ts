/**
 * The resident `network_admin` session (spec 072): a screen event raised
 * before the stores are read waits for them, over the real core.
 *
 * The wide layout's nav is on screen the moment the page is: a person can
 * open the RPC providers panel before the core has read a thing. That opening
 * was dropped (no loop yet) or answered over an empty ledger (no keys yet),
 * so the drafts were never seeded — and tabbing out of the key field the
 * panel showed then saved an empty key over the saved one.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
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

import { setItem } from '$lib/services/storage';
import { networkAdmin } from './network-admin.svelte';

beforeEach(() => {
	// Every key test probes the provider's endpoints; none of them answers here.
	vi.stubGlobal(
		'fetch',
		vi.fn(() => Promise.reject(new Error('offline')))
	);
});

describe('an event raised before the ledger is read', () => {
	it('waits for it: an early "providers opened" still seeds the saved key (P0)', async () => {
		kv.set('vela.rpcProviders', JSON.stringify({ alchemy: 'saved-key' }));

		networkAdmin.dispatch({ type: 'providers_opened' });
		await networkAdmin.boot();
		await vi.waitFor(() => expect(networkAdmin.view.loaded).toBe(true));
		expect(networkAdmin.view.providers.find((p) => p.provider === 'alchemy')?.key).toBe(
			'saved-key'
		);

		// Focus the field the panel shows, type nothing, leave it.
		networkAdmin.dispatch({ type: 'provider_key_blurred', provider: 'alchemy' });
		await vi.waitFor(() =>
			expect(setItem).toHaveBeenCalledWith('vela.rpcProviders', expect.any(String))
		);
		expect(JSON.parse(kv.get('vela.rpcProviders') as string)).toEqual({ alchemy: 'saved-key' });
	});
});

describe('the Vela networks shown on the person’s word (owner decision 2026-10-29)', () => {
	it('lists the core’s rows in the web table’s order, and shows one again in one step', async () => {
		const { CHAINS } = await import('$lib/services/chains');
		const { spaceNetworksSync } = await import('$lib/services/networks');
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
