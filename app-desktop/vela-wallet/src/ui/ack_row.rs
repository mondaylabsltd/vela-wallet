//! Acknowledgment row (spec 014 Form pattern): hairline checkbox square that
//! fills with the ink + inverse ✓ when checked, muted sentence text, and — for the
//! legal row — inline links that wrap across lines while staying individually
//! activatable (`InteractiveText` ranges), never toggling the checkbox.
//!
//! The SENTENCE toggles the box too, the way it does on web (founder-found
//! 2026-08-25: only the 20px square did, and the four shells each behaved
//! differently). On a row with links that cannot be a click handler on the
//! parent — `InteractiveText` does not stop propagation, so a link would open
//! its page AND tick the box — so the toggle is registered as the ranges
//! BETWEEN the links, and one dispatch decides which of the two happened.

use std::ops::Range;
use std::rc::Rc;

use crate::theme::{self, ACK_BOX, Theme};
use gpui::{
    App, Div, FontWeight, HighlightStyle, InteractiveElement, InteractiveText, ParentElement,
    SharedString, StatefulInteractiveElement, Styled, StyledText, Window, div, px,
};

/// Generic over the link id so this atom does not have to know which screen's
/// vocabulary it is carrying — the legal row's two links mean something to the
/// create flow and nothing here.
pub fn ack_row<Link: Copy + 'static>(
    ix: usize,
    theme: &Theme,
    checked: bool,
    text: SharedString,
    links: Vec<(Range<usize>, Link)>,
    on_toggle: impl Fn(&mut Window, &mut App) + 'static,
    on_link: impl Fn(Link, &mut Window, &mut App) + 'static,
) -> Div {
    // Shared: the box and the sentence are the same control.
    let on_toggle = Rc::new(on_toggle);

    let checkbox = {
        let base = div()
            .id(("ack-box", ix as u64))
            .size(px(ACK_BOX))
            .flex_none()
            // Optical alignment with the first text line (20 px line height).
            .mt(px(1.))
            .rounded(px(theme::RADIUS_ACK))
            .flex()
            .items_center()
            .justify_center()
            .cursor_pointer();
        let styled = if checked {
            // A check is a selected state: neutral ink, never the accent
            // (web `.ack input { accent-color: fg.base }`, Android's AckRow).
            base.bg(theme.fg_base).child(
                div()
                    .text_size(theme::text_flow_caption())
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.fg_inverse)
                    .child("✓"),
            )
        } else {
            base.border_1().border_color(theme.outline_strong)
        };
        let toggle = Rc::clone(&on_toggle);
        styled.on_click(move |_, window, cx| toggle(window, cx))
    };

    let body = if links.is_empty() {
        div().child(
            div()
                .id(("ack-text", ix as u64))
                .cursor_pointer()
                .child(text)
                .on_click(move |_, window, cx| on_toggle(window, cx)),
        )
    } else {
        // Links in the text's own ink, told apart by their underline: the
        // accent is only for the action that moves value (the web's `.gates a`,
        // vis-6).
        let ink = theme.fg_base;
        let highlights: Vec<(Range<usize>, HighlightStyle)> = links
            .iter()
            .map(|(range, _)| {
                (
                    range.clone(),
                    HighlightStyle {
                        color: Some(ink),
                        underline: Some(gpui::UnderlineStyle {
                            color: Some(ink),
                            thickness: px(1.),
                            wavy: false,
                        }),
                        ..HighlightStyle::default()
                    },
                )
            })
            .collect();

        // Every range of the sentence, in order: the link ones carry their id,
        // the gaps between them carry none and mean "toggle".
        let mut ranges: Vec<Range<usize>> = Vec::with_capacity(links.len() * 2 + 1);
        let mut ids: Vec<Option<Link>> = Vec::with_capacity(links.len() * 2 + 1);
        let mut cursor = 0usize;
        for (range, id) in &links {
            if cursor < range.start {
                ranges.push(cursor..range.start);
                ids.push(None);
            }
            ranges.push(range.clone());
            ids.push(Some(*id));
            cursor = range.end;
        }
        if cursor < text.len() {
            ranges.push(cursor..text.len());
            ids.push(None);
        }

        div().child(
            InteractiveText::new(
                ("ack-links", ix as u64),
                StyledText::new(text).with_highlights(highlights),
            )
            .on_click(ranges, move |range_ix, window, cx| {
                match ids.get(range_ix) {
                    Some(Some(id)) => on_link(*id, window, cx),
                    // A gap between the links: plain sentence text, and the
                    // whole sentence is part of the checkbox.
                    Some(None) => on_toggle(window, cx),
                    None => {}
                }
            }),
        )
    };

    div()
        .flex()
        .items_start()
        .gap(px(theme::FLOW_GAP_MD_SNUG))
        .child(checkbox)
        .child(
            div()
                .flex_1()
                .min_w(px(0.))
                .text_size(theme::text_body())
                .line_height(theme::line_height_ack())
                .text_color(theme.fg_muted)
                .child(body),
        )
}
