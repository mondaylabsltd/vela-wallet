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
