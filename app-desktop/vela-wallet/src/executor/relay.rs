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
use vela_core::app::tx_tracker::TrackLifecycle;
use vela_core::safe::ENTRY_POINT;
use vela_core::user_op::{GasEstimate, UserOperation, parse_hex_quantity, user_op_to_json};

use crate::executor::{pool, proxy, storage};

/// The built-in relay. Per-chain JSON-RPC is `{base}/{chain_id}`; REST is
/// `{base}/v1/…`.
pub const BUILTIN_BASE: &str = "https://vela-relay.getvela.app";

/// `NET_TIMEOUTS.bundlerRest` — the REST account and treasury lookups.
const REST_TIMEOUT: Duration = Duration::from_secs(10);
/// `INFO_CACHE_TTL` (`bundler-service.ts:105`).
const INFO_CACHE_TTL: Duration = Duration::from_secs(30);
/// `QUOTE_CACHE_TTL` (`bundler-service.ts:658`).
const QUOTE_CACHE_TTL: Duration = Duration::from_secs(8);
/// `submitUserOp`'s retry budget for a busy relay.
const SUBMIT_MAX_RETRIES: u32 = 3;
const SUBMIT_RETRY_DELAY: Duration = Duration::from_secs(3);

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
    // Invariant ②: the relay reads the chain through the endpoint this wallet
    // trusts, or through its own when the pool names none.
    let rpc = pool::best_rpc_url(chain_id);
    // Over the candidate chain (spec 038): a refused proxy is retried on the
    // next route, not reported as the relay being down.
    let mut response = match proxy::with_candidates(REST_TIMEOUT, |agent| {
        let mut request = agent.get(&url).header("accept", "application/json");
        if let Some(rpc) = rpc.as_deref() {
            request = request.header("X-Rpc-Url", rpc);
        }
        request.call()
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
    let body = pool::bundler_call(chain_id, "pimlico_getUserOperationGasPrice", json!([])).ok()?;
    if body.get("error").is_some() {
        return None;
    }
    let row = body.get("result")?.get(tier_key(tier))?;
    Some(FeeBundlerQuote {
        max_fee_per_gas: decimal_of_hex(row.get("maxFeePerGas"))?,
        network_fee_per_gas: decimal_of_hex(row.get("networkFeePerGas")),
        relayer_fee_per_gas: decimal_of_hex(row.get("relayerFeePerGas")),
    })
}

/// `eth_estimateUserOperationGas` — the relay's raw limits, or its words.
pub fn estimate_user_op_gas(op: &UserOperation, chain_id: u32) -> Result<GasEstimate, String> {
    let body = pool::bundler_call(
        chain_id,
        "eth_estimateUserOperationGas",
        json!([user_op_to_json(op, &[]), ENTRY_POINT]),
    )
    .map_err(|error| format!("gas estimation unreachable: {error:?}"))?;
    if let Some(error) = body.get("error") {
        return Err(error
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or("Gas estimation failed")
            .to_owned());
    }
    let result = body
        .get("result")
        .filter(|value| value.is_object())
        .ok_or_else(|| "Failed to estimate gas — empty result".to_owned())?;
    let field = |name: &str| {
        parse_hex_quantity(result.get(name).and_then(Value::as_str)).map_err(|e| e.to_string())
    };
    Ok(GasEstimate {
        verification_gas_limit: field("verificationGasLimit")?,
        call_gas_limit: field("callGasLimit")?,
        pre_verification_gas: field("preVerificationGas")?,
    })
}

/// Why a submit produced no hash.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitError {
    /// The relay answered and refused. Carries `parseBundlerError`'s
    /// sentence — the one the send executor classifies.
    Rejected(String),
    /// The relay could not be reached at all.
    Unreachable,
}

/// `eth_sendUserOperation`, with the retry loop a busy relay earns: up to
/// three more tries, 3 s apart, on "currently processing" / "Retry later".
/// The AA20 structural guard runs in the caller, which holds the deployment
/// read.
pub fn send_user_op(
    op: &UserOperation,
    chain_id: u32,
    extra: &[(&str, &str)],
) -> Result<String, SubmitError> {
    let dict = user_op_to_json(op, extra);
    eprintln!(
        "[vela-wallet] relay: submitting sender={} nonce={} initCode={} callData={}B signature={}B",
        op.sender,
        op.nonce,
        if op.init_code.len() >= 20 {
            "yes"
        } else {
            "no"
        },
        op.call_data.len(),
        op.signature.len()
    );
    for attempt in 0..=SUBMIT_MAX_RETRIES {
        let body = pool::bundler_call(
            chain_id,
            "eth_sendUserOperation",
            json!([dict, ENTRY_POINT]),
        )
        .map_err(|_| SubmitError::Unreachable)?;
        if let Some(hash) = body.get("result").and_then(Value::as_str) {
            return Ok(hash.to_owned());
        }
        let message = parse_bundler_error(body.get("error"));
        let retryable = message.contains("currently processing") || message.contains("Retry later");
        if !retryable || attempt == SUBMIT_MAX_RETRIES {
            return Err(SubmitError::Rejected(message));
        }
        eprintln!(
            "[vela-wallet] relay: busy, retry {}/{SUBMIT_MAX_RETRIES}",
            attempt + 1
        );
        std::thread::sleep(SUBMIT_RETRY_DELAY);
    }
    Err(SubmitError::Rejected(
        "Bundler unavailable after retries".to_owned(),
    ))
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

fn to_trust_log(log: &Value) -> Option<TrustReceiptLog> {
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
    let unreachable = ReceiptPoll {
        reached_bundler: false,
        resolution: None,
    };
    if user_op_hash.is_empty() {
        return unreachable;
    }
    let Ok(body) = pool::bundler_call(
        chain_id,
        "eth_getUserOperationReceipt",
        json!([user_op_hash]),
    ) else {
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

/// `eth_getUserOperationStatus` (a Vela extension): the lifecycle and the
/// executor stage that last touched the op. `None` for an older relay, an
/// error or an unreachable one (`pollUserOpStatus`).
pub fn user_op_status(
    user_op_hash: &str,
    chain_id: u32,
) -> Option<(TrackLifecycle, Option<String>)> {
    if user_op_hash.is_empty() {
        return None;
    }
    let body = pool::bundler_call(
        chain_id,
        "eth_getUserOperationStatus",
        json!([user_op_hash]),
    )
    .ok()?;
    if body.get("error").is_some() {
        return None;
    }
    let result = body.get("result").filter(|value| value.is_object())?;
    let status = match result.get("status").and_then(Value::as_str)? {
        "not_found" => TrackLifecycle::NotFound,
        "queued" => TrackLifecycle::Queued,
        "not_submitted" => TrackLifecycle::NotSubmitted,
        "submitted" => TrackLifecycle::Submitted,
        "rejected" => TrackLifecycle::Rejected,
        "included" => TrackLifecycle::Included,
        "failed" => TrackLifecycle::Failed,
        _ => return None,
    };
    let stage = result
        .get("last_executor_stage")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Some((status, stage))
}

// ---------------------------------------------------------------------------
// The wording layer the machines leave to the shell
// ---------------------------------------------------------------------------

/// `parseBundlerUnderfunded`: is this the relay saying the per-Safe gas
/// account is short? Wording-tolerant — the relay has reworded it before
/// ("…bundler EOA" → "…bundler gas account … Deposit to:").
pub fn is_bundler_underfunded(message: &str) -> bool {
    let lower = message.to_lowercase();
    lower.contains("dedicated bundler gas account")
        || lower.contains("dedicated bundler eoa")
        || (lower.contains("deposit to:")
            && lower
                .split("deposit to:")
                .nth(1)
                .is_some_and(|rest| rest.trim_start().starts_with("0x"))
            && lower.contains("required:"))
}

/// `parseBundlerError`: the relay's error member as one sentence. The known
/// AA codes get the words the Expo client always gave them; everything else
/// is the relay's own message, cleaned.
pub fn parse_bundler_error(error: Option<&Value>) -> String {
    let Some(error) = error else {
        return "Transaction failed: unknown error".to_owned();
    };
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| error.get("data").and_then(Value::as_str))
        .unwrap_or_default();
    let has = |needle: &str| message.contains(needle);
    if has("insufficient funds") || has("balance too low") {
        return "Insufficient balance to cover gas fees. Please fund your account.".to_owned();
    }
    if has("could not load bundle") || has("simulation failed") {
        return "Transaction simulation failed. The network may be congested or the transaction parameters are invalid. Please try again.".to_owned();
    }
    if has("AA21") || has("didn't pay prefund") {
        return "Insufficient gas funds. The bundler account needs more balance on this network."
            .to_owned();
    }
    if has("AA10") || has("sender already constructed") {
        return "Wallet deployment conflict. Please try again.".to_owned();
    }
    if has("AA13") || has("initCode failed") {
        return "Wallet deployment failed. Required contracts may not be deployed on this network."
            .to_owned();
    }
    if has("AA23") || has("reverted") {
        return "Transaction reverted during simulation. Check recipient address and amount."
            .to_owned();
    }
    if has("AA25") || has("invalid account nonce") {
        return "Transaction nonce mismatch. Please try again.".to_owned();
    }
    if has("rate limit") || has("429") {
        return "Bundler rate limit reached. Please wait a moment and try again.".to_owned();
    }
    let clean = message
        .trim_start_matches("execution reverted:")
        .trim_start_matches("Execution reverted:")
        .trim();
    if !clean.is_empty() {
        return format!("Transaction failed: {clean}");
    }
    let text = error.to_string();
    format!("Transaction failed: {}", &text[..text.len().min(200)])
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn the_underfunded_wording_is_recognised_in_both_generations() {
        assert!(is_bundler_underfunded(
            "The dedicated bundler gas account is underfunded. Deposit to: 0xabc required: 5"
        ));
        assert!(is_bundler_underfunded(
            "dedicated bundler EOA balance too low"
        ));
        assert!(is_bundler_underfunded(
            "Spendable: 0 required: 123 Deposit to: 0xB32a3965c4823Ea426de52C7E869DD0CFe154D03"
        ));
        assert!(!is_bundler_underfunded("Deposit to: nowhere required: 1"));
        assert!(!is_bundler_underfunded("AA25 invalid account nonce"));
        assert!(!is_bundler_underfunded(""));
    }

    #[test]
    fn relay_errors_get_the_words_the_clients_always_gave_them() {
        let message = |text: &str| parse_bundler_error(Some(&json!({ "message": text })));
        assert!(message("AA21 didn't pay prefund").starts_with("Insufficient gas funds"));
        assert!(message("AA25 invalid account nonce").starts_with("Transaction nonce mismatch"));
        assert!(message("rate limit exceeded").starts_with("Bundler rate limit"));
        // A ported quirk, kept: "execution reverted: …" carries the word the
        // AA23 rule matches, so it gets that sentence, never the cleaned tail.
        assert!(message("execution reverted: custom words").starts_with("Transaction reverted"));
        assert_eq!(message("custom words"), "Transaction failed: custom words");
        assert_eq!(
            parse_bundler_error(None),
            "Transaction failed: unknown error"
        );
        // `data` is read when `message` is absent.
        assert_eq!(
            parse_bundler_error(Some(&json!({ "data": "AA10 sender already constructed" }))),
            "Wallet deployment conflict. Please try again."
        );
        // An error with no words at all still says something bounded.
        assert!(
            parse_bundler_error(Some(&json!({ "code": -32000 })))
                .starts_with("Transaction failed:")
        );
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
        let status = user_op_status(&hash, 100);
        println!("status: {status:?}");
        assert!(matches!(status, None | Some((TrackLifecycle::NotFound, _))));
    }
}
