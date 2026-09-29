//! The dApp browser's engine — one system webview, living in one column.
//!
//! ## Why a native subview is fine here
//!
//! wry attaches the platform webview (WKWebView on macOS, WebView2 on Windows)
//! as a CHILD of the window gpui already draws, over `raw-window-handle` 0.6 —
//! the one version gpui and wry both pin, so the handle is literally the same
//! type on both sides. What that buys is real: the same engine the person's
//! other browser uses, no second rendering stack, no 200MB of CEF.
//!
//! What it costs is that the subview composites ABOVE everything gpui paints,
//! and that cost lands in exactly two places rather than everywhere:
//!
//! - **Leaving the browser.** A native subview does not disappear because a
//!   gpui route changed. Nothing hides it but this module, so [`hide`] is
//!   called on every frame that is not drawing the browser column. Forget it
//!   and the webview floats over the wallet.
//! - **Centred overlays.** A dialog gpui draws in the middle of the window
//!   (the scanner, `settings_dialog_overlay`) would be painted UNDER the
//!   browser. Those are the ones that must move the webview out of the way.
//!
//! The signing panel is NOT one of them, and that is a layout fact rather than
//! luck: on desktop it is a third COLUMN (`PanelId::Signing` →
//! `panel_scaffold`, the same scaffold Receive and the asset detail use), so it
//! sits beside the browser and shrinks it. The phone's clear-signing drawings
//! show a sheet over the page because a phone has one column to work with.
//!
//! ## What the page gets, and what it can say
//!
//! One initialization script from the core — `dapp_rpc::provider_script`,
//! the provider and its bridge, the same bytes Android and iOS inject with
//! only the post function different (spec 070 FR-007). Until 070 this file
//! assembled the provider itself from the extension's two module files,
//! stripping `import`/`export` line by line, and carried its own bridge.
//!
//! What comes back is a string, and this module attaches the two facts the
//! page cannot forge: the URL of the document that SENT it, as the platform
//! reports it (the `WKScriptMessage`'s frame on macOS, the message's `Source`
//! on Windows), and whether that document is the top one. The core derives
//! the origin from the first, ignores anything the second says is a
//! subframe, and ignores an `origin` a page puts in its own message. Every
//! answer goes back through [`deliver`], and the bridge drops one addressed
//! to another document.

use std::cell::RefCell;

use gpui::{Bounds, Pixels, Window};

use vela_core::app::dapp_rpc::{ProviderHost, provider_script};

use crate::explore::engine::EngineFailure;

/// What the page calls itself, reported to the wallet — UI plumbing, not the
/// provider.
///
/// The phone shells get the title and icon from the native WebView's own
/// callbacks; wry has none, so the page reports them — twice, because a title
/// is usually there at DOMContentLoaded and an icon link often is not, and
/// `browser_history` is written to take a second report without clobbering
/// what the first one captured. Top frame only, like the provider: wry puts
/// every initialization script into subframes on Windows.
///
/// Spec 079 R3: the address, the title and the icon are read in ONE script
/// from ONE document — the address was read from the webview later, so a late
/// `load` from the page being left could land under the next page's URL — and
/// the main document's HTTP status where the engine says (an error page is not
/// a visit).
const META_JS: &str = r#"
(() => {
  if (window.top !== window) return;
  const meta = () => {
    const icon = document.querySelector("link[rel~='icon']");
    let status = 0;
    try {
      const nav = performance.getEntriesByType('navigation')[0];
      status = (nav && nav.responseStatus) || 0;
    } catch (_) {}
    window.ipc.postMessage(JSON.stringify({
      vela: 'meta',
      href: location.href,
      title: document.title || '',
      favicon: icon ? icon.href : '',
      status,
    }));
  };
  document.addEventListener('DOMContentLoaded', meta);
  window.addEventListener('load', meta);
})();
"#;

/// The entries the top document adds or moves through with no load (spec 083
/// W15, macOS). WKWebView reports none of them, and the back arrow counts the
/// tab's own entries (`explore::tab_history`).
///
/// `popstate` fires for a traversal inside the document and for a fragment
/// link. The list's length tells them apart: a traversal keeps it, a new
/// entry grows it. The page's own `pushState` runs first and is never held
/// up: a report that fails is dropped. Windows does not get this script,
/// because the engine forgets other tabs' entries there instead.
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
const HISTORY_JS: &str = r#"
(() => {
  if (window.top !== window) return;
  const post = (vela) => {
    try { window.ipc.postMessage(JSON.stringify({ vela })); } catch (_) {}
  };
  let length = history.length;
  // A page back from the back-forward cache kept the length it left with.
  window.addEventListener('pageshow', () => { length = history.length; });
  const push = History.prototype.pushState;
  History.prototype.pushState = function pushState() {
    const result = push.apply(this, arguments);
    length = history.length;
    post('pushed');
    return result;
  };
  window.addEventListener('popstate', () => {
    post(history.length === length ? 'popped' : 'pushed');
    length = history.length;
  });
})();
"#;

/// One string from a page, with what the platform says about who sent it.
pub struct PageMessage {
    /// The URL of the document that posted.
    pub sender: String,
    /// Whether that document is the top one. See [`top_frame_sent`].
    pub is_main_frame: bool,
    pub message_json: String,
}

/// Where a page's strings go.
///
/// Installed by the page, because only the page knows which window and which
/// column should answer. Not `Send`: it runs on the main thread, where the
/// webview's callback already is.
type MessageSink = Box<dyn Fn(PageMessage)>;

/// A document load, as the webview reports it.
pub enum Load {
    /// The wallet asked the engine to load this address — Go, a tab, a
    /// favourite, reload, Retry (spec 079: progress from the request, not from
    /// the engine's commit, which on a slow network is seconds later).
    Requested(String),
    /// A document of the SITE committed (`didCommitNavigation` on macOS;
    /// WebView2's `ContentLoading` on Windows when it is not the engine's own
    /// error page — `webview2_events`, spec 083). Settles nothing by itself —
    /// the new document's hello is what retires the old one.
    Started(String),
    /// WebView2 put its own error page where the document was (spec 083): the
    /// old document is gone, and nothing of the site arrived — not a commit.
    #[cfg_attr(not(windows), allow(dead_code))]
    ErrorPage(String),
    /// The load finished. If no document said hello since it started (an
    /// error page, a PDF), the old one is gone and its requests are settled.
    Finished(String),
    /// The navigation failed in the engine's own words (spec 083, Windows);
    /// `status` is a `COREWEBVIEW2_WEB_ERROR_STATUS`.
    #[cfg_attr(not(windows), allow(dead_code))]
    Failed {
        url: String,
        status: i64,
        certificate: bool,
    },
    /// The renderer died: WKWebView's content process on macOS, WebView2's
    /// renderer or browser process on Windows (spec 083). The page is blank
    /// and can never answer again.
    Crashed,
    /// The engine's history moved, a single-page app's route included, which
    /// is no load at all (spec 083 W15, Windows): only the arrows change.
    #[cfg_attr(not(windows), allow(dead_code))]
    HistoryChanged,
    /// The top document added a history entry of its own with no load (a
    /// `pushState`, a fragment link). This is the page's own report, through
    /// [`HISTORY_JS`] (spec 083 W15, macOS).
    Pushed,
    /// The top document moved through its own entries with no load (spec
    /// 083 W15, macOS).
    Popped,
    /// The engine answered [`forget_history_behind`] for the tab change
    /// numbered here (083 W15, Windows). From now on its history is the
    /// arrows': the tab's own when it forgot, and as before 083 when it
    /// refused.
    #[cfg_attr(not(windows), allow(dead_code))]
    HistoryFloor(u64),
}

type LoadSink = Box<dyn Fn(Load)>;

/// Something a page asked for that this browser does not do in place (083).
#[derive(Debug, PartialEq, Eq)]
pub enum Leave {
    /// A person's `target=_blank` / `window.open` of a web address: a new tab.
    NewTab(String),
    /// `mailto:` / `tel:`: the system's handler, through the wallet's opener.
    External(String),
}

type LeaveSink = Box<dyn Fn(Leave)>;

/// Where an address may go (083) — one rule for navigations, new windows and
/// Windows' launches of other apps.
#[derive(Debug, PartialEq, Eq)]
enum Scheme {
    Web,
    Local,
    External,
    Refused,
}

fn scheme_of(url: &str) -> Scheme {
    let scheme = url
        .split_once(':')
        .map(|(scheme, _)| scheme.to_ascii_lowercase())
        .unwrap_or_default();
    match scheme.as_str() {
        "http" | "https" => Scheme::Web,
        // Documents and frames the page builds itself: never a tab, never an app.
        "about" | "blob" | "data" => Scheme::Local,
        // Only these two leave: Windows' other protocol handlers (ms-msdt:,
        // search-ms:) have been remote-code holes, and wc: is not how Vela
        // connects.
        "mailto" | "tel" => Scheme::External,
        _ => Scheme::Refused,
    }
}

/// A new-window request: only a person's gesture opens anything (083 W6).
/// A web address is a tab, whichever frame asked, as in any browser; another
/// app only from the page's own document (see [`other_app_leave`]). Windows
/// only for now: wry's macOS hook carries no gesture.
#[cfg_attr(not(windows), allow(dead_code))]
fn new_window_leave(url: &str, gesture: bool, from_page: bool) -> Option<Leave> {
    match (gesture, scheme_of(url)) {
        (true, Scheme::Web) => Some(Leave::NewTab(url.to_owned())),
        (true, Scheme::External) => other_app_leave(url, gesture, from_page),
        _ => None,
    }
}

/// WebView2 about to hand an address to another app, already cancelled (083
/// W7): only `mailto:`/`tel:`, only on a person's tap, only from the page's
/// own document — a tap inside a cross-origin frame (an ad) is not the page's
/// — and only as [`crate::executor::opener::command_value`] escapes it, since
/// Vela now launches what the engine would have escaped.
#[cfg_attr(not(windows), allow(dead_code))]
fn other_app_leave(url: &str, gesture: bool, from_page: bool) -> Option<Leave> {
    (gesture && from_page && scheme_of(url) == Scheme::External)
        .then(|| crate::executor::opener::command_value(url))
        .flatten()
        .map(Leave::External)
}

/// Whether the engine's word on who asked (`from`: a frame's address, or an
/// origin) names the top document's own origin (083 W7). Nothing said, or no
/// web origin on either side, is not the page.
#[cfg_attr(not(windows), allow(dead_code))]
fn asked_by_page(from: Option<&str>, top: &str) -> bool {
    let origin_of = vela_core::app::dapp_permissions::origin_of;
    match (from.and_then(origin_of), origin_of(top)) {
        (Some(from), Some(top)) => from.eq_ignore_ascii_case(&top),
        _ => false,
    }
}

/// What a page says it is called, read in one script from one document.
///
/// Untrusted, all of it, and treated as display text only. The address is the
/// document's own `location.href` when that is the SENDER's origin (the
/// platform's word, `WKScriptMessage`'s frame), and the sender's URL
/// otherwise — so a page cannot file itself under another site.
pub struct PageMeta {
    pub url: String,
    pub title: String,
    pub favicon: String,
    /// The main document's HTTP status, where the engine reports it.
    pub status: Option<u16>,
}

type MetaSink = Box<dyn Fn(PageMeta)>;

thread_local! {
    static MESSAGES: RefCell<Option<MessageSink>> = const { RefCell::new(None) };
    static LOADS: RefCell<Option<LoadSink>> = const { RefCell::new(None) };
    static META: RefCell<Option<MetaSink>> = const { RefCell::new(None) };
    /// The origin of the top document as last COMMITTED — `None` before the
    /// first commit, `Some(None)` for a top document with no web origin.
    static COMMITTED: RefCell<Option<Option<String>>> = const { RefCell::new(None) };
    /// The address of the top document as last committed (spec 079): the
    /// toolbar's host, which must not move to a site that has not loaded —
    /// WKWebView's own URL changes the moment a navigation STARTS.
    static COMMITTED_URL: RefCell<Option<String>> = const { RefCell::new(None) };
    static LEAVES: RefCell<Option<LeaveSink>> = const { RefCell::new(None) };
    /// Windows' LaunchingExternalUriScheme hook is in: mailto:/tel: pass on to
    /// it, where the engine says whether a person tapped and in which frame
    /// (083 W7). Never set on macOS.
    static EXTERNAL_GUARD: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Hand new windows and other apps' addresses to the page (083).
pub fn on_leave_to(sink: LeaveSink) {
    LEAVES.with(|slot| *slot.borrow_mut() = Some(sink));
}

#[cfg(windows)]
fn leave_to_page(leave: Leave) {
    LEAVES.with(|slot| {
        if let Some(sink) = slot.borrow().as_ref() {
            sink(leave);
        }
    });
}

/// wry's navigation handler (083): the top document on Windows, every frame
/// on macOS. Before it, anything went — on Windows the engine handed any
/// app's scheme to the system with nothing from Vela.
///
/// Nothing is launched from here: this handler never knows whether a person
/// tapped, and a timer or an ad frame setting `location = 'tel:…'` would open
/// FaceTime or Mail every time it ran. `mailto:`/`tel:` go on only where
/// Windows' launch event decides them; elsewhere (macOS, a runtime without
/// that event) they are refused, as they were before 083.
fn allow_navigation(url: String) -> bool {
    match scheme_of(&url) {
        Scheme::Web | Scheme::Local => true,
        Scheme::External => EXTERNAL_GUARD.get(),
        Scheme::Refused => false,
    }
}

/// Hand the page's strings to the wallet. Called once, when the page builds
/// the browser.
pub fn on_message_to(sink: MessageSink) {
    MESSAGES.with(|slot| *slot.borrow_mut() = Some(sink));
}

/// Hand document loads to the wallet.
///
/// The PAGE-LOAD handler, not the navigation-policy one this file used before
/// 070: that one is asked about every navigation, a subframe's included, so
/// an iframe loading was reported as the page going away.
pub fn on_load_to(sink: LoadSink) {
    LOADS.with(|slot| *slot.borrow_mut() = Some(sink));
}

/// Hand page metadata to the page (the wallet's page, that is).
pub fn on_meta_to(sink: MetaSink) {
    META.with(|slot| *slot.borrow_mut() = Some(sink));
}

/// The one browser. A `WebView` is neither `Send` nor `Sync` and belongs to the
/// window it was built as a child of, so it lives on the main thread with the
/// window rather than in a gpui global.
struct Browser {
    view: wry::WebView,
    /// What the webview was last told, so a frame that changed nothing does
    /// not cross the platform boundary sixty times a second.
    bounds: Option<Bounds<Pixels>>,
    visible: bool,
    /// The profile the view was built on (spec 083). wry reads it only while
    /// building, but documents a context as outliving its views.
    #[cfg(windows)]
    _context: wry::WebContext,
    /// The rectangle cut out of the view's window for a menu over the page
    /// (spec 083 D2), in the window's own pixels.
    #[cfg(windows)]
    hole: Option<[i32; 4]>,
}

thread_local! {
    static BROWSER: RefCell<Option<Browser>> = const { RefCell::new(None) };
    /// Why the engine did not start (spec 083 W1b). While set, nothing builds
    /// again until the person asks: a refused folder or a missing runtime does
    /// not fix itself between frames, and each attempt on Windows filed a
    /// crash report.
    static FAILED: std::cell::Cell<Option<EngineFailure>> = const { std::cell::Cell::new(None) };
}

/// Why the engine did not start, if it did not (spec 083).
#[must_use]
pub fn engine_failure() -> Option<EngineFailure> {
    FAILED.get()
}

/// The person asked to try the engine again: the next frame builds it.
pub fn retry_engine() {
    FAILED.set(None);
}

/// Whether a view exists to show.
#[must_use]
pub fn ready() -> bool {
    BROWSER.with(|slot| slot.borrow().is_some())
}

/// Whether a frame may start building one: not after a failure the person
/// has not retried, and — on Windows — not while one is being built.
fn may_build() -> bool {
    #[cfg(windows)]
    if BUILDING.get() {
        return false;
    }
    FAILED.get().is_none()
}

/// Keep what a build produced: the view, or why there is none.
fn settle(built: Result<Browser, EngineFailure>) {
    match built {
        Ok(browser) => BROWSER.with(|slot| *slot.borrow_mut() = Some(browser)),
        Err(failure) => FAILED.set(Some(failure)),
    }
}

/// Draw the browser at `bounds`, building it on first call.
///
/// Called from the paint pass of the element that OWNS that rectangle, so the
/// webview follows the column through window resizes and through a third
/// column opening beside it — the signing panel included.
pub fn place(bounds: Bounds<Pixels>, window: &Window, home: &str, cx: &mut gpui::App) {
    if !ready() {
        // Once per start, and again only on the person's Retry (spec 083):
        // this runs every frame, and before 083 a refused build was retried
        // sixty times a second.
        if !may_build() {
            return;
        }
        #[cfg(windows)]
        {
            build_later(window, home, cx);
            return;
        }
        #[cfg(not(windows))]
        {
            let _ = cx;
            settle(build(window, home));
        }
    }
    BROWSER.with(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(browser) = slot.as_mut() else {
            return;
        };
        if browser.bounds != Some(bounds) {
            let _ = browser.view.set_bounds(wry::Rect {
                position: wry::dpi::LogicalPosition::new(
                    f64::from(bounds.origin.x),
                    f64::from(bounds.origin.y),
                )
                .into(),
                size: wry::dpi::LogicalSize::new(
                    f64::from(bounds.size.width),
                    f64::from(bounds.size.height),
                )
                .into(),
            });
            browser.bounds = Some(bounds);
        }
        if !browser.visible {
            let _ = browser.view.set_visible(true);
            browser.visible = true;
        }
    });
}

/// The part of `menu` over `page`, in the page window's own pixels — `None`
/// when they do not overlap (spec 083 D2).
#[cfg(any(windows, test))]
fn hole_in(page: Bounds<Pixels>, menu: Bounds<Pixels>, scale: f32) -> Option<[i32; 4]> {
    let over = page.intersect(&menu);
    if over.size.width <= gpui::px(0.) || over.size.height <= gpui::px(0.) {
        return None;
    }
    // Outward, so no sliver of the page is left over the menu's edge.
    #[allow(clippy::cast_possible_truncation)]
    let at = |value: Pixels, round: fn(f32) -> f32| round(f32::from(value) * scale) as i32;
    Some([
        at(over.left() - page.left(), f32::floor),
        at(over.top() - page.top(), f32::floor),
        at(over.right() - page.left(), f32::ceil),
        at(over.bottom() - page.top(), f32::ceil),
    ])
}

/// A menu over the page shows through a hole cut out of the webview's window,
/// instead of the whole dApp blanking while it is open (spec 083 D2, Windows —
/// the owner's "打开 ⋯ 菜单时整个网页会消失"). `None` mends the window.
#[cfg(windows)]
pub fn cut_out(menu: Option<Bounds<Pixels>>, scale: f32) {
    BROWSER.with(|slot| {
        let Ok(mut slot) = slot.try_borrow_mut() else {
            return;
        };
        let Some(browser) = slot.as_mut() else {
            return;
        };
        let hole = menu
            .zip(browser.bounds)
            .and_then(|(menu, page)| hole_in(page, menu, scale));
        if browser.hole != hole {
            set_region(&browser.view, hole);
            browser.hole = hole;
        }
    });
}

#[cfg(windows)]
fn set_region(view: &wry::WebView, hole: Option<[i32; 4]>) {
    use windows_sys::Win32::Graphics::Gdi::{
        CombineRgn, CreateRectRgn, DeleteObject, RGN_DIFF, SetWindowRgn,
    };
    use wry::WebViewExtWindows as _;
    let hwnd = view.hwnd().0;
    // SAFETY: wry's container window, alive as long as the view. A region
    // SetWindowRgn accepted belongs to the system and is not deleted here.
    unsafe {
        let Some([left, top, right, bottom]) = hole else {
            SetWindowRgn(hwnd, std::ptr::null_mut(), 1);
            return;
        };
        // Wider than any window, so a resize under the menu never leaves part
        // of the page unpainted.
        let whole = CreateRectRgn(0, 0, i32::from(i16::MAX), i32::from(i16::MAX));
        let cut = CreateRectRgn(left, top, right, bottom);
        CombineRgn(whole, whole, cut, RGN_DIFF);
        DeleteObject(cut);
        if SetWindowRgn(hwnd, whole, 1) == 0 {
            DeleteObject(whole);
        }
    }
}

/// Take the browser off the screen.
///
/// Every frame that is not the browser column calls this, because a native
/// subview outlives the gpui route that put it there. It is idempotent and
/// costs nothing when already hidden — a render path may call it sixty times a
/// second and does. Hiding is not closing: the page keeps running, and what
/// it asked is still answered.
pub fn hide() {
    BROWSER.with(|slot| {
        if let Some(browser) = slot.borrow_mut().as_mut()
            && browser.visible
        {
            let _ = browser.view.set_visible(false);
            browser.visible = false;
        }
    });
}

/// Go to a URL the person typed or a link they picked.
pub fn navigate(url: &str) {
    let mut asked = false;
    with_view(|view| {
        asked = view.load_url(url).is_ok();
    });
    if asked {
        report(Load::Requested(url.to_owned()));
    }
}

/// Back and forward, through the engine's own history (083 W15) — the pair
/// wry has had since 0.56, and the one [`history`] reads. Asked at the click,
/// not from the arrow's last drawing: macOS has no event for a `pushState`,
/// so an arrow drawn a moment ago can be stale. `true` when the engine had
/// somewhere to go and was sent there.
pub fn back() -> bool {
    let mut went = false;
    with_view(|view| {
        went = view.can_go_back().unwrap_or(false) && view.go_back().is_ok();
    });
    went
}

pub fn forward() -> bool {
    let mut went = false;
    with_view(|view| {
        went = view.can_go_forward().unwrap_or(false) && view.go_forward().is_ok();
    });
    went
}

/// Forget every entry of the engine's history but the page on screen, which
/// is the tab's first page, numbered `floor` (083 W15). One view serves every
/// tab, so the other entries are other tabs' pages. On Windows the engine is
/// asked, and [`Load::HistoryFloor`] brings its answer. A refusal is only
/// logged: the arrows then follow the engine as before 083. macOS has no such
/// call, and the page counts the tab's entries instead.
pub fn forget_history_behind(floor: u64) {
    #[cfg(windows)]
    with_view(|view| {
        use wry::WebViewExtWindows as _;
        let asked = crate::webview2_events::forget_history_behind(&view.webview(), move |done| {
            if !done {
                eprintln!("[vela-wallet] browser: WebView2 kept the history behind a tab");
            }
            report(Load::HistoryFloor(floor));
        });
        if let Err(error) = asked {
            eprintln!("[vela-wallet] browser: WebView2 history: {error}");
            report(Load::HistoryFloor(floor));
        }
    });
    #[cfg(not(windows))]
    let _ = floor;
}

/// Whether the engine can go back / forward (083 W15): the arrows' state,
/// from the engine's own history, never the page's word.
#[must_use]
pub fn history() -> [bool; 2] {
    BROWSER.with(|slot| {
        slot.borrow().as_ref().map_or([false, false], |browser| {
            [
                browser.view.can_go_back().unwrap_or(false),
                browser.view.can_go_forward().unwrap_or(false),
            ]
        })
    })
}

/// Load the page on screen again. `true` when a load of it is under way, so
/// its landing is the same page and not a new entry (083 W15).
pub fn reload() -> bool {
    // Spec 083: after WebView2's browser process died this view can never load
    // again; dropping it lets the next frame build a new one at the page's
    // address.
    #[cfg(windows)]
    if ENGINE_GONE.replace(false) {
        let dead = BROWSER.with(|slot| slot.borrow_mut().take());
        drop(dead);
        return true;
    }
    let mut asked = false;
    with_view(|view| {
        asked = view.reload().is_ok();
    });
    if asked && let Some(url) = committed_url().or_else(current_url) {
        report(Load::Requested(url));
    }
    asked
}

/// The address of the document that last committed, whole.
#[must_use]
pub fn committed_url() -> Option<String> {
    COMMITTED_URL.with(|slot| slot.borrow().clone())
}

/// Forget every site this window has browsed (spec 081 FR-017).
///
/// The wallet's own records live in one JSON document, but the in-app browser
/// is a real web view on the platform's DEFAULT data store: cookies,
/// localStorage, IndexedDB, service workers and the HTTP cache for every dApp
/// the person opened, kept by WKWebView / WebView2 / WebKitGTK and reachable
/// from nothing in `executor::storage`. An erase that swept the document and
/// left that behind would leave the person still signed in to the exchanges
/// and dApps they visited, on a machine they had just been told was wiped.
///
/// Returns whether the platform was actually asked. `false` means there is no
/// web view in this process — the browser column was never opened — and there
/// is no portable way to reach the platform's store without one; the caller
/// logs that rather than claiming a clear it did not make. It is deliberately
/// NOT part of the erase's verification, which is over the state document:
/// a browser that was never opened this session has nothing on screen to
/// contradict, and failing the whole erase over an absent web view would tell
/// people their wallet records survived when they did not.
pub fn clear_browsing_data() -> bool {
    let live = BROWSER.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|browser| browser.view.clear_all_browsing_data().is_ok())
    });
    live.unwrap_or_else(clear_profile_on_disk)
}

/// No view this session (spec 083): on Windows the profile is a folder of
/// the wallet's own that nothing holds open, so the folder goes — the same
/// clear, without starting an engine to ask for it.
#[cfg(windows)]
fn clear_profile_on_disk() -> bool {
    if BUILDING.get() {
        // An engine is starting on that folder right now.
        return false;
    }
    crate::executor::storage::remove_browser_profile()
        .inspect_err(|error| eprintln!("[vela-wallet] erase: browser profile kept: {error}"))
        .is_ok()
}

/// macOS: WKWebView's default store, reachable only through a live view.
#[cfg(not(windows))]
fn clear_profile_on_disk() -> bool {
    false
}

/// The document the browser is on, whole. `None` before the first page.
#[must_use]
pub fn current_url() -> Option<String> {
    BROWSER.with(|slot| slot.borrow().as_ref()?.view.url().ok())
}

/// The host the toolbar shows.
///
/// From the WEBVIEW, so the lock and the name beside it describe the page that
/// is actually loaded — a label fed from anywhere else is a claim about an
/// origin, which is the one thing a browser chrome must never get wrong.
/// `None` before the first page, and the caller keeps drawing what the mock
/// draws rather than an empty bar.
///
/// The COMMITTED document's, since spec 079: the webview's own URL moves to
/// the next site the moment a navigation starts, so on a slow network the bar
/// named a site that had not loaded — and, on a failure, never would.
#[must_use]
pub fn host() -> Option<String> {
    // Only a WEB document counts as committed here: WKWebView commits an
    // empty `about:blank` when a fresh view's first load is refused.
    let url = committed_url()
        .filter(|url| vela_core::app::dapp_permissions::origin_of(url).is_some())
        .or_else(|| BROWSER.with(|slot| slot.borrow().as_ref()?.view.url().ok()))?;
    let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(&url);
    let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
    (!host.is_empty()).then(|| host.to_owned())
}

/// Hand one message from the core to the page in `tab`.
///
/// There is one webview, so there is one tab; a message for any other is
/// dropped — there is no "front tab" to fall back to, which is the rule that
/// keeps an answer out of a page that did not ask. The bridge drops it too
/// unless it names the document that is loaded now.
pub fn deliver(tab: &str, message_json: &str) {
    if tab != crate::wallet::browser_host::BROWSER_TAB {
        return;
    }
    with_view(|view| {
        // A JSON string literal is a JavaScript string literal: the message
        // reaches `__velaDeliver` as the exact bytes the core wrote.
        let script = format!(
            "window.__velaDeliver({})",
            serde_json::to_string(message_json).unwrap_or_else(|_| "\"{}\"".to_owned())
        );
        let _ = view.evaluate_script(&script);
    });
}

fn with_view(act: impl FnOnce(&wry::WebView)) {
    BROWSER.with(|slot| {
        if let Some(browser) = slot.borrow().as_ref() {
            act(&browser.view);
        }
    });
}

thread_local! {
    /// A WebView2 is being created (Windows): do not start a second.
    #[cfg(windows)]
    static BUILDING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// WebView2's browser process exited (spec 083): this view can never load
    /// again, and Reload builds a new one.
    #[cfg(windows)]
    static ENGINE_GONE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Build the browser OUTSIDE gpui's update (Windows).
///
/// WebView2's creation is synchronous and runs a nested message loop while it
/// waits for the browser process. Called from the paint pass — where
/// `place` runs, with gpui's `App` borrowed — that loop delivered a message
/// gpui answered by borrowing `App` again: "RefCell already borrowed", and the
/// process aborted the moment a page was opened. A foreground task's body runs
/// with nothing borrowed, so the nested loop is harmless there; the frame after
/// it lands places the view.
#[cfg(windows)]
fn build_later(window: &Window, home: &str, cx: &mut gpui::App) {
    if BUILDING.get() {
        return;
    }
    let hwnd = crate::onboarding::native_window_handle(window);
    if hwnd == 0 {
        return;
    }
    BUILDING.set(true);
    let home = home.to_owned();
    cx.spawn(async move |cx| {
        settle(build(&ParentHwnd(hwnd), &home));
        BUILDING.set(false);
        // Paint again, so `place` sizes and shows what was just built.
        cx.update(|cx| cx.refresh_windows());
    })
    .detach();
}

/// The wallet's window, as wry takes a parent (Windows).
#[cfg(windows)]
struct ParentHwnd(isize);

#[cfg(windows)]
impl wry::raw_window_handle::HasWindowHandle for ParentHwnd {
    fn window_handle(
        &self,
    ) -> Result<wry::raw_window_handle::WindowHandle<'_>, wry::raw_window_handle::HandleError> {
        let hwnd = std::num::NonZeroIsize::new(self.0)
            .ok_or(wry::raw_window_handle::HandleError::Unavailable)?;
        let raw = wry::raw_window_handle::RawWindowHandle::Win32(
            wry::raw_window_handle::Win32WindowHandle::new(hwnd),
        );
        // SAFETY: the wallet's own top-level window, alive for as long as the
        // app runs; the handle is only borrowed for the build call.
        Ok(unsafe { wry::raw_window_handle::WindowHandle::borrow_raw(raw) })
    }
}

fn build<W: wry::raw_window_handle::HasWindowHandle>(
    window: &W,
    home: &str,
) -> Result<Browser, EngineFailure> {
    // Spec 083 W1: WebView2 given no folder made its profile beside the exe —
    // `C:\Program Files\Vela Wallet` once installed, which the person cannot
    // write, so the installed browser never started. The person's own local
    // app data instead (or the isolated state dir).
    #[cfg(windows)]
    let profile = crate::executor::storage::browser_profile_dir();
    #[cfg(windows)]
    if let Some(dir) = &profile
        // Made if missing, so a missing parent is never why WebView2 refuses;
        // and written once, because a folder it may not write is not refused
        // (083 H7): the engine waited on it forever, blank, with no panel.
        && let Err(error) = crate::executor::storage::probe_profile_dir(dir)
    {
        eprintln!("[vela-wallet] browser: the profile folder takes no write: {error}");
        if let Some(failure) = EngineFailure::from_profile_probe(&error) {
            return Err(failure);
        }
    }
    #[cfg(windows)]
    let mut context = wry::WebContext::new(profile);
    // Hidden from creation: a failed build leaks wry's container window, and a
    // visible one would sit over the wallet (083 W1b).
    #[cfg(windows)]
    let builder = wry::WebViewBuilder::new_with_web_context(&mut context).with_visible(false);
    #[cfg(not(windows))]
    let builder = wry::WebViewBuilder::new();
    let builder = builder
        .with_initialization_script(provider_script(ProviderHost::Desktop))
        .with_initialization_script(META_JS)
        .with_ipc_handler(on_ipc)
        // 083: see `allow_navigation`.
        .with_navigation_handler(allow_navigation)
        // 083: no download on a desktop, as on the phones — wry's default
        // saved every one silently into Downloads, on Windows and macOS.
        .with_download_started_handler(|_, _| false)
        .with_url(home);
    // Windows hears its loads from WebView2 directly (spec 083): wry's handler
    // turns the engine's error page into a commit.
    #[cfg(not(windows))]
    let builder = builder.with_on_page_load_handler(|event, url| {
        report(match event {
            wry::PageLoadEvent::Started => Load::Started(url),
            wry::PageLoadEvent::Finished => Load::Finished(url),
        });
    });
    #[cfg(target_os = "macos")]
    let builder = {
        use wry::WebViewBuilderExtDarwin as _;
        builder
            .with_on_web_content_process_terminate_handler(|| report(Load::Crashed))
            // 083 W15: WKWebView has no word on a page's own entries.
            .with_initialization_script(HISTORY_JS)
    };
    match builder.build_as_child(window) {
        Ok(view) => {
            #[cfg(windows)]
            if let Err(error) = listen_to_webview2(&view) {
                eprintln!("[vela-wallet] browser: WebView2 events: {error}");
            }
            #[cfg(windows)]
            if let Err(error) = listen_for_leaves(&view) {
                eprintln!("[vela-wallet] browser: WebView2 new windows: {error}");
            }
            // Built hidden: `place` turns it on in the same frame, and a
            // webview that flashed into the wallet before its first layout
            // would be visible for exactly one frame in the wrong place.
            let _ = view.set_visible(false);
            // The first page is a load the wallet asked for, like any other.
            report(Load::Requested(home.to_owned()));
            Ok(Browser {
                view,
                bounds: None,
                visible: false,
                #[cfg(windows)]
                _context: context,
                #[cfg(windows)]
                hole: None,
            })
        }
        Err(error) => {
            let failure = EngineFailure::from_hresult(hresult_of(&error));
            // Once per attempt, and attempts are the person's (083 W1b).
            eprintln!("[vela-wallet] browser: the engine did not start ({failure:?}): {error}");
            Err(failure)
        }
    }
}

/// WebView2's load, crash and certificate events, into the wallet's `Load`
/// (spec 083).
#[cfg(windows)]
fn listen_to_webview2(view: &wry::WebView) -> windows_core::Result<()> {
    use crate::webview2_events::EngineLoad;
    use wry::WebViewExtWindows as _;
    crate::webview2_events::subscribe(&view.webview(), |event| {
        report(match event {
            EngineLoad::Committed(url) => Load::Started(url),
            EngineLoad::ErrorPage(url) => {
                // Off the screen this turn, before Edge's page paints a frame;
                // the render that follows keeps it there (083 W3).
                hide_if_free();
                Load::ErrorPage(url)
            }
            EngineLoad::Finished(url) => Load::Finished(url),
            EngineLoad::Failed {
                url,
                status,
                certificate,
            } => {
                eprintln!(
                    "[vela-wallet] browser: load failed, WebView2 status {status}{}",
                    if certificate { " (certificate)" } else { "" }
                );
                Load::Failed {
                    url,
                    status,
                    certificate,
                }
            }
            EngineLoad::RendererGone => Load::Crashed,
            EngineLoad::BrowserGone => {
                ENGINE_GONE.set(true);
                Load::Crashed
            }
            EngineLoad::HistoryChanged => Load::HistoryChanged,
        });
    })
}

/// WebView2's new windows and launches of other apps, through the one scheme
/// rule (083 W6/W7). wry answered a new window with no window — `window.open`
/// and `target=_blank` did nothing — and never heard the engine hand
/// `mailto:` to Windows' app picker.
#[cfg(windows)]
fn listen_for_leaves(view: &wry::WebView) -> windows_core::Result<()> {
    use wry::WebViewExtWindows as _;
    let webview = view.webview();
    // Other apps first: if it fails, nothing may reach one, and the guard
    // stays off so wry's navigation handler refuses them all.
    let guarded = crate::webview2_events::subscribe_other_apps(&webview, engine_leave);
    EXTERNAL_GUARD.set(guarded.as_ref().is_ok_and(|guarded| *guarded));
    let new_windows = crate::webview2_events::subscribe_new_windows(&webview, engine_leave);
    guarded.map(drop).and(new_windows)
}

/// One of WebView2's leaves, through the one rule (083 W6/W7).
#[cfg(windows)]
fn engine_leave(leave: crate::webview2_events::EngineLeave) {
    use crate::webview2_events::EngineLeave;
    let leave = match leave {
        EngineLeave::NewWindow {
            uri,
            gesture,
            from,
            top,
        } => new_window_leave(&uri, gesture, asked_by_page(from.as_deref(), &top)),
        // Already cancelled: only a person's mailto:/tel:, from the page, goes on.
        EngineLeave::OtherApp {
            uri,
            gesture,
            from,
            top,
        } => other_app_leave(&uri, gesture, asked_by_page(from.as_deref(), &top)),
    };
    if let Some(leave) = leave {
        leave_to_page(leave);
    }
}

/// [`hide`], unless the view is borrowed right now (an engine event arriving
/// inside a call on it): the next render hides it then.
#[cfg(windows)]
fn hide_if_free() {
    BROWSER.with(|slot| {
        if let Ok(mut slot) = slot.try_borrow_mut()
            && let Some(browser) = slot.as_mut()
            && browser.visible
        {
            let _ = browser.view.set_visible(false);
            browser.visible = false;
        }
    });
}

/// The platform's code inside a failed build: tells a missing runtime from a
/// refused profile folder (spec 083).
#[cfg(windows)]
fn hresult_of(error: &wry::Error) -> Option<i32> {
    match error {
        wry::Error::WebView2Error(webview2_com::Error::WindowsError(error)) => Some(error.code().0),
        _ => None,
    }
}

#[cfg(not(windows))]
fn hresult_of(_: &wry::Error) -> Option<i32> {
    None
}

fn report(load: Load) {
    if let Load::Started(url) = &load {
        let origin = vela_core::app::dapp_permissions::origin_of(url);
        COMMITTED.with(|slot| *slot.borrow_mut() = Some(origin));
        COMMITTED_URL.with(|slot| *slot.borrow_mut() = Some(url.clone()));
    }
    LOADS.with(|slot| {
        if let Some(sink) = slot.borrow().as_ref() {
            sink(load);
        }
    });
}

/// One string from a page.
///
/// The URL comes from the PLATFORM's record of the sender, never from the
/// string; the core reads everything else. The sink is always there first:
/// the page installs it in the same frame that builds this webview.
fn on_ipc(request: wry::http::Request<String>) {
    let body = request.body();
    let parsed = serde_json::from_str::<serde_json::Value>(body).ok();
    let vela = parsed
        .as_ref()
        .and_then(|parsed| parsed.get("vela"))
        .and_then(|v| v.as_str())
        .map(str::to_owned);
    // Spec 083 W15: the top document added or moved through an entry of its
    // own. Only the arrows hear it, and only the top document's counts: on
    // macOS any frame can reach this channel.
    let history = match vela.as_deref() {
        Some("pushed") => Some(Load::Pushed),
        Some("popped") => Some(Load::Popped),
        _ => None,
    };
    if let Some(history) = history {
        let sender = sender_url(&request.uri().to_string(), current_url);
        if top_frame_sent(&sender, current_url) {
            report(history);
        }
        return;
    }
    // The meta report is the one message that is not the provider's, and it
    // is display text: it never reaches the core.
    if let Some(parsed) = parsed.filter(|_| vela.as_deref() == Some("meta")) {
        let text = |key: &str| {
            parsed
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let sender = sender_url(&request.uri().to_string(), current_url);
        let meta = PageMeta {
            url: meta_url(&text("href"), &sender),
            title: text("title"),
            favicon: text("favicon"),
            status: parsed
                .get("status")
                .and_then(serde_json::Value::as_u64)
                .and_then(|status| u16::try_from(status).ok())
                .filter(|status| *status > 0),
        };
        META.with(|slot| {
            if let Some(sink) = slot.borrow().as_ref() {
                sink(meta);
            }
        });
        return;
    }
    let sender = sender_url(&request.uri().to_string(), current_url);
    let is_main_frame = top_frame_sent(&sender, current_url);
    MESSAGES.with(|slot| {
        if let Some(sink) = slot.borrow().as_ref() {
            sink(PageMessage {
                sender,
                is_main_frame,
                message_json: body.clone(),
            });
        }
    });
}

/// The address a meta report is filed under: the document's own
/// `location.href` — the one document the title and icon came from — when it
/// is the sender's origin, which is the platform's word and cannot be forged;
/// the sender's URL when it is not. A page cannot put itself under another
/// site's name in Recents.
fn meta_url(href: &str, sender: &str) -> String {
    let origin_of = vela_core::app::dapp_permissions::origin_of;
    match origin_of(href) {
        Some(origin) if Some(&origin) == origin_of(sender).as_ref() => href.to_owned(),
        _ => sender.to_owned(),
    }
}

/// Was this posted by the top document?
///
/// wry names no frames. What it does give is the sender's URL, and on macOS
/// WebKit hands the IPC channel to EVERY frame (`webkit.messageHandlers` is
/// not main-frame-only, even where wry's `window.ipc` and the core's script
/// are), so a cross-origin iframe — an ad — can post a hello of its own and
/// retire the page it sits in. The top document is the one whose origin last
/// committed (`didCommitNavigation`, which WebKit reports before any script of
/// the new document runs); a sender from any other origin is a subframe, and
/// the core ignores it. A same-origin frame passes — it can reach the top
/// page's own globals anyway, so there is no boundary between them to keep.
/// The webview's own URL counts too, for a page restored from the
/// back-forward cache, which is the top document without a new commit.
///
/// Elsewhere this is true by construction: WebView2 delivers only the top
/// document's messages to this handler, and its load-start event fires
/// before the commit, so the same comparison there would drop the old page's
/// last requests.
fn top_frame_sent(sender: &str, webview: impl FnOnce() -> Option<String>) -> bool {
    if !cfg!(target_os = "macos") {
        return true;
    }
    let origin_of = vela_core::app::dapp_permissions::origin_of;
    let sent_from = origin_of(sender);
    COMMITTED.with(|slot| match slot.borrow().as_ref() {
        None => true,
        Some(top) => sent_from == *top || sent_from == webview().as_deref().and_then(origin_of),
    })
}

/// The URL of the document that posted, as the platform reported it.
///
/// wry fills the request's URI from the sending frame (`WKScriptMessage.
/// frameInfo` on macOS, the message's `Source` on Windows), which is exactly
/// the fact the core needs: an old document still posting while the webview
/// is already loading the next site keeps its own origin, and a subframe that
/// reaches the IPC channel directly is named by its own origin rather than by
/// the page around it. The webview's URL is only a fallback for a platform
/// that reports none.
fn sender_url(reported: &str, webview: impl FnOnce() -> Option<String>) -> String {
    if reported.is_empty() || reported == "/" {
        return webview().unwrap_or_default();
    }
    reported.to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Spec 083 D2: the hole is the menu's part over the page, in the page
    /// window's pixels, rounded outward; nothing when they do not meet.
    #[test]
    fn a_menu_over_the_page_cuts_its_own_rectangle() {
        use gpui::{point, px, size};
        let page = Bounds::new(point(px(300.), px(92.)), size(px(900.), px(700.)));
        let menu = Bounds::new(point(px(1000.), px(72.)), size(px(180.), px(240.)));
        assert_eq!(hole_in(page, menu, 1.75), Some([1225, 0, 1540, 385]));
        let beside = Bounds::new(point(px(10.), px(10.)), size(px(100.), px(50.)));
        assert_eq!(hole_in(page, beside, 1.75), None);
    }

    /// The platform's word for who sent a message wins; the webview's URL is
    /// asked only when there is no such word.
    #[test]
    fn the_sender_is_the_platforms_report_of_the_frame() {
        let webview = || Some("https://next.example/".to_owned());
        assert_eq!(
            sender_url("https://old.example/swap", webview),
            "https://old.example/swap",
            "a document being navigated away keeps its own name"
        );
        assert_eq!(sender_url("", webview), "https://next.example/");
        assert_eq!(sender_url("/", webview), "https://next.example/");
        assert_eq!(sender_url("", || None), "");
    }

    /// An iframe of another origin reaching the IPC channel directly is not
    /// the top document — on macOS, where WebKit lets it reach the channel.
    #[test]
    fn a_frame_of_another_origin_is_not_the_top_document() {
        // Before the first commit nothing is known, and nothing is refused.
        assert!(top_frame_sent("https://ads.example/frame", || None));
        report(Load::Started("https://dapp.example/app".to_owned()));
        assert!(top_frame_sent("https://dapp.example/other", || None));
        assert_eq!(
            top_frame_sent("https://ads.example/frame", || None),
            !cfg!(target_os = "macos"),
            "a cross-origin frame is not the page it sits in"
        );
        // A page restored from the back-forward cache is the top document
        // with no new commit: the webview's own URL names it.
        assert!(top_frame_sent("https://back.example/", || Some(
            "https://back.example/#top".to_owned()
        )));
    }

    /// Spec 079 R3: the visit's address is the document's own, read in the
    /// same script as its title — but never another origin than the frame
    /// the platform says sent it.
    #[test]
    fn a_visit_is_filed_under_the_document_that_reported_it() {
        assert_eq!(
            meta_url("https://app.example/swap?x=1", "https://app.example/"),
            "https://app.example/swap?x=1",
            "a route the page moved to is the page's own address"
        );
        assert_eq!(
            meta_url("https://bank.example/", "https://evil.example/"),
            "https://evil.example/",
            "a page cannot file itself under another site"
        );
        assert_eq!(meta_url("", "https://app.example/"), "https://app.example/");
        assert!(META_JS.contains("location.href") && META_JS.contains("responseStatus"));
    }

    /// 083: one rule for where an address goes — pages load, only mail and
    /// phone leave, and nothing else is opened by anything.
    #[test]
    fn only_pages_load_and_only_mail_and_phone_leave() {
        EXTERNAL_GUARD.set(false);
        for url in [
            "https://a.example/",
            "http://127.0.0.1:8080/",
            "about:blank",
            "about:srcdoc",
            "blob:https://a.example/1",
            "data:text/html,x",
        ] {
            assert!(allow_navigation(url.to_owned()), "{url}");
        }
        assert_eq!(scheme_of("mailto:a@b.example"), Scheme::External);
        assert_eq!(scheme_of("TEL:+15551234"), Scheme::External);
        for url in [
            "wc:x@2",
            "metamask://dapp/x",
            "intent://x#Intent;end",
            "file:///C:/x",
            "javascript:alert(1)",
            "ms-msdt:/id",
            "search-ms:q",
            "",
            "no-scheme",
        ] {
            assert_eq!(scheme_of(url), Scheme::Refused, "{url}");
            assert!(!allow_navigation(url.to_owned()), "{url}");
        }
    }

    /// 083 W7: the navigation handler never knows whether a person tapped,
    /// so it never launches anything. `mailto:`/`tel:` go on to Windows'
    /// launch event where it is in; everywhere else they are refused.
    #[test]
    fn the_navigation_handler_launches_nothing() {
        let heard = std::rc::Rc::new(std::cell::Cell::new(0));
        let count = heard.clone();
        on_leave_to(Box::new(move |_| count.set(count.get() + 1)));
        for url in ["mailto:a@b.example", "tel:+15551234", "TEL:1\" --x"] {
            EXTERNAL_GUARD.set(false);
            assert!(
                !allow_navigation(url.to_owned()),
                "{url}: macOS, an old runtime"
            );
            EXTERNAL_GUARD.set(true);
            assert!(
                allow_navigation(url.to_owned()),
                "{url}: on to WebView2's launch event"
            );
        }
        EXTERNAL_GUARD.set(true);
        assert!(
            !allow_navigation("ms-settings:".to_owned()),
            "the guard is for mail and phone only"
        );
        EXTERNAL_GUARD.set(false);
        assert_eq!(heard.get(), 0, "no sink hears anything from this handler");
        LEAVES.with(|slot| slot.borrow_mut().take());
    }

    /// 083 W6: a new window opens a tab only on a person's gesture.
    #[test]
    fn a_new_window_is_a_tab_only_on_a_gesture() {
        let tx = "https://etherscan.io/tx/0x1";
        assert_eq!(
            new_window_leave(tx, true, true),
            Some(Leave::NewTab(tx.to_owned()))
        );
        assert_eq!(
            new_window_leave(tx, true, false),
            Some(Leave::NewTab(tx.to_owned())),
            "a frame's link opens a tab, as in any browser"
        );
        assert_eq!(new_window_leave(tx, false, true), None, "a timer's popup");
        assert_eq!(
            new_window_leave("mailto:a@b.example", true, true),
            Some(Leave::External("mailto:a@b.example".to_owned()))
        );
        assert_eq!(new_window_leave("mailto:a@b.example", false, true), None);
        assert_eq!(
            new_window_leave("mailto:a@b.example", true, false),
            None,
            "another app only from the page itself"
        );
        assert_eq!(new_window_leave("about:blank", true, true), None);
        assert_eq!(new_window_leave("metamask://x", true, true), None);
    }

    /// 083 W7: what reaches another app — a person's tap, in the page's own
    /// document, on mail or phone, escaped as the engine would have.
    #[test]
    fn another_app_opens_only_on_a_tap_in_the_page_itself() {
        let page = "https://dapp.example/swap";
        let mail = "mailto:a@b.example";
        let from_page = |from: &str| asked_by_page(Some(from), page);
        assert!(from_page("https://dapp.example"), "WebView2's origin form");
        assert!(from_page("HTTPS://DAPP.EXAMPLE/other"), "a frame's address");
        assert!(!from_page("https://ads.example"), "a cross-origin frame");
        assert!(
            !from_page(""),
            "the host's own navigation, which Vela never makes"
        );
        assert!(!asked_by_page(None, page), "a runtime that does not say");
        assert!(!asked_by_page(Some("https://dapp.example"), "about:blank"));

        assert_eq!(
            other_app_leave(mail, true, true),
            Some(Leave::External(mail.to_owned()))
        );
        assert_eq!(other_app_leave(mail, false, true), None, "no tap");
        assert_eq!(other_app_leave(mail, true, false), None, "an ad frame");
        for refused in [
            "ms-settings:",
            "wc:x@2",
            "metamask://x",
            "https://a.example/",
        ] {
            assert_eq!(other_app_leave(refused, true, true), None, "{refused}");
        }
        assert_eq!(
            other_app_leave("tel:1\" --x", true, true),
            Some(Leave::External("tel:1%22%20--x".to_owned())),
            "never the page's quote on a command line"
        );
        let long = format!("mailto:a@b.example?body={}", "x".repeat(2048));
        assert_eq!(other_app_leave(&long, true, true), None);
    }

    /// The injected provider is the core's, for THIS host: it posts through
    /// wry's IPC channel and does nothing outside the top frame.
    #[test]
    fn the_injected_script_is_the_cores_desktop_script() {
        let script = provider_script(ProviderHost::Desktop);
        assert!(script.contains("window.ipc.postMessage"));
        assert!(script.contains("window.top !== window"));
        assert!(script.contains("__velaDeliver"));
        // The meta report is plumbing, not a second provider.
        assert!(!META_JS.contains("ethereum"));
        assert!(META_JS.contains("window.top !== window"));
        assert!(
            !META_JS.contains("pushState"),
            "083: history is its own script"
        );
    }

    /// 083 W15: a page's report of its own entries reaches the arrows, never
    /// the core, and on macOS only from the top document's origin.
    #[test]
    fn a_pages_own_history_reaches_only_the_arrows() {
        use std::cell::RefCell;
        use std::rc::Rc;
        let heard = Rc::new(RefCell::new(Vec::new()));
        let loads = heard.clone();
        on_load_to(Box::new(move |load| match load {
            Load::Pushed => loads.borrow_mut().push("pushed"),
            Load::Popped => loads.borrow_mut().push("popped"),
            _ => {}
        }));
        let messages = Rc::new(std::cell::Cell::new(0));
        let count = messages.clone();
        on_message_to(Box::new(move |_| count.set(count.get() + 1)));
        report(Load::Started("https://dapp.example/app".to_owned()));
        let ipc = |uri: &str, body: &str| {
            on_ipc(
                wry::http::Request::builder()
                    .uri(uri)
                    .body(body.to_owned())
                    .expect("a request"),
            );
        };
        ipc("https://dapp.example/app", r#"{"vela":"pushed"}"#);
        ipc("https://dapp.example/app#faq", r#"{"vela":"popped"}"#);
        ipc("https://ads.example/frame", r#"{"vela":"pushed"}"#);
        let expected: &[&str] = if cfg!(target_os = "macos") {
            &["pushed", "popped"]
        } else {
            &["pushed", "popped", "pushed"]
        };
        assert_eq!(heard.borrow().as_slice(), expected);
        assert_eq!(messages.get(), 0, "never a provider message");
        LOADS.with(|slot| slot.borrow_mut().take());
        MESSAGES.with(|slot| slot.borrow_mut().take());
    }

    /// 083 W15 (macOS): the page's own entries are reported from the top
    /// document only, after the page's `pushState` has run, and a report that
    /// fails never breaks the page's call.
    #[test]
    fn the_history_script_reports_the_pages_own_entries() {
        assert!(HISTORY_JS.contains("window.top !== window"));
        assert!(!HISTORY_JS.contains("ethereum"));
        let push = HISTORY_JS
            .find("push.apply(this, arguments)")
            .unwrap_or(usize::MAX);
        let report = HISTORY_JS.find("post('pushed')").unwrap_or(0);
        assert!(push < report, "the page's push first");
        assert!(HISTORY_JS.contains("try {") && HISTORY_JS.contains("catch (_)"));
        assert!(HISTORY_JS.contains("'popstate'") && HISTORY_JS.contains("'popped'"));
        assert!(HISTORY_JS.contains("'pageshow'"));
    }
}
