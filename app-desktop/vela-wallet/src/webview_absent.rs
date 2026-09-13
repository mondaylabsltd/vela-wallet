//! Linux has no in-app browser — this is the shape of the one it does not have.
//!
//! `mod webview` is gated to the two platforms with an embeddable browser
//! (WKWebView on macOS, WebView2 on Windows). But the code that ANSWERS a page
//! is not platform code: `wallet/browser_host.rs` is a Crux host, and its match
//! over `DpermOperation` has to stay exhaustive and identical everywhere. That
//! is exactly the wrong place for a `cfg` per arm — a missed arm is a silently
//! different answer on one platform, which is the class of bug this shell
//! keeps out of hosts on purpose.
//!
//! So the module exists on Linux under the same name, and the two calls that
//! deliver something to a document do nothing, because there is no document.
//!
//! This is not scaffolding for a future Linux browser. When Linux gets one it
//! is `webview.rs` that grows a third arm and this file that goes away.

use vela_core::app::dapp_permissions::{DpermPageEvent, DpermRespondPayload};

/// The answer to an EIP-1193 request. Nowhere to put it.
pub fn respond_permission(_id: &str, _payload: &DpermRespondPayload) {}

/// An EIP-1193 event. No page is listening.
pub fn emit_page_event(_event: &DpermPageEvent) {}

/// The extension's protocol table, verbatim — the same file `webview.rs` reads.
///
/// The one function here that is NOT inert, and deliberately so: the allowlist
/// parity test in `executor/dapp_rpc.rs` compares this crate's read allowlist
/// against the JavaScript's. That is two tables in one binary; it has nothing
/// to do with whether a browser can be embedded, and Linux is where CI runs it.
#[cfg(test)]
#[must_use]
pub fn protocol_js() -> &'static str {
    include_str!("../../../app-web/vela-wallet/extension/lib/protocol.js")
}
