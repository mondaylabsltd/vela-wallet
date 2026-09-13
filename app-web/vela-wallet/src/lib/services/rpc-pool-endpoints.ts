/**
 * Endpoint COLLECTION and the shell-side pure helpers of the RPC pool — WEB.
 *
 * Ported from src/services/rpc-pool-endpoints.ts @ c13e89d4 (spec 025).
 * Deliberately *not* decision-making: building each chain's candidate list
 * from config, probing a single URL, parsing a provider's range-cap wording.
 * Which endpoint to try, when to ban, how long to back off — the core's
 * (`rpc_pool.rs`). NEVER_BANNED is load-bearing on web: the ban map is core
 * state and the core filters bans at SELECTION, not collection (invariant ⑧).
 */

import { fetchChainInfo } from './chain-registry';
import { getBundlerServiceURL } from './endpoints';
import { DEFAULT_NETWORKS, getAllNetworksSync } from './networks';
import { getNetworkConfig, getRpcProviderKeys } from './records';
import { buildProviderRpcUrl, PROVIDER_ORDER } from './rpc-providers';

/** Priority tier (see SOURCE_PRIORITY in the core). */
export type RpcEndpointSource = 'user' | 'provider' | 'builtin' | 'default' | 'public' | 'fallback';

/** One collected candidate, before any stats or scoring exist for it. */
export interface CollectedEndpoint {
	url: string;
	source: RpcEndpointSource;
}

export interface RPCResponse {
	jsonrpc: string;
	id: number;
	result?: unknown;
	error?: { code: number; message: string; data?: unknown };
}

/** Persisted ban map key. One key, one format, every client. */
export const BANNED_STORAGE_KEY = 'vela.rpc.banned';

/** The persisted ban record — `BanEntry`, unchanged on disk since before 017. */
export interface StoredBanEntry {
	url: string;
	bannedAt: number;
	permanent: boolean;
}

/** Built-in bundler base URL (user config, falling back to the default). */
export function getBuiltinBundlerUrl(): string {
	return getBundlerServiceURL();
}

/** Reliable public RPCs per chain (curated, CORS-friendly; Expo table verbatim). */
export const PUBLIC_RPCS: Record<number, string[]> = {
	1: ['https://ethereum-rpc.publicnode.com', 'https://1rpc.io/eth'],
	56: ['https://bsc-rpc.publicnode.com', 'https://bsc.drpc.org', 'https://bsc.meowrpc.com'],
	137: ['https://polygon-bor-rpc.publicnode.com', 'https://1rpc.io/matic'],
	42161: ['https://arbitrum-one-rpc.publicnode.com', 'https://1rpc.io/arb'],
	10: ['https://optimism-rpc.publicnode.com', 'https://1rpc.io/op'],
	8453: ['https://base-rpc.publicnode.com', 'https://1rpc.io/base'],
	43114: ['https://avalanche-c-chain-rpc.publicnode.com', 'https://1rpc.io/avax/c'],
	100: ['https://gnosis-rpc.publicnode.com', 'https://1rpc.io/gnosis'],
	196: ['https://rpc.xlayer.tech', 'https://xlayer.drpc.org']
};

/** Never-banned predicate — on web bans are core state. */
export const NEVER_BANNED = (url: string): boolean => {
	void url;
	return false;
};

export async function collectRpcUrls(
	chainId: number,
	isBanned: (url: string) => boolean
): Promise<CollectedEndpoint[]> {
	const entries: CollectedEndpoint[] = [];
	const seen = new Set<string>();

	const add = (url: string, source: RpcEndpointSource) => {
		if (!url || seen.has(url) || isBanned(url)) return;
		seen.add(url);
		entries.push({ url, source });
	};

	// Chain index — fetched once, reused for primary and deep-fallback tiers.
	let indexRpcs: string[] = [];
	try {
		const info = await fetchChainInfo(chainId);
		indexRpcs = info?.rpcUrls ?? [];
	} catch {
		// no index, no index tiers
	}

	// 1. User-configured per-network override (highest)
	try {
		const config = await getNetworkConfig(chainId);
		const defaultNet = DEFAULT_NETWORKS.find((n) => n.chainId === chainId);
		if (config?.rpcURL && config.rpcURL !== defaultNet?.rpcURL) {
			add(config.rpcURL, 'user');
		}
	} catch {
		// unreadable config contributes nothing
	}

	// 2. Provider keys (Alchemy/dRPC/Ankr), in PROVIDER_ORDER as the cold-start
	//    tiebreak; measured latency takes over once known (core state).
	try {
		const providerKeys = await getRpcProviderKeys();
		for (const id of PROVIDER_ORDER) {
			const key = providerKeys[id];
			if (!key) continue;
			const url = buildProviderRpcUrl(id, chainId, key);
			if (url) add(url, 'provider');
		}
	} catch {
		// no keys, no provider tier
	}

	// 3. Network default — Vela built-in, then a custom network's own RPC.
	const defaultNet = DEFAULT_NETWORKS.find((n) => n.chainId === chainId);
	if (defaultNet?.rpcURL) add(defaultNet.rpcURL, 'default');
	const customNet = getAllNetworksSync().find((n) => n.chainId === chainId);
	if (customNet?.rpcURL) add(customNet.rpcURL, 'default');

	// 4. Public fallback (curated)
	for (const url of PUBLIC_RPCS[chainId] ?? []) add(url, 'public');

	// 5./6. Chain index: first few primary, the rest as deep fallback.
	indexRpcs.slice(0, 5).forEach((url) => add(url, 'builtin'));
	indexRpcs.slice(5, 20).forEach((url) => add(url, 'fallback'));

	return entries;
}

export async function collectBundlerUrls(
	chainId: number,
	isBanned: (url: string) => boolean
): Promise<CollectedEndpoint[]> {
	const entries: CollectedEndpoint[] = [];
	const seen = new Set<string>();
	const defaultChainIds = new Set(DEFAULT_NETWORKS.map((n) => n.chainId));

	const add = (url: string, source: RpcEndpointSource) => {
		if (!url || seen.has(url) || isBanned(url)) return;
		seen.add(url);
		entries.push({ url, source });
	};

	// 1. User-configured override (NetworkConfig editor)
	try {
		const config = await getNetworkConfig(chainId);
		if (config?.bundlerURL) {
			const defaultNet = DEFAULT_NETWORKS.find((n) => n.chainId === chainId);
			if (!defaultNet || config.bundlerURL !== defaultNet.bundlerURL) {
				add(config.bundlerURL, 'user');
			}
		}
	} catch {
		// unreadable config contributes nothing
	}

	// 2. A custom network's own bundlerURL (set during Add Network)
	if (!defaultChainIds.has(chainId)) {
		const net = getAllNetworksSync().find((n) => n.chainId === chainId);
		if (net?.bundlerURL) add(net.bundlerURL, 'user');
	}

	// 3. Built-in vela relay (always the fallback)
	add(`${getBuiltinBundlerUrl()}/${chainId}`, 'builtin');

	return entries;
}

/**
 * Classify an `eth_getLogs` error as a range/size limit rather than an
 * endpoint fault (ported verbatim — see the Expo original for the provider
 * wording census). Stated max block span when present; 0 = halve; null = not
 * a range error.
 */
export function getLogsRangeCap(error: RPCResponse['error']): number | null {
	if (!error?.message) return null;
	const msg = error.message.toLowerCase();

	if (
		msg.includes('result') &&
		(msg.includes('more than') ||
			msg.includes('exceed') ||
			msg.includes('limit') ||
			msg.includes('too many'))
	) {
		return 0;
	}

	const isRangeError =
		msg.includes('block range') ||
		msg.includes('block height') ||
		msg.includes('too many blocks') ||
		msg.includes('range is too') ||
		msg.includes('range too') ||
		msg.includes('range limit') ||
		msg.includes('limited to') ||
		(msg.includes('range') &&
			(msg.includes('exceed') ||
				msg.includes('large') ||
				msg.includes('wide') ||
				msg.includes('maximum')));
	if (!isRangeError) return null;

	const m = msg.match(/(\d[\d,_]*)\s*([km])?/);
	if (m) {
		let n = parseInt(m[1].replace(/[,_]/g, ''), 10);
		if (m[2] === 'k') n *= 1_000;
		else if (m[2] === 'm') n *= 1_000_000;
		if (Number.isFinite(n) && n > 0) return n;
	}
	return 0;
}

export function shorten(url: string): string {
	try {
		const u = new URL(url);
		return u.hostname + u.pathname;
	} catch {
		return url.slice(0, 40);
	}
}
