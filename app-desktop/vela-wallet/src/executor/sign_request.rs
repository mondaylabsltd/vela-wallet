//! The signing panel's operations — the second path in this app that spends
//! money.
//!
//! Seven sentences, and the shell decides none of them. Single-flight, "a
//! rejected pipeline may not submit", the record-then-respond order and the
//! §12.1.6 account sequencing all live in `sign_request`; this file answers
//! the transport, writes the record, asks the relay, and runs the ceremony.
//!
//! ## The one that moves money
//!
//! `SignAndSubmit` is the passkey → build → submit pipeline that
//! `executor::user_op::submit` already is for the send flow. It is the SAME
//! pipeline deliberately: a dApp's transaction and a person's own transfer
//! must be assembled, priced and signed by one implementation, or the sheet
//! that shows what will happen and the code that makes it happen are two
//! different opinions.
//!
//! It reports twice, and the difference matters:
//!
//! - **mid-flight**, the accepted `user_op_hash`, as `Event::OpSubmitted`. The
//!   operation is a [`crate::resident::Answer::Streaming`] precisely so that
//!   hash reaches the core BEFORE the receipt wait — a window closed while a
//!   submitted operation is in flight must still know it was submitted.
//! - **once**, the final outcome. For a transaction that is the real tx hash
//!   from the receipt, because that is what a dApp's `eth_sendTransaction`
//!   resolves to; the userOpHash is not a tx hash and a dApp that treats it as
//!   one looks its transaction up forever.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde_json::{Value, json};

use vela_core::app::fee_policy::FeeCall;
use vela_core::app::sign_request::{
    Event, SignFundingNeeded, SignOperation, SignRecord, SignShellResult, SignSubmitOutcome,
    dapp_receipt_wait_ms,
};
use vela_core::app::{Account, KeyMethod};
use vela_core::user_op::{NOT_SENT_DAPP_DETAIL, WalletKey};

use crate::diag::{short, vlog};
use crate::executor::passkey::{self, Ceremony};
use crate::executor::trusted_signer::{self, Ask};
use crate::executor::user_op::{CeremonyEdge, Signer};
use crate::executor::{now_ms, relay, storage, user_op};

/// `vela.transactionHistory` — the shared local store.
const TX_KEY: &str = "vela.transactionHistory";

/// How this operation is performed, and therefore where. Mirrors
/// `send::SendAnswer` — the signing panel owns its machines the way the send
/// column owns its two, so `Screen` is the arm the host takes back.
pub enum SignAnswer {
    /// Answered on this thread, this frame — a local signal, no I/O.
    Now(SignShellResult),
    Blocking(Box<dyn FnOnce() -> SignShellResult + Send>),
    /// Reports events on the way and settles once — the submit.
    Streaming(Box<dyn FnOnce(&crate::resident::Sink<Event>) -> SignShellResult + Send>),
    /// Not this module's business: the transport belongs to whoever raised the
    /// request (the browser column today), and only the host knows which.
    Screen,
}

/// What the ceremony needs, fixed when the REQUEST opens.
///
/// The same fixing `SendContext` does and for the same reason: an account
/// switch while a signature is in flight must not change who signs. §12.1.6
/// lets a grant switch the active account, which makes that not hypothetical
/// here.
#[derive(Clone)]
pub struct SignContext {
    pub keys: Vec<WalletKey>,
    pub key_method: KeyMethod,
    pub pinned_credential: Option<String>,
    /// Spec 075: the Trusted Signer page this account signs on, and the one
    /// key it may sign with there, as
    /// [`crate::executor::send::SendContext::signer_page`] and `page_key`.
    pub signer_page: Option<String>,
    pub page_key: Option<String>,
    pub ceremony: Ceremony,
    /// Raised the instant the passkey prompt opens, so the host can tell the
    /// core the ceremony started rather than guessing from elapsed time.
    pub signing_started: Arc<AtomicBool>,
    /// Who asked, as the transport says — `None` for the wallet's own
    /// requests, which the Trusted Signer's page is told as the wallet's own
    /// send rather than as a site's.
    pub site: Option<String>,
    /// The account's name, for the Trusted Signer's page.
    pub account_name: Option<String>,
    /// The Trusted Signer (spec 071): whether THIS request goes to it, and its
    /// waiting sheet. Shared, because the context is cloned into the executor
    /// when the request opens and the host points it at the page afterwards.
    pub trusted_signer: Arc<trusted_signer::Channel>,
    /// When the person approved (spec 082 RA12): the dApp's answer window is
    /// measured from the tap, so a slow submit shortens the receipt wait
    /// rather than adding a second full one. Set by the host at the tap,
    /// before the submit is asked for.
    pub approved_at_ms: Option<f64>,
    /// The page that asked is gone (spec 082 RB2, the money rule). Shared
    /// with every clone, so the column's close reaches a submit already
    /// running: it is read before the ceremony, when it answers, and right
    /// before the relay POST — nothing is signed or sent for nobody.
    asker_gone: Arc<AtomicBool>,
    /// The core's word that the write-ahead record is on disk (spec 082
    /// RJ1): `ClearToPost` fills it, and the running submit POSTs only after
    /// it. Shared with every clone, like `asker_gone`.
    clearance: Arc<user_op::Clearance>,
    /// The core has answered the page (spec 082 RJ4): the tracker knew how
    /// the operation ended before the receipt wait did. The wait stops, so a
    /// worker thread does not poll a relay for an answer already given.
    answered: Arc<AtomicBool>,
}

impl SignContext {
    /// The credential this request's ceremony is pinned to, and its method:
    /// the account's sign-in route, fixed when the request opened.
    #[must_use]
    pub fn route(&self) -> (Option<String>, KeyMethod) {
        (self.pinned_credential.clone(), self.key_method)
    }

    /// Point the Trusted Signer channel at this account's page when it signs
    /// on one — see [`crate::executor::send::SendContext::follow_sign_in`].
    pub fn follow_sign_in(&self, settings_page: &str) {
        self.trusted_signer
            .choose(crate::executor::send::page_to_follow(
                self.signer_page.as_deref(),
                settings_page,
            ));
    }

    /// This request as the Trusted Signer's page is told it (contract §1): a
    /// site's own method, params and origin — the FINAL params, invariant ⑨
    /// — or, for the wallet's own transaction, just its calls. A message is
    /// always its own method: there are no calls to tell it by.
    fn ask(&self, method: &str, params_json: &str) -> Ask {
        match &self.site {
            None if !is_message(method) => Ask::own(self.account_name.clone()),
            site => Ask {
                method: method.to_owned(),
                params: serde_json::from_str(params_json).unwrap_or_default(),
                origin: site.clone().unwrap_or_default(),
                account_name: self.account_name.clone(),
            },
        }
    }

    /// The page that asked left (its tab closed or moved on): from now on
    /// nothing is signed or sent for this request (RB2). An operation
    /// already past the relay POST is recorded and followed as ever.
    pub fn asker_left(&self) {
        self.asker_gone.store(true, Ordering::SeqCst);
    }

    /// Is the page that asked still there?
    #[must_use]
    pub fn still_asked(&self) -> bool {
        !self.asker_gone.load(Ordering::SeqCst)
    }

    /// The core has answered the page (RJ4): a receipt wait still running
    /// for it stops at its next look.
    pub fn core_answered(&self) {
        self.answered.store(true, Ordering::SeqCst);
    }

    /// Has the core answered the page already?
    #[must_use]
    pub fn answered(&self) -> bool {
        self.answered.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn new(account: &Account, ceremony: Ceremony) -> Self {
        // Deliberately `SendContext::new`'s derivation, called rather than
        // copied: two answers about which ceremony an account's keys want is
        // one wallet asking for a phone on one screen and a security key on
        // the other.
        let send = crate::executor::send::SendContext::new(account, ceremony);
        Self {
            keys: send.keys,
            key_method: send.key_method,
            pinned_credential: send.pinned_credential,
            signer_page: send.signer_page,
            page_key: send.page_key,
            ceremony: send.ceremony,
            signing_started: send.signing_started,
            site: None,
            account_name: send.account_name,
            trusted_signer: send.trusted_signer,
            approved_at_ms: None,
            asker_gone: Arc::new(AtomicBool::new(false)),
            clearance: Arc::new(user_op::Clearance::default()),
            answered: Arc::new(AtomicBool::new(false)),
        }
    }
}

/// How often the receipt wait asks. The wait itself is the core's
/// (`dapp_receipt_wait_ms`, spec 082 RA12): a dApp's promise must SETTLE, and
/// every client settles it by the same clock.
const RECEIPT_POLL: Duration = Duration::from_secs(3);

/// The receipt wait left for a submit that finished `now` after an approve
/// at `approved_at` — what is left of the core's 120 s window, never less
/// than its floor.
fn receipt_budget(approved_at: Option<f64>, now: f64) -> Duration {
    let elapsed = approved_at.map_or(0.0, |at| (now - at).max(0.0));
    let wait_ms = dapp_receipt_wait_ms(elapsed);
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a wait of at most two minutes"
    )]
    Duration::from_millis(wait_ms.max(0.0) as u64)
}

pub fn perform(operation: &SignOperation, ctx: &SignContext) -> SignAnswer {
    match operation {
        // The transport is the host's: only it knows which surface raised
        // this request and how to answer it.
        SignOperation::SendResponse { .. } => SignAnswer::Screen,

        // After each write the feed is poked on the tracker's next tick
        // (spec 082 RG3): a dApp's row is on screen within seconds, not at the
        // feed's own pass.
        SignOperation::PersistRecord { record } => {
            let record = record.clone();
            SignAnswer::Blocking(Box::new(move || {
                persist_record(&record);
                crate::executor::tracker::records_written();
                SignShellResult::RecordPersisted
            }))
        }

        // Spec 082 RJ1: the write-ahead record is on disk — the submit
        // waiting in `sign_and_submit` may POST, and only now. Given only for
        // a record the store really holds (review): a refused write sends
        // nothing, and the wait ends in "not sent".
        SignOperation::ClearToPost { user_op_hash, .. } => {
            if crate::executor::tracker::resumable(user_op_hash) {
                ctx.clearance.clear(user_op_hash);
            } else {
                vlog!(
                    "relay",
                    "op={} not cleared: its record is not on disk",
                    short(user_op_hash)
                );
            }
            SignAnswer::Now(SignShellResult::Responded)
        }

        // Spec 082 RJ1: a write-ahead record whose op is proven never sent.
        SignOperation::DeleteRecord { record_id } => {
            let record_id = record_id.clone();
            SignAnswer::Blocking(Box::new(move || {
                delete_record(&record_id);
                crate::executor::tracker::records_written();
                SignShellResult::RecordUpdated
            }))
        }

        SignOperation::UpdateRecord { record_id, close } => {
            let (record_id, close) = (record_id.clone(), close.clone());
            SignAnswer::Blocking(Box::new(move || {
                update_record(&record_id, &close);
                crate::executor::tracker::records_written();
                SignShellResult::RecordUpdated
            }))
        }

        SignOperation::SwitchActiveAccount { index } => {
            let index = *index;
            SignAnswer::Blocking(Box::new(move || {
                // Best effort, and the core is told either way: it sequences
                // "switch first, then the approval surface may act" off this
                // acknowledgement, so withholding it would strand the grant.
                let _ = storage::save_active_index(index as usize);
                SignShellResult::AccountSwitched
            }))
        }

        SignOperation::CheckBundlerFunding {
            chain_id,
            account,
            bust_cache,
            bundler_cost_wei,
        } => {
            let (chain_id, account, bust_cache) = (*chain_id, account.clone(), *bust_cache);
            let cost = bundler_cost_wei.as_deref().and_then(parse_wei);
            SignAnswer::Blocking(Box::new(move || {
                if bust_cache {
                    // A retry after somebody funded the account must not read
                    // the balance from before they funded it.
                    relay::clear_cache(chain_id, Some(&account));
                }
                // `None` means "proceed to submit" — including when the check
                // itself could not be made: an errored pre-check is not a
                // refusal, and the submit's own underfunded answer is the
                // authority (078 W-05).
                SignShellResult::PreCheck {
                    funding: relay::check_funding(chain_id, &account, cost).map(|funding| {
                        SignFundingNeeded {
                            deposit_address: funding.deposit_address,
                            safe_address: funding.safe_address,
                            chain_id: funding.chain_id,
                            native_symbol: funding.native_symbol,
                            threshold_wei: funding.threshold_wei.to_string(),
                            recommended_wei: funding.recommended_wei.to_string(),
                            current_balance_wei: funding.current_balance.to_string(),
                        }
                    }),
                }
            }))
        }

        // `attemptSilentSponsorship`: the treasury, asked only now that the
        // person has approved (078 W-05).
        SignOperation::AttemptSponsorship { funding, force } => {
            let force = *force;
            let funding = relay::FundingNeeded {
                deposit_address: funding.deposit_address.clone(),
                safe_address: funding.safe_address.clone(),
                chain_id: funding.chain_id,
                native_symbol: funding.native_symbol.clone(),
                threshold_wei: parse_wei(&funding.threshold_wei).unwrap_or(0),
                recommended_wei: parse_wei(&funding.recommended_wei).unwrap_or(0),
                current_balance: parse_wei(&funding.current_balance_wei).unwrap_or(0),
            };
            SignAnswer::Blocking(Box::new(move || {
                use vela_core::app::sign_request::SignSponsorship;
                SignShellResult::Sponsorship {
                    outcome: match relay::attempt_sponsorship(&funding, force) {
                        relay::Sponsorship::Funded => SignSponsorship::Funded,
                        relay::Sponsorship::Confirming => SignSponsorship::Confirming,
                        relay::Sponsorship::Denied { reason } => SignSponsorship::Denied { reason },
                    },
                }
            }))
        }

        SignOperation::SignAndSubmit {
            id,
            method,
            params_json,
            chain_id,
            address,
            gas_fee_token,
            quoted_fee,
            ..
        } => {
            let (chain_id, address, method, id) =
                (*chain_id, address.clone(), method.clone(), id.clone());
            let params_json = params_json.clone();
            let gas_fee_token = gas_fee_token.clone();
            let quoted = quoted_fee.as_ref().and_then(|fee| {
                Some(user_op::QuotedFee {
                    amount: fee.amount.parse().ok()?,
                    recipient: fee.recipient.clone(),
                    // The speed this fee was priced at (spec 069).
                    tier: fee.tier,
                })
            });
            let ctx = ctx.clone();
            SignAnswer::Streaming(Box::new(move |sink| {
                let outcome = sign_and_submit(
                    &ctx,
                    &id,
                    chain_id,
                    &address,
                    &method,
                    &params_json,
                    gas_fee_token.as_deref(),
                    quoted,
                    sink,
                );
                SignShellResult::Submit {
                    outcome,
                    now_ms: now_ms(),
                }
            }))
        }
    }
}

#[allow(clippy::too_many_arguments, reason = "one pipeline, named parameters")]
fn sign_and_submit(
    ctx: &SignContext,
    id: &str,
    chain_id: u32,
    address: &str,
    method: &str,
    params_json: &str,
    gas_fee_token: Option<&str>,
    quoted: Option<user_op::QuotedFee>,
    sink: &crate::resident::Sink<Event>,
) -> SignSubmitOutcome {
    // RB2: the page left between the approval and this worker starting.
    if !ctx.still_asked() {
        return asker_gone(id);
    }
    if is_message(method) {
        return sign_message(ctx, id, chain_id, address, method, params_json);
    }
    let Some(calls) = calls_of(method, params_json) else {
        return SignSubmitOutcome::Failed {
            message: format!("{method} carried no transaction this wallet could read"),
            refused: false,
        };
    };

    let mut sign = |challenge: &[u8]| {
        ctx.signing_started.store(true, Ordering::SeqCst);
        let (credential, method) = ctx.route();
        passkey::assert(challenge, credential.as_deref(), method, &ctx.ceremony)
    };
    let ask = ctx.ask(method, params_json);
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
    let edges = |edge: CeremonyEdge| sink.send(ceremony_event(id, edge));
    let asked = || ctx.still_asked();
    // RJ1 (G34): signed, hashed and the head read — the core writes the
    // record before a byte goes to the relay, and says so (`ClearToPost`).
    let before_post = |user_op_hash: &str, submit_block: Option<u64>| {
        ctx.clearance.write_ahead(user_op_hash, || {
            sink.send(Event::OpSigned {
                id: id.to_owned(),
                user_op_hash: user_op_hash.to_owned(),
                submit_block,
                now_ms: now_ms(),
            });
        })
    };
    let submitted = user_op::submit(
        chain_id,
        address,
        &calls,
        gas_fee_token,
        &ctx.keys,
        signer,
        quoted,
        &edges,
        &asked,
        &before_post,
    );
    let submitted = match submitted {
        Ok(submitted) => submitted,
        Err(user_op::SubmitFailure::AskerGone) => return asker_gone(id),
        Err(failure) => return submit_failure(chain_id, address, failure),
    };
    let user_op_hash = submitted.user_op_hash.clone();

    // Told BEFORE the receipt wait. A window closed during that wait must
    // still know an operation left — otherwise a submitted transaction looks,
    // on reopen, like one that never happened. A lost reply is told the same
    // way (spec 082 RA2/RA3): recorded under the local hash, handed to the
    // tracker, and answered once like any other.
    sink.send(Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: user_op_hash.clone(),
        now_ms: now_ms(),
        maybe_sent: submitted.maybe_sent,
        submit_block: submitted.submit_block,
    });

    let budget = receipt_budget(ctx.approved_at_ms, now_ms());
    let answered = || ctx.answered();
    let receipt = await_receipt(&user_op_hash, chain_id, budget, &answered);
    if ctx.answered() {
        // RJ4: the tracker's verdict answered the page first; the core drops
        // this late result.
        vlog!(
            "dapp",
            "op={} receipt wait ended: answered from the tracker",
            short(&user_op_hash)
        );
    } else {
        vlog!(
            "dapp",
            "op={} answered with {} after the receipt wait",
            short(&user_op_hash),
            if receipt.is_some() {
                "its tx hash"
            } else {
                "the op hash"
            }
        );
    }
    after_receipt_wait(user_op_hash, receipt)
}

/// The page that asked is gone and nothing was signed or sent (RB2): the core
/// is told `AskerGone` — no answer, no record, the sheet clears.
fn asker_gone(id: &str) -> SignSubmitOutcome {
    vlog!(
        "dapp",
        "request {id}: the page that asked is gone; nothing signed or sent"
    );
    SignSubmitOutcome::AskerGone
}

/// The core event for one edge of request `id`'s ceremony (spec 082 RA9).
fn ceremony_event(id: &str, edge: CeremonyEdge) -> Event {
    match edge {
        CeremonyEdge::Started => Event::CeremonyStarted { id: id.to_owned() },
        CeremonyEdge::Signed => Event::CeremonyDone { id: id.to_owned() },
    }
}

/// A signature, not a transaction (the phones' `SignExecutor`): one
/// ceremony over the Safe's `SafeMessage` hash of what the site asked to
/// sign, answered as the EIP-1271 envelope — nothing submitted, no receipt.
fn sign_message(
    ctx: &SignContext,
    id: &str,
    chain_id: u32,
    address: &str,
    method: &str,
    params_json: &str,
) -> SignSubmitOutcome {
    let Some(original) = message_hash(method, params_json) else {
        return SignSubmitOutcome::Failed {
            message: format!("{method} carried nothing this wallet could sign"),
            refused: false,
        };
    };
    let mut sign = |challenge: &[u8]| {
        ctx.signing_started.store(true, Ordering::SeqCst);
        let (credential, method) = ctx.route();
        passkey::assert(challenge, credential.as_deref(), method, &ctx.ceremony)
    };
    let ask = ctx.ask(method, params_json);
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
    let asked = || ctx.still_asked();
    match user_op::sign_message(
        chain_id,
        address,
        &original,
        &ctx.keys,
        signer,
        &user_op::quiet,
        &asked,
    ) {
        Ok(signature) => SignSubmitOutcome::Succeeded { result: signature },
        Err(user_op::SubmitFailure::AskerGone) => asker_gone(id),
        Err(failure) => submit_failure(chain_id, address, failure),
    }
}

/// The methods the sheet signs as a message rather than submits.
fn is_message(method: &str) -> bool {
    vela_core::sign_message::is_message_method(method)
}

/// What the site asked to sign, before the Safe's wrap — the core's one rule
/// (`vela_core::sign_message`), which the Trusted Signer's page shares.
pub fn message_hash(method: &str, params_json: &str) -> Option<Vec<u8>> {
    vela_core::sign_message::original_hash(method, params_json)
}

/// What the receipt wait means for the core.
///
/// A dApp's `eth_sendTransaction` resolves to a TX hash — handing back the
/// userOpHash instead gives the site something it can look up forever and
/// never find. When the wait runs out the op is submitted, NOT confirmed:
/// the page is still answered with the op hash (a dApp left waiting cannot
/// tell a slow chain from a lost transaction — the double-spend risk 027 D37
/// names), but the core must hear it as `ReceiptPending` so the record stays
/// pending until the tracker sees the receipt (issue 262).
pub fn after_receipt_wait(user_op_hash: String, receipt: Option<String>) -> SignSubmitOutcome {
    match receipt {
        Some(tx_hash) => SignSubmitOutcome::Succeeded { result: tx_hash },
        None => SignSubmitOutcome::ReceiptPending { user_op_hash },
    }
}

/// The calls a request is asking for.
///
/// The params are FINAL by the time they reach here (the core's invariant ⑨
/// caps them), so this only reads them.
///
/// Shared with the host, which prices the SAME calls it will later submit —
/// two readings of one params array is how a quote ends up describing a
/// different transaction than the one that gets signed.
pub fn calls_of(method: &str, params_json: &str) -> Option<Vec<FeeCall>> {
    let params: Value = serde_json::from_str(params_json).ok()?;
    let first = params.get(0)?;
    match method {
        // EIP-5792: one entry carrying many calls.
        "wallet_sendCalls" => {
            let calls: Vec<FeeCall> = first
                .get("calls")?
                .as_array()?
                .iter()
                .filter_map(fee_call)
                .collect();
            // An empty batch is not a batch. It would assemble into a user
            // operation that does nothing and still costs a fee — and every
            // call being unreadable produces the same empty vector as a batch
            // that was empty to begin with, which is why this is checked
            // AFTER the mapping and not before it.
            (!calls.is_empty()).then_some(calls)
        }
        _ => Some(vec![fee_call(first)?]),
    }
}

fn fee_call(raw: &Value) -> Option<FeeCall> {
    Some(FeeCall {
        to: raw.get("to")?.as_str()?.to_owned(),
        value: wei_of(raw.get("value"))?,
        data: raw
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or("0x")
            .to_owned(),
    })
}

/// A JSON-RPC quantity as wei, or `None` when it is not a number.
///
/// **Absent is zero; unreadable is not.** Most contract calls carry no value,
/// so a missing field is 0 — but a field that is there and cannot be read used
/// to be priced as 0 too, which puts a wrong fee beside a real transaction.
/// Refusing it draws no fee instead, and no fee is better than a made-up one.
///
/// `"0x"` is zero. Several dApp libraries write it that way, and taking it for
/// unreadable is what cost the WEB shell its fee and its speed control on a
/// Uniswap swap (B-4, owner 2026-09-23).
fn wei_of(value: Option<&Value>) -> Option<String> {
    let Some(value) = value else {
        return Some("0".to_owned());
    };
    if value.is_null() {
        return Some("0".to_owned());
    }
    if let Some(number) = value.as_u64() {
        return Some(number.to_string());
    }
    let text = value.as_str()?.trim();
    let digits = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))
        .map(|hex| (hex, 16))
        .unwrap_or((text, 10));
    if digits.0.is_empty() {
        return Some("0".to_owned());
    }
    u128::from_str_radix(digits.0, digits.1)
        .ok()
        .map(|wei| wei.to_string())
}

/// Poll until the receipt lands, the budget runs out, or the core has
/// answered the page from the tracker (`answered`, spec 082 RJ4).
///
/// `None` is "not yet", never "failed": a relay that could not be reached is
/// not a transaction that did not happen, and the caller answers with the
/// userOpHash rather than an error.
fn await_receipt(
    user_op_hash: &str,
    chain_id: u32,
    budget: Duration,
    answered: &dyn Fn() -> bool,
) -> Option<String> {
    wait_within(budget, RECEIPT_POLL, answered, |left| {
        // A receipt that says the operation reverted is still a receipt: the
        // tx hash is real and the dApp should have it. What it is NOT is this
        // wallet's business to relabel.
        relay::user_op_receipt_until(user_op_hash, chain_id, left, answered)
            .resolution
            .map(|resolution| resolution.tx_hash)
    })
}

/// Ask `poll` until it answers, `budget` is spent or `stop` says the answer
/// is no longer wanted, sleeping `every` between asks — and giving each ask
/// only what is LEFT of the budget (spec 079, device-found on Android: with
/// the relay unreachable one poll hung for its own timeouts and retries, and
/// the page waited 268 s for a two-minute wait). The page is answered on
/// time; the tracker keeps following the operation after.
fn wait_within<T>(
    budget: Duration,
    every: Duration,
    stop: &dyn Fn() -> bool,
    mut poll: impl FnMut(Duration) -> Option<T>,
) -> Option<T> {
    /// How often a sleep between asks looks at `stop`.
    const GLANCE: Duration = Duration::from_millis(200);
    let deadline = std::time::Instant::now() + budget;
    loop {
        let left = deadline.saturating_duration_since(std::time::Instant::now());
        if left.is_zero() || stop() {
            return None;
        }
        if let Some(answer) = poll(left) {
            return Some(answer);
        }
        let resume = std::time::Instant::now() + every;
        loop {
            let now = std::time::Instant::now();
            if now >= deadline || stop() {
                return None;
            }
            if now >= resume {
                break;
            }
            std::thread::sleep(GLANCE.min(resume - now).min(deadline - now));
        }
    }
}

/// A submit that failed, in the core's vocabulary.
///
/// `SubmitFailure` is already typed, so nothing here matches on wording — the
/// regex layer the core's doc warns about (`parseBundlerUnderfunded`,
/// `PasskeyErrorCode.CANCELLED`) was already paid for by spec 032's send path
/// and is not paid for twice.
fn submit_failure(
    chain_id: u32,
    account: &str,
    failure: user_op::SubmitFailure,
) -> SignSubmitOutcome {
    match failure {
        // Dismissing the passkey sheet is never an error and never a
        // response: the person did not decide, and a 4001 would report that
        // they declined.
        user_op::SubmitFailure::PasskeyCancelled => SignSubmitOutcome::PasskeyCancelled,
        user_op::SubmitFailure::BundlerUnderfunded => {
            let message = "the relay's gas account is underfunded".to_owned();
            // `funding` stays `None` until the desktop has a source for the
            // threshold and recommended amounts. Those are POLICY numbers,
            // and a funding screen that invents them tells somebody to send
            // the wrong amount — the core's documented fallback for a funding
            // it cannot compose is a generic failure, which is honest.
            let _ = (chain_id, account);
            SignSubmitOutcome::Underfunded {
                message,
                funding: None,
            }
        }
        user_op::SubmitFailure::RelayerUnavailable => SignSubmitOutcome::Failed {
            message: "the relay could not be reached".to_owned(),
            refused: false,
        },
        // Nothing left the device (spec 082 RA10): the dApp's -32603 carries
        // the core's fixed sentence, never the pool's "all endpoints failed".
        user_op::SubmitFailure::NotSent => SignSubmitOutcome::Failed {
            message: NOT_SENT_DAPP_DETAIL.to_owned(),
            refused: false,
        },
        user_op::SubmitFailure::Other(message) => SignSubmitOutcome::Failed {
            message,
            refused: false,
        },
        // The relay refused it (spec 082 RJ3): the page is answered the
        // core's "refused" sentence, and the sheet never says "try again".
        user_op::SubmitFailure::Refused(message) => SignSubmitOutcome::Failed {
            message,
            refused: true,
        },
        // Nothing signed or sent for a page that has gone (RB2).
        user_op::SubmitFailure::AskerGone => SignSubmitOutcome::AskerGone,
    }
}

/// One dApp signature or transaction, in the store every other client reads.
///
/// These two functions were stubs until spec 032 phase 29, which means every
/// dApp transaction this wallet ever submitted **left no trace**: nothing in
/// the activity feed, and nothing for the tracker to settle on the next
/// launch. The send path has had this since phase 4; this is the same promise
/// (User Story 2 — money in flight outlives every window) for the path a dApp
/// drives.
///
/// Shape: `buildSigningRecord` (`dapp-history.ts:162-228`), field for field —
/// the row is read by the feed, by the phone and by the web.
pub(crate) fn persist_record(record: &SignRecord) {
    use vela_core::app::sign_request::SignRecordKind;

    let (kind, to, value, symbol, decimals) = match record.kind {
        SignRecordKind::DappTx => {
            let call = first_param(&record.params_json);
            (
                "dapp_tx",
                call.as_ref()
                    .and_then(|tx| tx.get("to"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                call.as_ref()
                    .and_then(|tx| tx.get("value"))
                    .and_then(Value::as_str)
                    .unwrap_or("0x0")
                    .to_owned(),
                native_symbol(record.chain_id),
                18,
            )
        }
        // A signature moves nothing, and a row that claimed a value and a
        // symbol would show up in the feed as money.
        SignRecordKind::SignTypedData => (
            "sign_typed_data",
            String::new(),
            "0".to_owned(),
            String::new(),
            0,
        ),
        SignRecordKind::SignMessage => (
            "sign_message",
            String::new(),
            "0".to_owned(),
            String::new(),
            0,
        ),
    };

    #[allow(
        clippy::cast_possible_truncation,
        reason = "seconds, as the store keeps them"
    )]
    let timestamp = (record.now_ms / 1000.0) as i64;
    // What was signed, kept for the replay view — clipped, because a payload
    // is untrusted and unbounded and this file is the one that decides how
    // much of it lives on the disk forever.
    let (signed_request, truncated) = clip(&record.params_json);
    let mut row = json!({
        "id": record.record_id,
        "userOpHash": record.user_op_hash,
        "txHash": record.result,
        "from": record.from,
        "to": to,
        "value": value,
        "symbol": symbol,
        "decimals": decimals,
        "chainId": record.chain_id,
        "timestamp": timestamp,
        "status": match record.status {
            vela_core::app::sign_request::SignRecordStatus::Pending => "pending",
            vela_core::app::sign_request::SignRecordStatus::Confirmed => "confirmed",
        },
        "type": kind,
        "dappOrigin": record.dapp_origin,
        "signedRequest": signed_request,
        "requestTruncated": truncated,
        // Spec 082 T181: a may-have-been-sent op and the head read before its
        // first POST travel with the record, so a relaunch hands the tracker
        // the same op it was following (`tracker::pending_records`).
        "maybeSent": record.maybe_sent,
        "submitBlock": record.submit_block,
    });
    if let Some(intent) = &record.intent {
        row["intent"] = json!(intent);
    }

    // One read-modify-write under the store's lock (spec 082 RJ1 review):
    // this runs on a worker while the tracker and the Send write the same
    // list, and the write-ahead's record must be the row that survives.
    let written = storage::update_list(TX_KEY, |rows| {
        // Same id, never a second row — a resubmit of the same request closes
        // the record it opened (the core's note on `SignRecordClose`).
        if let Some(existing) = rows
            .iter_mut()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(record.record_id.as_str()))
        {
            *existing = row;
        } else {
            rows.push(row);
        }
        true
    });
    if let Err(error) = written {
        vlog!("dapp", "record {} not written: {error}", record.record_id);
    }
}

/// Close a pending record IN PLACE.
fn update_record(record_id: &str, close: &vela_core::app::sign_request::SignRecordClose) {
    use vela_core::app::sign_request::SignRecordClose;

    let _ = storage::update_list(TX_KEY, |rows| {
        let Some(row) = rows
            .iter_mut()
            .find(|row| row.get("id").and_then(Value::as_str) == Some(record_id))
        else {
            return false;
        };
        match close {
            SignRecordClose::Confirmed { tx_hash } => {
                row["status"] = json!("confirmed");
                row["txHash"] = json!(tx_hash);
            }
            SignRecordClose::Failed => row["status"] = json!("failed"),
            // Spec 082 RJ1: the relay took the written-ahead op. It stays
            // pending — only the tracker closes it — and no longer "may have
            // been sent".
            SignRecordClose::Admitted => row["maybeSent"] = json!(false),
        }
        true
    });
}

/// Remove a record whose op is proven never sent (spec 082 RJ1): the
/// write-ahead wrote it before the POST, and nothing left the device.
fn delete_record(record_id: &str) {
    let _ = storage::update_list(TX_KEY, |rows| {
        let before = rows.len();
        rows.retain(|row| row.get("id").and_then(Value::as_str) != Some(record_id));
        rows.len() != before
    });
}

/// The first element of a JSON-RPC params array, when it is an object.
fn first_param(params_json: &str) -> Option<Value> {
    serde_json::from_str::<Value>(params_json)
        .ok()?
        .get(0)
        .filter(|first| first.is_object())
        .cloned()
}

/// The native coin of a chain, for the row's symbol.
fn native_symbol(chain_id: u32) -> String {
    vela_core::app::network_admin::BUILTIN_CHAINS
        .iter()
        .find(|chain| chain.chain_id == chain_id)
        .map_or_else(String::new, |chain| chain.native_symbol.to_owned())
}

/// The stored payload, and whether it was cut.
///
/// A page chooses this string's length; the wallet chooses how much of it it
/// keeps. 8 KB is well past any real request and far short of a store a site
/// could grow on purpose.
fn clip(params_json: &str) -> (String, bool) {
    const CAP: usize = 8 * 1024;
    if params_json.len() <= CAP {
        return (params_json.to_owned(), false);
    }
    // On a char boundary: a truncated multibyte tail is not JSON and not text.
    let mut end = CAP;
    while end > 0 && !params_json.is_char_boundary(end) {
        end -= 1;
    }
    (params_json[..end].to_owned(), true)
}

/// A wei figure off the wire — decimal, as the core writes it, or `0x` hex
/// (the web's `BigInt(value)` takes both). `None` when it is neither.
fn parse_wei(value: &str) -> Option<u128> {
    let value = value.trim();
    match value.strip_prefix("0x") {
        Some(hex) => u128::from_str_radix(hex, 16).ok(),
        None => value.parse().ok(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use vela_core::app::sign_request::{
        SignRecord, SignRecordClose, SignRecordKind, SignRecordStatus,
    };

    fn record(kind: SignRecordKind, params: &str) -> SignRecord {
        SignRecord {
            record_id: "dapp-1757000000000-tx".to_owned(),
            kind,
            method: "eth_sendTransaction".to_owned(),
            params_json: params.to_owned(),
            result: String::new(),
            from: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            chain_id: 100,
            now_ms: 1_757_000_000_000.0,
            status: SignRecordStatus::Pending,
            user_op_hash: "0xhash".to_owned(),
            dapp_origin: "https://app.uniswap.org".to_owned(),
            intent: Some("Swap".to_owned()),
            maybe_sent: false,
            submit_block: None,
        }
    }

    fn context(site: Option<&str>) -> SignContext {
        context_signed_in(site, None)
    }

    fn context_signed_in(
        site: Option<&str>,
        signed_in_with: Option<vela_core::app::SignInKey>,
    ) -> SignContext {
        let account = Account {
            id: "cred0".to_owned(),
            name: "savings".to_owned(),
            address: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            public_key_hex: "04aa".to_owned(),
            created_at_iso: String::new(),
            keys: vec![vela_core::app::AccountKey {
                credential_id: "cred0".to_owned(),
                public_key_hex: "04aa".to_owned(),
                name: String::new(),
                transports: "internal".to_owned(),
                signer_origin: None,
            }],
            signed_in_with,
        };
        let mut ctx = SignContext::new(
            &account,
            crate::ceremony::CeremonyChannel::new().ceremony(0),
        );
        ctx.site = site.map(str::to_owned);
        ctx
    }

    /// A site's request signs exactly as the wallet's own send does: with the
    /// key the account signed in with, over its route (founder, 2026-09-26) —
    /// the sheet offers no other.
    #[test]
    fn a_request_signs_with_the_sign_in_key() {
        let ctx = context_signed_in(
            Some("https://app.uniswap.org"),
            Some(vela_core::app::SignInKey {
                credential_id: "cred0".to_owned(),
                method: KeyMethod::Hybrid,
                transports: String::new(),
                signer_origin: None,
            }),
        );
        assert_eq!(ctx.route(), (Some("cred0".to_owned()), KeyMethod::Hybrid));
        ctx.follow_sign_in("https://sign.getvela.app/");
        assert_eq!(ctx.trusted_signer.chosen(), None);

        let through_page = context_signed_in(
            None,
            Some(vela_core::app::SignInKey {
                credential_id: "cred0".to_owned(),
                method: KeyMethod::TrustedSigner,
                transports: String::new(),
                signer_origin: None,
            }),
        );
        through_page.follow_sign_in("https://sign.getvela.app/");
        assert_eq!(
            through_page.trusted_signer.chosen().as_deref(),
            Some("https://sign.getvela.app/")
        );

        // A record from before the sign-in key: the first key, as always.
        let legacy = context(None);
        assert_eq!(
            legacy.route(),
            (Some("cred0".to_owned()), KeyMethod::Platform)
        );
    }

    /// A site's request reaches the page as the site's — its method, its
    /// FINAL params, its origin; the wallet's own is the wallet's own send.
    #[test]
    fn the_page_is_told_whose_request_it_is() {
        let params = r#"[{"to":"0xbbb","value":"0x1"}]"#;
        let site = context(Some("https://app.uniswap.org")).ask("eth_sendTransaction", params);
        assert_eq!(site.method, "eth_sendTransaction");
        assert_eq!(site.origin, "https://app.uniswap.org");
        assert_eq!(site.params[0]["to"], "0xbbb");
        assert_eq!(site.account_name.as_deref(), Some("savings"));

        let own = context(None).ask("eth_sendTransaction", params);
        assert_eq!(own.method, "", "the core builds the wallet's own intent");
        assert_eq!(own.origin, "");
    }

    /// `personal_sign` hashes its bytes under EIP-191 — hex when the
    /// payload is hex, the text itself otherwise — so "hello" and its hex
    /// sign the same thing, the digest every EIP-191 verifier knows.
    #[test]
    fn a_message_is_hashed_the_way_its_verifier_hashes_it() {
        let hello = "50b2c43fd39106bafbba0da34fc430e1f91e3c96ea2acee2bc34119f92b37750";
        let hashed = |method: &str, params: &str| {
            message_hash(method, params).map(|hash| vela_core::primitives::to_hex(&hash, false))
        };
        assert_eq!(
            hashed("personal_sign", r#"["hello","0xabc"]"#).as_deref(),
            Some(hello)
        );
        assert_eq!(
            hashed("personal_sign", r#"["0x68656c6c6f","0xabc"]"#).as_deref(),
            Some(hello)
        );
        assert_eq!(
            hashed("eth_sign", r#"["0xabc","0x68656c6c6f"]"#).as_deref(),
            Some(hello)
        );
        assert_eq!(hashed("personal_sign", r#"["","0xabc"]"#), None);
        assert_eq!(hashed("personal_sign", "[]"), None);
    }

    /// Typed data is the core's EIP-712 digest of the document the page
    /// derives it from: the second parameter, or the first for the legacy
    /// names and when the second is missing.
    #[test]
    fn typed_data_is_the_cores_digest_of_the_right_parameter() {
        let document = r#"{"types":{"EIP712Domain":[{"name":"name","type":"string"}],"Mail":[{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{"name":"Vela"},"message":{"contents":"hi"}}"#;
        let digest = vela_core::eip712::hash_typed_data(document).ok();
        assert!(digest.is_some());
        let quoted = serde_json::to_string(document).unwrap_or_default();
        assert_eq!(
            message_hash("eth_signTypedData_v4", &format!(r#"["0xabc",{quoted}]"#)),
            digest
        );
        assert_eq!(
            message_hash("eth_signTypedData_v4", &format!(r#"["0xabc",{document}]"#)),
            digest,
            "a document sent as an object is the same document"
        );
        assert_eq!(
            message_hash("eth_signTypedData_v4", &format!("[{quoted}]")),
            digest
        );
        assert_eq!(
            message_hash("eth_signTypedData", &format!(r#"[{quoted},"0xabc"]"#)),
            digest
        );
        assert!(is_message("eth_signTypedData_v4") && is_message("personal_sign"));
        assert!(!is_message("eth_sendTransaction") && !is_message("wallet_sendCalls"));
    }

    /// Issue 262: a receipt that is late is not a confirmation. The core hears
    /// `ReceiptPending` (answer the page, keep the record pending); only a
    /// receipt in time is `Succeeded` with the TX hash.
    #[test]
    fn every_poll_gets_only_what_is_left_of_the_wait() {
        let budget = Duration::from_millis(300);
        let started = std::time::Instant::now();
        let mut given = Vec::new();
        let answer: Option<()> =
            wait_within(budget, Duration::from_millis(40), &|| false, |left| {
                given.push(left);
                // A relay that holds every call for as long as it is allowed.
                std::thread::sleep(left.min(Duration::from_millis(120)));
                None
            });
        assert!(answer.is_none(), "no receipt is not a failure, it is none");
        assert!(
            started.elapsed() < budget + Duration::from_millis(150),
            "the wait ends on time: {:?}",
            started.elapsed()
        );
        assert!(given.len() >= 2, "it asks again while there is time");
        assert!(given.iter().all(|left| *left <= budget));
        assert!(
            given.windows(2).all(|pair| pair[1] < pair[0]),
            "each ask gets less: {given:?}"
        );
        // An answer ends the wait at once.
        assert_eq!(
            wait_within(budget, Duration::from_millis(40), &|| false, |_| Some(
                "0xtx"
            )),
            Some("0xtx")
        );
    }

    /// Spec 082 RJ4: once the core has answered the page from the tracker,
    /// the receipt wait stops at its next look — within a glance of the
    /// sleep between asks — instead of polling a relay for two minutes.
    #[test]
    fn the_wait_stops_once_the_core_has_answered() {
        let ctx = context(Some("http://127.0.0.1:8137"));
        let running = ctx.clone();
        let asked = std::sync::atomic::AtomicU32::new(0);
        let started = std::time::Instant::now();
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(150));
            ctx.core_answered();
        });
        let answered = || running.answered();
        let answer: Option<()> = wait_within(
            Duration::from_secs(10),
            Duration::from_secs(3),
            &answered,
            |_| {
                asked.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                None
            },
        );
        let _ = stopper.join();
        assert!(answer.is_none());
        assert_eq!(asked.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(
            started.elapsed() < Duration::from_millis(800),
            "stopped at {:?}, not at the next 3 s poll",
            started.elapsed()
        );
    }

    #[test]
    fn a_late_receipt_is_reported_pending_never_succeeded() {
        assert_eq!(
            after_receipt_wait("0xop".to_owned(), None),
            SignSubmitOutcome::ReceiptPending {
                user_op_hash: "0xop".to_owned()
            }
        );
        assert_eq!(
            after_receipt_wait("0xop".to_owned(), Some("0xtx".to_owned())),
            SignSubmitOutcome::Succeeded {
                result: "0xtx".to_owned()
            }
        );
    }

    // -- spec 082 T051: one answer after a lost reply, and the phases -------

    const LOCAL_OP: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
    const LANDED_TX: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
    const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    const TAB: &str = "tab-1";

    type Host = crate::core_host::CoreHost<vela_core::app::sign_request::SignRequest>;
    type Ops = Vec<crate::core_host::Pending<SignOperation>>;

    /// The desktop's host with one account and one plain dApp send on Gnosis
    /// from an in-app browser tab, approved: the pre-check is out.
    fn approved_send(id: &str) -> (Host, Ops) {
        use vela_core::app::sign_request::{SignAccountRef, SignApproveOpts};
        let mut host = Host::new();
        host.dispatch(Event::NetworksChanged {
            chain_ids: vec![100],
        });
        host.dispatch(Event::AccountsChanged {
            accounts: vec![SignAccountRef {
                address: ME.to_owned(),
                credential_id: "cred0".to_owned(),
            }],
            active_index: 0,
        });
        host.dispatch(Event::RequestArrived {
            id: id.to_owned(),
            method: "eth_sendTransaction".to_owned(),
            params_json:
                r#"[{"to":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","value":"0x38d7ea4c68000"}]"#
                    .to_owned(),
            origin: "http://127.0.0.1:8137".to_owned(),
            transport_id: TAB.to_owned(),
            dedicated_transport: true,
            per_request_chain: Some(100),
            dapp: None,
            granted_address: Some(ME.to_owned()),
            requested_address: None,
            request_ts_ms: None,
            now_ms: 1_000.0,
        });
        let ops = host.dispatch(Event::ApproveTapped {
            opts: SignApproveOpts::default(),
        });
        (host, ops)
    }

    fn only<'a>(ops: &'a Ops, what: &str) -> &'a crate::core_host::Pending<SignOperation> {
        assert_eq!(
            ops.len(),
            1,
            "{what}: {:?}",
            ops.iter().map(|op| &op.operation).collect::<Vec<_>>()
        );
        &ops[0]
    }

    /// What the page was told, over every operation.
    fn answers(ops: &[crate::core_host::Pending<SignOperation>]) -> Vec<Option<String>> {
        use vela_core::app::sign_request::SignResponsePayload;
        ops.iter()
            .filter_map(|op| match &op.operation {
                SignOperation::SendResponse {
                    payload: SignResponsePayload::Ok { result },
                    ..
                } => Some(result.clone()),
                SignOperation::SendResponse { payload, .. } => {
                    unreachable!("the page was refused: {payload:?}")
                }
                _ => None,
            })
            .collect()
    }

    /// Drive one submit to its end the way `sign_and_submit` does: the
    /// ceremony's edges, the op told before the wait (with its lost-reply
    /// facts), the record written, and then `after_receipt_wait`'s verdict.
    /// Everything the page was told, and the record the core asked for.
    fn submit_to_the_end(
        maybe_sent: bool,
        receipt: Option<&str>,
    ) -> (
        Vec<Option<String>>,
        SignRecord,
        Vec<vela_core::app::sign_request::SignPhase>,
    ) {
        let (mut host, ops) = approved_send("rid-1");
        let precheck = only(&ops, "the pre-check").id;
        let mut phases = vec![host.view().phase];
        let ops = host.resolve(precheck, SignShellResult::PreCheck { funding: None });
        let submit = only(&ops, "the submit");
        assert!(matches!(
            submit.operation,
            SignOperation::SignAndSubmit { .. }
        ));
        let submit = submit.id;
        phases.push(host.view().phase);
        for edge in [CeremonyEdge::Started, CeremonyEdge::Signed] {
            host.dispatch(ceremony_event("rid-1", edge));
            phases.push(host.view().phase);
        }
        let ops = host.dispatch(Event::OpSubmitted {
            id: "rid-1".to_owned(),
            user_op_hash: LOCAL_OP.to_owned(),
            now_ms: 5_000.0,
            maybe_sent,
            submit_block: maybe_sent.then_some(48_479_132),
        });
        assert_eq!(host.view().pending_op_maybe_sent, maybe_sent);
        let persist = only(&ops, "the record");
        let SignOperation::PersistRecord { record } = &persist.operation else {
            unreachable!("not a record: {:?}", persist.operation);
        };
        let record = record.clone();
        let mut told = answers(&host.resolve(persist.id, SignShellResult::RecordPersisted));
        let settled = host.resolve(
            submit,
            SignShellResult::Submit {
                outcome: after_receipt_wait(LOCAL_OP.to_owned(), receipt.map(str::to_owned)),
                now_ms: 90_000.0,
            },
        );
        told.extend(answers(&settled));
        assert!(
            !settled
                .iter()
                .any(|op| matches!(op.operation, SignOperation::UpdateRecord { .. })),
            "the tracker alone closes an on-chain record (RA8)"
        );
        (told, record, phases)
    }

    /// Ruling 1 / RA2: the relay's reply was lost. The op is recorded as may
    /// have been sent, with the head it was sent at, and the page gets
    /// exactly ONE answer — the op hash, never 4900 or -32603.
    #[test]
    fn a_mute_relay_is_answered_once_with_the_op_hash() {
        let (told, record, _) = submit_to_the_end(true, None);
        assert_eq!(told, vec![Some(LOCAL_OP.to_owned())]);
        assert!(record.maybe_sent);
        assert_eq!(record.submit_block, Some(48_479_132));
        assert_eq!(record.user_op_hash, LOCAL_OP);
    }

    /// Ruling 9: a receipt that came back reverted inside the wait is still a
    /// receipt — the page gets its tx hash, once.
    #[test]
    fn a_reverted_receipt_is_answered_with_its_tx_hash() {
        let (told, record, _) = submit_to_the_end(false, Some(LANDED_TX));
        assert_eq!(told, vec![Some(LANDED_TX.to_owned())]);
        assert!(!record.maybe_sent);
    }

    /// RA9: "preparing" through the pre-check, "awaiting signature" only
    /// while the ceremony is up, "submitting" once it has signed.
    #[test]
    fn the_phases_follow_the_ceremony_edges() {
        use vela_core::app::sign_request::SignPhase;
        let (_, _, phases) = submit_to_the_end(false, Some(LANDED_TX));
        assert_eq!(
            phases,
            vec![
                SignPhase::Preparing,
                SignPhase::Preparing,
                SignPhase::AwaitingSignature,
                SignPhase::Submitting,
            ]
        );
    }

    /// RB2: the tab that asked went away while the pre-check ran. The late
    /// pre-check answer is dropped by the host (`core_host` rule 3) and
    /// nothing is signed or sent for a page that is gone.
    #[test]
    fn an_aborted_attempt_never_reaches_the_relay() {
        let (mut host, ops) = approved_send("rid-gone");
        let precheck = only(&ops, "the pre-check").id;
        host.dispatch(Event::TransportDropped {
            transport_id: TAB.to_owned(),
        });
        let after = host.resolve(precheck, SignShellResult::PreCheck { funding: None });
        assert!(
            !after
                .iter()
                .any(|op| matches!(op.operation, SignOperation::SignAndSubmit { .. })),
            "a page that is gone gets nothing signed"
        );
    }

    /// RB2, the money rule, past the commitment point: the tab that asked
    /// closed while Touch ID was up. The core keeps an inflight it has
    /// already handed to the submit, so the shell is the one that must stop
    /// — the page's close marks the request's context, the running submit
    /// (the executor's clone of it) sees that before the ceremony and before
    /// the relay POST, and answers `AskerGone`: nothing sent, no answer to a
    /// page that is not there, no record, nothing handed to the tracker.
    #[test]
    fn a_page_gone_during_the_passkey_gets_nothing_sent_answered_or_recorded() {
        let (mut host, ops) = approved_send("rid-left");
        let precheck = only(&ops, "the pre-check").id;
        let ops = host.resolve(precheck, SignShellResult::PreCheck { funding: None });
        let submit = only(&ops, "the submit").id;
        host.dispatch(ceremony_event("rid-left", CeremonyEdge::Started));

        // The column's context, and the copy the running submit holds.
        let ctx = context(Some("http://127.0.0.1:8137"));
        let running = ctx.clone();
        assert!(running.still_asked());
        ctx.asker_left();
        let dropped = host.dispatch(Event::TransportDropped {
            transport_id: TAB.to_owned(),
        });
        assert!(
            answers(&dropped).is_empty(),
            "the page has its 4900 already"
        );
        assert!(
            !running.still_asked(),
            "the submit already running sees the page leave"
        );

        // The executor stops before the ceremony — no event, no relay.
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let outcome = sign_and_submit(
            &running,
            "rid-left",
            100,
            ME,
            "eth_sendTransaction",
            r#"[{"to":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","value":"0x38d7ea4c68000"}]"#,
            None,
            None,
            &crate::resident::Sink::new(tx),
        );
        assert_eq!(outcome, SignSubmitOutcome::AskerGone);
        assert!(rx.try_recv().is_err(), "no ceremony edge, no OpSubmitted");
        // A ceremony or a POST that the check stopped reports the same.
        assert_eq!(
            submit_failure(100, ME, user_op::SubmitFailure::AskerGone),
            SignSubmitOutcome::AskerGone
        );

        let after = host.resolve(
            submit,
            SignShellResult::Submit {
                outcome,
                now_ms: 9_000.0,
            },
        );
        assert!(answers(&after).is_empty(), "nobody is left to answer");
        assert!(
            !after.iter().any(|op| matches!(
                op.operation,
                SignOperation::PersistRecord { .. } | SignOperation::UpdateRecord { .. }
            )),
            "nothing was sent, so nothing is recorded"
        );
        assert!(host.view().tracker_handoff.is_none());
        assert!(host.view().pending_op_hash.is_none());
    }

    /// Past the ceremony, the op signed and hashed: `OpSigned`, and what
    /// the core asked for in return (spec 082 RJ1). The submit's effect id
    /// comes back with it.
    fn signed(id: &str) -> (Host, u64, Ops) {
        let (mut host, ops) = approved_send(id);
        let precheck = only(&ops, "the pre-check").id;
        let ops = host.resolve(precheck, SignShellResult::PreCheck { funding: None });
        let submit = only(&ops, "the submit").id;
        for edge in [CeremonyEdge::Started, CeremonyEdge::Signed] {
            host.dispatch(ceremony_event(id, edge));
        }
        let ops = host.dispatch(Event::OpSigned {
            id: id.to_owned(),
            user_op_hash: LOCAL_OP.to_owned(),
            submit_block: Some(48_487_620),
            now_ms: 5_000.0,
        });
        (host, submit, ops)
    }

    /// Perform one blocking (or immediate) operation the way the host does.
    fn run(operation: &SignOperation, ctx: &SignContext) -> SignShellResult {
        match perform(operation, ctx) {
            SignAnswer::Now(result) => result,
            SignAnswer::Blocking(work) => work(),
            _ => unreachable!("not a record or a clearance: {operation:?}"),
        }
    }

    fn stored_rows() -> Vec<Value> {
        match storage::read_value(TX_KEY) {
            Ok(Some(Value::Array(rows))) => rows,
            _ => Vec::new(),
        }
    }

    /// Spec 082 RJ1 (G34, DX9 — P0): the op is signed and its hash known, the
    /// record is written ahead, and then the process quits (no more events,
    /// no POST's verdict). The store holds exactly one pending dApp row that
    /// may have been sent, under the local hash with the head it was sent
    /// at, and the tracker's relaunch sweep hands it back — so an op that
    /// lands after the quit is found, recorded and followed.
    #[test]
    fn a_quit_after_the_op_is_signed_leaves_one_pending_maybe_sent_row() {
        storage::tests::with_temp_state("sign-dx9", || {
            let ctx = context(Some("http://127.0.0.1:8137"));
            let (host, _, ops) = signed("rid-dx9");
            let persist = ops
                .iter()
                .find(|op| matches!(op.operation, SignOperation::PersistRecord { .. }))
                .unwrap_or_else(|| unreachable!("the record is written ahead"));
            assert!(
                !ops.iter()
                    .any(|op| matches!(op.operation, SignOperation::ClearToPost { .. })),
                "no clearance before the record is on disk"
            );
            let handoff = host
                .view()
                .tracker_handoff
                .unwrap_or_else(|| unreachable!("the tracker is told before the POST"));
            assert!(handoff.maybe_sent && !handoff.admitted);
            assert_eq!(handoff.user_op_hash, LOCAL_OP);
            assert_eq!(
                run(&persist.operation, &ctx),
                SignShellResult::RecordPersisted
            );
            // …and the process quits here.

            let live = crate::executor::tracker::pending_records(&stored_rows());
            assert_eq!(live.len(), 1, "one row, pending");
            assert_eq!(live[0].user_op_hash, LOCAL_OP);
            assert!(live[0].maybe_sent, "it may have been sent");
            assert_eq!(live[0].submit_block, Some(48_487_620));
            assert_eq!(live[0].chain_id, 100);
        });
    }

    /// Spec 082 RJ1 + RA10 (S5): the relay was never reached. The write-ahead
    /// cleared the POST only once the record was on disk; the verdict then
    /// deletes it, tells the tracker to forget it, and the page gets exactly
    /// one -32603 "relay unreachable; nothing was sent" — no Activity row.
    #[test]
    fn a_not_sent_op_leaves_no_row_and_one_error() {
        use vela_core::app::sign_request::{SignResponsePayload, SignTrackerWithdraw};
        storage::tests::with_temp_state("sign-s5", || {
            let ctx = context(Some("http://127.0.0.1:8137"));
            let (mut host, submit, ops) = signed("rid-s5");
            let persist = only(&ops, "the record");
            let SignOperation::PersistRecord { record } = &persist.operation else {
                unreachable!("not a record");
            };
            let record_id = record.record_id.clone();
            ctx.clearance.arm(LOCAL_OP);
            let ops = host.resolve(persist.id, run(&persist.operation, &ctx));
            let clear = only(&ops, "the clearance");
            assert!(matches!(
                &clear.operation,
                SignOperation::ClearToPost { user_op_hash, .. } if user_op_hash == LOCAL_OP
            ));
            assert!(
                !ctx.clearance.wait(Duration::ZERO),
                "not cleared until the core's word is performed"
            );
            ctx.clearance.arm(LOCAL_OP);
            let _ = host.resolve(clear.id, run(&clear.operation, &ctx));
            assert!(
                ctx.clearance.wait(Duration::ZERO),
                "cleared: the POST may go"
            );
            assert_eq!(stored_rows().len(), 1, "on disk before the bytes leave");

            let settled = host.resolve(
                submit,
                SignShellResult::Submit {
                    outcome: submit_failure(100, ME, user_op::SubmitFailure::NotSent),
                    now_ms: 6_000.0,
                },
            );
            let mut told = Vec::new();
            for op in &settled {
                match &op.operation {
                    SignOperation::DeleteRecord { .. } => {
                        let _ = run(&op.operation, &ctx);
                    }
                    SignOperation::SendResponse { payload, .. } => told.push(payload.clone()),
                    _ => {}
                }
            }
            assert!(
                stored_rows().is_empty(),
                "no Activity row once the verdict is in"
            );
            assert_eq!(told.len(), 1, "one answer: {told:?}");
            assert!(matches!(
                &told[0],
                SignResponsePayload::Err { code: -32603, message: Some(message), .. }
                    if message == NOT_SENT_DAPP_DETAIL
            ));
            assert_eq!(
                host.view().tracker_withdraw,
                Some(SignTrackerWithdraw {
                    user_op_hash: LOCAL_OP.to_owned(),
                    record_ids: vec![record_id],
                }),
                "the tracker forgets it"
            );
            assert!(!host.view().failure_refused);
        });
    }

    /// Spec 082 RJ1 (review): the POST is cleared only for an op whose
    /// record is on disk the way the relaunch sweep reads it. `ClearToPost`
    /// follows `RecordPersisted`, which the write answers whether or not the
    /// store took the row — a full disk, or another writer's read-modify-write
    /// landing over it — and a payment with no record behind it is the one
    /// thing the write-ahead exists to prevent.
    #[test]
    fn the_post_is_cleared_only_with_its_record_on_disk() {
        storage::tests::with_temp_state("sign-clear-on-disk", || {
            let ctx = context(Some("http://127.0.0.1:8137"));
            let clear = SignOperation::ClearToPost {
                id: "rid-disk".to_owned(),
                user_op_hash: LOCAL_OP.to_owned(),
            };
            ctx.clearance.arm(LOCAL_OP);
            assert_eq!(run(&clear, &ctx), SignShellResult::Responded);
            assert!(
                !ctx.clearance.wait(Duration::ZERO),
                "nothing on disk: nothing may leave"
            );

            let mut written = record(
                SignRecordKind::DappTx,
                r#"[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x1"}]"#,
            );
            written.user_op_hash = LOCAL_OP.to_owned();
            written.maybe_sent = true;
            persist_record(&written);
            ctx.clearance.arm(LOCAL_OP);
            assert_eq!(run(&clear, &ctx), SignShellResult::Responded);
            assert!(
                ctx.clearance.wait(Duration::ZERO),
                "on disk: the POST may go"
            );
        });
    }

    /// Spec 082 RJ1 (review): the dApp's record is written on a worker thread
    /// while the tracker patches, the Send writes and the incoming scan adds
    /// rows to the same document from others. Each was a read, a change and
    /// a whole-key write with nothing held between the read and the write,
    /// so one writer's stale copy could land over another's row — a
    /// written-ahead record gone after its POST was cleared, the DX9 payment
    /// with no Activity row by another road. Every row written at once is
    /// still there.
    #[test]
    fn records_written_at_once_all_stay() {
        storage::tests::with_temp_state("tx-history-at-once", || {
            let start = Arc::new(std::sync::Barrier::new(9));
            let writers: Vec<_> = (0..8)
                .map(|writer| {
                    let start = Arc::clone(&start);
                    std::thread::spawn(move || {
                        start.wait();
                        for n in 0..6 {
                            let mut row = record(SignRecordKind::DappTx, "[]");
                            row.record_id = format!("dapp-{writer}-{n}-tx");
                            row.user_op_hash = format!("0x{writer}{n}");
                            persist_record(&row);
                        }
                    })
                })
                .collect();
            start.wait();
            for n in 0..6 {
                let hash = format!("0xsend{n}");
                let _ =
                    crate::executor::send::persist_records(&[vela_core::app::send::SendTxRecord {
                        id: hash.clone(),
                        user_op_hash: hash,
                        tx_hash: String::new(),
                        from: ME.to_owned(),
                        to: ME.to_owned(),
                        to_name: None,
                        value: "0.001".to_owned(),
                        symbol: "xDAI".to_owned(),
                        decimals: 18,
                        logo_urls: Vec::new(),
                        chain_id: 100,
                        timestamp_s: 1_757_000_000.0,
                        usd: None,
                        maybe_sent: true,
                        submit_block: None,
                    }]);
            }
            for writer in writers {
                writer
                    .join()
                    .unwrap_or_else(|_| unreachable!("a writer panicked"));
            }
            assert_eq!(stored_rows().len(), 8 * 6 + 6, "no row lost");
        });
    }

    /// Spec 082 RJ3 (G36): a relay that refuses the op at submit is a
    /// refusal — the page gets the core's "refused" sentence once, the sheet
    /// the refusal's words — never a "try again".
    #[test]
    fn a_relay_refusal_is_answered_refused() {
        use vela_core::app::sign_request::SignResponsePayload;
        let refused = submit_failure(
            100,
            ME,
            user_op::SubmitFailure::Refused("UserOperation reverted during simulation".to_owned()),
        );
        assert!(matches!(
            refused,
            SignSubmitOutcome::Failed { refused: true, .. }
        ));
        for kept in [
            user_op::SubmitFailure::RelayerUnavailable,
            user_op::SubmitFailure::NotSent,
            user_op::SubmitFailure::Other("the passkey ceremony failed".to_owned()),
        ] {
            assert!(
                matches!(
                    submit_failure(100, ME, kept.clone()),
                    SignSubmitOutcome::Failed { refused: false, .. }
                ),
                "{kept:?} is not a refusal"
            );
        }
        assert!(matches!(
            submit_failure(100, ME, user_op::SubmitFailure::BundlerUnderfunded),
            SignSubmitOutcome::Underfunded { .. }
        ));

        storage::tests::with_temp_state("sign-refused", || {
            let ctx = context(Some("http://127.0.0.1:8137"));
            let (mut host, submit, ops) = signed("rid-refused");
            let persist = only(&ops, "the record");
            let _ = host.resolve(persist.id, run(&persist.operation, &ctx));
            let settled = host.resolve(
                submit,
                SignShellResult::Submit {
                    outcome: refused,
                    now_ms: 6_000.0,
                },
            );
            let told: Vec<_> = settled
                .iter()
                .filter_map(|op| match &op.operation {
                    SignOperation::SendResponse { payload, .. } => Some(payload.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(told.len(), 1);
            assert!(matches!(
                &told[0],
                SignResponsePayload::Err { code: -32603, message: Some(message), .. }
                    if message == vela_core::user_op::REFUSED_DAPP_DETAIL
            ));
            assert!(host.view().failure_refused, "the sheet says refused");
        });
    }

    /// Spec 082 RJ1: the relay took the written-ahead op — its row stays
    /// pending (only the tracker closes it) and no longer "may have been
    /// sent"; a proven-not-sent one is deleted, and only that one.
    #[test]
    fn admitted_clears_maybe_sent_and_delete_removes_only_its_row() {
        use vela_core::app::sign_request::SignRecordClose;
        storage::tests::with_temp_state("sign-admitted", || {
            let mut kept = record(
                SignRecordKind::DappTx,
                r#"[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x1"}]"#,
            );
            kept.maybe_sent = true;
            let mut gone = kept.clone();
            gone.record_id = "dapp-1757000000001-tx".to_owned();
            persist_record(&kept);
            persist_record(&gone);
            let ctx = context(None);
            let _ = run(
                &SignOperation::UpdateRecord {
                    record_id: kept.record_id.clone(),
                    close: SignRecordClose::Admitted,
                },
                &ctx,
            );
            let _ = run(
                &SignOperation::DeleteRecord {
                    record_id: gone.record_id.clone(),
                },
                &ctx,
            );
            let rows = stored_rows();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["id"], kept.record_id.as_str());
            assert_eq!(rows[0]["status"], "pending");
            assert_eq!(rows[0]["maybeSent"], false);
        });
    }

    /// Spec 082 RG3 (T072): every background record write — the pending
    /// record, its close — pokes the feed once, on the tracker's next tick.
    #[test]
    fn each_record_write_pokes_the_feed_once() {
        use vela_core::app::sign_request::{SignRecordClose, SignRecordKind};
        storage::tests::with_temp_state("sign-poke", || {
            let run = |operation: SignOperation| match perform(&operation, &context(None)) {
                SignAnswer::Blocking(work) => {
                    let before = crate::executor::tracker::records_waiting();
                    let _ = work();
                    crate::executor::tracker::records_waiting() - before
                }
                _ => unreachable!("a record write is blocking work"),
            };
            let record = record(
                SignRecordKind::DappTx,
                r#"[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x1"}]"#,
            );
            let id = record.record_id.clone();
            assert_eq!(run(SignOperation::PersistRecord { record }), 1);
            assert_eq!(
                run(SignOperation::UpdateRecord {
                    record_id: id,
                    close: SignRecordClose::Confirmed {
                        tx_hash: "0xtx".to_owned(),
                    },
                }),
                1
            );
        });
    }

    /// A dApp transaction lands in the store the feed and the tracker read.
    ///
    /// This was a no-op stub until spec 032 phase 29 — every dApp transaction
    /// this wallet submitted left no trace at all.
    #[test]
    fn a_dapp_transaction_is_written_where_every_client_reads_it() {
        storage::tests::with_temp_state("sign-record", || {
            let record = record(
                SignRecordKind::DappTx,
                r#"[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x2386f26fc10000"}]"#,
            );
            persist_record(&record);

            let rows = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing was written"),
            };
            let row = rows.first().unwrap_or_else(|| unreachable!("no row"));
            assert_eq!(row.get("type").and_then(Value::as_str), Some("dapp_tx"));
            assert_eq!(row.get("status").and_then(Value::as_str), Some("pending"));
            assert_eq!(
                row.get("to").and_then(Value::as_str),
                Some("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
                "the recipient comes from the params the wallet is about to sign"
            );
            assert_eq!(
                row.get("value").and_then(Value::as_str),
                Some("0x2386f26fc10000")
            );
            // The chain table's own spelling — "xDAI", not a shell's guess at it.
            assert_eq!(row.get("symbol").and_then(Value::as_str), Some("xDAI"));
            assert_eq!(row.get("chainId").and_then(Value::as_u64), Some(100));
            assert_eq!(
                row.get("timestamp").and_then(Value::as_i64),
                Some(1_757_000_000)
            );

            // Closing PATCHES the row it opened — one request, one row, ever.
            update_record(
                &record.record_id,
                &SignRecordClose::Confirmed {
                    tx_hash: "0xdeadbeef".to_owned(),
                },
            );
            let rows = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("the store vanished"),
            };
            assert_eq!(rows.len(), 1, "closing wrote a second row");
            assert_eq!(
                rows[0].get("status").and_then(Value::as_str),
                Some("confirmed")
            );
            assert_eq!(
                rows[0].get("txHash").and_then(Value::as_str),
                Some("0xdeadbeef")
            );
        });
    }

    /// A signature moves nothing, and its row must not claim otherwise.
    #[test]
    fn a_signature_row_carries_no_amount() {
        storage::tests::with_temp_state("sign-record-msg", || {
            let mut record = record(SignRecordKind::SignMessage, r#"["0xdeadbeef","0xabc"]"#);
            record.record_id = "dapp-1757000000000-msg".to_owned();
            persist_record(&record);
            let rows = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            let row = &rows[0];
            assert_eq!(
                row.get("type").and_then(Value::as_str),
                Some("sign_message")
            );
            assert_eq!(row.get("value").and_then(Value::as_str), Some("0"));
            assert_eq!(row.get("symbol").and_then(Value::as_str), Some(""));
            assert_eq!(row.get("to").and_then(Value::as_str), Some(""));
        });
    }

    /// The stored payload is clipped on a character boundary.
    ///
    /// A page chooses the length of what it asks to sign; this file chooses
    /// how much of it lives on the disk forever.
    #[test]
    fn an_enormous_payload_is_clipped_and_says_so() {
        let long = format!("[\"{}\"]", "字".repeat(9000));
        let (kept, truncated) = clip(&long);
        assert!(truncated, "a 27 KB payload was stored whole");
        assert!(kept.len() <= 8 * 1024);
        assert!(
            std::str::from_utf8(kept.as_bytes()).is_ok(),
            "the clip cut a multibyte character in half"
        );
        let (kept, truncated) = clip("[]");
        assert!(!truncated);
        assert_eq!(kept, "[]");
    }

    /// A plain dApp transaction.
    #[test]
    fn a_transaction_becomes_one_call() {
        let params =
            r#"[{"from":"0xaaa","to":"0xbbb","value":"0xde0b6b3a7640000","data":"0xabcd"}]"#;
        let calls = calls_of("eth_sendTransaction", params)
            .unwrap_or_else(|| unreachable!("a transaction reads"));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].to, "0xbbb");
        // Hex on the wire, DECIMAL to the core — one ether, not "0xde0b…".
        assert_eq!(calls[0].value, "1000000000000000000");
        assert_eq!(calls[0].data, "0xabcd");
    }

    /// Most contract calls carry no value and no wallet should refuse them.
    /// A missing value is zero; a missing data is `0x`.
    #[test]
    fn the_absent_fields_are_zero_and_empty_rather_than_a_refusal() {
        let calls = calls_of("eth_sendTransaction", r#"[{"to":"0xbbb"}]"#)
            .unwrap_or_else(|| unreachable!("a bare call reads"));
        assert_eq!(calls[0].value, "0");
        assert_eq!(calls[0].data, "0x");
    }

    /// `"0x"` is ZERO, and a value nobody can read is not.
    ///
    /// The web shell took `"0x"` for unreadable (`BigInt("0x")` throws) and
    /// lost the whole request's fee AND its speed control, silently, on a
    /// Uniswap swap (B-4, owner 2026-09-23). This shell parsed it correctly by
    /// luck — and priced an unreadable value as zero, which is the other half
    /// of the same mistake: a wrong fee beside a real transaction.
    #[test]
    fn a_bare_0x_is_zero_and_an_unreadable_amount_is_refused() {
        let zero = calls_of("eth_sendTransaction", r#"[{"to":"0xbbb","value":"0x"}]"#)
            .unwrap_or_else(|| unreachable!("`0x` is zero, not unreadable"));
        assert_eq!(zero[0].value, "0");

        // Absent, empty and null are all zero too.
        for params in [
            r#"[{"to":"0xbbb"}]"#,
            r#"[{"to":"0xbbb","value":""}]"#,
            r#"[{"to":"0xbbb","value":null}]"#,
        ] {
            let calls = calls_of("eth_sendTransaction", params)
                .unwrap_or_else(|| unreachable!("{params} reads as zero"));
            assert_eq!(calls[0].value, "0", "{params}");
        }

        // And what is NOT a number draws no fee rather than a made-up one.
        for params in [
            r#"[{"to":"0xbbb","value":"soon"}]"#,
            r#"[{"to":"0xbbb","value":"0xzz"}]"#,
            r#"[{"to":"0xbbb","value":{}}]"#,
        ] {
            assert!(
                calls_of("eth_sendTransaction", params).is_none(),
                "{params} was priced anyway"
            );
        }
    }

    /// EIP-5792: one entry carrying many calls, and they stay in order —
    /// a batch reordered is a different transaction.
    #[test]
    fn a_batch_keeps_its_calls_and_their_order() {
        let params = r#"[{"calls":[{"to":"0x1","value":"0x1"},{"to":"0x2"},{"to":"0x3"}]}]"#;
        let calls =
            calls_of("wallet_sendCalls", params).unwrap_or_else(|| unreachable!("a batch reads"));
        assert_eq!(
            calls.iter().map(|c| c.to.as_str()).collect::<Vec<_>>(),
            ["0x1", "0x2", "0x3"]
        );
        assert_eq!(calls[0].value, "1");
    }

    /// Nothing to send is not an empty batch.
    ///
    /// An empty `Vec<FeeCall>` would assemble into a user operation that
    /// submits nothing and still charges a fee, so a request this cannot read
    /// must refuse rather than produce one.
    #[test]
    fn an_unreadable_request_refuses_rather_than_submitting_nothing() {
        assert!(calls_of("eth_sendTransaction", "[]").is_none(), "no params");
        assert!(calls_of("eth_sendTransaction", "not json").is_none());
        assert!(
            calls_of("eth_sendTransaction", r#"[{"value":"0x1"}]"#).is_none(),
            "a call with no `to` is not a call"
        );
        assert!(
            calls_of("wallet_sendCalls", r#"[{"calls":[]}]"#).is_none(),
            "an empty batch would submit nothing and still cost a fee"
        );
        assert!(
            calls_of("wallet_sendCalls", r#"[{"calls":[{"value":"0x1"}]}]"#).is_none(),
            "…and so would a batch whose every call is unreadable"
        );
    }
}
