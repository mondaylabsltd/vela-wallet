//! The tokens the wallet names by itself, per chain (spec 097 part D).
//!
//! One table, decided once: each built-in network's stablecoins and wrapped
//! native coin as the chain-data service lists them — the same registry every
//! shell's asset list reads — and the well-known tokens the wallet carried
//! before, each pinned to the chain it lives on. A token here is the
//! wallet's own word: the signing sheet names it without its address, its
//! decimals are never asked of the chain, Activity lets it lead a row, and
//! the trust rules treat it as trusted.
//!
//! The key is `(chain, address)`, never the address alone. The same bytes
//! are different tokens on different chains — `0x4200…0006` is WETH on the
//! OP-stack chains but Plume's wrapped coin, `0x779d…3736` is "USD₮0" on X
//! Layer and "USDT0" on Mantle, and USDC has 6 decimals on Ethereum but 18
//! on BNB Chain — and an Ethereum address on another chain may be anybody's
//! contract.
//!
//! The table is generated (`scripts/gen-token-registry.mjs`) and committed;
//! nothing here touches the network.

mod table;

/// A token the registry names.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryToken {
    pub symbol: &'static str,
    pub decimals: u32,
}

/// The registry's entry for `address` on `chain_id` (any case), or `None`
/// when the wallet does not know that token on that chain.
#[must_use]
pub fn registry_token(chain_id: u32, address: &str) -> Option<RegistryToken> {
    let lc = address.trim().to_ascii_lowercase();
    table::TABLE
        .binary_search_by(|(chain, addr, _, _)| (*chain, *addr).cmp(&(chain_id, lc.as_str())))
        .ok()
        .map(|i| {
            let (_, _, symbol, decimals) = table::TABLE[i];
            RegistryToken { symbol, decimals }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    const USDC_BSC: &str = "0x8ac76a51cc950d9822d68b83fe1ad97b32cd580d";
    const USDC_ETH: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";
    const OP_WETH: &str = "0x4200000000000000000000000000000000000006";

    fn token(symbol: &'static str, decimals: u32) -> Option<RegistryToken> {
        Some(RegistryToken { symbol, decimals })
    }

    /// Strictly increasing `(chain, address)`: sorted for the binary search,
    /// and no token listed twice. Every address lowercased and well formed,
    /// every symbol present, every width a `uint8` the amounts can carry.
    #[test]
    fn the_table_is_sorted_unique_and_well_formed() {
        for pair in table::TABLE.windows(2) {
            assert!(
                (pair[0].0, pair[0].1) < (pair[1].0, pair[1].1),
                "out of order or listed twice: {:?} then {:?}",
                pair[0],
                pair[1]
            );
        }
        for (chain, address, symbol, decimals) in table::TABLE {
            assert!(chain > 0);
            assert!(
                address.len() == 42
                    && address.starts_with("0x")
                    && address[2..]
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
                "{address}"
            );
            assert!(!symbol.trim().is_empty(), "{address}");
            assert!(
                decimals <= crate::l10n::number::MAX_TOKEN_DECIMALS,
                "{address}"
            );
        }
    }

    #[test]
    fn usdc_has_the_decimals_of_its_chain() {
        assert_eq!(registry_token(56, USDC_BSC), token("USDC", 18));
        assert_eq!(
            registry_token(8453, "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"),
            token("USDC", 6)
        );
        assert_eq!(registry_token(1, USDC_ETH), token("USDC", 6));
        assert_eq!(
            registry_token(56, &USDC_BSC.to_uppercase().replacen("0X", "0x", 1)),
            token("USDC", 18),
            "any case"
        );
    }

    /// An address is a token only on the chain the registry lists it for.
    #[test]
    fn the_same_address_resolves_per_chain() {
        assert_eq!(registry_token(1, USDC_BSC), None);
        assert_eq!(registry_token(56, USDC_ETH), None);
        for chain in [10, 8453, 130, 480, 1868, 4326, 57073] {
            assert_eq!(registry_token(chain, OP_WETH), token("WETH", 18), "{chain}");
        }
        // Plume's wrapped coin at the same address answers no decimals():
        // left out, not guessed.
        assert_eq!(registry_token(98866, OP_WETH), None);
        let usdt0 = "0x779ded0c9e1022225f8e0630b35a9b54be713736";
        assert_eq!(registry_token(196, usdt0), token("USD₮0", 6));
        assert_eq!(registry_token(5000, usdt0), token("USDT0", 6));
    }

    /// The well-known tokens kept from the old tables, on their own chains,
    /// and the wrapped native coins under their own symbols.
    #[test]
    fn well_known_tokens_live_on_their_chains() {
        let wbtc = "0x2260fac5e5542a773aa44fbcfedf7c193bc2c599";
        assert_eq!(registry_token(1, wbtc), token("WBTC", 8));
        assert_eq!(registry_token(137, wbtc), None);
        assert_eq!(
            registry_token(137, "0x2791bca1f2de4661ed88a30c99a7a9449aa84174"),
            token("USDC.e", 6)
        );
        assert_eq!(
            registry_token(42161, "0xfd086bc7cd5c481dcc9c85ebe478a1c0b69fcbb9"),
            token("USDT", 6)
        );
        assert_eq!(
            registry_token(56, "0xbb4cdb9cbd36b01bd1cbaebf2de08d9173bc095c"),
            token("WBNB", 18)
        );
        assert_eq!(
            registry_token(56, "0x0101010101010101010101010101010101010101"),
            None
        );
        assert_eq!(registry_token(56, ""), None);
    }
}
