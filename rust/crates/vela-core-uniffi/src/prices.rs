//! The native-coin price ladder, decided by the core (spec 051 phase 2c).
//!
//! `choose_native_price` has lived in
//! `vela-core/src/app/balance_dashboard.rs` since spec 017 and every platform
//! kept re-deciding around it in `wallet-api.ts`. Web opened the door in spec
//! 025 (`native-price.web.ts` asks the wasm bridge); this is the same door on
//! the uniffi side, and the reason iOS's price path contains no ladder of its
//! own.
//!
//! What is at stake is not style. The rung order and the (0.5, 2.0) sanity
//! band decide **which number is somebody's balance** when a DEX quote and a
//! Chainlink feed disagree — the X Layer WOKB case, where one near-empty pool
//! quoted $5 against a real ~$81. A shell that re-implements that band is a
//! second opinion nobody diffed.
//!
//! `best_native_dex_price` is deliberately NOT exported yet: iOS has no DEX
//! quotes to fold (the quote path is deferred — see results.md), and 050's D10
//! rule is that an export arrives with the code that calls it.

use vela_core::app::balance_dashboard::{self, NativePriceSource};

/// The chosen price and the rung it came from. `source` is `"none"` when
/// nothing could price the coin — which is **not** a price of zero, and the
/// caller must not turn it into one.
#[derive(uniffi::Record)]
pub struct NativePriceChoice {
    pub price: Option<f64>,
    /// The `NativePriceSource` variant name, spelled exactly as the wasm
    /// bridge spells it so a price log reads the same on every client.
    pub source: String,
}

fn source_name(source: NativePriceSource) -> &'static str {
    match source {
        NativePriceSource::Dex => "dex",
        NativePriceSource::ChainlinkSanity => "chainlinkSanity",
        NativePriceSource::ChainlinkLocal => "chainlinkLocal",
        NativePriceSource::ChainlinkEth => "chainlinkEth",
    }
}

/// The source ladder and its sanity band — DEX preferred, but a DEX price
/// deviating beyond ratio (0.5, 2.0) against the best Chainlink read means low
/// liquidity, so Chainlink wins.
#[uniffi::export]
pub fn choose_native_price(
    dex: Option<f64>,
    chainlink_local: Option<f64>,
    chainlink_eth: Option<f64>,
) -> NativePriceChoice {
    match balance_dashboard::choose_native_price(dex, chainlink_local, chainlink_eth) {
        Some(chosen) => NativePriceChoice {
            price: Some(chosen.price),
            source: source_name(chosen.source).to_owned(),
        },
        None => NativePriceChoice {
            price: None,
            source: "none".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The bridge must not gain an opinion on the way through: the same three
    /// inputs, the same verdict as the kernel it wraps.
    #[test]
    fn forwards_the_kernel_verdict() {
        let chosen = choose_native_price(Some(81.0), Some(80.0), None);
        assert_eq!(chosen.price, Some(81.0));
        assert_eq!(chosen.source, "dex");

        // The X Layer WOKB case: a near-empty pool quoting $5 against a real
        // ~$81 is outside the band, so Chainlink wins.
        let rescued = choose_native_price(Some(5.0), Some(81.0), None);
        assert_eq!(rescued.price, Some(81.0));
        assert_eq!(rescued.source, "chainlinkSanity");

        // No source at all is `None`, never a zero somebody would render as
        // "$0.00 of ETH".
        let nothing = choose_native_price(None, None, None);
        assert_eq!(nothing.price, None);
        assert_eq!(nothing.source, "none");
    }

    /// The local feed's decode gate travels with it: a non-finite or
    /// non-positive "price" is no price, and must not out-rank the mainnet
    /// fallback.
    #[test]
    fn a_broken_local_feed_falls_through_to_ethereum() {
        let chosen = choose_native_price(None, Some(0.0), Some(2500.0));
        assert_eq!(chosen.price, Some(2500.0));
        assert_eq!(chosen.source, "chainlinkEth");
    }
}
