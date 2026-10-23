//! What the person reviewed, bound to what the passkey signs (spec 104,
//! the security review's follow-up).
//!
//! The passkey's own sheet shows no transaction: the review screen is the
//! only place the person sees what they are about to sign. So the review is
//! frozen when they confirm it ([`SignedReview`], from the send core), and the
//! hash the passkey signs is only ever computed for an operation whose calls,
//! decoded back out of its `callData`, are exactly the transfers and the fee
//! that review showed ([`check`], [`reviewed_safe_op_hash`]). Every shell
//! assembles the operation itself; none can sign one that drifted from the
//! review — a re-quote, a re-derived amount, a call nobody showed.
//!
//! Decoding is the exact inverse of [`crate::user_op`]'s encoders: one
//! `executeUserOp` CALL, or `executeUserOp(MultiSend, 0, multiSend(…),
//! DELEGATECALL)` whose packed calls are all CALLs. Anything else is refused.
//!
//! Pure: no I/O, no clock.

use alloy_primitives::U256;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use crate::error::CoreError;
use crate::primitives::{function_selector, parse_address, to_hex};
use crate::safe::MULTI_SEND;
use crate::user_op::{calculate_safe_op_hash, UserOperation};

/// One transfer the review showed: `amount` of `token` (`None`: the chain's
/// native coin) to `to`, in base units as a decimal string.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ReviewedTransfer {
    pub to: String,
    pub token: Option<String>,
    pub amount: String,
}

/// The fee the review showed, paid in band: `amount` of `token` (`None`:
/// native) to the relay's `recipient`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ReviewedFee {
    pub recipient: String,
    pub token: Option<String>,
    pub amount: String,
}

/// The review the person confirmed: who sends, on which chain, every
/// transfer in the order the operation carries them, and the fee.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SignedReview {
    pub chain_id: u32,
    pub account: String,
    pub transfers: Vec<ReviewedTransfer>,
    pub fee: Option<ReviewedFee>,
}

/// A transfer as it is compared: lowercase addresses, the amount as a number.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Leg {
    to: String,
    token: Option<String>,
    amount: U256,
}

impl std::fmt::Display for Leg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.token {
            None => write!(f, "{} native to {}", self.amount, self.to),
            Some(token) => write!(f, "{} of {token} to {}", self.amount, self.to),
        }
    }
}

fn address(value: &str) -> Result<String, CoreError> {
    Ok(to_hex(parse_address(value.trim())?.as_slice(), true))
}

fn amount(value: &str) -> Result<U256, CoreError> {
    U256::from_str_radix(value.trim(), 10)
        .map_err(|e| CoreError::InvalidQuantity(format!("`{value}`: {e}")))
}

fn leg(to: &str, token: Option<&str>, value: &str) -> Result<Leg, CoreError> {
    Ok(Leg {
        to: address(to)?,
        token: token.map(address).transpose()?,
        amount: amount(value)?,
    })
}

impl SignedReview {
    /// The legs the operation must carry, in order: the transfers, then the
    /// fee.
    fn legs(&self) -> Result<Vec<Leg>, CoreError> {
        let mut legs = self
            .transfers
            .iter()
            .map(|t| leg(&t.to, t.token.as_deref(), &t.amount))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(fee) = &self.fee {
            legs.push(leg(&fee.recipient, fee.token.as_deref(), &fee.amount)?);
        }
        Ok(legs)
    }

    /// The review as one value: a hex SHA-256 over its normalized form, the
    /// same on every shell. What a confirm names to say which review it was
    /// drawn for.
    #[must_use]
    pub fn digest(&self) -> String {
        let account = address(&self.account).unwrap_or_else(|_| self.account.to_lowercase());
        let mut text = format!("vela.review.v1\n{}\n{account}\n", self.chain_id);
        match self.legs() {
            Ok(legs) => {
                for leg in legs {
                    text.push_str(&format!(
                        "{}|{}|{}\n",
                        leg.to,
                        leg.token.as_deref().unwrap_or("native"),
                        leg.amount
                    ));
                }
            }
            // Unreadable: digested as written, so it still differs from any
            // readable review.
            Err(_) => text.push_str(&format!("{self:?}")),
        }
        if self.fee.is_none() {
            text.push_str("no fee\n");
        }
        to_hex(&Sha256::digest(text.as_bytes()), true)
    }
}

/// One call the operation makes, decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Call {
    to: String,
    value: U256,
    data: Vec<u8>,
}

fn decode_error(why: &str) -> CoreError {
    CoreError::AbiDecode(format!("not an operation this wallet builds: {why}"))
}

fn word(bytes: &[u8], at: usize) -> Result<&[u8], CoreError> {
    bytes
        .get(at..at + 32)
        .ok_or_else(|| decode_error("cut short"))
}

fn word_address(bytes: &[u8], at: usize) -> Result<String, CoreError> {
    let w = word(bytes, at)?;
    if w[..12].iter().any(|b| *b != 0) {
        return Err(decode_error("an address word with high bytes set"));
    }
    Ok(to_hex(&w[12..], true))
}

fn word_small(bytes: &[u8], at: usize) -> Result<usize, CoreError> {
    let value = U256::from_be_slice(word(bytes, at)?);
    usize::try_from(value).map_err(|_| decode_error("a length out of range"))
}

/// `len` bytes of data at `at`, then zero padding to the word — and nothing
/// after it.
fn tail(bytes: &[u8], at: usize, len: usize) -> Result<Vec<u8>, CoreError> {
    let end = at
        .checked_add(len)
        .ok_or_else(|| decode_error("a length out of range"))?;
    let data = bytes
        .get(at..end)
        .ok_or_else(|| decode_error("data cut short"))?;
    let padded = end + (32 - len % 32) % 32;
    if bytes.len() != padded || bytes[end..].iter().any(|b| *b != 0) {
        return Err(decode_error("bytes after the data"));
    }
    Ok(data.to_vec())
}

/// `executeUserOp(address,uint256,bytes,uint8)`: its target, value, data and
/// operation (0 CALL, 1 DELEGATECALL).
fn execute_user_op(call_data: &[u8]) -> Result<(String, U256, Vec<u8>, u8), CoreError> {
    let selector = function_selector("executeUserOp(address,uint256,bytes,uint8)")?;
    if call_data.get(..4) != Some(selector.as_slice()) {
        return Err(decode_error("not executeUserOp"));
    }
    let body = &call_data[4..];
    let to = word_address(body, 0)?;
    let value = U256::from_be_slice(word(body, 32)?);
    if word_small(body, 64)? != 128 {
        return Err(decode_error("data not where executeUserOp puts it"));
    }
    let operation = match word_small(body, 96)? {
        0 => 0,
        1 => 1,
        _ => {
            return Err(decode_error(
                "an operation that is neither CALL nor DELEGATECALL",
            ))
        }
    };
    let len = word_small(body, 128)?;
    let data = tail(body, 160, len)?;
    Ok((to, value, data, operation))
}

/// `multiSend(bytes)`'s packed calls, every one a CALL.
fn multi_send(payload: &[u8]) -> Result<Vec<Call>, CoreError> {
    let selector = function_selector("multiSend(bytes)")?;
    if payload.get(..4) != Some(selector.as_slice()) {
        return Err(decode_error("a DELEGATECALL that is not multiSend"));
    }
    let body = &payload[4..];
    if word_small(body, 0)? != 32 {
        return Err(decode_error("multiSend's bytes not where it puts them"));
    }
    let len = word_small(body, 32)?;
    let packed = tail(body, 64, len)?;
    let mut calls = Vec::new();
    let mut at = 0;
    while at < packed.len() {
        let head = packed
            .get(at..at + 85)
            .ok_or_else(|| decode_error("a packed call cut short"))?;
        if head[0] != 0 {
            return Err(decode_error("a DELEGATECALL inside the batch"));
        }
        let to = to_hex(&head[1..21], true);
        let value = U256::from_be_slice(&head[21..53]);
        let data_len = usize::try_from(U256::from_be_slice(&head[53..85]))
            .map_err(|_| decode_error("a length out of range"))?;
        let start = at + 85;
        let data = packed
            .get(start..start + data_len)
            .ok_or_else(|| decode_error("a packed call's data cut short"))?;
        calls.push(Call {
            to,
            value,
            data: data.to_vec(),
        });
        at = start + data_len;
    }
    Ok(calls)
}

/// Every call the operation makes.
fn calls(call_data: &[u8]) -> Result<Vec<Call>, CoreError> {
    let (to, value, data, operation) = execute_user_op(call_data)?;
    if operation == 0 {
        return Ok(vec![Call { to, value, data }]);
    }
    if to != address(MULTI_SEND)? || !value.is_zero() {
        return Err(decode_error(
            "a DELEGATECALL to something other than MultiSend",
        ));
    }
    multi_send(&data)
}

/// A call as a transfer: a native value with no data, or an ERC-20
/// `transfer(address,uint256)` with no value. Anything else is not one.
fn as_leg(call: &Call) -> Result<Leg, CoreError> {
    if call.data.is_empty() {
        return Ok(Leg {
            to: call.to.clone(),
            token: None,
            amount: call.value,
        });
    }
    let transfer = function_selector("transfer(address,uint256)")?;
    if call.data.len() == 68 && call.data[..4] == transfer[..] && call.value.is_zero() {
        return Ok(Leg {
            to: word_address(&call.data, 4)?,
            token: Some(call.to.clone()),
            amount: U256::from_be_slice(word(&call.data, 36)?),
        });
    }
    Err(CoreError::Internal(format!(
        "the operation makes a call the review did not show: to {}, data {}",
        call.to,
        to_hex(call.data.get(..4).unwrap_or(&call.data), true)
    )))
}

/// Is `op`, on `chain_id`, exactly the transfer the person reviewed? The
/// sender, the chain, and every call — the transfers in order, then the
/// fee — decoded from its `callData`. A mismatch is the core's own
/// invariant broken ("displayed = signed"): `Internal`, and nothing is
/// signed.
pub fn check(op: &UserOperation, chain_id: u64, review: &SignedReview) -> Result<(), CoreError> {
    let broken = |why: String| CoreError::Internal(format!("not the reviewed transfer: {why}"));
    if u64::from(review.chain_id) != chain_id {
        return Err(broken(format!(
            "chain {chain_id}, reviewed on {}",
            review.chain_id
        )));
    }
    if address(&op.sender)? != address(&review.account)? {
        return Err(broken(format!(
            "sent from {}, reviewed from {}",
            op.sender, review.account
        )));
    }
    let made = calls(&op.call_data)?
        .iter()
        .map(as_leg)
        .collect::<Result<Vec<_>, _>>()?;
    let shown = review.legs()?;
    if made.len() != shown.len() {
        return Err(broken(format!(
            "{} calls, the review showed {}",
            made.len(),
            shown.len()
        )));
    }
    for (index, (made, shown)) in made.iter().zip(&shown).enumerate() {
        if made != shown {
            return Err(broken(format!(
                "call {}: {made}, the review showed {shown}",
                index + 1
            )));
        }
    }
    Ok(())
}

/// The review as the send core hands it down (`SubmitUserOp.review`), JSON.
pub fn from_json(review_json: &str) -> Result<SignedReview, CoreError> {
    serde_json::from_str(review_json).map_err(|e| CoreError::Internal(format!("not a review: {e}")))
}

/// The SafeOp hash the passkey signs ([`calculate_safe_op_hash`]) — only for
/// an operation that is the reviewed transfer ([`check`]).
pub fn reviewed_safe_op_hash(
    op: &UserOperation,
    chain_id: u64,
    review: &SignedReview,
) -> Result<Vec<u8>, CoreError> {
    check(op, chain_id, review)?;
    calculate_safe_op_hash(op, chain_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::user_op::{
        build_execute_call_data, build_in_band_fee_leg, build_multi_send_execute_call_data,
        encode_erc20_transfer, in_band_batch, MultiSendCall,
    };

    const SAFE: &str = "0x1100000000000000000000000000000000045aC0";
    const BOB: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    const CAROL: &str = "0x2222222222222222222222222222222222222222";
    const USDC: &str = "0x3600000000000000000000000000000000000000";
    const RELAY: &str = "0x9999999999999999999999999999999999999999";

    fn ok<T>(result: Result<T, CoreError>) -> T {
        result.unwrap_or_else(|error| unreachable!("{error}"))
    }

    /// Why it was refused — `unreachable` when it was not.
    fn refused_with(result: Result<(), CoreError>) -> CoreError {
        match result {
            Err(error) => error,
            Ok(()) => unreachable!("not refused"),
        }
    }

    fn operation(call_data: Vec<u8>) -> UserOperation {
        UserOperation {
            sender: SAFE.to_owned(),
            nonce: "0x1".to_owned(),
            init_code: Vec::new(),
            call_data,
            verification_gas_limit: 1,
            call_gas_limit: 1,
            pre_verification_gas: 1,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            paymaster_and_data: Vec::new(),
            signature: Vec::new(),
        }
    }

    fn native(to: &str, amount: u128) -> MultiSendCall {
        MultiSendCall {
            to: to.to_owned(),
            value_hex: format!("0x{amount:x}"),
            data: Vec::new(),
        }
    }

    fn usdc(to: &str, amount: u128) -> MultiSendCall {
        MultiSendCall {
            to: USDC.to_owned(),
            value_hex: "0".to_owned(),
            data: ok(encode_erc20_transfer(to, amount)),
        }
    }

    type Shown<'a> = &'a [(&'a str, Option<&'a str>, u128)];

    fn review(transfers: Shown<'_>, fee: Option<(Option<&str>, u128)>) -> SignedReview {
        SignedReview {
            chain_id: 56,
            account: SAFE.to_lowercase(),
            transfers: transfers
                .iter()
                .map(|(to, token, amount)| ReviewedTransfer {
                    to: (*to).to_owned(),
                    token: token.map(str::to_owned),
                    amount: amount.to_string(),
                })
                .collect(),
            fee: fee.map(|(token, amount)| ReviewedFee {
                recipient: RELAY.to_owned(),
                token: token.map(str::to_owned),
                amount: amount.to_string(),
            }),
        }
    }

    /// The operations this wallet builds: a batch with its in-band fee leg.
    fn built(inner: &[MultiSendCall], fee_token: Option<&str>, fee: u128) -> UserOperation {
        let calls = ok(in_band_batch(inner, fee_token, RELAY, fee));
        operation(ok(build_multi_send_execute_call_data(&calls)))
    }

    #[test]
    fn the_operation_the_wallet_builds_is_the_review() {
        let shown = review(&[(BOB, None, 1_000)], Some((None, 14_000)));
        let op = built(&[native(BOB, 1_000)], None, 14_000);
        assert_eq!(check(&op, 56, &shown), Ok(()));
        assert_eq!(
            ok(reviewed_safe_op_hash(&op, 56, &shown)),
            ok(calculate_safe_op_hash(&op, 56))
        );

        // Several recipients of a token, its fee in the same token.
        let shown = review(
            &[(BOB, Some(USDC), 10), (CAROL, Some(USDC), 20)],
            Some((Some(USDC), 3)),
        );
        let op = built(&[usdc(BOB, 10), usdc(CAROL, 20)], Some(USDC), 3);
        assert_eq!(check(&op, 56, &shown), Ok(()));

        // A lone call, no fee leg: one executeUserOp CALL.
        let shown = review(&[(BOB, Some(USDC), 10)], None);
        let single = usdc(BOB, 10);
        let op = operation(ok(build_execute_call_data(
            &single.to,
            &single.value_hex,
            &single.data,
        )));
        assert_eq!(check(&op, 56, &shown), Ok(()));
    }

    #[test]
    fn an_operation_that_drifted_from_the_review_is_not_signed() {
        let shown = review(&[(BOB, None, 1_000)], Some((None, 14_000)));
        let refused = |op: &UserOperation, chain: u64| {
            let Err(error) = reviewed_safe_op_hash(op, chain, &shown) else {
                unreachable!("an operation that drifted was hashed");
            };
            assert!(
                matches!(error, CoreError::Internal(_) | CoreError::AbiDecode(_)),
                "{error:?}"
            );
            error.to_string()
        };
        // Another amount, another recipient, another token.
        refused(&built(&[native(BOB, 1_001)], None, 14_000), 56);
        let why = refused(&built(&[native(CAROL, 1_000)], None, 14_000), 56);
        assert!(why.contains("call 1"), "{why}");
        refused(&built(&[usdc(BOB, 1_000)], None, 14_000), 56);
        // A fee re-quoted after the review, or paid in another coin.
        refused(&built(&[native(BOB, 1_000)], None, 28_000), 56);
        refused(&built(&[native(BOB, 1_000)], Some(USDC), 14_000), 56);
        // A transfer, or a fee leg, more or less than shown.
        refused(
            &built(&[native(BOB, 1_000), native(CAROL, 1)], None, 14_000),
            56,
        );
        let no_fee = operation(ok(build_multi_send_execute_call_data(&[native(
            BOB, 1_000,
        )])));
        refused(&no_fee, 56);
        // Another chain, another sender.
        refused(&built(&[native(BOB, 1_000)], None, 14_000), 1);
        let mut other = built(&[native(BOB, 1_000)], None, 14_000);
        other.sender = CAROL.to_owned();
        refused(&other, 56);
    }

    #[test]
    fn a_call_the_review_did_not_show_is_refused() {
        let shown = review(&[(BOB, Some(USDC), 10)], Some((Some(USDC), 3)));
        // An approve where a transfer was shown.
        let mut approve = ok(function_selector("approve(address,uint256)"));
        approve.extend_from_slice(&ok(encode_erc20_transfer(BOB, 10))[4..]);
        let call = MultiSendCall {
            to: USDC.to_owned(),
            value_hex: "0".to_owned(),
            data: approve,
        };
        let fee = ok(build_in_band_fee_leg(Some(USDC), RELAY, 3));
        let op = operation(ok(build_multi_send_execute_call_data(&[call, fee])));
        let why = refused_with(check(&op, 56, &shown)).to_string();
        assert!(why.contains("did not show"), "{why}");

        // A transfer that also carries value.
        let mut paid = usdc(BOB, 10);
        paid.value_hex = "0x1".to_owned();
        let fee = ok(build_in_band_fee_leg(Some(USDC), RELAY, 3));
        let op = operation(ok(build_multi_send_execute_call_data(&[paid, fee])));
        assert!(check(&op, 56, &shown).is_err());
    }

    #[test]
    fn only_the_shapes_this_wallet_builds_are_read() {
        let shown = review(&[(BOB, None, 1_000)], None);
        let single = || ok(build_execute_call_data(BOB, "0x3e8", &[]));
        // A DELEGATECALL to anything but MultiSend.
        let mut call_data = single();
        call_data[4 + 96 + 31] = 1;
        assert!(matches!(
            refused_with(check(&operation(call_data), 56, &shown)),
            CoreError::AbiDecode(_)
        ));
        // A DELEGATECALL inside the batch.
        let mut call_data = ok(build_multi_send_execute_call_data(&[native(BOB, 1_000)]));
        call_data[4 + 160 + 4 + 64] = 1;
        assert!(matches!(
            refused_with(check(&operation(call_data), 56, &shown)),
            CoreError::AbiDecode(_)
        ));
        // Another selector, bytes after the data, a cut.
        let mut call_data = single();
        call_data[0] ^= 0xff;
        assert!(check(&operation(call_data), 56, &shown).is_err());
        let mut call_data = single();
        call_data.extend([0u8; 32]);
        assert!(check(&operation(call_data), 56, &shown).is_err());
        assert!(check(&operation(single()[..100].to_vec()), 56, &shown).is_err());
        // The untouched one reads.
        assert_eq!(check(&operation(single()), 56, &shown), Ok(()));
    }

    #[test]
    fn the_digest_names_one_review() {
        let a = review(&[(BOB, None, 1_000)], Some((None, 14_000)));
        let mut same = a.clone();
        same.transfers[0].to = BOB.to_lowercase();
        same.account = SAFE.to_owned();
        assert_eq!(a.digest(), same.digest(), "spelling is not content");
        let mut more = a.clone();
        more.transfers[0].amount = "1001".to_owned();
        assert_ne!(a.digest(), more.digest());
        let mut fee = a.clone();
        fee.fee = None;
        assert_ne!(a.digest(), fee.digest());
        let mut chain = a.clone();
        chain.chain_id = 1;
        assert_ne!(a.digest(), chain.digest());
        assert_eq!(a.digest().len(), 66);
    }
}
