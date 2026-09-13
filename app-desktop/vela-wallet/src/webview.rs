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
//! ## Why the origin comes from here and not from the page
//!
//! The extension's content script (spec 027) exists to carry messages plus
//! "exactly two facts the page cannot forge: WHICH tab a request came from,
//! and WHICH origin sent it". This module is that boundary for the desktop,
//! and keeps the same rule: the origin is read from the WEBVIEW's current URL
//! on this side, and a page that puts an `origin` in its own envelope is
//! ignored.

use std::cell::RefCell;

use gpui::{Bounds, Pixels, Window};

use vela_core::app::dapp_permissions::{DpermPageEvent, DpermRejectReason, DpermRespondPayload};

/// The provider, verbatim from the extension (spec 027). Injected into every
/// page rather than reimplemented, because a second EIP-1193 implementation is
/// a second set of bugs — and this one has an extension's worth of use behind
/// it.
const INPAGE_JS: &str = include_str!("../../../app-web/vela-wallet/extension/inpage.js");
/// The provider's own constants — `inpage.js` imports these.
const PROTOCOL_JS: &str = include_str!("../../../app-web/vela-wallet/extension/lib/protocol.js");

/// The provider as ONE classic script.
///
/// `inpage.js` is an ES module: it opens with
/// `import { CHANNEL, … } from './lib/protocol.js'`. An initialization script
/// is classic, so injecting it verbatim is a syntax error — the file never
/// runs, `window.ethereum` never appears, and **every request silently never
/// happens**. That is exactly how it failed the first time this was wired,
/// and nothing said so: no error reached the host, the page simply had no
/// wallet in it.
///
/// Concatenating is the CSP-proof fix. Serving the module over a custom
/// protocol and pulling it in with a dynamic `import()` would keep both files
/// untouched, but a dApp with a strict Content-Security-Policy can refuse
/// that, and a provider that works on some sites and not others is worse than
/// one that works everywhere.
///
/// Both files stay byte-identical on disk; the two module keywords are removed
/// HERE, and the test below fails if either file grows another one.
fn provider_script() -> String {
    let constants: String = PROTOCOL_JS
        .lines()
        .map(|line| line.strip_prefix("export ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    let provider: String = INPAGE_JS
        .lines()
        .filter(|line| !line.trim_start().starts_with("import "))
        .collect::<Vec<_>>()
        .join("\n");
    // One scope, so the constants reach the provider and nothing on the page.
    format!("(() => {{\n{constants}\n{provider}\n}})();")
}

/// The content script's half, in eleven lines.
///
/// The extension puts `inpage.js` in the MAIN world and talks to it from an
/// isolated one over `window.postMessage`; wry has no isolated world, so this
/// forwards the same envelopes to the host over `window.ipc` and delivers
/// answers back the way the provider already expects them. The provider itself
/// is untouched — it cannot tell the difference, which is the point.
const BRIDGE_JS: &str = r#"
(() => {
  const CHANNEL = 'vela-1193';
  window.addEventListener('message', (ev) => {
    if (ev.source !== window) return;
    const d = ev.data;
    if (!d || d.ch !== CHANNEL || d.dir !== 'req') return;
    // The host adds the origin. Anything this envelope claims about who it is
    // would be the page describing itself.
    window.ipc.postMessage(JSON.stringify({ id: d.id, method: d.method, params: d.params }));
  });
  window.__velaDeliver = (json) => {
    const m = JSON.parse(json);
    window.postMessage({ ch: CHANNEL, ...m }, window.location.origin);
  };
  // What the page calls itself. The phone shells get this from the native
  // WebView's own title/favicon callbacks; wry has none, so the bridge reports
  // it — twice, because a title is usually there at DOMContentLoaded and an
  // icon link often is not, and `browser_history` is written to take a second
  // report without clobbering what the first one captured.
  const meta = () => {
    const icon = document.querySelector("link[rel~='icon']");
    window.ipc.postMessage(JSON.stringify({
      vela: 'meta',
      title: document.title || '',
      favicon: icon ? icon.href : '',
    }));
  };
  document.addEventListener('DOMContentLoaded', meta);
  window.addEventListener('load', meta);
})();
"#;

/// The extension's protocol table, verbatim.
///
/// Exposed so the desktop's own copy of the read allowlist can be CHECKED
/// against it rather than kept in step by hand — the same file, in the same
/// binary, is the page's provider.
#[cfg(test)]
#[must_use]
pub fn protocol_js() -> &'static str {
    PROTOCOL_JS
}

/// One EIP-1193 request, with the two facts the page cannot forge attached by
/// this side of the boundary.
pub struct Incoming {
    pub id: String,
    pub method: String,
    pub params_json: String,
    /// From the WEBVIEW's own URL, never from the envelope.
    pub origin: String,
}

/// Where a request goes once it has an origin.
///
/// Installed by the page, because only the page knows which window and which
/// column should answer. Not `Send`: it runs on the main thread, where the
/// webview's callback already is.
type RequestSink = Box<dyn Fn(Incoming)>;

thread_local! {
    static SINK: RefCell<Option<RequestSink>> = const { RefCell::new(None) };
}

/// Where a document load is reported. Same ownership as [`RequestSink`].
type NavSink = Box<dyn Fn(String)>;

/// What a page says it is called: `(url, title, favicon)`.
///
/// Untrusted, all three, and treated as display text only. The favicon is
/// stored for the cross-client record and never FETCHED: this shell draws a
/// letter, and fetching a URL a page handed us would be a beacon it gets for
/// free every time somebody opens their history.
type MetaSink = Box<dyn Fn(String, String, String)>;

thread_local! {
    static NAV: RefCell<Option<NavSink>> = const { RefCell::new(None) };
    static META: RefCell<Option<MetaSink>> = const { RefCell::new(None) };
}

/// Hand document loads to the page.
///
/// A NAVIGATION, not a URL poll: `dapp_permissions` settles the requests an
/// origin left pending when its document goes away, and a poll would report
/// that late — after the next page could have inherited an answer meant for
/// the last one. wry reports the load as it starts, which is the moment the
/// core's rule is written against.
pub fn on_navigation_to(sink: NavSink) {
    NAV.with(|slot| *slot.borrow_mut() = Some(sink));
}

/// Hand page metadata to the page (the wallet's page, that is).
pub fn on_meta_to(sink: MetaSink) {
    META.with(|slot| *slot.borrow_mut() = Some(sink));
}

/// Hand requests to the page. Called once, when the page builds the browser.
pub fn on_request_to(sink: RequestSink) {
    SINK.with(|slot| *slot.borrow_mut() = Some(sink));
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
pub fn place(bounds: Bounds<Pixels>, window: &Window, home: &str) {
    BROWSER.with(|slot| {
        let mut slot = slot.borrow_mut();
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
/// second and does.
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
    with_view(|view| {
        let _ = view.load_url(url);
    });
}

/// Back and forward. wry has no native pair, so this is the page's own
/// history — which is the same history the buttons in any browser drive.
pub fn back() {
    with_view(|view| {
        let _ = view.evaluate_script("history.back()");
    });
}

pub fn forward() {
    with_view(|view| {
        let _ = view.evaluate_script("history.forward()");
    });
}

pub fn reload() {
    with_view(|view| {
        let _ = view.reload();
    });
}

/// The document the browser is on, whole. `None` before the first page.
///
/// The URL and not the origin: the permissions machine derives its own origin
/// from it, and that derivation is the one its grants are keyed by.
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
#[must_use]
pub fn host() -> Option<String> {
    BROWSER.with(|slot| {
        let url = slot.borrow().as_ref()?.view.url().ok()?;
        let rest = url.split_once("://").map(|(_, rest)| rest).unwrap_or(&url);
        let host = rest.split(['/', '?', '#']).next().unwrap_or(rest);
        (!host.is_empty()).then(|| host.to_owned())
    })
}

fn with_view(act: impl FnOnce(&wry::WebView)) {
    BROWSER.with(|slot| {
        if let Some(browser) = slot.borrow().as_ref() {
            act(&browser.view);
        }
    });
}

fn build(window: &Window, home: &str) -> Option<wry::WebView> {
    let built = wry::WebViewBuilder::new()
        .with_initialization_script(provider_script())
        .with_initialization_script(BRIDGE_JS)
        .with_ipc_handler(on_request)
        .with_navigation_handler(|url| {
            NAV.with(|slot| {
                if let Some(sink) = slot.borrow().as_ref() {
                    sink(url);
                }
            });
            // Every navigation is allowed. This handler REPORTS; deciding
            // where a browser may go is not a thing this file gets to invent.
            true
        })
        .with_url(home)
        .build_as_child(window);
    match built {
        Ok(view) => {
            // Built hidden: `place` turns it on in the same frame, and a
            // webview that flashed into the wallet before its first layout
            // would be visible for exactly one frame in the wrong place.
            let _ = view.set_visible(false);
            Some(view)
        }
        Err(error) => {
            eprintln!("[vela-wallet] browser: {error}");
            None
        }
    }
}

/// One EIP-1193 request from a page.
///
/// The ORIGIN is read here, from the webview, and the envelope's own idea of
/// who it is is ignored — the rule the extension's content script exists to
/// enforce, and the reason this function and not the page attaches it.
///
/// A request with no sink installed is REFUSED rather than dropped: a
/// provider promise that never settles is the worst outcome this transport
/// can produce (spec 027 D37), and 4900 keeps "we could not answer" distinct
/// from the 4001 that would claim the person declined.
fn on_request(request: wry::http::Request<String>) {
    let Ok(parsed) = serde_json::from_str::<serde_json::Value>(request.body()) else {
        return;
    };
    // The bridge's other message. Checked first: it carries no `id`, so the
    // provider path below would drop it silently.
    if parsed.get("vela").and_then(|v| v.as_str()) == Some("meta") {
        let text = |key: &str| {
            parsed
                .get(key)
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_owned()
        };
        let url = BROWSER
            .with(|slot| slot.borrow().as_ref().and_then(|b| b.view.url().ok()))
            .unwrap_or_default();
        META.with(|slot| {
            if let Some(sink) = slot.borrow().as_ref() {
                sink(url, text("title"), text("favicon"));
            }
        });
        return;
    }
    let (Some(id), Some(method)) = (
        parsed.get("id").and_then(|v| v.as_str()),
        parsed.get("method").and_then(|v| v.as_str()),
    ) else {
        return;
    };
    let origin = BROWSER
        .with(|slot| slot.borrow().as_ref().and_then(|b| b.view.url().ok()))
        .map(|url| origin_of(&url))
        .unwrap_or_default();
    let incoming = Incoming {
        id: id.to_owned(),
        method: method.to_owned(),
        params_json: parsed
            .get("params")
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| "[]".to_owned()),
        origin,
    };
    let delivered = SINK.with(|slot| {
        slot.borrow().as_ref().map(|sink| {
            sink(incoming);
        })
    });
    if delivered.is_none() {
        let answer = serde_json::json!({
            "dir": "res",
            "id": id,
            "error": { "code": 4900, "message": "Vela cannot answer this request yet" },
        });
        deliver(&answer.to_string());
    }
}

/// Answer a request the signing panel decided.
///
/// The envelope the provider is waiting on: its own `id`, and either a result
/// or an error. The CODE is the core's — 4001 for a decline, 4900 for
/// stuck-but-submitted — because a shell that picked its own could report a
/// refusal as a failure, and a dApp treats those differently.
pub fn respond(id: &str, payload: &vela_core::app::sign_request::SignResponsePayload) {
    use vela_core::app::sign_request::SignResponsePayload;
    let answer = match payload {
        SignResponsePayload::Ok { result } => serde_json::json!({
            "dir": "res",
            "id": id,
            "result": result,
        }),
        SignResponsePayload::Err { code, message, .. } => serde_json::json!({
            "dir": "res",
            "id": id,
            "error": { "code": code, "message": message },
        }),
    };
    deliver(&answer.to_string());
}

/// The ORIGIN of a URL, by the CORE's own derivation.
///
/// The whole URL went over until spec 032 phase 27 measured what it wrote: a
/// grant keyed `http://127.0.0.1:8137/?v=3`. Every rule that reads this string
/// is written about an origin — a grant covers a SITE — so keying it by URL
/// asks again on the next page of the same site and leaves the connected chip
/// comparing two strings that cannot match.
///
/// `dapp_permissions::origin_of` and not a copy of it here: the machine
/// derives the origin from the page URL on every navigation, and a shell that
/// trimmed differently would write a grant under one spelling and look it up
/// under another. It also refuses anything that is not http(s), and normalises
/// the default port — two rules a hand-rolled `split` in this file would have
/// had to rediscover.
///
/// Empty when there is no origin, which matches no grant: fail closed.
fn origin_of(url: &str) -> String {
    vela_core::app::dapp_permissions::origin_of(url).unwrap_or_default()
}

/// Answer one provider request the permissions core decided.
///
/// The JSON shapes are the wire's, and the core said WHICH shape: an address
/// array for `eth_accounts`, EIP-2255's capability list for
/// `wallet_requestPermissions`, or an error whose code the core chose. The
/// words for a rejection are the shell's — the core carries a reason, not a
/// sentence — but nothing here decides whether it is a rejection.
pub fn respond_permission(id: &str, payload: &DpermRespondPayload) {
    let answer = match payload {
        DpermRespondPayload::Accounts { addresses } => serde_json::json!({
            "dir": "res",
            "id": id,
            "result": addresses,
        }),
        DpermRespondPayload::Permissions { granted } => serde_json::json!({
            "dir": "res",
            "id": id,
            "result": if *granted {
                serde_json::json!([{ "parentCapability": "eth_accounts" }])
            } else {
                serde_json::json!([])
            },
        }),
        DpermRespondPayload::Error { code, reason } => serde_json::json!({
            "dir": "res",
            "id": id,
            "error": { "code": code, "message": reject_message(*reason) },
        }),
    };
    deliver(&answer.to_string());
}

/// An EIP-1193 event, in the envelope the provider already listens for.
pub fn emit_page_event(event: &DpermPageEvent) {
    let answer = match event {
        DpermPageEvent::AccountsChanged { addresses } => serde_json::json!({
            "dir": "evt",
            "event": "accountsChanged",
            "data": addresses,
        }),
        DpermPageEvent::ChainChanged { chain_id_hex } => serde_json::json!({
            "dir": "evt",
            "event": "chainChanged",
            "data": chain_id_hex,
        }),
        DpermPageEvent::Disconnect => serde_json::json!({
            "dir": "evt",
            "event": "disconnect",
        }),
    };
    deliver(&answer.to_string());
}

/// What a rejection says. English by design and by precedent: this string goes
/// to a DEVELOPER's console, not onto a screen a person reads, and the
/// corpus's sentences are written for the second audience.
fn reject_message(reason: DpermRejectReason) -> &'static str {
    match reason {
        DpermRejectReason::UnauthorizedFrame => "Unauthorized frame",
        DpermRejectReason::NoAccountAvailable => "No wallet account available",
        DpermRejectReason::ConsentBusy => "Another connection request is open",
        DpermRejectReason::InsecureOrigin => "Signing requires a secure origin",
        DpermRejectReason::UserRejected => "User rejected the request",
        DpermRejectReason::NavigatedAway => "The page navigated away",
        DpermRejectReason::BrowserClosed => "The browser was closed",
        DpermRejectReason::NotConnected => "This site is not connected",
        DpermRejectReason::StaleAuthorizedAddress => "The authorized address changed",
    }
}

/// Answer a forwarded request the shell routed itself.
///
/// `eth_chainId`, a node read, a chain switch: an answer the core never sees
/// because it is not a permission. The envelope is the same one every other
/// answer uses, which is what keeps one id to one settled promise.
pub fn respond_json(id: &str, result: &serde_json::Value) {
    let answer = serde_json::json!({ "dir": "res", "id": id, "result": result });
    deliver(&answer.to_string());
}

/// The other half: a refusal with a code the caller chose deliberately.
pub fn respond_error(id: &str, code: i32, message: &str) {
    let answer = serde_json::json!({
        "dir": "res",
        "id": id,
        "error": { "code": code, "message": message },
    });
    deliver(&answer.to_string());
}

/// A request this wallet cannot answer yet.
///
/// 4900 and not 4001: the person did not decline, and a dApp that reads a
/// decline where there was none will tell them they refused something they
/// never saw.
pub fn refuse_unsupported(id: &str, method: &str) {
    let answer = serde_json::json!({
        "dir": "res",
        "id": id,
        "error": {
            "code": 4900,
            "message": format!("Vela's desktop cannot answer {method} yet"),
        },
    });
    deliver(&answer.to_string());
}

fn deliver(json: &str) {
    with_view(|view| {
        let script = format!(
            "window.__velaDeliver({})",
            serde_json::to_string(json).unwrap_or_else(|_| "\"{}\"".to_owned())
        );
        let _ = view.evaluate_script(&script);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grant covers a site, so the string the core judges must BE the site
    /// — and must be the SAME string the core derives on a navigation.
    #[test]
    fn an_origin_keeps_the_site_and_drops_the_page() {
        assert_eq!(
            origin_of("http://127.0.0.1:8137/?v=3"),
            "http://127.0.0.1:8137"
        );
        assert_eq!(
            origin_of("https://app.uniswap.org/swap?chain=gnosis#pool"),
            "https://app.uniswap.org"
        );
        // The port is part of the origin — :8137 and :9000 on one host are two
        // sites — except the scheme's own default, which is not spelled.
        assert_eq!(origin_of("http://localhost:3000/"), "http://localhost:3000");
        assert_eq!(
            origin_of("https://example.com:443/x"),
            "https://example.com"
        );
    }

    /// Nothing readable, nothing granted.
    #[test]
    fn an_unreadable_url_is_no_origin_at_all() {
        assert_eq!(origin_of("about:blank"), "");
        assert_eq!(origin_of(""), "");
        // A `file://` page is not a site anything may be granted to.
        assert_eq!(origin_of("file:///Users/me/index.html"), "");
    }

    /// The provider must be a CLASSIC script by the time it is injected.
    ///
    /// `inpage.js` is an ES module and an initialization script is not, so
    /// injecting it verbatim is a syntax error — the file never runs,
    /// `window.ethereum` never appears, and every request silently never
    /// happens. Nothing reports that: no error reaches the host, the page
    /// simply has no wallet in it. It is how this failed the first time, and
    /// this test is what makes it fail loudly the next time either file grows
    /// another module keyword.
    #[test]
    fn the_injected_provider_carries_no_module_syntax() {
        let script = provider_script();
        for (n, line) in script.lines().enumerate() {
            let line = line.trim_start();
            assert!(
                !line.starts_with("import ") && !line.starts_with("export "),
                "line {} is still module syntax: {line}",
                n + 1
            );
        }
    }

    /// …and that it is still the real provider, not an empty scope. A
    /// concatenation that silently produced nothing would pass the test above
    /// perfectly.
    #[test]
    fn the_injected_provider_is_the_real_one() {
        let script = provider_script();
        assert!(
            script.contains("'vela-1193'"),
            "the channel constant came across from protocol.js"
        );
        assert!(
            script.contains("eip6963:announceProvider"),
            "and the announcement came across from inpage.js"
        );
        assert!(
            script.len() > 10_000,
            "both files, not one: {}",
            script.len()
        );
    }
}
