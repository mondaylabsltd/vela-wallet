//! `vela.customTokens` — the ledger two machines write and a third reads.
//!
//! `manage_tokens` is where a person adds a token by hand; `token_trust` is
//! where one is admitted automatically from an authenticated receipt; and the
//! balance fetch reads whatever is there. Three callers, one file on disk, and
//! **one place that knows how it is spelled** — FR-005 keeps these bytes
//! readable by every other Vela client, so the field names are not ours to
//! drift.
//!
//! ## `networkName` is derived, not carried
//!
//! `TrustCustomToken` has no `network_name` and says why: "chain naming is
//! display vocabulary the shell derives from `chain_id`". `MtokCustomToken`
//! does carry one, because the panel that adds a token already knows which
//! network row it was on. Both end up in the same record, and this file is
//! where the derivation happens so the two paths cannot disagree about what
//! chain 100 is called.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use vela_core::app::manage_tokens::MtokCustomToken;
use vela_core::app::network_admin::BUILTIN_CHAINS;
use vela_core::app::token_trust::TrustCustomToken;

use crate::executor::storage;

/// The shared key. Do not rename: other clients read these bytes.
const TOKENS_KEY: &str = "vela.customTokens";

/// The on-disk record — `models/types.ts CustomToken`.
#[derive(Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct StoredToken {
    pub id: String,
    pub chain_id: u32,
    pub contract_address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u32,
    pub network_name: String,
}

/// What a chain is called on screen: the built-in name, the person's name for a
/// network they added, or the number itself — which is honest rather than
/// blank.
#[must_use]
pub fn network_name(chain_id: u32) -> String {
    if let Some(chain) = BUILTIN_CHAINS.iter().find(|c| c.chain_id == chain_id) {
        return chain.display_name.to_owned();
    }
    if let Ok(Some(Value::Array(items))) = storage::read_value(storage::KEY_CUSTOM_NETWORKS) {
        for item in items {
            if item.get("chainId").and_then(Value::as_u64) == Some(u64::from(chain_id)) {
                if let Some(name) = item.get("displayName").and_then(Value::as_str) {
                    if !name.is_empty() {
                        return name.to_owned();
                    }
                }
            }
        }
    }
    format!("Chain {chain_id}")
}

/// Every stored token, in the order it was written.
#[must_use]
pub fn read() -> Vec<StoredToken> {
    let Ok(Some(Value::Array(items))) = storage::read_value(TOKENS_KEY) else {
        return Vec::new();
    };
    items
        .into_iter()
        .filter_map(|item| serde_json::from_value::<StoredToken>(item).ok())
        .collect()
}

fn write(tokens: &[StoredToken]) -> bool {
    let encoded = Value::Array(
        tokens
            .iter()
            .map(|token| serde_json::to_value(token).unwrap_or(Value::Null))
            .collect(),
    );
    storage::write_value(TOKENS_KEY, encoded).is_ok()
}

/// Add or replace one token, by id.
///
/// **Replace, never append.** The id is `"{chainId}_{contract}"`, so adding the
/// same contract twice is a person correcting themselves — or an admission
/// re-running — and not two tokens (`token_trust` invariant ⑧).
pub fn save(token: StoredToken) -> bool {
    let mut tokens = read();
    tokens.retain(|existing| existing.id != token.id);
    tokens.push(token);
    write(&tokens)
}

/// Remove one token by id. `false` when there was nothing to remove — the row
/// is still on the person's screen and the core has to know that.
pub fn remove(id: &str) -> bool {
    let mut tokens = read();
    let before = tokens.len();
    tokens.retain(|token| token.id != id);
    tokens.len() != before && write(&tokens)
}

impl From<&MtokCustomToken> for StoredToken {
    fn from(token: &MtokCustomToken) -> Self {
        Self {
            id: token.id.clone(),
            chain_id: token.chain_id,
            contract_address: token.contract_address.clone(),
            symbol: token.symbol.clone(),
            name: token.name.clone(),
            decimals: u32::from(token.decimals),
            network_name: token.network_name.clone(),
        }
    }
}

impl From<&TrustCustomToken> for StoredToken {
    fn from(token: &TrustCustomToken) -> Self {
        Self {
            id: token.id.clone(),
            chain_id: token.chain_id,
            contract_address: token.contract_address.clone(),
            symbol: token.symbol.clone(),
            name: token.name.clone(),
            decimals: token.decimals,
            // The one field the core does not carry, derived here so both
            // writers spell chain 100 the same way.
            network_name: network_name(token.chain_id),
        }
    }
}

impl StoredToken {
    /// As `manage_tokens` speaks it. A scale past `u8` cannot be an ERC-20
    /// `decimals()`, so such a record is not offered rather than truncated into
    /// a different, plausible scale.
    #[must_use]
    pub fn to_mtok(&self) -> Option<MtokCustomToken> {
        Some(MtokCustomToken {
            id: self.id.clone(),
            chain_id: self.chain_id,
            contract_address: self.contract_address.clone(),
            symbol: self.symbol.clone(),
            name: self.name.clone(),
            decimals: u8::try_from(self.decimals).ok()?,
            network_name: self.network_name.clone(),
        })
    }

    /// As `token_trust` speaks it.
    #[must_use]
    pub fn to_trust(&self) -> TrustCustomToken {
        TrustCustomToken {
            id: self.id.clone(),
            chain_id: self.chain_id,
            contract_address: self.contract_address.clone(),
            symbol: self.symbol.clone(),
            name: self.name.clone(),
            decimals: self.decimals,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn trust(id: &str, chain_id: u32) -> TrustCustomToken {
        TrustCustomToken {
            id: id.to_owned(),
            chain_id,
            contract_address: "0xaaa".to_owned(),
            symbol: "AAA".to_owned(),
            name: "Triple A".to_owned(),
            decimals: 6,
        }
    }

    /// One ledger, two vocabularies, and a record written by either is readable
    /// by both.
    #[test]
    fn a_token_written_by_one_machine_is_read_by_the_other() {
        storage::tests::with_temp_state("custom-tokens-shared", || {
            assert!(save(StoredToken::from(&trust("100_0xaaa", 100))));

            let stored = read();
            assert_eq!(stored.len(), 1);
            // The derived field the core does not carry.
            assert_eq!(stored[0].network_name, "Gnosis");

            let as_mtok = stored[0]
                .to_mtok()
                .unwrap_or_else(|| unreachable!("6 decimals fits a u8"));
            assert_eq!(as_mtok.decimals, 6);
            assert_eq!(as_mtok.network_name, "Gnosis");
            assert_eq!(stored[0].to_trust(), trust("100_0xaaa", 100));
        });
    }

    /// Adding the same contract again replaces it. It is one token.
    #[test]
    fn the_same_token_saved_twice_is_still_one_token() {
        storage::tests::with_temp_state("custom-tokens-dedupe", || {
            assert!(save(StoredToken::from(&trust("100_0xaaa", 100))));
            let mut second = StoredToken::from(&trust("100_0xaaa", 100));
            second.symbol = "AAA2".to_owned();
            assert!(save(second));

            let stored = read();
            assert_eq!(stored.len(), 1, "replaced, not appended");
            assert_eq!(stored[0].symbol, "AAA2");

            assert!(remove("100_0xaaa"));
            assert!(read().is_empty());
            // Removing what is not there is a failure, not a quiet success.
            assert!(!remove("100_0xaaa"));
        });
    }

    /// The shared bytes, spelled the way every other client reads them.
    #[test]
    fn the_record_keeps_its_cross_client_field_names() {
        storage::tests::with_temp_state("custom-tokens-shape", || {
            assert!(save(StoredToken::from(&trust("100_0xaaa", 100))));
            let raw = storage::read_value(TOKENS_KEY)
                .ok()
                .flatten()
                .unwrap_or_else(|| unreachable!("nothing written"));
            let record = raw.get(0).unwrap_or_else(|| unreachable!("not an array"));
            for field in [
                "id",
                "chainId",
                "contractAddress",
                "symbol",
                "name",
                "decimals",
                "networkName",
            ] {
                assert!(record.get(field).is_some(), "missing `{field}`");
            }
        });
    }

    /// A network the person added is called what they called it; one nobody
    /// named is called by its number rather than by nothing.
    #[test]
    fn a_chain_is_named_by_the_wallet_then_by_the_person_then_by_its_number() {
        storage::tests::with_temp_state("custom-tokens-network-name", || {
            assert_eq!(network_name(1), "Ethereum");
            assert_eq!(network_name(100), "Gnosis");
            assert_eq!(network_name(7_777_777), "Chain 7777777");

            let networks = json!([{ "chainId": 7_777_777, "displayName": "My testnet" }]);
            if storage::write_value(storage::KEY_CUSTOM_NETWORKS, networks).is_err() {
                unreachable!("could not seed");
            }
            assert_eq!(network_name(7_777_777), "My testnet");
        });
    }

    /// A scale that cannot be an ERC-20 `decimals()` is not offered as one.
    #[test]
    fn an_impossible_scale_is_withheld_rather_than_truncated() {
        let token = StoredToken {
            id: "1_0xaaa".to_owned(),
            chain_id: 1,
            contract_address: "0xaaa".to_owned(),
            symbol: "AAA".to_owned(),
            name: "A".to_owned(),
            decimals: 300,
            network_name: "Ethereum".to_owned(),
        };
        assert!(token.to_mtok().is_none());
        // The trust vocabulary takes a u32, so it keeps the value it was given.
        assert_eq!(token.to_trust().decimals, 300);
    }
}
