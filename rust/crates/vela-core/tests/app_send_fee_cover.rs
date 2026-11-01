//! r10-3: amount + fee against ONE balance, on every path that confirms a
//! transfer — replayed from `vectors/send-fee-cover.json`, which the shells
//! driving the same machines replay too.
//!
//! The finding: a dApp asked 0.001 USDC on Arc from an account holding 0.4,
//! the sheet showed a 0.488 USDC fee, Confirm stayed live, and the operation
//! reverted on chain. On Arc the native coin (USDC, 18 places) and its ERC-20
//! interface `0x3600…` (6 places) are one balance, and every "the fee comes
//! out of the money sent" rule compared raw identities; the fee machine's own
//! gate also read the fee coin's bare balance while nothing was simulated,
//! never what the operation's calls take out of it first.

#![cfg(feature = "crux")]

mod support;

use serde_json::Value;
use support::DomainDriver;
use vela_core::app::fee_policy::{
    encode_erc20_transfer, to_base_units, Event as FeeEvent, FeeAssetKind, FeeAssetQuote,
    FeeAssetView, FeeBalanceChange, FeeBundlerQuote, FeeCall, FeeEstimateView, FeeGasOutcome,
    FeeOperation as FeeOp, FeePolicy, FeeShellResult as FeeRes, FeeTier, FeeView,
};
use vela_core::app::send::{
    Event, Send, SendAccountRef, SendChainInfo, SendDisplayContext, SendFeeOutcome, SendOpenParams,
    SendOperation as Op, SendShellResult as Res, SendStage, SendTimerTag, SendToken,
    SendTreasuryProbe, SendView,
};
use vela_core::l10n::number::NumberPreset;

const VECTORS: &str = include_str!("vectors/send-fee-cover.json");
const ACCOUNT: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const RECIPIENT: &str = "0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const FEE_COLLECTOR: &str = "0x1111111111111111111111111111111111111111";

fn vectors() -> Value {
    serde_json::from_str(VECTORS).expect("send-fee-cover vectors parse")
}

fn cases<'a>(root: &'a Value, section: &str) -> &'a [Value] {
    root[section]["cases"]
        .as_array()
        .unwrap_or_else(|| panic!("{section}.cases"))
}

fn text(value: &Value) -> String {
    value.as_str().expect("a string").to_owned()
}

fn opt_text(value: &Value) -> Option<String> {
    value.as_str().map(str::to_owned)
}

// ===========================================================================
// The send machine
// ===========================================================================

type Sut = DomainDriver<Send>;

fn chains() -> Vec<SendChainInfo> {
    vec![
        SendChainInfo {
            chain_id: 1,
            network: "ethereum".to_owned(),
            native_symbol: "ETH".to_owned(),
        },
        SendChainInfo {
            chain_id: 5_042_002,
            network: "arc-testnet".to_owned(),
            native_symbol: "USDC".to_owned(),
        },
    ]
}

fn token_of(row: &Value) -> SendToken {
    let chain_id = u32::try_from(row["chain_id"].as_u64().expect("chain_id")).expect("u32");
    SendToken {
        network: chains()
            .into_iter()
            .find(|c| c.chain_id == chain_id)
            .expect("a known chain")
            .network,
        chain_id,
        symbol: text(&row["symbol"]),
        balance: text(&row["balance"]),
        decimals: u32::try_from(row["decimals"].as_u64().expect("decimals")).expect("u32"),
        token_address: opt_text(&row["address"]),
        price_usd: Some(1.0),
        logo_urls: vec![],
        spam: false,
    }
}

fn fee_view(chain_id: u32, fee: &Value) -> FeeEstimateView {
    let erc20 = fee["asset"] == "erc20";
    FeeEstimateView {
        chain_id,
        total_wei: if erc20 {
            "0".to_owned()
        } else {
            text(&fee["total_wei"])
        },
        max_fee_per_gas: "0".to_owned(),
        network_fee_per_gas: "20000000000".to_owned(),
        relayer_fee_per_gas: "0".to_owned(),
        bundler_gas_price: "20000000000".to_owned(),
        in_band_gas_basis: "20000000000".to_owned(),
        effective_gas_price: None,
        max_gas_price: None,
        total_gas: "450000".to_owned(),
        deployed: true,
        tier: FeeTier::Fast,
        quoted: true,
        fee_asset: if erc20 {
            FeeAssetView::Erc20 {
                token: text(&fee["token"]),
                decimals: u32::try_from(fee["decimals"].as_u64().expect("decimals")).expect("u32"),
                amount: text(&fee["amount"]),
                symbol: opt_text(&fee["symbol"]),
            }
        } else {
            FeeAssetView::Native
        },
        fee_recipient: Some(FEE_COLLECTOR.to_owned()),
        prepared: None,
    }
}

/// Mounted on `tokens`, with `token` picked (its warm quote refused, so the
/// form holds no fee of its own).
fn picked(case: &Value) -> (Sut, SendToken) {
    let tokens: Vec<SendToken> = case["tokens"]
        .as_array()
        .expect("tokens")
        .iter()
        .map(token_of)
        .collect();
    let token =
        tokens[usize::try_from(case["token"].as_u64().expect("token")).expect("usize")].clone();
    let mut sut = Sut::new();
    sut.dispatch(Event::Open {
        account: Some(SendAccountRef {
            id: "cred-1".to_owned(),
            address: ACCOUNT.to_owned(),
            name: None,
            multisig: None,
        }),
        params: SendOpenParams::default(),
        display: SendDisplayContext {
            code: "USD".to_owned(),
            rate: Some(1.0),
            fiat_decimals: 2,
        },
    });
    sut.resolve(Res::TokensLoaded {
        tokens: Some(tokens),
        chains: chains(),
    });
    settle(&mut sut, None);
    sut.dispatch(Event::SelectToken {
        token_id: token.id(),
    });
    settle(&mut sut, None);
    (sut, token)
}

/// Answer what is outstanding the way a quiet shell would: the credential
/// with a key, every quote with `quote` (refused when `None`), the treasury
/// covered, every check with nothing to say; the pre-check's and the form's
/// timers never fire.
fn settle(sut: &mut Sut, quote: Option<&FeeEstimateView>) {
    for _ in 0..64 {
        let Some(op) = sut.outstanding().into_iter().find(|op| {
            !matches!(
                op,
                Op::StartTimer {
                    tag: SendTimerTag::EstimateTimeout | SendTimerTag::FormEstimate,
                    ..
                }
            )
        }) else {
            return;
        };
        let answer = match &op {
            Op::PrewarmFees { .. } => Res::FeesPrewarmed,
            Op::LoadAccountCredential { .. } => Res::AccountCredential {
                public_key_hex: Some("04deadbeef".to_owned()),
            },
            Op::EstimateFee { .. } => Res::FeeEstimated {
                outcome: match quote {
                    Some(view) => SendFeeOutcome::Ok {
                        estimate: view.clone(),
                    },
                    None => SendFeeOutcome::Failed {
                        kind: vela_core::app::send::SendEstimateFailure::QuoteUnavailable,
                    },
                },
            },
            Op::ProbeTreasury { .. } => Res::TreasuryProbed {
                probe: SendTreasuryProbe::Covered,
            },
            Op::ResolveRisk { .. } => Res::RiskResolved { risk: None },
            Op::ResolveIdentity { .. } => Res::IdentityResolved { identity: None },
            Op::SimulateCalls { .. } => Res::SimResolved { sim_json: None },
            Op::StartTimer { tag, .. } => Res::TimerElapsed { tag: *tag },
            other => panic!("unexpected operation {other:?}"),
        };
        let wanted = op.clone();
        sut.resolve_matching(|pending| *pending == wanted, answer);
    }
    panic!("the send machine never settled");
}

fn warning_kind(view: &SendView) -> Option<String> {
    view.amount_warning.as_ref().map(|warning| {
        serde_json::to_value(warning).expect("warning serializes")["type"]
            .as_str()
            .expect("tagged")
            .to_owned()
    })
}

/// Every case: type the amount, Continue to the confirm on a fee of nothing,
/// and let the case's fee land ON the confirm page — the path r10-3 took,
/// where only the confirm's own gate stands between the figure and the
/// passkey.
#[test]
fn send_confirm_weighs_amount_and_fee_against_one_balance() {
    let root = vectors();
    for case in cases(&root, "send") {
        let name = text(&case["name"]);
        let (mut sut, token) = picked(case);
        sut.dispatch(Event::SetRecipient {
            recipient: RECIPIENT.to_owned(),
        });
        settle(&mut sut, None);
        sut.dispatch(Event::SetAmount {
            amount: text(&case["amount"]),
        });
        settle(&mut sut, None);
        sut.dispatch(Event::Continue);
        let free = fee_view(
            token.chain_id,
            &serde_json::json!({ "asset": "native", "total_wei": "0" }),
        );
        settle(&mut sut, Some(&free));
        assert_eq!(
            sut.view().stage,
            SendStage::Confirm,
            "{name}: reached the confirm"
        );
        assert!(sut.view().can_confirm, "{name}: a free fee confirms");

        sut.dispatch(Event::FeeUpdated {
            estimate: fee_view(token.chain_id, &case["fee"]),
        });
        settle(&mut sut, None);
        let view = sut.view();
        let expect = &case["expect"];
        assert_eq!(
            view.can_confirm,
            expect["can_confirm"].as_bool().expect("can_confirm"),
            "{name}: can_confirm"
        );
        match (&view.same_asset_fee_issue, &expect["issue"]) {
            (None, Value::Null) => {}
            (Some(issue), want) if want.is_object() => {
                assert_eq!(
                    issue.fee_amount,
                    text(&want["fee_amount"]),
                    "{name}: fee_amount"
                );
                assert_eq!(issue.total, text(&want["total"]), "{name}: total");
                assert_eq!(
                    issue.max_transfer_amount,
                    text(&want["max_transfer_amount"]),
                    "{name}: max_transfer_amount"
                );
            }
            (got, want) => panic!("{name}: issue {got:?}, expected {want}"),
        }
        assert_eq!(
            warning_kind(&view),
            opt_text(&expect["warning"]),
            "{name}: amount_warning"
        );
        // The confirm that is shut says why: the issue, or the live warning
        // the web confirm draws under it (`live-send.ts` `sameFeeWords` ??
        // `liveWarning`).
        if !view.can_confirm {
            assert!(
                view.same_asset_fee_issue.is_some() || view.amount_warning.is_some(),
                "{name}: a shut confirm with nothing said"
            );
        }
        // …and a slide from a stale frame never reaches the passkey.
        let ops = sut.dispatch(Event::SlideConfirm { review: None });
        if !view.can_confirm {
            assert!(
                !ops.iter()
                    .any(|op| matches!(op, Op::ProbeTreasury { .. } | Op::SubmitUserOp { .. })),
                "{name}: a refused confirm signed anyway: {ops:?}"
            );
        }
    }
}

/// Max holds the fee back from the balance it is drawn from — the same coin,
/// or its other face.
#[test]
fn max_reserves_the_fee_from_the_balance_it_comes_out_of() {
    let root = vectors();
    for case in cases(&root, "max") {
        let name = text(&case["name"]);
        let (mut sut, token) = picked(case);
        sut.dispatch(Event::FeeUpdated {
            estimate: fee_view(token.chain_id, &case["fee"]),
        });
        settle(&mut sut, None);
        sut.dispatch(Event::TapMax);
        settle(&mut sut, None);
        let view = sut.view();
        let filled = to_base_units(&view.token_amount, token.decimals).expect("max parses");
        assert_eq!(
            filled.to_string(),
            text(&case["expect"]["max_units"]),
            "{name}: max fill"
        );
        assert_eq!(
            warning_kind(&view),
            opt_text(&case["expect"]["warning"]),
            "{name}: warning"
        );
    }
}

// ===========================================================================
// The fee machine (a dApp request's sheet)
// ===========================================================================

type FeeSut = DomainDriver<FeePolicy>;

fn calls_of(case: &Value) -> Vec<FeeCall> {
    case["calls"]
        .as_array()
        .expect("calls")
        .iter()
        .map(|call| match call.get("erc20_transfer") {
            Some(transfer) => FeeCall {
                to: text(&transfer["token"]),
                value: "0".to_owned(),
                data: encode_erc20_transfer(
                    &text(&transfer["to"]),
                    text(&transfer["amount"]).parse().expect("amount"),
                )
                .expect("encodes"),
            },
            None => FeeCall {
                to: text(&call["to"]),
                value: text(&call["value"]),
                data: "0x".to_owned(),
            },
        })
        .collect()
}

fn row(fee_token: Option<&str>, balance: u128) -> FeeAssetQuote {
    FeeAssetQuote {
        minimum_amount: None,
        recipient: FEE_COLLECTOR.to_owned(),
        asset: if fee_token.is_some() {
            FeeAssetKind::Erc20
        } else {
            FeeAssetKind::Native
        },
        fee_token: fee_token.map(str::to_owned),
        balance: balance.to_string(),
        decimals: if fee_token.is_some() { 6 } else { 18 },
        symbol: "USDC".to_owned(),
        usd_balance: "1".to_owned(),
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    }
}

/// One quote, answered the way a shell answers it; the block-time tick and the
/// deadline stay outstanding.
fn quote(case: &Value, rows: Vec<FeeAssetQuote>) -> FeeSut {
    let chain_id = u32::try_from(case["chain_id"].as_u64().expect("chain_id")).expect("u32");
    let mut sut = FeeSut::new();
    sut.dispatch(FeeEvent::QuoteRequested {
        read_deployment: None,
        chain_id,
        account: ACCOUNT.to_owned(),
        deployed: true,
        public_key_available: true,
        tier: FeeTier::Fast,
        calls: calls_of(case),
        fee_token: opt_text(&case["fee_token"]),
        auto_fee_token: false,
        number: NumberPreset::CommaDot,
    });
    let mut rows = Some(rows);
    for _ in 0..32 {
        let Some(op) = sut
            .outstanding()
            .into_iter()
            .find(|op| !matches!(op, FeeOp::StartDeadline { .. } | FeeOp::StartTtl { .. }))
        else {
            break;
        };
        let answer = match &op {
            FeeOp::FetchGasPrice { .. } => FeeRes::GasPrice {
                eth_gas_price: Some("20000000000".to_owned()),
                base_fee: Some("0".to_owned()),
                priority_fee: Some("0".to_owned()),
            },
            FeeOp::FetchBundlerQuote { .. } => FeeRes::BundlerQuote {
                quote: Some(FeeBundlerQuote {
                    max_fee_per_gas: "40000000000".to_owned(),
                    max_priority_fee_per_gas: None,
                    network_fee_per_gas: Some("20000000000".to_owned()),
                    relayer_fee_per_gas: Some("20000000000".to_owned()),
                    in_band_fee_per_gas: None,
                }),
            },
            FeeOp::FetchInBandQuotes { .. } => FeeRes::InBandQuotes {
                quotes: Some(rows.take().expect("rows asked once")),
            },
            FeeOp::EstimateUserOpGas { .. } => FeeRes::UserOpGas {
                outcome: FeeGasOutcome::Estimated {
                    verification_gas_limit: "100000".to_owned(),
                    call_gas_limit: "310000".to_owned(),
                    pre_verification_gas: "40000".to_owned(),
                    settlement_gas: None,
                },
            },
            FeeOp::MeasureInnerCalls { calls, .. } => FeeRes::InnerCallsMeasured {
                gas: vec![None; calls.len()],
            },
            other => panic!("unexpected fee operation {other:?}"),
        };
        let wanted = op.clone();
        sut.resolve_matching(|pending| *pending == wanted, answer);
    }
    if let Some(measured) = case.get("measured").and_then(Value::as_array) {
        sut.dispatch(FeeEvent::BalanceChangesMeasured {
            changes: measured
                .iter()
                .map(|change| FeeBalanceChange {
                    token: opt_text(&change["token"]),
                    delta: text(&change["delta"]),
                })
                .collect(),
        });
    }
    sut
}

/// The fee in force, in its own coin's base units.
fn fee_units(view: &FeeView) -> u128 {
    let fee = view.fee.as_ref().expect("a settled figure");
    match &fee.fee_asset {
        FeeAssetView::Native => fee.total_wei.parse().expect("wei"),
        FeeAssetView::Erc20 { amount, .. } => amount.parse().expect("units"),
    }
}

/// What the calls state they move of the balance that pays a fee in
/// `fee_token`, in that coin's places, rounded up — and, when a simulation
/// answered, what it measured instead.
fn outflow(case: &Value, alias: &str, fee_in_alias: bool) -> u128 {
    let places_up = |units: u128, from_native: bool| match (from_native, fee_in_alias) {
        (true, true) => units.div_ceil(1_000_000_000_000),
        (false, false) => units * 1_000_000_000_000,
        _ => units,
    };
    if let Some(measured) = case.get("measured").and_then(Value::as_array) {
        return measured
            .iter()
            .map(|change| {
                let delta: i128 = text(&change["delta"]).parse().expect("delta");
                places_up(delta.unsigned_abs(), change["token"].is_null())
            })
            .sum();
    }
    calls_of(case)
        .iter()
        .map(|call| {
            if call.data == "0x" {
                places_up(call.value.parse().expect("value"), true)
            } else {
                assert!(call.to.eq_ignore_ascii_case(alias), "only the alias moves");
                let amount = u128::from_str_radix(&call.data[call.data.len() - 64..], 16)
                    .expect("amount word");
                places_up(amount, false)
            }
        })
        .sum()
}

#[test]
fn a_dapp_requests_fee_is_confirmable_only_when_its_balance_covers_both() {
    let root = vectors();
    let alias = text(&root["alias"]);
    for case in cases(&root, "fee_machine") {
        let name = text(&case["name"]);
        let fee_in_alias = !case["fee_token"].is_null();
        let twin = case["twin"].as_bool().expect("twin");
        let rows_for = |paying: u128| {
            let (native, interface) = if fee_in_alias {
                (paying * 1_000_000_000_000, paying)
            } else {
                (paying, paying / 1_000_000_000_000)
            };
            let mut rows = vec![row(None, native)];
            if twin || fee_in_alias {
                rows.push(row(Some(&alias), interface));
            }
            rows
        };
        // The machine's own figure, read off a run with plenty to pay from.
        let rich = quote(case, rows_for(10u128.pow(24)));
        let fee = fee_units(&rich.view());
        let delta: i128 = text(&case["delta"]).parse().expect("delta");
        let paying = (fee + outflow(case, &alias, fee_in_alias))
            .checked_add_signed(delta)
            .expect("a balance");
        let sut = quote(case, rows_for(paying));
        let view = sut.view();
        assert_eq!(
            fee_units(&view),
            fee,
            "{name}: the balance does not move the fee"
        );
        let ready = case["expect"]["confirm_fee_ready"]
            .as_bool()
            .expect("confirm_fee_ready");
        assert_eq!(view.confirm_fee_ready, ready, "{name}: confirm_fee_ready");
        let selected = view
            .options
            .iter()
            .find(|option| option.selected)
            .expect("a coin in force");
        assert_eq!(
            selected.short_with_amount,
            case["expect"]["short_with_amount"]
                .as_bool()
                .expect("short_with_amount"),
            "{name}: the line says amount + fee"
        );
        if !ready {
            // `sign_confirm::confirm_state_of` reads this view as
            // `ConfirmBlock::FeeShort` — "the coin that pays is short", said
            // under the fee — exactly when a settled figure is in hand and
            // not ready: neither busy, nor failed, nor another coin's.
            assert!(
                view.fee.is_some() && !view.busy && view.failure.is_none() && !view.provisional,
                "{name}: a refusal the sheet reads as FeeShort: {view:?}"
            );
        }
    }
}

// ===========================================================================
// One balance, two faces — the pure rules underneath
// ===========================================================================

#[test]
fn one_balance_is_asked_by_chain_never_by_raw_identity() {
    use vela_core::app::manage_tokens::same_balance;
    const ALIAS: &str = "0x3600000000000000000000000000000000000000";
    const OTHER: &str = "0x2222222222222222222222222222222222222222";
    let table: [(u32, Option<&str>, Option<&str>, bool); 8] = [
        (5_042_002, None, None, true),
        (5_042_002, None, Some(ALIAS), true),
        (5_042_002, Some(ALIAS), None, true),
        (5_042_002, Some(ALIAS), Some(ALIAS), true),
        (5_042_002, None, Some(OTHER), false),
        (5_042, Some(ALIAS), None, true),
        // The same address elsewhere is an ordinary contract.
        (1, None, Some(ALIAS), false),
        (
            1,
            Some(OTHER),
            Some("0x2222222222222222222222222222222222222222"),
            true,
        ),
    ];
    for (chain, a, b, same) in table {
        assert_eq!(same_balance(chain, a, b), same, "{chain} {a:?} {b:?}");
    }
}

#[test]
fn rescaling_between_places_never_makes_money_appear() {
    use vela_core::app::manage_tokens::rescale_units;
    let table: [(u128, u32, u32, bool, u128); 8] = [
        (488_060, 6, 18, true, 488_060_000_000_000_000),
        (488_060_000_000_000_000, 18, 6, true, 488_060),
        (488_060_000_000_000_001, 18, 6, true, 488_061),
        (488_060_000_000_000_001, 18, 6, false, 488_060),
        (1, 18, 6, true, 1),
        (1, 18, 6, false, 0),
        (7, 6, 6, true, 7),
        (u128::MAX, 6, 18, true, u128::MAX),
    ];
    for (amount, from, to, up, want) in table {
        assert_eq!(
            rescale_units(amount, from, to, up),
            want,
            "{amount} {from}->{to} up={up}"
        );
    }
}
