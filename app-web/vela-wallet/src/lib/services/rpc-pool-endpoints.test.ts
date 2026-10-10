/**
 * Which bundler endpoints a chain offers, and in what tier.
 *
 * The pool is the only thing that decides where a user operation lands, so
 * this list is where "the relay I configured in Settings" is either true or
 * quietly untrue.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	collectBundlerUrls,
	collectRpcUrls,
	getLogsRangeCap,
	NEVER_BANNED,
	publicRpcs
} from './rpc-pool-endpoints';
import { DEFAULT_NETWORKS } from './networks';
import { saveServiceEndpoints } from '$lib/onboarding/core/storage';

// The wasm is up (initialised above, as the build-time engine does); a node
// test has no URL to fetch it from again.
vi.mock('$lib/core/client', async (original) => ({
	...(await original<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));
// The chain index: three more endpoints, the way the registry lists them.
vi.mock('./chain-registry', () => ({
	fetchChainInfo: async () => ({
		rpcUrls: ['https://index-one.example', 'https://index-two.example']
	})
}));

const local = new Map<string, string>();
Object.defineProperty(globalThis, 'localStorage', {
	configurable: true,
	value: {
		get length() {
			return local.size;
		},
		key: (i: number) => [...local.keys()][i] ?? null,
		getItem: (k: string) => local.get(k) ?? null,
		setItem: (k: string, v: string) => void local.set(k, v),
		removeItem: (k: string) => void local.delete(k),
		clear: () => local.clear()
	}
});

const config = vi.hoisted(() => ({ value: null as { bundlerURL?: string } | null }));
vi.mock('./records', () => ({
	getNetworkConfig: async () => config.value,
	getRpcProviderKeys: async () => ({})
}));

afterEach(() => {
	local.clear();
	config.value = null;
});

/**
 * The public tier is the core's ONE curated list (`network_admin::PUBLIC_RPCS`,
 * read through the wasm). This shell kept a copy of its own, and the copy had
 * gone stale: every `1rpc.io` endpoint answered HTTP 502 for every call and
 * `bsc.drpc.org` rate-limited most (measured 2026-10-10), so Polygon — whose
 * default IS its first public node — had nothing behind it at all.
 */
describe('the public tier is the core’s curated list', () => {
	const builtin = (chainId: number) => DEFAULT_NETWORKS.find((n) => n.chainId === chainId)!;

	it('Polygon: the dead 1rpc endpoint is gone, a live one stands behind the default', async () => {
		expect(publicRpcs(137)).toEqual([
			'https://polygon-bor-rpc.publicnode.com',
			'https://polygon.gateway.tenderly.co'
		]);
		const collected = await collectRpcUrls(137, NEVER_BANNED);
		expect(collected.map((e) => e.url)).not.toContain('https://1rpc.io/matic');
		expect(collected).toContainEqual({
			url: 'https://polygon.gateway.tenderly.co',
			source: 'public'
		});
	});

	it('sits after the built-in default and before the chain index, in the core’s order', async () => {
		for (const chainId of [1, 56, 137, 42161, 10, 8453, 43114, 100, 196, 42220, 57073]) {
			const network = builtin(chainId);
			const curated = publicRpcs(chainId);
			expect(curated.length, network.displayName).toBeGreaterThan(0);
			const collected = await collectRpcUrls(chainId, NEVER_BANNED);
			// A curated node that IS the default keeps the default's tier.
			const behind = curated.filter((url) => url !== network.rpcURL);
			expect(collected, network.displayName).toEqual([
				{ url: network.rpcURL, source: 'default' },
				...behind.map((url) => ({ url, source: 'public' })),
				{ url: 'https://index-one.example', source: 'builtin' },
				{ url: 'https://index-two.example', source: 'builtin' }
			]);
		}
	});

	it('names no endpoint that had stopped answering, and only https', () => {
		for (const network of DEFAULT_NETWORKS) {
			for (const url of publicRpcs(network.chainId)) {
				expect(url, network.displayName).toMatch(/^https:\/\//);
				expect(url, network.displayName).not.toMatch(/1rpc\.io|bsc\.drpc\.org/);
			}
		}
		// Celo and Ink have a public node now: their defaults time out from
		// some networks, and there was nothing behind them.
		expect(publicRpcs(42220)).toEqual(['https://celo-rpc.publicnode.com']);
		expect(publicRpcs(57073)).toEqual(['https://rpc-qnd.inkonchain.com']);
	});

	it('a chain the core curates nothing for has no public tier — a custom network, say', async () => {
		expect(publicRpcs(31_337)).toEqual([]);
		const collected = await collectRpcUrls(31_337, NEVER_BANNED);
		expect(collected.filter((e) => e.source === 'public')).toEqual([]);
	});

	it('a banned public node is left out, as every tier’s is', async () => {
		const banned = 'https://polygon.gateway.tenderly.co';
		const collected = await collectRpcUrls(137, (url) => url === banned);
		expect(collected.map((e) => e.url)).not.toContain(banned);
	});
});

describe("a built-in chain's bundler", () => {
	it('is the relay configured in Settings › Service nodes', async () => {
		saveServiceEndpoints({ bundlerServiceURL: 'https://my-relay.example' });
		expect(await collectBundlerUrls(100, NEVER_BANNED)).toEqual([
			{ url: 'https://my-relay.example/100', source: 'builtin' }
		]);
	});

	it('is the shipped relay when nothing is configured', async () => {
		expect(await collectBundlerUrls(100, NEVER_BANNED)).toEqual([
			{ url: 'https://vela-relay-cf.getvela.app/100', source: 'builtin' }
		]);
	});

	// The bundler is not editable per built-in network, so a `bundlerURL`
	// stored against one is a snapshot an RPC edit left behind — today's
	// default, or a host that stopped being the default releases ago. Reading
	// it back as a `user` tier outranks the configured relay, and Settings
	// goes on showing a green badge for a URL nothing calls.
	it('ignores a stale override snapshot, whatever host it names', async () => {
		saveServiceEndpoints({ bundlerServiceURL: 'https://my-relay.example' });
		config.value = { bundlerURL: 'https://vela-relay.getvela.app/100' };
		expect(await collectBundlerUrls(100, NEVER_BANNED)).toEqual([
			{ url: 'https://my-relay.example/100', source: 'builtin' }
		]);
	});
});

describe("a custom chain's bundler", () => {
	it('keeps the one the person recorded, above the relay', async () => {
		saveServiceEndpoints({ bundlerServiceURL: 'https://my-relay.example' });
		config.value = { bundlerURL: 'https://my-own-bundler.example/31337' };
		expect(await collectBundlerUrls(31_337, NEVER_BANNED)).toEqual([
			{ url: 'https://my-own-bundler.example/31337', source: 'user' },
			{ url: 'https://my-relay.example/31337', source: 'builtin' }
		]);
	});
});

describe('getLogsRangeCap — a span written as its bounds (issue #443)', () => {
	it('reads the upper bound, not the lower 0', () => {
		expect(
			getLogsRangeCap({ code: -32602, message: 'eth_getLogs is limited to 0 - 50 blocks range' })
		).toBe(50);
		expect(getLogsRangeCap({ code: -32000, message: 'block range limited to 0-2,000' })).toBe(2000);
	});

	it('keeps a stated span first, and halves when there is no upper bound', () => {
		expect(
			getLogsRangeCap({
				code: -32000,
				message: 'block range too large, max 500 (requested 1 - 9000)'
			})
		).toBe(500);
		expect(
			getLogsRangeCap({ code: -32000, message: 'eth_getLogs is limited to 0 blocks range' })
		).toBe(0);
	});
});
