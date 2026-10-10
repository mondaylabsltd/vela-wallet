/**
 * The network catalog the service worker reads (spec 027, reads and switching).
 *
 * The worker cannot run the core and does not carry a chain table of its own:
 * which chains exist — the built-in twelve plus whatever custom networks a
 * person added in Settings — and where their nodes and bundlers are, is the
 * wallet's knowledge. So the wallet publishes it, under `vela.ext.chains`, and
 * the worker only reads: a `wallet_switchEthereumChain` to a chain outside
 * this catalog is 4902, and a read goes to the endpoints listed here, in order.
 *
 * Facts, not judgements: this is the same table `getAllNetworksSync()` hands
 * every screen, plus the curated public nodes the RPC pool already collects.
 * Nothing is decided here; the pool's ban and latency state stays where it is
 * (core state, on the wallet's side of the boundary).
 */
import { DEFAULT_NETWORKS, getAllNetworksSync, nativeSymbol } from '$lib/services/networks';
import { getBuiltinBundlerUrl } from '$lib/services/rpc-pool-endpoints';
import { CHAINS_KEY } from '../keys';

/** One chain, as the worker needs it. */
export interface ExtChainEntry {
	chainId: number;
	name: string;
	symbol: string;
	/** Node endpoints, first choice first. */
	rpc: string[];
	/** The ERC-4337 bundler for the chain. */
	bundler: string;
	explorer: string;
}

export interface ExtChainCatalog {
	version: 1;
	chains: Record<string, ExtChainEntry>;
	updatedAtMs: number;
}

/**
 * The curated public nodes behind each built-in chain's default — a COPY of
 * the core's one list (`network_admin::PUBLIC_RPCS`), and the only copy the
 * web keeps.
 *
 * Why a copy: this catalog is built synchronously the moment the wallet
 * mounts, before the wasm is up, and what it is published for — the service
 * worker — cannot run the core at all. Waiting for the core here moved the
 * publish behind the wallet's whole boot, and the extension's request
 * surfaces are timed against it being there first. The pool
 * (`rpc-pool-endpoints.ts`) and the registry reads ask the core itself.
 *
 * It cannot drift unseen: `ext-chains.test.ts` holds it equal to the core's
 * list (through the wasm) for every built-in chain, and to having no chain the
 * core does not curate. A change to the core's list fails that test until
 * this follows.
 */
export const WORKER_PUBLIC_RPCS: Readonly<Record<number, readonly string[]>> = {
	1: ['https://ethereum-rpc.publicnode.com', 'https://mainnet.gateway.tenderly.co'],
	56: ['https://bsc-rpc.publicnode.com', 'https://bsc-dataseed1.bnbchain.org'],
	137: ['https://polygon-bor-rpc.publicnode.com', 'https://polygon.gateway.tenderly.co'],
	42161: ['https://arbitrum-one-rpc.publicnode.com', 'https://arbitrum.gateway.tenderly.co'],
	10: ['https://optimism-rpc.publicnode.com', 'https://optimism.gateway.tenderly.co'],
	8453: ['https://base-rpc.publicnode.com', 'https://base.gateway.tenderly.co'],
	43114: ['https://avalanche-c-chain-rpc.publicnode.com', 'https://avalanche.gateway.tenderly.co'],
	100: ['https://gnosis-rpc.publicnode.com', 'https://gnosis.gateway.tenderly.co'],
	196: ['https://rpc.xlayer.tech', 'https://xlayer.drpc.org'],
	42220: ['https://celo-rpc.publicnode.com'],
	57073: ['https://rpc-qnd.inkonchain.com']
};

/** The catalog as it is now — pure, so it can be asserted without storage. */
export function buildExtChainCatalog(nowMs = Date.now()): ExtChainCatalog {
	const chains: Record<string, ExtChainEntry> = {};
	// A built-in chain's bundler is the CONFIGURED relay (Settings › Service
	// nodes › Vela Relay), not the constant baked into `DEFAULT_NETWORKS`:
	// `collectBundlerUrls` has always read it that way, and a catalog that
	// disagrees sends every dApp call to a relay the wallet itself stopped
	// using. A custom network keeps its own bundler, which is the one the
	// person typed when they added it.
	const builtinChainIds = new Set(DEFAULT_NETWORKS.map((n) => n.chainId));
	for (const network of getAllNetworksSync()) {
		const rpc: string[] = [];
		const add = (url: string | undefined) => {
			if (url && !rpc.includes(url)) rpc.push(url);
		};
		add(network.rpcURL);
		for (const url of WORKER_PUBLIC_RPCS[network.chainId] ?? []) add(url);
		chains[String(network.chainId)] = {
			chainId: network.chainId,
			name: network.displayName,
			symbol: nativeSymbol(network.chainId),
			rpc,
			bundler: builtinChainIds.has(network.chainId)
				? `${getBuiltinBundlerUrl()}/${network.chainId}`
				: network.bundlerURL,
			explorer: network.explorerURL
		};
	}
	return { version: 1, chains, updatedAtMs: nowMs };
}

interface StorageAreaLike {
	set(items: Record<string, unknown>): Promise<void>;
}

function area(): StorageAreaLike | null {
	const local = (globalThis as { chrome?: { storage?: { local?: unknown } } }).chrome?.storage
		?.local;
	return (local as StorageAreaLike | undefined) ?? null;
}

/** Store the catalog for the worker. Best-effort, like the snapshot. */
export async function publishExtChains(): Promise<ExtChainCatalog | null> {
	const store = area();
	if (!store) return null;
	const catalog = buildExtChainCatalog();
	try {
		await store.set({ [CHAINS_KEY]: catalog });
		return catalog;
	} catch {
		return null;
	}
}
