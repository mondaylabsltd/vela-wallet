//! The signing sheet, built from what four machines decided.
//!
//! The **sibling** of `fixtures.rs`, never its replacement: both produce a
//! `SigningModel`, and the panel picks which one feeds it. That is what keeps
//! the 33 drawn scenarios reviewable after real requests arrive, and what
//! makes "the gallery is unchanged" something a diff can prove.
//!
//! Nothing here decides. The intent sentence, the risk grade, which fields
//! exist and whether a confirm may fire are `clear_signing`'s,
//! `approval_guard`'s, `fee_policy`'s and `sign_request`'s answers; this maps
//! them onto blocks and picks the words the corpus already has.

use gpui::SharedString;

use vela_core::app::approval_guard::{GuardAmountError, GuardEditorMode, GuardSurface, GuardView};
use vela_core::app::clear_signing::{
    ClearBlindTyped, ClearDangerClass, ClearMessageView, ClearRisk, ClearSignField,
    ClearSignResult, ClearSigningView, ClearSiweBinding, ClearSurface, UNKNOWN_AMOUNT,
};
use vela_core::app::fee_policy::{FeeTier, FeeView};
use vela_core::app::sign_request::{SignErrorKind, SignFundingPresentation, SignSurface, SignView};

use crate::signing::fixtures::{AllowanceInput, Block, ChipState, FeeModel, FeeTokenOption};
use crate::signing::{SigningStrings, Tone};

/// The core's risk grade in the drawn vocabulary.
fn tone_of(risk: ClearRisk) -> Tone {
    match risk {
        ClearRisk::Safe => Tone::Success,
        ClearRisk::Normal => Tone::Neutral,
        ClearRisk::Caution => Tone::Caution,
        ClearRisk::Danger => Tone::Danger,
    }
}

/// May the slide fire?
///
/// **Three machines, ANDed**, and the core's own doc says so: `SignView`'s
/// `confirm_gate_open` is "this machine's own approval gate" and "the shell
/// must AND it with `GuardView.confirm_allowed` and
/// `FeeView.confirm_fee_ready`". Taking any one of them alone arms a slide
/// over an unpriced fee, or over an unlimited approval nobody capped — each
/// of which is a signature the person did not agree to.
///
/// **The fee's say includes its speed** (spec 069): between a tap and that
/// speed's own figure landing, the core's `confirm_fee_ready` is still true on
/// the speed just left, and the slide must not sign it. `speed_tier` is the
/// tier in force; `None` is a sheet with no speed control.
///
/// **A signature has no fee to wait for** (the phones' rule, `SigningLive`):
/// nothing is quoted for a message, so a fee gate over one is a slide that
/// never opens.
#[must_use]
pub fn confirm_enabled(
    sign: &SignView,
    guard: &GuardView,
    clear: &ClearSigningView,
    fee: &FeeView,
    speed_tier: Option<FeeTier>,
) -> bool {
    let fee_ready =
        off_chain(clear) || (fee.confirm_fee_ready && !fee_of_another_tier(fee, speed_tier));
    sign.confirm_gate_open && guard.confirm_allowed && fee_ready
}

/// NEVER ANOTHER TIER'S FIGURE WEARING THIS TIER'S NAME (issue 681).
#[must_use]
pub fn fee_of_another_tier(fee: &FeeView, speed_tier: Option<FeeTier>) -> bool {
    let (Some(estimate), Some(tier)) = (&fee.fee, speed_tier) else {
        return false;
    };
    vela_core::app::fee_speed::offered(estimate.tier) != vela_core::app::fee_speed::offered(tier)
}

/// What the shell knows about the request that the core does not hand back.
///
/// Only reached on the blind rung, where by definition nothing was decoded:
/// what is still TRUE then is who it goes to and how many bytes nobody could
/// read. Parsed once, by the host, from the same params the machines were told
/// about — re-parsing it here would be a second reading of an untrusted
/// payload, and two readings can disagree.
#[derive(Clone, Default)]
pub struct RequestFacts {
    pub to: Option<String>,
    pub data_bytes: usize,
}

/// The sheet's body, dispatched by the core's own `ClearSurface`.
///
/// **This is the core's dispatch, not the shell's.** Reading `result.is_some()`
/// instead — which this file did until spec 032 phase 26 — collapses five
/// distinct surfaces into "decoded / not decoded", and the caller then had
/// nothing to draw for four of them. The panel filled that hole with the
/// GALLERY's blocks, so a real request from `127.0.0.1` was drawn, under its
/// own true header, as "Swap 0.5 ETH for 1,278.11 USDC · Uniswap V3 Router"
/// for as long as resolution took. Phase 22 fixed the mirror image of this (a
/// mock header over a live request); this half is worse, because a header the
/// reader can verify invites them to trust the body under it.
///
/// `Loading` draws a line rather than nothing for the same reason invariant ⑦
/// exists: a blind view must never flash before the clear one, and an empty
/// body reads as "this transaction does nothing".
#[must_use]
pub fn blocks(clear: &ClearSigningView, facts: &RequestFacts, s: &SigningStrings) -> Vec<Block> {
    match clear.surface {
        ClearSurface::None => Vec::new(),
        ClearSurface::Loading => vec![Block::Sentence {
            text: s.loading.clone(),
            tone: Tone::Neutral,
        }],
        ClearSurface::ClearSign => clear
            .result
            .as_ref()
            .map(|result| result_blocks(result, s))
            .unwrap_or_default(),
        ClearSurface::EthSign | ClearSurface::MessageSign => clear
            .message
            .as_ref()
            .map(|message| message_blocks(message, s))
            .unwrap_or_default(),
        ClearSurface::BlindTypedData => clear
            .blind_typed
            .as_ref()
            .map(|typed| blind_typed_blocks(typed, s))
            .unwrap_or_default(),
        ClearSurface::BlindTransaction => blind_tx_blocks(facts, s),
    }
}

/// What the CHAIN said this request would move — the one block on a signing
/// sheet a malicious site cannot author.
///
/// Every other line starts as something the dApp claimed: its calldata, its
/// intent, the token it names. This starts as the result of running the call.
/// That is why the deeper degradation rungs promote it from a footnote to the
/// protagonist, and why it earns a place even when it can only answer
/// sometimes.
///
/// The asymmetry is the CORE's and is not re-decided here (its invariant ⑥):
///
/// - an **outflow** renders its amount whenever metadata resolved — the real
///   token emits its own log, so what leaves cannot be understated;
/// - an **inflow** renders a number only when the token is trusted. A site can
///   emit any `Transfer` it likes from a contract it controls, so an
///   unverified receipt shows its DIRECTION and its name and no figure at all.
///
/// `unavailable` is a different sentence from an empty list, and the
/// difference is the whole point: "it ran and nothing moves" invites a
/// signature, "it could not be checked" is the corpus's own advice to reject.
#[must_use]
pub fn sim_blocks(
    judgments: &[vela_core::app::token_trust::TrustSimJudgment],
    unavailable: bool,
    chain_id: u32,
    s: &SigningStrings,
) -> Vec<Block> {
    use vela_core::app::token_trust::TrustSimJudgment as J;

    if unavailable {
        return vec![Block::Warning {
            tone: Tone::Danger,
            text: s.warn_sim_unavailable.clone(),
        }];
    }
    if judgments.is_empty() {
        return Vec::new();
    }

    let native = vela_core::app::network_admin::BUILTIN_CHAINS
        .iter()
        .find(|chain| chain.chain_id == chain_id)
        .map_or_else(|| "—".to_owned(), |chain| chain.native_symbol.to_owned());
    let rows: Vec<(SharedString, SharedString, Tone)> = judgments
        .iter()
        .map(|judgment| match judgment {
            J::Native { delta } => (
                SharedString::from(native.clone()),
                signed_amount(delta, 18),
                delta_tone(delta),
            ),
            J::Erc20Trusted {
                delta,
                symbol,
                decimals,
                ..
            } => (
                SharedString::from(symbol.clone()),
                signed_amount(delta, *decimals),
                delta_tone(delta),
            ),
            // No attacker-controlled amount on screen. The direction is the
            // core's and it is safe to state; the figure is not.
            J::Erc20Unverified { delta, .. } => (
                s.balance_unverified_token.clone(),
                SharedString::from(if delta.starts_with('-') { "−" } else { "+" }),
                Tone::Caution,
            ),
        })
        .collect();

    vec![Block::Balances {
        title: s.balances_title.clone(),
        rows,
        note: None,
        note_tone: Tone::Neutral,
    }]
}

/// `−8,450` / `+2.1`, from a signed base-unit string.
///
/// The minus is U+2212, as everywhere else money is negative in this app.
fn signed_amount(delta: &str, decimals: u32) -> SharedString {
    let negative = delta.starts_with('-');
    let digits = delta.trim_start_matches(['-', '+']);
    let Ok(raw) = digits.parse::<f64>() else {
        // Unparseable: the direction is still true, and a wrong number is
        // worse than no number.
        return SharedString::from(if negative { "−" } else { "+" });
    };
    #[allow(clippy::cast_possible_wrap, reason = "token decimals are small")]
    let amount = raw / 10f64.powi(decimals as i32);
    SharedString::from(format!(
        "{}{}",
        if negative { "\u{2212}" } else { "+" },
        vela_core::l10n::number::format_token_amount(
            amount,
            crate::executor::format_prefs::current().number,
            false,
        )
    ))
}

fn delta_tone(delta: &str) -> Tone {
    if delta.starts_with('-') {
        Tone::Neutral
    } else {
        Tone::Success
    }
}

#[cfg(test)]
mod sim_block_tests {
    use super::*;
    use vela_core::app::token_trust::TrustSimJudgment as J;

    fn strings() -> SigningStrings {
        SigningStrings::resolve(&crate::loc::Loc::from_env())
    }

    /// A swap, judged: what LEAVES carries its amount, what ARRIVES carries
    /// one only because the core said the token is trusted.
    #[test]
    fn what_leaves_and_what_arrives_are_not_shown_the_same_way() {
        let s = strings();
        let blocks = sim_blocks(
            &[
                J::Erc20Trusted {
                    token: "0xdd".to_owned(),
                    delta: "-8450000000".to_owned(),
                    symbol: "USDC".to_owned(),
                    decimals: 6,
                },
                J::Erc20Trusted {
                    token: "0xa0".to_owned(),
                    delta: "2100000000000000000".to_owned(),
                    symbol: "WETH".to_owned(),
                    decimals: 18,
                },
            ],
            false,
            100,
            &s,
        );
        let Some(Block::Balances { rows, .. }) = blocks.first() else {
            unreachable!("a balances block");
        };
        assert_eq!(rows[0].0, "USDC");
        assert_eq!(rows[0].1, "−8,450.00");
        assert_eq!(rows[1].0, "WETH");
        assert_eq!(rows[1].1, "+2.1");
        assert_eq!(rows[1].2, Tone::Success);
    }

    /// An unverified inflow shows a DIRECTION and never a figure.
    ///
    /// This is the whole asymmetry: a site can emit any `Transfer` it likes
    /// from a contract it controls, so "+1,000,000 SAFEMOON" on a signing
    /// sheet would be the attacker writing the wallet's own reassurance.
    #[test]
    fn an_unverified_inflow_carries_no_number() {
        let s = strings();
        let blocks = sim_blocks(
            &[J::Erc20Unverified {
                token: Some("0xbad".to_owned()),
                delta: "1000000000000000000000000".to_owned(),
            }],
            false,
            100,
            &s,
        );
        let Some(Block::Balances { rows, .. }) = blocks.first() else {
            unreachable!("a balances block");
        };
        assert_eq!(rows[0].0, s.balance_unverified_token);
        assert_eq!(rows[0].1, "+", "a direction, not an amount");
        assert!(
            !rows[0].1.contains('1'),
            "no attacker digits: {}",
            rows[0].1
        );
    }

    /// "Could not check" and "checked, nothing moves" are different sentences,
    /// and the difference is the point: one invites a signature, the other is
    /// the corpus's own advice to reject.
    #[test]
    fn an_unavailable_simulation_is_not_an_empty_one() {
        let s = strings();
        let unavailable = sim_blocks(&[], true, 100, &s);
        assert!(matches!(
            unavailable.first(),
            Some(Block::Warning {
                tone: Tone::Danger,
                ..
            })
        ));
        // Nothing to say, so nothing is said — the sheet's other blocks are
        // the transaction's account of itself.
        assert!(sim_blocks(&[], false, 100, &s).is_empty());
    }

    /// The chain's own coin is named from the registry, not from a guess.
    #[test]
    fn a_native_move_is_named_by_its_chain() {
        let s = strings();
        let blocks = sim_blocks(
            &[J::Native {
                delta: "-10000000000000000".to_owned(),
            }],
            false,
            100,
            &s,
        );
        let Some(Block::Balances { rows, .. }) = blocks.first() else {
            unreachable!("a balances block");
        };
        assert_eq!(rows[0].0, "xDAI", "Gnosis' own coin");
        assert_eq!(rows[0].1, "−0.01");
    }
}

/// The gas account cannot pay, drawn IN the sheet.
///
/// The core's own note on this surface: "the in-sheet funding swap (BUG-1:
/// never a stacked second modal)" — the bug this project spent a week on when
/// the phone stacked one and it rendered invisible. The desktop swaps the
/// body of the column it already has.
///
/// The relay's own words for a sponsorship denial are NOT shown: a screen
/// carries the wallet's sentences (SC-305), and the shortfall and the address
/// are what a person can act on anyway.
#[must_use]
pub fn funding_blocks(sign: &SignView, s: &SigningStrings) -> Vec<Block> {
    let Some(funding) = sign.funding.as_ref() else {
        return Vec::new();
    };
    let data = &funding.data;
    let mut out = vec![
        Block::Intent {
            text: s.funding_title.clone(),
            tone: Tone::Neutral,
        },
        Block::Sentence {
            text: SharedString::from(crate::signing::fill(
                &s.funding_lead,
                &[("symbol", &data.native_symbol)],
            )),
            tone: Tone::Neutral,
        },
    ];

    // What to send, and where. The shortfall rather than the recommendation
    // alone: somebody who already has half of it should not be asked for all
    // of it again. Saturating, because a balance that overtook the
    // recommendation between the check and this frame is not a negative
    // amount to send.
    let shortfall = data
        .recommended_wei
        .parse::<u128>()
        .unwrap_or(0)
        .saturating_sub(data.current_balance_wei.parse::<u128>().unwrap_or(0));
    out.push(Block::Card {
        title: None,
        rows: vec![
            (
                s.funding_address_label.clone(),
                SharedString::from(data.deposit_address.clone()),
                Tone::Neutral,
                true,
            ),
            (
                s.funding_amount_label.clone(),
                SharedString::from(format!(
                    "{} {}",
                    vela_core::app::fee_policy::from_base_units(shortfall, 18),
                    data.native_symbol
                )),
                Tone::Neutral,
                false,
            ),
        ],
        tone: Tone::Neutral,
    });

    if funding.presentation == SignFundingPresentation::Confirming {
        // The top-up is on its way. A positive line, because this is the one
        // state on this surface where the person has already done their part.
        out.push(Block::Positive(s.funding_confirming.clone()));
    }
    out
}

/// The never-unlimited spending-cap editor, and what each chip chooses.
///
/// `approval_guard` publishes ten fields and the desktop read exactly one of
/// them (`confirm_allowed`) until spec 032 phase 30, so an approval could only
/// ever be REFUSED here: `enforce_no_unlimited` fails closed at the submit
/// chokepoint and there was no way to pick a cap. Safe, and crippled.
///
/// What is deliberately NOT offered: "grant all anyway". The core has the
/// event, and this shell does not raise it — the founder's rule, and the
/// drawn scenarios never drew that chip either.
///
/// What is missing rather than declined: the custom-amount input. No desktop
/// scenario draws the field, and a chip that opens nothing is worse than a
/// chip that is not there.
#[must_use]
pub fn guard_editor(
    guard: &GuardView,
    s: &SigningStrings,
) -> Option<(Block, Vec<GuardEditorMode>)> {
    if guard.surface != GuardSurface::ApprovalEditor {
        return None;
    }
    let editor = guard.editor.as_ref()?;

    let mut chips = Vec::new();
    let mut modes = Vec::new();
    let mut chip = |label: SharedString, mode: GuardEditorMode, offered: bool| {
        let state = if !offered {
            // Disabled, not absent: "Requested" greyed out is the wallet
            // saying the amount the site asked for is the one thing it will
            // not sign, which is a fact about this request.
            ChipState::Disabled
        } else if editor.mode == Some(mode) {
            ChipState::Selected
        } else {
            ChipState::Idle
        };
        chips.push((label, state));
        modes.push(mode);
    };
    chip(
        s.chip_requested.clone(),
        GuardEditorMode::Requested,
        editor.requested_finite,
    );
    chip(
        s.chip_balance.clone(),
        GuardEditorMode::Balance,
        editor.has_balance_cap,
    );
    chip(s.chip_revoke.clone(), GuardEditorMode::Revoke, true);

    // The value row: the core's raw base units, formatted by the core's own
    // formatter with this shell's separators. A second formatter would be a
    // second answer to "how much am I approving".
    let value = match editor.display_amount_raw.as_deref().and_then(|raw| {
        raw.parse::<vela_core::app::approval_guard::GuardAmount>()
            .ok()
    }) {
        Some(units) => SharedString::from(format!(
            "{} {}",
            vela_core::app::approval_guard::format_token_amount(
                units,
                guard.meta.decimals,
                4,
                ",",
                ".",
                false,
            ),
            guard.meta.symbol
        )),
        // Nothing parses as an amount here only when the request IS unlimited
        // — which is exactly what the row must say.
        None => s.value_unlimited.clone(),
    };

    // Only a chosen, finite cap reads as settled.
    let value_tone = if editor.choice.is_some() {
        Tone::Neutral
    } else {
        Tone::Danger
    };

    let mut notes: Vec<String> = Vec::new();
    if !editor.requested_finite {
        // Two sentences, two LINES. The web shell settled this and says why
        // (`AllowanceEditor.svelte`): joining them with a space produces a
        // run-on in CJK, where a space is not a sentence break — and the
        // English corpus string carries no full stop either, so a space reads
        // as "…for your safety Set a finite amount…" in every locale.
        notes.push(format!("{}\n{}", s.unlimited_disabled, s.choose_prompt));
    }
    if guard.decimals_unverified {
        // An amount capped with decimals nobody verified is a cap at an
        // order of magnitude nobody verified.
        notes.push(s.decimals_unverified.to_string());
    }
    if guard.expired {
        notes.push(s.warn_expired.to_string());
    }

    // "increase by 100" must never read as "cap at 100" — the core computes
    // the resulting total, including the case where the on-chain read failed
    // and only the increment is known.
    let resulting_total = guard.increase_total.as_ref().map(|total| {
        let text = match total.total.as_deref() {
            Some(sum) => SharedString::from(sum.to_owned()),
            None => SharedString::from(crate::signing::fill(
                &s.resulting_total_unknown,
                &[("amount", &total.increment)],
            )),
        };
        (s.label_resulting_total.clone(), text)
    });

    Some((
        Block::Allowance {
            label: s.label_spending_cap.clone(),
            value,
            value_tone,
            chips,
            note: (!notes.is_empty()).then(|| SharedString::from(notes.join("\n"))),
            resulting_total,
            // The field appears only on the chip that needs one. Drawn from
            // the core's own `custom_text`, so a keystroke it rejected never
            // shows up on screen as though it had been taken.
            custom: (editor.mode == Some(GuardEditorMode::Custom)).then(|| AllowanceInput {
                value: SharedString::from(editor.custom_text.clone()),
                symbol: SharedString::from(guard.meta.symbol.clone()),
                placeholder: SharedString::from("0"),
                error: editor.error.map(|error| match error {
                    GuardAmountError::InvalidAmount => s.invalid_amount.clone(),
                    // Typing 2^256-1 by hand is still an unlimited approval,
                    // and it is refused with the same sentence the chip is.
                    GuardAmountError::UnlimitedDisabled => s.unlimited_disabled.clone(),
                }),
            }),
        },
        modes,
    ))
}

/// What the PIPELINE is doing, under whatever the request is.
///
/// `sign_request` publishes six judgements the desktop read none of before
/// spec 032 phase 29: it is signing, it is submitting, it has a hash, it
/// failed, it needs the gas account topped up. A sheet that shows none of
/// them is a sheet that looks identical while it works, while it succeeds and
/// while it refuses — and one of those refusals is the fail-closed
/// `enforce_no_unlimited`, which until now happened in total silence.
///
/// Every word here is a key the corpus already had.
#[must_use]
pub fn status_blocks(sign: &SignView, s: &SigningStrings) -> Vec<Block> {
    let mut out = Vec::new();

    // The gas account cannot pay. The full top-up flow is the send column's
    // and is not wired here yet (phase 29 records it); saying WHY the slide
    // will not move is the half that must not wait for it.
    if let Some(funding) = sign.funding.as_ref() {
        out.push(Block::Warning {
            tone: Tone::Caution,
            text: SharedString::from(crate::signing::fill(
                &s.funding_lead,
                &[("symbol", &funding.data.native_symbol)],
            )),
        });
    }

    if let Some(error) = sign.error.as_ref() {
        // The two the person can act on get their own sentence; the rest share
        // the send flow's, because a wallet should not have two ways of saying
        // "it did not go out and your funds are safe".
        let text = match error.kind {
            SignErrorKind::UnlimitedApproval => s.error_unlimited.clone(),
            SignErrorKind::UnsupportedChain => s.error_network.clone(),
            // A refusal the person just made needs no sentence telling them
            // they made it, and the sheet is closing anyway.
            SignErrorKind::UserRejected | SignErrorKind::WalletSwitchedChains => {
                SharedString::default()
            }
            _ => s.error_generic.clone(),
        };
        if !text.is_empty() {
            out.push(Block::Warning {
                tone: Tone::Danger,
                text,
            });
        }
    }

    // Submitted outranks signing: once there is a hash the ceremony is over,
    // and "Signing…" beside a submitted operation reads as a second signature.
    if sign.pending_op_hash.is_some() {
        out.push(Block::Positive(s.status_submitted.clone()));
    } else if sign.is_signing || sign.is_submitting {
        out.push(Block::Sentence {
            text: s.status_signing.clone(),
            tone: Tone::Neutral,
        });
    }
    out
}

/// A message the person is asked to sign, in the core's own classification.
///
/// The payload is shown as the core prepared it: `decoded_text` when the bytes
/// are readable, the short hex preview when they are not. Nothing here decodes
/// anything — a second reading of the payload is a second answer to "what am I
/// signing", and only one of them would be on screen.
fn message_blocks(message: &ClearMessageView, s: &SigningStrings) -> Vec<Block> {
    let signing_in = message.siwe.is_some();
    let danger = matches!(
        message.danger_class,
        ClearDangerClass::EthSign | ClearDangerClass::SiwePhish
    );
    let mut out = vec![Block::Intent {
        text: if signing_in {
            s.intent_sign_in.clone()
        } else {
            s.intent_message.clone()
        },
        tone: if danger { Tone::Danger } else { Tone::Neutral },
    }];

    if message.danger_class == ClearDangerClass::EthSign {
        // `eth_sign` signs an opaque digest: there is no text to read, and the
        // sentence says so before the digest is shown.
        out.push(Block::Sentence {
            text: s.body_eth_sign.clone(),
            tone: Tone::Danger,
        });
    }
    if let Some(text) = message.decoded_text.as_ref().filter(|t| !t.is_empty()) {
        out.push(Block::Sentence {
            text: SharedString::from(text.clone()),
            tone: Tone::Neutral,
        });
    }
    if let Some(preview) = message.binary_preview.as_ref() {
        out.push(Block::Code {
            lines: vec![SharedString::from(preview.clone())],
            note: None,
        });
    }
    if message.non_printable {
        out.push(Block::Warning {
            tone: Tone::Caution,
            text: SharedString::from(s.warn_hex_message.clone()),
        });
    }

    if let Some(siwe) = message.siwe.as_ref() {
        let mut rows = vec![(
            s.label_siwe_site.clone(),
            // The host the check RAN ON, never a prettier one: the core's own
            // field doc says showing a different string is how a lookalike
            // slips past.
            SharedString::from(
                siwe.domain_host
                    .clone()
                    .unwrap_or_else(|| siwe.domain.clone()),
            ),
            Tone::Neutral,
            false,
        )];
        if let Some(statement) = siwe.statement.as_ref() {
            rows.push((
                s.label_siwe_statement.clone(),
                SharedString::from(statement.clone()),
                Tone::Neutral,
                false,
            ));
        }
        if let Some(uri) = siwe.uri.as_ref() {
            rows.push((
                s.label_siwe_origin.clone(),
                SharedString::from(uri.clone()),
                Tone::Neutral,
                false,
            ));
        }
        out.push(Block::Rows(rows));
        match siwe_binding(message) {
            // Only a proven match is asserted. `Unknown` says nothing, which
            // is the fail-safe side: an unparseable authority is not evidence
            // of phishing and must not be sold as evidence of safety.
            Some(true) => out.push(Block::Positive(SharedString::from(s.ok_siwe.clone()))),
            Some(false) => out.push(Block::Warning {
                tone: Tone::Danger,
                text: SharedString::from(s.warn_siwe_mismatch.clone()),
            }),
            None => {}
        }
    }

    if message.danger_class == ClearDangerClass::EthSign {
        out.push(Block::Warning {
            tone: Tone::Danger,
            text: s.warn_eth_sign.clone(),
        });
    }
    out
}

fn siwe_binding(message: &ClearMessageView) -> Option<bool> {
    match message.binding? {
        ClearSiweBinding::Ok => Some(true),
        ClearSiweBinding::Mismatch => Some(false),
        ClearSiweBinding::Unknown => None,
    }
}

/// Typed data nobody published a descriptor for: the payload's own projection.
///
/// The core takes the first five `message` entries in payload order and the
/// domain; this draws them and says, in the corpus's words, that no descriptor
/// explained them.
fn blind_typed_blocks(typed: &ClearBlindTyped, s: &SigningStrings) -> Vec<Block> {
    let mut out = vec![
        Block::Intent {
            text: typed
                .primary_type
                .clone()
                .map_or_else(|| s.intent_typed_data.clone(), SharedString::from),
            tone: Tone::Caution,
        },
        Block::Warning {
            tone: Tone::Caution,
            text: s.warn_blind_typed.clone(),
        },
    ];
    if typed.has_domain {
        out.push(Block::Party {
            label: s.label_typed_domain.clone(),
            name: typed
                .domain_name
                .clone()
                .map_or_else(|| s.tag_unverified.clone(), SharedString::from),
            address: typed.verifying_contract.clone().map(SharedString::from),
            badge: None,
        });
    }
    let rows: Vec<crate::signing::fixtures::Row> = typed
        .fields
        .iter()
        .map(|field| {
            (
                SharedString::from(field.key.clone()),
                SharedString::from(field.value.clone()),
                Tone::Neutral,
                true,
            )
        })
        .collect();
    if !rows.is_empty() {
        out.push(Block::Rows(rows));
    }
    out
}

/// The bottom rung: a transaction nothing could read.
///
/// Two facts and no invention — how many bytes were not decoded, and who they
/// go to. The amount is deliberately absent: scaling a value is what the core
/// does for every other rung, and a number this file composed on its own would
/// be a second authority on "how much" (recorded as a gap in phase 26).
fn blind_tx_blocks(facts: &RequestFacts, s: &SigningStrings) -> Vec<Block> {
    let mut out = vec![
        Block::Intent {
            text: s.intent_contract_call.clone(),
            tone: Tone::Caution,
        },
        Block::Warning {
            tone: Tone::Caution,
            text: SharedString::from(crate::signing::fill(
                &s.warn_blind_decode,
                &[("bytes", &facts.data_bytes.to_string())],
            )),
        },
    ];
    if let Some(to) = facts.to.as_ref() {
        out.push(Block::Party {
            label: s.label_interacting.clone(),
            name: s.tag_unverified.clone(),
            address: Some(SharedString::from(to.clone())),
            badge: Some((s.tag_unverified.clone(), Tone::Caution)),
        });
    }
    out
}

/// The blocks a decoded request draws, in the order they are read.
///
/// Intent first — what this DOES — then what is wrong with it, then the
/// detail. A warning under the fields is a warning after the decision.
fn result_blocks(result: &ClearSignResult, s: &SigningStrings) -> Vec<Block> {
    let mut out = vec![Block::Intent {
        text: SharedString::from(result.intent.clone()),
        tone: tone_of(result.risk),
    }];
    out.extend(warnings(result, s));
    let rows: Vec<crate::signing::fixtures::Row> = result
        .fields
        .iter()
        // `detail` fields are the Advanced section's, not the summary's.
        // Promoting them here would bury the decision in parameters.
        .filter(|field| !field.detail)
        .map(|field| row_of(field, s))
        .collect();
    if !rows.is_empty() {
        out.push(Block::Rows(rows));
    }
    out
}

/// What is wrong with this request, from the core's flags alone.
///
/// Ordered worst-first, because a sheet is read from the top and the burn is
/// the one that cannot be undone.
fn warnings(result: &ClearSignResult, s: &SigningStrings) -> Vec<Block> {
    let mut out = Vec::new();
    if result.to_own_token {
        // Sending a token to its own contract burns it irreversibly.
        out.push(Block::Warning {
            tone: Tone::Danger,
            text: s.warn_token_to_contract.clone(),
        });
    }
    if result.best_effort {
        // Recovered from the 4-byte database and decoded generically: the
        // shape is a guess that parsed, not a descriptor anybody published.
        out.push(Block::Warning {
            tone: Tone::Caution,
            text: s.warn_best_effort.clone(),
        });
    }
    if result.partial {
        // The descriptor declared more fields than resolved. Saying nothing
        // would present an incomplete reading as a complete one.
        out.push(Block::Warning {
            tone: Tone::Caution,
            text: s.warn_verified_abi.clone(),
        });
    }
    if result.fields.iter().any(|field| field.unverified) {
        // An amount rendered with decimals nobody verified is an amount at a
        // magnitude nobody verified.
        out.push(Block::Warning {
            tone: Tone::Caution,
            text: s.warn_unverified_amount.clone(),
        });
    }
    if result.fields.iter().any(|field| field.expired) {
        out.push(Block::Warning {
            tone: Tone::Caution,
            text: s.warn_expired.clone(),
        });
    }
    out
}

/// One decoded field as a row, keeping the core's flags as the tone.
///
/// The one substitution: an amount the core could not scale. The core has no
/// words — it emits its em dash and sets `unverified` — and a dash under a
/// warning is honest but silent, so the shell says it in the reader's own
/// language. Only for a `tokenAmount`: `unverified` is set by no other field.
fn row_of(field: &ClearSignField, s: &SigningStrings) -> crate::signing::fixtures::Row {
    let tone = if field.warning {
        Tone::Danger
    } else if field.unverified || field.expired {
        Tone::Caution
    } else {
        Tone::Neutral
    };
    let value = if field.unverified && field.value.starts_with(UNKNOWN_AMOUNT) {
        s.amount_unknown.clone()
    } else {
        SharedString::from(field.value.clone())
    };
    (
        SharedString::from(field.label.clone()),
        value,
        tone,
        // Addresses and raw values read as monospace; a decoded amount does
        // not. The core says which is which by carrying an address.
        field.address.is_some(),
    )
}

/// A signature that never touches a chain: no fee, and no speed to choose.
/// A decoded one says so in its result; a message and raw typed data say so
/// by the surface they are drawn on, decoded or not (the phones' `offChain`).
#[must_use]
pub fn off_chain(clear: &ClearSigningView) -> bool {
    use vela_core::app::clear_signing::{ClearSignType, ClearSurface};
    clear
        .result
        .as_ref()
        .is_some_and(|result| result.sign_type != ClearSignType::Transaction)
        || matches!(
            clear.surface,
            ClearSurface::MessageSign | ClearSurface::EthSign | ClearSurface::BlindTypedData
        )
}

/// The fee row, or the line that says there is no fee.
///
/// An off-chain signature costs nothing, and saying "network fee: 0" would
/// invite the reader to look for one.
#[must_use]
pub fn fee_model(
    clear: &ClearSigningView,
    fee: &FeeView,
    chain_id: u32,
    open: bool,
    s: &SigningStrings,
    locale: &str,
    speed_tier: Option<FeeTier>,
) -> FeeModel {
    if off_chain(clear) {
        return FeeModel::OffChain(s.ok_no_network_fee.clone());
    }
    // For the moment between a speed being picked and its own figure landing,
    // the fee in hand is the previous speed's: "estimating", never its money.
    // Only the figure gives way — the coin list and its warning stay.
    let another_tier = fee_of_another_tier(fee, speed_tier);
    // The send screen's formatter, not a second one: two answers about what a
    // transaction costs, on two screens pricing the same operation, is how
    // they start disagreeing. An unpriced fee renders as its "—" rather than
    // vanishing — a row that is absent reads as "free", and the confirm gate
    // is shut for the same reason.
    FeeModel::OnChain {
        label: s.fee_label.clone(),
        value: if another_tier {
            s.fee_estimating.clone()
        } else {
            SharedString::from(crate::flows::live::fee_line(
                fee.fee.as_ref(),
                None,
                fee,
                locale,
            ))
        },
        // The coins the relay will take, open in the sheet when asked — each
        // with its balance and this request's cost in it. A coin that cannot
        // pay is DRAWN (for context) but the page binds it nothing.
        selector: open.then(|| {
            (
                s.fee_token_title.clone(),
                fee.options
                    .iter()
                    .map(|option| {
                        let scale = 10f64.powi(option.decimals as i32);
                        let units = |raw: &str| {
                            vela_core::l10n::number::format_token_amount(
                                raw.parse::<f64>().unwrap_or(0.0) / scale,
                                crate::executor::format_prefs::current().number,
                                false,
                            )
                        };
                        FeeTokenOption {
                            mark: (
                                SharedString::from(
                                    option.symbol.chars().take(1).collect::<String>(),
                                ),
                                crate::flows::live::chain_tint(chain_id),
                            ),
                            name: SharedString::from(option.symbol.clone()),
                            balance: SharedString::from(format!(
                                "{} {}",
                                s.fee_balance,
                                units(&option.balance)
                            )),
                            fee: SharedString::from(match &option.amount {
                                Some(amount) => format!("~{} {}", units(amount), option.symbol),
                                None => "—".to_owned(),
                            }),
                            selected: option.selected,
                            insufficient: option.insufficient,
                        }
                    })
                    .collect(),
            )
        }),
        warning: insufficient_gas_warning(fee, s),
    }
}

/// Issue #262: a wallet with 0 ETH and some USDT on mainnet was quoted in
/// ETH. The core shuts the slide in exactly that case — quoted, not
/// ready, and the coin it was quoted in cannot pay — and this is the sentence
/// that says why, in the send screen's words.
#[must_use]
pub fn insufficient_gas_warning(fee: &FeeView, s: &SigningStrings) -> Option<SharedString> {
    if fee.fee.is_none() || fee.confirm_fee_ready {
        return None;
    }
    let selected = fee.options.iter().find(|option| option.selected)?;
    selected.insufficient.then(|| {
        SharedString::from(crate::signing::fill(
            &s.warn_insufficient_gas,
            &[("sym", &selected.symbol)],
        ))
    })
}

/// Who is asking, and on which chain.
///
/// From the REQUEST, never from the fixture. A sheet that names the mock's
/// site while a different one is asking for a signature is not a cosmetic
/// error — it is the one fact the person is being asked to judge, wrong.
///
/// The name is the host itself. Deriving a friendly name from a domain is
/// guessing, and a guess in this position is how a look-alike domain gets to
/// present itself as the real thing; the drawings' pretty names come from a
/// dApp identity the request does not carry yet.
#[must_use]
pub fn dapp_identity(origin: &str) -> (SharedString, SharedString, SharedString) {
    let host = origin
        .split_once("://")
        .map_or(origin, |(_, rest)| rest)
        .split(['/', '?', '#'])
        .next()
        .unwrap_or(origin);
    let letter = host
        .chars()
        .find(char::is_ascii_alphanumeric)
        .map_or_else(|| "?".to_owned(), |c| c.to_uppercase().to_string());
    (
        SharedString::from(host.to_owned()),
        SharedString::from(host.to_owned()),
        SharedString::from(letter),
    )
}

/// The words on the slide, as the core graded them.
///
/// `Confirm` is never "Approve" — the core's own note says that verb belongs
/// only to an actual token approval, which is `approval_guard`'s surface. The
/// mock said "Confirm swap" over a plain transfer because a fixture cannot
/// know what it is confirming; this does.
#[must_use]
pub fn confirm_label(clear: &ClearSigningView, s: &SigningStrings) -> SharedString {
    use vela_core::app::clear_signing::ClearConfirm;
    let action = match &clear.confirm {
        ClearConfirm::Sign => return s.sign_label.clone(),
        ClearConfirm::Confirm => None,
        // The intent travels as a canonical English key; the shell localizes
        // the ones it has words for and shows the neutral verb for the rest,
        // which is better than showing an English key to somebody reading
        // Chinese.
        ClearConfirm::ConfirmIntent { intent } => match intent.as_str() {
            "send" => Some(s.confirm_send.clone()),
            "swap" => Some(s.confirm_swap.clone()),
            "deposit" => Some(s.confirm_deposit.clone()),
            "withdraw" => Some(s.confirm_withdraw.clone()),
            _ => None,
        },
    };
    match action {
        Some(action) => SharedString::from(format!("{} · {action}", s.slide_to_confirm)),
        None => SharedString::from(format!("{} · {}", s.slide_to_confirm, s.confirm_plain)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use vela_core::app::clear_signing::{ClearFieldRole, ClearSignType};

    fn strings() -> SigningStrings {
        SigningStrings::resolve(&crate::loc::Loc::from_env())
    }

    fn field(label: &str, value: &str) -> ClearSignField {
        ClearSignField {
            label: label.to_owned(),
            value: value.to_owned(),
            format: String::new(),
            token_address: None,
            warning: false,
            unverified: false,
            role: ClearFieldRole::Generic,
            detail: false,
            expired: false,
            address: None,
            usd_value: None,
        }
    }

    /// An amount the core could not scale reads as words, not as a dash and
    /// not as a number.
    ///
    /// The core stopped printing a number it cannot compute (spec 032 phase
    /// 25 — 1 USDC came out as "0"), and the dash it emits instead is correct
    /// but silent. This row is the one a person is asked to judge, so the
    /// shell spends a word on it. A verified amount is untouched: nothing here
    /// may rewrite a number the core did compute.
    #[test]
    fn an_amount_with_unverified_decimals_says_so_in_words() {
        let s = strings();
        let mut unknown = field("Amount", &format!("{UNKNOWN_AMOUNT} USDC.e"));
        unknown.unverified = true;
        let known = field("Amount", "500 USDC.e");

        let blocks = blocks(
            &view(result(vec![unknown, known])),
            &RequestFacts::default(),
            &s,
        );
        let rows = blocks
            .iter()
            .find_map(|block| match block {
                Block::Rows(rows) => Some(rows.clone()),
                _ => None,
            })
            .unwrap_or_else(|| unreachable!("the fields are drawn as rows"));

        assert_eq!(rows[0].1, s.amount_unknown, "the dash was left to speak");
        assert_ne!(
            rows[0].2,
            Tone::Neutral,
            "an unverified amount is a caution"
        );
        assert_eq!(rows[1].1, SharedString::from("500 USDC.e"));
        assert!(
            !s.amount_unknown.is_empty() && !s.amount_unknown.contains('.'),
            "the corpus key resolved to a phrase, not an echoed key"
        );
    }

    fn result(fields: Vec<ClearSignField>) -> ClearSignResult {
        ClearSignResult {
            intent: "Send 1 ETH".to_owned(),
            contract_name: None,
            owner: None,
            fields,
            risk: ClearRisk::Normal,
            contract_address: None,
            verified: true,
            sign_type: ClearSignType::Transaction,
            partial: false,
            best_effort: false,
            to_own_token: false,
        }
    }

    fn view(result: ClearSignResult) -> ClearSigningView {
        let mut host =
            crate::core_host::CoreHost::<vela_core::app::clear_signing::ClearSigning>::new();
        let _ = host.dispatch(vela_core::app::clear_signing::Event::Cleared);
        ClearSigningView {
            resolved: true,
            // The surface a decoded request is presented on. The core picks it
            // and the sheet follows it, so a fixture that set only `result`
            // would be testing a state the core never produces.
            surface: ClearSurface::ClearSign,
            result: Some(result),
            ..host.view()
        }
    }

    fn surfaced(surface: ClearSurface, view: ClearSigningView) -> ClearSigningView {
        ClearSigningView { surface, ..view }
    }

    fn pristine() -> ClearSigningView {
        crate::core_host::CoreHost::<vela_core::app::clear_signing::ClearSigning>::new().view()
    }

    fn message(danger: ClearDangerClass) -> ClearMessageView {
        ClearMessageView {
            payload: "0xdead".to_owned(),
            is_hex: true,
            decoded_text: Some("Sign in to Example".to_owned()),
            binary_preview: None,
            non_printable: false,
            siwe: None,
            binding: None,
            danger_class: danger,
        }
    }

    /// Every surface the core can present draws SOMETHING.
    ///
    /// This is the guard on the defect phase 26 fixed. The panel used to keep
    /// the gallery's blocks whenever the live builder returned nothing, so a
    /// real request wore a drawn swap under its own true header. The panel no
    /// longer has that fallback — which means an empty answer here is now a
    /// blank sheet, and a blank sheet reads as "this does nothing". Any
    /// surface that stops drawing must fail here first.
    #[test]
    fn every_surface_the_core_can_present_draws_something() {
        let s = strings();
        let facts = RequestFacts {
            to: Some("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned()),
            data_bytes: 196,
        };
        let cases: Vec<(ClearSurface, ClearSigningView)> = vec![
            (ClearSurface::Loading, pristine()),
            (
                ClearSurface::ClearSign,
                view(result(vec![field("Amount", "1 USDC.e")])),
            ),
            (
                ClearSurface::MessageSign,
                ClearSigningView {
                    message: Some(message(ClearDangerClass::Plain)),
                    ..pristine()
                },
            ),
            (
                ClearSurface::EthSign,
                ClearSigningView {
                    message: Some(message(ClearDangerClass::EthSign)),
                    ..pristine()
                },
            ),
            (
                ClearSurface::BlindTypedData,
                ClearSigningView {
                    blind_typed: Some(ClearBlindTyped {
                        primary_type: Some("Permit".to_owned()),
                        has_domain: true,
                        domain_name: Some("Example".to_owned()),
                        verifying_contract: Some("0xcccc".to_owned()),
                        fields: vec![vela_core::app::clear_signing::ClearBlindField {
                            key: "spender".to_owned(),
                            value: "0xdddd".to_owned(),
                        }],
                    }),
                    ..pristine()
                },
            ),
            (ClearSurface::BlindTransaction, pristine()),
        ];
        for (surface, base) in cases {
            let drawn = blocks(&surfaced(surface, base), &facts, &s);
            assert!(
                !drawn.is_empty(),
                "{surface:?} drew nothing — the sheet would be blank"
            );
        }
        // The one surface that is meant to be empty: the core presenting
        // nothing at all. Drawing something here would be the shell inventing
        // a request.
        assert!(blocks(&surfaced(ClearSurface::None, pristine()), &facts, &s).is_empty());
    }

    /// The blind rung says the two things that are still true, and no more.
    #[test]
    fn a_blind_transaction_says_only_what_is_true_about_it() {
        let s = strings();
        let facts = RequestFacts {
            to: Some("0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned()),
            data_bytes: 196,
        };
        let drawn = blocks(
            &surfaced(ClearSurface::BlindTransaction, pristine()),
            &facts,
            &s,
        );
        let warned = drawn.iter().any(|block| match block {
            Block::Warning { text, .. } => text.contains("196"),
            _ => false,
        });
        assert!(warned, "the byte count nobody could read is the warning");
        let named = drawn.iter().any(|block| match block {
            Block::Party { address, .. } => address.as_ref().is_some_and(|a| a.contains("0xbbbb")),
            _ => false,
        });
        assert!(named, "who it goes to is still known");
        // No amount: scaling a value is the core's job on every other rung,
        // and a number composed here would be a second authority on "how much".
        assert!(
            !drawn
                .iter()
                .any(|block| matches!(block, Block::Amount { .. })),
            "the shell invented an amount"
        );
    }

    /// `eth_sign` is the hard-warning surface, never the calm message view —
    /// and an unknown SIWE binding asserts nothing in either direction.
    #[test]
    fn the_message_surfaces_keep_the_cores_classification() {
        let s = strings();
        let facts = RequestFacts::default();

        let hard = blocks(
            &surfaced(
                ClearSurface::EthSign,
                ClearSigningView {
                    message: Some(message(ClearDangerClass::EthSign)),
                    ..pristine()
                },
            ),
            &facts,
            &s,
        );
        assert!(
            hard.iter().any(|block| matches!(
                block,
                Block::Warning {
                    tone: Tone::Danger,
                    ..
                }
            )),
            "eth_sign drew no danger warning"
        );

        let mut unknown_binding = message(ClearDangerClass::SiweOk);
        unknown_binding.siwe = Some(vela_core::app::clear_signing::ClearSiweFields {
            domain: "example.com".to_owned(),
            domain_host: Some("example.com".to_owned()),
            address: None,
            statement: Some("Sign in".to_owned()),
            uri: None,
            chain_id: None,
            nonce: None,
        });
        unknown_binding.binding = Some(ClearSiweBinding::Unknown);
        let calm = blocks(
            &surfaced(
                ClearSurface::MessageSign,
                ClearSigningView {
                    message: Some(unknown_binding),
                    ..pristine()
                },
            ),
            &facts,
            &s,
        );
        assert!(
            !calm.iter().any(|block| matches!(block, Block::Positive(_))),
            "an unparseable authority was sold as a verified match"
        );
        assert!(
            !calm.iter().any(|block| matches!(
                block,
                Block::Warning {
                    tone: Tone::Danger,
                    ..
                }
            )),
            "an unknown binding is not evidence of phishing either"
        );
    }

    fn pristine_sign() -> SignView {
        crate::core_host::CoreHost::<vela_core::app::sign_request::SignRequest>::new().view()
    }

    fn guard_view(editor: vela_core::app::approval_guard::GuardEditorView) -> GuardView {
        GuardView {
            surface: GuardSurface::ApprovalEditor,
            detected: None,
            meta: vela_core::app::approval_guard::GuardTokenMetaView {
                symbol: "USDC".to_owned(),
                decimals: 6,
                verified: true,
                loading: false,
            },
            editor: Some(editor),
            confirm_allowed: false,
            rewritten_params_json: None,
            increase_total: None,
            decimals_unverified: false,
            expired: false,
            batch: None,
        }
    }

    fn editor_view(
        mode: Option<GuardEditorMode>,
        requested_finite: bool,
        amount: Option<&str>,
    ) -> vela_core::app::approval_guard::GuardEditorView {
        vela_core::app::approval_guard::GuardEditorView {
            mode,
            custom_text: String::new(),
            error: None,
            choice: mode.map(|_| vela_core::app::approval_guard::GuardChoice::Amount {
                amount_raw: amount.unwrap_or("0").to_owned(),
            }),
            display_amount_raw: amount.map(str::to_owned),
            requested_finite,
            has_balance_cap: true,
            balance_raw: Some("2000000000".to_owned()),
        }
    }

    /// An unlimited request offers a cap and refuses the amount it was asked
    /// for — the founder's rule, drawn.
    #[test]
    fn an_unlimited_request_can_be_capped_but_never_granted() {
        let s = strings();
        let (block, modes) = guard_editor(&guard_view(editor_view(None, false, None)), &s)
            .unwrap_or_else(|| unreachable!("the editor surface drew nothing"));

        let Block::Allowance {
            value, chips, note, ..
        } = &block
        else {
            unreachable!("the editor is an allowance block")
        };
        assert_eq!(
            *value, s.value_unlimited,
            "an uncapped request reads as what it is"
        );
        assert_eq!(
            chips[0].1,
            ChipState::Disabled,
            "the requested amount is refused"
        );
        assert_eq!(modes[0], GuardEditorMode::Requested);
        assert!(
            note.as_ref()
                .is_some_and(|note| note.contains(s.unlimited_disabled.as_ref())),
            "the sheet never says WHY the requested chip is dead"
        );
        // Never offered, on this shell, at all.
        assert!(
            !modes.contains(&GuardEditorMode::Grant),
            "a `grant all anyway` chip reached a screen"
        );
    }

    /// A chosen cap is a number, formatted by the core's own formatter.
    #[test]
    fn a_chosen_cap_shows_the_amount_it_caps_at() {
        let s = strings();
        let chosen = editor_view(Some(GuardEditorMode::Balance), true, Some("1240000000"));
        let (block, _) =
            guard_editor(&guard_view(chosen), &s).unwrap_or_else(|| unreachable!("no editor"));
        let Block::Allowance {
            value,
            value_tone,
            chips,
            ..
        } = &block
        else {
            unreachable!("not an allowance block")
        };
        // 1,240 USDC at six decimals — the guard's formatter, not this file's.
        assert_eq!(*value, SharedString::from("1,240 USDC"));
        assert_eq!(*value_tone, Tone::Neutral);
        assert_eq!(
            chips[1].1,
            ChipState::Selected,
            "the chosen chip is the lit one"
        );
        assert_eq!(
            chips[0].1,
            ChipState::Idle,
            "a finite request may be taken as asked"
        );
    }

    /// Nothing to gate, nothing drawn — and a permit is never capped here.
    #[test]
    fn the_editor_belongs_to_one_surface_only() {
        let s = strings();
        let mut permit = guard_view(editor_view(None, true, Some("1")));
        permit.surface = vela_core::app::approval_guard::GuardSurface::PermitSign;
        assert!(
            guard_editor(&permit, &s).is_none(),
            "an off-chain permit was given a cap editor the wallet cannot enforce"
        );

        let mut none = guard_view(editor_view(None, true, Some("1")));
        none.surface = GuardSurface::None;
        assert!(guard_editor(&none, &s).is_none());
    }

    /// The top-up says how much is still missing, and where to send it.
    #[test]
    fn the_funding_surface_asks_for_the_shortfall_not_the_whole_reserve() {
        let s = strings();
        let sign = SignView {
            surface: SignSurface::Funding,
            funding: Some(vela_core::app::sign_request::SignFundingView {
                data: vela_core::app::sign_request::SignFundingNeeded {
                    deposit_address: "0xdep0517".to_owned(),
                    safe_address: "0x88cCA0".to_owned(),
                    chain_id: 100,
                    native_symbol: "xDAI".to_owned(),
                    threshold_wei: "1000000000000000000".to_owned(),
                    recommended_wei: "2000000000000000000".to_owned(),
                    // Half of it is already there.
                    current_balance_wei: "1500000000000000000".to_owned(),
                },
                presentation: SignFundingPresentation::Topup,
                denial_reason: Some("relayer said: sponsorship declined".to_owned()),
            }),
            ..pristine_sign()
        };
        let drawn = funding_blocks(&sign, &s);
        let rows = drawn
            .iter()
            .find_map(|block| match block {
                Block::Card { rows, .. } => Some(rows.clone()),
                _ => None,
            })
            .unwrap_or_else(|| unreachable!("the top-up drew no address and no amount"));
        assert_eq!(rows[0].1, SharedString::from("0xdep0517"));
        assert!(rows[0].3, "an address is read character by character");
        assert_eq!(
            rows[1].1,
            SharedString::from("0.5 xDAI"),
            "somebody who already holds half was asked for all of it again"
        );
        // The relay's own words stay off the screen (SC-305).
        assert!(
            !drawn.iter().any(|block| matches!(
                block,
                Block::Sentence { text, .. } if text.contains("sponsorship")
            )),
            "the relay's denial text reached the sheet"
        );

        // Once it is on its way, the sheet says so.
        let mut confirming = sign;
        if let Some(funding) = confirming.funding.as_mut() {
            funding.presentation = SignFundingPresentation::Confirming;
        }
        assert!(
            funding_blocks(&confirming, &s)
                .iter()
                .any(|block| matches!(block, Block::Positive(_)))
        );
    }

    /// The refusal that used to happen in silence.
    ///
    /// `enforce_no_unlimited` fails CLOSED at the submit chokepoint, so an
    /// unlimited approval cannot be signed — but until phase 29 the desktop
    /// drew nothing when it refused, and the person was left with a sheet that
    /// simply stopped working.
    #[test]
    fn a_refusal_says_which_refusal_it_was() {
        let s = strings();
        let unlimited = SignView {
            error: Some(vela_core::app::sign_request::SignErrorNotice {
                kind: SignErrorKind::UnlimitedApproval,
                detail: None,
            }),
            ..pristine_sign()
        };
        let drawn = status_blocks(&unlimited, &s);
        assert!(
            drawn.iter().any(|block| matches!(
                block,
                Block::Warning { tone: Tone::Danger, text } if *text == s.error_unlimited
            )),
            "the unlimited refusal drew nothing"
        );

        // A submit failure says the send flow's sentence, never the relay's
        // own words (SC-305: no raw relay text reaches a screen).
        let failed = SignView {
            error: Some(vela_core::app::sign_request::SignErrorNotice {
                kind: SignErrorKind::SubmitFailed,
                detail: Some("relayer said: nonce too low".to_owned()),
            }),
            ..pristine_sign()
        };
        let drawn = status_blocks(&failed, &s);
        assert!(
            drawn.iter().any(|block| matches!(
                block,
                Block::Warning { text, .. } if *text == s.error_generic
            )),
            "a submit failure drew nothing"
        );
        assert!(
            !drawn.iter().any(|block| matches!(
                block,
                Block::Warning { text, .. } if text.contains("nonce")
            )),
            "the relay's own words reached the screen"
        );

        // A rejection the person just made needs no sentence about it.
        let rejected = SignView {
            error: Some(vela_core::app::sign_request::SignErrorNotice {
                kind: SignErrorKind::UserRejected,
                detail: None,
            }),
            ..pristine_sign()
        };
        assert!(status_blocks(&rejected, &s).is_empty());
    }

    /// Working, then submitted — and never both.
    #[test]
    fn the_pipeline_says_what_it_is_doing() {
        let s = strings();
        let signing = SignView {
            is_signing: true,
            ..pristine_sign()
        };
        assert!(status_blocks(&signing, &s).iter().any(|block| matches!(
            block,
            Block::Sentence { text, .. } if *text == s.status_signing
        )));

        // A hash means the ceremony is over: "Signing…" beside a submitted
        // operation reads as a second signature.
        let submitted = SignView {
            is_signing: true,
            pending_op_hash: Some("0xhash".to_owned()),
            ..pristine_sign()
        };
        let drawn = status_blocks(&submitted, &s);
        assert!(
            drawn
                .iter()
                .any(|block| matches!(block, Block::Positive(_)))
        );
        assert!(
            !drawn.iter().any(|block| matches!(
                block,
                Block::Sentence { text, .. } if *text == s.status_signing
            )),
            "it claimed to be signing something it had already submitted"
        );

        // Nothing in flight, nothing said.
        assert!(status_blocks(&pristine_sign(), &s).is_empty());
    }

    /// The slide is three machines' answer, ANDed.
    ///
    /// The core's own doc says so, and each one alone is a different way to
    /// arm a signature nobody agreed to: without the fee's, over a price
    /// nobody has; without the guard's, over an unlimited approval nobody
    /// capped.
    #[test]
    fn the_confirm_needs_all_three_machines() {
        let mut sign =
            crate::core_host::CoreHost::<vela_core::app::sign_request::SignRequest>::new().view();
        let mut guard =
            crate::core_host::CoreHost::<vela_core::app::approval_guard::ApprovalGuard>::new()
                .view();
        let mut fee =
            crate::core_host::CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();

        let mut clear =
            crate::core_host::CoreHost::<vela_core::app::clear_signing::ClearSigning>::new().view();

        sign.confirm_gate_open = true;
        guard.confirm_allowed = true;
        fee.confirm_fee_ready = true;
        assert!(confirm_enabled(&sign, &guard, &clear, &fee, None));

        for drop_one in 0..3 {
            let (mut s, mut g, mut f) = (sign.clone(), guard.clone(), fee.clone());
            match drop_one {
                0 => s.confirm_gate_open = false,
                1 => g.confirm_allowed = false,
                _ => f.confirm_fee_ready = false,
            }
            assert!(
                !confirm_enabled(&s, &g, &clear, &f, None),
                "any one machine withholding shuts the slide ({drop_one})"
            );
        }

        // A message is quoted nothing: the fee has no say over it — and
        // the other two still do.
        clear.surface = vela_core::app::clear_signing::ClearSurface::MessageSign;
        fee.confirm_fee_ready = false;
        assert!(confirm_enabled(&sign, &guard, &clear, &fee, None));
        guard.confirm_allowed = false;
        assert!(!confirm_enabled(&sign, &guard, &clear, &fee, None));
    }

    /// Spec 069: between a speed being tapped and its own figure landing, the
    /// core's gate is still open — on the speed just left. The row says
    /// "estimating" and the slide stays shut; the figure lands, both follow.
    #[test]
    fn another_speeds_fee_neither_shows_nor_signs() {
        use vela_core::app::fee_policy::{FeeAssetView, FeeEstimateView};
        let mut sign =
            crate::core_host::CoreHost::<vela_core::app::sign_request::SignRequest>::new().view();
        let mut guard =
            crate::core_host::CoreHost::<vela_core::app::approval_guard::ApprovalGuard>::new()
                .view();
        let mut fee =
            crate::core_host::CoreHost::<vela_core::app::fee_policy::FeePolicy>::new().view();
        sign.confirm_gate_open = true;
        guard.confirm_allowed = true;
        fee.confirm_fee_ready = true;
        fee.fee = Some(FeeEstimateView {
            chain_id: 100,
            total_wei: "2100000000000000".to_owned(),
            max_fee_per_gas: "2000000000".to_owned(),
            network_fee_per_gas: "1000000000".to_owned(),
            relayer_fee_per_gas: "0".to_owned(),
            bundler_gas_price: "0".to_owned(),
            in_band_gas_basis: "0".to_owned(),
            effective_gas_price: None,
            max_gas_price: None,
            total_gas: "0".to_owned(),
            deployed: true,
            tier: FeeTier::Fast,
            quoted: true,
            fee_asset: FeeAssetView::Native,
            fee_recipient: Some("0xfee".to_owned()),
        });
        let s = SigningStrings::resolve(&crate::loc::Loc::from_env());
        let clear =
            crate::core_host::CoreHost::<vela_core::app::clear_signing::ClearSigning>::new().view();

        // Picked Slow; the fee in hand is still Fast's.
        assert!(!confirm_enabled(
            &sign,
            &guard,
            &clear,
            &fee,
            Some(FeeTier::Slow)
        ));
        let FeeModel::OnChain { value, .. } =
            fee_model(&clear, &fee, 100, false, &s, "en", Some(FeeTier::Slow))
        else {
            unreachable!("a transaction has an on-chain fee");
        };
        assert_eq!(value, s.fee_estimating);

        // Its own figure lands.
        if let Some(estimate) = fee.fee.as_mut() {
            estimate.tier = FeeTier::Slow;
        }
        assert!(confirm_enabled(
            &sign,
            &guard,
            &clear,
            &fee,
            Some(FeeTier::Slow)
        ));
        let FeeModel::OnChain { value, .. } =
            fee_model(&clear, &fee, 100, false, &s, "en", Some(FeeTier::Slow))
        else {
            unreachable!("a transaction has an on-chain fee");
        };
        assert_ne!(value, s.fee_estimating);
        // The dead `rapid` reads as the `fast` it became.
        if let Some(estimate) = fee.fee.as_mut() {
            estimate.tier = FeeTier::Rapid;
        }
        assert!(confirm_enabled(
            &sign,
            &guard,
            &clear,
            &fee,
            Some(FeeTier::Fast)
        ));
    }

    /// The detail fields belong to Advanced, not to the summary.
    ///
    /// Promoting them would bury the decision — what this DOES — under the
    /// parameters it does it with.
    #[test]
    fn advanced_fields_stay_out_of_the_summary() {
        let mut detail = field("calldata", "0xabcd");
        detail.detail = true;
        let blocks = blocks(
            &view(result(vec![field("To", "0xbbb"), detail])),
            &RequestFacts::default(),
            &strings(),
        );
        let rows = blocks
            .iter()
            .find_map(|block| match block {
                Block::Rows(rows) => Some(rows),
                _ => None,
            })
            .unwrap_or_else(|| unreachable!("a decoded request has rows"));
        assert_eq!(rows.len(), 1, "only the summary field: {rows:?}");
        assert_eq!(rows[0].0, "To");
    }

    /// Every flag the core raises reaches the screen, worst first.
    #[test]
    fn the_cores_flags_each_become_a_warning() {
        let s = strings();
        let mut burn = result(vec![field("To", "0xbbb")]);
        burn.to_own_token = true;
        burn.best_effort = true;
        let blocks = blocks(&view(burn), &RequestFacts::default(), &s);
        let warnings: Vec<_> = blocks
            .iter()
            .filter_map(|block| match block {
                Block::Warning { tone, text } => Some((*tone, text.clone())),
                _ => None,
            })
            .collect();
        assert_eq!(warnings.len(), 2, "{warnings:?}");
        // The irreversible one is read first.
        assert_eq!(
            warnings[0],
            (Tone::Danger, s.warn_token_to_contract.clone())
        );
        assert_eq!(warnings[1].0, Tone::Caution);
    }

    /// A pristine machine — nothing presented at all — draws nothing.
    ///
    /// Written before phase 26 as "nothing decoded draws nothing", which is no
    /// longer the same sentence: a request that decodes to nothing is the
    /// BlindTransaction surface and it draws the blind rung. What draws
    /// nothing is `ClearSurface::None`, which is the core saying it has not
    /// been given a request to present.
    #[test]
    fn a_machine_with_no_request_draws_nothing() {
        let host = crate::core_host::CoreHost::<vela_core::app::clear_signing::ClearSigning>::new();
        assert!(blocks(&host.view(), &RequestFacts::default(), &strings()).is_empty());
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    /// The header names the ORIGIN, and does not dress it up.
    ///
    /// Deriving a friendly name from a domain is guessing, and a guess here is
    /// how `uniswap-app.com` gets to present itself as Uniswap. The one fact
    /// the person is being asked to judge is who is asking, so it is shown
    /// exactly as the transport reported it.
    #[test]
    fn the_header_shows_the_origin_verbatim() {
        let (name, host, letter) = dapp_identity("https://app.uniswap.org/swap?x=1");
        assert_eq!(host, "app.uniswap.org");
        assert_eq!(name, host, "no invented display name");
        assert_eq!(letter, "A");

        // A look-alike stays a look-alike on screen.
        let (name, _, _) = dapp_identity("https://uniswap-app.com");
        assert_eq!(name, "uniswap-app.com");

        // Local pages and odd origins do not panic and do not go blank.
        let (name, _, letter) = dapp_identity("http://127.0.0.1:8137/");
        assert_eq!(name, "127.0.0.1:8137");
        assert_eq!(letter, "1");
        assert_eq!(dapp_identity("").2, "?");
    }
}

#[cfg(test)]
mod fee_tests {
    use super::*;
    use vela_core::app::fee_policy::{
        FeeAssetView, FeeEstimateView, FeeOptionView, FeePolicy, FeeTier,
    };

    fn strings() -> SigningStrings {
        SigningStrings::resolve(&crate::loc::Loc::from_env())
    }

    fn option(
        symbol: &str,
        contract: Option<&str>,
        insufficient: bool,
        selected: bool,
    ) -> FeeOptionView {
        FeeOptionView {
            symbol: symbol.to_owned(),
            contract: contract.map(str::to_owned),
            decimals: 6,
            balance: "2000000".to_owned(),
            recipient: "0xee2c".to_owned(),
            usd_balance: "2".to_owned(),
            usd_price: Some("1".to_owned()),
            amount: Some("1250000".to_owned()),
            insufficient,
            selected,
        }
    }

    /// A settled, native-quoted mainnet fee over the given coin list.
    fn quoted(options: Vec<FeeOptionView>, ready: bool) -> FeeView {
        let mut fee = crate::core_host::CoreHost::<FeePolicy>::new().view();
        fee.fee = Some(FeeEstimateView {
            chain_id: 1,
            total_wei: "91000000000000".to_owned(),
            max_fee_per_gas: "1".to_owned(),
            network_fee_per_gas: "1".to_owned(),
            relayer_fee_per_gas: "0".to_owned(),
            bundler_gas_price: "1".to_owned(),
            in_band_gas_basis: "21000".to_owned(),
            effective_gas_price: None,
            max_gas_price: None,
            total_gas: "21000".to_owned(),
            deployed: true,
            tier: FeeTier::Fast,
            quoted: true,
            fee_asset: FeeAssetView::Native,
            fee_recipient: Some("0xee2c".to_owned()),
        });
        fee.options = options;
        fee.confirm_fee_ready = ready;
        fee
    }

    /// The coin list opens in the sheet with every coin the relay takes —
    /// including one that cannot pay, drawn for context and marked so the
    /// page binds it nothing.
    #[test]
    fn the_open_fee_row_lists_every_coin_and_marks_the_ones_that_cannot_pay() {
        let s = strings();
        let clear =
            crate::core_host::CoreHost::<vela_core::app::clear_signing::ClearSigning>::new().view();
        let fee = quoted(
            vec![
                option("ETH", None, true, true),
                option(
                    "USDT",
                    Some("0xdac17f958d2ee523a2206206994597c13d831ec7"),
                    false,
                    false,
                ),
            ],
            false,
        );

        match fee_model(&clear, &fee, 1, false, &s, "en", None) {
            FeeModel::OnChain { selector, .. } => assert!(selector.is_none(), "closed"),
            _ => unreachable!("a transaction has a fee row"),
        }
        match fee_model(&clear, &fee, 1, true, &s, "en", None) {
            FeeModel::OnChain {
                selector: Some((title, options)),
                ..
            } => {
                assert_eq!(title, s.fee_token_title);
                assert_eq!(options.len(), 2);
                assert!(options[0].insufficient && options[0].selected);
                assert!(!options[1].insufficient);
                assert_eq!(options[1].name, "USDT");
                assert!(options[1].fee.contains("USDT"), "{}", options[1].fee);
            }
            _ => unreachable!("an open fee row lists its coins"),
        }
    }

    /// Issue #262: quoted in a coin the wallet cannot pay with, the slide is
    /// shut — and the sheet says why, in the send screen's words.
    #[test]
    fn a_fee_the_quoted_coin_cannot_pay_says_so() {
        let s = strings();
        let broke = quoted(vec![option("ETH", None, true, true)], false);
        let text = insufficient_gas_warning(&broke, &s)
            .unwrap_or_else(|| unreachable!("the gate is shut and nothing says why"));
        assert!(text.contains("ETH"), "{text}");
        assert!(!text.contains("{{"), "{text}");

        // Ready: nothing to explain.
        assert!(
            insufficient_gas_warning(&quoted(vec![option("ETH", None, true, true)], true), &s)
                .is_none()
        );
        // Not ready for another reason: this sentence is not the reason.
        assert!(
            insufficient_gas_warning(&quoted(vec![option("ETH", None, false, true)], false), &s)
                .is_none()
        );
        // Not quoted yet: no verdict at all.
        let mut unquoted = broke;
        unquoted.fee = None;
        assert!(insufficient_gas_warning(&unquoted, &s).is_none());
    }
}
