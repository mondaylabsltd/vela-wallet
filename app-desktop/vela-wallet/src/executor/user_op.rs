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

use vela_core::ClientDataKind;
use vela_core::app::fee_policy::{
    AssetPricing, FeeAssetKind, FeeAssetQuote, FeeCall, FeeGasOutcome, FeeTier,
    TEMPO_DEFAULT_FEE_TOKEN, TEMPO_FEE_TOKEN_DECIMALS, calculate_in_band_fee_amount,
    is_tempo_chain, tempo_call_gas_limit, tempo_expected_gas, tempo_minimum_fee_token_units,
    tempo_reimbursement,
};
use vela_core::app::send::SendSubmitFailure;
use vela_core::app::{Account, Assertion, FailureKind};
use vela_core::primitives::{from_hex, to_hex};
use vela_core::user_op::{
    CALL_GAS_LIMIT, MultiSendCall, PRE_VERIFICATION_GAS, UserOperation, VERIFICATION_GAS_DEPLOYED,
    VERIFICATION_GAS_UNDEPLOYED, WalletKey, build_dummy_signature, build_in_band_fee_leg,
    build_init_code_for_keys, build_multi_send_execute_call_data, build_user_op_signature,
    calculate_safe_op_hash, compute_safe_message_hash, eip1271_envelope_signature,
    encode_erc20_transfer, extract_client_data_fields, inner_calls_gas_floor,
    is_plain_transfer_call, pad_gas_estimate, parse_existing_user_op_hash, signer_address_for,
};
use vela_core::webauthn::{der_signature_to_raw_low_s, validate_client_data};

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
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubmitFailure {
    PasskeyCancelled,
    RelayerUnavailable,
    BundlerUnderfunded,
    /// Diagnostics only; the core words the screen.
    Other(String),
}

impl From<SubmitFailure> for SendSubmitFailure {
    fn from(failure: SubmitFailure) -> Self {
        match failure {
            SubmitFailure::PasskeyCancelled => SendSubmitFailure::PasskeyCancelled,
            SubmitFailure::RelayerUnavailable => SendSubmitFailure::RelayerUnavailable,
            SubmitFailure::BundlerUnderfunded => SendSubmitFailure::BundlerUnderfunded,
            SubmitFailure::Other(message) => SendSubmitFailure::Other {
                message: (!message.is_empty()).then_some(message),
            },
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
    /// The Trusted Signer: the request as the page is told it, the page, and
    /// the screen's channel to its waiting sheet.
    TrustedSigner {
        ask: &'a Ask,
        page: &'a str,
        channel: &'a Channel,
    },
}

impl Signer<'_> {
    /// One signature over `digest`. `operation` is what the Trusted Signer
    /// shows and derives the digest from — the assembled operation and the
    /// calls before its fee leg; `None` for a message.
    fn sign(
        &mut self,
        digest: &[u8],
        chain_id: u32,
        safe: &str,
        keys: &[WalletKey],
        operation: Option<(&UserOperation, &[MultiSendCall])>,
    ) -> Result<Assertion, SubmitFailure> {
        match self {
            Self::Passkey(sign) => sign(digest),
            Self::TrustedSigner { ask, page, channel } => {
                let request = ask.request(chain_id, safe, keys, operation);
                trusted_signer::sign(&request, page, digest, keys, channel)
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
/// without its keys); `SimulationFailed` is a truthful op the relay refused.
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
        Err(message) => {
            eprintln!("[vela-wallet] fee: relay estimation unavailable: {message}");
            FeeGasOutcome::SimulationFailed
        }
    }
}

// ---------------------------------------------------------------------------
// Submit
// ---------------------------------------------------------------------------

/// The whole sign→submit orchestration (`sendBatchCalls`): Tempo pays gas
/// in its stablecoin, every other chain settles in band. Answers the
/// accepted operation's hash, or why there is none.
pub fn submit(
    chain_id: u32,
    safe: &str,
    calls: &[FeeCall],
    gas_fee_token: Option<&str>,
    keys: &[WalletKey],
    signer: Signer<'_>,
    quoted_fee: Option<QuotedFee>,
) -> Result<String, SubmitFailure> {
    let outcome = submit_inner(
        chain_id,
        safe,
        calls,
        gas_fee_token,
        keys,
        signer,
        quoted_fee,
    );
    // The SCREEN gets the core's sentence — SC-305: no relay text reaches a
    // person. The OPERATOR gets the detail, because a submit that failed
    // before the relay leaves no other trace at all: the log showed the quote
    // being signed and then nothing, and "your funds are safe, try again" is
    // the same words whether the passkey was refused, the hash would not
    // compute, or the account is undeployed. One line, at the one place every
    // failure passes through.
    if let Err(failure) = &outcome {
        eprintln!("[vela-wallet] submit failed: {failure:?}");
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
) -> Result<String, SubmitFailure> {
    let inner: Vec<MultiSendCall> = calls
        .iter()
        .map(to_multi_send_call)
        .collect::<Result<_, _>>()
        .map_err(other)?;
    if is_tempo_chain(chain_id) {
        let fee_token = gas_fee_token.unwrap_or(TEMPO_DEFAULT_FEE_TOKEN);
        return submit_tempo(chain_id, safe, &inner, fee_token, keys, signer, quoted_fee);
    }
    submit_in_band(
        chain_id,
        safe,
        &inner,
        gas_fee_token,
        keys,
        signer,
        quoted_fee,
    )
}

/// Deployment status and the nonce, together, with the two refusals.
fn account_context(
    chain_id: u32,
    safe: &str,
    keys: &[WalletKey],
) -> Result<(bool, String, Vec<u8>), SubmitFailure> {
    chain::verify_chain_ready(chain_id).map_err(other)?;
    chain::forget_gas_price(chain_id);
    // …and the fee session's held readings (issue 212): a submit, and the
    // first quote after it, landed or not, measure again.
    crate::executor::fee_signals::invalidate(chain_id);
    let deployed = chain::is_deployed(safe, chain_id).map_err(other)?;
    let nonce = chain::nonce(safe, chain_id);
    // A deployed wallet MUST sign its real nonce; an undeployed one's IS 0.
    let nonce = if deployed {
        nonce.map_err(other)?
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
    eprintln!(
        "[vela-wallet] in-band fallback quote: token={} amount={amount} recipient={} gas={total_gas} price={gas_price}",
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

fn submit_in_band(
    chain_id: u32,
    safe: &str,
    inner: &[MultiSendCall],
    gas_fee_token: Option<&str>,
    keys: &[WalletKey],
    signer: Signer<'_>,
    quoted_fee: Option<QuotedFee>,
) -> Result<String, SubmitFailure> {
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
        Err(message) => {
            eprintln!("[vela-wallet] in-band: estimation failed, using defaults: {message}");
            if has_contract_call {
                return Err(other(
                    "Could not estimate gas for this transaction. The network may be busy — please try again.",
                ));
            }
        }
    }

    let fee = match usable(quoted_fee) {
        Some(fee) => {
            eprintln!(
                "[vela-wallet] in-band: signing DISPLAYED quote token={} amount={} recipient={}",
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
    sign_and_submit(op, chain_id, safe, inner, keys, signer, &[], fee.tier)
}

fn submit_tempo(
    chain_id: u32,
    safe: &str,
    inner: &[MultiSendCall],
    fee_token: &str,
    keys: &[WalletKey],
    signer: Signer<'_>,
    quoted_fee: Option<QuotedFee>,
) -> Result<String, SubmitFailure> {
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
        Err(message) => {
            eprintln!("[vela-wallet] tempo: estimation failed, using defaults: {message}");
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
    eprintln!(
        "[vela-wallet] tempo: feeToken={fee_token} reimbursement={reimbursement} calculated={calculated} realisticGas={realistic_gas} collector={collector}"
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
    )
}

/// The shared tail: hash, sign, envelope, submit (with the AA20 guard and
/// the in-flight-hash recovery), bump the nonce. `inner` is the person's
/// calls — the operation carries them and then its fee leg.
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
) -> Result<String, SubmitFailure> {
    let safe_op_hash =
        calculate_safe_op_hash(&op, u64::from(chain_id)).map_err(|e| other(e.to_string()))?;
    let assertion = signer.sign(&safe_op_hash, chain_id, safe, keys, Some((&op, inner)))?;
    op.signature = envelope(&assertion, keys)?;

    // The AA20 guard: an undeployed sender with no initCode is a guaranteed
    // on-chain failure that strands funds silently. The read is cache-hot on
    // a deployed account, so this costs nothing where it does not fire.
    if op.init_code.len() < 20 && !chain::is_deployed(safe, chain_id).map_err(other)? {
        return Err(other(
            "This account is not deployed on this network yet and the transaction is missing its deployment step. Please try again.",
        ));
    }

    let hash = match relay::send_user_op(&op, chain_id, extra, tier) {
        Ok(hash) => hash,
        Err(relay::SubmitError::Rejected(message)) => {
            // A previous op is still pending: poll ITS receipt instead of failing.
            if let Some(existing) = parse_existing_user_op_hash(&message) {
                eprintln!("[vela-wallet] relay: previous op pending ({existing}), tracking it");
                return Ok(existing);
            }
            return Err(classify_rejection(message));
        }
        Err(relay::SubmitError::Unreachable) => {
            return Err(other(
                "The gas relayer could not be reached. Please try again.",
            ));
        }
    };
    chain::bump_nonce(safe, chain_id);
    Ok(hash)
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
) -> Result<String, SubmitFailure> {
    let challenge = compute_safe_message_hash(original_hash, u64::from(chain_id), safe)
        .map_err(|e| other(e.to_string()))?;
    let assertion = signer.sign(&challenge, chain_id, safe, keys, None)?;
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

/// The relay's sentence, classified the way `classifySubmit` does.
fn classify_rejection(message: String) -> SubmitFailure {
    if message
        .to_lowercase()
        .contains("gas relayer is unavailable")
    {
        return SubmitFailure::RelayerUnavailable;
    }
    if relay::is_bundler_underfunded(&message) {
        return SubmitFailure::BundlerUnderfunded;
    }
    eprintln!("[vela-wallet] send: unhandled relay error: {message}");
    other(message)
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
            eprintln!(
                "[vela-wallet] callGasLimit raised to the inner calls' own estimate bundler={} inner={floor}",
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
        }
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
        assert_eq!(
            classify_rejection("The gas relayer is unavailable right now.".to_owned()),
            SubmitFailure::RelayerUnavailable
        );
        assert_eq!(
            classify_rejection("dedicated bundler gas account is short".to_owned()),
            SubmitFailure::BundlerUnderfunded
        );
        assert!(matches!(
            classify_rejection("AA25 invalid account nonce".to_owned()),
            SubmitFailure::Other(_)
        ));
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
        let signed = {
            let mut signer = Signer::TrustedSigner {
                ask: &ask,
                page: "https://sign.getvela.app/",
                channel: &channel,
            };
            signer.sign(&digest, 100, &op.sender, &keys, Some((&op, &inner)))
        };
        let _ = answering.join();
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
            };
            signer.sign(&digest, 100, &op.sender, &keys, Some((&op, &inner)))
        };
        let _ = declining.join();
        assert_eq!(declined.err(), Some(SubmitFailure::PasskeyCancelled));
        assert_eq!(
            channel.ended(),
            Some(crate::executor::trusted_signer::Refusal::Closed)
        );
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
            },
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
