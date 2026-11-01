//! The app's own pipelines behind the multisig shell ([`Deps`]): the
//! service over the app's proxy routes, the RPC pool, the store, the
//! passkey (in Vela, or on the member's trusted signing page), the send and
//! fee pipelines, the signing sheet's simulation, recipient facts and the
//! relay's word on a user operation — the web's `shell/live.ts`.
//!
//! Each answers in the vocabulary and never decides: a failure is reported
//! as what it was (a [`ShellFailure`] carried as the `detail` the core hands
//! back in its views), so the sheet can say it in the corpus' words.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use vela_core::app::fee_policy::{
    Event as FeeEvent, FeeAssetView, FeeCall, FeeFailure, FeePolicy, FeeTier,
};
use vela_core::app::multisig_shell::{
    MultisigShellResult as Res, PasskeyPurpose, SendPurpose, SentCall,
};
use vela_core::app::{Account, FailureKind, KeyMethod};
use vela_core::multisig_review::SimCall;
use vela_core::signing_venue::{SignatureKind, SigningRoute, signing_route};

use super::shell::{Deps, HttpReply, RecipientFacts, RpcReply};
use crate::core_host::CoreHost;
use crate::executor::passkey::{self, Ceremony, PasskeyFailure};
use crate::executor::{pool, proxy, relay, storage, trusted_signer, user_op};
use crate::resident::{Answer, Machine as _};

/// Why a passkey or a send did not happen, as the shell saw it: JSON in a
/// result's `detail` (the web's `shell/failure.ts`, the same shapes), read
/// back by the sheet ([`read_failure`]).
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ShellFailure {
    VenueBlocked {
        block: Value,
    },
    PageDeclined,
    PageSignFailed {
        page: String,
    },
    PageApproval {
        failure: String,
    },
    Passkey {
        failure: String,
        #[serde(rename = "timedOut")]
        timed_out: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        message: Option<String>,
    },
    NotSent,
    CannotPay {
        need: String,
        have: String,
    },
    Unknown {
        message: String,
    },
}

impl ShellFailure {
    /// As the `detail` the core carries.
    #[must_use]
    pub fn detail(&self) -> String {
        serde_json::to_string(self).unwrap_or_else(|_| "{\"kind\":\"unknown\"}".to_owned())
    }
}

/// A `detail` this shell wrote, or `None` for the core's own words.
#[must_use]
pub fn read_failure(detail: &str) -> Option<ShellFailure> {
    if !detail.starts_with('{') {
        return None;
    }
    serde_json::from_str(detail).ok()
}

/// The genesis service in use: Settings → Service endpoints, else the
/// core's default.
#[must_use]
pub fn service_url() -> String {
    storage::read_value(storage::KEY_SERVICE_ENDPOINTS)
        .ok()
        .flatten()
        .and_then(|v| v.get("genesisServiceURL").and_then(Value::as_str).map(str::to_owned))
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty())
        .unwrap_or_else(|| {
            vela_core::app::multisig_shell::DEFAULT_GENESIS_SERVICE_URL.to_owned()
        })
}

/// The PeerConnect relay set for this device (the web's
/// `vela.peerconnectRelay`), if any.
#[must_use]
pub fn relay_url() -> Option<String> {
    storage::read_value(super::peer::RELAY_KEY)
        .ok()
        .flatten()
        .and_then(|v| v.as_str().map(str::to_owned))
        .map(|url| url.trim().to_owned())
        .filter(|url| !url.is_empty())
}

/// `0x` + hex, from hex with or without it.
fn hex0x(hex: &str) -> String {
    if hex.starts_with("0x") {
        hex.to_owned()
    } else {
        format!("0x{hex}")
    }
}

fn bytes(hex: &str) -> Option<Vec<u8>> {
    let clean = hex.strip_prefix("0x").unwrap_or(hex);
    if !clean.len().is_multiple_of(2) {
        return None;
    }
    (0..clean.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(clean.get(at..at + 2)?, 16).ok())
        .collect()
}

/// A quantity in hex or decimal, as a `u128`.
fn quantity(text: &str) -> Option<u128> {
    let text = text.trim();
    match text.strip_prefix("0x") {
        Some("") => Some(0),
        Some(hex) => u128::from_str_radix(hex, 16).ok(),
        None if text.is_empty() => Some(0),
        None => text.parse().ok(),
    }
}

/// A value as `eth_simulateV1` takes it: a hex quantity.
fn hex_quantity(text: &str) -> String {
    quantity(text).map_or_else(|| "0x0".to_owned(), |v| format!("0x{v:x}"))
}

/// The account at `address` on this device, whole.
fn account(address: &str) -> Option<Account> {
    storage::load_accounts()
        .ok()?
        .into_iter()
        .find(|account| account.address.eq_ignore_ascii_case(address))
}

/// What a passkey failure is, in the vocabulary: a close is a cancel.
fn passkey_result(failure: &PasskeyFailure) -> Res {
    if failure.kind == FailureKind::Cancelled {
        return Res::PasskeyCancelled;
    }
    Res::PasskeyFailed {
        detail: ShellFailure::Passkey {
            failure: format!("{:?}", failure.kind).to_lowercase(),
            timed_out: false,
            message: failure.message.clone(),
        }
        .detail(),
    }
}

/// The calls a user operation runs, as the send pipeline takes them
/// (values decimal).
fn fee_calls(calls: &[SentCall]) -> Vec<FeeCall> {
    calls
        .iter()
        .map(|c| FeeCall {
            to: c.to.clone(),
            value: quantity(&c.value).unwrap_or(0).to_string(),
            data: if c.data.is_empty() {
                "0x".to_owned()
            } else {
                c.data.clone()
            },
        })
        .collect()
}

/// The fee the person saw, back as the send pipeline signs it.
fn quoted_fee(fee: Option<&Value>) -> Option<user_op::QuotedFee> {
    let fee = fee?;
    let recipient = fee.get("recipient")?.as_str()?.to_owned();
    let amount = match fee.get("amount")? {
        Value::String(s) => quantity(s)?,
        Value::Number(n) => n.as_u64().map(u128::from)?,
        _ => return None,
    };
    let tier = fee
        .get("tier")
        .and_then(Value::as_str)
        .and_then(|t| serde_json::from_value::<FeeTier>(json!(t)).ok());
    Some(user_op::QuotedFee {
        amount,
        recipient,
        tier,
    })
}

/// The pipelines, bound to the screen's ceremony (its PIN and touch
/// prompts, its trusted page's waiting sheet).
pub struct Live {
    ceremony: Ceremony,
    trusted: Arc<trusted_signer::Channel>,
}

impl Live {
    #[must_use]
    pub fn new(ceremony: Ceremony, trusted: Arc<trusted_signer::Channel>) -> Self {
        Self { ceremony, trusted }
    }

    /// The member's approval on its trusted page (`page_approved`), or why not.
    fn approve_on_page(
        &self,
        account: &Account,
        page: &str,
        chain_id: u64,
        purpose: (&str, &str, Option<&str>, &vela_core::multisig::SafeTx, bool),
        // r10 page: the members and approvals needed now, for before → after.
        now: (&[String], u64),
    ) -> Res {
        let (multisig, safe_tx_hash, name, tx, anyway) = purpose;
        let (owners, threshold) = now;
        let keys = user_op::key_set_of(account);
        let credential_ids: Vec<String> = keys.iter().map(|k| k.credential_id.clone()).collect();
        let plan = account.signing_plan();
        let chain = u32::try_from(chain_id).unwrap_or_default();
        let chain_name = crate::executor::custom_tokens::network_name(chain);
        let symbol = crate::flows::live::native_symbol(chain);
        let request = vela_core::trusted_signer::approval::request(
            &vela_core::trusted_signer::approval::ApprovalInput {
                chain_id,
                chain_name: Some(chain_name.as_str()),
                native_symbol: Some(symbol.as_str()),
                multisig,
                multisig_name: name,
                tx,
                member: &account.address,
                member_name: Some(account.name.as_str()).filter(|n| !n.is_empty()),
                credential_ids_hex: &credential_ids,
                key_route: plan.key.as_ref(),
                // r9-10: the page says "Approve anyway" where the core asks.
                anyway,
                owners,
                threshold: (threshold > 0).then_some(threshold),
            },
        );
        let request = match request {
            Ok(request) => request,
            Err(error) => {
                return Res::PasskeyFailed {
                    detail: ShellFailure::Unknown {
                        message: error.to_string(),
                    }
                    .detail(),
                };
            }
        };
        self.trusted.choose(Some(page.to_owned()));
        let answered = trusted_signer::approve(
            &request,
            page,
            chain_id,
            &account.address,
            safe_tx_hash,
            &keys,
            &self.trusted,
        );
        self.trusted.choose(None);
        match answered {
            Ok(approval) => Res::PageApproved {
                approval: serde_json::to_value(approval).unwrap_or(Value::Null),
            },
            Err(trusted_signer::PageEnd::Passkey(failure)) => passkey_result(&failure),
            Err(trusted_signer::PageEnd::Declined) => Res::PasskeyFailed {
                detail: ShellFailure::PageDeclined.detail(),
            },
            Err(trusted_signer::PageEnd::Failed(failure)) => Res::PasskeyFailed {
                detail: ShellFailure::PageApproval {
                    failure: format!("{failure:?}"),
                }
                .detail(),
            },
        }
    }

    /// The coin is checked before the passkey (the web's `beforeSign`): an
    /// executor that cannot pay the fee in the chain's coin is told so, and
    /// nothing is signed.
    fn cannot_pay(chain_id: u32, from: &str, need: u128) -> Option<ShellFailure> {
        let envelope = pool::call_within(
            chain_id,
            "eth_getBalance",
            json!([from, "latest"]),
            Duration::from_secs(6),
        )
        .ok()?;
        let have = quantity(envelope.get("result")?.as_str()?)?;
        (have < need).then(|| ShellFailure::CannotPay {
            need: need.to_string(),
            have: have.to_string(),
        })
    }

    /// The executor's own user operation (`vela_multisigExecute`): ONE
    /// signature — its passkey, or its page, which reads what runs from the
    /// operation's bytes and is told what they do not carry.
    fn execute(
        &self,
        chain_id: u64,
        from: &str,
        calls: &[SentCall],
        fee: Option<&Value>,
        purpose: Option<&SendPurpose>,
    ) -> Res {
        let Some(account) = account(from) else {
            return Res::CallsFailed {
                detail: ShellFailure::Unknown {
                    message: "the executor is not on this device".to_owned(),
                }
                .detail(),
                maybe_sent: false,
            };
        };
        let chain = u32::try_from(chain_id).unwrap_or_default();
        let calls = fee_calls(calls);
        let quoted = quoted_fee(fee);
        // A fee in the chain's coin: the executor must hold it, and the
        // calls' value, before anything is signed.
        let paid_in_coin = fee.is_some_and(|f| f.get("token").is_none_or(Value::is_null));
        if paid_in_coin && let Some(quoted) = &quoted {
            let value: u128 = calls.iter().filter_map(|c| quantity(&c.value)).sum();
            if let Some(failure) = Self::cannot_pay(chain, from, quoted.amount + value) {
                return Res::CallsFailed {
                    detail: failure.detail(),
                    maybe_sent: false,
                };
            }
        }
        let mut ctx = crate::executor::send::SendContext::new(&account, self.ceremony.clone());
        // The screen listens on this channel: a page venue opens there.
        ctx.trusted_signer = Arc::clone(&self.trusted);
        ctx.follow_venue();
        let params = match purpose {
            Some(SendPurpose::Execute {
                multisig,
                name,
                threshold,
                tx,
                owners,
            }) => json!({
                "tx": tx,
                "multisig": multisig,
                "name": name,
                "threshold": threshold,
                "owners": owners,
            }),
            _ => json!({}),
        };
        let ask = trusted_signer::Ask {
            method: "vela_multisigExecute".to_owned(),
            params,
            origin: String::new(),
            account_name: (!account.name.is_empty()).then(|| account.name.clone()),
        };
        let mut asked_passkey = false;
        let mut sign = |challenge: &[u8]| {
            asked_passkey = true;
            passkey::assert(
                challenge,
                ctx.pinned_credential.as_deref(),
                ctx.key_method,
                &ctx.ceremony,
            )
        };
        let signer = ctx.signer(&ask, &mut sign);
        let submitted = user_op::submit(
            chain,
            &account.address,
            &calls,
            None,
            &ctx.keys,
            signer,
            quoted,
            None,
            None,
            &user_op::quiet,
            &user_op::always_asked,
            &|_, _| true,
        );
        match submitted {
            Ok(submitted) if submitted.maybe_sent => Res::CallsFailed {
                // A lost answer after it may have left: followed by its
                // hash, never sent again.
                detail: submitted.user_op_hash,
                maybe_sent: true,
            },
            Ok(submitted) => Res::CallsSent {
                user_op_hash: submitted.user_op_hash,
                tx_hash: None,
            },
            Err(user_op::SubmitFailure::PasskeyCancelled) => Res::CallsCancelled,
            Err(user_op::SubmitFailure::PasskeyTimedOut) => Res::CallsFailed {
                detail: ShellFailure::Passkey {
                    failure: "cancelled".to_owned(),
                    timed_out: true,
                    message: None,
                }
                .detail(),
                maybe_sent: false,
            },
            Err(user_op::SubmitFailure::NotSent) => Res::CallsFailed {
                detail: ShellFailure::NotSent.detail(),
                maybe_sent: false,
            },
            Err(failure) => Res::CallsFailed {
                detail: ShellFailure::Unknown {
                    message: format!("{failure:?}"),
                }
                .detail(),
                maybe_sent: false,
            },
        }
    }
}

/// How long one fee quote may take before it is no price at all.
const QUOTE_BUDGET: Duration = Duration::from_secs(45);

/// `from` sending `calls` on `chain_id`, priced the way every other send of
/// the wallet is: one `fee_policy` quote of the real operation, at the
/// relay's price, in the chain's coin (the web's `quoteExecutionFee`). The
/// machine runs here, on this thread, to its first settled view.
#[must_use]
pub fn quote(chain_id: u64, from: &str, calls: &[SentCall]) -> Value {
    let chain = u32::try_from(chain_id).unwrap_or_default();
    let mut host = CoreHost::<FeePolicy>::new();
    let public_key_available = account(from).is_some_and(|a| !a.public_key_hex.is_empty());
    let mut pending = host.dispatch(FeeEvent::QuoteRequested {
        chain_id: chain,
        account: from.to_owned(),
        deployed: false,
        public_key_available,
        tier: FeeTier::Standard,
        calls: fee_calls(calls),
        fee_token: None,
        auto_fee_token: false,
        number: crate::executor::format_prefs::current().number,
        read_deployment: Some(true),
    });
    let started = Instant::now();
    // A timer (the quote's deadline, a block's requote) is answered only
    // once every read is in and its time has come — a shell's clock.
    let mut timers: Vec<(Instant, u64, vela_core::app::fee_policy::FeeShellResult)> = Vec::new();
    loop {
        let view = host.view();
        if !view.busy {
            if let Some(fee) = view.fee {
                let amount = match &fee.fee_asset {
                    FeeAssetView::Erc20 { amount, .. } => amount.clone(),
                    FeeAssetView::Native => fee.total_wei.clone(),
                };
                if let (Some(recipient), Some(value)) = (fee.fee_recipient.clone(), quantity(&amount))
                    && value > 0
                {
                    let mut out = json!({
                        "kind": "ok",
                        "fee": { "amount": value.to_string(), "recipient": recipient },
                        "totalGas": quantity(&fee.total_gas).unwrap_or(0).to_string(),
                        "executorDeployed": fee.deployed,
                    });
                    if fee.tier != FeeTier::Rapid {
                        out["fee"]["tier"] = serde_json::to_value(fee.tier).unwrap_or(Value::Null);
                    }
                    return out;
                }
                crate::diag::vlog!("multisig", "quote: no fee recipient or a zero fee");
                return json!({ "kind": "failed" });
            }
            if let Some(failure) = view.failed {
                crate::diag::vlog!("multisig", "quote: {failure:?}");
                return if failure == FeeFailure::WouldFail {
                    json!({ "kind": "reverts", "reason": Value::Null })
                } else {
                    json!({ "kind": "failed" })
                };
            }
        }
        if started.elapsed() > QUOTE_BUDGET {
            crate::diag::vlog!("multisig", "quote: out of time");
            return json!({ "kind": "failed" });
        }
        let Some(next) = pending.pop() else {
            // Only timers left: the earliest, when its time comes.
            timers.sort_by_key(|(due, _, _)| *due);
            if timers.is_empty() {
                crate::diag::vlog!("multisig", "quote: nothing left to answer");
                return json!({ "kind": "failed" });
            }
            let (due, id, result) = timers.remove(0);
            std::thread::sleep(due.saturating_duration_since(Instant::now()));
            pending.extend(host.resolve(id, result));
            continue;
        };
        let result = match FeePolicy::perform(&next.operation) {
            Answer::Now(result) => result,
            Answer::After(delay, result) => {
                timers.push((Instant::now() + delay, next.id, result));
                continue;
            }
            Answer::Blocking(work) => work(),
            Answer::Streaming(work) => {
                let (tx, mut rx) = futures::channel::mpsc::unbounded();
                let result = work(&crate::resident::Sink::new(tx));
                while let Ok(event) = rx.try_recv() {
                    pending.extend(host.dispatch(event));
                }
                result
            }
        };
        pending.extend(host.resolve(next.id, result));
    }
}

impl Deps for Live {
    fn http(
        &self,
        method: &str,
        url: &str,
        body: Option<&str>,
        timeout: Duration,
    ) -> Result<HttpReply, String> {
        let answer = proxy::with_routes(url, timeout, |agent| {
            let mut response = match (method, body) {
                ("GET", _) => agent
                    .get(url)
                    .config()
                    .http_status_as_error(false)
                    .build()
                    .header("accept", "application/json")
                    .call()?,
                ("DELETE", _) => agent
                    .delete(url)
                    .config()
                    .http_status_as_error(false)
                    .build()
                    .header("accept", "application/json")
                    .call()?,
                (_, body) => agent
                    .post(url)
                    .config()
                    .http_status_as_error(false)
                    .build()
                    .header("accept", "application/json")
                    .header("content-type", "application/json")
                    .send(body.unwrap_or_default())?,
            };
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get("retry-after")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let body = response.body_mut().read_to_string().unwrap_or_default();
            Ok(HttpReply {
                status,
                retry_after,
                body,
            })
        });
        answer.map_err(|failure| failure.error.to_string())
    }

    fn rpc(
        &self,
        chain_id: u64,
        method: &str,
        params: &Value,
        bound: Option<Duration>,
    ) -> Result<RpcReply, String> {
        let chain = u32::try_from(chain_id).map_err(|_| format!("no chain {chain_id}"))?;
        let params = if params.is_array() {
            params.clone()
        } else {
            json!([params])
        };
        let envelope = match bound {
            Some(budget) => pool::call_within(chain, method, params, budget),
            None => pool::call(chain, method, params),
        }
        .map_err(|error| format!("{error:?}"))?;
        if let Some(error) = envelope.get("error").filter(|e| !e.is_null()) {
            return Ok(RpcReply::Error {
                code: error.get("code").and_then(Value::as_i64),
                message: error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                data: error.get("data").cloned(),
            });
        }
        Ok(RpcReply::Result(
            envelope.get("result").cloned().unwrap_or(Value::Null),
        ))
    }

    fn load(&self, keys: &[String]) -> Result<BTreeMap<String, Option<String>>, String> {
        keys.iter()
            .map(|key| {
                let value = storage::read_value(key).map_err(|e| e.to_string())?;
                let raw = value.map(|value| match value {
                    Value::String(text) => text,
                    other => other.to_string(),
                });
                Ok((key.clone(), raw))
            })
            .collect()
    }

    fn write(&self, key: &str, value: Option<&str>) -> Result<(), String> {
        match value {
            // Kept as the text it is, never re-shaped: the core reads back
            // exactly what it wrote.
            Some(raw) => storage::write_value(key, Value::String(raw.to_owned())),
            None => storage::remove_value(key),
        }
        .map_err(|e| e.to_string())
    }

    fn now_ms(&self) -> f64 {
        crate::executor::now_ms()
    }

    fn random(&self, bytes: usize) -> Vec<u8> {
        let mut out = vec![0u8; bytes];
        if getrandom::fill(&mut out).is_err() {
            // No randomness is no salt: an empty answer, which the core
            // refuses rather than founding a guessable multisig.
            return Vec::new();
        }
        out
    }

    fn passkey(
        &self,
        owner: &str,
        chain_id: u64,
        challenge: &str,
        purpose: &PasskeyPurpose,
    ) -> Res {
        let Some(account) = account(owner) else {
            return Res::PasskeyFailed {
                detail: ShellFailure::Unknown {
                    message: "not an account on this device".to_owned(),
                }
                .detail(),
            };
        };
        if let PasskeyPurpose::Approve {
            multisig,
            safe_tx_hash,
            name,
            tx,
            anyway,
            owners,
            threshold,
        } = purpose
        {
            match signing_route(&account.signing_plan(), SignatureKind::MultisigApproval) {
                SigningRoute::Blocked { block } => {
                    return Res::PasskeyFailed {
                        detail: ShellFailure::VenueBlocked {
                            block: serde_json::to_value(block).unwrap_or(Value::Null),
                        }
                        .detail(),
                    };
                }
                SigningRoute::Page { url } => {
                    return self.approve_on_page(
                        &account,
                        &url,
                        chain_id,
                        (multisig, safe_tx_hash, name.as_deref(), tx, *anyway),
                        (owners, *threshold),
                    );
                }
                SigningRoute::InVela => {}
            }
        }
        let Some(challenge) = bytes(challenge) else {
            return Res::PasskeyFailed {
                detail: ShellFailure::Unknown {
                    message: "the challenge is not hex".to_owned(),
                }
                .detail(),
            };
        };
        let ctx = crate::executor::send::SendContext::new(&account, self.ceremony.clone());
        let method: KeyMethod = ctx.key_method;
        match passkey::assert(
            &challenge,
            ctx.pinned_credential.as_deref(),
            method,
            &self.ceremony,
        ) {
            Ok(assertion) => Res::Asserted {
                credential_id: assertion.credential_id,
                authenticator_data: hex0x(&assertion.authenticator_data_hex),
                client_data_json: hex0x(&assertion.client_data_json_hex),
                signature: hex0x(&assertion.signature_der_hex),
            },
            Err(failure) => passkey_result(&failure),
        }
    }

    fn send_calls(
        &self,
        chain_id: u64,
        from: &str,
        calls: &[SentCall],
        fee: Option<&Value>,
        purpose: Option<&SendPurpose>,
    ) -> Res {
        // An older machine says no purpose: an execution is the one with a fee.
        let record = match purpose {
            Some(SendPurpose::Record { .. }) => true,
            Some(SendPurpose::Execute { .. }) => false,
            None => fee.is_none(),
        };
        if record {
            // The Ethereum backup's record goes through the signing sheet
            // (a first-party transaction, reviewed there); this shell does
            // not offer it yet, and says so rather than signing blind.
            return Res::CallsFailed {
                detail: ShellFailure::Unknown {
                    message: "saving a record to Ethereum is not on the desktop yet".to_owned(),
                }
                .detail(),
                maybe_sent: false,
            };
        }
        self.execute(chain_id, from, calls, fee, purpose)
    }

    fn quote_calls(&self, chain_id: u64, from: &str, calls: &[SentCall]) -> Res {
        Res::Quoted {
            quote: quote(chain_id, from, calls),
        }
    }

    fn simulate(&self, chain_id: u64, from: &str, calls: &[SimCall]) -> Option<String> {
        // Nothing was asked: an answer nobody can read, never "nothing moves".
        if calls.first().is_none_or(|c| c.to.is_empty()) {
            return Some("{}".to_owned());
        }
        let chain = u32::try_from(chain_id).ok()?;
        let payload = json!({
            "blockStateCalls": [{
                "calls": calls.iter().map(|c| {
                    let mut entry = json!({
                        "from": from,
                        "to": c.to,
                        "value": hex_quantity(&c.value),
                    });
                    if !c.data.is_empty() && c.data != "0x" {
                        entry["data"] = json!(c.data);
                    }
                    entry
                }).collect::<Vec<_>>(),
            }],
            "traceTransfers": true,
            "validation": false,
            "returnFullTransactions": false,
        });
        Some(
            match pool::call(chain, "eth_simulateV1", json!([payload, "latest"])) {
                Ok(envelope) => envelope.to_string(),
                Err(_) => "{\"unreachable\":true}".to_owned(),
            },
        )
    }

    fn recipient_facts(&self, chain_id: u64, address: &str) -> RecipientFacts {
        let Ok(chain) = u32::try_from(chain_id) else {
            return RecipientFacts::default();
        };
        let risk = crate::executor::send::recipient_risk(chain, address);
        RecipientFacts {
            is_contract: risk.is_contract,
            first_time: risk.first_time,
        }
    }

    fn user_op_status(
        &self,
        chain_id: u64,
        user_op_hash: &str,
    ) -> Option<(Option<String>, Option<String>)> {
        let chain = u32::try_from(chain_id).ok()?;
        let answer = relay::user_op_status(user_op_hash, chain)?;
        let status = serde_json::to_value(answer.status)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned));
        Some((status, answer.tx_hash))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_is_carried_as_the_webs_json() {
        let detail = ShellFailure::CannotPay {
            need: "10".to_owned(),
            have: "3".to_owned(),
        }
        .detail();
        assert_eq!(detail, r#"{"kind":"cannot_pay","need":"10","have":"3"}"#);
        assert_eq!(
            read_failure(&detail),
            Some(ShellFailure::CannotPay {
                need: "10".to_owned(),
                have: "3".to_owned()
            })
        );
        let passkey = ShellFailure::Passkey {
            failure: "other".to_owned(),
            timed_out: true,
            message: None,
        }
        .detail();
        assert_eq!(passkey, r#"{"kind":"passkey","failure":"other","timedOut":true}"#);
        assert_eq!(read_failure("the core's own words"), None);
    }

    #[test]
    fn a_quoted_fee_reads_back_as_the_pipeline_signs_it() {
        let fee = json!({"amount": "12345", "recipient": "0xfee", "tier": "standard"});
        assert_eq!(
            quoted_fee(Some(&fee)),
            Some(user_op::QuotedFee {
                amount: 12345,
                recipient: "0xfee".to_owned(),
                tier: Some(FeeTier::Standard)
            })
        );
        assert_eq!(quoted_fee(Some(&json!({"amount": "1"}))), None, "no recipient");
        assert_eq!(quoted_fee(None), None);
        assert_eq!(
            quoted_fee(Some(&json!({"amount": "0x10", "recipient": "0x1"})))
                .map(|q| q.amount),
            Some(16)
        );
    }

    #[test]
    fn calls_travel_with_decimal_values_and_simulate_with_hex_ones() {
        let calls = vec![SentCall {
            to: "0x1".to_owned(),
            value: "0x2a".to_owned(),
            data: String::new(),
        }];
        let fee = fee_calls(&calls);
        assert_eq!((fee[0].value.as_str(), fee[0].data.as_str()), ("42", "0x"));
        assert_eq!(hex_quantity("42"), "0x2a");
        assert_eq!(hex_quantity("0x2a"), "0x2a");
        assert_eq!(hex_quantity("junk"), "0x0");
    }

    #[test]
    fn a_hex_challenge_reads_with_or_without_its_prefix() {
        assert_eq!(bytes("0x0aff"), Some(vec![0x0a, 0xff]));
        assert_eq!(bytes("0aff"), Some(vec![0x0a, 0xff]));
        assert_eq!(bytes("0xabc"), None);
        assert_eq!(hex0x("ab"), "0xab");
        assert_eq!(hex0x("0xab"), "0xab");
    }

    #[test]
    fn a_closed_prompt_is_a_cancel_never_a_failure() {
        assert_eq!(
            passkey_result(&PasskeyFailure {
                kind: FailureKind::Cancelled,
                message: None,
                facts: None,
            }),
            Res::PasskeyCancelled
        );
        let Res::PasskeyFailed { detail } = passkey_result(&PasskeyFailure {
            kind: FailureKind::NotSupported,
            message: Some("no authenticator".to_owned()),
            facts: None,
        }) else {
            panic!("a failure");
        };
        assert_eq!(
            read_failure(&detail),
            Some(ShellFailure::Passkey {
                failure: "notsupported".to_owned(),
                timed_out: false,
                message: Some("no authenticator".to_owned())
            })
        );
    }
}
