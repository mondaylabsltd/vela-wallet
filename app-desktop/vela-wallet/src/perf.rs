//! The performance recorder (native parity Phase 4): what a launch, a frame
//! and a core dispatch cost, measured in the running app rather than guessed.
//!
//! **Off unless asked.** `VELA_PERF=1` in a developer build (`dev_env`), or a
//! build with the `perf` feature (`cargo build --release --features perf`),
//! which records without the variable — the way to measure the shipping
//! optimisation level with no developer switch compiled in. Off, every entry
//! point is one load of a cached `bool`.
//!
//! What it writes, one line each on stderr, prefixed `[vela-perf]` so a
//! harness can `grep` them out of the ordinary log (`scripts/perf-run.sh`
//! does):
//!
//! ```text
//! [vela-perf] mark first_paint exec=212.4ms main=148.0ms rss=142.3MB
//! [vela-perf] hist dispatch/activity_feed n=14 p50=0.081ms p95=0.410ms p99=0.410ms max=0.410ms
//! [vela-perf] hist frame/build n=480 p50=2.1ms p95=3.4ms p99=4.0ms max=5.2ms
//! ```
//!
//! * `exec` is measured from the kernel's own start time for this process
//!   (before dyld, static initialisers and `main`), `main` from the first line
//!   of `main`. A mark is written once per name.
//! * `hist` lines are written by [`report`]: at the end of a scroll run, at
//!   the idle checkpoints, and on quit.
//!
//! What it never writes: anything the person owns — no address, amount, key or
//! URL. Names and durations only.

use std::collections::{BTreeMap, HashSet};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// When `main` started. Set by [`start`]; read by every mark.
static MAIN_START: OnceLock<Instant> = OnceLock::new();

/// Whether this run records. Read once.
pub fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| cfg!(feature = "perf") || crate::dev_env::flag!("VELA_PERF"))
}

/// The first line of `main`: the clock every `main=` is read against.
pub fn start() {
    MAIN_START.get_or_init(Instant::now);
    if enabled() {
        emit(format_args!(
            "start exec_to_main={} rss={}",
            exec_ms().map_or_else(|| "?".to_owned(), |ms| format!("{ms:.1}ms")),
            rss_text()
        ));
    }
}

/// Milliseconds since `main`.
fn main_ms() -> f64 {
    MAIN_START
        .get()
        .map_or(0.0, |start| start.elapsed().as_secs_f64() * 1000.0)
}

/// A named moment, written once: `first_paint`, `home_painted`, ….
pub fn mark(name: &'static str) {
    if !enabled() {
        return;
    }
    static SEEN: Mutex<Option<HashSet<&'static str>>> = Mutex::new(None);
    {
        let mut seen = SEEN.lock().unwrap_or_else(PoisonError::into_inner);
        if !seen.get_or_insert_with(HashSet::new).insert(name) {
            return;
        }
    }
    let (reads, parses) = {
        let series = SERIES.lock().unwrap_or_else(PoisonError::into_inner);
        let count = |name| series.get(&("store", name)).map_or(0, Vec::len);
        (count("read"), count("parse"))
    };
    emit(format_args!(
        "mark {name} exec={} main={:.1}ms rss={} store_reads={reads} store_parses={parses}",
        exec_ms().map_or_else(|| "?".to_owned(), |ms| format!("{ms:.1}ms")),
        main_ms(),
        rss_text()
    ));
}

/// A free-form line, for what is not a mark or a histogram (a checkpoint's
/// RSS, a scroll run's summary).
pub fn note(message: std::fmt::Arguments<'_>) {
    if enabled() {
        emit(message);
    }
}

fn emit(message: std::fmt::Arguments<'_>) {
    eprintln!("[vela-perf] {message}");
}

// ---------------------------------------------------------------------------
// Histograms
// ---------------------------------------------------------------------------

/// The most samples kept per series: a 5-minute idle at 120 Hz would be
/// 36 000 frames, and a percentile over the latest 50 000 is as good as one
/// over all of them.
const MAX_SAMPLES: usize = 50_000;

type Series = BTreeMap<(&'static str, &'static str), Vec<u32>>;

static SERIES: Mutex<Series> = Mutex::new(BTreeMap::new());

/// One sample of `kind/name` (`dispatch/session`, `frame/build`), in
/// microseconds.
pub fn record(kind: &'static str, name: &'static str, took: Duration) {
    if !enabled() {
        return;
    }
    let micros = u32::try_from(took.as_micros()).unwrap_or(u32::MAX);
    let mut series = SERIES.lock().unwrap_or_else(PoisonError::into_inner);
    let samples = series.entry((kind, name)).or_default();
    if samples.len() >= MAX_SAMPLES {
        samples.remove(0);
    }
    samples.push(micros);
}

/// A stopwatch that records into `kind/name` when dropped — `None` inside
/// when recording is off, so an off build pays one branch.
pub struct Span {
    kind: &'static str,
    name: &'static str,
    started: Option<Instant>,
}

/// Start a [`Span`].
pub fn span(kind: &'static str, name: &'static str) -> Span {
    Span {
        kind,
        name,
        started: enabled().then(Instant::now),
    }
}

impl Drop for Span {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            record(self.kind, self.name, started.elapsed());
        }
    }
}

/// The short name of a machine type: `vela_core::app::session::Session` →
/// `Session`. Generic parameters are dropped with the path.
pub fn short_type_name<T: ?Sized>() -> &'static str {
    let full = std::any::type_name::<T>();
    let head = full.split('<').next().unwrap_or(full);
    head.rsplit("::").next().unwrap_or(head)
}

/// p50/p95/p99/max of `samples` (microseconds), nearest-rank.
#[must_use]
pub fn percentiles(samples: &[u32]) -> Option<[u32; 4]> {
    if samples.is_empty() {
        return None;
    }
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let rank = |p: f64| {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss,
            reason = "a rank inside a Vec's length"
        )]
        let index = ((p * sorted.len() as f64).ceil() as usize).clamp(1, sorted.len()) - 1;
        sorted[index]
    };
    Some([rank(0.50), rank(0.95), rank(0.99), sorted[sorted.len() - 1]])
}

/// Write every series' percentiles under `reason`, keep the samples.
pub fn report(reason: &str) {
    if !enabled() {
        return;
    }
    let series = SERIES.lock().unwrap_or_else(PoisonError::into_inner);
    emit(format_args!(
        "report {reason} main={:.1}ms rss={}",
        main_ms(),
        rss_text()
    ));
    for ((kind, name), samples) in series.iter() {
        if let Some([p50, p95, p99, max]) = percentiles(samples) {
            let ms = |micros: u32| f64::from(micros) / 1000.0;
            emit(format_args!(
                "hist {kind}/{name} n={} p50={:.3}ms p95={:.3}ms p99={:.3}ms max={:.3}ms",
                samples.len(),
                ms(p50),
                ms(p95),
                ms(p99),
                ms(max)
            ));
        }
    }
}

/// Forget the series of `kind` — a scroll run measures its own frames, not
/// the launch's.
pub fn clear(kind: &'static str) {
    if !enabled() {
        return;
    }
    SERIES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .retain(|(series_kind, _), _| *series_kind != kind);
}

// ---------------------------------------------------------------------------
// Frames
// ---------------------------------------------------------------------------

thread_local! {
    /// When the current frame's root render began, for [`frame_painted`].
    static FRAME_BEGAN: std::cell::Cell<Option<Instant>> = const { std::cell::Cell::new(None) };
}

/// The root view's `render` has begun: the frame's build starts here. A
/// view rendered inside the root calls it too, and is then a no-op — the
/// frame began at the outermost render.
pub fn frame_begin() {
    if enabled() {
        FRAME_BEGAN.with(|began| {
            if began.get().is_none() {
                began.set(Some(Instant::now()));
            }
        });
    }
}

/// An element that, painted last in the root, closes the frame's build:
/// render → layout → prepaint → paint, recorded as `frame/build`. Zero size;
/// draws nothing. `None` when recording is off, so the tree is unchanged.
pub fn frame_probe() -> Option<gpui::Canvas<()>> {
    if !enabled() {
        return None;
    }
    Some(gpui::canvas(
        |_, _, _| {},
        |_, (), _, _| {
            if let Some(began) = FRAME_BEGAN.with(std::cell::Cell::take) {
                record("frame", "build", began.elapsed());
            }
            mark("first_paint");
        },
    ))
}

/// An element that writes the mark `name` when it is first PAINTED — the
/// frame that puts it on screen, not the render that decided to.
pub fn paint_mark(name: &'static str) -> Option<gpui::Canvas<()>> {
    if !enabled() {
        return None;
    }
    Some(gpui::canvas(|_, _, _| {}, move |_, (), _, _| mark(name)))
}

/// The idle checkpoints: every series and the RSS, 10 s, 60 s and 5 min
/// after the window opened, whatever the person is doing.
pub fn checkpoints(cx: &mut gpui::App) {
    if !enabled() {
        return;
    }
    cx.spawn(async move |cx| {
        let mut waited = 0;
        for at in [10_u64, 60, 300] {
            cx.background_executor()
                .timer(Duration::from_secs(at - waited))
                .await;
            waited = at;
            report(&format!("idle_{at}s"));
        }
    })
    .detach();
}

// ---------------------------------------------------------------------------
// The scroll run
// ---------------------------------------------------------------------------

/// A scripted scroll over a column, a frame at a time — what a trackpad
/// flick does to the layout, with no trackpad. Moves `SPEED` px/s down to the
/// end, then back up, for [`ScrollRun::LENGTH`], recording the time between
/// frames (`scroll/interval`) beside each frame's build (`frame/build`).
pub struct ScrollRun {
    started: Instant,
    last: Option<Instant>,
    down: bool,
    offset: f32,
    frames: u32,
}

impl ScrollRun {
    /// How long a run lasts.
    pub const LENGTH: Duration = Duration::from_secs(6);
    /// How fast it moves, px/s: a brisk flick.
    const SPEED: f32 = 2400.;

    pub fn new() -> Self {
        clear("frame");
        clear("scroll");
        note(format_args!("scroll begin rss={}", rss_text()));
        Self {
            started: Instant::now(),
            last: None,
            down: true,
            offset: 0.,
            frames: 0,
        }
    }

    /// One frame: move `handle`, ask for the next frame. `false` once the run
    /// is over (its report written).
    pub fn step(&mut self, handle: &gpui::ScrollHandle, window: &mut gpui::Window) -> bool {
        let now = Instant::now();
        let dt = self.last.map_or(0., |last| {
            let gap = now.duration_since(last);
            record("scroll", "interval", gap);
            gap.as_secs_f32()
        });
        self.last = Some(now);
        self.frames += 1;
        let max = f32::from(handle.max_offset().y);
        let delta = Self::SPEED * dt;
        if self.down {
            self.offset = (self.offset + delta).min(max);
            if self.offset >= max {
                self.down = false;
            }
        } else {
            self.offset = (self.offset - delta).max(0.);
            if self.offset <= 0. {
                self.down = true;
            }
        }
        handle.set_offset(gpui::point(gpui::px(0.), gpui::px(-self.offset)));
        if now.duration_since(self.started) >= Self::LENGTH {
            let series = SERIES.lock().unwrap_or_else(PoisonError::into_inner);
            let over = |budget_us: u32| {
                series.get(&("scroll", "interval")).map_or(0, |samples| {
                    samples.iter().filter(|&&s| s > budget_us).count()
                })
            };
            // A frame is dropped when its gap is half a period over: 8.3 ms
            // at 120 Hz arrives as 8.3 ± jitter, a dropped one as 16.7.
            let (over_120, over_60) = (over(12_500), over(25_000));
            drop(series);
            #[allow(clippy::cast_precision_loss, reason = "a frame count over seconds")]
            let fps = self.frames as f32 / Self::LENGTH.as_secs_f32();
            note(format_args!(
                "scroll end frames={} fps={fps:.1} max_offset={max:.0}px dropped_at_120hz={over_120} dropped_at_60hz={over_60}",
                self.frames
            ));
            report("scroll");
            return false;
        }
        window.request_animation_frame();
        true
    }
}

// ---------------------------------------------------------------------------
// The process
// ---------------------------------------------------------------------------

/// Resident set size, bytes.
#[cfg(target_os = "macos")]
pub fn rss_bytes() -> Option<u64> {
    let mut info: libc::proc_taskinfo = unsafe { std::mem::zeroed() };
    let size = i32::try_from(std::mem::size_of::<libc::proc_taskinfo>()).ok()?;
    // SAFETY: `info` is a zeroed `proc_taskinfo` and `size` is its size; the
    // call writes at most that many bytes.
    let written = unsafe {
        libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTASKINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            size,
        )
    };
    (written == size).then_some(info.pti_resident_size)
}

#[cfg(target_os = "linux")]
pub fn rss_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * 4096)
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
pub fn rss_bytes() -> Option<u64> {
    None
}

fn rss_text() -> String {
    #[allow(clippy::cast_precision_loss, reason = "megabytes for a log line")]
    rss_bytes().map_or_else(
        || "?".to_owned(),
        |bytes| format!("{:.1}MB", bytes as f64 / 1_048_576.0),
    )
}

/// Milliseconds since the kernel started this process — dyld, static
/// initialisers and all — where the platform says when that was.
#[cfg(target_os = "macos")]
fn exec_ms() -> Option<f64> {
    let mut info: libc::proc_bsdinfo = unsafe { std::mem::zeroed() };
    let size = i32::try_from(std::mem::size_of::<libc::proc_bsdinfo>()).ok()?;
    // SAFETY: as in `rss_bytes`, for `proc_bsdinfo`.
    let written = unsafe {
        libc::proc_pidinfo(
            libc::getpid(),
            libc::PROC_PIDTBSDINFO,
            0,
            std::ptr::from_mut(&mut info).cast(),
            size,
        )
    };
    if written != size {
        return None;
    }
    #[allow(clippy::cast_precision_loss, reason = "epoch seconds and micros")]
    let started = info.pbi_start_tvsec as f64 * 1000.0 + info.pbi_start_tvusec as f64 / 1000.0;
    Some(crate::executor::now_ms() - started)
}

#[cfg(not(target_os = "macos"))]
fn exec_ms() -> Option<f64> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_are_nearest_rank() {
        let samples: Vec<u32> = (1..=100).collect();
        assert_eq!(percentiles(&samples), Some([50, 95, 99, 100]));
        assert_eq!(percentiles(&[7]), Some([7, 7, 7, 7]));
        assert_eq!(percentiles(&[]), None);
    }

    #[test]
    fn a_machine_is_named_by_its_last_segment() {
        struct Machine;
        assert_eq!(short_type_name::<Machine>(), "Machine");
        assert_eq!(short_type_name::<Vec<u8>>(), "Vec");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn the_process_reports_its_own_size_and_age() {
        assert!(rss_bytes().is_some_and(|bytes| bytes > 0));
        assert!(exec_ms().is_some_and(|ms| ms >= 0.0));
    }
}
