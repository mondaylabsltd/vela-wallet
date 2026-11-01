//! The notices, worded and posted: the switch that turns them on, the words
//! (`componentsUi.multisigNotify.*` and the transaction's title, the
//! multisig's name as the title), the platform's notification center, and
//! the in-app line when that center cannot be used.
//!
//! Posting is gpui's `App::show_system_notification` on every desktop:
//!
//! - **macOS** — `UNUserNotificationCenter` (vendored `gpui_macos`). Needs
//!   the app bundle and the person's permission, asked when the switch is
//!   turned on ([`super::macos`]); a bare `cargo run` binary has no bundle,
//!   and a refusal is a refusal: then the in-app line says it instead.
//! - **Windows** — a toast (`ToastNotificationManager`) under the app's
//!   AppUserModelID, set at launch (`main.rs`, [`APP_ID`]) and carried by
//!   the installer's shortcuts. Chosen over the notification-area icon's
//!   balloon: the balloon needs the tray icon on (or a hidden one made just
//!   for it), Windows 10 and 11 turn it into a toast anyway, and its click
//!   arrives as a window message rather than the toast's activation.
//! - **Linux** — `org.freedesktop.Notifications` over D-Bus (`notify-rust`
//!   inside gpui). A desktop without a notification daemon shows nothing;
//!   gpui logs it.
//!
//! A click on a notification (or the in-app line) brings the window forward
//! and opens the transaction's sheet on its multisig ([`clicked`]).

use std::sync::Mutex;
use std::time::Duration;

use gpui::{App, Global, SharedString, SystemNotification};

use super::{Kind, Notice};
use crate::loc::Loc;
use crate::multisig::words;

/// The store key: `"1"` when the person turned notifications on; absent
/// otherwise.
pub const PREF_KEY: &str = "vela.desktop.multisigNotifications";

/// The switch's words (title, hint): Settings → Appearance draws the switch
/// only once the corpus has both — a switch labelled with its own key path
/// is not shipped.
pub const SWITCH_KEYS: (&str, &str) = (
    "settingsModals.appearance.multisigNotifications",
    "settingsModals.appearance.multisigNotificationsHint",
);

/// The app's identity to the platform's notification center: the macOS
/// bundle identifier (`packaging/macos/Info.plist.in`), Windows'
/// AppUserModelID (`installer/VelaWallet.iss`), and the name a Linux
/// notification is from.
pub const APP_ID: &str = "app.getvela.VelaWallet";
pub const APP_NAME: &str = "Vela Wallet";

/// A notification's tag starts with this (the click's way back).
const TAG: &str = "vela-multisig";

static WANTED: Mutex<Option<bool>> = Mutex::new(None);

/// Whether the person turned notifications on (`VELA_MULTISIG_NOTIFY=1`
/// turns them on in a developer build). Read once, then kept in step by
/// [`set_wanted`].
#[must_use]
pub fn wanted() -> bool {
    if crate::dev_env::flag!("VELA_MULTISIG_NOTIFY") {
        return true;
    }
    let Ok(mut cached) = WANTED.lock() else {
        return false;
    };
    *cached.get_or_insert_with(|| {
        crate::executor::storage::read_value(PREF_KEY)
            .ok()
            .flatten()
            .is_some_and(|value| value.as_str() == Some("1"))
    })
}

/// Turn notifications on or off for good (the Settings switch). Turned on,
/// the platform's permission is asked now — never on a first post.
pub fn set_wanted(on: bool) {
    let saved = if on {
        crate::executor::storage::write_value(PREF_KEY, serde_json::json!("1"))
    } else {
        crate::executor::storage::remove_value(PREF_KEY)
    };
    if let Err(error) = saved {
        crate::diag::vlog!("multisig-notify", "the choice was not saved: {error}");
    }
    if let Ok(mut cached) = WANTED.lock() {
        *cached = Some(on);
    }
    if on {
        prepare();
    }
    // r11-10: the switch says what it did — the log, and (on) where the
    // notices will show: the platform's center, or the window's line.
    crate::diag::vlog!(
        "multisig-notify",
        "turned {}{}",
        if on { "on" } else { "off" },
        if on {
            // After the ask: a bundle-less build is known by now.
            if system_can_show() {
                " (the notification center)"
            } else {
                " (the window's line: the notification center can't be used)"
            }
        } else {
            ""
        }
    );
}

/// Ready the platform: on macOS, the permission asked (once; a later call
/// answers with the person's choice). At launch when the switch is on, and
/// whenever it is turned on.
pub fn prepare() {
    #[cfg(target_os = "macos")]
    super::macos::ask();
}

/// Where the platform's permission stands, for the log ("posted … (Granted)").
fn permission_now() -> &'static str {
    #[cfg(target_os = "macos")]
    {
        match super::macos::permission() {
            super::macos::Permission::Granted => "granted",
            super::macos::Permission::Denied => "denied",
            super::macos::Permission::Unavailable => "unavailable",
            super::macos::Permission::Unknown => "not answered yet",
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        "the platform's"
    }
}

/// The platform said it shows them (macOS: the permission granted); else the
/// in-app line says each one as well.
fn system_confirmed() -> bool {
    #[cfg(target_os = "macos")]
    {
        super::macos::permission() == super::macos::Permission::Granted
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// The platform's center can show it (else the in-app line does).
#[must_use]
pub fn system_can_show() -> bool {
    #[cfg(target_os = "macos")]
    {
        // Still waiting on the person's answer: posted all the same — gpui
        // posts into the center, which shows it once allowed.
        !matches!(
            super::macos::permission(),
            super::macos::Permission::Denied | super::macos::Permission::Unavailable
        )
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

// -- words --------------------------------------------------------------------

/// One notification, worded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Worded {
    pub tag: String,
    pub title: String,
    pub body: String,
}

/// Where a notification leads: the transaction on its multisig.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Target {
    pub multisig: String,
    pub chain_id: u64,
    pub safe_tx_hash: String,
}

fn kind_tag(kind: Kind) -> String {
    match kind {
        Kind::ApprovalNeeded => "needed".to_owned(),
        Kind::ApprovalCounted { approved, .. } => format!("counted{approved}"),
        Kind::ReadyToExecute => "ready".to_owned(),
        Kind::Executed => "executed".to_owned(),
    }
}

/// The notification's tag: one per (multisig, transaction, kind), so a
/// platform that replaces by tag never replaces another's — and (r11-10)
/// per count, so "3 need your approval" is posted, not swallowed as a
/// replacement of "2 need your approval".
#[must_use]
pub fn tag_of(notice: &Notice) -> String {
    let count = notice.count.map(|n| format!("x{n}")).unwrap_or_default();
    format!(
        "{TAG}|{}|{}|{}|{}{count}",
        notice.multisig.to_ascii_lowercase(),
        notice.chain_id,
        notice.safe_tx_hash.to_ascii_lowercase(),
        kind_tag(notice.kind)
    )
}

/// The transaction a tag leads to; `None` for any other tag.
#[must_use]
pub fn target_of(tag: &str) -> Option<Target> {
    let mut parts = tag.split('|');
    if parts.next()? != TAG {
        return None;
    }
    let multisig = parts.next()?.to_owned();
    let chain_id = parts.next()?.parse().ok()?;
    let safe_tx_hash = parts.next()?.to_owned();
    (multisig.starts_with("0x") && safe_tx_hash.starts_with("0x")).then_some(Target {
        multisig,
        chain_id,
        safe_tx_hash,
    })
}

/// The kind's words (`componentsUi.multisigNotify.*`).
#[must_use]
pub fn kind_words(loc: &Loc, kind: Kind) -> String {
    match kind {
        Kind::ApprovalNeeded => loc
            .t("componentsUi.multisigNotify.approvalNeeded")
            .to_string(),
        Kind::ApprovalCounted {
            approved,
            threshold,
        } => loc
            .t_texts(
                "componentsUi.multisigNotify.approvalCounted",
                &[
                    ("approved", &approved.to_string()),
                    ("threshold", &threshold.to_string()),
                ],
            )
            .to_string(),
        Kind::ReadyToExecute => loc
            .t("componentsUi.multisigNotify.readyToExecute")
            .to_string(),
        Kind::Executed => loc.t("componentsUi.multisigNotify.executed").to_string(),
    }
}

/// One notice in words: the multisig's name (else its short address) over
/// the kind's words and the transaction's title (the web's `actionTitle`,
/// when the view says what it is). `who` names a member, `token` reads a
/// token's symbol and decimals.
#[must_use]
pub fn word(
    loc: &Loc,
    notice: &Notice,
    who: &dyn Fn(&str) -> String,
    token: &dyn Fn(u64, Option<&str>) -> words::TokenInfo,
) -> Worded {
    let title = notice
        .name
        .clone()
        .unwrap_or_else(|| words::short(&notice.multisig));
    // r11-10: a count said for a multisig not shown ("2 need your
    // approval"), in the words its switcher row and home use.
    let kind = match (notice.kind, notice.count) {
        (Kind::ApprovalNeeded, Some(n)) => {
            words::tc(loc, "presence.needsYou", usize::try_from(n).unwrap_or(usize::MAX))
        }
        (Kind::ReadyToExecute, Some(n)) => {
            words::tc(loc, "presence.ready", usize::try_from(n).unwrap_or(usize::MAX))
        }
        (kind, _) => kind_words(loc, kind),
    };
    let body = match &notice.what {
        Some(what) => {
            let chain_id = notice.chain_id;
            let action = words::action_title(
                loc,
                what.action.as_ref(),
                &what.nonce.to_string(),
                who,
                &|t| token(chain_id, t),
            );
            format!("{kind} · {action}")
        }
        None => kind,
    };
    Worded {
        tag: tag_of(notice),
        title,
        body,
    }
}

/// Every notice worded in the language in force, the names the person
/// knows, a token's facts read from its chain. BLOCKS (a token contract's
/// symbol): run off the UI thread.
fn word_all(notices: &[Notice]) -> Vec<Worded> {
    let loc = Loc::from_env();
    let mut known: Vec<(String, String)> = crate::multisig::host::local_accounts()
        .into_iter()
        .map(|a| (a.address, a.name))
        .collect();
    known.extend(crate::executor::contacts::named_contacts());
    let who = |address: &str| {
        known
            .iter()
            .find(|(a, _)| a.eq_ignore_ascii_case(address))
            .map(|(_, name)| name.clone())
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| words::short(address))
    };
    notices
        .iter()
        .map(|n| word(&loc, n, &who, &words::token_info))
        .collect()
}

// -- posting ------------------------------------------------------------------

/// The line shown in the window when the platform cannot show it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InAppLine {
    pub worded: Worded,
    pub target: Option<Target>,
    /// Bumped per line, so an older line's timer never takes a newer one.
    pub serial: u64,
}

/// The in-app line now, if any (the wallet page draws it).
#[derive(Default)]
pub struct InApp(pub Option<InAppLine>);

impl Global for InApp {}

/// The in-app line now.
#[must_use]
pub fn in_app(cx: &App) -> Option<InAppLine> {
    cx.try_global::<InApp>().and_then(|g| g.0.clone())
}

/// Take the in-app line away (pressed, dismissed, or read: in front for
/// [`super::attention::SEEN_FOR`]).
pub fn dismiss_in_app(cx: &mut App) {
    if in_app(cx).is_some() {
        cx.set_global(InApp(None));
        cx.refresh_windows();
    }
    super::attention::seen();
}

/// Word `notices` off the UI thread, then post them.
pub fn post(notices: Vec<Notice>, cx: &mut App) {
    if notices.is_empty() {
        return;
    }
    let (tx, rx) = futures::channel::oneshot::channel();
    let started = std::thread::Builder::new()
        .name("vela-multisig-notify".to_owned())
        .spawn(move || {
            let _ = tx.send(word_all(&notices));
        });
    if started.is_err() {
        return;
    }
    cx.spawn(async move |cx| {
        if let Ok(worded) = rx.await {
            cx.update(|cx| show(worded, cx));
        }
    })
    .detach();
}

/// Post worded notifications now: the platform's center, else the line.
pub fn show(worded: Vec<Worded>, cx: &mut App) {
    // r10 S4: posted while the window is not in front, the app's icon says
    // so until it is (the Dock badge, the taskbar's flash).
    super::attention::noticed(worded.len(), cx);
    if system_can_show() {
        for w in &worded {
            crate::diag::vlog!("multisig-notify", "posted {} ({})", w.tag, permission_now());
            cx.show_system_notification(SystemNotification {
                tag: SharedString::from(w.tag.clone()),
                title: SharedString::from(w.title.clone()),
                body: SharedString::from(w.body.clone()),
                actions: Vec::new(),
            });
        }
        // r9-8: posted while the person has not answered the permission yet
        // (or the system has not told): the center may drop it — an ad-hoc
        // bundle is refused outright — so the window says it too.
        if system_confirmed() {
            return;
        }
    }
    // The newest one says it; the ones before it were the same moment's.
    let Some(last) = worded.into_iter().last() else {
        return;
    };
    crate::diag::vlog!(
        "multisig-notify",
        "in-app line {} ({})",
        last.tag,
        permission_now()
    );
    let serial = in_app(cx).map_or(1, |l| l.serial.wrapping_add(1));
    let target = target_of(&last.tag);
    cx.set_global(InApp(Some(InAppLine {
        worded: last,
        target,
        serial,
    })));
    cx.refresh_windows();
    // r10 S4: it stays until it was seen — the window in front for a while —
    // or dismissed; 8 s on a clock missed it with the window off-screen.
    super::attention::watch(cx);
}

/// A notification was clicked (or the in-app line pressed): the window in
/// front, and the transaction's sheet on its multisig — shown first when
/// another is, then its list read, then the sheet (a few seconds at most,
/// as a payment link waits for its page).
pub fn clicked(tag: &str, cx: &mut App) {
    cx.activate(true);
    if let Some(window) = cx.windows().first().copied() {
        let _ = window.update(cx, |_, window, _| window.activate_window());
    }
    let Some(target) = target_of(tag) else {
        return;
    };
    dismiss_in_app(cx);
    cx.spawn(async move |cx| {
        use crate::wallet::page::multisig_notify::NoticedOpen;
        let mut asked = false;
        for _ in 0..40 {
            let activate = !asked;
            let step = crate::agent::on_page(cx, |page, _, cx| {
                page.ms_open_noticed(&target, activate, cx)
            });
            match step {
                Some(NoticedOpen::Opened | NoticedOpen::Gone) => break,
                Some(NoticedOpen::Showing) => asked = true,
                Some(NoticedOpen::Waiting) | None => {}
            }
            cx.background_executor()
                .timer(Duration::from_millis(250))
                .await;
        }
    })
    .detach();
}

/// `VELA_MULTISIG_NOTIFY_TEST=1` (developer builds): one sample
/// notification a few seconds after launch, through the same words and
/// the same door as a real one — to see delivery work on a machine.
#[cfg(any(debug_assertions, feature = "dev-fixtures"))]
pub fn sample_at_launch(cx: &mut App) {
    if !crate::dev_env::flag!("VELA_MULTISIG_NOTIFY_TEST") {
        return;
    }
    prepare();
    cx.spawn(async move |cx| {
        cx.background_executor().timer(Duration::from_secs(4)).await;
        let notice = Notice {
            multisig: "0x00000000000000000000000000000000000000aa".to_owned(),
            name: Some(APP_NAME.to_owned()),
            kind: Kind::ApprovalNeeded,
            chain_id: 100,
            safe_tx_hash: format!("0x{}", "11".repeat(32)),
            what: Some(super::What {
                action: Some(vela_core::multisig::Action::Transfer {
                    token: None,
                    to: "0x00000000000000000000000000000000000000bb".to_owned(),
                    amount: "1500000000000000000".to_owned(),
                }),
                nonce: 4,
            }),
            count: None,
        };
        let loc = Loc::from_env();
        let worded = word(&loc, &notice, &|a: &str| words::short(a), &|_, _| {
            words::TokenInfo::coin("xDAI".to_owned())
        });
        crate::diag::vlog!(
            "multisig-notify",
            "sample: {} — {} (system center: {})",
            worded.title,
            worded.body,
            system_can_show()
        );
        cx.update(|cx| show(vec![worded], cx));
    })
    .detach();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::multisig::notify::What;

    fn notice(kind: Kind, what: Option<What>, name: Option<&str>) -> Notice {
        Notice {
            multisig: "0x00000000000000000000000000000000000000AA".to_owned(),
            name: name.map(str::to_owned),
            kind,
            chain_id: 100,
            safe_tx_hash: "0xABCD".to_owned(),
            what,
            count: None,
        }
    }

    /// r11-10: a count is said in the switcher's words, and its tag differs
    /// per count (a platform that replaces by tag posts the new one) while
    /// still leading to the first transaction.
    #[test]
    fn a_count_is_said_and_tagged_apart() {
        let loc = Loc::for_tag("en");
        let mut n = notice(Kind::ApprovalNeeded, None, Some("R11 Team"));
        let one = tag_of(&n);
        n.count = Some(2);
        let w = word(&loc, &n, &|a: &str| words::short(a), &coin);
        assert_eq!(w.title, "R11 Team");
        assert_eq!(w.body, "2 need your approval");
        assert_ne!(w.tag, one);
        assert_eq!(
            target_of(&w.tag).map(|t| t.safe_tx_hash),
            Some("0xabcd".to_owned())
        );
    }

    fn coin(_: u64, _: Option<&str>) -> words::TokenInfo {
        words::TokenInfo::coin("xDAI".to_owned())
    }

    #[test]
    fn a_tag_leads_back_to_its_transaction() {
        let n = notice(Kind::ReadyToExecute, None, None);
        let tag = tag_of(&n);
        assert_eq!(
            target_of(&tag),
            Some(Target {
                multisig: "0x00000000000000000000000000000000000000aa".to_owned(),
                chain_id: 100,
                safe_tx_hash: "0xabcd".to_owned(),
            })
        );
        assert_ne!(tag, tag_of(&notice(Kind::Executed, None, None)));
        assert_ne!(
            tag_of(&notice(
                Kind::ApprovalCounted {
                    approved: 1,
                    threshold: 3
                },
                None,
                None
            )),
            tag_of(&notice(
                Kind::ApprovalCounted {
                    approved: 2,
                    threshold: 3
                },
                None,
                None
            )),
        );
        assert_eq!(target_of("something-else|0xaa|1|0xbb|ready"), None);
        assert_eq!(target_of("vela-multisig|0xaa|x|0xbb|ready"), None);
    }

    #[test]
    fn titled_by_the_multisig_and_said_with_the_transaction() {
        let loc = Loc::for_tag("en");
        let what = What {
            action: Some(vela_core::multisig::Action::Transfer {
                token: None,
                to: "0x1".to_owned(),
                amount: "1500000000000000000".to_owned(),
            }),
            nonce: 4,
        };
        let w = word(
            &loc,
            &notice(Kind::ApprovalNeeded, Some(what), Some("Quiet Harbor")),
            &|a: &str| a.to_owned(),
            &coin,
        );
        assert_eq!(w.title, "Quiet Harbor");
        let kind = kind_words(&loc, Kind::ApprovalNeeded);
        assert_eq!(w.body, format!("{kind} · Send 1.5 xDAI"));
        // No name: its short address. No action known: the kind alone.
        let w = word(
            &loc,
            &notice(Kind::Executed, None, None),
            &|a: &str| a.to_owned(),
            &coin,
        );
        assert_eq!(
            w.title,
            words::short("0x00000000000000000000000000000000000000aa")
        );
        assert_eq!(w.body, kind_words(&loc, Kind::Executed));
    }

    /// The words are the corpus's in every language, or — before the
    /// corpus carries them — nowhere (never in some languages and a key
    /// path in others).
    #[test]
    fn the_words_resolve_in_every_language_or_in_none() {
        let keys = [
            SWITCH_KEYS.0,
            SWITCH_KEYS.1,
            "componentsUi.multisigNotify.approvalNeeded",
            "componentsUi.multisigNotify.approvalCounted",
            "componentsUi.multisigNotify.readyToExecute",
            "componentsUi.multisigNotify.executed",
        ];
        let en = Loc::for_tag("en");
        let landed = en.t(keys[0]).as_ref() != keys[0];
        for (tag, loc) in Loc::every_language() {
            for key in keys {
                let word = loc.t(key);
                assert_eq!(word.as_ref() != key, landed, "{tag}: {key}");
            }
            if landed {
                let counted = kind_words(
                    &loc,
                    Kind::ApprovalCounted {
                        approved: 2,
                        threshold: 3,
                    },
                );
                assert!(
                    counted.contains('2') && counted.contains('3') && !counted.contains("{{"),
                    "{tag}: {counted}"
                );
            }
        }
    }

    #[test]
    fn notifications_are_off_until_the_person_turns_them_on() {
        crate::executor::storage::tests::with_temp_state("multisig-notify-pref", || {
            if let Ok(mut cached) = WANTED.lock() {
                *cached = None;
            }
            assert!(!wanted(), "off by default");
            set_wanted(true);
            assert!(wanted());
            assert_eq!(
                crate::executor::storage::read_value(PREF_KEY)
                    .ok()
                    .flatten(),
                Some(serde_json::json!("1"))
            );
            set_wanted(false);
            assert!(!wanted());
            assert_eq!(
                crate::executor::storage::read_value(PREF_KEY)
                    .ok()
                    .flatten(),
                None
            );
        });
    }
}
