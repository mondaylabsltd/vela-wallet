//! Wrapped text that keeps the line-breaking rules a reader of Chinese,
//! Japanese or Korean takes for granted (禁则 / kinsoku) — and the glue the
//! core puts in its strings.
//!
//! gpui's wrapper (`LineLayout::compute_wrap_boundaries`, in the pinned gpui
//! itself, not in this repository) allows a break before ANY character that
//! is not part of a Latin word. So it will open a line with 「。」 — a full
//! stop alone on the last line, which reads as a rendering fault — or with
//! the "·" the corpus glued to the word before it with U+00A0. Every other
//! shell's text engine (CoreText, Android's, a browser's) follows UAX #14 and
//! never makes those breaks.
//!
//! The wrapper cannot be changed from here, so this does what a typesetter
//! does by hand: when the natural break is a forbidden one, the measure is
//! narrowed by one em at a time until the break is clean — which carries the
//! character before the mark down with it (「…由网站提 / 供。」). Text with
//! nothing to forbid (every English string) is shaped once, exactly as gpui's
//! own text element shapes it.
//!
//! [`prose`] is the element; [`forbidden_break`] is the rule, in one place;
//! [`clean_wrap_width`] is the search, for a caller that lays text out itself.

use std::cell::RefCell;
use std::rc::Rc;

use gpui::{
    AnyElement, App, AvailableSpace, Bounds, Element, ElementId, GlobalElementId,
    InspectorElementId, IntoElement, LayoutId, Pixels, SharedString, Size, TextRun, WhiteSpace,
    Window, WrappedLine,
};

/// Marks a line may not OPEN with: closing punctuation and its kin — the
/// sentence can end a line, but its full stop cannot start the next.
///
/// Not the Latin middle dot "·": every other shell's engine may break
/// before it, and the corpus says where it must not by gluing it (U+00A0,
/// [`GLUE`]) — this shell follows the same strings, not a rule of its own.
const NO_LINE_START: &str = "。，、；：？！）」』》〉】〕〗〙”’…‥・ー々〜～%‰℃.,;:!?)]}";

/// Marks a line may not END with: opening brackets and quotes — the bracket
/// belongs to what it opens.
const NO_LINE_END: &str = "（「『《〈【〔〖〘“‘([{";

/// Glue: no break on either side (U+00A0 no-break space, U+202F narrow
/// no-break space, U+2060 word joiner, U+2011 non-breaking hyphen). The core
/// writes U+00A0 into a checked time ("下午\u{a0}2:32") and before the "·" of
/// a signing-page line.
const GLUE: [char; 4] = ['\u{00A0}', '\u{202F}', '\u{2060}', '\u{2011}'];

/// Marks written in pairs that are one mark: 「——」, 「……」.
const PAIRED: [char; 3] = ['—', '…', '‥'];

/// Whether `text` may NOT be broken so that a new line starts at byte `at`.
///
/// The rule, in one place:
/// - a line never opens with a closing mark ([`NO_LINE_START`]);
/// - a line never ends on an opening mark ([`NO_LINE_END`]);
/// - glue holds both its neighbours ([`GLUE`]);
/// - a doubled dash or ellipsis is not split.
///
/// `false` for an index that is not a character boundary inside the text —
/// there is no break there to forbid.
#[must_use]
pub fn forbidden_break(text: &str, at: usize) -> bool {
    if at == 0 || at >= text.len() || !text.is_char_boundary(at) {
        return false;
    }
    let (Some(before), Some(after)) = (text[..at].chars().next_back(), text[at..].chars().next())
    else {
        return false;
    };
    NO_LINE_START.contains(after)
        || NO_LINE_END.contains(before)
        || GLUE.contains(&before)
        || GLUE.contains(&after)
        || (before == after && PAIRED.contains(&after))
}

/// Whether `text` has anything the rule could forbid — the fast path's
/// question. A text with none of the marks is shaped once and never looked
/// at again. The marks gpui already binds to a Latin word (".", ",", ":",
/// ";", "%") do not count: they cannot open a line there.
#[must_use]
pub fn needs_care(text: &str) -> bool {
    text.chars().any(|c| {
        !matches!(c, '.' | ',' | ':' | ';' | '%')
            && (NO_LINE_START.contains(c)
                || NO_LINE_END.contains(c)
                || GLUE.contains(&c)
                || PAIRED.contains(&c))
    })
}

/// Whether any wrap in `lines` is a forbidden one.
#[must_use]
pub fn strands(lines: &[WrappedLine]) -> bool {
    lines.iter().any(|line| {
        line.wrap_boundaries().iter().any(|boundary| {
            // A boundary names the first glyph of the new line.
            line.unwrapped_layout
                .runs
                .get(boundary.run_ix)
                .and_then(|run| run.glyphs.get(boundary.glyph_ix))
                .is_some_and(|glyph| forbidden_break(&line.text, glyph.index))
        })
    })
}

/// How many one-em steps the measure may be narrowed by before giving up.
/// Twelve characters is more than any run of marks; past it the text is laid
/// out at its full width rather than squeezed into a column of nothing.
const MAX_STEPS: usize = 12;

/// `text` shaped at the widest width, at most `max`, whose breaks are all
/// allowed — with that width. The lines at `max` unchanged when nothing
/// strands, or when no narrower width is clean.
fn shape_clean(
    window: &Window,
    text: &SharedString,
    font_size: Pixels,
    runs: &[TextRun],
    max: Pixels,
    line_clamp: Option<usize>,
) -> Option<(Vec<WrappedLine>, Pixels)> {
    let shape = |width: Pixels| {
        window
            .text_system()
            .shape_text(text.clone(), font_size, runs, Some(width), line_clamp)
            .ok()
            .map(|lines| lines.into_iter().collect::<Vec<_>>())
    };
    let natural = shape(max)?;
    if !needs_care(text) || !strands(&natural) {
        return Some((natural, max));
    }
    let mut width = max;
    for _ in 0..MAX_STEPS {
        width -= font_size;
        if width <= font_size * 2. {
            break;
        }
        if let Some(lines) = shape(width)
            && !strands(&lines)
        {
            return Some((lines, width));
        }
    }
    Some((natural, max))
}

/// The widest width, at most `max`, at which `text` wraps with no forbidden
/// break — for a caller that sizes a box itself. `max` when nothing strands.
#[must_use]
pub fn clean_wrap_width(
    window: &Window,
    text: &SharedString,
    font_size: Pixels,
    runs: &[TextRun],
    max: Pixels,
) -> Pixels {
    shape_clean(window, text, font_size, runs, max, None).map_or(max, |(_, width)| width)
}

/// Text that wraps by the rule above. Use it wherever a sentence may wrap:
/// `div().child(prose(text))`. It takes its font, size, colour, line height
/// and alignment from the element around it, exactly as a bare string child
/// does.
///
/// It IS that string child — gpui's own text element, untouched — for any
/// text the rule has nothing to say about ([`needs_care`]: every English
/// sentence without a bracket), and wherever the text cannot wrap (a
/// one-line or truncated box). Only a wrapping text with a mark in it is
/// laid out here.
#[must_use]
pub fn prose(text: impl Into<SharedString>) -> Prose {
    Prose { text: text.into() }
}

/// See [`prose`].
pub struct Prose {
    text: SharedString,
}

/// What a measure pass settled, kept for the paint.
pub struct Laid {
    lines: Vec<WrappedLine>,
    line_height: Pixels,
    /// The width the layout asked the text to wrap at (not the narrowed one):
    /// the cache key, as gpui's own text element keys it.
    wrap_width: Option<Pixels>,
    size: Size<Pixels>,
}

/// The element's layout between its three passes.
pub enum ProseLayout {
    /// gpui's own text element draws it.
    Plain(AnyElement),
    /// Laid out here, by the rule; shared with the measure closure.
    Ruled(Rc<RefCell<Option<Laid>>>),
}

impl IntoElement for Prose {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for Prose {
    type RequestLayoutState = ProseLayout;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let text = self.text.clone();
        let text_style = window.text_style();
        // Nothing to forbid, or nowhere to wrap: the string child itself.
        if !needs_care(&text)
            || text_style.white_space != WhiteSpace::Normal
            || text_style.text_overflow.is_some()
        {
            let mut plain = text.into_any_element();
            let layout_id = plain.request_layout(window, cx);
            return (layout_id, ProseLayout::Plain(plain));
        }

        let state: Rc<RefCell<Option<Laid>>> = Rc::default();
        let font_size = text_style.font_size.to_pixels(window.rem_size());
        let line_height = window.pixel_snap(
            text_style
                .line_height
                .to_pixels(font_size.into(), window.rem_size()),
        );
        let runs = vec![text_style.to_run(text.len())];
        let line_clamp = text_style.line_clamp;

        let layout_id = window.request_measured_layout(Default::default(), {
            let state = state.clone();
            move |known, available, window, _cx| {
                // The width the text may wrap at — gpui's own rule: the
                // width the layout already knows, else a definite offer.
                let wrap_width = known.width.or(match available.width {
                    AvailableSpace::Definite(width) => Some(width),
                    _ => None,
                });
                if let Some(laid) = state.borrow().as_ref()
                    && (wrap_width.is_none() || wrap_width == laid.wrap_width)
                {
                    return laid.size;
                }
                let lines = match wrap_width {
                    Some(width) => shape_clean(window, &text, font_size, &runs, width, line_clamp)
                        .map(|(lines, _)| lines),
                    None => window
                        .text_system()
                        .shape_text(text.clone(), font_size, &runs, None, line_clamp)
                        .ok()
                        .map(|lines| lines.into_iter().collect()),
                }
                .unwrap_or_default();
                let mut size = Size::<Pixels>::default();
                for line in &lines {
                    let line_size = line.size(line_height);
                    size.height += line_size.height;
                    size.width = size.width.max(line_size.width).ceil();
                }
                // A text that could not be shaped still holds its line.
                if lines.is_empty() {
                    size.height = line_height;
                }
                state.borrow_mut().replace(Laid {
                    lines,
                    line_height,
                    wrap_width,
                    size,
                });
                size
            }
        });
        (layout_id, ProseLayout::Ruled(state))
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let ProseLayout::Plain(plain) = layout {
            plain.prepaint(window, cx);
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let state = match layout {
            ProseLayout::Plain(plain) => {
                plain.paint(window, cx);
                return;
            }
            ProseLayout::Ruled(state) => state.borrow(),
        };
        let Some(laid) = state.as_ref() else {
            return;
        };
        let align = window.text_style().text_align;
        let mut origin = bounds.origin;
        for line in &laid.lines {
            // A line that cannot be painted is skipped, as gpui's own text
            // element skips it; nothing here can act on the failure.
            let _ =
                line.paint_background(origin, laid.line_height, align, Some(bounds), window, cx);
            let _ = line.paint(origin, laid.line_height, align, Some(bounds), window, cx);
            origin.y += line.size(laid.line_height).height;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Where byte `at` falls: the index of the first occurrence of `mark`.
    fn before(text: &str, mark: &str) -> usize {
        text.find(mark)
            .unwrap_or_else(|| unreachable!("{mark:?} is in {text:?}"))
    }

    /// PR 3 item 12: 「…名称和币种由网站提供 / 。」 — the break gpui made on
    /// the add-network sheet. A line may not open with the full stop; the
    /// character before it is free to move, which is how the pair goes down
    /// together.
    #[test]
    fn a_line_never_opens_with_closing_punctuation() {
        let text = "不在 Vela 的网络列表中——名称和币种由网站提供。";
        assert!(forbidden_break(text, before(text, "。")));
        assert!(
            !forbidden_break(text, before(text, "供")),
            "the pair may move"
        );
        assert!(!forbidden_break(text, before(text, "名")));
        for mark in [
            "，", "、", "；", "：", "？", "！", "）", "」", "』", "》", "】", "…",
        ] {
            let text = format!("前文{mark}后文");
            assert!(forbidden_break(&text, before(&text, mark)), "{mark}");
            assert!(
                !forbidden_break(&text, before(&text, "后")),
                "after {mark} a line may begin"
            );
        }
        // Japanese and the Latin kin.
        assert!(forbidden_break("そうです。", before("そうです。", "。")));
        assert!(forbidden_break("ユーザー", before("ユーザー", "ー")));
        assert!(forbidden_break("50 %", before("50 %", "%")));
    }

    /// A bracket or quote belongs to what it opens: no line ends on it.
    #[test]
    fn a_line_never_ends_on_an_opening_mark() {
        for mark in ["（", "「", "『", "《", "【", "“"] {
            let text = format!("前文{mark}引文");
            assert!(forbidden_break(&text, before(&text, "引")), "{mark}");
            assert!(
                !forbidden_break(&text, before(&text, mark)),
                "{mark} may open a line"
            );
        }
    }

    /// PR 3 item 10: the core glues with U+00A0 — the "·" of a signing-page
    /// line to the word before it, and the parts of a checked time to each
    /// other. gpui breaks before any non-Latin character, glue or not; the
    /// rule holds both sides.
    #[test]
    fn glue_holds_both_its_neighbours() {
        let name = "自己部署的签名页\u{a0}· sign.example.com";
        assert!(forbidden_break(name, before(name, "\u{a0}")));
        assert!(
            forbidden_break(name, before(name, "·")),
            "never opens a line"
        );
        // Unglued, the dot is the corpus's to bind: the other shells' engines
        // may break before it, and so may this one.
        let loose = "Ethereum · Balance";
        assert!(!forbidden_break(loose, before(loose, "·")));
        let time = "检查于 下午\u{a0}10:07";
        assert!(forbidden_break(time, before(time, "10:07")));
        assert!(
            !forbidden_break(time, before(time, "下")),
            "before the unit"
        );
        // …and a doubled dash is one mark.
        let dash = "列表中——名称";
        assert!(forbidden_break(dash, before(dash, "——") + "—".len()));
        assert!(!forbidden_break(dash, before(dash, "——")));
    }

    /// The fast path: English prose has nothing for the rule to say, so it
    /// is shaped once, exactly as gpui's own text element shapes it.
    #[test]
    fn plain_prose_needs_no_care() {
        assert!(!needs_care(
            "Some contracts Vela needs aren't on this network yet."
        ));
        assert!(!needs_care("Ethereum · Balance 53.48, 50%: done"));
        // A bracket can strand in English too ("(optional" / ")"): looked at.
        assert!(needs_care("Not copied yet (optional)"));
        assert!(needs_care("资产不受影响，只是现在读不到。"));
        assert!(needs_care("Self-hosted\u{a0}· sign.example.com"));
        // Not a boundary, not inside the text: nothing to forbid.
        assert!(!forbidden_break("。", 0));
        assert!(!forbidden_break("好。", "好。".len()));
        assert!(!forbidden_break("好。", 1), "inside a character");
    }
}
