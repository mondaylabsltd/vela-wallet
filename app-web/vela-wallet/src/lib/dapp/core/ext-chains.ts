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
import {
	DEFAULT_NETWORKS,
	ensureShownNetworks,
	explorerBaseURL,
	getAllNetworksSync,
	isHiddenNetwork,
	networkConfigSync,
	spaceNetworksSync,
	nativeSymbol
} from '$lib/services/networks';
import { getBuiltinBundlerUrl } from '$lib/services/rpc-pool-endpoints';
import { networkSpace } from '$lib/network-space/network-space.svelte';
import { ACCOUNT_CHAINS_KEY, CHAINS_KEY } from '../keys';

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
	/**
	 * Every network the worker can serve, BOTH spaces (r10: a wallet in the
	 * mainnet space refused a dApp's switch to Arc Testnet — "Chain 5042002 is
	 * not in Vela's networks" — because only the space in front was here).
	 * A network the person hid stays out: adding it shows it again (spec 100).
	 */
	chains: Record<string, ExtChainEntry>;
	/**
	 * dapp-17: the space in front (spec 105) and ITS networks in the wallet's
	 * list order — where a new site starts (`network_space::site_start_chain_for`,
	 * the worker's `startChainOf`).
	 */
	space: 'mainnet' | 'testnet';
	order: number[];
	updatedAtMs: number;
}

/**
 * Where each account lives (`vela.ext.accountChains`): `{ [address, lower
 * case]: chain ids }`, most telling first — a new site starts on the first
 * the space offers (`network_space::site_start_chain_for`).
 */
export type ExtAccountChains = Record<string, number[]>;

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

/**
 * A chain's endpoint list for the worker, as the pool collects it (dapp-5):
 * the person's own RPC first — an override that is not the shipped default —
 * then the network's default and the curated public nodes behind it. With
 * "Only use my node" on and an own RPC set, the person's RPC alone: a dApp's
 * read about the account never reaches a public node either.
 *
 * The rule is the core's (`rpc_pool::own_only_pool`); this is its mirror for
 * a catalog built before the wasm is up, held to the core by
 * `ext-chains.test.ts` through the wasm.
 */
export function workerRpcList(
	network: { chainId: number; rpcURL: string },
	config: { rpcURL?: string; onlyMine?: boolean } | null
): { rpc: string[]; ownOnly: boolean } {
	const shipped = DEFAULT_NETWORKS.find((n) => n.chainId === network.chainId)?.rpcURL;
	const own = config?.rpcURL && config.rpcURL !== shipped ? config.rpcURL : undefined;
	if (own !== undefined && config?.onlyMine === true) return { rpc: [own], ownOnly: true };
	const rpc: string[] = [];
	const add = (url: string | undefined) => {
		if (url && !rpc.includes(url)) rpc.push(url);
	};
	add(own);
	add(network.rpcURL);
	for (const url of WORKER_PUBLIC_RPCS[network.chainId] ?? []) add(url);
	return { rpc, ownOnly: false };
}

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
	const order = spaceNetworksSync().map((network) => network.chainId);
	for (const network of getAllNetworksSync()) {
		if (isHiddenNetwork(network.chainId)) continue;
		// dapp-5: the person's RPC override, as the wallet's own pool reads it.
		const { rpc } = workerRpcList(network, networkConfigSync(network.chainId));
		chains[String(network.chainId)] = {
			chainId: network.chainId,
			name: network.displayName,
			symbol: nativeSymbol(network.chainId),
			rpc,
			bundler: builtinChainIds.has(network.chainId)
				? `${getBuiltinBundlerUrl()}/${network.chainId}`
				: network.bundlerURL,
			// The explorer the wallet links to: the person's, else the network's,
			// and never one the URL rule refuses — a page is handed this.
			explorer: explorerBaseURL(network.chainId) ?? ''
		};
	}
	return {
		version: 1,
		chains,
		space: networkSpace.testnet ? 'testnet' : 'mainnet',
		order,
		updatedAtMs: nowMs
	};
}

interface StorageAreaLike {
	get(keys: string): Promise<Record<string, unknown>>;
	set(items: Record<string, unknown>): Promise<void>;
}

function area(): StorageAreaLike | null {
	const local = (globalThis as { chrome?: { storage?: { local?: unknown } } }).chrome?.storage
		?.local;
	return (local as StorageAreaLike | undefined) ?? null;
}

/** What this document last stored, without its timestamp: the same table is not stored twice. */
let lastPublished: string | null = null;

/**
 * Store the catalog for the worker. Best-effort, like the snapshot.
 *
 * Not before the stored networks are read: the wallet mounts before its
 * custom networks and per-network overrides are loaded, and a catalog built
 * then names none of them — a site's reads went to the public nodes of a
 * person who set "Only use my node", and a site on a custom network was on a
 * chain the worker did not have, until the read landed and the table was
 * published again. And once the table is what the worker has, publishing it
 * again only rewrites the same thing under a new timestamp (the boot read's
 * own notification does exactly that).
 */
export async function publishExtChains(): Promise<ExtChainCatalog | null> {
	const store = area();
	if (!store) return null;
	// A failed read leaves the built-in table, which is still the right one
	// to publish rather than none.
	await ensureShownNetworks();
	const catalog = buildExtChainCatalog();
	const table = JSON.stringify({ ...catalog, updatedAtMs: 0 });
	if (table === lastPublished) return catalog;
	try {
		await store.set({ [CHAINS_KEY]: catalog });
		lastPublished = table;
		return catalog;
	} catch {
		return null;
	}
}

/** What this document last stored per account, so the same list is not stored twice. */
const lastLives = new Map<string, string>();

/**
 * r9/r10: where `address` lives — the networks it holds something on or
 * used, most telling first — stored for the worker under that account
 * (`vela.ext.accountChains`), so a new site starts there.
 *
 * Only a page that READ the account's holdings calls this (the wallet page).
 * r10: the list rode in the catalog before, which every extension page
 * publishes; the request window — which never reads holdings — published it
 * again as it opened for a connect, saying the account lived nowhere, and an
 * account holding only Arc USDC was asked to connect on Sepolia. Kept apart
 * and per account, it outlives the page that wrote it, and a page that does
 * not know an account's holdings has no way to say them.
 */
export async function setExtAccountChains(address: string, ids: number[]): Promise<void> {
	const store = area();
	if (!store || !address) return;
	const key = address.toLowerCase();
	const lives = [...new Set(ids.filter((id) => Number.isInteger(id) && id > 0))];
	if (lastLives.get(key) === lives.join(',')) return;
	lastLives.set(key, lives.join(','));
	try {
		const all = await store.get(ACCOUNT_CHAINS_KEY);
		const stored = all[ACCOUNT_CHAINS_KEY];
		const before: ExtAccountChains =
			stored && typeof stored === 'object' ? (stored as ExtAccountChains) : {};
		// The holdings are the space in front's; where the account lives in
		// the OTHER space stays as an earlier read said it.
		const here = new Set(spaceNetworksSync().map((network) => network.chainId));
		const elsewhere = (Array.isArray(before[key]) ? before[key] : []).filter(
			(id) => !here.has(id) && !lives.includes(id)
		);
		const next = [...lives, ...elsewhere];
		if ((before[key] ?? []).join(',') === next.join(',')) return;
		await store.set({ [ACCOUNT_CHAINS_KEY]: { ...before, [key]: next } });
	} catch {
		// Best-effort, like the catalog; the next change tries again.
		lastLives.delete(key);
	}
}

/** Test seam: forget what was published, so the next publish is observable. */
export function resetExtChainsForTests(): void {
	lastPublished = null;
	lastLives.clear();
}
