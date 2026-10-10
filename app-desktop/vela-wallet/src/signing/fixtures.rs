//! Canonical signing fixtures — the desktop port of
//! `specs/022-explore-signing-ui/data-model.md` §3 (web reference:
//! `src/lib/signing/fixtures.ts`). Amounts, addresses and contract names are
//! verbatim mock content; every label resolves through the corpus.
//!
//! The catalogue doubles as the degradation ladder's regression suite: CS23–24
//! and CS30–32 are the rungs below "verified descriptor", and they are here so
//! that any change to the renderer has to face what a wallet shows when it does
//! NOT know what a transaction does.

use gpui::{Hsla, SharedString, rgb};

use vela_core::app::clear_signing::ClearTerm;

use super::{SigningStrings, Tone, fill};
use crate::explore::fixtures as explore_fixtures;
use crate::wallet::fixtures::{ADDRESS_FULL, WALLET_NAME, chain_ethereum};

#[derive(Clone)]
pub struct AmountLine {
    pub sign: SharedString,
    pub value: SharedString,
    pub symbol: SharedString,
    /// The coin's token mark, drawn beside the figure.
    pub token: Option<crate::flows::fixtures::TokenMark>,
    pub fiat: Option<SharedString>,
    pub caption: Option<SharedString>,
    pub tone: Tone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChipState {
    Idle,
    Selected,
    Disabled,
}

/// A key/value row: label, value, the value's tone, and whether it is mono.
pub type Row = (SharedString, SharedString, Tone, bool);

/// The custom-cap field, as data. The live page hands the same block a real
/// input; the gallery draws this and nothing types into it.
#[derive(Clone)]
pub struct AllowanceInput {
    /// What has been typed so far — the CORE's `custom_text`, never a local
    /// echo, so a rejected keystroke never appears on screen.
    pub value: SharedString,
    /// The coin the number counts in, drawn as the field's label.
    pub symbol: SharedString,
    pub placeholder: SharedString,
    /// The core's verdict on what is typed so far.
    pub error: Option<SharedString>,
}

#[derive(Clone)]
pub enum Block {
    Intent {
        text: SharedString,
        tone: Tone,
    },
    Amount {
        line: AmountLine,
        card: bool,
        note: Option<SharedString>,
        compact: bool,
    },
    Swap {
        pay: AmountLine,
        receive: AmountLine,
    },
    Nft {
        id: SharedString,
        collection: SharedString,
    },
    Sentence {
        text: SharedString,
        tone: Tone,
    },
    Allowance {
        label: SharedString,
        value: SharedString,
        value_tone: Tone,
        chips: Vec<(SharedString, ChipState)>,
        note: Option<SharedString>,
        resulting_total: Option<(SharedString, SharedString)>,
        /// The typed cap, when `Custom` is the chosen chip.
        ///
        /// Drawn UNDER the chips and above the note, so the reading order
        /// stays "what the cap is · how to change it · what is wrong with
        /// it". The big value above keeps counting as the number is typed —
        /// that feedback is what makes typing a cap safe, and it is the
        /// core's own recomputation rather than an echo of the keystrokes.
        custom: Option<AllowanceInput>,
    },
    Party {
        label: SharedString,
        name: SharedString,
        address: Option<SharedString>,
        badge: Option<(SharedString, Tone)>,
    },
    Rows(Vec<Row>),
    Warning {
        tone: Tone,
        text: SharedString,
    },
    Positive(SharedString),
    Code {
        lines: Vec<SharedString>,
        note: Option<SharedString>,
    },
    Card {
        title: Option<SharedString>,
        rows: Vec<Row>,
        tone: Tone,
    },
    Balances {
        title: SharedString,
        rows: Vec<(SharedString, SharedString, Tone)>,
        note: Option<SharedString>,
        note_tone: Tone,
    },
}

pub struct FeeTokenOption {
    /// The coin's own mark — the send flow's fee-coin sheet's: its logo from
    /// the chain-data endpoint over the drawn ticker, the request's chain as
    /// the badge. It was a letter on a tinted disc, so USDC and USDT were the
    /// same "U".
    pub mark: crate::flows::fixtures::TokenMark,
    pub name: SharedString,
    pub balance: SharedString,
    pub fee: SharedString,
    pub selected: bool,
    /// The core's balance<fee gate: drawn for context, never pickable.
    pub insufficient: bool,
    /// Issue #408: why a greyed coin cannot pay, drawn under its row — the
    /// core's shortfall, need and have in the coin's own unit.
    pub reason: Option<SharedString>,
}

pub enum FeeModel {
    OnChain {
        label: SharedString,
        value: SharedString,
        selector: Option<(SharedString, Vec<FeeTokenOption>)>,
        /// The row has something to DO when it is pressed: ask a failed quote
        /// again, or open the list of coins that can pay. On a chain with one
        /// fee coin and a quote in hand there is nothing to choose — so the
        /// row is a fact, not a control, and is drawn without the chevron it
        /// cannot honour. The handler has said this since 032
        /// (`signing_host::fee_tapped`, "One coin and a quote: nothing to
        /// choose"); only the drawing kept the chevron. Android's `tappable`
        /// and the web's are the same flag, same rule.
        tappable: bool,
        /// Under the row, in the error colour: why the confirm is shut
        /// when the coin the fee was quoted in cannot pay it (issue #262) — or,
        /// spec 079, that the service could not be reached and the wallet
        /// will ask again by itself.
        warning: Option<SharedString>,
        /// The warning is the last settled quote's, held while the fee is
        /// measured again: its line keeps its height, drawn invisibly — the
        /// verdict is about the last quote, so it is not said.
        warning_held: bool,
        /// Spec 079: the send form's refresh control beside the row (its
        /// label is what a screen reader says), `None` in the drawings.
        refresh: Option<SharedString>,
        /// A measurement is out: the control turns and the row waits.
        refreshing: bool,
        /// The figure is from a while ago (`FeeView.stale`) — a fact about a
        /// number, so only ever beside one.
        stale_note: Option<SharedString>,
    },
    /// Off-chain signature: the ✓ line, in place of a fee row.
    OffChain(SharedString),
    /// Nothing at all — CS20–CS22, where there is no fee and no reassurance.
    Hidden,
}

pub struct SigningModel {
    #[allow(dead_code, reason = "scenario identity, asserted by the fixtures test")]
    pub id: &'static str,
    pub dapp_name: SharedString,
    pub dapp_host: SharedString,
    pub dapp_letter: SharedString,
    pub dapp_tint: Hsla,
    pub network_name: SharedString,
    pub network_dot: Hsla,
    /// The wallet asking ITSELF (the key backup) — the core's
    /// `SignRequestView.first_party`, never read off the request's bytes: no
    /// requester header, and the intent leads as the headline.
    pub first_party: bool,
    /// The site's own icon, tried in order OVER the letter (founder ruling 2026-09-19).
    pub dapp_icon_urls: Vec<SharedString>,
    /// The chain's logo; the dot shows until it lands, and when there is none.
    pub network_logo: Option<SharedString>,
    pub blocks: Vec<Block>,
    pub fee: FeeModel,
    pub signer_label: SharedString,
    pub signer_name: SharedString,
    pub signer_seed: SharedString,
    /// `滑动以确认 · {action}` — there is no reject button anywhere in this
    /// vocabulary: closing the column is the rejection.
    pub confirm_label: SharedString,
    pub confirm_enabled: bool,
    /// Why the confirm is shut, in the core's words (spec 099 R7) — under it,
    /// so a dead control is never without its reason.
    pub confirm_note: Option<SharedString>,
    /// The third column's heading. The panel scaffold takes it from the page,
    /// which reads it from the same strings — kept here so a phone shell can
    /// use the model alone.
    #[allow(dead_code, reason = "shell-agnostic panel title")]
    pub panel_title: SharedString,
}

/// The eight scenarios the desktop mocks pinned (DCS1–8 + DE4), in order.
#[allow(
    dead_code,
    reason = "cross-platform scenario inventory (data-model.md §3)"
)]
pub const DESKTOP_STATES: [&str; 10] = [
    "cs1", "cs5", "cs11", "cs16", "cs24", "cs26", "cs32", "cs33", "cs12", "cs34",
];

/// Every scenario in the catalogue, phone and desktop alike.
#[allow(
    dead_code,
    reason = "cross-platform scenario inventory (data-model.md §3)"
)]
pub const ALL_STATES: [&str; 46] = [
    "cs1", "cs2", "cs3", "cs4", "cs5", "cs6", "cs7", "cs8", "cs9", "cs10", "cs11", "cs12", "cs13",
    "cs14", "cs15", "cs16", "cs17", "cs18", "cs19", "cs20", "cs21", "cs22", "cs23", "cs24", "cs25",
    "cs26", "cs27", "cs28", "cs29", "cs30", "cs31", "cs32", "cs33",
    // Spec 032 phase 39: the state nobody had drawn — a cap being TYPED, and
    // the same field with the core refusing what is in it.
    "cs34", "cs35",
    // The wallet asking itself: the key backup to Ethereum, first-party —
    // no requester header, the intent as the headline, the network as the
    // first row, the confirm saying the intent.
    "cs36",
    // The correctness batch: cs1's transfer with the fee row and the confirm
    // drawn by the live builders from a real fee view — the chain out of
    // reach, a fault inside Vela (#483), the account's previous transaction
    // holding the confirm, and a fee coin switched (provisional, then
    // measured).
    "cs37", "cs38", "cs39", "cs40", "cs41",
    // PR 2 note 1: the fee's failure in one truth on the row and the footer —
    // the core's re-ask out after the chain-down (the reason kept, the
    // measuring sign turning), and a failure only a tap retries.
    "cs42", "cs43",
    // PR 2 polish: the relay answered that the transfer fails — a tap opens
    // the coins while another is left ("Pay with another coin"), closed and
    // then open; with none left the row is no control.
    "cs44", "cs45", "cs46",
];

/// The scenario `VELA_SIGNING_STATE=cs36` names, if it names one — with
/// `VELA_PAGE=gallery`, the window opens with the signing column on that
/// drawing. Same env-pin family as `VELA_FLOW` and `VELA_GALLERY_TAB`: a
/// screenshot pass cannot click its way to a request nobody raised.
pub fn from_env() -> Option<&'static str> {
    let want = crate::dev_env::var!("VELA_SIGNING_STATE")?;
    ALL_STATES
        .into_iter()
        .find(|state| state.eq_ignore_ascii_case(want.trim()))
}

/// A coin on an amount line as the drawings show it: its token mark on
/// Ethereum, by the core's rule — its logo over its glyph, as a live line
/// would wear it (the gallery's network rows fetch their logos the same way;
/// offline, the glyph is what stays). `contract` is the coin's mainnet
/// contract, `None` for ETH; a coin the drawings do not name one for is a
/// contract the rule cannot place, so it gets no guessed logo.
fn coin(ticker: &'static str, contract: Option<&str>) -> crate::flows::fixtures::TokenMark {
    crate::flows::fixtures::TokenMark {
        ticker: ticker.into(),
        badge: chain_ethereum(),
        logos: crate::marks::token_logos(1, ticker, contract, &[]),
    }
}

/// A fee coin's mark as the drawings show it: the ticker glyph and the
/// chain's badge colour, with no logos — the documented fallback, so the
/// gallery never reaches the network. The badge is hidden where the live
/// mark hides it, on the chain's own coin (ETH on Ethereum).
fn fee_mark(ticker: &'static str) -> crate::flows::fixtures::TokenMark {
    crate::flows::fixtures::TokenMark {
        ticker: ticker.into(),
        badge: chain_ethereum(),
        logos: crate::marks::Logos {
            badge_hidden: ticker == "ETH",
            ..crate::marks::Logos::default()
        },
    }
}

fn amount(
    sign: &'static str,
    value: &'static str,
    symbol: &'static str,
    token: crate::flows::fixtures::TokenMark,
    tone: Tone,
) -> AmountLine {
    AmountLine {
        sign: sign.into(),
        value: value.into(),
        symbol: symbol.into(),
        token: Some(token),
        fiat: None,
        caption: None,
        tone,
    }
}

fn with_fiat(mut line: AmountLine, fiat: &'static str) -> AmountLine {
    line.fiat = Some(fiat.into());
    line
}

fn with_caption(mut line: AmountLine, caption: SharedString) -> AmountLine {
    line.caption = Some(caption);
    line
}

fn row(label: SharedString, value: impl Into<SharedString>) -> Row {
    (label, value.into(), Tone::Neutral, false)
}

fn mono_row(label: SharedString, value: impl Into<SharedString>) -> Row {
    (label, value.into(), Tone::Neutral, true)
}

fn toned_row(label: SharedString, value: impl Into<SharedString>, tone: Tone) -> Row {
    (label, value.into(), tone, false)
}

struct Dapp {
    name: &'static str,
    host: &'static str,
    letter: &'static str,
    tint: Hsla,
}

fn unknown_tint() -> Hsla {
    rgb(0x6e6b62).into()
}

/// Everything but the blocks — the parts every scenario fills the same way.
fn base(
    s: &SigningStrings,
    id: &'static str,
    dapp: Dapp,
    blocks: Vec<Block>,
    confirm_action: &SharedString,
) -> SigningModel {
    SigningModel {
        id,
        dapp_name: dapp.name.into(),
        dapp_host: dapp.host.into(),
        dapp_letter: dapp.letter.into(),
        dapp_tint: dapp.tint,
        network_name: "Ethereum".into(),
        network_dot: chain_ethereum(),
        first_party: false,
        dapp_icon_urls: Vec::new(),
        network_logo: None,
        blocks,
        fee: FeeModel::OnChain {
            label: s.fee_label.clone(),
            value: "~0.0021 ETH ≈ $5.40".into(),
            selector: None,
            warning: None,
            warning_held: false,
            tappable: true,
            refresh: None,
            refreshing: false,
            stale_note: None,
        },
        signer_label: s.signing_account.clone(),
        signer_name: WALLET_NAME.into(),
        signer_seed: ADDRESS_FULL.into(),
        confirm_label: confirm_action.clone(),
        confirm_enabled: true,
        confirm_note: None,
        panel_title: s.panel_title.clone(),
    }
}

const ALICE: &str = "0xaF5e…b3e1";
const VITALIK: &str = "0xd8dA…6045";
const ONEINCH_ROUTER: &str = "0x1111…0582";
const UNIVERSAL_ROUTER: &str = "0x3fC9…7FAD";
const UNISWAP_V3: &str = "0x68b3…4dC5";
const BAYC: &str = "0xBC4C…f13D";
const CONDUIT: &str = "0x1E00…3c71";
const MORPHO_VAULT: &str = "0x38989B…21eB";
const UNKNOWN_CONTRACT: &str = "0x4e1dC6…A9C1";
const REWARDS: &str = "0x067d3D…2ed1";
const USDT_CONTRACT: &str = "0xdAC1…1ec7";
const SAFE_CONTRACT: &str = "0x4167…461a";
const DEPLOYED: &str = "0x1A2b…9304";
const DEEPEST: &str = "0x004C22…6819";
const ADDRESS_DISPLAY: &str = "0x14fB1f…D1eA5c";

#[allow(clippy::too_many_lines, reason = "33 scenarios, one arm each")]
pub fn build(state: &str, s: &SigningStrings) -> SigningModel {
    let usdc = coin("USDC", Some("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"));
    let eth = coin("ETH", None);
    let weth = coin("WETH", Some("0xc02aaa39b223fe8d0a0e5c4f27ead9083c756cc2"));
    let spweth = coin("spWETH", Some(""));
    let usdt = coin("USDT", Some("0xdac17f958d2ee523a2206206994597c13d831ec7"));

    let uniswap = || Dapp {
        name: "Uniswap",
        host: "app.uniswap.org",
        letter: "U",
        tint: explore_fixtures::brand_uniswap(),
    };
    let oneinch = || Dapp {
        name: "1inch",
        host: "app.1inch.io",
        letter: "1",
        tint: rgb(0xc2352d).into(),
    };
    let opensea = || Dapp {
        name: "OpenSea",
        host: "opensea.io",
        letter: "O",
        tint: explore_fixtures::brand_opensea(),
    };
    let morpho = || Dapp {
        name: "Morpho",
        host: "app.morpho.org",
        letter: "M",
        tint: rgb(0x2e5bff).into(),
    };
    let safe = || Dapp {
        name: "Safe",
        host: "app.safe.global",
        letter: "S",
        tint: rgb(0x12ff80).into(),
    };
    let ens = || Dapp {
        name: "ENS",
        host: "app.ens.domains",
        letter: "E",
        tint: explore_fixtures::brand_ens(),
    };
    let unknown = || Dapp {
        name: "",
        host: "dapp.example.com",
        letter: "D",
        tint: unknown_tint(),
    };
    let verified = || Some((s.tag_verified.clone(), Tone::Success));
    let unverified_badge = || Some((s.tag_unverified.clone(), Tone::Caution));

    let mut model = match state {
        "cs1" | "cs29" => base(
            s,
            "cs1",
            uniswap(),
            vec![
                Block::Intent {
                    text: s.intent_send.clone(),
                    tone: Tone::Neutral,
                },
                Block::Amount {
                    line: with_fiat(
                        amount("", "1,000", "USDC", usdc.clone(), Tone::Neutral),
                        "≈ $1,000.00",
                    ),
                    card: false,
                    note: None,
                    compact: false,
                },
                Block::Sentence {
                    text: fill(
                        &s.summary_send,
                        &[("amount", "1,000 USDC"), ("to", "Alice Chen")],
                    )
                    .into(),
                    tone: Tone::Accent,
                },
                Block::Party {
                    label: s.label_recipient.clone(),
                    name: "Alice Chen".into(),
                    address: Some(ALICE.into()),
                    badge: Some((s.tag_contact.clone(), Tone::Neutral)),
                },
            ],
            &s.confirm_send,
        ),

        "cs2" => base(
            s,
            "cs2",
            uniswap(),
            vec![
                Block::Intent {
                    text: s.intent_send.clone(),
                    tone: Tone::Neutral,
                },
                Block::Amount {
                    line: with_fiat(
                        amount("", "10", "ETH", eth.clone(), Tone::Neutral),
                        "≈ $25,604.00",
                    ),
                    card: false,
                    note: None,
                    compact: false,
                },
                Block::Sentence {
                    text: fill(&s.summary_send, &[("amount", "10 ETH"), ("to", VITALIK)]).into(),
                    tone: Tone::Accent,
                },
                Block::Party {
                    label: s.label_recipient.clone(),
                    name: "vitalik.eth".into(),
                    address: Some(VITALIK.into()),
                    badge: Some((s.tag_first_time.clone(), Tone::Caution)),
                },
            ],
            &s.confirm_send,
        ),

        "cs3" => base(
            s,
            "cs3",
            safe(),
            vec![
                Block::Intent {
                    text: s.intent_send.clone(),
                    tone: Tone::Neutral,
                },
                Block::Amount {
                    line: with_fiat(
                        amount("", "0.5", "ETH", eth.clone(), Tone::Neutral),
                        "≈ $1,280.20",
                    ),
                    card: false,
                    note: None,
                    compact: false,
                },
                Block::Positive(s.ok_self_transfer.clone()),
                Block::Party {
                    label: s.label_recipient.clone(),
                    name: fill(&s.self_name, &[("name", WALLET_NAME)]).into(),
                    address: Some(ADDRESS_DISPLAY.into()),
                    badge: Some((s.tag_wallet.clone(), Tone::Success)),
                },
            ],
            &s.confirm_send,
        ),

        "cs4" => base(
            s,
            "cs4",
            uniswap(),
            vec![
                Block::Intent {
                    text: s.intent_send.clone(),
                    tone: Tone::Neutral,
                },
                Block::Amount {
                    line: with_fiat(
                        amount("", "100", "USDC", usdc.clone(), Tone::Neutral),
                        "≈ $100.00",
                    ),
                    card: false,
                    note: None,
                    compact: false,
                },
                Block::Sentence {
                    text: fill(
                        &s.summary_send_from,
                        &[("amount", "100 USDC"), ("to", VITALIK)],
                    )
                    .into(),
                    tone: Tone::Accent,
                },
                Block::Rows(vec![mono_row(s.label_from.clone(), ALICE)]),
                Block::Party {
                    label: s.label_recipient.clone(),
                    name: "vitalik.eth".into(),
                    address: Some(VITALIK.into()),
                    badge: None,
                },
            ],
            &s.confirm_send,
        ),

        "cs5" => {
            let mut m = base(
                s,
                "cs5",
                oneinch(),
                vec![
                    Block::Intent {
                        text: s.intent_approve.clone(),
                        tone: Tone::Danger,
                    },
                    Block::Allowance {
                        label: s.label_spending_cap.clone(),
                        value: s.value_unlimited.clone(),
                        value_tone: Tone::Danger,
                        // The site's own ask, preselected (2026-09-26): Permit2
                        // bundles revert when the wallet re-encodes the
                        // approve. A cap is one chip away.
                        chips: vec![
                            (s.chip_requested.clone(), ChipState::Selected),
                            (s.chip_balance.clone(), ChipState::Idle),
                            (s.chip_custom.clone(), ChipState::Idle),
                            (s.chip_revoke.clone(), ChipState::Idle),
                        ],
                        note: None,
                        resulting_total: None,
                        custom: None,
                    },
                    Block::Party {
                        label: s.label_spender.clone(),
                        name: "1inch Router".into(),
                        address: Some(ONEINCH_ROUTER.into()),
                        badge: verified(),
                    },
                    Block::Warning {
                        tone: Tone::Danger,
                        text: s.warn_unlimited.clone(),
                    },
                ],
                &s.intent_approve,
            );
            // Unlimited, seen and said — signable as asked.
            m.confirm_enabled = true;
            m
        }

        // The typed cap. cs5 is where this starts — an unlimited request kept
        // on its Requested chip — and this is what the card becomes once
        // somebody picks Custom: the field under the chips, the big number
        // above counting what has been typed, and the confirm shut while the
        // typed amount is not one.
        "cs34" => {
            let mut m = base(
                s,
                "cs34",
                oneinch(),
                vec![
                    Block::Intent {
                        text: s.intent_approve.clone(),
                        tone: Tone::Neutral,
                    },
                    Block::Allowance {
                        label: s.label_spending_cap.clone(),
                        value: "500 USDC".into(),
                        value_tone: Tone::Neutral,
                        chips: vec![
                            (s.chip_requested.clone(), ChipState::Idle),
                            (s.chip_balance.clone(), ChipState::Idle),
                            (s.chip_custom.clone(), ChipState::Selected),
                            (s.chip_revoke.clone(), ChipState::Idle),
                        ],
                        note: None,
                        resulting_total: None,
                        custom: Some(AllowanceInput {
                            value: "500".into(),
                            symbol: "USDC".into(),
                            placeholder: "0".into(),
                            error: None,
                        }),
                    },
                    Block::Party {
                        label: s.label_spender.clone(),
                        name: "1inch Router".into(),
                        address: Some(ONEINCH_ROUTER.into()),
                        badge: verified(),
                    },
                ],
                &s.intent_approve,
            );
            // A finite cap is a cap: the confirm may arm.
            m.confirm_enabled = true;
            m
        }

        // The same field with something in it the core will not take. The
        // number above falls back to what is still true — the request is
        // unlimited — and the confirm is shut, because a cap nobody could parse
        // is not a cap.
        "cs35" => {
            let mut m = base(
                s,
                "cs35",
                oneinch(),
                vec![
                    Block::Intent {
                        text: s.intent_approve.clone(),
                        tone: Tone::Neutral,
                    },
                    Block::Allowance {
                        label: s.label_spending_cap.clone(),
                        value: s.value_unlimited.clone(),
                        value_tone: Tone::Danger,
                        chips: vec![
                            (s.chip_requested.clone(), ChipState::Idle),
                            (s.chip_balance.clone(), ChipState::Idle),
                            (s.chip_custom.clone(), ChipState::Selected),
                            (s.chip_revoke.clone(), ChipState::Idle),
                        ],
                        note: None,
                        resulting_total: None,
                        custom: Some(AllowanceInput {
                            value: "12.3.4".into(),
                            symbol: "USDC".into(),
                            placeholder: "0".into(),
                            error: Some(s.invalid_amount.clone()),
                        }),
                    },
                    Block::Party {
                        label: s.label_spender.clone(),
                        name: "1inch Router".into(),
                        address: Some(ONEINCH_ROUTER.into()),
                        badge: verified(),
                    },
                ],
                &s.intent_approve,
            );
            m.confirm_enabled = false;
            m
        }

        // The wallet's own key backup to Ethereum, as the live column draws
        // the core's reading of it (a test holds the two together): the
        // intent leads as the headline, then Network / Address / Public keys,
        // each in the core's words; the confirm says the intent.
        "cs36" => {
            let term = |term: ClearTerm| s.terms.get(&term).cloned().unwrap_or_default();
            let intent = term(ClearTerm::IntentBackUpPublicKeys);
            let mut m = base(
                s,
                "cs36",
                Dapp {
                    name: "Vela Wallet",
                    host: "",
                    letter: "V",
                    tint: unknown_tint(),
                },
                vec![
                    // Success: the core grades the backup safe — the
                    // headline still draws it in the base ink.
                    Block::Intent {
                        text: intent.clone(),
                        tone: Tone::Success,
                    },
                    Block::Rows(vec![
                        row(term(ClearTerm::LabelNetwork), "Ethereum"),
                        mono_row(term(ClearTerm::LabelAddress), "0x88cCA0…266894"),
                        row(term(ClearTerm::LabelPublicKeys), "3"),
                    ]),
                ],
                &intent,
            );
            m.first_party = true;
            m
        }

        "cs6" => base(
            s,
            "cs6",
            oneinch(),
            vec![
                Block::Intent {
                    text: s.intent_approve.clone(),
                    tone: Tone::Neutral,
                },
                Block::Allowance {
                    label: s.label_spending_cap.clone(),
                    value: "1,240 USDC".into(),
                    value_tone: Tone::Neutral,
                    chips: vec![
                        (s.chip_requested.clone(), ChipState::Idle),
                        (s.chip_balance.clone(), ChipState::Selected),
                        (s.chip_custom.clone(), ChipState::Idle),
                        (s.chip_revoke.clone(), ChipState::Idle),
                    ],
                    note: None,
                    resulting_total: None,
                    custom: None,
                },
                Block::Sentence {
                    text: fill(
                        &s.summary_approve,
                        &[("spender", "1inch Router"), ("amount", "1,240 USDC")],
                    )
                    .into(),
                    tone: Tone::Neutral,
                },
                Block::Party {
                    label: s.label_spender.clone(),
                    name: "1inch Router".into(),
                    address: Some(ONEINCH_ROUTER.into()),
                    badge: verified(),
                },
            ],
            &s.intent_approve,
        ),

        "cs7" => base(
            s,
            "cs7",
            uniswap(),
            vec![
                Block::Intent {
                    text: s.intent_approve.clone(),
                    tone: Tone::Neutral,
                },
                Block::Allowance {
                    label: s.label_spending_cap.clone(),
                    value: "+100 USDC".into(),
                    value_tone: Tone::Neutral,
                    chips: vec![
                        (s.chip_requested.clone(), ChipState::Selected),
                        (s.chip_balance.clone(), ChipState::Idle),
                        (s.chip_custom.clone(), ChipState::Idle),
                        (s.chip_revoke.clone(), ChipState::Idle),
                    ],
                    note: None,
                    // increaseAllowance is an INCREMENT: the number that
                    // matters is the one it lands on, so the sheet adds up.
                    resulting_total: Some((s.label_resulting_total.clone(), "350 USDC".into())),
                    custom: None,
                },
                Block::Party {
                    label: s.label_spender.clone(),
                    name: "Uniswap Router".into(),
                    address: Some(UNIVERSAL_ROUTER.into()),
                    badge: verified(),
                },
            ],
            &s.intent_approve,
        ),

        "cs8" => base(
            s,
            "cs8",
            oneinch(),
            vec![
                Block::Intent {
                    text: s.intent_revoke.clone(),
                    tone: Tone::Neutral,
                },
                Block::Allowance {
                    label: s.label_spending_cap.clone(),
                    value: s.value_revoke.clone(),
                    value_tone: Tone::Neutral,
                    chips: vec![
                        (s.chip_requested.clone(), ChipState::Idle),
                        (s.chip_balance.clone(), ChipState::Idle),
                        (s.chip_custom.clone(), ChipState::Idle),
                        (s.chip_revoke.clone(), ChipState::Selected),
                    ],
                    note: None,
                    resulting_total: None,
                    custom: None,
                },
                Block::Sentence {
                    text: fill(&s.summary_revoke, &[("spender", "1inch Router")]).into(),
                    tone: Tone::Neutral,
                },
                Block::Party {
                    label: s.label_spender.clone(),
                    name: "1inch Router".into(),
                    address: Some(ONEINCH_ROUTER.into()),
                    badge: verified(),
                },
            ],
            &s.intent_revoke,
        ),

        "cs9" => base(
            s,
            "cs9",
            opensea(),
            vec![
                Block::Intent {
                    text: s.intent_transfer_nft.clone(),
                    tone: Tone::Neutral,
                },
                Block::Nft {
                    id: "#6529".into(),
                    collection: "Bored Ape Yacht Club".into(),
                },
                Block::Sentence {
                    text: fill(
                        &s.summary_transfer_nft,
                        &[("id", "#6529"), ("to", "Alice Chen")],
                    )
                    .into(),
                    tone: Tone::Accent,
                },
                Block::Party {
                    label: s.label_recipient.clone(),
                    name: "Alice Chen".into(),
                    address: Some(ALICE.into()),
                    badge: Some((s.tag_contact.clone(), Tone::Neutral)),
                },
            ],
            &s.confirm_plain,
        ),

        "cs10" => base(
            s,
            "cs10",
            opensea(),
            vec![
                Block::Intent {
                    text: s.intent_approve_all.clone(),
                    tone: Tone::Danger,
                },
                Block::Allowance {
                    label: s.label_spending_cap.clone(),
                    value: s.value_all_nfts.clone(),
                    value_tone: Tone::Danger,
                    // setApprovalForAll has no finite form to offer — two chips
                    // are the only honest choices.
                    chips: vec![
                        (s.chip_revoke_access.clone(), ChipState::Idle),
                        (s.chip_grant_all.clone(), ChipState::Selected),
                    ],
                    note: None,
                    resulting_total: None,
                    custom: None,
                },
                Block::Sentence {
                    text: fill(&s.summary_approve_nft, &[("operator", "OpenSea Conduit")]).into(),
                    tone: Tone::Accent,
                },
                Block::Party {
                    label: s.label_collection.clone(),
                    name: "Bored Ape Yacht Club".into(),
                    address: Some(BAYC.into()),
                    badge: verified(),
                },
                Block::Party {
                    label: s.label_operator.clone(),
                    name: "OpenSea Conduit".into(),
                    address: Some(CONDUIT.into()),
                    badge: None,
                },
                Block::Warning {
                    tone: Tone::Caution,
                    text: s.warn_approve_all.clone(),
                },
            ],
            &s.intent_approve_all,
        ),

        "cs11" | "cs33" => {
            let mut m = base(
                s,
                if state == "cs33" { "cs33" } else { "cs11" },
                oneinch(),
                vec![
                    Block::Intent {
                        text: s.intent_swap.clone(),
                        tone: Tone::Neutral,
                    },
                    Block::Swap {
                        pay: with_caption(
                            with_fiat(
                                amount("−", "1,000", "USDC", usdc.clone(), Tone::Neutral),
                                "≈ $1,000.00",
                            ),
                            s.label_pay.clone(),
                        ),
                        receive: with_caption(
                            with_fiat(
                                amount("+", "0.3042", "WETH", weth.clone(), Tone::Success),
                                "≈ $778.90",
                            ),
                            s.label_min_received.clone(),
                        ),
                    },
                    Block::Sentence {
                        text: fill(
                            &s.summary_swap,
                            &[("pay", "1,000 USDC"), ("receive", "0.3042 WETH")],
                        )
                        .into(),
                        tone: Tone::Accent,
                    },
                    Block::Party {
                        label: s.label_interacting.clone(),
                        name: "1inch Aggregation Router · 1inch Network".into(),
                        address: Some(ONEINCH_ROUTER.into()),
                        badge: verified(),
                    },
                ],
                &s.confirm_swap,
            );
            if state == "cs33" {
                m.fee = FeeModel::OnChain {
                    label: s.fee_label.clone(),
                    value: "~0.0021 ETH ≈ $5.40".into(),
                    tappable: true,
                    selector: Some((
                        s.fee_token_title.clone(),
                        vec![
                            FeeTokenOption {
                                mark: fee_mark("ETH"),
                                name: "ETH".into(),
                                balance: format!("{} 0.0689", s.fee_balance).into(),
                                fee: "~0.0021 ETH".into(),
                                selected: true,
                                insufficient: false,
                                reason: None,
                            },
                            FeeTokenOption {
                                mark: fee_mark("USDC"),
                                name: "USDC".into(),
                                balance: format!("{} 1,240.00", s.fee_balance).into(),
                                fee: "~5.55 USDC".into(),
                                selected: false,
                                insufficient: false,
                                reason: None,
                            },
                        ],
                    )),
                    warning: None,
                    warning_held: false,
                    refresh: None,
                    refreshing: false,
                    stale_note: None,
                };
            }
            m
        }

        "cs12" => base(
            s,
            "cs12",
            uniswap(),
            vec![
                Block::Intent {
                    text: s.intent_swap.clone(),
                    tone: Tone::Neutral,
                },
                Block::Swap {
                    pay: with_caption(
                        with_fiat(
                            amount("−", "0.5", "ETH", eth.clone(), Tone::Neutral),
                            "≈ $1,280.20",
                        ),
                        s.label_pay.clone(),
                    ),
                    receive: with_caption(
                        with_fiat(
                            amount("+", "1,278.11", "USDC", usdc.clone(), Tone::Success),
                            "≈ $1,278.11",
                        ),
                        s.label_min_received.clone(),
                    ),
                },
                Block::Sentence {
                    text: fill(
                        &s.summary_swap,
                        &[("pay", "0.5 ETH"), ("receive", "1,278.11 USDC")],
                    )
                    .into(),
                    tone: Tone::Accent,
                },
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: "Uniswap V3 Router".into(),
                    address: Some(UNISWAP_V3.into()),
                    badge: verified(),
                },
            ],
            &s.confirm_swap,
        ),

        "cs13" => base(
            s,
            "cs13",
            uniswap(),
            vec![
                Block::Intent {
                    text: s.intent_swap.clone(),
                    tone: Tone::Neutral,
                },
                Block::Swap {
                    pay: with_caption(
                        amount("−", "1,000", "USDC", usdc.clone(), Tone::Neutral),
                        s.label_pay.clone(),
                    ),
                    receive: with_caption(
                        amount("+", "0.3042", "WETH", weth.clone(), Tone::Success),
                        s.label_min_received.clone(),
                    ),
                },
                Block::Rows(vec![toned_row(
                    s.label_deadline.clone(),
                    fill(&s.expired_value, &[("time", "2026-08-14 18:00")]),
                    Tone::Caution,
                )]),
                Block::Warning {
                    tone: Tone::Caution,
                    text: s.warn_expired.clone(),
                },
                Block::Warning {
                    tone: Tone::Danger,
                    text: s.warn_will_fail.clone(),
                },
            ],
            &s.confirm_swap,
        ),

        "cs14" => base(
            s,
            "cs14",
            morpho(),
            vec![
                Block::Intent {
                    text: s.intent_deposit.clone(),
                    tone: Tone::Neutral,
                },
                Block::Swap {
                    pay: with_caption(
                        with_fiat(
                            amount("−", "2", "WETH", weth.clone(), Tone::Neutral),
                            "≈ $5,120.80",
                        ),
                        s.label_deposit_asset.clone(),
                    ),
                    receive: with_caption(
                        amount("+", "1.9631", "spWETH", spweth.clone(), Tone::Success),
                        s.label_shares_received.clone(),
                    ),
                },
                Block::Warning {
                    tone: Tone::Caution,
                    text: s.warn_unverified_amount.clone(),
                },
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: "Morpho Vault · Morpho Labs".into(),
                    address: Some(MORPHO_VAULT.into()),
                    badge: verified(),
                },
            ],
            &s.confirm_deposit,
        ),

        "cs15" => base(
            s,
            "cs15",
            morpho(),
            vec![
                Block::Intent {
                    text: s.intent_withdraw.clone(),
                    tone: Tone::Neutral,
                },
                Block::Amount {
                    line: with_fiat(
                        amount("+", "2", "WETH", weth.clone(), Tone::Success),
                        "≈ $5,120.80",
                    ),
                    card: false,
                    note: None,
                    compact: false,
                },
                Block::Sentence {
                    text: fill(&s.summary_receive, &[("amount", "2 WETH")]).into(),
                    tone: Tone::Accent,
                },
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: "Morpho Vault · Morpho Labs".into(),
                    address: Some(MORPHO_VAULT.into()),
                    badge: verified(),
                },
            ],
            &s.confirm_withdraw,
        ),

        "cs16" => {
            let mut m = base(
                s,
                "cs16",
                uniswap(),
                vec![
                    Block::Intent {
                        text: s.intent_permit.clone(),
                        tone: Tone::Danger,
                    },
                    Block::Sentence {
                        text: fill(
                            &s.summary_permit_unlimited,
                            &[("spender", "Universal Router"), ("token", "USDC")],
                        )
                        .into(),
                        tone: Tone::Danger,
                    },
                    Block::Party {
                        label: s.label_spender.clone(),
                        name: "Universal Router".into(),
                        address: Some(UNIVERSAL_ROUTER.into()),
                        badge: verified(),
                    },
                    Block::Rows(vec![
                        toned_row(
                            s.label_spending_cap.clone(),
                            format!("{} USDC", s.value_unlimited),
                            Tone::Danger,
                        ),
                        row(s.label_expires.clone(), "2026-09-14 19:30"),
                    ]),
                    // The whole reason this is danger and not caution: there is
                    // no editor to offer, because a signature cannot be capped.
                    Block::Warning {
                        tone: Tone::Danger,
                        text: s.warn_permit_cant_cap.clone(),
                    },
                ],
                &s.sign_label,
            );
            m.fee = FeeModel::OffChain(s.ok_no_network_fee.clone());
            m
        }

        "cs17" => {
            let mut m = base(
                s,
                "cs17",
                uniswap(),
                vec![
                    Block::Intent {
                        text: s.intent_permit.clone(),
                        tone: Tone::Neutral,
                    },
                    Block::Sentence {
                        text: fill(
                            &s.summary_permit,
                            &[("spender", "Universal Router"), ("amount", "1,000 USDC")],
                        )
                        .into(),
                        tone: Tone::Accent,
                    },
                    Block::Party {
                        label: s.label_spender.clone(),
                        name: "Universal Router".into(),
                        address: Some(UNIVERSAL_ROUTER.into()),
                        badge: verified(),
                    },
                    Block::Rows(vec![
                        row(s.label_spending_cap.clone(), "1,000 USDC"),
                        row(s.label_deadline.clone(), "2030-03-14 08:26"),
                    ]),
                ],
                &s.sign_label,
            );
            m.fee = FeeModel::OffChain(s.ok_no_network_fee.clone());
            m
        }

        "cs18" => {
            let mut m = base(
                s,
                "cs18",
                unknown(),
                vec![
                    Block::Intent {
                        text: s.intent_typed_data.clone(),
                        tone: Tone::Neutral,
                    },
                    Block::Warning {
                        tone: Tone::Caution,
                        text: s.warn_blind_typed.clone(),
                    },
                    Block::Rows(vec![
                        row(s.label_typed_domain.clone(), "CoolProtocol · v2"),
                        row(s.label_type.clone(), "Order"),
                        toned_row(
                            s.label_signing_for.clone(),
                            "dapp.example.com",
                            Tone::Accent,
                        ),
                    ]),
                    Block::Code {
                        lines: vec![
                            "{ \"maker\": \"0x14fB1f…D1eA5c\",".into(),
                            "  \"taker\": \"0x0000…0000\",".into(),
                            "  \"makerAmount\": \"1000000000\", … }".into(),
                        ],
                        note: None,
                    },
                ],
                &s.sign_label,
            );
            m.fee = FeeModel::OffChain(s.ok_no_network_fee.clone());
            m
        }

        "cs19" => {
            let mut m = base(
                s,
                "cs19",
                ens(),
                vec![
                    Block::Intent {
                        text: s.intent_sign_in.clone(),
                        tone: Tone::Neutral,
                    },
                    Block::Rows(vec![
                        row(s.label_siwe_site.clone(), "app.ens.domains"),
                        row(s.label_siwe_statement.clone(), "登录以管理你的 ENS 名称"),
                    ]),
                    Block::Code {
                        lines: vec![
                            "app.ens.domains wants you to sign in".into(),
                            "with your Ethereum account:".into(),
                            ADDRESS_DISPLAY.into(),
                        ],
                        note: None,
                    },
                    Block::Positive(fill(&s.ok_siwe, &[("domain", "app.ens.domains")]).into()),
                ],
                &s.sign_label,
            );
            m.fee = FeeModel::OffChain(s.ok_no_network_fee.clone());
            m
        }

        "cs20" => {
            let mut m = base(
                s,
                "cs20",
                Dapp {
                    name: "opensae-mint",
                    host: "opensae-mint.xyz",
                    letter: "O",
                    tint: unknown_tint(),
                },
                vec![
                    Block::Intent {
                        text: s.intent_sign_in.clone(),
                        tone: Tone::Danger,
                    },
                    // The mismatch goes ABOVE the facts: by the time somebody
                    // has read a login screen they have already decided.
                    Block::Warning {
                        tone: Tone::Danger,
                        text: fill(
                            &s.warn_siwe_mismatch,
                            &[("domain", "opensea.io"), ("origin", "opensae-mint.xyz")],
                        )
                        .into(),
                    },
                    Block::Rows(vec![
                        toned_row(s.label_siwe_site.clone(), "opensea.io", Tone::Danger),
                        mono_row(s.label_siwe_origin.clone(), "opensae-mint.xyz"),
                        row(s.label_siwe_statement.clone(), "登录以查看你的 NFT"),
                    ]),
                    Block::Code {
                        lines: vec![
                            "opensea.io wants you to sign in".into(),
                            "with your Ethereum account:".into(),
                            ADDRESS_DISPLAY.into(),
                        ],
                        note: None,
                    },
                ],
                &s.sign_label,
            );
            m.fee = FeeModel::Hidden;
            m
        }

        "cs21" => {
            let mut m = base(
                s,
                "cs21",
                unknown(),
                vec![
                    Block::Intent {
                        text: s.intent_message.clone(),
                        tone: Tone::Neutral,
                    },
                    Block::Warning {
                        tone: Tone::Caution,
                        text: s.warn_hex_message.clone(),
                    },
                    Block::Code {
                        lines: vec![
                            "0xdeadbeefcafebabe0102030405".into(),
                            "060708091011121314151617181920".into(),
                            "2122232425262728293031…".into(),
                        ],
                        note: Some(format!("({})", fill(&s.byte_size, &[("n", "80")])).into()),
                    },
                    Block::Rows(vec![row(s.label_signing_for.clone(), "dapp.example.com")]),
                ],
                &s.sign_label,
            );
            m.fee = FeeModel::Hidden;
            m
        }

        "cs22" => {
            let mut m = base(
                s,
                "cs22",
                unknown(),
                vec![
                    Block::Intent {
                        text: s.intent_blind.clone(),
                        tone: Tone::Danger,
                    },
                    Block::Sentence {
                        text: s.body_eth_sign.clone(),
                        tone: Tone::Danger,
                    },
                    Block::Code {
                        lines: vec![
                            "0x9c22ff5f21f0b81b113e63f7db6da9".into(),
                            "4fedef11b2119b4088b89664fb9a3c".into(),
                            "b658".into(),
                        ],
                        note: None,
                    },
                    Block::Warning {
                        tone: Tone::Danger,
                        text: s.warn_eth_sign.clone(),
                    },
                ],
                &s.confirm_plain,
            );
            m.fee = FeeModel::Hidden;
            m
        }

        "cs23" => base(
            s,
            "cs23",
            unknown(),
            vec![
                Block::Intent {
                    text: s.intent_contract_call.clone(),
                    tone: Tone::Neutral,
                },
                Block::Warning {
                    tone: Tone::Caution,
                    text: fill(&s.warn_blind_decode, &[("bytes", "196")]).into(),
                },
                Block::Rows(vec![row(s.label_amount.clone(), "0.1 ETH ≈ $256.04")]),
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: s.tag_unverified.clone(),
                    address: Some(UNKNOWN_CONTRACT.into()),
                    badge: unverified_badge(),
                },
                Block::Balances {
                    title: s.balances_title.clone(),
                    rows: vec![("ETH".into(), "−0.1".into(), Tone::Neutral)],
                    note: Some(s.balances_blind_simulated.clone()),
                    note_tone: Tone::Neutral,
                },
            ],
            &s.confirm_plain,
        ),

        "cs24" => base(
            s,
            "cs24",
            unknown(),
            vec![
                Block::Intent {
                    text: s.intent_contract_call.clone(),
                    tone: Tone::Danger,
                },
                Block::Sentence {
                    text: s.summary_drain.clone(),
                    tone: Tone::Danger,
                },
                Block::Balances {
                    title: s.balances_title.clone(),
                    rows: vec![
                        ("USDC".into(), "−8,450".into(), Tone::Danger),
                        ("ETH".into(), "−0.8".into(), Tone::Danger),
                    ],
                    note: Some(s.warn_drain.clone()),
                    note_tone: Tone::Danger,
                },
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: s.tag_unverified.clone(),
                    address: Some(UNKNOWN_CONTRACT.into()),
                    badge: unverified_badge(),
                },
                Block::Warning {
                    tone: Tone::Danger,
                    text: fill(&s.warn_blind_decode, &[("bytes", "4")]).into(),
                },
            ],
            &s.confirm_plain,
        ),

        "cs25" => base(
            s,
            "cs25",
            safe(),
            vec![
                Block::Intent {
                    text: s.intent_deploy.clone(),
                    tone: Tone::Neutral,
                },
                Block::Sentence {
                    text: s.summary_deploy.clone(),
                    tone: Tone::Accent,
                },
                Block::Rows(vec![
                    row(
                        s.label_bytecode.clone(),
                        fill(&s.byte_size, &[("n", "246")]),
                    ),
                    mono_row(s.label_predicted_address.clone(), DEPLOYED),
                ]),
            ],
            &s.confirm_plain,
        ),

        "cs26" => base(
            s,
            "cs26",
            oneinch(),
            vec![
                Block::Intent {
                    text: s.intent_batch.clone(),
                    tone: Tone::Neutral,
                },
                Block::Sentence {
                    text: fill(&s.summary_batch, &[("count", "2")]).into(),
                    tone: Tone::Accent,
                },
                Block::Card {
                    title: Some(
                        fill(
                            &s.batch_step,
                            &[("index", "1"), ("action", &s.intent_approve)],
                        )
                        .into(),
                    ),
                    rows: vec![
                        row(s.label_spending_cap.clone(), "100 USDC"),
                        row(s.label_spender.clone(), "1inch Router"),
                    ],
                    tone: Tone::Neutral,
                },
                Block::Card {
                    title: Some(
                        fill(&s.batch_step, &[("index", "2"), ("action", &s.intent_swap)]).into(),
                    ),
                    rows: vec![
                        row(s.label_pay.clone(), "−100 USDC"),
                        row(s.label_min_received.clone(), "+0.0304 WETH"),
                    ],
                    tone: Tone::Neutral,
                },
                Block::Balances {
                    title: s.balances_title.clone(),
                    rows: vec![
                        ("USDC".into(), "−100".into(), Tone::Neutral),
                        ("WETH".into(), "+0.0304".into(), Tone::Success),
                    ],
                    note: Some(s.balances_match_hero.clone()),
                    note_tone: Tone::Neutral,
                },
            ],
            &s.confirm_plain,
        ),

        "cs27" => base(
            s,
            "cs27",
            safe(),
            vec![
                Block::Intent {
                    text: s.intent_safe.clone(),
                    tone: Tone::Neutral,
                },
                Block::Sentence {
                    text: s.summary_safe.clone(),
                    tone: Tone::Accent,
                },
                // Safe's calldata nests, so the panel decodes the inner call
                // too: a wrapper that showed only the outer call would show
                // nothing at all.
                Block::Card {
                    title: Some(fill(&s.safe_inner_call, &[("action", &s.intent_send)]).into()),
                    rows: vec![
                        row(s.label_amount.clone(), "250 USDC"),
                        row(s.label_recipient.clone(), "Alice Chen"),
                    ],
                    tone: Tone::Neutral,
                },
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: "Safe 1.4.1 · Safe Ecosystem".into(),
                    address: Some(SAFE_CONTRACT.into()),
                    badge: verified(),
                },
            ],
            &s.confirm_plain,
        ),

        "cs28" => base(
            s,
            "cs28",
            unknown(),
            vec![
                Block::Intent {
                    text: s.intent_send.clone(),
                    tone: Tone::Danger,
                },
                Block::Amount {
                    line: amount("", "500", "USDT", usdt.clone(), Tone::Danger),
                    card: true,
                    note: Some(s.sent_to_token_contract.clone()),
                    compact: false,
                },
                Block::Party {
                    label: s.label_recipient.clone(),
                    name: "Tether USD".into(),
                    address: Some(USDT_CONTRACT.into()),
                    badge: Some((s.tag_contract.clone(), Tone::Danger)),
                },
                Block::Warning {
                    tone: Tone::Danger,
                    text: s.warn_token_to_contract.clone(),
                },
            ],
            &s.confirm_send,
        ),

        "cs30" => base(
            s,
            "cs30",
            unknown(),
            vec![
                Block::Intent {
                    text: s.intent_contract_call.clone(),
                    tone: Tone::Neutral,
                },
                Block::Sentence {
                    text: fill(&s.summary_best_effort, &[("fn", "execute(…)")]).into(),
                    tone: Tone::Accent,
                },
                Block::Warning {
                    tone: Tone::Caution,
                    text: s.warn_best_effort.clone(),
                },
                Block::Rows(vec![
                    mono_row(s.tech_function.clone(), "execute(bytes,bytes[],uint256)"),
                    row(
                        fill(&s.tech_param, &[("index", "1"), ("name", "bytes")]).into(),
                        "0x0b00… (2)",
                    ),
                    row(
                        fill(&s.tech_param, &[("index", "2"), ("name", "bytes[]")]).into(),
                        "2",
                    ),
                    row(
                        fill(&s.tech_param, &[("index", "3"), ("name", "deadline")]).into(),
                        "2026-08-15 20:00",
                    ),
                ]),
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: s.tag_unverified.clone(),
                    address: Some(UNKNOWN_CONTRACT.into()),
                    badge: unverified_badge(),
                },
                Block::Balances {
                    title: s.balances_title.clone(),
                    rows: vec![
                        ("ETH".into(), "−0.1".into(), Tone::Neutral),
                        ("USDC".into(), "+255.8".into(), Tone::Success),
                    ],
                    note: Some(s.balances_best_effort.clone()),
                    note_tone: Tone::Neutral,
                },
            ],
            &s.confirm_plain,
        ),

        "cs31" => base(
            s,
            "cs31",
            unknown(),
            vec![
                Block::Intent {
                    text: s.intent_contract_call.clone(),
                    tone: Tone::Neutral,
                },
                Block::Sentence {
                    text: s.summary_verified_abi.clone(),
                    tone: Tone::Neutral,
                },
                Block::Rows(vec![
                    row("claimRewards · ids".into(), "[128, 129, 130]"),
                    mono_row(
                        "beneficiary".into(),
                        fill(&s.self_name, &[("name", ADDRESS_DISPLAY)]),
                    ),
                    row("restake".into(), "true"),
                ]),
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: "RewardsVault".into(),
                    address: Some(REWARDS.into()),
                    badge: Some((s.tag_contract.clone(), Tone::Neutral)),
                },
                Block::Warning {
                    tone: Tone::Caution,
                    text: s.warn_verified_abi.clone(),
                },
                Block::Balances {
                    title: s.balances_title.clone(),
                    rows: vec![("stETH".into(), "+4.21".into(), Tone::Success)],
                    note: Some(s.balances_match_hero.clone()),
                    note_tone: Tone::Neutral,
                },
            ],
            &s.confirm_plain,
        ),

        // The deepest rung: neither decode nor simulation. Both failures are
        // stated plainly, and the amount is still shown — facts that ARE
        // knowable are never withheld because the rest is not.
        "cs32" => base(
            s,
            "cs32",
            unknown(),
            vec![
                Block::Intent {
                    text: s.intent_contract_call.clone(),
                    tone: Tone::Neutral,
                },
                Block::Warning {
                    tone: Tone::Caution,
                    text: fill(&s.warn_selector_not_listed, &[("bytes", "4")]).into(),
                },
                // Spec 082 L-D5: a node that could not check is a caution.
                Block::Warning {
                    tone: Tone::Caution,
                    text: s.warn_sim_unavailable.clone(),
                },
                Block::Rows(vec![row(s.label_amount.clone(), "0.25 ETH ≈ $640.10")]),
                Block::Party {
                    label: s.label_interacting.clone(),
                    name: s.tag_unverified.clone(),
                    address: Some(DEEPEST.into()),
                    badge: unverified_badge(),
                },
                Block::Code {
                    lines: vec![
                        "0x8fabe4c2000000000000000000000000".into(),
                        "d400866e00b055b20752a826cd5c89b8".into(),
                        "11de130b…".into(),
                    ],
                    note: Some(format!("({})", fill(&s.byte_size, &[("n", "132")])).into()),
                },
            ],
            &s.confirm_plain,
        ),

        "cs37" => correctness_state("cs37", s),
        "cs38" => correctness_state("cs38", s),
        "cs39" => correctness_state("cs39", s),
        "cs40" => correctness_state("cs40", s),
        "cs41" => correctness_state("cs41", s),
        "cs42" => correctness_state("cs42", s),
        "cs43" => correctness_state("cs43", s),
        "cs44" => correctness_state("cs44", s),
        "cs45" => correctness_state("cs45", s),
        "cs46" => correctness_state("cs46", s),

        other => panic!("unknown signing state `{other}`"),
    };

    // The unknown-site header carries the corpus's own word for it rather than
    // an empty name.
    if model.dapp_name.is_empty() {
        model.dapp_name = s.tag_unverified.clone();
    }
    model
}

/// `VELA_SIGNING_REFUSAL=held|refused|went-first|fees` — with `VELA_PAGE=
/// gallery` and a signing state: the sheet after the relay did not take the
/// operation, through the real `sign_request` core, so its failure line is
/// the core's one sentence for it (`SignView.failure_refusal_key`, PR 2 note
/// 9). At submit: another operation of the account holds the nonce (`held`
/// — "Try again" stays) or a plain refusal; after it, the tracker's verdict
/// with its reason (`went-first`: the nonce another operation used; `fees`:
/// fees stayed above). Same env-pin family as `VELA_SIGNING_STATE`.
#[must_use]
pub fn refusal_pin() -> Option<vela_core::app::sign_request::SignView> {
    let want = crate::dev_env::var!("VELA_SIGNING_REFUSAL")?;
    refusal_view(want.trim())
}

/// [`refusal_pin`]'s sheet for `want`, or `None` for a name it does not know.
#[must_use]
pub fn refusal_view(want: &str) -> Option<vela_core::app::sign_request::SignView> {
    use crate::core_host::CoreHost;
    use vela_core::app::sign_request::{
        Event, SignAccountRef, SignApproveOpts, SignOperation, SignRequest, SignShellResult,
        SignSubmitOutcome,
    };
    use vela_core::app::tx_tracker::{RefusalReason, TrackStatus};
    const ME: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
    const OP: &str = "0xa974c5dd0a00000000000000000000000000000000000000000000000000beef";
    const ID: &str = "rid-gallery";
    enum Ends {
        AtSubmit { message: String, refused: bool },
        Tracked(RefusalReason),
    }
    let ends = match want {
        "held" => Ends::AtSubmit {
            message: vela_core::user_op::PREVIOUS_PENDING_DETAIL.to_owned(),
            refused: false,
        },
        "refused" => Ends::AtSubmit {
            message: "UserOperation refused".to_owned(),
            refused: true,
        },
        "went-first" => Ends::Tracked(RefusalReason::NonceUsed),
        "fees" => Ends::Tracked(RefusalReason::FeeBelowMarket),
        _ => return None,
    };
    let mut host = CoreHost::<SignRequest>::new();
    host.dispatch(Event::NetworksChanged {
        chain_ids: vec![100],
    });
    host.dispatch(Event::AccountsChanged {
        accounts: vec![SignAccountRef {
            address: ME.to_owned(),
            credential_id: "cred0".to_owned(),
        }],
        active_index: 0,
    });
    host.dispatch(Event::RequestArrived {
        id: ID.to_owned(),
        method: "eth_sendTransaction".to_owned(),
        params_json:
            r#"[{"to":"0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141","value":"0x38d7ea4c68000"}]"#
                .to_owned(),
        origin: "https://app.uniswap.org".to_owned(),
        transport_id: "tab-gallery".to_owned(),
        dedicated_transport: true,
        per_request_chain: Some(100),
        dapp: None,
        granted_address: Some(ME.to_owned()),
        requested_address: None,
        request_ts_ms: None,
        now_ms: 1_000.0,
        first_party: false,
    });
    let ops = host.dispatch(Event::ApproveTapped {
        opts: SignApproveOpts::default(),
    });
    let precheck = ops.first()?.id;
    let ops = host.resolve(precheck, SignShellResult::PreCheck { funding: None });
    let submit = ops
        .iter()
        .find(|op| matches!(op.operation, SignOperation::SignAndSubmit { .. }))?
        .id;
    let persist = |host: &mut CoreHost<SignRequest>,
                   ops: Vec<crate::core_host::Pending<SignOperation>>| {
        for op in ops {
            if matches!(op.operation, SignOperation::PersistRecord { .. }) {
                let _ = host.resolve(op.id, SignShellResult::RecordPersisted);
            }
        }
    };
    match ends {
        Ends::AtSubmit { message, refused } => {
            host.dispatch(Event::CeremonyStarted { id: ID.to_owned() });
            host.dispatch(Event::CeremonyDone { id: ID.to_owned() });
            let ops = host.dispatch(Event::OpSigned {
                id: ID.to_owned(),
                user_op_hash: OP.to_owned(),
                submit_block: Some(48_487_620),
                now_ms: 5_000.0,
            });
            persist(&mut host, ops);
            let _ = host.resolve(
                submit,
                SignShellResult::Submit {
                    outcome: SignSubmitOutcome::Failed {
                        message,
                        refused,
                        signer: None,
                    },
                    now_ms: 6_000.0,
                },
            );
        }
        Ends::Tracked(reason) => {
            let ops = host.dispatch(Event::OpSubmitted {
                id: ID.to_owned(),
                user_op_hash: OP.to_owned(),
                now_ms: 5_000.0,
                maybe_sent: false,
                submit_block: None,
            });
            persist(&mut host, ops);
            let _ = host.dispatch(Event::OpTracked {
                user_op_hash: OP.to_owned(),
                status: TrackStatus::Rejected,
                tx_hash: None,
                now_ms: 20_000.0,
                refusal: Some(reason),
            });
        }
    }
    Some(host.view())
}

/// The correctness batch's sheet states, on cs1's transfer: its fee row
/// drawn by the live `fee_model` from a fee view the core produced, and its
/// confirm held with the core's line for it — a failed fee's footer is the
/// fee view's own (`FeeFailureView.footer_key`, the key `sign_confirm` hands
/// the sheet), so the row and the footer say one thing.
fn correctness_state(state: &'static str, s: &SigningStrings) -> SigningModel {
    let mut model = build("cs1", s);
    model.id = state;
    let failures = fee_failures();
    let [_, provisional, settled] = fee_coin_switch();
    let [choose_coin, nothing] = fee_would_fail();
    let (fee, note) = match state {
        // The chain out of reach, and a fault inside Vela: the core asks
        // again by itself — "Retrying…", never a tap.
        "cs37" => (Some(failures.down), None),
        "cs38" => (Some(failures.internal), None),
        // The fee is settled; the account's previous transaction on this
        // network holds the confirm — one line, nothing else in its place.
        "cs39" => (None, Some(s.note_previous_pending.clone())),
        "cs40" => (Some(provisional), Some(s.note_fee_measuring.clone())),
        // The re-ask out after the chain-down: the reason kept, the
        // measuring sign turning beside it.
        "cs42" => (Some(failures.retrying), None),
        // Only a tap retries it: "Tap to retry", "Tap it to retry".
        "cs43" => (Some(failures.tap), None),
        // The relay answered that it fails: "Pay with another coin" over
        // "This would fail if sent as it is." — and the list that tap
        // opens; with no coin left, the dash and no chevron.
        "cs44" | "cs46" => (Some(choose_coin), None),
        "cs45" => (Some(nothing), None),
        _ => (Some(settled), None),
    };
    if let Some(fee) = fee {
        let clear =
            crate::core_host::CoreHost::<vela_core::app::clear_signing::ClearSigning>::new().view();
        let mut row = crate::signing::live::fee_model(
            &clear,
            &fee,
            1,
            state == "cs46",
            s,
            "en",
            None,
            crate::wallet::live::Money::usd(),
        );
        crate::signing::live::fee_row_state(&mut row, fee.busy || fee.provisional);
        model.fee = row;
        // The footer of a failed fee: the fee view's own line.
        if let Some(failure) = fee.failure.as_ref() {
            model.confirm_enabled = false;
            model.confirm_note = Some(s.fee_failure.row(failure, "Ethereum").footer);
            return model;
        }
    }
    model.confirm_enabled = note.is_none();
    model.confirm_note = note;
    model
}

/// PR 2 note 1's fee failures, each through the real `fee_policy` core on
/// cs1's transfer (Ethereum), the account read answered as the executor
/// would answer it.
pub struct FeeFailures {
    /// The chain's nodes did not answer: the core asks again by itself.
    pub down: vela_core::app::fee_policy::FeeView,
    /// The read never left the app (issue 483): asked again by itself too.
    pub internal: vela_core::app::fee_policy::FeeView,
    /// The core's own re-ask after `down`, out now (`retrying`).
    pub retrying: vela_core::app::fee_policy::FeeView,
    /// An account not yet deployed with no public key: only a tap retries.
    pub tap: vela_core::app::fee_policy::FeeView,
}

#[must_use]
pub fn fee_failures() -> FeeFailures {
    use crate::core_host::CoreHost;
    use vela_core::app::fee_policy::{
        DeploymentRead, Event as FeeEvent, FeeCall, FeeOperation, FeePolicy, FeeShellResult,
        FeeTier,
    };
    let run = |read: DeploymentRead, public_key: bool, retry: bool| {
        let mut host = CoreHost::<FeePolicy>::new();
        let pending = host.dispatch(FeeEvent::QuoteRequested {
            chain_id: 1,
            account: "0x88cca0eedbf2c4426110bbfc998f048689266894".to_owned(),
            deployed: false,
            public_key_available: public_key,
            tier: FeeTier::Standard,
            calls: vec![FeeCall {
                to: "0x2222222222222222222222222222222222222222".to_owned(),
                value: "1000".to_owned(),
                data: "0x".to_owned(),
            }],
            fee_token: None,
            auto_fee_token: false,
            number: Default::default(),
            read_deployment: Some(true),
        });
        let Some(asked) = pending
            .iter()
            .find(|effect| matches!(effect.operation, FeeOperation::ReadDeployment { .. }))
        else {
            return host.view();
        };
        let after = host.resolve(asked.id, FeeShellResult::Deployment { read });
        if retry
            && let Some(timer) = after
                .iter()
                .find(|effect| matches!(effect.operation, FeeOperation::StartTtl { .. }))
        {
            // The core's own timer came due: its re-ask is out.
            let _ = host.resolve(timer.id, FeeShellResult::TtlElapsed);
        }
        host.view()
    };
    let down = || DeploymentRead::Unreachable {
        rate_limited: false,
    };
    FeeFailures {
        down: run(down(), true, false),
        internal: run(
            DeploymentRead::Internal {
                kind: "rpc: pool_unavailable".to_owned(),
            },
            true,
            false,
        ),
        retrying: run(down(), true, true),
        tap: run(DeploymentRead::Read { deployed: false }, false, false),
    }
}

/// The mainnet USDC the canned fee answers offer beside ETH.
const FEE_USDC: &str = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48";

/// The relay's in-band row for ETH (`native`) or USDC, as the canned fee
/// answers give it: 1 ETH at $2,500, 50 USDC.
fn fee_quote_row(native: bool) -> vela_core::app::fee_policy::FeeAssetQuote {
    use vela_core::app::fee_policy::{FeeAssetKind, FeeAssetQuote};
    FeeAssetQuote {
        recipient: "0x1111111111111111111111111111111111111111".to_owned(),
        asset: if native {
            FeeAssetKind::Native
        } else {
            FeeAssetKind::Erc20
        },
        fee_token: (!native).then(|| FEE_USDC.to_owned()),
        balance: if native {
            "1000000000000000000"
        } else {
            "50000000"
        }
        .to_owned(),
        decimals: if native { 18 } else { 6 },
        symbol: if native { "ETH" } else { "USDC" }.to_owned(),
        usd_balance: if native { "2500" } else { "50" }.to_owned(),
        usd_price: Some(if native { "2500" } else { "1" }.to_owned()),
        native_usd_floor_price: None,
        minimum_amount: None,
    }
}

/// Answer every effect the fee core asks, as the executor would on a
/// healthy Ethereum — the relay's simulation `refused` when asked to (spec
/// 083 fee: it ANSWERED that the operation fails with the coin in force).
fn answer_fee(
    host: &mut crate::core_host::CoreHost<vela_core::app::fee_policy::FeePolicy>,
    mut pending: Vec<crate::core_host::Pending<vela_core::app::fee_policy::FeeOperation>>,
    refused: bool,
) {
    use vela_core::app::fee_policy::{
        DeploymentRead, FeeBundlerQuote, FeeGasOutcome, FeeOperation, FeeShellResult as Res,
    };
    while let Some(effect) = pending.pop() {
        let result = match effect.operation {
            FeeOperation::ReadDeployment { .. } => Res::Deployment {
                read: DeploymentRead::Read { deployed: true },
            },
            FeeOperation::FetchGasPrice { .. } => Res::GasPrice {
                eth_gas_price: Some("1000000000".to_owned()),
                base_fee: Some("1000000000".to_owned()),
                priority_fee: Some("1000000".to_owned()),
            },
            FeeOperation::FetchBundlerQuote { .. } => Res::BundlerQuote {
                quote: Some(FeeBundlerQuote {
                    max_fee_per_gas: "2000000000".to_owned(),
                    max_priority_fee_per_gas: None,
                    network_fee_per_gas: Some("1000000000".to_owned()),
                    relayer_fee_per_gas: Some("1000000000".to_owned()),
                    in_band_fee_per_gas: None,
                }),
            },
            FeeOperation::FetchInBandQuotes { .. } => Res::InBandQuotes {
                quotes: Some(vec![fee_quote_row(true), fee_quote_row(false)]),
            },
            FeeOperation::EstimateUserOpGas { .. } if refused => Res::UserOpGas {
                outcome: FeeGasOutcome::Refused,
            },
            // The fee leg is the last call: a USDC `transfer` is one
            // SSTORE heavier than a native leg — the reason a switched
            // coin is measured again (its figure moves when it lands).
            FeeOperation::EstimateUserOpGas { calls, .. } => Res::UserOpGas {
                outcome: FeeGasOutcome::Estimated {
                    verification_gas_limit: "100000".to_owned(),
                    call_gas_limit: if calls.last().is_some_and(|leg| leg.data != "0x") {
                        "330000"
                    } else {
                        "300000"
                    }
                    .to_owned(),
                    pre_verification_gas: "50000".to_owned(),
                    settlement_gas: None,
                },
            },
            FeeOperation::MeasureInnerCalls { calls, .. } => Res::InnerCallsMeasured {
                gas: calls.iter().map(|_| None).collect(),
            },
            // Timers stay out: nothing here waits on a clock.
            _ => continue,
        };
        pending.extend(host.resolve(effect.id, result));
    }
}

/// cs1's transfer priced on Ethereum by a fresh fee core — the coin chosen
/// by the person (`auto` false) or left to the machine; `data` the call's.
fn fee_quote(
    auto: bool,
    data: &str,
) -> (
    crate::core_host::CoreHost<vela_core::app::fee_policy::FeePolicy>,
    Vec<crate::core_host::Pending<vela_core::app::fee_policy::FeeOperation>>,
) {
    use crate::core_host::CoreHost;
    use vela_core::app::fee_policy::{Event as FeeEvent, FeeCall, FeePolicy, FeeTier};
    let mut host = CoreHost::<FeePolicy>::new();
    let pending = host.dispatch(FeeEvent::QuoteRequested {
        chain_id: 1,
        account: "0x88cca0eedbf2c4426110bbfc998f048689266894".to_owned(),
        deployed: false,
        public_key_available: true,
        tier: FeeTier::Standard,
        calls: vec![FeeCall {
            to: "0x2222222222222222222222222222222222222222".to_owned(),
            value: "1000".to_owned(),
            data: data.to_owned(),
        }],
        fee_token: None,
        auto_fee_token: auto,
        number: Default::default(),
        read_deployment: Some(true),
    });
    (host, pending)
}

/// A fee coin switched, through the real `fee_policy` core with canned
/// answers: quoted in ETH, then USDC picked (`provisional` — the switched
/// figure, measured again with the USDC leg before it can be confirmed),
/// then measured. On Ethereum, for cs1's transfer.
#[must_use]
pub fn fee_coin_switch() -> [vela_core::app::fee_policy::FeeView; 3] {
    use vela_core::app::fee_policy::Event as FeeEvent;
    let (mut host, pending) = fee_quote(false, "0x");
    answer_fee(&mut host, pending, false);
    let quoted = host.view();
    let pending = host.dispatch(FeeEvent::SelectFeeAsset {
        token: Some(FEE_USDC.to_owned()),
    });
    let provisional = host.view();
    answer_fee(&mut host, pending, false);
    [quoted, provisional, host.view()]
}

/// The relay answered that a contract call fails (spec 083 fee,
/// `FeeFailure::WouldFail`), through the real `fee_policy` core (PR 2
/// polish): `[choose_coin, nothing]`. The call is a swap-sized one — a
/// small one keeps the static fallback whatever the simulation says. First
/// with ETH, the coin the person chose — USDC is still untried and has
/// something to pay from, so a tap opens the coins ("Pay with another
/// coin"); then with the coin left to the machine, which tried ETH and USDC
/// both — no coin is left, and the row is no control.
#[must_use]
pub fn fee_would_fail() -> [vela_core::app::fee_policy::FeeView; 2] {
    let call = format!("0x{}", "ab".repeat(1_200));
    let run = |auto: bool| {
        let (mut host, pending) = fee_quote(auto, &call);
        answer_fee(&mut host, pending, true);
        host.view()
    };
    [run(false), run(true)]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loc::Loc;

    /// The correctness batch's states draw what they are named for: the
    /// fee row says why it failed (the chain by name, or Vela's own fault),
    /// the confirm is held with the core's line, and a switched coin's
    /// figure turns until it is measured. PR 2 note 1: the row and the
    /// footer say one thing — "Retrying…" with the dash while the core asks
    /// again by itself (the reason kept and the sign turning through the
    /// re-ask), "Tap to retry" and "Tap it to retry" only when a tap is the
    /// one way.
    #[test]
    fn the_correctness_states_draw_their_cause() {
        use vela_core::app::fee_policy::{ESTIMATE_FAILED_KEY, FEE_FAILED_KEY, FEE_RETRYING_KEY};
        let loc = Loc::from_env();
        let s = SigningStrings::resolve(&loc);
        let row = |state| match build(state, &s).fee {
            FeeModel::OnChain {
                value,
                warning,
                refreshing,
                ..
            } => (value, warning, refreshing),
            _ => unreachable!("a transfer has a fee row"),
        };
        let (down_figure, down, down_turning) = row("cs37");
        let (_, internal, _) = row("cs38");
        assert!(down.as_ref().is_some_and(|line| line.contains("Ethereum")));
        assert!(internal.is_some() && internal != down);
        assert_eq!(down_figure.as_ref(), "—", "never \"Tap to retry\" here");
        assert!(!down_turning);
        let (figure, kept, turning) = row("cs42");
        assert_eq!(kept, down, "the reason is kept through the re-ask");
        assert_eq!(figure.as_ref(), "—");
        assert!(turning, "the measuring sign turns while it retries");
        let (tap_figure, _, _) = row("cs43");
        assert_eq!(tap_figure, loc.t(ESTIMATE_FAILED_KEY));
        let retrying = loc.t(FEE_RETRYING_KEY);
        let tap = loc.t(FEE_FAILED_KEY);
        for (state, note) in [
            ("cs37", Some(&retrying)),
            ("cs38", Some(&retrying)),
            ("cs39", Some(&s.note_previous_pending)),
            ("cs40", Some(&s.note_fee_measuring)),
            ("cs41", None),
            ("cs42", Some(&retrying)),
            ("cs43", Some(&tap)),
        ] {
            let model = build(state, &s);
            assert_eq!(model.confirm_note.as_ref(), note, "{state}");
            assert_eq!(model.confirm_enabled, note.is_none(), "{state}");
        }
        let (value, _, refreshing) = row("cs40");
        assert!(value.contains("USDC") && refreshing, "{value}");
        let (value, _, refreshing) = row("cs41");
        assert!(value.contains("USDC") && !refreshing, "{value}");
    }

    /// PR 2 polish: the would_fail boards are what they are named for,
    /// through the real fee core — a tap that opens the coins while USDC is
    /// untried, and nothing once every coin was tried; neither asks again.
    #[test]
    fn the_would_fail_views_say_what_a_tap_does() {
        use vela_core::app::fee_policy::{
            FEE_WOULD_FAIL_KEY, FeeFailure, FeeFailureTap, PAY_WITH_ANOTHER_COIN_KEY,
        };
        let [choose, nothing] = fee_would_fail();
        let choose = choose
            .failure
            .unwrap_or_else(|| unreachable!("the relay answered that it fails"));
        assert_eq!(choose.failure, FeeFailure::WouldFail);
        assert_eq!(choose.tap, FeeFailureTap::ChooseCoin);
        assert_eq!(
            choose.figure_key.as_deref(),
            Some(PAY_WITH_ANOTHER_COIN_KEY)
        );
        assert_eq!(choose.footer_key, FEE_WOULD_FAIL_KEY);
        assert_eq!(choose.chain_id, Some(1));
        let nothing = nothing
            .failure
            .unwrap_or_else(|| unreachable!("the relay answered that it fails"));
        assert_eq!(nothing.failure, FeeFailure::WouldFail);
        assert_eq!(nothing.tap, FeeFailureTap::Nothing);
        assert_eq!(nothing.figure_key, None, "the dash");
        assert_eq!(nothing.footer_key, FEE_WOULD_FAIL_KEY);
    }

    /// PR 2 note 9: each refusal the gallery pins reaches the sheet through
    /// the real core with its reason — the held nonce (tried again), the
    /// plain refusal, the nonce another operation used, the fee — and the
    /// sheet's failure says it.
    #[test]
    fn the_refusal_pins_say_why() {
        use crate::flows::fixtures::ReceiptStage;
        use vela_core::app::sign_confirm::NOT_SENT_BODY_KEY;
        use vela_core::app::tx_tracker::{REFUSED_FEES_KEY, REFUSED_KEY, REFUSED_NONCE_KEY};
        let s = SigningStrings::resolve(&Loc::from_env());
        let clock = crate::signing::status::Clock {
            now_ms: 20_000.0,
            typical_s: Some(5),
            chain_name: "Gnosis".to_owned(),
            seen_submitted_ms: None,
        };
        for (want, key, retry) in [
            ("held", NOT_SENT_BODY_KEY, true),
            ("refused", REFUSED_KEY, false),
            ("went-first", REFUSED_NONCE_KEY, false),
            ("fees", REFUSED_FEES_KEY, false),
        ] {
            let view = refusal_view(want).unwrap_or_else(|| unreachable!("{want}"));
            assert_eq!(view.failure_refusal_key.as_deref(), Some(key), "{want}");
            let receipt = crate::signing::status::approved(&view, true, None, None, &clock, &s)
                .unwrap_or_else(|| unreachable!("{want}: the failure is drawn"));
            assert_eq!(
                receipt.captions,
                vec![crate::flows::refusal_of(&s.refusals, Some(key))],
                "{want}"
            );
            assert_eq!(receipt.retry.is_some(), retry, "{want}");
            // PR 2 polish: the held nonce is "Not sent yet" — the calm disc
            // and its own title, never "Failed" in red.
            if want == "held" {
                assert!(view.failure_not_sent);
                assert_eq!(receipt.stage, ReceiptStage::NotSent);
                assert_eq!(receipt.title, s.not_sent_title);
                assert_ne!(receipt.title, s.receipt_failed);
            } else {
                assert_eq!(receipt.stage, ReceiptStage::Failed, "{want}");
            }
        }
        assert!(refusal_view("nonsense").is_none());
    }

    /// Every scenario builds, and none of them ships an empty confirm label —
    /// the confirm is the only way to say yes, so it must always say what to.
    #[test]
    fn every_scenario_builds() {
        let strings = SigningStrings::resolve(&Loc::from_env());
        for state in ALL_STATES {
            let model = build(state, &strings);
            assert!(!model.blocks.is_empty(), "{state} has no blocks");
            assert!(
                !model.confirm_label.is_empty(),
                "{state} has no confirm label"
            );
            assert!(!model.dapp_name.is_empty(), "{state} has no dApp name");
        }
    }

    /// Issue #461: every drawn confirm is a tap that says the action alone —
    /// the scenario's own verb, with no "Slide to confirm ·" before it.
    #[test]
    fn every_drawn_confirm_says_the_action_alone() {
        let s = SigningStrings::resolve(&Loc::from_env());
        let verbs = [
            &s.confirm_send,
            &s.confirm_swap,
            &s.confirm_deposit,
            &s.confirm_withdraw,
            &s.confirm_plain,
            &s.sign_label,
            &s.intent_approve,
            &s.intent_approve_all,
            &s.intent_revoke,
            &s.intent_transfer_nft,
            &s.intent_deploy,
            &s.intent_permit,
            &s.intent_safe,
            &s.intent_sign_in,
        ];
        let backup = s
            .terms
            .get(&ClearTerm::IntentBackUpPublicKeys)
            .cloned()
            .unwrap_or_default();
        let verbs: Vec<&SharedString> = verbs.into_iter().chain([&backup]).collect();
        for state in ALL_STATES {
            let model = build(state, &s);
            assert!(
                verbs.contains(&&model.confirm_label),
                "{state}: `{}` is not one action's words",
                model.confirm_label
            );
        }
        assert_eq!(build("cs11", &s).confirm_label, s.confirm_swap);
    }

    /// The kind rule on the amount lines: a coin beside a figure wears its
    /// own token mark, lettered from the same ticker the line names — never
    /// a one-letter disc that could stand for two coins.
    #[test]
    fn every_amount_line_wears_its_own_coins_mark() {
        let s = SigningStrings::resolve(&Loc::from_env());
        let mut seen = 0;
        for state in ALL_STATES {
            for block in build(state, &s).blocks {
                let lines = match block {
                    Block::Amount { line, .. } => vec![line],
                    Block::Swap { pay, receive } => vec![pay, receive],
                    _ => Vec::new(),
                };
                for line in lines {
                    let Some(mark) = line.token else { continue };
                    seen += 1;
                    assert_eq!(mark.ticker, line.symbol, "{state}");
                }
            }
        }
        assert!(seen > 0, "the drawings draw coins beside their figures");
    }

    /// Unlimited kept as asked, asserted rather than trusted (2026-09-26):
    /// CS5 opens on its requested chip, says the danger, and may be confirmed.
    #[test]
    fn unlimited_approval_is_kept_as_requested_and_said() {
        let strings = SigningStrings::resolve(&Loc::from_env());
        let model = build("cs5", &strings);
        assert!(model.confirm_enabled, "cs5 must be confirmable as asked");
        let kept = model.blocks.iter().any(|b| match b {
            Block::Allowance { chips, .. } => chips
                .first()
                .is_some_and(|(_, state)| *state == ChipState::Selected),
            _ => false,
        });
        assert!(kept, "cs5 must open on the requested-amount chip");
        let said = model.blocks.iter().any(|b| {
            matches!(b, Block::Warning { tone: Tone::Danger, text } if *text == strings.warn_unlimited)
        });
        assert!(said, "cs5 must say the approval is unlimited");
    }
}
