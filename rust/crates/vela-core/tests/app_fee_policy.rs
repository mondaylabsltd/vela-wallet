//! Rules of the fee-policy machine, one test per rule.
//!
//! The pure-math vectors are lifted verbatim from the jest suites so the Rust
//! port and the TS canon can never drift silently:
//! `src/__tests__/services/safe-transaction.test.ts` (calcMaxFeePerGas,
//! deriveChainGasPrice, sameAssetFeeLimit), `inband-send.test.ts`
//! (calculateInBandFeeAmount), `tempo.test.ts` (the whole Tempo model) and
//! `batch-send.test.ts` (reserve math + string-exact Max).
//!
//! The machine tests drive the quote lifecycle exactly the way the shell will:
//! dispatch an event, answer the operations one at a time. Inventory invariants
//! ①–⑨ each have at least one test named after the rule.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::fee_policy::{
    atto_to_token_units, calc_max_fee_per_gas, calculate_in_band_fee_amount,
    derive_chain_gas_price, effective_gas_price, encode_erc20_transfer, from_base_units,
    gas_price_range, is_tempo_chain, max_native_sendable, min_gas_price_wei, raw_bundler_gas_cost,
    reserve_fee_token, reserve_native_gas, same_asset_fee_limit, tempo_call_gas_limit,
    tempo_expected_gas, tempo_fee_token_units, tempo_minimum_fee_token_units, tempo_quote_is_stale,
    tempo_reimbursement, tempo_settlement_split, tempo_split_safety_gas, tier_multiplier,
    to_base_units, usd_price_scaled, AssetPricing, Event, FeeAsset, FeeAssetKind, FeeAssetQuote,
    FeeAssetView, FeeBundlerQuote, FeeCall, FeeEstimate, FeeFailure, FeeGasOutcome,
    FeeOperation as Op, FeePolicy, FeeShellResult as Res, FeeTier, GasSignals, MultiTokenSpec,
    TEMPO_BASE_FEE_ATTO, TEMPO_CALL_GAS_PER_SUBCALL, TEMPO_COST_BUFFER_GAS,
    TEMPO_DEFAULT_FEE_TOKEN, TEMPO_DEPLOYED_GAS_EST, TEMPO_DEPLOY_GAS_EST,
    TEMPO_PER_SUBCALL_GAS_EST, TEMPO_SPLIT_SAFETY_BPS, TEMPO_SPLIT_SAFETY_GAS,
};

/// The machine, driven as the shell drives it — except that every run's
/// deadline timer (`StartDeadline`, spec 094) is a shell timer these tests
/// never let fire: dropped as it is asked for, so each test answers the reads
/// it is about in order. The deadline's own tests use [`Timed`].
struct Sut(DomainDriver<FeePolicy>);

fn is_deadline(op: &Op) -> bool {
    matches!(op, Op::StartDeadline { .. })
}

impl Sut {
    fn new() -> Self {
        Self(DomainDriver::new())
    }
    fn quiet(&mut self, ops: Vec<Op>) -> Vec<Op> {
        self.0.drop_matching(is_deadline);
        ops.into_iter().filter(|op| !is_deadline(op)).collect()
    }
    fn dispatch(&mut self, event: Event) -> Vec<Op> {
        let ops = self.0.dispatch(event);
        self.quiet(ops)
    }
    fn resolve(&mut self, result: Res) -> Vec<Op> {
        let ops = self.0.resolve(result);
        self.quiet(ops)
    }
    fn resolve_matching(&mut self, predicate: impl Fn(&Op) -> bool, result: Res) -> Vec<Op> {
        let ops = self.0.resolve_matching(predicate, result);
        self.quiet(ops)
    }
}

impl std::ops::Deref for Sut {
    type Target = DomainDriver<FeePolicy>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl std::ops::DerefMut for Sut {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// The machine with its deadline timers left in, for the tests of the bound.
type Timed = DomainDriver<FeePolicy>;

const CHAIN: u32 = 1;
const TEMPO_CHAIN: u32 = 4_217;
const ACCOUNT: &str = "0xaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const USDC: &str = "0x2222222222222222222222222222222222222222";
const NATIVE_RECIPIENT: &str = "0x1111111111111111111111111111111111111111";
const USDC_RECIPIENT: &str = "0x3333333333333333333333333333333333333333";
const COLLECTOR: &str = "0x4444444444444444444444444444444444444444";

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn native_row(balance: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        recipient: NATIVE_RECIPIENT.to_owned(),
        asset: FeeAssetKind::Native,
        fee_token: None,
        balance: balance.to_owned(),
        decimals: 18,
        symbol: "ETH".to_owned(),
        usd_balance: "1868.70".to_owned(),
        usd_price: Some("1868.70000000".to_owned()),
        native_usd_floor_price: None,
    }
}

fn usdc_row(balance: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        recipient: USDC_RECIPIENT.to_owned(),
        asset: FeeAssetKind::Erc20,
        fee_token: Some(USDC.to_owned()),
        balance: balance.to_owned(),
        decimals: 6,
        symbol: "USDC".to_owned(),
        usd_balance: "5.00".to_owned(),
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    }
}

fn pathusd_row(balance: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        recipient: COLLECTOR.to_owned(),
        asset: FeeAssetKind::Erc20,
        fee_token: Some(TEMPO_DEFAULT_FEE_TOKEN.to_owned()),
        balance: balance.to_owned(),
        decimals: 6,
        symbol: "pathUSD".to_owned(),
        usd_balance: "5.00".to_owned(),
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    }
}

fn request(chain_id: u32, calls: Vec<FeeCall>) -> Event {
    request_in(chain_id, calls, None)
}

/// The same request, denominated in a specific fee asset — the shape the send
/// and signing surfaces use once a chip has been tapped.
fn request_in(chain_id: u32, calls: Vec<FeeCall>, fee_token: Option<&str>) -> Event {
    Event::QuoteRequested {
        chain_id,
        account: ACCOUNT.to_owned(),
        deployed: true,
        public_key_available: true,
        tier: FeeTier::Fast,
        calls,
        fee_token: fee_token.map(str::to_owned),
        auto_fee_token: false,
    }
}

fn gas_ok() -> Res {
    // eth_gasPrice 1 gwei, baseFee 0, tip 0 → derived chain price 1 gwei.
    Res::GasPrice {
        eth_gas_price: Some("1000000000".to_owned()),
        base_fee: Some("0".to_owned()),
        priority_fee: Some("0".to_owned()),
    }
}

fn bundler_ok() -> Res {
    Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "2000000000".to_owned(),
            max_priority_fee_per_gas: None,
            network_fee_per_gas: Some("1000000000".to_owned()),
            relayer_fee_per_gas: Some("1000000000".to_owned()),
        }),
    }
}

fn quotes_ok() -> Res {
    Res::InBandQuotes {
        quotes: Some(vec![native_row("1000000000000000000"), usdc_row("5000000")]),
    }
}

fn estimated() -> Res {
    // Padded (`safe-transaction.ts:697-702`): vgl 100k×1.5 → floor 300k,
    // cgl 50k×1.5 → floor 100k, pvg 40k+10k → 50k. Total = 450_000.
    Res::UserOpGas {
        outcome: FeeGasOutcome::Estimated {
            verification_gas_limit: "100000".to_owned(),
            call_gas_limit: "50000".to_owned(),
            pre_verification_gas: "40000".to_owned(),
        },
    }
}

/// The happy-path gas basis the fixtures above produce.
const TOTAL_GAS: u128 = 450_000;
const NETWORK_FEE: u128 = 1_000_000_000;
/// 450_000 × 1e9 × 3.
const NATIVE_FEE_WEI: u128 = 1_350_000_000_000_000;
/// The same fee converted to USDC at $1868.70 / $1 (= $2.522745).
const USDC_FEE_UNITS: u128 = 2_522_745;

/// Drive a fresh machine to a settled native quote; the 30s TTL timer is left
/// outstanding (as it is in production).
fn quoted_native(calls: Vec<FeeCall>) -> Sut {
    let mut sut = Sut::new();
    let ops = sut.dispatch(request(CHAIN, calls));
    assert_eq!(
        ops,
        vec![
            Op::FetchGasPrice {
                chain_id: CHAIN,
                want_tip: true
            },
            Op::FetchBundlerQuote {
                chain_id: CHAIN,
                tier: FeeTier::Fast
            },
            Op::FetchInBandQuotes {
                chain_id: CHAIN,
                account: ACCOUNT.to_owned()
            },
        ]
    );
    assert!(sut.resolve(gas_ok()).is_empty(), "still gathering");
    assert!(sut.resolve(bundler_ok()).is_empty(), "still gathering");
    let ops = sut.resolve(quotes_ok());
    assert_eq!(ops.len(), 1, "context complete → one estimate simulation");
    let ops = sut.resolve(estimated());
    assert_eq!(ops, vec![Op::StartTtl { ms: 30_000 }]);
    sut
}

// ===========================================================================
// Pure money math — jest vectors ported verbatim
// ===========================================================================

/// `calcMaxFeePerGas` vectors (`safe-transaction.test.ts:112-135`): the ×2.0
/// bundler margin is the shipped behavior (the `// 150` comment is stale).
#[test]
fn calc_max_fee_per_gas_matches_tier_vectors() {
    let gas_price: u128 = 10_000_000_000; // 10 gwei
    assert_eq!(
        calc_max_fee_per_gas(gas_price, FeeTier::Standard),
        24_000_000_000
    );
    assert_eq!(
        calc_max_fee_per_gas(gas_price, FeeTier::Slow),
        22_000_000_000
    );
    assert_eq!(
        calc_max_fee_per_gas(gas_price, FeeTier::Rapid),
        30_000_000_000
    );
    assert_eq!(
        calc_max_fee_per_gas(gas_price, FeeTier::Fast),
        40_000_000_000
    );
    // floor: gasPrice=0 → 1 wei
    assert_eq!(calc_max_fee_per_gas(0, FeeTier::Standard), 1);
    // user cost scales with tier
    assert!(
        calc_max_fee_per_gas(gas_price, FeeTier::Slow)
            < calc_max_fee_per_gas(gas_price, FeeTier::Standard)
    );
    assert!(
        calc_max_fee_per_gas(gas_price, FeeTier::Standard)
            < calc_max_fee_per_gas(gas_price, FeeTier::Rapid)
    );
    assert!(
        calc_max_fee_per_gas(gas_price, FeeTier::Rapid)
            < calc_max_fee_per_gas(gas_price, FeeTier::Fast)
    );
}

/// `GAS_TIER_MULTIPLIERS` (`safe-transaction.ts:244-249`).
#[test]
fn tier_multiplier_table_is_verbatim() {
    assert_eq!(tier_multiplier(FeeTier::Slow), (11, 10));
    assert_eq!(tier_multiplier(FeeTier::Standard), (12, 10));
    assert_eq!(tier_multiplier(FeeTier::Rapid), (15, 10));
    assert_eq!(tier_multiplier(FeeTier::Fast), (20, 10));
}

/// `deriveChainGasPrice` vectors (`safe-transaction.test.ts:170-229`) — the
/// Gnosis priority-tip regression pinned in Rust.
#[test]
fn derive_chain_gas_price_includes_priority_tip() {
    // Gnosis: baseFee + tip dominates a tiny eth_gasPrice.
    let derived = derive_chain_gas_price(&GasSignals {
        chain_id: 100,
        eth_gas_price: 21,
        base_fee: 17,
        priority_fee: 1_202,
        tip_measured: Some(true),
    });
    assert_eq!(derived.gas_price, 1_219); // 17 + 1202, NOT max(21, 17)
    assert_eq!(derived.base_fee, 17);
    assert_eq!(derived.priority_fee, 1_202);
    assert!(derived.tip_measured);

    // eth_gasPrice stays a floor on min-gas-price chains (Polygon/BSC).
    let derived = derive_chain_gas_price(&GasSignals {
        chain_id: 137,
        eth_gas_price: 30_000_000_000,
        base_fee: 1_000_000_000,
        priority_fee: 500_000_000,
        tip_measured: None,
    });
    assert_eq!(derived.gas_price, 30_000_000_000);

    // Missing tip (0) is recovered from eth_gasPrice — never below legacy.
    let derived = derive_chain_gas_price(&GasSignals {
        chain_id: 1,
        eth_gas_price: 30,
        base_fee: 10,
        priority_fee: 0,
        tip_measured: None,
    });
    assert_eq!(derived.gas_price, 30);
    assert_eq!(derived.priority_fee, 20);
    assert!(!derived.tip_measured);

    // L2 with zero tip (Arbitrum/OP): no over-pricing.
    let derived = derive_chain_gas_price(&GasSignals {
        chain_id: 42_161,
        eth_gas_price: 100_000_000,
        base_fee: 100_000_000,
        priority_fee: 0,
        tip_measured: None,
    });
    assert_eq!(derived.gas_price, 100_000_000);
}

/// Arc's 20 gwei floor (spec 060). Arc DISCARDS an underpriced transaction
/// without an error, so a price under the floor is a payment that silently
/// never happens — the floor is not an optimisation, it is the difference
/// between a loud failure and a lost send.
#[test]
fn arc_gas_price_never_falls_below_the_chain_floor() {
    const ARC: u32 = 5_042;
    const FLOOR: u128 = 20_000_000_000;

    // A read that comes back far under the floor is still floored.
    let derived = derive_chain_gas_price(&GasSignals {
        chain_id: ARC,
        eth_gas_price: 1_000_000_000,
        base_fee: 1_000_000_000,
        priority_fee: 0,
        tip_measured: None,
    });
    assert_eq!(derived.gas_price, FLOOR);
    assert_eq!(derived.base_fee, FLOOR);

    // A genuine read ABOVE the floor is untouched — the floor only raises.
    let derived = derive_chain_gas_price(&GasSignals {
        chain_id: ARC,
        eth_gas_price: 25_000_000_000,
        base_fee: 20_000_000_000,
        priority_fee: 1_000_000_000,
        tip_measured: Some(true),
    });
    assert_eq!(derived.gas_price, 25_000_000_000);

    // Arc testnet carries the same floor.
    assert_eq!(min_gas_price_wei(5_042_002), FLOOR);
}

/// The regression that matters more than the feature: every chain that had no
/// floor before must price bit-identically after. A floor that leaked into
/// another chain would silently change what people pay.
#[test]
fn no_other_chain_gained_a_gas_floor() {
    for chain_id in [
        1u32, 10, 56, 100, 130, 137, 143, 480, 4_217, 8_453, 42_161, 43_114, 196, 988, 1_868,
        4_326, 4_663, 5_000, 8_217, 42_220, 57_073, 98_866, 1_440_000,
    ] {
        assert_eq!(
            min_gas_price_wei(chain_id),
            0,
            "chain {chain_id} must have no gas-price floor"
        );
        let derived = derive_chain_gas_price(&GasSignals {
            chain_id,
            eth_gas_price: 1_000,
            base_fee: 400,
            priority_fee: 100,
            tip_measured: Some(true),
        });
        assert_eq!(derived.gas_price, 1_000, "chain {chain_id} gas price moved");
        assert_eq!(derived.base_fee, 400, "chain {chain_id} base fee moved");
    }
}

/// `usdPriceScaled` (`safe-transaction.ts:353-363`): 8-dp fixed point, strict
/// decimal grammar, directional rounding.
#[test]
fn usd_price_scaled_parses_strictly() {
    assert_eq!(
        usd_price_scaled(Some("1868.70000000"), true),
        Some(186_870_000_000)
    );
    assert_eq!(usd_price_scaled(Some("1"), false), Some(100_000_000));
    assert_eq!(usd_price_scaled(Some(" 5 "), false), Some(500_000_000));
    assert_eq!(usd_price_scaled(Some("0"), true), Some(0));
    // The regex `^(\d+)(?:\.(\d+))?$` rejects all of these.
    assert_eq!(usd_price_scaled(Some(""), true), None);
    assert_eq!(usd_price_scaled(Some("5."), true), None);
    assert_eq!(usd_price_scaled(Some(".5"), true), None);
    assert_eq!(usd_price_scaled(Some("1.2.3"), true), None);
    assert_eq!(usd_price_scaled(Some("abc"), true), None);
    assert_eq!(usd_price_scaled(Some("-1"), true), None);
    assert_eq!(usd_price_scaled(None, true), None);
}

/// Invariant ② — conversion never undercharges: the native price rounds UP
/// past 8 dp, the fee-token price truncates DOWN (`safe-transaction.ts:361`).
#[test]
fn conversion_rounds_native_up_fee_token_down_never_undercharging() {
    // 9th decimal digit: round_up bumps, round_down truncates.
    assert_eq!(
        usd_price_scaled(Some("1868.700000005"), true),
        Some(186_870_000_001)
    );
    assert_eq!(
        usd_price_scaled(Some("1868.700000005"), false),
        Some(186_870_000_000)
    );
    // A trailing zero tail is not a digit — no bump.
    assert_eq!(
        usd_price_scaled(Some("1868.700000000"), true),
        Some(186_870_000_000)
    );

    // And the conversion itself ceils: $18687.1 worth of fee units → 18688.
    let native = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("1868.71".to_owned()),
        native_usd_floor_price: None,
    };
    let usdc = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(1, 1, &usdc, &native),
        Some(18_688)
    );
}

/// `calculateInBandFeeAmount` native vectors (`inband-send.test.ts:191-201`).
#[test]
fn in_band_fee_native_is_gas_times_price_times_three() {
    let native = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("1868.70000000".to_owned()),
        native_usd_floor_price: None,
    };
    let native_without_price = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: None,
        native_usd_floor_price: None,
    };
    let usdc = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &native, &native),
        Some(600_000_000_000_000)
    );
    // Native payment works without any oracle price — it just floors at 0.001 of
    // the coin (the blind fallback) when the real fee is below it. Here the fee
    // (6e14) is under 0.001 native (1e15), so the floor binds.
    assert_eq!(
        calculate_in_band_fee_amount(
            200_000,
            1_000_000_000,
            &native_without_price,
            &native_without_price
        ),
        Some(1_000_000_000_000_000)
    );
    // Above the 0.001-coin fallback, the real gas fee flows through unpriced.
    assert_eq!(
        calculate_in_band_fee_amount(
            2_000_000,
            1_000_000_000,
            &native_without_price,
            &native_without_price
        ),
        Some(6_000_000_000_000_000)
    );
    // …but a stablecoin conversion without one is unsafe → cannot quote.
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc, &native_without_price),
        None
    );
}

/// Issue 682, the measured one: XLayer's OKB comes back from the relay with
/// `usdPrice: null`, so the blind 0.001-coin floor fired and every send paid
/// 0.001 OKB ≈ $0.12 — twelve times the cent that floor stands in for. The
/// wallet knew the price all along (the fee line rendered "≈$0.12" from it);
/// this hands it to the core for the MINIMUM only.
///
/// The real numbers, measured 2026-09-21: OKB ≈ $120, base 0.02 gwei, tip 0,
/// ~600k gas.
#[test]
fn an_unpriced_native_coin_floors_at_a_cent_when_the_shell_can_price_it() {
    let okb_blind = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: None,
        native_usd_floor_price: None,
    };
    let okb_shell_priced = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: None,
        native_usd_floor_price: Some("120".to_owned()),
    };

    // 600k gas × 0.02 gwei = 1.2e13 wei = 0.000012 OKB of real relay cost.
    let total_gas = 600_000u128;
    let gas_price = 20_000_000u128; // 0.02 gwei
    let relay_cost = total_gas * gas_price;
    assert_eq!(relay_cost, 12_000_000_000_000);

    // Before: a flat 0.001 OKB = 1e15 wei = $0.12.
    assert_eq!(
        calculate_in_band_fee_amount(total_gas, gas_price, &okb_blind, &okb_blind),
        Some(1_000_000_000_000_000)
    );

    // After: $0.01 worth at $120/OKB = 0.0000833… OKB, ceiled.
    let fee =
        calculate_in_band_fee_amount(total_gas, gas_price, &okb_shell_priced, &okb_shell_priced)
            .expect("a shell-priced native coin is quotable");
    assert_eq!(fee, 83_333_333_333_334);

    // The three inequalities the fix has to keep true, spelled out because a
    // cheaper floor that stops getting the transaction admitted is not a fix.
    //
    // ① still above the relay's 0.00001-coin admission floor (`admission.rs`).
    let admission_floor = 10_000_000_000_000u128; // 0.00001 OKB
    assert!(
        fee > admission_floor,
        "{fee} must clear the admission floor {admission_floor}"
    );
    assert!(fee / admission_floor >= 8, "8.3× the admission floor");
    // ② still above what the relay's settlement wants (1.4 × its own cost).
    let settlement = relay_cost * 14 / 10; // 0.0000168 OKB
    assert!(
        fee > settlement,
        "{fee} must fund settlement's {settlement}"
    );
    assert!(fee / settlement >= 4, "~5× the settlement requirement");
    // ③ and it is twelve times cheaper than the blind floor it replaces.
    assert_eq!(1_000_000_000_000_000u128 / fee, 11); // 12.0× before integer truncation
    assert!(fee * 12 > 1_000_000_000_000_000);
}

/// The clamp that makes the cheaper floor safe. On a coin dearer than ~$1000,
/// $0.01 buys LESS than 0.00001 of it, and a payment under the relay's
/// admission floor is simply refused — so `.max(admission_floor)` is
/// load-bearing on the new path exactly as it is on the relay-priced one.
#[test]
fn a_very_expensive_shell_priced_coin_still_meets_the_admission_floor() {
    // 0.00001 of an 18-decimal coin.
    let admission_floor = 10_000_000_000_000u128;
    // $2,000/coin: $0.01 is 5e-6 of it = 5e12 wei, HALF the admission floor.
    let dear = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: None,
        native_usd_floor_price: Some("2000".to_owned()),
    };
    assert_eq!(
        calculate_in_band_fee_amount(1, 1, &dear, &dear),
        Some(admission_floor)
    );
    // And a coin cheap enough for $0.01 to exceed the floor is NOT clamped up.
    let cheap = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: None,
        native_usd_floor_price: Some("120".to_owned()),
    };
    assert_eq!(
        calculate_in_band_fee_amount(1, 1, &cheap, &cheap),
        Some(83_333_333_333_334)
    );
}

/// The ruling this fix is fenced by: the floor price is for the FLOOR. The
/// stablecoin conversion keeps reading `usd_price` alone, so an unpriced native
/// coin still refuses a stablecoin fee — "a zero/absent USD price is 'cannot
/// quote', never rate 1". A display fallback is fine; a conversion fallback is
/// not, and this asserts the shell's price never became one.
#[test]
fn a_floor_price_never_becomes_a_stablecoin_conversion_rate() {
    let native_shell_priced = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: None,
        native_usd_floor_price: Some("120".to_owned()),
    };
    let usdc = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc, &native_shell_priced),
        None
    );
    // The native row of the very same quote IS payable — that asymmetry is the
    // point: we can floor without a rate, we cannot convert without one.
    assert!(calculate_in_band_fee_amount(
        200_000,
        1_000_000_000,
        &native_shell_priced,
        &native_shell_priced
    )
    .is_some());
}

/// The owner's constraint: nothing about what a PRICED send costs may move. A
/// relay price present wins outright, and the floor price beside it changes no
/// byte — on the native path or the stablecoin one.
#[test]
fn a_relay_priced_coin_ignores_the_floor_price_entirely() {
    let with_floor = |floor: Option<&str>| AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("1868.70000000".to_owned()),
        native_usd_floor_price: floor.map(str::to_owned),
    };
    let usdc = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    for floor in [None, Some("1"), Some("999999")] {
        let native = with_floor(floor);
        // The pre-issue-682 vectors of `in_band_fee_native_is_gas_times_price_times_three`
        // and `in_band_fee_converts_to_stable_with_cent_floor`, unchanged.
        assert_eq!(
            calculate_in_band_fee_amount(200_000, 1_000_000_000, &native, &native),
            Some(600_000_000_000_000)
        );
        assert_eq!(
            calculate_in_band_fee_amount(1, 1, &native, &native),
            Some(10_000_000_000_000)
        );
        assert_eq!(
            calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc, &native),
            Some(1_121_220)
        );
        assert_eq!(
            calculate_in_band_fee_amount(1, 1, &usdc, &native),
            Some(18_687)
        );
    }
}

/// A floor price that is not a price is not a price. Zero, blank and garbage
/// all fall through to the blind 0.001-coin fallback — the last resort stays,
/// and none of them is quietly read as 1.
#[test]
fn an_unusable_floor_price_falls_through_to_the_blind_fallback() {
    let blind_floor = 1_000_000_000_000_000u128; // 0.001 of an 18-dp coin
    for floor in [None, Some("0"), Some("0.00000000"), Some(""), Some("abc")] {
        let native = AssetPricing {
            is_native: true,
            decimals: 18,
            usd_price: None,
            native_usd_floor_price: floor.map(str::to_owned),
        };
        assert_eq!(
            calculate_in_band_fee_amount(600_000, 20_000_000, &native, &native),
            Some(blind_floor),
            "floor price {floor:?} must not be read as a price"
        );
    }
    // Below 3 decimals the blind fallback is one base unit, and the cap on the
    // shell-priced path is that same one unit.
    let tiny_blind = AssetPricing {
        is_native: true,
        decimals: 2,
        usd_price: None,
        native_usd_floor_price: None,
    };
    // Zero gas so the floor is the only thing left standing — at 1 gas × 1 wei
    // the ×3 markup (3 units) already clears both floors and would hide them.
    assert_eq!(
        calculate_in_band_fee_amount(0, 0, &tiny_blind, &tiny_blind),
        Some(1)
    );
    let tiny_priced = AssetPricing {
        is_native: true,
        decimals: 2,
        usd_price: None,
        native_usd_floor_price: Some("0.5".to_owned()),
    };
    // $0.01 at $0.50/coin would be 0.02 coin = 2 base units — ABOVE the blind
    // floor of 1 unit, so the cap holds it to 1: the shell's price only ever
    // lowers this floor. See `a_shell_price_may_only_lower_the_floor`.
    assert_eq!(
        calculate_in_band_fee_amount(0, 0, &tiny_priced, &tiny_priced),
        Some(1)
    );
}

/// The cap on the new path, and the reason for it.
///
/// The floor is `$0.01 ÷ price`, so an UNDER-estimated price RAISES what the
/// user pays. The shell's price is its own on-chain derivation, and on exactly
/// the coins this branch serves — no relay price, therefore almost never a
/// Chainlink feed — `choose_native_price` hands back the raw DEX quote with no
/// sanity band; `wallet-api.ts` records X Layer's WOKB/USDC pool quoting OKB at
/// ~$5 against a true ~$120. Uncapped, that junk quote would charge 0.002 OKB,
/// twice the blind floor this issue exists to cut. So the shell's price may
/// only ever LOWER the floor: the worst case is what today already charges.
///
/// The same cap answers the other direction: a genuinely CHEAP coin (under $10,
/// where a cent is worth more than 0.001 of it) keeps today's 0.001-coin floor
/// instead of a fee 10× or 500× larger. A person holding a sliver of a cheap
/// coin who can send today can still send.
#[test]
fn a_shell_price_may_only_lower_the_floor_never_raise_it() {
    let blind_floor = 1_000_000_000_000_000u128; // 0.001 of an 18-dp coin
    let shell_priced = |price: &str| AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: None,
        native_usd_floor_price: Some(price.to_owned()),
    };

    // The measured junk quote: OKB read at $5 instead of ~$120. Uncapped this
    // is ceil(1e6 × 1e18 / 5e8) = 2e15 wei = 0.002 coin, twice the blind floor.
    let junk = shell_priced("5");
    assert_eq!(
        calculate_in_band_fee_amount(0, 0, &junk, &junk),
        Some(blind_floor),
        "a low price must not raise the floor above what the blind fallback charges"
    );

    // A cheap coin, no bad price involved: $1/coin values a cent at 0.01 coin,
    // ten times the blind floor. Capped, it stays at the blind floor.
    let cheap = shell_priced("1");
    assert_eq!(
        calculate_in_band_fee_amount(0, 0, &cheap, &cheap),
        Some(blind_floor)
    );
    // $0.02/coin — 0.5 coin uncapped, 500× — likewise.
    let very_cheap = shell_priced("0.02");
    assert_eq!(
        calculate_in_band_fee_amount(0, 0, &very_cheap, &very_cheap),
        Some(blind_floor)
    );

    // $10 is the crossover: at exactly $10 a cent IS 0.001 of the coin, so cap
    // and cent agree, and every coin dearer than that gets the saving.
    let crossover = shell_priced("10");
    assert_eq!(
        calculate_in_band_fee_amount(0, 0, &crossover, &crossover),
        Some(blind_floor)
    );
    let dearer = shell_priced("10.01");
    let fee = calculate_in_band_fee_amount(0, 0, &dearer, &dearer).expect("priceable");
    assert!(fee < blind_floor, "{fee} must be under the blind floor");

    // The relay-priced path is NOT capped: a price we did not derive ourselves
    // is the one we quote against, and $0.01 worth of a $1 coin is 0.01 of it.
    // This is the pre-682 answer, unchanged.
    let relay_priced_cheap = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: Some("1".to_owned()),
    };
    assert_eq!(
        calculate_in_band_fee_amount(0, 0, &relay_priced_cheap, &relay_priced_cheap),
        Some(10_000_000_000_000_000)
    );
}

/// `calculateInBandFeeAmount` stablecoin vectors (`inband-send.test.ts:203-214`),
/// including the native 0.00001 floor and the $0.01 stable floor (invariant ③).
#[test]
fn in_band_fee_converts_to_stable_with_cent_floor() {
    let native = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("1868.70000000".to_owned()),
        native_usd_floor_price: None,
    };
    let usdc = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    // 0.0006 ETH × $1868.70 = $1.12122 = 1.121220 USDC.
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc, &native),
        Some(1_121_220)
    );
    // 1 wei × 3 is below the 0.00001 ETH floor; $0.018687 exceeds $0.01.
    assert_eq!(
        calculate_in_band_fee_amount(1, 1, &usdc, &native),
        Some(18_687)
    );
    // If the native floor converts below one cent, stablecoin payment still
    // floors at $0.01.
    let low_price_native = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("100".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(1, 1, &usdc, &low_price_native),
        Some(10_000)
    );
}

/// Invariant ④'s sibling: a ZERO price is unpriceable, never rate 1 — JS
/// `!nativeUsdPrice` is falsy for 0n too (`safe-transaction.ts:386`).
#[test]
fn zero_usd_price_is_unpriceable_not_rate_one() {
    let native_zero = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("0".to_owned()),
        native_usd_floor_price: None,
    };
    let native = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("1868.70".to_owned()),
        native_usd_floor_price: None,
    };
    let usdc_zero = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("0".to_owned()),
        native_usd_floor_price: None,
    };
    let usdc = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc, &native_zero),
        None
    );
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc_zero, &native),
        None
    );
    // A non-native "native" asset is a caller error → refuse.
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc, &usdc),
        None
    );
}

/// The blocker this round exists for: an 18-decimal gas asset was quoted at the
/// $0.01 floor because the conversion's numerator was clamped to `u128::MAX`
/// and the following division pulled it back into a plausible-looking range.
///
/// The exact numerator for a 700k-gas send at 30 gwei with ETH at $2,000 and an
/// 18-decimal fee token is `6.3e16 × 2e11 × 1e18 = 1.26e46`, twenty-four decimal
/// orders past `u128::MAX = 3.4e38`. Clamped-then-divided it lands on 3.4e12,
/// below the `stable_minimum` of 1e16 — 126 DAI quoted as one cent.
///
/// DAI, plus USDT and USDC on BNB Chain, are all 18 decimals; every vector and
/// every scenario before this used 6, which is why both drift gates stayed
/// green through it.
#[test]
fn eighteen_decimal_fee_asset_is_priced_exactly_not_clamped_to_the_cent_floor() {
    let eth = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("2000".to_owned()),
        native_usd_floor_price: None,
    };
    let dai = AssetPricing {
        is_native: false,
        decimals: 18,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    // 700_000 × 30 gwei × 3 = 0.063 ETH = $126 = 126 DAI.
    assert_eq!(
        calculate_in_band_fee_amount(700_000, 30_000_000_000, &dai, &eth),
        Some(126_000_000_000_000_000_000)
    );
    // …and NOT the $0.01 floor, which is what the clamp produced.
    assert_ne!(
        calculate_in_band_fee_amount(700_000, 30_000_000_000, &dai, &eth),
        Some(10_000_000_000_000_000)
    );
    // The floor itself is still honest when the fee genuinely is below a cent.
    let cheap_native = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("100".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(1, 1, &dai, &cheap_native),
        Some(10_000_000_000_000_000)
    );
    // The other precisions the relay can publish: 8 (WBTC) and 0.
    let wbtc = AssetPricing {
        is_native: false,
        decimals: 8,
        usd_price: Some("60000".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &wbtc, &eth),
        Some(2_000)
    );
    let whole = AssetPricing {
        is_native: false,
        decimals: 0,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(700_000, 30_000_000_000, &whole, &eth),
        Some(126)
    );
}

/// Monotonicity is the property a clamp destroys: with everything else fixed,
/// more gas must cost MORE fee-token units. The old code broke it flat — past
/// the overflow point every basis collapsed onto the same cent floor, so a
/// 30M-gas batch and a 21k transfer quoted the identical $0.01. Strict `>` is
/// therefore the assertion; `>=` would have been satisfied by the collapse.
#[test]
fn a_bigger_gas_basis_never_produces_a_smaller_fee() {
    let eth = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("2000".to_owned()),
        native_usd_floor_price: None,
    };
    let dai = AssetPricing {
        is_native: false,
        decimals: 18,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    let mut previous = 0u128;
    for gas in [
        1u128, 21_000, 200_000, 450_000, 700_000, 3_000_000, 30_000_000,
    ] {
        let amount = calculate_in_band_fee_amount(gas, 30_000_000_000, &dai, &eth)
            .expect("a real gas basis is priceable");
        assert!(
            amount > previous,
            "{gas} gas priced {amount}, not more than the previous step's {previous} — \
             a flat line here is the clamp collapsing every basis onto one floor"
        );
        previous = amount;
    }
}

/// Where the exact answer genuinely does not fit `u128`, the machine REFUSES
/// (`None` → `FeeFailure::CalculationFailed`). It must never emit a clamped,
/// smaller-than-true number, which is the only failure mode that can quietly
/// undercharge.
#[test]
fn an_unrepresentable_conversion_refuses_instead_of_shrinking() {
    let eth = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("2000".to_owned()),
        native_usd_floor_price: None,
    };
    let absurd_precision = AssetPricing {
        is_native: false,
        decimals: 60,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    // 0.063 ETH → $126 → 126e60 units, which no `u128` can hold.
    assert_eq!(
        calculate_in_band_fee_amount(700_000, 30_000_000_000, &absurd_precision, &eth),
        None
    );
    // A `decimals` past even 256-bit representability is a refusal too, not a
    // clamped unit that would deflate the quotient.
    let nonsense = AssetPricing {
        is_native: false,
        decimals: 200,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(700_000, 30_000_000_000, &nonsense, &eth),
        None
    );
    // Native side, same rule: a gas basis whose ×3 markup cannot be represented
    // is refused rather than clamped to a payable-looking `u128::MAX`.
    let native_pair = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some("2000".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(u128::MAX, u128::MAX, &native_pair, &eth),
        None
    );
}

/// `usd_price_scaled` is the conversion's denominator for the fee token and its
/// numerator for the native coin, so a clamp there moves the quote in opposite
/// directions depending on which side asked. Unrepresentable ⇒ unpriceable.
#[test]
fn an_unrepresentable_usd_price_is_unpriceable_not_clamped() {
    let huge = "1".to_owned() + &"0".repeat(35); // 1e35 × 1e8 = 1e43 > u128::MAX
    assert_eq!(usd_price_scaled(Some(&huge), false), None);
    assert_eq!(usd_price_scaled(Some(&huge), true), None);
    // …and it propagates as a refusal, not as a rate.
    let native = AssetPricing {
        is_native: true,
        decimals: 18,
        usd_price: Some(huge.clone()),
        native_usd_floor_price: None,
    };
    let usdc = AssetPricing {
        is_native: false,
        decimals: 6,
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    };
    assert_eq!(
        calculate_in_band_fee_amount(200_000, 1_000_000_000, &usdc, &native),
        None
    );
}

fn erc20_fee_estimate() -> FeeEstimate {
    // The jest fixture (`safe-transaction.test.ts:33-46`).
    FeeEstimate {
        chain_id: 1,
        total_wei: 0,
        max_fee_per_gas: 0,
        network_fee_per_gas: 1,
        relayer_fee_per_gas: 0,
        bundler_gas_price: 1,
        in_band_gas_basis: 1,
        effective_gas_price: None,
        max_gas_price: None,
        total_gas: 1,
        deployed: true,
        tier: FeeTier::Fast,
        quoted: true,
        fee_asset: FeeAsset::Erc20 {
            token: "0x1111111111111111111111111111111111111111".to_owned(),
            decimals: 6,
            amount: 69_800,
            symbol: None,
        },
        fee_recipient: None,
    }
}

/// `sameAssetFeeLimit` vectors (`safe-transaction.test.ts:30-65`).
#[test]
fn same_asset_fee_limit_matches_ts_vectors() {
    let usdt = "0x1111111111111111111111111111111111111111";
    let usdc = "0x2222222222222222222222222222222222222222";
    let fee = erc20_fee_estimate();

    // Reserves an ERC-20 fee only when it is the token being transferred
    // (case-insensitively).
    let limit = same_asset_fee_limit(Some(&fee), Some(&usdt.to_uppercase()), 15_000_000)
        .expect("same token reserves");
    assert_eq!(limit.fee_amount, 69_800);
    assert_eq!(limit.max_transfer_amount, 14_930_200);
    assert!(same_asset_fee_limit(Some(&fee), Some(usdc), 15_000_000).is_none());

    // Reserves the native fee only for a native transfer.
    let native_fee = FeeEstimate {
        total_wei: 42,
        fee_asset: FeeAsset::Native,
        ..erc20_fee_estimate()
    };
    let limit = same_asset_fee_limit(Some(&native_fee), None, 100).expect("native reserves");
    assert_eq!(limit.fee_amount, 42);
    assert_eq!(limit.max_transfer_amount, 58);
    assert!(same_asset_fee_limit(Some(&native_fee), Some(usdt), 100).is_none());

    // Clamps the maximum to zero when the fee itself exhausts the balance.
    let limit = same_asset_fee_limit(Some(&fee), Some(usdt), 69_800).expect("clamps");
    assert_eq!(limit.fee_amount, 69_800);
    assert_eq!(limit.max_transfer_amount, 0);

    // No estimate → no limit.
    assert!(same_asset_fee_limit(None, Some(usdt), 100).is_none());
}

/// `rawBundlerGasCost` (`safe-transaction.ts:431-434`): divide the tier markup
/// back out.
#[test]
fn raw_bundler_gas_cost_divides_tier_markup_out() {
    let fee = FeeEstimate {
        total_wei: 40,
        tier: FeeTier::Fast,
        ..erc20_fee_estimate()
    };
    assert_eq!(raw_bundler_gas_cost(&fee), 20); // 40 × 10/20
    let fee = FeeEstimate {
        total_wei: 24,
        tier: FeeTier::Standard,
        ..erc20_fee_estimate()
    };
    assert_eq!(raw_bundler_gas_cost(&fee), 20); // 24 × 10/12
}

// ===========================================================================
// Base-unit string math + reserve math (`batch-send.test.ts` vectors)
// ===========================================================================

#[test]
fn base_unit_round_trip_is_lossless() {
    // `full-balance multiSelect precision (round-trip)` vectors.
    for (raw, dec) in [
        (1u128, 18u32),
        (31_743_219_870_000_000_000, 18),
        (123_456, 6),
        (10u128.pow(30) + 7, 18),
        (999_999_999, 0),
    ] {
        assert_eq!(
            to_base_units(&from_base_units(raw, dec), dec),
            Some(raw),
            "round-trip {raw} @ {dec}dp"
        );
    }
}

#[test]
fn to_base_units_keeps_ts_quirks_and_refuses_garbage() {
    assert_eq!(to_base_units("1.5", 18), Some(1_500_000_000_000_000_000));
    assert_eq!(to_base_units("", 18), Some(0));
    assert_eq!(to_base_units("  ", 18), Some(0));
    // TS `split('.')` destructuring quirk: everything after a second dot
    // vanishes — "1.2.3" reads as 1.2 (ported verbatim).
    assert_eq!(to_base_units("1.2.3", 6), Some(1_200_000));
    // Excess fractional digits truncate.
    assert_eq!(to_base_units("0.1234567", 6), Some(123_456));
    // Deliberate u128 strictness: negative/garbage answers None where BigInt
    // would go negative or throw — garbage never mints units.
    assert_eq!(to_base_units("-1", 18), None);
    assert_eq!(to_base_units("abc", 18), None);
    assert_eq!(to_base_units("1.x", 18), None);
}

fn native_line(amount: &str) -> MultiTokenSpec {
    MultiTokenSpec {
        token_address: None,
        decimals: 18,
        amount: amount.to_owned(),
    }
}

fn erc20_line(token: &str, decimals: u32, amount: &str) -> MultiTokenSpec {
    MultiTokenSpec {
        token_address: Some(token.to_owned()),
        decimals,
        amount: amount.to_owned(),
    }
}

/// `reserveNativeGas` vectors (`batch-send.test.ts:182-200`).
#[test]
fn reserve_native_gas_trims_only_the_native_line() {
    let erc20 = erc20_line(USDC, 6, "10");
    let native = native_line("1");
    let out = reserve_native_gas(&[erc20.clone(), native.clone()], 200_000_000_000_000_000);
    assert_eq!(out[0], erc20, "ERC-20 untouched");
    assert_eq!(out[1], native_line("0.8"));

    // Drops the native line if the balance cannot cover the reserve.
    let out = reserve_native_gas(&[erc20.clone(), native.clone()], 5_000_000_000_000_000_000);
    assert_eq!(out, vec![erc20.clone()]);

    // No-op for a zero reserve (e.g. Tempo).
    let out = reserve_native_gas(&[erc20.clone(), native.clone()], 0);
    assert_eq!(out, vec![erc20, native]);
}

/// `reserveFeeToken` vectors (`batch-send.test.ts:234-261`).
#[test]
fn reserve_fee_token_trims_only_the_fee_asset_line() {
    let pathusd = erc20_line(TEMPO_DEFAULT_FEE_TOKEN, 6, "1");
    let other = erc20_line(USDC, 6, "10");

    let out = reserve_fee_token(
        &[other.clone(), pathusd.clone()],
        TEMPO_DEFAULT_FEE_TOKEN,
        200_000,
    );
    assert_eq!(out[0], other, "other TIP-20s pay no gas and pass through");
    assert_eq!(out[1], erc20_line(TEMPO_DEFAULT_FEE_TOKEN, 6, "0.8"));

    // Case-insensitive token match.
    let upper = TEMPO_DEFAULT_FEE_TOKEN.to_uppercase().replace("0X", "0x");
    let out = reserve_fee_token(std::slice::from_ref(&pathusd), &upper, 200_000);
    assert_eq!(out[0].amount, "0.8");

    // Drops the line if the whole balance is needed for gas.
    let out = reserve_fee_token(
        &[other.clone(), pathusd.clone()],
        TEMPO_DEFAULT_FEE_TOKEN,
        5_000_000,
    );
    assert_eq!(out, vec![other.clone()]);

    // No-op when the fee token is absent, and for a zero reserve.
    let out = reserve_fee_token(
        std::slice::from_ref(&other),
        TEMPO_DEFAULT_FEE_TOKEN,
        200_000,
    );
    assert_eq!(out, vec![other.clone()]);
    let out = reserve_fee_token(
        &[other.clone(), pathusd.clone()],
        TEMPO_DEFAULT_FEE_TOKEN,
        0,
    );
    assert_eq!(out, vec![other, pathusd]);
}

/// `maxNativeSendable` vectors (`batch-send.test.ts:202-231`) — string-exact,
/// so the send screen's own "insufficient for gas" pre-check never trips on
/// its own Max fill.
#[test]
fn max_native_sendable_is_string_exact() {
    let reserve: u128 = 3_000_000_000_000_000; // 0.003
    for (bal, expect) in [
        ("1.5", "1.497"),
        ("2500.55", "2500.547"),
        ("12345.6789", "12345.6759"),
        ("0.5", "0.497"),
        ("100.123456789012345678", "100.120456789012345678"),
    ] {
        let balance = to_base_units(bal, 18).expect("fixture balance parses");
        let max = max_native_sendable(balance, reserve, 18);
        assert_eq!(max, expect, "no float garbage for balance {bal}");
        // Exact: the sent amount plus the reserve equals the whole balance.
        assert_eq!(to_base_units(&max, 18), Some(balance - reserve));
    }

    // "0" when the balance cannot even cover the reserve (or exactly equals it).
    assert_eq!(
        max_native_sendable(to_base_units("0.001", 18).expect("parses"), reserve, 18),
        "0"
    );
    assert_eq!(max_native_sendable(reserve, reserve, 18), "0");

    // Respects non-18-decimal tokens.
    assert_eq!(max_native_sendable(5_000_000, 1_250_000, 6), "3.75");
}

/// `encodeErc20Transfer` layout (`safe-transaction.test.ts:235-250`).
#[test]
fn encode_erc20_transfer_matches_canonical_layout() {
    let usdc = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
    let out = encode_erc20_transfer(usdc, 1_000_000).expect("valid recipient encodes");
    assert_eq!(out.len(), 2 + 8 + 64 + 64);
    assert!(
        out.starts_with("0xa9059cbb"),
        "transfer(address,uint256) selector"
    );
    let recipient_word = &out[10..74];
    assert_eq!(
        recipient_word,
        format!("{:0>64}", usdc[2..].to_lowercase()),
        "recipient lowercased and left-padded"
    );
    let amount_word = &out[74..138];
    assert_eq!(amount_word, format!("{:0>64}", format!("{:x}", 1_000_000)));

    // Zero amount encodes a zero word.
    let out = encode_erc20_transfer(usdc, 0).expect("zero encodes");
    assert_eq!(&out[74..138], "0".repeat(64));

    // Invalid recipient → None (TS throws BatchSendError).
    assert!(encode_erc20_transfer("0x123", 1).is_none());
    assert!(encode_erc20_transfer("not an address", 1).is_none());
}

// ===========================================================================
// Tempo gas model (`tempo.test.ts` vectors)
// ===========================================================================

#[test]
fn tempo_chain_set_is_verbatim() {
    assert!(is_tempo_chain(4_217));
    assert!(is_tempo_chain(42_431));
    for id in [1, 56, 137, 42_161, 10, 8_453, 100, 43_114] {
        assert!(!is_tempo_chain(id), "chain {id} is not Tempo");
    }
    assert_eq!(
        TEMPO_DEFAULT_FEE_TOKEN,
        "0x20c0000000000000000000000000000000000000"
    );
}

#[test]
fn atto_to_token_units_converts_attodollars() {
    // 1e15 attodollars = $0.001 = 1000 microdollars.
    assert_eq!(atto_to_token_units(1_000_000_000_000_000, 6), 1_000);
    assert_eq!(atto_to_token_units(10u128.pow(18), 6), 10u128.pow(6));
    assert_eq!(atto_to_token_units(0, 6), 0);
}

#[test]
fn tempo_fee_token_units_prices_gas_plus_overhead() {
    // (50_000 + 150_000) × 20e9 atto = 4e15 atto = $0.004 = 4000 units.
    assert_eq!(tempo_fee_token_units(50_000, TEMPO_BASE_FEE_ATTO, 6), 4_000);
    // Falls back to the protocol base fee when gasPrice is 0.
    assert_eq!(
        tempo_fee_token_units(50_000, 0, 6),
        tempo_fee_token_units(50_000, TEMPO_BASE_FEE_ATTO, 6)
    );
}

#[test]
fn tempo_expected_gas_prices_realistic_batches() {
    let gas = tempo_expected_gas(true, 2);
    assert_eq!(gas, TEMPO_DEPLOYED_GAS_EST + 2 * TEMPO_PER_SUBCALL_GAS_EST);
    assert!(gas > 380_000 && gas < 520_000, "near the measured ~420k");
    assert!(tempo_expected_gas(false, 2) > TEMPO_DEPLOY_GAS_EST);
    // At least one sub-call is always budgeted.
    assert_eq!(tempo_expected_gas(true, 0), tempo_expected_gas(true, 1));
}

/// Invariant ③ — the $0.01 stablecoin floor (`tempo.test.ts:89-105`).
#[test]
fn tempo_reimbursement_charges_double_with_cent_floor() {
    // 2× (100% margin) the realistic cost, NOT the padded limits.
    let gas: u128 = 500_000;
    let raw = atto_to_token_units(gas * TEMPO_BASE_FEE_ATTO, 6);
    assert_eq!(tempo_reimbursement(gas, TEMPO_BASE_FEE_ATTO, 6), raw * 2);

    // Floors every USD stablecoin reimbursement at $0.01.
    assert_eq!(tempo_minimum_fee_token_units(6), 10_000);
    assert_eq!(tempo_reimbursement(0, 0, 6), 10_000);
    assert_eq!(tempo_reimbursement(100_000, 1_000_000_000, 6), 10_000);

    // Rounds the floor up to a transferable unit for low-decimal assets.
    assert_eq!(tempo_minimum_fee_token_units(1), 1);
    assert_eq!(tempo_reimbursement(0, 0, 1), 1);
}

/// Bundler accept-check cost basis: ceilDiv((simGas + buffer) × price → units)
/// — matches vela-relay `tempoCostInFeeToken` (`tempo.test.ts:26-30`).
fn bundler_cost_units(sim_gas: u128, price: u128) -> u128 {
    let atto = (sim_gas + TEMPO_COST_BUFFER_GAS) * price;
    let num = atto * 10u128.pow(6);
    let den = 10u128.pow(18);
    num.div_ceil(den)
}

#[test]
fn tempo_settlement_split_floors_the_eoa_at_bundler_cost() {
    let price = TEMPO_BASE_FEE_ATTO;
    let gas: u128 = 500_000;
    let reimbursement = tempo_reimbursement(gas, price, 6); // 2× base
    let split = tempo_settlement_split(reimbursement, gas, price, 6);
    let expected_floor = atto_to_token_units(
        (gas + TEMPO_COST_BUFFER_GAS + TEMPO_SPLIT_SAFETY_GAS) * price,
        6,
    );
    assert_eq!(split.eoa, expected_floor);
    assert_eq!(split.treasury, reimbursement - expected_floor);
    // Conserves the total.
    assert_eq!(split.eoa + split.treasury, reimbursement);
    // The EOA share always clears the bundler's cost (realGas + buffer).
    assert!(split.eoa >= atto_to_token_units((gas + TEMPO_COST_BUFFER_GAS) * price, 6));
    assert!(split.treasury > 0);
}

#[test]
fn tempo_settlement_split_keeps_everything_on_the_eoa_when_thin() {
    let price = TEMPO_BASE_FEE_ATTO;
    let gas: u128 = 500_000;
    let floor = atto_to_token_units(
        (gas + TEMPO_COST_BUFFER_GAS + TEMPO_SPLIT_SAFETY_GAS) * price,
        6,
    );
    let thin = floor - 1;
    let split = tempo_settlement_split(thin, gas, price, 6);
    assert_eq!(split.eoa, thin);
    assert_eq!(split.treasury, 0, "never a rejection");
}

/// Regression for the Tempo deploy rejection (reimbursed=89700 < cost=90025):
/// the wallet's realistic-gas estimate sits BELOW the bundler's simulated gas;
/// the proportional cushion must still carry the EOA floor over the cost
/// (`tempo.ts:167-183`; `tempo.test.ts:144-169`).
#[test]
fn tempo_eoa_floor_clears_bundler_cost_despite_estimate_drift() {
    let price = TEMPO_BASE_FEE_ATTO;
    let wallet_gas: u128 = 4_385_000; // wallet model for a 3-sub-call undeployed send
    let bundler_sim_gas: u128 = 4_421_208; // actual simulated gas — 36,208 higher
    let reimbursement = tempo_reimbursement(wallet_gas, price, 6);
    let split = tempo_settlement_split(reimbursement, wallet_gas, price, 6);
    assert!(split.eoa >= bundler_cost_units(bundler_sim_gas, price));
    assert!(
        split.eoa > 90_025,
        "beats the incident's rejection threshold"
    );
    assert!(split.treasury > 0, "still routes surplus to the treasury");

    // And across a wide range of estimate error, up to +3% simGas drift.
    for wallet_gas in [500_000u128, 1_500_000, 4_385_000, 6_000_000] {
        let reimbursement = tempo_reimbursement(wallet_gas, price, 6);
        let split = tempo_settlement_split(reimbursement, wallet_gas, price, 6);
        let sim_gas = wallet_gas + (wallet_gas * 3) / 100;
        assert!(
            split.eoa >= bundler_cost_units(sim_gas, price),
            "drift-proof at walletGas {wallet_gas}"
        );
    }
}

#[test]
fn tempo_split_safety_gas_is_flat_then_proportional() {
    // 500k × 3% = 15k < 20k flat → flat wins.
    assert_eq!(tempo_split_safety_gas(500_000), TEMPO_SPLIT_SAFETY_GAS);
    // Scales with the op for large ops.
    let gas: u128 = 4_385_000;
    assert_eq!(
        tempo_split_safety_gas(gas),
        (gas * TEMPO_SPLIT_SAFETY_BPS) / 10_000
    );
    assert!(tempo_split_safety_gas(gas) > 130_000);
}

#[test]
fn tempo_call_gas_limit_scales_per_subcall() {
    assert_eq!(tempo_call_gas_limit(2), 2 * TEMPO_CALL_GAS_PER_SUBCALL);
    assert_eq!(tempo_call_gas_limit(3), 3 * TEMPO_CALL_GAS_PER_SUBCALL);
    // Comfortably exceeds the measured ~308k of a single TIP-20 transfer.
    assert!(tempo_call_gas_limit(1) > 308_000);
    // Never 0.
    assert_eq!(tempo_call_gas_limit(0), TEMPO_CALL_GAS_PER_SUBCALL);
}

/// Invariant ③ submit-side — a quote whose recipient changed or whose amount
/// predates the $0.01 floor is stale and must be re-reviewed
/// (`safe-transaction.ts:1087-1095`).
#[test]
fn tempo_recipient_change_or_subfloor_quote_is_stale() {
    // Recipient matches (case-insensitively) and the amount meets the floor.
    assert!(!tempo_quote_is_stale(
        10_000,
        COLLECTOR,
        &COLLECTOR.to_uppercase().replace("0X", "0x"),
        6
    ));
    // The relay rotated its recipient → stale.
    assert!(tempo_quote_is_stale(10_000, COLLECTOR, NATIVE_RECIPIENT, 6));
    // A cached pre-floor amount → stale.
    assert!(tempo_quote_is_stale(9_999, COLLECTOR, COLLECTOR, 6));
}

// ===========================================================================
// Machine — quote lifecycle
// ===========================================================================

#[test]
fn happy_path_settles_a_native_quote() {
    let sut = quoted_native(vec![]);
    let view = sut.view();
    assert!(!view.busy);
    assert!(view.failed.is_none());
    assert!(!view.stale);
    assert!(view.confirm_fee_ready);
    assert_eq!(view.fee_token, None, "starts on the native asset");
    let fee = view.fee.expect("settled quote");
    assert_eq!(fee.chain_id, CHAIN);
    assert_eq!(fee.total_gas, TOTAL_GAS.to_string());
    assert_eq!(fee.network_fee_per_gas, NETWORK_FEE.to_string());
    assert_eq!(fee.total_wei, NATIVE_FEE_WEI.to_string());
    // Every signed in-band UserOp pays maxFeePerGas = 0; the fee rides in the leg.
    assert_eq!(fee.max_fee_per_gas, "0");
    assert!(fee.quoted, "priced from the bundler quote");
    assert_eq!(fee.fee_asset, FeeAssetView::Native);
    assert_eq!(fee.fee_recipient.as_deref(), Some(NATIVE_RECIPIENT));
}

/// The fee-asset picker: the native row always, zero-balance stables never
/// (`use-inband-fee-tokens.ts:50-53`), amounts priced off the shared basis.
#[test]
fn picker_shows_native_always_and_held_stables_only() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![
            native_row("0"),
            usdc_row("5000000"),
            FeeAssetQuote {
                balance: "0".to_owned(),
                symbol: "DAI".to_owned(),
                ..usdc_row("0")
            },
        ]),
    });
    sut.resolve(estimated());
    let view = sut.view();
    let symbols: Vec<&str> = view.options.iter().map(|o| o.symbol.as_str()).collect();
    assert_eq!(
        symbols,
        vec!["ETH", "USDC"],
        "zero-balance stable omitted, empty native kept"
    );
    // A zero-balance native row is shown for context but cannot pay.
    assert!(view.options[0].insufficient);
    assert!(view.options[0].selected);
    assert_eq!(
        view.options[1].amount.as_deref(),
        Some(&*USDC_FEE_UNITS.to_string())
    );
    assert!(!view.options[1].insufficient);
}

/// Issue #262: an account holding 0 of the native coin (2 USDT and 0 ETH on
/// mainnet) was quoted in ETH and the confirm gate stayed open — the op was
/// signed, accepted and never bundled. The native row is still the default
/// and still quoted; it is simply not confirmable, and switching to a coin
/// that can pay opens the gate.
#[test]
fn a_native_fee_the_account_does_not_hold_is_not_confirmable() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![native_row("0"), usdc_row("5000000")]),
    });
    sut.resolve(estimated());
    let view = sut.view();
    assert!(
        view.fee.is_some(),
        "still quoted: the person sees the figure"
    );
    assert!(view.failed.is_none());
    let native = view.options.iter().find(|o| o.contract.is_none()).unwrap();
    assert!(native.selected && native.insufficient);
    assert!(!view.confirm_fee_ready, "a fee in a coin that is not there");

    sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_owned()),
    });
    assert!(sut.view().confirm_fee_ready, "USDC covers it");
}

/// The gate refuses only a PROVABLE shortfall: a native balance that covers
/// the fee stays confirmable (the happy path above), and one exactly equal to
/// it is enough.
#[test]
fn a_native_balance_exactly_covering_the_fee_is_confirmable() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![native_row(&NATIVE_FEE_WEI.to_string()), usdc_row("0")]),
    });
    sut.resolve(estimated());
    assert!(sut.view().confirm_fee_ready);
}

/// `GasFeeCard.handleFeeTokenSelect` fast path: a known option recomputes
/// locally from the shared gas basis — no RPC round trip.
#[test]
fn select_fee_asset_recomputes_locally_without_rpc() {
    let mut sut = quoted_native(vec![]);
    let ops = sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_uppercase().replace("0X", "0x")), // case-insensitive
    });
    assert!(ops.is_empty(), "local recompute issues no operations");
    let view = sut.view();
    assert!(view.confirm_fee_ready);
    assert_eq!(
        view.fee_token.as_deref().map(str::to_lowercase),
        Some(USDC.to_lowercase())
    );
    let fee = view.fee.expect("quote survives the switch");
    assert_eq!(
        fee.total_wei, "0",
        "erc20 fee rides in fee_asset, not totalWei"
    );
    assert_eq!(
        fee.fee_asset,
        FeeAssetView::Erc20 {
            token: USDC.to_owned(),
            decimals: 6,
            amount: USDC_FEE_UNITS.to_string(),
            // The row's own ticker rides with the quote: a fee with no symbol
            // is drawn as the native coin on every surface.
            symbol: Some("USDC".to_owned()),
        }
    );
    // The recipient switches WITH the asset so approve/submit sends exactly
    // what was quoted.
    assert_eq!(fee.fee_recipient.as_deref(), Some(USDC_RECIPIENT));

    // Selecting the already-active asset is a no-op.
    let ops = sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_owned()),
    });
    assert!(ops.is_empty());
}

/// Invariant ⑧ — a fee asset whose balance is below the fee it would cost is
/// shown for context but NOT selectable (`FeeTokenSelector.tsx:74`).
#[test]
fn balance_below_fee_asset_cannot_be_selected() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    // 2 USDC held < the 2.522745 USDC this tx costs.
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![native_row("1000000000000000000"), usdc_row("2000000")]),
    });
    sut.resolve(estimated());
    let view = sut.view();
    assert!(view.options[1].insufficient);

    let ops = sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_owned()),
    });
    assert!(ops.is_empty(), "a doomed op is never quoted");
    let view = sut.view();
    assert_eq!(view.fee_token, None, "selection unchanged");
    assert_eq!(
        view.fee.expect("quote intact").fee_asset,
        FeeAssetView::Native
    );
}

/// The slow path: an asset missing from the cached rows falls back to a full
/// re-estimate whose failure reverts the selection
/// (`GasFeeCard.handleFeeTokenSelect` catch → `onFeeTokenChange(prev)`).
#[test]
fn select_unknown_asset_requotes_and_reverts_on_failure() {
    let dai = "0x5555555555555555555555555555555555555555";
    let mut sut = quoted_native(vec![]);
    let ops = sut.dispatch(Event::SelectFeeAsset {
        token: Some(dai.to_owned()),
    });
    assert_eq!(ops.len(), 3, "full pipeline re-runs");
    assert!(
        sut.view().busy,
        "re-quoting → confirm stays disabled (invariant ⑦)"
    );

    // The superseded TTL timer from the first quote resolves late → dropped.
    assert!(sut.resolve(Res::TtlElapsed).is_empty());
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    // The relay does not quote DAI → the switch fails…
    let ops = sut.resolve(quotes_ok());
    assert!(ops.is_empty());
    let view = sut.view();
    // …and the selection reverts to the previous asset with the old quote intact.
    assert!(!view.busy);
    assert!(
        view.failed.is_none(),
        "a failed switch never scraps a good quote"
    );
    assert_eq!(view.fee_token, None);
    assert_eq!(
        view.fee.expect("old quote survives").total_wei,
        NATIVE_FEE_WEI.to_string()
    );
    assert!(view.confirm_fee_ready);
}

/// Invariant ⑨, the version that costs money: when the caller asks for a
/// stablecoin denomination, the op that is SIMULATED must be the op that is
/// SUBMITTED — `sendUserOpInBand` batches `token.transfer(recipient, amount)`
/// (`safe-transaction.ts:1265-1268`), which is 68 more bytes of calldata and
/// one real ERC-20 SSTORE than the native `{to: recipient, value: 1}` leg.
/// Quoting the native shape and re-denominating afterwards prices a cheaper,
/// shorter operation than the one the user signs — and, right at the 1 KiB
/// `ESTIMATION_REQUIRED_CALLDATA` line, turns a refusal into a fee.
#[test]
fn a_requested_fee_token_is_part_of_the_simulated_operation() {
    let user_call = FeeCall {
        to: NATIVE_RECIPIENT.to_owned(),
        value: "1000".to_owned(),
        data: "0x".to_owned(),
    };
    let mut sut = Sut::new();
    sut.dispatch(request_in(CHAIN, vec![user_call.clone()], Some(USDC)));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(quotes_ok());
    let expected_leg = FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: encode_erc20_transfer(USDC_RECIPIENT, 1).expect("erc20 leg"),
    };
    assert_eq!(
        ops,
        vec![Op::EstimateUserOpGas {
            chain_id: CHAIN,
            account: ACCOUNT.to_owned(),
            deployed: true,
            calls: vec![user_call, expected_leg],
        }],
        "the fee leg the submit path builds is the fee leg that gets simulated"
    );

    // …and the quote it produces is denominated in that asset, with that
    // asset's recipient, without any second round trip.
    sut.resolve(estimated());
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(USDC));
    let fee = view.fee.expect("erc20 quote");
    assert_eq!(fee.total_wei, "0");
    assert_eq!(fee.fee_recipient.as_deref(), Some(USDC_RECIPIENT));
    assert_eq!(
        fee.fee_asset,
        FeeAssetView::Erc20 {
            token: USDC.to_owned(),
            decimals: 6,
            amount: USDC_FEE_UNITS.to_string(),
            // The row's own ticker rides with the quote: a fee with no symbol
            // is drawn as the native coin on every surface.
            symbol: Some("USDC".to_owned()),
        }
    );
}

/// Invariant ⑧ on the request path: a REQUESTED fee asset the Safe cannot
/// afford is refused out loud (`FeeTokenUnavailable` → "pick a different gas
/// asset"), never silently downgraded to a native quote nobody asked for.
#[test]
fn a_requested_fee_token_the_balance_cannot_cover_is_refused() {
    let mut sut = Sut::new();
    sut.dispatch(request_in(CHAIN, vec![], Some(USDC)));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    // 2 USDC held < the 2.522745 USDC this tx costs.
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![native_row("1000000000000000000"), usdc_row("2000000")]),
    });
    sut.resolve(estimated());
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::FeeTokenUnavailable));
    assert!(view.fee.is_none(), "a doomed op is never quoted");
    assert!(!view.confirm_fee_ready);

    // The native row itself is never REFUSED this way — it is the only
    // denomination left, and `estimateTransactionFee` does not gate it
    // either: it is still quoted, so the person sees what it would cost. What
    // it is not, since issue #262, is confirmable while the balance is
    // provably under that cost (`a_native_fee_the_account_does_not_hold_…`).
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![native_row("1")]),
    });
    sut.resolve(estimated());
    let view = sut.view();
    assert!(view.failed.is_none());
    assert!(view.fee.is_some(), "quoted, never refused");
    assert!(
        !view.confirm_fee_ready,
        "1 wei cannot pay a 0.00135 ETH fee"
    );
}

/// `GasFeeCard.handleRefresh`: ignored while busy; a failed refresh keeps the
/// old quote showing (the `catch {}`).
#[test]
fn requote_is_single_flight_and_keeps_old_quote_on_failure() {
    let mut sut = quoted_native(vec![]);
    let ops = sut.dispatch(Event::Requote);
    assert_eq!(ops.len(), 3);
    assert!(sut.view().busy);
    // A second refresh while one is running is ignored.
    assert!(sut.dispatch(Event::Requote).is_empty());

    // Old TTL resolves late → dropped by the attempt guard.
    assert!(sut.resolve(Res::TtlElapsed).is_empty());
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    // The refresh fails (quotes unavailable)…
    sut.resolve(Res::InBandQuotes { quotes: None });
    let view = sut.view();
    // …but the old quote keeps showing.
    assert!(!view.busy);
    assert!(view.failed.is_none());
    assert_eq!(
        view.fee.expect("old quote").total_wei,
        NATIVE_FEE_WEI.to_string()
    );
}

/// Invariant ⑨ across requotes: the refresh re-runs the SAME transaction shape.
#[test]
fn requote_reuses_the_same_transaction_shape() {
    let call = FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: encode_erc20_transfer(NATIVE_RECIPIENT, 5).expect("encodes"),
    };
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![call.clone()]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(quotes_ok());
    let first_calls = match &ops[0] {
        Op::EstimateUserOpGas { calls, .. } => calls.clone(),
        other => panic!("expected estimate, got {other:?}"),
    };
    sut.resolve(estimated());

    sut.dispatch(Event::Requote);
    assert!(sut.resolve(Res::TtlElapsed).is_empty()); // superseded timer
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(quotes_ok());
    match &ops[0] {
        Op::EstimateUserOpGas { calls, .. } => assert_eq!(calls, &first_calls),
        other => panic!("expected estimate, got {other:?}"),
    }
}

/// The 30s TTL: the displayed quote goes stale (advisory), and a superseded
/// run's timer can never mark a newer quote stale.
#[test]
fn quote_ttl_marks_stale_and_superseded_timers_are_dropped() {
    let mut sut = quoted_native(vec![]);
    assert!(!sut.view().stale);
    assert!(sut.resolve(Res::TtlElapsed).is_empty());
    assert!(sut.view().stale, "TTL elapsed → refresh affordance");
    assert!(
        sut.view().confirm_fee_ready,
        "staleness is advisory, not a gate"
    );

    // Refresh: the new quote resets staleness and arms a NEW timer.
    sut.dispatch(Event::Requote);
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    let ops = sut.resolve(estimated());
    assert_eq!(ops, vec![Op::StartTtl { ms: 30_000 }]);
    assert!(!sut.view().stale);
    assert!(sut.resolve(Res::TtlElapsed).is_empty());
    assert!(
        sut.view().stale,
        "the second timer belongs to the new quote"
    );
}

/// `QuoteExpired` (external staleness, e.g. app resume) only applies to a
/// settled quote.
#[test]
fn external_expiry_only_applies_to_a_settled_quote() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    assert!(
        sut.dispatch(Event::QuoteExpired).is_empty(),
        "inert while gathering"
    );
    assert!(!sut.view().stale);
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    sut.dispatch(Event::QuoteExpired);
    assert!(sut.view().stale);
}

/// A result arriving in a phase that no longer expects it is inert — the
/// (stage, result) pairing, not a panic and not a state change.
#[test]
fn mispaired_result_in_wrong_phase_is_inert() {
    let mut sut = quoted_native(vec![]);
    let before = sut.view();
    // Answer the outstanding TTL request with a UserOpGas result: Quoted
    // expects no simulation — dropped.
    assert!(sut.resolve(estimated()).is_empty());
    assert_eq!(sut.view(), before);
}

// ===========================================================================
// Machine — invariants
// ===========================================================================

/// Invariant ① (first half) — a quote is valid only for the chain it was
/// calculated on (`useSendController.ts:119-121`).
#[test]
fn quote_is_only_valid_for_its_chain() {
    let mut sut = quoted_native(vec![]);
    assert!(sut.view().fee.is_some());

    let ops = sut.dispatch(Event::ChainChanged { chain_id: 10 });
    assert!(ops.is_empty(), "a chain switch alone starts nothing");
    let view = sut.view();
    assert!(
        view.fee.is_none(),
        "the old-chain estimate must not price the new form"
    );
    assert!(!view.confirm_fee_ready);
    assert!(view.options.is_empty(), "old-chain fee assets are gone too");

    // Switching back makes the still-held estimate visible again — exactly
    // the `selectedFeeEstimate` memo.
    sut.dispatch(Event::ChainChanged { chain_id: CHAIN });
    assert_eq!(sut.view().fee.expect("estimate kept").chain_id, CHAIN);
}

/// Invariant ① (second half) — a late result computed for the old chain never
/// pollutes the new form.
#[test]
fn late_old_chain_results_never_pollute_the_new_form() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![])); // 3 ops in flight for chain 1
    sut.dispatch(Event::ChainChanged { chain_id: 10 });
    let ops = sut.dispatch(request(10, vec![])); // 3 new ops for chain 10
    assert_eq!(ops.len(), 3);

    // The three chain-1 answers arrive late: all dropped, no estimate issued.
    assert!(sut.resolve(gas_ok()).is_empty());
    assert!(sut.resolve(bundler_ok()).is_empty());
    assert!(
        sut.resolve(quotes_ok()).is_empty(),
        "stale quotes must not advance the run"
    );
    assert!(sut.view().busy, "the chain-10 run is still gathering");

    // The chain-10 answers complete normally.
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(quotes_ok());
    assert_eq!(ops.len(), 1);
    sut.resolve(estimated());
    assert_eq!(sut.view().fee.expect("new quote").chain_id, 10);
}

/// Invariant ② machine-side — the settled stablecoin amount uses the
/// never-undercharge conversion (vector-pinned above).
#[test]
fn settled_stable_amount_uses_never_undercharge_conversion() {
    let mut sut = quoted_native(vec![]);
    sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_owned()),
    });
    let fee = sut.view().fee.expect("switched quote");
    // The oracle: the vector-pinned pure function on the same basis.
    let expected = calculate_in_band_fee_amount(
        TOTAL_GAS,
        NETWORK_FEE,
        &AssetPricing {
            is_native: false,
            decimals: 6,
            usd_price: Some("1".to_owned()),
            native_usd_floor_price: None,
        },
        &AssetPricing {
            is_native: true,
            decimals: 18,
            usd_price: Some("1868.70000000".to_owned()),
            native_usd_floor_price: None,
        },
    )
    .expect("priceable");
    match fee.fee_asset {
        FeeAssetView::Erc20 { amount, .. } => assert_eq!(amount, expected.to_string()),
        other => panic!("expected erc20 fee, got {other:?}"),
    }
}

/// Invariant ④ — a zero bundler quote is "cannot quote", not authority: the
/// local fallback prices the op instead (`safe-transaction.ts:2070-2076`).
#[test]
fn zero_bundler_quote_falls_back_locally() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "0".to_owned(),
            max_priority_fee_per_gas: None,
            network_fee_per_gas: Some("0".to_owned()),
            relayer_fee_per_gas: Some("0".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("locally priced quote");
    assert!(!fee.quoted, "the degenerate quote was refused");
    // Local fallback at fast tier: bundlerGasPrice = 1 gwei × 2.0 = 2 gwei;
    // relayer = calcMaxFeePerGas(1 gwei, fast) − 2 gwei = 2 gwei.
    assert_eq!(fee.network_fee_per_gas, "2000000000");
    assert_eq!(fee.relayer_fee_per_gas, "2000000000");
    assert_eq!(fee.bundler_gas_price, "2000000000");
    // The fee amount is priced off the honest local basis — never ~0.
    assert_eq!(fee.total_wei, (TOTAL_GAS * 2_000_000_000 * 3).to_string());
}

/// An absent bundler quote (method unsupported) takes the same local fallback.
#[test]
fn missing_bundler_quote_falls_back_locally() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(Res::BundlerQuote { quote: None });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("locally priced quote");
    assert!(!fee.quoted);
    assert_eq!(fee.network_fee_per_gas, "2000000000");
}

/// A failed `eth_gasPrice` read is no quote at all (founder ruling,
/// 2026-09-26). It used to fall to a fixed 5 gwei — during an RPC blip an
/// Ethereum transfer at ~0.06 gwei was quoted eighty times its price.
#[test]
fn a_failed_gas_price_read_refuses_the_quote_rather_than_guessing() {
    for eth_gas_price in [None, Some("0".to_owned())] {
        let mut sut = Sut::new();
        sut.dispatch(request(CHAIN, vec![]));
        let ops = sut.resolve(Res::GasPrice {
            eth_gas_price,
            base_fee: None,
            priority_fee: None,
        });
        assert!(
            !ops.iter()
                .any(|op| matches!(op, Op::EstimateUserOpGas { .. })),
            "nothing is simulated on a guess: {ops:?}"
        );
        // The other gathering answers land on a run that already refused.
        sut.resolve(bundler_ok());
        sut.resolve(quotes_ok());
        let view = sut.view();
        assert_eq!(view.failed, Some(FeeFailure::QuoteUnavailable));
        assert!(view.fee.is_none());
        assert!(!view.confirm_fee_ready);
    }
}

/// …and a refresh that meets the same failure keeps the last good quote on
/// screen rather than blanking it (the refresh's own `catch {}`).
#[test]
fn a_refresh_whose_gas_read_fails_keeps_the_last_good_quote() {
    let mut sut = quoted_native(vec![]);
    sut.resolve(Res::TtlElapsed);
    sut.dispatch(Event::Requote);
    sut.resolve(Res::GasPrice {
        eth_gas_price: None,
        base_fee: None,
        priority_fee: None,
    });
    let view = sut.view();
    assert_eq!(view.failed, None);
    assert_eq!(
        view.fee.expect("the last good quote").total_wei,
        NATIVE_FEE_WEI.to_string()
    );
}

/// G05 — a bundler quote more than 3× the client's own on-chain gas
/// measurement is refused, not signed: the client will not pay 3× on top of a
/// runaway or hostile relayer price.
#[test]
fn rejects_a_bundler_quote_far_above_the_chain_rate() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    // Chain price 1 gwei; the 4 gwei network fee > 3 × 1 gwei chain price → refused.
    sut.resolve(gas_ok());
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "8000000000".to_owned(),
            max_priority_fee_per_gas: None,
            network_fee_per_gas: Some("4000000000".to_owned()),
            relayer_fee_per_gas: Some("4000000000".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::GasQuoteTooHigh));
    assert!(
        sut.view().fee.is_none(),
        "no fee is priced against a rejected quote"
    );
}

/// A quote at exactly 3× the chain rate is the boundary — still accepted, and
/// priced on the quote (which is ≥ our measurement).
#[test]
fn accepts_a_bundler_quote_at_the_three_times_boundary() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok()); // 1 gwei
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "6000000000".to_owned(),
            max_priority_fee_per_gas: None,
            network_fee_per_gas: Some("3000000000".to_owned()), // exactly 3×
            relayer_fee_per_gas: Some("3000000000".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("boundary quote accepted");
    assert_eq!(fee.in_band_gas_basis, "3000000000");
    assert_eq!(fee.total_wei, (TOTAL_GAS * 3_000_000_000 * 3).to_string());
}

/// When the bundler UNDER-reports the network fee, the in-band charge anchors
/// on the client's own (larger) on-chain measurement — never underpaid on a
/// low quote, even though the quote's reported network fee is shown verbatim.
#[test]
fn a_bundler_under_report_is_floored_at_the_chain_measurement() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    // Chain price 1 gwei; the bundler reports only 0.5 gwei — below our own measurement.
    sut.resolve(gas_ok());
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "1000000000".to_owned(),
            max_priority_fee_per_gas: None,
            network_fee_per_gas: Some("500000000".to_owned()),
            relayer_fee_per_gas: Some("500000000".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("under-report priced");
    // The quote still SHOWS the reported 0.5 gwei network fee...
    assert_eq!(fee.network_fee_per_gas, "500000000");
    // ...but the in-band charge anchors on max(1 gwei chain, 0.5 gwei quote) =
    // 1 gwei, so a low quote can never make the payment underpay.
    assert_eq!(fee.in_band_gas_basis, "1000000000");
    assert_eq!(fee.total_wei, (TOTAL_GAS * 1_000_000_000 * 3).to_string());
}

/// Invariant ⑤ — an undeployed account without its public key can never build
/// the real initCode, so it must never estimate (`safe-transaction.ts:634-642`).
#[test]
fn undeployed_without_public_key_never_estimates() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::QuoteRequested {
        chain_id: CHAIN,
        account: ACCOUNT.to_owned(),
        deployed: false,
        public_key_available: false,
        tier: FeeTier::Fast,
        calls: vec![],
        fee_token: None,
        auto_fee_token: false,
    });
    assert!(ops.is_empty(), "no RPC is ever issued");
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::MissingPublicKey));
    assert!(!view.confirm_fee_ready);

    // With the key available, the undeployed account estimates normally.
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::QuoteRequested {
        chain_id: CHAIN,
        account: ACCOUNT.to_owned(),
        deployed: false,
        public_key_available: true,
        tier: FeeTier::Fast,
        calls: vec![],
        fee_token: None,
        auto_fee_token: false,
    });
    assert_eq!(ops.len(), 3);

    // Tempo is exempt: it never simulates an undeployed op (the static model
    // covers the deploy), so it needs no initCode and `estimateTempoFee`
    // quotes it without a public key. Refusing would block a new user's first
    // Tempo send on web while native quoted it.
    let mut sut = Sut::new();
    let ops = sut.dispatch(Event::QuoteRequested {
        chain_id: TEMPO_CHAIN,
        account: ACCOUNT.to_owned(),
        deployed: false,
        public_key_available: false,
        tier: FeeTier::Fast,
        calls: vec![],
        fee_token: None,
        auto_fee_token: false,
    });
    assert_eq!(
        ops.len(),
        3,
        "tempo gathers its context and prices statically"
    );
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    let view = sut.view();
    assert!(view.failed.is_none());
    assert_eq!(
        view.fee.expect("static tempo quote").total_gas,
        tempo_expected_gas(false, 2).to_string()
    );
}

/// Invariant ⑥ — leaving confirm clears an erc20 estimate (so downstream
/// reserve math never reads totalWei=0) and resets the fee asset; a native
/// estimate survives (`useSendController.ts:467-473`).
#[test]
fn leave_confirm_clears_erc20_estimate_and_resets_fee_asset() {
    let mut sut = quoted_native(vec![]);
    sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_owned()),
    });
    assert!(sut.view().fee.is_some());

    sut.dispatch(Event::LeaveConfirm);
    let view = sut.view();
    assert!(
        view.fee.is_none(),
        "an erc20 estimate (totalWei=0) must not linger"
    );
    assert_eq!(view.fee_token, None, "next entry re-quotes in native");
    assert!(!view.confirm_fee_ready);

    // A native estimate survives leaving confirm, exactly as today.
    let mut sut = quoted_native(vec![]);
    sut.dispatch(Event::LeaveConfirm);
    let view = sut.view();
    assert_eq!(
        view.fee.expect("native estimate kept").total_wei,
        NATIVE_FEE_WEI.to_string()
    );
    assert_eq!(view.fee_token, None);
}

/// Invariant ⑦ — while estimating or after a failure, confirm must stay
/// disabled (`SigningSheet.tsx:576-583`).
#[test]
fn confirm_disabled_while_estimating_or_failed() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    let view = sut.view();
    assert!(view.busy && !view.confirm_fee_ready, "gathering");
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    let view = sut.view();
    assert!(view.busy && !view.confirm_fee_ready, "estimating");
    sut.resolve(estimated());
    assert!(sut.view().confirm_fee_ready);

    // Initial-run failure surfaces and gates confirm.
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(Res::InBandQuotes { quotes: None });
    let view = sut.view();
    assert!(!view.busy);
    assert_eq!(view.failed, Some(FeeFailure::QuoteUnavailable));
    assert!(!view.confirm_fee_ready);
}

/// Invariant ⑦ (second half) — a new signing request resets the fee asset and
/// discards the previous estimate (`SigningSheet.tsx:247-249`).
#[test]
fn new_request_resets_fee_asset_and_estimate() {
    let mut sut = quoted_native(vec![]);
    sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_owned()),
    });
    assert!(sut.view().fee_token.is_some());

    let ops = sut.dispatch(request(CHAIN, vec![]));
    assert_eq!(ops.len(), 3, "fresh pipeline");
    let view = sut.view();
    assert!(view.busy);
    assert_eq!(view.fee_token, None, "back to native");
    assert!(
        view.fee.is_none(),
        "no leftover estimate prices the new request"
    );
}

/// Invariant ⑨ — the estimate simulates the REAL calldata shape: the user's
/// calls plus the in-band fee leg, never a padded stand-in
/// (`useSendController.ts:724-753`; the rough model over-charged ~8× on Arbitrum).
#[test]
fn estimate_uses_real_calldata_shape() {
    let call = FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: encode_erc20_transfer(NATIVE_RECIPIENT, 123).expect("encodes"),
    };
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![call.clone()]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(quotes_ok());
    match &ops[0] {
        Op::EstimateUserOpGas {
            chain_id,
            account,
            deployed,
            calls,
        } => {
            assert_eq!(*chain_id, CHAIN);
            assert_eq!(account, ACCOUNT);
            assert!(deployed);
            assert_eq!(calls.len(), 2, "user call + fee leg, nothing else");
            assert_eq!(calls[0], call, "the REAL call, verbatim");
            // Native fee leg: a plain 1-wei placeholder transfer to the
            // quote's recipient (`safe-transaction.ts:665-668`).
            assert_eq!(
                calls[1],
                FeeCall {
                    to: NATIVE_RECIPIENT.to_owned(),
                    value: "1".to_owned(),
                    data: "0x".to_owned(),
                }
            );
        }
        other => panic!("expected estimate, got {other:?}"),
    }
}

/// A plain transfer without calls simulates the identity-precompile dummy with
/// an ERC-20-sized payload — never the Safe itself (`safe-transaction.ts:608-624`).
#[test]
fn empty_calls_estimate_with_the_erc20_sized_dummy() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(quotes_ok());
    match &ops[0] {
        Op::EstimateUserOpGas { calls, .. } => {
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].to, "0x0000000000000000000000000000000000000004");
            assert_eq!(calls[0].value, "0");
            assert_eq!(
                calls[0].data.len(),
                2 + 68 * 2,
                "68 zero bytes — transfer-sized"
            );
        }
        other => panic!("expected estimate, got {other:?}"),
    }
}

/// A stablecoin fee leg is a token `transfer`, not a value transfer.
#[test]
fn erc20_fee_leg_is_a_token_transfer() {
    let mut sut = quoted_native(vec![]);
    // Force the slow requote path with USDC selected: unknown-asset select
    // falls back to the pipeline with fee_token set.
    sut.dispatch(Event::SelectFeeAsset {
        token: Some(USDC.to_owned()),
    }); // local switch first
    sut.dispatch(Event::Requote);
    assert!(sut.resolve(Res::TtlElapsed).is_empty());
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(quotes_ok());
    match &ops[0] {
        Op::EstimateUserOpGas { calls, .. } => {
            let leg = &calls[1];
            assert_eq!(leg.to, USDC, "the token contract is the target");
            assert_eq!(leg.value, "0");
            assert_eq!(
                leg.data,
                encode_erc20_transfer(USDC_RECIPIENT, 1).expect("encodes"),
                "1-unit placeholder transfer to the quote's recipient"
            );
        }
        other => panic!("expected estimate, got {other:?}"),
    }
}

/// A failed simulation on a small op keeps the static fallback, with the L2
/// data-fee adders (`safe-transaction.ts:715-730`).
#[test]
fn failed_simulation_uses_static_fallback_with_l2_adders() {
    // Mainnet: 300k + 200k + 100k = 600k.
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::SimulationFailed,
    });
    assert_eq!(sut.view().fee.expect("static quote").total_gas, "600000");

    // Arbitrum: + 600k rollup data-fee adder.
    let mut sut = Sut::new();
    sut.dispatch(request(42_161, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::SimulationFailed,
    });
    assert_eq!(sut.view().fee.expect("static quote").total_gas, "1200000");

    // OP-stack (Base): + 150k.
    let mut sut = Sut::new();
    sut.dispatch(request(8_453, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::SimulationFailed,
    });
    assert_eq!(sut.view().fee.expect("static quote").total_gas, "750000");
}

/// For a large/complex op the static fallback would mislead and the submit
/// would refuse it anyway — the failure surfaces (`safe-transaction.ts:703-713`).
#[test]
fn failed_simulation_of_large_calldata_surfaces() {
    let big_call = FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: format!("0x{}", "ab".repeat(1_200)),
    };
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![big_call]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::SimulationFailed,
    });
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::EstimateFailed));
    assert!(view.fee.is_none());
}

/// The shell reporting missing account context (nonce/initCode) is never
/// papered over with a static number (`safe-transaction.ts:585-588`).
#[test]
fn missing_account_context_fails_the_estimate() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::ContextUnavailable,
    });
    assert_eq!(sut.view().failed, Some(FeeFailure::EstimateFailed));
}

// ===========================================================================
// Machine — Tempo
// ===========================================================================

/// Tempo gathers a different context (no bundler quote, no tip, a settlement
/// recipient) and prices the stablecoin reimbursement statically for transfers
/// (`safe-transaction.ts:450-546`).
#[test]
fn tempo_transfer_prices_the_stablecoin_reimbursement_statically() {
    let mut sut = Sut::new();
    let ops = sut.dispatch(request(
        TEMPO_CHAIN,
        vec![FeeCall {
            to: NATIVE_RECIPIENT.to_owned(),
            value: "1000".to_owned(),
            data: "0x".to_owned(),
        }],
    ));
    assert_eq!(
        ops,
        vec![
            // attodollar gas makes eth_maxPriorityFeePerGas meaningless.
            Op::FetchGasPrice {
                chain_id: TEMPO_CHAIN,
                want_tip: false
            },
            Op::FetchFeeRecipient {
                chain_id: TEMPO_CHAIN,
                account: ACCOUNT.to_owned()
            },
            Op::FetchInBandQuotes {
                chain_id: TEMPO_CHAIN,
                account: ACCOUNT.to_owned()
            },
        ]
    );
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    // A transfer needs no simulation: pricing settles as soon as the rows land.
    let ops = sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    assert_eq!(ops, vec![Op::StartTtl { ms: 30_000 }]);

    let view = sut.view();
    let fee = view.fee.expect("tempo quote");
    // 1 transfer + 1 reimbursement = 2 sub-calls → 250k + 2×110k = 470k.
    let expected_gas = tempo_expected_gas(true, 2);
    assert_eq!(fee.total_gas, expected_gas.to_string());
    let reimbursement = tempo_reimbursement(expected_gas, TEMPO_BASE_FEE_ATTO, 6);
    assert_eq!(
        fee.fee_asset,
        FeeAssetView::Erc20 {
            token: TEMPO_DEFAULT_FEE_TOKEN.to_owned(),
            decimals: 6,
            amount: reimbursement.to_string(),
            symbol: Some("pathUSD".to_owned()),
        }
    );
    // totalWei carries the reimbursement scaled to attodollars for the USD
    // display path (`safe-transaction.ts:443-448`).
    assert_eq!(fee.total_wei, (reimbursement * 10u128.pow(12)).to_string());
    assert!(!fee.quoted);
    assert_eq!(fee.fee_recipient.as_deref(), Some(COLLECTOR));
    // The native coin cannot pay gas on Tempo — no native picker row exists,
    // and the pathUSD row prices at the reimbursement.
    assert_eq!(view.options.len(), 1);
    assert_eq!(
        view.options[0].amount.as_deref(),
        Some(&*reimbursement.to_string())
    );
}

/// A deployed Tempo contract call refines off the bundler's estimate of the
/// REAL batch (call + two placeholder reimbursement legs — the split case)
/// (`safe-transaction.ts:482-514`).
#[test]
fn tempo_contract_call_refines_off_the_real_estimate() {
    let call = FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: "0x4e71d92d".to_owned(), // claim() — tiny calldata, heavy call
    };
    let mut sut = Sut::new();
    sut.dispatch(request(TEMPO_CHAIN, vec![call.clone()]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    let ops = sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    match &ops[0] {
        Op::EstimateUserOpGas { calls, .. } => {
            assert_eq!(
                calls.len(),
                3,
                "the call + two placeholder legs (split case)"
            );
            assert_eq!(calls[0], call);
            let leg_data = encode_erc20_transfer(ACCOUNT, 1).expect("encodes");
            for leg in &calls[1..] {
                assert_eq!(leg.to, TEMPO_DEFAULT_FEE_TOKEN);
                assert_eq!(leg.value, "0");
                assert_eq!(leg.data, leg_data);
            }
        }
        other => panic!("expected estimate, got {other:?}"),
    }

    // The refine takes max(static, un-padded sum) (`safe-transaction.ts:514`).
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::Estimated {
            verification_gas_limit: "2000000".to_owned(),
            call_gas_limit: "1500000".to_owned(),
            pre_verification_gas: "100000".to_owned(),
        },
    });
    let fee = sut.view().fee.expect("refined quote");
    assert_eq!(
        fee.total_gas, "3600000",
        "un-padded sum beats the static model"
    );
    let reimbursement = tempo_reimbursement(3_600_000, TEMPO_BASE_FEE_ATTO, 6);
    match fee.fee_asset {
        FeeAssetView::Erc20 { amount, .. } => assert_eq!(amount, reimbursement.to_string()),
        other => panic!("expected erc20 fee, got {other:?}"),
    }
}

/// A Tempo contract call that cannot be estimated surfaces — a transfer-sized
/// fee would mislead and then be rejected (`safe-transaction.ts:515-520`).
#[test]
fn tempo_unestimable_contract_call_surfaces() {
    let call = FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: "0x4e71d92d".to_owned(),
    };
    let mut sut = Sut::new();
    sut.dispatch(request(TEMPO_CHAIN, vec![call]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::SimulationFailed,
    });
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::EstimateFailed));
    assert!(view.fee.is_none());
}

/// An undeployed Tempo sender keeps the static model — the deploy cost
/// dominates (`safe-transaction.ts:481`).
#[test]
fn tempo_undeployed_contract_call_keeps_the_static_model() {
    let call = FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: "0x4e71d92d".to_owned(),
    };
    let mut sut = Sut::new();
    sut.dispatch(Event::QuoteRequested {
        chain_id: TEMPO_CHAIN,
        account: ACCOUNT.to_owned(),
        deployed: false,
        public_key_available: true,
        tier: FeeTier::Fast,
        calls: vec![call],
        fee_token: None,
        auto_fee_token: false,
    });
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    // No simulation is requested: the quote settles directly.
    let ops = sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    assert_eq!(ops, vec![Op::StartTtl { ms: 30_000 }]);
    // 1 call + 2 reimbursement legs (contract call) = 3 sub-calls, deploy fixed cost.
    let expected = tempo_expected_gas(false, 3);
    assert_eq!(
        sut.view().fee.expect("static tempo quote").total_gas,
        expected.to_string()
    );
}

/// A Tempo fee-asset switch NEVER takes the generic local-recompute path.
///
/// That path exists for in-band chains, where `total_wei = 0`, the fee rides
/// in the ERC-20 leg, and the recipient is the picked row's. Applied to a
/// Tempo estimate it destroys all three facts at once: `total_wei` stops being
/// the attodollar reimbursement the USD display divides by 1e18, the pathUSD
/// symbol is lost, and — the one that costs a send — `fee_recipient` becomes
/// the in-band row's recipient instead of the bundler's `settlementRecipient`.
/// `sendUserOpTempo` compares that address byte for byte and throws "The gas
/// quote has expired" (`safe-transaction.ts:1196-1198`), so every Tempo send
/// whose fee chip had been touched died at submit, and Refresh regenerated the
/// same wrong address forever. Tempo re-prices through `advance_tempo`.
#[test]
fn tempo_fee_asset_switch_reprices_through_the_tempo_model() {
    let other = "0x20c0000000000000000000000000000000000001";
    let mut sut = Sut::new();
    sut.dispatch(request(TEMPO_CHAIN, vec![]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![
            pathusd_row("5000000"),
            FeeAssetQuote {
                recipient: USDC_RECIPIENT.to_owned(),
                fee_token: Some(other.to_owned()),
                symbol: "othUSD".to_owned(),
                ..pathusd_row("5000000")
            },
        ]),
    });
    assert!(sut.view().fee.is_some(), "tempo quote settled");

    // The switch runs the WHOLE Tempo pipeline again rather than patching the
    // settled estimate in place.
    let ops = sut.dispatch(Event::SelectFeeAsset {
        token: Some(other.to_owned()),
    });
    assert_eq!(
        ops,
        vec![
            Op::FetchGasPrice {
                chain_id: TEMPO_CHAIN,
                want_tip: false
            },
            Op::FetchFeeRecipient {
                chain_id: TEMPO_CHAIN,
                account: ACCOUNT.to_owned()
            },
            Op::FetchInBandQuotes {
                chain_id: TEMPO_CHAIN,
                account: ACCOUNT.to_owned()
            },
        ],
        "a Tempo denomination change is a re-quote, never an in-place patch"
    );
    assert!(
        sut.view().busy,
        "invariant ⑦ — confirm is disabled while re-pricing"
    );

    assert!(
        sut.resolve(Res::TtlElapsed).is_empty(),
        "superseded timer dropped"
    );
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![
            pathusd_row("5000000"),
            FeeAssetQuote {
                recipient: USDC_RECIPIENT.to_owned(),
                fee_token: Some(other.to_owned()),
                symbol: "othUSD".to_owned(),
                ..pathusd_row("5000000")
            },
        ]),
    });

    let expected_gas = tempo_expected_gas(true, 2);
    let reimbursement = tempo_reimbursement(expected_gas, TEMPO_BASE_FEE_ATTO, 6);
    let fee = sut.view().fee.expect("re-priced tempo quote");
    // The settlement recipient survives — the address sendUserOpTempo checks.
    assert_eq!(fee.fee_recipient.as_deref(), Some(COLLECTOR));
    // totalWei stays the attodollar reimbursement, never 0.
    assert_eq!(fee.total_wei, (reimbursement * 10u128.pow(12)).to_string());
    assert_eq!(
        fee.fee_asset,
        FeeAssetView::Erc20 {
            token: other.to_owned(),
            decimals: 6,
            amount: reimbursement.to_string(),
            // Not the default TIP-20 → no pathUSD label, as `estimateTempoFee`.
            symbol: None,
        }
    );
}

/// A malformed settlement recipient never becomes part of the signed fee
/// instruction (`safe-transaction.ts:546`).
#[test]
fn tempo_malformed_recipient_is_not_signed() {
    let mut sut = Sut::new();
    sut.dispatch(request(TEMPO_CHAIN, vec![]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some("not-an-address".to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    let fee = sut.view().fee.expect("quote without recipient");
    assert_eq!(fee.fee_recipient, None);
}

// ---------------------------------------------------------------------------
// What a speed actually buys — the effective gas price (issue 684)
// ---------------------------------------------------------------------------

/// The three tiers on Polygon, as three MINED receipts reported them.
///
/// `base + signed tip` per tier, to the wei. These are not derived from the
/// formula under test — they are what the chain charged, read off the
/// receipts — so the formula has to reproduce them rather than agree with
/// itself.
#[test]
fn the_three_polygon_receipts_reproduce_their_effective_gas_price_to_the_wei() {
    // (base_fee, signed tip, the receipt's own "Gas Price (effective)")
    let receipts: [(u128, u128, u128); 3] = [
        (243_164_376_946, 27_000_100_203, 270_164_477_149), // slow
        (249_964_783_233, 32_500_000_000, 282_464_783_233), // standard
        (247_589_616_979, 52_000_200_406, 299_589_817_385), // fast
    ];
    for (base_fee, tip, charged) in receipts {
        // The cap is `m × base + tip` and is never what the chain charges —
        // `fast`'s receipt read `Max: 805.065222658 Gwei` beside an effective
        // 299.589817385. Whichever multiple is in force, the answer is the same.
        for multiple in [3, 2, 1] {
            let cap = base_fee * multiple + tip;
            assert_eq!(
                effective_gas_price(base_fee, cap, tip),
                charged,
                "base {base_fee} tip {tip} at a {multiple}× cap"
            );
        }
    }
}

/// A cap too low to deliver the whole tip is the ceiling, not the sum.
///
/// The relay asserts this can never happen (`OuterFee::delivers_full_tip_at`),
/// which is exactly why it is derived rather than assumed: the wallet talks to
/// whatever answers, and a cap below `base + tip` would silently truncate the
/// priority somebody paid for.
#[test]
fn a_cap_that_cannot_deliver_the_whole_tip_is_the_price() {
    let base = 100_u128;
    // Room for only 5 of the 30 wei tip.
    assert_eq!(effective_gas_price(base, 105, 30), 105);
    // Exactly enough is the sum, not the cap — the boundary is not off by one.
    assert_eq!(effective_gas_price(base, 130, 30), 130);
    // A cap that cannot even pay the base fee is not turned into one that can.
    assert_eq!(effective_gas_price(base, 40, 30), 40);
    // No tip at all: the base fee is the whole price, on any cap.
    assert_eq!(effective_gas_price(base, 300, 0), 100);
}

/// The quote's own numbers reach the view, unrounded.
#[test]
fn a_quote_that_reports_its_tip_publishes_the_gas_price_that_speed_buys() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    // The measured Polygon `fast` market: base 247.589616979 gwei, tip
    // 52.000200406 gwei, and the relay's row for that tier around it.
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some("299589817385".to_owned()),
        base_fee: Some("247589616979".to_owned()),
        priority_fee: Some("52000200406".to_owned()),
    });
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            // 3 × base + tip — the cap, which the chain never charges.
            max_fee_per_gas: "794769051343".to_owned(),
            max_priority_fee_per_gas: Some("52000200406".to_owned()),
            network_fee_per_gas: Some("445661310562".to_owned()),
            relayer_fee_per_gas: Some("297107540375".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("quote settled");
    assert_eq!(
        fee.effective_gas_price.as_deref(),
        Some("299589817385"),
        "base + signed tip, not the 794.77 gwei cap"
    );
}

/// A row that reports no tip yields NOTHING, never a zero.
///
/// A zero would read as "this speed buys a free chain" and, drawn beside its
/// two neighbours, would claim all three tiers are equal — the exact false
/// statement this figure exists to test.
#[test]
fn a_quote_with_no_reported_tip_publishes_no_gas_price_at_all() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok()); // reports no maxPriorityFeePerGas
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("quote settled");
    assert_eq!(fee.effective_gas_price, None);
    // Nor half a range: the cap alone is not a gas price anybody bids (issue 685).
    assert_eq!(fee.max_gas_price, None);
}

/// A MEASURED zero is published as zero, not treated like a missing factor.
///
/// A third-party bundler that answers `maxPriorityFeePerGas: 0` on a chain
/// whose block reports `baseFeePerGas: 0` really is quoting a zero gas price,
/// and the picker then says "0 wei" on every row. That is the chain's own
/// statement, not a fabrication — the rule is that an ABSENT factor yields no
/// number, never that an inconvenient one is hidden. Unreachable through the
/// Vela relay: a zero market tip there makes the cap `m × 0 + 0 = 0`, which
/// `accept_bundler_quote` already refuses as degenerate. Pinned so the next
/// reader does not have to re-derive either half.
#[test]
fn a_measured_zero_gas_price_is_published_as_zero() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some("1000000000".to_owned()),
        base_fee: Some("0".to_owned()),
        priority_fee: Some("0".to_owned()),
    });
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "2000000000".to_owned(),
            max_priority_fee_per_gas: Some("0".to_owned()),
            network_fee_per_gas: Some("1000000000".to_owned()),
            relayer_fee_per_gas: Some("1000000000".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("quote settled");
    assert_eq!(fee.effective_gas_price.as_deref(), Some("0"));
    // And its range tops out at the relay's own cap — a measured ceiling, so
    // it is published beside the measured zero (issue 685).
    assert_eq!(fee.max_gas_price.as_deref(), Some("2000000000"));
}

/// A base fee the chain never reported is not replaced by the 5-gwei guess.
///
/// `resolve_gas_price` is allowed to guess, because a gas price that is too
/// high only over-funds a quote. A number a person READS has no such safe
/// direction: 5 gwei stated as Gnosis's gas price is wrong by eight orders of
/// magnitude.
#[test]
fn an_unreported_base_fee_publishes_no_gas_price_rather_than_the_fallback() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some("1000000000".to_owned()),
        base_fee: None,
        priority_fee: Some("500000000".to_owned()),
    });
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "2000000000".to_owned(),
            max_priority_fee_per_gas: Some("500000000".to_owned()),
            network_fee_per_gas: Some("1000000000".to_owned()),
            relayer_fee_per_gas: Some("1000000000".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("quote settled");
    assert_eq!(fee.effective_gas_price, None);
    // Nor half a range: the cap alone is not a gas price anybody bids (issue 685).
    assert_eq!(fee.max_gas_price, None);
}

/// Tempo has no priority fee anywhere in its model, so it states none.
#[test]
fn tempo_publishes_no_gas_price_because_it_has_no_tip_to_report() {
    let mut sut = Sut::new();
    sut.dispatch(request(TEMPO_CHAIN, vec![]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    let fee = sut.view().fee.expect("tempo quote");
    assert_eq!(fee.effective_gas_price, None);
    assert_eq!(fee.max_gas_price, None);
}

/// A shell that has not learned to send the tip still parses.
///
/// iOS, Android and desktop build this row as JSON. The field is additive, so
/// an older one omits it and must go on getting exactly the answer it got
/// before — no gas price, and everything else unchanged.
#[test]
fn a_quote_row_without_the_tip_field_still_deserializes() {
    let row: FeeBundlerQuote = serde_json::from_str(
        r#"{"max_fee_per_gas":"2000000000","network_fee_per_gas":"1000000000","relayer_fee_per_gas":"1000000000"}"#,
    )
    .expect("an older shell's row");
    assert_eq!(row.max_priority_fee_per_gas, None);
}

// ---------------------------------------------------------------------------
// The gas price as a range — what a speed bids now, how high it will go
// (issue 685)
// ---------------------------------------------------------------------------

/// The same three mined Polygon receipts, read as ranges, to the wei.
///
/// Low end `base + tip` — what the receipt charged. High end the receipt's own
/// `Max:` — the cap the relay signed. Neither is derived from the formula
/// under test; both are what the chain reported.
#[test]
fn the_three_polygon_receipts_reproduce_their_gas_price_range_to_the_wei() {
    // (base_fee, signed tip, maxFeePerGas, the range the picker must show)
    let receipts: [(u128, u128, u128, (u128, u128)); 3] = [
        // slow: 270.164477149 ~ 390.302745042 gwei
        (
            243_164_376_946,
            27_000_100_203,
            390_302_745_042,
            (270_164_477_149, 390_302_745_042),
        ),
        // standard: 282.464783233 ~ 527.288291674 gwei
        (
            249_964_783_233,
            32_500_000_000,
            527_288_291_674,
            (282_464_783_233, 527_288_291_674),
        ),
        // fast: 299.589817385 ~ 805.065222658 gwei
        (
            247_589_616_979,
            52_000_200_406,
            805_065_222_658,
            (299_589_817_385, 805_065_222_658),
        ),
    ];
    for (base_fee, tip, cap, range) in receipts {
        assert_eq!(gas_price_range(base_fee, cap, tip), range, "cap {cap}");
    }
}

/// The high end is the CAP, never `base + cap + tip`.
///
/// The owner's first proposal. The cap already contains the tip (`m × base +
/// tip`), so the sum double-counts it and lands above the cap — above anything
/// EIP-1559 can ever charge, since the chain takes `min(cap, base + tip)`. On
/// the receipts it would have read 660.47 / 809.75 / 1104.66 gwei: every one of
/// them an impossible price for its own transaction.
#[test]
fn the_top_of_the_range_is_the_cap_and_never_base_plus_cap_plus_tip() {
    let receipts: [(u128, u128, u128, u128); 3] = [
        // (base_fee, tip, cap, the double-counted figure that must NOT appear)
        (
            243_164_376_946,
            27_000_100_203,
            390_302_745_042,
            660_467_222_191,
        ),
        (
            249_964_783_233,
            32_500_000_000,
            527_288_291_674,
            809_753_074_907,
        ),
        (
            247_589_616_979,
            52_000_200_406,
            805_065_222_658,
            1_104_655_040_043,
        ),
    ];
    for (base_fee, tip, cap, double_counted) in receipts {
        assert_eq!(
            base_fee + cap + tip,
            double_counted,
            "the fixture's own sum"
        );
        let (low, high) = gas_price_range(base_fee, cap, tip);
        assert_eq!(high, cap);
        assert_ne!(high, double_counted);
        // Nothing the chain can charge is above the top of the range.
        assert!(low <= high);
    }
}

/// When both ends are one number, the range IS one number.
///
/// A cap that binds (too low to deliver the whole tip), and a chain with no
/// base fee (BSC: the cap is `m × 0 + tip = tip`), both give equal ends. The
/// core publishes them equal rather than inventing a spread; the shell then
/// draws the single figure.
#[test]
fn a_range_whose_ends_meet_is_published_with_equal_ends() {
    // The cap binds: room for only 5 of the 30 wei tip.
    assert_eq!(gas_price_range(100, 105, 30), (105, 105));
    // No base fee: whatever the tier's multiple, `m × 0 + tip` is the tip.
    let tip = 50_000_000_u128;
    assert_eq!(gas_price_range(0, tip, tip), (tip, tip));
}

/// The view carries both ends of a reported quote, unrounded.
#[test]
fn a_quote_that_reports_its_tip_publishes_both_ends_of_the_range() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    // The Polygon `fast` receipt's market and the cap it was signed with.
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some("299589817385".to_owned()),
        base_fee: Some("247589616979".to_owned()),
        priority_fee: Some("52000200406".to_owned()),
    });
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "805065222658".to_owned(),
            max_priority_fee_per_gas: Some("52000200406".to_owned()),
            network_fee_per_gas: Some("445661310562".to_owned()),
            relayer_fee_per_gas: Some("297107540375".to_owned()),
        }),
    });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("quote settled");
    assert_eq!(fee.effective_gas_price.as_deref(), Some("299589817385"));
    assert_eq!(fee.max_gas_price.as_deref(), Some("805065222658"));
}

/// No bundler quote means the local fallback, and the local fallback's cap is
/// our own guess — so there is no range, not a measured bid under a made-up
/// ceiling.
#[test]
fn a_locally_estimated_fee_publishes_no_range() {
    let mut sut = Sut::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some("1000000000".to_owned()),
        base_fee: Some("900000000".to_owned()),
        priority_fee: Some("100000000".to_owned()),
    });
    sut.resolve(Res::BundlerQuote { quote: None });
    sut.resolve(quotes_ok());
    sut.resolve(estimated());
    let fee = sut.view().fee.expect("quote settled");
    assert_eq!(fee.effective_gas_price, None);
    assert_eq!(fee.max_gas_price, None);
}

// ===========================================================================
// Machine — the inner calls' measured gas floor (issue #262 follow-up)
// ===========================================================================

/// A contract call (not a plain transfer): `claim()`-shaped calldata.
fn contract_call() -> FeeCall {
    FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: "0x4e71d92d".to_owned(),
    }
}

fn erc20_transfer_call() -> FeeCall {
    FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: encode_erc20_transfer(NATIVE_RECIPIENT, 1).expect("transfer"),
    }
}

fn request_undeployed(calls: Vec<FeeCall>) -> Event {
    Event::QuoteRequested {
        chain_id: CHAIN,
        account: ACCOUNT.to_owned(),
        deployed: false,
        public_key_available: true,
        tier: FeeTier::Fast,
        calls,
        fee_token: None,
        auto_fee_token: false,
    }
}

/// The relay's "no code at the sender" figure for an undeployed Safe: 21k
/// plus calldata, whatever the call does.
fn relay_trivial_estimate() -> Res {
    Res::UserOpGas {
        outcome: FeeGasOutcome::Estimated {
            verification_gas_limit: "100000".to_owned(),
            call_gas_limit: "118000".to_owned(),
            pre_verification_gas: "40000".to_owned(),
        },
    }
}

fn measured(gas: &[Option<&str>]) -> Res {
    Res::InnerCallsMeasured {
        gas: gas.iter().map(|g| g.map(str::to_owned)).collect(),
    }
}

/// Dispatch + the three context reads; returns the estimating phase's ops.
fn to_estimating(sut: &mut Sut, event: Event) -> Vec<Op> {
    sut.dispatch(event);
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok())
}

/// The spec-062 live figure: a 4,308,125-gas registry call.
const MEASURED_REGISTRY_CALL: u128 = 4_308_125;

/// THE BUG: an undeployed Safe's first contract call was priced on the relay's
/// trivial `callGasLimit`, the submit raised it to the measured floor, and the
/// signed fee could not cover the op the relay then had to bundle. The quote
/// now measures what the submit measures and prices the raised limit.
#[test]
fn undeployed_contract_call_is_priced_on_the_measured_floor() {
    let mut sut = Sut::new();
    let ops = to_estimating(&mut sut, request_undeployed(vec![contract_call()]));
    assert_eq!(ops.len(), 2, "simulation and measurement run side by side");
    assert!(matches!(
        ops[0],
        Op::EstimateUserOpGas {
            deployed: false,
            ..
        }
    ));
    assert_eq!(
        ops[1],
        Op::MeasureInnerCalls {
            chain_id: CHAIN,
            from: ACCOUNT.to_owned(),
            calls: vec![contract_call()],
        }
    );
    assert!(
        sut.resolve(relay_trivial_estimate()).is_empty(),
        "never priced before the measurement answers"
    );
    assert!(sut.view().busy);
    let ops = sut.resolve(measured(&[Some(&MEASURED_REGISTRY_CALL.to_string())]));
    assert_eq!(ops, vec![Op::StartTtl { ms: 30_000 }]);

    // The floor the submit raises to (`user_op_raise_call_gas`): 5,495,156.
    let floor =
        vela_core::user_op::inner_calls_gas_floor(&[MEASURED_REGISTRY_CALL], 1).expect("a floor");
    assert_eq!(floor, 5_495_156);
    // Displayed = signed: the submit's padded relay figure is under the floor,
    // so the op carries exactly `floor` — the quote must price exactly that.
    let submit_cgl = vela_core::user_op::pad_gas_estimate(
        vela_core::user_op::GasEstimate {
            verification_gas_limit: 100_000,
            call_gas_limit: 118_000,
            pre_verification_gas: 40_000,
        },
        2_000_000,
        200_000,
    )
    .call_gas_limit
    .max(floor);
    assert_eq!(submit_cgl, floor);

    let fee = sut.view().fee.expect("quoted");
    // 2,000,000 (undeployed vgl floor) + 5,495,156 + (40,000 + 10,000).
    let total = 2_000_000 + submit_cgl + 50_000;
    assert_eq!(fee.total_gas, total.to_string());
    assert_eq!(fee.total_wei, (total * NETWORK_FEE * 3).to_string());
}

/// The measurement may answer first; the result is the same.
#[test]
fn the_measurement_may_answer_before_the_simulation() {
    let mut sut = Sut::new();
    to_estimating(&mut sut, request_undeployed(vec![contract_call()]));
    let ops = sut.resolve_matching(
        |op| matches!(op, Op::MeasureInnerCalls { .. }),
        measured(&[Some(&MEASURED_REGISTRY_CALL.to_string())]),
    );
    assert!(ops.is_empty(), "still waiting for the simulation");
    sut.resolve(relay_trivial_estimate());
    assert_eq!(
        sut.view().fee.expect("quoted").total_gas,
        (2_000_000 + 5_495_156 + 50_000u128).to_string()
    );
}

/// A call nobody could measure leaves the relay's figure — the submit's own
/// behaviour, which drops its floor the same way.
#[test]
fn an_unmeasurable_call_keeps_the_relay_figure() {
    // Relay: 118k × 1.5 = 177k.
    let relay_total = (2_000_000 + 177_000 + 50_000u128).to_string();

    let mut sut = Sut::new();
    to_estimating(&mut sut, request_undeployed(vec![contract_call()]));
    sut.resolve(relay_trivial_estimate());
    sut.resolve(measured(&[None]));
    assert_eq!(sut.view().fee.expect("quoted").total_gas, relay_total);

    // One of two unmeasured is no floor at all, not half a floor.
    let mut sut = Sut::new();
    to_estimating(
        &mut sut,
        request_undeployed(vec![contract_call(), contract_call()]),
    );
    sut.resolve(relay_trivial_estimate());
    sut.resolve(measured(&[Some("4308125"), None]));
    assert_eq!(sut.view().fee.expect("quoted").total_gas, relay_total);

    // An answer that does not line up with the question is not a floor.
    let mut sut = Sut::new();
    to_estimating(&mut sut, request_undeployed(vec![contract_call()]));
    sut.resolve(relay_trivial_estimate());
    sut.resolve(measured(&[Some("4308125"), Some("4308125")]));
    assert_eq!(sut.view().fee.expect("quoted").total_gas, relay_total);
}

/// The web submit measures every contract call, deployed or not, so the
/// quote does too: a small call on a DEPLOYED Safe whose floor (×1.25 + 60k +
/// 50k/call) out-grows the relay's padded figure is priced on the floor.
#[test]
fn a_deployed_safe_contract_call_is_measured_too() {
    let mut sut = Sut::new();
    let ops = to_estimating(&mut sut, request(CHAIN, vec![contract_call()]));
    assert_eq!(ops.len(), 2);
    assert!(matches!(ops[1], Op::MeasureInnerCalls { .. }));
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::Estimated {
            verification_gas_limit: "100000".to_owned(),
            call_gas_limit: "80000".to_owned(),
            pre_verification_gas: "40000".to_owned(),
        },
    });
    sut.resolve(measured(&[Some("46000")]));
    // Floor 46,000 × 1.25 + 60,000 + 50,000 = 167,500 > 80,000 × 1.5.
    // 300,000 (deployed vgl floor) + 167,500 + 50,000.
    assert_eq!(sut.view().fee.expect("quoted").total_gas, "517500");

    // A floor BELOW the relay's padded figure changes nothing.
    let mut sut = Sut::new();
    to_estimating(&mut sut, request(CHAIN, vec![contract_call()]));
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::Estimated {
            verification_gas_limit: "100000".to_owned(),
            call_gas_limit: "400000".to_owned(),
            pre_verification_gas: "40000".to_owned(),
        },
    });
    sut.resolve(measured(&[Some("46000")]));
    assert_eq!(
        sut.view().fee.expect("quoted").total_gas,
        (300_000 + 600_000 + 50_000u128).to_string()
    );
}

/// Plain transfers are never measured — no operation is asked for — and a
/// mixed batch measures only its contract calls while every inner call counts
/// toward the per-call frame allowance.
#[test]
fn only_contract_calls_are_measured() {
    let native_send = FeeCall {
        to: NATIVE_RECIPIENT.to_owned(),
        value: "1".to_owned(),
        data: "0x".to_owned(),
    };
    let mut sut = Sut::new();
    let ops = to_estimating(
        &mut sut,
        request_undeployed(vec![native_send.clone(), erc20_transfer_call()]),
    );
    assert_eq!(ops.len(), 1, "transfers only → the simulation alone");
    sut.resolve(relay_trivial_estimate());
    assert!(sut.view().fee.is_some(), "priced without a measurement");

    let mut sut = Sut::new();
    let ops = to_estimating(
        &mut sut,
        request_undeployed(vec![native_send, contract_call()]),
    );
    assert_eq!(
        ops[1],
        Op::MeasureInnerCalls {
            chain_id: CHAIN,
            from: ACCOUNT.to_owned(),
            calls: vec![contract_call()],
        }
    );
    sut.resolve(relay_trivial_estimate());
    sut.resolve(measured(&[Some("4308125")]));
    // 5,385,156 + 60,000 + 50,000 × 2 inner calls = 5,545,156.
    assert_eq!(
        sut.view().fee.expect("quoted").total_gas,
        (2_000_000 + 5_545_156 + 50_000u128).to_string()
    );
}

/// A small op the relay could not simulate keeps the static fallback, raised
/// to the measured floor when that is higher.
#[test]
fn the_static_fallback_is_raised_to_the_measured_floor() {
    let mut sut = Sut::new();
    to_estimating(&mut sut, request_undeployed(vec![contract_call()]));
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::SimulationFailed,
    });
    assert!(sut.view().busy, "the floor still matters to the fallback");
    sut.resolve(measured(&[Some("4308125")]));
    // 2,000,000 + max(200,000, 5,495,156) + 100,000.
    assert_eq!(
        sut.view().fee.expect("static quote").total_gas,
        (2_000_000 + 5_495_156 + 100_000u128).to_string()
    );
}

/// Missing account context fails at once — it never waits on a measurement
/// that could not change the answer.
#[test]
fn a_context_failure_does_not_wait_for_the_measurement() {
    let mut sut = Sut::new();
    to_estimating(&mut sut, request_undeployed(vec![contract_call()]));
    sut.resolve(Res::UserOpGas {
        outcome: FeeGasOutcome::ContextUnavailable,
    });
    assert_eq!(sut.view().failed, Some(FeeFailure::EstimateFailed));
    // The late measurement is a no-op.
    assert!(sut.resolve(measured(&[Some("4308125")])).is_empty());
    assert_eq!(sut.view().failed, Some(FeeFailure::EstimateFailed));
}

/// Tempo prices the un-padded simulation, never `callGasLimit`: nothing is
/// measured there.
#[test]
fn tempo_never_measures() {
    let mut sut = Sut::new();
    sut.dispatch(request(TEMPO_CHAIN, vec![contract_call()]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    let ops = sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    assert_eq!(ops.len(), 1);
    assert!(matches!(ops[0], Op::EstimateUserOpGas { .. }));
}

// -- the fee-signal cache (issue 212): what a shell may hold -----------------

mod fee_signal_cache {
    use vela_core::app::fee_policy::{
        bundler_quote_cacheable, gas_signals_cacheable, FEE_SIGNALS_CACHE_TTL_MS,
    };

    #[test]
    fn the_window_is_fifteen_seconds() {
        assert_eq!(FEE_SIGNALS_CACHE_TTL_MS, 15_000);
    }

    /// Only a complete, real reading is held — every leg that was asked for.
    #[test]
    fn a_complete_reading_is_held() {
        assert!(gas_signals_cacheable(
            Some("1000000000"),
            true,
            true,
            Some("27773221947")
        ));
        // A block that answered without `baseFeePerGas` is a real pre-London
        // reading; the tip was not asked for.
        assert!(gas_signals_cacheable(Some("50000000"), true, false, None));
    }

    /// A zero or missing gas price would pin the 5 gwei default — 100× on BSC.
    #[test]
    fn no_price_or_a_zero_price_is_never_held() {
        assert!(!gas_signals_cacheable(None, true, false, None));
        assert!(!gas_signals_cacheable(Some("0"), true, false, None));
        assert!(!gas_signals_cacheable(Some(""), true, false, None));
        // Not decimal wei: a hex quantity the shell forgot to convert is no
        // measurement at all, rather than a number read in the wrong base.
        assert!(!gas_signals_cacheable(
            Some("0x3b9aca00"),
            true,
            false,
            None
        ));
    }

    /// A block leg that did not answer is a failed read, not an old chain.
    #[test]
    fn a_block_that_did_not_answer_is_never_held() {
        assert!(!gas_signals_cacheable(
            Some("1000000000"),
            false,
            false,
            None
        ));
    }

    /// Gnosis: the tip is nearly the whole price, so a tipless read asked
    /// with a tip under-prices ~40× and must not own the next 15 s.
    #[test]
    fn a_tip_asked_for_and_missing_is_never_held() {
        assert!(!gas_signals_cacheable(Some("1000000007"), true, true, None));
    }

    /// A zero relay cap is degenerate — the core rejects it — and is not kept.
    #[test]
    fn the_relay_quote_is_held_only_when_it_is_a_real_cap() {
        assert!(bundler_quote_cacheable("2000000000"));
        assert!(!bundler_quote_cacheable("0"));
        assert!(!bundler_quote_cacheable(""));
        assert!(!bundler_quote_cacheable("0x77359400"));
    }
}

// ===========================================================================
// The coin nobody chose (auto_fee_token)
// ===========================================================================

const USDT: &str = "0x5555555555555555555555555555555555555555";

fn usdt_row(balance: &str, usd: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        recipient: USDC_RECIPIENT.to_owned(),
        asset: FeeAssetKind::Erc20,
        fee_token: Some(USDT.to_owned()),
        balance: balance.to_owned(),
        decimals: 6,
        symbol: "USDT".to_owned(),
        usd_balance: usd.to_owned(),
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    }
}

fn auto_request(chain_id: u32, calls: Vec<FeeCall>) -> Event {
    Event::QuoteRequested {
        chain_id,
        account: ACCOUNT.to_owned(),
        deployed: true,
        public_key_available: true,
        tier: FeeTier::Fast,
        calls,
        fee_token: None,
        auto_fee_token: true,
    }
}

fn usdc_transfer(units: u128) -> FeeCall {
    FeeCall {
        to: USDC.to_owned(),
        value: "0".to_owned(),
        data: encode_erc20_transfer(NATIVE_RECIPIENT, units).expect("encodes"),
    }
}

/// Gather with `rows`, then answer the simulation with `gas`. Returns the
/// simulated calls so a test can see which fee leg was priced.
fn settle_with(sut: &mut Sut, rows: Vec<FeeAssetQuote>, gas: Res) -> Vec<FeeCall> {
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let ops = sut.resolve(Res::InBandQuotes { quotes: Some(rows) });
    let calls = match ops.as_slice() {
        [Op::EstimateUserOpGas { calls, .. }, ..] => calls.clone(),
        other => panic!("one simulation: {other:?}"),
    };
    // Answer the simulation (and a measurement, if one was asked).
    let mut ops = sut.resolve(gas);
    while let Some(Op::MeasureInnerCalls { .. }) = ops.first() {
        ops = sut.resolve(Res::InnerCallsMeasured { gas: vec![] });
    }
    calls
}

fn fee_leg_token(calls: &[FeeCall]) -> Option<String> {
    let leg = calls.last().expect("a fee leg");
    (leg.value == "0" && leg.data.starts_with("0xa9059cbb")).then(|| leg.to.to_lowercase())
}

#[test]
fn nobody_chose_a_coin_so_a_wallet_without_eth_pays_in_the_stablecoin_it_holds() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![]));
    let simulated = settle_with(
        &mut sut,
        vec![native_row("0"), usdc_row("5000000")],
        estimated(),
    );
    assert_eq!(
        fee_leg_token(&simulated).as_deref(),
        Some(USDC),
        "the op priced carries the USDC leg it will be submitted with"
    );
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(USDC));
    let fee = view.fee.expect("quoted");
    assert_eq!(
        fee.fee_asset,
        FeeAssetView::Erc20 {
            token: USDC.to_owned(),
            decimals: 6,
            amount: USDC_FEE_UNITS.to_string(),
            symbol: Some("USDC".to_owned()),
        }
    );
    assert!(view.confirm_fee_ready, "the default works — nothing to fix");
}

#[test]
fn a_stablecoin_pays_before_the_chains_coin_when_both_can() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![]));
    settle_with(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
        estimated(),
    );
    assert_eq!(sut.view().fee_token.as_deref(), Some(USDC));
}

/// Sending 1 USDC of 5: USDC could pay, but the fee would then come out of
/// the very coin being sent. The chain's coin pays instead.
#[test]
fn the_fee_never_comes_out_of_the_coin_being_sent_while_another_can_pay() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![usdc_transfer(1_000_000)]));
    settle_with(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
        estimated(),
    );
    let view = sut.view();
    assert_eq!(view.fee_token, None, "ETH pays: {view:?}");
    assert_eq!(view.fee.expect("quoted").fee_asset, FeeAssetView::Native);
}

/// …and a Max of that coin — the whole balance — leaves nothing to pay its
/// own fee with, so the coin is not a candidate at all.
#[test]
fn a_coin_whose_whole_balance_is_being_sent_cannot_also_pay_the_fee() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![usdc_transfer(5_000_000)]));
    settle_with(
        &mut sut,
        vec![
            native_row("1000000000000000000"),
            usdc_row("5000000"),
            usdt_row("50000000", "50.00"),
        ],
        estimated(),
    );
    assert_eq!(sut.view().fee_token.as_deref(), Some(USDT));
}

#[test]
fn among_stablecoins_the_larger_balance_pays() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![]));
    settle_with(
        &mut sut,
        vec![
            native_row("1000000000000000000"),
            usdc_row("5000000"),
            usdt_row("50000000", "50.00"),
        ],
        estimated(),
    );
    assert_eq!(sut.view().fee_token.as_deref(), Some(USDT));
}

#[test]
fn when_no_coin_can_pay_the_requested_coin_stands_and_says_so() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![]));
    settle_with(
        &mut sut,
        vec![native_row("0"), usdc_row("1000000")],
        estimated(),
    );
    let view = sut.view();
    assert_eq!(view.fee_token, None);
    assert_eq!(view.fee.expect("quoted").fee_asset, FeeAssetView::Native);
    assert!(
        !view.confirm_fee_ready,
        "the native shortfall is what shows"
    );
}

#[test]
fn a_coin_the_person_picked_is_priced_as_picked() {
    let mut sut = Sut::new();
    // auto off, native named: priced native even though USDC could pay.
    sut.dispatch(request(CHAIN, vec![]));
    settle_with(
        &mut sut,
        vec![native_row("0"), usdc_row("5000000")],
        estimated(),
    );
    let view = sut.view();
    assert_eq!(view.fee.expect("quoted").fee_asset, FeeAssetView::Native);
    assert!(!view.confirm_fee_ready);
}

#[test]
fn a_chip_tap_ends_the_auto_pick_and_a_requote_keeps_it() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![]));
    settle_with(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
        estimated(),
    );
    assert_eq!(sut.view().fee_token.as_deref(), Some(USDC));
    sut.dispatch(Event::SelectFeeAsset { token: None });
    assert_eq!(sut.view().fee_token, None, "the tap is local");
    // The TTL timer from the first quote is still outstanding; drop it.
    sut.resolve(Res::TtlElapsed);
    sut.dispatch(Event::Requote);
    settle_with(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
        estimated(),
    );
    assert_eq!(
        sut.view().fee.expect("requoted").fee_asset,
        FeeAssetView::Native,
        "the person's coin, not the machine's"
    );
}

/// The pick is made on the static model (600k gas here) before anything is
/// simulated; a simulation that comes back ABOVE it (750k) can leave the
/// picked coin short. It is picked again on the real figure.
#[test]
fn a_pick_the_real_gas_outgrows_is_made_again() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![]));
    // 3.5 USDC: covers 600k gas (≈3.36 USDC), not 750k (≈4.20 USDC).
    let simulated = settle_with(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("3500000")],
        Res::UserOpGas {
            outcome: FeeGasOutcome::Estimated {
                verification_gas_limit: "400000".to_owned(), // ×1.5 = 600k
                call_gas_limit: "50000".to_owned(),          // → 100k floor
                pre_verification_gas: "40000".to_owned(),    // +10k = 50k
            },
        },
    );
    assert_eq!(
        fee_leg_token(&simulated).as_deref(),
        Some(USDC),
        "picked first"
    );
    let view = sut.view();
    assert_eq!(view.fee_token, None, "…and moved to ETH on the real gas");
    assert_eq!(view.fee.expect("quoted").fee_asset, FeeAssetView::Native);
    assert!(view.confirm_fee_ready);
}

#[test]
fn tempo_pays_in_a_held_stablecoin_when_the_default_is_not_held() {
    let other = "0x20c0000000000000000000000000000000000001";
    let mut sut = Sut::new();
    sut.dispatch(auto_request(TEMPO_CHAIN, vec![]));
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![
            pathusd_row("0"),
            FeeAssetQuote {
                fee_token: Some(other.to_owned()),
                symbol: "othUSD".to_owned(),
                ..pathusd_row("5000000")
            },
        ]),
    });
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(other));
    match view.fee.expect("tempo quote").fee_asset {
        FeeAssetView::Erc20 { token, .. } => assert_eq!(token, other),
        FeeAssetView::Native => panic!("Tempo has no native gas"),
    }
    assert!(view.confirm_fee_ready);
}

// ---------------------------------------------------------------------------
// Spec 079 — a fee that failed for a reason that can pass is asked again
// ---------------------------------------------------------------------------

/// Every failure a recovering relay or chain node can clear (spec 082 RJ12,
/// RJ13): 3 s, 6 s, then every 8 s — no 12 s / 15 s steps, so the fee is back
/// within 8 + 6 = 14 s of the relay returning (SC-003).
fn recoverable() -> [vela_core::app::fee_policy::FeeFailure; 6] {
    use vela_core::app::fee_policy::FeeFailure;
    [
        FeeFailure::QuoteUnavailable,
        FeeFailure::FeeTokenUnavailable,
        FeeFailure::EstimateFailed,
        FeeFailure::GasQuoteTooHigh,
        FeeFailure::ChainRead { rate_limited: true },
        FeeFailure::ChainRead {
            rate_limited: false,
        },
    ]
}

#[test]
fn a_recoverable_quote_failure_is_asked_again_on_a_growing_wait() {
    use vela_core::app::fee_policy::requote_delay_ms;
    for failure in recoverable() {
        let schedule: Vec<_> = (1..=6)
            .map(|attempt| requote_delay_ms(failure, attempt))
            .collect();
        assert_eq!(
            schedule,
            vec![
                Some(3_000),
                Some(6_000),
                Some(8_000),
                Some(8_000),
                Some(8_000),
                Some(8_000)
            ],
            "{failure:?}"
        );
    }
}

/// G47: each automatic re-quote is bounded, so the wait plus the ask that
/// follows it never passes the 15 s the fee has to be back in.
#[test]
fn every_wait_plus_its_re_quote_fits_in_fifteen_seconds() {
    use vela_core::app::fee_policy::{requote_delay_ms, REQUOTE_TIMEOUT_MS};
    assert_eq!(REQUOTE_TIMEOUT_MS, 6_000);
    for failure in recoverable() {
        for attempt in 0..=40 {
            let delay = requote_delay_ms(failure, attempt).unwrap_or(0);
            assert!(
                delay + REQUOTE_TIMEOUT_MS <= 15_000,
                "{failure:?} attempt {attempt}: {delay} + {REQUOTE_TIMEOUT_MS}"
            );
        }
    }
}

/// RJ13: the words under the fee row are the core's choice, not each shell's.
/// Each relay failure says what happened — no price, a coin it cannot price,
/// a simulation with no answer, a price past the chain's — never "check your
/// connection" over a connection that works (the Arbitrum swap of
/// 2026-10-03). A chain node that cannot be read names the chain (or the rate
/// limit) — never Vela; a failure no network caused gets no reason line.
#[test]
fn the_fee_row_s_reason_is_the_core_s_key() {
    use vela_core::app::fee_policy::{failure_reason_key, FeeFailure};
    for (failure, key) in [
        (FeeFailure::QuoteUnavailable, "componentsUi.gas.reasonQuote"),
        (
            FeeFailure::FeeTokenUnavailable,
            "componentsUi.gas.reasonFeeToken",
        ),
        (
            FeeFailure::EstimateFailed,
            "componentsUi.gas.reasonSimulation",
        ),
        (
            FeeFailure::GasQuoteTooHigh,
            "componentsUi.gas.reasonQuoteHigh",
        ),
    ] {
        assert_eq!(failure_reason_key(failure), Some(key), "{failure:?}");
    }
    assert_eq!(
        failure_reason_key(FeeFailure::ChainRead { rate_limited: true }),
        Some("home.balanceDetailStatusRetrying")
    );
    assert_eq!(
        failure_reason_key(FeeFailure::ChainRead {
            rate_limited: false
        }),
        Some("explore.chainDown")
    );
    for failure in [FeeFailure::MissingPublicKey, FeeFailure::CalculationFailed] {
        assert_eq!(failure_reason_key(failure), None, "{failure:?}");
    }
}

/// The wire: the old failures stay plain strings; the chain read is tagged.
#[test]
fn the_chain_read_failure_s_wire_shape() {
    use vela_core::app::fee_policy::FeeFailure;
    assert_eq!(
        serde_json::to_value(FeeFailure::QuoteUnavailable).unwrap_or_default(),
        serde_json::json!("quote_unavailable")
    );
    let chain = FeeFailure::ChainRead { rate_limited: true };
    let json = serde_json::to_value(chain).unwrap_or_default();
    assert_eq!(
        json,
        serde_json::json!({ "chain_read": { "rate_limited": true } })
    );
    assert_eq!(serde_json::from_value::<FeeFailure>(json).ok(), Some(chain));
}

#[test]
fn a_failure_no_retry_can_fix_is_never_asked_again() {
    use vela_core::app::fee_policy::{requote_delay_ms, FeeFailure};
    for failure in [
        FeeFailure::MissingPublicKey,
        FeeFailure::CalculationFailed,
        // Spec 083 fee: the same operation gets the same answer.
        FeeFailure::WouldFail,
    ] {
        assert_eq!(requote_delay_ms(failure, 1), None, "{failure:?}");
    }
}

// ---------------------------------------------------------------------------
// Spec 083 fee — a fee coin the operation itself spends, and a relay that
// answered "UserOperation simulation failed"
//
// The device pass: Uniswap USDC → ETH with 最大 (all 0.271741 USDC). The
// router pulls the USDC through Permit2, so the calls never say it moves; the
// machine picked USDC for the fee, the relay's simulation of "swap all USDC,
// then pay a fee in USDC" failed, and the column said the network was down —
// retrying forever, with the fee coins out of reach.
// ---------------------------------------------------------------------------

use vela_core::app::fee_policy::FeeBalanceChange;

const ROUTER: &str = "0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40";

/// A Universal Router `execute` — far over the 1 KiB estimation line, and no
/// `transfer` a decoder could read the USDC outflow from.
fn router_call() -> FeeCall {
    FeeCall {
        to: ROUTER.to_owned(),
        value: "0".to_owned(),
        data: format!("0x3593564c{}", "ab".repeat(1_200)),
    }
}

fn change(token: Option<&str>, delta: &str) -> FeeBalanceChange {
    FeeBalanceChange {
        token: token.map(str::to_owned),
        delta: delta.to_owned(),
    }
}

/// What the column's simulation drew for the Max swap: all the USDC out,
/// some ETH in.
fn max_usdc_swap() -> Event {
    Event::BalanceChangesMeasured {
        changes: vec![
            change(Some(USDC), "-5000000"),
            change(None, "101000000000000"),
        ],
    }
}

fn is_estimate(op: &Op) -> bool {
    matches!(op, Op::EstimateUserOpGas { .. })
}

fn is_measure(op: &Op) -> bool {
    matches!(op, Op::MeasureInnerCalls { .. })
}

/// The fee leg of the (only) simulation among `ops`: `None` = native.
fn simulated_leg(ops: &[Op]) -> Option<String> {
    let calls = ops
        .iter()
        .find_map(|op| match op {
            Op::EstimateUserOpGas { calls, .. } => Some(calls.clone()),
            _ => None,
        })
        .expect("a simulation was asked for");
    let leg = calls.last().expect("a fee leg").clone();
    if leg.data == "0x" {
        assert_eq!(leg.to, NATIVE_RECIPIENT, "a native leg pays the native row");
        None
    } else {
        Some(leg.to.to_lowercase())
    }
}

/// Gather with `rows`; returns the operations the gathering ended with (the
/// simulation, and the inner calls' measurement when there is a contract call).
fn gather(sut: &mut Sut, rows: Vec<FeeAssetQuote>) -> Vec<Op> {
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(Res::InBandQuotes { quotes: Some(rows) })
}

fn refused() -> Res {
    Res::UserOpGas {
        outcome: FeeGasOutcome::Refused,
    }
}

fn unmeasured() -> Res {
    Res::InnerCallsMeasured { gas: vec![None] }
}

/// With the swap's balance changes in hand before anything is simulated, the
/// USDC the swap takes is not there to pay a fee from: the chain's coin pays,
/// the op that is simulated carries the native leg, and the slide opens.
#[test]
fn a_max_sell_of_the_fee_coin_pays_in_the_chains_coin() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    sut.dispatch(max_usdc_swap());
    let ops = gather(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
    );
    assert_eq!(simulated_leg(&ops), None, "the native leg is simulated");
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    let view = sut.view();
    assert_eq!(view.failed, None);
    assert_eq!(view.fee_token, None, "ETH pays: {view:?}");
    assert_eq!(view.fee.expect("quoted").fee_asset, FeeAssetView::Native);
    assert!(view.confirm_fee_ready);
    let usdc = view
        .options
        .iter()
        .find(|option| option.contract.as_deref() == Some(USDC))
        .expect("USDC is still listed");
    assert!(usdc.insufficient, "nothing of it is left to pay from");
}

/// The coin the machine would pick from the calls alone (with no ETH to pay
/// in, the larger stablecoin) is the one the operation drains. The chain's
/// own coin takes over before any other stablecoin; only when it cannot pay
/// does the next coin in the usual order.
#[test]
fn a_drained_default_coin_gives_way_to_the_chains_coin_first() {
    let drain_usdt = || Event::BalanceChangesMeasured {
        changes: vec![change(Some(USDT), "-10000000")],
    };
    let rows = |native: &str| {
        vec![
            native_row(native),
            usdc_row("5000000"),
            usdt_row("10000000", "10.00"),
        ]
    };

    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    let ops = gather(&mut sut, rows("0"));
    assert_eq!(
        simulated_leg(&ops).as_deref(),
        Some(USDT),
        "unmeasured and no ETH, the larger stablecoin pays"
    );

    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    sut.dispatch(drain_usdt());
    let ops = gather(&mut sut, rows("1000000000000000000"));
    assert_eq!(simulated_leg(&ops), None, "the chain's coin, not USDC");
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    let view = sut.view();
    assert_eq!(view.fee_token, None);
    assert!(view.confirm_fee_ready, "{view:?}");

    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    sut.dispatch(drain_usdt());
    let ops = gather(&mut sut, rows("0"));
    assert_eq!(
        simulated_leg(&ops).as_deref(),
        Some(USDC),
        "no ETH to pay with: the next stablecoin"
    );
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(USDC));
    assert!(view.confirm_fee_ready, "{view:?}");
}

/// The simulation can land after the quote. A coin that the swap drains is
/// moved off at once — locally, at the gas already measured, recipient and
/// all — and never asked of the relay again. (No ETH held, and a router call
/// that names no fee coin, so USDC is the pick until the simulation answers;
/// the swap's ETH output then pays.)
#[test]
fn balance_changes_that_land_after_the_quote_move_the_fee_off_a_drained_coin() {
    let small = FeeCall {
        to: ROUTER.to_owned(),
        value: "0".to_owned(),
        data: format!("0x3593564c{}", "ab".repeat(100)),
    };
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![small]));
    let ops = gather(&mut sut, vec![native_row("0"), usdc_row("5000000")]);
    assert_eq!(simulated_leg(&ops).as_deref(), Some(USDC), "USDC first");
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    assert_eq!(sut.view().fee_token.as_deref(), Some(USDC));

    let ops = sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![
            change(Some(USDC), "-5000000"),
            change(None, "1000000000000000000"),
        ],
    });
    assert!(ops.is_empty(), "a local switch, nothing asked: {ops:?}");
    let view = sut.view();
    assert_eq!(view.fee_token, None);
    let fee = view.fee.expect("still quoted");
    assert_eq!(fee.fee_asset, FeeAssetView::Native);
    assert_eq!(fee.total_wei, NATIVE_FEE_WEI.to_string());
    assert_eq!(fee.fee_recipient.as_deref(), Some(NATIVE_RECIPIENT));
    assert!(view.confirm_fee_ready);
}

/// No simulation of its own to read (an RPC without `eth_simulateV1`): the
/// relay's refusal of the op paid in the machine's coin is the signal. The
/// next coin with something left — one the calls do not name — is simulated
/// next, and it quotes. (Spec 096: the chain's coin is now the first pick
/// for a contract call, so it is the one refused here.)
#[test]
fn a_relay_refusal_of_the_machines_coin_is_retried_in_the_next_coin() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    let ops = gather(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
    );
    assert_eq!(simulated_leg(&ops), None, "the chain's coin first");
    let ops = sut.resolve_matching(is_estimate, refused());
    assert_eq!(ops.len(), 1, "one more simulation, nothing else: {ops:?}");
    assert_eq!(simulated_leg(&ops).as_deref(), Some(USDC), "…with USDC");
    assert!(sut.view().busy, "still estimating — no failure flashes");
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    let view = sut.view();
    assert_eq!(view.failed, None);
    assert_eq!(view.fee_token.as_deref(), Some(USDC));
    assert!(matches!(
        view.fee.expect("quoted").fee_asset,
        FeeAssetView::Erc20 { .. }
    ));
    assert!(view.confirm_fee_ready);
}

/// Every coin drained, the chain's included: nothing is left to try, and
/// the failure is the operation's — not the network's. Every coin is marked,
/// so the shell says the coin cannot pay.
#[test]
fn when_nothing_can_pay_the_failure_is_not_the_network() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![change(Some(USDC), "-5000000")],
    });
    let ops = gather(&mut sut, vec![native_row("0"), usdc_row("5000000")]);
    // No coin covers; the requested coin (native) stands.
    assert_eq!(simulated_leg(&ops), None);
    let ops = sut.resolve_matching(is_estimate, refused());
    assert!(
        ops.is_empty(),
        "nothing left to pay from, so nothing else is tried: {ops:?}"
    );
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::WouldFail));
    assert!(view.fee.is_none());
    assert!(!view.confirm_fee_ready);
    assert!(
        view.options.iter().all(|option| option.insufficient),
        "{:?}",
        view.options
    );
}

/// A native balance short of the fee, with the USDC drained: the op itself
/// simulates (the leg is one unit), the quote is in the chain's coin, and the
/// slide stays shut on the Issue #262 path — the coin cannot pay.
#[test]
fn a_short_native_balance_is_the_insufficient_path() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![change(Some(USDC), "-5000000")],
    });
    let ops = gather(
        &mut sut,
        vec![native_row("1000000000000"), usdc_row("5000000")],
    );
    assert_eq!(simulated_leg(&ops), None);
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    let view = sut.view();
    assert_eq!(view.failed, None, "not a failure: a shortfall");
    assert_eq!(view.fee.expect("quoted").fee_asset, FeeAssetView::Native);
    assert!(!view.confirm_fee_ready);
    let selected = view
        .options
        .iter()
        .find(|option| option.selected)
        .expect("a selected coin");
    assert!(selected.contract.is_none() && selected.insufficient);
}

/// A coin the person chose is never swapped behind their back: the relay's
/// refusal is its own failure at once, never asked again by itself — and the
/// other coins stay choosable, from the failure.
#[test]
fn a_refusal_of_a_chosen_coin_fails_as_itself_and_the_picker_still_works() {
    use vela_core::app::fee_policy::requote_delay_ms;
    let mut sut = Sut::new();
    sut.dispatch(request_in(CHAIN, vec![router_call()], Some(USDC)));
    let ops = gather(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
    );
    assert_eq!(simulated_leg(&ops).as_deref(), Some(USDC));
    let ops = sut.resolve_matching(is_estimate, refused());
    assert!(ops.is_empty(), "no other coin is tried: {ops:?}");
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::WouldFail));
    assert_eq!(requote_delay_ms(FeeFailure::WouldFail, 1), None);
    assert_eq!(
        view.fee_token.as_deref(),
        Some(USDC),
        "the chosen coin stays"
    );
    assert_eq!(view.options.len(), 2);
    assert!(
        view.options.iter().all(|option| !option.insufficient),
        "both coins can be chosen: {:?}",
        view.options
    );

    // Picking the chain's coin from the failure prices the operation in it.
    let ops = sut.dispatch(Event::SelectFeeAsset { token: None });
    assert_eq!(
        ops.first(),
        Some(&Op::FetchGasPrice {
            chain_id: CHAIN,
            want_tip: true
        }),
        "{ops:?}"
    );
    assert_eq!(sut.view().fee_token, None);
}

/// A relay that could not be reached (or a shell that cannot tell) is still
/// the estimate failure it always was — asked again on the schedule, and no
/// other coin tried on an outage.
#[test]
fn an_unanswered_simulation_is_still_the_estimate_failure() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    gather(
        &mut sut,
        vec![native_row("1000000000000000000"), usdc_row("5000000")],
    );
    let ops = sut.resolve_matching(
        is_estimate,
        Res::UserOpGas {
            outcome: FeeGasOutcome::SimulationFailed,
        },
    );
    assert!(ops.is_empty(), "{ops:?}");
    assert_eq!(sut.view().failed, Some(FeeFailure::EstimateFailed));
}

/// Balance changes that leave the coin enough change nothing: a swap of
/// 1 USDC of 5 still pays its fee in USDC, the same figure as before. (No
/// ETH held, and a router call that names no fee coin.)
#[test]
fn a_swap_that_leaves_enough_of_the_fee_coin_is_priced_as_before() {
    let small = FeeCall {
        to: ROUTER.to_owned(),
        value: "0".to_owned(),
        data: format!("0x3593564c{}", "ab".repeat(100)),
    };
    let quote = |measured: bool| {
        let mut sut = Sut::new();
        sut.dispatch(auto_request(CHAIN, vec![small.clone()]));
        if measured {
            sut.dispatch(Event::BalanceChangesMeasured {
                changes: vec![
                    change(Some(USDC), "-1000000"),
                    change(None, "500000000000000"),
                ],
            });
        }
        let ops = gather(&mut sut, vec![native_row("0"), usdc_row("5000000")]);
        assert_eq!(simulated_leg(&ops).as_deref(), Some(USDC));
        sut.resolve_matching(is_estimate, estimated());
        sut.resolve_matching(is_measure, unmeasured());
        sut.view()
    };
    let (before, after) = (quote(false), quote(true));
    assert_eq!(after.fee, before.fee);
    assert_eq!(after.fee_token.as_deref(), Some(USDC));
    assert!(after.confirm_fee_ready);
    assert_eq!(
        after.fee.expect("quoted").fee_asset,
        FeeAssetView::Erc20 {
            token: USDC.to_owned(),
            decimals: 6,
            amount: USDC_FEE_UNITS.to_string(),
            symbol: Some("USDC".to_owned()),
        }
    );
}

/// The simulation lands on a quote that already failed: 0 ETH, so the relay
/// refused the USDC-paid op and nothing was left to try. The swap brings ETH
/// in before the fee leg runs — counted at half, still far above the fee —
/// so the chain's coin can pay now, and the operation is priced again in it,
/// once.
#[test]
fn balance_changes_that_land_on_a_failed_quote_price_it_again_in_a_coin_that_pays() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    let ops = gather(&mut sut, vec![native_row("0"), usdc_row("5000000")]);
    assert_eq!(simulated_leg(&ops).as_deref(), Some(USDC));
    assert!(sut.resolve_matching(is_estimate, refused()).is_empty());
    assert_eq!(sut.view().failed, Some(FeeFailure::WouldFail));
    sut.drop_matching(is_measure);

    let ops = sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![
            change(Some(USDC), "-5000000"),
            change(None, "1000000000000000000"),
        ],
    });
    assert_eq!(
        ops.first(),
        Some(&Op::FetchGasPrice {
            chain_id: CHAIN,
            want_tip: true
        }),
        "asked again: {ops:?}"
    );
    let ops = gather(&mut sut, vec![native_row("0"), usdc_row("5000000")]);
    assert_eq!(simulated_leg(&ops), None, "in the chain's coin");
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    let view = sut.view();
    assert_eq!(view.failed, None);
    assert_eq!(view.fee.expect("quoted").fee_asset, FeeAssetView::Native);
    assert!(view.confirm_fee_ready, "paid from what the swap brings in");
}

/// Tempo's fee model is its own; the balance changes do not touch it.
#[test]
fn tempo_ignores_measured_balance_changes() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(TEMPO_CHAIN, vec![]));
    sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![change(Some(TEMPO_DEFAULT_FEE_TOKEN), "-5000000")],
    });
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some(TEMPO_BASE_FEE_ATTO.to_string()),
        base_fee: None,
        priority_fee: None,
    });
    sut.resolve(Res::FeeRecipient {
        recipient: Some(COLLECTOR.to_owned()),
    });
    sut.resolve(Res::InBandQuotes {
        quotes: Some(vec![pathusd_row("5000000")]),
    });
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(TEMPO_DEFAULT_FEE_TOKEN));
    assert!(view.confirm_fee_ready, "{view:?}");
}

// ---------------------------------------------------------------------------
// Spec 083 fee, review fixes: a refresh that fails keeps its quote's coin, a
// run refused with every coin ends on one that could have paid, and what a
// swap brings in pays a fee only at half
// ---------------------------------------------------------------------------

fn two_coins() -> Vec<FeeAssetQuote> {
    vec![native_row("1000000000000000000"), usdc_row("5000000")]
}

/// Quoted in USDC after a relay refusal, with no simulation of its own: the
/// chain's coin was the machine's pick for the contract call (spec 096), the
/// relay refused it, and USDC — which the calls do not name — quoted.
fn quoted_in_usdc_after_eth_was_refused() -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    let ops = gather(&mut sut, two_coins());
    assert_eq!(simulated_leg(&ops), None);
    let ops = sut.resolve_matching(is_estimate, refused());
    assert_eq!(simulated_leg(&ops).as_deref(), Some(USDC));
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(USDC));
    assert!(matches!(
        view.fee.expect("quoted").fee_asset,
        FeeAssetView::Erc20 { .. }
    ));
    sut.drop_matching(|op| matches!(op, Op::StartTtl { .. }));
    sut
}

/// The coin in force and the coin the quote is priced in are the same coin —
/// the approve carries both, as `gas_fee_token` and the quoted amount.
fn coin_matches_quote(view: &vela_core::app::fee_policy::FeeView) -> bool {
    match view.fee.as_ref().map(|fee| &fee.fee_asset) {
        Some(FeeAssetView::Native) => view.fee_token.is_none(),
        Some(FeeAssetView::Erc20 { token, .. }) => view
            .fee_token
            .as_deref()
            .is_some_and(|coin| coin.eq_ignore_ascii_case(token)),
        None => true,
    }
}

/// Review P1: that USDC quote, refreshed. The refresh picks the chain's coin
/// afresh, and then the relay cannot be reached. A refresh keeps the old
/// quote on screen — and now its coin with it: a coin in force that is not
/// the quote's had the approve sign a transfer of one coin's amount to the
/// other row's recipient.
#[test]
fn a_refresh_that_fails_keeps_the_coin_of_the_quote_it_keeps() {
    let mut sut = quoted_in_usdc_after_eth_was_refused();
    sut.dispatch(Event::Requote);
    let ops = gather(&mut sut, two_coins());
    assert_eq!(simulated_leg(&ops), None, "the refresh picks ETH afresh");
    sut.resolve_matching(
        is_estimate,
        Res::UserOpGas {
            outcome: FeeGasOutcome::SimulationFailed,
        },
    );
    sut.drop_matching(is_measure);
    let view = sut.view();
    assert_eq!(view.failed, None, "a refresh keeps the quote on screen");
    assert!(coin_matches_quote(&view), "{view:?}");
    assert_eq!(view.fee_token.as_deref(), Some(USDC));
    let fee = view.fee.clone().expect("the old quote");
    assert!(matches!(fee.fee_asset, FeeAssetView::Erc20 { .. }));
    assert_eq!(fee.fee_recipient.as_deref(), Some(USDC_RECIPIENT));
    assert!(view.confirm_fee_ready);
    let selected = view
        .options
        .iter()
        .find(|option| option.selected)
        .expect("a selected coin");
    assert_eq!(
        selected.contract.as_deref(),
        Some(USDC),
        "the USDC row is the one ticked"
    );
}

/// Review P2: the same refresh, and the relay refuses the operation with
/// every coin (a swap whose deadline passed while the sheet was open). The
/// relay has just said the operation fails, so the old figure is not kept as
/// one to sign: the row says the operation would fail, on the last coin
/// tried that had something left to pay from — and nothing asks again by
/// itself.
#[test]
fn a_refresh_the_relay_refuses_with_every_coin_would_fail() {
    use vela_core::app::fee_policy::requote_delay_ms;
    let mut sut = quoted_in_usdc_after_eth_was_refused();
    sut.dispatch(Event::Requote);
    let ops = gather(&mut sut, two_coins());
    assert_eq!(simulated_leg(&ops), None);
    let ops = sut.resolve_matching(is_estimate, refused());
    assert_eq!(simulated_leg(&ops).as_deref(), Some(USDC), "USDC next");
    assert!(sut.resolve_matching(is_estimate, refused()).is_empty());
    sut.drop_matching(is_measure);
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::WouldFail));
    assert!(view.fee.is_none(), "the old quote is not one to sign");
    assert!(!view.confirm_fee_ready);
    assert_eq!(view.fee_token.as_deref(), Some(USDC), "ends on USDC");
    assert!(
        view.options.iter().all(|option| !option.insufficient),
        "{:?}",
        view.options
    );
    assert_eq!(requote_delay_ms(FeeFailure::WouldFail, 1), None);
}

/// Review P3: with the balance changes known, the refresh picks ETH again and
/// its failure keeps the ETH quote — the coin and the quote agree either way.
#[test]
fn a_measured_refresh_that_fails_keeps_the_same_coin() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![router_call()]));
    sut.dispatch(max_usdc_swap());
    let ops = gather(&mut sut, two_coins());
    assert_eq!(simulated_leg(&ops), None);
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
    sut.drop_matching(|op| matches!(op, Op::StartTtl { .. }));
    assert_eq!(sut.view().fee_token, None, "quoted in ETH");
    sut.dispatch(Event::Requote);
    let ops = gather(&mut sut, two_coins());
    assert_eq!(simulated_leg(&ops), None, "ETH, from the balance changes");
    sut.resolve_matching(
        is_estimate,
        Res::UserOpGas {
            outcome: FeeGasOutcome::SimulationFailed,
        },
    );
    sut.drop_matching(is_measure);
    let view = sut.view();
    assert!(coin_matches_quote(&view), "{view:?}");
    assert_eq!(view.fee_token, None);
    assert!(view.confirm_fee_ready);
}

/// Review P4: every coin that could pay refused — the chain's coin, since
/// the swap's router names the USDC it spends (spec 096) and the USDC is
/// never tried — and the simulation then shows the swap drains the USDC.
/// The run ends on ETH, the coin that had something left, so the row says
/// the operation would fail rather than that USDC cannot pay; and the
/// balance changes, told again and again, ask nothing.
#[test]
fn when_every_coin_is_refused_the_run_ends_on_one_that_could_have_paid() {
    let names_usdc = FeeCall {
        data: format!("{}{}", router_call().data, &USDC[2..]),
        ..router_call()
    };
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![names_usdc]));
    let ops = gather(&mut sut, two_coins());
    assert_eq!(simulated_leg(&ops), None);
    assert!(
        sut.resolve_matching(is_estimate, refused()).is_empty(),
        "the USDC the swap spends is not tried"
    );
    sut.drop_matching(is_measure);
    for _ in 0..3 {
        let ops = sut.dispatch(max_usdc_swap());
        assert!(ops.is_empty(), "nothing left to try: {ops:?}");
    }
    let view = sut.view();
    assert_eq!(view.failed, Some(FeeFailure::WouldFail));
    assert_eq!(view.fee_token, None, "ETH, the coin that could have paid");
    let eth = view
        .options
        .iter()
        .find(|option| option.contract.is_none())
        .expect("ETH is listed");
    assert!(eth.selected && !eth.insufficient, "{eth:?}");
    let usdc = view
        .options
        .iter()
        .find(|option| option.contract.as_deref() == Some(USDC))
        .expect("USDC is listed");
    assert!(!usdc.selected && usdc.insufficient, "the swap drains it");
}

/// What a swap brings in counts towards its fee at half: a swap may deliver
/// less than the simulation measured, and nothing prices the fee leg against
/// the real output before it is signed. With no ETH held and the USDC
/// drained, a swap bringing in exactly the fee leaves the slide shut on the
/// Issue #262 path; one bringing in twice the fee pays.
#[test]
fn what_a_swap_brings_in_pays_its_fee_only_at_half() {
    let quote = |inflow: u128| {
        let mut sut = Sut::new();
        sut.dispatch(auto_request(CHAIN, vec![router_call()]));
        sut.dispatch(Event::BalanceChangesMeasured {
            changes: vec![
                change(Some(USDC), "-5000000"),
                change(None, &inflow.to_string()),
            ],
        });
        let ops = gather(&mut sut, vec![native_row("0"), usdc_row("5000000")]);
        assert_eq!(simulated_leg(&ops), None, "the USDC is drained");
        sut.resolve_matching(is_estimate, estimated());
        sut.resolve_matching(is_measure, unmeasured());
        sut.view()
    };

    let short = quote(NATIVE_FEE_WEI);
    let fee = short.fee.clone().expect("quoted");
    assert_eq!(fee.total_wei, NATIVE_FEE_WEI.to_string(), "the premise");
    assert_eq!(fee.fee_asset, FeeAssetView::Native);
    assert_eq!(short.failed, None, "a shortfall, not a failure");
    assert!(!short.confirm_fee_ready, "half of the fee is not the fee");
    assert!(
        short
            .options
            .iter()
            .find(|option| option.selected)
            .is_some_and(|option| option.contract.is_none() && option.insufficient),
        "{:?}",
        short.options
    );

    let enough = quote(2 * NATIVE_FEE_WEI);
    assert_eq!(enough.fee_token, None);
    assert!(enough.confirm_fee_ready, "{enough:?}");
}

// ====================================================================}

// Spec 096 F2 — the machine never pays in a coin the operation may spend
//
// The real-dApp pass on BNB Chain: PancakeSwap USDC → BNB, all 2.3417 USDC.
// The batch approves the USDC to PancakeSwap's Permit2, Permit2-approves the
// router, and the router swaps it — not one `transfer` in it. With no
// simulation on the web, the machine weighed USDC by the calls alone, found
// nothing leaving, picked it (a stablecoin first), and the relay rejected the
// op. The batch below is that request, byte for byte
// (`dapp096/logs/req-pcs-usdc2.json`).
// ===========================================================================

const BSC: u32 = 56;
const BSC_USDC: &str = "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d";
const BSC_USDT: &str = "0x55d398326f99059fF775485246999027B3197955";
/// 0x207f7079df8f2000 — every USDC the wallet held, and every USDC the batch
/// approves and swaps.
const ALL_THE_USDC: &str = "2341700000000000000";

/// The three legs of the PancakeSwap USDC → BNB batch.
fn pancakeswap_usdc_to_bnb() -> Vec<FeeCall> {
    let leg = |to: &str, data: &str| FeeCall {
        to: to.to_owned(),
        value: "0".to_owned(),
        data: data.to_owned(),
    };
    vec![
        leg(
            "0x8AC76a51cc950d9822D68b83fE1Ad97B32Cd580d",
            concat!(
                "0x095ea7b300000000000000000000000031c2f6fcff4f8759b3bd5bf0e1084a055615c768000000",
                "000000000000000000000000000000000000000000207f7079df8f2000",
            ),
        ),
        leg(
            "0x31c2F6fcFf4F8759b3Bd5Bf0e1084A055615c768",
            concat!(
                "0x87517c450000000000000000000000008ac76a51cc950d9822d68b83fe1ad97b32cd580d000000",
                "000000000000000000d9c500dff816a1da21a48a732d3498bf09dc9aeb0000000000000000000000",
                "00000000000000000000000000207f7079df8f200000000000000000000000000000000000000000",
                "0000000000000000006ae73b90",
            ),
        ),
        leg(
            "0xd9C500DfF816a1Da21A48A732d3498Bf09dc9AEB",
            concat!(
                "0x3593564c0000000000000000000000000000000000000000000000000000000000000060000000",
                "00000000000000000000000000000000000000000000000000000000a00000000000000000000000",
                "00000000000000000000000000000000006abfb33800000000000000000000000000000000000000",
                "00000000000000000000000002000c00000000000000000000000000000000000000000000000000",
                "00000000000000000000000000000000000000000000000000000000000000000000000002000000",
                "00000000000000000000000000000000000000000000000000000000400000000000000000000000",
                "00000000000000000000000000000000000000016000000000000000000000000000000000000000",
                "00000000000000000000000100000000000000000000000000000000000000000000000000000000",
                "0000000002000000000000000000000000000000000000000000000000207f7079df8f2000000000",
                "000000000000000000000000000000000000000000000a993fd56a9c5a0000000000000000000000",
                "0000000000000000000000000000000000000000a000000000000000000000000000000000000000",
                "00000000000000000000000001000000000000000000000000000000000000000000000000000000",
                "000000002b8ac76a51cc950d9822d68b83fe1ad97b32cd580d000064bb4cdb9cbd36b01bd1cbaebf",
                "2de08d9173bc095c0000000000000000000000000000000000000000000000000000000000000000",
                "00000000000000000000000000000000000000004000000000000000000000000088cca0eedbf2c4",
                "426110bbfc998f048689266894000000000000000000000000000000000000000000000000000a99",
                "3fd56a9c5a",
            ),
        ),
    ]
}

fn bnb_row(balance: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        symbol: "BNB".to_owned(),
        usd_balance: "6.00".to_owned(),
        usd_price: Some("600.00000000".to_owned()),
        ..native_row(balance)
    }
}

/// A BNB Chain stablecoin row: 18 decimals, $1.
fn bsc_stable_row(token: &str, symbol: &str, balance: &str, usd: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        recipient: USDC_RECIPIENT.to_owned(),
        asset: FeeAssetKind::Erc20,
        fee_token: Some(token.to_owned()),
        balance: balance.to_owned(),
        decimals: 18,
        symbol: symbol.to_owned(),
        usd_balance: usd.to_owned(),
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    }
}

fn bsc_usdc_row() -> FeeAssetQuote {
    bsc_stable_row(BSC_USDC, "USDC", ALL_THE_USDC, "2.34")
}

/// Ask for the batch with nobody choosing the coin; answer the gathering
/// with `rows`. Returns the operations the gathering ended with.
fn ask_pancakeswap(sut: &mut Sut, rows: Vec<FeeAssetQuote>) -> Vec<Op> {
    sut.dispatch(auto_request(BSC, pancakeswap_usdc_to_bnb()));
    gather(sut, rows)
}

/// Answer the simulation and the inner calls' measurement.
fn settle(sut: &mut Sut) {
    sut.resolve_matching(is_estimate, estimated());
    sut.resolve_matching(is_measure, unmeasured());
}

fn option<'a>(
    view: &'a vela_core::app::fee_policy::FeeView,
    contract: Option<&str>,
) -> &'a vela_core::app::fee_policy::FeeOptionView {
    view.options
        .iter()
        .find(|option| match (option.contract.as_deref(), contract) {
            (None, None) => true,
            (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
            _ => false,
        })
        .expect("the coin is listed")
}

/// The finding itself: BNB can pay, and it does — never the USDC the swap
/// spends. (The machine used to pick USDC here: a stablecoin first, and the
/// calls "sent" none of it.)
#[test]
fn the_pancakeswap_usdc_swap_never_pays_its_fee_in_the_usdc_it_spends() {
    let mut sut = Sut::new();
    let ops = ask_pancakeswap(&mut sut, vec![bnb_row("10000000000000000"), bsc_usdc_row()]);
    assert_eq!(simulated_leg(&ops), None, "the op is priced with a BNB leg");
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.failed, None);
    assert_eq!(view.fee_token, None, "{view:?}");
    assert_eq!(
        view.fee.clone().expect("quoted").fee_asset,
        FeeAssetView::Native
    );
    assert!(view.confirm_fee_ready);
    let usdc = option(&view, Some(BSC_USDC));
    assert!(usdc.spent_by_operation, "{usdc:?}");
    assert!(!usdc.insufficient, "still the person's to choose");
    assert!(!option(&view, None).spent_by_operation);
}

/// No BNB to pay with: a stablecoin the batch does not touch pays.
#[test]
fn with_no_bnb_a_coin_the_swap_does_not_touch_pays() {
    let mut sut = Sut::new();
    let ops = ask_pancakeswap(
        &mut sut,
        vec![
            bnb_row("0"),
            bsc_usdc_row(),
            bsc_stable_row(BSC_USDT, "USDT", "5000000000000000000", "5.00"),
        ],
    );
    assert_eq!(
        simulated_leg(&ops).as_deref(),
        Some(BSC_USDT.to_lowercase().as_str())
    );
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(BSC_USDT));
    assert!(view.confirm_fee_ready, "{view:?}");
    assert!(!option(&view, Some(BSC_USDT)).spent_by_operation);
}

/// Nothing but the USDC the swap spends: the machine does not pay in it.
/// The coin it was asked for (BNB) stands, short, and says so; the person
/// may still choose the USDC — and while it is the coin in force, the core
/// says the operation spends it, for the sheet to warn.
#[test]
fn with_nothing_else_the_machine_says_so_and_a_usdc_pick_is_warned() {
    let mut sut = Sut::new();
    let ops = ask_pancakeswap(&mut sut, vec![bnb_row("0"), bsc_usdc_row()]);
    assert_eq!(simulated_leg(&ops), None, "never the USDC");
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.failed, None);
    assert_eq!(view.fee_token, None);
    assert!(!view.confirm_fee_ready, "BNB cannot pay, and that shows");
    let bnb = option(&view, None);
    assert!(bnb.selected && bnb.insufficient, "{bnb:?}");
    let usdc = option(&view, Some(BSC_USDC));
    assert!(!usdc.insufficient && usdc.spent_by_operation, "{usdc:?}");

    sut.dispatch(Event::SelectFeeAsset {
        token: Some(BSC_USDC.to_owned()),
    });
    let view = sut.view();
    assert_eq!(
        view.fee_token.as_deref(),
        Some(BSC_USDC),
        "the person's pick"
    );
    assert!(view.confirm_fee_ready, "a warning, not a block");
    let usdc = option(&view, Some(BSC_USDC));
    assert!(usdc.selected && usdc.spent_by_operation, "{usdc:?}");
}

/// A simulation that shows the swap leaves enough of the coin is the proof
/// the calls cannot give: the machine may pay in it, and nothing warns.
#[test]
fn a_simulation_that_leaves_enough_lets_the_machine_pay_in_a_spent_coin() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(BSC, pancakeswap_usdc_to_bnb()));
    sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![change(Some(BSC_USDC), "-1000000000000000000")],
    });
    let ops = gather(&mut sut, vec![bnb_row("0"), bsc_usdc_row()]);
    assert_eq!(
        simulated_leg(&ops).as_deref(),
        Some(BSC_USDC.to_lowercase().as_str()),
        "1.34 USDC is left, the fee is under one"
    );
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(BSC_USDC));
    assert!(view.confirm_fee_ready, "{view:?}");
    assert!(!option(&view, Some(BSC_USDC)).spent_by_operation);
}

/// Each leg alone makes the USDC a coin the operation may spend: the
/// approve is made TO it, Permit2's `approve` names it as an argument, and
/// the router names it inside its packed swap path.
#[test]
fn every_leg_of_the_batch_alone_spends_the_usdc() {
    for (n, leg) in pancakeswap_usdc_to_bnb().into_iter().enumerate() {
        let mut sut = Sut::new();
        sut.dispatch(auto_request(BSC, vec![leg]));
        let ops = gather(&mut sut, vec![bnb_row("10000000000000000"), bsc_usdc_row()]);
        assert_eq!(simulated_leg(&ops), None, "leg {n}");
        settle(&mut sut);
        assert!(
            option(&sut.view(), Some(BSC_USDC)).spent_by_operation,
            "leg {n}"
        );
    }
}

/// A contract call that names no fee coin may still pull one through an
/// allowance given earlier (a vault's `deposit(amount)` names nothing), so
/// with nothing simulated the chain's coin pays first — and a stablecoin the
/// call does not name pays when the chain's coin cannot. Neither is flagged:
/// nothing in the calls says the operation spends them.
#[test]
fn a_contract_call_pays_in_the_chains_coin_first_until_something_measures_it() {
    let deposit = FeeCall {
        to: "0x9999999999999999999999999999999999999999".to_owned(),
        value: "0".to_owned(),
        // deposit(uint256 assets, address receiver)
        data: format!("0x6e553f65{:064x}{:0>64}", 1_000_000u128, &ACCOUNT[2..]),
    };
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![deposit.clone()]));
    settle_with_contract(&mut sut, two_coins());
    let view = sut.view();
    assert_eq!(view.fee_token, None, "{view:?}");
    assert!(!option(&view, Some(USDC)).spent_by_operation);

    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![deposit]));
    settle_with_contract(&mut sut, vec![native_row("0"), usdc_row("5000000")]);
    assert_eq!(sut.view().fee_token.as_deref(), Some(USDC));
}

fn settle_with_contract(sut: &mut Sut, rows: Vec<FeeAssetQuote>) {
    gather(sut, rows);
    settle(sut);
}

/// Plain sends keep the rule they had: a transfer states its amount, so a
/// coin being sent pays its own fee when it holds both — the fee on top of
/// the amount — and is never flagged; a Max of it cannot.
#[test]
fn a_plain_send_still_pays_its_fee_in_the_coin_it_sends_when_that_is_all_there_is() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![usdc_transfer(1_000_000)]));
    settle_with(
        &mut sut,
        vec![native_row("0"), usdc_row("5000000")],
        estimated(),
    );
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(USDC), "{view:?}");
    assert!(view.confirm_fee_ready);
    assert!(view.options.iter().all(|option| !option.spent_by_operation));

    let mut sut = Sut::new();
    sut.dispatch(auto_request(CHAIN, vec![usdc_transfer(5_000_000)]));
    settle_with(
        &mut sut,
        vec![native_row("0"), usdc_row("5000000")],
        estimated(),
    );
    let view = sut.view();
    assert_eq!(
        view.fee_token, None,
        "a Max cannot pay its own fee: {view:?}"
    );
    assert!(!view.confirm_fee_ready);
    assert!(view.options.iter().all(|option| !option.spent_by_operation));
}

// ---------------------------------------------------------------------------
// Issue #411 — a Uniswap swap on Polygon preselected POL, which the wallet
// does not hold
//
// Android 0.9.6, the in-app browser: pUSD −128.51 → USDC +128.4946. The
// fee-coin list read pUSD (257.02, fee ~0.317), USDC (99.99, fee ~0.317) and
// POL (0, fee ~2.95) — and POL, which cannot pay, was the one selected. The
// router's swap path names both stablecoins, so until a simulation says how
// much of each the swap leaves, neither provably pays (spec 096 F2), the
// contract call puts the chain's coin first, and with no coin that pays the
// requested one (native) stands. The shell's own simulation is what lets the
// machine see that both stablecoins can pay; only the desktop told it.
// ---------------------------------------------------------------------------

const POLYGON: u32 = 137;
/// Polygon's native USDC, checksummed as the relay writes it — the
/// simulation's deltas carry it lower-case.
const POLYGON_USDC: &str = "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359";
/// The swap's input coin (a synthetic address: only its role matters).
const PUSD: &str = "0x7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a7a";

/// POL at $0.1075, none of it held.
fn pol_row(balance: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        symbol: "POL".to_owned(),
        balance: balance.to_owned(),
        usd_balance: "0.00".to_owned(),
        usd_price: Some("0.10750000".to_owned()),
        ..native_row(balance)
    }
}

fn polygon_stable_row(token: &str, symbol: &str, balance: &str, usd: &str) -> FeeAssetQuote {
    FeeAssetQuote {
        recipient: USDC_RECIPIENT.to_owned(),
        asset: FeeAssetKind::Erc20,
        fee_token: Some(token.to_owned()),
        balance: balance.to_owned(),
        decimals: 6,
        symbol: symbol.to_owned(),
        usd_balance: usd.to_owned(),
        usd_price: Some("1".to_owned()),
        native_usd_floor_price: None,
    }
}

/// The three rows the sheet listed: POL 0, pUSD 257.02, USDC 99.99.
fn issue_411_rows() -> Vec<FeeAssetQuote> {
    vec![
        pol_row("0"),
        polygon_stable_row(PUSD, "pUSD", "257020000", "257.02"),
        polygon_stable_row(POLYGON_USDC, "USDC", "99990000", "99.99"),
    ]
}

/// A Universal Router `execute` whose V3 path is pUSD → (0.01 %) → USDC:
/// both coins named as packed 20-byte words, no `transfer` anywhere.
fn uniswap_pusd_to_usdc() -> FeeCall {
    let bare = |address: &str| address.trim_start_matches("0x").to_ascii_lowercase();
    FeeCall {
        to: ROUTER.to_owned(),
        value: "0".to_owned(),
        data: format!(
            "0x3593564c{}{}000064{}{}",
            "00".repeat(32 * 9),
            bare(PUSD),
            bare(POLYGON_USDC),
            "00".repeat(32 * 2 + 9),
        ),
    }
}

/// Polygon at 2185 gwei: 450 000 gas × 3 = 2.94975 POL, ≈ $0.317 — the
/// sheet's figures.
fn polygon_gather(sut: &mut Sut) -> Vec<Op> {
    polygon_gather_with(sut, issue_411_rows())
}

/// [`polygon_gather`] with other rows.
fn polygon_gather_with(sut: &mut Sut, rows: Vec<FeeAssetQuote>) -> Vec<Op> {
    sut.resolve(Res::GasPrice {
        eth_gas_price: Some("2185000000000".to_owned()),
        base_fee: Some("0".to_owned()),
        priority_fee: Some("0".to_owned()),
    });
    sut.resolve(Res::BundlerQuote {
        quote: Some(FeeBundlerQuote {
            max_fee_per_gas: "4370000000000".to_owned(),
            max_priority_fee_per_gas: None,
            network_fee_per_gas: Some("2185000000000".to_owned()),
            relayer_fee_per_gas: Some("2185000000000".to_owned()),
        }),
    });
    sut.resolve(Res::InBandQuotes { quotes: Some(rows) })
}

/// What the Android sheet's simulation drew, as the shell hands it over:
/// the coins' own `Transfer` logs, lower-case contracts, signed base units.
fn issue_411_swap() -> Event {
    Event::BalanceChangesMeasured {
        changes: vec![
            change(Some(&PUSD.to_ascii_lowercase()), "-128510000"),
            change(Some(&POLYGON_USDC.to_ascii_lowercase()), "128494600"),
        ],
    }
}

/// Today's fallback, then the fix: unmeasured, no coin provably pays and the
/// requested coin (POL, held at 0) stands, short; the simulation answers and
/// the fee moves — locally, nothing asked of the relay — to pUSD, the larger
/// balance of the two coins that pay. Never POL.
#[test]
fn issue_411_a_swap_the_simulation_measured_pays_in_a_coin_that_can() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    let ops = polygon_gather(&mut sut);
    assert_eq!(simulated_leg(&ops), None, "unmeasured: the native leg");
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.failed, None);
    assert_eq!(view.fee_token, None, "today's fallback: {view:?}");
    let pol = option(&view, None);
    assert!(pol.selected && pol.insufficient, "{pol:?}");
    assert_eq!(pol.amount.as_deref(), Some("2949750000000000000"));
    assert!(!view.confirm_fee_ready, "POL cannot pay, and that shows");
    for coin in [PUSD, POLYGON_USDC] {
        let row = option(&view, Some(coin));
        assert!(row.spent_by_operation && !row.insufficient, "{row:?}");
    }

    let ops = sut.dispatch(issue_411_swap());
    assert!(ops.is_empty(), "a local switch, nothing asked: {ops:?}");
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(PUSD), "{view:?}");
    assert!(view.confirm_fee_ready, "{view:?}");
    let fee = view.fee.clone().expect("still quoted");
    let FeeAssetView::Erc20 {
        token,
        amount,
        symbol,
        ..
    } = fee.fee_asset
    else {
        panic!("paid in a stablecoin: {fee:?}");
    };
    assert_eq!(token, PUSD);
    assert_eq!(symbol.as_deref(), Some("pUSD"));
    assert_eq!(
        Some(amount.as_str()),
        option(&view, Some(PUSD)).amount.as_deref()
    );
    assert_eq!(fee.fee_recipient.as_deref(), Some(USDC_RECIPIENT));
    assert!(
        !option(&view, None).selected,
        "never the POL it does not hold"
    );
    for coin in [PUSD, POLYGON_USDC] {
        let row = option(&view, Some(coin));
        assert!(!row.spent_by_operation && !row.insufficient, "{row:?}");
    }
}

/// The simulation can answer before the quote is priced: then the op the
/// relay simulates already carries the pUSD leg, and POL is never shown as
/// the pick at all.
#[test]
fn issue_411_measured_before_the_quote_prices_the_pusd_leg_from_the_start() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    sut.dispatch(issue_411_swap());
    let ops = polygon_gather(&mut sut);
    assert_eq!(
        simulated_leg(&ops).as_deref(),
        Some(PUSD),
        "the pUSD leg is simulated"
    );
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(PUSD), "{view:?}");
    assert!(view.confirm_fee_ready);
}

/// A swap of ALL the pUSD leaves none of it to pay with: USDC — which the
/// swap brings in, counted at half — pays. Still never POL.
#[test]
fn issue_411_a_swap_of_all_the_pusd_pays_in_the_usdc() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    polygon_gather(&mut sut);
    settle(&mut sut);
    sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![
            change(Some(&PUSD.to_ascii_lowercase()), "-257020000"),
            change(Some(&POLYGON_USDC.to_ascii_lowercase()), "256990000"),
        ],
    });
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(POLYGON_USDC), "{view:?}");
    assert!(view.confirm_fee_ready);
    assert!(option(&view, Some(PUSD)).insufficient);
}

/// The measured pick can rest on what the swap brings in. A wallet holding
/// 0.10 USDC — under the ~0.317 fee — swaps ALL its pUSD into USDC: only the
/// USDC the swap delivers (counted at half) can pay, and the machine picks
/// it. The quote it settles must be in that coin, with its amount, ready to
/// confirm — the machine never refuses the coin it chose itself ("the relay
/// cannot quote this coin", no amounts, nothing the sheet can explain), and
/// never falls back to the POL nobody holds. Whether the simulation lands
/// before the quote or after it.
#[test]
fn issue_411_a_coin_the_swap_funds_is_quoted_never_refused() {
    let usdc = POLYGON_USDC.to_ascii_lowercase();
    let rows = || {
        vec![
            pol_row("0"),
            polygon_stable_row(PUSD, "pUSD", "257020000", "257.02"),
            polygon_stable_row(POLYGON_USDC, "USDC", "100000", "0.10"),
        ]
    };
    let all_the_pusd = || Event::BalanceChangesMeasured {
        changes: vec![
            change(Some(&PUSD.to_ascii_lowercase()), "-257020000"),
            change(Some(&usdc), "256990000"),
        ],
    };
    let in_usdc = |view: &vela_core::app::fee_policy::FeeView| {
        assert_eq!(view.failed, None, "{view:?}");
        assert_eq!(view.fee_token.as_deref(), Some(POLYGON_USDC), "{view:?}");
        assert!(view.confirm_fee_ready, "{view:?}");
        let fee = view.fee.clone().expect("quoted, with its amount");
        assert!(
            matches!(&fee.fee_asset, FeeAssetView::Erc20 { token, .. } if token == POLYGON_USDC),
            "{fee:?}"
        );
        assert!(option(view, Some(POLYGON_USDC)).amount.is_some());
    };

    // Measured first: the USDC leg is what the relay simulates.
    let mut sut = Sut::new();
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    sut.dispatch(all_the_pusd());
    let ops = polygon_gather_with(&mut sut, rows());
    assert_eq!(simulated_leg(&ops).as_deref(), Some(usdc.as_str()));
    settle(&mut sut);
    in_usdc(&sut.view());

    // Measured after the quote: the local switch lands on the same coin.
    let mut sut = Sut::new();
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    polygon_gather_with(&mut sut, rows());
    settle(&mut sut);
    assert_eq!(sut.view().fee_token, None, "unmeasured: the fallback");
    sut.dispatch(all_the_pusd());
    in_usdc(&sut.view());
}

/// The other side of that rule, kept as it was: a coin the PERSON picks that
/// the swap drains is still quoted, amount and all, and marked short — the
/// sheet says why the slide is shut — rather than refused with nothing to
/// show.
#[test]
fn issue_411_a_picked_coin_the_swap_drains_is_quoted_short_not_refused() {
    let mut sut = Sut::new();
    sut.dispatch(request_in(
        POLYGON,
        vec![uniswap_pusd_to_usdc()],
        Some(PUSD),
    ));
    sut.dispatch(Event::BalanceChangesMeasured {
        changes: vec![
            change(Some(&PUSD.to_ascii_lowercase()), "-257020000"),
            change(Some(&POLYGON_USDC.to_ascii_lowercase()), "256990000"),
        ],
    });
    polygon_gather(&mut sut);
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.failed, None, "{view:?}");
    assert_eq!(view.fee_token.as_deref(), Some(PUSD));
    assert!(
        view.fee.is_some(),
        "the amount is there to explain the shortfall"
    );
    assert!(!view.confirm_fee_ready);
    assert!(option(&view, Some(PUSD)).insufficient);
}

/// The machine forgets a measurement on every new question (a stale quote's
/// re-ask, the refresh's), so the shell tells it again after each — the
/// rule every shell's fee sessions keep. Told, the re-ask lands on pUSD.
#[test]
fn issue_411_a_re_ask_forgets_the_measurement_until_it_is_told_again() {
    let mut sut = Sut::new();
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    sut.dispatch(issue_411_swap());
    polygon_gather(&mut sut);
    settle(&mut sut);
    assert_eq!(sut.view().fee_token.as_deref(), Some(PUSD));

    // The settled quote's TTL timer is the shell's; a re-ask supersedes it.
    let is_ttl = |op: &Op| matches!(op, Op::StartTtl { .. });
    sut.drop_matching(is_ttl);
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    let ops = polygon_gather(&mut sut);
    assert_eq!(simulated_leg(&ops), None, "the fallback, until told");
    settle(&mut sut);
    assert_eq!(sut.view().fee_token, None);

    sut.drop_matching(is_ttl);
    sut.dispatch(auto_request(POLYGON, vec![uniswap_pusd_to_usdc()]));
    sut.dispatch(issue_411_swap());
    let ops = polygon_gather(&mut sut);
    assert_eq!(simulated_leg(&ops).as_deref(), Some(PUSD));
    settle(&mut sut);
    let view = sut.view();
    assert_eq!(view.fee_token.as_deref(), Some(PUSD));
    assert!(view.confirm_fee_ready);
}

// ---------------------------------------------------------------------------
// The bound on a whole run (spec 094 S9, 089 F06)
// ---------------------------------------------------------------------------

/// Offline, the run's reads each time out on their own, one after another, and
/// the row read "Estimating…" far past 15 s with no words. Every run is
/// bounded now, by a timer the core asks for with its reads.
#[test]
fn every_run_asks_for_its_deadline_with_its_reads() {
    let mut sut = Timed::new();
    let ops = sut.dispatch(request(CHAIN, vec![]));
    assert_eq!(
        ops.last(),
        Some(&Op::StartDeadline {
            ms: vela_core::app::fee_policy::QUOTE_DEADLINE_MS
        })
    );
    assert_eq!(vela_core::app::fee_policy::QUOTE_DEADLINE_MS, 15_000);
}

#[test]
fn a_run_with_no_chain_price_by_the_deadline_is_a_chain_read_failure() {
    let mut sut = Timed::new();
    sut.dispatch(request(CHAIN, vec![]));
    assert!(sut.view().busy);
    let ops = sut.resolve_matching(is_deadline, Res::DeadlineElapsed);
    assert!(
        ops.is_empty(),
        "nothing more is asked by the deadline itself"
    );
    let view = sut.view();
    assert!(!view.busy, "no longer Estimating…");
    assert_eq!(
        view.failed,
        Some(FeeFailure::ChainRead {
            rate_limited: false
        })
    );
    assert!(!view.confirm_fee_ready);
    // The reads that come back late belong to a run that is over.
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    let late = sut.resolve(quotes_ok());
    assert!(late.is_empty(), "a late answer starts nothing");
    assert!(sut.view().failed.is_some());
    // …and the failure is one the schedule asks again for.
    assert!(vela_core::app::fee_policy::requote_delay_ms(
        FeeFailure::ChainRead {
            rate_limited: false
        },
        1
    )
    .is_some());
}

#[test]
fn a_run_stuck_on_the_relay_is_a_quote_failure_and_a_retry_starts_over() {
    let mut sut = Timed::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    // Estimating: the relay's simulation never answers.
    sut.resolve_matching(is_deadline, Res::DeadlineElapsed);
    assert_eq!(sut.view().failed, Some(FeeFailure::QuoteUnavailable));
    // The schedule's re-ask is a fresh run, with a deadline of its own.
    let ops = sut.dispatch(Event::Requote);
    assert!(ops.iter().any(is_deadline));
    assert!(sut.view().busy);
}

#[test]
fn a_priced_quote_ignores_its_deadline_and_a_refresh_keeps_the_one_on_screen() {
    let mut sut = Timed::new();
    sut.dispatch(request(CHAIN, vec![]));
    sut.resolve(gas_ok());
    sut.resolve(bundler_ok());
    sut.resolve(quotes_ok());
    sut.resolve_matching(|op| matches!(op, Op::EstimateUserOpGas { .. }), estimated());
    assert!(sut.view().fee.is_some());
    // The first run's deadline fires after it priced: nothing happens.
    sut.resolve_matching(is_deadline, Res::DeadlineElapsed);
    assert!(sut.view().failed.is_none());
    assert!(sut.view().confirm_fee_ready);
    // A refresh that hangs keeps the quote it had (`fail` on a refresh).
    sut.drop_matching(|op| matches!(op, Op::StartTtl { .. }));
    sut.dispatch(Event::Requote);
    sut.resolve_matching(is_deadline, Res::DeadlineElapsed);
    let view = sut.view();
    assert!(!view.busy);
    assert!(
        view.fee.is_some(),
        "the quote on screen survives a hung refresh"
    );
}
