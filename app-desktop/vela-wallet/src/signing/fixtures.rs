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

use super::{SigningStrings, Tone, fill};
use crate::explore::fixtures as explore_fixtures;
use crate::wallet::fixtures::{ADDRESS_FULL, WALLET_NAME, chain_ethereum};

/// A token's mark: its letter and its brand colour (content, not tokens).
pub type Mark = (SharedString, Hsla);

#[derive(Clone)]
pub struct AmountLine {
    pub sign: SharedString,
    pub value: SharedString,
    pub symbol: SharedString,
    pub token: Option<Mark>,
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
    pub mark: Mark,
    pub name: SharedString,
    pub balance: SharedString,
    pub fee: SharedString,
    pub selected: bool,
}

pub enum FeeModel {
    OnChain {
        label: SharedString,
        value: SharedString,
        selector: Option<(SharedString, Vec<FeeTokenOption>)>,
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
    pub blocks: Vec<Block>,
    pub fee: FeeModel,
    pub signer_label: SharedString,
    pub signer_name: SharedString,
    pub signer_seed: SharedString,
    /// `滑动以确认 · {action}` — there is no reject button anywhere in this
    /// vocabulary: closing the column is the rejection.
    pub confirm_label: SharedString,
    pub confirm_enabled: bool,
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
pub const ALL_STATES: [&str; 35] = [
    "cs1", "cs2", "cs3", "cs4", "cs5", "cs6", "cs7", "cs8", "cs9", "cs10", "cs11", "cs12", "cs13",
    "cs14", "cs15", "cs16", "cs17", "cs18", "cs19", "cs20", "cs21", "cs22", "cs23", "cs24", "cs25",
    "cs26", "cs27", "cs28", "cs29", "cs30", "cs31", "cs32", "cs33",
    // Spec 032 phase 39: the state nobody had drawn — a cap being TYPED, and
    // the same field with the core refusing what is in it.
    "cs34", "cs35",
];

fn mark(letter: &'static str, hex: u32) -> Mark {
    (letter.into(), rgb(hex).into())
}

fn amount(
    sign: &'static str,
    value: &'static str,
    symbol: &'static str,
    token: Mark,
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
        blocks,
        fee: FeeModel::OnChain {
            label: s.fee_label.clone(),
            value: "~0.0021 ETH ≈ $5.40".into(),
            selector: None,
        },
        signer_label: s.signing_account.clone(),
        signer_name: WALLET_NAME.into(),
        signer_seed: ADDRESS_FULL.into(),
        confirm_label: format!("{} · {}", s.slide_to_confirm, confirm_action).into(),
        confirm_enabled: true,
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
    let usdc = mark("U", 0x2775ca);
    let eth = mark("E", 0x627eea);
    let weth = mark("W", 0x8a92b2);
    let spweth = mark("S", 0x4c6fff);
    let usdt = mark("T", 0x26a17b);

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
                        // Permanently disabled, not merely unselected: an
                        // unlimited request is the one thing this wallet will
                        // not sign as asked.
                        chips: vec![
                            (s.chip_requested.clone(), ChipState::Disabled),
                            (s.chip_balance.clone(), ChipState::Idle),
                            (s.chip_custom.clone(), ChipState::Idle),
                            (s.chip_revoke.clone(), ChipState::Idle),
                        ],
                        // Two sentences, two LINES — the web's `AllowanceEditor.svelte`
                        // rule: a space is not a sentence break in CJK, and
                        // the first string carries no full stop.
                        note: Some(format!("{}\n{}", s.unlimited_disabled, s.choose_prompt).into()),
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
            // Nothing to slide until a finite amount exists.
            m.confirm_enabled = false;
            m
        }

        // The typed cap. cs5 is where this starts — an unlimited request with
        // its Requested chip dead — and this is what the card becomes once
        // somebody picks Custom: the field under the chips, the big number
        // above counting what has been typed, and the slide still shut until
        // the core says the amount is finite and real.
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
                            (s.chip_requested.clone(), ChipState::Disabled),
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
            // A finite cap is a cap: the slide may arm.
            m.confirm_enabled = true;
            m
        }

        // The same field with something in it the core will not take. The
        // number above falls back to what is still true — the request is
        // unlimited — and the slide is shut, because a cap nobody could parse
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
                            (s.chip_requested.clone(), ChipState::Disabled),
                            (s.chip_balance.clone(), ChipState::Idle),
                            (s.chip_custom.clone(), ChipState::Selected),
                            (s.chip_revoke.clone(), ChipState::Idle),
                        ],
                        note: Some(format!("{}\n{}", s.unlimited_disabled, s.choose_prompt).into()),
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
                        (s.chip_requested.clone(), ChipState::Disabled),
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
                        (s.chip_requested.clone(), ChipState::Disabled),
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
                    selector: Some((
                        s.fee_token_title.clone(),
                        vec![
                            FeeTokenOption {
                                mark: eth.clone(),
                                name: "ETH".into(),
                                balance: format!("{} 0.0689", s.fee_balance).into(),
                                fee: "~0.0021 ETH".into(),
                                selected: true,
                            },
                            FeeTokenOption {
                                mark: usdc.clone(),
                                name: "USDC".into(),
                                balance: format!("{} 1,240.00", s.fee_balance).into(),
                                fee: "~5.55 USDC".into(),
                                selected: false,
                            },
                        ],
                    )),
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
                Block::Warning {
                    tone: Tone::Danger,
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

        other => panic!("unknown signing state `{other}`"),
    };

    // The unknown-site header carries the corpus's own word for it rather than
    // an empty name.
    if model.dapp_name.is_empty() {
        model.dapp_name = s.tag_unverified.clone();
    }
    model
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loc::Loc;

    /// Every scenario builds, and none of them ships an empty confirm label —
    /// the slide is the only way to say yes, so it must always say what to.
    #[test]
    fn every_scenario_builds() {
        let strings = SigningStrings::resolve(&Loc::from_env());
        for state in ALL_STATES {
            let model = build(state, &strings);
            assert!(!model.blocks.is_empty(), "{state} has no blocks");
            assert!(
                !model.confirm_label.is_empty(),
                "{state} has no slide label"
            );
            assert!(!model.dapp_name.is_empty(), "{state} has no dApp name");
        }
    }

    /// The never-unlimited mandate, asserted rather than trusted: CS5's
    /// requested chip is disabled AND its slide is off.
    #[test]
    fn unlimited_approval_cannot_be_confirmed_as_requested() {
        let strings = SigningStrings::resolve(&Loc::from_env());
        let model = build("cs5", &strings);
        assert!(!model.confirm_enabled, "cs5 must not be confirmable");
        let disabled = model.blocks.iter().any(|b| match b {
            Block::Allowance { chips, .. } => {
                chips.iter().any(|(_, state)| *state == ChipState::Disabled)
            }
            _ => false,
        });
        assert!(disabled, "cs5 must disable the requested-amount chip");
    }
}
