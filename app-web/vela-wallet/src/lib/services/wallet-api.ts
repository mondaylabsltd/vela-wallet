// Ported from src/services/wallet-api.ts @ c13e89d4 (spec 025). Import seams
// only; the multicall/price pipeline is verbatim.
/**
 * On-chain asset query engine.
 *
 * Direct on-chain queries using Multicall3. Each network is queried with a
 * single eth_call containing batched balance + DEX price queries.
 *
 * Architecture:
 *   1. Discover tokens per chain (native + stablecoins + wrapped native + custom ERC-20s)
 *   2. Query balances via Multicall3 (one RPC call per network)
 *   3. Query prices via DEX swap quotes in the same Multicall3 batch
 *   4. Fall back to Chainlink feeds on Ethereum mainnet for missing prices
 *   5. Filter to non-zero balances, sort by USD value
 */

import type { APIToken, CustomToken } from './tokens-model';
import { tokenUsdValue, tokenChainId, isNativeToken } from './tokens-model';
import { getAllNetworksSync, networkId, chainName, nativeSymbol } from './networks';
import { loadCustomTokens } from './records';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';
import { poolRpcCall, getFailedRpcChains } from './rpc-pool';
import { priceShouldNull } from './fault-injection';
import { fetchChainTokens, pickQuoteToken, type ChainTokenData } from './chain-tokens';
// The platform seam for the native-coin price rules (spec 017 wave C): web
// resolves to `native-price.web.ts` and the CORE decides; iOS/Android resolve
// to `native-price.ts`, the TypeScript twin, because Hermes has no wasm.
import {
	bestNativeDexPrice,
	chooseNativePrice,
	peggedNativeUsd,
	type NativeQuoteGroup
} from './native-price';
import { fetchChainlinkPrices, resolveChainlinkPrice } from './price-service';
import {
	MULTICALL3,
	encAggregate3,
	decAggregate3,
	encBalanceOf,
	encDecimals,
	encGetEthBalance,
	encQuoteV3,
	encGetAmountsOut,
	encLatestRound,
	decChainlinkUsd,
	decU256,
	decU8,
	decAmountsOut,
	type Call3,
	type McResult
} from './abi';

// ---------------------------------------------------------------------------
// Per-chain Chainlink native/USD feed addresses.
// Queried directly on each chain as part of the multicall — no extra RPC call.
// All feeds use 8 decimals (answer / 1e8 = USD price).
// ---------------------------------------------------------------------------

const NATIVE_CHAINLINK_FEEDS: Record<number, string> = {
	1: '0x5f4eC3Df9cbd43714FE2740f5E3616155c5b8419', // ETH/USD on Ethereum
	56: '0x0567F2323251f0Aab15c8dFb1967E4e8A7D42aeE', // BNB/USD on BSC
	// 137: Polygon — no working Chainlink feed (MATIC→POL migration), DEX covers it
	42161: '0x639Fe6ab55C921f74e7fac1ee960C0B6293ba612', // ETH/USD on Arbitrum
	10: '0x13e3Ee699D1909E989722E753853AE30b17e08c5', // ETH/USD on Optimism
	8453: '0x71041dddad3595F9CEd3DcCFBe3D1F4b0a16Bb70', // ETH/USD on Base
	43114: '0x0A77230d17318075983913bC2145DB16C7366156', // AVAX/USD on Avalanche
	100: '0x678df3415fc31947dA4324eC63212874be5a82f8' // DAI/USD on Gnosis
};

// ---------------------------------------------------------------------------
// Named holdings rule.
//
// `sortAndFilterHoldings` is ALSO implemented in `rust/crates/vela-core/src/
// app/balance_dashboard.rs` (`sort_by_usd_desc` + the
// `token_balance_double(...) > 0.0` merge filter) — the core re-applies it on
// web, and Hermes has no wasm so this file stays native's only
// implementation.
//
// The native-COIN pricing rules used to sit here too. They now live behind the
// `@/services/native-price` seam: on web that resolves to `native-price.web.ts`
// and the RULES are executed by the core (`best_group_price`,
// `best_native_dex_price`, `choose_native_price`, which were dead code on every
// platform until spec 017 wave C); on iOS/Android it resolves to
// `native-price.ts`, the TypeScript twin. `src/__tests__/services/
// native-price-parity.test.ts` drives the real core and the twin over the same
// scenarios, which is what turns a one-sided edit red — and
// `src/__tests__/services/core-table-parity.test.ts` reads the band constants
// straight out of the Rust source. (An earlier version of this comment named
// `balance-core-parity.test.ts`, which has never existed in this repo.)
// ---------------------------------------------------------------------------

/**
 * Holdings as every surface consumes them: zero balances dropped (unless the
 * caller asked for the superset), most valuable first.
 */
export function sortAndFilterHoldings(
	tokens: APIToken[],
	includeZeroBalance?: boolean
): APIToken[] {
	return tokens
		.filter((t) => includeZeroBalance || parseFloat(t.balance) > 0)
		.sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
}

/**
 * A chain whose RPC never answered this round. Deliberately distinct from a
 * chain that answered with nothing: the first means "unknown", the second
 * means "empty", and collapsing the two is what made held tokens disappear
 * from the Assets list and from the total (issue 196).
 */
class ChainUnreachableError extends Error {
	readonly chainId: number;
	constructor(chainId: number, options?: ErrorOptions) {
		super(`chain ${chainId} did not answer`, options);
		this.name = 'ChainUnreachableError';
		this.chainId = chainId;
	}
}

/**
 * The previous snapshot's holdings on chains that did not answer, kept
 * alongside the ones that did.
 *
 * The core's streaming merge already holds a chain's last value while it is
 * in flight (`chain_assets_arrived`, invariant ④), but the settled list
 * replaces everything — so before this, one round in which Gnosis or Base
 * timed out removed those tokens from the Assets list and dropped their
 * value out of the Total balance, while the send picker, which snapshots the
 * same fetch at a different moment, still listed them. A chain that ANSWERED
 * stays authoritative: tokens it no longer reports really have been spent.
 */
export function carryOverUnansweredChains(
	previous: readonly APIToken[],
	fresh: readonly APIToken[],
	answeredChainIds: ReadonlySet<number>
): APIToken[] {
	const carried = previous.filter((t) => !answeredChainIds.has(tokenChainId(t)));
	return carried.length === 0 ? [...fresh] : [...fresh, ...carried];
}

/**
 * Quote-token decimals when the `decimals()` read failed — USDC's 6.
 *
 * Shell-side default for the CUSTOM-token price paths below, which the core
 * has no rule for. The native-coin path does not use it: it forwards
 * `quoteDecimals: null` and the core applies its own `DEFAULT_QUOTE_DECIMALS`
 * (pinned to this value by `core-table-parity.test.ts`).
 */
export const DEFAULT_QUOTE_DECIMALS = 6;

// Re-exported so existing importers (and the drift gate) keep one name for the
// native-price vocabulary. On web these ARE the core's answers.
export { bestNativeDexPrice, chooseNativePrice, peggedNativeUsd } from './native-price';
export type { NativePrice, NativePriceSource, NativeQuoteGroup } from './native-price';

// ---------------------------------------------------------------------------
// Cache (same interface as before)
// ---------------------------------------------------------------------------

const TOKEN_CACHE_TTL_MS = 5 * 60 * 1000; // 5 min

type TokenCacheEntry = {
	fetchedAt: number;
	tokens: APIToken[];
	inFlight?: Promise<APIToken[]>;
};

const tokenCache = new Map<string, TokenCacheEntry>();

export type FetchTokensOptions = {
	forceRefresh?: boolean;
	maxAgeMs?: number;
	/** Include tokens with zero balance (for managing watchlist). Default: false. */
	includeZeroBalance?: boolean;
	/** Called each time a chain finishes, with the accumulated tokens so far (sorted by USD value). */
	onProgress?: (tokens: APIToken[]) => void;
	/** Called after all chains finish, with the chain IDs whose RPC endpoints all failed. */
	onFailedChains?: (chainIds: number[]) => void;
};

export class APIError extends Error {
	constructor(message = 'Failed to fetch data from server.') {
		super(message);
		this.name = 'APIError';
	}
}

// ---------------------------------------------------------------------------
// Public API (same interface as before)
// ---------------------------------------------------------------------------

/** Fetch token balances across all supported networks. */
export async function fetchTokens(
	address: string,
	options: FetchTokensOptions = {}
): Promise<APIToken[]> {
	const cacheKey = address.trim().toLowerCase();
	const maxAgeMs = options.maxAgeMs ?? TOKEN_CACHE_TTL_MS;
	const cached = tokenCache.get(cacheKey);
	const now = Date.now();

	// includeZeroBalance bypasses cache (different result set)
	if (!options.forceRefresh && !options.includeZeroBalance && cached) {
		if (cached.inFlight) return cloneTokens(await cached.inFlight);
		if (now - cached.fetchedAt < maxAgeMs) return cloneTokens(cached.tokens);
	}

	const request = fetchAllChainTokens(
		address,
		// The snapshot a chain falls back to when it does not answer. The
		// zero-balance superset is a different result set with its own
		// (uncached) shape, so it carries nothing over.
		options.includeZeroBalance ? [] : (cached?.tokens ?? []),
		options.onProgress,
		options.onFailedChains,
		options.includeZeroBalance
	);

	// Don't pollute the main cache with includeZeroBalance results
	if (!options.includeZeroBalance) {
		tokenCache.set(cacheKey, {
			fetchedAt: cached?.fetchedAt ?? 0,
			tokens: cached?.tokens ?? [],
			inFlight: request
		});
	}

	try {
		const tokens = await request;
		if (!options.includeZeroBalance) {
			tokenCache.set(cacheKey, { fetchedAt: Date.now(), tokens });
		}
		return cloneTokens(tokens);
	} catch (error) {
		if (cached?.tokens.length) {
			tokenCache.set(cacheKey, cached);
		} else {
			tokenCache.delete(cacheKey);
		}
		throw error;
	}
}

export function clearTokenCache(address?: string): void {
	if (address) tokenCache.delete(address.trim().toLowerCase());
	else tokenCache.clear();
}

/**
 * Synchronously read the ERC-20 token addresses the user is known to hold on a
 * chain, from the in-memory token cache (lowercased). Empty when the cache is
 * cold — never triggers a fetch. Used by transaction simulation to trust a
 * *received* token the user already holds (a real token, not a spoofed one).
 */
export function getCachedHeldTokens(address: string | undefined, chainId: number): string[] {
	if (!address) return [];
	const entry = tokenCache.get(address.trim().toLowerCase());
	if (!entry?.tokens?.length) return [];
	const out: string[] = [];
	for (const t of entry.tokens) {
		if (tokenChainId(t) === chainId && !isNativeToken(t) && t.tokenAddress) {
			out.push(t.tokenAddress.toLowerCase());
		}
	}
	return out;
}

/** Fetch USD to target currency exchange rate (unchanged). */
export async function fetchExchangeRate(currency = 'CNY'): Promise<number> {
	const url = `https://getvela.app/api/exchange-rate?currency=${encodeURIComponent(currency)}`;
	const response = await fetchWithTimeout(url, {}, { timeoutMs: NET_TIMEOUTS.fiatRates });
	if (!response.ok) throw new APIError(`/exchange-rate failed: HTTP ${response.status}`);
	const data: { currency: string; rate: number } = await response.json();
	return data.rate;
}

// ---------------------------------------------------------------------------
// Core: orchestrate all chains
// ---------------------------------------------------------------------------

type ChainOutcome = { answered: boolean; tokens: APIToken[] };

async function fetchAllChainTokens(
	address: string,
	previous: readonly APIToken[],
	onProgress?: (tokens: APIToken[]) => void,
	onFailedChains?: (chainIds: number[]) => void,
	includeZeroBalance?: boolean
): Promise<APIToken[]> {
	// Phase 1: load prerequisites in parallel
	const [customTokens, clPrices] = await Promise.all([loadCustomTokens(), fetchChainlinkPrices()]);

	// Phase 2: query each chain in parallel, streaming results as each chain finishes
	const networks = getAllNetworksSync();
	const accumulated: APIToken[] = [];
	const answered = new Set<number>();
	const carryable = new Set(previous.map(tokenChainId));

	const sortAndFilter = () =>
		sortAndFilterHoldings(
			carryOverUnansweredChains(previous, accumulated, answered),
			includeZeroBalance
		);

	// Cap each chain so one dead/slow RPC can't hold the whole fetch (a chain
	// with no healthy endpoint can otherwise burn ~60s on sequential failover).
	// A capped-out chain contributes nothing this round and is retried next time.
	const PER_CHAIN_TIMEOUT_MS = 18_000;
	await Promise.allSettled(
		networks.map((net) => {
			const chainTokensP: Promise<ChainOutcome> = queryChainAssets(
				address,
				net.chainId,
				customTokens.filter((ct) => ct.chainId === net.chainId),
				clPrices
			).then(
				(tokens) => ({ answered: true, tokens }),
				() => ({ answered: false, tokens: [] })
			);
			const bounded = Promise.race([
				chainTokensP,
				new Promise<ChainOutcome>((resolve) =>
					setTimeout(() => resolve({ answered: false, tokens: [] }), PER_CHAIN_TIMEOUT_MS)
				)
			]);
			return bounded.then((outcome) => {
				// A chain that did not answer — RPC failure or the cap above —
				// keeps whatever the previous snapshot knew it held (issue 196);
				// only one that answered may shorten the list.
				if (!outcome.answered) return;
				answered.add(net.chainId);
				accumulated.push(...outcome.tokens);
				// An answered-empty chain still changes the list when it had
				// something to carry over; otherwise the snapshot is unmoved
				// and a tick would only churn the screen (issue 188).
				if (outcome.tokens.length > 0 || carryable.has(net.chainId)) {
					onProgress?.(sortAndFilter());
				}
			});
		})
	);

	// Report every chain this round could not read: the ones whose RPC
	// endpoints all failed, plus the ones that never answered (a rejection or
	// the per-chain cap). The core treats a round with any of these as partial
	// and refuses to promote its total to last-known-good — which it must,
	// now that such a round can be carrying a chain's stale holdings forward.
	const failed = networks
		.map((n) => n.chainId)
		.filter((id) => getFailedRpcChains().has(id) || !answered.has(id));
	if (failed.length > 0) onFailedChains?.(failed);

	return sortAndFilter();
}

// ---------------------------------------------------------------------------
// Per-chain: build multicall, execute, decode
// ---------------------------------------------------------------------------

/** Category for internal token tracking. */
type TokenCategory = 'native' | 'stable' | 'wrapped' | 'custom';

interface TokenSlot {
	symbol: string;
	name: string;
	contract: string | null; // null = native token
	category: TokenCategory;
	knownDecimals: number | null;
}

/**
 * Chains whose native asset is itself an ERC-20 — the chain data names that
 * contract as the "wrapped" native, but nothing is wrapped: `balanceOf` and
 * the native balance are one balance. Listing both doubles the holding.
 */
const NATIVE_TOKEN_AS_ERC20: Readonly<Record<number, string>> = {
	// Celo mainnet — the GoldToken.
	42220: '0x471ece3750da237f93b8e339c536989b8978a438',
	// Celo Alfajores.
	44787: '0xf194afdf50b03e69bd7d057c1aa9e10c9954e4c9'
};

export function wrappedNativeIsTheNative(chainId: number, wrappedNative: string): boolean {
	return NATIVE_TOKEN_AS_ERC20[chainId] === wrappedNative.toLowerCase();
}

async function queryChainAssets(
	address: string,
	chainId: number,
	customTokens: CustomToken[],
	chainlinkPrices: Record<string, number>
): Promise<APIToken[]> {
	// 1. Discover tokens on this chain
	const chainData = await fetchChainTokens(chainId);
	const nativeCurrency = chainData?.nativeCurrency ?? {
		name: nativeSymbol(chainId),
		symbol: nativeSymbol(chainId),
		decimals: 18
	};
	const stables = chainData?.stables ?? [];
	const wrappedNative = chainData?.wrappedNativeToken ?? null;
	const dex = chainData?.dex ?? null;

	// 2. Build token slots and multicall batch
	const slots: TokenSlot[] = [];
	const calls: Call3[] = [];
	const balIdx: number[] = []; // calls index for each slot's balance
	const decIdx: (number | null)[] = []; // calls index for each slot's decimals

	// --- Native token ---
	addSlot('native', nativeCurrency.symbol, nativeCurrency.name, null, nativeCurrency.decimals);
	balIdx.push(calls.length);
	calls.push(mc(MULTICALL3, encGetEthBalance(address)));
	decIdx.push(null);

	// --- Stablecoins ---
	for (const s of stables) {
		addSlot('stable', s.symbol, s.symbol, s.contract, null);
		balIdx.push(calls.length);
		calls.push(mc(s.contract, encBalanceOf(address)));
		decIdx.push(calls.length);
		calls.push(mc(s.contract, encDecimals()));
	}

	// --- Wrapped native token ---
	// Not on a chain whose "wrapped" native IS the native (spec 038, the
	// founder's Celo report): CELO is an ERC-20 at the GoldToken address, so
	// the native balance call and the token's balanceOf return the SAME
	// coins, and the list read CELO 6.96 + WCELO 6.96 — counted twice in the
	// total. The address still quotes the native price below.
	if (wrappedNative && !wrappedNativeIsTheNative(chainId, wrappedNative)) {
		addSlot(
			'wrapped',
			'W' + nativeCurrency.symbol,
			'Wrapped ' + nativeCurrency.name,
			wrappedNative,
			null
		);
		balIdx.push(calls.length);
		calls.push(mc(wrappedNative, encBalanceOf(address)));
		decIdx.push(calls.length);
		calls.push(mc(wrappedNative, encDecimals()));
	}

	// --- Custom ERC-20s ---
	for (const ct of customTokens) {
		addSlot('custom', ct.symbol, ct.name, ct.contractAddress, ct.decimals);
		balIdx.push(calls.length);
		calls.push(mc(ct.contractAddress, encBalanceOf(address)));
		decIdx.push(null); // decimals already known
	}

	// --- DEX price queries ---
	// The preferred quote token — it only orders the custom-token price attempts
	// now. It is deliberately NOT a source of decimals for anyone else's quote:
	// every quote is scaled by the decimals of the stable it was quoted against.
	const quoteToken = pickQuoteToken(stables);

	// Native price: quote 1 wrappedNative against EVERY stable and keep the DEEPEST pool's
	// result (see extractBestPrice), not the first that happens to return. One stable's pool can
	// be broken/near-empty and quote a wildly-off price (e.g. X Layer's WOKB/USDC quotes OKB at
	// ~$5) while another stable holds the liquid pool (WOKB/USD₮0 ≈ $81); taking the first would
	// lock in the junk. Each stable is its OWN group so its own decimals normalize the amount
	// (USDC=6 vs DAI=18 must never be compared under one shared scale).
	const nativeQuoteGroups: { idxs: number[]; decCallIdx: number | null }[] = [];
	if (wrappedNative && dex) {
		const amountIn = 10n ** BigInt(nativeCurrency.decimals);
		for (let si = 0; si < stables.length; si++) {
			const idxs: number[] = [];
			addDexPriceCalls(dex, wrappedNative, stables[si].contract, amountIn, idxs);
			if (idxs.length > 0) nativeQuoteGroups.push({ idxs, decCallIdx: decIdx[1 + si] }); // +1: native is slot 0
		}
	}

	// Custom ERC-20 prices:
	//   Path A: token → ANY stablecoin (USDC, USDC.e, USDT, etc.)
	//   Path B: token → wrappedNative (WETH/WMATIC) — then multiply by nativePriceUsd
	//
	// Path A is grouped PER STABLE, exactly like the native path above, because
	// the amount a quote returns is denominated in that stable's own base units.
	// A flat list of quotes across USDC (6) and DAI (18) scaled by one shared
	// `quoteDecimals` mis-prices by 10^12 the moment the first surviving quote is
	// not the token whose decimals were read — see `firstGroupedQuotePrice`.
	//
	// The groups are ordered preferred-quote-token first (`pickQuoteToken`, the
	// choice this code always declared but never actually honoured), then the
	// remaining stables in chain order, so USDC's deeper pool wins a tie and a
	// chain-specific bridge stable (USDC.e) still answers when it does not.
	const customDirectGroups = new Map<number, { idxs: number[]; decCallIdx: number | null }[]>();
	const customViaNativeIdxs = new Map<number, number[]>();
	if (dex) {
		// Stable indices in the order Path A should try them.
		const preferredIdx = quoteToken ? stables.indexOf(quoteToken) : -1;
		const stableOrder = stables.map((_, si) => si);
		if (preferredIdx > 0) {
			stableOrder.splice(preferredIdx, 1);
			stableOrder.unshift(preferredIdx);
		}
		for (let i = 0; i < slots.length; i++) {
			if (slots[i].category !== 'custom' || !slots[i].contract) continue;
			const dec = slots[i].knownDecimals ?? 18;
			const amountIn = 10n ** BigInt(dec);
			// Path A: token → stablecoin (try ALL stablecoins — e.g. USDC.e has pools native USDC doesn't)
			const directGroups: { idxs: number[]; decCallIdx: number | null }[] = [];
			for (const si of stableOrder) {
				const idxs: number[] = [];
				addDexPriceCalls(dex, slots[i].contract!, stables[si].contract, amountIn, idxs);
				// +1 because native is slot 0, so stable `si` is slot `1 + si`.
				if (idxs.length > 0) directGroups.push({ idxs, decCallIdx: decIdx[1 + si] });
			}
			if (directGroups.length > 0) customDirectGroups.set(i, directGroups);
			// Path B: token → wrappedNative (most tokens have WETH/WMATIC pools)
			if (wrappedNative) {
				const nativeIdxs: number[] = [];
				addDexPriceCalls(dex, slots[i].contract!, wrappedNative, amountIn, nativeIdxs);
				if (nativeIdxs.length > 0) customViaNativeIdxs.set(i, nativeIdxs);
			}
		}
	}

	// --- Per-chain Chainlink native/USD feed (queried in the same multicall) ---
	let onChainChainlinkIdx: number | null = null;
	const nativeFeed = NATIVE_CHAINLINK_FEEDS[chainId];
	if (nativeFeed) {
		onChainChainlinkIdx = calls.length;
		calls.push(mc(nativeFeed, encLatestRound()));
	}

	// 3. Execute multicall
	if (calls.length === 0) return [];

	let results: McResult[];
	try {
		const encoded = encAggregate3(calls);
		const raw = await ethCall(chainId, MULTICALL3, encoded);
		results = decAggregate3(raw);
	} catch (cause) {
		// NOT `return []` (issue 196): an empty list is how a chain says "this
		// account holds nothing here", and the core takes a settled list at
		// its word (`balance_dashboard.rs` `accept`). Returning it for an RPC
		// failure deleted that chain's holdings from the Assets list and from
		// the total. Rejecting instead lets `fetchAllChainTokens` keep the
		// last known holdings for a chain that never answered.
		throw new ChainUnreachableError(chainId, { cause });
	}

	// 4. Resolve native price: DEX → on-chain Chainlink → Ethereum Chainlink
	//
	// Everything below the decode is the seam's — WHICH pool wins inside a
	// stable, which stable wins across them, whether the DEX price survives the
	// sanity band and which rung of the ladder answers. This function only
	// decodes: raw `amountOut`s per group and this group's `decimals()` read, or
	// `null` when that read failed so the seam applies its own default rather
	// than a second copy of it here.
	const dexDecoder = dex?.protocol === 'solidly' ? decAmountsOut : decU256;
	const dexGroups: NativeQuoteGroup[] = nativeQuoteGroups.map((g) => {
		const decR = g.decCallIdx != null ? results[g.decCallIdx] : null;
		return {
			amountsOut: decodeQuoteAmounts(results, g.idxs, dexDecoder),
			quoteDecimals: decR?.success ? decU8(decR.data) : null
		};
	});
	const dexPrice = bestNativeDexPrice(dexGroups);

	// Try on-chain Chainlink feed (queried in same multicall, zero extra cost)
	let onChainClPrice: number | null = null;
	if (onChainChainlinkIdx != null) {
		const clr = results[onChainChainlinkIdx];
		if (clr?.success && clr.data.length >= 66) {
			const usd = decChainlinkUsd(clr.data);
			if (Number.isFinite(usd) && usd > 0) onChainClPrice = usd;
		}
	}

	// Try Ethereum mainnet Chainlink
	// A coin that IS a dollar (Tempo's USD, Arc's USDC) is pegged by the core:
	// there is no feed to read, because there is nothing to measure. The peg is
	// also what earns such a chain the ordinary $0.01 fee floor, which is
	// "$0.01 worth of the native coin" and needs a price to exist (spec 060).
	const ethClPrice =
		peggedNativeUsd(nativeCurrency.symbol) ??
		resolveChainlinkPrice(nativeCurrency.symbol, chainlinkPrices);

	// Pick best price: DEX preferred, but sanity-check against Chainlink.
	// If DEX price deviates >50% from Chainlink, DEX likely has low liquidity → prefer Chainlink.
	const { price: nativePriceUsd, source: nativePriceSource } = chooseNativePrice(
		dexPrice,
		onChainClPrice,
		ethClPrice
	);

	// Log summary: which source was used + status of all sources
	const dexOk = nativeQuoteGroups.reduce(
		(n, g) => n + g.idxs.filter((i) => results[i]?.success).length,
		0
	);
	const dexTotal = nativeQuoteGroups.reduce((n, g) => n + g.idxs.length, 0);
	console.log(
		`[Price] chain=${chainId} ${nativeCurrency.symbol} → $${nativePriceUsd?.toFixed(2) ?? '?'} via ${nativePriceSource}` +
			` | DEX: ${dexPrice != null ? `$${dexPrice.toFixed(2)}` : `FAIL(${dexOk}/${dexTotal})`}` +
			` | CL-local: ${onChainClPrice != null ? `$${onChainClPrice.toFixed(2)}` : 'n/a'}` +
			` | CL-ETH: ${ethClPrice != null ? `$${ethClPrice.toFixed(2)}` : 'n/a'}`
	);

	// 6. Build APIToken array
	const netId = networkId(chainId);
	const netName = chainName(chainId);
	const tokens: APIToken[] = [];

	for (let i = 0; i < slots.length; i++) {
		const slot = slots[i];

		// Balance
		const balR = results[balIdx[i]];
		const rawBal = balR?.success ? decU256(balR.data) : 0n;

		// Decimals
		let dec = slot.knownDecimals;
		if (dec == null && decIdx[i] != null) {
			const decR = results[decIdx[i]!];
			dec = decR?.success ? decU8(decR.data) : 18;
		}
		dec = dec ?? 18;

		// Price
		let priceUsd: number | null = null;
		switch (slot.category) {
			case 'native':
			case 'wrapped':
				priceUsd = nativePriceUsd;
				break;
			case 'stable':
				// ── This one is the SHELL's, on purpose (spec 017 no_core_owns_it) ──
				//
				// $1.00 here is not a missing factor defaulting to 1: `stable` is a
				// MEMBERSHIP verdict — the token came from this chain's curated
				// stablecoin list (`chain-tokens.ts`) — and ≈$1 is the definition of
				// that membership. The same approximation is already core-owned on the
				// two paths that matter for records and signing (`activity_feed.rs::
				// is_stable` / `tx_usd_value`, `clear_signing.rs::STABLE_SYMBOLS`), and
				// `core-table-parity.test.ts` gates the table both platforms read.
				//
				// A de-peg GATE was considered and rejected. The only independent
				// measurement available here is the same DEX quote whose near-empty
				// pools `chooseNativePrice` already has to defend against; nulling a
				// stablecoin's price on its say-so would silently drop that holding out
				// of the user's total with no explanation and no way to proceed — the
				// exact failure mode a gate is supposed to prevent, inverted. A wrong
				// de-peg verdict costs more than the approximation it replaces.
				//
				// The one thing this value must never do is enter a conversion,
				// signing or persistence path that has its own owner. It does not: it
				// reaches display (`tokenUsdValue`) and, for tokens the user holds, the
				// ingest valuation in `activity.ts` — which is documented there and
				// re-derived on read by the core.
				priceUsd = 1.0;
				break;
			case 'custom': {
				// Path A: direct token → stablecoin, each group scaled by ITS OWN
				// quote token's decimals (see `customDirectGroups`).
				const directGroups = customDirectGroups.get(i) ?? [];
				priceUsd = firstGroupedQuotePrice(
					directGroups.map((g) => ({
						amountsOut: decodeQuoteAmounts(results, g.idxs, dexDecoder),
						quoteDecimals:
							g.decCallIdx != null && results[g.decCallIdx]?.success
								? decU8(results[g.decCallIdx]!.data)
								: null
					}))
				);
				let erc20Source = priceUsd != null ? 'direct' : '';
				// Path B: token → wrappedNative, then multiply by native USD price.
				// One quote token (the wrapped native), so one scale — and wrapped
				// native mirrors the coin's decimals by construction.
				if (priceUsd == null && nativePriceUsd != null) {
					const viaNativeIdxs = customViaNativeIdxs.get(i);
					const priceInNative = extractPrice(
						results,
						viaNativeIdxs ?? [],
						nativeCurrency.decimals,
						dexDecoder
					);
					if (priceInNative != null) {
						priceUsd = priceInNative * nativePriceUsd;
						erc20Source = 'viaNative';
					}
				}
				const directIdxs = directGroups.flatMap((g) => g.idxs);
				const dOk = directIdxs.filter((j) => results[j]?.success).length;
				const dTot = directIdxs.length;
				const nOk = (customViaNativeIdxs.get(i) ?? []).filter((j) => results[j]?.success).length;
				const nTot = customViaNativeIdxs.get(i)?.length ?? 0;
				console.log(
					`[Price] chain=${chainId} ERC20 ${slot.symbol} → $${priceUsd?.toFixed(4) ?? '?'} via ${erc20Source || 'FAIL'} | direct(${dOk}/${dTot}) viaNative(${nOk}/${nTot})`
				);
				break;
			}
		}

		tokens.push({
			network: netId,
			chainName: netName,
			symbol: slot.symbol,
			balance: formatRawBalance(rawBal, dec),
			decimals: dec,
			logo: null,
			name: slot.name,
			tokenAddress: slot.contract,
			priceUsd,
			spam: false
		});
	}

	// Dev fault injection: drop prices so held tokens still render but the total
	// undercounts — reproduces the "balance silently dropped" scenario.
	if (priceShouldNull(chainId)) {
		for (const tk of tokens) tk.priceUsd = null;
	}

	return tokens;

	// --- Local helpers ---

	function addSlot(
		category: TokenCategory,
		symbol: string,
		name: string,
		contract: string | null,
		knownDecimals: number | null
	) {
		slots.push({ symbol, name, contract, category, knownDecimals });
	}

	function addDexPriceCalls(
		dexInfo: NonNullable<ChainTokenData['dex']>,
		tokenIn: string,
		tokenOut: string,
		amountIn: bigint,
		outIdxs: number[]
	) {
		const { protocol, contracts } = dexInfo;

		if (protocol === 'uniswap-v3' && contracts.quoterV2) {
			// Try common fee tiers: 500 (0.05%), 3000 (0.3%), 2500 (0.25% — PancakeSwap V3), 10000 (1% — exotic pairs)
			for (const fee of [500, 3000, 2500, 10000]) {
				outIdxs.push(calls.length);
				calls.push(mc(contracts.quoterV2, encQuoteV3(tokenIn, tokenOut, amountIn, fee)));
			}
		} else if (protocol === 'solidly' && contracts.router) {
			// Aerodrome/Velodrome V2: getAmountsOut with Route struct, try both volatile and stable
			for (const stable of [false, true]) {
				outIdxs.push(calls.length);
				calls.push(mc(contracts.router, encGetAmountsOut(amountIn, tokenIn, tokenOut, stable)));
			}
		}
		// liquidity-book & curve: rely on Chainlink fallback
	}
}

// ---------------------------------------------------------------------------
// Multicall helper
// ---------------------------------------------------------------------------

function mc(target: string, callData: string): Call3 {
	return { target, allowFailure: true, callData };
}

// ---------------------------------------------------------------------------
// Price extraction
// ---------------------------------------------------------------------------

/**
 * First usable USD price across quote groups, each scaled by ITS OWN quote
 * token's decimals.
 *
 * The rule this replaces took the first surviving quote out of a flat list that
 * mixed quote tokens and divided it by ONE token's `decimals()`. On any chain
 * whose stablecoin list holds both a 6-decimal (USDC/USDT) and an 18-decimal
 * (DAI/WXDAI) entry, a custom token with no USDC pool but a live DAI pool was
 * priced 10^12 times too high, and that number is what the portfolio total, the
 * sort order and the ingest valuation in `activity.ts` all consume.
 *
 * `quoteDecimals: null` means that group's `decimals()` read failed; it falls
 * back to {@link DEFAULT_QUOTE_DECIMALS} — the SAME group's fallback, never a
 * neighbour's real value. Groups are tried in order and a zero amount does not
 * price (a zero-output quote is a dead pool, not a free token).
 */
export function firstGroupedQuotePrice(
	groups: { amountsOut: string[]; quoteDecimals: number | null }[]
): number | null {
	for (const group of groups) {
		const dec = group.quoteDecimals ?? DEFAULT_QUOTE_DECIMALS;
		for (const raw of group.amountsOut) {
			const amountOut = BigInt(raw);
			if (amountOut > 0n) return Number(amountOut) / 10 ** dec;
		}
	}
	return null;
}

/** Extract a USD price from DEX quote results. Takes the first successful quote. Callers must
 *  pass only quotes that share `quoteDecimals` — today that is the token→wrappedNative path,
 *  which has exactly one quote token. Mixed quote tokens go through
 *  {@link firstGroupedQuotePrice} instead. */
function extractPrice(
	results: McResult[],
	callIdxs: number[],
	quoteDecimals: number,
	decoder: (hex: string) => bigint = decU256
): number | null {
	for (const idx of callIdxs) {
		const r = results[idx];
		if (r?.success && r.data.length >= 66) {
			// 0x + 64 hex chars minimum for uint256
			const amountOut = decoder(r.data);
			if (amountOut > 0n) {
				return Number(amountOut) / 10 ** quoteDecimals;
			}
		}
	}
	return null;
}

/** Best (deepest-pool) USD price among DEX quotes that share ONE `quoteDecimals`. For a fixed
 *  input amount, a more-liquid Uniswap-V3 pool returns more output (less slippage), so the max
 *  output is the least-distorted price — this dodges a broken/near-empty pool that would quote
 *  a garbage low value. Only safe within a single quote token (same decimals); callers group
 *  per token and compare the per-token results afterward. */
/**
 * The successful quote outputs of one group, in that stable's base units.
 *
 * Decode only — no comparison, no scaling, no "which pool is best". A failed
 * or too-short result is simply absent (there is no amount to report), and a
 * zero amount is reported as `"0"` because deciding that a zero quote cannot
 * price is the core's rule, not this function's.
 */
function decodeQuoteAmounts(
	results: McResult[],
	callIdxs: number[],
	decoder: (hex: string) => bigint = decU256
): string[] {
	const amounts: string[] = [];
	for (const idx of callIdxs) {
		const r = results[idx];
		if (r?.success && r.data.length >= 66) amounts.push(decoder(r.data).toString());
	}
	return amounts;
}

// ---------------------------------------------------------------------------
// Balance formatting
// ---------------------------------------------------------------------------

/** Convert a raw bigint balance to a human-readable decimal string. */
function formatRawBalance(raw: bigint, decimals: number): string {
	if (raw === 0n) return '0';
	const s = raw.toString();
	if (decimals === 0) return s;

	let intPart: string;
	let decPart: string;

	if (s.length <= decimals) {
		intPart = '0';
		decPart = s.padStart(decimals, '0');
	} else {
		intPart = s.slice(0, s.length - decimals);
		decPart = s.slice(s.length - decimals);
	}

	// Trim trailing zeros
	decPart = decPart.replace(/0+$/, '');
	return decPart ? `${intPart}.${decPart}` : intPart;
}

// ---------------------------------------------------------------------------
// RPC helper (uses the global RPC pool for load balancing + failover)
// ---------------------------------------------------------------------------

async function ethCall(chainId: number, to: string, data: string): Promise<string> {
	const response = await poolRpcCall('eth_call', [{ to, data }, 'latest'], chainId);
	if (response.error) throw new Error(response.error.message);
	const result = response.result;
	if (typeof result !== 'string' || result === '0x') throw new Error('Empty result');
	return result;
}

// ---------------------------------------------------------------------------
// Utilities
// ---------------------------------------------------------------------------

function cloneTokens(tokens: APIToken[]): APIToken[] {
	return tokens.map((t) => ({ ...t }));
}
