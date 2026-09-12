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
    Event, SignOperation, SignRecord, SignShellResult, SignSubmitOutcome,
};
use vela_core::app::{Account, KeyMethod};
use vela_core::user_op::WalletKey;

use crate::executor::passkey::{self, Ceremony};
use crate::executor::{now_ms, relay, storage, user_op};

/// `vela.transactionHistory` — the shared local store.
const TX_KEY: &str = "vela.transactionHistory";

/// How this operation is performed, and therefore where. Mirrors
/// `send::SendAnswer` — the signing panel owns its machines the way the send
/// column owns its two, so `Screen` is the arm the host takes back.
pub enum SignAnswer {
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
    pub ceremony: Ceremony,
    /// Raised the instant the passkey prompt opens, so the host can tell the
    /// core the ceremony started rather than guessing from elapsed time.
    pub signing_started: Arc<AtomicBool>,
}

impl SignContext {
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
            ceremony: send.ceremony,
            signing_started: send.signing_started,
        }
    }
}

/// How long a dApp's transaction waits for its receipt before the answer is
/// the userOpHash instead.
///
/// A dApp's promise must SETTLE. Waiting forever for a receipt is the failure
/// mode that looks like success from inside the wallet and like a hang from
/// inside the site.
const RECEIPT_BUDGET: Duration = Duration::from_secs(90);
const RECEIPT_POLL: Duration = Duration::from_secs(3);

pub fn perform(operation: &SignOperation, ctx: &SignContext) -> SignAnswer {
    match operation {
        // The transport is the host's: only it knows which surface raised
        // this request and how to answer it.
        SignOperation::SendResponse { .. } => SignAnswer::Screen,

        SignOperation::PersistRecord { record } => {
            let record = record.clone();
            SignAnswer::Blocking(Box::new(move || {
                persist_record(&record);
                SignShellResult::RecordPersisted
            }))
        }

        SignOperation::UpdateRecord { record_id, close } => {
            let (record_id, close) = (record_id.clone(), close.clone());
            SignAnswer::Blocking(Box::new(move || {
                update_record(&record_id, &close);
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
            ..
        } => {
            let (chain_id, account, bust_cache) = (*chain_id, account.clone(), *bust_cache);
            SignAnswer::Blocking(Box::new(move || {
                if bust_cache {
                    // A retry after somebody funded the account must not read
                    // the balance from before they funded it.
                    relay::clear_cache(chain_id, Some(&account));
                }
                // `None` here means "proceed to submit" — including when the
                // check itself failed. The core's doc is explicit that a
                // timed-out or errored pre-check is not a refusal, and the
                // submit's own underfunded answer is the authority.
                SignShellResult::PreCheck { funding: None }
            }))
        }

        // Silent sponsorship is a relay feature the desktop does not reach
        // yet. Answered, never left hanging.
        SignOperation::AttemptSponsorship { .. } => SignAnswer::Now(SignShellResult::Sponsorship {
            // Denied with no reason rather than invented: the desktop has no
            // sponsorship path, and `Funded` would be a claim that somebody
            // else paid.
            outcome: vela_core::app::sign_request::SignSponsorship::Denied { reason: None },
        }),

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
    let Some(calls) = calls_of(method, params_json) else {
        return SignSubmitOutcome::Failed {
            message: format!("{method} carried no transaction this wallet could read"),
        };
    };

    let mut sign = |challenge: &[u8]| {
        ctx.signing_started.store(true, Ordering::SeqCst);
        passkey::assert(
            challenge,
            ctx.pinned_credential.as_deref(),
            ctx.key_method,
            &ctx.ceremony,
        )
    };
    let submitted = user_op::submit(
        chain_id,
        address,
        &calls,
        gas_fee_token,
        &ctx.keys,
        &mut sign,
        quoted,
    );
    let user_op_hash = match submitted {
        Ok(hash) => hash,
        Err(failure) => return submit_failure(chain_id, address, failure),
    };

    // Told BEFORE the receipt wait. A window closed during that wait must
    // still know an operation was accepted — otherwise a submitted
    // transaction looks, on reopen, like one that never happened.
    sink.send(Event::OpSubmitted {
        id: id.to_owned(),
        user_op_hash: user_op_hash.clone(),
        now_ms: now_ms(),
    });

    match await_receipt(&user_op_hash, chain_id) {
        // A dApp's `eth_sendTransaction` resolves to a TX hash. Handing back
        // the userOpHash instead gives the site something it can look up
        // forever and never find.
        Some(tx_hash) => SignSubmitOutcome::Succeeded { result: tx_hash },
        // Submitted, not yet confirmed. The hash still answers the promise —
        // a dApp left waiting cannot tell a slow chain from a lost
        // transaction, and that ambiguity is the double-spend risk 027 D37
        // names.
        None => SignSubmitOutcome::Succeeded {
            result: user_op_hash,
        },
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
        // A missing value is zero, not a failure: most contract calls carry
        // none. Hex on the wire, decimal to the core.
        value: raw
            .get("value")
            .and_then(Value::as_str)
            .and_then(|hex| u128::from_str_radix(hex.trim_start_matches("0x"), 16).ok())
            .unwrap_or(0)
            .to_string(),
        data: raw
            .get("data")
            .and_then(Value::as_str)
            .unwrap_or("0x")
            .to_owned(),
    })
}

/// Poll until the receipt lands or the budget runs out.
///
/// `None` is "not yet", never "failed": a relay that could not be reached is
/// not a transaction that did not happen, and the caller answers with the
/// userOpHash rather than an error.
fn await_receipt(user_op_hash: &str, chain_id: u32) -> Option<String> {
    let deadline = std::time::Instant::now() + RECEIPT_BUDGET;
    while std::time::Instant::now() < deadline {
        let poll = relay::user_op_receipt(user_op_hash, chain_id);
        if let Some(resolution) = poll.resolution {
            // A receipt that says the operation reverted is still a receipt:
            // the tx hash is real and the dApp should have it. What it is NOT
            // is this wallet's business to relabel.
            return Some(resolution.tx_hash);
        }
        std::thread::sleep(RECEIPT_POLL);
    }
    None
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
        },
        user_op::SubmitFailure::Other(message) => SignSubmitOutcome::Failed { message },
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
fn persist_record(record: &SignRecord) {
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
    });
    if let Some(intent) = &record.intent {
        row["intent"] = json!(intent);
    }

    let mut rows = match storage::read_value(TX_KEY) {
        Ok(Some(Value::Array(rows))) => rows,
        _ => Vec::new(),
    };
    // Same id, never a second row — a resubmit of the same request closes the
    // record it opened (the core's note on `SignRecordClose`).
    if let Some(existing) = rows
        .iter_mut()
        .find(|row| row.get("id").and_then(Value::as_str) == Some(record.record_id.as_str()))
    {
        *existing = row;
    } else {
        rows.push(row);
    }
    let _ = storage::write_value(TX_KEY, Value::Array(rows));
}

/// Close a pending record IN PLACE.
fn update_record(record_id: &str, close: &vela_core::app::sign_request::SignRecordClose) {
    use vela_core::app::sign_request::SignRecordClose;

    let Ok(Some(Value::Array(mut rows))) = storage::read_value(TX_KEY) else {
        return;
    };
    let Some(row) = rows
        .iter_mut()
        .find(|row| row.get("id").and_then(Value::as_str) == Some(record_id))
    else {
        return;
    };
    match close {
        SignRecordClose::Confirmed { tx_hash } => {
            row["status"] = json!("confirmed");
            row["txHash"] = json!(tx_hash);
        }
        SignRecordClose::Failed => row["status"] = json!("failed"),
    }
    let _ = storage::write_value(TX_KEY, Value::Array(rows));
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
        }
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
