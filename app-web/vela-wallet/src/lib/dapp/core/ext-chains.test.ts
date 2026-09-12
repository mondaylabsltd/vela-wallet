/**
 * The catalog the worker reads and switches from is the wallet's own network
 * table — every built-in chain, its node list with the curated public nodes
 * behind the default, and its bundler.
 */
import { describe, expect, it } from 'vitest';
import { buildExtChainCatalog } from './ext-chains';
import { DEFAULT_NETWORKS } from '$lib/services/networks';
import { PUBLIC_RPCS } from '$lib/services/rpc-pool-endpoints';

describe('the published network catalog', () => {
	const catalog = buildExtChainCatalog(1_800_000_000_000);

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
});
