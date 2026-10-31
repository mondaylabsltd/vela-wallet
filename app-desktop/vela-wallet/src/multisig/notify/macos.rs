//! macOS: whether the notification center can be used here, and the
//! person's permission, asked the first time the switch is turned on.
//!
//! Posting itself is gpui's (`App::show_system_notification`, the vendored
//! `gpui_macos/src/system_notifications.rs`: `UNUserNotificationCenter`).
//! This only asks first — gpui asks lazily, on its first post, so without
//! this the first notification would be posted before the answer and lost —
//! and remembers the answer, so a refusal shows the in-app line instead of
//! posting into nothing.
//!
//! Plain `msg_send!` against the frameworks' classes (the objc2 generation
//! gpui pins for this crate, as `tray/macos.rs`), so no binding crate
//! enters the build. `UNUserNotificationCenter` raises — and aborts the
//! process — when the binary is not inside an app bundle (`cargo run`), so
//! nothing here touches it without a bundle identifier.

use std::sync::atomic::{AtomicU8, Ordering};

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::{msg_send, msg_send_id};

/// Where the permission stands.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Permission {
    /// Not asked yet, or the answer has not come back.
    Unknown,
    Granted,
    Denied,
    /// No bundle (a bare `cargo run` binary): the center cannot be used.
    Unavailable,
}

static PERMISSION: AtomicU8 = AtomicU8::new(0);

fn store(p: Permission) {
    PERMISSION.store(
        match p {
            Permission::Unknown => 0,
            Permission::Granted => 1,
            Permission::Denied => 2,
            Permission::Unavailable => 3,
        },
        Ordering::SeqCst,
    );
}

/// The permission as last heard.
pub fn permission() -> Permission {
    match PERMISSION.load(Ordering::SeqCst) {
        1 => Permission::Granted,
        2 => Permission::Denied,
        3 => Permission::Unavailable,
        _ => Permission::Unknown,
    }
}

/// The process runs from an app bundle (it has a bundle identifier).
pub fn bundled() -> bool {
    let Some(class) = AnyClass::get("NSBundle") else {
        return false;
    };
    // SAFETY: `+[NSBundle mainBundle]` and `-[NSBundle bundleIdentifier]`,
    // documented, callable from any thread; both may be nil.
    unsafe {
        let bundle: Option<Retained<AnyObject>> = msg_send_id![class, mainBundle];
        let Some(bundle) = bundle else {
            return false;
        };
        let identifier: Option<Retained<AnyObject>> = msg_send_id![&bundle, bundleIdentifier];
        identifier.is_some()
    }
}

/// `UNAuthorizationOptionSound | UNAuthorizationOptionAlert`.
const ALERT_AND_SOUND: usize = (1 << 1) | (1 << 2);

/// Ask for the permission (the system asks the person once; later calls
/// answer at once with what they chose). Without a bundle the center is
/// never touched: [`Permission::Unavailable`], said once in the log.
pub fn ask() {
    if !bundled() {
        if permission() != Permission::Unavailable {
            crate::diag::vlog!(
                "multisig-notify",
                "no app bundle (cargo run): system notifications unavailable, the in-app line shows them"
            );
        }
        store(Permission::Unavailable);
        return;
    }
    let Some(class) = AnyClass::get("UNUserNotificationCenter") else {
        store(Permission::Unavailable);
        return;
    };
    let answered = block2::RcBlock::new(|granted: Bool, error: *mut AnyObject| {
        if !error.is_null() {
            // SAFETY: a non-nil `NSError` for the duration of the callback.
            let why: Option<Retained<objc2_foundation::NSString>> =
                unsafe { msg_send_id![error, localizedDescription] };
            crate::diag::vlog!(
                "multisig-notify",
                "permission not given: {}",
                why.map(|w| w.to_string()).unwrap_or_default()
            );
        } else if !granted.as_bool() {
            crate::diag::vlog!(
                "multisig-notify",
                "permission refused: the in-app line shows them"
            );
        } else {
            crate::diag::vlog!("multisig-notify", "permission granted: the system center shows them");
        }
        store(if granted.as_bool() {
            Permission::Granted
        } else {
            Permission::Denied
        });
    });
    // SAFETY: inside a bundle (checked above) the center exists; the
    // framework copies the completion block and calls it once, on a thread
    // of its own — the block only stores an atomic.
    unsafe {
        let center: Option<Retained<AnyObject>> = msg_send_id![class, currentNotificationCenter];
        let Some(center) = center else {
            store(Permission::Unavailable);
            return;
        };
        let () = msg_send![
            &center,
            requestAuthorizationWithOptions: ALERT_AND_SOUND,
            completionHandler: &*answered
        ];
    }
}
