//! Where a signature goes (founder, 2026-09-26): the key the account was
//! created or signed in with — never a choice made per signature.

use wasm_bindgen::prelude::*;

/// How an account signs on this device (spec 102), as JSON — a
/// `SigningPlan`: `{domain, venue, blocked?, key?}`. `key` is the key route
/// (`{credential_id, method, transports, hints}`), absent for a record written
/// before the account named its sign-in key, which signs as it always did.
/// `null` for a record this build cannot read. `account_json` is the stored
/// account record. See `vela_core::app::Account::signing_plan`.
#[wasm_bindgen(js_name = signingPlan)]
pub fn signing_plan(account_json: &str) -> Option<String> {
    vela_core::app::signing_plan_json(account_json)
}

/// Every venue an account on `domain` could pick, as a JSON `VenueChoice[]` —
/// Vela's sheet, the official page, then the saved pages, each reachable or
/// blocked with its reason (spec 102 R1, R2). `active_json` is the account's
/// `signing_venue`; `saved_json` the `SigningPage[]` Settings keeps. `null`
/// when either does not read.
#[wasm_bindgen(js_name = signingVenueChoices)]
pub fn signing_venue_choices(domain: &str, active_json: &str, saved_json: &str) -> Option<String> {
    let active = serde_json::from_str(active_json).ok()?;
    let saved: Vec<vela_core::signing_venue::SigningPage> =
        serde_json::from_str(saved_json).ok()?;
    serde_json::to_string(&vela_core::signing_venue::venue_choices(
        domain, &active, &saved,
    ))
    .ok()
}

/// R1: why `venue_json` cannot reach the keys of an account on `domain`, as a
/// JSON `VenueBlock` — or `null` when it can (or does not read as a venue,
/// which reaches nothing: the string `"invalid"`).
#[wasm_bindgen(js_name = signingVenueBlock)]
pub fn signing_venue_block(domain: &str, venue_json: &str) -> Option<String> {
    let Ok(venue) = serde_json::from_str(venue_json) else {
        return Some("\"invalid\"".to_owned());
    };
    vela_core::signing_venue::reachability(domain, &venue)
        .err()
        .and_then(|block| serde_json::to_string(&block).ok())
}

/// The domain whose keys a page at `url` can use (spec 102): its host, or
/// `getvela.app` for every `*.getvela.app` page. Empty for something that is
/// not an address.
#[wasm_bindgen(js_name = signingPageDomain)]
#[must_use]
pub fn signing_page_domain(url: &str) -> String {
    vela_core::signing_venue::domain_of_page(url)
}

/// A venue row's words as JSON (`{title_key, line_key, line_name}`) — `"in_vela"`,
/// `"page"`, or `"own_page"` (the choosers' "Use my own signing page") — or
/// `null` for a name the core does not know. See
/// `vela_core::app::method_words::venue_words`.
#[wasm_bindgen(js_name = venueWords)]
#[must_use]
pub fn venue_words(row: &str) -> Option<String> {
    vela_core::app::method_words::venue_words_json(row)
}

/// A key-method row's words as JSON (`{title_key, line_key, line_name}`), or
/// `null` for a wire name the core does not know (087 F01, F02). See
/// `vela_core::app::method_words`.
#[wasm_bindgen(js_name = keyMethodWords)]
#[must_use]
pub fn key_method_words(method: &str, chooser: &str, unlock: &str) -> Option<String> {
    vela_core::app::method_words::method_words_json(method, chooser, unlock)
}

/// A `FeeFailure` from the page: its wire name (`"quote_unavailable"`), or
/// for a failure that carries data its JSON text
/// (`{"chain_read":{"rate_limited":true}}`, spec 082 RJ13).
pub(crate) fn fee_failure_of(failure: &str) -> Option<vela_core::app::fee_policy::FeeFailure> {
    serde_json::from_value(serde_json::Value::String(failure.to_owned()))
        .ok()
        .or_else(|| serde_json::from_str(failure).ok())
}

/// The wait in ms before automatic fee re-quote `attempt` (1-based) after
/// `failure` (see [`fee_failure_of`]), or `undefined` when no retry can fix
/// it (spec 079 FR-008) — the extension's signing sheet re-asks on the same
/// schedule as every other client: 3 s, 6 s, then every 8 s (spec 082 RJ12).
#[wasm_bindgen(js_name = feeRequoteDelayMs)]
pub fn fee_requote_delay_ms(failure: &str, attempt: u32) -> Option<u32> {
    vela_core::app::fee_policy::requote_delay_ms(fee_failure_of(failure)?, attempt)
}

/// The bound on each automatic fee re-quote, in ms (spec 082 RJ12).
#[wasm_bindgen(js_name = feeRequoteTimeoutMs)]
#[must_use]
pub fn fee_requote_timeout_ms() -> u32 {
    vela_core::app::fee_policy::REQUOTE_TIMEOUT_MS
}

/// The corpus key of the reason line under a failed fee, or `undefined` for
/// none (spec 082 RJ13) — the one choice every shell used to make itself.
/// `explore.chainDown` takes `{{chain}}`. `failure` as for
/// [`fee_requote_delay_ms`].
#[wasm_bindgen(js_name = feeFailureReasonKey)]
#[must_use]
pub fn fee_failure_reason_key(failure: &str) -> Option<String> {
    vela_core::app::fee_policy::failure_reason_key(fee_failure_of(failure)?).map(str::to_owned)
}
