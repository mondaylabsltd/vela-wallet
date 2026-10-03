//! The relay (bundler), over HTTP — the REST half and the JSON-RPC half.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/services/bundler-service.ts`,
//! `tx-reconciler.ts` and the bundler RPC calls of `safe-transaction.ts`
//! @ `origin/main` (FR-304). Every *decision* about a fee, a receipt or a
//! rejection is a machine's; what lives here is the wire: which URL, which
//! header, what the JSON said, and the two wording parsers the machines
//! deliberately leave to the shell (`parseBundlerUnderfunded`, the
//! `[existingHash]` marker) plus the retry loop `submitUserOp` always had.
//!
//! ## One relay, chosen by the pool
//!
//! `/v1/account`, `/v1/treasury` and every JSON-RPC call reach the SAME relay
//! the user operation will be submitted to (invariant ③): the base is asked
//! of `rpc_pool` — the routing authority — and only falls back to the built-in
//! host when the pool has nothing eligible. A reimbursement recipient read
//! from one relay and submitted to another is refused (`reimbursed = 0`),
//! which is how the Tempo path once failed on the Expo client.
//!
//! ## Two caches, both short
//!
//! Account info for 30 s and the in-band quote rows for 8 s, exactly the web's
//! TTLs — the quote cache is what makes a fee-coin chip switch free. A
//! transient failure is never cached.

use crate::executor::single_flight::SingleFlight;
use std::collections::HashMap;
use std::io::Read as _;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use vela_core::app::fee_policy::{
    FeeAssetKind, FeeAssetQuote, FeeBundlerQuote, FeeTier, TEMPO_DEFAULT_FEE_TOKEN, is_tempo_chain,
};
use vela_core::app::send::{SendTreasuryAsset, SendTreasuryProbe, SendTreasuryStatus};
use vela_core::app::token_trust::TrustReceiptLog;
use vela_core::app::tx_tracker::{
    TrackLifecycle, TrackStatusAnswer, USER_OP_STATUS_METHOD, parse_user_op_status,
};
use vela_core::safe::ENTRY_POINT;
use vela_core::user_op::{
    GasEstimate, SubmitReply, SubmitStep, SubmitVerdict, UserOperation, parse_hex_quantity,
    submit_step, user_op_to_json,
};

use crate::diag::{short, vlog};
use crate::executor::{pool, proxy, storage};

/// The built-in relay. Per-chain JSON-RPC is `{base}/{chain_id}`; REST is
/// `{base}/v1/…`.
pub const BUILTIN_BASE: &str = "https://vela-relay-cf.getvela.app";

/// `NET_TIMEOUTS.bundlerRest` — the REST account and treasury lookups.
const REST_TIMEOUT: Duration = Duration::from_secs(10);
/// `INFO_CACHE_TTL` (`bundler-service.ts:105`).
const INFO_CACHE_TTL: Duration = Duration::from_secs(30);
/// `NET_TIMEOUTS.bundlerSponsor` — the relay waits up to 15 s for the
/// treasury transfer's receipt before it answers.
const SPONSOR_TIMEOUT: Duration = Duration::from_secs(20);
/// `MIN_BALANCE_WEI`: the threshold when no gas cost was estimated.
const MIN_BALANCE_WEI: u128 = 100_000_000_000_000;
/// `FUNDING_BUFFER_BPS` (150 %): the relay's own volatility buffer.
const FUNDING_BUFFER_BPS: u128 = 15_000;
/// `SILENT_DENY_TTL`: a denial is not asked again for this long.
const SILENT_DENY_TTL: Duration = Duration::from_secs(25);
/// `QUOTE_CACHE_TTL` (`bundler-service.ts:658`).
const QUOTE_CACHE_TTL: Duration = Duration::from_secs(8);

// ---------------------------------------------------------------------------
// Where the relay is
// ---------------------------------------------------------------------------

/// The configured relay host, or the built-in one (`getBuiltinBundlerUrl`).
pub fn builtin_base() -> String {
    storage::read_value(storage::KEY_SERVICE_ENDPOINTS)
        .ok()
        .flatten()
        .and_then(|value| {
            value
                .get("bundlerServiceURL")
                .and_then(Value::as_str)
                .map(|url| url.trim_end_matches('/').to_owned())
        })
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| BUILTIN_BASE.to_owned())
}

/// The REST base of the relay the pool would submit to (`getActiveBundlerBaseUrl`).
/// **Blocks** on the pool thread; call it from a worker.
pub fn base_url(chain_id: u32) -> String {
    pool::bundler_base(chain_id).unwrap_or_else(builtin_base)
}

/// The header the relay reads a chain's RPC from (`vela-relay/src/utils/rpc.rs`).
///
/// Spec 098 §5. Before 081 the apps sent `X-Rpc-Url`, a name the relay never
/// read; 081 removed it as inert, and a network the relay's directory cannot
/// reach could then never be probed, quoted or served. The URL may carry a
/// provider key: ruled acceptable (098 §0.1), and said where it is set.
pub const RELAY_RPC_URL_HEADER: &str = "x-vela-rpc-url";

// ---------------------------------------------------------------------------
// REST
// ---------------------------------------------------------------------------

enum Rest {
    Ok(Value),
    /// The server answered with a non-2xx status.
    Status(u16),
    /// Never reached, timed out, or unreadable.
    Failed,
}

fn rest_get(chain_id: u32, path: &str) -> Rest {
    let url = format!("{}{path}", base_url(chain_id));
    // Spec 098 §5: the relay reads the chain through the RPC named here first —
    // without it a treasury on a network its directory cannot reach is a 503.
    let rpc = pool::best_rpc_url(chain_id);
    //
    // Over the system's routes (spec 082 RD2): a proxy that cannot be
    // reached moves on to the next route, and says so in the log.
    let mut response = match proxy::with_routes(&url, REST_TIMEOUT, |agent| {
        let request = agent.get(&url).header("accept", "application/json");
        match &rpc {
            Some(rpc) => request.header(RELAY_RPC_URL_HEADER, rpc).call(),
            None => request.call(),
        }
    }) {
        Ok(response) => response,
        Err(failure) => {
            return match failure.error {
                ureq::Error::StatusCode(status) => Rest::Status(status),
                _ => Rest::Failed,
            };
        }
    };
    let mut text = String::new();
    if response
        .body_mut()
        .as_reader()
        .read_to_string(&mut text)
        .is_err()
    {
        return Rest::Failed;
    }
    serde_json::from_str::<Value>(&text).map_or(Rest::Failed, Rest::Ok)
}

/// `parseBigIntHex`: a `0x` hex string, a bare hex string, or a JSON number;
/// anything else — including a negative number — is zero.
fn big_hex(value: Option<&Value>) -> u128 {
    match value {
        Some(Value::String(text)) if !text.is_empty() => {
            let clean = text.strip_prefix("0x").unwrap_or(text);
            u128::from_str_radix(clean, 16).unwrap_or(0)
        }
        Some(Value::Number(number)) => number.as_u64().map_or(0, u128::from),
        _ => 0,
    }
}

fn is_address(value: &str) -> bool {
    value.len() == 42
        && value.starts_with("0x")
        && value[2..].bytes().all(|b| b.is_ascii_hexdigit())
}

/// The relay's view of one Safe's gas account on one chain (`BundlerAccountInfo`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountInfo {
    pub chain_id: u32,
    /// The dedicated relay EOA to fund.
    pub deposit_address: String,
    /// Where the in-band reimbursement goes: the relay's treasury when its
    /// vault mode is on, absent on an old relay — callers fall back to
    /// `deposit_address`. Funding flows keep using `deposit_address`.
    pub settlement_recipient: Option<String>,
    pub onchain_balance: u128,
    pub spendable_balance: u128,
    pub status: String,
    pub native_symbol: String,
}

impl AccountInfo {
    /// The reimbursement leg's recipient (`settlementRecipient ?? depositAddress`).
    pub fn fee_recipient(&self) -> Option<String> {
        self.settlement_recipient
            .clone()
            .or_else(|| (!self.deposit_address.is_empty()).then(|| self.deposit_address.clone()))
    }
}

static INFO_CACHE: Mutex<Option<HashMap<String, (AccountInfo, Instant)>>> = Mutex::new(None);

fn cache_key(chain_id: u32, safe: &str) -> String {
    format!("{chain_id}:{}", safe.to_lowercase())
}

/// `GET /v1/account/{chain}/{safe}` (`fetchBundlerAccountInfo`), cached 30 s.
/// `None` for any failure — the machines read that as "no recipient".
pub fn account_info(chain_id: u32, safe: &str) -> Option<AccountInfo> {
    let key = cache_key(chain_id, safe);
    if let Ok(cache) = INFO_CACHE.lock()
        && let Some((info, at)) = cache.as_ref().and_then(|map| map.get(&key))
        && at.elapsed() < INFO_CACHE_TTL
    {
        return Some(info.clone());
    }

    let Rest::Ok(data) = rest_get(
        chain_id,
        &format!("/v1/account/{chain_id}/{}", safe.to_lowercase()),
    ) else {
        return None;
    };

    let deposit_address = data
        .get("activeDepositAddress")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned();
    let mut onchain_balance = big_hex(data.get("onchainBalance"));
    let mut spendable_balance = big_hex(data.get("spendableBalance"));
    let mut native_symbol = native_symbol(chain_id);

    // Tempo has no native coin: the gas account's pathUSD balance, scaled to
    // 18 decimals so the wei-based funding UI renders the USD value.
    if is_tempo_chain(chain_id) && !deposit_address.is_empty() {
        let call = format!(
            "0x70a08231000000000000000000000000{}",
            deposit_address.trim_start_matches("0x").to_lowercase()
        );
        if let Ok(body) = pool::call(
            chain_id,
            "eth_call",
            json!([{ "to": TEMPO_DEFAULT_FEE_TOKEN, "data": call }, "latest"]),
        ) {
            let path6 = big_hex(body.get("result"));
            onchain_balance = path6.saturating_mul(1_000_000_000_000);
            spendable_balance = onchain_balance;
            native_symbol = "pathUSD".to_owned();
        }
    }

    // A corrupted field degrades to the deposit-address fallback; it must
    // never poison the fee leg.
    let settlement_recipient = data
        .get("settlementRecipient")
        .and_then(Value::as_str)
        .filter(|value| is_address(value))
        .map(str::to_owned);

    let info = AccountInfo {
        chain_id,
        deposit_address,
        settlement_recipient,
        onchain_balance,
        spendable_balance,
        status: data
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("UNKNOWN")
            .to_owned(),
        native_symbol,
    };
    if let Ok(mut cache) = INFO_CACHE.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(key, (info.clone(), Instant::now()));
    }
    Some(info)
}

/// `clearBundlerCache`: one Safe's entry, or every entry for the chain.
pub fn clear_cache(chain_id: u32, safe: Option<&str>) {
    if let Ok(mut cache) = INFO_CACHE.lock()
        && let Some(map) = cache.as_mut()
    {
        match safe {
            Some(safe) => {
                map.remove(&cache_key(chain_id, safe));
            }
            None => map.retain(|key, _| !key.starts_with(&format!("{chain_id}:"))),
        }
    }
    if let Ok(mut cache) = QUOTE_CACHE.lock()
        && let Some(map) = cache.as_mut()
    {
        map.retain(|key, _| !key.starts_with(&format!("{chain_id}:")));
    }
}

fn native_symbol(chain_id: u32) -> String {
    vela_core::app::network_admin::BUILTIN_CHAINS
        .iter()
        .find(|chain| chain.chain_id == chain_id)
        .map_or_else(|| "ETH".to_owned(), |chain| chain.native_symbol.to_owned())
}

/// `GET /v1/treasury/{chain}` (`probeTreasury`), four ways: 404 is the relay
/// saying it does not serve this chain; any other non-2xx, a timeout or a
/// malformed body is TRANSIENT and answers `Unknown` — never `Uncovered`.
pub fn probe_treasury(chain_id: u32) -> SendTreasuryProbe {
    let data = match rest_get(chain_id, &format!("/v1/treasury/{chain_id}")) {
        Rest::Ok(data) => data,
        Rest::Status(404) => return SendTreasuryProbe::Uncovered,
        Rest::Status(_) | Rest::Failed => return SendTreasuryProbe::Unknown,
    };
    let Some(address) = data
        .get("address")
        .and_then(Value::as_str)
        .filter(|value| is_address(value))
    else {
        return SendTreasuryProbe::Unknown;
    };
    let status = SendTreasuryStatus {
        chain_id,
        address: address.to_owned(),
        asset: if data.get("asset").and_then(Value::as_str) == Some("pathUSD") {
            SendTreasuryAsset::PathUsd
        } else {
            SendTreasuryAsset::Native
        },
        balance: big_hex(data.get("balance")).to_string(),
        floor: big_hex(data.get("floor")).to_string(),
        bootstrap_needed: data.get("bootstrapNeeded") == Some(&Value::Bool(true)),
        // The CORE decides whether this is a network Vela ships, and therefore
        // whose relayer the operator owns. This executor reports the probe; it
        // does not judge it (vela-wallet spec 060).
        operator_served: false,
    };
    if status.bootstrap_needed {
        SendTreasuryProbe::LowFloat { status }
    } else {
        SendTreasuryProbe::Covered
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC, through the pool
// ---------------------------------------------------------------------------

static QUOTE_CACHE: Mutex<Option<HashMap<String, (Vec<FeeAssetQuote>, Instant)>>> =
    Mutex::new(None);

/// A decimal-looking string or number, kept as text so conversion never
/// loses precision (`parseDecimalString`).
fn decimal_text(value: Option<&Value>) -> Option<String> {
    let raw = match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Number(number)) => number.to_string(),
        _ => return None,
    };
    let parsed: f64 = raw.trim().parse().ok()?;
    (parsed.is_finite() && parsed >= 0.0).then_some(raw)
}

/// One `vela_getInBandGasQuote` row, or nothing if it is not well-formed.
fn parse_quote_row(row: &Value) -> Option<FeeAssetQuote> {
    let recipient = row
        .get("recipient")
        .and_then(Value::as_str)
        .filter(|value| is_address(value))?;
    let asset = match row.get("asset").and_then(Value::as_str) {
        Some("native") => FeeAssetKind::Native,
        Some("erc20") => FeeAssetKind::Erc20,
        _ => return None,
    };
    let fee_token = row
        .get("feeToken")
        .and_then(Value::as_str)
        .filter(|value| is_address(value))
        .map(str::to_owned);
    let balance = big_hex(row.get("balance"));
    let decimals = row
        .get("decimals")
        .and_then(Value::as_u64)
        .and_then(|d| u32::try_from(d).ok())?;
    let symbol = row
        .get("symbol")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())?;
    // USD values are conversion metadata: native gas prices from gas units
    // alone, a stablecoin needs its own price or it cannot be converted.
    let usd_balance = decimal_text(row.get("usdBalance")).unwrap_or_else(|| "0".to_owned());
    let usd_price = decimal_text(row.get("usdPrice"));
    if asset == FeeAssetKind::Erc20 && (fee_token.is_none() || usd_price.is_none()) {
        return None;
    }
    Some(FeeAssetQuote {
        recipient: recipient.to_owned(),
        asset,
        fee_token: if asset == FeeAssetKind::Erc20 {
            fee_token
        } else {
            None
        },
        balance: balance.to_string(),
        decimals,
        symbol: symbol.to_owned(),
        usd_balance,
        usd_price,
        // Issue 682's floor price is a WEB seam so far: this shell has no
        // balances feed to read a native price out of yet, and `None` is
        // exactly the pre-682 answer (the blind 0.001-coin floor), so
        // desktop's behaviour is unchanged rather than half-changed.
        native_usd_floor_price: None,
    })
}

/// Every in-band fee asset in one call (`fetchInBandGasQuotes`), cached 8 s.
/// `None` for unavailable, malformed or unsupported — the core's
/// `missing_quote_failure` decides what that means.
pub fn in_band_quotes(chain_id: u32, safe: &str) -> Option<Vec<FeeAssetQuote>> {
    let key = cache_key(chain_id, safe);
    if let Ok(cache) = QUOTE_CACHE.lock()
        && let Some((quotes, at)) = cache.as_ref().and_then(|map| map.get(&key))
        && at.elapsed() < QUOTE_CACHE_TTL
    {
        return Some(quotes.clone());
    }
    // Every speed's session asks for these rows at the same instant: one
    // request answers them all (`single_flight`).
    static IN_FLIGHT: SingleFlight<String, Option<Vec<FeeAssetQuote>>> = SingleFlight::new();
    IN_FLIGHT.run(key.clone(), || read_in_band_quotes(chain_id, safe, key))
}

fn read_in_band_quotes(chain_id: u32, safe: &str, key: String) -> Option<Vec<FeeAssetQuote>> {
    let body = pool::bundler_call(
        chain_id,
        "vela_getInBandGasQuote",
        json!([{ "safeAddress": safe }]),
    )
    .ok()?;
    if body.get("error").is_some() {
        return None;
    }
    let rows = body.get("result").and_then(Value::as_array)?;
    let quotes: Vec<FeeAssetQuote> = rows.iter().filter_map(parse_quote_row).collect();
    // No native USD price: keep the payable native row, drop every stablecoin
    // whose amount could not be converted safely.
    let native_unpriced = quotes
        .iter()
        .find(|quote| quote.asset == FeeAssetKind::Native)
        .is_some_and(|native| native.usd_price.is_none());
    let usable: Vec<FeeAssetQuote> = if native_unpriced {
        quotes
            .into_iter()
            .filter(|quote| quote.asset == FeeAssetKind::Native)
            .collect()
    } else {
        quotes
    };
    if usable.is_empty() {
        return None;
    }
    if let Ok(mut cache) = QUOTE_CACHE.lock() {
        cache
            .get_or_insert_with(HashMap::new)
            .insert(key, (usable.clone(), Instant::now()));
    }
    Some(usable)
}

fn tier_key(tier: FeeTier) -> &'static str {
    match tier {
        FeeTier::Slow => "slow",
        FeeTier::Standard => "standard",
        FeeTier::Rapid => "rapid",
        FeeTier::Fast => "fast",
    }
}

fn decimal_of_hex(value: Option<&Value>) -> Option<String> {
    let text = value?.as_str()?;
    parse_hex_quantity(Some(text)).ok().map(|n| n.to_string())
}

/// One tier of `pimlico_getUserOperationGasPrice`, UNJUDGED (`fetchRawBundlerQuote`):
/// a zero quote is handed over as zero, because refusing it is the core's rule.
pub fn raw_bundler_quote(chain_id: u32, tier: FeeTier) -> Option<FeeBundlerQuote> {
    raw_bundler_quotes(chain_id)?
        .into_iter()
        .find_map(|(key, quote)| (key == tier_key(tier)).then_some(quote))
}

/// Every tier of `pimlico_getUserOperationGasPrice` from ONE call, keyed by
/// the relay's tier name — the answer names slow, standard and fast together,
/// so the three speed rows never need three requests. `None` when the relay
/// did not answer; a tier row it did not publish is simply absent.
pub fn raw_bundler_quotes(chain_id: u32) -> Option<Vec<(&'static str, FeeBundlerQuote)>> {
    let body = pool::bundler_call(chain_id, "pimlico_getUserOperationGasPrice", json!([])).ok()?;
    if body.get("error").is_some() {
        return None;
    }
    let result = body.get("result")?;
    Some(
        [
            FeeTier::Slow,
            FeeTier::Standard,
            FeeTier::Fast,
            FeeTier::Rapid,
        ]
        .into_iter()
        .filter_map(|tier| {
            let row = result.get(tier_key(tier))?;
            Some((
                tier_key(tier),
                FeeBundlerQuote {
                    max_fee_per_gas: decimal_of_hex(row.get("maxFeePerGas"))?,
                    // The tip this tier is actually signed with — the half
                    // of the quote that buys priority, and what the core
                    // turns into the per-tier gas price on screen (issue
                    // 684). Absent on a generic bundler.
                    max_priority_fee_per_gas: decimal_of_hex(row.get("maxPriorityFeePerGas")),
                    network_fee_per_gas: decimal_of_hex(row.get("networkFeePerGas")),
                    relayer_fee_per_gas: decimal_of_hex(row.get("relayerFeePerGas")),
                },
            ))
        })
        .collect(),
    )
}

/// The relay's name for a tier, as `raw_bundler_quotes` keys its rows.
pub fn tier_name(tier: FeeTier) -> &'static str {
    tier_key(tier)
}

/// Why the relay gave no gas estimate: its words for the log, and the
/// JSON-RPC `error` member as it came — what the core's
/// `user_op::estimate_failure` reads to tell "the call reverts" from "no
/// answer" (spec 082 RJ19). `error_json` is `None` when nothing answered.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EstimateError {
    pub message: String,
    pub error_json: Option<String>,
}

impl EstimateError {
    fn unanswered(message: String) -> Self {
        Self {
            message,
            error_json: None,
        }
    }

    /// The relay ANSWERED that the operation fails when it runs — its
    /// simulation failed, or the execution reverted (spec 083 fee): the fee
    /// machine's `FeeGasOutcome::Refused`, never "cannot reach Vela".
    #[must_use]
    pub fn refuses(&self) -> bool {
        self.error_json.is_some() && is_simulation_refusal(&self.message)
    }

    /// The core's reading of it (RJ19): `Reverts` only on the relay's own
    /// word, never on a missing answer.
    #[must_use]
    pub fn classified(&self) -> vela_core::user_op::EstimateFailure {
        self.error_json.as_deref().map_or(
            vela_core::user_op::EstimateFailure::Unavailable,
            vela_core::user_op::estimate_failure,
        )
    }
}

impl std::fmt::Display for EstimateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The relay's error says the operation itself fails when it runs — its
/// simulation failed, or the execution reverted — rather than that the relay
/// is busy, limited or broken. Only those words are an answer about the
/// operation; anything else stays an unexplained failure, asked again on
/// the growing wait as it always was.
///
/// An EntryPoint validation code (`AA10`…`AA3x`: "AA23 reverted (or OOG)",
/// "AA25 invalid account nonce") is never one of them, whatever words wrap
/// it. It is the account's own check — its signature, nonce or prefund — and
/// nothing another fee coin changes; a nonce that moved on can clear by the
/// next ask. Spec 083 fee review.
fn is_simulation_refusal(message: &str) -> bool {
    if names_validation_code(message) {
        return false;
    }
    let message = message.to_ascii_lowercase();
    message.contains("simulation failed")
        || message.contains("execution reverted")
        || message.contains("reverted during simulation")
}

/// `message` carries an EntryPoint code — `AA` and two digits, standing as a
/// word of its own (never the inside of an address or of hex data).
fn names_validation_code(message: &str) -> bool {
    let bytes = message.as_bytes();
    bytes.windows(4).enumerate().any(|(at, window)| {
        window[0] == b'A'
            && window[1] == b'A'
            && window[2].is_ascii_digit()
            && window[3].is_ascii_digit()
            && (at == 0 || !bytes[at - 1].is_ascii_alphanumeric())
            && bytes
                .get(at + 4)
                .is_none_or(|next| !next.is_ascii_alphanumeric())
    })
}

/// `eth_estimateUserOperationGas` — the relay's raw limits, or why not.
pub fn estimate_user_op_gas(
    op: &UserOperation,
    chain_id: u32,
) -> Result<GasEstimate, EstimateError> {
    let body = pool::bundler_call(
        chain_id,
        "eth_estimateUserOperationGas",
        json!([user_op_to_json(op, &[]), ENTRY_POINT]),
    )
    .map_err(|error| EstimateError::unanswered(format!("gas estimation unreachable: {error:?}")))?;
    estimate_of(&body)
}

/// One estimate body read: the limits, or the relay's error member kept
/// whole for the core.
fn estimate_of(body: &Value) -> Result<GasEstimate, EstimateError> {
    if let Some(error) = body.get("error").filter(|error| !error.is_null()) {
        return Err(EstimateError {
            message: error
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("Gas estimation failed")
                .to_owned(),
            error_json: Some(error.to_string()),
        });
    }
    let result = body
        .get("result")
        .filter(|value| value.is_object())
        .ok_or_else(|| {
            EstimateError::unanswered("Failed to estimate gas — empty result".to_owned())
        })?;
    let field = |name: &str| {
        parse_hex_quantity(result.get(name).and_then(Value::as_str))
            .map_err(|e| EstimateError::unanswered(e.to_string()))
    };
    Ok(GasEstimate {
        verification_gas_limit: field("verificationGasLimit")?,
        call_gas_limit: field("callGasLimit")?,
        pre_verification_gas: field("preVerificationGas")?,
    })
}

/// `[userOperation, entryPoint, tier?]` — the relay's wire since it learned
/// about speed. Two elements when no tier is named, which is the pre-068 wire
/// exactly; never the dead `rapid`.
fn submit_params(dict: Value, tier: Option<FeeTier>) -> Value {
    match tier.and_then(vela_core::app::fee_speed::wire_tier) {
        Some(tier) => json!([dict, ENTRY_POINT, tier_key(tier)]),
        None => json!([dict, ENTRY_POINT]),
    }
}

// ---------------------------------------------------------------------------
// A submit in flight (spec 082 RD14, W17)
// ---------------------------------------------------------------------------

/// Relay POSTs of `eth_sendUserOperation` running right now. The window's
/// close and the Quit action read it: quitting mid-POST is how an operation
/// ends up on chain with no record and no tracker behind it.
static SUBMITS_IN_FLIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// Counted while alive; the drop is what un-counts it, so an early return or
/// a panic on the submit path can never leave the window unable to close.
pub struct SubmitGuard(());

impl SubmitGuard {
    #[must_use]
    pub fn enter() -> Self {
        SUBMITS_IN_FLIGHT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Self(())
    }
}

impl Drop for SubmitGuard {
    fn drop(&mut self) {
        SUBMITS_IN_FLIGHT.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
    }
}

/// Is a submit POST running?
pub fn submit_in_flight() -> bool {
    SUBMITS_IN_FLIGHT.load(std::sync::atomic::Ordering::SeqCst) > 0
}

/// How long a held close stays held: a second close inside it goes through.
pub const CLOSE_AGAIN_WITHIN: Duration = Duration::from_secs(5);

/// When the last close was held, while a submit ran.
static CLOSE_HELD_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);

/// Whether a close (the window's, or Quit) goes through now (spec 082 RD14,
/// W17): refused ONCE while a submit POST is in flight — an operation that
/// may land needs its record and its tracker, and quitting mid-POST is how it
/// ends up on chain with neither — and let through by a second close within
/// [`CLOSE_AGAIN_WITHIN`]: the person always gets the last word.
#[must_use]
pub fn may_close(now: std::time::Instant) -> bool {
    let mut held = CLOSE_HELD_AT
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let (allow, next) = close_verdict(submit_in_flight(), *held, now);
    *held = next;
    if !allow {
        vlog!("window", "close held (submit in flight)");
    }
    allow
}

/// The rule, with its state passed in: `(may close, the held stamp to keep)`.
fn close_verdict(
    in_flight: bool,
    held_at: Option<std::time::Instant>,
    now: std::time::Instant,
) -> (bool, Option<std::time::Instant>) {
    if !in_flight {
        return (true, None);
    }
    match held_at {
        Some(at) if now.saturating_duration_since(at) <= CLOSE_AGAIN_WITHIN => (true, None),
        _ => (false, Some(now)),
    }
}

/// One routed reply as the core's `SubmitReply`: the result hash, the
/// JSON-RPC `error` member as JSON text, or no answer at all.
fn reply_of(answer: &Result<Value, pool::PoolError>) -> SubmitReply {
    let Ok(body) = answer else {
        return SubmitReply::NoAnswer;
    };
    if let Some(error) = body.get("error").filter(|error| !error.is_null()) {
        return SubmitReply::Error(error.to_string());
    }
    match body.get("result") {
        Some(Value::String(hash)) => SubmitReply::Hash(hash.clone()),
        // An answer nobody can read — the core says "may have been sent".
        Some(other) => SubmitReply::Hash(other.to_string()),
        None => SubmitReply::Hash(String::new()),
    }
}

/// The submit loop (spec 082 RA1): POST, fold the pool's delivery bit into
/// the running OR, ask `submit_step`, and either wait and POST the identical
/// operation again or stop with the core's verdict. `post` is the pool call,
/// a seam so the loop runs against a fake relay in the tests.
///
/// Every rule — the busy retry, "an `[existingHash]` is accepted", "a lost
/// reply after any POST is may-have-been-sent", "refused with nothing
/// delivered is not sent" — is `submit_step`'s. This decides nothing.
fn submit_loop(
    local_hash: &str,
    mut post: impl FnMut() -> pool::Routed,
    mut sleep: impl FnMut(Duration),
) -> (SubmitVerdict, u32) {
    let mut maybe_delivered = false;
    let mut attempt: u32 = 0;
    loop {
        let routed = post();
        maybe_delivered |= routed.maybe_delivered;
        match submit_step(
            &reply_of(&routed.answer),
            attempt,
            maybe_delivered,
            local_hash,
        ) {
            SubmitStep::Done(verdict) => return (verdict, attempt + 1),
            SubmitStep::RetryAfter { delay_ms } => {
                vlog!("relay", "busy, retry {} in {delay_ms} ms", attempt + 1);
                sleep(Duration::from_millis(u64::from(delay_ms)));
                attempt += 1;
            }
        }
    }
}

/// `eth_sendUserOperation`, to its end: the core's verdict on it (spec 082
/// RA1, ruling 1). `local_hash` is the operation's hash computed before the
/// first POST, followed when the relay's reply is lost.
///
/// `tier` is the speed the displayed fee was priced at, sent as the optional
/// third parameter (spec 068's relay contract, on the desktop since 069). It
/// is a NAME, never a wei figure: the relay resolves it against the base fee
/// it reads at submit time and clamps the result between its inclusion floor
/// and what the signed reimbursement funds. `None` sends the pre-068
/// two-element params exactly. The dead `rapid` never gets here — the core
/// drops it from the quoted fee — and is dropped again below regardless,
/// because a relay refuses an unknown name with -32602 before any handler
/// runs.
pub fn send_user_op(
    op: &UserOperation,
    chain_id: u32,
    extra: &[(&str, &str)],
    tier: Option<FeeTier>,
    local_hash: &str,
) -> SubmitVerdict {
    let params = submit_params(user_op_to_json(op, extra), tier);
    // Never the sender: an address is not a log line (FR-019).
    vlog!(
        "relay",
        "submitting chain={chain_id} op={} initCode={} callData={}B",
        short(local_hash),
        if op.init_code.len() >= 20 {
            "yes"
        } else {
            "no"
        },
        op.call_data.len(),
    );
    let _in_flight = SubmitGuard::enter();
    let started = std::time::Instant::now();
    let (verdict, attempts) = submit_loop(
        local_hash,
        || pool::bundler_routed(chain_id, "eth_sendUserOperation", params.clone()),
        std::thread::sleep,
    );
    let (name, hash) = match &verdict {
        SubmitVerdict::Accepted { user_op_hash } => ("accepted", user_op_hash.as_str()),
        SubmitVerdict::MaybeSent { user_op_hash } => ("maybe_sent", user_op_hash.as_str()),
        SubmitVerdict::NotSent { .. } => ("not_sent", local_hash),
    };
    vlog!(
        "relay",
        "submit verdict={name} hash={} attempts={attempts} in={} ms",
        short(hash),
        started.elapsed().as_millis()
    );
    if let SubmitVerdict::Accepted { user_op_hash } = &verdict
        && !user_op_hash.eq_ignore_ascii_case(local_hash)
    {
        // The relay's hash wins; the difference is worth a line.
        vlog!(
            "relay",
            "userop.hash_mismatch relay={} local={}",
            short(user_op_hash),
            short(local_hash)
        );
    }
    verdict
}

/// A definitive receipt (`UserOpResolution`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Resolution {
    /// `success !== false`.
    pub confirmed: bool,
    pub tx_hash: String,
    /// The sender the receipt names — what `token_trust` admits tokens FOR.
    pub sender: Option<String>,
    /// The AUTHENTIC logs, kept only with the three fields the netting reads.
    pub logs: Vec<TrustReceiptLog>,
}

/// One `eth_getUserOperationReceipt` (`requestUserOpReceipt`'s classification):
/// the relay could not answer (`reached_bundler: false` — NEVER a failure), it
/// answered with nothing yet, or it answered definitively.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReceiptPoll {
    pub reached_bundler: bool,
    pub resolution: Option<Resolution>,
}

pub(crate) fn to_trust_log(log: &Value) -> Option<TrustReceiptLog> {
    let address = log.get("address")?.as_str()?;
    let topics = log.get("topics")?.as_array()?;
    Some(TrustReceiptLog {
        address: address.to_owned(),
        topics: topics
            .iter()
            .filter_map(|topic| topic.as_str().map(str::to_owned))
            .collect(),
        data: log
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or("0x")
            .to_owned(),
    })
}

pub fn user_op_receipt(user_op_hash: &str, chain_id: u32) -> ReceiptPoll {
    receipt_poll(user_op_hash, |params| {
        pool::bundler_call(chain_id, "eth_getUserOperationReceipt", params)
    })
}

/// [`user_op_receipt`], answered within `budget` (spec 079): the dApp's
/// receipt wait gives each poll only what is left of its window. Past the
/// budget — or once `stop` says the answer is no longer wanted (spec 082
/// RJ4: the core answered the page from the tracker) — the relay counts as
/// not reached, never as a failure.
pub fn user_op_receipt_until(
    user_op_hash: &str,
    chain_id: u32,
    budget: std::time::Duration,
    stop: &dyn Fn() -> bool,
) -> ReceiptPoll {
    receipt_poll(user_op_hash, |params| {
        pool::bundler_call_until(
            chain_id,
            "eth_getUserOperationReceipt",
            params,
            budget,
            stop,
        )
    })
}

fn receipt_poll(
    user_op_hash: &str,
    call: impl FnOnce(Value) -> Result<Value, pool::PoolError>,
) -> ReceiptPoll {
    let unreachable = ReceiptPoll {
        reached_bundler: false,
        resolution: None,
    };
    if user_op_hash.is_empty() {
        return unreachable;
    }
    let Ok(body) = call(json!([user_op_hash])) else {
        return unreachable;
    };
    if body.get("error").is_some() {
        return unreachable;
    }
    let pending = ReceiptPoll {
        reached_bundler: true,
        resolution: None,
    };
    let Some(result) = body.get("result").filter(|value| value.is_object()) else {
        return pending;
    };
    let Some(tx_hash) = result
        .pointer("/receipt/transactionHash")
        .and_then(Value::as_str)
    else {
        return pending;
    };
    let logs = result
        .pointer("/receipt/logs")
        .and_then(Value::as_array)
        .map(|logs| logs.iter().filter_map(to_trust_log).collect())
        .unwrap_or_default();
    ReceiptPoll {
        reached_bundler: true,
        resolution: Some(Resolution {
            confirmed: result.get("success") != Some(&Value::Bool(false)),
            tx_hash: tx_hash.to_owned(),
            sender: result
                .get("sender")
                .and_then(Value::as_str)
                .map(str::to_owned),
            logs,
        }),
    }
}

/// The relay's status for an op with no receipt (spec 082 RA7, G13): the
/// core's method name and the core's parser — the desktop asked for
/// `eth_getUserOperationStatus`, which the relay does not serve, and read
/// every -32601 as "unavailable". `None` for an error, an unreachable relay
/// or a status the core does not know.
pub fn user_op_status(user_op_hash: &str, chain_id: u32) -> Option<TrackStatusAnswer> {
    if user_op_hash.is_empty() {
        return None;
    }
    let answer = pool::bundler_call(chain_id, USER_OP_STATUS_METHOD, json!([user_op_hash]));
    let status = match &answer {
        Ok(body) => parse_user_op_status(&body.to_string()),
        Err(_) => None,
    };
    match &status {
        Some(found) => vlog!(
            "tracker",
            "op={} status={:?}{}",
            short(user_op_hash),
            found.status,
            found
                .stage
                .as_deref()
                .map(|stage| format!(" stage={stage}"))
                .unwrap_or_default()
        ),
        None => vlog!("tracker", "op={} status unavailable", short(user_op_hash)),
    }
    status
}

/// [`user_op_status`] within `budget` — the dApp's landing wait asks it for
/// a relay that refused the operation after accepting it (083). The relay's
/// own method and the core's parser (spec 082 RA7).
pub fn user_op_status_within(
    user_op_hash: &str,
    chain_id: u32,
    budget: std::time::Duration,
) -> Option<(TrackLifecycle, Option<String>)> {
    if user_op_hash.is_empty() {
        return None;
    }
    let body = pool::bundler_call_within(
        chain_id,
        USER_OP_STATUS_METHOD,
        json!([user_op_hash]),
        budget,
    )
    .ok()?;
    let answer = parse_user_op_status(&body.to_string())?;
    Some((answer.status, answer.stage))
}

/// `eth_getUserOperationByHash` within `budget`: the hash of the transaction
/// that carried the operation, once the bundler reports one (083). `None`
/// while it is pending — ERC-7769 answers `transactionHash: null` then —
/// and for an unknown op, an error or a relay that could not be reached.
/// The relay has answered this method since before 083 (`null` for an
/// unknown hash, probed 2026-09-28), which is why the landing wait may ask
/// it when the receipt has not come.
pub fn user_op_transaction_within(
    user_op_hash: &str,
    chain_id: u32,
    budget: std::time::Duration,
) -> Option<String> {
    if user_op_hash.is_empty() {
        return None;
    }
    let body = pool::bundler_call_within(
        chain_id,
        "eth_getUserOperationByHash",
        json!([user_op_hash]),
        budget,
    )
    .ok()?;
    transaction_of(&body)
}

/// The bundle transaction an `eth_getUserOperationByHash` answer names.
fn transaction_of(body: &Value) -> Option<String> {
    let hash = body.get("result")?.get("transactionHash")?.as_str()?;
    let digits = hash.strip_prefix("0x")?;
    (digits.len() == 64
        && digits.bytes().all(|b| b.is_ascii_hexdigit())
        && digits.bytes().any(|b| b != b'0'))
    .then(|| hash.to_owned())
}

// ---------------------------------------------------------------------------
// The wording layer the machines leave to the shell
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// The gas account: is it funded, and will the treasury fund it (078 W-05)
// ---------------------------------------------------------------------------

/// The web's `vela.forceFunding()`, as a seam: `VELA_FORCE_FUNDING=1` reads
/// every gas account as short and every sponsorship as denied, so the
/// funding card can be exercised without draining a real account. Developer
/// builds only — the web's `fundingShouldForce` is always false in production.
fn funding_forced() -> bool {
    crate::dev_env::var_os!("VELA_FORCE_FUNDING").is_some()
}

/// `recommendedFundingWei`: lift the gas account to the threshold, plus the
/// buffer — or, when it is already there, the buffered threshold itself.
pub fn recommended_funding_wei(threshold: u128, current: u128) -> u128 {
    let deficit = threshold.saturating_sub(current);
    let base = if deficit > 0 { deficit } else { threshold };
    base.saturating_mul(FUNDING_BUFFER_BPS) / 10_000
}

/// A gas account that cannot cover the operation (`FundingNeeded`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FundingNeeded {
    pub deposit_address: String,
    pub safe_address: String,
    pub chain_id: u32,
    pub native_symbol: String,
    pub threshold_wei: u128,
    pub recommended_wei: u128,
    pub current_balance: u128,
}

/// `checkBundlerFunding`: `None` when the account covers `cost` (or the
/// minimum), when a user-set bundler is in use, and when the relay cannot be
/// reached — an unreadable pre-check lets the attempt proceed, and the
/// submit's own "underfunded" answer is the authority. **Blocks.**
pub fn check_funding(chain_id: u32, safe: &str, cost: Option<u128>) -> Option<FundingNeeded> {
    if !pool::uses_builtin_bundler(chain_id) {
        return None;
    }
    let info = account_info(chain_id, safe)?;
    let threshold = cost.unwrap_or(MIN_BALANCE_WEI);
    if !funding_forced() && info.spendable_balance >= threshold {
        return None;
    }
    Some(FundingNeeded {
        deposit_address: info.deposit_address,
        safe_address: safe.to_owned(),
        chain_id,
        native_symbol: info.native_symbol,
        threshold_wei: threshold,
        recommended_wei: recommended_funding_wei(threshold, info.spendable_balance),
        current_balance: info.spendable_balance,
    })
}

/// What a silent sponsorship came to (`SilentSponsorship`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Sponsorship {
    /// The gas account is usable now.
    Funded,
    /// Money is (probably) on its way; the balance read has not caught up.
    Confirming,
    Denied {
        reason: Option<String>,
    },
}

type Denials = HashMap<String, (Option<String>, Instant)>;
static SILENT_DENIALS: Mutex<Option<Denials>> = Mutex::new(None);

/// `attemptSilentSponsorship`: ask the treasury to fund the gas account and
/// re-read the balance. The ONLY automatic treasury touchpoint, and it runs
/// only when somebody is about to transact. A denial is remembered for 25 s
/// unless `force`; a grant or a maybe-grant never is — balances move.
/// **Blocks** for up to the sponsor timeout.
pub fn attempt_sponsorship(funding: &FundingNeeded, force: bool) -> Sponsorship {
    if funding_forced() {
        return Sponsorship::Denied { reason: None };
    }
    let (chain_id, safe) = (funding.chain_id, funding.safe_address.as_str());
    let key = cache_key(chain_id, safe);
    if !force
        && let Ok(denials) = SILENT_DENIALS.lock()
        && let Some((reason, at)) = denials.as_ref().and_then(|map| map.get(&key))
        && at.elapsed() < SILENT_DENY_TTL
    {
        return Sponsorship::Denied {
            reason: reason.clone(),
        };
    }
    let denials = |remember: Option<Option<String>>| {
        if let Ok(mut denials) = SILENT_DENIALS.lock() {
            let map = denials.get_or_insert_with(HashMap::new);
            match remember {
                Some(reason) => {
                    map.insert(key.clone(), (reason, Instant::now()));
                }
                None => {
                    map.remove(&key);
                }
            }
        }
    };

    let (sponsored, reason) = request_sponsorship(chain_id, safe, funding.threshold_wei);
    if sponsored || reason.as_deref() == Some("already_funded") {
        // The relay waits for the transfer's receipt before answering, so the
        // money is normally there — a lagging read is "confirming", NEVER a
        // denial (the old flow showed a granted sponsorship as refused).
        denials(None);
        clear_cache(chain_id, Some(safe));
        let covered = account_info(chain_id, safe)
            .is_some_and(|info| info.spendable_balance >= funding.threshold_wei);
        return if covered {
            Sponsorship::Funded
        } else {
            Sponsorship::Confirming
        };
    }
    if matches!(
        reason.as_deref(),
        Some("pending_unknown" | "already_in_progress")
    ) {
        // A timeout mid-transfer, or another grant in flight: money may be
        // arriving, and a second request could pay twice.
        denials(None);
        return Sponsorship::Confirming;
    }
    denials(Some(reason.clone()));
    Sponsorship::Denied { reason }
}

/// `POST /v1/sponsor/{chain}/{safe}` (`requestSponsorship`): whether the
/// treasury paid, and the relay's reason when it did not. A non-2xx keeps the
/// server's own reason when it gave one — a 503 `passkey_index_unavailable`
/// is "try later", not a refusal. A timeout is `pending_unknown`: the
/// transfer may have gone through unseen. **Blocks.**
fn request_sponsorship(chain_id: u32, safe: &str, required_wei: u128) -> (bool, Option<String>) {
    let safe = safe.to_lowercase();
    let url = format!("{}/v1/sponsor/{chain_id}/{safe}", base_url(chain_id));
    let required = format!("0x{required_wei:x}");
    // A treasury transfer is not idempotent: a timeout-then-retry must not
    // pay twice, and a relay that honours the key collapses the two.
    let idempotency = format!("sponsor:{chain_id}:{safe}:{required}");
    let body = json!({ "requiredWei": required });
    let rpc = pool::best_rpc_url(chain_id);
    let answer = proxy::with_routes(&url, SPONSOR_TIMEOUT, |agent| {
        let request = agent
            .post(&url)
            .config()
            .http_status_as_error(false)
            .build()
            .header("accept", "application/json")
            .header("idempotency-key", &idempotency);
        // Spec 098 §5, as on every other request to the relay.
        let request = match &rpc {
            Some(rpc) => request.header(RELAY_RPC_URL_HEADER, rpc),
            None => request,
        };
        let mut response = request.send_json(&body)?;
        let status = response.status().as_u16();
        let text = response.body_mut().read_to_string().unwrap_or_default();
        Ok((status, text))
    });
    let (status, text) = match answer {
        Ok(answer) => answer,
        Err(failure) if matches!(failure.error, ureq::Error::Timeout(_)) => {
            return (false, Some("pending_unknown".to_owned()));
        }
        Err(_) => return (false, Some("network_error".to_owned())),
    };
    sponsorship_answer(status, &text)
}

/// The sponsor endpoint's answer, read: `(sponsored, reason)`.
fn sponsorship_answer(status: u16, text: &str) -> (bool, Option<String>) {
    let data = serde_json::from_str::<Value>(text).unwrap_or(Value::Null);
    let reason = data
        .get("reason")
        .and_then(Value::as_str)
        .filter(|reason| !reason.is_empty())
        .map(str::to_owned);
    if !(200..300).contains(&status) {
        let fallback = if matches!(status, 503 | 429) {
            "service_unavailable"
        } else {
            "request_failed"
        };
        return (false, Some(reason.unwrap_or_else(|| fallback.to_owned())));
    }
    (
        data.get("sponsored")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        reason,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 083 fee: the relay's "UserOperation simulation failed" (the
    /// device log's words) and a revert are answers about the operation; a
    /// busy or limited relay is not. The words themselves are unchanged for
    /// the submit, which reads only them.
    #[test]
    fn a_failed_simulation_is_an_answer_and_a_busy_relay_is_not() {
        assert!(is_simulation_refusal("UserOperation simulation failed"));
        assert!(is_simulation_refusal("execution reverted: STF"));
        assert!(is_simulation_refusal(
            "user operation reverted during simulation"
        ));
        // The review: an EntryPoint validation code is the account's own
        // check, retried on the growing wait as before — whatever words wrap
        // it.
        assert!(!is_simulation_refusal("AA23 reverted (or OOG)"));
        assert!(!is_simulation_refusal(
            "UserOperation reverted during simulation with reason: AA23 reverted"
        ));
        assert!(!is_simulation_refusal(
            "UserOperation simulation failed: AA25 invalid account nonce"
        ));
        assert!(!is_simulation_refusal("reverted"), "no operation named");
        // Hex that happens to hold "AA" and two digits is not a code.
        assert!(is_simulation_refusal(
            "execution reverted: 0x08c379a0AA12ff"
        ));
        assert!(is_simulation_refusal("execution reverted: to 0x12AA34"));
        assert!(!is_simulation_refusal("Retry later"));
        assert!(!is_simulation_refusal("rate limited"));
        assert!(!is_simulation_refusal("Gas estimation failed"));
        // The relay's word that it fails is a refusal; no answer never is.
        let answered = |message: &str| EstimateError {
            message: message.to_owned(),
            error_json: Some(json!({ "code": -32500, "message": message }).to_string()),
        };
        assert!(answered("UserOperation simulation failed").refuses());
        assert!(!answered("Gas estimation failed").refuses());
        assert!(
            !EstimateError::unanswered("UserOperation simulation failed".to_owned()).refuses(),
            "no answer is never the relay's word"
        );
        assert_eq!(
            answered("UserOperation simulation failed").to_string(),
            "UserOperation simulation failed"
        );
    }

    /// 078 W-05, `recommendedFundingWei`: the shortfall plus half again; an
    /// account already at the threshold is asked for the buffered threshold,
    /// not for nothing.
    #[test]
    fn a_top_up_is_the_shortfall_with_the_relays_buffer() {
        assert_eq!(recommended_funding_wei(1_000, 400), 900);
        assert_eq!(recommended_funding_wei(1_000, 1_000), 1_500);
        assert_eq!(recommended_funding_wei(1_000, 5_000), 1_500);
    }

    /// 078 W-05, `requestSponsorship`'s reading: a refusal keeps the relay's
    /// own reason — a 503 `passkey_index_unavailable` is "try later", not "no"
    /// — and a bare status says only what the status says.
    #[test]
    fn the_sponsor_answer_keeps_the_relays_reason() {
        assert_eq!(
            sponsorship_answer(200, r#"{"sponsored":true}"#),
            (true, None)
        );
        assert_eq!(
            sponsorship_answer(200, r#"{"sponsored":false,"reason":"already_funded"}"#),
            (false, Some("already_funded".to_owned()))
        );
        assert_eq!(
            sponsorship_answer(503, r#"{"reason":"passkey_index_unavailable"}"#),
            (false, Some("passkey_index_unavailable".to_owned()))
        );
        assert_eq!(
            sponsorship_answer(429, ""),
            (false, Some("service_unavailable".to_owned()))
        );
        assert_eq!(
            sponsorship_answer(400, "not json"),
            (false, Some("request_failed".to_owned()))
        );
    }

    /// Spec 069: the speed the displayed fee was priced at is the third
    /// parameter, by name; no speed is the two-element wire; `rapid` is never
    /// sent, because the relay refuses it before any handler runs.
    #[test]
    fn a_submission_names_its_speed_as_the_third_parameter() {
        let dict = json!({ "sender": "0x1" });
        for (tier, name) in [
            (FeeTier::Fast, "fast"),
            (FeeTier::Standard, "standard"),
            (FeeTier::Slow, "slow"),
        ] {
            assert_eq!(
                submit_params(dict.clone(), Some(tier)),
                json!([dict, ENTRY_POINT, name])
            );
        }
        assert_eq!(
            submit_params(dict.clone(), None),
            json!([dict, ENTRY_POINT])
        );
        assert_eq!(
            submit_params(dict.clone(), Some(FeeTier::Rapid)),
            json!([dict, ENTRY_POINT])
        );
    }

    #[test]
    fn a_quote_row_needs_every_field_it_prices_with() {
        let native = json!({
            "recipient": "0x1111111111111111111111111111111111111111",
            "asset": "native", "balance": "0xde0b6b3a7640000", "decimals": 18,
            "symbol": "xDAI", "usdBalance": "1.00", "usdPrice": "1"
        });
        let row = parse_quote_row(&native).unwrap_or_else(|| unreachable!("well-formed"));
        assert_eq!(row.asset, FeeAssetKind::Native);
        assert_eq!(row.fee_token, None);
        assert_eq!(row.balance, "1000000000000000000");
        assert_eq!(row.usd_price.as_deref(), Some("1"));

        // A stablecoin without a price cannot be converted into — dropped.
        let unpriced = json!({
            "recipient": "0x1111111111111111111111111111111111111111",
            "asset": "erc20", "feeToken": "0x2222222222222222222222222222222222222222",
            "balance": "0x0", "decimals": 6, "symbol": "USDC", "usdBalance": "0"
        });
        assert!(parse_quote_row(&unpriced).is_none());
        // A bad recipient can never become a fee leg.
        let bad = json!({ "recipient": "not-an-address", "asset": "native",
            "balance": "0x0", "decimals": 18, "symbol": "ETH" });
        assert!(parse_quote_row(&bad).is_none());
        // usdBalance defaults to "0"; a negative usdPrice is unusable.
        let odd = json!({ "recipient": "0x1111111111111111111111111111111111111111",
            "asset": "native", "balance": 5, "decimals": 18, "symbol": "ETH", "usdPrice": "-1" });
        let row = parse_quote_row(&odd).unwrap_or_else(|| unreachable!("well-formed"));
        assert_eq!(row.usd_balance, "0");
        assert_eq!(row.usd_price, None);
        assert_eq!(row.balance, "5");
    }

    #[test]
    fn big_hex_reads_the_three_shapes_the_relay_uses() {
        assert_eq!(big_hex(Some(&json!("0xff"))), 255);
        assert_eq!(big_hex(Some(&json!("ff"))), 255);
        assert_eq!(big_hex(Some(&json!(12))), 12);
        assert_eq!(big_hex(Some(&json!(""))), 0);
        assert_eq!(big_hex(Some(&json!(null))), 0);
        assert_eq!(big_hex(None), 0);
    }

    // -- the submit loop against a fake relay (spec 082 T050) --

    const LOCAL: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
    const RELAY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";

    fn answered(body: Value) -> pool::Routed {
        pool::Routed {
            answer: Ok(body),
            maybe_delivered: false,
        }
    }

    /// A relay that gives up with the pool's own verdict.
    fn silent(maybe_delivered: bool) -> pool::Routed {
        pool::Routed {
            answer: Err(pool::PoolError::Failed {
                rate_limited: false,
            }),
            maybe_delivered,
        }
    }

    /// Run the loop over a scripted relay; the count of POSTs and sleeps back.
    fn run(script: Vec<pool::Routed>) -> (SubmitVerdict, u32, usize, usize) {
        let mut replies = script.into_iter();
        let mut posts = 0;
        let mut sleeps = 0;
        let (verdict, attempts) = submit_loop(
            LOCAL,
            || {
                posts += 1;
                replies.next().unwrap_or_else(|| silent(false))
            },
            |_| sleeps += 1,
        );
        (verdict, attempts, posts, sleeps)
    }

    /// `mute`: the request reached the relay and the reply never came back —
    /// every endpoint timed out. The op may be on chain: followed under the
    /// local hash, never "not sent".
    #[test]
    fn a_mute_relay_is_may_have_been_sent_under_the_local_hash() {
        let (verdict, attempts, posts, _) = run(vec![silent(true)]);
        assert_eq!(
            verdict,
            SubmitVerdict::MaybeSent {
                user_op_hash: LOCAL.to_owned()
            }
        );
        assert_eq!((attempts, posts), (1, 1));
    }

    /// Nothing left the machine (DNS, refused, the proxy's CONNECT): not sent.
    #[test]
    fn a_refused_relay_is_not_sent() {
        let (verdict, _, posts, _) = run(vec![silent(false)]);
        assert_eq!(verdict, SubmitVerdict::NotSent { rejection: None });
        assert_eq!(posts, 1);
    }

    /// The relay already holds this operation: accepted, under its hash.
    /// Another operation's hash holds the nonce (083): never this one's.
    #[test]
    fn an_existing_hash_marker_is_accepted() {
        let error = json!({"error": {"code": -32000, "message": format!("already queued [existingHash:{LOCAL}]")}});
        let (verdict, ..) = run(vec![answered(error)]);
        assert_eq!(
            verdict,
            SubmitVerdict::Accepted {
                user_op_hash: LOCAL.to_owned()
            }
        );
        let error = json!({"error": {"code": -32000, "message": format!("already queued [existingHash:{RELAY}]")}});
        let (verdict, ..) = run(vec![answered(error)]);
        assert_eq!(
            verdict,
            SubmitVerdict::NotSent {
                rejection: Some(vela_core::user_op::RelayRejection::NonceHeld {
                    user_op_hash: RELAY.to_owned()
                })
            }
        );
        let (verdict, ..) = run(vec![answered(json!({ "result": RELAY }))]);
        assert_eq!(
            verdict,
            SubmitVerdict::Accepted {
                user_op_hash: RELAY.to_owned()
            }
        );
    }

    /// A busy relay is asked again — the identical operation, 3 s apart, at
    /// most three more times — and a lost reply on any earlier POST keeps the
    /// verdict at "may have been sent" even when a later one is refused.
    #[test]
    fn busy_is_retried_and_an_earlier_lost_reply_is_remembered() {
        let busy =
            || answered(json!({"error": {"message": "sender currently processing, Retry later"}}));
        let (verdict, attempts, posts, sleeps) =
            run(vec![busy(), busy(), answered(json!({ "result": RELAY }))]);
        assert!(matches!(verdict, SubmitVerdict::Accepted { .. }));
        assert_eq!((attempts, posts, sleeps), (3, 3, 2));

        let (verdict, _, posts, sleeps) = run(vec![busy(), busy(), busy(), busy()]);
        assert!(matches!(
            verdict,
            SubmitVerdict::NotSent { rejection: Some(_) }
        ));
        assert_eq!((posts, sleeps), (4, 3), "three retries, then the verdict");

        // The first POST: one endpoint's reply was lost before another said
        // "busy". The retry is refused — which proves nothing about the POST
        // whose reply was lost, and that one is what may have landed.
        let busy_after_a_lost_reply = pool::Routed {
            answer: busy().answer,
            maybe_delivered: true,
        };
        let refused = answered(json!({"error": {"message": "AA25 invalid account nonce"}}));
        let (verdict, _, posts, _) = run(vec![busy_after_a_lost_reply, refused]);
        assert_eq!(
            verdict,
            SubmitVerdict::MaybeSent {
                user_op_hash: LOCAL.to_owned()
            }
        );
        assert_eq!(posts, 2);
    }

    /// A JSON answer that is not a hash is an answer nobody can read.
    #[test]
    fn an_unreadable_result_is_may_have_been_sent() {
        let (verdict, ..) = run(vec![answered(json!({ "result": null }))]);
        assert!(matches!(verdict, SubmitVerdict::MaybeSent { .. }));
    }

    /// RD14 (W17): the first close during a submit is held; a second within
    /// 5 s goes through, a close after 5 s is held again, and with nothing in
    /// flight every close goes.
    #[test]
    fn a_close_during_a_submit_is_held_once() {
        let start = std::time::Instant::now();
        let later = |ms: u64| start + Duration::from_millis(ms);
        assert_eq!(close_verdict(false, None, start), (true, None));
        let (allow, held) = close_verdict(true, None, start);
        assert!(!allow, "the first close is held");
        assert_eq!(held, Some(start));
        assert_eq!(
            close_verdict(true, held, later(4_000)),
            (true, None),
            "a second within 5 s"
        );
        let (allow, held_again) = close_verdict(true, held, later(6_000));
        assert!(!allow, "too late to count as the second: held again");
        assert_eq!(held_again, Some(later(6_000)));
        assert_eq!(close_verdict(false, held, later(1_000)), (true, None));
    }

    #[test]
    fn the_in_flight_guard_counts_and_uncounts() {
        let before = SUBMITS_IN_FLIGHT.load(std::sync::atomic::Ordering::SeqCst);
        {
            let _one = SubmitGuard::enter();
            assert!(submit_in_flight());
            let _two = SubmitGuard::enter();
            assert_eq!(
                SUBMITS_IN_FLIGHT.load(std::sync::atomic::Ordering::SeqCst),
                before + 2
            );
        }
        assert_eq!(
            SUBMITS_IN_FLIGHT.load(std::sync::atomic::Ordering::SeqCst),
            before
        );
    }

    /// 083: `eth_getUserOperationByHash` names the bundle transaction only
    /// once there is one — `null` (pending, ERC-7769) and unknown ops are
    /// nothing, never a zero hash.
    #[test]
    fn the_bundler_names_a_transaction_only_once_there_is_one() {
        let tx = format!("0x{}", "cd".repeat(32));
        assert_eq!(
            transaction_of(&json!({ "result": { "transactionHash": tx, "blockNumber": "0x11" } })),
            Some(tx)
        );
        for body in [
            json!({ "result": null }),
            json!({ "result": { "transactionHash": null, "blockNumber": null } }),
            json!({ "result": { "transactionHash": format!("0x{}", "0".repeat(64)) } }),
            json!({ "result": { "transactionHash": "0xabc" } }),
            json!({ "error": { "code": -32601, "message": "method not found" } }),
        ] {
            assert_eq!(transaction_of(&body), None, "{body}");
        }
    }

    #[test]
    fn a_receipt_log_keeps_only_the_three_fields_the_netting_reads() {
        let log = json!({ "address": "0xabc", "topics": ["0x1", 2, "0x3"], "data": "0xdead", "extra": true });
        let trust = to_trust_log(&log).unwrap_or_else(|| unreachable!("shaped"));
        assert_eq!(trust.topics, vec!["0x1", "0x3"]);
        assert_eq!(trust.data, "0xdead");
        assert!(to_trust_log(&json!({ "topics": [] })).is_none());
        assert_eq!(
            to_trust_log(&json!({ "address": "0xabc", "topics": [] })).map(|l| l.data),
            Some("0x".to_owned())
        );
    }

    #[test]
    fn the_builtin_base_is_the_relay_unless_configured() {
        crate::executor::storage::tests::with_temp_state("relay-base", || {
            assert_eq!(builtin_base(), BUILTIN_BASE);
            let _ = storage::write_value(
                storage::KEY_SERVICE_ENDPOINTS,
                json!({ "bundlerServiceURL": "https://relay.example/" }),
            );
            assert_eq!(builtin_base(), "https://relay.example");
        });
    }

    // -- live (`cargo test executor::relay -- --ignored --test-threads=1`) --

    const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";

    /// The relay serves Gnosis: the treasury probe answers a coverage verdict,
    /// never `Unknown`, and the base the pool names is the relay's own host.
    #[test]
    #[ignore = "real network"]
    fn live_the_relay_covers_gnosis() {
        let base = base_url(100);
        println!("relay base for gnosis: {base}");
        assert!(base.starts_with("https://"));
        assert!(
            !base.ends_with("/100"),
            "the REST base carries no chain suffix"
        );
        let probe = probe_treasury(100);
        println!("treasury probe: {probe:?}");
        assert!(
            !matches!(probe, SendTreasuryProbe::Unknown),
            "a transient answer from a healthy relay"
        );
    }

    /// The golden Safe's in-band quote rows: a native row with a recipient
    /// the fee leg can be paid to, and the account info's recipient agrees.
    #[test]
    #[ignore = "real network"]
    fn live_the_golden_safe_is_quoted_in_band() {
        let quotes = in_band_quotes(100, GOLDEN).unwrap_or_else(|| unreachable!("no quotes"));
        for quote in &quotes {
            println!(
                "quote: {} {:?} recipient={} balance={}",
                quote.symbol, quote.asset, quote.recipient, quote.balance
            );
        }
        let native = quotes
            .iter()
            .find(|quote| quote.asset == FeeAssetKind::Native)
            .unwrap_or_else(|| unreachable!("a native row"));
        assert!(is_address(&native.recipient));
        let raw = raw_bundler_quote(100, FeeTier::Fast).unwrap_or_else(|| unreachable!("no quote"));
        println!("bundler fast quote: {raw:?}");
        assert_ne!(raw.max_fee_per_gas, "0");
        let info = account_info(100, GOLDEN).unwrap_or_else(|| unreachable!("no account info"));
        println!("account info: {info:?}");
        assert!(info.fee_recipient().is_some());
    }

    /// A hash nobody submitted: the relay answers, and says it has nothing.
    #[test]
    #[ignore = "real network"]
    fn live_an_unknown_hash_is_pending_not_unreachable() {
        let hash = format!("0x{}", "ab".repeat(32));
        let poll = user_op_receipt(&hash, 100);
        println!("receipt poll: {poll:?}");
        assert!(poll.reached_bundler);
        assert!(poll.resolution.is_none());
        // Spec 082 RA7: the relay's own method, answered `not_found` — never
        // None, which is what -32601 on the old name produced.
        let status = user_op_status(&hash, 100);
        println!("status: {status:?}");
        let status = status.unwrap_or_else(|| unreachable!("the relay did not answer the status"));
        assert_eq!(
            status.status,
            vela_core::app::tx_tracker::TrackLifecycle::NotFound
        );
    }
}
