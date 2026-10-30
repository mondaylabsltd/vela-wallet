//! The core's small rules the web reads over wasm, for Android and iOS: the
//! read path's clocks and hedging (`rpc_pool`), the fee and gas figures a
//! shell bounds its own reads by, the dApp RPC routing and EIP-5792 answers
//! (`dapp_rpc`), a token's provenance, the chain a site starts on, a typed
//! name's address, and a currency's fraction digits.
//!
//! Each function answers what its wasm twin answers (`rpcOwnOnlyPool`,
//! `dappRpcCapabilities`, …), in the same JSON — the shared vectors
//! (`vela-core/tests/vectors/native-parity/exports.json`) hold every surface
//! to one answer. One table each, so no native copies a number that drifts.

use vela_core::app::{dapp_rpc, rpc_pool};

// ---------------------------------------------------------------------------
// rpc_pool — the read path's clocks and hedging (spec 041, 083 H6, RF2)
// ---------------------------------------------------------------------------

/// "Only use my node" applied to a collected endpoint list —
/// `rpc_pool::own_only_pool`: `seeds_json` is `[{url, source}]`, the answer
/// `{"endpoints": [...], "own_only": bool}`; `""` for a list the core
/// cannot read.
#[uniffi::export]
#[must_use]
pub fn rpc_own_only_pool(seeds_json: String, own_only: bool) -> String {
    let Ok(seeds) = serde_json::from_str::<Vec<rpc_pool::RpcEndpointSeed>>(&seeds_json) else {
        return String::new();
    };
    let (endpoints, own_only) = rpc_pool::own_only_pool(seeds, own_only);
    serde_json::json!({ "endpoints": endpoints, "own_only": own_only }).to_string()
}

/// How long a read waits on one endpoint before the shell also asks the one
/// the pool will ask next, ms — `rpc_pool::HEDGE_AFTER_MS`.
#[uniffi::export]
#[must_use]
pub fn rpc_hedge_after_ms() -> u32 {
    rpc_pool::HEDGE_AFTER_MS
}

/// May this routed call be hedged — `rpc_pool::is_hedged_read`. `kind` is the
/// wire word (`rpc` / `bundler`); anything but `rpc` is not a hedged read.
#[uniffi::export]
#[must_use]
pub fn rpc_is_hedged_read(kind: String, method: String) -> bool {
    kind == "rpc" && rpc_pool::is_hedged_read(rpc_pool::RpcKind::Rpc, &method)
}

/// The answer a hedge's outcome may hand the caller before the pool reaches
/// that endpoint in its own order — `rpc_pool::early_verdict` — as the
/// verdict's JSON, or `""` when it must wait (an outcome that does not read
/// is never an answer). `kind` is `rpc` or `bundler`; `outcome_json` an
/// `RpcTransportOutcome`.
#[uniffi::export]
#[must_use]
pub fn rpc_early_verdict(kind: String, method: String, url: String, outcome_json: String) -> String {
    let kind = match kind.as_str() {
        "rpc" => rpc_pool::RpcKind::Rpc,
        "bundler" => rpc_pool::RpcKind::Bundler,
        _ => return String::new(),
    };
    let Ok(outcome) = serde_json::from_str::<rpc_pool::RpcTransportOutcome>(&outcome_json) else {
        return String::new();
    };
    rpc_pool::early_verdict(kind, &method, &url, &outcome)
        .and_then(|verdict| serde_json::to_string(&verdict).ok())
        .unwrap_or_default()
}

/// The per-endpoint timeout of a chain read, ms — `rpc_pool::RPC_READ_TIMEOUT_MS`.
#[uniffi::export]
#[must_use]
pub fn rpc_read_timeout_ms() -> u32 {
    rpc_pool::RPC_READ_TIMEOUT_MS
}

/// How long an endpoint rests after `consecutive_failures` failures in a
/// row, ms: `30 s · 2^(n−1)`, capped at 300 s, `0` for none —
/// `rpc_pool::cooldown_ms`.
#[uniffi::export]
#[must_use]
pub fn rpc_cooldown_ms(consecutive_failures: u32) -> f64 {
    rpc_pool::cooldown_ms(consecutive_failures)
}

// ---------------------------------------------------------------------------
// Fees and gas
// ---------------------------------------------------------------------------

/// The lowest gas price a chain accepts, as a decimal string: `"0"` on every
/// chain but Arc, which discards an underpriced transaction silently —
/// `fee_policy::min_gas_price_wei`.
#[uniffi::export]
#[must_use]
pub fn min_gas_price_wei(chain_id: u32) -> String {
    vela_core::app::fee_policy::min_gas_price_wei(chain_id).to_string()
}

/// The bound on one whole fee quote, ms — `fee_policy::QUOTE_DEADLINE_MS`
/// (spec 094 S9). A shell bounds the deployment read it makes before a quote
/// by the same figure.
#[uniffi::export]
#[must_use]
pub fn fee_quote_deadline_ms() -> u32 {
    vela_core::app::fee_policy::QUOTE_DEADLINE_MS
}

// ---------------------------------------------------------------------------
// dapp_rpc — routing and EIP-5792 (spec 070, 094, 096)
// ---------------------------------------------------------------------------

/// The core's route for `method`, as JSON (`{"type":"read","bundler":true}`).
#[uniffi::export]
#[must_use]
pub fn dapp_rpc_classify(method: String) -> String {
    serde_json::to_string(&dapp_rpc::classify(&method))
        .unwrap_or_else(|_| "{\"type\":\"unsupported\"}".to_owned())
}

/// EIP-5792 `wallet_getCapabilities` — `dapp_rpc::capabilities` over the
/// request's params, the site's granted addresses and the wallet's chains
/// (all JSON). Answers `{"result":…}` or `{"error":{"code","message"}}`.
#[uniffi::export]
#[must_use]
pub fn dapp_rpc_capabilities(params_json: String, granted_json: String, chains_json: String) -> String {
    let params: serde_json::Value =
        serde_json::from_str(&params_json).unwrap_or(serde_json::Value::Null);
    let granted: Vec<String> = serde_json::from_str(&granted_json).unwrap_or_default();
    let chains: Vec<u32> = serde_json::from_str(&chains_json).unwrap_or_default();
    match dapp_rpc::capabilities(&params, &granted, &chains) {
        Ok(result) => serde_json::json!({ "result": result }).to_string(),
        Err((code, message)) => {
            serde_json::json!({ "error": { "code": code, "message": message } }).to_string()
        }
    }
}

/// EIP-5792 `wallet_getCallsStatus`'s answer for batch `id` on `chain_id`,
/// from the bundler's `eth_getUserOperationReceipt` result JSON (`None` or
/// `null`: not landed) and, with no receipt, the relay's
/// `pimlico_getUserOperationStatus` result JSON — `dapp_rpc::calls_status`,
/// as JSON.
#[uniffi::export]
#[must_use]
pub fn dapp_rpc_calls_status(
    id: String,
    chain_id: u32,
    receipt_json: Option<String>,
    relay_status_json: Option<String>,
) -> String {
    let parse = |json: Option<String>| -> Option<serde_json::Value> {
        json.as_deref()
            .and_then(|json| serde_json::from_str(json).ok())
    };
    let receipt = parse(receipt_json);
    let relay = parse(relay_status_json);
    dapp_rpc::calls_status(&id, chain_id, receipt.as_ref(), relay.as_ref()).to_string()
}

// ---------------------------------------------------------------------------
// Tokens, sites, names, currencies
// ---------------------------------------------------------------------------

/// Who vouches for a token's name on its chain (`app::token_provenance`):
/// `{"trust": "native" | "vouched" | "unverified", "imitates": "USDC" | null}`.
/// `token` empty is the native coin.
#[uniffi::export]
#[must_use]
pub fn token_provenance(chain_id: u32, token: String, symbol: String, native_symbol: String) -> String {
    let token = Some(token.as_str()).filter(|t| !t.trim().is_empty());
    serde_json::to_string(&vela_core::app::token_provenance::token_provenance(
        chain_id,
        token,
        &symbol,
        &native_symbol,
    ))
    .unwrap_or_default()
}

/// The chain a site with no chain of its own starts on (dapp-17):
/// `network_space::site_start_chain` — the space's chain ids in the list's
/// order, and the chain the person last put a site on there (`None`: none).
#[uniffi::export]
#[must_use]
#[allow(clippy::needless_pass_by_value)]
pub fn site_start_chain(space_chain_ids: Vec<u32>, last_used: Option<u32>) -> u32 {
    vela_core::app::network_space::site_start_chain(&space_chain_ids, last_used)
}

/// **The address a typed name points to** (`alice.eth`, a member to add):
/// `name_verify::resolve_step` over JSON — the same forward walk as
/// `verified_name_step`, handing back the address (EIP-55) for the
/// shell to show beside the name before anything uses it. `answers_json` is
/// the transcript (`LookupAnswer[]`); the return a `ResolveStep`.
#[uniffi::export]
#[must_use]
pub fn resolved_name_step(chain_id: u32, registry: String, name: String, answers_json: String) -> String {
    vela_core::app::name_verify::resolve_step_json(chain_id, &registry, &name, &answers_json)
}

/// How many fraction digits a fiat figure in `code` carries
/// (`l10n::currency_fraction_digits`): none for the yen, the won and the
/// other zero-decimal currencies, three for the dinars, two otherwise.
#[uniffi::export]
#[must_use]
pub fn currency_fraction_digits(code: String) -> u32 {
    u32::try_from(vela_core::l10n::currency_fraction_digits(&code)).unwrap_or(2)
}
