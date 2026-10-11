//! `VELA_LAYOUT_PROBE=<file>` (developer builds): where the layout put the
//! elements a measurement pass asks about, written to `<file>`.
//!
//! A screenshot pass can count pixels, and for a colour that is enough (the
//! confirm is the one accent box in the column). It cannot say how tall a
//! box is against what stands in it, or where a row sits once the column has
//! scrolled it out of the window. This says it from the layout itself: an
//! element that is asked about carries a [`mark`], the mark reports the box
//! gpui gave its parent on every frame, and the file holds the last answer
//! per name — one line each, `name x y width height`, in window points, with
//! the column's scroll already applied (a row scrolled above the window has
//! a negative `y`).
//!
//! Nothing here exists outside a developer build: the switch is read through
//! [`crate::dev_env`], so in a release binary [`on`] is `false` at compile
//! time, no mark is added to any element, and the variable's name is not in
//! the binary (`scripts/check-store-binary.sh`).

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock, PoisonError};

use gpui::{Bounds, IntoElement, Pixels, Styled as _, canvas};

struct Sink {
    path: PathBuf,
    /// The last box reported under each name.
    seen: BTreeMap<String, [f32; 4]>,
}

fn sink() -> Option<&'static Mutex<Sink>> {
    static SINK: OnceLock<Option<Mutex<Sink>>> = OnceLock::new();
    SINK.get_or_init(|| {
        let path = crate::dev_env::var_os!("VELA_LAYOUT_PROBE")?;
        Some(Mutex::new(Sink {
            path: PathBuf::from(path),
            seen: BTreeMap::new(),
        }))
    })
    .as_ref()
}

/// Whether a measurement pass is listening.
#[must_use]
pub fn on() -> bool {
    sink().is_some()
}

/// The box the layout gave `name` this frame.
pub fn record(name: &str, bounds: Bounds<Pixels>) {
    write(
        name,
        [
            f32::from(bounds.origin.x),
            f32::from(bounds.origin.y),
            f32::from(bounds.size.width),
            f32::from(bounds.size.height),
        ],
    );
}

/// The file is rewritten only when an answer changed, so a settled window
/// stops writing.
fn write(name: &str, now: [f32; 4]) {
    let Some(sink) = sink() else {
        return;
    };
    let mut sink = sink.lock().unwrap_or_else(PoisonError::into_inner);
    if sink.seen.get(name) == Some(&now) {
        return;
    }
    sink.seen.insert(name.to_owned(), now);
    let text: String = sink
        .seen
        .iter()
        .map(|(name, [x, y, width, height])| {
            format!("{name} {x:.2} {y:.2} {width:.2} {height:.2}\n")
        })
        .collect();
    // Whole and at once, so a reader never sees half a list.
    let staged = sink.path.with_extension("tmp");
    if std::fs::write(&staged, text).is_ok() {
        let _ = std::fs::rename(&staged, &sink.path);
    }
}

/// A child that reports its PARENT's box under `name`: it fills the parent
/// (which has to be positioned — `relative()`), draws nothing and takes no
/// room. `None` — nothing added to the tree — when nobody is listening.
#[must_use]
pub fn mark(name: impl Into<String>) -> Option<impl IntoElement> {
    if !on() {
        return None;
    }
    let name = name.into();
    Some(
        canvas(move |bounds, _, _| record(&name, bounds), |_, (), _, _| {})
            .absolute()
            .inset_0(),
    )
}

/// A child that reports where a scrolling column stands, under `name`: how
/// far down it is scrolled, how far it can scroll, and how tall and how far
/// down the window its own box is (`name scrolled max height top`). Put
/// AFTER the column among its parent's children, so the column has been
/// placed this frame when it reports.
#[must_use]
pub fn scroll_mark(
    name: impl Into<String>,
    column: gpui::ScrollHandle,
) -> Option<impl IntoElement> {
    if !on() {
        return None;
    }
    let name = name.into();
    Some(
        canvas(
            move |_, _, _| {
                let viewport = column.bounds();
                write(
                    &name,
                    [
                        -f32::from(column.offset().y),
                        f32::from(column.max_offset().y),
                        f32::from(viewport.size.height),
                        f32::from(viewport.origin.y),
                    ],
                );
            },
            |_, (), _, _| {},
        )
        .absolute()
        .size_0(),
    )
}
