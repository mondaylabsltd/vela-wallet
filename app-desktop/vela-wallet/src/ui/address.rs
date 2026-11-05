//! An address (or any long hex) drawn so that no line break ever falls inside
//! it at a place nobody chose (r13 S4: the header's 6+6 broke over two lines;
//! a sheet's full address wrapped its last digit alone onto a line).
//!
//! A short form (`0x777E98…61875a`) is one unbreakable line. A full one is
//! two unbreakable halves side by side: on one line where the column holds
//! it, else the second half under the first — always the same split, never
//! one digit on its own. The caller's text style (mono, size, colour) is
//! inherited by both halves.

use gpui::{Div, ParentElement, SharedString, Styled, div};

/// Hex longer than this is split in two (a 6+6 short form never is).
const SPLIT_FROM: usize = 24;
/// Longer hex (calldata) wraps as text does: two halves would not fit.
const SPLIT_UP_TO: usize = 66;

/// The two halves [`address_text`] draws `text` as, or `None` when it is
/// drawn whole (short, not `0x`-hex, or not ASCII).
pub fn address_halves(text: &str) -> Option<(&str, &str)> {
    let hex = text
        .strip_prefix("0x")
        .or_else(|| text.strip_prefix("0X"))?;
    if !(SPLIT_FROM..=SPLIT_UP_TO).contains(&text.len())
        || !hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return None;
    }
    // The `0x` stays with the first half; both halves as near even as hex allows.
    let mid = 2 + hex.len().div_ceil(2);
    Some(text.split_at(mid))
}

/// `text` (an address, a hash, its short form) with line breaks only where
/// [`address_halves`] chose them.
pub fn address_text(text: impl Into<SharedString>) -> Div {
    let text: SharedString = text.into();
    match address_halves(&text) {
        Some((head, tail)) => div()
            .flex()
            .flex_wrap()
            .min_w(gpui::px(0.))
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .child(SharedString::from(head.to_owned())),
            )
            .child(
                div()
                    .flex_none()
                    .whitespace_nowrap()
                    .child(SharedString::from(tail.to_owned())),
            ),
        // A short form is one line; anything else (a name, calldata) wraps
        // as text does.
        None if is_short_form(&text) => div().whitespace_nowrap().child(text),
        None => div().min_w(gpui::px(0.)).child(text),
    }
}

/// `0x777E98…61875a`: hex, an ellipsis, hex — and nothing that could wrap.
fn is_short_form(text: &str) -> bool {
    text.len() <= SPLIT_FROM + 4
        && (text.starts_with("0x") || text.starts_with("0X"))
        && text.contains('…')
        && !text.contains(char::is_whitespace)
}

/// The end (byte index) of a short form starting at `at`: `0x`, two or
/// more hex digits, "…" or "...", two or more hex digits — the core's 6+6
/// (`names.short`), its 6+4, and the three-dot form a decoded call uses (the
/// web's `addressRuns` pattern). A whole address has no ellipsis.
fn short_form_end(text: &str, at: usize) -> Option<usize> {
    if !text.get(at..)?.starts_with("0x") {
        return None;
    }
    let hex_run = |from: usize| {
        text[from..]
            .bytes()
            .take_while(u8::is_ascii_hexdigit)
            .count()
    };
    let head = hex_run(at + 2);
    if head < 2 {
        return None;
    }
    let after = at + 2 + head;
    let dots = ["…", "..."]
        .iter()
        .find(|d| text[after..].starts_with(**d))?
        .len();
    let tail = hex_run(after + dots);
    (tail >= 2).then_some(after + dots + tail)
}

/// Where the short address forms in `text` lie (byte ranges, in order) —
/// r14-11a: each is one unbreakable run inside a line of other words
/// (`ui::prose` breaks around them, never inside: [`breaks_a_short_form`]).
#[must_use]
pub fn short_form_spans(text: &str) -> Vec<std::ops::Range<usize>> {
    let mut spans = Vec::new();
    let mut at = 0;
    while at < text.len() {
        if let Some(end) = short_form_end(text, at) {
            spans.push(at..end);
            at = end;
        } else {
            at += text[at..].chars().next().map_or(1, char::len_utf8);
        }
    }
    spans
}

/// Whether a line starting at byte `at` would cut a short form in two —
/// after its "…" (Unicode's break) or before it (gpui's).
#[must_use]
pub fn breaks_a_short_form(text: &str, at: usize) -> bool {
    short_form_spans(text)
        .iter()
        .any(|span| span.start < at && at < span.end)
}

#[cfg(test)]
mod tests {
    use super::address_halves;

    #[test]
    fn a_full_address_is_split_once_evenly_and_a_short_one_never() {
        let full = "0x777E989C821a7F1b031696f044a2fF114261875a";
        let (head, tail) = address_halves(full).expect("split");
        assert_eq!(format!("{head}{tail}"), full);
        assert_eq!((head.len(), tail.len()), (22, 20));
        // Never one digit on its own line.
        assert!(tail.len() > 1);
        assert_eq!(address_halves("0x777E98…61875a"), None);
        assert_eq!(address_halves("Interleave"), None);
        assert!(super::is_short_form("0x777E98…61875a"));
        assert!(!super::is_short_form("Interleave · Vela account"));
        // Calldata is not split (each half would run off the sheet).
        assert_eq!(address_halves(&format!("0x{}", "ab".repeat(68))), None);
        // A hash splits the same way.
        let hash = format!("0x{}", "ab".repeat(32));
        let (h, t) = address_halves(&hash).expect("split");
        assert_eq!((h.len(), t.len()), (34, 32));
    }

    /// r14-11a: the short forms inside a line are found whole, and no break
    /// may fall inside one — not after its "…", not before it.
    #[test]
    fn a_short_form_inside_a_line_is_one_unbreakable_run() {
        use super::{breaks_a_short_form, short_form_spans};
        let line = "0x777E98…61875a · 2 of 3 · Gnosis";
        assert_eq!(short_form_spans(line), vec![0.."0x777E98…61875a".len()]);
        let ellipsis = line.find('…').unwrap_or_default();
        assert!(breaks_a_short_form(line, ellipsis), "before its \"…\"");
        assert!(
            breaks_a_short_form(line, ellipsis + '…'.len_utf8()),
            "after it"
        );
        assert!(breaks_a_short_form(line, 2));
        // Around it, the line breaks as text does.
        assert!(!breaks_a_short_form(line, 0));
        let gnosis = line.find("Gnosis").unwrap_or_default();
        assert!(!breaks_a_short_form(line, gnosis));
        // Glued to its brackets (a member named in a title), and in a CJK
        // line with no spaces: the span is the address alone.
        let title = "Add Ana (0x88cCA0…266894)";
        let open = title.find("0x").unwrap_or_default();
        assert_eq!(
            short_form_spans(title),
            vec![open..open + "0x88cCA0…266894".len()]
        );
        let zh = "添加成员0x88cCA0…266894为签名人";
        let at = zh.find("0x").unwrap_or_default();
        assert_eq!(short_form_spans(zh), vec![at..at + "0x88cCA0…266894".len()]);
        // The three-dot form a decoded call uses is a short form too.
        assert_eq!(short_form_spans("to 0xabcd...ef01").len(), 1);
        // A whole address, and text with no address, hold none.
        assert!(short_form_spans("0x777E989C821a7F1b031696f044a2fF114261875a").is_empty());
        assert!(short_form_spans("Nothing here… 2 of 3").is_empty());
    }
}
