/**
 * The catalog the worker reads and switches from is the wallet's own network
 * table — every built-in chain, its node list with the curated public nodes
 * behind the default, and its bundler.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it } from 'vitest';
import { buildExtChainCatalog, WORKER_PUBLIC_RPCS } from './ext-chains';
import { publicRpcUrls } from '$lib/core/client';
import { DEFAULT_NETWORKS } from '$lib/services/networks';
import { getBuiltinBundlerUrl } from '$lib/services/rpc-pool-endpoints';
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
		// The public nodes are the core's curated list, in its order, behind
		// the default — for every built-in chain.
		expect(publicRpcUrls(100).length).toBeGreaterThan(0);
		for (const network of DEFAULT_NETWORKS) {
			const listed = catalog.chains[String(network.chainId)].rpc;
			const curated = publicRpcUrls(network.chainId).filter((url) => url !== network.rpcURL);
			expect(listed, network.displayName).toEqual([network.rpcURL, ...curated]);
		}
		expect(new Set(gnosis.rpc).size).toBe(gnosis.rpc.length);
		// The endpoints that had stopped answering are gone from what the
		// worker is told to call.
		expect(JSON.stringify(catalog)).not.toMatch(/1rpc\.io|bsc\.drpc\.org/);
		expect(gnosis.bundler).toBe(network.bundlerURL);
		expect(gnosis.symbol).toBe('xDAI');
		expect(gnosis.name).toBe(network.displayName);
	});

	// The catalog is built before the wasm is up (and for a worker that cannot
	// run the core), so it keeps a COPY of the core's curated list — the one
	// copy on the web. This is what stops the copy drifting: it is the core's
	// list, chain for chain and in order, and names no chain the core does not.
	it('its copy of the curated public nodes IS the core’s list', () => {
		for (const network of DEFAULT_NETWORKS) {
			expect(WORKER_PUBLIC_RPCS[network.chainId] ?? [], network.displayName).toEqual(
				publicRpcUrls(network.chainId)
			);
		}
		for (const [chainId, urls] of Object.entries(WORKER_PUBLIC_RPCS)) {
			expect(urls, chainId).toEqual(publicRpcUrls(Number(chainId)));
			expect(urls.length, chainId).toBeGreaterThan(0);
		}
		// Not vacuous: the core curates for eleven chains today.
		expect(Object.keys(WORKER_PUBLIC_RPCS)).toHaveLength(11);
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
