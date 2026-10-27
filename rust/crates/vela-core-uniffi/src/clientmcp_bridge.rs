//! Connecting the AI agent with clientmcp.dev, for Android and iOS (spec
//! 106): `vela_core::clientmcp_link`, JSON in and out, the same functions
//! the web and the desktop call. Every answer is `{"ok": …}` or
//! `{"error": {"kind": …, "detail": …}}`.

use vela_core::clientmcp_link::{self, json};

/// The phones' return address, as registered on clientmcp.dev.
#[uniffi::export]
#[must_use]
pub fn clientmcp_app_redirect() -> String {
    clientmcp_link::APP_REDIRECT.to_owned()
}

/// Starts a sign-in: the pending sign-in to keep, with the page to open.
/// `random` is at least 64 bytes from the platform's secure generator.
#[uniffi::export]
#[must_use]
pub fn clientmcp_link_begin(
    site: String,
    redirect_uri: String,
    label: String,
    installation: String,
    random: Vec<u8>,
    now_ms: u64,
) -> String {
    json::begin(&site, &redirect_uri, &label, &installation, &random, now_ms)
}

/// The code in the address the browser came back to, if it is this sign-in's.
#[uniffi::export]
#[must_use]
pub fn clientmcp_link_returned(pending_json: String, url: String, now_ms: u64) -> String {
    json::returned(&pending_json, &url, now_ms)
}

/// The one request that trades the code: `{url, body}`, POSTed as JSON.
#[uniffi::export]
#[must_use]
pub fn clientmcp_link_exchange(pending_json: String, code: String) -> String {
    json::exchange(&pending_json, &code)
}

/// The site's answer: the bridge address and client token, or why not.
#[uniffi::export]
#[must_use]
pub fn clientmcp_link_accept(status: u16, body: String) -> String {
    json::accept(status, &body)
}

/// A new installation ID from 16 random bytes.
#[uniffi::export]
#[must_use]
pub fn clientmcp_installation_id(random: Vec<u8>) -> String {
    clientmcp_link::installation_id(&random)
}

/// The site's account page, where the person connects their AI agent.
#[uniffi::export]
#[must_use]
pub fn clientmcp_account_url(site: String) -> String {
    clientmcp_link::account_url(&site)
}
