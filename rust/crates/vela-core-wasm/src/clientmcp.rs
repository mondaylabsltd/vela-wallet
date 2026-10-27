//! Connecting the AI agent with clientmcp.dev, for the web (spec 106):
//! `vela_core::clientmcp_link`, JSON in and out. Every answer is
//! `{"ok": …}` or `{"error": {"kind": …, "detail": …}}`.

use vela_core::clientmcp_link::{self, json};
use wasm_bindgen::prelude::*;

/// Starts a sign-in: the pending sign-in to keep, with the page to open.
/// `random` is at least 64 bytes from `crypto.getRandomValues`.
#[wasm_bindgen(js_name = clientmcpLinkBegin)]
#[must_use]
pub fn begin(
    site: &str,
    redirect_uri: &str,
    label: &str,
    installation: &str,
    random: &[u8],
    now_ms: f64,
) -> String {
    json::begin(
        site,
        redirect_uri,
        label,
        installation,
        random,
        now_ms as u64,
    )
}

/// The code in the address the browser came back to, if it is this sign-in's.
#[wasm_bindgen(js_name = clientmcpLinkReturned)]
#[must_use]
pub fn returned(pending_json: &str, url: &str, now_ms: f64) -> String {
    json::returned(pending_json, url, now_ms as u64)
}

/// The one request that trades the code: `{url, body}`, POSTed as JSON.
#[wasm_bindgen(js_name = clientmcpLinkExchange)]
#[must_use]
pub fn exchange(pending_json: &str, code: &str) -> String {
    json::exchange(pending_json, code)
}

/// The site's answer: the bridge address and client token, or why not.
#[wasm_bindgen(js_name = clientmcpLinkAccept)]
#[must_use]
pub fn accept(status: u16, body: &str) -> String {
    json::accept(status, body)
}

/// A new installation ID from 16 random bytes.
#[wasm_bindgen(js_name = clientmcpInstallationId)]
#[must_use]
pub fn installation_id(random: &[u8]) -> String {
    clientmcp_link::installation_id(random)
}
