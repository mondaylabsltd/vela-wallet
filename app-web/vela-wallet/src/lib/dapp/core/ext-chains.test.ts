/**
 * The catalog the worker reads and switches from is the wallet's own network
 * table — every built-in chain, its node list with the curated public nodes
 * behind the default, and its bundler.
 */
import { afterEach, describe, expect, it } from 'vitest';
import { buildExtChainCatalog } from './ext-chains';
import { DEFAULT_NETWORKS } from '$lib/services/networks';
import { getBuiltinBundlerUrl, PUBLIC_RPCS } from '$lib/services/rpc-pool-endpoints';
import { saveServiceEndpoints } from '$lib/onboarding/core/storage';

// The stored endpoints live in local storage, which node does not have.
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

describe('the published network catalog', () => {
	const catalog = buildExtChainCatalog(1_800_000_000_000);

	afterEach(() => local.clear());

	it('lists every built-in chain by id, and nothing the wallet does not have', () => {
		expect(
			Object.keys(catalog.chains)
				.map(Number)
				.sort((a, b) => a - b)
		).toEqual(DEFAULT_NETWORKS.map((n) => n.chainId).sort((a, b) => a - b));
		expect(catalog.version).toBe(1);
		expect(catalog.updatedAtMs).toBe(1_800_000_000_000);
	});

	it('puts the default node first and the public nodes behind it, once each', () => {
		const gnosis = catalog.chains['100'];
		const network = DEFAULT_NETWORKS.find((n) => n.chainId === 100)!;
		expect(gnosis.rpc[0]).toBe(network.rpcURL);
		for (const url of PUBLIC_RPCS[100]) expect(gnosis.rpc).toContain(url);
		expect(new Set(gnosis.rpc).size).toBe(gnosis.rpc.length);
		expect(gnosis.bundler).toBe(network.bundlerURL);
		expect(gnosis.symbol).toBe('xDAI');
		expect(gnosis.name).toBe(network.displayName);
	});

	// The catalog is the only thing the worker has: a dApp's bundler call goes
	// where this says, so a chain pinned to the shipped relay would keep every
	// dApp on a host the wallet itself had been told to stop using.
	it("follows Settings' Vela Relay for a built-in chain's bundler", () => {
		saveServiceEndpoints({ bundlerServiceURL: 'https://my-relay.example' });
		expect(getBuiltinBundlerUrl()).toBe('https://my-relay.example');
		expect(buildExtChainCatalog(1_800_000_000_000).chains['100'].bundler).toBe(
			'https://my-relay.example/100'
		);
	});
});
