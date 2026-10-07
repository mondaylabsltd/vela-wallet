//! Machine — the unified token trust model (spec `016-crux-wallet-state`,
//! token_trust). The wallet's anti-scam security core.
//!
//! ```text
//! PollRequested ─► ReadCustomTokens ─► per held chain: blockNumber ─► getLogs(allowlist)
//!                                            │ range-capped              │ logs
//!                                            ▼ (retry ONCE)              ▼
//!                                       getLogs(span)          local re-verify topics[2]
//!                                                                        ▼
//!                                                     timestamps ─► metadata gate ─► feed
//!
//! ReceiptLogsConfirmed (tx_tracker, AUTHENTIC) ─► net deltas ─► admission ─► WriteCustomToken
//! SimDeltasComputed (sign sheet, UNTRUSTED)   ─► asymmetric judgment ─► render only, NEVER a write
//! ```
//!
//! Three formerly separate reports — the transfer-monitor allowlist
//! (`transfer-monitor.ts`), auto-add admission (`token-autoadd.ts`) and the
//! simulation's asymmetric trust set (`tx-simulation.ts:206-286`) — are one
//! security model, coupled today through `getCachedHeldTokens`
//! (`wallet-api.ts:141-152`). The core risk is the held→trusted cascade:
//! admitting one scam token poisons the simulation trust set AND the transfer
//! allowlist, so both decision chains live here as pure functions.
//!
//! The soul of this machine is `token-autoadd.ts:5-14`: tokens are admitted
//! ONLY from AUTHENTICATED on-chain receipt logs, NEVER from a sign-time
//! simulation — a hostile dApp can synthesize a fake `Transfer(_, you, big)`
//! and answer `symbol()` to spoof a token. Structurally enforced:
//! [`Event::ReceiptLogsConfirmed`] is the single constructor of an admission
//! session, and the [`Event::SimDeltasComputed`] path can reach
//! [`TrustOperation::WriteCustomToken`] through no code path at all.
//!
//! Inventory invariants:
//! - ① never trust the RPC's topic filter — topics[2] is re-verified locally
//!   (`transfer-monitor.ts:136-168`), so a malicious pool endpoint cannot
//!   surface someone else's transfer as a fake "Received".
//! - ② `eth_getLogs` is restricted to the trusted-contract allowlist
//!   (`:188-221`): chain stables + user-added tokens + the EIP-7708 native
//!   sentinels. Held-but-never-added tokens are deliberately NOT watched.
//! - ③ an ERC-20 whose metadata can't resolve never reaches the feed — the
//!   18-decimals fallback would record a 6-decimals stablecoin as "+0 tokens"
//!   (`activity.ts:408-415`).
//! - ④ at most TWO `eth_getLogs` per chain per poll for transfers: probe +
//!   one capped retry, never a fan-out (`transfer-monitor.ts:96-118`); then
//!   ONE more for the wallet's own `SafeReceived` over the span that answered
//!   (issue #443) — native money, which emits no `Transfer`.
//! - ⑤ auto-add's data source is never a simulation log (`token-autoadd.ts:5-14`).
//! - ⑥ asymmetric sim trust: SENT renders whenever metadata resolves (a real
//!   token emits its own log, so an outflow can't be understated); RECEIVED
//!   renders a confident amount only inside the trusted set
//!   (`tx-simulation.ts:243-286`).
//! - ⑦ unknown is `null` (no information), never `false`/a default — an
//!   unresolvable token stays unverified, and a cold registry means every
//!   received token falls back to unverified, the safe direction.
//! - ⑧ no symbol ⇒ no listing; `${chainId}_${addr}` ids never duplicate; a
//!   successful admission invalidates the token cache (`token-autoadd.ts:60-81`).
//!
//! Resident machine (app lifetime, not screen lifetime): every shell result
//! carries the address/chain it was fetched for, and results for a different
//! account are dropped by construction. The scan-chain decision — formerly a
//! `fetchTokens` cache side-effect (`activity.ts:395-404`) — is an explicit
//! event input here ([`Event::HeldChainsSnapshot`]).
//!
//! The shell owns the RPC transport, the range-cap message parsing
//! (`getLogsRangeCap` wording → [`TrustLogsOutcome::RangeCapped`]), storage,
//! and all naming/formatting (`networkName`, native symbols); this core owns
//! every acceptance and admission decision.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

use super::token_registry::registry_token;

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// keccak256("Transfer(address,address,uint256)") (`transfer-monitor.ts:31`).
pub const TRANSFER_TOPIC: &str =
    "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef";

/// keccak256("SafeReceived(address,uint256)") — Safe v1.4.1's `receive()`:
/// the wallet's OWN log for every native coin paid to it, sender indexed,
/// value in `data`. A plain coin transfer emits no `Transfer` log on most
/// chains (Gnosis has no EIP-7708), so this is how a deployed wallet sees
/// native money arrive (issue #443: 0.0001 xDAI received, Activity empty).
pub const SAFE_RECEIVED_TOPIC: &str =
    "0x3d0ce9bfc3ed7d6862dbb28b2dea94561fe714a1b4d019aa8af39730d1ad7c3d";

/// Contract-address sentinels marking a log as a *native* (EIP-7708) transfer
/// (`transfer-monitor.ts:38-42`). Everything else is treated as ERC-20.
pub const NATIVE_LOG_ADDRESSES: [&str; 3] = [
    "0xfffffffffffffffffffffffffffffffffffffffe",
    "0x0000000000000000000000000000000000000000",
    "0xeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
];

/// Incremental monitor window, in blocks (`transfer-monitor.ts:54`).
pub const LIVE_SCAN_BLOCKS: u64 = 100;

/// Main payment chains to monitor when the wallet has no balances yet
/// (`activity.ts:393`).
pub const DEFAULT_MONITOR_CHAINS: [u32; 6] = [1, 56, 137, 42_161, 8_453, 100];

/// Distinct blocks whose timestamps are resolved per chain per poll — the
/// `slice(0, 25)` cap (`transfer-monitor.ts:172`); the rest fall back to now.
pub const TIMESTAMP_BLOCK_CAP: usize = 25;

// ---------------------------------------------------------------------------
// Wire value types
// ---------------------------------------------------------------------------

/// A raw `eth_getLogs` entry, untouched by the shell — the whole point is
/// that this core, not the endpoint, decides what the log means.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustRawLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub transaction_hash: String,
    /// Hex quantity; absent reads as `0x0` (`log.blockNumber ?? '0x0'`).
    pub block_number: Option<String>,
    /// Hex quantity; absent reads as `0x0` (`log.logIndex ?? '0x0'`).
    pub log_index: Option<String>,
}

/// An eth log as returned in a UserOp/tx receipt — the `SimLog` shape
/// `deriveAssetDeltas` nets (`token-autoadd.ts:24`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustReceiptLog {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrustDeltaKind {
    Native,
    Erc20,
}

/// One signed per-asset balance change from a *simulation*, as the shell's
/// engine derived it. Untrusted input by definition (invariant ⑤/⑥).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustAssetDelta {
    pub kind: TrustDeltaKind,
    /// Lowercased ERC-20 contract address; `None` for the native coin.
    pub token: Option<String>,
    /// Signed decimal string in the asset's smallest unit.
    pub delta: String,
}

/// `{symbol, decimals}` for one ERC-20 (`token-metadata.ts TokenMetadata`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustTokenMeta {
    pub symbol: String,
    pub decimals: u32,
}

/// The persisted custom-token record (`models/types.ts CustomToken`), minus
/// `networkName` — chain naming is display vocabulary the shell derives from
/// `chain_id` when it maps this onto its storage shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustCustomToken {
    /// `"{chainId}_{contractAddress}"` — the de-dupe identity (invariant ⑧).
    pub id: String,
    pub chain_id: u32,
    pub contract_address: String,
    pub symbol: String,
    pub name: String,
    pub decimals: u32,
}

/// One entry of a Multicall3 metadata answer. `meta: None` means "looked up
/// and unresolvable" — a fact worth remembering (session negative memo,
/// `token-metadata.ts:126`), and NOT license to invent defaults (invariant ⑦).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustMetaEntry {
    pub addr: String,
    pub meta: Option<TrustTokenMeta>,
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. RPC operations carry the
/// account address so the shell can echo it back on the result — the tag that
/// lets a stale answer be dropped by construction.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "TrustOperation"))]
pub enum TrustOperation {
    /// `eth_blockNumber`.
    RpcBlockNumber { address: String, chain_id: u32 },
    /// `eth_getLogs` for the Transfer topic, recipient = this wallet,
    /// restricted to `contracts` — the trusted-token allowlist (invariant ②).
    RpcGetLogs {
        address: String,
        chain_id: u32,
        from_block: String,
        to_block: String,
        recipient_topic: String,
        contracts: Vec<String>,
    },
    /// `eth_getLogs` for logs EMITTED BY `address` (the wallet itself) with
    /// `topics[0] = topic` — the Safe's `SafeReceived` ([`SAFE_RECEIVED_TOPIC`]).
    /// Answered with [`TrustShellResult::SafeReceivedLogs`], classified as
    /// `RpcGetLogs`' outcome is.
    RpcGetSafeReceivedLogs {
        address: String,
        chain_id: u32,
        from_block: String,
        to_block: String,
        topic: String,
    },
    /// `eth_getBlockByNumber(block, false)` — timestamp only.
    RpcGetBlockByNumber {
        address: String,
        chain_id: u32,
        block: String,
    },
    /// Batched `symbol()` + `decimals()` via Multicall3
    /// (`token-metadata.ts:97-132`). The shell answers EVERY requested
    /// address, resolved or not.
    MulticallErc20Meta { chain_id: u32, addrs: Vec<String> },
    /// `loadCustomTokens()` — all chains.
    ReadCustomTokens,
    /// `saveCustomToken` — replaces by id, never duplicates (invariant ⑧).
    WriteCustomToken { token: TrustCustomToken },
    /// `clearTokenCache(address)` — without this the admitted token would not
    /// appear until the 5-min fetchTokens TTL lapsed (`token-autoadd.ts:79-81`).
    InvalidateTokenCache { address: String },
}

/// One `eth_getLogs` outcome, classified by the shell's wording layer
/// (`getLogsRangeCap`) into a typed axis the core can act on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrustLogsOutcome {
    Ok {
        logs: Vec<TrustRawLog>,
    },
    /// The endpoint capped the span. `cap` is the parsed block count; `0`
    /// means "capped, but no number could be parsed" → stay conservative
    /// (`transfer-monitor.ts:104-110`).
    RangeCapped {
        cap: u32,
    },
    /// Non-range failure — this chain yields nothing this tick
    /// (`transfer-monitor.ts:122-128`).
    Failed,
}

/// What the shell observed. Scan results carry the address they were fetched
/// for; a result tagged with another account is dropped by construction.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "TrustShellResult"))]
pub enum TrustShellResult {
    BlockNumber {
        address: String,
        chain_id: u32,
        /// `eth_blockNumber` hex, or `None` on RPC failure.
        block_hex: Option<String>,
    },
    /// The answer to [`TrustOperation::RpcGetSafeReceivedLogs`].
    SafeReceivedLogs {
        address: String,
        chain_id: u32,
        outcome: TrustLogsOutcome,
    },
    Logs {
        address: String,
        chain_id: u32,
        outcome: TrustLogsOutcome,
    },
    BlockTimestamp {
        address: String,
        chain_id: u32,
        block_number: f64,
        /// Unix seconds from the block header, or `None` when the lookup
        /// failed — the transfer then falls back to "now".
        timestamp_sec: Option<f64>,
        /// Epoch ms, carried by the result (the 011 `now_iso` pattern) so the
        /// core never reads a clock.
        now_ms: f64,
    },
    /// Metadata facts — chain-global, so they are absorbed unconditionally
    /// and every flow waiting on them re-checks its needs.
    ErcMeta {
        chain_id: u32,
        entries: Vec<TrustMetaEntry>,
    },
    /// `None` = the read itself failed (⇒ the admission aborts, fail-closed;
    /// the poll allowlist degrades to `[]` customs, `loadCustomTokens().catch(() => [])`).
    CustomTokens {
        tokens: Option<Vec<TrustCustomToken>>,
    },
    TokenWritten {
        ok: bool,
    },
    CacheInvalidated,
}

impl Operation for TrustOperation {
    type Output = TrustShellResult;
}

#[effect]
pub enum TrustEffect {
    Render(RenderOperation),
    Shell(TrustOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "TrustEvent"))]
pub enum Event {
    /// Which chains this account actually uses (has balances on) — the scan
    /// set. Formerly inferred from the `fetchTokens` cache side-effect
    /// (`activity.ts:395-404`); now an explicit input. An empty list means
    /// "brand-new wallet" and the poll falls back to
    /// [`DEFAULT_MONITOR_CHAINS`]. A different address switches the account:
    /// the feed and any in-flight scan are discarded.
    HeldChainsSnapshot {
        address: String,
        chain_ids: Vec<u32>,
    },
    /// The ERC-20 addresses this account holds on one chain — the
    /// `getCachedHeldTokens` feed for the trusted receive set. Cold cache =
    /// no event = empty set = everything unverified, the safe direction.
    HeldTokensSnapshot {
        address: String,
        chain_id: u32,
        tokens: Vec<String>,
    },
    /// The token registry's facts for one chain: canonical stablecoins (the
    /// allowlist + trusted set) and the wrapped native token (trusted set
    /// only), from `fetchChainTokens`.
    RegistryTokensSnapshot {
        chain_id: u32,
        stables: Vec<String>,
        wrapped_native: Option<String>,
    },
    /// The user's custom tokens changed (added/removed in the panel). The
    /// poll also re-reads them via [`TrustOperation::ReadCustomTokens`] at
    /// poll start, exactly as `transferAllowlist` does.
    CustomTokensLoaded { tokens: Vec<TrustCustomToken> },
    /// One scan poll across the held chains (the Activity surface's ~10s
    /// cadence lives in the shell). Single-flight: a request while a poll is
    /// running is ignored — the next tick retries.
    PollRequested { address: String },
    /// AUTHENTIC receipt logs of a confirmed transaction, forwarded from
    /// tx_tracker's `NotifyConfirmed`. The ONLY auto-add entry point
    /// (invariant ⑤).
    ReceiptLogsConfirmed {
        from: String,
        chain_id: u32,
        logs: Vec<TrustReceiptLog>,
    },
    /// UNTRUSTED sign-time simulation deltas for the preview. Judged
    /// asymmetrically (invariant ⑥) and rendered — this path can never write
    /// a token (invariant ⑤).
    SimDeltasComputed {
        address: String,
        chain_id: u32,
        deltas: Vec<TrustAssetDelta>,
    },
    /// Internal: an effect resolved. `attempt` guards the account-scoped scan
    /// pipeline (bumped on account switch); admission and metadata results
    /// are correlated by construction instead — see `accept`.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: TrustShellResult,
    },
}

// ---------------------------------------------------------------------------
// Pure decision chains (exported for property tests)
// ---------------------------------------------------------------------------

/// A locally verified incoming transfer (`IncomingTransfer` minus the
/// timestamp, which arrives later). Internal/test type — not wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustTransfer {
    /// Stable id: `{chainId}-{txHash}-{logIndex}` — the overlap de-dupe key.
    pub id: String,
    pub chain_id: u32,
    /// Lowercased token contract, or `None` for native.
    pub token: Option<String>,
    pub is_native: bool,
    pub from: String,
    pub value: u128,
    pub tx_hash: String,
    pub block_number: u64,
    pub log_index: u32,
}

/// One net per-asset delta from receipt-log netting. Internal/test type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrustNetDelta {
    pub is_native: bool,
    /// Lowercased token contract; `None` for native.
    pub token: Option<String>,
    pub delta: i128,
}

/// `addressTopic` (`transfer-monitor.ts:77-79`): `0x` + 24 zeros + the
/// lowercased 40-hex address tail.
pub fn address_topic(address: &str) -> String {
    let tail = address.get(2..).unwrap_or("");
    format!("0x{}{}", "0".repeat(24), tail.to_lowercase())
}

pub fn is_native_log_address(addr: &str) -> bool {
    let lc = addr.to_lowercase();
    NATIVE_LOG_ADDRESSES.contains(&lc.as_str())
}

/// The registry's metadata for `token` on `chain_id` (097 D: one
/// chain-scoped table, [`registry_token`]), case-insensitive.
fn registry_meta(chain_id: u32, token: &str) -> Option<TrustTokenMeta> {
    registry_token(chain_id, token).map(|known| TrustTokenMeta {
        symbol: known.symbol.to_owned(),
        decimals: known.decimals,
    })
}

/// Issue #443 — the wallet's own `SafeReceived` logs, as the native transfers
/// they are, joined to the transfer logs. Never trusts the RPC's filter
/// (invariant ①): a log is taken only when THIS wallet emitted it, its topic
/// is [`SAFE_RECEIVED_TOPIC`] and it names exactly one sender. It is rewritten
/// into the shape [`decode_transfer_logs`] reads — a `Transfer(sender →
/// wallet)` from a native sentinel — so everything after (netting, rows,
/// timestamps, de-duplication) is the same path. A chain that ALSO emits an
/// EIP-7708 native `Transfer` for the same payment keeps that one: the
/// receipt is counted once.
pub fn merge_safe_received(
    mut logs: Vec<TrustRawLog>,
    received: &[TrustRawLog],
    wallet: &str,
) -> Vec<TrustRawLog> {
    let wallet_topic = address_topic(wallet);
    let native_seen: BTreeSet<(String, String)> = logs
        .iter()
        .filter(|log| is_native_log_address(&log.address))
        .map(|log| (log.transaction_hash.to_lowercase(), log.data.to_lowercase()))
        .collect();
    for log in received {
        if !log.address.eq_ignore_ascii_case(wallet)
            || log.topics.len() != 2
            || !log.topics[0].eq_ignore_ascii_case(SAFE_RECEIVED_TOPIC)
        {
            continue;
        }
        let key = (log.transaction_hash.to_lowercase(), log.data.to_lowercase());
        if native_seen.contains(&key) {
            continue;
        }
        logs.push(TrustRawLog {
            address: NATIVE_LOG_ADDRESSES[2].to_owned(),
            topics: vec![
                TRANSFER_TOPIC.to_owned(),
                log.topics[1].clone(),
                wallet_topic.clone(),
            ],
            data: log.data.clone(),
            transaction_hash: log.transaction_hash.clone(),
            block_number: log.block_number.clone(),
            log_index: log.log_index.clone(),
        });
    }
    logs
}

/// Decision chain 1 — log → transfer acceptance (`decodeTransferLogs`,
/// `transfer-monitor.ts:136-168`). Never trusts the RPC's topic filter:
/// only logs whose `topics[2]` is actually this wallet are accepted, so a
/// buggy/caching/malicious endpoint cannot surface someone else's transfer as
/// a fake "Received" (invariant ①). Malformed logs are skipped, exactly as
/// the TS `try/catch` does.
///
/// Ported verbatim quirks: the topic-count guard is `< 3` (not `== 3`), so a
/// four-topic log with non-zero `data` passes — ERC-721 transfers are in
/// practice excluded by their empty `data` (value 0); and `topics[0]` is NOT
/// re-checked (the getLogs filter pinned it). Fail-closed deviation: a value
/// beyond u128 (JS BigInt is unbounded) drops the log instead of carrying a
/// number this core cannot represent.
pub fn decode_transfer_logs(
    raw_logs: &[TrustRawLog],
    address: &str,
    chain_id: u32,
) -> Vec<TrustTransfer> {
    let want = address_topic(address);
    let mut out = Vec::new();
    for log in raw_logs {
        if log.topics.len() < 3 {
            continue;
        }
        let recipient = log
            .topics
            .get(2)
            .map(|t| t.to_lowercase())
            .unwrap_or_default();
        if recipient != want {
            continue;
        }
        // `'0x' + topics[1].slice(26)` — case preserved, verbatim.
        let from = format!(
            "0x{}",
            log.topics.get(1).and_then(|t| t.get(26..)).unwrap_or("")
        );
        let Ok(value) = hex_value_u128(&log.data) else {
            // Invalid hex → the TS try/catch skips; overflow → fail-closed.
            continue;
        };
        if value == 0 {
            continue;
        }
        let contract = log.address.to_lowercase();
        let is_native = NATIVE_LOG_ADDRESSES.contains(&contract.as_str());
        // JS `parseInt` would yield NaN here and thread it into the id; NaN
        // has no u32/u64 counterpart, so a garbled index/block drops the log
        // (fail-closed, ported with a note).
        let Some(log_index) = parse_hex_u64(log.log_index.as_deref().unwrap_or("0x0")) else {
            continue;
        };
        let Ok(log_index) = u32::try_from(log_index) else {
            continue;
        };
        let Some(block_number) = parse_hex_u64(log.block_number.as_deref().unwrap_or("0x0")) else {
            continue;
        };
        out.push(TrustTransfer {
            id: format!("{chain_id}-{}-{log_index}", log.transaction_hash),
            chain_id,
            token: if is_native {
                None
            } else {
                Some(contract.clone())
            },
            is_native,
            from,
            value,
            tx_hash: log.transaction_hash.clone(),
            block_number,
            log_index,
        });
    }
    out
}

/// `deriveAssetDeltas` (`sim-assets.ts:134-166`): net the value transfers in
/// `logs` that touch `user`, one signed delta per asset. Only fungible
/// Transfer logs count — exactly three topics, value in `data`. Self-transfers
/// cancel to zero and net-zero assets are dropped.
///
/// Fail-closed deviation from unbounded JS BigInt: an amount or a running net
/// that exceeds i128 poisons that asset and it is dropped entirely — for the
/// admission decision (which only reads the sign) "dropped" is the safe side.
pub fn derive_asset_deltas(logs: &[TrustReceiptLog], user: &str) -> Vec<TrustNetDelta> {
    struct Acc {
        is_native: bool,
        token: Option<String>,
        delta: i128,
        poisoned: bool,
    }
    let u = user.to_lowercase();
    let mut order: Vec<String> = Vec::new();
    let mut acc: BTreeMap<String, Acc> = BTreeMap::new();

    for log in logs {
        if log.topics.len() != 3 {
            continue;
        }
        let topic0 = log
            .topics
            .first()
            .map(|t| t.to_lowercase())
            .unwrap_or_default();
        if topic0 != TRANSFER_TOPIC {
            continue;
        }
        let from = topic_to_address(log.topics.get(1).map(String::as_str).unwrap_or(""));
        let to = topic_to_address(log.topics.get(2).map(String::as_str).unwrap_or(""));
        if from != u && to != u {
            continue;
        }
        let value = first_word_value(&log.data);
        let addr_lc = log.address.to_lowercase();
        let is_native = NATIVE_LOG_ADDRESSES.contains(&addr_lc.as_str());
        let key = if is_native {
            "native".to_owned()
        } else {
            addr_lc.clone()
        };
        if key.is_empty() {
            continue;
        }
        let entry = acc.entry(key.clone()).or_insert_with(|| {
            order.push(key.clone());
            Acc {
                is_native,
                token: if is_native {
                    None
                } else {
                    Some(addr_lc.clone())
                },
                delta: 0,
                poisoned: false,
            }
        });
        let signed = match value {
            Ok(0) => continue,
            Ok(v) => match i128::try_from(v) {
                Ok(v) => v,
                Err(_) => {
                    entry.poisoned = true;
                    continue;
                }
            },
            Err(HexValue::Invalid) => continue, // JS BigInt throws → 0n → skipped
            Err(HexValue::Overflow) => {
                entry.poisoned = true;
                continue;
            }
        };
        if to == u {
            match entry.delta.checked_add(signed) {
                Some(d) => entry.delta = d,
                None => entry.poisoned = true,
            }
        }
        if from == u {
            match entry.delta.checked_sub(signed) {
                Some(d) => entry.delta = d,
                None => entry.poisoned = true,
            }
        }
    }

    order
        .into_iter()
        .filter_map(|key| {
            let a = acc.remove(&key)?;
            (!a.poisoned && a.delta != 0).then_some(TrustNetDelta {
                is_native: a.is_native,
                token: a.token,
                delta: a.delta,
            })
        })
        .collect()
}

/// The ERC-20s a confirmed receipt NET-delivered to `from`, lowercased, in
/// first-appearance order (`token-autoadd.ts:39-43`). One entry per token —
/// the netting already merged duplicates.
pub fn auto_add_candidates(logs: &[TrustReceiptLog], from: &str) -> Vec<String> {
    derive_asset_deltas(logs, from)
        .into_iter()
        .filter(|d| !d.is_native && d.delta > 0)
        .filter_map(|d| d.token)
        .collect()
}

/// Decision chain 2 — token → admission (`token-autoadd.ts:39-67`), as a
/// single predicate: a token is admitted iff it was net-received from
/// authentic logs AND is not already listed, not held, not in the registry
/// on its chain, AND its symbol resolved on-chain (invariants ⑤⑧).
pub fn admission_allows(
    net_received: bool,
    already_listed: bool,
    held: bool,
    known: bool,
    symbol: Option<&str>,
) -> bool {
    net_received && !already_listed && !held && !known && symbol.is_some_and(|s| !s.is_empty())
}

/// The per-chain `eth_getLogs` allowlist (`transferAllowlist`,
/// `transfer-monitor.ts:208-221`): the EIP-7708 native sentinels + the
/// chain's known stablecoins + the user's custom tokens on that chain. All
/// lowercased, first-occurrence de-duped. Tokens the user holds but never
/// added are intentionally NOT included — listening to them is exactly how
/// spam slips in (invariant ②).
pub fn allowlist_for_chain(
    stables: &[String],
    custom_tokens: &[TrustCustomToken],
    chain_id: u32,
) -> Vec<String> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    let mut push = |addr: String| {
        if !addr.is_empty() && seen.insert(addr.clone()) {
            out.push(addr);
        }
    };
    for sentinel in NATIVE_LOG_ADDRESSES {
        push(sentinel.to_owned());
    }
    for stable in stables {
        push(stable.to_lowercase());
    }
    for token in custom_tokens.iter().filter(|t| t.chain_id == chain_id) {
        push(token.contract_address.to_lowercase());
    }
    out
}

/// The judgment surfaced for one simulated delta.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum TrustSimJudgment {
    /// Native value moves render whenever they appear — the chain symbol and
    /// 18 decimals are the shell's vocabulary.
    Native { delta: String },
    /// Metadata resolved AND (an outflow, or a trusted-set inflow).
    Erc20Trusted {
        token: String,
        delta: String,
        symbol: String,
        decimals: u32,
        /// The token is one this wallet already trusts — the chain's registry
        /// stables or wrapped coin, a token the account holds —
        /// and not merely one whose `symbol()` answered. Always so for an
        /// inflow (that is what earned it a figure); an OUTFLOW renders on
        /// metadata alone, so this is what tells a known coin leaving from a
        /// contract's own claim to be one (083 F1 review: a site's token that
        /// emits `Transfer(you, …)` and answers "USDC" is `false` here).
        /// Absent on the wire when `false`, so older readers are unchanged.
        #[serde(default, skip_serializing_if = "std::ops::Not::not")]
        in_trusted_set: bool,
    },
    /// Direction + caution, no attacker-controlled amount rendering
    /// (`tx-simulation.ts:243-256`).
    Erc20Unverified {
        token: Option<String>,
        delta: String,
    },
}

/// Asymmetric-trust judgment for one delta (`enrichDeltas`,
/// `tx-simulation.ts:257-286`). SENT (negative) renders whenever metadata
/// resolved — the real token emits its own log, so an outflow can't be
/// understated. RECEIVED (positive) renders a confident amount only when the
/// token is in the chain's trusted set (invariant ⑥) — the caller's verdict:
/// the chain's registry stables and wrapped coin (the service's, or this
/// build's [`registry_token`]) and the tokens the account holds. Everything
/// else — no metadata, no token address, an unparseable delta — is
/// unverified, never an error and never a default (invariant ⑦).
pub fn judge_delta(
    delta: &TrustAssetDelta,
    meta: Option<&TrustTokenMeta>,
    in_trusted_set: bool,
) -> TrustSimJudgment {
    if delta.kind == TrustDeltaKind::Native {
        return TrustSimJudgment::Native {
            delta: delta.delta.clone(),
        };
    }
    let unverified = TrustSimJudgment::Erc20Unverified {
        token: delta.token.clone(),
        delta: delta.delta.clone(),
    };
    let Some(token) = delta.token.as_deref() else {
        return unverified;
    };
    // A delta this core cannot even read the sign of gets the cautious
    // treatment (fail-closed; the TS deltas are in-process bigints and can't
    // be malformed, so this branch has no JS counterpart).
    let Some(received) = parse_delta_positive(&delta.delta) else {
        return unverified;
    };
    let trusted = in_trusted_set;
    // ERC-20 `decimals` is a uint8: metadata claiming more is not readable
    // metadata (082 review) — the amount at that width is no amount.
    let readable = |m: &TrustTokenMeta| {
        !m.symbol.is_empty() && m.decimals <= crate::l10n::number::MAX_TOKEN_DECIMALS
    };
    match meta {
        Some(m) if readable(m) && (!received || trusted) => TrustSimJudgment::Erc20Trusted {
            token: token.to_owned(),
            delta: delta.delta.clone(),
            symbol: m.symbol.clone(),
            decimals: m.decimals,
            in_trusted_set: trusted,
        },
        _ => unverified,
    }
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
struct RegistryEntry {
    stables: Vec<String>,
    wrapped: Option<String>,
}

#[derive(Clone, Debug)]
struct FeedItem {
    transfer: TrustTransfer,
    timestamp_sec: f64,
    symbol: Option<String>,
    decimals: Option<u32>,
}

#[derive(Clone, Debug)]
enum ChainPhase {
    AwaitingBlockNumber,
    /// First getLogs out (invariant ④'s "probe").
    Probe {
        latest: u64,
    },
    /// The one capped retry is out — a second cap ends the chain (④).
    Retry {
        from: u64,
        latest: u64,
    },
    /// The transfer logs are in; the wallet's own `SafeReceived` over the
    /// same span is out (issue #443).
    SafeReceived {
        transfer_logs: Vec<TrustRawLog>,
    },
    Timestamps {
        transfers: Vec<TrustTransfer>,
        pending: BTreeSet<u64>,
        resolved: BTreeMap<u64, f64>,
        fallback_sec: f64,
    },
    Meta {
        transfers: Vec<TrustTransfer>,
        resolved: BTreeMap<u64, f64>,
        fallback_sec: f64,
        needed: BTreeSet<String>,
    },
}

#[derive(Clone, Debug)]
struct ChainScan {
    allowlist: Vec<String>,
    phase: ChainPhase,
}

#[derive(Clone, Debug)]
struct PendingAutoAdd {
    address: String,
    chain_id: u32,
    received: Vec<String>,
}

#[derive(Clone, Debug)]
#[allow(clippy::enum_variant_names)] // the phases are named for what they await; that is the axis
enum AutoAddPhase {
    AwaitingCustoms,
    AwaitingMeta { fresh: Vec<String> },
    AwaitingWrites { remaining: usize, failed: bool },
}

#[derive(Clone, Debug)]
struct AutoAddSession {
    address: String,
    chain_id: u32,
    received: Vec<String>,
    phase: AutoAddPhase,
}

#[derive(Clone, Debug)]
struct SimSession {
    address: String,
    chain_id: u32,
    deltas: Vec<TrustAssetDelta>,
    needed: BTreeSet<String>,
    judgments: Option<Vec<TrustSimJudgment>>,
}

#[derive(Default)]
pub struct Model {
    /// The active account, lowercased. Set by the first address-bearing
    /// snapshot/poll; a different address resets every account-scoped field.
    address: Option<String>,
    /// From [`Event::HeldChainsSnapshot`]; empty ⇒ [`DEFAULT_MONITOR_CHAINS`].
    scan_chains: Vec<u32>,
    registry: BTreeMap<u32, RegistryEntry>,
    custom: Vec<TrustCustomToken>,
    /// address → chain → held token set (`getCachedHeldTokens` semantics:
    /// anything not snapshotted is simply empty — the safe direction).
    held: BTreeMap<String, BTreeMap<u32, BTreeSet<String>>>,
    /// (chain, addr) → metadata; `Some(None)` is the session negative memo —
    /// a known dud is not re-queried but never gets invented defaults (⑦).
    meta: BTreeMap<(u32, String), Option<TrustTokenMeta>>,
    /// The judged incoming feed for the active account, keyed by stable id
    /// (overlapping scan windows de-dupe here).
    feed: BTreeMap<String, FeedItem>,
    /// In-flight poll, per chain.
    scan: BTreeMap<u32, ChainScan>,
    /// Poll bootstrap: waiting on `ReadCustomTokens` before fanning out.
    poll_pending: bool,
    auto_add: Option<AutoAddSession>,
    auto_add_queue: VecDeque<PendingAutoAdd>,
    sim: Option<SimSession>,
    /// Bumped on account switch — poisons in-flight scan results even when
    /// the user switches away and back to the same address.
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// One judged incoming transfer, metadata attached (native rows have none —
/// the chain symbol is the shell's).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustIncomingView {
    pub id: String,
    pub chain_id: u32,
    pub token: Option<String>,
    pub is_native: bool,
    pub from: String,
    /// Raw on-chain amount as a decimal string (not divided by decimals).
    pub value: String,
    pub tx_hash: String,
    pub block_number: f64,
    pub log_index: u32,
    /// Unix seconds (block time, falling back to receipt time).
    pub timestamp_sec: f64,
    pub symbol: Option<String>,
    pub decimals: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustSimView {
    pub address: String,
    pub chain_id: u32,
    /// False while metadata for the judgment is still resolving.
    pub ready: bool,
    pub judgments: Vec<TrustSimJudgment>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TrustView {
    pub address: Option<String>,
    /// A poll is in progress.
    pub scanning: bool,
    /// Newest first (block desc, then log index desc) —
    /// `fetchIncomingTransfers`' sort.
    pub incoming: Vec<TrustIncomingView>,
    pub sim: Option<TrustSimView>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct TokenTrust;

impl App for TokenTrust {
    type Event = Event;
    type Model = Model;
    type ViewModel = TrustView;
    type Effect = TrustEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<TrustEffect, Event> {
        match event {
            Event::HeldChainsSnapshot { address, chain_ids } => {
                let lc = address.to_lowercase();
                adopt_address(model, &lc);
                model.scan_chains = dedupe_chains(&chain_ids);
                render()
            }
            Event::HeldTokensSnapshot {
                address,
                chain_id,
                tokens,
            } => {
                let set: BTreeSet<String> = tokens.iter().map(|t| t.to_lowercase()).collect();
                model
                    .held
                    .entry(address.to_lowercase())
                    .or_default()
                    .insert(chain_id, set);
                Command::done()
            }
            Event::RegistryTokensSnapshot {
                chain_id,
                stables,
                wrapped_native,
            } => {
                model.registry.insert(
                    chain_id,
                    RegistryEntry {
                        stables: stables.iter().map(|s| s.to_lowercase()).collect(),
                        wrapped: wrapped_native.map(|w| w.to_lowercase()),
                    },
                );
                Command::done()
            }
            Event::CustomTokensLoaded { tokens } => {
                model.custom = tokens;
                Command::done()
            }
            Event::PollRequested { address } => poll_requested(model, &address),
            Event::ReceiptLogsConfirmed {
                from,
                chain_id,
                logs,
            } => receipt_confirmed(model, &from, chain_id, &logs),
            Event::SimDeltasComputed {
                address,
                chain_id,
                deltas,
            } => sim_requested(model, &address, chain_id, deltas),
            Event::ShellCompleted { attempt, result } => accept(model, attempt, result),
        }
    }

    fn view(&self, model: &Model) -> TrustView {
        let mut incoming: Vec<TrustIncomingView> = model
            .feed
            .values()
            .map(|item| TrustIncomingView {
                id: item.transfer.id.clone(),
                chain_id: item.transfer.chain_id,
                token: item.transfer.token.clone(),
                is_native: item.transfer.is_native,
                from: item.transfer.from.clone(),
                value: item.transfer.value.to_string(),
                tx_hash: item.transfer.tx_hash.clone(),
                block_number: item.transfer.block_number as f64,
                log_index: item.transfer.log_index,
                timestamp_sec: item.timestamp_sec,
                symbol: item.symbol.clone(),
                decimals: item.decimals,
            })
            .collect();
        incoming.sort_by(|a, b| {
            b.block_number
                .total_cmp(&a.block_number)
                .then_with(|| b.log_index.cmp(&a.log_index))
                .then_with(|| a.id.cmp(&b.id))
        });
        TrustView {
            address: model.address.clone(),
            scanning: model.poll_pending || !model.scan.is_empty(),
            incoming,
            sim: model.sim.as_ref().map(|s| TrustSimView {
                address: s.address.clone(),
                chain_id: s.chain_id,
                ready: s.judgments.is_some(),
                judgments: s.judgments.clone().unwrap_or_default(),
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// User/shell-initiated transitions
// ---------------------------------------------------------------------------

/// A different address means an account switch: the feed and any in-flight
/// scan belong to the old account and are discarded; the attempt bump poisons
/// even a switch-away-and-back to the same address. The admission pipeline
/// and the sim session are NOT account-display state — they carry their own
/// address and survive (a half-written admission must finish; custom-token
/// storage is global).
fn adopt_address(model: &mut Model, lc: &str) {
    if model.address.as_deref() == Some(lc) {
        return;
    }
    model.address = Some(lc.to_owned());
    model.feed.clear();
    model.scan.clear();
    model.poll_pending = false;
    model.scan_chains = Vec::new();
    model.attempt += 1;
}

fn poll_requested(model: &mut Model, address: &str) -> Command<TrustEffect, Event> {
    let lc = address.to_lowercase();
    adopt_address(model, &lc);
    // Single-flight: while a poll runs, another request is ignored — the
    // Activity surface's next 10s tick retries (`_seedPromise`-style).
    if model.poll_pending || !model.scan.is_empty() {
        return Command::done();
    }
    model.poll_pending = true;
    // Fresh custom-token read at poll start, exactly as `transferAllowlist`
    // does (`transfer-monitor.ts:209`).
    Command::all([
        shell_request(model.attempt, TrustOperation::ReadCustomTokens),
        render(),
    ])
}

fn receipt_confirmed(
    model: &mut Model,
    from: &str,
    chain_id: u32,
    logs: &[TrustReceiptLog],
) -> Command<TrustEffect, Event> {
    // `if (!from || !Array.isArray(logs) || logs.length === 0) return 0`.
    if from.is_empty() || logs.is_empty() {
        return Command::done();
    }
    let received = auto_add_candidates(logs, from);
    if received.is_empty() {
        return Command::done();
    }
    model.auto_add_queue.push_back(PendingAutoAdd {
        address: from.to_lowercase(),
        chain_id,
        received,
    });
    match start_next_auto_add(model) {
        Some(command) => command,
        None => Command::done(), // one already running — queued behind it
    }
}

fn sim_requested(
    model: &mut Model,
    address: &str,
    chain_id: u32,
    deltas: Vec<TrustAssetDelta>,
) -> Command<TrustEffect, Event> {
    // Latest wins — the sign sheet shows one preview at a time.
    let mut needed: BTreeSet<String> = BTreeSet::new();
    for delta in &deltas {
        if delta.kind != TrustDeltaKind::Erc20 {
            continue;
        }
        let Some(token) = delta.token.as_deref() else {
            continue;
        };
        let lc = token.to_lowercase();
        // `resolveTokenMetadata` consults the registry, then its caches;
        // only real misses go on-chain (`token-metadata.ts:58-74`).
        if registry_token(chain_id, &lc).is_none()
            && !model.meta.contains_key(&(chain_id, lc.clone()))
        {
            needed.insert(lc);
        }
    }
    let mut session = SimSession {
        address: address.to_lowercase(),
        chain_id,
        deltas,
        needed: needed.clone(),
        judgments: None,
    };
    if needed.is_empty() {
        session.judgments = Some(judge_session(model, &session));
        model.sim = Some(session);
        return render();
    }
    model.sim = Some(session);
    Command::all([
        shell_request(
            model.attempt,
            TrustOperation::MulticallErc20Meta {
                chain_id,
                addrs: needed.into_iter().collect(),
            },
        ),
        render(),
    ])
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(
    model: &mut Model,
    attempt: u64,
    result: TrustShellResult,
) -> Command<TrustEffect, Event> {
    match result {
        // -- account-scoped scan pipeline: attempt + address tags ------------
        TrustShellResult::BlockNumber {
            address,
            chain_id,
            block_hex,
        } => {
            if attempt != model.attempt || model.address.as_deref() != Some(address.as_str()) {
                return Command::done();
            }
            on_block_number(model, chain_id, block_hex)
        }
        TrustShellResult::Logs {
            address,
            chain_id,
            outcome,
        } => {
            if attempt != model.attempt || model.address.as_deref() != Some(address.as_str()) {
                return Command::done();
            }
            on_logs(model, chain_id, outcome)
        }
        TrustShellResult::SafeReceivedLogs {
            address,
            chain_id,
            outcome,
        } => {
            if attempt != model.attempt || model.address.as_deref() != Some(address.as_str()) {
                return Command::done();
            }
            on_safe_received(model, chain_id, outcome)
        }
        TrustShellResult::BlockTimestamp {
            address,
            chain_id,
            block_number,
            timestamp_sec,
            now_ms,
        } => {
            if attempt != model.attempt || model.address.as_deref() != Some(address.as_str()) {
                return Command::done();
            }
            on_block_timestamp(model, chain_id, block_number, timestamp_sec, now_ms)
        }

        // -- chain-global facts: absorbed unconditionally ---------------------
        TrustShellResult::ErcMeta { chain_id, entries } => on_meta(model, chain_id, entries),

        // -- shared read: the poll bootstrap first, else the admission --------
        TrustShellResult::CustomTokens { tokens } => {
            if model.poll_pending && attempt == model.attempt {
                on_poll_customs(model, tokens)
            } else {
                on_auto_add_customs(model, tokens)
            }
        }

        // -- admission pipeline: correlated to the single active session ------
        TrustShellResult::TokenWritten { ok } => on_token_written(model, ok),
        TrustShellResult::CacheInvalidated => Command::done(),
    }
}

// -- poll -------------------------------------------------------------------

fn on_poll_customs(
    model: &mut Model,
    tokens: Option<Vec<TrustCustomToken>>,
) -> Command<TrustEffect, Event> {
    model.poll_pending = false;
    let Some(address) = model.address.clone() else {
        return render();
    };
    // A failed read degrades to no customs for THIS poll's allowlist
    // (`loadCustomTokens().catch(() => [])`); a successful one also refreshes
    // the warm copy.
    let customs = match tokens {
        Some(tokens) => {
            model.custom = tokens.clone();
            tokens
        }
        None => Vec::new(),
    };
    let chains = if model.scan_chains.is_empty() {
        DEFAULT_MONITOR_CHAINS.to_vec()
    } else {
        model.scan_chains.clone()
    };
    let attempt = model.attempt;
    let mut commands = Vec::new();
    for chain_id in chains {
        let stables = model
            .registry
            .get(&chain_id)
            .map(|r| r.stables.clone())
            .unwrap_or_default();
        model.scan.insert(
            chain_id,
            ChainScan {
                allowlist: allowlist_for_chain(&stables, &customs, chain_id),
                phase: ChainPhase::AwaitingBlockNumber,
            },
        );
        commands.push(shell_request(
            attempt,
            TrustOperation::RpcBlockNumber {
                address: address.clone(),
                chain_id,
            },
        ));
    }
    commands.push(render());
    Command::all(commands)
}

fn on_block_number(
    model: &mut Model,
    chain_id: u32,
    block_hex: Option<String>,
) -> Command<TrustEffect, Event> {
    let Some(entry) = model.scan.get_mut(&chain_id) else {
        return Command::done();
    };
    if !matches!(entry.phase, ChainPhase::AwaitingBlockNumber) {
        return Command::done();
    }
    // `!Number.isFinite(latest) || latest <= 0` → the chain yields nothing
    // this tick (`scanChain`'s catch).
    let latest = block_hex
        .as_deref()
        .and_then(parse_hex_u64)
        .filter(|l| *l > 0);
    let Some(latest) = latest else {
        model.scan.remove(&chain_id);
        return render();
    };
    // Ported quirk: `from = latest - lookback` spans lookback+1 blocks
    // inclusive (the retry's `span - 1` is the exact-span variant).
    let from = latest.saturating_sub(LIVE_SCAN_BLOCKS);
    entry.phase = ChainPhase::Probe { latest };
    let op = get_logs_op(model, chain_id, from, latest);
    Command::all([op, render()])
}

fn on_logs(
    model: &mut Model,
    chain_id: u32,
    outcome: TrustLogsOutcome,
) -> Command<TrustEffect, Event> {
    let Some(entry) = model.scan.get(&chain_id) else {
        return Command::done();
    };
    match (&entry.phase, outcome) {
        (ChainPhase::Probe { .. } | ChainPhase::Retry { .. }, TrustLogsOutcome::Ok { logs }) => {
            // Issue #443: the same span, for the wallet's own `SafeReceived`.
            let (from, latest) = match entry.phase {
                ChainPhase::Probe { latest } => (latest.saturating_sub(LIVE_SCAN_BLOCKS), latest),
                ChainPhase::Retry { from, latest } => (from, latest),
                _ => unreachable!("matched above"),
            };
            if let Some(entry) = model.scan.get_mut(&chain_id) {
                entry.phase = ChainPhase::SafeReceived {
                    transfer_logs: logs,
                };
            }
            let op = safe_received_op(model, chain_id, from, latest);
            Command::all([op, render()])
        }
        (ChainPhase::Probe { latest }, TrustLogsOutcome::RangeCapped { cap }) => {
            // Span cap: retry ONCE for just the most-recent span — never a
            // chunked fan-out (invariant ④). A cap with no usable number
            // HALVES the window, as `get_logs_range_cap` promises its callers:
            // retrying 100 after a 101-block refusal was refused again, and
            // the chain found nothing on such an endpoint (issue #443).
            let latest = *latest;
            let span = if cap > 0 {
                u64::from(cap)
            } else {
                LIVE_SCAN_BLOCKS / 2
            };
            let from = latest.saturating_sub(span.saturating_sub(1));
            if let Some(entry) = model.scan.get_mut(&chain_id) {
                entry.phase = ChainPhase::Retry { from, latest };
            }
            let op = get_logs_op(model, chain_id, from, latest);
            Command::all([op, render()])
        }
        // A second cap, or any hard failure: this chain yields nothing this
        // tick — windows overlap, the next poll retries.
        (ChainPhase::Retry { .. }, TrustLogsOutcome::RangeCapped { .. })
        | (ChainPhase::Probe { .. } | ChainPhase::Retry { .. }, TrustLogsOutcome::Failed) => {
            model.scan.remove(&chain_id);
            render()
        }
        _ => Command::done(),
    }
}

/// Issue #443: the wallet's own `SafeReceived` logs join the transfer logs as
/// native receipts. A span the endpoint will not serve for this query, or a
/// failure, costs only the native receipts — the transfers still land.
fn on_safe_received(
    model: &mut Model,
    chain_id: u32,
    outcome: TrustLogsOutcome,
) -> Command<TrustEffect, Event> {
    let Some(entry) = model.scan.get_mut(&chain_id) else {
        return Command::done();
    };
    let ChainPhase::SafeReceived { transfer_logs } = &mut entry.phase else {
        return Command::done();
    };
    let mut logs = std::mem::take(transfer_logs);
    if let TrustLogsOutcome::Ok { logs: received } = outcome {
        let address = model.address.clone().unwrap_or_default();
        logs = merge_safe_received(logs, &received, &address);
    }
    begin_timestamps(model, chain_id, &logs)
}

/// The logs' transfers, on to their blocks' timestamps — or the chain is done
/// when none is this wallet's.
fn begin_timestamps(
    model: &mut Model,
    chain_id: u32,
    logs: &[TrustRawLog],
) -> Command<TrustEffect, Event> {
    let address = model.address.clone().unwrap_or_default();
    let transfers = decode_transfer_logs(logs, &address, chain_id);
    if transfers.is_empty() {
        model.scan.remove(&chain_id);
        return render();
    }
    // Distinct blocks in first-seen order, capped at 25; the rest
    // fall back to now (`resolveTimestamps`).
    let mut blocks: Vec<u64> = Vec::new();
    for t in &transfers {
        if !blocks.contains(&t.block_number) {
            blocks.push(t.block_number);
        }
    }
    blocks.truncate(TIMESTAMP_BLOCK_CAP);
    let attempt = model.attempt;
    let mut commands = Vec::new();
    for block in &blocks {
        commands.push(shell_request(
            attempt,
            TrustOperation::RpcGetBlockByNumber {
                address: address.clone(),
                chain_id,
                block: format!("0x{block:x}"),
            },
        ));
    }
    if let Some(entry) = model.scan.get_mut(&chain_id) {
        entry.phase = ChainPhase::Timestamps {
            transfers,
            pending: blocks.iter().copied().collect(),
            resolved: BTreeMap::new(),
            fallback_sec: 0.0,
        };
    }
    commands.push(render());
    Command::all(commands)
}

fn on_block_timestamp(
    model: &mut Model,
    chain_id: u32,
    block_number: f64,
    timestamp_sec: Option<f64>,
    now_ms: f64,
) -> Command<TrustEffect, Event> {
    let Some(entry) = model.scan.get_mut(&chain_id) else {
        return Command::done();
    };
    let ChainPhase::Timestamps {
        pending,
        resolved,
        fallback_sec,
        ..
    } = &mut entry.phase
    else {
        return Command::done();
    };
    let block = exact_u64(block_number);
    let Some(block) = block else {
        return Command::done();
    };
    if !pending.remove(&block) {
        return Command::done();
    }
    if let Some(sec) = timestamp_sec {
        resolved.insert(block, sec);
    }
    // `Math.floor(Date.now() / 1000)` — the fallback for failed lookups and
    // beyond-cap blocks, from the shell's clock (never the core's).
    *fallback_sec = (now_ms / 1000.0).floor();
    if !pending.is_empty() {
        return render();
    }

    // All timestamps answered → the metadata gate (invariant ③).
    let ChainPhase::Timestamps {
        transfers,
        resolved,
        fallback_sec,
        ..
    } = std::mem::replace(&mut entry.phase, ChainPhase::Retry { from: 0, latest: 0 })
    else {
        return Command::done();
    };
    let needed = meta_needed_for_scan(model, chain_id, &transfers);
    if needed.is_empty() {
        finalize_chain(model, chain_id, transfers, &resolved, fallback_sec);
        return render();
    }
    let attempt = model.attempt;
    let op = shell_request(
        attempt,
        TrustOperation::MulticallErc20Meta {
            chain_id,
            addrs: needed.iter().cloned().collect(),
        },
    );
    if let Some(entry) = model.scan.get_mut(&chain_id) {
        entry.phase = ChainPhase::Meta {
            transfers,
            resolved,
            fallback_sec,
            needed,
        };
    }
    Command::all([op, render()])
}

/// Tokens among `transfers` whose metadata is still a true unknown: not in
/// the registry on their chain, not carried by a custom-token record, no
/// cache entry (not even a negative memo).
fn meta_needed_for_scan(
    model: &Model,
    chain_id: u32,
    transfers: &[TrustTransfer],
) -> BTreeSet<String> {
    let mut needed = BTreeSet::new();
    for t in transfers {
        let Some(token) = t.token.as_deref() else {
            continue;
        };
        if scan_meta_of(model, chain_id, token).is_none()
            && !model.meta.contains_key(&(chain_id, token.to_owned()))
        {
            needed.insert(token.to_owned());
        }
    }
    needed
}

/// Metadata for a scanned token: the registry on its chain, then the user's
/// own custom record (a listed token carries its symbol/decimals —
/// `buildTokenIndex` territory), then the resolved cache.
fn scan_meta_of(model: &Model, chain_id: u32, token: &str) -> Option<TrustTokenMeta> {
    if let Some(known) = registry_meta(chain_id, token) {
        return Some(known);
    }
    if let Some(custom) = model
        .custom
        .iter()
        .find(|c| c.chain_id == chain_id && c.contract_address.to_lowercase() == token)
    {
        return Some(TrustTokenMeta {
            symbol: custom.symbol.clone(),
            decimals: custom.decimals,
        });
    }
    model
        .meta
        .get(&(chain_id, token.to_owned()))
        .cloned()
        .flatten()
}

/// Emit a finished chain's transfers into the judged feed. Native rows always
/// pass; an ERC-20 without resolvable metadata is withheld — persisting it
/// would store a misleading "+0 tokens" (invariant ③, `activity.ts:408-415`).
/// It stays inside the scan window and is retried by a later poll.
fn finalize_chain(
    model: &mut Model,
    chain_id: u32,
    transfers: Vec<TrustTransfer>,
    resolved: &BTreeMap<u64, f64>,
    fallback_sec: f64,
) {
    model.scan.remove(&chain_id);
    for transfer in transfers {
        let timestamp_sec = resolved
            .get(&transfer.block_number)
            .copied()
            .unwrap_or(fallback_sec);
        let (symbol, decimals) = match transfer.token.as_deref() {
            None => (None, None),
            Some(token) => match scan_meta_of(model, chain_id, token) {
                Some(meta) => (Some(meta.symbol), Some(meta.decimals)),
                None => continue, // ③ — no metadata, no feed entry
            },
        };
        model.feed.insert(
            transfer.id.clone(),
            FeedItem {
                transfer,
                timestamp_sec,
                symbol,
                decimals,
            },
        );
    }
}

// -- metadata facts ---------------------------------------------------------

/// Metadata answers are absorbed as facts, then every flow waiting on them —
/// scan chains, the admission session, the sim judgment — re-checks its
/// needs. Facts have no owner, which is what makes the correlation ambiguity
/// between concurrent flows harmless.
fn on_meta(
    model: &mut Model,
    chain_id: u32,
    entries: Vec<TrustMetaEntry>,
) -> Command<TrustEffect, Event> {
    for entry in entries {
        let lc = entry.addr.to_lowercase();
        // Session memo, negatives included (`token-metadata.ts:126`) — a dud
        // is not re-queried, and never gets defaults (invariant ⑦).
        model.meta.insert((chain_id, lc), entry.meta);
    }

    let mut commands = Vec::new();

    // Scan chains whose metadata gate is now satisfied.
    let ready_chains: Vec<u32> = model
        .scan
        .iter()
        .filter_map(|(id, entry)| match &entry.phase {
            ChainPhase::Meta { needed, .. }
                if needed
                    .iter()
                    .all(|t| model.meta.contains_key(&(*id, t.clone()))) =>
            {
                Some(*id)
            }
            _ => None,
        })
        .collect();
    for id in ready_chains {
        if let Some(entry) = model.scan.get_mut(&id) {
            if let ChainPhase::Meta {
                transfers,
                resolved,
                fallback_sec,
                ..
            } = std::mem::replace(&mut entry.phase, ChainPhase::Retry { from: 0, latest: 0 })
            {
                finalize_chain(model, id, transfers, &resolved, fallback_sec);
            }
        }
    }

    // The admission session's fresh list.
    let auto_ready = matches!(
        &model.auto_add,
        Some(session) if matches!(&session.phase, AutoAddPhase::AwaitingMeta { fresh }
            if fresh
                .iter()
                .all(|t| model.meta.contains_key(&(session.chain_id, t.clone()))))
    );
    if auto_ready {
        commands.push(auto_add_write_phase(model));
    }

    // The sim judgment.
    let sim_ready = matches!(
        &model.sim,
        Some(session) if session.judgments.is_none()
            && session
                .needed
                .iter()
                .all(|t| model.meta.contains_key(&(session.chain_id, t.clone())))
    );
    if sim_ready {
        if let Some(session) = model.sim.take() {
            let judged = judge_session(model, &session);
            model.sim = Some(SimSession {
                judgments: Some(judged),
                ..session
            });
        }
    }

    commands.push(render());
    Command::all(commands)
}

// -- admission (auto-add) ---------------------------------------------------

fn start_next_auto_add(model: &mut Model) -> Option<Command<TrustEffect, Event>> {
    if model.auto_add.is_some() {
        return None;
    }
    let pending = model.auto_add_queue.pop_front()?;
    model.auto_add = Some(AutoAddSession {
        address: pending.address,
        chain_id: pending.chain_id,
        received: pending.received,
        phase: AutoAddPhase::AwaitingCustoms,
    });
    Some(shell_request(
        model.attempt,
        TrustOperation::ReadCustomTokens,
    ))
}

fn on_auto_add_customs(
    model: &mut Model,
    tokens: Option<Vec<TrustCustomToken>>,
) -> Command<TrustEffect, Event> {
    let Some(session) = model.auto_add.as_ref() else {
        return Command::done();
    };
    if !matches!(session.phase, AutoAddPhase::AwaitingCustoms) {
        return Command::done();
    }
    // A failed read aborts the whole admission with 0 added — in TS the
    // un-caught `loadCustomTokens` rejection lands in the outer catch
    // (`token-autoadd.ts:83-85`). Fail-closed: nothing is written.
    let Some(tokens) = tokens else {
        return finish_auto_add(model, None);
    };
    let (address, chain_id, received) = {
        let s = model
            .auto_add
            .as_ref()
            .map(|s| (s.address.clone(), s.chain_id, s.received.clone()));
        match s {
            Some(t) => t,
            None => return Command::done(),
        }
    };
    let listed: BTreeSet<String> = tokens
        .iter()
        .filter(|t| t.chain_id == chain_id)
        .map(|t| t.contract_address.to_lowercase())
        .collect();
    let held = held_for(model, &address, chain_id);
    // The admission filter (`token-autoadd.ts:54-56`): skip anything already
    // visible — listed, held, or in the registry on this chain.
    let fresh: Vec<String> = received
        .into_iter()
        .filter(|addr| {
            admission_allows(
                true, // net-received by construction of `received`
                listed.contains(addr),
                held.contains(addr),
                registry_token(chain_id, addr).is_some(),
                Some("pending"), // the symbol gate applies after resolution
            )
        })
        .collect();
    if fresh.is_empty() {
        return finish_auto_add(model, None);
    }
    let needed: Vec<String> = fresh
        .iter()
        .filter(|addr| !model.meta.contains_key(&(chain_id, (*addr).clone())))
        .cloned()
        .collect();
    if let Some(session) = model.auto_add.as_mut() {
        session.phase = AutoAddPhase::AwaitingMeta { fresh };
    }
    if needed.is_empty() {
        // Everything already resolved this session — write straight away.
        return auto_add_write_phase(model);
    }
    shell_request(
        model.attempt,
        TrustOperation::MulticallErc20Meta {
            chain_id,
            addrs: needed,
        },
    )
}

/// Metadata is in — write every admissible token. No symbol ⇒ no listing
/// (invariant ⑧, "don't seed a '?'"); `name` defaults to the symbol
/// (`token-autoadd.ts:60-76`).
fn auto_add_write_phase(model: &mut Model) -> Command<TrustEffect, Event> {
    let Some(session) = model.auto_add.as_ref() else {
        return Command::done();
    };
    let AutoAddPhase::AwaitingMeta { fresh } = &session.phase else {
        return Command::done();
    };
    let chain_id = session.chain_id;
    let fresh = fresh.clone();
    let attempt = model.attempt;

    let mut writes = Vec::new();
    for addr in &fresh {
        let meta = model.meta.get(&(chain_id, addr.clone())).cloned().flatten();
        let Some(meta) = meta else {
            continue; // unresolvable → never listed (⑧)
        };
        if meta.symbol.is_empty() {
            continue;
        }
        writes.push(shell_request(
            attempt,
            TrustOperation::WriteCustomToken {
                token: TrustCustomToken {
                    id: format!("{chain_id}_{addr}"),
                    chain_id,
                    contract_address: addr.clone(),
                    symbol: meta.symbol.clone(),
                    name: meta.symbol,
                    decimals: meta.decimals,
                },
            },
        ));
    }
    if writes.is_empty() {
        return finish_auto_add(model, None);
    }
    if let Some(session) = model.auto_add.as_mut() {
        session.phase = AutoAddPhase::AwaitingWrites {
            remaining: writes.len(),
            failed: false,
        };
    }
    writes.push(render());
    Command::all(writes)
}

fn on_token_written(model: &mut Model, ok: bool) -> Command<TrustEffect, Event> {
    let Some(session) = model.auto_add.as_mut() else {
        return Command::done();
    };
    let AutoAddPhase::AwaitingWrites { remaining, failed } = &mut session.phase else {
        return Command::done();
    };
    *failed |= !ok;
    *remaining = remaining.saturating_sub(1);
    if *remaining > 0 {
        return Command::done();
    }
    // TS invalidates only when the whole loop completed without a throw
    // (`token-autoadd.ts:79-81`); a failed save suppresses it (fail-closed —
    // the writes are batched here instead of sequential, ported with a note).
    let invalidate = (!*failed).then(|| TrustOperation::InvalidateTokenCache {
        address: session.address.clone(),
    });
    finish_auto_add(model, invalidate)
}

fn finish_auto_add(
    model: &mut Model,
    followup: Option<TrustOperation>,
) -> Command<TrustEffect, Event> {
    model.auto_add = None;
    let attempt = model.attempt;
    let mut commands = Vec::new();
    if let Some(op) = followup {
        commands.push(shell_request(attempt, op));
    }
    if let Some(next) = start_next_auto_add(model) {
        commands.push(next);
    }
    commands.push(render());
    Command::all(commands)
}

// -- sim judgment -----------------------------------------------------------

/// Judge every delta of a sim session against the trusted receive set
/// (`trustedReceiveSet`, `tx-simulation.ts:228-241`): the chain's registry
/// stables + wrapped native + tokens this account holds. The TS `hasReceive`
/// check merely gates the set's network fetch — here the set is model state,
/// so the gate has no decision to make. Custom tokens are deliberately NOT in
/// this set (verbatim: `trustedReceiveSet` never reads them) even though they
/// are in the transfer allowlist.
fn judge_session(model: &Model, session: &SimSession) -> Vec<TrustSimJudgment> {
    let registry = model.registry.get(&session.chain_id);
    let held = held_for(model, &session.address, session.chain_id);
    session
        .deltas
        .iter()
        .map(|delta| {
            let (meta, trusted) = match delta.token.as_deref() {
                None => (None, false),
                Some(token) => {
                    let lc = token.to_lowercase();
                    let meta = registry_meta(session.chain_id, &lc).or_else(|| {
                        model
                            .meta
                            .get(&(session.chain_id, lc.clone()))
                            .cloned()
                            .flatten()
                    });
                    let trusted = registry.is_some_and(|r| {
                        r.stables.contains(&lc) || r.wrapped.as_deref() == Some(lc.as_str())
                    }) || registry_token(session.chain_id, &lc).is_some()
                        || held.contains(&lc);
                    (meta, trusted)
                }
            };
            judge_delta(delta, meta.as_ref(), trusted)
        })
        .collect()
}

fn held_for(model: &Model, address: &str, chain_id: u32) -> BTreeSet<String> {
    model
        .held
        .get(address)
        .and_then(|chains| chains.get(&chain_id))
        .cloned()
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn dedupe_chains(chain_ids: &[u32]) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    chain_ids
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect()
}

fn get_logs_op(model: &Model, chain_id: u32, from: u64, to: u64) -> Command<TrustEffect, Event> {
    let address = model.address.clone().unwrap_or_default();
    let contracts = model
        .scan
        .get(&chain_id)
        .map(|entry| entry.allowlist.clone())
        .unwrap_or_default();
    shell_request(
        model.attempt,
        TrustOperation::RpcGetLogs {
            address: address.clone(),
            chain_id,
            from_block: format!("0x{from:x}"),
            to_block: format!("0x{to:x}"),
            recipient_topic: address_topic(&address),
            contracts,
        },
    )
}

fn safe_received_op(
    model: &Model,
    chain_id: u32,
    from: u64,
    to: u64,
) -> Command<TrustEffect, Event> {
    shell_request(
        model.attempt,
        TrustOperation::RpcGetSafeReceivedLogs {
            address: model.address.clone().unwrap_or_default(),
            chain_id,
            from_block: format!("0x{from:x}"),
            to_block: format!("0x{to:x}"),
            topic: SAFE_RECEIVED_TOPIC.to_owned(),
        },
    )
}

/// `topicToAddress` (`sim-assets.ts:107-110`): the lowercased last 20 bytes.
fn topic_to_address(topic: &str) -> String {
    let h = topic.strip_prefix("0x").unwrap_or(topic);
    let start = h.len().saturating_sub(40);
    format!("0x{}", h.get(start..).unwrap_or("").to_lowercase())
}

enum HexValue {
    Invalid,
    Overflow,
}

/// `BigInt(log.data !== '0x' ? data : '0x0')` — the whole data word as one
/// unsigned number. Invalid hex maps to the TS throw (the caller skips);
/// beyond-u128 maps to `Overflow` (fail-closed, no JS counterpart).
fn hex_value_u128(data: &str) -> Result<u128, HexValue> {
    if data.is_empty() || data == "0x" {
        return Ok(0);
    }
    let h = data.strip_prefix("0x").unwrap_or(data);
    if h.is_empty() || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(HexValue::Invalid);
    }
    let trimmed = h.trim_start_matches('0');
    if trimmed.len() > 32 {
        return Err(HexValue::Overflow);
    }
    if trimmed.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(trimmed, 16).map_err(|_| HexValue::Invalid)
}

/// `firstWord` (`sim-assets.ts:113-122`): the first 32-byte word of a log's
/// data; absent/garbage reads as 0 (the TS catch), beyond-u128 is `Overflow`.
fn first_word_value(data: &str) -> Result<u128, HexValue> {
    if data.is_empty() {
        return Ok(0);
    }
    let h = data.strip_prefix("0x").unwrap_or(data);
    if h.is_empty() {
        return Ok(0);
    }
    let word: String = h.chars().take(64).collect();
    if !word.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(0); // BigInt throws → catch → 0n, verbatim
    }
    let trimmed = word.trim_start_matches('0');
    if trimmed.len() > 32 {
        return Err(HexValue::Overflow);
    }
    if trimmed.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(trimmed, 16).map_err(|_| HexValue::Invalid)
}

fn parse_hex_u64(hex: &str) -> Option<u64> {
    let h = hex
        .strip_prefix("0x")
        .or_else(|| hex.strip_prefix("0X"))
        .unwrap_or(hex);
    if h.is_empty() {
        return None;
    }
    u64::from_str_radix(h, 16).ok()
}

/// `Some(true)` iff the decimal string is a strictly positive integer;
/// `None` when it is not a canonical signed decimal at all.
fn parse_delta_positive(s: &str) -> Option<bool> {
    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(!negative && digits.bytes().any(|b| b != b'0'))
}

/// An f64 that is exactly a u64 (block numbers over the wire ride as JS
/// numbers); anything else is dropped rather than rounded.
fn exact_u64(value: f64) -> Option<u64> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > u64::MAX as f64 {
        return None;
    }
    Some(value as u64)
}

/// Issue one operation whose answer carries the current attempt.
fn shell_request(attempt: u64, operation: TrustOperation) -> Command<TrustEffect, Event> {
    Command::request_from_shell(operation)
        .then_send(move |result| Event::ShellCompleted { attempt, result })
}

impl super::SplitEffect for TrustEffect {
    type Op = TrustOperation;
    fn into_shell(self) -> Option<crux_core::Request<TrustOperation>> {
        match self {
            TrustEffect::Render(_) => None,
            TrustEffect::Shell(request) => Some(request),
        }
    }
}
