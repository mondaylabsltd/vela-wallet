/**
 * Chain registry client — WEB.
 *
 * Ported (trimmed) from src/services/chain-registry.ts @ e78afdfa (spec 024):
 * the two RAW fetches the `network_admin` executor supplies to the core. All
 * parsing DECISIONS — defaults, the HTTPS filter, the key-placeholder
 * rejection, the search ranking — are core rules; this module only carries
 * documents.
 */

import { readChainDoc } from './chain-doc';
import { getEthereumDataURL } from './endpoints';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';

/**
 * The unparsed `/chains/eip155-{id}.json` body, or null on any failure —
 * a `null` body reaches the core and lands in the same not-found rule the
 * TS parser's `catch` always fed.
 */
export async function fetchRawChainData(chainId: number): Promise<unknown> {
	// The same document the balance read uses, fetched once for both
	// (`chain-doc.ts`, perf-7).
	const doc = await readChainDoc(chainId);
	return doc.kind === 'doc' ? doc.body : null;
}

/** The chain search index: 30-minute cache, stale-on-failure fallback. */
const SEARCH_CACHE_TTL = 30 * 60 * 1000;
let _searchCache: unknown[] | null = null;
let _searchCacheTime = 0;

export async function loadSearchIndex(): Promise<unknown[]> {
	const now = Date.now();
	if (_searchCache && now - _searchCacheTime < SEARCH_CACHE_TTL) {
		return _searchCache;
	}
	try {
		const res = await fetchWithTimeout(
			`${getEthereumDataURL()}/index/fuse-chains.json`,
			{},
			{ timeoutMs: NET_TIMEOUTS.ethereumData }
		);
		if (!res.ok) return _searchCache ?? [];
		const json = (await res.json()) as { data?: unknown };
		_searchCache = Array.isArray(json.data) ? json.data : [];
		_searchCacheTime = now;
		return _searchCache;
	} catch {
		return _searchCache ?? [];
	}
}

/** Parsed chain metadata the endpoint collector needs (Expo `ChainInfo`, trimmed). */
export interface ChainInfo {
	chainId: number;
	name: string;
	rpcUrls: string[];
}

/** Placeholder-key URLs never make usable candidates (Expo parse rule). */
const KEY_PLACEHOLDER = /\$\{|\{[A-Z_]+\}|YOUR[-_]?API[-_]?KEY/i;

/**
 * `/chains/eip155-{id}.json`, parsed just far enough for endpoint collection:
 * clean HTTPS rpc URLs, placeholders rejected. The full parsing DECISIONS for
 * the add-network wizard stay in the core (024); this trimmed view exists for
 * `collectRpcUrls`' index tiers, exactly as the Expo `fetchChainInfo` did.
 */
export async function fetchChainInfo(chainId: number): Promise<ChainInfo | null> {
	const data = (await fetchRawChainData(chainId)) as {
		chainId?: unknown;
		name?: unknown;
		rpc?: unknown;
	} | null;
	if (!data || typeof data.chainId !== 'number') return null;
	const rpc = Array.isArray(data.rpc) ? data.rpc : [];
	const rpcUrls = rpc.filter(
		(u): u is string =>
			typeof u === 'string' && u.startsWith('https://') && !KEY_PLACEHOLDER.test(u)
	);
	return {
		chainId: data.chainId,
		name: typeof data.name === 'string' ? data.name : '',
		rpcUrls
	};
}
