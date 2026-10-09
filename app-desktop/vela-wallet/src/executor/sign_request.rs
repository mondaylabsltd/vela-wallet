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
//!   one looks its transaction up forever. Only an operation that EXECUTED
//!   is answered with its hash (083): one that reverted is reported as such,
//!   and the page gets an error — the bundle's own status is `0x1` either
//!   way, which is how Uniswap and this column both called a swap done that
//!   moved nothing. And the page waits for the hash — past the 90 s that
//!   used to end in the op hash — until the core's cap, which ends in an
//!   error, never in a hash nobody can find ([`crate::executor::landing`]).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json::{Value, json};

use vela_core::app::fee_policy::FeeCall;
use vela_core::app::sign_request::{
    Event, SignFundingNeeded, SignOperation, SignRecord, SignShellResult, SignSubmitOutcome,
};
use vela_core::app::{Account, Assertion, KeyMethod};
use vela_core::signing_venue::{KeyRoute, VenueBlock};
use vela_core::user_op::{NOT_SENT_DAPP_DETAIL, WalletKey};

use crate::diag::{short, vlog};
use crate::executor::landing::{self, Landing};
use crate::executor::passkey::{self, Ceremony, PasskeyFailure};
use crate::executor::trusted_signer::{self, Ask};
use crate::executor::user_op::{CeremonyEdge, SignFn, Signer};
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

/// A phone's ceremony that came back unsigned with the request still open,
/// for a reason another try can fix (083). The column names which, and
/// offers the approval again; its close is the person's refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PhoneStop {
    /// The QR ran its whole scan window with no phone (W19).
    ScanExpired,
    /// The phone scanned, and its relay tunnel or the encrypted channel
    /// through it never came up (H4).
    LinkFailed,
    /// The phone was connected and asked, and the connection dropped before
    /// its answer arrived (H4 review) — whatever it did, nothing signed
    /// reached this desktop, so nothing went anywhere.
    Dropped,
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
    /// Spec 102: where this account reviews and signs, as
    /// [`crate::executor::send::SendContext`] has it — its trusted page (or
    /// none), why its keys are out of reach (or not), and the key route.
    pub venue_page: Option<String>,
    pub venue_block: Option<VenueBlock>,
    pub key_route: Option<KeyRoute>,
    pub key_name: Option<String>,
    pub ceremony: Ceremony,
    /// Raised the instant the passkey prompt opens, so the host can tell the
    /// core the ceremony started rather than guessing from elapsed time.
    pub signing_started: Arc<AtomicBool>,
    /// Raised once that prompt has signed (083 W11): the core's `Submitting`
    /// spans building the operation, the prompt and the submission, and only
    /// these two flags say which of them is running. A prompt that said no
    /// raises nothing — the column's close reads this as "the phone signed,
    /// the operation goes on", and must not read it after a no (083 review).
    pub signature_done: Arc<AtomicBool>,
    /// Why the phone's ceremony came back unsigned, when it was the phone's
    /// doing and another try can help (083 W19, H4) — told to the column,
    /// which says which; the page is told nothing.
    pub phone_stop: Arc<Mutex<Option<PhoneStop>>>,
    /// The column was closed while this approval's signature was still to
    /// come (083 review): a prompt not yet open never opens — the ceremony
    /// comes back unsigned at once, and the column's close becomes a refusal
    /// instead of a QR or a Windows Hello dialog nobody will see.
    pub abandoned: Arc<AtomicBool>,
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

    /// A pipeline starting over — an approval, a retry, a top-up's Continue —
    /// starts with no prompt asked yet (083).
    pub fn prompt_reset(&self) {
        for flag in [&self.signing_started, &self.signature_done, &self.abandoned] {
            flag.store(false, Ordering::SeqCst);
        }
        self.take_phone_stop();
    }

    /// Why the last prompt came back unsigned, when it was the phone's doing
    /// and another try can help (083): nobody scanned its QR in time, the
    /// connection to it never came up, or it dropped once the phone was
    /// asked. Taken once.
    pub fn take_phone_stop(&self) -> Option<PhoneStop> {
        self.phone_stop.lock().ok().and_then(|mut stop| stop.take())
    }

    /// The column is told why the prompt came back unsigned.
    fn tell_phone_stop(&self, stop: PhoneStop) {
        if let Ok(mut slot) = self.phone_stop.lock() {
            *slot = Some(stop);
        }
    }

    /// Point the trusted page's channel at this account's venue — see
    /// [`crate::executor::send::SendContext::follow_venue`].
    pub fn follow_venue(&self) {
        self.trusted_signer.choose(self.venue_page.clone());
    }

    /// The hand-off card's facts — see
    /// [`crate::executor::send::SendContext::handoff`].
    #[must_use]
    pub fn handoff(&self) -> Option<crate::executor::send::Handoff> {
        crate::executor::send::handoff_of(
            self.venue_page.as_deref(),
            self.venue_block.as_ref(),
            self.key_name.as_deref(),
            self.key_method,
        )
    }

    /// Who signs this request — the one rule
    /// ([`crate::executor::send::signer_for`]).
    fn signer<'a>(&'a self, ask: &'a Ask, native: SignFn<'a>) -> Signer<'a> {
        crate::executor::send::signer_for(
            self.trusted_signer.chosen(),
            self.venue_block.as_ref(),
            self.key_route.as_ref(),
            ask,
            &self.trusted_signer,
            native,
        )
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
            venue_page: send.venue_page,
            venue_block: send.venue_block,
            key_route: send.key_route,
            key_name: send.key_name,
            ceremony: send.ceremony,
            signing_started: send.signing_started,
            signature_done: Arc::new(AtomicBool::new(false)),
            phone_stop: Arc::default(),
            abandoned: Arc::new(AtomicBool::new(false)),
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

/// How long a dApp's transaction waits for its operation to land before the
/// page is told it has not — the core's number
/// ([`vela_core::app::sign_request::PAGE_WAIT_CAP_MS`], ten minutes, where
/// 079 US1 answered the userOpHash at 90 s).
///
/// A dApp's promise must SETTLE, and with the truth: waiting forever is the
/// failure mode that looks like success from inside the wallet and like a
/// hang from inside the site; answering a hash no node knows is the same hang
/// with extra steps (083, Uniswap).
const LANDING_CAP: Duration =
    Duration::from_millis(vela_core::app::sign_request::PAGE_WAIT_CAP_MS as u64);

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
            signer: None,
        };
    };

    let mut sign = |challenge: &[u8]| {
        around_prompt(ctx, || {
            let (credential, method) = ctx.route();
            passkey::assert(challenge, credential.as_deref(), method, &ctx.ceremony)
        })
    };
    let ask = ctx.ask(method, params_json);
    let signer = ctx.signer(&ask, &mut sign);
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

    // 083: the page waits for the operation to land, up to the core's cap —
    // the transaction hash, the revert, or "not confirmed yet". The core's
    // answer from the tracker (spec 082 RJ4) ends the wait at once.
    let answered = || ctx.answered();
    let landing = landing::await_landing_until(&user_op_hash, chain_id, LANDING_CAP, &answered);
    if matches!(landing, Some(Landing::Landed(_) | Landing::Reverted(_))) {
        // Its nonce is spent: the account's next operation builds on it.
        user_op::note_landed(&user_op_hash);
    }
    if ctx.answered() {
        vlog!(
            "dapp",
            "op={} landing wait ended: answered from the tracker",
            short(&user_op_hash)
        );
    } else {
        vlog!(
            "dapp",
            "op={} landing: {}",
            short(&user_op_hash),
            match &landing {
                Some(Landing::Landed(_)) => "landed",
                Some(Landing::Reverted(_)) => "reverted",
                Some(Landing::Refused) => "refused by the relay",
                None => "not confirmed by the cap",
            }
        );
    }
    after_landing(user_op_hash, landing)
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
            signer: None,
        };
    };
    let mut sign = |challenge: &[u8]| {
        around_prompt(ctx, || {
            let (credential, method) = ctx.route();
            passkey::assert(challenge, credential.as_deref(), method, &ctx.ceremony)
        })
    };
    let ask = ctx.ask(method, params_json);
    let signer = ctx.signer(&ask, &mut sign);
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

/// One passkey prompt, as the column follows it (083).
///
/// W11: `signing_started` goes up as the prompt opens and `signature_done`
/// once it has signed, so "waiting for biometric" is said only while
/// something is asking — the owner saw it for seconds over the funding check
/// and the nonce read, with no prompt anywhere.
///
/// W19: a phone's QR that ran its whole scan window with nobody scanning is
/// a cancellation, not a failure. As a failure the core answered the page
/// -32603 with the scan's own English; as a cancellation it keeps the request
/// open and sends nothing, and the column offers the scan again.
///
/// H4: so is a phone whose connection never came up. As a failure the page
/// got the transport's words and the column a transaction's failure — with
/// only 完成, where another try (a new QR) is what can work. And (review) so
/// is one that dropped once the phone was asked: the page got "The security
/// key stopped responding" or the OS's socket error, though nothing signed
/// had reached this desktop — and the desktop submits only what it holds,
/// so another try cannot send anything twice.
///
/// Review: a column closed before the prompt opened asks nothing — the
/// ceremony comes back unsigned at once, and the column's waiting close
/// refuses. Otherwise the QR went up in a column nobody drew, and the core,
/// which had only stopped watching, heard the scan run out 90 s later.
pub(crate) fn around_prompt(
    ctx: &SignContext,
    prompt: impl FnOnce() -> Result<Assertion, PasskeyFailure>,
) -> Result<Assertion, PasskeyFailure> {
    if ctx.abandoned.load(Ordering::SeqCst) {
        return Err(PasskeyFailure::cancelled());
    }
    ctx.signing_started.store(true, Ordering::SeqCst);
    let answer = prompt();
    if answer.is_ok() {
        ctx.signature_done.store(true, Ordering::SeqCst);
    }
    answer.map_err(|failure| match failure.phone_stop() {
        Some(stop) => {
            ctx.tell_phone_stop(stop);
            PasskeyFailure::cancelled()
        }
        None => failure,
    })
}

/// What the site asked to sign, before the Safe's wrap — the core's one rule
/// (`vela_core::sign_message`), which the Trusted Signer's page shares.
pub fn message_hash(method: &str, params_json: &str) -> Option<Vec<u8>> {
    vela_core::sign_message::original_hash(method, params_json)
}

/// What the landing wait means for the core — one outcome, and never a hash
/// that is not the answer (083).
///
/// - Executed: `Succeeded` with the TX hash, what `eth_sendTransaction`
///   resolves to. A `wallet_sendCalls` is answered with its id instead —
///   the op hash, the moment the relay took it (spec 097 E, the core's rule):
///   this result then answers nothing.
/// - Reverted: `Reverted` — the core answers the page an error and closes the
///   record failed. The hash stays out of the page's answer: the bundle's
///   status is `0x1`, and a site reading it calls the operation done.
/// - Refused by the relay after it accepted it: a failure, as the tracker
///   rules it (`rejected` is final there too).
/// - Nothing by the cap: `NotConfirmed` — the page is told so, and the record
///   stays pending for the tracker (issue 262: a late receipt is not a
///   confirmation, and a timeout is not a failure). Never `ReceiptPending`,
///   whose answer is the op hash.
pub fn after_landing(user_op_hash: String, landing: Option<Landing>) -> SignSubmitOutcome {
    match landing {
        Some(Landing::Landed(tx_hash)) => SignSubmitOutcome::Succeeded { result: tx_hash },
        Some(Landing::Reverted(tx_hash)) => SignSubmitOutcome::Reverted {
            user_op_hash,
            tx_hash,
        },
        Some(Landing::Refused) => SignSubmitOutcome::Failed {
            message: "The relay refused the transaction after accepting it; it was not sent"
                .to_owned(),
            // The relay's refusal (spec 082 RJ3): no "try again".
            refused: true,
            signer: None,
        },
        None => SignSubmitOutcome::NotConfirmed { user_op_hash },
    }
}

/// The calls a request is asking for — the core's one reading
/// (`tx_request::calls_of`, spec 096 F1): every call or none, value in
/// DECIMAL wei.
///
/// The params are FINAL by the time they reach here (the core's invariant ⑨
/// caps them, and its arrival made every `value` canonical), so this only
/// reads them.
///
/// Shared with the host, which prices the SAME calls it will later submit —
/// two readings of one params array is how a quote ends up describing a
/// different transaction than the one that gets signed. This shell used to
/// read decimal text as decimal where every other shell read it as hex, and
/// to drop a batch leg it could not read and send the rest.
pub fn calls_of(method: &str, params_json: &str) -> Option<Vec<FeeCall>> {
    let calls = vela_core::tx_request::calls_of(method, params_json)?;
    Some(
        calls
            .into_iter()
            .map(|call| FeeCall {
                to: call.to,
                value: call.value,
                data: call.data,
            })
            .collect(),
    )
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
            signer: None,
        },
        // Nothing left the device (spec 082 RA10): the dApp's -32603 carries
        // the core's fixed sentence, never the pool's "all endpoints failed".
        user_op::SubmitFailure::NotSent => SignSubmitOutcome::Failed {
            message: NOT_SENT_DAPP_DETAIL.to_owned(),
            refused: false,
            signer: None,
        },
        user_op::SubmitFailure::Other(message) => SignSubmitOutcome::Failed {
            message,
            refused: false,
            signer: None,
        },
        // Spec 099 R8: the passkey failed, and the core is told how.
        user_op::SubmitFailure::Signer { kind, message } => SignSubmitOutcome::Failed {
            message,
            refused: false,
            signer: Some(kind),
        },
        // The relay refused it (spec 082 RJ3): the page is answered the
        // core's "refused" sentence, and the sheet never says "try again".
        user_op::SubmitFailure::Refused(message) => SignSubmitOutcome::Failed {
            message,
            refused: true,
            signer: None,
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
            // Only a single transaction's `to` and `value` are the ones it
            // submitted. A batch (`wallet_sendCalls`) submits its `calls`, so
            // a top-level `to` or `value` beside them is whatever the page
            // wrote, which the sheet never showed: kept, it drew a recipient
            // and an amount that never moved (083 H2 review). A batch row has
            // no single recipient and no single figure.
            let call = (record.method == "eth_sendTransaction")
                .then(|| first_param(&record.params_json))
                .flatten();
            (
                "dapp_tx",
                call.as_ref()
                    .and_then(|tx| tx.get("to"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                stored_value(call.as_ref().and_then(|tx| tx.get("value"))),
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
    let mut row = json!({
        "id": record.record_id,
        "userOpHash": record.user_op_hash,
        // A transaction's hash (or a batch's id, which the tracker follows);
        // empty for a signature, which the core keeps off the disk (spec 093).
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
        // The origin itself, beside the name: Activity names the site from
        // this one only (083 H2 review).
        "dappUrl": record.dapp_url,
        // What was signed, kept for the detail's technical section — as the
        // core cut it (spec 093: 8 KB, string values clipped, JSON kept), the
        // one cut every client stores. A page chooses how long a request is;
        // the core decides how much of it lives on the disk.
        "signedRequest": record.stored_request,
        "requestTruncated": record.request_truncated,
        // Spec 082 T181: a may-have-been-sent op and the head read before its
        // first POST travel with the record, so a relaunch hands the tracker
        // the same op it was following (`tracker::pending_records`).
        "maybeSent": record.maybe_sent,
        "submitBlock": record.submit_block,
    });
    if let Some(intent) = &record.intent {
        row["intent"] = json!(intent);
    }
    // What the request was, as Activity states it (spec 093) — the core's
    // summary, read once from the WHOLE request at approve time, kept
    // verbatim and handed back to the feed untouched.
    if let Some(summary) = record
        .summary
        .as_ref()
        .and_then(|summary| serde_json::to_value(summary).ok())
    {
        row["dappSummary"] = summary;
    }
    // What the sheet's simulation showed when the person approved (083 F1),
    // which Activity reads the row's figure from. The core keeps these for a
    // transaction only, and only from the approve.
    if let Some(changes) = record.balance_changes.as_deref().filter(|c| !c.is_empty()) {
        row["assetChanges"] = stored_changes(changes, record.chain_id);
    }
    // Whether it called a contract (083 F3), decided here from the WHOLE
    // final request by the reading the submit path uses (`calls_of`) — not
    // later from `signedRequest`, which is clipped (a page controls its
    // length, so it would control the label) and which Activity cannot
    // read by method (a page-written `calls` beside a single transaction
    // would decide it).
    if let Some(calldata) = (record.kind == SignRecordKind::DappTx)
        .then(|| calls_of(&record.method, &record.params_json))
        .flatten()
        .map(|calls| calls.iter().any(|call| carries_calldata(&call.data)))
    {
        row["calldata"] = json!(calldata);
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

/// The call's value as the record keeps it: a `0x` quantity, read by the
/// core's rule the submit path read it with (`tx_request::value_wei`, spec
/// 096 F1; 083 H2 review). Unreadable is `"0x0"` as absent is: the core
/// refuses a request whose value it cannot read at arrival, so no such
/// transaction reaches a record.
fn stored_value(value: Option<&Value>) -> String {
    vela_core::tx_request::value_wei(value)
        .and_then(|wei| wei.parse::<u128>().ok())
        .map_or_else(|| "0x0".to_owned(), |wei| format!("{wei:#x}"))
}

/// The sheet's balance changes as the store keeps them (083 F1), in the
/// bytes the web writes the same thing (`assetChanges`, its
/// `StoredAssetSim`): each line an `AssetChange` — a native line named by the
/// chain's coin at 18 decimals, a judged token with its symbol and decimals
/// (and `trusted: true` when it is in the set the wallet trusts), an
/// unverified one with neither and `unverified: true`. No `ok`: this
/// shell's simulation gives no revert verdict, so the row claims none.
/// `activity_feed::stored_changes` reads it back.
fn stored_changes(
    changes: &[vela_core::app::token_trust::TrustSimJudgment],
    chain_id: u32,
) -> Value {
    use vela_core::app::token_trust::TrustSimJudgment as J;
    let lines: Vec<Value> = changes
        .iter()
        .map(|change| match change {
            J::Native { delta } => json!({
                "kind": "native",
                "delta": delta,
                "symbol": native_symbol(chain_id),
                "decimals": 18,
            }),
            J::Erc20Trusted {
                token,
                delta,
                symbol,
                decimals,
                in_trusted_set,
            } => {
                let mut line = json!({
                    "kind": "erc20",
                    "token": token,
                    "delta": delta,
                    "symbol": symbol,
                    "decimals": decimals,
                });
                // The judgment's word that the wallet already trusts this
                // token, which only then can be the row's figure (083 F1
                // review); absent, the line is drawn in the detail only.
                if *in_trusted_set {
                    line["trusted"] = json!(true);
                }
                line
            }
            J::Erc20Unverified { token, delta } => {
                let mut line = json!({ "kind": "erc20", "delta": delta, "unverified": true });
                if let Some(token) = token {
                    line["token"] = json!(token);
                }
                line
            }
        })
        .collect();
    json!({ "engine": "rpc", "changes": lines })
}

/// Calldata as the submit path sends it: anything but nothing or a bare `0x`.
pub(crate) fn carries_calldata(data: &str) -> bool {
    let data = data.trim();
    !data.is_empty() && data != "0x"
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

    use vela_core::app::FailureKind;
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
            dapp_url: "https://app.uniswap.org".to_owned(),
            intent: Some("Swap".to_owned()),
            maybe_sent: false,
            submit_block: None,
            balance_changes: None,
            summary: None,
            stored_request: String::new(),
            request_truncated: false,
        }
    }

    /// A record as the signing core hands it over (spec 093): its summary and
    /// its stored request are the core's own, read from its `method`,
    /// `params_json` and `dapp_url` — so every test here writes what a real
    /// approve writes. It shadows the executor's `persist_record` for this
    /// module only; a field a test set itself is kept.
    fn persist_record(record: &SignRecord) {
        let mut record = record.clone();
        if record.summary.is_none() {
            let params = serde_json::from_str(&record.params_json).unwrap_or(Value::Null);
            record.summary = Some(vela_core::app::dapp_activity::summarize(
                &record.method,
                &params,
                record.chain_id,
                &record.dapp_url,
                None,
            ));
        }
        if record.stored_request.is_empty() {
            (record.stored_request, record.request_truncated) =
                vela_core::app::dapp_activity::stored_request(&record.params_json);
        }
        super::persist_record(&record);
    }

    fn context(site: Option<&str>) -> SignContext {
        context_signed_in(site, None)
    }

    fn context_signed_in(
        site: Option<&str>,
        sign_in_key: Option<vela_core::app::SignInKey>,
    ) -> SignContext {
        context_on(
            site,
            sign_in_key,
            vela_core::signing_venue::SigningVenue::InVela,
        )
    }

    fn context_on(
        site: Option<&str>,
        sign_in_key: Option<vela_core::app::SignInKey>,
        signing_venue: vela_core::signing_venue::SigningVenue,
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
            sign_in_key,
            signing_domain: vela_core::signing_venue::APP_DOMAIN.to_owned(),
            signing_venue,
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
            }),
        );
        assert_eq!(ctx.route(), (Some("cred0".to_owned()), KeyMethod::Hybrid));
        ctx.follow_venue();
        assert_eq!(ctx.trusted_signer.chosen(), None);

        // Spec 102 R4: the account's venue is the official page — the site's
        // request goes there, told the sign-in key (R5).
        let through_page = context_on(
            None,
            Some(vela_core::app::SignInKey {
                credential_id: "cred0".to_owned(),
                method: KeyMethod::Platform,
                transports: String::new(),
            }),
            vela_core::signing_venue::SigningVenue::official(),
        );
        through_page.follow_venue();
        assert_eq!(
            through_page.trusted_signer.chosen().as_deref(),
            Some("https://sign.getvela.app/")
        );
        assert_eq!(
            through_page
                .key_route
                .as_ref()
                .map(|route| route.method.as_str()),
            Some("platform")
        );

        // A record from before the sign-in key: the first key, as always.
        let legacy = context(None);
        assert_eq!(
            legacy.route(),
            (Some("cred0".to_owned()), KeyMethod::Platform)
        );
    }

    /// 083 W11: the prompt is marked open while it asks, and signed only
    /// once it signed — a no is not a signature (083 review: the column's
    /// close reads "signed" as "the operation goes on", and a close in the
    /// breath after a cancelled Windows Hello then answered nothing) — and a
    /// new pipeline starts with neither.
    #[test]
    fn around_prompt_marks_the_prompt_then_the_signature() {
        let ctx = context(Some("https://app.uniswap.org"));
        let answer = around_prompt(&ctx, || {
            assert!(ctx.signing_started.load(Ordering::SeqCst), "asking now");
            assert!(
                !ctx.signature_done.load(Ordering::SeqCst),
                "not answered yet"
            );
            Err(PasskeyFailure::cancelled())
        });
        assert_eq!(
            answer.err().map(|failure| failure.kind),
            Some(FailureKind::Cancelled)
        );
        assert!(
            !ctx.signature_done.load(Ordering::SeqCst),
            "a no is not a signature"
        );
        assert_eq!(ctx.take_phone_stop(), None);

        let signed = around_prompt(&ctx, || {
            Ok(Assertion {
                credential_id: "cred0".to_owned(),
                signature_der_hex: String::new(),
                authenticator_data_hex: String::new(),
                client_data_json_hex: String::new(),
                user_id_hex: None,
                authenticator_attachment: String::new(),
                signer_origin: None,
            })
        });
        assert!(signed.is_ok());
        assert!(ctx.signature_done.load(Ordering::SeqCst), "signed");

        ctx.prompt_reset();
        assert!(!ctx.signing_started.load(Ordering::SeqCst));
        assert!(!ctx.signature_done.load(Ordering::SeqCst));
    }

    /// 083 W19: a QR nobody scanned in the whole window is not an answer to
    /// the page. It reaches the core as the passkey dismissed — which keeps
    /// the request open and sends nothing while its form is up (one already
    /// closed is refused: vela-core's `app_sign_request.rs`,
    /// `a_close_then_a_cancelled_prompt_answers_4001_once`) — and the column
    /// is told, so it can say what happened and offer the scan again. Any
    /// other failure is still the failure it was.
    #[test]
    fn scan_timeout_is_not_an_answer() {
        let ctx = context(Some("https://app.uniswap.org"));
        let ran_out = around_prompt(&ctx, || {
            Err(PasskeyFailure::other(crate::ctap::cable::NO_ADVERT))
        })
        .err()
        .unwrap_or_else(|| unreachable!("no phone, no assertion"));
        assert_eq!(ran_out.kind, FailureKind::Cancelled);
        assert_eq!(ran_out.message, None, "no scan English for the page");
        assert_eq!(
            ctx.take_phone_stop(),
            Some(PhoneStop::ScanExpired),
            "the column is told"
        );
        assert_eq!(
            submit_failure(100, "0xabc", user_op::SubmitFailure::PasskeyCancelled),
            SignSubmitOutcome::PasskeyCancelled,
            "…which the core answers with nothing while the form is up"
        );

        ctx.prompt_reset();
        let broken = around_prompt(&ctx, || {
            Err(PasskeyFailure::other(
                crate::ctap::cable::HybridError::BadAdvert.to_string(),
            ))
        })
        .err()
        .unwrap_or_else(|| unreachable!("a failure"));
        assert_eq!(broken.kind, FailureKind::Other);
        assert_eq!(ctx.take_phone_stop(), None);
    }

    /// 083 H4: a phone that scanned but never connected is not an answer to
    /// the page either — not "-32603 the encrypted channel failed: Other(…)".
    /// The request stays open, and the column is told which of the two
    /// stops it was, once, so it can say so and offer the approval again.
    #[test]
    fn a_phone_that_never_connected_is_not_an_answer() {
        use crate::ctap::cable::HybridError;
        let ctx = context(Some("https://app.uniswap.org"));
        for (error, stop) in [
            (
                HybridError::Handshake("the phone stopped answering".to_owned()),
                PhoneStop::LinkFailed,
            ),
            (
                HybridError::Tunnel("cannot reach cable.ua5v.com".to_owned()),
                PhoneStop::LinkFailed,
            ),
            (HybridError::NoAdvert, PhoneStop::ScanExpired),
        ] {
            ctx.prompt_reset();
            let failure = around_prompt(&ctx, || Err(PasskeyFailure::other(error.to_string())))
                .err()
                .unwrap_or_else(|| unreachable!("no phone, no assertion"));
            assert_eq!(failure.kind, FailureKind::Cancelled, "{error}");
            assert_eq!(failure.message, None, "no transport English: {error}");
            assert_eq!(ctx.take_phone_stop(), Some(stop), "{error}");
            assert_eq!(ctx.take_phone_stop(), None, "told once");
        }
    }

    /// 083 H4 review: a phone that was asked and then dropped is not an
    /// answer either — not "-32603 The security key stopped responding", nor
    /// the OS's socket error. Nothing signed reached this desktop, so the
    /// request stays open for another try, and the column is told this was
    /// the third stop. A new approval starts with none told.
    #[test]
    fn a_phone_that_dropped_once_asked_is_not_an_answer() {
        let ctx = context(Some("https://app.uniswap.org"));
        let dropped = around_prompt(&ctx, || {
            Err(PasskeyFailure::other(format!(
                "{}: the phone stopped answering",
                crate::ctap::cable::PHONE_DROPPED
            )))
        })
        .err()
        .unwrap_or_else(|| unreachable!("a dropped phone signs nothing"));
        assert_eq!(dropped.kind, FailureKind::Cancelled);
        assert_eq!(dropped.message, None, "no transport English");
        assert!(!ctx.signature_done.load(Ordering::SeqCst));

        ctx.prompt_reset();
        assert_eq!(ctx.take_phone_stop(), None, "an approval starts clean");

        let _ = around_prompt(&ctx, || {
            Err(PasskeyFailure::other(format!(
                "{}: connection reset",
                crate::ctap::cable::PHONE_DROPPED
            )))
        });
        assert_eq!(ctx.take_phone_stop(), Some(PhoneStop::Dropped));

        // The phone's own cancel (a tunnel it closed) is the person's: no
        // stop, and the core hears a dismissal as before.
        let cancelled = around_prompt(&ctx, || {
            Err(PasskeyFailure::classified(
                FailureKind::Cancelled,
                "your phone ended the session before signing",
            ))
        })
        .err()
        .unwrap_or_else(|| unreachable!("a cancel signs nothing"));
        assert_eq!(cancelled.kind, FailureKind::Cancelled);
        assert_eq!(ctx.take_phone_stop(), None);
    }

    /// 083 (review): a column closed before its prompt opened asks nothing —
    /// no QR, no Windows Hello — and the ceremony comes back unsigned at
    /// once, so the column's waiting close can refuse. The next approval
    /// starts asking again.
    #[test]
    fn a_closed_column_opens_no_prompt() {
        let ctx = context(Some("https://app.uniswap.org"));
        ctx.abandoned.store(true, Ordering::SeqCst);
        let mut asked = false;
        let answer = around_prompt(&ctx, || {
            asked = true;
            Err(PasskeyFailure::other("never reached"))
        });
        assert!(!asked, "no prompt opened");
        assert_eq!(
            answer.err().map(|failure| failure.kind),
            Some(FailureKind::Cancelled)
        );
        assert!(
            !ctx.signing_started.load(Ordering::SeqCst),
            "nothing is said to be asking"
        );

        ctx.prompt_reset();
        assert!(!ctx.abandoned.load(Ordering::SeqCst));
        let answer = around_prompt(&ctx, || {
            asked = true;
            Err(PasskeyFailure::cancelled())
        });
        assert!(asked, "a new approval asks");
        assert!(answer.is_err());
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

    /// Typed data is the core's EIP-712 digest of the request's ONE document,
    /// read strictly (085, the EIP-712 WYSIWYS audit): the second parameter
    /// of `[address, document]`, the first of the legacy `[document, address]`
    /// — and nothing at all for one param, the wrong order or two documents.
    #[test]
    fn typed_data_is_the_cores_digest_of_the_right_parameter() {
        let account = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let document = r#"{"types":{"EIP712Domain":[{"name":"name","type":"string"}],"Mail":[{"name":"contents","type":"string"}]},"primaryType":"Mail","domain":{"name":"Vela"},"message":{"contents":"hi"}}"#;
        let digest = vela_core::eip712::hash_typed_data(document).ok();
        assert!(digest.is_some());
        let quoted = serde_json::to_string(document).unwrap_or_default();
        assert_eq!(
            message_hash(
                "eth_signTypedData_v4",
                &format!(r#"["{account}",{quoted}]"#)
            ),
            digest
        );
        assert_eq!(
            message_hash(
                "eth_signTypedData_v4",
                &format!(r#"["{account}",{document}]"#)
            ),
            digest,
            "a document sent as an object is the same document"
        );
        assert_eq!(
            message_hash("eth_signTypedData", &format!(r#"[{quoted},"{account}"]"#)),
            digest
        );
        assert_eq!(
            message_hash("eth_signTypedData_v4", &format!("[{quoted}]")),
            None
        );
        assert_eq!(
            message_hash("eth_signTypedData_v4", &format!("[{quoted},{quoted}]")),
            None
        );
        assert_eq!(
            message_hash("eth_signTypedData", &format!("[{quoted},{quoted}]")),
            None
        );
        assert!(is_message("eth_signTypedData_v4") && is_message("personal_sign"));
        assert!(!is_message("eth_sendTransaction") && !is_message("wallet_sendCalls"));
    }

    /// 083: each way an operation can end reaches the core as exactly one
    /// outcome, and the page is answered a hash only for an operation that
    /// EXECUTED. A reverted one is `Reverted` (the core answers an error and
    /// fails the record — the bundle's `0x1` is not the op's success); a wait
    /// that ran the whole cap is `NotConfirmed` (an error, the record left to
    /// the tracker) — never `ReceiptPending`, whose answer is the op hash
    /// Uniswap looked up on its own node forever.
    #[test]
    fn a_landing_reaches_the_core_as_one_honest_outcome() {
        let op = || "0xop".to_owned();
        assert_eq!(
            after_landing(op(), Some(Landing::Landed("0xtx".to_owned()))),
            SignSubmitOutcome::Succeeded {
                result: "0xtx".to_owned()
            }
        );
        assert_eq!(
            after_landing(op(), Some(Landing::Reverted("0xtx".to_owned()))),
            SignSubmitOutcome::Reverted {
                user_op_hash: op(),
                tx_hash: "0xtx".to_owned()
            }
        );
        assert_eq!(
            after_landing(op(), None),
            SignSubmitOutcome::NotConfirmed { user_op_hash: op() }
        );
        assert!(matches!(
            after_landing(op(), Some(Landing::Refused)),
            SignSubmitOutcome::Failed { message, refused: true, .. } if !message.contains("0xop")
        ));
        for landing in [
            None,
            Some(Landing::Landed("0xtx".to_owned())),
            Some(Landing::Reverted("0xtx".to_owned())),
            Some(Landing::Refused),
        ] {
            assert!(
                !matches!(
                    after_landing(op(), landing.clone()),
                    SignSubmitOutcome::ReceiptPending { .. }
                ) && after_landing(op(), landing.clone())
                    != SignSubmitOutcome::Succeeded { result: op() },
                "{landing:?} answered the op hash"
            );
        }
    }

    /// The page waits the core's cap — ten minutes, where 079 answered the
    /// op hash at 90 s.
    #[test]
    fn the_page_waits_the_cores_cap() {
        assert_eq!(LANDING_CAP, Duration::from_secs(600));
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
            first_party: false,
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
                // A string as it is; anything else (a batch's `{ id }`, spec
                // 097 G) as its JSON, so a shape never reads as a bare hash.
                SignOperation::SendResponse {
                    payload: SignResponsePayload::Ok { result },
                    ..
                } => Some(result.as_ref().map(|result| {
                    result
                        .as_str()
                        .map_or_else(|| result.to_string(), str::to_owned)
                })),
                // An error the page was told — its message, marked as one.
                SignOperation::SendResponse {
                    payload: SignResponsePayload::Err { message, .. },
                    ..
                } => Some(Some(format!(
                    "error: {}",
                    message.clone().unwrap_or_default()
                ))),
                _ => None,
            })
            .collect()
    }

    /// Drive one submit to its end the way `sign_and_submit` does: the
    /// ceremony's edges, the op told before the wait (with its lost-reply
    /// facts), the record written, and then `after_landing`'s verdict.
    /// Everything the page was told, and the record the core asked for.
    fn submit_to_the_end(
        maybe_sent: bool,
        landing: Option<Landing>,
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
                outcome: after_landing(LOCAL_OP.to_owned(), landing),
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

    /// RA2: the relay's reply was lost. The op is recorded as may have been
    /// sent, with the head it was sent at, and the page gets exactly ONE
    /// answer — "not confirmed yet" (083, owner ruling 2026-10-01), never
    /// 4900 and never the op hash.
    #[test]
    fn a_mute_relay_is_answered_once_not_confirmed() {
        let (told, record, _) = submit_to_the_end(true, None);
        assert_eq!(
            told,
            vec![Some(format!(
                "error: {}",
                vela_core::app::sign_request::not_confirmed_detail(LOCAL_OP)
            ))]
        );
        assert!(record.maybe_sent);
        assert_eq!(record.submit_block, Some(48_479_132));
        assert_eq!(record.user_op_hash, LOCAL_OP);
    }

    /// A receipt that came back reverted is answered the revert, naming its
    /// transaction, once — never the tx hash a site reads as done (083,
    /// owner ruling 2026-10-01, superseding ruling 9).
    #[test]
    fn a_reverted_receipt_is_answered_the_revert() {
        let (told, record, _) =
            submit_to_the_end(false, Some(Landing::Reverted(LANDED_TX.to_owned())));
        assert_eq!(
            told,
            vec![Some(format!(
                "error: {}",
                vela_core::app::sign_request::reverted_detail(LANDED_TX)
            ))]
        );
        assert!(!record.maybe_sent);
    }

    /// RA9: "preparing" through the pre-check, "awaiting signature" only
    /// while the ceremony is up, "submitting" once it has signed.
    #[test]
    fn the_phases_follow_the_ceremony_edges() {
        use vela_core::app::sign_request::SignPhase;
        let (_, _, phases) = submit_to_the_end(false, Some(Landing::Landed(LANDED_TX.to_owned())));
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
            // Spec 096 F8: held while the sheet shows the failure — nothing
            // was sent, so it may be tried again — and answered on the close.
            assert!(told.is_empty(), "held for the sheet: {told:?}");
            assert!(!host.view().failure_refused);
            assert!(host.view().failure_retryable);
            assert_eq!(
                host.view().tracker_withdraw,
                Some(SignTrackerWithdraw {
                    user_op_hash: LOCAL_OP.to_owned(),
                    record_ids: vec![record_id],
                }),
                "the tracker forgets it"
            );
            let told: Vec<SignResponsePayload> = host
                .dispatch(Event::SwipeDismissed)
                .iter()
                .filter_map(|op| match &op.operation {
                    SignOperation::SendResponse { payload, .. } => Some(payload.clone()),
                    _ => None,
                })
                .collect();
            assert_eq!(told.len(), 1, "one answer: {told:?}");
            assert!(matches!(
                &told[0],
                SignResponsePayload::Err { code: -32603, message: Some(message), .. }
                    if message == NOT_SENT_DAPP_DETAIL
            ));
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
            // Spec 096 F8: held while the sheet says "refused" (no retry: the
            // same op is refused again), answered on the close.
            assert!(
                !settled
                    .iter()
                    .any(|op| matches!(op.operation, SignOperation::SendResponse { .. })),
                "held for the sheet"
            );
            assert!(host.view().failure_refused, "the sheet says refused");
            assert!(!host.view().failure_retryable);
            let told: Vec<_> = host
                .dispatch(Event::SwipeDismissed)
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

    /// The row this writes is the one Activity draws (083 H2, 079 D3): read
    /// back by the feed's own reader, folded by the core, titled by what the
    /// site asked, labelled with the site — and closed in place when it lands.
    /// Before 083 the core dropped every `dapp_tx` row, so a dApp's operation
    /// was on disk and on no screen.
    /// Answer the feed's asks from the store, as its executor does. Returns
    /// every name lookup the core asked for.
    fn settle(
        host: &mut crate::core_host::CoreHost<vela_core::app::activity_feed::ActivityFeed>,
        mut pending: Vec<crate::core_host::Pending<vela_core::app::activity_feed::FeedOperation>>,
    ) -> Vec<String> {
        use vela_core::app::activity_feed::{FeedOperation, FeedShellResult};

        let mut lookups = Vec::new();
        while let Some(next) = pending.pop() {
            let result = match &next.operation {
                FeedOperation::ReadTxStore { read_id, .. } => FeedShellResult::StoreLoaded {
                    records: crate::executor::activity_feed::read_records(),
                    now_ms: 1_757_000_001_000.0,
                    read_id: *read_id,
                },
                FeedOperation::ScanIncomingTransfers { .. } => {
                    FeedShellResult::SyncCompleted { new_count: 0 }
                }
                FeedOperation::ResolveRecipientIdentity { addr } => {
                    lookups.push(addr.clone());
                    FeedShellResult::AliasResolved {
                        addr: addr.clone(),
                        name: None,
                    }
                }
                _ => continue,
            };
            pending.extend(host.resolve(next.id, result));
        }
        lookups
    }

    /// A dApp detail's "Balance changes" fact, its lines as drawn.
    fn changes_of(
        detail: &crate::flows::fixtures::TxDetail,
        flow: &crate::flows::FlowStrings,
    ) -> Option<String> {
        detail
            .facts
            .iter()
            .find(|fact| fact.label == flow.detail_changes)
            .map(|fact| fact.value.to_string())
    }

    #[test]
    fn a_dapp_transaction_is_in_activity_with_its_site_until_it_lands() {
        use crate::core_host::CoreHost;
        use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent, FeedTxStatus};
        use vela_core::app::clear_signing::ClearTerm;

        storage::tests::with_temp_state("sign-record-feed", || {
            let mut record = record(
                SignRecordKind::DappTx,
                r#"[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x2386f26fc10000"}]"#,
            );
            // The name a dApp could have given itself, beside the origin it
            // came from: the site is read from the origin only.
            record.dapp_origin = "app.uniswap.org".to_owned();
            record.dapp_url = "http://127.0.0.1:5173".to_owned();
            record.intent = Some("Send".to_owned());
            persist_record(&record);

            let mut host = CoreHost::<ActivityFeed>::new();
            let asks = host.dispatch(FeedEvent::AccountSwitched {
                address: record.from.clone(),
            });
            settle(&mut host, asks);

            let loc = crate::loc::Loc::from_env();
            let s = crate::wallet::WalletStrings::resolve(&loc);
            let flow = crate::flows::FlowStrings::resolve(&loc);
            let view = host.view();
            let rows = crate::wallet::live::activity_rows(&view, &s, &flow, false);
            assert_eq!(rows.len(), 1, "the dApp's transaction is in Activity");
            let row = &rows[0];
            assert_eq!(row.kind, crate::wallet::fixtures::ActivityKind::Dapp);
            // A plain send reads as one, at the site as the address bar names
            // it (spec 082 RG1: host[:port]) — never the name the dApp gave.
            let send = s
                .terms
                .get(&ClearTerm::IntentSend)
                .cloned()
                .unwrap_or_default();
            assert_eq!(
                row.title.to_string(),
                crate::wallet::fill(
                    &crate::wallet::fill(&s.dapp_row_title, "place", "127.0.0.1:5173"),
                    "intent",
                    &send
                )
            );
            // Pending until it lands, on its network; the site is in the title.
            assert_eq!(
                row.subtitle.to_string(),
                format!(
                    "{} · {}",
                    s.status_pending,
                    crate::flows::live::chain_name(100)
                )
            );
            // 0x2386f26fc10000 wei is 0.01 of the coin, going out.
            assert!(
                row.amount.starts_with('\u{2212}') && row.amount.ends_with("01"),
                "{}",
                row.amount
            );
            assert_eq!(row.unit.as_ref(), "xDAI");
            assert_eq!(view.transactions[0].status, FeedTxStatus::Pending);

            // It lands: the same row closes, and the feed's re-read shows it.
            update_record(
                &record.record_id,
                &SignRecordClose::Confirmed {
                    tx_hash: "0xdeadbeef".to_owned(),
                },
            );
            let asks = host.dispatch(FeedEvent::ReconcileCompleted { resolved_count: 1 });
            settle(&mut host, asks);
            let view = host.view();
            assert_eq!(view.transactions.len(), 1);
            assert_eq!(view.transactions[0].status, FeedTxStatus::Confirmed);
            assert_eq!(view.transactions[0].tx_hash, "0xdeadbeef");
        });
    }

    /// A batch submits its `calls` and nothing else, so a top-level `to` or
    /// `value` beside them is the page's to write and the sheet never shows
    /// it (083 H2 review). Kept, `"to":"日本語日本語"` was shortened by byte on
    /// the home page and crashed the wallet on every launch, and a forged
    /// `value` drew −1,208,925 xDAI that never moved. The record keeps
    /// neither, and the row that reaches the screen has no recipient, no
    /// figure and no name lookup — the site is what labels it.
    #[test]
    fn a_batch_keeps_no_recipient_or_figure_the_page_wrote_beside_its_calls() {
        use crate::core_host::CoreHost;
        use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};

        storage::tests::with_temp_state("sign-record-batch", || {
            let mut record = record(
                SignRecordKind::DappTx,
                r#"[{"version":"2.0.0","chainId":"0x64",
                    "calls":[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x0"}],
                    "to":"日本語日本語","value":"0xffffffffffffffffffff"}]"#,
            );
            record.method = "wallet_sendCalls".to_owned();
            record.intent = None;
            persist_record(&record);
            let stored = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            assert_eq!(stored[0].get("to").and_then(Value::as_str), Some(""));
            assert_eq!(stored[0].get("value").and_then(Value::as_str), Some("0x0"));

            let mut host = CoreHost::<ActivityFeed>::new();
            let asks = host.dispatch(FeedEvent::AccountSwitched {
                address: record.from.clone(),
            });
            let lookups = settle(&mut host, asks);
            assert!(lookups.is_empty(), "no name lookup: {lookups:?}");

            let loc = crate::loc::Loc::from_env();
            let s = crate::wallet::WalletStrings::resolve(&loc);
            let flow = crate::flows::FlowStrings::resolve(&loc);
            let view = host.view();
            // A call nobody read, at the site that asked (spec 093).
            let title = crate::wallet::fill(
                &crate::wallet::fill(&s.dapp_row_title, "place", "app.uniswap.org"),
                "intent",
                &s.intent_contract_call,
            );
            for hidden in [false, true] {
                let rows = crate::wallet::live::activity_rows(&view, &s, &flow, hidden);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].title.as_ref(), title);
                // A row the tracker has not closed leads with its status
                // (spec 082 RG2), then its network; the site is in the title.
                assert_eq!(
                    rows[0].subtitle.to_string(),
                    format!(
                        "{} · {}",
                        s.status_pending,
                        crate::flows::live::chain_name(100)
                    )
                );
                assert!(!rows[0].subtitle.contains("日本語"));
                assert!(!rows[0].title.contains("日本語"));
                assert_eq!((rows[0].amount.as_ref(), rows[0].unit.as_ref()), ("", ""));
            }
        });
    }

    /// The record keeps the figure the submit moved, as the `0x` quantity
    /// every other row carries, rather than `"0x0"` (083 H2 review) — read by
    /// the core's rule, whatever case the page wrote it in (a JSON number no
    /// longer reaches a record: the core refuses it at arrival, spec 096 F1).
    /// The record also keeps the origin itself beside the name.
    #[test]
    fn a_value_is_recorded_as_the_quantity_it_moved() {
        storage::tests::with_temp_state("sign-record-number", || {
            let record = record(
                SignRecordKind::DappTx,
                r#"[{"to":"0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","value":"0x2386F26FC10000"}]"#,
            );
            persist_record(&record);
            let rows = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            assert_eq!(
                rows[0].get("value").and_then(Value::as_str),
                Some("0x2386f26fc10000"),
                "0.01 of the coin, not nothing"
            );
            assert_eq!(
                rows[0].get("dappUrl").and_then(Value::as_str),
                Some("https://app.uniswap.org")
            );
        });
        // Absent, empty and unreadable are all "moved nothing".
        assert_eq!(stored_value(None), "0x0");
        assert_eq!(stored_value(Some(&json!("0x"))), "0x0");
        assert_eq!(stored_value(Some(&json!("banana"))), "0x0");
        assert_eq!(
            stored_value(Some(&json!("0x2386F26FC10000"))),
            "0x2386f26fc10000"
        );
        // Decimal text is not a quantity by the core's rule (spec 096 F1):
        // no shell signs it, so no row claims it.
        assert_eq!(stored_value(Some(&json!("10000000000000000"))), "0x0");
    }

    /// The Universal Router the device pass's swaps called (0xd614…9c40).
    const ROUTER: &str = "0xd614000000000000000000000000000000009c40";
    const USDC_BASE: &str = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";

    /// Base's USDC as the sheet judged it: one of the chain's stables, so in
    /// the set the wallet trusts.
    fn usdc(delta: &str) -> vela_core::app::token_trust::TrustSimJudgment {
        vela_core::app::token_trust::TrustSimJudgment::Erc20Trusted {
            token: USDC_BASE.to_owned(),
            delta: delta.to_owned(),
            symbol: "USDC".to_owned(),
            decimals: 6,
            in_trusted_set: true,
        }
    }

    fn eth(delta: &str) -> vela_core::app::token_trust::TrustSimJudgment {
        vela_core::app::token_trust::TrustSimJudgment::Native {
            delta: delta.to_owned(),
        }
    }

    /// One Uniswap swap on Base as the column records it: the router call
    /// with its calldata and value, no decoded intent, what the sheet's
    /// simulation showed, landed under `hash`.
    fn swap(
        at_ms: f64,
        value: &str,
        changes: Vec<vela_core::app::token_trust::TrustSimJudgment>,
        hash: &str,
    ) -> SignRecord {
        let mut record = record(
            SignRecordKind::DappTx,
            &format!(r#"[{{"to":"{ROUTER}","value":"{value}","data":"0x3593564c0000"}}]"#),
        );
        record.record_id = format!("dapp-{at_ms}-tx");
        record.now_ms = at_ms;
        record.chain_id = 8453;
        record.intent = None;
        record.balance_changes = Some(changes);
        record.result = hash.to_owned();
        record.status = SignRecordStatus::Confirmed;
        record
    }

    /// 083 F1-F3, the device pass's three swaps (2026-09-29), written by
    /// this executor and read back by the feed's own reader, in Chinese:
    /// each row says what left — the figure the sheet showed, "≈" unless it
    /// is the coin the wallet itself sent — and what was expected back; the
    /// detail lists the sheet's "余额变化" lines, names the router a contract
    /// (合约), and fits its hash.
    #[test]
    fn the_device_passs_swaps_say_what_they_moved() {
        use crate::core_host::CoreHost;
        use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};

        const HASH: &str = "0x9f2c4e5d6a7b8c9d0e1f2a3b4c5d6e7f8091a2b3c4d5e6f708192a3b4c5d6e7f";
        storage::tests::with_temp_state("sign-record-swaps", || {
            let swaps = [
                swap(
                    1_759_100_000_000.0,
                    "0x0",
                    vec![usdc("-100000"), eth("37000000000000")],
                    HASH,
                ),
                swap(
                    1_759_100_100_000.0,
                    "0x0",
                    vec![usdc("-271741"), eth("100548000000000")],
                    HASH,
                ),
                swap(
                    1_759_100_200_000.0,
                    "0x5af3107a4000",
                    vec![eth("-100000000000000"), usdc("269487")],
                    HASH,
                ),
            ];
            for record in &swaps {
                persist_record(record);
            }
            let stored = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            // The web's shape for the same lines, and the judgment's word
            // that the wallet trusts the token.
            assert_eq!(
                stored[0].get("assetChanges"),
                Some(&json!({
                    "engine": "rpc",
                    "changes": [
                        { "kind": "erc20", "token": USDC_BASE, "delta": "-100000",
                          "symbol": "USDC", "decimals": 6, "trusted": true },
                        { "kind": "native", "delta": "37000000000000",
                          "symbol": "ETH", "decimals": 18 },
                    ],
                }))
            );
            assert_eq!(stored[0].get("calldata"), Some(&json!(true)));

            let mut host = CoreHost::<ActivityFeed>::new();
            let asks = host.dispatch(FeedEvent::AccountSwitched {
                address: swaps[0].from.clone(),
            });
            settle(&mut host, asks);

            let loc = crate::loc::Loc::for_language("zh");
            let s = crate::wallet::WalletStrings::resolve(&loc);
            let flow = crate::flows::FlowStrings::resolve(&loc);
            let view = host.view();
            let rows = crate::wallet::live::activity_rows(&view, &s, &flow, false);
            let drawn: Vec<(&str, &str, &str, &str, Option<&str>)> = rows
                .iter()
                .map(|row| {
                    (
                        row.title.as_ref(),
                        row.subtitle.as_ref(),
                        row.amount.as_ref(),
                        row.unit.as_ref(),
                        row.received.as_ref().map(AsRef::as_ref),
                    )
                })
                .collect();
            let base = crate::flows::live::chain_name(8453);
            assert_eq!(
                drawn,
                vec![
                    (
                        "在 app.uniswap.org 合约交互",
                        base.as_str(),
                        "\u{2212}0.0001",
                        "ETH",
                        Some("≈ +0.269487 USDC")
                    ),
                    (
                        "在 app.uniswap.org 合约交互",
                        base.as_str(),
                        "≈ \u{2212}0.271741",
                        "USDC",
                        Some("≈ +0.000101 ETH")
                    ),
                    (
                        "在 app.uniswap.org 合约交互",
                        base.as_str(),
                        "≈ \u{2212}0.1",
                        "USDC",
                        Some("≈ +0.000037 ETH")
                    ),
                ],
                "newest first; the title stays what the wallet knows, at the site"
            );
            // Privacy masks both figures and keeps their units.
            let masked = crate::wallet::live::activity_rows(&view, &s, &flow, true);
            assert_eq!(masked[2].amount.as_ref(), crate::wallet::fixtures::MASK);
            assert_eq!(
                masked[2].received.as_ref().map(AsRef::as_ref),
                Some("≈ +•••• ETH")
            );

            let id = view.transactions[0].id.clone();
            let detail = crate::flows::live::tx_detail(
                &view,
                &swaps[0].record_id,
                &flow,
                &s,
                false,
                "zh-CN",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("the row exists: {id}"));
            assert_eq!(detail.title.as_ref(), "在 app.uniswap.org 合约交互");
            assert_eq!(detail.amount.as_ref(), "≈ \u{2212}0.1 USDC");
            assert!(detail.fiat.contains("0.10"), "{}", detail.fiat);
            let facts: Vec<(&str, &str)> = detail
                .facts
                .iter()
                .map(|fact| (fact.label.as_ref(), fact.value.as_ref()))
                .collect();
            assert_eq!(facts[0], ("应用", "app.uniswap.org"));
            assert_eq!(facts[1].0, "网络");
            assert_eq!(facts[2], ("合约", "0xd614…9c40"));
            assert_eq!(
                facts[3],
                ("余额变化", "≈ \u{2212}0.1 USDC\n≈ +0.000037 ETH")
            );
            assert_eq!(facts[4].0, "日期");
            // The hash is a technical detail (spec 093), shortened to fit,
            // whole on its copy button.
            let mut opened = detail.clone();
            crate::flows::live::open_technical(
                &mut opened,
                &view,
                &swaps[0].record_id,
                None,
                &flow,
                &s,
                "zh-CN",
            );
            let hash = opened
                .technical
                .and_then(|technical| technical.lines)
                .unwrap_or_default()
                .into_iter()
                .find_map(|line| match line {
                    crate::flows::fixtures::TechnicalLine::Fact(fact)
                        if fact.label == flow.detail_hash =>
                    {
                        Some(fact)
                    }
                    _ => None,
                })
                .unwrap_or_else(|| unreachable!("a landed swap has its hash"));
            assert_eq!(hash.value.as_ref(), "0x9f2c…6e7f");
            assert_eq!(
                hash.copy.as_ref().map(AsRef::as_ref),
                Some(HASH),
                "the copy button holds the whole hash"
            );

            // What the wallet sent reads bare, in the row and in its lines.
            let detail = crate::flows::live::tx_detail(
                &view,
                &swaps[2].record_id,
                &flow,
                &s,
                false,
                "zh-CN",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("the row exists"));
            assert_eq!(detail.amount.as_ref(), "\u{2212}0.0001 ETH");
            assert_eq!(
                changes_of(&detail, &flow).as_deref(),
                Some("\u{2212}0.0001 ETH\n≈ +0.269487 USDC")
            );
        });
    }

    /// 083 F1 review: a swap that FAILED moved nothing, and Activity's rows
    /// carry no status mark — so it keeps none of what the sheet expected.
    /// Closed failed in place (the store keeps its `assetChanges`), its row
    /// draws as before F1: no figure from the simulation, nothing "≈ back";
    /// its detail wears 失败 and lists no 余额变化.
    #[test]
    fn a_failed_swap_says_nothing_moved() {
        use crate::core_host::CoreHost;
        use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};

        storage::tests::with_temp_state("sign-record-failed-swap", || {
            let mut failed = swap(
                1_759_100_000_000.0,
                "0x0",
                vec![usdc("-100000"), eth("37000000000000")],
                "",
            );
            failed.status = SignRecordStatus::Pending;
            persist_record(&failed);
            update_record(&failed.record_id, &SignRecordClose::Failed);

            let mut host = CoreHost::<ActivityFeed>::new();
            let asks = host.dispatch(FeedEvent::AccountSwitched {
                address: failed.from.clone(),
            });
            settle(&mut host, asks);
            let loc = crate::loc::Loc::for_language("zh");
            let s = crate::wallet::WalletStrings::resolve(&loc);
            let flow = crate::flows::FlowStrings::resolve(&loc);
            let view = host.view();
            let rows = crate::wallet::live::activity_rows(&view, &s, &flow, false);
            assert_eq!(
                (
                    rows[0].title.as_ref(),
                    rows[0].amount.as_ref(),
                    rows[0].unit.as_ref()
                ),
                ("在 app.uniswap.org 合约交互", "", "")
            );
            assert_eq!(rows[0].received, None, "nothing \"≈ back\" on a revert");

            let detail = crate::flows::live::tx_detail(
                &view,
                &failed.record_id,
                &flow,
                &s,
                false,
                "zh-CN",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("the row exists"));
            assert_eq!(
                detail.status.as_ref().map(|chip| &chip.text),
                Some(&flow.status_failed)
            );
            assert_eq!(detail.amount.as_ref(), "");
            assert_eq!(
                changes_of(&detail, &flow),
                None,
                "no balance changes that never were"
            );
            assert!(detail.breakdown.is_empty());
        });
    }

    /// 083 F1 review: a site's own contract that emits `Transfer(you, …)` and
    /// answers "USDC" is on the sheet as an outflow — but it is not a token
    /// the wallet trusts, so it is never Activity's figure or its price. The
    /// detail still lists it, as the sheet drew it.
    #[test]
    fn a_site_token_does_not_write_the_rows_figure() {
        use crate::core_host::CoreHost;
        use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};

        storage::tests::with_temp_state("sign-record-site-token", || {
            let fake = vela_core::app::token_trust::TrustSimJudgment::Erc20Trusted {
                token: "0x00000000000000000000000000000000000bad01".to_owned(),
                delta: "-100000000000".to_owned(),
                symbol: "USDC".to_owned(),
                decimals: 6,
                in_trusted_set: false,
            };
            let lure = swap(1_759_100_000_000.0, "0x0", vec![fake], "");
            persist_record(&lure);
            let stored = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            assert!(
                stored[0]["assetChanges"]["changes"][0]
                    .get("trusted")
                    .is_none(),
                "no word of trust the judgment did not give"
            );

            let mut host = CoreHost::<ActivityFeed>::new();
            let asks = host.dispatch(FeedEvent::AccountSwitched {
                address: lure.from.clone(),
            });
            settle(&mut host, asks);
            let loc = crate::loc::Loc::for_language("zh");
            let s = crate::wallet::WalletStrings::resolve(&loc);
            let flow = crate::flows::FlowStrings::resolve(&loc);
            let view = host.view();
            let rows = crate::wallet::live::activity_rows(&view, &s, &flow, false);
            assert_eq!((rows[0].amount.as_ref(), rows[0].unit.as_ref()), ("", ""));
            let detail = crate::flows::live::tx_detail(
                &view,
                &lure.record_id,
                &flow,
                &s,
                false,
                "zh-CN",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("the row exists"));
            assert_eq!(detail.amount.as_ref(), "");
            assert_eq!(detail.fiat.as_ref(), "", "no dollars for a site's claim");
            assert_eq!(
                changes_of(&detail, &flow).as_deref(),
                Some("≈ \u{2212}100,000.00 USDC")
            );
        });
    }

    /// 083 F3 review: the executor decides "called a contract" from the WHOLE
    /// final request, by the reading the submit path uses — so a request past
    /// the 8 KB clip still names its router a contract, and a `calls` a page
    /// wrote beside a plain send does not turn its recipient into one.
    #[test]
    fn the_record_says_whether_it_called_a_contract() {
        storage::tests::with_temp_state("sign-record-calldata", || {
            let padding = "a".repeat(9 * 1024);
            let mut long = record(
                SignRecordKind::DappTx,
                &format!(r#"[{{"to":"{ROUTER}","data":"0x3593564c","pad":"{padding}"}}]"#),
            );
            long.record_id = "dapp-1-tx".to_owned();
            persist_record(&long);
            let mut smuggled = record(
                SignRecordKind::DappTx,
                r#"[{"to":"0xeoa","value":"0xde0b6b3a7640000","calls":[{"data":"0x01"}]}]"#,
            );
            smuggled.record_id = "dapp-2-tx".to_owned();
            persist_record(&smuggled);
            let mut batch = record(
                SignRecordKind::DappTx,
                r#"[{"calls":[{"to":"0xb","value":"0x1"},{"to":"0xr","data":"0x095ea7b3"}]}]"#,
            );
            batch.record_id = "dapp-3-tx".to_owned();
            batch.method = "wallet_sendCalls".to_owned();
            persist_record(&batch);
            let mut message = record(SignRecordKind::SignMessage, r#"["0xdeadbeef","0xme"]"#);
            message.record_id = "dapp-4-msg".to_owned();
            message.method = "personal_sign".to_owned();
            persist_record(&message);

            let stored = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            assert_eq!(stored[0].get("requestTruncated"), Some(&json!(true)));
            let flags: Vec<Option<&Value>> = stored.iter().map(|row| row.get("calldata")).collect();
            assert_eq!(
                flags,
                vec![
                    Some(&json!(true)),
                    Some(&json!(false)),
                    Some(&json!(true)),
                    None
                ]
            );
            let read: Vec<Option<bool>> = crate::executor::activity_feed::read_records()
                .into_iter()
                .map(|record| record.calldata)
                .collect();
            assert_eq!(read, vec![Some(true), Some(false), Some(true), None]);
        });
    }

    /// A page cannot put a figure on its row (083 F1): lines only ever come
    /// from the approve, so a request that carries an `assetChanges` of its
    /// own stores none, and its row draws as a call that moved no coin — and
    /// an unverified token the sheet showed without a number has none in
    /// Activity either.
    #[test]
    fn a_page_cannot_put_a_figure_on_its_row() {
        use crate::core_host::CoreHost;
        use vela_core::app::activity_feed::{ActivityFeed, Event as FeedEvent};

        storage::tests::with_temp_state("sign-record-forged-sim", || {
            let mut forged = record(
                SignRecordKind::DappTx,
                &format!(
                    r#"[{{"to":"{ROUTER}","value":"0x0","data":"0xabcdef",
                        "assetChanges":{{"engine":"rpc","changes":[{{"kind":"native","delta":"-5000000000000000000"}}]}}}}]"#
                ),
            );
            forged.chain_id = 8453;
            forged.intent = None;
            persist_record(&forged);
            let mut lure = swap(
                1_759_100_300_000.0,
                "0x0",
                vec![
                    usdc("-100000"),
                    vela_core::app::token_trust::TrustSimJudgment::Erc20Unverified {
                        token: Some("0x00000000000000000000000000000000000bad01".to_owned()),
                        delta: "1000000000000000000000000".to_owned(),
                    },
                ],
                "",
            );
            lure.status = SignRecordStatus::Pending;
            persist_record(&lure);

            let stored = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            assert!(
                stored[0].get("assetChanges").is_none(),
                "nothing the page sent"
            );

            let mut host = CoreHost::<ActivityFeed>::new();
            let asks = host.dispatch(FeedEvent::AccountSwitched {
                address: forged.from.clone(),
            });
            settle(&mut host, asks);
            let loc = crate::loc::Loc::for_language("zh");
            let s = crate::wallet::WalletStrings::resolve(&loc);
            let flow = crate::flows::FlowStrings::resolve(&loc);
            let view = host.view();
            let rows = crate::wallet::live::activity_rows(&view, &s, &flow, false);
            // Newest first: the lure, then the forged call.
            assert_eq!(
                (rows[0].amount.as_ref(), rows[0].unit.as_ref()),
                ("≈ \u{2212}0.1", "USDC")
            );
            assert_eq!(rows[0].received, None, "no figure the site's token wrote");
            assert_eq!((rows[1].amount.as_ref(), rows[1].unit.as_ref()), ("", ""));
            assert_eq!(rows[1].received, None);

            let detail = crate::flows::live::tx_detail(
                &view,
                &lure.record_id,
                &flow,
                &s,
                false,
                "zh-CN",
                crate::wallet::live::Money::usd(),
            )
            .unwrap_or_else(|| unreachable!("the row exists"));
            assert_eq!(
                changes_of(&detail, &flow).as_deref(),
                Some("≈ \u{2212}0.1 USDC\n+ 未验证代币")
            );
            // Still pending, and the hash row waits for a hash.
            assert_eq!(
                detail.status.as_ref().map(|chip| &chip.text),
                Some(&flow.status_pending)
            );
            assert!(
                detail
                    .facts
                    .iter()
                    .all(|fact| fact.label != flow.detail_hash)
            );
        });
    }

    /// A signature moves nothing, and its row must not claim otherwise.
    #[test]
    fn a_signature_row_carries_no_amount() {
        storage::tests::with_temp_state("sign-record-msg", || {
            let mut record = record(SignRecordKind::SignMessage, r#"["0xdeadbeef","0xabc"]"#);
            record.record_id = "dapp-1757000000000-msg".to_owned();
            record.method = "personal_sign".to_owned();
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
            // The core keeps the signature itself off the disk (spec 093).
            assert_eq!(row.get("txHash").and_then(Value::as_str), Some(""));
            assert_eq!(
                row.get("dappSummary")
                    .and_then(|summary| summary.get("action"))
                    .and_then(Value::as_str),
                Some("message")
            );
        });
    }

    /// Spec 093: the record keeps the request as the CORE cut it
    /// (`SignRecord::stored_request`, `request_truncated`) — the shell's own
    /// 8 KB cut is gone — and the core's summary verbatim as `dappSummary`,
    /// which the feed's reader hands back untouched. The summary was read
    /// from the whole request, so a permit past the cut still says what it
    /// granted.
    #[test]
    fn the_record_keeps_the_cores_cut_and_summary() {
        use vela_core::app::dapp_activity::{DappAction, stored_request, summarize};

        storage::tests::with_temp_state("sign-record-kept", || {
            let mut record = record(SignRecordKind::SignTypedData, "[]");
            record.method = "eth_signTypedData_v4".to_owned();
            let typed = json!({
                "types": {
                    "EIP712Domain": [{"name": "name", "type": "string"}],
                    "Note": [{"name": "text", "type": "string"}]
                },
                "primaryType": "Note",
                "domain": {"name": "Example"},
                "message": {"text": "字".repeat(9000)}
            });
            record.params_json = json!(["0xme", typed.to_string()]).to_string();
            let (kept, truncated) = stored_request(&record.params_json);
            assert!(truncated, "a 27 KB request is past the cut");
            record.stored_request = kept.clone();
            record.request_truncated = truncated;
            record.summary = Some(summarize(
                &record.method,
                &serde_json::from_str(&record.params_json).unwrap_or(Value::Null),
                record.chain_id,
                &record.dapp_url,
                None,
            ));
            super::persist_record(&record);

            let rows = match storage::read_value(TX_KEY).ok().flatten() {
                Some(Value::Array(rows)) => rows,
                _ => unreachable!("nothing written"),
            };
            assert_eq!(
                rows[0].get("signedRequest").and_then(Value::as_str),
                Some(kept.as_str()),
                "the core's cut, byte for byte"
            );
            assert_eq!(rows[0].get("requestTruncated"), Some(&json!(true)));
            assert_eq!(
                rows[0].get("dappSummary"),
                serde_json::to_value(&record.summary).ok().as_ref()
            );
            let read = crate::executor::activity_feed::read_records();
            assert_eq!(read[0].summary, record.summary);
            assert_eq!(
                read[0].summary.as_ref().map(|summary| summary.action),
                Some(DappAction::TypedData)
            );
        });
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

        // And what is NOT a hex quantity draws no fee rather than a made-up
        // one — decimal text, bare hex and a JSON number included: this shell
        // read "1000" as decimal where every other read it as hex (spec 096).
        for params in [
            r#"[{"to":"0xbbb","value":"soon"}]"#,
            r#"[{"to":"0xbbb","value":"0xzz"}]"#,
            r#"[{"to":"0xbbb","value":{}}]"#,
            r#"[{"to":"0xbbb","value":"1000"}]"#,
            r#"[{"to":"0xbbb","value":"aa87bee538000"}]"#,
            r#"[{"to":"0xbbb","value":1000}]"#,
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
        // Every leg or none: a leg it cannot read is never dropped and the
        // rest sent — a batch the page never asked for (spec 096).
        assert!(
            calls_of(
                "wallet_sendCalls",
                r#"[{"calls":[{"to":"0x1","value":"0x1"},{"value":"0x1"}]}]"#
            )
            .is_none()
        );
    }

    /// Spec 096 F1: PancakeSwap's BNB → USDC — the value every shell must send
    /// as 0.003 BNB, the core's reading.
    #[test]
    fn a_dapp_native_value_is_the_cores_reading() {
        let swap = r#"[{"to":"0x13f4EA83D0bd40E75C8222255bc855a974568Dd4","value":"0xaa87bee538000","data":"0x3593564c"}]"#;
        let calls =
            calls_of("eth_sendTransaction", swap).unwrap_or_else(|| unreachable!("a swap reads"));
        assert_eq!(calls[0].value, "3000000000000000");
        let batch = r#"[{"calls":[{"to":"0x1","data":"0x095ea7b3"},{"to":"0x2","value":"0xAA87BEE538000"}]}]"#;
        let legs = calls_of("wallet_sendCalls", batch).unwrap_or_else(|| unreachable!("reads"));
        assert_eq!(
            legs.iter().map(|c| c.value.as_str()).collect::<Vec<_>>(),
            ["0", "3000000000000000"]
        );
    }

    #[test]
    fn every_poll_gets_only_what_is_left_of_the_wait() {
        let budget = Duration::from_millis(300);
        let started = std::time::Instant::now();
        let mut given = Vec::new();
        let answer: Option<()> = landing::wait_until(
            budget,
            |_| Duration::from_millis(40),
            &|| false,
            |left| {
                given.push(left);
                // A relay that holds every call for as long as it is allowed.
                std::thread::sleep(left.min(Duration::from_millis(120)));
                None
            },
        );
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
            landing::wait_until(
                budget,
                |_| Duration::from_millis(40),
                &|| false,
                |_| Some("0xtx")
            ),
            Some("0xtx")
        );
    }

    /// Spec 082 RJ4: once the core has answered the page from the tracker,
    /// the landing wait stops at its next look — within a glance of the
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
        let answer: Option<()> = landing::wait_until(
            Duration::from_secs(10),
            |_| Duration::from_secs(3),
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
}
