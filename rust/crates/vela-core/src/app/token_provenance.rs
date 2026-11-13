//! Who vouches for a token's name, on its chain (round 4, r4a-2).
//!
//! A token's symbol is whatever its contract's `symbol()` answers: anyone can
//! deploy a contract that calls itself "USDC". A token-picker that drew two
//! rows "USDC · Arc Testnet" — the chain's own coin and a stranger's contract
//! — let a member propose sending the stranger's token believing it was the
//! real one, and every other member approve it reading the same word.
//!
//! The rule, one for every surface that names a token (picker, form,
//! confirm, a multisig's rows and sheet, Activity):
//!
//! - the chain's **native** coin, and a token the wallet's own registry lists
//!   on that chain ([`super::token_registry`], plus a chain's native coin
//!   behind its ERC-20 interface), are **vouched**: their name is the
//!   wallet's word;
//! - any other token is **unverified**: a surface shows its contract address
//!   beside its symbol and says it is unverified;
//! - an unverified token whose symbol reads as the native coin's or a vouched
//!   token's on the same chain — whatever its case, invisible characters, or
//!   letters from another script that draw the same — is a **lookalike**, and
//!   a surface warns that it is not the real one.
//!
//! The key is `(chain, address)`: the same address is a different token on
//! another chain, and a symbol collides only with the tokens of its own.

use serde::{Deserialize, Serialize};

use super::invisible_chars::is_invisible;
use super::manage_tokens::native_alias_token;
use super::token_registry::{registry_token, registry_tokens_on};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// Who vouches for a token's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TokenTrust {
    /// The chain's own coin.
    Native,
    /// A token the wallet's registry lists on this chain.
    Vouched,
    /// Anybody's contract: only its address says which token it is.
    Unverified,
}

/// A token's provenance on its chain.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TokenProvenance {
    pub trust: TokenTrust,
    /// For an unverified token: the vouched symbol its own reads as (the
    /// native coin's, or a registry token's on this chain) — the real
    /// token's spelling, so a surface can say "Not the real USDC".
    pub imitates: Option<String>,
}

/// A symbol as a person reads it: no invisible characters, no case, and the
/// letters of other scripts (Cyrillic, Greek, full-width) that draw as Latin
/// ones folded to them. Only for comparing — never shown.
fn reads_as(symbol: &str) -> String {
    symbol
        .chars()
        .filter(|c| !is_invisible(*c) && !c.is_whitespace())
        .map(|c| match c {
            // Full-width ASCII.
            '\u{FF01}'..='\u{FF5E}' => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
            // Cyrillic letters that draw as Latin ones.
            'А' | 'а' => 'a',
            'В' | 'в' => 'b',
            'С' | 'с' => 'c',
            'Е' | 'е' => 'e',
            'Н' | 'н' => 'h',
            'І' | 'і' => 'i',
            'Ј' | 'ј' => 'j',
            'К' | 'к' => 'k',
            'М' | 'м' => 'm',
            'О' | 'о' => 'o',
            'Р' | 'р' => 'p',
            'Ѕ' | 'ѕ' => 's',
            'Т' | 'т' => 't',
            'Х' | 'х' => 'x',
            'У' | 'у' => 'y',
            // Greek letters that draw as Latin ones.
            'Α' | 'α' => 'a',
            'Β' | 'β' => 'b',
            'Ε' | 'ε' => 'e',
            'Ζ' => 'z',
            'Η' => 'h',
            'Ι' | 'ι' => 'i',
            'Κ' | 'κ' => 'k',
            'Μ' => 'm',
            'Ν' => 'n',
            'ν' => 'v',
            'Ο' | 'ο' => 'o',
            'Ρ' | 'ρ' => 'p',
            'Τ' | 'τ' => 't',
            'Υ' | 'υ' => 'y',
            'Χ' | 'χ' => 'x',
            // Currency signs drawn as a letter they stand in for (USD₮).
            '₮' => 't',
            other => other,
        })
        .flat_map(char::to_lowercase)
        .collect()
}

/// The provenance of `token` (`None`: the native coin) named `symbol` on
/// `chain_id`, whose native coin is `native_symbol` (the shell's chain data).
#[must_use]
pub fn token_provenance(
    chain_id: u32,
    token: Option<&str>,
    symbol: &str,
    native_symbol: &str,
) -> TokenProvenance {
    let vouched = |trust| TokenProvenance {
        trust,
        imitates: None,
    };
    let Some(token) = token.map(str::trim).filter(|t| !t.is_empty()) else {
        return vouched(TokenTrust::Native);
    };
    if registry_token(chain_id, token).is_some()
        || native_alias_token(chain_id).is_some_and(|alias| alias.eq_ignore_ascii_case(token))
    {
        return vouched(TokenTrust::Vouched);
    }
    let read = reads_as(symbol);
    let imitates = (!read.is_empty())
        .then(|| {
            std::iter::once(native_symbol)
                .chain(registry_tokens_on(chain_id).map(|t| t.symbol))
                .find(|real| !real.trim().is_empty() && reads_as(real) == read)
                .map(str::to_owned)
        })
        .flatten();
    TokenProvenance {
        trust: TokenTrust::Unverified,
        imitates,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const ARC: u32 = 5_042_002;
    const FAKE: &str = "0x57e64022d8d9a420dFE08a22d6a687caaBc6B7C6";
    const BASE_USDC: &str = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";

    fn unverified(imitates: Option<&str>) -> TokenProvenance {
        TokenProvenance {
            trust: TokenTrust::Unverified,
            imitates: imitates.map(str::to_owned),
        }
    }

    #[test]
    fn the_native_coin_and_the_registrys_tokens_are_vouched() {
        assert_eq!(
            token_provenance(ARC, None, "USDC", "USDC").trust,
            TokenTrust::Native
        );
        assert_eq!(
            token_provenance(8453, Some(BASE_USDC), "USDC", "ETH").trust,
            TokenTrust::Vouched
        );
        // Arc's native USDC behind its ERC-20 interface is the same coin.
        assert_eq!(
            token_provenance(
                ARC,
                Some("0x3600000000000000000000000000000000000000"),
                "USDC",
                "USDC"
            )
            .trust,
            TokenTrust::Vouched
        );
    }

    /// The round-4 case: a stranger's 6-decimal "USDC" on Arc, where USDC is
    /// the chain's own coin.
    #[test]
    fn a_token_named_as_the_native_coin_is_a_lookalike() {
        assert_eq!(
            token_provenance(ARC, Some(FAKE), "USDC", "USDC"),
            unverified(Some("USDC"))
        );
    }

    #[test]
    fn a_token_named_as_a_registry_token_on_its_chain_is_a_lookalike() {
        let other = "0x1111111111111111111111111111111111111111";
        assert_eq!(
            token_provenance(8453, Some(other), "usdc", "ETH"),
            unverified(Some("USDC"))
        );
        // The same name on a chain whose registry has no USDC, and whose coin
        // is not one, imitates nothing there.
        assert_eq!(
            token_provenance(ARC, Some(other), "WETH", "USDC"),
            unverified(None)
        );
    }

    /// Case, invisible characters and letters of other scripts that draw the
    /// same do not make another name.
    #[test]
    fn a_symbol_that_only_looks_different_still_imitates() {
        for disguised in ["usdc", "USD\u{0421}", "U\u{200B}SDC", "ＵＳＤＣ", " USDC "] {
            assert_eq!(
                token_provenance(ARC, Some(FAKE), disguised, "USDC")
                    .imitates
                    .as_deref(),
                Some("USDC"),
                "{disguised:?}"
            );
        }
    }

    #[test]
    fn an_unknown_token_with_its_own_name_is_only_unverified() {
        assert_eq!(
            token_provenance(ARC, Some(FAKE), "R4T", "USDC"),
            unverified(None)
        );
        assert_eq!(
            token_provenance(ARC, Some(FAKE), "", "USDC"),
            unverified(None)
        );
    }
}
