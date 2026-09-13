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
import { getAllNetworksSync, nativeSymbol } from '$lib/services/networks';
import { PUBLIC_RPCS } from '$lib/services/rpc-pool-endpoints';
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

/** The catalog as it is now — pure, so it can be asserted without storage. */
export function buildExtChainCatalog(nowMs = Date.now()): ExtChainCatalog {
	const chains: Record<string, ExtChainEntry> = {};
	for (const network of getAllNetworksSync()) {
		const rpc: string[] = [];
		const add = (url: string | undefined) => {
			if (url && !rpc.includes(url)) rpc.push(url);
		};
		add(network.rpcURL);
		for (const url of PUBLIC_RPCS[network.chainId] ?? []) add(url);
		chains[String(network.chainId)] = {
			chainId: network.chainId,
			name: network.displayName,
			symbol: nativeSymbol(network.chainId),
			rpc,
			bundler: network.bundlerURL,
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
