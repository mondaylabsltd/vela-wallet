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

import { networkSpace } from '$lib/network-space/network-space.svelte';
import { CHAINS, chainIdToApiNetwork, chainMeta } from './chains';
import {
	loadCustomNetworks,
	loadNetworkConfigs,
	type CustomNetworkRecord,
	type NetworkConfigRecord
} from './records';
import { allowedOrNull } from './url-policy';
import { onWalletSyncElsewhere } from '$lib/wallet/core/wallet-sync';

/**
 * A network as the services know it. Its logo is not a field: it lives on the
 * endpoint the person chose, and the core says where (`chainLogoURL` in
 * `$lib/flows/marks`); the built-in host spelled out here went stale the
 * moment someone re-pointed it.
 */
export interface Network {
	id: string;
	displayName: string;
	chainId: number;
	iconLabel: string;
	iconColor: string;
	iconBg: string;
	isL2: boolean;
	rpcURL: string;
	explorerURL: string;
	bundlerURL: string;
	/** Its space (spec 105): a testnet's coins have no real value. */
	isTestnet: boolean;
}

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
	isL2: c.isL2,
	rpcURL: c.rpcURL,
	explorerURL: c.explorerURL,
	bundlerURL: `${BUNDLER_BASE}/${c.chainId}`,
	isTestnet: c.testnet === true
}));

const BUILTIN_IDS = new Set(DEFAULT_NETWORKS.map((n) => n.chainId));

/** In-memory cache of custom networks for synchronous lookups. */
let _customNetworkCache: CustomNetworkRecord[] = [];
/** The per-network overrides Settings writes (`vela.networkConfig`) — the
 *  explorer links read the one a person set, built-in network or not. */
let _networkConfigCache: NetworkConfigRecord[] = [];
let _allNetworksSnapshot: Network[] = DEFAULT_NETWORKS;

let _spaceSnapshot: Network[] = [];

function rebuildNetworkSnapshot(): void {
	// A network the person added that is now built in (Arc Testnet, say) is
	// the built-in one: listed, and read, once.
	const custom = _customNetworkCache.filter((n) => !BUILTIN_IDS.has(n.chainId));
	_allNetworksSnapshot =
		custom.length === 0 ? DEFAULT_NETWORKS : [...DEFAULT_NETWORKS, ...custom.map(customToNetwork)];
	rebuildSpaceSnapshot();
}

function rebuildSpaceSnapshot(): void {
	const testnet = networkSpace.testnet;
	_spaceSnapshot = _allNetworksSnapshot.filter((n) => n.isTestnet === testnet);
}
rebuildSpaceSnapshot();

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
	[_customNetworkCache, _networkConfigCache] = await Promise.all([
		loadCustomNetworks(),
		loadNetworkConfigs()
	]);
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
	if (_firstLoad) return _firstLoad;
	_firstLoad = refreshCustomNetworks();
	// A network added or edited in another document (a tab, the side panel)
	// reaches this one: IndexedDB tells nobody, the writer says so.
	onWalletSyncElsewhere((message) => {
		if (message.topic === 'networks') void refreshCustomNetworks().catch(() => {});
	});
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
		isL2: cn.isL2,
		rpcURL: cn.rpcURL,
		explorerURL: cn.explorerURL,
		bundlerURL: cn.bundlerURL,
		// Nobody vouched for it as a mainnet: a testnet (spec 105).
		isTestnet: cn.isTestnet ?? true
	};
}

/**
 * Stable snapshot: default + custom, BOTH spaces. For what is not a person's
 * list — the RPC pool, a lookup by id. What a person sees reads
 * {@link spaceNetworksSync}.
 */
export function getAllNetworksSync(): Network[] {
	return _allNetworksSnapshot;
}

/**
 * The networks of the space in front (spec 105) — what the balances read and
 * the total counts, and what receive, send and the dApps offer.
 */
export function spaceNetworksSync(): Network[] {
	return _spaceSnapshot;
}

/** Whether `chainId` is in the space in front. */
export function inSpace(chainId: number): boolean {
	return _spaceSnapshot.some((n) => n.chainId === chainId);
}

// A switch of space is a change of the network set, for every subscriber.
networkSpace.subscribe(() => {
	rebuildSpaceSnapshot();
	for (const l of _networkListeners) l();
});

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

/**
 * Explorer base (trailing slash stripped), or null for an unknown chain —
 * security surfaces show NO link rather than a misleading one.
 *
 * The explorer a person set in Settings comes first, for a built-in network
 * too: it was saved and then never used, every link still going to the
 * shipped explorer. Either one only when it is an address the wallet may
 * link to ({@link allowedOrNull}) — a `javascript:` explorer stored on a
 * custom network became the `href` of "View on explorer".
 */
/**
 * The person's saved override for `chainId` (`vela.networkConfig`), as last
 * loaded — for a reader that cannot wait: the extension's chain catalog.
 */
export function networkConfigSync(chainId: number): NetworkConfigRecord | null {
	return _networkConfigCache.find((c) => c.chainId === chainId) ?? null;
}

export function explorerBaseURL(chainId: number): string | null {
	const configured = _networkConfigCache.find((c) => c.chainId === chainId)?.explorerURL;
	const url = allowedOrNull(configured) ?? allowedOrNull(networkForChainId(chainId)?.explorerURL);
	return url === null ? null : url.replace(/\/$/, '');
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
