/**
 * Network configuration model — WEB.
 *
 * Ported (trimmed) from src/models/network.ts @ c13e89d4 (spec 025): the
 * Network shape, the built-in table derived from CHAINS, the custom-network
 * cache with its subscription (Svelte consumers subscribe exactly as the
 * React hook did), and the lookup/explorer helpers the services share. The
 * React-specific hook plumbing stayed behind; the snapshot-stability contract
 * did not change.
 */

import { CHAINS, chainIdToApiNetwork, chainMeta } from './chains';
import { loadCustomNetworks, type CustomNetworkRecord } from './records';

export interface Network {
	id: string;
	displayName: string;
	chainId: number;
	iconLabel: string;
	iconColor: string;
	iconBg: string;
	logoURL: string;
	isL2: boolean;
	rpcURL: string;
	explorerURL: string;
	bundlerURL: string;
}

/** Base URL for chain logos from ethereum-data (content, as on Expo). */
const CHAIN_LOGO_BASE = 'https://ethereum-data.getvela.app/chainlogos';
/** Base URL of Vela's per-chain ERC-4337 bundler. */
const BUNDLER_BASE = 'https://vela-relay-cf.getvela.app';

/** Built-in networks, derived from the canonical CHAINS table. */
export const DEFAULT_NETWORKS: Network[] = CHAINS.map((c) => ({
	id: c.id,
	displayName: c.displayName,
	chainId: c.chainId,
	iconLabel: c.iconLabel,
	iconColor: c.iconColor,
	iconBg: c.iconBg,
	logoURL: `${CHAIN_LOGO_BASE}/eip155-${c.chainId}.png`,
	isL2: c.isL2,
	rpcURL: c.rpcURL,
	explorerURL: c.explorerURL,
	bundlerURL: `${BUNDLER_BASE}/${c.chainId}`
}));

/** In-memory cache of custom networks for synchronous lookups. */
let _customNetworkCache: CustomNetworkRecord[] = [];
let _allNetworksSnapshot: Network[] = DEFAULT_NETWORKS;

function rebuildNetworkSnapshot(): void {
	_allNetworksSnapshot =
		_customNetworkCache.length === 0
			? DEFAULT_NETWORKS
			: [...DEFAULT_NETWORKS, ..._customNetworkCache.map(customToNetwork)];
}

const _networkListeners = new Set<() => void>();

/** Subscribe to network-set changes (custom added/removed, cache reloaded). */
export function subscribeNetworks(listener: () => void): () => void {
	_networkListeners.add(listener);
	return () => {
		_networkListeners.delete(listener);
	};
}

/**
 * Refresh the custom-network cache from storage, then notify subscribers.
 * Call at boot and after the settings editor writes (the network-admin
 * executor does — restoring the invalidation 024 had to leave out).
 */
export async function refreshCustomNetworks(): Promise<void> {
	_customNetworkCache = await loadCustomNetworks();
	rebuildNetworkSnapshot();
	for (const l of _networkListeners) l();
}

let _firstLoad: Promise<void> | null = null;

/**
 * The first read of the stored custom networks, once per document — awaited
 * by whatever reads the snapshot first (the balance boot), kicked off by the
 * root layout for everything else (spec 038 #E9, the founder's second
 * report). Nothing called `refreshCustomNetworks` at boot: the snapshot was
 * the builtin table until a Settings write, so a fresh load of the wallet
 * neither listed nor fetched a network the person had added. The refresh
 * after a write stays as it is; this only guarantees the first one.
 */
export function ensureCustomNetworks(): Promise<void> {
	_firstLoad ??= refreshCustomNetworks();
	return _firstLoad;
}

export function customToNetwork(cn: CustomNetworkRecord): Network {
	return {
		id: cn.id,
		displayName: cn.displayName,
		chainId: cn.chainId,
		iconLabel: cn.iconLabel,
		iconColor: cn.iconColor,
		iconBg: cn.iconBg,
		logoURL: cn.logoURL,
		isL2: cn.isL2,
		rpcURL: cn.rpcURL,
		explorerURL: cn.explorerURL,
		bundlerURL: cn.bundlerURL
	};
}

/** Stable snapshot: default + custom. Populate via refreshCustomNetworks(). */
export function getAllNetworksSync(): Network[] {
	return _allNetworksSnapshot;
}

/** The networks the person added (spec 038 #E9) — the snapshot minus the builtins. */
export function getCustomChainIdsSync(): number[] {
	const builtin = new Set(DEFAULT_NETWORKS.map((n) => n.chainId));
	return _allNetworksSnapshot.map((n) => n.chainId).filter((id) => !builtin.has(id));
}

export async function getAllNetworks(): Promise<Network[]> {
	await refreshCustomNetworks();
	return getAllNetworksSync();
}

export function chainName(chainId: number): string {
	return (
		DEFAULT_NETWORKS.find((n) => n.chainId === chainId)?.displayName ??
		_customNetworkCache.find((n) => n.chainId === chainId)?.displayName ??
		`Chain ${chainId}`
	);
}

export function networkForChainId(chainId: number): Network | null {
	const def = DEFAULT_NETWORKS.find((n) => n.chainId === chainId);
	if (def) return def;
	const custom = _customNetworkCache.find((n) => n.chainId === chainId);
	return custom ? customToNetwork(custom) : null;
}

/** Etherscan as the display fallback when a chain has no configured explorer. */
const FALLBACK_EXPLORER = 'https://etherscan.io';

/** Explorer base (trailing slash stripped), or null for an unknown chain —
 *  security surfaces show NO link rather than a misleading one. */
export function explorerBaseURL(chainId: number): string | null {
	const url = networkForChainId(chainId)?.explorerURL;
	return url ? url.replace(/\/$/, '') : null;
}

export function explorerTxURL(chainId: number, txHash: string): string {
	return `${explorerBaseURL(chainId) ?? FALLBACK_EXPLORER}/tx/${txHash}`;
}

export function explorerAddressURL(chainId: number, address: string): string {
	return `${explorerBaseURL(chainId) ?? FALLBACK_EXPLORER}/address/${address}`;
}

export function nativeSymbol(chainId: number): string {
	const meta = chainMeta(chainId);
	if (meta) return meta.nativeSymbol;
	return _customNetworkCache.find((n) => n.chainId === chainId)?.nativeSymbol ?? 'ETH';
}

export function networkId(chainId: number): string {
	return chainIdToApiNetwork(chainId);
}
