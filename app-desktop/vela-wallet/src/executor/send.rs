//! The only place the `send` machine touches the outside world.
//!
//! Nineteen operations, each one existing call — the vocabulary the core
//! declares. No branching on business meaning: the step machine, the 15 s
//! pre-check race, the re-entry lock, the cancel checkpoints, the
//! displayed-is-signed gate and the persist-then-track ordering all live in
//! Rust. What DOES live here, because the core's own notes put it here:
//!
//! - **The passkey ceremony.** `SubmitUserOp` is one sentence to the core;
//!   here it is the closure the submit spine calls with the SafeOp hash, and
//!   the flag that tells the host to dispatch `SigningStarted` the moment the
//!   prompt opens.
//! - **The wording layer**, already done in `executor::user_op` — the
//!   failure vocabulary arrives typed.
//! - **The amount codec** (`to_multi_send_call`): the core states base units
//!   as decimal strings, the assembly reads hex.
//!
//! Four operations are the SCREEN's, not this file's, and answer
//! [`SendAnswer::Screen`]: `EstimateFee` is asked of the live `fee_policy`
//! session the confirm card renders (one number, one owner — the web tier
//! failed four times by splitting it), `TrackSubmitted` hands off to the
//! app-resident tracker, `ShowAlert` needs the panel, `Close` needs the
//! column.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/flows/core/send-executor.ts`
//! and `services/recipient-risk.ts` @ `origin/main`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use vela_core::app::balance_dashboard::BalanceToken;
use vela_core::app::fee_policy::FeeCall;
use vela_core::app::network_admin::BUILTIN_CHAINS;
use vela_core::app::send::{
    SendAddNetworkOutcome, SendChainInfo, SendOperation, SendRecipientIdentity, SendRecipientRisk,
    SendShellResult, SendToken, SendTokenMeta, SendTxRecord,
};
use vela_core::app::{Account, KeyMethod};
use vela_core::user_op::WalletKey;

use crate::executor::passkey::{self, Ceremony};
use crate::executor::user_op::{self, QuotedFee};
use crate::executor::{abi, balances, identity, pool, relay, storage};

/// `vela.transactionHistory` — the shared local store.
const TX_KEY: &str = "vela.transactionHistory";

/// How an operation is performed — the resident's shape, plus the arm that
/// belongs to the screen.
pub enum SendAnswer {
    Now(SendShellResult),
    Blocking(Box<dyn FnOnce() -> SendShellResult + Send>),
    After(Duration, SendShellResult),
    /// Not this module's business; the host performs it and answers.
    Screen,
}

/// What the submit needs from the account that is sending, fixed when the
/// flow opens so an account switch mid-flight cannot change who signs.
#[derive(Clone)]
pub struct SendContext {
    /// The founding keys, in order; the assertion's own credential picks the
    /// verifier among them.
    pub keys: Vec<WalletKey>,
    /// Which ceremony to run — derived from the first key's transports.
    pub key_method: KeyMethod,
    /// The credential the ceremony is pinned to, or `None` for the
    /// discoverable "any key on this device" ask.
    pub pinned_credential: Option<String>,
    pub ceremony: Ceremony,
    /// Raised by the sign closure the instant the prompt opens; the host
    /// polls it and dispatches `SigningStarted` once.
    pub signing_started: Arc<AtomicBool>,
}

impl SendContext {
    /// From the stored account. The method routes on what the first key
    /// recorded when it was minted: a phone signs over caBLE, a platform key
    /// through the vault, everything else is a security key on the desk —
    /// asked discoverably, so a multi-key wallet's OTHER keys on the same
    /// device can answer too.
    pub fn new(account: &Account, ceremony: Ceremony) -> Self {
        let keys = user_op::key_set_of(account);
        let transports = account
            .keys
            .first()
            .map(|key| key.transports.as_str())
            .unwrap_or_default();
        let key_method = if transports.contains("hybrid") {
            KeyMethod::Hybrid
        } else if transports.contains("internal") {
            KeyMethod::Platform
        } else {
            KeyMethod::SecurityKey
        };
        let pinned_credential = match key_method {
            KeyMethod::SecurityKey => None,
            _ => keys.first().map(|key| key.credential_id.clone()),
        };
        Self {
            keys,
            key_method,
            pinned_credential,
            ceremony,
            signing_started: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ---------------------------------------------------------------------------
// Tokens and chains
// ---------------------------------------------------------------------------

/// The API network id the core keys `tokenId()` on: the built-in id, or a
/// stable synthetic one for a custom chain. Tokens and chains use the same
/// function, which is all the core needs of it.
pub fn network_id(chain_id: u32) -> String {
    BUILTIN_CHAINS
        .iter()
        .find(|chain| chain.chain_id == chain_id)
        .map_or_else(|| format!("chain-{chain_id}"), |chain| chain.id.to_owned())
}

/// The chains the core validates a locked request against and synthesises
/// placeholder tokens for: the built-ins plus whatever the person added.
pub fn chain_infos() -> Vec<SendChainInfo> {
    let mut out: Vec<SendChainInfo> = BUILTIN_CHAINS
        .iter()
        .map(|chain| SendChainInfo {
            chain_id: chain.chain_id,
            network: chain.id.to_owned(),
            native_symbol: chain.native_symbol.to_owned(),
        })
        .collect();
    if let Ok(Some(Value::Array(items))) = storage::read_value(storage::KEY_CUSTOM_NETWORKS) {
        for item in items {
            let Some(chain_id) = item
                .get("chainId")
                .and_then(Value::as_u64)
                .and_then(|id| u32::try_from(id).ok())
            else {
                continue;
            };
            if out.iter().any(|chain| chain.chain_id == chain_id) {
                continue;
            }
            out.push(SendChainInfo {
                chain_id,
                network: network_id(chain_id),
                native_symbol: item
                    .get("nativeSymbol")
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_owned(),
            });
        }
    }
    out
}

/// A balance row as the send machine's token (`toSendToken`).
pub fn to_send_token(token: &BalanceToken) -> SendToken {
    SendToken {
        network: network_id(token.chain_id),
        chain_id: token.chain_id,
        symbol: token.symbol.clone(),
        balance: token.balance.clone(),
        decimals: token.decimals,
        token_address: token.token_address.clone(),
        price_usd: token.price_usd,
        logo_urls: Vec::new(),
        spam: token.spam,
    }
}

// ---------------------------------------------------------------------------
// Records
// ---------------------------------------------------------------------------

/// One core record as the stored row every client reads (`toLocalTransaction`).
fn to_row(record: &SendTxRecord) -> Value {
    let mut row = json!({
        "id": record.id,
        "userOpHash": record.user_op_hash,
        "txHash": record.tx_hash,
        "from": record.from,
        "to": record.to,
        "value": record.value,
        "symbol": record.symbol,
        "decimals": record.decimals,
        "logoUrls": record.logo_urls,
        "chainId": record.chain_id,
        "timestamp": record.timestamp_s,
        "status": "pending",
        "type": "send",
    });
    if let Some(name) = &record.to_name {
        row["toName"] = json!(name);
    }
    if let Some(usd) = &record.usd {
        row["usd"] = json!(usd);
    }
    row
}

/// ONE atomic write for every sibling record (invariant ⑥). A record whose
/// id the store already holds is left as it is — a resubmit shares its hash.
pub fn persist_records(records: &[SendTxRecord]) -> bool {
    let mut rows = match storage::read_value(TX_KEY) {
        Ok(Some(Value::Array(rows))) => rows,
        _ => Vec::new(),
    };
    let known: std::collections::BTreeSet<String> = rows
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_owned))
        .collect();
    for record in records {
        if !known.contains(&record.id) {
            rows.push(to_row(record));
        }
    }
    storage::write_value(TX_KEY, Value::Array(rows)).is_ok()
}

// ---------------------------------------------------------------------------
// Recipient risk (`recipient-risk.ts`)
// ---------------------------------------------------------------------------

/// `eth_getCode` tells a wallet from a contract. An EIP-7702 delegated EOA
/// (`0xef0100 ‖ impl`, 23 bytes) is a WALLET with smart-account features —
/// a person's account, never badged "contract". `None` = unreachable.
fn is_contract(chain_id: u32, address: &str) -> Option<bool> {
    let body = pool::call(chain_id, "eth_getCode", json!([address, "latest"])).ok()?;
    if body.get("error").is_some() {
        return None;
    }
    let code = body.get("result").and_then(Value::as_str)?;
    let lower = code.to_lowercase();
    if lower.len() == 2 + 46 && lower.starts_with("0xef0100") {
        return Some(false);
    }
    Some(code != "0x" && code.len() > 2)
}

/// Have we ever SENT to this address — an outgoing transfer or a dApp
/// transaction, with a legacy untyped row counting as a send?
fn has_prior_interaction(address: &str) -> bool {
    let Ok(Some(Value::Array(rows))) = storage::read_value(TX_KEY) else {
        return false;
    };
    rows.iter().any(|row| {
        let to = row.get("to").and_then(Value::as_str).unwrap_or_default();
        let kind = row.get("type").and_then(Value::as_str).unwrap_or("send");
        to.eq_ignore_ascii_case(address) && (kind == "send" || kind == "dapp_tx")
    })
}

pub fn recipient_risk(chain_id: u32, address: &str) -> SendRecipientRisk {
    SendRecipientRisk {
        is_contract: is_contract(chain_id, address),
        first_time: Some(!has_prior_interaction(address)),
    }
}

// ---------------------------------------------------------------------------
// Perform
// ---------------------------------------------------------------------------

/// The stored public key of the account a credential belongs to.
fn stored_public_key(account_id: &str) -> Option<String> {
    storage::load_accounts()
        .ok()?
        .into_iter()
        .find_map(|account| {
            let matches = account.id == account_id
                || account
                    .keys
                    .iter()
                    .any(|key| key.credential_id == account_id);
            (matches && !account.public_key_hex.is_empty()).then_some(account.public_key_hex)
        })
}

/// `symbol()` and `decimals()` of one ERC-20, for a locked request's
/// unknown token.
fn token_metadata(chain_id: u32, address: &str) -> Option<SendTokenMeta> {
    let call = |data: String| {
        pool::call(
            chain_id,
            "eth_call",
            json!([{ "to": address, "data": data }, "latest"]),
        )
        .ok()
        .and_then(|body| {
            body.get("result")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
    };
    let symbol = call(abi::enc_symbol()).and_then(|hex| abi::dec_string(&hex))?;
    let decimals = call(abi::enc_decimals()).and_then(|hex| abi::dec_u8(&hex))?;
    Some(SendTokenMeta {
        symbol,
        decimals: u32::from(decimals),
    })
}

/// Perform one operation. Never fails outward: every arm answers with the
/// variant it owes, on both paths.
pub fn perform(operation: &SendOperation, ctx: &SendContext) -> SendAnswer {
    let now_ms = crate::executor::now_ms;
    match operation {
        SendOperation::FetchTokens { address } => {
            let address = address.clone();
            SendAnswer::Blocking(Box::new(move || {
                let (tokens, failed) = balances::fetch_all(&address);
                // Nothing came back and something failed: the load failed
                // (`catch` → `send.alertLoadTokensError`). An empty wallet on
                // reachable chains is an empty list, not an error.
                let tokens = if tokens.is_empty() && !failed.is_empty() {
                    None
                } else {
                    Some(tokens.iter().map(to_send_token).collect())
                };
                // Read AFTER the fetch, so a chain just added is in the snapshot.
                SendShellResult::TokensLoaded {
                    tokens,
                    chains: chain_infos(),
                }
            }))
        }

        // The balance cache belongs to `balance_dashboard`, which re-reads on
        // its own pull; the send flow's own refresh re-fetches regardless.
        SendOperation::ClearTokenCache { .. } => {
            SendAnswer::Now(SendShellResult::TokenCacheCleared)
        }

        SendOperation::ResolveTokenMetadata { chain_id, address } => {
            let (chain_id, address) = (*chain_id, address.clone());
            SendAnswer::Blocking(Box::new(move || SendShellResult::TokenMetadata {
                meta: token_metadata(chain_id, &address),
            }))
        }

        // Adding a network from a locked request is the settings wizard's
        // journey (search index → chain document → probe → save); the send
        // flow has no such wizard on the desktop yet, so the answer is the
        // ported `catch` — the person is told it did not work, and can add
        // the network from Settings. Recorded, not hidden.
        SendOperation::AddNetwork { .. } => SendAnswer::Now(SendShellResult::NetworkAdded {
            outcome: SendAddNetworkOutcome::Error,
        }),

        SendOperation::EstimateFee { .. } => SendAnswer::Screen,

        SendOperation::ProbeTreasury { chain_id } => {
            let chain_id = *chain_id;
            SendAnswer::Blocking(Box::new(move || SendShellResult::TreasuryProbed {
                probe: relay::probe_treasury(chain_id),
            }))
        }

        SendOperation::LoadAccountCredential { account_id } => {
            SendAnswer::Now(SendShellResult::AccountCredential {
                public_key_hex: stored_public_key(account_id),
            })
        }

        SendOperation::SubmitUserOp {
            chain_id,
            account,
            public_key_hex,
            calls,
            gas_fee_token,
            quoted_fee,
            // In-band operations sign zero native-fee fields; the override
            // the core carries for the legacy path has no reader here.
            max_fee_per_gas: _,
        } => {
            let chain_id = *chain_id;
            let account = account.clone();
            let calls: Vec<FeeCall> = calls.clone();
            let gas_fee_token = gas_fee_token.clone();
            let quoted = quoted_fee.as_ref().and_then(|fee| {
                Some(QuotedFee {
                    amount: fee.amount.parse().ok()?,
                    recipient: fee.recipient.clone(),
                })
            });
            // The keys the flow opened with; a legacy record with none
            // projects the public key the core just read.
            let keys = if ctx.keys.is_empty() {
                vec![WalletKey {
                    credential_id: ctx.pinned_credential.clone().unwrap_or_default(),
                    public_key_hex: public_key_hex.clone(),
                }]
            } else {
                ctx.keys.clone()
            };
            let ctx = ctx.clone();
            SendAnswer::Blocking(Box::new(move || {
                let mut sign = |challenge: &[u8]| {
                    // The prompt is opening: the core moves to `signing` here,
                    // exactly where `setTxStatus('signing')` sat.
                    ctx.signing_started.store(true, Ordering::SeqCst);
                    passkey::assert(
                        challenge,
                        ctx.pinned_credential.as_deref(),
                        ctx.key_method,
                        &ctx.ceremony,
                    )
                };
                match user_op::submit(
                    chain_id,
                    &account,
                    &calls,
                    gas_fee_token.as_deref(),
                    &keys,
                    &mut sign,
                    quoted,
                ) {
                    Ok(user_op_hash) => SendShellResult::Submitted {
                        user_op_hash,
                        now_ms: now_ms(),
                    },
                    Err(failure) => SendShellResult::SubmitFailed {
                        failure: failure.into(),
                    },
                }
            }))
        }

        // The host closes the ceremony channel (which is what cancels a
        // waiting ceremony) before this arm is reached; the answer is owed
        // either way.
        SendOperation::CancelPasskeySign => {
            SendAnswer::Now(SendShellResult::PasskeyCancelAcknowledged)
        }

        SendOperation::PersistTxRecords { records } => {
            // Best effort, like the TS `.catch(() => {})` — and the core must
            // still be told, or `TrackSubmitted` never fires.
            if !persist_records(records) {
                eprintln!("[vela-wallet] send: the transaction store refused the pending records");
            }
            SendAnswer::Now(SendShellResult::RecordsPersisted)
        }

        SendOperation::TrackSubmitted { .. } => SendAnswer::Screen,

        SendOperation::ResolveIdentity { address } => {
            let address = address.clone();
            SendAnswer::Blocking(Box::new(move || SendShellResult::IdentityResolved {
                identity: identity::resolve(&address).map(|found| SendRecipientIdentity {
                    name: Some(found.name),
                    source: Some(found.source),
                }),
            }))
        }

        SendOperation::ResolveRisk { chain_id, address } => {
            let (chain_id, address) = (*chain_id, address.clone());
            SendAnswer::Blocking(Box::new(move || SendShellResult::RiskResolved {
                risk: Some(recipient_risk(chain_id, &address)),
            }))
        }

        // No simulation engine on the desktop yet: the confirm surface stays
        // empty, which is the honest reading of "best effort".
        SendOperation::SimulateCalls { .. } => {
            SendAnswer::Now(SendShellResult::SimResolved { sim_json: None })
        }

        SendOperation::StartTimer { ms, tag } => SendAnswer::After(
            Duration::from_millis(u64::from(*ms)),
            SendShellResult::TimerElapsed { tag: *tag },
        ),

        // The desktop has no haptics; the visual feedback is the panel's.
        SendOperation::Haptic { .. } => SendAnswer::Now(SendShellResult::HapticPlayed),

        SendOperation::ShowAlert { .. } | SendOperation::Close => SendAnswer::Screen,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::AccountKey;

    fn account(transports: &str, keys: usize) -> Account {
        Account {
            id: "cred0".to_owned(),
            name: "Wallet".to_owned(),
            address: "0x0000000000000000000000000000000000000001".to_owned(),
            public_key_hex: "04aa".to_owned(),
            created_at_iso: String::new(),
            keys: (0..keys)
                .map(|i| AccountKey {
                    credential_id: format!("cred{i}"),
                    public_key_hex: format!("04{i:02x}"),
                    name: String::new(),
                    transports: transports.to_owned(),
                })
                .collect(),
        }
    }

    fn ceremony() -> Ceremony {
        crate::ceremony::CeremonyChannel::new().ceremony(0)
    }

    /// The method the first key was minted with decides the ceremony, and a
    /// security key is asked discoverably so any founding key on it can answer.
    #[test]
    fn the_context_routes_on_the_first_key_s_transports() {
        let usb = SendContext::new(&account("usb", 2), ceremony());
        assert_eq!(usb.key_method, KeyMethod::SecurityKey);
        assert_eq!(usb.pinned_credential, None);
        assert_eq!(usb.keys.len(), 2);

        let phone = SendContext::new(&account("hybrid", 1), ceremony());
        assert_eq!(phone.key_method, KeyMethod::Hybrid);
        assert_eq!(phone.pinned_credential.as_deref(), Some("cred0"));

        let vault = SendContext::new(&account("internal", 3), ceremony());
        assert_eq!(vault.key_method, KeyMethod::Platform);
        assert_eq!(vault.pinned_credential.as_deref(), Some("cred0"));

        // A legacy record: one projected key, a security key.
        let legacy = SendContext::new(&account("", 0), ceremony());
        assert_eq!(legacy.keys.len(), 1);
        assert_eq!(legacy.key_method, KeyMethod::SecurityKey);
    }

    #[test]
    fn a_balance_row_becomes_a_send_token_on_its_network() {
        let token = BalanceToken {
            chain_id: 100,
            symbol: "xDAI".to_owned(),
            name: "xDAI".to_owned(),
            balance: "0.75897".to_owned(),
            decimals: 18,
            token_address: None,
            price_usd: Some(1.0),
            spam: false,
        };
        let wire = to_send_token(&token);
        assert_eq!(wire.network, "gnosis");
        assert_eq!(wire.id(), "gnosis_native_xDAI");
        assert_eq!(network_id(424242), "chain-424242");
        let chains = chain_infos();
        assert!(
            chains
                .iter()
                .any(|c| c.chain_id == 100 && c.native_symbol == "xDAI")
        );
    }

    /// Records land as the rows every client reads, once each, in one write.
    #[test]
    fn records_are_persisted_once_in_the_shared_shape() {
        crate::executor::storage::tests::with_temp_state("send-records", || {
            let record = |id: &str| SendTxRecord {
                id: id.to_owned(),
                user_op_hash: "0xop".to_owned(),
                tx_hash: String::new(),
                from: "0xme".to_owned(),
                to: "0xyou".to_owned(),
                to_name: Some("Alice".to_owned()),
                value: "0.001".to_owned(),
                symbol: "xDAI".to_owned(),
                decimals: 18,
                logo_urls: Vec::new(),
                chain_id: 100,
                timestamp_s: 1_700_000_000.0,
                usd: Some("$0.00".to_owned()),
            };
            assert!(persist_records(&[record("0xop-0"), record("0xop-1")]));
            assert!(persist_records(&[record("0xop-1")]), "a resubmit merges");
            let rows = match storage::read_value(TX_KEY) {
                Ok(Some(Value::Array(rows))) => rows,
                _ => Vec::new(),
            };
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0]["status"], "pending");
            assert_eq!(rows[0]["type"], "send");
            assert_eq!(rows[0]["toName"], "Alice");
            assert_eq!(rows[0]["timestamp"], 1_700_000_000.0);
            // The address book's first-time tell reads the same rows.
            assert!(has_prior_interaction("0xYOU"));
            assert!(!has_prior_interaction("0xnobody"));
        });
    }

    #[test]
    fn a_credential_finds_its_account_s_public_key() {
        crate::executor::storage::tests::with_temp_state("send-credential", || {
            let _ = storage::save_account(&account("usb", 2));
            assert_eq!(stored_public_key("cred0").as_deref(), Some("04aa"));
            assert_eq!(stored_public_key("cred1").as_deref(), Some("04aa"));
            assert_eq!(stored_public_key("nobody"), None);
        });
    }

    /// Which arms are the screen's, which block, which answer now.
    #[test]
    fn the_operation_split_matches_the_web_ports() {
        let ctx = SendContext::new(&account("usb", 1), ceremony());
        let screen = [
            SendOperation::EstimateFee {
                chain_id: 100,
                account: String::new(),
                tx: None,
                batch: None,
                gas_fee_token: None,
                public_key_hex: None,
            },
            SendOperation::TrackSubmitted {
                user_op_hash: String::new(),
                record_ids: Vec::new(),
                chain_id: 100,
            },
            SendOperation::ShowAlert {
                kind: vela_core::app::send::SendAlertKind::InvalidAddress,
            },
            SendOperation::Close,
        ];
        for op in &screen {
            assert!(matches!(perform(op, &ctx), SendAnswer::Screen), "{op:?}");
        }
        assert!(matches!(
            perform(&SendOperation::CancelPasskeySign, &ctx),
            SendAnswer::Now(SendShellResult::PasskeyCancelAcknowledged)
        ));
        assert!(matches!(
            perform(
                &SendOperation::SimulateCalls {
                    chain_id: 1,
                    account: String::new(),
                    calls: Vec::new()
                },
                &ctx
            ),
            SendAnswer::Now(SendShellResult::SimResolved { sim_json: None })
        ));
        match perform(
            &SendOperation::StartTimer {
                ms: 15_000,
                tag: vela_core::app::send::SendTimerTag::EstimateTimeout,
            },
            &ctx,
        ) {
            SendAnswer::After(delay, SendShellResult::TimerElapsed { .. }) => {
                assert_eq!(delay, Duration::from_secs(15));
            }
            _ => unreachable!("the timer is a timer"),
        }
        assert!(matches!(
            perform(
                &SendOperation::FetchTokens {
                    address: "0x0".to_owned()
                },
                &ctx
            ),
            SendAnswer::Blocking(_)
        ));
    }
}
