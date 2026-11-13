/**
 * The catalog the worker reads and switches from is the wallet's own network
 * table — every built-in chain, its node list with the curated public nodes
 * behind the default, and its bundler.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it } from 'vitest';
import { buildExtChainCatalog, WORKER_PUBLIC_RPCS, workerRpcList } from './ext-chains';
import { rpcOwnOnlyPool } from '$lib/core/kernels';
import { publicRpcUrls } from '$lib/core/client';
import { DEFAULT_NETWORKS } from '$lib/services/networks';
import { getBuiltinBundlerUrl } from '$lib/services/rpc-pool-endpoints';
import { saveServiceEndpoints } from '$lib/onboarding/core/storage';
import { networkSpace } from '$lib/network-space/network-space.svelte';

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

	// Spec 105: a new site starts among the space in front's chains — the
	// mainnet space here; the worker still serves both spaces (r10).
	const MAINNETS = DEFAULT_NETWORKS.filter((n) => !n.isTestnet);

	it('lists every built-in chain of BOTH spaces by id, and nothing the wallet does not have', () => {
		// r10: the worker serves every network Vela has — Safari, in the
		// mainnet space, refused a switch to Arc Testnet as "not in Vela's
		// networks". Where a new site starts is the space's (`order`).
		expect(
			Object.keys(catalog.chains)
				.map(Number)
				.sort((a, b) => a - b)
		).toEqual(DEFAULT_NETWORKS.map((n) => n.chainId).sort((a, b) => a - b));
		expect(catalog.chains['5042002']?.name).toBe('Arc Testnet');
		expect(MAINNETS.length).toBeLessThan(DEFAULT_NETWORKS.length);
		expect(catalog.version).toBe(1);
		expect(catalog.updatedAtMs).toBe(1_800_000_000_000);
	});

	// dapp-17: where a new site starts is read from these two.
	it('says its space and its chains in the wallet’s order', () => {
		expect(catalog.space).toBe('mainnet');
		expect(catalog.order).toEqual(MAINNETS.map((n) => n.chainId));
		networkSpace.set('testnet');
		try {
			const testnet = buildExtChainCatalog();
			expect(testnet.space).toBe('testnet');
			expect(testnet.order).toEqual(
				DEFAULT_NETWORKS.filter((n) => n.isTestnet).map((n) => n.chainId)
			);
		} finally {
			networkSpace.set('mainnet');
		}
	});

	it('in the testnet space, starts sites on the testnets and still serves the mainnets', () => {
		networkSpace.set('testnet');
		try {
			const testnet = buildExtChainCatalog();
			const ids = Object.keys(testnet.chains).map(Number);
			expect(ids).toContain(11155111);
			expect(ids).toContain(1);
			expect(testnet.order).toContain(11155111);
			expect(testnet.order).not.toContain(1);
		} finally {
			networkSpace.set('mainnet');
		}
	});

	it('puts the default node first and the public nodes behind it, once each', () => {
		const gnosis = catalog.chains['100'];
		const network = DEFAULT_NETWORKS.find((n) => n.chainId === 100)!;
		expect(gnosis.rpc[0]).toBe(network.rpcURL);
		// The public nodes are the core's curated list, in its order, behind
		// the default — for every built-in chain.
		expect(publicRpcUrls(100).length).toBeGreaterThan(0);
		for (const network of MAINNETS) {
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

/**
 * dapp-5: a dApp's reads follow the person's RPC override as the wallet's own
 * pool does — their node first, the public nodes behind it, and with "Only use
 * my node" on, their node alone (owner decision 2026-10-28).
 */
describe("the person's own node in the catalog (dapp-5)", () => {
	const gnosis = DEFAULT_NETWORKS.find((n) => n.chainId === 100)!;
	const MINE = 'https://my-node.example/gnosis';

	it('puts their RPC first, the shipped default and the public nodes behind it', () => {
		const { rpc, ownOnly } = workerRpcList(gnosis, { rpcURL: MINE });
		expect(rpc[0]).toBe(MINE);
		expect(rpc[1]).toBe(gnosis.rpcURL);
		expect(rpc.slice(2)).toEqual(WORKER_PUBLIC_RPCS[100].filter((u) => u !== gnosis.rpcURL));
		expect(ownOnly).toBe(false);
	});

	it('with "Only use my node", lists their RPC alone', () => {
		expect(workerRpcList(gnosis, { rpcURL: MINE, onlyMine: true })).toEqual({
			rpc: [MINE],
			ownOnly: true
		});
	});

	it('a saved default, or no override, is not their node; the switch alone means nothing', () => {
		const plain = workerRpcList(gnosis, null).rpc;
		expect(plain[0]).toBe(gnosis.rpcURL);
		expect(workerRpcList(gnosis, { rpcURL: gnosis.rpcURL, onlyMine: true }).rpc).toEqual(plain);
		expect(workerRpcList(gnosis, { onlyMine: true }).rpc).toEqual(plain);
	});

	// The catalog is built before the wasm is up, so it mirrors the core's
	// rule; this holds the mirror to it.
	it('is the core’s own-node rule (`own_only_pool`)', () => {
		for (const [config, ownOnly] of [
			[{ rpcURL: MINE }, false],
			[{ rpcURL: MINE, onlyMine: true }, true],
			[null, false],
			[{ rpcURL: gnosis.rpcURL, onlyMine: true }, true]
		] as const) {
			const mirrored = workerRpcList(gnosis, config);
			const seeds = [
				...(config?.rpcURL && config.rpcURL !== gnosis.rpcURL
					? [{ url: config.rpcURL, source: 'user' }]
					: []),
				{ url: gnosis.rpcURL, source: 'default' },
				...WORKER_PUBLIC_RPCS[100]
					.filter((u) => u !== gnosis.rpcURL)
					.map((url) => ({ url, source: 'public' }))
			];
			const core = rpcOwnOnlyPool(seeds, ownOnly);
			expect(mirrored.rpc, JSON.stringify(config)).toEqual(core.endpoints.map((e) => e.url));
			expect(mirrored.ownOnly).toBe(core.own_only);
		}
	});
});
