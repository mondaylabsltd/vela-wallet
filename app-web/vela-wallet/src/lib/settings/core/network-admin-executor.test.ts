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
import { useWalletSyncChannel } from '$lib/wallet/core/wallet-sync';

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
		endpointsStore.record = {
			passkeyIndexURL: 'https://idx.example',
			genesisServiceURL: 'http://127.0.0.1:8788'
		};

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
			bundler_url: '',
			// "Only use my node": absent, off.
			only_mine: false
		});
		expect(result.provider_keys).toEqual({ alchemy: 'key-a', drpc: null, ankr: null });
		// Absent endpoint fields stay absent (null) — the core applies defaults.
		expect(result.endpoints).toEqual({
			ethereum_data_url: null,
			passkey_index_url: 'https://idx.example',
			bundler_service_url: null,
			fiat_rates_url: null,
			genesis_service_url: 'http://127.0.0.1:8788'
		});
		// The web keeps multisig accounts: their service is one of its endpoints.
		expect(result.multisig_service).toBe(true);
	});

	// set-2: `[null]`, a record with no chain, a chain id serde cannot read as
	// a u32 — each would have refused the whole load. They are dropped; the
	// good record beside them still loads.
	it('drops records that name no chain the core can read, keeps the rest', async () => {
		kv.set(
			'vela.customNetworks',
			JSON.stringify([
				null,
				{},
				{ ...STORED_CUSTOM, chainId: -1 },
				{ ...STORED_CUSTOM, chainId: 1.5 },
				{ ...STORED_CUSTOM, chainId: 2 ** 40 },
				'Linea',
				{ chainId: 59144 },
				STORED_CUSTOM
			])
		);
		kv.set('vela.networkConfig', JSON.stringify([null, { chainId: 'x' }, { chainId: 1 }]));
		const result = await executeNetworkAdminOperation(effect({ type: 'read_store' }));
		if (result.type !== 'store_loaded') throw new Error(result.type);
		expect(result.custom_networks.map((n) => [n.id, n.chain_id])).toEqual([
			['custom-59144', 59144],
			['xlayer', 196]
		]);
		expect(result.network_configs.map((c) => c.chain_id)).toEqual([1]);
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

	it('a network write is said to the other documents (a tab, the side panel)', async () => {
		const posted: unknown[] = [];
		useWalletSyncChannel({
			postMessage: (m) => posted.push(m),
			addEventListener: () => {},
			removeEventListener: () => {}
		});
		try {
			await executeNetworkAdminOperation(effect({ type: 'write_custom_networks', networks: [] }));
			await executeNetworkAdminOperation(effect({ type: 'write_network_configs', configs: [] }));
			expect(posted).toEqual([{ topic: 'networks' }, { topic: 'networks' }]);
		} finally {
			useWalletSyncChannel(undefined);
		}
	});

	it('write_service_endpoints goes through the onboarding module (D3a)', async () => {
		await executeNetworkAdminOperation(
			effect({
				type: 'write_service_endpoints',
				endpoints: {
					ethereum_data_url: 'https://data.example',
					passkey_index_url: 'https://idx.example',
					bundler_service_url: 'https://relay.example',
					fiat_rates_url: 'https://fx.example',
					genesis_service_url: 'http://127.0.0.1:8788'
				}
			})
		);
		expect(saveServiceEndpoints).toHaveBeenCalledWith({
			ethereumDataURL: 'https://data.example',
			passkeyIndexURL: 'https://idx.example',
			bundlerServiceURL: 'https://relay.example',
			fiatRatesURL: 'https://fx.example',
			genesisServiceURL: 'http://127.0.0.1:8788'
		});
	});
});

describe('retire_provider_keys (owner decision 2026-10-29)', () => {
	const config = {
		chain_id: 1,
		rpc_url: 'https://eth-mainnet.g.alchemy.com/v2/fake-key',
		explorer_url: 'https://etherscan.io',
		bundler_url: '',
		only_mine: false
	};

	it('writes the configs, then deletes the keys, and says so', async () => {
		kv.set('vela.rpcProviders', JSON.stringify({ alchemy: 'fake-key' }));
		const result = await executeNetworkAdminOperation(
			effect({ type: 'retire_provider_keys', configs: [config] })
		);
		expect(result).toEqual({
			type: 'provider_keys_retired',
			configs_written: true,
			keys_removed: true
		});
		expect(kv.has('vela.rpcProviders')).toBe(false);
		expect(JSON.parse(kv.get('vela.networkConfig')!)).toEqual([
			{
				chainId: 1,
				rpcURL: 'https://eth-mainnet.g.alchemy.com/v2/fake-key',
				explorerURL: 'https://etherscan.io',
				bundlerURL: ''
			}
		]);
	});

	it('with nothing to write, only deletes the keys', async () => {
		kv.set('vela.rpcProviders', JSON.stringify({ ankr: 'fake-key' }));
		const result = await executeNetworkAdminOperation(
			effect({ type: 'retire_provider_keys', configs: null })
		);
		expect(result).toMatchObject({ configs_written: true, keys_removed: true });
		expect(kv.has('vela.networkConfig')).toBe(false);
	});

	it('keeps the keys when the configs could not be written', async () => {
		const { setItem } = await import('$lib/services/storage');
		vi.mocked(setItem).mockRejectedValueOnce(new Error('quota'));
		kv.set('vela.rpcProviders', JSON.stringify({ alchemy: 'fake-key' }));
		const result = await executeNetworkAdminOperation(
			effect({ type: 'retire_provider_keys', configs: [config] })
		);
		expect(result).toEqual({
			type: 'provider_keys_retired',
			configs_written: false,
			keys_removed: false
		});
		expect(kv.has('vela.rpcProviders')).toBe(true);
	});

	it('says the keys are still there when only their delete failed', async () => {
		const { removeItem } = await import('$lib/services/storage');
		vi.mocked(removeItem).mockRejectedValueOnce(new Error('blocked'));
		const result = await executeNetworkAdminOperation(
			effect({ type: 'retire_provider_keys', configs: [config] })
		);
		expect(result).toMatchObject({ configs_written: true, keys_removed: false });
	});

	it('its failure twin claims nothing was done', () => {
		expect(
			networkAdminOperationFailure(effect({ type: 'retire_provider_keys', configs: [config] }))
		).toEqual({ type: 'provider_keys_retired', configs_written: false, keys_removed: false });
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
			has_logo: true,
			vela: false
		});
	});
});

describe('fetch_chain_info', () => {
	it('hands the core the document’s slip44, whose meaning is the core’s (set-5)', async () => {
		const { fetchRawChainData } = await import('$lib/services/chain-registry');
		const doc = (extra: Record<string, unknown>) => ({
			chainId: 10200,
			name: 'Gnosis Chiado Testnet',
			nativeCurrency: { name: 'Chiado xDAI', symbol: 'XDAI', decimals: 18 },
			rpc: ['https://rpc.chiadochain.net'],
			explorers: [{ url: 'https://blockscout.chiadochain.net' }],
			...extra
		});
		const read = async (extra: Record<string, unknown>) => {
			vi.mocked(fetchRawChainData).mockResolvedValueOnce(doc(extra));
			const result = await executeNetworkAdminOperation(
				effect({ type: 'fetch_chain_info', chain_id: 10200 })
			);
			if (result.type !== 'chain_info') throw new Error('unreachable');
			return result.data;
		};
		expect((await read({ slip44: 1 }))?.slip44).toBe(1);
		expect(await read({})).not.toHaveProperty('slip44');
		expect(await read({ slip44: 'one' })).not.toHaveProperty('slip44');
		expect(await read({ slip44: -1 })).not.toHaveProperty('slip44');
	});
});
