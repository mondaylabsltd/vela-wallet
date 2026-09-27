//! Linux has no in-app browser — this is the shape of the one it does not have.
//!
//! `mod webview` is gated to the two platforms with an embeddable browser
//! (WKWebView on macOS, WebView2 on Windows). But the code that ANSWERS a page
//! is not platform code: `wallet/browser_host.rs` drives the core's browser
//! machine, and the connected-sites list in Settings reads the same machine
//! on every desktop. That is exactly the wrong place for a `cfg` per arm — a
//! missed arm is a silently different answer on one platform, which is the
//! class of bug this shell keeps out of hosts on purpose.
//!
//! So the module exists on Linux under the same name, and the one call that
//! delivers something to a document does nothing, because there is no
//! document.
//!
//! This is not scaffolding for a future Linux browser. When Linux gets one it
//! is `webview.rs` that grows a third arm and this file that goes away.
//!
//! Nor is there anything to reach it: Linux's sidebar has no Explore
//! (`Section::available`, owner call 2026-09-24), as the web has none.

/// A message for a page. No page is listening.
pub fn deliver(_tab: &str, _message_json: &str) {}

/// Erase's sweep of the browser's own store (spec 081 FR-017).
///
/// `true`, not `false`: the bool answers "was the platform asked", and the
/// caller logs a `false` as "no web view to clear". On Linux there is no
/// browser and therefore no browsing data, so there is nothing to have failed
/// to clear — printing a warning about an absent web view on every erase would
/// be noise about a state that is permanent here, not a shortfall.
#[must_use]
pub fn clear_browsing_data() -> bool {
    true
}
