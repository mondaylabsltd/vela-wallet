//! The integrity line (spec 102 R6) — the one sentence that backs the word
//! "trusted" wherever a signing page is named: the hand-off card, Settings →
//! Signing pages, the choosers' own-page entry and the account's venue rows.
//!
//! What it says is the core's (`trusted_signer::launch::IntegrityLine`: a
//! state, a short version, when it was checked, the corpus key and whether the
//! page may open). What is here is drawing it: the words with their version
//! and time filled in, a tone, and a board that redraws a screen when a check
//! it is showing finishes.
//!
//! Wording, per the spec: "matches Vela's published build list · checked
//! <time>", never "certified untampered" — the check catches a build replaced
//! for everyone, not a server that serves one person other bytes.

use gpui::prelude::FluentBuilder as _;
use gpui::{
    App, AppContext as _, Div, Entity, Global, ParentElement as _, SharedString, Styled as _, div,
    px,
};

use vela_core::trusted_signer::launch::{IntegrityLine, IntegrityState};

use crate::executor::signer_integrity;
use crate::icons::{Icon, IconCache};
use crate::loc::Loc;
use crate::signing::Tone;
use crate::theme::{self, Theme};

/// The screens' handle on the checks: reading it during a frame is what makes
/// that frame's window redraw when a check finishes (gpui redraws a window
/// when an entity it read notifies).
pub struct Board;

struct BoardHandle(Entity<Board>);

impl Global for BoardHandle {}

/// The board, made — and wired to the checks' announcements — on first use.
fn board(cx: &mut App) -> Entity<Board> {
    if let Some(handle) = cx.try_global::<BoardHandle>() {
        return handle.0.clone();
    }
    let board = cx.new(|_| Board);
    cx.set_global(BoardHandle(board.clone()));
    let mut finished = signer_integrity::subscribe();
    let listening = board.downgrade();
    cx.spawn(async move |cx| {
        use futures::StreamExt as _;
        while finished.next().await.is_some() {
            let Some(board) = listening.upgrade() else {
                break;
            };
            board.update(cx, |_, cx| cx.notify());
        }
    })
    .detach();
    board
}

/// The line for `page` now — and, when nothing vouches for the page and no
/// check is running, a check started in the background (its result redraws
/// whoever read this). Never blocks.
pub fn line(page: &str, cx: &mut App) -> IntegrityLine {
    let board = board(cx);
    let _ = board.read(cx);
    signer_integrity::check_in_background(page);
    signer_integrity::line(page, crate::executor::now_ms() as u64)
}

/// "Check again" on a refused line: forget the refusal, and check.
pub fn recheck(page: &str, cx: &mut App) {
    signer_integrity::forget_refusal(page);
    let _ = line(page, cx);
}

/// "Trust this version" on a self-hosted page's question (spec 076 FR-009,
/// spec 102 D-15): the version the check asked about is stored on THAT page
/// (`SigningPagesEvent::VersionTrusted`, the core's rule — refused for the
/// official page and anything that is not a sha256), then the page is checked
/// again, which now reads "trusted on this device" and opens. Nothing happens
/// when the line no longer asks.
pub fn trust(page: &str, cx: &mut App) {
    use vela_core::app::signing_pages::{Event, SigningPages};
    let Some(version) = signer_integrity::version_to_trust(page) else {
        return;
    };
    crate::resident::resident::<SigningPages>(cx).update(cx, |pages, cx| {
        pages.dispatch(
            Event::VersionTrusted {
                url: page.to_owned(),
                version,
            },
            cx,
        );
    });
    recheck(page, cx);
}

/// The line's sentence, its `{{version}}` and `{{time}}` filled — the time
/// as the core words it for this clock and this person ("checked 14:32",
/// or the date too for a check from before today, D-13).
#[must_use]
pub fn text(loc: &Loc, line: &IntegrityLine) -> SharedString {
    text_at(loc, line, crate::executor::now_ms() as u64)
}

/// [`text`] at `now_ms` — the seam the tests use.
#[must_use]
pub fn text_at(loc: &Loc, line: &IntegrityLine, now_ms: u64) -> SharedString {
    let time = line
        .checked_at_ms
        .map(|at| checked_time(at, now_ms, loc.language()))
        .unwrap_or_default();
    loc.t_texts(
        &line.key,
        &[("version", line.version.as_str()), ("time", time.as_str())],
    )
}

/// `{{time}}` — the core's `launch::checked_time`, on this machine's clock
/// and offset and in the person's date and time formats.
fn checked_time(at_ms: u64, now_ms: u64, language: &str) -> String {
    use crate::executor::format_prefs;
    let formats = format_prefs::current();
    vela_core::trusted_signer::launch::checked_time(
        at_ms,
        now_ms,
        crate::executor::local_utc_offset_minutes(),
        format_prefs::date_word(formats.date),
        format_prefs::time_word(formats.time),
        language,
    )
}

/// How the line is coloured: good news green, a check under way quiet, a
/// question for the person amber, a page that will not open red.
#[must_use]
pub fn tone(line: &IntegrityLine) -> Tone {
    match line.state {
        IntegrityState::Matches | IntegrityState::TrustedHere => Tone::Success,
        IntegrityState::Checking => Tone::Neutral,
        IntegrityState::Unchecked | IntegrityState::AskToTrust => Tone::Caution,
        IntegrityState::Mismatch
        | IntegrityState::Blocked
        | IntegrityState::CouldNotCheck
        | IntegrityState::NoVersion
        | IntegrityState::AllBlocked => Tone::Danger,
    }
}

/// The mark beside the line: a tick, a clock, or a warning.
#[must_use]
pub fn icon(tone: Tone) -> Icon {
    match tone {
        Tone::Success => Icon::Check,
        Tone::Neutral | Tone::Accent => Icon::Clock,
        Tone::Caution => Icon::TriangleAlert,
        Tone::Danger => Icon::CircleAlert,
    }
}

/// The tone's ink.
#[must_use]
pub fn ink(theme: &Theme, tone: Tone) -> gpui::Hsla {
    match tone {
        Tone::Success => theme.success_base,
        Tone::Neutral => theme.fg_muted,
        Tone::Accent => theme.accent,
        Tone::Caution => theme.warning_base,
        Tone::Danger => theme.error_base,
    }
}

/// The sentence's ink (D7: the sheet's palette, one accent, nothing coloured
/// for its own sake): every line reads in the quiet secondary ink — the mark
/// beside it says which kind it is: a tick, a clock, or the caution triangle
/// of a question for the person ("Trust it on this device?", with its answer
/// beside it) — and only a page that will not open in the danger ink,
/// because that one must read as a refusal.
#[must_use]
pub fn words_ink(theme: &Theme, tone: Tone) -> gpui::Hsla {
    match tone {
        Tone::Success | Tone::Neutral | Tone::Accent | Tone::Caution => theme.fg_muted,
        Tone::Danger => theme.error_base,
    }
}

/// The line's leading, as a multiple of its type size.
pub const LINE_HEIGHT: f32 = 1.4;

/// The line's words, in two lines' room (`reserve`) — what a verdict takes
/// — so the check landing ("Checking the page…" → "Version … · checked
/// 14:32") never moves what is under it (the card's Open, the next row). A
/// line with its own answer under it ("Trust this version", "Try again") is
/// not padded: the answer is what follows the words, and a blank line
/// between a question and its answer would part them.
#[must_use]
pub fn words(theme: &Theme, said: SharedString, tone: Tone, reserve: bool) -> Div {
    div()
        .when(reserve, |words| {
            words.min_h(theme::text_row_sub() * (2. * LINE_HEIGHT))
        })
        .text_size(theme::text_row_sub())
        .line_height(gpui::relative(LINE_HEIGHT))
        .text_color(words_ink(theme, tone))
        .child(crate::ui::prose(said))
}

/// The line as a row: its mark and its sentence, wrapping under itself, in
/// two lines' room unless an answer follows it ([`words`]).
pub fn row(
    theme: &Theme,
    icons: &mut IconCache,
    said: SharedString,
    tone: Tone,
    reserve: bool,
) -> Div {
    let colour = ink(theme, tone);
    div()
        .flex()
        .items_start()
        .gap(px(8.))
        .child(
            div()
                .flex_none()
                .pt(px(2.))
                .child(crate::wallet::components::icon_img(
                    icons,
                    icon(tone),
                    false,
                    colour,
                    14.,
                )),
        )
        .child(words(theme, said, tone, reserve).flex_1().min_w(px(0.)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use vela_core::trusted_signer::integrity::{NoVersion, Verdict};

    fn admitted(at: u64) -> IntegrityLine {
        IntegrityLine::of(
            &Verdict::Open,
            vela_core::trusted_signer::integrity::LAUNCH,
            Some(at),
        )
    }

    /// The sentence names the version and the time, and never echoes a key
    /// or leaves a placeholder — in every language this build ships.
    #[test]
    fn every_line_reads_whole_in_every_language() {
        let lines = [
            IntegrityLine::checking(),
            admitted(1_760_000_000_000),
            IntegrityLine::of(
                &Verdict::Refused {
                    actual: "ab".repeat(32),
                    expected: Vec::new(),
                },
                "",
                None,
            ),
            IntegrityLine::no_version(NoVersion::NothingPublishedThisWalletKnows),
            IntegrityLine::no_version(NoVersion::EverythingUsableIsBlocked),
        ];
        for (tag, loc) in Loc::every_language() {
            for line in &lines {
                let said = text(&loc, line);
                assert_ne!(said.as_ref(), line.key, "{tag}: `{}` echoed", line.key);
                assert!(!said.contains("{{"), "{tag}: a placeholder left in {said}");
            }
            let said = text(&loc, &lines[1]);
            assert!(
                said.contains(&lines[1].version),
                "{tag}: no version in {said}"
            );
        }
    }

    /// D-13: `{{time}}` is a moment, worded by the core — the clock time for
    /// a check made today, the date with it for one from before (a check
    /// vouches for a day, so "14:32" alone could be yesterday's).
    #[test]
    fn checked_names_the_time_and_the_date_once_it_is_not_today() {
        let loc = Loc::for_tag("en");
        let at = 1_760_000_000_000;
        let today = text_at(&loc, &admitted(at), at + 60_000);
        assert!(!today.contains("2025"), "{today}");
        assert!(today.contains(':'), "{today}");
        let older = text_at(&loc, &admitted(at), at + 2 * 24 * 60 * 60 * 1000);
        assert!(older.contains("2025"), "{older}");
    }

    /// PR 3 item 10: the time is one unbreakable unit and the "·" before
    /// "checked" binds to the word before it — both the core's (U+00A0), so
    /// a wrapped line can read neither 「检查于 下午 / 10:07」 nor open with
    /// "·". In Chinese with a 12-hour clock, yesterday's check: the date,
    /// the day period and the clock are joined, and nothing here re-spaces
    /// them.
    #[test]
    fn a_checked_time_and_its_dot_never_break_badly() {
        let zh = Loc::for_tag("zh");
        let at = 1_760_000_000_000;
        let now = at + 2 * 24 * 60 * 60 * 1000;
        let time = checked_time(at, now, zh.language());
        assert!(!time.is_empty());
        assert!(
            !time.contains(' '),
            "a breakable space inside the time: {time:?}"
        );
        let line = text_at(&zh, &admitted(at), now);
        assert!(line.contains(&time), "{line:?} carries the time whole");
        // Every "·" in the line is glued to what comes before it.
        for (at, _) in line.match_indices('·') {
            assert!(
                line[..at].ends_with('\u{a0}'),
                "a line could open with the dot: {line:?}"
            );
        }
    }

    /// "Matches the published list" is the only good news, and it says so in
    /// green; a refusal is never drawn as anything but a refusal.
    #[test]
    fn the_tone_follows_whether_the_page_opens() {
        assert_eq!(tone(&admitted(1)), Tone::Success);
        assert_eq!(tone(&IntegrityLine::checking()), Tone::Neutral);
        for state in [
            IntegrityState::Mismatch,
            IntegrityState::Blocked,
            IntegrityState::CouldNotCheck,
            IntegrityState::NoVersion,
            IntegrityState::AllBlocked,
        ] {
            assert!(!state.opens());
            let line = IntegrityLine {
                state,
                version: String::new(),
                checked_at_ms: None,
                key: state.key().to_owned(),
                opens: false,
            };
            assert_eq!(tone(&line), Tone::Danger, "{state:?}");
        }
    }

    /// The words never claim more than the check can support.
    #[test]
    fn the_words_never_say_certified_or_untampered() {
        let loc = Loc::for_tag("en");
        for state in [
            IntegrityState::Matches,
            IntegrityState::TrustedHere,
            IntegrityState::Checking,
        ] {
            let said = loc.t(state.key()).to_lowercase();
            for claim in ["certif", "untamper", "guarantee", "verified safe"] {
                assert!(!said.contains(claim), "{said}");
            }
        }
    }
}
