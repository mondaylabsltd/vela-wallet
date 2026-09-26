//! Rules of the approval guard, one test per rule.
//!
//! The pure sections (detect / rewrite / enforce / parse / format) replay the
//! full jest vector set from `src/__tests__/services/approval-guard.test.ts` —
//! the existing regression net for the "unlimited can never leave the wallet"
//! invariant. The machine sections cover the editor choice derivation, the
//! reads, the batch per-leg gating and stale-result handling.

#![cfg(feature = "crux")]

mod support;

use alloy_primitives::U256;
use serde_json::{json, Value};
use support::DomainDriver;
use vela_core::app::approval_guard::{
    detect_approval, enforce_no_unlimited, format_token_amount, is_unbounded_amount,
    leg_needs_choice, parse_token_amount, rewrite_approval_params, AmountBits, ApprovalGuard,
    Event, GuardAmountError, GuardApprovalKind, GuardBlockReason, GuardChoice, GuardEditorMode,
    GuardLocus, GuardOperation as Op, GuardRewriteError, GuardShellResult as Res, GuardSurface,
    GuardTokenMetaEntry, UNLIMITED_CAP_160, UNLIMITED_CAP_256,
};

type Sut = DomainDriver<ApprovalGuard>;

// ---------------------------------------------------------------------------
// Fixtures — byte-for-byte the jest fixtures
// ---------------------------------------------------------------------------

const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const SPENDER: &str = "0x111111125421cA6dc452d289314280a0f8842A65";
const PERMIT2: &str = "0x000000000022D473030F116dDEE9F6B43aC78BA3";
const WALLET: &str = "0x00000000000000000000000000000000000000aa";

/// 2026-08-09, epoch ms — any time after the 1750000000s fixtures' deadline.
const NOW_MS: f64 = 1_754_700_000_000.0;

fn max_u256() -> U256 {
    U256::MAX
}

fn max_u160() -> U256 {
    (U256::from(1u8) << 160) - U256::from(1u8)
}

fn addr_word(addr: &str) -> String {
    let clean = addr.trim_start_matches("0x").to_lowercase();
    format!("{}{clean}", "0".repeat(64 - clean.len()))
}

fn amt_word(v: U256) -> String {
    let hex = format!("{v:x}");
    format!("{}{hex}", "0".repeat(64 - hex.len()))
}

fn approve_calldata(spender: &str, amount: U256) -> String {
    format!("0x095ea7b3{}{}", addr_word(spender), amt_word(amount))
}

fn increase_calldata(spender: &str, amount: U256) -> String {
    format!("0x39509351{}{}", addr_word(spender), amt_word(amount))
}

fn decrease_calldata(spender: &str, amount: U256) -> String {
    format!("0xa457c2d7{}{}", addr_word(spender), amt_word(amount))
}

fn set_approval_for_all_calldata(operator: &str, approved: bool) -> String {
    format!(
        "0xa22cb465{}{}",
        addr_word(operator),
        amt_word(U256::from(u8::from(approved)))
    )
}

fn permit2_calldata(token: &str, spender: &str, amount: U256) -> String {
    format!(
        "0x87517c45{}{}{}{}",
        addr_word(token),
        addr_word(spender),
        amt_word(amount),
        amt_word(U256::from(1_750_000_000u64))
    )
}

fn tx_params(to: &str, data: &str) -> Value {
    json!([{ "to": to, "data": data, "value": "0x0" }])
}

fn erc2612_params(value: &str) -> Value {
    let td = json!({
        "types": { "Permit": [
            { "name": "owner", "type": "address" }, { "name": "spender", "type": "address" },
            { "name": "value", "type": "uint256" }, { "name": "nonce", "type": "uint256" },
            { "name": "deadline", "type": "uint256" },
        ] },
        "primaryType": "Permit",
        "domain": { "name": "USD Coin", "chainId": 1, "verifyingContract": USDC },
        "message": { "owner": "0xaf5e", "spender": SPENDER, "value": value, "nonce": "0", "deadline": "1750000000" },
    });
    json!(["0x0", td.to_string()])
}

fn dai_permit_params(allowed: bool) -> Value {
    let td = json!({
        "types": { "Permit": [
            { "name": "holder", "type": "address" }, { "name": "spender", "type": "address" },
            { "name": "nonce", "type": "uint256" }, { "name": "expiry", "type": "uint256" },
            { "name": "allowed", "type": "bool" },
        ] },
        "primaryType": "Permit",
        "domain": { "name": "Dai", "verifyingContract": "0x6b175474e89094c44da98b954eedeac495271d0f" },
        "message": { "holder": "0xaf5e", "spender": SPENDER, "nonce": "0", "expiry": "1750000000", "allowed": allowed },
    });
    json!(["0x0", td.to_string()])
}

fn permit2_single_params(amount: &str) -> Value {
    let td = json!({
        "types": { "PermitSingle": [], "PermitDetails": [] },
        "primaryType": "PermitSingle",
        "domain": { "name": "Permit2", "chainId": 1, "verifyingContract": PERMIT2 },
        "message": {
            "details": { "token": USDC, "amount": amount, "expiration": "1750000000", "nonce": "0" },
            "spender": SPENDER, "sigDeadline": "1750000000",
        },
    });
    json!(["0x0", td.to_string()])
}

const TYPED: &str = "eth_signTypedData_v4";
const TX: &str = "eth_sendTransaction";

fn detect(method: &str, params: &Value) -> vela_core::app::approval_guard::GuardDetectedApproval {
    detect_approval(method, Some(params)).expect("approval expected")
}

fn redetect_data(data: &str) -> vela_core::app::approval_guard::GuardDetectedApproval {
    detect(TX, &tx_params(USDC, data))
}

fn amount_choice(raw: U256) -> GuardChoice {
    GuardChoice::Amount {
        amount_raw: raw.to_string(),
    }
}

fn out_data(out: &Value) -> String {
    out[0]["data"].as_str().expect("rewritten data").to_owned()
}

// ---------------------------------------------------------------------------
// Constants pinned
// ---------------------------------------------------------------------------

/// The caps are 2^200 (uint256 fields) and 2^152 (uint160 fields) — the
/// separating line between "big finite" and "unlimited".
#[test]
fn cap_constants_are_2_pow_200_and_2_pow_152() {
    assert_eq!(UNLIMITED_CAP_256, U256::from(1u8) << 200);
    assert_eq!(UNLIMITED_CAP_160, U256::from(1u8) << 152);
}

/// The hardcoded 4-byte selectors match the canonical keccak selectors from
/// `primitives::function_selector` — the same source the TS `sel()` uses.
#[test]
fn selectors_match_canonical_signatures() {
    let sel = |sig: &str| {
        vela_core::primitives::to_hex(
            &vela_core::primitives::function_selector(sig).expect("selector"),
            true,
        )
    };
    assert_eq!(sel("approve(address,uint256)"), "0x095ea7b3");
    assert_eq!(sel("increaseAllowance(address,uint256)"), "0x39509351");
    assert_eq!(sel("decreaseAllowance(address,uint256)"), "0xa457c2d7");
    assert_eq!(sel("setApprovalForAll(address,bool)"), "0xa22cb465");
    assert_eq!(sel("approve(address,address,uint160,uint48)"), "0x87517c45");
}

// ---------------------------------------------------------------------------
// isUnboundedAmount (jest vectors)
// ---------------------------------------------------------------------------

#[test]
fn uint256_sentinels_are_unbounded() {
    assert!(is_unbounded_amount(max_u256(), AmountBits::B256));
    assert!(is_unbounded_amount(
        U256::from(1u8) << 255,
        AmountBits::B256
    ));
    assert!(is_unbounded_amount(UNLIMITED_CAP_256, AmountBits::B256));
}

#[test]
fn uint160_sentinel_is_unbounded() {
    assert!(is_unbounded_amount(max_u160(), AmountBits::B160));
    assert!(is_unbounded_amount(UNLIMITED_CAP_160, AmountBits::B160));
}

#[test]
fn large_but_legit_amounts_are_not_unbounded() {
    // 1 quadrillion tokens * 1e18 ≈ 2^110 — well under the cap.
    let big = U256::from(10u8).pow(U256::from(33u8));
    assert!(!is_unbounded_amount(big, AmountBits::B256));
    assert!(!is_unbounded_amount(big, AmountBits::B160));
    assert!(!is_unbounded_amount(
        U256::from(500_000_000u64),
        AmountBits::B256
    ));
}

/// A "cap at balance" (issue #86) is always finite — even a whale balance.
#[test]
fn whale_balance_caps_are_always_finite() {
    let whale_usdt = U256::from(1_000_000_000u64) * U256::from(10u64).pow(U256::from(6u8));
    let whale_eth = U256::from(1_000_000_000_000u64) * U256::from(10u64).pow(U256::from(18u8));
    for bits in [AmountBits::B256, AmountBits::B160] {
        assert!(!is_unbounded_amount(whale_usdt, bits));
        assert!(!is_unbounded_amount(whale_eth, bits));
    }
}

// ---------------------------------------------------------------------------
// detectApproval — calldata (jest vectors)
// ---------------------------------------------------------------------------

#[test]
fn detects_unlimited_erc20_approve() {
    let d = detect(TX, &tx_params(USDC, &approve_calldata(SPENDER, max_u256())));
    assert_eq!(d.kind, GuardApprovalKind::Erc20Approve);
    assert_eq!(
        d.token_address.as_deref(),
        Some(USDC.to_lowercase().as_str())
    );
    assert_eq!(d.spender, SPENDER.to_lowercase());
    assert!(d.is_unbounded);
    assert_eq!(d.amount_bits, Some(256));
    assert!(!d.is_boolean_grant);
    assert!(d.editable);
    assert_eq!(
        d.amount_raw.as_deref(),
        Some(max_u256().to_string().as_str())
    );
    assert_eq!(d.locus, GuardLocus::CalldataWord { word_index: 1 });
}

#[test]
fn detects_limited_erc20_approve() {
    let d = detect(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, U256::from(500_000_000u64))),
    );
    assert!(!d.is_unbounded);
    assert_eq!(d.amount_raw.as_deref(), Some("500000000"));
}

#[test]
fn approve_to_zero_is_reducing() {
    let d = detect(TX, &tx_params(USDC, &approve_calldata(SPENDER, U256::ZERO)));
    assert!(d.is_reducing);
}

#[test]
fn detects_increase_allowance() {
    let d = detect(
        TX,
        &tx_params(USDC, &increase_calldata(SPENDER, max_u256())),
    );
    assert_eq!(d.kind, GuardApprovalKind::IncreaseAllowance);
    assert!(d.is_unbounded);
}

#[test]
fn decrease_allowance_is_reducing_never_unbounded() {
    let d = detect(
        TX,
        &tx_params(USDC, &decrease_calldata(SPENDER, max_u256())),
    );
    assert_eq!(d.kind, GuardApprovalKind::DecreaseAllowance);
    assert!(d.is_reducing);
    assert!(!d.is_unbounded);
}

#[test]
fn detects_set_approval_for_all_grant() {
    let d = detect(
        TX,
        &tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true)),
    );
    assert_eq!(d.kind, GuardApprovalKind::SetApprovalForAll);
    assert!(d.is_boolean_grant);
    assert!(d.is_unbounded);
    assert!(!d.is_reducing);
}

#[test]
fn detects_set_approval_for_all_revoke() {
    let d = detect(
        TX,
        &tx_params(USDC, &set_approval_for_all_calldata(SPENDER, false)),
    );
    assert!(d.is_boolean_grant);
    assert!(!d.is_unbounded);
    assert!(d.is_reducing);
}

/// Permit2 on-chain approve: the token is the FIRST arg (not the tx `to`,
/// which is the Permit2 contract), and the amount is a uint160.
#[test]
fn detects_permit2_onchain_approve() {
    let d = detect(
        TX,
        &tx_params(PERMIT2, &permit2_calldata(USDC, SPENDER, max_u160())),
    );
    assert_eq!(d.kind, GuardApprovalKind::Permit2Single);
    assert_eq!(
        d.token_address.as_deref(),
        Some(USDC.to_lowercase().as_str())
    );
    assert_eq!(d.spender, SPENDER.to_lowercase());
    assert_eq!(d.amount_bits, Some(160));
    assert!(d.is_unbounded);
    assert!(d.editable);
    assert_eq!(d.deadline.as_deref(), Some("1750000000"));
    assert_eq!(d.locus, GuardLocus::CalldataWord { word_index: 2 });
}

#[test]
fn non_approval_calldata_detects_nothing() {
    // transfer(address,uint256)
    let transfer = format!(
        "0xa9059cbb{}{}",
        addr_word(SPENDER),
        amt_word(U256::from(1000u64))
    );
    assert!(detect_approval(TX, Some(&tx_params(USDC, &transfer))).is_none());
}

#[test]
fn plain_eth_send_detects_nothing() {
    let params = json!([{ "to": SPENDER, "data": "0x", "value": "0x1" }]);
    assert!(detect_approval(TX, Some(&params)).is_none());
    assert!(detect_approval(TX, Some(&json!([]))).is_none());
}

// ---------------------------------------------------------------------------
// detectApproval — typed data (jest vectors)
// ---------------------------------------------------------------------------

#[test]
fn detects_unlimited_erc2612_permit() {
    let d = detect(TYPED, &erc2612_params(&max_u256().to_string()));
    assert_eq!(d.kind, GuardApprovalKind::Erc2612Permit);
    assert_eq!(
        d.token_address.as_deref(),
        Some(USDC.to_lowercase().as_str())
    );
    assert!(d.is_unbounded);
    assert_eq!(d.amount_bits, Some(256));
    assert_eq!(d.deadline.as_deref(), Some("1750000000"));
    assert!(!d.editable, "off-chain permits are never editable");
    assert_eq!(d.block_reason, Some(GuardBlockReason::OffChainPermit));
}

#[test]
fn detects_finite_erc2612_permit() {
    let d = detect(TYPED, &erc2612_params("1000000000"));
    assert!(!d.is_unbounded);
    assert_eq!(d.amount_raw.as_deref(), Some("1000000000"));
}

#[test]
fn dai_permit_allowed_true_is_boolean_unbounded_grant() {
    let d = detect(TYPED, &dai_permit_params(true));
    assert_eq!(d.kind, GuardApprovalKind::DaiPermit);
    assert!(d.is_boolean_grant);
    assert!(d.is_unbounded);
    assert!(d.block_reason.is_some());
}

#[test]
fn dai_permit_allowed_false_is_revoke() {
    let d = detect(TYPED, &dai_permit_params(false));
    assert_eq!(d.kind, GuardApprovalKind::DaiPermit);
    assert!(d.is_reducing);
    assert!(!d.is_unbounded);
}

#[test]
fn detects_unlimited_permit2_single_typed() {
    let d = detect(TYPED, &permit2_single_params(&max_u160().to_string()));
    assert_eq!(d.kind, GuardApprovalKind::Permit2Single);
    assert_eq!(d.amount_bits, Some(160));
    assert!(d.is_unbounded);
}

#[test]
fn detects_finite_permit2_single_typed() {
    let d = detect(TYPED, &permit2_single_params("1000000000"));
    assert!(!d.is_unbounded);
}

#[test]
fn malformed_typed_data_json_detects_nothing() {
    let params = json!(["0x0", "{not json"]);
    assert!(detect_approval(TYPED, Some(&params)).is_none());
}

// ---------------------------------------------------------------------------
// rewriteApprovalParams — calldata (jest vectors)
// ---------------------------------------------------------------------------

/// Caps an unlimited approve to a finite amount; spender preserved; no max
/// word remains anywhere in the calldata.
#[test]
fn rewrite_caps_unlimited_approve() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, max_u256()));
    let d = detect(TX, &params);
    let out = rewrite_approval_params(TX, &params, &d, &amount_choice(U256::from(500_000_000u64)))
        .expect("rewrite");
    let new_data = out_data(&out);
    let d2 = redetect_data(&new_data);
    assert_eq!(d2.amount_raw.as_deref(), Some("500000000"));
    assert_eq!(d2.spender, SPENDER.to_lowercase());
    assert!(!d2.is_unbounded);
    assert!(
        !new_data.to_lowercase().contains(&"f".repeat(64)),
        "no 2^256-1 word"
    );
}

#[test]
fn rewrite_revoke_sets_amount_to_zero() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, max_u256()));
    let d = detect(TX, &params);
    let out = rewrite_approval_params(TX, &params, &d, &GuardChoice::Revoke).expect("rewrite");
    let d2 = redetect_data(&out_data(&out));
    assert_eq!(d2.amount_raw.as_deref(), Some("0"));
}

/// Invariant ① — the rewrite itself refuses to emit an unbounded allowance.
#[test]
fn rewriting_to_an_unbounded_amount_errors() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, U256::from(100u64)));
    let d = detect(TX, &params);
    assert_eq!(
        rewrite_approval_params(TX, &params, &d, &amount_choice(max_u256())),
        Err(GuardRewriteError::UnlimitedAmount)
    );
    assert_eq!(
        rewrite_approval_params(TX, &params, &d, &amount_choice(UNLIMITED_CAP_256)),
        Err(GuardRewriteError::UnlimitedAmount)
    );
}

/// "Keep it as asked" is an identity on an unbounded request — the exact
/// bytes, Permit2's uint160 sentinel included — and meaningless (refused) on
/// a finite amount or a boolean grant.
#[test]
fn keeping_unlimited_is_an_identity_rewrite() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, max_u256()));
    let d = detect(TX, &params);
    let out = rewrite_approval_params(TX, &params, &d, &GuardChoice::Unlimited).expect("keep");
    assert_eq!(out_data(&out), approve_calldata(SPENDER, max_u256()));

    let permit2 = tx_params(PERMIT2, &permit2_calldata(USDC, SPENDER, max_u160()));
    let d = detect(TX, &permit2);
    let out = rewrite_approval_params(TX, &permit2, &d, &GuardChoice::Unlimited).expect("keep");
    assert_eq!(out_data(&out), permit2_calldata(USDC, SPENDER, max_u160()));

    let finite = tx_params(USDC, &approve_calldata(SPENDER, U256::from(100u64)));
    let d = detect(TX, &finite);
    assert_eq!(
        rewrite_approval_params(TX, &finite, &d, &GuardChoice::Unlimited),
        Err(GuardRewriteError::InvalidChoiceAmount)
    );

    let nft = tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true));
    let d = detect(TX, &nft);
    assert_eq!(
        rewrite_approval_params(TX, &nft, &d, &GuardChoice::Unlimited),
        Err(GuardRewriteError::AmountForBooleanShape)
    );
}

#[test]
fn rewrite_does_not_mutate_the_input_params() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, max_u256()));
    let before = params.to_string();
    let d = detect(TX, &params);
    let _ = rewrite_approval_params(TX, &params, &d, &amount_choice(U256::from(1u8)));
    assert_eq!(params.to_string(), before);
}

#[test]
fn rewrite_set_approval_for_all_revoke_flips_bool_to_false() {
    let params = tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true));
    let d = detect(TX, &params);
    let out = rewrite_approval_params(TX, &params, &d, &GuardChoice::Revoke).expect("rewrite");
    let d2 = redetect_data(&out_data(&out));
    assert!(!d2.is_unbounded);
    assert!(d2.is_reducing);
}

/// Invariant ② — only the target 32-byte word changes; a truncated calldata
/// cannot be rewritten at all.
#[test]
fn rewrite_touches_only_the_amount_word() {
    let data = approve_calldata(SPENDER, max_u256());
    let params = tx_params(USDC, &data);
    let d = detect(TX, &params);
    let out =
        rewrite_approval_params(TX, &params, &d, &amount_choice(U256::from(7u8))).expect("rewrite");
    let new_data = out_data(&out);
    // selector + spender word byte-identical; only the amount word moved.
    assert_eq!(&new_data[..8 + 2 + 64], &data[..8 + 2 + 64]);
    assert_eq!(new_data.len(), data.len());

    let short = &data[..data.len() - 2];
    let short_params = tx_params(USDC, short);
    assert_eq!(
        rewrite_approval_params(TX, &short_params, &d, &amount_choice(U256::from(7u8))),
        Err(GuardRewriteError::CalldataTooShort)
    );
}

#[test]
fn set_approval_for_all_has_no_amount_to_set() {
    let params = tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true));
    let d = detect(TX, &params);
    assert_eq!(
        rewrite_approval_params(TX, &params, &d, &amount_choice(U256::from(1u8))),
        Err(GuardRewriteError::AmountForBooleanShape)
    );
}

// ---------------------------------------------------------------------------
// rewriteApprovalParams — typed data (jest vectors)
// ---------------------------------------------------------------------------

#[test]
fn rewrite_caps_unlimited_erc2612_preserving_every_other_field() {
    let params = erc2612_params(&max_u256().to_string());
    let d = detect(TYPED, &params);
    let out = rewrite_approval_params(
        TYPED,
        &params,
        &d,
        &amount_choice(U256::from(1_000_000_000u64)),
    )
    .expect("rewrite");
    let td: Value =
        serde_json::from_str(out[1].as_str().expect("string typed data")).expect("json");
    assert_eq!(td["message"]["value"], json!("1000000000"));
    assert_eq!(td["message"]["spender"], json!(SPENDER));
    assert_eq!(td["message"]["nonce"], json!("0"));
    assert_eq!(td["message"]["deadline"], json!("1750000000"));
}

#[test]
fn rewrite_caps_unlimited_permit2_single_typed() {
    let params = permit2_single_params(&max_u160().to_string());
    let d = detect(TYPED, &params);
    let out = rewrite_approval_params(
        TYPED,
        &params,
        &d,
        &amount_choice(U256::from(1_000_000_000u64)),
    )
    .expect("rewrite");
    let td: Value =
        serde_json::from_str(out[1].as_str().expect("string typed data")).expect("json");
    assert_eq!(td["message"]["details"]["amount"], json!("1000000000"));
}

#[test]
fn rewrite_dai_permit_revoke_sets_allowed_false() {
    let params = dai_permit_params(true);
    let d = detect(TYPED, &params);
    let out = rewrite_approval_params(TYPED, &params, &d, &GuardChoice::Revoke).expect("rewrite");
    let td: Value =
        serde_json::from_str(out[1].as_str().expect("string typed data")).expect("json");
    assert_eq!(td["message"]["allowed"], json!(false));
}

#[test]
fn rewriting_a_typed_permit_to_unbounded_errors() {
    let params = erc2612_params("100");
    let d = detect(TYPED, &params);
    assert_eq!(
        rewrite_approval_params(TYPED, &params, &d, &amount_choice(max_u256())),
        Err(GuardRewriteError::UnlimitedAmount)
    );
}

// ---------------------------------------------------------------------------
// enforceNoUnlimited — the final guard (jest vectors; invariant ①)
// ---------------------------------------------------------------------------

#[test]
fn enforce_refuses_raw_unlimited_approve() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, max_u256()));
    let err = enforce_no_unlimited(TX, Some(&params)).expect_err("must refuse");
    assert_eq!(err.kind, GuardApprovalKind::Erc20Approve);
    assert_eq!(err.amount_raw, max_u256().to_string());
}

#[test]
fn enforce_refuses_unlimited_increase_allowance() {
    let params = tx_params(USDC, &increase_calldata(SPENDER, max_u256()));
    assert!(enforce_no_unlimited(TX, Some(&params)).is_err());
}

/// Invariant ③ — off-chain permit SIGNATURES are signed verbatim under
/// deliberate UI consent: the dApp redeems its own struct, so a forced cap
/// would only desync the signature and revert the swap.
#[test]
fn enforce_allows_unlimited_erc2612_offchain_signature() {
    let params = erc2612_params(&max_u256().to_string());
    assert!(enforce_no_unlimited(TYPED, Some(&params)).is_ok());
}

#[test]
fn enforce_allows_unlimited_permit2_single_offchain_signature() {
    let params = permit2_single_params(&max_u160().to_string());
    assert!(enforce_no_unlimited(TYPED, Some(&params)).is_ok());
}

/// The on-chain Permit2 approve is CALLDATA the wallet submits — refused.
#[test]
fn enforce_refuses_unlimited_permit2_onchain_approve() {
    let params = tx_params(PERMIT2, &permit2_calldata(USDC, SPENDER, max_u160()));
    assert!(enforce_no_unlimited(TX, Some(&params)).is_err());
}

#[test]
fn enforce_allows_a_finite_approve() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, U256::from(500_000_000u64)));
    assert!(enforce_no_unlimited(TX, Some(&params)).is_ok());
}

#[test]
fn enforce_allows_decrease_allowance_max() {
    let params = tx_params(USDC, &decrease_calldata(SPENDER, max_u256()));
    assert!(enforce_no_unlimited(TX, Some(&params)).is_ok());
}

#[test]
fn enforce_allows_set_approval_for_all_true() {
    // Boolean grants are handled by explicit UI consent, not this amount guard.
    let params = tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true));
    assert!(enforce_no_unlimited(TX, Some(&params)).is_ok());
}

#[test]
fn enforce_allows_a_plain_transfer() {
    let transfer = format!(
        "0xa9059cbb{}{}",
        addr_word(SPENDER),
        amt_word(U256::from(1000u64))
    );
    let params = tx_params(USDC, &transfer);
    assert!(enforce_no_unlimited(TX, Some(&params)).is_ok());
}

#[test]
fn end_to_end_rewrite_then_guard_passes() {
    let params = tx_params(USDC, &approve_calldata(SPENDER, max_u256()));
    let d = detect(TX, &params);
    let out = rewrite_approval_params(TX, &params, &d, &amount_choice(U256::from(500_000_000u64)))
        .expect("rewrite");
    assert!(enforce_no_unlimited(TX, Some(&out)).is_ok());
}

// ---------------------------------------------------------------------------
// parseTokenAmount / formatTokenAmount (jest vectors; invariant ⑧)
// ---------------------------------------------------------------------------

#[test]
fn parses_with_decimals_and_commas() {
    assert_eq!(
        parse_token_amount("1,234.5", 6),
        Some(U256::from(1_234_500_000u64))
    );
    assert_eq!(
        parse_token_amount("1000", 6),
        Some(U256::from(1_000_000_000u64))
    );
    assert_eq!(parse_token_amount("0.000001", 6), Some(U256::from(1u8)));
}

#[test]
fn rejects_over_precision_and_junk() {
    assert_eq!(parse_token_amount("0.0000001", 6), None); // 7 dp on a 6dp token
    assert_eq!(parse_token_amount("abc", 18), None);
    assert_eq!(parse_token_amount("1.2.3", 18), None);
}

#[test]
fn format_round_trips_with_thousands_separators() {
    let canon = |raw: u64| format_token_amount(U256::from(raw), 6, 6, ",", ".", false);
    assert_eq!(canon(1_234_500_000), "1,234.5");
    assert_eq!(canon(1_000_000_000), "1,000");
    assert_eq!(canon(0), "0");
    assert_eq!(
        parse_token_amount(&canon(1_234_500_000), 6),
        Some(U256::from(1_234_500_000u64))
    );
}

#[test]
fn format_localizes_with_injected_separators_without_precision_loss() {
    // European (dot_comma)
    assert_eq!(
        format_token_amount(U256::from(1_234_500_000u64), 6, 6, ".", ",", false),
        "1.234,5"
    );
    assert_eq!(
        format_token_amount(U256::from(1_000_000_000u64), 6, 6, ".", ",", false),
        "1.000"
    );
    // space_comma
    assert_eq!(
        format_token_amount(U256::from(1_234_567_000_000u64), 6, 6, " ", ",", false),
        "1 234 567"
    );
    // Indian 2-2-3 grouping
    assert_eq!(
        format_token_amount(U256::from(1_234_567_000_000u64), 6, 6, ",", ".", true),
        "12,34,567"
    );
}

// ---------------------------------------------------------------------------
// Machine — single-approval editor
// ---------------------------------------------------------------------------

fn approval_event(method: &str, params: &Value) -> Event {
    Event::ApprovalDetected {
        method: method.to_owned(),
        params_json: params.to_string(),
        chain_id: 1,
        wallet_address: Some(WALLET.to_owned()),
        read_only: false,
        now_ms: NOW_MS,
    }
}

fn usdc_meta() -> Res {
    Res::MetaResolved {
        metas: Some(vec![GuardTokenMetaEntry {
            token: USDC.to_lowercase(),
            symbol: "USDC".to_owned(),
            decimals: 6,
        }]),
    }
}

/// Invariant ④ (2026-09-26 ruling) — an unbounded request is kept AS ASKED
/// by default: Requested is preselected, the choice is `Unlimited`, nothing
/// is re-encoded, and the view reports the consent the submit guard needs.
/// Re-encoding it by default is what reverted Permit2 bundles.
#[test]
fn unbounded_request_is_kept_as_asked_by_default() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    assert_eq!(
        ops,
        vec![
            Op::ReadTokenMetadata {
                chain_id: 1,
                tokens: vec![USDC.to_lowercase()]
            },
            Op::ReadErc20Balance {
                chain_id: 1,
                token: USDC.to_lowercase(),
                owner: WALLET.to_owned()
            },
        ]
    );
    let view = sut.view();
    assert_eq!(view.surface, GuardSurface::ApprovalEditor);
    let editor = view.editor.expect("editor");
    assert_eq!(editor.mode, Some(GuardEditorMode::Requested));
    assert_eq!(editor.choice, Some(GuardChoice::Unlimited));
    assert!(!editor.requested_finite);
    assert!(editor.requested_unlimited);
    assert_eq!(
        editor.display_amount_raw, None,
        "the value row says Unlimited, never 2^256-1 in token units"
    );
    assert_eq!(
        editor.custom_text, "",
        "no finite number to seed the cap with"
    );
    assert!(view.confirm_allowed, "kept as asked needs no extra tap");
    assert!(view.unlimited_consented);
    assert_eq!(
        view.rewritten_params_json, None,
        "the site's own bytes go out — nothing re-encoded"
    );
}

/// A cap is one chip away, and choosing it withdraws the consent: the
/// capped re-encode is what gets submitted, and it carries no unbounded word.
#[test]
fn capping_an_unbounded_request_withdraws_the_consent() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.resolve(usdc_meta());
    sut.resolve(Res::BalanceRead {
        balance: Some("1234000000".to_owned()),
    });
    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Balance,
    });
    let view = sut.view();
    assert!(!view.unlimited_consented);
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    assert!(enforce_no_unlimited(TX, Some(&rewritten)).is_ok());

    // …and back: Requested keeps the site's ask again.
    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Requested,
    });
    let view = sut.view();
    assert_eq!(
        view.editor.as_ref().and_then(|e| e.choice.clone()),
        Some(GuardChoice::Unlimited)
    );
    assert!(view.unlimited_consented);
    assert_eq!(view.rewritten_params_json, None);
}

/// Typing a cap is choosing one — the keystroke moves the editor off the
/// preselected Requested chip, so the number on screen is the choice.
#[test]
fn typing_a_cap_selects_custom() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.resolve(usdc_meta());
    sut.dispatch(Event::CustomAmountChanged {
        text: "25".to_owned(),
    });
    let view = sut.view();
    let editor = view.editor.expect("editor");
    assert_eq!(editor.mode, Some(GuardEditorMode::Custom));
    assert_eq!(
        editor.choice,
        Some(GuardChoice::Amount {
            amount_raw: "25000000".to_owned()
        })
    );
    assert!(!view.unlimited_consented);
}

/// A FINITE request never derives the unlimited choice or the consent.
#[test]
fn finite_request_never_reports_unlimited_consent() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, U256::from(500_000_000u64))),
    ));
    let view = sut.view();
    let editor = view.editor.expect("editor");
    assert!(!editor.requested_unlimited);
    assert!(!view.unlimited_consented);
}

/// A finite, reasonable request is pre-accepted (mode `requested`).
#[test]
fn finite_request_is_preaccepted() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, U256::from(500_000_000u64))),
    ));
    let view = sut.view();
    let editor = view.editor.expect("editor");
    assert_eq!(editor.mode, Some(GuardEditorMode::Requested));
    assert!(editor.requested_finite);
    assert_eq!(
        editor.choice,
        Some(GuardChoice::Amount {
            amount_raw: "500000000".to_owned()
        })
    );
    assert!(view.confirm_allowed);
    // The re-encode of the accepted request is available for submit.
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    let d2 = redetect_data(&out_data(&rewritten));
    assert_eq!(d2.amount_raw.as_deref(), Some("500000000"));
}

#[test]
fn custom_below_cap_derives_choice_and_rewrites() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.resolve(usdc_meta());
    sut.resolve(Res::BalanceRead { balance: None });
    sut.dispatch(Event::CustomAmountChanged {
        text: "500".to_owned(),
    });
    let view = sut.view();
    let editor = view.editor.expect("editor");
    assert_eq!(
        editor.choice,
        Some(GuardChoice::Amount {
            amount_raw: "500000000".to_owned()
        }),
        "500 at the resolved 6 decimals"
    );
    assert!(view.confirm_allowed);
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    let d2 = redetect_data(&out_data(&rewritten));
    assert_eq!(d2.amount_raw.as_deref(), Some("500000000"));
    assert_eq!(d2.spender, SPENDER.to_lowercase());
    assert!(!d2.is_unbounded);
}

/// Invariant ④ — a custom amount at/above the cap derives NO choice, an
/// `unlimited_disabled` error, and keeps confirm gated.
#[test]
fn custom_at_or_above_cap_derives_null_choice_and_error() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.resolve(usdc_meta());
    // 1e60 human × 1e6 decimals = 1e66 raw ≥ 2^200.
    let huge = format!("1{}", "0".repeat(60));
    sut.dispatch(Event::CustomAmountChanged { text: huge });
    let view = sut.view();
    let editor = view.editor.expect("editor");
    assert_eq!(editor.choice, None);
    assert_eq!(editor.error, Some(GuardAmountError::UnlimitedDisabled));
    assert!(!view.confirm_allowed);
    assert_eq!(view.rewritten_params_json, None, "nothing to submit");
}

#[test]
fn custom_junk_is_an_invalid_amount() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.dispatch(Event::CustomAmountChanged {
        text: "abc".to_owned(),
    });
    let editor = sut.view().editor.expect("editor");
    assert_eq!(editor.error, Some(GuardAmountError::InvalidAmount));
    assert_eq!(editor.choice, None);
}

#[test]
fn revoke_preset_rewrites_to_zero() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Revoke,
    });
    let view = sut.view();
    assert_eq!(
        view.editor.as_ref().and_then(|e| e.choice.clone()),
        Some(GuardChoice::Revoke)
    );
    assert!(view.confirm_allowed);
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    let d2 = redetect_data(&out_data(&rewritten));
    assert_eq!(d2.amount_raw.as_deref(), Some("0"));
    assert!(d2.is_reducing);
}

/// Issue #86 — the one-tap Balance preset is a FINITE cap at the read
/// balance: enough for any swap, never unlimited.
#[test]
fn balance_preset_offers_a_finite_cap() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.resolve(usdc_meta());
    sut.resolve(Res::BalanceRead {
        balance: Some("1234000000".to_owned()),
    });
    let editor = sut.view().editor.expect("editor");
    assert!(editor.has_balance_cap);

    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Balance,
    });
    let view = sut.view();
    let editor = view.editor.expect("editor");
    assert_eq!(editor.mode, Some(GuardEditorMode::Balance));
    assert_eq!(
        editor.choice,
        Some(GuardChoice::Amount {
            amount_raw: "1234000000".to_owned()
        })
    );
    assert!(view.confirm_allowed);
}

#[test]
fn balance_read_failure_degrades_to_no_preset() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.resolve(usdc_meta());
    sut.resolve(Res::BalanceRead { balance: None });
    let editor = sut.view().editor.expect("editor");
    assert!(!editor.has_balance_cap);
    // A press on the absent chip is a no-op.
    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Balance,
    });
    assert_eq!(
        sut.view().editor.expect("editor").mode,
        Some(GuardEditorMode::Requested)
    );
}

/// Invariant ⑤ — a boolean grant-all is never preselected; the grant is a
/// deliberate tap, and only revoke/grant rewrites exist.
#[test]
fn boolean_grant_forces_a_deliberate_tap() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true)),
    ));
    let view = sut.view();
    let editor = view.editor.expect("editor");
    assert_eq!(editor.mode, None, "nothing preselected");
    assert_eq!(editor.choice, None);
    assert!(!view.confirm_allowed);

    sut.dispatch(Event::GrantDeliberatelyChosen);
    let view = sut.view();
    assert_eq!(
        view.editor.as_ref().and_then(|e| e.choice.clone()),
        Some(GuardChoice::Grant)
    );
    assert!(view.confirm_allowed);
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    assert!(
        redetect_data(&out_data(&rewritten)).is_unbounded,
        "grant keeps true"
    );

    sut.dispatch(Event::RevokeChosen);
    let view = sut.view();
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    assert!(
        redetect_data(&out_data(&rewritten)).is_reducing,
        "revoke flips to false"
    );
}

#[test]
fn incoming_boolean_revoke_preselects_the_safe_action() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &set_approval_for_all_calldata(SPENDER, false)),
    ));
    let view = sut.view();
    assert_eq!(
        view.editor.as_ref().and_then(|e| e.choice.clone()),
        Some(GuardChoice::Revoke)
    );
    assert!(view.confirm_allowed);
}

/// Invariant ③ — an off-chain permit is NEVER rewritten: the machine offers
/// the sign-verbatim surface, no editor, no rewrite, and does not gate
/// confirm on a choice (consent is the slide, owned by the shell).
#[test]
fn typed_permit_is_never_rewritten() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(approval_event(
        TYPED,
        &erc2612_params(&max_u256().to_string()),
    ));
    // Metadata still resolves (the permit surface shows the real symbol).
    assert_eq!(
        ops,
        vec![Op::ReadTokenMetadata {
            chain_id: 1,
            tokens: vec![USDC.to_lowercase()]
        }]
    );
    let view = sut.view();
    assert_eq!(view.surface, GuardSurface::PermitSign);
    assert!(view.editor.is_none());
    assert!(view.confirm_allowed);
    assert_eq!(view.rewritten_params_json, None);
    let d = view.detected.expect("detected");
    assert!(!d.editable);
    assert_eq!(d.block_reason, Some(GuardBlockReason::OffChainPermit));
}

/// Invariant ⑦ — "increase by 100" must never read as "cap at 100": the view
/// carries current + increment = total once the allowance read lands.
#[test]
fn increase_totals_show_the_resulting_sum() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(approval_event(
        TX,
        &tx_params(
            USDC,
            &increase_calldata(SPENDER, U256::from(100_000_000u64)),
        ),
    ));
    assert_eq!(
        ops,
        vec![
            Op::ReadTokenMetadata {
                chain_id: 1,
                tokens: vec![USDC.to_lowercase()]
            },
            Op::ReadErc20Allowance {
                chain_id: 1,
                token: USDC.to_lowercase(),
                owner: WALLET.to_owned(),
                spender: SPENDER.to_lowercase(),
            },
            Op::ReadErc20Balance {
                chain_id: 1,
                token: USDC.to_lowercase(),
                owner: WALLET.to_owned()
            },
        ]
    );
    assert_eq!(
        sut.view().increase_total,
        None,
        "no half-rendered total while in flight"
    );
    sut.resolve(usdc_meta());
    sut.resolve(Res::AllowanceRead {
        allowance: Some("250000000".to_owned()),
    });
    let total = sut.view().increase_total.expect("resulting total");
    assert_eq!(total.current.as_deref(), Some("250000000"));
    assert_eq!(total.increment, "100000000");
    assert_eq!(total.total.as_deref(), Some("350000000"));
}

/// Invariant ⑦ — a failed allowance read still warns the increment ADDS to
/// an existing allowance (total unknown), never hides the row.
#[test]
fn increase_total_read_failure_still_warns_additive() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(
            USDC,
            &increase_calldata(SPENDER, U256::from(100_000_000u64)),
        ),
    ));
    sut.resolve(usdc_meta());
    sut.resolve(Res::AllowanceRead { allowance: None });
    let total = sut.view().increase_total.expect("row still present");
    assert_eq!(total.current, None);
    assert_eq!(total.increment, "100000000");
    assert_eq!(total.total, None, "honest unknown");
}

/// `increaseAllowance` offers no Revoke: only the increment can be
/// re-encoded, so "revoke" would sign an increase of 0 and leave the existing
/// allowance where it was — while the resulting-total row said 0. Shown
/// must be signed; the chip is withheld and a press on it does nothing.
#[test]
fn increase_allowance_offers_no_revoke() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(
            USDC,
            &increase_calldata(SPENDER, U256::from(100_000_000u64)),
        ),
    ));
    sut.resolve(usdc_meta());
    sut.resolve(Res::AllowanceRead {
        allowance: Some("250000000".to_owned()),
    });
    assert!(!sut.view().editor.expect("editor").revoke_offered);
    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Revoke,
    });
    let view = sut.view();
    assert_eq!(
        view.editor.as_ref().and_then(|e| e.mode),
        Some(GuardEditorMode::Requested),
        "the withheld chip changed nothing"
    );
    let total = view.increase_total.expect("total");
    assert_eq!(total.total.as_deref(), Some("350000000"));
}

/// Every other amount card offers Revoke, and the boolean card answers the
/// same chip — the one row the shells draw for both, which is what made the
/// NFT card's 撤销 do nothing.
#[test]
fn revoke_is_offered_where_it_really_revokes() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    assert!(sut.view().editor.expect("editor").revoke_offered);

    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true)),
    ));
    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Revoke,
    });
    let view = sut.view();
    assert_eq!(
        view.editor.as_ref().and_then(|e| e.choice.clone()),
        Some(GuardChoice::Revoke)
    );
    assert!(view.confirm_allowed);
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    assert!(redetect_data(&out_data(&rewritten)).is_reducing);
}

/// Invariant ⑨ — unverified decimals are explicitly flagged, with the
/// short-address symbol fallback.
#[test]
fn unverified_decimals_are_flagged() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    assert!(
        sut.view().decimals_unverified,
        "unverified while loading too"
    );
    sut.resolve(Res::MetaResolved { metas: None });
    let view = sut.view();
    assert!(view.decimals_unverified);
    assert_eq!(view.meta.symbol, "0xa0b8…");
    assert_eq!(view.meta.decimals, 18);
    assert!(!view.meta.verified);

    // A verified read clears the flag.
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    sut.resolve(usdc_meta());
    let view = sut.view();
    assert!(!view.decimals_unverified);
    assert_eq!(view.meta.symbol, "USDC");
    assert_eq!(view.meta.decimals, 6);
}

/// Invariant ⑨ (permit surface) — a BOUNDED permit scales its amount with
/// `meta.decimals`, so unverified decimals must warn; an unlimited permit
/// shows no scaled amount, so it does not.
#[test]
fn typed_bounded_permit_flags_unverified_decimals() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(TYPED, &permit2_single_params("1000000000")));
    sut.resolve(Res::MetaResolved { metas: None });
    assert!(sut.view().decimals_unverified);

    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TYPED,
        &permit2_single_params(&max_u160().to_string()),
    ));
    sut.resolve(Res::MetaResolved { metas: None });
    assert!(!sut.view().decimals_unverified);
}

#[test]
fn expired_deadline_is_flagged() {
    let mut sut = Sut::new();
    // Fixture deadline 1750000000s < NOW_MS/1000 = 1754700000s.
    sut.dispatch(approval_event(TYPED, &erc2612_params("100")));
    assert!(sut.view().expired);

    let mut sut = Sut::new();
    let far = tx_params(PERMIT2, &permit2_calldata(USDC, SPENDER, U256::from(5u8)));
    // permit2 fixture deadline is also 1750000000 — expired.
    sut.dispatch(approval_event(TX, &far));
    assert!(sut.view().expired);
}

/// Ported quirk — the custom input is seeded with the 18-decimals fallback at
/// detection time and only re-seeds on a preset press once real decimals are
/// known (mount-time `useState` + `key={requestId}` semantics).
#[test]
fn custom_seed_uses_fallback_decimals_until_a_preset_press() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, U256::from(500_000_000u64))),
    ));
    // 500000000 raw at 18 dp, 6 max frac digits → "0".
    assert_eq!(sut.view().editor.expect("editor").custom_text, "0");
    sut.resolve(usdc_meta());
    assert_eq!(
        sut.view().editor.expect("editor").custom_text,
        "0",
        "not re-seeded by meta"
    );
    sut.dispatch(Event::PresetSelected {
        mode: GuardEditorMode::Requested,
    });
    assert_eq!(sut.view().editor.expect("editor").custom_text, "500");
}

#[test]
fn non_approval_request_is_inert() {
    let mut sut = Sut::new();
    let transfer = format!(
        "0xa9059cbb{}{}",
        addr_word(SPENDER),
        amt_word(U256::from(1000u64))
    );
    let ops = sut.dispatch(approval_event(TX, &tx_params(USDC, &transfer)));
    assert!(ops.is_empty(), "nothing to read");
    let view = sut.view();
    assert_eq!(view.surface, GuardSurface::None);
    assert!(view.confirm_allowed, "this machine imposes no gate");
    assert!(view.detected.is_none());
}

/// A superseded request's slow reads must never paint the new request's
/// state (the `cancelled` flags of today's effects).
#[test]
fn stale_results_are_dropped() {
    let mut sut = Sut::new();
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    // A new request supersedes before the first one's reads resolve.
    sut.dispatch(approval_event(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, max_u256())),
    ));
    // Resolve run 1's two reads — both stale, both dropped.
    sut.resolve(usdc_meta());
    sut.resolve(Res::BalanceRead {
        balance: Some("77".to_owned()),
    });
    let view = sut.view();
    assert!(view.meta.loading, "run 2's read is still outstanding");
    assert!(!view.editor.expect("editor").has_balance_cap);
    // Run 2's own read applies.
    sut.resolve(usdc_meta());
    assert_eq!(sut.view().meta.symbol, "USDC");
}

// ---------------------------------------------------------------------------
// Machine — EIP-5792 batch (invariant ⑥)
// ---------------------------------------------------------------------------

fn batch_params(calls: Vec<Value>) -> Value {
    json!([{ "version": "1.0", "chainId": "0x1", "calls": calls }])
}

fn batch_event(calls: Vec<Value>) -> Event {
    approval_event("wallet_sendCalls", &batch_params(calls))
}

fn transfer_call() -> Value {
    let transfer = format!(
        "0xa9059cbb{}{}",
        addr_word(SPENDER),
        amt_word(U256::from(1000u64))
    );
    json!({ "to": USDC, "data": transfer, "value": "0x0" })
}

/// An unbounded batch leg is kept AS ASKED by default — the atomic bundle's
/// next leg spends against it, so re-encoding it would revert the whole
/// batch. It still flags the danger banner and reports the consent; a cap
/// stays one chip away and re-encodes only that leg.
#[test]
fn batch_keeps_unbounded_legs_as_asked_until_capped() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(batch_event(vec![
        transfer_call(),
        json!({ "to": USDC, "data": approve_calldata(SPENDER, max_u256()), "value": "0x0" }),
        json!({ "to": USDC, "data": approve_calldata(SPENDER, U256::from(500_000_000u64)), "value": "0x0" }),
    ]));
    assert_eq!(
        ops,
        vec![Op::ReadTokenMetadata {
            chain_id: 1,
            tokens: vec![USDC.to_lowercase()]
        }],
        "one Multicall3 read for the whole batch"
    );
    let view = sut.view();
    assert_eq!(view.surface, GuardSurface::Batch);
    let batch = view.batch.expect("batch");
    let needs: Vec<bool> = batch.legs.iter().map(|l| l.needs_choice).collect();
    assert_eq!(
        needs,
        vec![false, false, false],
        "kept as asked settles the unbounded leg"
    );
    assert!(batch.legs[1].needs_editor);
    // Only the leg that MOUNTS a card carries an editor — a finite leg can
    // never acquire a choice, so the rebuild leaves it byte-identical.
    assert!(batch.legs[0].editor.is_none(), "non-approval leg");
    assert!(batch.legs[2].editor.is_none(), "finite leg");
    let editor = batch.legs[1].editor.clone().expect("leg 1 editor");
    assert_eq!(editor.mode, Some(GuardEditorMode::Requested));
    assert_eq!(editor.choice, Some(GuardChoice::Unlimited));
    assert!(batch.legs[1].grants_broad, "still unlimited — and said so");
    assert!(batch.any_uncapped);
    assert!(view.confirm_allowed);
    assert!(view.unlimited_consented);
    assert_eq!(
        view.rewritten_params_json, None,
        "the bundle goes out byte-identical"
    );

    // The leg's own editor decides — the shell types, the core derives.
    sut.resolve(usdc_meta());
    sut.dispatch(Event::LegCustomAmountChanged {
        index: 1,
        text: "500".to_owned(),
    });
    let view = sut.view();
    assert!(view.confirm_allowed, "every granting leg settled");
    assert!(
        !view.unlimited_consented,
        "nothing unbounded left to consent to"
    );
    let batch = view.batch.expect("batch");
    assert!(!batch.any_uncapped, "banner reflects the EFFECTIVE state");

    // The rebuilt bundle caps leg 1 and leaves the others byte-identical.
    let rewritten: Value =
        serde_json::from_str(&view.rewritten_params_json.expect("rewritten")).expect("json");
    let calls = rewritten[0]["calls"].as_array().expect("calls");
    let leg1 = detect_approval(
        TX,
        Some(&json!([{ "to": USDC, "data": calls[1]["data"], "value": "0x0" }])),
    )
    .expect("leg 1 still an approve");
    assert_eq!(leg1.amount_raw.as_deref(), Some("500000000"));
    assert!(!leg1.is_unbounded);
    assert_eq!(
        calls[0]["data"],
        transfer_call()["data"],
        "non-approval leg untouched"
    );
    assert_eq!(
        calls[2]["data"].as_str(),
        Some(approve_calldata(SPENDER, U256::from(500_000_000u64)).as_str()),
        "finite leg untouched (no choice made)"
    );
}

#[test]
fn batch_boolean_leg_requires_an_explicit_choice() {
    let mut sut = Sut::new();
    sut.dispatch(batch_event(vec![json!({
        "to": USDC, "data": set_approval_for_all_calldata(SPENDER, true), "value": "0x0"
    })]));
    let view = sut.view();
    assert!(!view.confirm_allowed);

    // Nothing is preselected — the deliberate tap is the consent (⑭).
    assert_eq!(
        view.batch.expect("batch").legs[0]
            .editor
            .clone()
            .expect("editor")
            .mode,
        None,
    );
    sut.dispatch(Event::LegGrantDeliberatelyChosen { index: 0 });
    let view = sut.view();
    assert!(view.confirm_allowed, "an explicit grant settles the leg");
    let batch = view.batch.expect("batch");
    assert!(
        batch.legs[0].grants_broad,
        "…but it still grants broad access"
    );
    assert!(batch.any_uncapped, "…and the banner says so");
}

/// The consent is one flag for the whole bundle, so it must not outlive a
/// cap that failed to land: a leg the person capped but whose re-encode
/// failed would go out with the site's unbounded bytes. No consent then —
/// the submit guard refuses the bundle (fail closed).
#[test]
fn a_cap_that_failed_to_land_withdraws_the_bundles_consent() {
    let mut sut = Sut::new();
    // 63 hex digits of amount: detected (≥ 2^200) but one nibble short of a
    // word, so the re-encode refuses it.
    let short = format!("0x095ea7b3{}{}", addr_word(SPENDER), "f".repeat(63));
    sut.dispatch(batch_event(vec![
        json!({ "to": USDC, "data": approve_calldata(SPENDER, max_u256()), "value": "0x0" }),
        json!({ "to": USDC, "data": short, "value": "0x0" }),
    ]));
    assert!(sut.view().unlimited_consented, "both kept as asked");
    sut.dispatch(Event::LegPresetSelected {
        index: 1,
        mode: GuardEditorMode::Revoke,
    });
    let view = sut.view();
    assert_eq!(
        view.batch.as_ref().and_then(|b| b.legs[1].choice.clone()),
        Some(GuardChoice::Revoke)
    );
    assert!(
        !view.unlimited_consented,
        "leg 0's consent must not carry leg 1's failed cap out unbounded"
    );
}

/// Fail-closed: a leg's custom amount at or above the cap derives NO choice —
/// the leg stays unsettled instead of passing a typed "cap" that is none.
/// (Keeping the site's unlimited ask is the Requested chip, which reports
/// its consent; a typed 2^200 reports nothing.)
#[test]
fn batch_leg_rejects_an_unbounded_custom_amount() {
    let mut sut = Sut::new();
    sut.dispatch(batch_event(vec![json!({
        "to": USDC, "data": approve_calldata(SPENDER, max_u256()), "value": "0x0"
    })]));
    sut.resolve(usdc_meta());
    // 2^200 base units at 6 decimals — comfortably past the uint256 cap.
    sut.dispatch(Event::LegCustomAmountChanged {
        index: 0,
        text: (UNLIMITED_CAP_256 / U256::from(1_000_000u64) + U256::from(1u8)).to_string(),
    });
    let view = sut.view();
    assert!(!view.confirm_allowed, "still gated");
    let batch = view.batch.expect("batch");
    let editor = batch.legs[0].editor.clone().expect("editor");
    assert_eq!(editor.error, Some(GuardAmountError::UnlimitedDisabled));
    assert_eq!(batch.legs[0].choice, None);
    assert!(batch.legs[0].needs_choice);
    assert!(!view.unlimited_consented);
    assert_eq!(view.rewritten_params_json, None);
}

/// ⑰ — a leg that sends a token to the token's OWN contract burns it. The
/// shell forwards the descriptor pipeline's recipients; the rule is here.
#[test]
fn batch_flags_a_token_sent_to_its_own_contract() {
    let mut sut = Sut::new();
    sut.dispatch(batch_event(vec![transfer_call()]));
    assert!(
        !sut.view().batch.expect("batch").any_to_own_token,
        "no descriptors yet"
    );

    sut.dispatch(Event::BatchRecipientsResolved {
        recipients: vec![vec![SPENDER.to_owned()]],
    });
    assert!(
        !sut.view().batch.expect("batch").any_to_own_token,
        "an ordinary recipient"
    );

    // Case-insensitively the leg's own `to` — the burn.
    sut.dispatch(Event::BatchRecipientsResolved {
        recipients: vec![vec![USDC.to_lowercase()]],
    });
    assert!(sut.view().batch.expect("batch").any_to_own_token);
}

/// A read-only replay mounts no leg editors at all — nothing to decide, and
/// the banner reports the RAW request rather than an effective state.
#[test]
fn read_only_batch_has_no_leg_editors() {
    let mut sut = Sut::new();
    sut.dispatch(Event::ApprovalDetected {
        method: "wallet_sendCalls".to_owned(),
        params_json: batch_params(vec![json!({
            "to": USDC, "data": approve_calldata(SPENDER, max_u256()), "value": "0x0"
        })])
        .to_string(),
        chain_id: 1,
        wallet_address: Some(WALLET.to_owned()),
        read_only: true,
        now_ms: NOW_MS,
    });
    let batch = sut.view().batch.expect("batch");
    assert!(!batch.legs[0].needs_editor);
    assert!(batch.legs[0].editor.is_none());
    assert!(batch.any_uncapped, "the raw request still grants unlimited");
}

#[test]
fn batch_meta_failure_defaults_legs_and_missing_tokens_get_fallback() {
    // Whole read fails → legs default to …/18/unverified.
    let mut sut = Sut::new();
    sut.dispatch(batch_event(vec![json!({
        "to": USDC, "data": approve_calldata(SPENDER, max_u256()), "value": "0x0"
    })]));
    sut.resolve(Res::MetaResolved { metas: None });
    let batch = sut.view().batch.expect("batch");
    assert_eq!(batch.legs[0].meta.symbol, "…");
    assert_eq!(batch.legs[0].meta.decimals, 18);
    assert!(!batch.legs[0].meta.verified);

    // Read succeeds but the token is missing → short-address fallback.
    let mut sut = Sut::new();
    sut.dispatch(batch_event(vec![json!({
        "to": USDC, "data": approve_calldata(SPENDER, max_u256()), "value": "0x0"
    })]));
    sut.resolve(Res::MetaResolved {
        metas: Some(vec![]),
    });
    let batch = sut.view().batch.expect("batch");
    assert_eq!(batch.legs[0].meta.symbol, "0xa0b8…");
    assert!(!batch.legs[0].meta.verified);
}

#[test]
fn leg_needs_choice_matches_the_component_rule() {
    let unbounded = detect(TX, &tx_params(USDC, &approve_calldata(SPENDER, max_u256())));
    let finite = detect(
        TX,
        &tx_params(USDC, &approve_calldata(SPENDER, U256::from(5u8))),
    );
    let reduce = detect(
        TX,
        &tx_params(USDC, &decrease_calldata(SPENDER, max_u256())),
    );
    let boolean = detect(
        TX,
        &tx_params(USDC, &set_approval_for_all_calldata(SPENDER, true)),
    );

    assert!(leg_needs_choice(Some(&unbounded), None));
    assert!(!leg_needs_choice(
        Some(&unbounded),
        Some(&GuardChoice::Revoke)
    ));
    assert!(
        leg_needs_choice(Some(&unbounded), Some(&GuardChoice::Grant)),
        "grant does not settle an amount leg"
    );
    assert!(
        !leg_needs_choice(Some(&finite), None),
        "finite amounts are pre-accepted"
    );
    assert!(!leg_needs_choice(Some(&reduce), None));
    assert!(leg_needs_choice(Some(&boolean), None));
    assert!(!leg_needs_choice(Some(&boolean), Some(&GuardChoice::Grant)));
    assert!(!leg_needs_choice(None, None));
}
