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
    SendChainInfo, SendOperation, SendRecipientIdentity, SendRecipientRisk, SendShellResult,
    SendToken, SendTokenMeta, SendTxRecord,
};
use vela_core::app::{Account, KeyMethod};
use vela_core::user_op::WalletKey;
use vela_core::wallet_keys::{DeviceKey, SignRoute};

use crate::executor::passkey::{self, Ceremony};
use crate::executor::trusted_signer::{self, Ask};
use crate::executor::user_op::{self, QuotedFee, Signer};
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
    /// Which ceremony to run: the route the account signed in over.
    pub key_method: KeyMethod,
    /// The credential the ceremony is pinned to, or `None` for the
    /// discoverable "any key on this device" ask (a record from before the
    /// sign-in key whose first key is a security key).
    pub pinned_credential: Option<String>,
    pub ceremony: Ceremony,
    /// Raised by the sign closure the instant the prompt opens; the host
    /// polls it and dispatches `SigningStarted` once.
    pub signing_started: Arc<AtomicBool>,
    /// Spec 075: the Trusted Signer page this account signs on — empty for
    /// the page Settings names — or `None` when a passkey signs here.
    pub signer_page: Option<String>,
    /// The one key that page may sign with: the sign-in key. `None` for a
    /// record from before it, whose every founding key may answer there.
    pub page_key: Option<String>,
    /// The account's name: the Trusted Signer's page points the person at a
    /// passkey with it.
    pub account_name: Option<String>,
    /// The Trusted Signer (spec 071): whether this send goes to it, and its
    /// waiting sheet. The host swaps in the channel it listens to and then
    /// hands it the page ([`Self::follow_sign_in`]); the one made here routes
    /// nothing.
    pub trusted_signer: Arc<trusted_signer::Channel>,
}

/// Where a signature goes — the core's route, in this shell's vocabulary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Route {
    /// A ceremony this machine runs, pinned to one credential.
    Passkey(String, KeyMethod),
    /// Spec 075: the Trusted Signer, at this page — empty for the page
    /// Settings names.
    TrustedSigner(String),
}

/// The core's answer as this shell routes it; `None` for a method this build
/// does not know.
fn route_of(route: SignRoute) -> Option<Route> {
    Some(match route.method.as_str() {
        "platform" => Route::Passkey(route.credential_id, KeyMethod::Platform),
        "hybrid" => Route::Passkey(route.credential_id, KeyMethod::Hybrid),
        "security_key" => Route::Passkey(route.credential_id, KeyMethod::SecurityKey),
        vela_core::trusted_signer::METHOD => Route::TrustedSigner(route.signer_origin),
        _ => return None,
    })
}

/// The page the Trusted Signer channel is pointed at: the account's own, or
/// the page Settings names when its route names none.
pub fn page_to_follow(signer_page: Option<&str>, settings_page: &str) -> Option<String> {
    signer_page.map(|page| {
        if page.is_empty() {
            settings_page.to_owned()
        } else {
            page.to_owned()
        }
    })
}

impl SendContext {
    /// From the stored account (founder, 2026-09-26): the key it was created
    /// or signed in with, over the route that reached it — chosen there and
    /// never asked again. A record from before that was kept signs as it
    /// always did: the route its first key recorded when it was minted, or
    /// the page that key lives behind.
    pub fn new(account: &Account, ceremony: Ceremony) -> Self {
        let keys = user_op::key_set_of(account);
        let (legacy_method, legacy_pinned) = first_key_route(account, &keys);
        let sign_in = account.sign_in_route();
        let page_key = sign_in
            .as_ref()
            .filter(|route| route.method == vela_core::trusted_signer::METHOD)
            .map(|route| route.credential_id.clone());
        let route = sign_in
            .or_else(|| vela_core::wallet_keys::sign_route(&device_keys_of(account), "auto"))
            .and_then(route_of);
        let (key_method, pinned_credential, signer_page) = match route {
            Some(Route::Passkey(credential, method)) => (method, Some(credential), None),
            Some(Route::TrustedSigner(page)) => (legacy_method, legacy_pinned, Some(page)),
            None => (legacy_method, legacy_pinned, None),
        };
        Self {
            keys,
            key_method,
            pinned_credential,
            ceremony,
            signing_started: Arc::new(AtomicBool::new(false)),
            signer_page,
            page_key,
            account_name: (!account.name.is_empty()).then(|| account.name.clone()),
            trusted_signer: trusted_signer::Channel::new().0,
        }
    }

    /// Point the Trusted Signer channel at this account's page when it signs
    /// on one. The host calls it once it has swapped in the channel it listens
    /// to; `settings_page` stands in for a route that names no page.
    pub fn follow_sign_in(&self, settings_page: &str) {
        self.trusted_signer
            .choose(page_to_follow(self.signer_page.as_deref(), settings_page));
    }
}

/// How a record from before the sign-in key signs: on what its first key
/// recorded when it was minted — a phone signs over caBLE, a platform key
/// through the vault, everything else is a security key on the desk, asked
/// discoverably so a multi-key wallet's OTHER keys on the same device can
/// answer too.
fn first_key_route(account: &Account, keys: &[WalletKey]) -> (KeyMethod, Option<String>) {
    let transports = account
        .keys
        .first()
        .map(|key| key.transports.as_str())
        .unwrap_or_default();
    let method = if transports.contains("hybrid") {
        KeyMethod::Hybrid
    } else if transports.contains("internal") {
        KeyMethod::Platform
    } else {
        KeyMethod::SecurityKey
    };
    let pinned = match method {
        KeyMethod::SecurityKey => None,
        _ => keys.first().map(|key| key.credential_id.clone()),
    };
    (method, pinned)
}

/// The founding keys as the core's `sign_route` reads them. Spec 075: each
/// with the page it lives behind, untouched — dropping it would make the
/// route forget where the key is.
fn device_keys_of(account: &Account) -> Vec<DeviceKey> {
    account
        .keys
        .iter()
        .map(|key| DeviceKey {
            credential_id: key.credential_id.clone(),
            public_key_hex: key.public_key_hex.clone(),
            name: key.name.clone(),
            transports: key.transports.clone(),
            signer_origin: key.signer_origin.clone(),
        })
        .collect()
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
/// One multi-chain round as the picker's answer.
///
/// Nothing came back and something failed: the load failed (`catch` →
/// `send.alertLoadTokensError`). An empty wallet on reachable chains is an
/// empty list, not an error. The chains are read at the answer, not the
/// fetch, so a chain just added is in the snapshot.
///
/// Shared by the executor's own fetch and the host's answer from the
/// dashboard's settled round (`SendHost::answer_tokens`): one rule for both.
/// Every read a first quote on these chains makes before its simulation, all
/// at once, each through the cache the fee session reads (and each
/// single-flight, so a pick landing mid-read waits for it instead of asking
/// again). Errors are nobody's business here: the quote reads for itself.
fn prewarm_fees(account: &str, chain_ids: &[u32]) {
    use crate::executor::{chain, fee_signals};
    use vela_core::app::fee_policy::{FeeTier, is_tempo_chain};
    std::thread::scope(|scope| {
        for &chain_id in chain_ids {
            let tempo = is_tempo_chain(chain_id);
            scope.spawn(move || {
                let _ = chain::is_deployed(account, chain_id);
            });
            scope.spawn(move || {
                let _ = fee_signals::gas_signals(chain_id, !tempo);
            });
            scope.spawn(move || {
                if tempo {
                    let _ = relay::account_info(chain_id, account);
                } else {
                    // One call answers every tier.
                    let _ = fee_signals::bundler_quote(chain_id, FeeTier::Fast);
                }
            });
            scope.spawn(move || {
                let _ = relay::in_band_quotes(chain_id, account);
            });
        }
    });
}

pub fn tokens_loaded(tokens: &[BalanceToken], failed_chain_ids: &[u32]) -> SendShellResult {
    let tokens = if tokens.is_empty() && !failed_chain_ids.is_empty() {
        None
    } else {
        Some(tokens.iter().map(to_send_token).collect())
    };
    SendShellResult::TokensLoaded {
        tokens,
        chains: chain_infos(),
    }
}

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
        first_time: Some(first_time(address)),
    }
}

/// `true` = never sent to this address from this device. A non-address is
/// never "first" (the web's `resolveRecipientRisk`).
fn first_time(address: &str) -> bool {
    let hex = address.strip_prefix("0x").unwrap_or_default();
    hex.len() == 40 && hex.bytes().all(|b| b.is_ascii_hexdigit()) && !has_prior_interaction(address)
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
                tokens_loaded(&tokens, &failed)
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
        // journey (registry → chain document → probe → save), run by the
        // resident network admin — which only the screen can reach (078 W-04,
        // `SendHost::perform_send`).
        SendOperation::AddNetwork { .. } => SendAnswer::Screen,

        SendOperation::EstimateFee { .. } => SendAnswer::Screen,

        SendOperation::ProbeTreasury { chain_id } => {
            let chain_id = *chain_id;
            SendAnswer::Blocking(Box::new(move || SendShellResult::TreasuryProbed {
                probe: relay::probe_treasury(chain_id),
            }))
        }

        // Read ahead while the person chooses: the same cached readers the fee
        // session calls, so the quote a pick starts finds them answered. The
        // core does not wait for any of it.
        SendOperation::PrewarmFees { account, chain_ids } => {
            let (account, chain_ids) = (account.clone(), chain_ids.clone());
            let started = std::thread::Builder::new()
                .name("vela-fee-prewarm".to_owned())
                .spawn(move || prewarm_fees(&account, &chain_ids));
            drop(started);
            SendAnswer::Now(SendShellResult::FeesPrewarmed)
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
                    // The speed this fee was priced at (spec 069).
                    tier: fee.tier,
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
                // The person's own send: no site asked, so the page is told
                // the operation's calls (contract §1).
                let ask = Ask::own(ctx.account_name.clone());
                let page = ctx.trusted_signer.chosen();
                let signer = match &page {
                    Some(page) => Signer::TrustedSigner {
                        ask: &ask,
                        page,
                        channel: &ctx.trusted_signer,
                        only: ctx.page_key.as_deref(),
                    },
                    None => Signer::Passkey(&mut sign),
                };
                match user_op::submit(
                    chain_id,
                    &account,
                    &calls,
                    gas_fee_token.as_deref(),
                    &keys,
                    signer,
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
    use vela_core::app::{AccountKey, SignInKey};

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
                    signer_origin: None,
                })
                .collect(),
            signed_in_with: None,
        }
    }

    fn signed_in(mut account: Account, credential: &str, method: KeyMethod) -> Account {
        account.signed_in_with = Some(SignInKey {
            credential_id: credential.to_owned(),
            method,
            transports: String::new(),
            signer_origin: None,
        });
        account
    }

    fn ceremony() -> Ceremony {
        crate::ceremony::CeremonyChannel::new().ceremony(0)
    }

    const SETTINGS_PAGE: &str = "https://sign.getvela.app/";

    /// Founder, 2026-09-26: the key the account signed in with signs, over the
    /// route it took — not the first key, not where a key was registered, and
    /// never a choice made at signing time.
    #[test]
    fn the_account_signs_with_its_sign_in_key() {
        let ctx = SendContext::new(
            &signed_in(account("internal", 3), "cred2", KeyMethod::SecurityKey),
            ceremony(),
        );
        assert_eq!(ctx.key_method, KeyMethod::SecurityKey);
        assert_eq!(
            ctx.pinned_credential.as_deref(),
            Some("cred2"),
            "pinned, even on a security key: only this key signs"
        );
        assert_eq!(ctx.keys.len(), 3, "every key still verifies");
        ctx.follow_sign_in(SETTINGS_PAGE);
        assert_eq!(ctx.trusted_signer.chosen(), None);

        // A synced passkey registered as `internal`, reached here by a scan.
        let ctx = SendContext::new(
            &signed_in(account("internal", 1), "cred0", KeyMethod::Hybrid),
            ceremony(),
        );
        assert_eq!(ctx.key_method, KeyMethod::Hybrid);
        assert_eq!(ctx.pinned_credential.as_deref(), Some("cred0"));
    }

    /// Signed in through the Trusted Signer: every signature goes to that
    /// page, and to the page Settings names when the sign-in named none.
    #[test]
    fn a_trusted_signer_sign_in_signs_on_its_page() {
        let mut through_page = signed_in(account("internal", 1), "cred0", KeyMethod::TrustedSigner);
        let ctx = SendContext::new(&through_page, ceremony());
        ctx.follow_sign_in("http://localhost:8140/");
        assert_eq!(
            ctx.trusted_signer.chosen().as_deref(),
            Some("http://localhost:8140/")
        );

        if let Some(key) = through_page.signed_in_with.as_mut() {
            key.signer_origin = Some("https://sign.example.test".to_owned());
        }
        let ctx = SendContext::new(&through_page, ceremony());
        ctx.follow_sign_in(SETTINGS_PAGE);
        assert_eq!(
            ctx.trusted_signer.chosen().as_deref(),
            Some("https://sign.example.test")
        );
        assert_eq!(ctx.account_name.as_deref(), Some("Wallet"));
    }

    /// A record from before the sign-in key, or one naming a key the wallet
    /// does not hold, signs exactly as it did: on the first key's transports,
    /// a security key asked discoverably so any founding key on it can answer.
    #[test]
    fn a_record_without_a_sign_in_key_routes_on_its_first_key() {
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
        vault.follow_sign_in(SETTINGS_PAGE);
        assert_eq!(vault.trusted_signer.chosen(), None);

        let stranger = SendContext::new(
            &signed_in(account("internal", 2), "not-ours", KeyMethod::SecurityKey),
            ceremony(),
        );
        assert_eq!(stranger.key_method, KeyMethod::Platform);
        assert_eq!(stranger.pinned_credential.as_deref(), Some("cred0"));

        // A legacy record: one projected key, a security key.
        let legacy = SendContext::new(&account("", 0), ceremony());
        assert_eq!(legacy.keys.len(), 1);
        assert_eq!(legacy.key_method, KeyMethod::SecurityKey);
    }

    /// Spec 075, for a record without a sign-in key: a first key minted behind
    /// somebody's page is reachable nowhere else, so it still signs there.
    #[test]
    fn a_first_key_behind_a_page_still_signs_there() {
        let mut hosted = account("internal", 2);
        hosted.keys[0].signer_origin = Some("https://sign.example.test".to_owned());
        let ctx = SendContext::new(&hosted, ceremony());
        ctx.follow_sign_in(SETTINGS_PAGE);
        assert_eq!(
            ctx.trusted_signer.chosen().as_deref(),
            Some("https://sign.example.test")
        );
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

    /// The web's `resolveRecipientRisk`: an address never sent to is
    /// "first"; one in the history is not; a non-address never is.
    #[test]
    fn first_time_is_the_wallets_own_sends_and_only_for_an_address() {
        crate::executor::storage::tests::with_temp_state("send-first-time", || {
            let paid = format!("0x{}", "ab".repeat(20));
            let _ = storage::write_value(
                TX_KEY,
                json!([{ "to": paid.to_uppercase().replace("0X", "0x"), "type": "send" }]),
            );
            assert!(!first_time(&paid));
            assert!(first_time(&format!("0x{}", "cd".repeat(20))));
            assert!(!first_time("alice.eth"));
            assert!(!first_time("0xYOU"));
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
                auto_fee_token: true,
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
            SendOperation::AddNetwork { chain_id: 146 },
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
