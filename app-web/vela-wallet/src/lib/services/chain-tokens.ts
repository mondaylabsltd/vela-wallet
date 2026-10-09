// Ported from src/services/chain-tokens.ts @ c13e89d4 (spec 025).
/**
 * Chain token discovery from ethereum-data API.
 *
 * Fetches stablecoins, wrapped native token, and DEX info per chain.
 * Data source: https://ethereum-data.getvela.app/chains/eip155-{chainId}.json
 */

import { getEthereumDataURL } from './endpoints';
import { fetchWithTimeout, NET_TIMEOUTS } from './net';
const CACHE_TTL = 30 * 60 * 1000; // 30 min

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export interface StableToken {
	symbol: string;
	type: string; // "native" | "bridge"
	contract: string;
}

export interface DexInfo {
	dex: string; // "Uniswap", "PancakeSwap", "Aerodrome", etc.
	protocol: string; // "uniswap-v3", "solidly", "liquidity-book", "curve"
	contracts: Record<string, string>;
	url?: string;
}

export interface ChainTokenData {
	chainId: number;
	nativeCurrency: { name: string; symbol: string; decimals: number };
	stables: StableToken[];
	wrappedNativeToken: string | null;
	dex: DexInfo | null;
}

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

/**
 * What reading a chain's registry document came to (PR 2 polish) — three
 * outcomes, not two. `doc`: the server answered with the document. `absent`:
 * the server answered that there is none (HTTP 404) — a definitive "no such
 * document". `unread`: nothing definitive came back — the request failed or
 * timed out, the server answered 5xx, 429 or any other non-2xx, or the body
 * could not be read. Folding `unread` into "no document" made a chain with no
 * native coin (Tempo) read as "answered, holds nothing" — $0.00 — whenever the
 * document's server could not be reached.
 */
export type ChainTokensRead =
	{ kind: 'doc'; data: ChainTokenData } | { kind: 'absent' } | { kind: 'unread'; cause: unknown };

/** The definitive answers, kept {@link CACHE_TTL}. An `unread` is never kept: the next ask reads again. */
const cache = new Map<number, { read: Exclude<ChainTokensRead, { kind: 'unread' }>; at: number }>();

// ---------------------------------------------------------------------------
// Built-in DEX overrides — guaranteed correct, never depend on remote API.
// Each entry uses the most mainstream DEX on that chain.
// ---------------------------------------------------------------------------

const BUILTIN_DEX: Record<number, DexInfo> = {
	// Ethereum — Uniswap V3 (dominant DEX)
	1: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
			quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
		}
	},
	// BSC — PancakeSwap V3 (dominant DEX)
	56: {
		dex: 'PancakeSwap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x0BFbCF9fa4f9C56B0F40a671Ad40E0805A091865',
			quoterV2: '0xB048Bbc1Ee6b733FFfCFb9e9CeF7375518e25997'
		}
	},
	// Polygon — Uniswap V3 (dominant DEX)
	137: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
			quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
		}
	},
	// Arbitrum — Uniswap V3 (dominant DEX)
	42161: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
			quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
		}
	},
	// Optimism — Uniswap V3 (dominant DEX)
	10: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
			quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
		}
	},
	// Base — Aerodrome (dominant DEX by TVL)
	8453: {
		dex: 'Aerodrome',
		protocol: 'solidly',
		contracts: {
			factory: '0x420DD381b31aEf6683db6B902084cB0FFECe40Da',
			router: '0xcF77a3Ba9A5CA399B7c97c74d54e5b1Beb874E43'
		}
	},
	// Avalanche — Uniswap V3 (compatible quoter, Trader Joe uses unsupported liquidity-book)
	43114: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x740b1c1de25031C31FF4fC9A62f554A55cdC1baD',
			quoterV2: '0xbe0F5544EC67e9B3b2D979aaA43f18Fd87E6257F'
		}
	},
	// Gnosis — SushiSwap V3 (best available V3 quoter)
	100: {
		dex: 'SushiSwap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0xf78031CBCA409F2FB6876BDFDBc1b2df24cF9bEf',
			quoterV2: '0xb1E835Dc2785b52265711e17fCCb0fd018226a6e'
		}
	},
	// Unichain — Uniswap V3 (canonical deploy addresses)
	130: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
			quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
		}
	},
	// Monad — Uniswap V3 (canonical deploy addresses)
	143: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
			quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
		}
	},
	// World Chain — Uniswap V3 (canonical deploy addresses)
	480: {
		dex: 'Uniswap',
		protocol: 'uniswap-v3',
		contracts: {
			factory: '0x1F98431c8aD98523631AE4a59f267346ea31F984',
			quoterV2: '0x61fFE014bA17989E743c5F6cB21bF9697530B21e'
		}
	}
};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/**
 * The chain's registry document, as one of three outcomes
 * ({@link ChainTokensRead}). `doc` and `absent` are cached for 30 min;
 * `unread` never is.
 */
export async function readChainTokens(chainId: number): Promise<ChainTokensRead> {
	const cached = cache.get(chainId);
	if (cached && Date.now() - cached.at < CACHE_TTL) return cached.read;

	let res: Response;
	try {
		res = await fetchWithTimeout(
			`${getEthereumDataURL()}/chains/eip155-${chainId}.json`,
			{},
			{ timeoutMs: NET_TIMEOUTS.ethereumData }
		);
	} catch (cause) {
		// No answer at all: offline, refused, timed out.
		return { kind: 'unread', cause };
	}
	if (res.status === 404) {
		// The server answered: there is no document for this chain.
		const read = { kind: 'absent' } as const;
		cache.set(chainId, { read, at: Date.now() });
		return read;
	}
	if (!res.ok) {
		// 5xx, 429, any other refusal: the server said nothing about the chain.
		return { kind: 'unread', cause: new Error(`registry document: HTTP ${res.status}`) };
	}

	// The registry document, trusted only field by field (the Expo module had
	// `any` here; strict mode wants the shape spelled out).
	let raw: {
		nativeCurrency?: { name?: string; symbol?: string; decimals?: unknown };
		stables?: unknown;
		wrappedNativeToken?: string | null;
		dex?: ChainTokenData['dex'];
	};
	try {
		const body: unknown = await res.json();
		if (typeof body !== 'object' || body === null || Array.isArray(body)) {
			return { kind: 'unread', cause: new Error('registry document: not an object') };
		}
		raw = body as typeof raw;
	} catch (cause) {
		// A body cut off or not JSON is no answer either.
		return { kind: 'unread', cause };
	}
	const rawDec = raw.nativeCurrency?.decimals;
	const decimals = typeof rawDec === 'number' && rawDec >= 0 && rawDec <= 255 ? rawDec : 18;

	const data: ChainTokenData = {
		chainId,
		nativeCurrency: {
			name: raw.nativeCurrency?.name ?? 'Ether',
			symbol: raw.nativeCurrency?.symbol ?? 'ETH',
			decimals
		},
		stables: Array.isArray(raw.stables) ? (raw.stables as ChainTokenData['stables']) : [],
		wrappedNativeToken: raw.wrappedNativeToken ?? null,
		// Built-in DEX overrides take priority over API data
		dex: BUILTIN_DEX[chainId] ?? raw.dex ?? null
	};

	const read = { kind: 'doc', data } as const;
	cache.set(chainId, { read, at: Date.now() });
	return read;
}

/**
 * The chain's registry facts, or `null` when there are none to be had —
 * `absent` and `unread` alike. For callers that only want facts (token
 * trust); the balance read tells the two apart ({@link readChainTokens}).
 */
export async function fetchChainTokens(chainId: number): Promise<ChainTokenData | null> {
	const read = await readChainTokens(chainId);
	return read.kind === 'doc' ? read.data : null;
}

/**
 * Pick the best quote token from the stables list for DEX price queries.
 * Prefers native USDC > any USDC > USDT > first stablecoin.
 */
export function pickQuoteToken(stables: StableToken[]): StableToken | null {
	return (
		stables.find((s) => s.symbol === 'USDC' && s.type === 'native') ??
		stables.find((s) => s.symbol === 'USDC') ??
		stables.find((s) => s.symbol === 'USDT') ??
		stables[0] ??
		null
	);
}
