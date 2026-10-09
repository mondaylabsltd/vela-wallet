//! The Ethereum fee fix (owner-approved 2026-10-09, the wallet ↔ relay
//! contract v2), replayed through the real `fee_policy` machine on what was
//! measured on Ethereum mainnet:
//!
//! - block 26,149,237 (2026-10-08 17:40 UTC): the node's base fee
//!   2.779303959 gwei and tip 0, the next block's base fee the relay prices
//!   on 2.784444443 gwei, the tier tips it reads from the reward percentiles
//!   0.147 / 1.0 / 1.795 gwei, ETH $2,423.92; the relay's estimates and
//!   quotes of that moment; the gas each operation really USED; Uniswap's own
//!   network cost for the swap, $0.92 tipless and $1.53 with its 1 gwei tip;
//! - the 2026-10-02 operation 0x7132ee31… (Safe 0x88cC…6894), whose fee leg
//!   702,534,304,258,794 wei was 23.5× what the chain charged.
//!
//! Two relays are priced. One OLDER than the contract publishes neither
//! `settlementGas` nor `inBandFeePerGas`: the wallet prices its limits as
//! returned at `3 ×` the basis — no 1.5× on top of the relay's own, no 300k
//! verification floor, the 2M undeployed floor kept. One that keeps the
//! contract (vela-relay `docs/fees.md` §1a–§3): the wallet pays
//! `settlementGas × inBandFeePerGas[tier]`, its own reading only a floor and a
//! bound.
//!
//! The safety property, for every operation class and tier: what the person
//! pays funds what the relay requires at the cap it quoted, is still accepted
//! with the tier's whole tip after the largest base-fee rise one block
//! allows, and never leaves the relay below what the chain charges.

mod support;

use support::DomainDriver;
use vela_core::app::fee_policy::{
    Event, FeeAssetKind, FeeAssetQuote, FeeBundlerQuote, FeeCall, FeeFailure, FeeGasOutcome,
    FeeOperation as Op, FeePolicy, FeeShellResult as Res, FeeTier, IN_BAND_PRICE,
};
use vela_core::l10n::number::NumberPreset;
use vela_core::user_op::{in_band_gas_limits, inner_calls_gas_floor, GasEstimate};

const CHAIN: u32 = 1;
const SAFE: &str = "0x88cca0eedbf2c4426110bbfc998f048689266894";
const RECIPIENT: &str = "0x3e59292e18417f814112f731e7163534c6d2fe3c";
const RELAY_RECIPIENT: &str = "0x1111111111111111111111111111111111111111";
const USDT: &str = "0xdac17f958d2ee523a2206206994597c13d831ec7";
const UNIVERSAL_ROUTER: &str = "0x23617e59a5925b2a4bf75d73ff6711cd0b29de85";

/// Block 26,149,237's base fee — what the node and the relay both read.
const BASE: u128 = 2_779_303_959;
/// ETH in dollars at that block.
const ETH_USD: f64 = 2_423.92;
/// Uniswap's network cost for the same ETH → USDC swap, with its 1 gwei tip.
const UNISWAP_WITH_TIP_USD: f64 = 1.528_020_690_382_775_5;

const TIERS: [FeeTier; 3] = [FeeTier::Slow, FeeTier::Standard, FeeTier::Fast];

// ---------------------------------------------------------------------------
// The relay of 2026-10-08 (older than the contract)
// ---------------------------------------------------------------------------

/// `pimlico_getUserOperationGasPrice` at the measured block: tip 0 at every
/// tier, `networkFeePerGas` = 0.6 × the cap, no `inBandFeePerGas`.
fn old_row(tier: FeeTier) -> FeeBundlerQuote {
    let (max_fee, network, relayer) = match tier {
        FeeTier::Slow => (4_176_666_664u128, 2_505_999_999u128, 1_670_666_665u128),
        FeeTier::Standard => (5_568_888_886, 3_341_333_332, 2_227_555_554),
        _ => (8_353_333_329, 5_011_999_998, 3_341_333_331),
    };
    FeeBundlerQuote {
        max_fee_per_gas: max_fee.to_string(),
        max_priority_fee_per_gas: Some("0".to_owned()),
        network_fee_per_gas: Some(network.to_string()),
        relayer_fee_per_gas: Some(relayer.to_string()),
        in_band_fee_per_gas: None,
    }
}

fn old_cap(tier: FeeTier) -> u128 {
    old_row(tier).max_fee_per_gas.parse().expect("cap")
}

fn old_network(tier: FeeTier) -> u128 {
    old_row(tier)
        .network_fee_per_gas
        .and_then(|value| value.parse().ok())
        .expect("network fee")
}

// ---------------------------------------------------------------------------
// A relay that keeps the contract (vela-relay `docs/fees.md` §2b, §2c)
// ---------------------------------------------------------------------------

/// The next block's base fee, which the relay prices every tier on.
const RELAY_BASE: u128 = 2_784_444_443;
/// The relay's inclusion floor, 1.125 × the base fee (`settlement.rs`): the
/// lowest cap it signs a tier's whole tip under.
const INCLUSION_FLOOR_BPS: u128 = 11_250;

/// The tip the relay signs each tier with at the measured block: the median
/// over 20 blocks of each block's 25th / 50th / 70th percentile reward (the
/// relay's `the_ethereum_tiers_at_the_block_the_overcharge_was_measured`).
fn tip(tier: FeeTier) -> u128 {
    match tier {
        FeeTier::Slow => 147_320_634,
        FeeTier::Standard => 1_000_000_000,
        _ => 1_795_116_512,
    }
}

/// The tier's cap — its `maxFeePerGas` — at `base`.
fn cap_at(tier: FeeTier, base: u128) -> u128 {
    IN_BAND_PRICE
        .cap(tier, base, tip(tier))
        .expect("a representable cap")
}

/// The published price of one tier at `base`, as the relay computes it: the
/// same formula, so the same function.
fn published(tier: FeeTier, base: u128) -> u128 {
    IN_BAND_PRICE
        .per_gas(tier, base, tip(tier))
        .expect("a representable price")
}

/// `networkFeePerGas`, frozen at its meaning before the tiers tipped by
/// percentile: `0.9 / 1.2 / 1.8 × base + the market tip` (0 on Ethereum).
fn frozen_network(tier: FeeTier, base: u128) -> u128 {
    let bps = match tier {
        FeeTier::Slow => 9_000,
        FeeTier::Standard => 12_000,
        _ => 18_000,
    };
    (bps * base).div_ceil(10_000)
}

fn new_row(tier: FeeTier, relay_base: u128) -> FeeBundlerQuote {
    let cap = cap_at(tier, relay_base);
    let network = frozen_network(tier, relay_base);
    FeeBundlerQuote {
        max_fee_per_gas: cap.to_string(),
        max_priority_fee_per_gas: Some(tip(tier).to_string()),
        network_fee_per_gas: Some(network.to_string()),
        relayer_fee_per_gas: Some(cap.saturating_sub(network).to_string()),
        in_band_fee_per_gas: Some(published(tier, relay_base).to_string()),
    }
}

/// What the relay bills an operation that used `used` gas
/// (`cost::buffered_gas`): `used + ⌈15% × used⌉ + 30,000`. Its estimate's
/// `settlementGas` is this figure or 0.2–4.6% above it; the tests take it
/// exactly, the worst case for the person's margin.
fn settlement_gas(used: u128) -> u128 {
    used + (used * 1_500).div_ceil(10_000) + 30_000
}

/// What the relay REQUIRES at a cap: `relay_markup × settlementGas × cap`.
fn relay_requires(settlement: u128, cap: u128) -> u128 {
    (IN_BAND_PRICE.relay_markup_bps * settlement * cap).div_ceil(10_000)
}

/// Whether `paid` keeps the tier's whole tip after the base fee rose to
/// `base`: the cap it funds is at least the inclusion floor plus that tip.
fn keeps_the_whole_tip(paid: u128, settlement: u128, tier: FeeTier, base: u128) -> bool {
    let floor = (INCLUSION_FLOOR_BPS * base).div_ceil(10_000);
    paid >= relay_requires(settlement, floor + tip(tier))
}

// ---------------------------------------------------------------------------
// The operations the investigation measured
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct Measured {
    name: &'static str,
    deployed: bool,
    calls: Vec<FeeCall>,
    /// The relay's raw estimate (verification, call, pre-verification).
    estimate: (u128, u128, u128),
    /// `eth_estimateGas` of each contract call from the Safe, when any.
    inner: Option<u128>,
    /// The gas the operation really used.
    used: u128,
    /// The gas today's wallet priced it on (the investigation's table).
    old_priced_gas: u128,
}

fn eth_send() -> FeeCall {
    FeeCall {
        to: RECIPIENT.to_owned(),
        value: "1000000000000000".to_owned(),
        data: "0x".to_owned(),
    }
}

fn measured_ops() -> Vec<Measured> {
    vec![
        Measured {
            name: "ETH send",
            deployed: true,
            calls: vec![eth_send()],
            estimate: (100_000, 114_894, 101_613),
            inner: None,
            used: 146_752,
            old_priced_gas: 583_954,
        },
        Measured {
            name: "USDT send",
            deployed: true,
            calls: vec![FeeCall {
                to: USDT.to_owned(),
                value: "0".to_owned(),
                data: vela_core::app::fee_policy::encode_erc20_transfer(RECIPIENT, 1_000_000)
                    .expect("transfer"),
            }],
            estimate: (100_000, 140_345, 102_172),
            inner: None,
            used: 163_719,
            old_priced_gas: 622_689,
        },
        Measured {
            name: "swap",
            deployed: true,
            calls: vec![FeeCall {
                to: UNIVERSAL_ROUTER.to_owned(),
                value: "1000000000000000".to_owned(),
                data: "0x3593564c0000000000000000000000000000000000000000000000000000000000000060"
                    .to_owned(),
            }],
            estimate: (100_000, 329_093, 110_083),
            inner: Some(138_291),
            used: 291_357,
            old_priced_gas: 913_722,
        },
        Measured {
            name: "first op, undeployed",
            deployed: false,
            calls: vec![eth_send()],
            estimate: (100_000, 50_000, 109_986),
            inner: None,
            used: 504_609,
            old_priced_gas: 2_219_986,
        },
    ]
}

// ---------------------------------------------------------------------------
// The machine, driven as a shell drives it
// ---------------------------------------------------------------------------

struct Sut(DomainDriver<FeePolicy>);

impl Sut {
    fn new() -> Self {
        Self(DomainDriver::new())
    }
    fn quiet(&mut self) {
        self.0
            .drop_matching(|op| matches!(op, Op::StartDeadline { .. }));
    }
}

fn native_row() -> FeeAssetQuote {
    FeeAssetQuote {
        recipient: RELAY_RECIPIENT.to_owned(),
        asset: FeeAssetKind::Native,
        fee_token: None,
        balance: "10000000000000000000".to_owned(),
        decimals: 18,
        symbol: "ETH".to_owned(),
        usd_balance: "24239.20".to_owned(),
        usd_price: Some("2423.92".to_owned()),
        native_usd_floor_price: None,
    }
}

struct Quote {
    total_gas: u128,
    total_wei: u128,
}

/// Price `op` at `tier`: the node reads `wallet_base`, the relay answers
/// `row`, and the simulation answers the measured estimate with `settlement`.
fn quote(
    op: &Measured,
    tier: FeeTier,
    wallet_base: u128,
    row: FeeBundlerQuote,
    settlement: Option<u128>,
) -> Result<Quote, FeeFailure> {
    let mut sut = Sut::new();
    sut.0.dispatch(Event::QuoteRequested {
        chain_id: CHAIN,
        account: SAFE.to_owned(),
        deployed: op.deployed,
        public_key_available: true,
        tier,
        calls: op.calls.clone(),
        fee_token: None,
        auto_fee_token: false,
        number: NumberPreset::CommaDot,
    });
    sut.quiet();
    sut.0.resolve(Res::GasPrice {
        eth_gas_price: Some(wallet_base.to_string()),
        base_fee: Some(wallet_base.to_string()),
        priority_fee: Some("0".to_owned()),
    });
    sut.0.resolve(Res::BundlerQuote { quote: Some(row) });
    sut.0.resolve(Res::InBandQuotes {
        quotes: Some(vec![native_row()]),
    });
    sut.quiet();
    if let Some(failed) = sut.0.view().failed {
        return Err(failed);
    }
    let (vgl, cgl, pvg) = op.estimate;
    sut.0.resolve_matching(
        |op| matches!(op, Op::EstimateUserOpGas { .. }),
        Res::UserOpGas {
            outcome: FeeGasOutcome::Estimated {
                verification_gas_limit: vgl.to_string(),
                call_gas_limit: cgl.to_string(),
                pre_verification_gas: pvg.to_string(),
                settlement_gas: settlement.map(|gas| gas.to_string()),
            },
        },
    );
    if let Some(inner) = op.inner {
        sut.0.resolve_matching(
            |op| matches!(op, Op::MeasureInnerCalls { .. }),
            Res::InnerCallsMeasured {
                gas: vec![Some(inner.to_string())],
            },
        );
    }
    let view = sut.0.view();
    if let Some(failed) = view.failed {
        return Err(failed);
    }
    let fee = view.fee.expect("quoted");
    Ok(Quote {
        total_gas: fee.total_gas.parse().expect("gas"),
        total_wei: fee.total_wei.parse().expect("wei"),
    })
}

fn usd(wei: u128) -> f64 {
    wei as f64 * ETH_USD / 1e18
}

fn name(tier: FeeTier) -> &'static str {
    match tier {
        FeeTier::Slow => "slow",
        FeeTier::Standard => "standard",
        FeeTier::Rapid => "rapid",
        FeeTier::Fast => "fast",
    }
}

// ---------------------------------------------------------------------------
// An older relay: its limits, as returned
// ---------------------------------------------------------------------------

/// W1: the relay of 2026-10-08 publishes no settlement figure. The wallet
/// prices the limits it signs — the relay's own, as returned, raised only by
/// the inner calls' floor and an undeployed Safe's floors — at `3 × max(C,
/// R)`. Exact to the wei, and never below what that relay requires (its
/// documented margins: 1.43 slow, 1.29 standard and fast, against the
/// settlement allocation the investigation measured).
#[test]
fn an_older_relay_is_priced_on_its_limits_as_returned() {
    for op in measured_ops() {
        let (vgl, cgl, pvg) = op.estimate;
        let limits = in_band_gas_limits(
            GasEstimate {
                verification_gas_limit: vgl,
                call_gas_limit: cgl,
                pre_verification_gas: pvg,
                settlement_gas: None,
            },
            op.deployed,
            op.inner
                .and_then(|gas| inner_calls_gas_floor(&[gas], op.calls.len())),
        );
        for tier in TIERS {
            let quoted = quote(&op, tier, BASE, old_row(tier), None).expect("quoted");
            assert_eq!(
                quoted.total_gas,
                limits.total(),
                "{} {}",
                op.name,
                name(tier)
            );
            let basis = BASE.max(old_network(tier));
            assert_eq!(quoted.total_wei, 3 * limits.total() * basis);
            let old = 3 * op.old_priced_gas * basis;
            assert!(quoted.total_wei <= old, "never dearer than before");
            let requires = 14 * settlement_gas(op.used) * old_cap(tier) / 10;
            let margin = if tier == FeeTier::Slow { 1.43 } else { 1.29 };
            assert!(
                quoted.total_wei as f64 >= requires as f64 * margin,
                "{} {}: {} < {} × {margin}",
                op.name,
                name(tier),
                quoted.total_wei,
                requires
            );
        }
    }
}

/// The figures W1 lands on at the measured block, pinned: an ETH send's
/// limits drop from 583,954 to 316,507 gas (the investigation's "P1"), the
/// swap's from 913,722 to 539,176; an undeployed Safe's first operation keeps
/// the 2M verification floor, because that relay answers its verification as
/// the constant 100,000 against a real 412,195.
#[test]
fn the_older_relays_figures_at_the_measured_block() {
    let ops = measured_ops();
    let gas = |index: usize| {
        quote(
            &ops[index],
            FeeTier::Standard,
            BASE,
            old_row(FeeTier::Standard),
            None,
        )
        .expect("quoted")
        .total_gas
    };
    assert_eq!(gas(0), 316_507);
    assert_eq!(gas(1), 342_517);
    assert_eq!(gas(2), 539_176);
    assert_eq!(gas(3), 2_000_000 + 100_000 + 109_986);
    // $7.69 for the standard ETH send (was $14.19).
    let eth = quote(
        &ops[0],
        FeeTier::Standard,
        BASE,
        old_row(FeeTier::Standard),
        None,
    )
    .expect("quoted");
    assert_eq!(eth.total_wei, 3 * 316_507 * 3_341_333_332);
    assert!((usd(eth.total_wei) - 7.69).abs() < 0.01);
}

/// The 2026-10-02 operation, replayed: the fee leg was
/// `3 × 583,954 × 401,021,487` = 702,534,304,258,794 wei, 23.5× the chain's
/// 146,824 × 0.203343141 gwei. The same quote on its limits as returned is
/// `3 × 316,507 × 401,021,487` = 380,778,323,357,727 wei — 12.8× — and still
/// covers what that relay required to bundle it (the outer transaction's
/// `eth_estimateGas` of 322,126 at its 0.690795364 gwei cap, ×1.4; measured
/// with the OLD, larger limits, so an upper bound for the new ones).
#[test]
fn the_2026_10_02_operation_replayed() {
    let basis = 401_021_487u128;
    let old = 3 * 583_954 * basis;
    assert_eq!(old, 702_534_304_258_794);
    let limits = in_band_gas_limits(
        GasEstimate {
            verification_gas_limit: 100_000,
            call_gas_limit: 114_894,
            pre_verification_gas: 101_613,
            settlement_gas: None,
        },
        true,
        None,
    );
    let new = vela_core::app::fee_policy::calculate_in_band_fee_amount(
        limits.total(),
        basis,
        &vela_core::app::fee_policy::AssetPricing {
            is_native: true,
            decimals: 18,
            usd_price: Some("2423.92".to_owned()),
            native_usd_floor_price: None,
        },
        &vela_core::app::fee_policy::AssetPricing {
            is_native: true,
            decimals: 18,
            usd_price: Some("2423.92".to_owned()),
            native_usd_floor_price: None,
        },
    )
    .expect("priced");
    assert_eq!(new, 380_778_323_357_727);
    let chain = 146_824u128 * 203_343_141;
    let times = new as f64 / chain as f64;
    assert!((12.7..12.8).contains(&times), "{times}× the chain");
    let required = 14 * 322_126 * 690_795_364u128 / 10;
    assert!(new > required, "{new} ≤ {required}");
}

// ---------------------------------------------------------------------------
// A relay that keeps the contract: settlement gas at its published price
// ---------------------------------------------------------------------------

/// The wallet pays `settlementGas × inBandFeePerGas[tier]`. For every
/// operation class and tier that funds the relay's requirement at the cap it
/// quoted; after the largest rise one block allows (+12.5%), the relay
/// reprices the cap down and still signs the tier's WHOLE tip; and it never
/// leaves the relay below the most the chain can charge (`used × cap`).
#[test]
fn a_contract_relay_is_paid_its_published_price_on_settlement_gas_and_is_covered() {
    let risen = (RELAY_BASE * 1_125).div_ceil(1_000);
    for op in measured_ops() {
        let settlement = settlement_gas(op.used);
        for tier in TIERS {
            let quoted = quote(&op, tier, BASE, new_row(tier, RELAY_BASE), Some(settlement))
                .expect("quoted");
            let at = |what: &str| format!("{} {}: {what}", op.name, name(tier));
            assert_eq!(quoted.total_gas, settlement, "{}", at("priced gas"));
            assert_eq!(
                quoted.total_wei,
                settlement * published(tier, RELAY_BASE),
                "{}",
                at("the relay's price, not the wallet's own reading")
            );
            let cap = cap_at(tier, RELAY_BASE);
            assert!(
                quoted.total_wei >= relay_requires(settlement, cap),
                "{}",
                at("funds the quoted cap")
            );
            assert!(
                keeps_the_whole_tip(quoted.total_wei, settlement, tier, risen),
                "{}",
                at("keeps the whole tip after a block's largest rise")
            );
            assert!(
                quoted.total_wei >= op.used * cap,
                "{}",
                at("the relay never below the chain's charge")
            );
        }
    }
}

/// The figures at the measured block, pinned to the cent — the relay's own
/// replay of the same block (`docs/fees.md` §3) — and printed with
/// `--nocapture` beside today's and Uniswap's. The swap at `standard` is
/// $5.04 against Uniswap's $1.53 (3.3×), where today it is $22.20 (14.5×).
#[test]
fn a_contract_relays_figures_at_the_measured_block() {
    assert_eq!(
        TIERS.map(|tier| published(tier, RELAY_BASE)),
        [4_756_386_028, 5_694_333_331, 7_334_683_716],
        "1.1 × (1.5 / 1.5 / 1.75 × 2.784444443 + 0.147 / 1.0 / 1.795) gwei"
    );
    let expected = [
        ("ETH send", [2.29, 2.74, 3.53]),
        ("USDT send", [2.52, 3.01, 3.88]),
        ("swap", [4.21, 5.04, 6.49]),
        ("first op, undeployed", [7.04, 8.42, 10.85]),
    ];
    println!("op, tier: today | older relay (W1) | contract relay | ÷ Uniswap $1.53");
    for (op, (label, dollars)) in measured_ops().iter().zip(expected) {
        assert_eq!(op.name, label);
        for (tier, want) in TIERS.into_iter().zip(dollars) {
            let basis = BASE.max(old_network(tier));
            let today = 3 * op.old_priced_gas * basis;
            let w1 = quote(op, tier, BASE, old_row(tier), None).expect("w1");
            let v2 = quote(
                op,
                tier,
                BASE,
                new_row(tier, RELAY_BASE),
                Some(settlement_gas(op.used)),
            )
            .expect("v2");
            assert!(
                (usd(v2.total_wei) - want).abs() < 0.005,
                "{} {}: ${:.4} ≠ ${want}",
                op.name,
                name(tier),
                usd(v2.total_wei)
            );
            println!(
                "{}, {}: ${:.2} | ${:.2} | ${:.2} | {:.1}×",
                op.name,
                name(tier),
                usd(today),
                usd(w1.total_wei),
                usd(v2.total_wei),
                usd(v2.total_wei) / UNISWAP_WITH_TIP_USD
            );
        }
    }
}

/// The 2026-10-02 send at the relay's own estimate of it — `settlementGas`
/// 208,070, 4.6% above the 198,848 the executor bills — costs $2.40 / $2.87 /
/// $3.70 at the measured block, where it was quoted $11.80 / $14.19 / $21.28.
#[test]
fn the_2026_10_02_send_at_the_relays_own_estimate() {
    let send = measured_ops().remove(0);
    let paid = TIERS.map(|tier| {
        quote(&send, tier, BASE, new_row(tier, RELAY_BASE), Some(208_070))
            .expect("quoted")
            .total_wei
    });
    let dollars = paid.map(|wei| (usd(wei) * 100.0).round() / 100.0);
    assert_eq!(dollars, [2.40, 2.87, 3.70]);
    for (tier, wei) in TIERS.into_iter().zip(paid) {
        assert!(wei >= relay_requires(198_848, cap_at(tier, RELAY_BASE)));
    }
}

/// The wallet's own reading is a floor, never a second price: a relay whose
/// base fee is within a block's fall of ours is paid exactly; one whose base
/// fee is lower than the next block's can be — a market it read a while ago —
/// is raised by the cap the lowest possible next base fee implies.
#[test]
fn the_wallets_reading_is_only_a_floor_under_a_lagging_relay() {
    let op = measured_ops().remove(0);
    let settlement = settlement_gas(op.used);
    let tier = FeeTier::Standard;
    // A block's largest fall below our reading: the relay's price stands.
    let fallen = BASE - BASE / 8;
    let quoted = quote(&op, tier, BASE, new_row(tier, fallen), Some(settlement)).expect("quoted");
    assert_eq!(quoted.total_wei, settlement * published(tier, fallen));
    // Further below than any next block can be: raised to that next block.
    let lagging = BASE * 3 / 4;
    let quoted = quote(&op, tier, BASE, new_row(tier, lagging), Some(settlement)).expect("quoted");
    let floor = (published(tier, lagging) * cap_at(tier, fallen)).div_ceil(cap_at(tier, lagging));
    assert_eq!(quoted.total_wei, settlement * floor);
    assert!(floor > published(tier, lagging));
}

/// A published price more than three times the same formula over the
/// wallet's own reading is refused, not paid (requirement G05). A REAL tier
/// tip at a quiet base fee is not: at 0.1 gwei base, `fast`'s 1.8 gwei tip is
/// eighteen times the base fee and the node's own tip reading is 0 — the tip
/// is taken as the relay reports it, and `networkFeePerGas` stays frozen so
/// the older `R > 3 × C` bound never sees it.
#[test]
fn an_absurd_price_is_refused_and_a_real_tip_is_not() {
    let op = measured_ops().remove(0);
    let settlement = settlement_gas(op.used);
    let mut absurd = new_row(FeeTier::Standard, RELAY_BASE);
    let own = IN_BAND_PRICE
        .per_gas(FeeTier::Standard, BASE, tip(FeeTier::Standard))
        .expect("price");
    absurd.in_band_fee_per_gas = Some((3 * own + 1).to_string());
    assert_eq!(
        quote(&op, FeeTier::Standard, BASE, absurd, Some(settlement)).err(),
        Some(FeeFailure::GasQuoteTooHigh)
    );

    let quiet = 100_000_000; // 0.1 gwei
    for tier in TIERS {
        let quoted = quote(&op, tier, quiet, new_row(tier, quiet), Some(settlement));
        assert!(quoted.is_ok(), "{}: {:?}", name(tier), quoted.err());
    }
}

/// `settlementGas` and `inBandFeePerGas` are used together or not at all:
/// with either missing the operation is priced the older way, which every
/// relay accepts.
#[test]
fn half_a_contract_is_priced_the_older_way() {
    let op = measured_ops().remove(0);
    let w1_gas = 316_507u128;
    let no_settlement = quote(
        &op,
        FeeTier::Fast,
        BASE,
        new_row(FeeTier::Fast, RELAY_BASE),
        None,
    )
    .expect("quoted");
    assert_eq!(no_settlement.total_gas, w1_gas);
    let no_price = quote(
        &op,
        FeeTier::Fast,
        BASE,
        old_row(FeeTier::Fast),
        Some(settlement_gas(op.used)),
    )
    .expect("quoted");
    assert_eq!(no_price.total_gas, w1_gas);
}

/// The older relay answers an undeployed Safe's verification as 100,000; its
/// first operation is still priced — and signed — with the 2M floor. A
/// contract relay's real figure (1.5 × 412,195) is believed, and the fee is
/// its settlement gas.
#[test]
fn an_undeployed_safe_keeps_the_2m_floor_only_with_an_older_relay() {
    let op = measured_ops().remove(3);
    let older = quote(&op, FeeTier::Slow, BASE, old_row(FeeTier::Slow), None).expect("quoted");
    assert_eq!(older.total_gas, 2_209_986);
    let signed = in_band_gas_limits(
        GasEstimate {
            verification_gas_limit: 100_000,
            call_gas_limit: 50_000,
            pre_verification_gas: 109_986,
            settlement_gas: None,
        },
        false,
        None,
    );
    assert_eq!(signed.verification_gas_limit, 2_000_000);
    assert_eq!(signed.total(), older.total_gas, "priced = signed");

    let contract = Measured {
        estimate: (618_293, 50_000, 109_986),
        ..op
    };
    let settlement = settlement_gas(contract.used);
    assert_eq!(settlement, 610_301);
    let quoted = quote(
        &contract,
        FeeTier::Slow,
        BASE,
        new_row(FeeTier::Slow, RELAY_BASE),
        Some(settlement),
    )
    .expect("quoted");
    assert_eq!(quoted.total_gas, settlement);
}

/// A registry backup as an undeployed Safe's first operation (tx
/// 0x86795d08…): the relay measures its 4.3M-gas call with the Safe's code in
/// place, so its `settlementGas` — 5,831,838, 0.2% above the 5,820,096 it
/// bills — is priced as it is, never the larger limits the operation carries.
#[test]
fn an_undeployed_safes_contract_call_is_priced_on_its_settlement_gas() {
    let backup = Measured {
        name: "registry backup, undeployed",
        deployed: false,
        calls: vec![FeeCall {
            to: UNIVERSAL_ROUTER.to_owned(),
            value: "0".to_owned(),
            data: "0x4e71d92d".to_owned(),
        }],
        estimate: (618_293, 6_472_557, 154_039),
        inner: Some(4_308_125),
        used: 5_034_866,
        old_priced_gas: 7_659_195,
    };
    for tier in TIERS {
        let quoted = quote(
            &backup,
            tier,
            BASE,
            new_row(tier, RELAY_BASE),
            Some(5_831_838),
        )
        .expect("quoted");
        assert_eq!(quoted.total_gas, 5_831_838);
        assert!(quoted.total_wei >= relay_requires(5_820_096, cap_at(tier, RELAY_BASE)));
    }
}

/// The minimum holds on the published path exactly as on the older one: gas
/// too cheap to cost a cent costs a cent — and on ETH, dearer than $1,000, a
/// cent is less than the relay's 0.00001-coin admission floor, which wins.
#[test]
fn the_minimum_holds_on_the_published_path() {
    let op = measured_ops().remove(0);
    let settlement = settlement_gas(op.used);
    let dust_base = 1_000; // wei, and no tip
    let per_gas = IN_BAND_PRICE
        .per_gas(FeeTier::Standard, dust_base, 0)
        .expect("price");
    let row = FeeBundlerQuote {
        max_fee_per_gas: (2 * dust_base).to_string(),
        max_priority_fee_per_gas: Some("0".to_owned()),
        network_fee_per_gas: Some((12 * dust_base / 10).to_string()),
        relayer_fee_per_gas: Some((8 * dust_base / 10).to_string()),
        in_band_fee_per_gas: Some(per_gas.to_string()),
    };
    let quoted = quote(&op, FeeTier::Standard, dust_base, row, Some(settlement)).expect("quoted");
    assert!(settlement * per_gas < 10_000_000_000_000);
    assert_eq!(quoted.total_wei, 10_000_000_000_000, "0.00001 ETH");
}

/// The block-time tick re-asks the relay, so the quote that is signed carries
/// at most one block of drift — the `drift_allowance` the published price is
/// built on.
#[test]
fn a_quote_on_ethereum_is_priced_again_every_slot() {
    let op = measured_ops().remove(0);
    let mut sut = Sut::new();
    sut.0.dispatch(Event::QuoteRequested {
        chain_id: CHAIN,
        account: SAFE.to_owned(),
        deployed: true,
        public_key_available: true,
        tier: FeeTier::Standard,
        calls: op.calls.clone(),
        fee_token: None,
        auto_fee_token: false,
        number: NumberPreset::CommaDot,
    });
    sut.quiet();
    sut.0.resolve(Res::GasPrice {
        eth_gas_price: Some(BASE.to_string()),
        base_fee: Some(BASE.to_string()),
        priority_fee: Some("0".to_owned()),
    });
    sut.0.resolve(Res::BundlerQuote {
        quote: Some(new_row(FeeTier::Standard, BASE)),
    });
    sut.0.resolve(Res::InBandQuotes {
        quotes: Some(vec![native_row()]),
    });
    sut.quiet();
    let ops = sut.0.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::Estimated {
            verification_gas_limit: "100000".to_owned(),
            call_gas_limit: "114894".to_owned(),
            pre_verification_gas: "101613".to_owned(),
            settlement_gas: Some(settlement_gas(op.used).to_string()),
        },
    });
    assert_eq!(ops, vec![Op::StartTtl { ms: 12_000 }]);
}
