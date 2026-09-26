//! Wallet-flow visuals (spec 021): theme + resolved strings in, elements out.
//! No i18n keys, no page state — the same contract `wallet/components.rs`
//! established.
//!
//! The spec-015 vocabulary is reused rather than re-drawn: `activity_row`,
//! `asset_row`, `token_icon`, `identicon_avatar` and `empty_state` all come
//! from next door. What is here is what those did not already cover.

use gpui::IntoElement as _;
use gpui::prelude::FluentBuilder as _;
use gpui::{
    Div, ElementId, Hsla, InteractiveElement as _, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, div, px,
};
use qrcode::{Color as QrColorModule, QrCode};

use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::theme::{self, Theme};
use crate::wallet::components::{icon_img, identicon_avatar, token_icon_logos};

use super::fixtures::{
    FactLead, FactRow, FeeRow, FeeSpeedModel, FeeSpeedOption, FilterChip, NetworkRow,
    RecipientCard, StatusChip, StatusTone, TokenMark,
};

/// The receive network-row chain badge, measured 40 in R1. Larger than the 32
/// token icon because this row IS the network, not a token that happens to be
/// on one.
// Cards, fields and rows in these panels sit on `bg_sunken`, not `bg_raised`:
// sampled off the light mocks they are the warm grey #f5f3ef against the
// column's #fafaf8, and white makes the third column read brighter than the
// two beside it. `bg_raised` stays for what sits ON one of those surfaces —
// the network pill, the chosen segment, the chosen fee row.
pub const CHAIN_BADGE: f32 = 40.;
/// The token mark inside a line of text (fee row, fact row, notice banner).
pub const INLINE_MARK: f32 = 26.;
/// DT1L's network pill, measured off the mock: a 30 px capsule whose three
/// 16 px dots overlap by 3 px each. Overlapped, not spaced — the cluster stands
/// for "several networks", and three separate dots read as three controls.
pub const PILL_H: f32 = 30.;
pub const PILL_DOT: f32 = 16.;
pub const PILL_DOT_OVERLAP: f32 = 3.;
/// The QR card, measured 344x344 in R2 — fixed, never fluid.
pub const QR_CARD: f32 = 344.;

/// One row of DR1L: the chain, the address on it, and the two things a person
/// does with an address.
///
/// Both actions sit on the row rather than behind it. The point of the panel is
/// that ONE address serves every network, so the fastest path is to copy it
/// from whichever line you looked at, without opening anything.
/// R1's network row (`flows/ui/NetworkRow.svelte`): the chain, the address on
/// it, and the two things a person does with an address — copy it, or show it.
/// `copy` is the copy button's state and click (the tick holds 150 ms); `qr`
/// opens that network's code. `None` draws the glyph inert.
pub fn network_row(
    theme: &Theme,
    icons: &mut IconCache,
    row: &NetworkRow,
    index: usize,
    copy: Option<CopyButton>,
    qr: Option<super::panels::Click>,
) -> Div {
    let copied = copy.as_ref().is_some_and(|copy| copy.copied);
    let copy_glyph = icon_img(
        icons,
        if copied { Icon::Check } else { Icon::Copy },
        false,
        if copied {
            theme.success_base
        } else {
            theme.fg_muted
        },
        18.,
    )
    .into_any_element();
    let qr_glyph = icon_img(icons, Icon::QrCode, false, theme.fg_muted, 18.).into_any_element();
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(12.))
        // The chain's own logo over its lettered badge (078 F-11), as the
        // web's `NetworkRow` and the asset rows wear theirs: the letters
        // stand while it loads and where there is none.
        .child(
            div()
                .relative()
                .flex_none()
                .w(px(CHAIN_BADGE))
                .h(px(CHAIN_BADGE))
                .rounded(px(CHAIN_BADGE / 2.))
                .bg(row.badge)
                .flex()
                .items_center()
                .justify_center()
                .text_size(theme::text_glyph())
                .font_weight(gpui::FontWeight::BOLD)
                // The chain colours are brand fills, dark enough for white in
                // both appearances — so the mode-invariant white.
                .text_color(gpui::Hsla::from(gpui::rgb(0xffffff)))
                .child(row.code.clone())
                .children(row.logo.clone().map(|url| {
                    gpui::img(url)
                        .absolute()
                        .top_0()
                        .left_0()
                        .size(px(CHAIN_BADGE))
                        .rounded(px(CHAIN_BADGE / 2.))
                })),
        )
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(row.name.clone()),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .truncate()
                        .child(row.address.clone()),
                ),
        )
        .child(row_button(
            theme,
            ElementId::from(("network-copy", index)),
            copy_glyph,
            copy.map(|copy| copy.on_click),
        ))
        .child(row_button(
            theme,
            ElementId::from(("network-qr", index)),
            qr_glyph,
            qr,
        ))
}

/// A copy button's state and its click, as the page bound it: `copied` while
/// the tick shows.
pub struct CopyButton {
    pub copied: bool,
    pub on_click: super::panels::Click,
}

/// A 36 round glyph button on a row (`--size-control-sm`), raised on hover.
/// Its click is the row's own: it stops there, so a row that also opens
/// something on a click is not opened by the button inside it.
fn row_button(
    theme: &Theme,
    id: ElementId,
    glyph: impl gpui::IntoElement,
    on_click: Option<super::panels::Click>,
) -> gpui::AnyElement {
    let button = div()
        .size(px(36.))
        .flex_none()
        .rounded_full()
        .flex()
        .items_center()
        .justify_center()
        .child(glyph);
    match on_click {
        Some(on_click) => {
            let raised = theme.bg_raised;
            button
                .id(id)
                .cursor_pointer()
                .hover(move |el| el.bg(raised))
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_click(event, window, cx);
                })
                .into_any_element()
        }
        None => button.into_any_element(),
    }
}

/// The token mark inside a line of text.
///
/// A component and not a scaled `token_icon`: the glyph has to shrink with the
/// circle, and scaling only the box clips a three-letter ticker out of it.
/// The chain-filter pill (DT1L): overlapped dots, a label, a chevron.
pub fn network_pill(
    theme: &Theme,
    icons: &mut IconCache,
    dots: &[Hsla],
    label: SharedString,
) -> Div {
    // The cluster's width is stated, not measured. The overlap is negative
    // margins, and a row whose children carry them does not report the width
    // it actually paints — left to flex, the cluster measures zero and the
    // dots paint straight over the label beside them.
    let span = PILL_DOT + (dots.len().saturating_sub(1) as f32) * (PILL_DOT - PILL_DOT_OVERLAP);
    let mut cluster = div().flex().items_center().w(px(span)).flex_none();
    for (i, dot) in dots.iter().enumerate() {
        let mut disc = div()
            .w(px(PILL_DOT))
            .h(px(PILL_DOT))
            .rounded(px(PILL_DOT / 2.))
            .flex_none()
            .border_2()
            .border_color(theme.bg_raised)
            .bg(*dot);
        if i > 0 {
            disc = disc.ml(px(-PILL_DOT_OVERLAP));
        }
        cluster = cluster.child(disc);
    }
    div()
        .flex()
        .items_center()
        .gap(px(6.))
        .pl(px(12.))
        .pr(px(10.))
        .h(px(PILL_H))
        .rounded(px(PILL_H / 2.))
        .bg(theme.bg_raised)
        .child(cluster)
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(label),
        )
        .child(icon_img(
            icons,
            Icon::ChevronDown,
            false,
            theme.fg_muted,
            12.,
        ))
}

pub fn inline_mark(theme: &Theme, mark: &TokenMark) -> Div {
    let circle = div()
        .relative()
        .w(px(INLINE_MARK))
        .h(px(INLINE_MARK))
        .rounded(px(INLINE_MARK / 2.))
        .flex_none()
        .bg(theme.bg_sunken)
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::text_label())
        .text_color(theme.fg_muted)
        .child(SharedString::from(
            mark.ticker
                .chars()
                .take(3)
                .collect::<String>()
                .to_uppercase(),
        ));
    // The logo over the glyph, never instead of it (issue 201): gpui draws
    // nothing at all while a remote image is in flight, and an inline mark
    // that blinks out is worse than one that never changed.
    let Some(url) = mark.logos.logo_urls.first() else {
        return circle;
    };
    circle.child(
        gpui::img(url.clone())
            .absolute()
            .top_0()
            .left_0()
            .w(px(INLINE_MARK))
            .h(px(INLINE_MARK))
            .rounded(px(INLINE_MARK / 2.)),
    )
}

/// The label-value row — the single label-value primitive for the feature.
pub fn fact_row(
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    fact: &FactRow,
    copy: Option<CopyButton>,
) -> Div {
    let mut value_side = div().flex().items_center().gap(px(6.)).min_w(px(0.));

    value_side = match &fact.lead {
        FactLead::None => value_side,
        FactLead::Token(mark) => value_side.child(inline_mark(theme, mark)),
        FactLead::Identicon(seed) => {
            value_side.child(identicon_avatar(identicons, seed.as_ref(), 20.))
        }
    };

    let value = if fact.mono {
        div()
            .font_family(theme::font_mono())
            .text_size(theme::text_mono_address())
    } else {
        div().text_size(theme::text_row_sub())
    };
    value_side = value_side.child(value.text_color(theme.fg_base).child(fact.value.clone()));

    // `FactRow.svelte`: a 20 box with a 14 glyph in `fg-subtle`, a tick in
    // the success colour for 150 ms after it copies.
    if fact.copy.is_some() {
        let copied = copy.as_ref().is_some_and(|copy| copy.copied);
        let glyph = icon_img(
            icons,
            if copied { Icon::Check } else { Icon::Copy },
            false,
            if copied {
                theme.success_base
            } else {
                theme.fg_subtle
            },
            14.,
        );
        let button = div()
            .size(px(20.))
            .flex_none()
            .flex()
            .items_center()
            .justify_center()
            .child(glyph);
        value_side = value_side.child(match copy {
            Some(copy) => button
                .id(ElementId::Name(SharedString::from(format!(
                    "fact-copy-{}",
                    fact.label
                ))))
                .cursor_pointer()
                .on_click(copy.on_click)
                .into_any_element(),
            None => button.into_any_element(),
        });
    }

    let row = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .py(px(10.))
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(fact.label.clone()),
        )
        .child(value_side);
    match &fact.note {
        None => row,
        // The reason sits under its row, in the row's own quiet voice — a
        // fact about the value, not a warning about it.
        Some(note) => div().flex().flex_col().child(row.pb(px(2.))).child(
            div()
                .pb(px(10.))
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(note.clone()),
        ),
    }
}

/// The small status pill.
///
/// Four tones off the semantic colour pairs, so a chip never invents a colour —
/// and never uses the accent, which in this product means "this moves money",
/// not "this is fine".
pub fn status_chip(theme: &Theme, chip: &StatusChip) -> Div {
    let (bg, fg) = match chip.tone {
        StatusTone::Success => (theme.success_soft, theme.success_base),
        StatusTone::Warning => (theme.warning_soft, theme.warning_base),
        StatusTone::Error => (theme.error_soft, theme.error_base),
        StatusTone::Info => (theme.info_soft, theme.info_base),
    };
    div()
        .px(px(8.))
        .py(px(2.))
        .rounded(px(999.))
        .bg(bg)
        // `--text-xs` semibold (078 F-11).
        .text_size(theme::text_glyph())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(fg)
        .child(chip.text.clone())
}

/// The filled search field — the web's `flows/ui/SearchField.svelte`: 52 high
/// (`--size-control-lg`), padding 12, gap 8, radius 12 on `bg-raised`, an 18
/// glyph in `fg-subtle`, 13 text, and while focused the one hairline edge in
/// `fg-muted` every field wears (`[data-field]:focus-within`).
///
/// Filtering is live and animation-free by design; the panel does it. `field`
/// is the input the page owns — `None` (the gallery) draws the placeholder.
pub fn flow_search(
    theme: &Theme,
    icons: &mut IconCache,
    placeholder: SharedString,
    field: Option<super::panels::AddressField>,
    window: &gpui::Window,
) -> Div {
    let focused = field.as_ref().is_some_and(|f| f.focus.is_focused(window));
    let well = div()
        .flex()
        .flex_none()
        .items_center()
        .gap(px(8.))
        .h(px(52.))
        .px(px(12.))
        .rounded(px(12.))
        .bg(theme.bg_raised)
        .border_1()
        .border_color(if focused {
            theme.fg_muted
        } else {
            gpui::transparent_black()
        })
        .child(icon_img(icons, Icon::Search, false, theme.fg_subtle, 18.));
    match field {
        Some(field) => well.child(crate::ui::search_input(
            "flow-search",
            theme,
            &field.value,
            placeholder,
            &field.focus,
            window,
            field.on_change,
        )),
        None => well.child(
            div()
                .flex_1()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_subtle)
                .child(placeholder),
        ),
    }
}

/// The web's live filter (`query.trim().toLowerCase()` in each screen's
/// `shown`): nothing typed keeps every row; otherwise a row stays when what it
/// is called contains the query, ignoring case.
pub fn search_matches(query: &str, haystack: &str) -> bool {
    let query = query.trim().to_lowercase();
    query.is_empty() || haystack.to_lowercase().contains(&query)
}

/// The line a search that hides every row leaves (13 `fg-subtle`, centred,
/// 24 above and below).
pub fn search_empty(theme: &Theme, text: SharedString) -> Div {
    div()
        .py(px(24.))
        .flex()
        .justify_center()
        .text_size(theme::text_row_sub())
        .text_color(theme.fg_subtle)
        .child(text)
}

/// The token-class filter chips.
///
/// Distinct from a segmented toggle on purpose. That control divides ONE space
/// into named halves and fills its width; this is a row of independent
/// narrowings that hugs its labels.
///
/// `clicks` pairs with `chips` by position; a chip without one is drawn and
/// inert (the mock). They were inert everywhere until 2026-09-24: 全部 was
/// lit for good and the other three did nothing when pressed.
pub fn filter_chips(theme: &Theme, chips: &[FilterChip], clicks: Vec<super::panels::Click>) -> Div {
    // No wrap WITHIN the strip: a second line of chips pushes the list down
    // and changes the panel's shape depending on how long a locale's words
    // are. `min_w`/`overflow_hidden` clip as a last resort, for the locale
    // whose chips alone are wider than the column.
    let mut row = div()
        .flex()
        .gap(px(6.))
        .flex_initial()
        .min_w(px(0.))
        .overflow_hidden();
    let mut clicks = clicks.into_iter();
    for (i, chip) in chips.iter().enumerate() {
        row = row.child(super::panels::clickable(
            gpui::ElementId::from(("flow-filter-chip", i)),
            clicks.next(),
            // The web's chip (078 F-10): raised, padded 4/12, 11 medium on
            // a button's line — the sunken 5-padded chip stood taller than
            // the web's and read as a well rather than a control.
            div()
                .px(px(12.))
                .py(px(4.))
                .rounded(px(999.))
                // The selected chip inverts rather than taking the accent:
                // accent means "moves money", and narrowing a list does not.
                .bg(if chip.selected {
                    theme.fg_base
                } else {
                    theme.bg_raised
                })
                .text_size(theme::text_label())
                .line_height(gpui::relative(crate::wallet::components::LINE_NORMAL))
                .font_weight(if chip.selected {
                    gpui::FontWeight::SEMIBOLD
                } else {
                    gpui::FontWeight::MEDIUM
                })
                .text_color(if chip.selected {
                    theme.bg_base
                } else {
                    theme.fg_muted
                })
                .child(chip.label.clone()),
        ));
    }
    row
}

/// The two-segment toggle — the ONE segmented control in the product.
///
/// `clicks` makes each half a press (left, right); `None` draws the mock's
/// inert control.
pub fn segmented_toggle(
    theme: &Theme,
    id: &'static str,
    left: SharedString,
    right: SharedString,
    left_on: bool,
    clicks: Option<(super::panels::Click, super::panels::Click)>,
) -> Div {
    let seg = |label: SharedString, on: bool| {
        let base = div()
            .py(px(8.))
            .rounded(px(10.))
            .flex()
            .items_center()
            .justify_center();
        let base = if on { base.bg(theme.bg_raised) } else { base };
        base.text_size(theme::text_row_sub())
            .text_color(if on { theme.fg_base } else { theme.fg_muted })
            .child(label)
    };
    let (on_left, on_right) = match clicks {
        Some((left, right)) => (Some(left), Some(right)),
        None => (None, None),
    };
    div()
        .flex()
        .gap(px(2.))
        .p(px(2.))
        .rounded(px(12.))
        .bg(theme.bg_sunken)
        .child(
            super::panels::clickable(
                gpui::ElementId::from((id, 0usize)),
                on_left,
                seg(left, left_on),
            )
            .flex_1(),
        )
        .child(
            super::panels::clickable(
                gpui::ElementId::from((id, 1usize)),
                on_right,
                seg(right, !left_on),
            )
            .flex_1(),
        )
}

/// The monospace field. Addresses are compared character by character by the
/// people pasting them, which is the whole reason for the face.
pub fn mono_field(theme: &Theme, label: Option<SharedString>, value: SharedString) -> Div {
    // The web's `MonoField` (078 F-11): an 11 label over a raised field.
    let mut col = div().flex().flex_col().gap(px(6.));
    if let Some(label) = label {
        col = col.child(
            div()
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(label),
        );
    }
    col.child(
        div()
            .p(px(12.))
            .rounded(px(12.))
            .bg(theme.bg_raised)
            .font_family(theme::font_mono())
            .text_size(theme::text_mono_address())
            .text_color(theme.fg_base)
            .child(value),
    )
}

/// The account card above every QR: whose address this is, in full.
///
/// The address wraps to exactly two lines and never truncates. DR2L is the
/// panel a person reads an address OFF, and an ellipsis in the middle of it
/// would defeat the only job it has.
pub fn address_card(
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    name: SharedString,
    seed: &str,
    lines: (SharedString, SharedString),
    // The whole address, when there is a real one to copy. `None` in the mocks.
    copy: Option<SharedString>,
) -> gpui::Stateful<Div> {
    div()
        .id("receive-address-card")
        // No well (078 F-11): the web's `AddressCard` is a row on the page —
        // the name 15 semibold, the address in mono 11 on 1.4 beneath it.
        .flex()
        .items_center()
        .gap(px(12.))
        .py(px(12.))
        .child(identicon_avatar(identicons, seed, 36.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(name),
                )
                .child(
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_label())
                        .line_height(gpui::relative(crate::wallet::components::LINE_BODY))
                        .text_color(theme.fg_muted)
                        .child(lines.0)
                        .child(lines.1),
                ),
        )
        .child(icon_img(icons, Icon::Copy, false, theme.fg_muted, 16.))
        .when_some(copy, |el, address| {
            el.cursor_pointer()
                .on_click(move |_, _, cx: &mut gpui::App| {
                    cx.write_to_clipboard(gpui::ClipboardItem::new_string(address.to_string()));
                })
        })
}

/// The receive QR card.
///
/// White in BOTH appearances and a fixed square: a code is read by a camera,
/// inverting it in dark mode is the classic way to make one unscannable, and a
/// code that shrinks to make room for its caption stops scanning.
pub fn qr_card(theme: &Theme, centre: Option<Div>, payload: Option<&str>) -> Div {
    let ink = gpui::Hsla::from(gpui::rgb(0x1a1a18));
    let white = gpui::Hsla::from(gpui::rgb(0xffffff));
    let _ = theme;

    // A REAL code when there is something to encode.
    //
    // Until 031 this card always drew the demo pattern, including on a live
    // receive screen — a person pointed a phone at their own wallet and got
    // nothing, or worse, believed they had. The mocks keep the pattern (a
    // gallery has no address to encode and the drawing is what it is meant to
    // show); a signed-in receive screen gets a code that scans.
    if let Some(payload) = payload.filter(|text| !text.is_empty()) {
        return encoded_qr_card(payload, centre, ink, white);
    }

    const N: usize = 29;
    // The deterministic demo pattern spec 015 established, denser because this
    // card draws large. Never encodes data.
    let cells: Vec<bool> = {
        let mut s: u32 = 0xbeef;
        let mut next = move || {
            s ^= s << 13;
            s ^= s >> 17;
            s ^= s << 5;
            s
        };
        let in_finder =
            |r: usize, c: usize| (r < 7 && c < 7) || (r < 7 && c >= N - 7) || (r >= N - 7 && c < 7);
        let finder_on = |r: usize, c: usize| {
            let lr = if r < 7 { r } else { r - (N - 7) };
            let lc = if c < 7 { c } else { c - (N - 7) };
            lr.min(lc).min(6 - lr).min(6 - lc) != 1
        };
        (0..N * N)
            .map(|i| {
                let (r, c) = (i / N, i % N);
                if in_finder(r, c) {
                    finder_on(r, c)
                } else if next() & 3 == 0 {
                    false
                } else {
                    next() % 2 == 0
                }
            })
            .collect()
    };

    let module = (QR_CARD - 40.) / N as f32;
    let mut grid = div().flex().flex_col();
    for r in 0..N {
        let mut line = div().flex();
        for c in 0..N {
            line = line.child(div().w(px(module)).h(px(module)).bg(if cells[r * N + c] {
                ink
            } else {
                white
            }));
        }
        grid = grid.child(line);
    }

    let mut card = div()
        .w(px(QR_CARD))
        .h(px(QR_CARD))
        .flex_none()
        .rounded(px(16.))
        .bg(white)
        .flex()
        .items_center()
        .justify_center()
        .relative()
        .child(grid);

    if let Some(centre) = centre {
        card = card.child(
            div()
                .absolute()
                .p(px(3.))
                .rounded(px(999.))
                // The cut-out reads as part of the card, so it takes the card's
                // white rather than a theme surface that would flip underneath.
                .bg(white)
                .child(centre),
        );
    }
    card
}

/// A scannable code for a real payload.
///
/// **Black on white, always — not themed.** A QR is not UI chrome; it is a
/// target for a camera, and a camera needs dark modules on a light field
/// whatever the app's palette is. The same rule the onboarding caBLE card
/// states in its own words.
///
/// The module size is derived from the code's own width rather than fixed, so
/// a longer payload (a bigger version) still fills the same card instead of
/// overflowing it. An address is version 3-ish; an EIP-681 URI with an amount
/// is larger, and both have to fit the drawing.
fn encoded_qr_card(payload: &str, centre: Option<Div>, ink: gpui::Hsla, white: gpui::Hsla) -> Div {
    let Ok(code) = QrCode::new(payload.as_bytes()) else {
        // A payload too large to encode. Draw an empty card rather than a
        // pattern: a decorative code on a screen that is supposed to be
        // scannable is worse than an obvious blank.
        return div()
            .w(px(QR_CARD))
            .h(px(QR_CARD))
            .flex_none()
            .rounded(px(16.))
            .bg(white);
    };
    let width = code.width();
    let colors = code.to_colors();
    // The quiet zone is part of the spec, not padding: a code drawn edge to
    // edge on a card is one a camera can fail to find.
    #[allow(clippy::cast_precision_loss, reason = "a QR is at most 177 modules")]
    let module = (QR_CARD - 40.) / width as f32;
    let mut grid = div().flex().flex_col();
    for row in 0..width {
        let mut line = div().flex();
        for col in 0..width {
            let dark = matches!(colors.get(row * width + col), Some(QrColorModule::Dark));
            line = line.child(
                div()
                    .w(px(module))
                    .h(px(module))
                    .bg(if dark { ink } else { white }),
            );
        }
        grid = grid.child(line);
    }

    let mut card = div()
        .w(px(QR_CARD))
        .h(px(QR_CARD))
        .flex_none()
        .rounded(px(16.))
        .bg(white)
        .flex()
        .items_center()
        .justify_center()
        .relative()
        .child(grid);
    if let Some(centre) = centre {
        card = card.child(
            div()
                .absolute()
                .p(px(3.))
                .rounded(px(999.))
                .bg(white)
                .child(centre),
        );
    }
    card
}

/// The network-fee row.
///
/// A row and not a card: the fee is a fact about the transfer. Clicking it
/// changes which token pays; the refresh control beside it (spec 068, on the
/// desktop since 069) measures again, and sits OUTSIDE the row's own click so
/// a refresh never opens the fee-coin sheet. The speed control lives under it
/// ([`fee_speed`]), folded.
pub fn fee_row(theme: &Theme, icons: &mut IconCache, fee: &FeeRow) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(8.))
        .p(px(12.))
        .rounded(px(12.))
        // The web's raised surface: the fee sits on the same card colour as
        // the token and the recipient above it.
        .bg(theme.bg_raised)
        .child(
            div()
                .flex_1()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(fee.label.clone()),
        )
        .child(inline_mark(theme, &fee.mark))
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_base)
                .child(fee.value.clone()),
        )
        .child(icon_img(
            icons,
            Icon::ChevronRight,
            false,
            theme.fg_muted,
            12.,
        ))
}

/// The refresh control's face: the icon, dimmed while a measurement is out
/// — whoever started it — so a second tap is never ambiguous.
pub fn fee_refresh_icon(theme: &Theme, icons: &mut IconCache, fee: &FeeRow) -> Div {
    div()
        .flex()
        .items_center()
        .justify_center()
        .size(px(36.))
        .mr(px(4.))
        .rounded_full()
        .hover(|el| el.bg(theme.bg_sunken))
        .child(icon_img(
            icons,
            Icon::RefreshCw,
            false,
            if fee.refreshing {
                theme.fg_subtle
            } else {
                theme.fg_muted
            },
            14.,
        ))
}

/// The stale line under the fee row: calm, muted, and always drawn at its
/// full height so Continue never moves under a pointer when it appears.
pub fn fee_stale_line(theme: &Theme, fee: &FeeRow) -> Div {
    div()
        .min_h(px(16.))
        .px(px(12.))
        .text_size(theme::text_label())
        .text_color(theme.fg_subtle)
        .child(fee.stale_note.clone().unwrap_or_default())
}

/// The speed control's folded summary: the word, the tier in force, and the
/// chevron that says it opens.
pub fn fee_speed_summary(theme: &Theme, icons: &mut IconCache, speed: &FeeSpeedModel) -> Div {
    div()
        .flex()
        .items_center()
        // The web's folded row (078 F-11): 11, padded 4/12.
        .gap(px(8.))
        .px(px(12.))
        .py(px(4.))
        .child(
            div()
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .child(speed.label.clone()),
        )
        .child(
            div()
                .flex_1()
                .text_size(theme::text_label())
                .text_color(theme.fg_base)
                .flex()
                .justify_end()
                .child(speed.value.clone()),
        )
        .child(icon_img(
            icons,
            if speed.open {
                Icon::ChevronUp
            } else {
                Icon::ChevronDown
            },
            false,
            theme.fg_muted,
            12.,
        ))
}

/// A one-line note under the summary — the free upgrade's reason, the
/// one-shot promise, or the one-speed statement.
pub fn fee_speed_note(theme: &Theme, text: &SharedString) -> Div {
    div()
        .px(px(12.))
        .pb(px(4.))
        .text_size(theme::text_label())
        .text_color(theme.fg_subtle)
        .child(text.clone())
}

/// One option, opened: its name and its own fee on the first line, its gas
/// bid under the fee, and what the speed buys under both.
pub fn fee_speed_option(
    theme: &Theme,
    icons: &mut IconCache,
    speed: &FeeSpeedModel,
    option: &FeeSpeedOption,
) -> Div {
    let mut body = div().flex().flex_col().gap(px(2.)).flex_1().min_w(px(0.));
    body = body.child(
        div()
            .flex()
            .items_center()
            .gap(px(8.))
            .child(
                div()
                    .flex_1()
                    .text_size(theme::text_row_sub())
                    .text_color(if option.selected {
                        theme.accent
                    } else {
                        theme.fg_base
                    })
                    .child(option.label.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(option.value.clone()),
            ),
    );
    if speed.gas_price_line {
        // Named, because an unnamed "3,244 wei" under a fee reads as a second
        // charge; held open empty while the set is measuring, so the option
        // does not lose a line and regain it.
        body = body.child(
            div()
                .flex()
                .justify_end()
                .gap(px(6.))
                .min_h(px(14.))
                .text_size(theme::text_label())
                .text_color(theme.fg_subtle)
                .children(option.gas_price.as_ref().map(|gas| {
                    div()
                        .flex()
                        .gap(px(6.))
                        .child(speed.gas_price_label.clone())
                        .child(div().font_family(theme::font_mono()).child(gas.clone()))
                })),
        );
    }
    body = body.child(
        div()
            .text_size(theme::text_label())
            .text_color(theme.fg_subtle)
            .child(option.detail.clone()),
    );
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .px(px(12.))
        .py(px(8.))
        .rounded(px(10.))
        .when(option.selected, |row| row.bg(theme.bg_sunken))
        .child(body)
        .child(
            div().w(px(14.)).pt(px(2.)).children(
                option
                    .selected
                    .then(|| icon_img(icons, Icon::Check, false, theme.accent, 14.)),
            ),
        )
}

/// DSD2bL's split row: one of N people, what they get, and the way to drop them.
/// One payee in a split, with the two things that make a split editable: the
/// amount THIS row gets, and the way to drop it.
///
/// `None` for either draws the mock's read-only card — which is what the
/// gallery gets, and what this screen was on the desktop until spec 033.
pub struct RecipientRowActions {
    pub amount: Option<crate::flows::panels::AddressField>,
    /// The row's address, typed (078 F-06) — and the book for this row.
    pub address: Option<crate::flows::panels::AddressField>,
    pub pick: Option<crate::flows::panels::Click>,
    pub remove: Option<crate::flows::panels::Click>,
}

pub fn recipient_card(
    theme: &Theme,
    icons: &mut IconCache,
    identicons: &mut IdenticonCache,
    recipient: &RecipientCard,
    index: usize,
    row: RecipientRowActions,
    window: &gpui::Window,
) -> Div {
    let RecipientRowActions {
        amount,
        address,
        pick,
        remove,
    } = row;
    // Live, the row is also where the person is typed (the web's
    // `RecipientCard`, spec 028 Phase 10): a well holding the address, with
    // the book's door beside it. The drawn card keeps its name line.
    let who: gpui::AnyElement = match address {
        // At rest a filled row reads as the drawn card does — the address's
        // two ends — and a click puts the whole of it back in hand (the web's
        // `.reading` over the input): forty clipped hex strings in boxes is a
        // form, not a payroll.
        Some(field) if !field.value.is_empty() && !field.focus.is_focused(window) => {
            let focus = field.focus.clone();
            let mut reading = div().flex().items_center().gap(px(4.)).child(
                div()
                    .id(gpui::ElementId::from(("split-reading", index)))
                    .cursor_text()
                    .font_family(theme::font_mono())
                    .text_size(theme::text_mono_address())
                    .text_color(theme.fg_base)
                    .child(gpui::SharedString::from(
                        crate::wallet::live::shorten_address(&field.value),
                    ))
                    .on_click(move |_, window, cx| focus.focus(window, cx)),
            );
            if let Some(pick) = pick {
                reading = reading.child(crate::flows::panels::clickable(
                    gpui::ElementId::from(("split-pick", index)),
                    Some(pick),
                    div().p(px(4.)).rounded_full().child(icon_img(
                        icons,
                        Icon::NavContacts,
                        false,
                        theme.fg_muted,
                        14.,
                    )),
                ));
            }
            reading.into_any_element()
        }
        Some(field) => {
            let mut well = div()
                .flex()
                .items_center()
                .gap(px(4.))
                .px(px(8.))
                .rounded(px(8.))
                .bg(theme.bg_base)
                .border_1()
                .border_color(if field.focus.is_focused(window) {
                    theme.fg_muted
                } else {
                    theme.border_card
                })
                .child(crate::ui::bare_text_field(
                    gpui::ElementId::from(("split-address", index)),
                    theme,
                    &field.value,
                    field.placeholder.clone(),
                    &field.focus,
                    window,
                    field.on_change,
                ));
            if let Some(pick) = pick {
                well = well.child(crate::flows::panels::clickable(
                    gpui::ElementId::from(("split-pick", index)),
                    Some(pick),
                    div().p(px(4.)).rounded_full().child(icon_img(
                        icons,
                        Icon::NavContacts,
                        false,
                        theme.fg_muted,
                        14.,
                    )),
                ));
            }
            well.into_any_element()
        }
        None => div()
            .font_family(theme::font_mono())
            .text_size(theme::text_mono_address())
            .text_color(theme.fg_base)
            .child(recipient.name.clone())
            .into_any_element(),
    };
    // A raised card padded 12, as the web's `RecipientCard` (078 F-11); its
    // wells are the page colour inside it.
    let card = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(12.))
        .bg(theme.bg_raised)
        .child(identicon_avatar(identicons, recipient.seed.as_ref(), 28.))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(recipient.ordinal.clone()),
                )
                .child(who),
        )
        .child(match amount {
            // Live: this row's own amount, typed. A split whose rows cannot be
            // given amounts is a screen that can be filled in and never sent.
            Some(field) => div()
                .w(px(120.))
                .flex_none()
                .child(crate::ui::text_field(
                    gpui::ElementId::from(("split-amount", index)),
                    theme,
                    &crate::ui::NameFieldStrings {
                        label: gpui::SharedString::from(""),
                        placeholder: field.placeholder.clone(),
                        helper: gpui::SharedString::from(""),
                        too_long_hint: gpui::SharedString::from(""),
                    },
                    &field.value,
                    false,
                    false,
                    &field.focus,
                    window,
                    field.on_change,
                ))
                .into_any_element(),
            None => div()
                .text_size(theme::text_row_title())
                .text_color(theme.fg_base)
                .child(recipient.amount.clone())
                .into_any_element(),
        })
        // The X has been drawn on this card since spec 021 and did nothing.
        .child(crate::flows::panels::clickable(
            gpui::ElementId::from(("split-remove", index)),
            remove,
            div().p(px(4.)).rounded(px(6.)).child(icon_img(
                icons,
                Icon::X,
                false,
                theme.fg_subtle,
                14.,
            )),
        ));
    // What the core says about this row, under the row it is about — a
    // repeated payee (issue 203), a field it will not take. Amber: the person
    // is still filling the form in, and the gate's own words say what blocks.
    let mut block = div().flex().flex_col().gap(px(4.)).child(card);
    for note in &recipient.notes {
        block = block.child(
            div()
                .pl(px(48.))
                .text_size(theme::text_label())
                .text_color(theme.warning_base)
                .child(note.clone()),
        );
    }
    block
}

/// The send form's token card: which token, off which chain, out of how much.
pub fn token_header_card(
    theme: &Theme,
    mark: &TokenMark,
    symbol: SharedString,
    detail: SharedString,
    max: Option<SharedString>,
) -> Div {
    let mut card = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .p(px(12.))
        .rounded(px(12.))
        // The web's `TokenHeaderCard`: a raised card, the Max chip sunken on it.
        .bg(theme.bg_raised)
        .child(token_icon_logos(
            theme,
            mark.ticker.as_ref(),
            mark.badge,
            &mark.logos,
        ))
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .child(symbol),
                )
                .gap(px(2.))
                .child(
                    // `--text-sm`, as the web's `TokenHeaderCard` detail.
                    div()
                        .text_size(theme::text_label())
                        .text_color(theme.fg_muted)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(detail),
                ),
        );
    if let Some(max) = max {
        card = card.child(max_chip(theme, max));
    }
    card
}

/// The token card's Max chip. A live form passes the card no label and hangs
/// this, clickable, where the card would have drawn it — one Max, not two.
pub fn max_chip(theme: &Theme, max: SharedString) -> Div {
    div()
        .px(px(12.))
        .py(px(4.))
        .rounded(px(999.))
        .bg(theme.bg_sunken)
        .text_size(theme::text_row_sub())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(theme.fg_base)
        .child(max)
}

/// The web's `Button` (`ui/Button.svelte`), one shape for every CTA: at
/// least 52 high (`--size-control-lg`), padding 8/24, 17 semibold
/// (`--text-xl`), hover at 0.92. The desktop's buttons were 37 high and 13
/// regular, the largest single reason the flows read as a rough copy.
fn button_base(label: SharedString) -> Div {
    button_face(label).hover(|el| el.opacity(0.92))
}

/// The CTA's shape and type without its hover — gpui takes ONE hover style
/// per element, so a button that must not react is built without it rather
/// than given a second.
fn button_face(label: SharedString) -> Div {
    div()
        .w_full()
        .min_h(px(52.))
        .px(px(24.))
        .py(px(8.))
        .flex()
        .items_center()
        .justify_center()
        .text_size(theme::text_button())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .child(label)
}

/// `secondary`, pill-shaped: a hairline in `--color-border-strong` and muted
/// text — the outline every secondary action wears.
pub fn ghost_button(theme: &Theme, label: SharedString) -> Div {
    button_base(label)
        .rounded(px(999.))
        .border_1()
        .border_color(theme.border_strong)
        .text_color(theme.fg_muted)
}

/// `secondary`, `rounded`: the outline at the web's 12 radius — the "cancel"
/// under a confirm sheet's answer, which the web does not draw as a pill.
pub fn secondary_button(theme: &Theme, label: SharedString) -> Div {
    ghost_button(theme, label).rounded(px(12.))
}

/// The accent CTA that cannot act yet: the same fill at
/// `--opacity-disabled`, with no hover to lift it. The caller withholds the
/// click and the pointer. (It took the finished button once, and stacking a
/// second hover on it panics a debug build.)
pub fn disabled_accent_button(theme: &Theme, label: SharedString) -> Div {
    accent_fill(button_face(label), theme).opacity(theme::OPACITY_DISABLED)
}

/// `primary`, `rounded`: the accent CTA. In this product the accent means
/// "this moves the money" — and "done", on a receipt that has landed.
pub fn accent_button(theme: &Theme, label: SharedString) -> Div {
    accent_fill(button_base(label), theme)
}

fn accent_fill(button: Div, theme: &Theme) -> Div {
    button
        .rounded(px(12.))
        .bg(theme.accent)
        .text_color(gpui::Hsla::from(gpui::rgb(0xffffff)))
}

/// `danger`: filled with the error colour — delete a transaction, a contact,
/// a group. Only where the web draws one.
pub fn danger_button(theme: &Theme, label: SharedString) -> Div {
    button_base(label)
        .rounded(px(12.))
        .bg(theme.error_base)
        .text_color(gpui::Hsla::from(gpui::rgb(0xffffff)))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The web's filter: trimmed, case-blind, a substring of what the row is
    /// called; nothing typed keeps everything (078 X-05).
    #[test]
    fn a_search_matches_the_way_the_web_filters() {
        assert!(search_matches("", "USDC Ethereum"));
        assert!(search_matches("   ", "USDC Ethereum"));
        assert!(search_matches(" usdc ", "USDC Ethereum"));
        assert!(search_matches("ETHER", "USDC Ethereum"));
        assert!(search_matches("c eth", "USDC Ethereum"));
        assert!(!search_matches("gnosis", "USDC Ethereum"));
    }

    /// The card encodes a payload that a camera could actually read back.
    ///
    /// Not a pixel comparison: what must hold is that a real payload produces a
    /// DIFFERENT matrix per payload and the same one twice, which the demo
    /// pattern — identical for every wallet on earth — cannot do.
    #[test]
    fn a_real_payload_produces_a_real_code() {
        const ADDR: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let code =
            QrCode::new(ADDR.as_bytes()).unwrap_or_else(|_| unreachable!("an address fits a QR"));
        // Decoding is not something a QR encoder offers, so the check is the
        // property that separates a code from a picture: the modules depend on
        // the payload.
        let other = QrCode::new("0x0000000000000000000000000000000000000001".as_bytes())
            .unwrap_or_else(|_| unreachable!("also fits"));
        assert_ne!(
            code.to_colors(),
            other.to_colors(),
            "two addresses produced the same code"
        );
        let again = QrCode::new(ADDR.as_bytes()).unwrap_or_else(|_| unreachable!("deterministic"));
        assert_eq!(code.to_colors(), again.to_colors());

        // An EIP-681 URI with an amount is longer and must still encode.
        assert!(
            QrCode::new(
                format!("ethereum:{ADDR}@100/transfer?address={ADDR}&uint256=1500000000000000000")
                    .as_bytes()
            )
            .is_ok(),
            "a payment URI must fit"
        );
    }
}
