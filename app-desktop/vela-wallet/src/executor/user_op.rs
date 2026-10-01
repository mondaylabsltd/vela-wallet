//! The submit spine: build, estimate, sign, submit — in `sendUserOpInBand`'s order.
//!
//! **Ported from** `app-web/vela-wallet/src/lib/services/safe-transaction.ts`
//! @ `origin/main` (`sendUserOpInBand`, `sendUserOpTempo`, `simulateUserOpGas`,
//! `sendBatchCalls`' call codec). The pure assembly is `vela_core::user_op`'s;
//! this file is the ORDER — which read happens before which, what is fatal
//! and what falls back — and the one seam the core cannot have, the passkey.
//!
//! ## What is signed is what was displayed
//!
//! The send machine hands over the fee it showed (`SendQuotedFee`), and this
//! path signs EXACTLY that amount to EXACTLY that recipient (invariant ①). The
//! relay's own 2×-real-cost gate rejects a stale quote loudly, so a mismatch
//! between screen and charge can never be silent. Only a caller with no
//! confirm screen falls back to a fresh send-time quote, and that fallback is
//! the web's, line for line.
//!
//! ## Two refusals before any signature
//!
//! A deployed wallet whose nonce could not be read, and a batch carrying a
//! real contract call whose gas could not be estimated, both stop HERE — a
//! passkey prompt on an operation the relay must reject is a prompt wasted
//! and, worse, a person told they signed something that then vanished.
//!
//! ## Who signs
//!
//! A passkey over the SafeOp hash — or the Trusted Signer (spec 071), which is
//! handed the ASSEMBLED operation as well: its page derives the same hash
//! from the operation's own bytes and refuses what it cannot derive, and the
//! answer is accepted only as a signature over the hash computed HERE.
//! Either way the assertion that comes back goes into the same envelope. A
//! message ([`sign_message`]) is the same choice over the Safe's
//! `SafeMessage` hash, with nothing submitted.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use vela_core::ClientDataKind;
use vela_core::app::fee_policy::{
    AssetPricing, FeeAssetKind, FeeAssetQuote, FeeCall, FeeGasOutcome, FeeTier,
    TEMPO_DEFAULT_FEE_TOKEN, TEMPO_FEE_TOKEN_DECIMALS, calculate_in_band_fee_amount,
    is_tempo_chain, tempo_call_gas_limit, tempo_expected_gas, tempo_minimum_fee_token_units,
    tempo_reimbursement,
};
use vela_core::app::send::SendSubmitFailure;
use vela_core::app::tx_tracker::WAIT_WINDOW_MS;
use vela_core::app::{Account, Assertion, FailureKind};
use vela_core::primitives::{from_hex, to_hex};
use vela_core::user_op::{
    CALL_GAS_LIMIT, EstimateFailure, MultiSendCall, NOT_SENT_DAPP_DETAIL, PRE_VERIFICATION_GAS,
    RelayRejection, SubmitVerdict, UserOperation, VERIFICATION_GAS_DEPLOYED,
    VERIFICATION_GAS_UNDEPLOYED, WRITE_AHEAD_WAIT_MS, WalletKey, build_dummy_signature,
    build_in_band_fee_leg, build_init_code_for_keys, build_multi_send_execute_call_data,
    build_user_op_signature, calculate_safe_op_hash, compute_safe_message_hash,
    eip1271_envelope_signature, encode_erc20_transfer, extract_client_data_fields,
    inner_calls_gas_floor, is_plain_transfer_call, pad_gas_estimate, parse_hex_quantity,
    signer_address_for, user_op_hash,
};
use vela_core::webauthn::{der_signature_to_raw_low_s, validate_client_data};

use crate::diag::{short, vlog};
use crate::executor::landing::{self, Landing};
use crate::executor::passkey::PasskeyFailure;
use crate::executor::trusted_signer::{self, Ask, Channel};
use crate::executor::{chain, pool, relay};

/// `TEMPO_VERIFICATION_GAS_UNDEPLOYED` (`tempo.ts:89`) — the one Tempo
/// constant `fee_policy` does not carry, because only the submit path
/// requests it.
const TEMPO_VERIFICATION_GAS_UNDEPLOYED: u128 = 6_000_000;
/// `MAX_QUOTE_VS_CHAIN_MULTIPLE` (`safe-transaction.ts:521`).
const MAX_QUOTE_VS_CHAIN_MULTIPLE: u128 = 3;

/// How a submit failed — the send machine's vocabulary, decided here from
/// the relay's words and the ceremony's kind (`classifySubmit`).
///
/// Every one of these means **nothing left the device**, or the relay
/// refused the operation with no earlier POST that could have delivered it
/// (spec 082 RA1). An operation that may have been sent is never a failure:
/// it is a [`Submitted`] with `maybe_sent`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitFailure {
    PasskeyCancelled,
    RelayerUnavailable,
    BundlerUnderfunded,
    /// The relay was never reached (`NotSent { rejection: None }`), or the
    /// write-ahead got no clearance and nothing was POSTed (RJ1): the dApp
    /// is told the core's fixed sentence, never the pool's text.
    NotSent,
    /// The relay refused the operation (`NotSent { rejection:
    /// Some(Other(..)) }`, spec 082 RJ3): nothing was sent, and trying again
    /// sends the same refusal. The words are the relay's, for diagnostics.
    Refused(String),
    /// The page that asked is gone (spec 082 RB2): the check before the
    /// ceremony, after it, or right before the relay POST stopped the
    /// submit. Nothing was signed or sent.
    AskerGone,
    /// Diagnostics only; the core words the screen.
    Other(String),
}

impl From<SubmitFailure> for SendSubmitFailure {
    fn from(failure: SubmitFailure) -> Self {
        match failure {
            SubmitFailure::PasskeyCancelled => SendSubmitFailure::PasskeyCancelled,
            SubmitFailure::RelayerUnavailable => SendSubmitFailure::RelayerUnavailable,
            SubmitFailure::BundlerUnderfunded => SendSubmitFailure::BundlerUnderfunded,
            SubmitFailure::NotSent => SendSubmitFailure::Other {
                message: Some(NOT_SENT_DAPP_DETAIL.to_owned()),
            },
            // The wallet's own Send is never withdrawn by a page
            // ([`always_asked`]); diagnostics only, should that change.
            SubmitFailure::AskerGone => SendSubmitFailure::Other {
                message: Some("the request was withdrawn; nothing was sent".to_owned()),
            },
            SubmitFailure::Other(message) | SubmitFailure::Refused(message) => {
                SendSubmitFailure::Other {
                    message: (!message.is_empty()).then_some(message),
                }
            }
        }
    }
}

/// An operation that left the device (spec 082 RA1–RA4).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Submitted {
    /// The relay's hash when it answered; the locally computed one when its
    /// reply was lost.
    pub user_op_hash: String,
    /// The reply was lost: the operation may be on its way. Recorded,
    /// tracked and answered all the same — never "failed, try again".
    pub maybe_sent: bool,
    /// The chain head read before the first POST, where the tracker's
    /// relay-independent landing check starts (ruling 8). Best effort.
    pub submit_block: Option<u64>,
}

/// The ceremony's two edges (spec 082 RA9): the prompt opened, and it came
/// back with a signature. The signing sheet words its stage from them —
/// "waiting for biometric" only while the prompt is really up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CeremonyEdge {
    Started,
    Signed,
}

/// Who hears the edges. The wallet's own Send reads its flags instead and
/// passes [`quiet`].
pub type Edges<'a> = &'a dyn Fn(CeremonyEdge);

/// Edges nobody listens to.
pub fn quiet(_: CeremonyEdge) {}

/// Is the request still wanted — is the page that asked still there (spec
/// 082 RB2, the money rule)? Asked before the ceremony opens, when it
/// answers, and right before the relay POST: nothing is signed or sent for a
/// page that has gone, however far its submit had got.
pub type Asked<'a> = &'a dyn Fn() -> bool;

/// A request no page can withdraw — the wallet's own Send.
pub fn always_asked() -> bool {
    true
}

/// The write-ahead (spec 082 RJ1, G34): told the operation's local hash and
/// the head read before its first POST, once it is signed and before any
/// byte goes to the relay; answers whether the POST may go. The caller tells
/// its core (`OpSigned`) and waits, at most [`WRITE_AHEAD_WAIT_MS`], for the
/// core's word that the record is on disk (`ClearToPost`). `false` — no word
/// in time — sends nothing: a payment that could land with no record behind
/// it is the one thing this must never do.
pub type BeforePost<'a> = &'a dyn Fn(&str, Option<u64>) -> bool;

/// The core's clearance to POST one operation (spec 082 RJ1), handed from
/// the thread that performs `ClearToPost` to the submit waiting for it.
///
/// Armed for one hash before `OpSigned` goes out, so a clearance that arrives
/// before the wait begins is not lost, and consumed by the wait, so a second
/// attempt at the same hash (a deterministic hash does repeat — S5 and both
/// DX9 runs shared one) waits for its own record rather than the last one's.
#[derive(Debug, Default)]
pub struct Clearance {
    state: std::sync::Mutex<ClearanceState>,
    signal: std::sync::Condvar,
}

#[derive(Debug, Default)]
struct ClearanceState {
    armed: Option<String>,
    cleared: bool,
}

impl Clearance {
    /// Expect the core's clearance for `user_op_hash`, and only for it.
    pub fn arm(&self, user_op_hash: &str) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.armed = Some(user_op_hash.to_lowercase());
        state.cleared = false;
    }

    /// The core cleared `user_op_hash`. A clearance for anything but the
    /// armed hash is somebody else's and changes nothing.
    pub fn clear(&self, user_op_hash: &str) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state
            .armed
            .as_deref()
            .is_some_and(|armed| armed.eq_ignore_ascii_case(user_op_hash))
        {
            state.cleared = true;
            self.signal.notify_all();
        }
    }

    /// Wait at most `within` for the armed hash's clearance, and disarm.
    #[must_use]
    pub fn wait(&self, within: std::time::Duration) -> bool {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let (mut state, _) = self
            .signal
            .wait_timeout_while(state, within, |state| !state.cleared)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cleared = state.cleared;
        state.armed = None;
        state.cleared = false;
        cleared
    }

    /// The whole write-ahead for one operation: arm, tell the core (`tell`
    /// sends `OpSigned`), and wait [`WRITE_AHEAD_WAIT_MS`] for its word.
    #[must_use]
    pub fn write_ahead(&self, user_op_hash: &str, tell: impl FnOnce()) -> bool {
        self.arm(user_op_hash);
        tell();
        self.wait(std::time::Duration::from_millis(u64::from(
            WRITE_AHEAD_WAIT_MS,
        )))
    }
}

/// One failed relay estimate, as the core reads it (RJ19), in the log: which
/// chain, and whether the relay said the call reverts — with its reason,
/// sanitised by the core — or simply gave no estimate.
fn log_estimate_failure(area: &str, chain_id: u32, error: &relay::EstimateError) {
    match error.classified() {
        EstimateFailure::Reverts { reason } => vlog!(
            area,
            "relay estimate reverts chain={chain_id} reason={}",
            reason.as_deref().unwrap_or("none given")
        ),
        EstimateFailure::Unavailable => {
            vlog!(area, "relay estimate unavailable chain={chain_id}: {error}")
        }
    }
}

fn other(message: impl Into<String>) -> SubmitFailure {
    SubmitFailure::Other(message.into())
}

/// The fee the confirm screen displayed, in the fee asset's own base units.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuotedFee {
    pub amount: u128,
    pub recipient: String,
    /// The speed the displayed fee was priced at, named on the wire beside
    /// it (spec 069) — the core took it from the same estimate as `amount`.
    /// `None` names nothing: the pre-068 wire.
    pub tier: Option<FeeTier>,
}

/// The passkey ceremony, as this path sees it: a challenge in, an assertion
/// (or the reason there is none) out. The send host supplies it with the
/// screen's ceremony channel bound; the parallel space's signer answers the
/// same shape.
pub type SignFn<'a> = &'a mut dyn FnMut(&[u8]) -> Result<Assertion, PasskeyFailure>;

/// Who signs this request (spec 071).
pub enum Signer<'a> {
    /// A passkey, over the digest alone.
    Passkey(SignFn<'a>),
    /// The Trusted Signer: the request as the page is told it, the page, the
    /// screen's channel to its waiting sheet, and the one key the page may
    /// sign with — the account's sign-in key (founder, 2026-09-26). `None`
    /// for a record from before it, whose every founding key may answer.
    TrustedSigner {
        ask: &'a Ask,
        page: &'a str,
        channel: &'a Channel,
        only: Option<&'a str>,
    },
}

impl Signer<'_> {
    /// One signature over `digest`. `operation` is what the Trusted Signer
    /// shows and derives the digest from — the assembled operation and the
    /// calls before its fee leg; `None` for a message. `edges` hears the
    /// prompt open and the signature come back, whichever signer it is;
    /// `asked` says whether the page that asked is still there (RB2).
    #[allow(
        clippy::too_many_arguments,
        reason = "the digest, what the page is shown, who hears the edges and whether it is still wanted"
    )]
    fn sign(
        &mut self,
        digest: &[u8],
        chain_id: u32,
        safe: &str,
        keys: &[WalletKey],
        operation: Option<(&UserOperation, &[MultiSendCall])>,
        edges: Edges<'_>,
        asked: Asked<'_>,
    ) -> Result<Assertion, SubmitFailure> {
        // No prompt for a page that has gone (RB2).
        if !asked() {
            return Err(SubmitFailure::AskerGone);
        }
        edges(CeremonyEdge::Started);
        let signed = self.ceremony(digest, chain_id, safe, keys, operation)?;
        // The page left while the prompt was up: the signature is dropped
        // here, so the relay POST after it never happens.
        if !asked() {
            return Err(SubmitFailure::AskerGone);
        }
        edges(CeremonyEdge::Signed);
        Ok(signed)
    }

    fn ceremony(
        &mut self,
        digest: &[u8],
        chain_id: u32,
        safe: &str,
        keys: &[WalletKey],
        operation: Option<(&UserOperation, &[MultiSendCall])>,
    ) -> Result<Assertion, SubmitFailure> {
        match self {
            Self::Passkey(sign) => sign(digest),
            Self::TrustedSigner {
                ask,
                page,
                channel,
                only,
            } => {
                // The page is offered that key alone, and no other key's
                // answer is taken — the same rule a passkey ceremony pinned
                // to it keeps.
                let allowed: Vec<WalletKey> = match only {
                    Some(credential) => keys
                        .iter()
                        .filter(|key| key.credential_id.eq_ignore_ascii_case(credential))
                        .cloned()
                        .collect(),
                    None => keys.to_vec(),
                };
                let request = ask.request(chain_id, safe, &allowed, operation);
                trusted_signer::sign(&request, page, digest, &allowed, channel)
            }
        }
        .map_err(|failure| match failure.kind {
            FailureKind::Cancelled => SubmitFailure::PasskeyCancelled,
            _ => other(
                failure
                    .message
                    .unwrap_or_else(|| "the passkey ceremony failed".to_owned()),
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// The key set and the call codec
// ---------------------------------------------------------------------------

/// A stored account's founding keys; a legacy record projects its scalar
/// fields as the sole key (`keySetOf`).
pub fn key_set_of(account: &Account) -> Vec<WalletKey> {
    if account.keys.is_empty() {
        return vec![WalletKey {
            credential_id: account.id.clone(),
            public_key_hex: account.public_key_hex.clone(),
        }];
    }
    account
        .keys
        .iter()
        .map(|key| WalletKey {
            credential_id: key.credential_id.clone(),
            public_key_hex: key.public_key_hex.clone(),
        })
        .collect()
}

/// The core's call — base units as a DECIMAL string, `0x`-hex data — onto
/// the MultiSend call, whose value is HEX (`toShellCall`, spec 026 D25). The
/// one codec that decides whether the displayed amount is the signed amount.
pub fn to_multi_send_call(call: &FeeCall) -> Result<MultiSendCall, String> {
    let value: u128 = call
        .value
        .trim()
        .parse()
        .map_err(|_| format!("call value is not a base-unit integer: `{}`", call.value))?;
    let data = if call.data.is_empty() || call.data == "0x" {
        Vec::new()
    } else {
        from_hex(&call.data).map_err(|e| format!("call data is not hex: {e}"))?
    };
    Ok(MultiSendCall {
        to: call.to.clone(),
        value_hex: format!("0x{value:x}"),
        data,
    })
}

// ---------------------------------------------------------------------------
// Estimation for the quote (`simulateUserOpGas`)
// ---------------------------------------------------------------------------

/// Simulate the operation the core built — the person's calls plus the fee
/// leg the core already appended — and return the relay's RAW limits. Every
/// padding rule is `fee_policy`'s. `ContextUnavailable` is the shell unable
/// to build a TRUTHFUL dummy op (a failed nonce read, an undeployed account
/// without its keys); `Refused` is a truthful op the relay ANSWERED fails
/// when it runs (spec 083 fee); `SimulationFailed` is no answer to use — the
/// relay out of reach, or an error that says nothing about the operation.
pub fn simulate_gas(
    chain_id: u32,
    account: &str,
    deployed: bool,
    calls: &[FeeCall],
    key_hexes: &[String],
) -> FeeGasOutcome {
    let (nonce, init_code) = if deployed {
        match chain::nonce(account, chain_id) {
            Ok(nonce) => (nonce, Vec::new()),
            Err(_) => return FeeGasOutcome::ContextUnavailable,
        }
    } else {
        match build_init_code_for_keys(key_hexes) {
            Ok(init_code) => ("0x0".to_owned(), init_code),
            Err(_) => return FeeGasOutcome::ContextUnavailable,
        }
    };
    let Ok(calls) = calls
        .iter()
        .map(to_multi_send_call)
        .collect::<Result<Vec<_>, _>>()
    else {
        return FeeGasOutcome::ContextUnavailable;
    };
    // Submission always wraps the calls in a MultiSend, so the estimate does
    // too — byte-identical to what the submit builds.
    let (Ok(call_data), Ok(signature)) = (
        build_multi_send_execute_call_data(&calls),
        build_dummy_signature(),
    ) else {
        return FeeGasOutcome::ContextUnavailable;
    };
    let op = UserOperation {
        sender: account.to_owned(),
        nonce,
        init_code,
        call_data,
        verification_gas_limit: if deployed {
            VERIFICATION_GAS_DEPLOYED
        } else {
            VERIFICATION_GAS_UNDEPLOYED
        },
        call_gas_limit: CALL_GAS_LIMIT,
        pre_verification_gas: PRE_VERIFICATION_GAS,
        // Every Vela operation pays maxFeePerGas = 0 and is reimbursed in band.
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        paymaster_and_data: Vec::new(),
        signature,
    };
    match relay::estimate_user_op_gas(&op, chain_id) {
        Ok(estimate) => FeeGasOutcome::Estimated {
            verification_gas_limit: estimate.verification_gas_limit.to_string(),
            call_gas_limit: estimate.call_gas_limit.to_string(),
            pre_verification_gas: estimate.pre_verification_gas.to_string(),
        },
        Err(error) if error.refuses() => {
            vlog!(
                "fee",
                "chain={chain_id} the relay says the operation fails: {}",
                error.message
            );
            FeeGasOutcome::Refused
        }
        Err(error) => {
            log_estimate_failure("fee", chain_id, &error);
            FeeGasOutcome::SimulationFailed
        }
    }
}

// ---------------------------------------------------------------------------
// Submit
// ---------------------------------------------------------------------------

/// The whole sign→submit orchestration (`sendBatchCalls`): Tempo pays gas
/// in its stablecoin, every other chain settles in band. Answers the
/// operation that left the device — accepted, or may have been sent — or
/// why nothing did. `before_post` is the write-ahead (RJ1): nothing is
/// POSTed until it says the record is on disk.
#[allow(
    clippy::too_many_arguments,
    reason = "the operation, its signer, who hears the ceremony, whether it is still wanted and the write-ahead"
)]
pub fn submit(
    chain_id: u32,
    safe: &str,
    calls: &[FeeCall],
    gas_fee_token: Option<&str>,
    keys: &[WalletKey],
    signer: Signer<'_>,
    quoted_fee: Option<QuotedFee>,
    edges: Edges<'_>,
    asked: Asked<'_>,
    before_post: BeforePost<'_>,
) -> Result<Submitted, SubmitFailure> {
    let outcome = submit_inner(
        chain_id,
        safe,
        calls,
        gas_fee_token,
        keys,
        signer,
        quoted_fee,
        Tail {
            head: None,
            edges,
            asked,
            before_post,
        },
    );
    // The SCREEN gets the core's sentence — SC-305: no relay text reaches a
    // person. The OPERATOR gets the detail, because a submit that failed
    // before the relay leaves no other trace at all: the log showed the quote
    // being signed and then nothing, and "your funds are safe, try again" is
    // the same words whether the passkey was refused, the hash would not
    // compute, or the account is undeployed. One line, at the one place every
    // failure passes through.
    // A relay refusal already has its `submit verdict` line; this one is for
    // everything that failed before the relay (or instead of it).
    if let Err(failure) = &outcome
        && *failure != SubmitFailure::NotSent
    {
        vlog!("relay", "submit failed: {failure:?}");
    }
    outcome
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the arguments are the operation"
)]
fn submit_inner(
    chain_id: u32,
    safe: &str,
    calls: &[FeeCall],
    gas_fee_token: Option<&str>,
    keys: &[WalletKey],
    signer: Signer<'_>,
    quoted_fee: Option<QuotedFee>,
    tail: Tail<'_>,
) -> Result<Submitted, SubmitFailure> {
    let inner: Vec<MultiSendCall> = calls
        .iter()
        .map(to_multi_send_call)
        .collect::<Result<_, _>>()
        .map_err(other)?;
    // The head, read while the ceremony runs (spec 082 ruling 8): where the
    // tracker starts looking for the operation's own event if the relay's
    // reply is lost. Best effort and bounded — a chain that cannot answer
    // leaves it unknown, and the tracker then scans back from the head.
    let head = std::thread::Builder::new()
        .name("vela-submit-head".to_owned())
        .spawn(move || chain::head_block(chain_id))
        .ok();
    let tail = Tail { head, ..tail };
    if is_tempo_chain(chain_id) {
        let fee_token = gas_fee_token.unwrap_or(TEMPO_DEFAULT_FEE_TOKEN);
        return submit_tempo(
            chain_id, safe, &inner, fee_token, keys, signer, quoted_fee, tail,
        );
    }
    submit_in_band(
        chain_id,
        safe,
        &inner,
        gas_fee_token,
        keys,
        signer,
        quoted_fee,
        tail,
    )
}

/// What the shared tail needs besides the operation: the head read started
/// before the ceremony, who hears the ceremony's edges, whether the page
/// that asked is still there (RB2), and the write-ahead (RJ1).
struct Tail<'a> {
    head: Option<std::thread::JoinHandle<Option<u64>>>,
    edges: Edges<'a>,
    asked: Asked<'a>,
    before_post: BeforePost<'a>,
}

/// Deployment status and the nonce, together, with the two refusals.
fn account_context(
    chain_id: u32,
    safe: &str,
    keys: &[WalletKey],
) -> Result<(bool, String, Vec<u8>), SubmitFailure> {
    chain::verify_chain_ready(chain_id).map_err(other)?;
    // 083: the account's previous operation lands first — before the
    // deployment read (it may be the deploying one) and the nonce.
    let floor = settle_previous(chain_id, safe, |previous| {
        landing::await_landing(previous, chain_id, PREVIOUS_OP_WAIT)
    })?;
    chain::forget_gas_price(chain_id);
    // …and the fee session's held readings (issue 212): a submit, and the
    // first quote after it, landed or not, measure again.
    crate::executor::fee_signals::invalidate(chain_id);
    let deployed = chain::is_deployed(safe, chain_id).map_err(other)?;
    let nonce = chain::nonce(safe, chain_id);
    // A deployed wallet MUST sign its real nonce; an undeployed one's IS 0.
    let nonce = if deployed {
        at_least(nonce.map_err(other)?, floor)
    } else {
        "0x0".to_owned()
    };
    let init_code = if deployed {
        Vec::new()
    } else {
        let hexes: Vec<String> = keys.iter().map(|k| k.public_key_hex.clone()).collect();
        build_init_code_for_keys(&hexes).map_err(|e| other(e.to_string()))?
    };
    Ok((deployed, nonce, init_code))
}

fn find_quote<'a>(
    quotes: &'a [FeeAssetQuote],
    fee_token: Option<&str>,
) -> Option<&'a FeeAssetQuote> {
    let wanted = fee_token.map(str::to_lowercase);
    quotes.iter().find(|quote| match &wanted {
        Some(token) => {
            quote.asset == FeeAssetKind::Erc20
                && quote.fee_token.as_deref().map(str::to_lowercase).as_deref() == Some(token)
        }
        None => quote.asset == FeeAssetKind::Native,
    })
}

fn pricing(quote: &FeeAssetQuote) -> AssetPricing {
    AssetPricing {
        is_native: quote.asset == FeeAssetKind::Native,
        decimals: quote.decimals,
        usd_price: quote.usd_price.clone(),
        native_usd_floor_price: quote.native_usd_floor_price.clone(),
    }
}

/// A displayed quote is usable when it is positive and names a real address.
fn usable(quoted: Option<QuotedFee>) -> Option<QuotedFee> {
    quoted.filter(|fee| {
        fee.amount > 0
            && fee.recipient.len() == 42
            && fee.recipient.starts_with("0x")
            && fee.recipient[2..].bytes().all(|b| b.is_ascii_hexdigit())
    })
}

/// The send-time quote for a caller that displayed none (`sendUserOpInBand`'s
/// fallback branch): the relay's rows, its outer quote checked against the
/// chain's own price, and `fee_policy`'s amount rule.
fn fallback_fee(
    chain_id: u32,
    safe: &str,
    gas_fee_token: Option<&str>,
    total_gas: u128,
) -> Result<QuotedFee, SubmitFailure> {
    let quotes = relay::in_band_quotes(chain_id, safe).unwrap_or_default();
    let (Some(quote), Some(native)) = (
        find_quote(&quotes, gas_fee_token),
        find_quote(&quotes, None),
    ) else {
        return Err(if gas_fee_token.is_some() {
            other(
                "The gas relayer cannot accept the selected fee token right now. Please pick a different gas asset.",
            )
        } else {
            SubmitFailure::RelayerUnavailable
        });
    };
    // `getBundlerGasQuote`'s acceptance: a zero maxFee is no quote; an absent
    // network field falls back to the chain's own price.
    let chain_price = chain::chain_gas_price(chain_id).gas_price;
    let reported_network = relay::raw_bundler_quote(chain_id, FeeTier::Fast)
        .filter(|quote| {
            quote
                .max_fee_per_gas
                .parse::<u128>()
                .is_ok_and(|fee| fee > 0)
        })
        .map(|quote| {
            quote
                .network_fee_per_gas
                .and_then(|text| text.parse::<u128>().ok())
                .filter(|fee| *fee > 0)
                .unwrap_or(chain_price)
        })
        .unwrap_or(0);
    if reported_network > 0
        && chain_price > 0
        && reported_network > chain_price.saturating_mul(MAX_QUOTE_VS_CHAIN_MULTIPLE)
    {
        return Err(other(
            "The gas relayer quoted a price far above the current network rate. Please try again.",
        ));
    }
    let gas_price = reported_network.max(chain_price);
    let amount =
        calculate_in_band_fee_amount(total_gas, gas_price, &pricing(quote), &pricing(native))
            .ok_or_else(|| other("Could not calculate the selected gas fee. Please try again."))?;
    vlog!(
        "in-band",
        "fallback quote: token={} amount={amount} recipient={} gas={total_gas} price={gas_price}",
        gas_fee_token.unwrap_or("native"),
        quote.recipient
    );
    Ok(QuotedFee {
        amount,
        recipient: quote.recipient.clone(),
        // Nothing was displayed, so no speed was chosen: the relay keeps its
        // own pace, exactly as before spec 068.
        tier: None,
    })
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the arguments are the operation"
)]
fn submit_in_band(
    chain_id: u32,
    safe: &str,
    inner: &[MultiSendCall],
    gas_fee_token: Option<&str>,
    keys: &[WalletKey],
    signer: Signer<'_>,
    quoted_fee: Option<QuotedFee>,
    tail: Tail<'_>,
) -> Result<Submitted, SubmitFailure> {
    let (deployed, nonce, init_code) = account_context(chain_id, safe, keys)?;

    // The batch: the person's calls + one fee leg. Estimated with a
    // PLACEHOLDER leg whose recipient is the Safe itself — a self-transfer
    // always succeeds and has the real leg's exact calldata shape, where
    // `transfer(0x0, …)` would revert and poison every stablecoin estimate.
    let batch = |amount: u128, recipient: &str| -> Result<Vec<u8>, SubmitFailure> {
        let mut calls = inner.to_vec();
        calls.push(
            build_in_band_fee_leg(gas_fee_token, recipient, amount)
                .map_err(|e| other(e.to_string()))?,
        );
        build_multi_send_execute_call_data(&calls).map_err(|e| other(e.to_string()))
    };
    let verification_floor = if deployed {
        VERIFICATION_GAS_DEPLOYED
    } else {
        VERIFICATION_GAS_UNDEPLOYED
    };
    let mut op = UserOperation {
        sender: safe.to_owned(),
        nonce,
        init_code,
        call_data: batch(1, safe)?,
        verification_gas_limit: verification_floor,
        call_gas_limit: CALL_GAS_LIMIT,
        pre_verification_gas: PRE_VERIFICATION_GAS,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        paymaster_and_data: Vec::new(),
        signature: build_dummy_signature().map_err(|e| other(e.to_string()))?,
    };

    // A batch carrying a real contract call can burn far more than the
    // defaults; if its estimate fails the op must not be submitted to OOG
    // on-chain. Pure transfers keep the defaults, which cover them.
    let has_contract_call = inner.iter().any(|call| !is_plain_transfer_call(&call.data));
    match relay::estimate_user_op_gas(&op, chain_id) {
        Ok(estimate) => {
            let padded = pad_gas_estimate(estimate, verification_floor, CALL_GAS_LIMIT);
            op.verification_gas_limit = padded.verification_gas_limit;
            op.call_gas_limit = padded.call_gas_limit;
            op.pre_verification_gas = padded.pre_verification_gas;
            raise_to_measured_floor(&mut op, chain_id, safe, inner, has_contract_call);
        }
        Err(error) => {
            log_estimate_failure("in-band", chain_id, &error);
            if has_contract_call {
                return Err(other(
                    "Could not estimate gas for this transaction. The network may be busy — please try again.",
                ));
            }
        }
    }

    let fee = match usable(quoted_fee) {
        Some(fee) => {
            vlog!(
                "in-band",
                "signing DISPLAYED quote token={} amount={} recipient={}",
                gas_fee_token.unwrap_or("native"),
                fee.amount,
                fee.recipient
            );
            fee
        }
        None => {
            let total_gas = op.verification_gas_limit + op.call_gas_limit + op.pre_verification_gas;
            fallback_fee(chain_id, safe, gas_fee_token, total_gas)?
        }
    };
    op.call_data = batch(fee.amount, &fee.recipient)?;

    // The displayed quote's speed, or none for the fallback nobody saw.
    sign_and_submit(op, chain_id, safe, inner, keys, signer, &[], fee.tier, tail)
}

#[allow(
    clippy::too_many_arguments,
    reason = "one call site; the arguments are the operation"
)]
fn submit_tempo(
    chain_id: u32,
    safe: &str,
    inner: &[MultiSendCall],
    fee_token: &str,
    keys: &[WalletKey],
    signer: Signer<'_>,
    quoted_fee: Option<QuotedFee>,
    tail: Tail<'_>,
) -> Result<Submitted, SubmitFailure> {
    chain::verify_chain_ready(chain_id).map_err(other)?;
    // The reimbursement recipient MUST come from the relay that submits.
    let collector = relay::account_info(chain_id, safe)
        .and_then(|info| info.fee_recipient())
        .filter(|address| address.len() == 42 && address.starts_with("0x"))
        .ok_or_else(|| {
            other("The Tempo gas relayer is unavailable right now. Please try again.")
        })?;

    let (deployed, nonce, init_code) = account_context(chain_id, safe, keys)?;
    let gas_price = chain::chain_gas_price(chain_id).gas_price;
    let sub_calls = u32::try_from(inner.len() + 1).unwrap_or(u32::MAX);
    let static_gas = tempo_expected_gas(deployed, sub_calls);
    let call_floor = tempo_call_gas_limit(sub_calls);
    let verification_floor = if deployed {
        VERIFICATION_GAS_DEPLOYED
    } else {
        TEMPO_VERIFICATION_GAS_UNDEPLOYED
    };

    let batch = |reimbursement: u128| -> Result<Vec<u8>, SubmitFailure> {
        let mut calls = inner.to_vec();
        calls.push(MultiSendCall {
            to: fee_token.to_owned(),
            value_hex: "0".to_owned(),
            data: encode_erc20_transfer(&collector, reimbursement)
                .map_err(|e| other(e.to_string()))?,
        });
        build_multi_send_execute_call_data(&calls).map_err(|e| other(e.to_string()))
    };
    let mut op = UserOperation {
        sender: safe.to_owned(),
        nonce,
        init_code,
        call_data: batch(1)?,
        verification_gas_limit: verification_floor,
        call_gas_limit: call_floor,
        pre_verification_gas: PRE_VERIFICATION_GAS,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        paymaster_and_data: Vec::new(),
        signature: build_dummy_signature().map_err(|e| other(e.to_string()))?,
    };

    let has_contract_call = inner.iter().any(|call| !is_plain_transfer_call(&call.data));
    let mut estimated_gas: Option<u128> = None;
    match relay::estimate_user_op_gas(&op, chain_id) {
        Ok(estimate) => {
            estimated_gas = Some(
                estimate.verification_gas_limit
                    + estimate.call_gas_limit
                    + estimate.pre_verification_gas,
            );
            let padded = pad_gas_estimate(estimate, verification_floor, call_floor);
            op.verification_gas_limit = padded.verification_gas_limit;
            op.call_gas_limit = padded.call_gas_limit;
            op.pre_verification_gas = padded.pre_verification_gas;
            raise_to_measured_floor(&mut op, chain_id, safe, inner, has_contract_call);
        }
        Err(error) => {
            log_estimate_failure("tempo", chain_id, &error);
            if has_contract_call {
                return Err(other(
                    "Could not estimate gas for this transaction. The network may be busy — please try again.",
                ));
            }
        }
    }

    // Price the reimbursement off the realistic gas — the relay's estimate
    // when there is one, never below the static model.
    let realistic_gas = estimated_gas.map_or(static_gas, |est| est.max(static_gas));
    let calculated = tempo_reimbursement(realistic_gas, gas_price, TEMPO_FEE_TOKEN_DECIMALS);
    if let Some(quoted) = &quoted_fee {
        if !quoted.recipient.eq_ignore_ascii_case(&collector)
            || quoted.amount < tempo_minimum_fee_token_units(TEMPO_FEE_TOKEN_DECIMALS)
        {
            return Err(other(
                "The gas quote has expired. Please review the updated fee and try again.",
            ));
        }
    }
    // The relay ignores a speed on Tempo (no priority fee to buy), but the
    // name still travels with the fee it was shown beside, as on the web.
    let tier = quoted_fee.as_ref().and_then(|fee| fee.tier);
    let reimbursement = usable(quoted_fee).map_or(calculated, |fee| fee.amount);
    op.call_data = batch(reimbursement)?;
    vlog!(
        "tempo",
        "feeToken={fee_token} reimbursement={reimbursement} calculated={calculated} realisticGas={realistic_gas} collector={collector}"
    );

    sign_and_submit(
        op,
        chain_id,
        safe,
        inner,
        keys,
        signer,
        &[("feeToken", fee_token)],
        tier,
        tail,
    )
}

/// The shared tail: hash, sign, envelope, submit through the core's verdict
/// (spec 082 RA1), and bump the nonce only when the relay took it. `inner`
/// is the person's calls — the operation carries them and then its fee leg.
#[allow(
    clippy::too_many_arguments,
    reason = "the operation, its calls, its signer, and the two things the wire names beside it"
)]
fn sign_and_submit(
    mut op: UserOperation,
    chain_id: u32,
    safe: &str,
    inner: &[MultiSendCall],
    keys: &[WalletKey],
    mut signer: Signer<'_>,
    extra: &[(&str, &str)],
    tier: Option<FeeTier>,
    tail: Tail<'_>,
) -> Result<Submitted, SubmitFailure> {
    let safe_op_hash =
        calculate_safe_op_hash(&op, u64::from(chain_id)).map_err(|e| other(e.to_string()))?;
    let assertion = signer.sign(
        &safe_op_hash,
        chain_id,
        safe,
        keys,
        Some((&op, inner)),
        tail.edges,
        tail.asked,
    )?;
    op.signature = envelope(&assertion, keys)?;

    // The AA20 guard: an undeployed sender with no initCode is a guaranteed
    // on-chain failure that strands funds silently. The read is cache-hot on
    // a deployed account, so this costs nothing where it does not fire.
    if op.init_code.len() < 20 && !chain::is_deployed(safe, chain_id).map_err(other)? {
        return Err(other(
            "This account is not deployed on this network yet and the transaction is missing its deployment step. Please try again.",
        ));
    }

    // Known before the first POST (RA6): the name the operation is followed
    // by when the relay's reply never comes back. The signature is not part
    // of it, so this is the hash of exactly what goes out.
    let local_hash = user_op_hash(&op, u64::from(chain_id)).map_err(|e| other(e.to_string()))?;
    let submit_block = tail.head.and_then(|head| head.join().ok()).flatten();
    clear_and_post(
        &local_hash,
        submit_block,
        tail.before_post,
        tail.asked,
        // 083 S3b: another operation of the account holds the nonce — wait
        // for it to land, then send this one again, as signed.
        || {
            after_collision(
                relay::send_user_op(&op, chain_id, extra, tier, &local_hash),
                |previous| landing::await_landing(previous, chain_id, PREVIOUS_OP_WAIT),
                || relay::send_user_op(&op, chain_id, extra, tier, &local_hash),
            )
        },
        // RA5: the local nonce moves only for an operation the relay holds.
        // One that may have been sent keeps nonce N, so a second attempt can
        // never become a second payment: the EntryPoint lets at most one
        // operation with N land.
        |accepted| {
            chain::bump_nonce(safe, chain_id);
            // The next submit for this account waits for this one (083).
            note_submitted(chain_id, safe, accepted, &op.nonce);
        },
    )
}

/// The last steps before the bytes leave, in the order the money rules set:
/// the write-ahead (RJ1: the record is on disk, or nothing is sent), then
/// the asker check (RB2: nobody is left to answer, so nothing is sent), then
/// the POST — its verdict the core's (`post`, a seam for the tests) — and
/// the nonce bump for an operation the relay took (`accepted`).
fn clear_and_post(
    local_hash: &str,
    submit_block: Option<u64>,
    before_post: BeforePost<'_>,
    asked: Asked<'_>,
    post: impl FnOnce() -> SubmitVerdict,
    accepted: impl FnOnce(&str),
) -> Result<Submitted, SubmitFailure> {
    if !before_post(local_hash, submit_block) {
        vlog!(
            "relay",
            "op={} not sent: its record was not written within {WRITE_AHEAD_WAIT_MS} ms",
            short(local_hash)
        );
        return Err(SubmitFailure::NotSent);
    }
    // The last moment a page that left can still stop it (RB2): past this
    // line the operation may reach the relay, and then it is recorded and
    // followed like any other.
    if !asked() {
        vlog!(
            "relay",
            "op={} not sent: the page that asked is gone",
            short(local_hash)
        );
        return Err(SubmitFailure::AskerGone);
    }
    match post() {
        SubmitVerdict::Accepted { user_op_hash } => {
            accepted(&user_op_hash);
            Ok(Submitted {
                user_op_hash,
                maybe_sent: false,
                submit_block,
            })
        }
        SubmitVerdict::MaybeSent { user_op_hash } => Ok(Submitted {
            user_op_hash,
            maybe_sent: true,
            submit_block,
        }),
        SubmitVerdict::NotSent { rejection } => Err(match rejection {
            None => SubmitFailure::NotSent,
            Some(RelayRejection::RelayerUnavailable) => SubmitFailure::RelayerUnavailable,
            Some(RelayRejection::BundlerUnderfunded) => SubmitFailure::BundlerUnderfunded,
            Some(RelayRejection::NonceHeld { .. }) => other(PREVIOUS_PENDING),
            Some(RelayRejection::Other(message)) => SubmitFailure::Refused(message),
        }),
    }
}

// ---------------------------------------------------------------------------
// This account's previous operation (083)
// ---------------------------------------------------------------------------

/// How long a submit waits for the account's previous operation to land —
/// the tracker's active window — before it gives up and says so.
const PREVIOUS_OP_WAIT: Duration = Duration::from_millis(WAIT_WINDOW_MS as u64);

/// What a request that could not go out behind another of the account's
/// operations tells its page — for the developer; the screen has its own
/// words.
pub const PREVIOUS_PENDING: &str = vela_core::user_op::PREVIOUS_PENDING_DETAIL;

/// The last operation this app submitted for an account on a chain, and
/// whether it has been seen to land (spent its nonce).
#[derive(Clone, Debug, PartialEq, Eq)]
struct Previous {
    user_op_hash: String,
    nonce: u128,
    landed: bool,
}

static PREVIOUS: Mutex<Option<HashMap<String, Previous>>> = Mutex::new(None);

fn account_key(chain_id: u32, safe: &str) -> String {
    format!("{chain_id}:{}", safe.to_lowercase())
}

fn note_submitted(chain_id: u32, safe: &str, user_op_hash: &str, nonce: &str) {
    let Ok(nonce) = parse_hex_quantity(Some(nonce)) else {
        return;
    };
    if let Ok(mut previous) = PREVIOUS.lock() {
        previous.get_or_insert_with(HashMap::new).insert(
            account_key(chain_id, safe),
            Previous {
                user_op_hash: user_op_hash.to_owned(),
                nonce,
                landed: false,
            },
        );
    }
}

/// An operation was seen to land — included, executed or reverted: its
/// nonce is spent. Told by whoever sees its receipt (the dApp's landing
/// wait, the tracker).
pub fn note_landed(user_op_hash: &str) {
    if let Ok(mut previous) = PREVIOUS.lock()
        && let Some(map) = previous.as_mut()
    {
        for entry in map.values_mut() {
            if entry.user_op_hash.eq_ignore_ascii_case(user_op_hash) {
                entry.landed = true;
            }
        }
    }
}

fn previous_of(chain_id: u32, safe: &str) -> Option<Previous> {
    PREVIOUS
        .lock()
        .ok()?
        .as_ref()?
        .get(&account_key(chain_id, safe))
        .cloned()
}

/// Forget the account's previous operation — only if it is still `hash`.
fn forget_previous(chain_id: u32, safe: &str, hash: &str) {
    if let Ok(mut previous) = PREVIOUS.lock()
        && let Some(map) = previous.as_mut()
    {
        let key = account_key(chain_id, safe);
        if map
            .get(&key)
            .is_some_and(|entry| entry.user_op_hash.eq_ignore_ascii_case(hash))
        {
            map.remove(&key);
        }
    }
}

/// Before a new operation is built (083, S3b): the account's previous one —
/// Uniswap's approval, when its swap comes next — must land first, or the
/// new one is signed over the same nonce and the relay refuses it. Waits up
/// to [`PREVIOUS_OP_WAIT`] (the column says "preparing" meanwhile), and
/// answers the nonce the new operation may not go below: the previous one's
/// plus one, whatever a lagging node says.
///
/// Not landed by then: this request fails clearly, before any prompt — and
/// the account is let go, so the next attempt asks the relay rather than
/// waiting on an operation that may never land.
fn settle_previous(
    chain_id: u32,
    safe: &str,
    wait: impl FnOnce(&str) -> Option<Landing>,
) -> Result<Option<u128>, SubmitFailure> {
    let Some(previous) = previous_of(chain_id, safe) else {
        return Ok(None);
    };
    if !previous.landed {
        eprintln!(
            "[vela-wallet] submit: waiting for this account's previous operation {} to land",
            previous.user_op_hash
        );
        match wait(&previous.user_op_hash) {
            Some(Landing::Landed(_) | Landing::Reverted(_)) => {
                note_landed(&previous.user_op_hash);
            }
            // Refused: it will never spend its nonce.
            Some(Landing::Refused) => {
                forget_previous(chain_id, safe, &previous.user_op_hash);
                return Ok(None);
            }
            None => {
                forget_previous(chain_id, safe, &previous.user_op_hash);
                return Err(other(PREVIOUS_PENDING));
            }
        }
    }
    Ok(Some(previous.nonce.saturating_add(1)))
}

/// The nonce read, raised to `floor` when it is below it.
fn at_least(nonce: String, floor: Option<u128>) -> String {
    match (parse_hex_quantity(Some(&nonce)), floor) {
        (Ok(read), Some(floor)) if read < floor => format!("0x{floor:x}"),
        _ => nonce,
    }
}

/// The relay refused a signed operation because another of the account's
/// is already pending at its nonce (083, S3b; the core's
/// `RelayRejection::NonceHeld`). That operation's hash is NEVER this
/// request's answer — answered with it, Uniswap reported a swap done when
/// only its approval had happened. So wait for it to land
/// ([`PREVIOUS_OP_WAIT`]), then send this one again, as signed: the relay
/// takes it when the two nonces differ, and refuses it when they were the
/// same (the earlier operation spent it). When the other never lands, or the
/// relay still refuses, nothing of this one went out and the request fails
/// clearly ([`PREVIOUS_PENDING`]). Any other verdict passes through.
fn after_collision(
    verdict: SubmitVerdict,
    wait: impl FnOnce(&str) -> Option<Landing>,
    resubmit: impl FnOnce() -> SubmitVerdict,
) -> SubmitVerdict {
    let SubmitVerdict::NotSent {
        rejection: Some(RelayRejection::NonceHeld {
            user_op_hash: previous,
        }),
    } = &verdict
    else {
        return verdict;
    };
    vlog!(
        "relay",
        "another operation of this account is pending ({}); waiting for it",
        short(previous)
    );
    match wait(previous) {
        Some(Landing::Landed(_) | Landing::Reverted(_)) => note_landed(previous),
        Some(Landing::Refused) => {}
        None => return verdict,
    }
    resubmit()
}

/// A message (EIP-1271, spec 044 on the phones): the key signs the Safe's
/// own `SafeMessage` hash over `original_hash` — a Safe verifies nothing
/// else — and the envelope carries no validity window. One ceremony,
/// nothing submitted. Answers the signature hex.
pub fn sign_message(
    chain_id: u32,
    safe: &str,
    original_hash: &[u8],
    keys: &[WalletKey],
    mut signer: Signer<'_>,
    edges: Edges<'_>,
    asked: Asked<'_>,
) -> Result<String, SubmitFailure> {
    let challenge = compute_safe_message_hash(original_hash, u64::from(chain_id), safe)
        .map_err(|e| other(e.to_string()))?;
    let assertion = signer.sign(&challenge, chain_id, safe, keys, None, edges, asked)?;
    let hex = |text: &str| from_hex(text).map_err(|e| other(e.to_string()));
    let signature = eip1271_envelope_signature(
        &hex(&assertion.authenticator_data_hex)?,
        &hex(&assertion.client_data_json_hex)?,
        &hex(&assertion.signature_der_hex)?,
        &assertion.credential_id,
        keys,
    )
    .map_err(|e| other(e.to_string()))?;
    Ok(to_hex(&signature, true))
}

/// The assertion as the Safe's contract signature: compatibility-checked,
/// DER→raw low-S, the client-data fields cut out, the verifier named by the
/// credential that signed.
fn envelope(assertion: &Assertion, keys: &[WalletKey]) -> Result<Vec<u8>, SubmitFailure> {
    let auth_data =
        from_hex(&assertion.authenticator_data_hex).map_err(|e| other(e.to_string()))?;
    let client_data =
        from_hex(&assertion.client_data_json_hex).map_err(|e| other(e.to_string()))?;
    let der = from_hex(&assertion.signature_der_hex).map_err(|e| other(e.to_string()))?;
    if let Err(reason) = validate_client_data(ClientDataKind::Get, &client_data, &auth_data) {
        return Err(other(format!(
            "Your device's identity provider is not compatible with Vela Wallet. Please switch to Google Password Manager.\n\n{reason}"
        )));
    }
    let raw = der_signature_to_raw_low_s(&der)
        .map_err(|_| other("Failed to create signature: DER to raw conversion failed"))?;
    let fields = extract_client_data_fields(&client_data);
    let signer = signer_address_for(keys, Some(&assertion.credential_id))
        .map_err(|e| other(e.to_string()))?;
    build_user_op_signature(&auth_data, &fields, &raw[..32], &raw[32..], &signer)
        .map_err(|e| other(e.to_string()))
}

/// The keys' public points, for the estimate's initCode.
pub fn key_hexes(keys: &[WalletKey]) -> Vec<String> {
    keys.iter().map(|key| key.public_key_hex.clone()).collect()
}

/// `eth_estimateGas({from, to, value, data})` for ONE inner call, from the
/// Safe's own address — the measurement the submit's floor pads and the fee
/// quote hands the core (`FeeOperation::MeasureInnerCalls`), read the same
/// way by both. `None` = nobody could measure it.
pub fn measure_call_gas(
    chain_id: u32,
    from: &str,
    to: &str,
    value_hex: &str,
    data_hex: &str,
) -> Option<u128> {
    let params = serde_json::json!([{
        "from": from,
        "to": to,
        "value": value_hex,
        "data": data_hex,
    }]);
    pool::call(chain_id, "eth_estimateGas", params)
        .ok()
        .and_then(|answer| answer.get("result")?.as_str().map(str::to_owned))
        .and_then(|hex| u128::from_str_radix(hex.trim_start_matches("0x"), 16).ok())
}

/// The inner calls' own gas floor (`vela_core::user_op::inner_calls_gas_floor`):
/// every real contract call measured from the Safe's address — a codeless
/// account estimates like any other — so an UNDEPLOYED Safe's first contract
/// call is not sent with the bundler's trivial "no code here" figure. Nothing
/// measurable (transfers only, or a chain that did not answer) keeps the
/// padded estimate and its existing guard.
fn raise_to_measured_floor(
    op: &mut UserOperation,
    chain_id: u32,
    safe: &str,
    inner: &[MultiSendCall],
    has_contract_call: bool,
) {
    if !has_contract_call {
        return;
    }
    let mut measured = Vec::new();
    for call in inner
        .iter()
        .filter(|call| !is_plain_transfer_call(&call.data))
    {
        let data = format!("0x{}", vela_core::primitives::to_hex(&call.data, false));
        let Some(gas) = measure_call_gas(chain_id, safe, &call.to, &call.value_hex, &data) else {
            return;
        };
        measured.push(gas);
    }
    match inner_calls_gas_floor(&measured, inner.len()) {
        Some(floor) if floor > op.call_gas_limit => {
            vlog!(
                "fee",
                "callGasLimit raised to the inner calls' own estimate bundler={} inner={floor}",
                op.call_gas_limit
            );
            op.call_gas_limit = floor;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::app::AccountKey;

    fn account(keys: usize) -> Account {
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
                    transports: String::new(),
                    signer_origin: None,
                })
                .collect(),
            signed_in_with: None,
        }
    }

    const OWN: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";
    const APPROVAL: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";

    fn held_by(hash: &str) -> SubmitVerdict {
        SubmitVerdict::NotSent {
            rejection: Some(RelayRejection::NonceHeld {
                user_op_hash: hash.to_owned(),
            }),
        }
    }

    fn accepted(hash: &str) -> SubmitVerdict {
        SubmitVerdict::Accepted {
            user_op_hash: hash.to_owned(),
        }
    }

    /// 083 S3b: the relay names ANOTHER pending operation of the account —
    /// Uniswap's approval, when its swap is submitted (the core's
    /// `NonceHeld`). That hash is never the swap's answer: the swap waits for
    /// the approval to land, then goes out as signed; if the relay still will
    /// not take it, the request fails clearly and nothing of it went out.
    #[test]
    fn another_pending_operation_is_never_this_requests_hash() {
        // It lands; this one goes out after it.
        let mut waited_for = None;
        let sent = after_collision(
            held_by(APPROVAL),
            |previous| {
                waited_for = Some(previous.to_owned());
                Some(Landing::Landed("0xtx".to_owned()))
            },
            || accepted(OWN),
        );
        assert_eq!(sent, accepted(OWN), "this operation's own hash");
        assert_eq!(waited_for.as_deref(), Some(APPROVAL));

        // It lands, and the relay refuses this one: the approval spent the
        // nonce it was signed with. The relay's refusal stands — not sent.
        let refused = SubmitVerdict::NotSent {
            rejection: Some(RelayRejection::Other(
                "Transaction nonce mismatch. Please try again.".to_owned(),
            )),
        };
        let spent = after_collision(
            held_by(APPROVAL),
            |_| Some(Landing::Landed("0xtx".to_owned())),
            || refused.clone(),
        );
        assert_eq!(spent, refused);

        // It never lands in the window: nothing is sent again, and the
        // request fails as held behind it.
        let mut resent = false;
        let stuck = after_collision(
            held_by(APPROVAL),
            |_| None,
            || {
                resent = true;
                accepted(OWN)
            },
        );
        assert_eq!(stuck, held_by(APPROVAL));
        assert!(!resent);

        for answer in [&sent, &spent, &stuck] {
            assert_ne!(*answer, accepted(APPROVAL), "the approval's hash");
        }
        assert!(PREVIOUS_PENDING.starts_with("Another transaction from this account"));
    }

    /// Any other verdict passes through untouched — nothing to wait for,
    /// nothing to resend.
    #[test]
    fn a_verdict_without_a_held_nonce_passes_through() {
        for verdict in [
            accepted(OWN),
            SubmitVerdict::MaybeSent {
                user_op_hash: OWN.to_owned(),
            },
            SubmitVerdict::NotSent { rejection: None },
        ] {
            let answer = after_collision(
                verdict.clone(),
                |_| unreachable!("nothing to wait for"),
                || unreachable!("nothing to resend"),
            );
            assert_eq!(answer, verdict);
        }
    }

    /// Before building: this app's previous operation for the account lands
    /// first, and the new one's nonce is never below the one after it — a
    /// node that has not seen the landing yet reads the old nonce, which the
    /// relay would refuse. Not landed in the window: a clear failure before
    /// any prompt, and the account is let go so the next attempt asks the
    /// relay. The registry is per chain and account.
    #[test]
    fn the_previous_operation_lands_before_the_next_is_built() {
        let safe = "0xAAAA000000000000000000000000000000000001";
        assert_eq!(
            settle_previous(100, safe, |_| unreachable!("nothing submitted yet")),
            Ok(None)
        );

        note_submitted(100, safe, APPROVAL, "0x7");
        let mut waited = None;
        let floor = settle_previous(100, &safe.to_lowercase(), |hash| {
            waited = Some(hash.to_owned());
            Some(Landing::Reverted("0xtx".to_owned()))
        });
        assert_eq!(floor, Ok(Some(8)), "a reverted op spent its nonce too");
        assert_eq!(waited.as_deref(), Some(APPROVAL));
        assert_eq!(
            settle_previous(100, safe, |_| unreachable!("seen landed")),
            Ok(Some(8)),
            "once landed, only the floor is left"
        );
        assert_eq!(at_least("0x7".to_owned(), Some(8)), "0x8", "a lagging node");
        assert_eq!(
            at_least(format!("0x{:064x}", 9), Some(8)),
            format!("0x{:064x}", 9)
        );
        assert_eq!(at_least("0x7".to_owned(), None), "0x7");
        assert_eq!(
            settle_previous(137, safe, |_| unreachable!("another chain")),
            Ok(None)
        );

        // The next one, seen by the tracker or the page's wait.
        note_submitted(100, safe, OWN, "0x8");
        note_landed(&OWN.to_uppercase().replace("0X", "0x"));
        assert_eq!(settle_previous(100, safe, |_| unreachable!()), Ok(Some(9)));

        // Stuck: fail clearly, and let the account go.
        note_submitted(100, safe, APPROVAL, "0x9");
        assert_eq!(
            settle_previous(100, safe, |_| None),
            Err(other(PREVIOUS_PENDING))
        );
        assert_eq!(settle_previous(100, safe, |_| unreachable!()), Ok(None));

        // Refused by the relay: its nonce is free again — no floor.
        note_submitted(100, safe, APPROVAL, "0x9");
        assert_eq!(
            settle_previous(100, safe, |_| Some(Landing::Refused)),
            Ok(None)
        );
    }

    #[test]
    fn a_legacy_account_projects_its_scalar_key() {
        let keys = key_set_of(&account(0));
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].credential_id, "cred0");
        assert_eq!(keys[0].public_key_hex, "04aa");
        let keys = key_set_of(&account(3));
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[2].credential_id, "cred2");
        assert_eq!(key_hexes(&keys), vec!["0400", "0401", "0402"]);
    }

    /// The fund-safety codec: a decimal base-unit string becomes hex, and
    /// nothing else is accepted.
    #[test]
    fn the_call_codec_turns_decimal_units_into_hex() {
        let call = FeeCall {
            to: "0x1111111111111111111111111111111111111111".to_owned(),
            value: "1000000000000000".to_owned(),
            data: "0x".to_owned(),
        };
        let shell = to_multi_send_call(&call).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(shell.value_hex, "0x38d7ea4c68000");
        assert!(shell.data.is_empty());
        let with_data = FeeCall {
            data: "0xa9059cbb".to_owned(),
            value: "0".to_owned(),
            ..call.clone()
        };
        let shell = to_multi_send_call(&with_data).unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(shell.value_hex, "0x0");
        assert_eq!(shell.data, vec![0xa9, 0x05, 0x9c, 0xbb]);
        // Hex where a decimal is owed is refused: a decimal reading of
        // "0x10" would move sixteen units and a hex one a different number.
        assert!(
            to_multi_send_call(&FeeCall {
                value: "0x10".to_owned(),
                ..call.clone()
            })
            .is_err()
        );
        assert!(
            to_multi_send_call(&FeeCall {
                value: "1.5".to_owned(),
                ..call
            })
            .is_err()
        );
    }

    #[test]
    fn a_displayed_quote_is_only_usable_when_positive_and_addressed() {
        let good = QuotedFee {
            amount: 1,
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            tier: None,
        };
        assert_eq!(usable(Some(good.clone())), Some(good.clone()));
        assert_eq!(
            usable(Some(QuotedFee {
                amount: 0,
                ..good.clone()
            })),
            None
        );
        assert_eq!(
            usable(Some(QuotedFee {
                recipient: "0x11".to_owned(),
                ..good
            })),
            None
        );
        assert_eq!(usable(None), None);
    }

    const LOCAL: &str = "0x7df211eddc000000000000000000000000000000000000000000000000000000";

    /// What one run of the last steps did, against a fake relay.
    struct Posted {
        outcome: Result<Submitted, SubmitFailure>,
        posts: u32,
        bumps: u32,
    }

    fn post_after(
        clearance: bool,
        still_asked: bool,
        verdict: SubmitVerdict,
        order: &std::cell::RefCell<Vec<&'static str>>,
    ) -> Posted {
        let posts = std::cell::Cell::new(0);
        let bumps = std::cell::Cell::new(0);
        let before_post = |hash: &str, block: Option<u64>| {
            assert_eq!(hash, LOCAL);
            assert_eq!(block, Some(48_487_620));
            order.borrow_mut().push("written ahead");
            clearance
        };
        let asked = || {
            order.borrow_mut().push("asker checked");
            still_asked
        };
        let outcome = clear_and_post(
            LOCAL,
            Some(48_487_620),
            &before_post,
            &asked,
            || {
                order.borrow_mut().push("POST");
                posts.set(posts.get() + 1);
                verdict
            },
            |_| bumps.set(bumps.get() + 1),
        );
        Posted {
            outcome,
            posts: posts.get(),
            bumps: bumps.get(),
        }
    }

    /// Spec 082 RJ1 (G34): nothing is POSTed before the core's word that the
    /// record is on disk, and the page's check comes after it, right before
    /// the POST. Accepted still moves the nonce (RA5), once.
    #[test]
    fn nothing_is_posted_before_the_record_is_written() {
        let order = std::cell::RefCell::new(Vec::new());
        let run = post_after(
            true,
            true,
            SubmitVerdict::Accepted {
                user_op_hash: LOCAL.to_owned(),
            },
            &order,
        );
        assert_eq!(
            *order.borrow(),
            vec!["written ahead", "asker checked", "POST"]
        );
        assert_eq!(run.posts, 1);
        assert_eq!(
            run.bumps, 1,
            "RA5: the nonce moves for an op the relay holds"
        );
        assert_eq!(
            run.outcome,
            Ok(Submitted {
                user_op_hash: LOCAL.to_owned(),
                maybe_sent: false,
                submit_block: Some(48_487_620),
            })
        );

        // A lost reply keeps nonce N: at most one op with it can land.
        let order = std::cell::RefCell::new(Vec::new());
        let maybe = post_after(
            true,
            true,
            SubmitVerdict::MaybeSent {
                user_op_hash: LOCAL.to_owned(),
            },
            &order,
        );
        assert_eq!((maybe.posts, maybe.bumps), (1, 0));
        assert!(maybe.outcome.is_ok_and(|op| op.maybe_sent));
    }

    /// No clearance in time: not one POST, and the failure says nothing was
    /// sent — the dApp's "relay unreachable; nothing was sent", Send's
    /// "failed, try again" (it may: nothing left).
    #[test]
    fn no_clearance_means_no_post() {
        let order = std::cell::RefCell::new(Vec::new());
        let run = post_after(
            false,
            true,
            SubmitVerdict::Accepted {
                user_op_hash: LOCAL.to_owned(),
            },
            &order,
        );
        assert_eq!((run.posts, run.bumps), (0, 0));
        assert_eq!(run.outcome, Err(SubmitFailure::NotSent));
        assert_eq!(*order.borrow(), vec!["written ahead"]);

        // The page left after the record was written: still nothing sent.
        let order = std::cell::RefCell::new(Vec::new());
        let gone = post_after(
            true,
            false,
            SubmitVerdict::Accepted {
                user_op_hash: LOCAL.to_owned(),
            },
            &order,
        );
        assert_eq!((gone.posts, gone.bumps), (0, 0));
        assert_eq!(gone.outcome, Err(SubmitFailure::AskerGone));
    }

    /// RJ3: the relay's refusal is a refusal, apart from "unavailable" and
    /// "underfunded", which keep their own ways.
    #[test]
    fn a_relay_rejection_is_a_refusal() {
        let order = std::cell::RefCell::new(Vec::new());
        let refused = post_after(
            true,
            true,
            SubmitVerdict::NotSent {
                rejection: Some(RelayRejection::Other("AA23 reverted".to_owned())),
            },
            &order,
        );
        assert_eq!(
            refused.outcome,
            Err(SubmitFailure::Refused("AA23 reverted".to_owned()))
        );
        assert_eq!(refused.bumps, 0);
        let unavailable = post_after(
            true,
            true,
            SubmitVerdict::NotSent {
                rejection: Some(RelayRejection::RelayerUnavailable),
            },
            &order,
        );
        assert_eq!(unavailable.outcome, Err(SubmitFailure::RelayerUnavailable));
        let never = post_after(
            true,
            true,
            SubmitVerdict::NotSent { rejection: None },
            &order,
        );
        assert_eq!(never.outcome, Err(SubmitFailure::NotSent));
    }

    /// The clearance is for one hash, is not lost when it comes before the
    /// wait, and is consumed: a second attempt at the same hash (S5 and both
    /// DX9 runs shared one) waits for its own record.
    #[test]
    fn a_clearance_is_for_one_hash_and_one_attempt() {
        let clearance = Clearance::default();
        clearance.arm(LOCAL);
        clearance.clear("0xsomebody-else");
        assert!(
            !clearance.wait(std::time::Duration::ZERO),
            "another op's word"
        );

        clearance.arm(LOCAL);
        clearance.clear(&LOCAL.to_uppercase().replace("0X", "0x"));
        assert!(
            clearance.wait(std::time::Duration::ZERO),
            "cleared before the wait began"
        );
        clearance.arm(LOCAL);
        assert!(
            !clearance.wait(std::time::Duration::from_millis(20)),
            "the last attempt's word is spent"
        );

        // Across threads, the way the host and the worker meet.
        let clearance = std::sync::Arc::new(Clearance::default());
        let from_host = std::sync::Arc::clone(&clearance);
        let started = std::time::Instant::now();
        let cleared = clearance.write_ahead(LOCAL, move || {
            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(50));
                from_host.clear(LOCAL);
            });
        });
        assert!(cleared);
        assert!(started.elapsed() < std::time::Duration::from_secs(2));
    }

    /// RJ19: a relay estimate that says the call reverts reads as such, and
    /// one that simply got no answer does not.
    #[test]
    fn a_failed_estimate_is_read_by_the_core() {
        let reverts = relay::EstimateError {
            message: "UserOperation reverted during simulation with reason: AA23".to_owned(),
            error_json: Some(
                r#"{"code":-32500,"message":"UserOperation reverted during simulation with reason: AA23 reverted"}"#
                    .to_owned(),
            ),
        };
        assert!(matches!(
            reverts.classified(),
            EstimateFailure::Reverts { .. }
        ));
        let lost = relay::EstimateError {
            message: "gas estimation unreachable".to_owned(),
            error_json: None,
        };
        assert_eq!(lost.classified(), EstimateFailure::Unavailable);
    }

    #[test]
    fn the_failure_vocabulary_maps_onto_the_core_s() {
        assert_eq!(
            SendSubmitFailure::from(SubmitFailure::PasskeyCancelled),
            SendSubmitFailure::PasskeyCancelled
        );
        assert_eq!(
            SendSubmitFailure::from(SubmitFailure::Other(String::new())),
            SendSubmitFailure::Other { message: None }
        );
        // Spec 082 RA10: "the relay was never reached" reaches the send
        // machine as its generic failure, carrying the fixed sentence the
        // dApp is told — never the pool's text.
        assert_eq!(
            SendSubmitFailure::from(SubmitFailure::NotSent),
            SendSubmitFailure::Other {
                message: Some(NOT_SENT_DAPP_DETAIL.to_owned())
            }
        );
    }

    /// Spec 082 RA9 (T051): the Touch ID prompt is bracketed by the two edges
    /// the sheet words its stage from — "started" before the prompt opens,
    /// "signed" only after it came back with a signature, and no "signed" for
    /// a prompt the person dismissed.
    #[test]
    fn the_passkey_is_bracketed_by_the_ceremony_edges() {
        let log = std::cell::RefCell::new(Vec::<&str>::new());
        let edges = |edge: CeremonyEdge| {
            log.borrow_mut().push(match edge {
                CeremonyEdge::Started => "started",
                CeremonyEdge::Signed => "signed",
            });
        };
        let assertion = Assertion {
            credential_id: "cred0".to_owned(),
            signature_der_hex: String::new(),
            authenticator_data_hex: String::new(),
            client_data_json_hex: String::new(),
            user_id_hex: None,
            authenticator_attachment: String::new(),
            signer_origin: None,
        };
        let mut touch = |_: &[u8]| {
            log.borrow_mut().push("touch id");
            Ok(assertion.clone())
        };
        let signed = Signer::Passkey(&mut touch).sign(
            &[7; 32],
            100,
            "0xsafe",
            &[],
            None,
            &edges,
            &always_asked,
        );
        assert!(signed.is_ok());
        assert_eq!(log.take(), vec!["started", "touch id", "signed"]);

        let mut dismissed = |_: &[u8]| {
            log.borrow_mut().push("touch id");
            Err(PasskeyFailure {
                kind: FailureKind::Cancelled,
                message: None,
            })
        };
        let declined = Signer::Passkey(&mut dismissed).sign(
            &[7; 32],
            100,
            "0xsafe",
            &[],
            None,
            &edges,
            &always_asked,
        );
        assert_eq!(declined.err(), Some(SubmitFailure::PasskeyCancelled));
        assert_eq!(log.take(), vec!["started", "touch id"]);
    }

    /// Spec 082 RB2 (money rule): nothing is signed for a page that has gone.
    /// Asked before the prompt opens — a gone page gets no prompt at all —
    /// and again when it answers: a page that left while Touch ID was up gets
    /// its signature dropped, so the relay POST that follows never happens.
    #[test]
    fn a_page_that_left_gets_nothing_signed() {
        let log = std::cell::RefCell::new(Vec::<&str>::new());
        let edges = |edge: CeremonyEdge| {
            log.borrow_mut().push(match edge {
                CeremonyEdge::Started => "started",
                CeremonyEdge::Signed => "signed",
            });
        };
        let assertion = Assertion {
            credential_id: "cred0".to_owned(),
            signature_der_hex: String::new(),
            authenticator_data_hex: String::new(),
            client_data_json_hex: String::new(),
            user_id_hex: None,
            authenticator_attachment: String::new(),
            signer_origin: None,
        };
        let here = std::cell::Cell::new(false);
        let asked = || here.get();
        let mut touch = |_: &[u8]| {
            log.borrow_mut().push("touch id");
            Ok(assertion.clone())
        };
        let gone =
            Signer::Passkey(&mut touch).sign(&[7; 32], 100, "0xsafe", &[], None, &edges, &asked);
        assert_eq!(gone.err(), Some(SubmitFailure::AskerGone));
        assert!(log.take().is_empty(), "no prompt for a page that is gone");

        // Here when the prompt opened, gone by the time it answered.
        here.set(true);
        let mut leaves = |_: &[u8]| {
            log.borrow_mut().push("touch id");
            here.set(false);
            Ok(assertion.clone())
        };
        let left =
            Signer::Passkey(&mut leaves).sign(&[7; 32], 100, "0xsafe", &[], None, &edges, &asked);
        assert_eq!(left.err(), Some(SubmitFailure::AskerGone));
        assert_eq!(log.take(), vec!["started", "touch id"]);
    }

    #[test]
    fn the_quote_row_lookup_is_by_asset_and_token() {
        let row = |asset: FeeAssetKind, token: Option<&str>| FeeAssetQuote {
            recipient: "0x1111111111111111111111111111111111111111".to_owned(),
            asset,
            fee_token: token.map(str::to_owned),
            balance: "0".to_owned(),
            decimals: 18,
            symbol: "X".to_owned(),
            usd_balance: "0".to_owned(),
            usd_price: None,
            native_usd_floor_price: None,
        };
        let usdc = "0x2222222222222222222222222222222222222222";
        let quotes = vec![
            row(FeeAssetKind::Native, None),
            row(FeeAssetKind::Erc20, Some(usdc)),
        ];
        assert_eq!(
            find_quote(&quotes, None).map(|q| q.asset),
            Some(FeeAssetKind::Native)
        );
        assert_eq!(
            find_quote(&quotes, Some(&usdc.to_uppercase())).map(|q| q.asset),
            Some(FeeAssetKind::Erc20)
        );
        assert!(find_quote(&quotes, Some("0x3333333333333333333333333333333333333333")).is_none());
    }

    /// The signature envelope is built from the parallel space's assertion —
    /// the fixture signs the SafeOp hash and the envelope names the shared
    /// verifier for a one-key wallet, the proxy for a later key.
    #[cfg(feature = "dev-fixtures")]
    #[test]
    fn the_fixture_assertion_becomes_a_contract_signature() {
        use vela_core::dev_fixtures as fixtures;
        let accounts = fixtures::accounts().unwrap_or_else(|e| unreachable!("{e}"));
        let keys: Vec<WalletKey> = accounts
            .iter()
            .map(|a| WalletKey {
                credential_id: a.credential_id_hex.clone(),
                public_key_hex: a.public_key_hex.clone(),
            })
            .collect();
        let challenge = [0x42u8; 32];
        let signed =
            fixtures::build_assertion(&accounts[1], &challenge, fixtures::RP_ID, fixtures::ORIGIN)
                .unwrap_or_else(|e| unreachable!("{e}"));
        let assertion = Assertion {
            credential_id: signed.credential_id_hex,
            signature_der_hex: signed.signature_der_hex,
            authenticator_data_hex: signed.authenticator_data_hex,
            client_data_json_hex: signed.client_data_json_hex,
            user_id_hex: None,
            authenticator_attachment: "platform".to_owned(),
            signer_origin: None,
        };
        let sig = envelope(&assertion, &keys).unwrap_or_else(|e| unreachable!("{e:?}"));
        // 12-byte window, then r = the SECOND key's own proxy, not the shared signer.
        let key = vela_core::safe::parse_public_key(&keys[1].public_key_hex)
            .unwrap_or_else(|e| unreachable!("{e}"));
        let proxy = vela_core::safe::compute_webauthn_signer_address(&key.x, &key.y)
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(&sig[..12], &[0u8; 12]);
        assert_eq!(
            to_hex(&sig[24..44], true).to_lowercase(),
            proxy.to_lowercase()
        );
        // A credential outside the wallet is refused, never mis-encoded.
        let foreign = Assertion {
            credential_id: "cafe".to_owned(),
            ..assertion
        };
        assert!(envelope(&foreign, &keys).is_err());
    }

    /// The Trusted Signer's answer goes into the envelope exactly as a
    /// passkey's does (spec 071). The page is handed the ASSEMBLED operation
    /// with the fee leg after the person's calls, the answer is accepted only
    /// over the digest computed here, and the contract signature names the
    /// verifier of the key that actually signed.
    #[test]
    fn the_trusted_signers_answer_becomes_the_same_envelope() {
        use crate::executor::trusted_signer::tests::{
            answers_once, page_result, refuses_once, signing_key, wallet_key,
        };
        let credential = [0x11_u8, 0x22, 0x33];
        let keys = vec![
            wallet_key(&signing_key(9), &[0x99]),
            wallet_key(&signing_key(7), &credential),
        ];
        let op = UserOperation {
            sender: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            nonce: "0x7".to_owned(),
            init_code: vec![],
            call_data: vec![0x7b, 0xb3, 0x74, 0x28],
            verification_gas_limit: 300_000,
            call_gas_limit: 200_000,
            pre_verification_gas: 110_000,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            paymaster_and_data: vec![],
            signature: vec![],
        };
        let inner = vec![MultiSendCall {
            to: "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
            value_hex: "0x1".to_owned(),
            data: vec![],
        }];
        let digest = calculate_safe_op_hash(&op, 100).unwrap_or_else(|e| unreachable!("{e}"));
        let (channel, _changed) = Channel::new();
        let ask = Ask::own(None);
        let signed_over = digest.clone();
        let answering = answers_once(&channel, move || {
            page_result(&signing_key(7), &credential, &signed_over)
        });
        // Spec 082 RA9: the page is a ceremony like a passkey's — the sheet
        // hears it open, then hears it answer, in that order.
        let heard = std::cell::RefCell::new(Vec::new());
        let edges = |edge: CeremonyEdge| heard.borrow_mut().push(edge);
        let signed = {
            let mut signer = Signer::TrustedSigner {
                ask: &ask,
                page: "https://sign.getvela.app/",
                channel: &channel,
                only: None,
            };
            signer.sign(
                &digest,
                100,
                &op.sender,
                &keys,
                Some((&op, &inner)),
                &edges,
                &always_asked,
            )
        };
        let _ = answering.join();
        assert_eq!(
            heard.take(),
            vec![CeremonyEdge::Started, CeremonyEdge::Signed]
        );
        let assertion = signed.unwrap_or_else(|failure| unreachable!("{failure:?}"));
        let signature = envelope(&assertion, &keys).unwrap_or_else(|e| unreachable!("{e:?}"));
        // The second key signed, so its own proxy verifies — not the shared
        // signer the first key uses.
        let key = vela_core::safe::parse_public_key(&keys[1].public_key_hex)
            .unwrap_or_else(|e| unreachable!("{e}"));
        let proxy = vela_core::safe::compute_webauthn_signer_address(&key.x, &key.y)
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            vela_core::primitives::to_hex(&signature[24..44], true).to_lowercase(),
            proxy.to_lowercase()
        );

        // Closed without signing: the request stays open, as for a
        // dismissed passkey sheet — and the sheet has its sentence.
        let declining = refuses_once(&channel, "user_rejected");
        let declined = {
            let mut signer = Signer::TrustedSigner {
                ask: &ask,
                page: "https://sign.getvela.app/",
                channel: &channel,
                only: None,
            };
            signer.sign(
                &digest,
                100,
                &op.sender,
                &keys,
                Some((&op, &inner)),
                &edges,
                &always_asked,
            )
        };
        let _ = declining.join();
        // Opened, never signed: no "done" edge for a ceremony that returned
        // nothing.
        assert_eq!(heard.take(), vec![CeremonyEdge::Started]);
        assert_eq!(declined.err(), Some(SubmitFailure::PasskeyCancelled));
        assert_eq!(
            channel.ended(),
            Some(crate::executor::trusted_signer::Refusal::Closed)
        );
    }

    /// Founder, 2026-09-26: an account signed in through the Trusted Signer
    /// signs there with its sign-in key alone. The page is offered only that
    /// key, and an answer by another of the wallet's keys — one the page could
    /// reach — is refused rather than taken.
    #[test]
    fn the_trusted_signer_takes_only_the_sign_in_key() {
        use crate::executor::trusted_signer::tests::{
            answers_once, page_result, signing_key, wallet_key,
        };
        const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let (first, second) = ([0x99_u8], [0x11_u8, 0x22, 0x33]);
        let keys = vec![
            wallet_key(&signing_key(9), &first),
            wallet_key(&signing_key(7), &second),
        ];
        let original = [0x42_u8; 32];
        let challenge =
            compute_safe_message_hash(&original, 100, SAFE).unwrap_or_else(|e| unreachable!("{e}"));
        let ask = Ask {
            method: "personal_sign".to_owned(),
            params: serde_json::json!(["0x68656c6c6f", SAFE]),
            origin: "https://app.example".to_owned(),
            account_name: None,
        };
        let through_page = |answer: serde_json::Value| {
            let (channel, _changed) = Channel::new();
            let answering = answers_once(&channel, move || answer);
            let signed = sign_message(
                100,
                SAFE,
                &original,
                &keys,
                Signer::TrustedSigner {
                    ask: &ask,
                    page: "https://sign.getvela.app/",
                    channel: &channel,
                    only: Some(&keys[1].credential_id),
                },
                &quiet,
                &always_asked,
            );
            let _ = answering.join();
            (signed, channel.ended())
        };

        // Refused as any unusable answer is: the request stays open, and the
        // sheet says the answer was not this wallet's for this request.
        let (other_key, ended) = through_page(page_result(&signing_key(9), &first, &challenge));
        assert_eq!(other_key.err(), Some(SubmitFailure::PasskeyCancelled));
        assert_eq!(
            ended,
            Some(crate::executor::trusted_signer::Refusal::Mismatch),
            "another founding key's answer was taken"
        );

        let (sign_in_key, ended) = through_page(page_result(&signing_key(7), &second, &challenge));
        assert!(sign_in_key.is_ok(), "{sign_in_key:?}");
        assert_eq!(ended, None);
    }

    /// A message through the Trusted Signer (spec 071): the page is told the
    /// site's own request and no operation, the answer is accepted only over
    /// the Safe's `SafeMessage` hash of the original computed here, and it
    /// comes back as the EIP-1271 envelope a passkey's would.
    #[test]
    fn a_message_through_the_trusted_signer_is_the_same_1271_signature() {
        use crate::executor::trusted_signer::tests::{
            answers_once, page_result, signing_key, wallet_key,
        };
        const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let credential = [0x11_u8, 0x22, 0x33];
        let keys = vec![wallet_key(&signing_key(7), &credential)];
        let original = [0x42_u8; 32];
        let challenge =
            compute_safe_message_hash(&original, 100, SAFE).unwrap_or_else(|e| unreachable!("{e}"));
        let (channel, _changed) = Channel::new();
        let ask = Ask {
            method: "personal_sign".to_owned(),
            params: serde_json::json!(["0x68656c6c6f", SAFE]),
            origin: "https://app.example".to_owned(),
            account_name: None,
        };
        // The request the page would get: the site's, and no operation.
        let request = ask.request(100, SAFE, &keys, None);
        assert_eq!(request["intent"]["method"], "personal_sign");
        assert!(request["context"].get("operation").is_none());

        let answering = answers_once(&channel, move || {
            page_result(&signing_key(7), &credential, &challenge)
        });
        let signed = sign_message(
            100,
            SAFE,
            &original,
            &keys,
            Signer::TrustedSigner {
                ask: &ask,
                page: "https://sign.getvela.app/",
                channel: &channel,
                only: None,
            },
            &quiet,
            &always_asked,
        );
        let _ = answering.join();
        let signature = signed.unwrap_or_else(|failure| unreachable!("{failure:?}"));
        assert!(signature.starts_with("0x"));
        // A one-key wallet signs through the shared WebAuthn signer.
        assert!(
            signature.to_lowercase().contains(
                &vela_core::safe::WEBAUTHN_SIGNER
                    .trim_start_matches("0x")
                    .to_lowercase()
            ),
            "{signature}"
        );
    }

    // -- live (`cargo test executor::user_op -- --ignored --test-threads=1`) --

    /// The golden Safe's dust transfer is estimable through the relay: the
    /// same op the confirm screen prices, no signature involved.
    #[test]
    #[ignore = "real network"]
    fn live_a_dust_transfer_from_the_golden_safe_estimates() {
        const GOLDEN: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let calls = vec![
            FeeCall {
                to: "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
                value: "1000000000000000".to_owned(),
                data: "0x".to_owned(),
            },
            // The fee leg the core would append: a native self-transfer placeholder.
            FeeCall {
                to: GOLDEN.to_owned(),
                value: "1".to_owned(),
                data: "0x".to_owned(),
            },
        ];
        let outcome = simulate_gas(100, GOLDEN, true, &calls, &[]);
        println!("estimate: {outcome:?}");
        assert!(matches!(outcome, FeeGasOutcome::Estimated { .. }));
    }
}
