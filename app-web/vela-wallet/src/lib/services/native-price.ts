// Ported from src/services/native-price.ts @ c13e89d4 (spec 025).
/**
 * Native-coin price selection — WEB, and the CORE's answer.
 *
 * `best_group_price`, `best_native_dex_price` and `choose_native_price` have
 * always existed in `rust/crates/vela-core/src/app/balance_dashboard.rs` and
 * were never called: every platform re-decided in `wallet-api.ts`. This module
 * is the door. On web the shell now decodes the multicall and asks the core
 * which price to publish — the deepest-pool max within a stable, the
 * cross-stable max, the (0.5, 2.0) sanity band against Chainlink and the
 * source ladder are all read out of Rust.
 *
 * Pure kernels, so this is `connect-entry.web.ts`'s "ask and read the verdict"
 * shape without even a throwaway core: two synchronous function calls.
 * Importing `@/services/vela-core` first is load-bearing — its web entry
 * initialises the wasm module at import time, so these calls need no async
 * gate (which matters: the caller is deep inside a per-chain multicall
 * decode).
 *
 * `native-price.ts` is the iOS/Android twin (Hermes has no WebAssembly) and
 * carries the same rules in TypeScript. Only TYPES are imported from it —
 * importing its values here would make the web bundle run the copy this file
 * exists to replace, and there is no `DEFAULT_QUOTE_DECIMALS` on this side at
 * all because the core applies its own. The two are pinned together by
 * `src/__tests__/services/native-price-parity.test.ts`.
 */

import {
	bestNativeDexPrice as coreBestNativeDexPrice,
	chooseNativePrice as coreChooseNativePrice,
	peggedNativeUsd as corePeggedNativeUsd
} from '$lib/core/client';

/** Quote-token decimals when the `decimals()` read failed — USDC's 6. */
export const DEFAULT_QUOTE_DECIMALS = 6;

/**
 * One stable's DEX quotes for 1 native coin, as the shell decodes them out of
 * the multicall. Each stable is its OWN group so its own decimals normalise
 * the amount — USDC (6) and DAI (18) must never be compared under one shared
 * scale.
 */
export interface NativeQuoteGroup {
	/**
	 * Successful quote outputs in THIS stable's base units, as decimal strings
	 * (failed calls are simply absent). Strings, not `bigint`, because that is
	 * the shape that crosses the wasm boundary losslessly.
	 */
	amountsOut: string[];
	/** This stable's `decimals()` read; `null` = it failed → {@link DEFAULT_QUOTE_DECIMALS}. */
	quoteDecimals: number | null;
}

/** Where the chosen native price came from — surfaced in the price log. */
export type NativePriceSource =
	'none' | 'DEX' | 'Chainlink(sanity)' | 'Chainlink(local)' | 'Chainlink(ETH)';

export interface NativePrice {
	price: number | null;
	source: NativePriceSource;
}

/**
 * The core's `NativePriceSource` variant name → the label the price log has
 * always printed. Wire translation only: the core owns WHICH rung was taken,
 * the shell owns what that rung is called on screen and in the log.
 */
const SOURCE_LABELS: Record<string, NativePriceSource> = {
	dex: 'DEX',
	chainlinkSanity: 'Chainlink(sanity)',
	chainlinkLocal: 'Chainlink(local)',
	chainlinkEth: 'Chainlink(ETH)',
	none: 'none'
};

export function bestNativeDexPrice(groups: NativeQuoteGroup[]): number | null {
	// `Option<u32>` crosses as `number | undefined`, and the answer as
	// `number | undefined`; both sides of this seam speak `null`. A missing
	// `quoteDecimals` is the "the decimals() read failed" signal either way —
	// the core, not this line, decides what to use instead.
	const wire = groups.map((group) => ({
		amountsOut: group.amountsOut,
		quoteDecimals: group.quoteDecimals ?? undefined
	}));
	return coreBestNativeDexPrice({ groups: wire }) ?? null;
}

/**
 * The $1 peg for a native gas coin that IS a dollar stablecoin — Tempo's `USD`,
 * Arc's `USDC`. `null` means "not pegged": the caller falls through to the
 * Chainlink/DEX ladder unchanged.
 *
 * Here, and not in `price-service`, for the same reason the rest of this module
 * is here: it is a CORE call, and the core is loaded before the price path
 * runs. `price-service` stays free of the core so it can be read and mocked as
 * pure feed plumbing.
 */
export function peggedNativeUsd(symbol: string): number | null {
	return corePeggedNativeUsd(symbol) ?? null;
}

export function chooseNativePrice(
	dexPrice: number | null,
	onChainClPrice: number | null,
	ethClPrice: number | null
): NativePrice {
	const chosen = coreChooseNativePrice(dexPrice, onChainClPrice, ethClPrice);
	return {
		price: chosen.price ?? null,
		// An unknown variant name means the core grew a rung this shell has no
		// label for. Reporting it verbatim keeps the log honest instead of
		// silently mislabelling the source; the PRICE is unaffected either way.
		source: SOURCE_LABELS[chosen.source] ?? (chosen.source as NativePriceSource)
	};
}
