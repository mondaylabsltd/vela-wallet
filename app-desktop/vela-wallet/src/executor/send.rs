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

use crate::diag::vlog;
use crate::executor::passkey::{self, Ceremony};
use crate::executor::trusted_signer::{self, Ask};
use crate::executor::user_op::{self, QuotedFee, Signer};
use crate::executor::{abi, balances, identity, pool, relay, storage};

/// `vela.transactionHistory` — the shared local store.
const TX_KEY: &str = "vela.transactionHistory";

/// The submit's work: it reports `OpSigned` on the way (spec 082 RJ1) and
/// settles once.
pub type SubmitWork =
    Box<dyn FnOnce(&crate::resident::Sink<vela_core::app::send::Event>) -> SendShellResult + Send>;

/// How an operation is performed — the resident's shape, plus the arm that
/// belongs to the screen.
pub enum SendAnswer {
    Now(SendShellResult),
    Blocking(Box<dyn FnOnce() -> SendShellResult + Send>),
    /// Reports events on the way and settles once — the submit, which tells
    /// the core it has signed (`OpSigned`, spec 082 RJ1) and waits for its
    /// clearance before the POST.
    Streaming(SubmitWork),
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
    /// The core's word that this payment's records are on disk and the
    /// tracker holds them (spec 082 RJ1): `ClearToPost` fills it, and the
    /// running submit POSTs only after it. Shared with every clone.
    pub clearance: Arc<user_op::Clearance>,
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
            clearance: Arc::new(user_op::Clearance::default()),
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

/// A holding as the send machine takes it.
///
/// `logo_urls` are the coin's logo candidates by the core's one rule (the
/// web's `toSendToken` fills them the same way): the core copies them into
/// the records and the receipt it writes, so a send made here still wears its
/// coins' logos when any shell opens it later. They used to be empty, and a
/// desktop sweep's coins could only ever be letters afterwards.
pub fn to_send_token(token: &BalanceToken) -> SendToken {
    SendToken {
        network: network_id(token.chain_id),
        chain_id: token.chain_id,
        symbol: token.symbol.clone(),
        balance: token.balance.clone(),
        decimals: token.decimals,
        token_address: token.token_address.clone(),
        price_usd: token.price_usd,
        logo_urls: crate::marks::token_logos(
            token.chain_id,
            &token.symbol,
            token.token_address.as_deref(),
            &[],
        )
        .logo_urls
        .iter()
        .map(ToString::to_string)
        .collect(),
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
        // Spec 082 T181: what the tracker needs to keep following a payment
        // whose submit reply was lost, across a relaunch.
        "maybeSent": record.maybe_sent,
        "submitBlock": record.submit_block,
    });
    if let Some(name) = &record.to_name {
        row["toName"] = json!(name);
    }
    if let Some(usd) = &record.usd {
        row["usd"] = json!(usd);
    }
    row
}

/// Has the tracker closed this row as landed? Such a row is a payment's only
/// record — a Send's id is its op hash — and no later write-ahead or
/// withdrawal under the same id may touch it.
fn landed(row: &Value) -> bool {
    row.get("status").and_then(Value::as_str) == Some("confirmed")
}

/// ONE atomic write for every sibling record (invariant ⑥), under the
/// store's lock (spec 082 RJ1 review: other threads write the same list).
///
/// A record whose id the store already holds REPLACES it, as the web's
/// `saveTransactions` does — the id is the op hash, which repeats for an
/// identical op (S5 and both DX9 runs shared one), so the row there is an
/// earlier attempt's: "failed" after a lost reply the relay never had, say.
/// Kept as it was, the op about to be POSTed had no pending record, and a
/// quit mid-POST left a landed payment the relaunch never followed. A landed
/// payment's row is the exception: it is never reopened.
pub fn persist_records(records: &[SendTxRecord]) -> bool {
    storage::update_list(TX_KEY, |rows| {
        for record in records {
            match rows
                .iter_mut()
                .find(|row| row.get("id").and_then(Value::as_str) == Some(record.id.as_str()))
            {
                Some(row) if landed(row) => {}
                Some(row) => *row = to_row(record),
                None => rows.push(to_row(record)),
            }
        }
        true
    })
    .is_ok()
}

/// The relay took these written-ahead records' payment (spec 082 RJ1): each
/// row's `maybeSent` becomes false, in ONE write. They stay pending.
pub fn mark_admitted(ids: &[String]) -> bool {
    storage::update_list(TX_KEY, |rows| {
        let mut named = false;
        for row in rows.iter_mut() {
            if row
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| ids.iter().any(|wanted| wanted == id))
            {
                row["maybeSent"] = json!(false);
                named = true;
            }
        }
        named
    })
    .is_ok()
}

/// Remove written-ahead records whose payment is proven never sent (spec 082
/// RJ1), in ONE write — never a landed payment's row under the same id.
pub fn delete_records(ids: &[String]) -> bool {
    storage::update_list(TX_KEY, |rows| {
        let before = rows.len();
        rows.retain(|row| {
            landed(row)
                || row
                    .get("id")
                    .and_then(Value::as_str)
                    .is_none_or(|id| !ids.iter().any(|gone| gone == id))
        });
        rows.len() != before
    })
    .is_ok()
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
            SendAnswer::Streaming(Box::new(move |sink| {
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
                // RJ1 (G34): signed and hashed — every recipient's record is
                // written and handed to the tracker before a byte goes out.
                let before_post = |user_op_hash: &str, submit_block: Option<u64>| {
                    ctx.clearance.write_ahead(user_op_hash, || {
                        sink.send(vela_core::app::send::Event::OpSigned {
                            user_op_hash: user_op_hash.to_owned(),
                            submit_block,
                            now_ms: now_ms(),
                        });
                    })
                };
                match user_op::submit(
                    chain_id,
                    &account,
                    &calls,
                    gas_fee_token.as_deref(),
                    &keys,
                    signer,
                    quoted,
                    // The send column reads its own flags for the prompt
                    // (`signing_started`, the Trusted Signer's channel).
                    &user_op::quiet,
                    // The person's own payment: no page can withdraw it.
                    &user_op::always_asked,
                    &before_post,
                ) {
                    // Accepted, or may have been sent (spec 082 RA4): either
                    // way it is recorded and tracked — a lost reply is never
                    // "failed, try again".
                    Ok(submitted) => SendShellResult::Submitted {
                        user_op_hash: submitted.user_op_hash,
                        now_ms: now_ms(),
                        maybe_sent: submitted.maybe_sent,
                        submit_block: submitted.submit_block,
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
                vlog!("send", "the transaction store refused the pending records");
            }
            SendAnswer::Now(SendShellResult::RecordsPersisted)
        }

        SendOperation::TrackSubmitted { .. } => SendAnswer::Screen,

        // Spec 082 RJ1: the records are on disk and tracked — the submit
        // waiting in `SubmitUserOp` may POST, and only now. Given only for
        // records the store really holds (review): the write answers
        // "persisted" either way, and a refused write must send nothing — the
        // wait then ends in "not sent".
        SendOperation::ClearToPost { user_op_hash } => {
            if crate::executor::tracker::resumable(user_op_hash) {
                ctx.clearance.clear(user_op_hash);
            } else {
                vlog!(
                    "relay",
                    "op={} not cleared: its records are not on disk",
                    crate::diag::short(user_op_hash)
                );
            }
            SendAnswer::Now(SendShellResult::PostCleared)
        }

        // The relay took the written-ahead payment: its records no longer
        // "may have been sent" — still pending, only the tracker closes them.
        SendOperation::MarkAdmitted { record_ids } => {
            if !mark_admitted(record_ids) {
                vlog!("send", "the transaction store refused the admitted mark");
            }
            SendAnswer::Now(SendShellResult::RecordsPersisted)
        }

        // Proven never sent: the written-ahead records go, in one write.
        SendOperation::DeleteTxRecords { ids } => {
            if !delete_records(ids) {
                vlog!("send", "the transaction store refused the withdrawal");
            }
            SendAnswer::Now(SendShellResult::RecordsPersisted)
        }

        // The tracker is the app-resident's: only the screen can reach it.
        SendOperation::TrackWithdrawn { .. } => SendAnswer::Screen,

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

    /// A holding goes to the send machine wearing its logo candidates — the
    /// core's rule, on the person's endpoint — which the records and the
    /// receipt then carry: an ERC-20 its asset entry (checksummed, then
    /// lowercase), the chain's own coin its home chain's logo.
    #[test]
    fn a_send_token_carries_its_coins_logos() {
        crate::executor::storage::tests::with_temp_state("send-token-logos", || {
            let holding = |chain_id: u32, symbol: &str, address: Option<&str>| BalanceToken {
                chain_id,
                symbol: symbol.to_owned(),
                name: symbol.to_owned(),
                balance: "1".to_owned(),
                decimals: 18,
                token_address: address.map(str::to_owned),
                price_usd: None,
                spam: false,
            };
            let usdc = to_send_token(&holding(
                100,
                "USDC",
                Some("0xddafbb505ad214d7b80b1f830fccc89b60fb7a83"),
            ));
            assert_eq!(
                usdc.logo_urls,
                vec![
                    "https://ethereum-data.getvela.app/assets/eip155-100/0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83/logo.png".to_owned(),
                    "https://ethereum-data.getvela.app/assets/eip155-100/0xddafbb505ad214d7b80b1f830fccc89b60fb7a83/logo.png".to_owned(),
                ]
            );
            let eth_on_base = to_send_token(&holding(8453, "ETH", None));
            assert_eq!(
                eth_on_base.logo_urls,
                vec!["https://ethereum-data.getvela.app/chainlogos/eip155-1.png".to_owned()]
            );
        });
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
                maybe_sent: id.ends_with("-1"),
                submit_block: id.ends_with("-1").then_some(48_479_132),
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
            // T181: the lost-reply facts travel with the row.
            assert_eq!(rows[0]["maybeSent"], false);
            assert!(rows[0]["submitBlock"].is_null());
            assert_eq!(rows[1]["maybeSent"], true);
            assert_eq!(rows[1]["submitBlock"], 48_479_132);
            // The address book's first-time tell reads the same rows.
            assert!(has_prior_interaction("0xYOU"));
            assert!(!has_prior_interaction("0xnobody"));
        });
    }

    fn stored() -> Vec<Value> {
        match storage::read_value(TX_KEY) {
            Ok(Some(Value::Array(rows))) => rows,
            _ => Vec::new(),
        }
    }

    fn written_ahead(id: &str, hash: &str) -> SendTxRecord {
        SendTxRecord {
            id: id.to_owned(),
            user_op_hash: hash.to_owned(),
            tx_hash: String::new(),
            from: "0xme".to_owned(),
            to: "0xyou".to_owned(),
            to_name: None,
            value: "0.001".to_owned(),
            symbol: "xDAI".to_owned(),
            decimals: 18,
            logo_urls: Vec::new(),
            chain_id: 100,
            timestamp_s: 1_757_000_000.0,
            usd: None,
            maybe_sent: true,
            submit_block: Some(48_487_620),
        }
    }

    /// Spec 082 RJ1 (T220), the wallet's own Send: its records are on disk,
    /// pending and "may have been sent", before the POST is cleared; the
    /// relay taking the op clears `maybeSent` (still pending, only the
    /// tracker closes it); a proven "not sent" leaves no row. The clearance
    /// is given only for an op the relaunch sweep would resume.
    #[test]
    fn a_send_is_on_disk_before_its_post_and_gone_when_never_sent() {
        const OP: &str = "0x7df211eddc00000000000000000000000000000000000000000000000000aaaa";
        crate::executor::storage::tests::with_temp_state("send-write-ahead", || {
            let ctx = SendContext::new(&account("internal", 1), ceremony());
            let cleared = |ctx: &SendContext| {
                ctx.clearance.arm(OP);
                let answer = perform(
                    &SendOperation::ClearToPost {
                        user_op_hash: OP.to_owned(),
                    },
                    ctx,
                );
                assert!(matches!(
                    answer,
                    SendAnswer::Now(SendShellResult::PostCleared)
                ));
                ctx.clearance.wait(Duration::ZERO)
            };
            // The store refused the write-ahead (a full disk): nothing is on
            // disk, so nothing may leave.
            assert!(!cleared(&ctx), "no record on disk, no POST");

            let ids = [format!("{OP}-0"), format!("{OP}-1")];
            let records = [written_ahead(&ids[0], OP), written_ahead(&ids[1], OP)];
            assert!(matches!(
                perform(
                    &SendOperation::PersistTxRecords {
                        records: records.to_vec()
                    },
                    &ctx
                ),
                SendAnswer::Now(SendShellResult::RecordsPersisted)
            ));
            let resumed = crate::executor::tracker::pending_records(&stored());
            assert_eq!(resumed.len(), 2, "both legs of the split, pending");
            assert!(resumed.iter().all(|r| r.maybe_sent && r.user_op_hash == OP));
            assert!(cleared(&ctx), "on disk: the POST may go");

            let _ = perform(
                &SendOperation::MarkAdmitted {
                    record_ids: ids.to_vec(),
                },
                &ctx,
            );
            let rows = stored();
            assert!(rows.iter().all(|row| row["status"] == "pending"));
            assert!(rows.iter().all(|row| row["maybeSent"] == false));

            let _ = perform(&SendOperation::DeleteTxRecords { ids: ids.to_vec() }, &ctx);
            assert!(stored().is_empty(), "never sent: no Activity row");
        });
    }

    /// Spec 082 RJ1 (review): a Send's hash is deterministic — S5 and both
    /// DX9 runs shared 0x7df211ed… — so a write-ahead can land on the row an
    /// earlier attempt at the SAME op left: "failed" after the tracker's
    /// NotSent for a lost reply. Kept as it was, the op about to be POSTed
    /// had no pending record — a quit mid-POST and the relaunch sweep never
    /// saw the payment land (DX9's P0, on the wallet's own Send). The
    /// write-ahead row replaces it, as the web's `saveTransactions` does. A
    /// landed payment's row is never replaced, and never deleted by a later
    /// attempt's "not sent".
    #[test]
    fn a_write_ahead_replaces_an_earlier_attempt_but_never_a_landed_payment() {
        const OP: &str = "0x7df211eddc00000000000000000000000000000000000000000000000000bbbb";
        crate::executor::storage::tests::with_temp_state("send-write-ahead-same-hash", || {
            let earlier = |status: &str, tx_hash: &str| {
                json!({
                    "id": OP, "userOpHash": OP, "txHash": tx_hash, "from": "0xme",
                    "to": "0xyou", "value": "0.001", "symbol": "xDAI", "decimals": 18,
                    "chainId": 100, "timestamp": 1_756_000_000, "status": status,
                    "type": "send", "maybeSent": false,
                })
            };
            if storage::write_value(TX_KEY, json!([earlier("failed", "")])).is_err() {
                unreachable!("seed");
            }
            assert!(persist_records(&[written_ahead(OP, OP)]));
            let rows = stored();
            assert_eq!(rows.len(), 1, "one row per id");
            let resumed = crate::executor::tracker::pending_records(&rows);
            assert_eq!(resumed.len(), 1, "the relaunch resumes the op about to go");
            assert!(resumed[0].maybe_sent);
            assert_eq!(resumed[0].submit_block, Some(48_487_620));

            // A landed payment under the same id (a stale nonce read would
            // rebuild its op): kept whole, and not withdrawn by a refusal.
            if storage::write_value(TX_KEY, json!([earlier("confirmed", "0xtx")])).is_err() {
                unreachable!("seed");
            }
            assert!(persist_records(&[written_ahead(OP, OP)]));
            assert!(delete_records(&[OP.to_owned()]));
            let rows = stored();
            assert_eq!(rows.len(), 1, "a landed payment's row stays");
            assert_eq!(rows[0]["status"], "confirmed");
            assert_eq!(rows[0]["txHash"], "0xtx");
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
                maybe_sent: false,
                submit_block: None,
                admitted: false,
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
