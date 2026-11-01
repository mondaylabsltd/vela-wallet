//! Notices the person has not seen yet (r10 S4): the in-app line lasted 8 s
//! and was missed with the window off-screen. Now a notice is kept until the
//! window has been in front long enough to read it ([`SEEN_FOR`]) or the
//! person dismisses it, and while any is unseen the app's icon says so where
//! the platform lets an app say it:
//!
//! - **macOS** — the Dock icon's badge (`NSDockTile.badgeLabel`, the count),
//!   and one informational bounce (gpui's `request_attention`);
//! - **Windows** — the taskbar button flashes (gpui's `request_attention`:
//!   `FlashWindowEx`); a numbered overlay needs an icon drawn per count, which
//!   the toast already carries;
//! - **Linux** — the window manager's urgency hint, where gpui sets one.
//!
//! Nothing here decides what is news — that is the notice rule's
//! (`super::super::notify`) — only whether the person has had it in front of
//! them.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use gpui::App;

/// How long the window must be in front for an in-app line to count as
/// read — the old line's whole life, now counted only while it can be seen.
pub const SEEN_FOR: Duration = Duration::from_secs(8);

/// How often the watcher looks.
const TICK: Duration = Duration::from_millis(500);

/// Notices posted while the app was not in front, not seen yet.
static UNSEEN: AtomicU32 = AtomicU32::new(0);
/// The watcher is running.
static WATCHING: AtomicBool = AtomicBool::new(false);

/// The app is in front: one of its windows is the key window.
#[must_use]
pub fn in_front(cx: &App) -> bool {
    cx.active_window().is_some()
}

/// How many notices are waiting to be seen (the badge's count).
#[must_use]
pub fn unseen() -> u32 {
    UNSEEN.load(Ordering::SeqCst)
}

/// `count` notices were just posted. In front, the person sees them now:
/// nothing to raise. Otherwise the icon says so, and the watcher clears it
/// once the app comes forward.
pub fn noticed(count: usize, cx: &mut App) {
    if count == 0 || in_front(cx) {
        return;
    }
    let count = u32::try_from(count).unwrap_or(u32::MAX);
    let now = UNSEEN
        .fetch_add(count, Ordering::SeqCst)
        .saturating_add(count);
    set_badge(now);
    if let Some(window) = cx.windows().first().copied() {
        let _ = window.update(cx, |_, window, _| window.request_attention());
    }
    crate::diag::vlog!(
        "multisig-notify",
        "unseen {now}: the icon says so until the window is in front"
    );
    watch(cx);
}

/// Everything is seen (the app came forward, or the line was dismissed).
pub fn seen() {
    if UNSEEN.swap(0, Ordering::SeqCst) > 0 {
        set_badge(0);
    }
}

/// While notices are unseen or an in-app line is up: clear the badge once the
/// app is in front, and take the line away once it has been in front for
/// [`SEEN_FOR`] in all.
pub fn watch(cx: &mut App) {
    if WATCHING.swap(true, Ordering::SeqCst) {
        return;
    }
    cx.spawn(async move |cx| {
        let mut front = Duration::ZERO;
        let mut line_serial = None;
        loop {
            cx.background_executor().timer(TICK).await;
            let more = cx.update(|cx| {
                let shown = in_front(cx);
                if shown {
                    seen();
                }
                let line = super::deliver::in_app(cx);
                // A newer line starts its own count.
                if line.as_ref().map(|l| l.serial) != line_serial {
                    line_serial = line.as_ref().map(|l| l.serial);
                    front = Duration::ZERO;
                }
                if line.is_some() && shown {
                    front += TICK;
                    if front >= SEEN_FOR {
                        super::deliver::dismiss_in_app(cx);
                    }
                }
                super::deliver::in_app(cx).is_some() || unseen() > 0
            });
            if !more {
                break;
            }
        }
        WATCHING.store(false, Ordering::SeqCst);
    })
    .detach();
}

/// The Dock badge: the count, or none.
#[cfg(target_os = "macos")]
fn set_badge(count: u32) {
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2::{msg_send, msg_send_id};
    let Some(class) = AnyClass::get("NSApplication") else {
        return;
    };
    // SAFETY: `+[NSApplication sharedApplication]` and `-[NSApplication
    // dockTile]` are documented and return the app's objects; this runs on
    // the main thread (gpui's foreground executor, or a UI callback).
    // `-[NSDockTile setBadgeLabel:]` takes an NSString or nil.
    unsafe {
        let app: Option<Retained<AnyObject>> = msg_send_id![class, sharedApplication];
        let Some(app) = app else {
            return;
        };
        let tile: Option<Retained<AnyObject>> = msg_send_id![&app, dockTile];
        let Some(tile) = tile else {
            return;
        };
        if count == 0 {
            let nil: *const AnyObject = std::ptr::null();
            let () = msg_send![&tile, setBadgeLabel: nil];
        } else {
            let label = objc2_foundation::NSString::from_str(&count.to_string());
            let () = msg_send![&tile, setBadgeLabel: &*label];
        }
    }
}

#[cfg(not(target_os = "macos"))]
fn set_badge(_count: u32) {}
