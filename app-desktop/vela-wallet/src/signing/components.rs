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
use crate::wallet::components::{icon_img, identicon_avatar};

use super::Tone;
use super::fixtures::{Block, FeeModel, SigningModel};
use crate::explore::components::letter_avatar;

/// Slide-to-confirm geometry, measured off CS1 (342×56 track, 48 knob).
pub const SLIDE_H: f32 = 56.;
pub const SLIDE_KNOB: f32 = 48.;

fn tone_color(theme: &Theme, tone: Tone) -> Hsla {
    match tone {
        Tone::Neutral => theme.fg_base,
        Tone::Accent => theme.accent,
        Tone::Success => theme.success_base,
        Tone::Caution => theme.warning_base,
        Tone::Danger => theme.error_base,
    }
}

/// The dApp header: who is asking, and on which network.
pub fn header(theme: &Theme, model: &SigningModel) -> Div {
    div()
        .flex()
        .items_center()
        .gap(px(12.))
        .child(letter_avatar(
            model.dapp_letter.clone(),
            model.dapp_tint,
            36.,
        ))
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
                        .child(model.dapp_name.clone()),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
                        .truncate()
                        .child(model.dapp_host.clone()),
                ),
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
                .child(
                    div()
                        .w(px(8.))
                        .h(px(8.))
                        .rounded_full()
                        .bg(model.network_dot),
                )
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(model.network_name.clone()),
                ),
        )
}

/// One block, rendered.
/// One block, with the allowance chips ARMED.
///
/// The chips are a control, not a picture, and only when a machine is behind
/// them: `actions` carries one per chip, in the order the block lists them.
/// Passing none draws exactly what the gallery draws — the same rule the slide
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
            if let Some(mark) = &line.token {
                value_row = value_row.child(letter_avatar(mark.0.clone(), mark.1, 22.));
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
            let mut who = div().flex().flex_col().gap(px(2.)).child(
                div()
                    .text_size(theme::text_row_title())
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(theme.fg_base)
                    .child(name.clone()),
            );
            if let Some(address) = address {
                who = who.child(
                    div()
                        .font_family("monospace")
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_muted)
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
                line = line.child(
                    div()
                        .px(px(8.))
                        .py(px(2.))
                        .rounded(px(4.))
                        .bg(fill)
                        .text_size(theme::text_row_sub())
                        .text_color(ink)
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
            div()
                .p(px(12.))
                .rounded(px(12.))
                .bg(if danger {
                    theme.error_soft
                } else {
                    theme.warning_soft
                })
                .border_1()
                .border_color(ink)
                .flex()
                .items_start()
                .gap(px(12.))
                .child(icon_img(icons, Icon::TriangleAlert, false, ink, 14.))
                .child(
                    div()
                        .flex_1()
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
                .text_size(theme::text_row_sub())
                .text_color(theme.fg_muted)
                .child(label.clone()),
        )
        .child(
            div()
                .when(mono, |d| d.font_family("monospace"))
                .when(!mono, |d| d.font_weight(gpui::FontWeight::SEMIBOLD))
                .text_size(theme::text_row_sub())
                .text_color(tone_color(theme, tone))
                .child(value.clone()),
        )
}

/// The fee row, or the expanded fee-token selector (CS33 / DCS8).
pub fn fee(theme: &Theme, icons: &mut IconCache, fee: &FeeModel) -> Option<Div> {
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
        } => {
            let Some((title, options)) = selector else {
                return Some(
                    div()
                        .px(px(16.))
                        .py(px(12.))
                        .rounded(px(12.))
                        .bg(theme.bg_sunken)
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
                                .child(icon_img(
                                    icons,
                                    Icon::ChevronRight,
                                    false,
                                    theme.fg_muted,
                                    12.,
                                )),
                        ),
                );
            };
            let mut col = div()
                .px(px(16.))
                .py(px(8.))
                .rounded(px(12.))
                .bg(theme.bg_sunken)
                .flex()
                .flex_col()
                .child(
                    div()
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
                        )),
                );
            for option in options {
                let mut row = div()
                    .p(px(8.))
                    .rounded(px(12.))
                    .flex()
                    .items_center()
                    .gap(px(12.))
                    .when(option.selected, |d| d.bg(theme.bg_raised))
                    .child(letter_avatar(option.mark.0.clone(), option.mark.1, 32.))
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
                col = col.child(row);
            }
            Some(col)
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
                .child(identicon_avatar(identicons, seed, 18.))
                .child(
                    div()
                        .text_size(theme::text_row_sub())
                        .text_color(theme.fg_base)
                        .child(name),
                ),
        )
}

/// The one way to confirm (spec 022 §4). There is no reject button beside it:
/// closing the column IS the rejection, so the only deliberate act here is the
/// affirmative one.
/// The confirm. `action` is `None` for the mocks — a drawn slide that answers
/// to nothing — and `Some` for a live request. It is only ever passed when
/// `enabled`, so a shut slide cannot be fired by a click that lands on it.
pub fn slide_to_confirm(
    theme: &Theme,
    icons: &mut IconCache,
    label: SharedString,
    enabled: bool,
    action: Option<crate::flows::panels::Click>,
) -> Div {
    let slide = div()
        .h(px(SLIDE_H))
        .rounded_full()
        .bg(theme.bg_sunken)
        .flex()
        .items_center()
        .opacity(if enabled { 1.0 } else { 0.45 })
        .child(
            div()
                .ml(px(4.))
                .w(px(SLIDE_KNOB))
                .h(px(SLIDE_KNOB))
                .rounded_full()
                .bg(theme.accent)
                .flex()
                .items_center()
                .justify_center()
                .child(icon_img(
                    icons,
                    Icon::ArrowRight,
                    false,
                    theme.fg_inverse,
                    20.,
                )),
        )
        .child(
            div()
                .flex_1()
                .text_center()
                .text_size(theme::text_row_title())
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(theme.fg_muted)
                .child(label),
        )
        .child(div().w(px(SLIDE_KNOB)));
    // A shut slide answers to nothing. The drawings have no reject button —
    // closing the column IS the rejection — so the only thing this control
    // can do is confirm, and it may only do that when all three machines
    // agreed.
    crate::flows::panels::clickable("signing-confirm", action, slide)
}

use super::fixtures::ChipState;
