//! Machine — the speed control of one send surface (spec `069-native-fee-speed`).
//!
//! ```text
//!             Configure {preferred, number} ─┐
//!   Reset ─► (no pick, no upgrade, folded) ──┼─► view { tier, previews, options… }
//!   Toggle / Pick {tier} / StageChanged ─────┤          │
//!   QuotesChanged {in_force, previews} ──────┘          ▼
//!                                        the shell reconciles its fee sessions
//! ```
//!
//! Spec 068 shipped the speed control on the web, and every rule it needs was
//! written there in TypeScript: which tier is in force, when a slower default
//! quietly goes Fast because Fast costs no more (issue 686 A), when a network
//! has only one speed and says so (686 B), how a tier's gas bid reads as a
//! range (684/685), and which extra quotes a surface has to keep alive for all
//! of that. Porting those rules to three more shells would have meant four
//! copies of the subtlest code on the send screen, so they live here instead,
//! and every shell — the web included — drives this machine.
//!
//! # What stays in the shell
//!
//! The fee SESSIONS. Each tier is priced by its own `fee_policy` session, and
//! those are bridge objects the shell owns; a machine cannot hold another
//! machine. So the division is:
//!
//! - this machine DECIDES: the tier in force ([`FeeSpeedView::tier`]), which
//!   other tiers must be kept priced ([`FeeSpeedView::previews`]), and what the
//!   control says about every option;
//! - the shell DOES: it keeps one session per tier in `tier ∪ previews`, and
//!   reports every session's state back with [`Event::QuotesChanged`].
//!
//! The shell's reconcile rule is the whole of its half, and it is the same on
//! every platform:
//!
//! 1. When the fee session in force is not pricing [`FeeSpeedView::tier`],
//!    PROMOTE the preview session for that tier if it holds a settled quote of
//!    the same operation — the price somebody tapped is the price they get
//!    (issue 681) — and otherwise re-price the session in force at that tier
//!    once it is not measuring.
//! 2. Keep a preview session for every tier in [`FeeSpeedView::previews`],
//!    pricing the same operation as the session in force, re-priced whenever
//!    that one is (a new amount, a refresh); drop every other preview.
//! 3. After any session's view changes, dispatch [`Event::QuotesChanged`].
//!
//! # What this machine never does
//!
//! It never prices anything, and it never derives one tier's fee from
//! another's. Every figure it hands back is a quote the core settled FOR THAT
//! TIER, echoed as it was reported — the reported tier price and the relay's
//! submit cap are different quantities (spec 068, "Money and speed are two
//! different levers"), so arithmetic across tiers would be wrong as well as a
//! second writer of the same number.
//!
//! It never touches the stored preference either. A pick here is one-shot:
//! only Settings dispatches `fee_tier_pref`'s `UserChose`, and this machine
//! only ever reads the preference it is handed in [`Event::Configure`].

use std::collections::BTreeSet;

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::fee_policy::{FeeEstimateView, FeeTier};
use super::fee_tier_pref::{FACTORY_DEFAULT, OFFERED};
use crate::l10n::number::{group_digits, NumberPreset};

// ---------------------------------------------------------------------------
// The tiers
// ---------------------------------------------------------------------------

/// The fastest speed anybody is offered — what a free upgrade goes to.
pub const FASTEST: FeeTier = FeeTier::Fast;

/// A tier as one this build may offer. Only the dead `rapid` can fail (spec
/// 068 keeps the variant and never offers it), and it lands on the factory
/// default — the same answer `fee_tier_pref` gives a stored name it does not
/// recognise, and never a quiet downgrade.
pub fn offered(tier: FeeTier) -> FeeTier {
    if OFFERED.contains(&tier) {
        tier
    } else {
        FACTORY_DEFAULT
    }
}

/// The tier a submission names on the wire, or `None` to name nothing.
///
/// The relay accepts the three offered names as `eth_sendUserOperation`'s
/// optional third parameter and refuses anything else with -32602 before any
/// handler runs, so the dead `rapid` must never reach it: a quote that somehow
/// carried it names nothing (the pre-068 wire, exactly) rather than a
/// neighbouring tier it was not priced at.
pub fn wire_tier(tier: FeeTier) -> Option<FeeTier> {
    OFFERED.contains(&tier).then_some(tier)
}

// ---------------------------------------------------------------------------
// What a speed buys, as text (issues 684 / 685)
// ---------------------------------------------------------------------------

/// 0.001 gwei. A set whose largest price is at or above it reads in gwei;
/// otherwise it reads in wei.
const GWEI_BRANCH_FLOOR_WEI: u128 = 1_000_000;

const WEI_PER_GWEI: u128 = 1_000_000_000;

/// Where the gwei branch starts, and how far it may widen.
///
/// Three is what reads: `300 gwei`, `0.05 gwei`, `0.005 gwei`. Five is a
/// deliberate give-up point: two tiers that still print alike at five digits
/// differ by less than one part in ten thousand, which no choice of speed
/// turns on — chasing them further is how a quiet `0.05 gwei` became
/// `0.0500001 gwei`.
const MIN_SIGNIFICANT: usize = 3;
const MAX_SIGNIFICANT: usize = 5;

/// One tier's gas price: what it bids now (`low`, the core's
/// `effective_gas_price`) and how high it will go (`high`, its
/// `max_gas_price` — the cap). `high` is `None` when no top was published, and
/// the tier then reads as the single figure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GasPriceRange {
    pub low: u128,
    pub high: Option<u128>,
}

/// A decimal wei string from the core as a number, or `None`.
///
/// `None` for anything that is not plain decimal digits — and never `0` as a
/// stand-in: "the core published nothing" and "the price is zero" are
/// different facts, and only the second is a number the picker may draw. A
/// figure beyond `u128` (no chain's gas price is within fifteen orders of
/// magnitude of it) is treated the same way rather than truncated.
pub fn gas_price_wei(value: Option<&str>) -> Option<u128> {
    let value = value?;
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

/// A settled quote's gas price as a range. Both ends are the core's own
/// (`fee_policy::gas_price_range`) and are only read here.
///
/// No bid, no figure: the top of a range is not a gas price anybody offers,
/// and alone it would read as the price. A top that is missing, or that sits
/// BELOW the bid, is no top — the bid is `min(cap, base + tip)`, so it cannot
/// exceed the cap, and a range drawn upside down is a claim nothing measured.
pub fn gas_price_range_of(quote: Option<&FeeEstimateView>) -> Option<GasPriceRange> {
    let quote = quote?;
    let low = gas_price_wei(quote.effective_gas_price.as_deref())?;
    let high = gas_price_wei(quote.max_gas_price.as_deref()).filter(|high| *high >= low);
    Some(GasPriceRange { low, high })
}

/// The gas-price ranges of a set of tiers, as text, in the order given:
/// `0.02011 ~ 0.03016 gwei`, or `0.005 gwei` where the two ends print alike.
///
/// **The set is formatted together, which is why there is no single-value
/// entry point.** The figure's one job is to tell tiers apart, so:
///
/// - ONE unit for the set, chosen by its largest number: if any end of any
///   range reaches 0.001 gwei, everything reads in gwei; otherwise everything
///   reads in exact wei. Rows in two units cannot be compared at a glance, and
///   a range written `3,000 wei ~ 0.005 gwei` cannot be read as a range at all.
/// - ONE precision for the set: three significant digits, widened (to five at
///   most) only while two genuinely different numbers would print alike.
/// - A range whose ends print alike is written as the one figure — never
///   `x ~ x` — and votes on the precision as one figure.
///
/// `gwei` and `wei` are proper nouns and are not translated; only the digits
/// and the separators follow the person's number preset. `~` because that is
/// how the owner writes a range; it is always spaced and always BETWEEN two
/// numbers, so it never meets the `~x` that means "about x" elsewhere.
///
/// `None` in, `None` out: a chain that reports no priority fee at all (Tempo)
/// has no honest number here, and a fabricated `0` would claim the tiers are
/// equal when nothing measured them.
pub fn gas_price_range_texts(
    ranges: &[Option<GasPriceRange>],
    preset: NumberPreset,
) -> Vec<Option<String>> {
    let present: Vec<GasPriceRange> = ranges.iter().flatten().copied().collect();
    let in_gwei = present.iter().any(|range| {
        range.low >= GWEI_BRANCH_FLOOR_WEI
            || range.high.is_some_and(|high| high >= GWEI_BRANCH_FLOOR_WEI)
    });
    // A range whose ends print alike even at the widest precision is ONE
    // price (a Linea-shaped chain puts its cap a few wei over its bid). It is
    // written as that figure, and it votes on the precision as one figure —
    // its own ends "colliding" would otherwise widen all six numbers for a
    // difference no precision here can show. In wei every integer prints
    // exactly, so only truly equal ends are one price.
    let top_of = |range: &GasPriceRange| -> Option<u128> {
        let high = range.high?;
        if in_gwei
            && gwei_digits(high, MAX_SIGNIFICANT, preset)
                == gwei_digits(range.low, MAX_SIGNIFICANT, preset)
        {
            return None;
        }
        Some(high)
    };
    let known: Vec<u128> = present
        .iter()
        .flat_map(|range| std::iter::once(range.low).chain(top_of(range)))
        .collect();
    let significant = if in_gwei {
        significant_digits(&known, preset)
    } else {
        0
    };
    let digits = |wei: u128| -> String {
        if in_gwei {
            gwei_digits(wei, significant, preset)
        } else {
            group_digits(&wei.to_string(), preset)
        }
    };
    let unit = if in_gwei { "gwei" } else { "wei" };
    ranges
        .iter()
        .map(|range| {
            let range = range.as_ref()?;
            let low = digits(range.low);
            let high = top_of(range).map_or_else(|| low.clone(), digits);
            Some(if high == low {
                format!("{low} {unit}")
            } else {
                format!("{low} ~ {high} {unit}")
            })
        })
        .collect()
}

/// The fewest significant digits (three to five) that keep every distinct
/// price distinct. Equal prices print equally, which is the truth and not a
/// collision: on an OP-stack L2 the market tip really is 0.
fn significant_digits(values: &[u128], preset: NumberPreset) -> usize {
    for significant in MIN_SIGNIFICANT..MAX_SIGNIFICANT {
        let mut seen: Vec<(String, u128)> = Vec::with_capacity(values.len());
        let collided = values.iter().any(|&wei| {
            let text = gwei_digits(wei, significant, preset);
            let clash = seen
                .iter()
                .any(|(other_text, other)| *other_text == text && *other != wei);
            seen.push((text, wei));
            clash
        });
        if !collided {
            return significant;
        }
    }
    MAX_SIGNIFICANT
}

/// A wei amount as gwei digits, without the unit, in integer arithmetic only.
fn gwei_digits(wei: u128, significant: usize, preset: NumberPreset) -> String {
    let value = round_to_significant(wei, significant);
    let whole = group_digits(&(value / WEI_PER_GWEI).to_string(), preset);
    let fraction = format!("{:09}", value % WEI_PER_GWEI);
    let fraction = fraction.trim_end_matches('0');
    if fraction.is_empty() {
        whole
    } else {
        format!("{whole}{}{fraction}", preset.separators().decimal)
    }
}

/// Round half up to `significant` digits — to NEAREST, not truncated the way
/// a fee is. A fee is a charge and rounding it down is the safe direction; a
/// gas price is a rate nobody is billed for, so nearest is the only unbiased
/// answer.
fn round_to_significant(wei: u128, significant: usize) -> u128 {
    let digits = wei.to_string().len();
    if digits <= significant {
        return wei;
    }
    let Ok(exponent) = u32::try_from(digits - significant) else {
        return wei;
    };
    let scale = 10u128.pow(exponent);
    let quotient = wei / scale;
    let remainder = wei % scale;
    // `remainder * 2 >= scale`, without the doubling that could overflow.
    let rounded = if remainder >= scale - remainder {
        quotient + 1
    } else {
        quotient
    };
    rounded.saturating_mul(scale)
}

// ---------------------------------------------------------------------------
// What picking a speed buys on this network (issue 686)
// ---------------------------------------------------------------------------

/// Two quotes that charge the person exactly the same thing: the same chain,
/// the same wei, in the same coin (and the same token amount). Compared as the
/// core's own decimal strings, which are exact — nothing here rounds.
fn same_charge(a: &FeeEstimateView, b: &FeeEstimateView) -> bool {
    a.chain_id == b.chain_id && a.total_wei == b.total_wei && a.fee_asset == b.fee_asset
}

fn has_gas_price(quote: &FeeEstimateView) -> bool {
    gas_price_wei(quote.effective_gas_price.as_deref()).is_some()
}

/// Nothing the screen can show tells these tiers apart: every one settled, all
/// charging the same, and none with a gas-price range. `false` for fewer than
/// two rows and for any row without a settled quote — a `None` is "no
/// evidence", never "equal".
pub fn indistinguishable(rows: &[(FeeTier, Option<&FeeEstimateView>)]) -> bool {
    if rows.len() < 2 {
        return false;
    }
    let Some(first) = rows[0].1 else {
        return false;
    };
    rows.iter().all(|(_, quote)| {
        quote.is_some_and(|quote| same_charge(quote, first) && !has_gas_price(quote))
    })
}

/// B: this network has one speed. Only once EVERY offered tier has settled,
/// so the statement never arrives early over a row that is about to differ.
///
/// Equal fees alone are NOT this: on a chain clamped to the $0.01 floor the
/// ranges still differ, and that difference is exactly what the choice buys.
pub fn one_speed(rows: &[(FeeTier, Option<&FeeEstimateView>)]) -> bool {
    if !OFFERED
        .iter()
        .all(|tier| rows.iter().any(|(row_tier, _)| row_tier == tier))
    {
        return false;
    }
    let offered_rows: Vec<(FeeTier, Option<&FeeEstimateView>)> = rows
        .iter()
        .filter(|(tier, _)| OFFERED.contains(tier))
        .copied()
        .collect();
    indistinguishable(&offered_rows)
}

/// A: the fastest speed costs exactly what `preferred` costs, and is actually
/// different from it, so this send should go at the fastest speed.
///
/// Both quotes must be in `rows` and settled; with either missing the answer
/// is `false` — hold, do not guess. **B before A**: tiers nothing tells apart
/// are not an upgrade, and a line saying "Fast costs no more, so this send
/// goes Fast" over a speed that buys nothing would be a small lie.
pub fn speed_is_free(preferred: FeeTier, rows: &[(FeeTier, Option<&FeeEstimateView>)]) -> bool {
    if preferred == FASTEST {
        return false;
    }
    let find = |tier: FeeTier| {
        rows.iter()
            .find(|(row_tier, _)| *row_tier == tier)
            .and_then(|(_, quote)| *quote)
    };
    let (Some(mine), Some(fastest)) = (find(preferred), find(FASTEST)) else {
        return false;
    };
    if indistinguishable(&[(preferred, Some(mine)), (FASTEST, Some(fastest))]) {
        return false;
    }
    same_charge(mine, fastest)
}

/// The one tier a free upgrade is judged against, or `None` when there is no
/// such question: the fastest while the send runs at the person's default,
/// their default while it runs upgraded.
///
/// `None` — so not one extra quote — for everybody whose default is already
/// the fastest (the factory default, so most people); `None` the moment the
/// person picks a tier on this send, because an upgrade may never overrule a
/// decision; and `None` once the send has left its form, because the confirm
/// is the last screen before a signature and the tier it names must not change
/// under somebody reading it.
pub fn free_speed_partner(
    picked: Option<FeeTier>,
    preferred: FeeTier,
    in_force: FeeTier,
    on_form: bool,
) -> Option<FeeTier> {
    if !on_form || picked.is_some() || preferred == FASTEST {
        return None;
    }
    Some(if in_force == FASTEST {
        preferred
    } else {
        FASTEST
    })
}

/// What this send's one-shot pick becomes when the person taps the tier that
/// is ALREADY in force.
///
/// While a free upgrade is in question (`partner` set), that tap is a
/// decision like any other: left unrecorded, a Slow tapped while Fast was
/// still being measured would be overruled a moment later when Fast settled at
/// the same fee. With no such question the tap changes nothing — tapping the
/// option already ticked is not a new decision, and the confirm keeps saying
/// nothing about a speed nobody moved.
pub fn pick_in_force(
    picked: Option<FeeTier>,
    partner: Option<FeeTier>,
    tapped: FeeTier,
) -> Option<FeeTier> {
    if picked.is_some() {
        return picked;
    }
    partner.map(|_| tapped)
}

/// The tiers to keep priced beside the one in force: every other offered tier
/// while the control is open (each option shows its own fee), only the free
/// partner while it is folded, and nothing otherwise.
pub fn preview_tiers(open: bool, in_force: FeeTier, partner: Option<FeeTier>) -> Vec<FeeTier> {
    if open {
        return OFFERED
            .iter()
            .copied()
            .filter(|tier| *tier != in_force)
            .collect();
    }
    partner.into_iter().collect()
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// This machine asks the platform for nothing: the platform reconciles its fee
/// sessions against the view (see the module docs). The type exists because
/// every machine is exported through the same bridge, whose contract is
/// "operations in, results out" — an empty one keeps that contract honest
/// rather than special-casing a machine.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeeSpeedOperation"))]
pub enum FeeSpeedOperation {}

/// See [`FeeSpeedOperation`]: there is nothing to answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeeSpeedShellResult"))]
pub enum FeeSpeedShellResult {}

impl Operation for FeeSpeedOperation {
    type Output = FeeSpeedShellResult;
}

#[effect]
pub enum FeeSpeedEffect {
    Render(RenderOperation),
    Shell(FeeSpeedOperation),
}

/// One fee session's state, as the shell holds it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TierQuote {
    /// A measurement is out: the session's own `busy`, OR'd with anything the
    /// shell does before the core hears the question (reading whether the
    /// account is deployed, say). Both halves of "measuring" belong here, or a
    /// row sits still showing the previous tier's money for a round trip.
    pub busy: bool,
    /// The session's settled quote, of whatever tier it last priced — this
    /// machine judges the tier, so the shell never filters.
    pub fee: Option<FeeEstimateView>,
}

/// A preview session: the tier it was asked to price, and its state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct TierPreviewQuote {
    pub tier: FeeTier,
    pub busy: bool,
    pub fee: Option<FeeEstimateView>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeeSpeedEvent"))]
// `QuotesChanged` carries whole quotes, as `send`'s shell results do (see
// `SendShellResult`): a wire type whose JSON is pinned by the generated TS,
// where a box would buy an allocation per report and change nothing a shell
// can see.
#[allow(clippy::large_enum_variant)]
pub enum Event {
    /// The person's stored default (`fee_tier_pref`'s view) and number preset.
    /// Dispatch on mount and whenever either changes; repeating it is free.
    Configure {
        preferred: FeeTier,
        number: NumberPreset,
    },
    /// A send starts, or ends. Forgets the one-shot pick, a free upgrade, the
    /// fold and every quote — a new send starts at the stored default, which is
    /// the promise the picker makes. Remembers which networks have one speed:
    /// that does not change between payments.
    Reset,
    /// The send is on its form (`true`) or has moved past it (`false`). A free
    /// upgrade is only ever decided on the form.
    StageChanged { on_form: bool },
    /// The folded control was opened or closed.
    Toggle,
    /// The person tapped an option. One-shot: it prices and submits this send
    /// and never reaches the stored preference. Folds the control.
    Pick { tier: FeeTier },
    /// Every fee session's state, whole, after any of them changed.
    ///
    /// `chain_id` is the chain the session in force was last ASKED about —
    /// known before its answer lands, which is what lets a control reopened on
    /// a one-speed network say so at once. `in_force` is that session, of
    /// whatever tier it last priced; `previews` is every other live session,
    /// by the tier it was asked for.
    QuotesChanged {
        chain_id: Option<u32>,
        in_force: TierQuote,
        previews: Vec<TierPreviewQuote>,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Model {
    /// `None` until configured: the factory default stands in.
    preferred: Option<FeeTier>,
    number: NumberPreset,
    picked: Option<FeeTier>,
    /// This send runs at the fastest speed because it is free here (issue 686
    /// A). Nobody chose it, so it never outranks a pick.
    upgraded: bool,
    open: bool,
    on_form: bool,
    chain_id: Option<u32>,
    in_force: TierQuote,
    previews: Vec<TierPreviewQuote>,
    /// Chains whose speeds last settled as one (686 B), so the control
    /// reopened there says so at once instead of drawing three rows that
    /// collapse a round trip later. Only ever what the numbers last said.
    one_speed_chains: BTreeSet<u32>,
}

impl Model {
    fn preferred(&self) -> FeeTier {
        offered(self.preferred.unwrap_or(FACTORY_DEFAULT))
    }

    /// A pick, else the fastest when it is free, else the stored default.
    fn tier(&self) -> FeeTier {
        if let Some(picked) = self.picked {
            return picked;
        }
        let preferred = self.preferred();
        if self.upgraded && preferred != FASTEST {
            FASTEST
        } else {
            preferred
        }
    }

    fn partner(&self) -> Option<FeeTier> {
        free_speed_partner(self.picked, self.preferred(), self.tier(), self.on_form)
    }

    /// The session pricing `tier` — the one in force for the tier in force,
    /// else its preview. A tier with no session yet is being measured: "…",
    /// never "—", which would claim it cannot be priced.
    fn row(&self, tier: FeeTier) -> TierQuote {
        if tier == self.tier() {
            return self.in_force.clone();
        }
        self.previews
            .iter()
            .find(|preview| preview.tier == tier)
            .map_or(
                TierQuote {
                    busy: true,
                    fee: None,
                },
                |preview| TierQuote {
                    busy: preview.busy,
                    fee: preview.fee.clone(),
                },
            )
    }

    /// Every offered tier's session, fastest first.
    fn rows(&self) -> Vec<(FeeTier, TierQuote)> {
        OFFERED
            .iter()
            .map(|tier| (*tier, self.row(*tier)))
            .collect()
    }
}

/// A row's quote — priced at ITS OWN tier, or nothing (issue 681). Between a
/// tier being named and its figure landing, the session in force still holds
/// the previous speed's number, which is the one thing never drawn under a
/// different name.
fn own_quote(tier: FeeTier, row: &TierQuote) -> Option<&FeeEstimateView> {
    row.fee.as_ref().filter(|fee| fee.tier == tier)
}

fn own_quotes(rows: &[(FeeTier, TierQuote)]) -> Vec<(FeeTier, Option<&FeeEstimateView>)> {
    rows.iter()
        .map(|(tier, row)| (*tier, own_quote(*tier, row)))
        .collect()
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// One option of the control, fastest first.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeSpeedOptionView {
    pub tier: FeeTier,
    /// The tier this send runs at.
    pub selected: bool,
    /// This tier's OWN settled quote, echoed as the shell reported it — the
    /// fee the option shows, formatted by the shell exactly as its fee row
    /// formats one (with the fee-coin options of the session pricing it).
    pub fee: Option<FeeEstimateView>,
    /// No figure of its own, but one is on its way: draw "…". With neither a
    /// figure nor this, draw "—" — the tier could not be priced.
    pub measuring: bool,
    /// What this speed buys: its gas bid as a range (`0.02011 ~ 0.03016 gwei`),
    /// already formatted over the whole set. `None` while any tier is still
    /// being measured (a partial set would reshape as the rest arrived), and
    /// wherever there is no honest number.
    pub gas_price: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeSpeedView {
    /// The tier THIS send runs at: a one-shot pick, else the fastest when it
    /// is free here, else the stored default. The session in force must be
    /// pricing it, and the submission names it.
    pub tier: FeeTier,
    /// The person's stored default, as offered.
    pub preferred: FeeTier,
    /// The tiers, besides [`Self::tier`], the shell must keep priced — all of
    /// them while the control is open, the free-upgrade partner while it is
    /// folded, and none for somebody whose default is already the fastest.
    pub previews: Vec<FeeTier>,
    pub open: bool,
    /// [`Self::tier`] is a deliberate pick on this send. The confirm restates
    /// the speed only for a pick or a free upgrade.
    pub picked: bool,
    /// [`Self::tier`] is the fastest because on this network it costs exactly
    /// what the person's default costs (issue 686 A) — not a pick, and never
    /// written back to the preference. The confirm restates it WITH the
    /// reason (`send.feeSpeedFree`).
    pub free: bool,
    /// Say `send.feeSpeedFree` under the folded summary: [`Self::free`], and
    /// the network has more than one speed.
    pub free_note: bool,
    /// This network has one speed (686 B): while open, the options give way to
    /// `send.feeSpeedSingle`.
    pub single: bool,
    /// Whether the options carry a gas-price line at all — reserved while
    /// anything is still measuring, so rows keep one height; dropped only once
    /// the set settled with no figure anywhere (Tempo).
    pub gas_price_line: bool,
    pub options: Vec<FeeSpeedOptionView>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FeeSpeed;

impl App for FeeSpeed {
    type Event = Event;
    type Model = Model;
    type ViewModel = FeeSpeedView;
    type Effect = FeeSpeedEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<FeeSpeedEffect, Event> {
        match event {
            Event::Configure { preferred, number } => {
                model.preferred = Some(offered(preferred));
                model.number = number;
            }
            Event::Reset => {
                model.picked = None;
                model.upgraded = false;
                model.open = false;
                model.on_form = false;
                model.chain_id = None;
                model.in_force = TierQuote::default();
                model.previews.clear();
            }
            Event::StageChanged { on_form } => model.on_form = on_form,
            Event::Toggle => model.open = !model.open,
            Event::Pick { tier } => {
                if !OFFERED.contains(&tier) {
                    // `rapid` has no option to tap. A shell that sent it is a
                    // bug, and a no-op is safer than a tier the relay refuses.
                    return Command::done();
                }
                model.open = false;
                model.picked = if tier == model.tier() {
                    pick_in_force(model.picked, model.partner(), tier)
                } else {
                    Some(tier)
                };
            }
            Event::QuotesChanged {
                chain_id,
                in_force,
                previews,
            } => {
                model.chain_id = chain_id;
                model.in_force = in_force;
                model.previews = previews;
                remember_verdict(model);
            }
        }
        take_free_speed(model);
        render()
    }

    fn view(&self, model: &Model) -> FeeSpeedView {
        let tier = model.tier();
        let rows = model.rows();
        let own = own_quotes(&rows);
        let measuring: Vec<bool> = rows
            .iter()
            .zip(&own)
            .map(|((_, row), (_, quote))| quote.is_none() && (row.busy || row.fee.is_some()))
            .collect();
        // Nothing is formatted until every row has answered: the tiers land
        // one at a time, and a set formatted as they came would reshape
        // ("270 gwei" → "270.2 gwei") when a neighbour arrived. A row still
        // holding its OWN settled figure through a refresh is not waiting, so
        // figures hold still through a refresh as the fees do.
        let waiting = measuring.iter().any(|m| *m);
        let gas_prices = if waiting {
            vec![None; rows.len()]
        } else {
            let ranges: Vec<Option<GasPriceRange>> = own
                .iter()
                .map(|(_, quote)| gas_price_range_of(*quote))
                .collect();
            gas_price_range_texts(&ranges, model.number)
        };
        // Judged under the same hold, and held through a refresh, so the
        // statement does not blink back into three rows on every ⟳. While the
        // set is being measured, what this chain last settled as stands in.
        let single = if waiting {
            model
                .chain_id
                .is_some_and(|chain| model.one_speed_chains.contains(&chain))
        } else {
            one_speed(&own)
        };
        let free = model.picked.is_none() && tier != model.preferred();
        let gas_price_line = waiting || gas_prices.iter().any(Option::is_some);
        let options = own
            .iter()
            .zip(measuring)
            .zip(gas_prices)
            .map(
                |(((option, quote), measuring), gas_price)| FeeSpeedOptionView {
                    tier: *option,
                    selected: *option == tier,
                    fee: quote.cloned(),
                    measuring,
                    gas_price,
                },
            )
            .collect();
        FeeSpeedView {
            tier,
            preferred: model.preferred(),
            previews: preview_tiers(model.open, tier, model.partner()),
            open: model.open,
            picked: model.picked.is_some(),
            free,
            free_note: free && !single,
            single,
            gas_price_line,
            options,
        }
    }
}

/// Remember (or forget) that the chain in force has one speed — only ever
/// from a fully SETTLED set. A set still measuring, failed, or holding another
/// tier's figure is no evidence either way.
fn remember_verdict(model: &mut Model) {
    let rows = model.rows();
    let own = own_quotes(&rows);
    if own.iter().any(|(_, quote)| quote.is_none()) {
        return;
    }
    let Some(chain) = model.in_force.fee.as_ref().map(|fee| fee.chain_id) else {
        return;
    };
    let one = one_speed(&own);
    if one {
        model.one_speed_chains.insert(chain);
    } else {
        model.one_speed_chains.remove(&chain);
    }
}

/// Take the fastest speed when it is free, and give it back when it is not
/// (issue 686 A) — decided from two SETTLED quotes of this send: the one in
/// force and its partner's.
///
/// Never while a measurement of the fee in force is out: it may be one the
/// `send` core is waiting on, and a tier flip under it would make the shell
/// re-price over a question the core asked. The flip changes only
/// [`FeeSpeedView::tier`]; the shell then promotes the partner's session,
/// which already priced this operation at that tier, so what the row shows,
/// what the send machine signs and the tier on the wire stay one quote.
fn take_free_speed(model: &mut Model) {
    let in_force = model.tier();
    let Some(partner) = model.partner() else {
        return;
    };
    if model.in_force.busy {
        return;
    }
    let Some(mine) = own_quote(in_force, &model.in_force) else {
        return;
    };
    let Some(theirs) = model
        .previews
        .iter()
        .find(|preview| preview.tier == partner && !preview.busy)
        .and_then(|preview| preview.fee.as_ref())
        .filter(|fee| fee.tier == partner)
    else {
        return;
    };
    let free = speed_is_free(
        model.preferred(),
        &[(in_force, Some(mine)), (partner, Some(theirs))],
    );
    if free != (in_force == FASTEST) {
        model.upgraded = free;
    }
}

impl super::SplitEffect for FeeSpeedEffect {
    type Op = FeeSpeedOperation;
    fn into_shell(self) -> Option<crux_core::Request<FeeSpeedOperation>> {
        match self {
            FeeSpeedEffect::Render(_) => None,
            FeeSpeedEffect::Shell(request) => Some(request),
        }
    }
}
