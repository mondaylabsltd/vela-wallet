//! When the launch animation plays (spec 012's replay rule, now one rule for
//! every shell).
//!
//! The animation is a brand moment, not a gate: it plays on a first start,
//! then stays quiet for [`LAUNCH_ANIMATION_REPLAY_AFTER_MS`] (a week), so a
//! regular user never sits through it twice and someone returning after a
//! while still gets it. A shell records the time it played under
//! [`LAUNCH_ANIMATION_STORAGE_KEY`] (epoch milliseconds, the same key on
//! every client) and asks [`launch_animation_due`] on every cold start.
//!
//! Until this module the rule lived in the web (`$lib/launch/constants.ts`
//! and inline in `app.html`) and the desktop (`theme::launch_due`), while iOS
//! played on every cold start: 2.5 s of somebody's morning, every morning.
//!
//! The web's pre-paint block in `app.html` must decide before any module (or
//! any wasm) loads, so it keeps an inline copy; `tests/vectors/
//! launch-animation.json` is what both that copy and this function are held
//! to.
//!
//! Pure: no I/O, no clock.

/// Where a shell keeps the time the animation last played: epoch
/// milliseconds as a number (the web: its decimal text in `localStorage`).
pub const LAUNCH_ANIMATION_STORAGE_KEY: &str = "vela.launch.played";

/// How long after playing it stays quiet: seven days.
pub const LAUNCH_ANIMATION_REPLAY_AFTER_MS: f64 = 7.0 * 24.0 * 60.0 * 60.0 * 1000.0;

/// Does the launch animation play on this start?
///
/// - `stored_last_played_ms`: what the shell read under
///   [`LAUNCH_ANIMATION_STORAGE_KEY`]; `None` when nothing was stored, or
///   when what was stored does not read as a number.
/// - `now_ms`: the shell's clock, epoch milliseconds.
/// - `disabled`: the person turned it off, or the start is one that never
///   shows it (a test's deterministic skip, a utility page). Wins over
///   everything.
///
/// A value that is not a finite number is treated as never played: a bad
/// write must not silence the animation for good. A time in the future (a
/// clock that was set back) keeps it quiet until a week past that time, the
/// rule every shell has always applied, kept so the web's pre-paint copy and
/// this one cannot disagree.
#[must_use]
pub fn launch_animation_due(
    stored_last_played_ms: Option<f64>,
    now_ms: f64,
    disabled: bool,
) -> bool {
    if disabled {
        return false;
    }
    match stored_last_played_ms {
        None => true,
        Some(last) if !last.is_finite() => true,
        Some(last) => now_ms - last >= LAUNCH_ANIMATION_REPLAY_AFTER_MS,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[derive(serde::Deserialize)]
    struct Case {
        name: String,
        last_played_ms: Option<f64>,
        now_ms: f64,
        disabled: bool,
        due: bool,
    }

    #[derive(serde::Deserialize)]
    struct Vectors {
        storage_key: String,
        replay_after_ms: f64,
        cases: Vec<Case>,
    }

    const VECTORS: &str = include_str!("../tests/vectors/launch-animation.json");

    #[test]
    fn the_shared_vectors_hold() {
        let vectors: Vectors = serde_json::from_str(VECTORS).expect("vectors parse");
        assert_eq!(vectors.storage_key, LAUNCH_ANIMATION_STORAGE_KEY);
        assert_eq!(
            vectors.replay_after_ms.to_bits(),
            LAUNCH_ANIMATION_REPLAY_AFTER_MS.to_bits()
        );
        assert!(vectors.cases.len() >= 8, "the vectors lost their cases");
        for case in vectors.cases {
            assert_eq!(
                launch_animation_due(case.last_played_ms, case.now_ms, case.disabled),
                case.due,
                "{}",
                case.name
            );
        }
    }

    #[test]
    fn the_week_is_the_week_every_shell_shipped() {
        assert_eq!(
            LAUNCH_ANIMATION_REPLAY_AFTER_MS.to_bits(),
            604_800_000_f64.to_bits()
        );
        assert_eq!(LAUNCH_ANIMATION_STORAGE_KEY, "vela.launch.played");
    }

    #[test]
    fn a_number_that_is_no_number_is_never_played() {
        assert!(launch_animation_due(Some(f64::NAN), 0.0, false));
        assert!(launch_animation_due(Some(f64::INFINITY), 0.0, false));
        assert!(!launch_animation_due(Some(f64::NAN), 0.0, true));
    }
}
