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
    /// A new document committed (`didCommitNavigation` on macOS,
    /// `NavigationStarting` on Windows). Settles nothing by itself — the new
    /// document's hello is what retires the old one.
    Started(String),
    /// The load finished. If no document said hello since it started (an
    /// error page, a PDF), the old one is gone and its requests are settled.
    Finished(String),
    /// The web content process died (macOS only — wry reports no such thing
    /// for WebView2). The page is blank and can never answer again.
    Crashed,
}

type LoadSink = Box<dyn Fn(Load)>;

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
}

thread_local! {
    static BROWSER: RefCell<Option<Browser>> = const { RefCell::new(None) };
}

/// Draw the browser at `bounds`, building it on first call.
///
/// Called from the paint pass of the element that OWNS that rectangle, so the
/// webview follows the column through window resizes and through a third
/// column opening beside it — the signing panel included.
pub fn place(bounds: Bounds<Pixels>, window: &Window, home: &str, cx: &mut gpui::App) {
    #[cfg(windows)]
    if BROWSER.with(|slot| slot.borrow().is_none()) {
        build_later(window, home, cx);
        return;
    }
    #[cfg(not(windows))]
    let _ = cx;
    BROWSER.with(|slot| {
        let mut slot = slot.borrow_mut();
        #[cfg(not(windows))]
        if slot.is_none() {
            *slot = build(window, home).map(|view| Browser {
                view,
                bounds: None,
                visible: false,
            });
        }
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

/// Back and forward: the engine's own (spec 082 RD6, wry's `go_back` /
/// `go_forward`), not a script in the page — a page that overrides
/// `history.back` cannot keep the person on it, and a page that has not
/// loaded (a failed first load) has no script to run one.
pub fn back() {
    with_view(|view| {
        let _ = view.go_back();
    });
}

pub fn forward() {
    with_view(|view| {
        let _ = view.go_forward();
    });
}

/// Whether the one webview has been built yet — for a log line.
#[must_use]
pub fn built() -> bool {
    BROWSER.with(|slot| slot.borrow().is_some())
}

/// What the engine says about itself (spec 082 RD3, RD6).
pub struct Engine {
    /// Its load state — `None` where it cannot be read (WebView2: the
    /// Windows build stays probe-only).
    pub sample: Option<vela_core::app::browser_load::EngineSample>,
    pub can_back: bool,
    pub can_forward: bool,
}

/// One poll of the engine. `None` before the webview is built.
#[must_use]
pub fn engine() -> Option<Engine> {
    BROWSER.with(|slot| {
        let slot = slot.borrow();
        let view = &slot.as_ref()?.view;
        Some(Engine {
            sample: engine_sample(view),
            can_back: view.can_go_back().unwrap_or(false),
            can_forward: view.can_go_forward().unwrap_or(false),
        })
    })
}

/// WKWebView's `isLoading`, `estimatedProgress` and `URL` — the facts wry's
/// callbacks leave out (it reports no `didFail*`). Read through the same
/// pointer [`view_url`] uses, on the main thread, where the view lives.
#[cfg(target_os = "macos")]
fn engine_sample(view: &wry::WebView) -> Option<vela_core::app::browser_load::EngineSample> {
    use wry::WebViewExtMacOS as _;
    let webview = view.webview();
    let raw = std::ptr::from_ref(&*webview)
        .cast::<objc2::runtime::AnyObject>()
        .cast_mut();
    // SAFETY: `raw` is the live WKWebView wry owns for as long as `view`;
    // both are property getters with no arguments.
    let (loading, progress): (objc2::runtime::Bool, f64) = unsafe {
        (
            objc2::msg_send![raw, isLoading],
            objc2::msg_send![raw, estimatedProgress],
        )
    };
    Some(vela_core::app::browser_load::EngineSample {
        loading: loading.as_bool(),
        progress,
        url: view_url(view),
    })
}

#[cfg(not(target_os = "macos"))]
fn engine_sample(_view: &wry::WebView) -> Option<vela_core::app::browser_load::EngineSample> {
    None
}

pub fn reload() {
    let mut asked = false;
    with_view(|view| {
        asked = view.reload().is_ok();
    });
    if asked && let Some(url) = committed_url().or_else(current_url) {
        report(Load::Requested(url));
    }
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
    BROWSER.with(|slot| {
        slot.borrow()
            .as_ref()
            .is_some_and(|browser| browser.view.clear_all_browsing_data().is_ok())
    })
}

/// The document the browser is on, whole. `None` before the first page.
#[must_use]
pub fn current_url() -> Option<String> {
    BROWSER.with(|slot| view_url(&slot.borrow().as_ref()?.view))
}

/// The webview's URL, or `None` while it has none.
///
/// `wry::WebView::url` unwraps WKWebView's `URL`, which is nil while a fresh
/// view's first navigation has not committed — a refused CONNECT through a
/// proxy leaves it nil, with no `about:blank` either. Reading it then panicked
/// inside a draw, which AppKit cannot unwind, and the app aborted on the first
/// failed page of a new tab (082 G29). So WebKit is asked first.
fn view_url(view: &wry::WebView) -> Option<String> {
    #[cfg(target_os = "macos")]
    {
        use wry::WebViewExtMacOS as _;
        let webview = view.webview();
        // wry speaks objc2 0.6 and this crate 0.5, so the object crosses as a
        // plain pointer; only whether `URL` is nil is read through it.
        let raw = std::ptr::from_ref(&*webview)
            .cast::<objc2::runtime::AnyObject>()
            .cast_mut();
        // SAFETY: `raw` is the live WKWebView wry owns for as long as `view`;
        // `URL` takes no arguments and returns an NSURL or nil.
        let url: *mut objc2::runtime::AnyObject = unsafe { objc2::msg_send![raw, URL] };
        if url.is_null() {
            return None;
        }
    }
    view.url().ok()
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
        .or_else(|| BROWSER.with(|slot| view_url(&slot.borrow().as_ref()?.view)))?;
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
        let built = build(&ParentHwnd(hwnd), &home);
        BROWSER.with(|slot| {
            *slot.borrow_mut() = built.map(|view| Browser {
                view,
                bounds: None,
                visible: false,
            });
        });
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
) -> Option<wry::WebView> {
    let builder = wry::WebViewBuilder::new()
        .with_initialization_script(provider_script(ProviderHost::Desktop))
        .with_initialization_script(META_JS)
        .with_ipc_handler(on_ipc)
        .with_on_page_load_handler(|event, url| {
            report(match event {
                wry::PageLoadEvent::Started => Load::Started(url),
                wry::PageLoadEvent::Finished => Load::Finished(url),
            });
        })
        .with_url(home);
    #[cfg(target_os = "macos")]
    let builder = {
        use wry::WebViewBuilderExtDarwin as _;
        builder.with_on_web_content_process_terminate_handler(|| report(Load::Crashed))
    };
    // Spec 082: a dev build's fault proxy carries the page too. It is set on
    // this webview's own data store, not the system, so nothing else on the
    // machine is pointed at it. macOS 14+, through wry's `mac-proxy`, which
    // only `dev-fixtures` turns on (Cargo.toml).
    #[cfg(feature = "dev-fixtures")]
    let builder = match crate::executor::proxy::dev_proxy() {
        Some(proxy) => builder.with_proxy_config(wry::ProxyConfig::Http(wry::ProxyEndpoint {
            host: proxy.host().to_owned(),
            port: proxy.port().to_string(),
        })),
        None => builder,
    };
    match builder.build_as_child(window) {
        Ok(view) => {
            // Built hidden: `place` turns it on in the same frame, and a
            // webview that flashed into the wallet before its first layout
            // would be visible for exactly one frame in the wrong place.
            let _ = view.set_visible(false);
            // The first page is a load the wallet asked for, like any other.
            report(Load::Requested(home.to_owned()));
            Some(view)
        }
        Err(error) => {
            crate::diag::vlog!("browser", "the webview could not be built: {error}");
            None
        }
    }
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
    // The meta report is the one message that is not the provider's, and it
    // is display text: it never reaches the core.
    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(body)
        && parsed.get("vela").and_then(|v| v.as_str()) == Some("meta")
    {
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
    }
}
