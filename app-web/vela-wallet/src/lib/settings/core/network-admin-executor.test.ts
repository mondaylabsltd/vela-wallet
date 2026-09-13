/**
 * The web network_admin executor (spec 024 T018): one op ↔ one call, stored
 * bytes stay Expo-compatible, the failure twin answers every operation.
 * Transports are not exercised here — the probe bodies are ports and the
 * fetch they ride is mocked shut; what is pinned is shape translation.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { NetEffect } from './network-admin-types';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));

const endpointsStore: { record: Record<string, string> } = { record: {} };
vi.mock('$lib/onboarding/core/storage', () => ({
	loadServiceEndpoints: vi.fn(() => endpointsStore.record),
	saveServiceEndpoints: vi.fn((record: Record<string, string>) => {
		endpointsStore.record = record;
	})
}));

const indexRows: { rows: unknown[] } = { rows: [] };
vi.mock('$lib/services/chain-registry', () => ({
	loadSearchIndex: vi.fn(async () => indexRows.rows),
	fetchRawChainData: vi.fn(async () => null)
}));

import {
	executeNetworkAdminOperation,
	networkAdminOperationFailure
} from './network-admin-executor';
import { saveServiceEndpoints } from '$lib/onboarding/core/storage';

const effect = (operation: NetEffect['operation']): NetEffect => ({ id: 1, operation });

const STORED_CUSTOM = {
	id: 'xlayer',
	displayName: 'X Layer',
	chainId: 196,
	iconLabel: 'X',
	iconColor: '#fff',
	iconBg: '#000',
	logoURL: '',
	isL2: true,
	rpcURL: 'https://rpc.xlayer.tech',
	explorerURL: 'https://www.oklink.com/xlayer',
	bundlerURL: '',
	nativeSymbol: 'OKB',
	addedAt: '2026-01-01T00:00:00.000Z'
};

beforeEach(() => {
	kv.clear();
	endpointsStore.record = {};
	vi.clearAllMocks();
});

describe('read_store', () => {
	it('translates the stored camelCase records to the wire', async () => {
		kv.set('vela.customNetworks', JSON.stringify([STORED_CUSTOM]));
		kv.set('vela.networkConfig', JSON.stringify([{ chainId: 1, rpcURL: 'https://rpc.example' }]));
		kv.set('vela.rpcProviders', JSON.stringify({ alchemy: 'key-a' }));
		endpointsStore.record = { passkeyIndexURL: 'https://idx.example' };

		const result = await executeNetworkAdminOperation(effect({ type: 'read_store' }));
		expect(result.type).toBe('store_loaded');
		if (result.type !== 'store_loaded') return;
		expect(result.custom_networks[0]).toMatchObject({
			id: 'xlayer',
			display_name: 'X Layer',
			chain_id: 196,
			rpc_url: 'https://rpc.xlayer.tech',
			native_symbol: 'OKB',
			added_at_iso: '2026-01-01T00:00:00.000Z'
		});
		// Missing stored fields coerce, never reject (a rejected StoreLoaded
		// would strand the core unloaded forever).
		expect(result.network_configs[0]).toEqual({
			chain_id: 1,
			rpc_url: 'https://rpc.example',
			explorer_url: '',
			bundler_url: ''
		});
		expect(result.provider_keys).toEqual({ alchemy: 'key-a', drpc: null, ankr: null });
		// Absent endpoint fields stay absent (null) — the core applies defaults.
		expect(result.endpoints).toEqual({
			ethereum_data_url: null,
			passkey_index_url: 'https://idx.example',
			bundler_service_url: null,
			fiat_rates_url: null
		});
	});

	it('reads junk as nothing-configured, still loaded', async () => {
		kv.set('vela.customNetworks', '{not json');
		const result = await executeNetworkAdminOperation(effect({ type: 'read_store' }));
		expect(result.type).toBe('store_loaded');
		if (result.type === 'store_loaded') expect(result.custom_networks).toEqual([]);
	});
});

describe('writes', () => {
	it('write_custom_networks stores the Expo camelCase shape, byte-compatible', async () => {
		const wire = {
			id: 'xlayer',
			display_name: 'X Layer',
			chain_id: 196,
			icon_label: 'X',
			icon_color: '#fff',
			icon_bg: '#000',
			logo_url: '',
			is_l2: true,
			rpc_url: 'https://rpc.xlayer.tech',
			explorer_url: 'https://www.oklink.com/xlayer',
			bundler_url: '',
			native_symbol: 'OKB',
			added_at_iso: '2026-01-01T00:00:00.000Z'
		};
		const result = await executeNetworkAdminOperation(
			effect({ type: 'write_custom_networks', networks: [wire] })
		);
		expect(result).toEqual({ type: 'written' });
		expect(JSON.parse(kv.get('vela.customNetworks')!)).toEqual([STORED_CUSTOM]);
	});

	it('write_service_endpoints goes through the onboarding module (D3a)', async () => {
		await executeNetworkAdminOperation(
			effect({
				type: 'write_service_endpoints',
				endpoints: {
					ethereum_data_url: 'https://data.example',
					passkey_index_url: 'https://idx.example',
					bundler_service_url: 'https://relay.example',
					fiat_rates_url: 'https://fx.example'
				}
			})
		);
		expect(saveServiceEndpoints).toHaveBeenCalledWith({
			ethereumDataURL: 'https://data.example',
			passkeyIndexURL: 'https://idx.example',
			bundlerServiceURL: 'https://relay.example',
			fiatRatesURL: 'https://fx.example'
		});
	});

	it('write_rpc_providers drops cleared keys instead of storing null', async () => {
		await executeNetworkAdminOperation(
			effect({ type: 'write_rpc_providers', keys: { alchemy: 'k', drpc: null, ankr: null } })
		);
		expect(JSON.parse(kv.get('vela.rpcProviders')!)).toEqual({ alchemy: 'k' });
	});
});

describe('platform-absent caches are acknowledged, never skipped', () => {
	it('invalidate_pools answers', async () => {
		expect(
			await executeNetworkAdminOperation(effect({ type: 'invalidate_pools', chain_id: null }))
		).toEqual({ type: 'invalidated' });
	});
	it('clear_bundler_cache answers', async () => {
		expect(
			await executeNetworkAdminOperation(effect({ type: 'clear_bundler_cache', chain_id: 1 }))
		).toEqual({ type: 'bundler_cache_cleared' });
	});
});

describe('the failure twin', () => {
	it('read_store failure reads as nothing-configured, still loaded', () => {
		const result = networkAdminOperationFailure(effect({ type: 'read_store' }));
		expect(result).toMatchObject({ type: 'store_loaded', custom_networks: [] });
	});
	it('probe_rpc failure answers the probed shape with an unknown chain id', () => {
		const result = networkAdminOperationFailure(
			effect({ type: 'probe_rpc', url: 'https://rpc.example' })
		);
		expect(result).toEqual({
			type: 'probed',
			url: 'https://rpc.example',
			reported_chain_id: null,
			latency_ms: 0
		});
	});
	it('fetch_service_health failure lands in the failed body', () => {
		const result = networkAdminOperationFailure(
			effect({ type: 'fetch_service_health', field: 'passkey_index', base_url: 'https://x' })
		);
		expect(result).toMatchObject({ type: 'service_health', body: { type: 'failed' } });
	});
});

describe('fetch_search_index', () => {
	it('drops a row whose chain id the core cannot represent instead of poisoning the whole index', async () => {
		indexRows.rows = [
			{
				chainId: 1,
				name: 'Ethereum',
				shortName: 'eth',
				nativeCurrencySymbol: 'ETH',
				hasLogo: true
			},
			// The live index's first over-u32 id: one row, and serde used to refuse ALL of them.
			{ chainId: 7078815900, name: 'Mezo Matsnet', shortName: 'mezo', nativeCurrencySymbol: 'BTC' },
			{ chainId: '100', name: 'Gnosis', shortName: 'gno', nativeCurrencySymbol: 'XDAI' },
			{ chainId: -5, name: 'Nope' },
			{ chainId: 1.5, name: 'Nope' },
			{ chainId: 'abc', name: 'Nope' }
		];
		const result = await executeNetworkAdminOperation(effect({ type: 'fetch_search_index' }));
		expect(result.type).toBe('search_index');
		if (result.type !== 'search_index') throw new Error('unreachable');
		expect(result.chains.map((c) => c.chain_id)).toEqual([1, 100]);
		expect(result.chains[0]).toEqual({
			chain_id: 1,
			name: 'Ethereum',
			short_name: 'eth',
			native_currency_symbol: 'ETH',
			has_logo: true
		});
	});
});
