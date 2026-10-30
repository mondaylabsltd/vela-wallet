//! F0.5 — developer launch inputs that point a profile at a local harness
//! (the fork, the relay, the mailbox), the desktop twin of the web e2e
//! harness's `useHarness` (`app-web/vela-wallet/e2e/onchain/harness.ts`).
//!
//! | Variable | Shape | Written to |
//! |---|---|---|
//! | `VELA_SERVICE_ENDPOINTS` | JSON object, the stored camelCase names: `{"bundlerServiceURL": "http://127.0.0.1:9190", "genesisServiceURL": …, "passkeyIndexURL": …, "ethereumDataURL": …, "fiatRatesURL": …}` | `vela.serviceEndpoints`, merged field by field |
//! | `VELA_RPC_OVERRIDES` | JSON object, chain id → RPC URL: `{"100": "http://127.0.0.1:9145"}` | `vela.networkConfig`, each chain's `rpcURL` (its other settings kept) |
//! | `VELA_PEERCONNECT_RELAY` | a URL | `vela.peerconnectRelay` (the web's key; read once multisig lands here) |
//!
//! Each is read ONCE, at launch, before anything reads the store, and is
//! written into the store as a person's own Settings would write it — so
//! every reader (the pool, the relay client, the endpoint ledger) sees the
//! harness with no second code path, and the core's own URL rule still
//! applies (`http://localhost|127.0.0.1|[::1]` only, besides `https`).
//! Pair them with `VELA_STATE_DIR=<scratch dir>`: they are writes, and a
//! developer's real profile is not a harness.
//!
//! **Developer builds only** (owner rule 2026-10-02, spec 095): the module
//! is compiled only into a debug, test or `dev-fixtures` build, and the names
//! go through `dev_env::var!`, so a release binary carries neither the code
//! nor the variable names (`scripts/check-store-binary.sh` asserts the names).

use serde_json::{Map, Value};

use crate::diag::vlog;
use crate::executor::storage;

/// Where the web keeps the PeerConnect relay (`vela.peerconnectRelay`).
pub const PEERCONNECT_KEY: &str = "vela.peerconnectRelay";

/// The stored endpoint names a seed may set — the five the ledger reads.
const ENDPOINT_FIELDS: [&str; 5] = [
    "ethereumDataURL",
    "passkeyIndexURL",
    "bundlerServiceURL",
    "fiatRatesURL",
    "genesisServiceURL",
];

/// Apply every launch input that is set. Never fails the launch: a seed that
/// does not parse is said in the log and skipped.
pub fn apply() {
    if let Some(seed) = crate::dev_env::var!("VELA_SERVICE_ENDPOINTS") {
        let stored = storage::read_value(storage::KEY_SERVICE_ENDPOINTS).ok().flatten();
        match merge_endpoints(stored, &seed) {
            Ok(value) => write(storage::KEY_SERVICE_ENDPOINTS, value),
            Err(why) => vlog!("dev_seed", "VELA_SERVICE_ENDPOINTS skipped: {why}"),
        }
    }
    if let Some(seed) = crate::dev_env::var!("VELA_RPC_OVERRIDES") {
        let stored = storage::read_value(storage::KEY_NETWORK_CONFIG).ok().flatten();
        match merge_rpc_overrides(stored, &seed) {
            Ok(value) => write(storage::KEY_NETWORK_CONFIG, value),
            Err(why) => vlog!("dev_seed", "VELA_RPC_OVERRIDES skipped: {why}"),
        }
    }
    if let Some(relay) = crate::dev_env::var!("VELA_PEERCONNECT_RELAY") {
        let relay = relay.trim();
        if relay.is_empty() {
            vlog!("dev_seed", "VELA_PEERCONNECT_RELAY skipped: empty");
        } else {
            write(PEERCONNECT_KEY, Value::String(relay.to_owned()));
        }
    }
}

fn write(key: &str, value: Value) {
    match storage::write_value(key, value) {
        Ok(()) => vlog!("dev_seed", "{key} seeded for this profile"),
        Err(error) => vlog!("dev_seed", "{key} could not be seeded: {error}"),
    }
}

/// The stored endpoints with the seed's fields laid over them. Only the
/// ledger's five names, each a non-empty string; anything else is refused
/// rather than half-applied.
pub fn merge_endpoints(stored: Option<Value>, seed: &str) -> Result<Value, String> {
    let Value::Object(seed) = serde_json::from_str::<Value>(seed).map_err(|e| e.to_string())?
    else {
        return Err("not a JSON object".to_owned());
    };
    let mut merged = match stored {
        Some(Value::Object(map)) => map,
        _ => Map::new(),
    };
    for (field, value) in seed {
        if !ENDPOINT_FIELDS.contains(&field.as_str()) {
            return Err(format!("unknown endpoint `{field}`"));
        }
        let Some(url) = value.as_str().map(str::trim).filter(|url| !url.is_empty()) else {
            return Err(format!("`{field}` is not a URL"));
        };
        merged.insert(field, Value::String(url.to_owned()));
    }
    Ok(Value::Object(merged))
}

/// The stored per-network settings with each seeded chain's `rpcURL` set —
/// its explorer, bundler and "Only use my node" kept — and a row added, in
/// the stored shape, for a chain that had none.
pub fn merge_rpc_overrides(stored: Option<Value>, seed: &str) -> Result<Value, String> {
    let Value::Object(seed) = serde_json::from_str::<Value>(seed).map_err(|e| e.to_string())?
    else {
        return Err("not a JSON object of chain id → URL".to_owned());
    };
    let mut rows = match stored {
        Some(Value::Array(rows)) => rows,
        _ => Vec::new(),
    };
    for (chain, url) in seed {
        let chain_id: u32 = chain
            .trim()
            .parse()
            .ok()
            .filter(|id| *id > 0)
            .ok_or_else(|| format!("`{chain}` is not a chain id"))?;
        let Some(url) = url.as_str().map(str::trim).filter(|url| !url.is_empty()) else {
            return Err(format!("chain {chain_id}: not a URL"));
        };
        let at = rows
            .iter()
            .position(|row| row.get("chainId").and_then(Value::as_u64) == Some(u64::from(chain_id)));
        match at {
            Some(at) => rows[at]["rpcURL"] = Value::String(url.to_owned()),
            None => rows.push(serde_json::json!({
                "chainId": chain_id,
                "rpcURL": url,
                "explorerURL": "",
                "bundlerURL": "",
            })),
        }
    }
    Ok(Value::Array(rows))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn endpoints_are_laid_over_what_is_stored() {
        let stored = json!({ "fiatRatesURL": "https://rates.example" });
        let merged = merge_endpoints(
            Some(stored),
            r#"{"bundlerServiceURL":"http://127.0.0.1:9190","genesisServiceURL":" http://127.0.0.1:9187 "}"#,
        )
        .unwrap_or_default();
        assert_eq!(merged["fiatRatesURL"], "https://rates.example");
        assert_eq!(merged["bundlerServiceURL"], "http://127.0.0.1:9190");
        assert_eq!(merged["genesisServiceURL"], "http://127.0.0.1:9187");
        assert!(merge_endpoints(None, r#"{"bundler":"x"}"#).is_err(), "an unknown name");
        assert!(merge_endpoints(None, r#"{"bundlerServiceURL":""}"#).is_err());
        assert!(merge_endpoints(None, "[1]").is_err());
        assert!(merge_endpoints(None, "not json").is_err());
    }

    #[test]
    fn an_rpc_override_keeps_the_chains_other_settings() {
        let stored = json!([
            { "chainId": 100, "rpcURL": "https://mine", "explorerURL": "https://scan", "bundlerURL": "", "onlyMine": true },
            { "chainId": 1, "rpcURL": "", "explorerURL": "", "bundlerURL": "" },
        ]);
        let merged = merge_rpc_overrides(
            Some(stored),
            r#"{"100":"http://127.0.0.1:9145","8453":"http://127.0.0.1:9146"}"#,
        )
        .unwrap_or_default();
        let rows = merged.as_array().cloned().unwrap_or_default();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0]["rpcURL"], "http://127.0.0.1:9145");
        assert_eq!(rows[0]["explorerURL"], "https://scan");
        assert_eq!(rows[0]["onlyMine"], true);
        assert_eq!(rows[1]["rpcURL"], "");
        assert_eq!(rows[2]["chainId"], 8453);
        assert_eq!(rows[2]["rpcURL"], "http://127.0.0.1:9146");
        assert!(merge_rpc_overrides(None, r#"{"gnosis":"http://x"}"#).is_err());
        assert!(merge_rpc_overrides(None, r#"{"100":7}"#).is_err());
    }

    /// The whole path, through the store: a seeded profile reads back what
    /// the harness pointed it at.
    #[test]
    fn a_seeded_store_reads_back_as_seeded() {
        crate::executor::storage::tests::with_temp_state("dev-seed", || {
            let endpoints = merge_endpoints(None, r#"{"bundlerServiceURL":"http://127.0.0.1:9190"}"#)
                .unwrap_or_default();
            write(storage::KEY_SERVICE_ENDPOINTS, endpoints);
            write(PEERCONNECT_KEY, Value::String("ws://127.0.0.1:9192".to_owned()));
            let stored = storage::read_value(storage::KEY_SERVICE_ENDPOINTS)
                .ok()
                .flatten()
                .unwrap_or_default();
            assert_eq!(stored["bundlerServiceURL"], "http://127.0.0.1:9190");
            assert_eq!(
                storage::read_value(PEERCONNECT_KEY).ok().flatten(),
                Some(Value::String("ws://127.0.0.1:9192".to_owned()))
            );
        });
    }
}
