//! The speed control's rules (spec 069), one test per rule.
//!
//! These moved here from the web's `gas-price.test.ts`, `speed-choice.test.ts`
//! and `free-speed.svelte.test.ts` along with the code they pin, number for
//! number: the seven measured chains, the three mined Polygon receipts, the
//! owner's L2 and the Tempo / floor-clamped / priced shapes are the same
//! vectors, so a shell cannot drift from what the web shipped in spec 068.

#![cfg(feature = "crux")]

mod support;

use support::DomainDriver;
use vela_core::app::fee_policy::{FeeAssetView, FeeEstimateView, FeeTier};
use vela_core::app::fee_speed::{
    gas_price_range_texts, gas_price_wei, indistinguishable, one_speed, speed_is_free, Event,
    FeeSpeed, FeeSpeedView, GasPriceRange, TierPreviewQuote, TierQuote,
};
use vela_core::l10n::number::NumberPreset;

const EN: NumberPreset = NumberPreset::CommaDot;

fn texts(values: &[Option<u128>]) -> Vec<Option<String>> {
    texts_in(values, EN)
}

fn texts_in(values: &[Option<u128>], preset: NumberPreset) -> Vec<Option<String>> {
    let ranges: Vec<Option<GasPriceRange>> = values
        .iter()
        .map(|low| low.map(|low| GasPriceRange { low, high: None }))
        .collect();
    gas_price_range_texts(&ranges, preset)
}

fn one(wei: u128) -> String {
    texts(&[Some(wei)])[0].clone().unwrap()
}

fn range(low: u128, high: u128) -> Option<GasPriceRange> {
    Some(GasPriceRange {
        low,
        high: Some(high),
    })
}

fn strings(values: &[&str]) -> Vec<Option<String>> {
    values.iter().map(|s| Some((*s).to_owned())).collect()
}

// ---------------------------------------------------------------------------
// The unit is chosen by the number (issue 684)
// ---------------------------------------------------------------------------

/// Seven REAL measured chains spanning eight orders of magnitude — the reason
/// the unit is picked by the number instead of fixed at `gwei`, which would
/// have printed `0.000000` on four of these rows.
#[test]
fn every_measured_chain_reads_the_way_a_person_would_say_it() {
    assert_eq!(one(300_000_000_000), "300 gwei"); // Polygon
    assert_eq!(one(25_000_000_000), "25 gwei"); // Kaia
    assert_eq!(one(50_000_000), "0.05 gwei"); // Ethereum / BSC
    assert_eq!(one(5_000_000), "0.005 gwei"); // Base
    assert_eq!(one(3_000), "3,000 wei"); // Optimism
    assert_eq!(one(10), "10 wei"); // Gnosis
}

#[test]
fn the_unit_switches_at_a_thousandth_of_a_gwei_and_nowhere_else() {
    assert_eq!(one(999_999), "999,999 wei");
    assert_eq!(one(1_000_000), "0.001 gwei");
    assert_eq!(one(1_000_001), "0.001 gwei");
}

#[test]
fn zero_is_said_only_when_the_chain_charges_zero() {
    assert_eq!(one(0), "0 wei");
    assert_eq!(one(1), "1 wei");
}

/// Tempo's sign request carries no priority fee: a fabricated "0 wei" would
/// claim the tiers are equal.
#[test]
fn there_is_nothing_to_say_without_an_honest_number() {
    assert_eq!(texts(&[None]), vec![None]);
    assert_eq!(
        texts(&[None, Some(10), None]),
        vec![None, Some("10 wei".to_owned()), None]
    );
}

#[test]
fn the_digits_follow_the_persons_number_preset() {
    assert_eq!(
        texts_in(&[Some(50_000_000)], NumberPreset::DotComma),
        strings(&["0,05 gwei"])
    );
    assert_eq!(
        texts_in(&[Some(3_000)], NumberPreset::DotComma),
        strings(&["3.000 wei"])
    );
    assert_eq!(
        texts_in(&[Some(3_000)], NumberPreset::SpaceComma),
        strings(&["3 000 wei"])
    );
}

// ---------------------------------------------------------------------------
// Precision across the tiers being compared
// ---------------------------------------------------------------------------

const POLYGON: [Option<u128>; 3] = [
    Some(270_164_477_149),
    Some(282_464_783_233),
    Some(299_589_817_385),
];

#[test]
fn three_significant_digits_when_that_tells_the_tiers_apart() {
    assert_eq!(
        texts(&POLYGON),
        strings(&["270 gwei", "282 gwei", "300 gwei"])
    );
}

#[test]
fn precision_widens_only_as_far_as_it_must() {
    assert_eq!(
        texts(&[Some(270_100_000_000), Some(270_400_000_000)]),
        strings(&["270.1 gwei", "270.4 gwei"])
    );
    assert_eq!(
        texts(&[
            Some(270_100_000_000),
            Some(270_400_000_000),
            Some(299_000_000_000)
        ]),
        strings(&["270.1 gwei", "270.4 gwei", "299 gwei"])
    );
}

/// Every OP-stack L2 answers a 0 tip, so all three tiers really do buy the
/// same price — inventing a difference is the lie the row exists to prevent.
#[test]
fn equal_prices_print_equally() {
    assert_eq!(
        texts(&[Some(5_000_000), Some(5_000_000), Some(5_000_000)]),
        strings(&["0.005 gwei", "0.005 gwei", "0.005 gwei"])
    );
}

#[test]
fn the_wei_branch_stays_exact() {
    assert_eq!(
        texts(&[Some(1_937), Some(2_377), Some(3_244)]),
        strings(&["1,937 wei", "2,377 wei", "3,244 wei"])
    );
}

/// Chosen per value, the unit split the rows — "0.0012 gwei" above
/// "750,000 wei" — and the dearest tier read as the smallest number.
#[test]
fn every_tier_reads_in_one_unit_even_across_the_boundary() {
    assert_eq!(
        texts(&[Some(1_200_000), Some(750_000), Some(600_000)]),
        strings(&["0.0012 gwei", "0.00075 gwei", "0.0006 gwei"])
    );
    assert_eq!(
        texts(&[Some(999_999), Some(1_000_000), Some(1_000_001)]),
        strings(&["0.001 gwei", "0.001 gwei", "0.001 gwei"])
    );
    assert_eq!(
        texts(&[Some(999_999), Some(875_000), Some(800_000)]),
        strings(&["999,999 wei", "875,000 wei", "800,000 wei"])
    );
    assert_eq!(
        texts(&[None, Some(1_200_000), Some(750_000)]),
        vec![
            None,
            Some("0.0012 gwei".to_owned()),
            Some("0.00075 gwei".to_owned())
        ]
    );
}

#[test]
fn precision_never_grows_a_long_tail() {
    assert_eq!(
        texts(&[Some(50_000_100), Some(50_000_000), Some(50_000_000)]),
        strings(&["0.05 gwei", "0.05 gwei", "0.05 gwei"])
    );
    assert_eq!(
        texts(&[Some(270_110_000_000), Some(270_140_000_000)]),
        strings(&["270.11 gwei", "270.14 gwei"])
    );
    assert_eq!(
        texts(&[
            Some(20_200_000_000),
            Some(20_125_000_000),
            Some(20_100_000_000)
        ]),
        strings(&["20.2 gwei", "20.13 gwei", "20.1 gwei"])
    );
}

#[test]
fn a_rate_rounds_to_nearest_rather_than_truncating() {
    assert_eq!(one(299_589_817_385), "300 gwei");
    assert_eq!(one(999_999_999), "1 gwei");
}

// ---------------------------------------------------------------------------
// The gas price as a range (issue 685)
// ---------------------------------------------------------------------------

/// The owner, on an ETH L2: fees 1.8× apart over bids 0.5% apart. The cap is
/// where the tiers differ.
#[test]
fn the_owners_l2_tiers_differ_the_way_their_fees_do() {
    assert_eq!(
        gas_price_range_texts(
            &[
                range(20_210_000, 60_410_000),
                range(20_130_000, 40_230_000),
                range(20_110_000, 30_160_000),
            ],
            EN
        ),
        strings(&[
            "0.02021 ~ 0.06041 gwei",
            "0.02013 ~ 0.04023 gwei",
            "0.02011 ~ 0.03016 gwei"
        ])
    );
}

#[test]
fn the_three_mined_polygon_receipts_read_as_their_ranges() {
    assert_eq!(
        gas_price_range_texts(
            &[
                range(299_589_817_385, 805_065_222_658),
                range(282_464_783_233, 527_288_291_674),
                range(270_164_477_149, 390_302_745_042),
            ],
            EN
        ),
        strings(&["300 ~ 805 gwei", "282 ~ 527 gwei", "270 ~ 390 gwei"])
    );
}

#[test]
fn both_ends_of_every_range_share_one_unit() {
    let texts = gas_price_range_texts(
        &[
            range(750_000, 1_500_000),
            range(650_000, 1_050_000),
            range(600_000, 900_000),
        ],
        EN,
    );
    assert_eq!(
        texts,
        strings(&[
            "0.00075 ~ 0.0015 gwei",
            "0.00065 ~ 0.00105 gwei",
            "0.0006 ~ 0.0009 gwei"
        ])
    );
    for text in texts.into_iter().flatten() {
        assert_eq!(text.matches("wei").count(), 1, "the unit once: {text}");
        assert!(text.ends_with(" gwei"));
    }
}

#[test]
fn a_set_under_a_thousandth_of_a_gwei_stays_in_exact_wei() {
    assert_eq!(
        gas_price_range_texts(&[range(3_244, 5_000), range(1_937, 2_900)], EN),
        strings(&["3,244 ~ 5,000 wei", "1,937 ~ 2,900 wei"])
    );
}

#[test]
fn a_range_whose_ends_meet_is_one_figure_never_x_to_x() {
    assert_eq!(
        gas_price_range_texts(&[range(50_000_000, 50_000_000)], EN),
        strings(&["0.05 gwei"])
    );
    assert_eq!(
        gas_price_range_texts(
            &[range(50_000_000, 50_000_100), range(50_000_000, 50_000_000)],
            EN
        ),
        strings(&["0.05 gwei", "0.05 gwei"])
    );
    assert_eq!(
        gas_price_range_texts(&[range(10, 10)], EN),
        strings(&["10 wei"])
    );
}

/// A Linea-shaped cap a few wei over its bid is one price and votes as one.
#[test]
fn a_range_a_few_wei_wide_does_not_widen_the_set() {
    let bids = [51_234_567u128, 64_043_209];
    assert_eq!(
        gas_price_range_texts(
            &[range(bids[0], bids[0] + 3), range(bids[1], bids[1] + 3)],
            EN
        ),
        texts(&[Some(bids[0]), Some(bids[1])])
    );
    assert_eq!(
        texts(&[Some(bids[0]), Some(bids[1])]),
        strings(&["0.0512 gwei", "0.064 gwei"])
    );
    assert_eq!(
        gas_price_range_texts(
            &[
                range(51_234_567, 51_234_574),
                range(64_043_209, 128_086_411)
            ],
            EN
        ),
        strings(&["0.0512 gwei", "0.064 ~ 0.128 gwei"])
    );
}

#[test]
fn precision_widens_over_all_six_numbers() {
    assert_eq!(
        gas_price_range_texts(
            &[
                range(270_100_000_000, 270_400_000_000),
                range(250_000_000_000, 390_000_000_000)
            ],
            EN
        ),
        strings(&["270.1 ~ 270.4 gwei", "250 ~ 390 gwei"])
    );
}

#[test]
fn no_bid_is_nothing_and_a_bid_without_a_top_is_one_figure() {
    assert_eq!(
        gas_price_range_texts(
            &[
                None,
                Some(GasPriceRange {
                    low: 270_164_477_149,
                    high: None
                }),
                range(282_464_783_233, 527_288_291_674),
            ],
            EN
        ),
        vec![
            None,
            Some("270 gwei".to_owned()),
            Some("282 ~ 527 gwei".to_owned())
        ]
    );
}

#[test]
fn a_range_follows_the_persons_number_preset() {
    assert_eq!(
        gas_price_range_texts(&[range(750_000, 1_500_000)], NumberPreset::DotComma),
        strings(&["0,00075 ~ 0,0015 gwei"])
    );
    assert_eq!(
        gas_price_range_texts(&[range(1_937, 3_000)], NumberPreset::SpaceComma),
        strings(&["1 937 ~ 3 000 wei"])
    );
}

#[test]
fn gas_price_wei_takes_decimal_strings_and_refuses_anything_else() {
    assert_eq!(gas_price_wei(Some("270164477149")), Some(270_164_477_149));
    assert_eq!(gas_price_wei(Some("0")), Some(0));
    assert_eq!(gas_price_wei(None), None);
    assert_eq!(gas_price_wei(Some("")), None);
    assert_eq!(gas_price_wei(Some("0x10")), None);
    assert_eq!(gas_price_wei(Some("12.5")), None);
}

// ---------------------------------------------------------------------------
// What picking a speed buys (issue 686)
// ---------------------------------------------------------------------------

fn base() -> FeeEstimateView {
    FeeEstimateView {
        chain_id: 10,
        total_wei: "0".to_owned(),
        max_fee_per_gas: "0".to_owned(),
        network_fee_per_gas: "0".to_owned(),
        relayer_fee_per_gas: "0".to_owned(),
        bundler_gas_price: "0".to_owned(),
        in_band_gas_basis: "0".to_owned(),
        effective_gas_price: None,
        max_gas_price: None,
        total_gas: "0".to_owned(),
        deployed: true,
        tier: FeeTier::Fast,
        quoted: true,
        fee_asset: usdc("10000"),
        fee_recipient: Some("0xfee".to_owned()),
    }
}

fn usdc(amount: &str) -> FeeAssetView {
    FeeAssetView::Erc20 {
        token: "0xdead".to_owned(),
        decimals: 6,
        amount: amount.to_owned(),
        symbol: Some("USDC".to_owned()),
    }
}

fn quote(tier: FeeTier, patch: impl FnOnce(&mut FeeEstimateView)) -> FeeEstimateView {
    let mut quote = base();
    quote.tier = tier;
    patch(&mut quote);
    quote
}

fn ranged(tier: FeeTier, low: &str, high: &str) -> FeeEstimateView {
    quote(tier, |q| {
        q.effective_gas_price = Some(low.to_owned());
        q.max_gas_price = Some(high.to_owned());
    })
}

/// Tempo: the relay ignores the tier, so every speed is the same and draws no
/// gas price.
fn tempo() -> Vec<FeeEstimateView> {
    [FeeTier::Fast, FeeTier::Standard, FeeTier::Slow]
        .into_iter()
        .map(|tier| quote(tier, |q| q.max_fee_per_gas = "20000000000".to_owned()))
        .collect()
}

/// Optimism's real tiers, all clamped to the $0.01 floor — each its own range.
fn floor_clamped() -> Vec<FeeEstimateView> {
    vec![
        ranged(FeeTier::Fast, "3244", "9000"),
        ranged(FeeTier::Standard, "2377", "6000"),
        ranged(FeeTier::Slow, "1937", "4500"),
    ]
}

/// An ordinary chain: the faster speed is dearer.
fn priced() -> Vec<FeeEstimateView> {
    vec![
        quote(FeeTier::Fast, |q| {
            q.effective_gas_price = Some("3244".to_owned());
            q.max_gas_price = Some("9000".to_owned());
            q.total_wei = "30".to_owned();
        }),
        quote(FeeTier::Standard, |q| {
            q.effective_gas_price = Some("2377".to_owned());
            q.max_gas_price = Some("6000".to_owned());
            q.total_wei = "20".to_owned();
        }),
        quote(FeeTier::Slow, |q| {
            q.effective_gas_price = Some("1937".to_owned());
            q.max_gas_price = Some("4500".to_owned());
            q.fee_asset = usdc("9000");
        }),
    ]
}

fn evidence(quotes: &[FeeEstimateView]) -> Vec<(FeeTier, Option<&FeeEstimateView>)> {
    quotes.iter().map(|q| (q.tier, Some(q))).collect()
}

#[test]
fn b_one_speed_fires_on_tempos_shape() {
    assert!(one_speed(&evidence(&tempo())));
}

/// The owner's ruling: on a floor-clamped chain the range IS the choice.
#[test]
fn b_one_speed_never_fires_merely_because_the_fees_are_equal() {
    assert!(!one_speed(&evidence(&floor_clamped())));
}

#[test]
fn b_one_speed_never_fires_where_the_fees_differ() {
    assert!(!one_speed(&evidence(&priced())));
    let t = tempo();
    let dearer = quote(FeeTier::Slow, |q| q.total_wei = "1".to_owned());
    assert!(!one_speed(&[
        (t[0].tier, Some(&t[0])),
        (t[1].tier, Some(&t[1])),
        (FeeTier::Slow, Some(&dearer)),
    ]));
}

#[test]
fn b_one_speed_waits_for_every_speed() {
    let t = tempo();
    assert!(!one_speed(&[
        (t[0].tier, Some(&t[0])),
        (t[1].tier, Some(&t[1])),
        (FeeTier::Slow, None),
    ]));
    assert!(!one_speed(&[
        (t[0].tier, Some(&t[0])),
        (t[1].tier, Some(&t[1]))
    ]));
}

#[test]
fn b_the_charge_is_read_in_the_coin_not_just_the_wei() {
    let t = tempo();
    let coin = quote(FeeTier::Slow, |q| q.fee_asset = usdc("10001"));
    assert!(!indistinguishable(&[
        (t[0].tier, Some(&t[0])),
        (FeeTier::Slow, Some(&coin))
    ]));
}

#[test]
fn a_takes_the_fastest_for_a_slower_default_on_a_floor_clamped_chain() {
    let f = floor_clamped();
    assert!(speed_is_free(FeeTier::Slow, &evidence(&f)));
    assert!(speed_is_free(FeeTier::Standard, &evidence(&f)));
}

#[test]
fn a_does_nothing_for_somebody_already_on_the_fastest() {
    assert!(!speed_is_free(FeeTier::Fast, &evidence(&floor_clamped())));
}

#[test]
fn a_does_not_take_the_fastest_when_it_is_dearer() {
    let p = priced();
    assert!(!speed_is_free(FeeTier::Standard, &evidence(&p)));
    assert!(!speed_is_free(FeeTier::Slow, &evidence(&p)));
    // Same wei, dearer in the coin actually charged.
    let coin_slow = quote(FeeTier::Slow, |q| {
        q.effective_gas_price = Some("1937".to_owned());
        q.total_wei = "30".to_owned();
        q.fee_asset = usdc("9000");
    });
    assert!(!speed_is_free(
        FeeTier::Slow,
        &[
            (FeeTier::Fast, Some(&p[0])),
            (FeeTier::Slow, Some(&coin_slow))
        ]
    ));
    let same_coin = quote(FeeTier::Slow, |q| {
        q.effective_gas_price = Some("1937".to_owned());
        q.total_wei = "30".to_owned();
    });
    assert!(speed_is_free(
        FeeTier::Slow,
        &[
            (FeeTier::Fast, Some(&p[0])),
            (FeeTier::Slow, Some(&same_coin))
        ]
    ));
}

#[test]
fn a_holds_until_both_quotes_have_settled() {
    let f = floor_clamped();
    assert!(!speed_is_free(
        FeeTier::Slow,
        &[(FeeTier::Fast, Some(&f[0])), (FeeTier::Slow, None)]
    ));
    assert!(!speed_is_free(
        FeeTier::Slow,
        &[(FeeTier::Fast, None), (FeeTier::Slow, Some(&f[2]))]
    ));
    assert!(!speed_is_free(
        FeeTier::Slow,
        &[(FeeTier::Slow, Some(&f[2]))]
    ));
}

/// One wei apart: the same "0.01 USDC" on screen, but not the same charge.
#[test]
fn a_compares_exact_amounts_never_what_they_print_as() {
    let fast = quote(FeeTier::Fast, |q| {
        q.effective_gas_price = Some("3244".to_owned());
        q.total_wei = "1000000000000000001".to_owned();
    });
    let slow = quote(FeeTier::Slow, |q| {
        q.effective_gas_price = Some("1937".to_owned());
        q.total_wei = "1000000000000000000".to_owned();
    });
    assert!(!speed_is_free(
        FeeTier::Slow,
        &[(FeeTier::Fast, Some(&fast)), (FeeTier::Slow, Some(&slow))]
    ));
}

#[test]
fn b_before_a_no_upgrade_where_nothing_tells_the_speeds_apart() {
    let t = tempo();
    assert!(!speed_is_free(FeeTier::Slow, &evidence(&t)));
    assert!(!speed_is_free(
        FeeTier::Slow,
        &[(t[0].tier, Some(&t[0])), (t[2].tier, Some(&t[2]))]
    ));
}

#[test]
fn b_before_a_an_upgrade_when_fees_match_but_ranges_differ() {
    let f = floor_clamped();
    assert!(!one_speed(&evidence(&f)));
    assert!(speed_is_free(FeeTier::Slow, &evidence(&f)));
}

// ---------------------------------------------------------------------------
// The machine
// ---------------------------------------------------------------------------

type Sut = DomainDriver<FeeSpeed>;

/// Optimism, floor-clamped: every tier the same $0.01, each its own range.
fn settled(tier: FeeTier, total_wei: &str, with_range: bool) -> FeeEstimateView {
    let (low, high) = match tier {
        FeeTier::Fast => ("3244", "9000"),
        FeeTier::Standard => ("2377", "6000"),
        _ => ("1937", "4500"),
    };
    quote(tier, |q| {
        q.total_wei = total_wei.to_owned();
        q.fee_asset = FeeAssetView::Native;
        q.max_fee_per_gas = high.to_owned();
        if with_range {
            q.effective_gas_price = Some(low.to_owned());
            q.max_gas_price = Some(high.to_owned());
        }
    })
}

fn on_form(preferred: FeeTier) -> Sut {
    let mut sut = Sut::new();
    sut.dispatch(Event::Configure {
        preferred,
        number: EN,
    });
    sut.dispatch(Event::Reset);
    sut.dispatch(Event::StageChanged { on_form: true });
    sut
}

fn in_force(busy: bool, fee: Option<FeeEstimateView>) -> TierQuote {
    TierQuote { busy, fee }
}

fn preview(tier: FeeTier, busy: bool, fee: Option<FeeEstimateView>) -> TierPreviewQuote {
    TierPreviewQuote { tier, busy, fee }
}

fn report(sut: &mut Sut, main: TierQuote, previews: Vec<TierPreviewQuote>) -> FeeSpeedView {
    sut.dispatch(Event::QuotesChanged {
        chain_id: Some(10),
        in_force: main,
        previews,
    });
    sut.view()
}

/// The factory default costs nothing: no preview, no partner, one quote.
#[test]
fn the_fastest_default_prices_nothing_beside_itself() {
    let mut sut = on_form(FeeTier::Fast);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "10000", true))),
        vec![],
    );
    assert_eq!(view.tier, FeeTier::Fast);
    assert!(view.previews.is_empty(), "not one extra quote");
    assert!(!view.free && !view.picked);
}

#[test]
fn an_unconfigured_machine_runs_at_the_factory_default() {
    let sut = Sut::new();
    assert_eq!(sut.view().tier, FeeTier::Fast);
    assert_eq!(sut.view().preferred, FeeTier::Fast);
}

#[test]
fn a_slower_default_prices_the_fastest_beside_it_while_on_the_form() {
    let sut = on_form(FeeTier::Slow);
    let view = sut.view();
    assert_eq!(view.tier, FeeTier::Slow);
    assert_eq!(view.previews, vec![FeeTier::Fast]);
}

/// 686 A end to end: the fastest takes over, and the partner flips to the
/// default so the machine can give it back if the fees part later.
#[test]
fn a_slower_default_goes_fast_when_fast_costs_the_same() {
    let mut sut = on_form(FeeTier::Slow);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(
            FeeTier::Fast,
            false,
            Some(settled(FeeTier::Fast, "10000", true)),
        )],
    );
    assert_eq!(view.tier, FeeTier::Fast);
    assert!(view.free && view.free_note && !view.picked);
    assert_eq!(view.previews, vec![FeeTier::Slow]);

    // The shell promotes Fast's session; Slow is priced beside it again.
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "10000", true))),
        vec![preview(
            FeeTier::Slow,
            false,
            Some(settled(FeeTier::Slow, "10000", true)),
        )],
    );
    assert_eq!(view.tier, FeeTier::Fast, "stable once taken");

    // The fees part: the default comes back.
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "10001", true))),
        vec![preview(
            FeeTier::Slow,
            false,
            Some(settled(FeeTier::Slow, "10000", true)),
        )],
    );
    assert_eq!(view.tier, FeeTier::Slow);
    assert!(!view.free);
}

#[test]
fn the_default_stays_when_the_fastest_is_dearer() {
    let mut sut = on_form(FeeTier::Slow);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(
            FeeTier::Fast,
            false,
            Some(settled(FeeTier::Fast, "10001", true)),
        )],
    );
    assert_eq!(view.tier, FeeTier::Slow);
    assert!(!view.free);
}

#[test]
fn a_pick_is_honoured_even_when_a_faster_speed_is_free() {
    let mut sut = on_form(FeeTier::Standard);
    sut.dispatch(Event::Pick {
        tier: FeeTier::Slow,
    });
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![],
    );
    assert_eq!(view.tier, FeeTier::Slow);
    assert!(view.picked && !view.free);
    assert!(
        view.previews.is_empty(),
        "a pick is a decision, not a question"
    );
}

#[test]
fn no_upgrade_where_nothing_tells_the_speeds_apart() {
    let mut sut = on_form(FeeTier::Slow);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", false))),
        vec![preview(
            FeeTier::Fast,
            false,
            Some(settled(FeeTier::Fast, "10000", false)),
        )],
    );
    assert_eq!(view.tier, FeeTier::Slow);
    assert!(!view.free);
}

/// Slow is ticked, Fast still reads "…", and the person taps Slow. When Fast
/// later settles at the same fee, the decision stands.
#[test]
fn a_tap_on_the_default_while_fast_measures_is_a_decision() {
    let mut sut = on_form(FeeTier::Slow);
    report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(FeeTier::Fast, true, None)],
    );
    sut.dispatch(Event::Pick {
        tier: FeeTier::Slow,
    });
    assert!(sut.view().picked);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(
            FeeTier::Fast,
            false,
            Some(settled(FeeTier::Fast, "10000", true)),
        )],
    );
    assert_eq!(view.tier, FeeTier::Slow);
    assert!(!view.free);
    assert!(view.previews.is_empty(), "no question left to price");
}

#[test]
fn a_tap_on_the_ticked_fast_of_a_fast_default_is_no_pick() {
    let mut sut = on_form(FeeTier::Fast);
    sut.dispatch(Event::Pick {
        tier: FeeTier::Fast,
    });
    assert!(!sut.view().picked);
}

#[test]
fn nothing_is_asked_once_the_send_has_left_its_form() {
    let mut sut = on_form(FeeTier::Slow);
    sut.dispatch(Event::StageChanged { on_form: false });
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![],
    );
    assert!(view.previews.is_empty());
    assert_eq!(view.tier, FeeTier::Slow);
}

#[test]
fn an_upgrade_taken_on_the_form_holds_through_the_confirm() {
    let mut sut = on_form(FeeTier::Slow);
    report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(
            FeeTier::Fast,
            false,
            Some(settled(FeeTier::Fast, "10000", true)),
        )],
    );
    sut.dispatch(Event::StageChanged { on_form: false });
    let view = sut.view();
    assert_eq!(view.tier, FeeTier::Fast);
    assert!(view.previews.is_empty());
    assert!(view.free, "the confirm restates it with the reason");
}

#[test]
fn the_upgrade_holds_while_the_fastest_is_still_measuring() {
    let mut sut = on_form(FeeTier::Slow);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(FeeTier::Fast, true, None)],
    );
    assert_eq!(view.tier, FeeTier::Slow);
}

/// A measurement of the fee in force may be one the send core awaits; a tier
/// flip under it is not decided.
#[test]
fn nothing_flips_while_the_fee_in_force_is_measuring() {
    let mut sut = on_form(FeeTier::Slow);
    let view = report(
        &mut sut,
        in_force(true, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(
            FeeTier::Fast,
            false,
            Some(settled(FeeTier::Fast, "10000", true)),
        )],
    );
    assert_eq!(view.tier, FeeTier::Slow);
}

#[test]
fn opening_the_control_prices_every_other_tier_and_a_pick_folds_it() {
    let mut sut = on_form(FeeTier::Fast);
    sut.dispatch(Event::Toggle);
    let view = sut.view();
    assert!(view.open);
    assert_eq!(view.previews, vec![FeeTier::Standard, FeeTier::Slow]);
    sut.dispatch(Event::Pick {
        tier: FeeTier::Standard,
    });
    let view = sut.view();
    assert!(!view.open);
    assert_eq!(view.tier, FeeTier::Standard);
    assert!(view.picked);
    assert!(view.previews.is_empty());
}

#[test]
fn a_reset_forgets_the_pick_and_returns_to_the_default() {
    let mut sut = on_form(FeeTier::Standard);
    sut.dispatch(Event::Toggle);
    sut.dispatch(Event::Pick {
        tier: FeeTier::Fast,
    });
    sut.dispatch(Event::Reset);
    let view = sut.view();
    assert_eq!(view.tier, FeeTier::Standard);
    assert!(!view.picked && !view.open);
}

#[test]
fn rapid_is_never_offered_nor_in_force() {
    let mut sut = on_form(FeeTier::Rapid);
    assert_eq!(
        sut.view().tier,
        FeeTier::Fast,
        "a dead tier reads as the default"
    );
    sut.dispatch(Event::Pick {
        tier: FeeTier::Rapid,
    });
    let view = sut.view();
    assert_eq!(view.tier, FeeTier::Fast);
    assert!(!view.picked);
    assert!(view.options.iter().all(|o| o.tier != FeeTier::Rapid));
    assert_eq!(
        view.options.iter().map(|o| o.tier).collect::<Vec<_>>(),
        vec![FeeTier::Fast, FeeTier::Standard, FeeTier::Slow]
    );
}

// ---------------------------------------------------------------------------
// What the options say
// ---------------------------------------------------------------------------

fn open_with(preferred: FeeTier) -> Sut {
    let mut sut = on_form(preferred);
    sut.dispatch(Event::Toggle);
    sut
}

#[test]
fn an_option_with_no_session_yet_is_measuring_not_unpriceable() {
    let sut = open_with(FeeTier::Fast);
    let view = sut.view();
    let standard = &view.options[1];
    assert!(standard.fee.is_none());
    assert!(standard.measuring, "\"…\", never \"—\"");
    assert!(view.gas_price_line, "the line is reserved while measuring");
    assert!(view.options.iter().all(|o| o.gas_price.is_none()));
}

/// Issue 681: between a tier being named and its figure landing, the session
/// in force still holds the previous speed's money. That is never drawn under
/// the new name.
#[test]
fn another_tiers_figure_never_wears_this_tiers_name() {
    let mut sut = open_with(FeeTier::Fast);
    sut.dispatch(Event::Pick {
        tier: FeeTier::Slow,
    });
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "10000", true))),
        vec![],
    );
    let slow = view
        .options
        .iter()
        .find(|o| o.tier == FeeTier::Slow)
        .unwrap();
    assert!(slow.selected);
    assert!(slow.fee.is_none());
    assert!(slow.measuring);
}

#[test]
fn a_failed_tier_reads_as_unpriceable() {
    let mut sut = open_with(FeeTier::Fast);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "10000", true))),
        vec![
            preview(FeeTier::Standard, false, None),
            preview(
                FeeTier::Slow,
                false,
                Some(settled(FeeTier::Slow, "9000", true)),
            ),
        ],
    );
    let standard = &view.options[1];
    assert!(standard.fee.is_none() && !standard.measuring, "\"—\"");
    // A failed row is not "waiting": the others' gas prices are drawn.
    assert!(view.options[0].gas_price.is_some());
    assert!(view.options[1].gas_price.is_none());
}

#[test]
fn gas_prices_are_drawn_only_once_every_tier_has_answered() {
    let mut sut = open_with(FeeTier::Fast);
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "30", true))),
        vec![
            preview(
                FeeTier::Standard,
                false,
                Some(settled(FeeTier::Standard, "20", true)),
            ),
            preview(FeeTier::Slow, true, None),
        ],
    );
    assert!(view.options.iter().all(|o| o.gas_price.is_none()));
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "30", true))),
        vec![
            preview(
                FeeTier::Standard,
                false,
                Some(settled(FeeTier::Standard, "20", true)),
            ),
            preview(
                FeeTier::Slow,
                false,
                Some(settled(FeeTier::Slow, "10", true)),
            ),
        ],
    );
    assert_eq!(
        view.options
            .iter()
            .map(|o| o.gas_price.clone())
            .collect::<Vec<_>>(),
        strings(&[
            "3,244 ~ 9,000 wei",
            "2,377 ~ 6,000 wei",
            "1,937 ~ 4,500 wei"
        ])
    );
    assert!(!view.single);
}

/// A row holding its OWN settled figure through a refresh is not waiting: the
/// gas prices hold still, as the fees do.
#[test]
fn gas_prices_hold_still_through_a_refresh() {
    let mut sut = open_with(FeeTier::Fast);
    let settled_rows = |busy: bool| {
        (
            in_force(busy, Some(settled(FeeTier::Fast, "30", true))),
            vec![
                preview(
                    FeeTier::Standard,
                    busy,
                    Some(settled(FeeTier::Standard, "20", true)),
                ),
                preview(
                    FeeTier::Slow,
                    busy,
                    Some(settled(FeeTier::Slow, "10", true)),
                ),
            ],
        )
    };
    let (main, previews) = settled_rows(false);
    report(&mut sut, main, previews);
    let (main, previews) = settled_rows(true);
    let view = report(&mut sut, main, previews);
    assert!(view.options.iter().all(|o| o.gas_price.is_some()));
    assert!(view.options.iter().all(|o| !o.measuring));
}

/// 686 B on Tempo: the options give way to one statement, which a control
/// reopened on the same chain says at once, before its previews land.
#[test]
fn a_one_speed_network_says_so_and_remembers_it() {
    let mut sut = open_with(FeeTier::Fast);
    let tempo_rows = |fee_tier: FeeTier| settled(fee_tier, "10000", false);
    let view = report(
        &mut sut,
        in_force(false, Some(tempo_rows(FeeTier::Fast))),
        vec![
            preview(
                FeeTier::Standard,
                false,
                Some(tempo_rows(FeeTier::Standard)),
            ),
            preview(FeeTier::Slow, false, Some(tempo_rows(FeeTier::Slow))),
        ],
    );
    assert!(view.single);
    assert!(!view.gas_price_line, "no figure anywhere: the line goes");

    // Next send on the same chain: the previews are measuring again.
    sut.dispatch(Event::Reset);
    sut.dispatch(Event::StageChanged { on_form: true });
    sut.dispatch(Event::Toggle);
    let view = report(
        &mut sut,
        in_force(true, None),
        vec![
            preview(FeeTier::Standard, true, None),
            preview(FeeTier::Slow, true, None),
        ],
    );
    assert!(view.single, "remembered per chain, across sends");

    // A settled set that differs forgets it again.
    let view = report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Fast, "30", true))),
        vec![
            preview(
                FeeTier::Standard,
                false,
                Some(settled(FeeTier::Standard, "20", true)),
            ),
            preview(
                FeeTier::Slow,
                false,
                Some(settled(FeeTier::Slow, "10", true)),
            ),
        ],
    );
    assert!(!view.single);
    sut.dispatch(Event::Reset);
    sut.dispatch(Event::Toggle);
    let view = report(&mut sut, in_force(true, None), vec![]);
    assert!(!view.single);
}

/// A free upgrade over a network with one speed is not said: a speed that
/// buys nothing is not an upgrade.
#[test]
fn the_free_note_is_never_beside_the_one_speed_statement() {
    let mut sut = on_form(FeeTier::Slow);
    // An upgrade taken on a floor-clamped reading…
    report(
        &mut sut,
        in_force(false, Some(settled(FeeTier::Slow, "10000", true))),
        vec![preview(
            FeeTier::Fast,
            false,
            Some(settled(FeeTier::Fast, "10000", true)),
        )],
    );
    assert!(sut.view().free_note);
    // …then the chain reads as one speed.
    sut.dispatch(Event::Toggle);
    let flat = |tier: FeeTier| settled(tier, "10000", false);
    let view = report(
        &mut sut,
        in_force(false, Some(flat(FeeTier::Fast))),
        vec![
            preview(FeeTier::Standard, false, Some(flat(FeeTier::Standard))),
            preview(FeeTier::Slow, false, Some(flat(FeeTier::Slow))),
        ],
    );
    assert!(view.single);
    assert!(!view.free_note);
}

/// The widest ranges a picker can be asked to draw — five significant digits
/// on both ends of all three rows. The web's layout test draws exactly these
/// at 272px, so the strings are pinned here, where they are made.
#[test]
fn the_widest_ranges_read_at_five_digits() {
    assert_eq!(
        gas_price_range_texts(
            &[
                range(999_990, 2_999_870),
                range(999_950, 1_999_810),
                range(999_910, 1_499_780)
            ],
            EN
        ),
        strings(&[
            "0.00099999 ~ 0.0029999 gwei",
            "0.00099995 ~ 0.0019998 gwei",
            "0.00099991 ~ 0.0014998 gwei"
        ])
    );
}
