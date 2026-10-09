//! Signing visuals (spec 022 §3): theme + resolved strings in, elements out.
//!
//! The block renderer is universal — blocks in mock order, out. Nothing here
//! knows what a swap or a permit IS, which is what lets all 33 scenarios, and
//! the ones nobody has drawn yet, come out of one code path.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    Div, Hsla, InteractiveElement as _, ParentElement, SharedString,
    StatefulInteractiveElement as _, Styled, div, px,
};

use crate::icons::{Icon, IconCache};
use crate::identicon::IdenticonCache;
use crate::theme::{self, Theme};
use crate::wallet::components::{icon_img, openable_identicon};

use super::Tone;
use super::fixtures::{Block, ChipState, FeeModel, SigningModel};
use crate::explore::components::letter_avatar;

/// The confirm's element id — the stable hook a test finds it by, whatever
/// its words say ("Confirm swap", "Sign", "备份公钥", …) and whether or not
/// the core has armed it.
pub const CONFIRM_ID: &str = "signing-confirm";

/// Where a fee coin's shortfall reason starts: past the row's padding (8),
/// the coin's mark (the wallet row icon, 40) and the gap (12) — under the
/// coin's name.
const FEE_REASON_INDENT: f32 = 8. + theme::WALLET_ROW_ICON + 12.;

fn tone_color(theme: &Theme, tone: Tone) -> Hsla {
    match tone {
        Tone::Neutral => theme.fg_base,
        Tone::Accent => theme.accent,
        Tone::Success => theme.success_base,
        Tone::Caution => theme.warning_base,
        Tone::Danger => theme.error_base,
    }
}

/// Who is asking and on which network — the header's facts alone, kept by
/// the page so the column's ending (spec 079: the tick, "not landed yet")
/// still names the site after the core has closed the request.
#[derive(Clone)]
pub struct HeaderModel {
    pub dapp_name: SharedString,
    pub dapp_host: SharedString,
    pub dapp_letter: SharedString,
    pub dapp_tint: Hsla,
    /// The wallet asked itself (the core's `SignRequestView.first_party`):
    /// there is no requester to name, so no header is drawn at all — the
    /// column's scaffold already says what this is and owns the ✕.
    pub first_party: bool,
    pub dapp_icon_urls: Vec<SharedString>,
    pub network_name: SharedString,
    pub network_dot: Hsla,
    pub network_logo: Option<SharedString>,
}

impl HeaderModel {
    #[must_use]
    pub fn of(model: &SigningModel) -> Self {
        Self {
            dapp_name: model.dapp_name.clone(),
            dapp_host: model.dapp_host.clone(),
            dapp_letter: model.dapp_letter.clone(),
            dapp_tint: model.dapp_tint,
            first_party: model.first_party,
            dapp_icon_urls: model.dapp_icon_urls.clone(),
            network_name: model.network_name.clone(),
            network_dot: model.network_dot,
            network_logo: model.network_logo.clone(),
        }
    }
}

/// The dApp header: who is asking, and on which network — `None` for the
/// wallet's own request ([`HeaderModel::first_party`]).
pub fn header(theme: &Theme, model: &SigningModel) -> Option<Div> {
    header_view(theme, &HeaderModel::of(model))
}

/// The header from its facts alone.
///
/// `None` when the wallet asked itself (the key backup): no app mark, no
/// "Vela Wallet", no network chip. The column's scaffold already titles the
/// request and owns the ✕, so nothing is drawn in the header's place — the
/// caller adds it with `.children(…)`, which leaves no gap where it was. The
/// network the request is on is the reading's first row instead (the core's
/// backup reading names it).
pub fn header_view(theme: &Theme, model: &HeaderModel) -> Option<Div> {
    if model.first_party {
        return None;
    }
    let label = vela_core::app::browser_load::site_label(&model.dapp_name, &model.dapp_host);
    // The site's own icon over its initial: a picture that fails to load
    // draws nothing, so the letter beneath is what stays.
    let mut mark = div()
        .relative()
        .size(px(36.))
        .flex_none()
        .child(letter_avatar(
            model.dapp_letter.clone(),
            model.dapp_tint,
            36.,
        ));
    for url in model.dapp_icon_urls.iter().rev() {
        mark = mark.child(
            gpui::img(url.clone())
                .absolute()
                .top_0()
                .left_0()
                .size(px(36.))
                .rounded_full(),
        );
    }
    let header = div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(mark)
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .flex_1()
                .min_w(px(0.))
                .child(
                    div()
                        .text_size(theme::text_row_title())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_base)
                        .truncate()
                        .child(SharedString::from(label.name)),
                )
                // A site whose name IS its host says it once (spec 079 F14:
                // "127.0.0.1:8137" over "127.0.0.1:8137") — the core's rule
                // (spec 082 RE7), not this file's comparison.
                .children(label.host_line.map(|host| {
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .truncate()
                        .child(SharedString::from(host))
                })),
        )
        .child(
            div()
                .h(px(26.))
                .px(px(12.))
                .rounded_full()
                .bg(theme.bg_sunken)
                .flex()
                .items_center()
                .gap(px(8.))
                .child({
                    // The chain's logo over the drawn dot.
                    let mut chain = div()
                        .relative()
                        .size(px(16.))
                        .flex_none()
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(div().size(px(8.)).rounded_full().bg(model.network_dot));
                    if let Some(url) = &model.network_logo {
                        chain = chain.child(
                            gpui::img(url.clone())
                                .absolute()
                                .top_0()
                                .left_0()
                                .size(px(16.))
                                .rounded_full(),
                        );
                    }
                    chain
                })
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(model.network_name.clone()),
                ),
        );
    Some(header)
}

/// The intent as the sheet's headline (issue #314): a request with no figure
/// to lead with — the wallet's own — leads with what it does. The base ink
/// unless its tone is a warning.
pub fn headline(theme: &Theme, text: SharedString, tone: Tone) -> Div {
    div()
        .text_size(theme::text_panel_title())
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(if matches!(tone, Tone::Caution | Tone::Danger) {
            tone_color(theme, tone)
        } else {
            theme.fg_base
        })
        .child(text)
}

/// One block, rendered.
/// One block, with the allowance chips ARMED.
///
/// The chips are a control, not a picture, and only when a machine is behind
/// them: `actions` carries one per chip, in the order the block lists them.
/// Passing none draws exactly what the gallery draws — the same rule the confirm
/// follows (spec 032 phase 21), and what keeps the 33 drawn scenarios
/// pixel-identical after the editor went live.
pub fn block_with_actions(
    theme: &Theme,
    icons: &mut IconCache,
    item: &Block,
    actions: Vec<Option<crate::flows::panels::Click>>,
    field: Option<crate::flows::panels::AddressField>,
    window: &gpui::Window,
) -> Div {
    block_inner(theme, icons, item, actions, field, Some(window))
}

pub fn block(theme: &Theme, icons: &mut IconCache, item: &Block) -> Div {
    block_inner(theme, icons, item, Vec::new(), None, None)
}

fn block_inner(
    theme: &Theme,
    icons: &mut IconCache,
    item: &Block,
    mut actions: Vec<Option<crate::flows::panels::Click>>,
    mut input: Option<crate::flows::panels::AddressField>,
    window: Option<&gpui::Window>,
) -> Div {
    let _ = &mut actions;
    match item {
        Block::Intent { text, tone } => div()
            .text_size(theme::text_row_sub())
            .font_weight(gpui::FontWeight::SEMIBOLD)
            .text_color(if *tone == Tone::Neutral {
                theme.fg_muted
            } else {
                tone_color(theme, *tone)
            })
            .child(text.clone()),

        Block::Amount {
            line,
            card,
            note,
            compact,
        } => {
            let ink = if line.tone == Tone::Neutral {
                theme.fg_base
            } else {
                tone_color(theme, line.tone)
            };
            let mut col = div().flex().flex_col().gap(px(4.));
            if let Some(caption) = line.caption.clone().filter(|_| !*card) {
                col = col.child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(caption),
                );
            }
            let mut value_row = div().flex().items_center().gap(px(8.)).child(
                div()
                    .text_size(if *card {
                        px(20.)
                    } else if *compact {
                        px(26.)
                    } else {
                        px(32.)
                    })
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(ink)
                    .child(SharedString::from(format!("{}{}", line.sign, line.value))),
            );
            // The coin's token mark (the core's kind rule: a coin wears
            // `token_mark`) — its logo over its glyph, the inline mark every
            // shell draws at 26 — not a one-letter disc in a brand colour.
            if let Some(mark) = &line.token {
                value_row = value_row.child(crate::flows::components::inline_mark(theme, mark));
            }
            col = col.child(
                value_row.child(
                    div()
                        .text_size(theme::text_row_title())
                        .text_color(if line.tone == Tone::Neutral {
                            theme.fg_muted
                        } else {
                            ink
                        })
                        .child(line.symbol.clone()),
                ),
            );
            if let Some(text) = note.clone().or_else(|| line.fiat.clone()) {
                col = col.child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(if note.is_some() {
                            theme.fg_muted
                        } else {
                            theme.fg_subtle
                        })
                        .child(text),
                );
            }
            if *card {
                div()
                    .p(px(16.))
                    .rounded(px(16.))
                    .bg(if line.tone == Tone::Danger {
                        theme.error_soft
                    } else {
                        theme.bg_sunken
                    })
                    .border_1()
                    .border_color(if line.tone == Tone::Danger {
                        theme.error_base
                    } else {
                        theme.bg_sunken
                    })
                    .child(col)
            } else {
                col
            }
        }

        Block::Swap { pay, receive } => div()
            .flex()
            .flex_col()
            .gap(px(12.))
            .child(block(
                theme,
                icons,
                &Block::Amount {
                    line: pay.clone(),
                    card: false,
                    note: None,
                    compact: true,
                },
            ))
            .child(
                div()
                    .w(px(32.))
                    .h(px(32.))
                    .rounded_full()
                    .bg(theme.bg_sunken)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(icon_img(icons, Icon::ArrowDown, false, theme.fg_muted, 14.)),
            )
            .child(block(
                theme,
                icons,
                &Block::Amount {
                    line: receive.clone(),
                    card: false,
                    note: None,
                    compact: true,
                },
            )),

        Block::Nft { id, collection } => div()
            .flex()
            .flex_col()
            .gap(px(4.))
            .child(
                div()
                    .text_size(px(32.))
                    .font_weight(gpui::FontWeight::BOLD)
                    .text_color(theme.fg_base)
                    .child(id.clone()),
            )
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_muted)
                    .child(collection.clone()),
            ),

        Block::Sentence { text, tone } => div()
            .text_size(theme::text_row_title())
            .text_color(tone_color(theme, *tone))
            .child(text.clone()),

        Block::Allowance {
            label,
            value,
            value_tone,
            chips,
            note,
            resulting_total,
            custom,
        } => {
            let mut chip_row = div().flex().flex_wrap().gap(px(8.));
            let mut chip_actions = actions.into_iter();
            for (index, (chip_label, state)) in chips.iter().enumerate() {
                let selected = *state == ChipState::Selected;
                let disabled = *state == ChipState::Disabled;
                // A disabled chip is never armed, whatever the caller passed:
                // "Requested" greyed out is this wallet refusing that amount,
                // and a click that took it would be the refusal undone by the
                // control that states it.
                let action = chip_actions.next().flatten().filter(|_| !disabled);
                let chip = div()
                    .h(px(36.))
                    .px(px(12.))
                    .rounded_full()
                    .border_1()
                    .border_color(if selected {
                        theme.accent
                    } else {
                        theme.outline_strong
                    })
                    .flex()
                    .items_center()
                    .opacity(if disabled { 0.45 } else { 1.0 })
                    .text_size(theme::text_row_sub())
                    .text_color(if selected {
                        theme.accent
                    } else {
                        theme.fg_base
                    })
                    .child(chip_label.clone());
                // Stateful only when it is a control. A gallery chip stays the
                // element the drawn scenarios have always rendered.
                chip_row = match action {
                    Some(action) => chip_row.child(
                        chip.id(("allowance-chip", index))
                            .cursor_pointer()
                            .on_click(move |event, window, cx| action(event, window, cx)),
                    ),
                    None => chip_row.child(chip),
                };
            }
            // The typed cap, under the chips. A field with no input bound —
            // the gallery's — still shows what is there and what is wrong
            // with it, because that is the state being reviewed.
            let live = input.take().zip(window);
            let field = custom.as_ref().map(|input| {
                // A real input when the page bound one; the drawn field
                // otherwise, which is what the gallery reviews. Same
                // primitive as the send screen's amount, so a cap and an
                // amount are typed into the same-looking thing.
                if let Some((field, window)) = live {
                    let strings = crate::ui::NameFieldStrings {
                        label: input.symbol.clone(),
                        placeholder: input.placeholder.clone(),
                        helper: SharedString::from(""),
                        too_long_hint: SharedString::from(""),
                    };
                    let mut col = div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(crate::ui::text_field(
                            "allowance-cap",
                            theme,
                            &strings,
                            &field.value,
                            false,
                            false,
                            &field.focus,
                            window,
                            field.on_change,
                        ));
                    if let Some(error) = &input.error {
                        col = col.child(
                            div()
                                .text_size(theme::text_row_sub())
                                .text_color(tone_color(theme, Tone::Danger))
                                .child(error.clone()),
                        );
                    }
                    return col;
                }
                let mut col = div().flex().flex_col().gap(px(6.)).child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(px(8.))
                        .p(px(12.))
                        .rounded(px(12.))
                        .bg(theme.bg_sunken)
                        .child(
                            div()
                                .font_family(theme::font_mono())
                                .text_size(theme::text_mono_address())
                                .text_color(if input.value.is_empty() {
                                    theme.fg_subtle
                                } else {
                                    theme.fg_base
                                })
                                .child(if input.value.is_empty() {
                                    input.placeholder.clone()
                                } else {
                                    input.value.clone()
                                }),
                        )
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_muted)
                                .child(input.symbol.clone()),
                        ),
                );
                if let Some(error) = &input.error {
                    col = col.child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(tone_color(theme, Tone::Danger))
                            .child(error.clone()),
                    );
                }
                col
            });

            let mut card = div()
                .p(px(16.))
                .rounded(px(16.))
                .border_1()
                .border_color(theme.border_card)
                .flex()
                .flex_col()
                .gap(px(12.))
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(theme::text_row_sub())
                                .text_color(theme.fg_muted)
                                .child(label.clone()),
                        )
                        .child(
                            div()
                                .text_size(px(20.))
                                .font_weight(gpui::FontWeight::BOLD)
                                .text_color(if *value_tone == Tone::Neutral {
                                    theme.fg_base
                                } else {
                                    tone_color(theme, *value_tone)
                                })
                                .child(value.clone()),
                        ),
                )
                .child(chip_row);
            if let Some(field) = field {
                card = card.child(field);
            }
            if let Some(note) = note {
                card = card.child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(note.clone()),
                );
            }
            let mut wrap = div().flex().flex_col().gap(px(12.)).child(card);
            if let Some((total_label, total_value)) = resulting_total {
                wrap = wrap.child(kv_row(
                    theme,
                    total_label,
                    total_value,
                    Tone::Neutral,
                    false,
                ));
            }
            wrap
        }

        Block::Party {
            label,
            name,
            address,
            badge,
        } => {
            // May shrink, so the badge beside it stays on the column (078
            // G-06): "Unverified" was pushed out past the edge by a full
            // address that had nowhere to break.
            let mut who = div()
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
                        .child(name.clone()),
                );
            if let Some(address) = address {
                who = who.child(
                    // Middle-truncated: both ends of an address are what a
                    // person checks, and hex has no space to wrap at.
                    div()
                        .font_family(theme::font_mono())
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis_middle()
                        .child(address.clone()),
                );
            }
            let mut line = div()
                .flex()
                .items_start()
                .justify_between()
                .gap(px(12.))
                .child(who);
            if let Some((text, tone)) = badge {
                let (ink, fill) = match tone {
                    Tone::Success => (theme.success_base, theme.success_soft),
                    Tone::Caution => (theme.warning_base, theme.warning_soft),
                    Tone::Danger => (theme.error_base, theme.error_soft),
                    _ => (theme.fg_muted, theme.bg_sunken),
                };
                // The web's `.badge`: never shrinks, 11, padded 2/8.
                line = line.child(
                    div()
                        .flex_none()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .bg(fill)
                        .text_size(theme::text_label())
                        .text_color(ink)
                        .whitespace_nowrap()
                        .child(text.clone()),
                );
            }
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(label.clone()),
                )
                .child(line)
        }

        Block::Rows(rows) => {
            let mut col = div().flex().flex_col();
            for (label, value, tone, mono) in rows {
                col = col.child(kv_row(theme, label, value, *tone, *mono));
            }
            col
        }

        Block::Warning { tone, text } => {
            let danger = *tone == Tone::Danger;
            let ink = if danger {
                theme.error_base
            } else {
                theme.warning_base
            };
            // The web's `WarningBanner` (078 G-06): padded 12/16, a 16 glyph,
            // a caution edged in the soft warning border (the danger one in
            // full red), and words that WRAP — without `min_w(0)` a sentence
            // with no break it liked ran straight out of the card.
            div()
                .py(px(12.))
                .px(px(16.))
                .rounded(px(12.))
                .bg(if danger {
                    theme.error_soft
                } else {
                    theme.warning_soft
                })
                .border_1()
                .border_color(if danger {
                    theme.error_base
                } else {
                    theme.warning_border
                })
                .flex()
                .items_start()
                .gap(px(12.))
                .child(div().flex_none().mt(px(2.)).child(icon_img(
                    icons,
                    Icon::TriangleAlert,
                    false,
                    ink,
                    16.,
                )))
                .child(
                    div()
                        .flex_1()
                        .min_w(px(0.))
                        .text_size(theme::text_row_sub())
                        .text_color(ink)
                        .child(text.clone()),
                )
        }

        Block::Positive(text) => div()
            .p(px(12.))
            .rounded(px(12.))
            .bg(theme.bg_sunken)
            .flex()
            .items_center()
            .gap(px(12.))
            .child(icon_img(icons, Icon::Check, false, theme.fg_base, 14.))
            .child(
                div()
                    .text_size(theme::text_row_sub())
                    .text_color(theme.fg_base)
                    .child(text.clone()),
            ),

        Block::Code { lines, note } => {
            let mut col = div()
                .p(px(16.))
                .rounded(px(12.))
                .bg(theme.bg_sunken)
                .flex()
                .flex_col()
                .gap(px(2.))
                .font_family("monospace")
                .text_size(theme::text_row_sub());
            for line in lines {
                col = col.child(div().text_color(theme.fg_base).child(line.clone()));
            }
            if let Some(note) = note {
                col = col.child(div().text_color(theme.fg_muted).child(note.clone()));
            }
            col
        }

        Block::Card { title, rows, tone } => {
            let mut col = div()
                .px(px(16.))
                .py(px(4.))
                .rounded(px(16.))
                .border_1()
                .border_color(if *tone == Tone::Danger {
                    theme.error_base
                } else {
                    theme.border_card
                })
                .flex()
                .flex_col();
            if let Some(title) = title {
                col = col.child(
                    div()
                        .py(px(8.))
                        .text_size(theme::text_row_sub())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_muted)
                        .child(title.clone()),
                );
            }
            for (label, value, tone, mono) in rows {
                col = col.child(kv_row(theme, label, value, *tone, *mono));
            }
            col
        }

        // The simulation's own account of what moves. It is the ONE part of a
        // signing sheet a malicious site cannot author, which is why the
        // deeper degradation rungs promote it from footnote to protagonist.
        Block::Balances {
            title,
            rows,
            note,
            note_tone,
        } => {
            let mut col = div()
                .px(px(16.))
                .py(px(4.))
                .rounded(px(16.))
                .border_1()
                .border_color(theme.border_card)
                .flex()
                .flex_col()
                .child(
                    div()
                        .py(px(8.))
                        .text_size(theme::text_row_sub())
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(theme.fg_muted)
                        .child(title.clone()),
                );
            for (symbol, delta, tone) in rows {
                col = col.child(
                    div()
                        .py(px(4.))
                        .flex()
                        .items_center()
                        .justify_between()
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .text_color(theme.fg_base)
                                .child(symbol.clone()),
                        )
                        .child(
                            div()
                                .text_size(theme::text_row_title())
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(tone_color(theme, *tone))
                                .child(delta.clone()),
                        ),
                );
            }
            if let Some(note) = note {
                col = col.child(
                    div()
                        .py(px(8.))
                        .text_size(theme::text_row_sub())
                        .text_color(if *note_tone == Tone::Neutral {
                            theme.fg_subtle
                        } else {
                            tone_color(theme, *note_tone)
                        })
                        .child(note.clone()),
                );
            }
            col
        }
    }
}

fn kv_row(
    theme: &Theme,
    label: &SharedString,
    value: &SharedString,
    tone: Tone,
    mono: bool,
) -> Div {
    div()
        .py(px(10.))
        .flex()
        .items_start()
        .justify_between()
        .gap(px(16.))
        .child(
            div()
                .flex_none()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(label.clone()),
        )
        // The value gives way and wraps — a deposit address is 42 characters
        // and ran off the card's edge (078 W-05).
        .child(
            div()
                .min_w(px(0.))
                .when(mono, |d| d.font_family("monospace"))
                .when(!mono, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                .text_size(theme::text_row_sub())
                .text_color(tone_color(theme, tone))
                .child(value.clone()),
        )
}

/// The fee row, or the expanded fee-token selector (CS33 / DCS8).
///
/// `on_row` is the row's own tap — retry a failed quote, or open / close the
/// coin list — and `on_pick` one listener per coin in drawn order, `None` for
/// a coin that cannot pay (drawn for context, answers to nothing).
/// `on_refresh` is the refresh control beside the row (spec 079: the send
/// form's own, outside the row's click so measuring again never opens the
/// coin list). The mocks pass none of them.
pub fn fee(
    theme: &Theme,
    icons: &mut IconCache,
    fee: &FeeModel,
    on_row: Option<crate::flows::panels::Click>,
    on_pick: Vec<Option<crate::flows::panels::Click>>,
    on_refresh: Option<crate::flows::panels::Click>,
) -> Option<Div> {
    match fee {
        FeeModel::Hidden => None,
        FeeModel::OffChain(note) => Some(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(icon_img(icons, Icon::Check, false, theme.success_base, 14.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.success_base)
                        .child(note.clone()),
                ),
        ),
        FeeModel::OnChain {
            label,
            value,
            selector,
            warning,
            warning_held,
            tappable,
            refresh,
            refreshing,
            stale_note,
        } => {
            // Said under the row, in the error colour: why the confirm is shut.
            // Held while the fee is measured again, the last words keep the
            // line's height and are not said.
            let warning = warning.clone().map(|text| {
                div()
                    .px(px(16.))
                    .text_size(theme::text_row_sub())
                    .text_color(if *warning_held {
                        gpui::transparent_black()
                    } else {
                        theme.error_base
                    })
                    .child(text)
            });
            let Some((title, options)) = selector else {
                let row = div()
                    .px(px(16.))
                    .py(px(12.))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_muted)
                            .child(label.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(px(8.))
                            .child(
                                div()
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_base)
                                    .child(value.clone()),
                            )
                            // One coin and a quote in hand: there is nothing
                            // to choose and nothing to ask again, so this is
                            // the fee STATED — no chevron, and no listener
                            // under it. A control that cannot act is not
                            // drawn as one (spec 081, dead controls).
                            .when(*tappable, |el| {
                                el.child(icon_img(
                                    icons,
                                    Icon::ChevronRight,
                                    false,
                                    theme.fg_muted,
                                    12.,
                                ))
                            }),
                    );
                let on_row = if *tappable { on_row } else { None };
                // One sunken surface, as the send form's `FeeRow` draws it:
                // the row, and the refresh at its end — two controls side by
                // side, not one inside the other.
                let mut line = div()
                    .flex()
                    .items_center()
                    .rounded(px(12.))
                    .bg(theme.bg_sunken)
                    .child(
                        div()
                            .flex_1()
                            .min_w(px(0.))
                            .child(crate::flows::panels::clickable("signing-fee", on_row, row)),
                    );
                if refresh.is_some() {
                    // While a measurement is out the control turns — the row
                    // is working, not stuck.
                    let control = div()
                        .flex()
                        .flex_none()
                        .items_center()
                        .justify_center()
                        .size(px(36.))
                        .mr(px(4.))
                        .rounded_full()
                        .hover(|el| el.bg(theme.bg_base))
                        .child(if *refreshing {
                            crate::ui::spinner(theme.fg_muted, px(14.), px(1.5))
                        } else {
                            gpui::IntoElement::into_any_element(icon_img(
                                icons,
                                Icon::RefreshCw,
                                false,
                                theme.fg_muted,
                                14.,
                            ))
                        });
                    line = line.child(crate::flows::panels::clickable(
                        "signing-fee-refresh",
                        on_refresh.filter(|_| !*refreshing),
                        control,
                    ));
                }
                let stale = stale_note.clone().map(|note| {
                    div()
                        .px(px(16.))
                        .text_size(theme::text_label())
                        .text_color(theme.fg_subtle)
                        .child(note)
                });
                return Some(
                    div()
                        .flex()
                        .flex_col()
                        .gap(px(6.))
                        .child(line)
                        .children(stale)
                        .children(warning),
                );
            };
            let header = div()
                .py(px(8.))
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .child(title.clone()),
                )
                .child(icon_img(
                    icons,
                    Icon::ChevronDown,
                    false,
                    theme.fg_muted,
                    12.,
                ));
            let mut col = div()
                .px(px(16.))
                .py(px(8.))
                .rounded(px(12.))
                .bg(theme.bg_sunken)
                .flex()
                .flex_col()
                .child(crate::flows::panels::clickable(
                    "signing-fee-title",
                    on_row,
                    header,
                ));
            let mut on_pick = on_pick.into_iter();
            for (i, option) in options.iter().enumerate() {
                let mut row = div()
                    .p(px(8.))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(option.selected, |d| d.bg(theme.bg_raised))
                    // A coin that cannot pay is shown, dimmed, and not offered.
                    .when(option.insufficient, |d| d.opacity(0.45))
                    .child(crate::wallet::components::token_icon_logos(
                        theme,
                        option.mark.ticker.as_ref(),
                        option.mark.badge,
                        &option.mark.logos,
                    ))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .flex_1()
                            .child(
                                div()
                                    .text_size(theme::text_row_title())
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .text_color(theme.fg_base)
                                    .child(option.name.clone()),
                            )
                            .child(
                                div()
                                    .text_size(theme::text_row_sub())
                                    .text_color(theme.fg_muted)
                                    .child(option.balance.clone()),
                            ),
                    )
                    .child(
                        div()
                            .text_size(theme::text_row_sub())
                            .text_color(theme.fg_base)
                            .child(option.fee.clone()),
                    );
                if option.selected {
                    row = row.child(icon_img(icons, Icon::Check, false, theme.accent, 14.));
                }
                let action = on_pick.next().flatten().filter(|_| !option.insufficient);
                col = col.child(crate::flows::panels::clickable(
                    gpui::ElementId::from(("signing-fee-coin", i)),
                    action,
                    row,
                ));
                // Issue #408: why a greyed coin cannot pay, under its row and
                // at full strength — the dimming is not a reason. Set in past
                // the mark (8 + 40 + 12), under the name.
                if let Some(reason) = option.reason.clone() {
                    col = col.child(
                        div()
                            .pl(px(FEE_REASON_INDENT))
                            .pr(px(8.))
                            .pb(px(4.))
                            .text_size(theme::text_row_sub())
                            .text_color(theme.error_base)
                            .child(reason),
                    );
                }
            }
            Some(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(6.))
                    .child(col)
                    .children(warning),
            )
        }
    }
}

/// The signer row — whose key is about to sign.
pub fn signer_row(
    theme: &Theme,
    identicons: &mut IdenticonCache,
    label: SharedString,
    name: SharedString,
    seed: &str,
) -> Div {
    div()
        .flex()
        .items_center()
        .justify_between()
        .child(
            div()
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(label),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(openable_identicon(identicons, seed, 18.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(name),
                ),
        )
}

/// The one way to confirm (spec 022 §4) — a tap since issue #461: the send
/// Confirm's own primary button ([`cta_button`]), full width, its label the
/// action alone ("Confirm swap", "Sign", "Approve", "备份公钥", …). There is
/// no reject button beside it: closing the column IS the rejection, so the
/// only deliberate act here is the affirmative one.
///
/// `action` is `None` for the drawings — a button that answers to nothing —
/// and is only ever passed when the core's confirm state is enabled; a shut
/// button is drawn shut, with the core's note under it. It is never drawn
/// busy: the approval replaces the form with the receipt in the same frame
/// (spec 079), so there is no moment for a faded button to read as a refusal.
///
/// [`cta_button`]: crate::flows::panels::cta_button
pub fn confirm_button(
    theme: &Theme,
    label: SharedString,
    enabled: bool,
    action: Option<crate::flows::panels::Click>,
) -> Div {
    crate::flows::panels::cta_button(CONFIRM_ID, theme, label, armed(enabled), action)
}

/// The confirm when the account signs on the Trusted Signer's page (spec 079
/// US7): the same button, saying it goes there — the page asks for the one
/// consent. Armed on the same terms: `action` is only ever passed when the
/// three machines agreed.
pub fn open_signer_button(
    theme: &Theme,
    label: SharedString,
    enabled: bool,
    action: Option<crate::flows::panels::Click>,
) -> Div {
    crate::flows::panels::cta_button("signing-open-signer", theme, label, armed(enabled), action)
}

/// The gas top-up's one action, "check again" (`treasuryBootstrap.retryBtn`):
/// a plain button, armed on its own terms — it is not a signature, it is
/// "I have sent it, look again", and the only way forward from a top-up.
/// Armed only when it has something to do: a button with no action is drawn
/// shut, so it can never look live and answer to nothing.
pub fn funding_check_button(
    theme: &Theme,
    label: SharedString,
    action: Option<crate::flows::panels::Click>,
) -> Div {
    let state = armed(action.is_some());
    crate::flows::panels::cta_button("signing-funding-check", theme, label, state, action)
}

/// A confirm the core armed is enabled; one it shut is drawn shut.
fn armed(enabled: bool) -> crate::flows::fixtures::CtaState {
    if enabled {
        crate::flows::fixtures::CtaState::Enabled
    } else {
        crate::flows::fixtures::CtaState::Disabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loc::Loc;
    use crate::signing::SigningStrings;
    use crate::theme::ThemeMode;

    /// Issue #461: the confirm is found by its id, whatever its words say —
    /// the same hook the web (`data-testid`), iOS and Android carry.
    #[test]
    fn the_confirm_keeps_its_stable_hook() {
        assert_eq!(CONFIRM_ID, "signing-confirm");
    }

    /// The wallet's own request draws no requester header — no app mark, no
    /// "Vela Wallet", no network chip — on the form and on its receipt; the
    /// column's scaffold already titles it and owns the ✕. A site's request
    /// always names the site.
    #[test]
    fn only_a_site_gets_a_requester_header() {
        let theme = Theme::of(ThemeMode::Light);
        let s = SigningStrings::resolve(&Loc::from_env());
        let mut model = super::super::fixtures::build("cs1", &s);
        assert!(header(&theme, &model).is_some(), "a site names itself");
        assert!(header_view(&theme, &HeaderModel::of(&model)).is_some());
        model.first_party = true;
        assert!(header(&theme, &model).is_none(), "the form draws none");
        assert!(
            header_view(&theme, &HeaderModel::of(&model)).is_none(),
            "nor does the receipt the column ends on"
        );
    }
}
