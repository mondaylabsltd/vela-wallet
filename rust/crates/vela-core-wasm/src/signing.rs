//! Where a signature goes (founder, 2026-09-26): the key the account was
//! created or signed in with — never a choice made per signature.

use wasm_bindgen::prelude::*;

/// Is `surface` the web's? The doors below take an optional last argument,
/// `"web"`, and answer as the web wallet must: it opens no signing page
/// (owner, 2026-09-23).
fn on_web(surface: Option<&str>) -> bool {
    surface == Some("web")
}

/// How an account signs on this device (spec 102), as JSON — a
/// `SigningPlan`: `{domain, venue, blocked?, key?, key_label}`. `key` is the
/// key route (`{credential_id, method, transports, hints}`), absent for a
/// record written before the account named its sign-in key, which signs as it
/// always did; `key_label` the name of that key as the person reads it
/// (`{name?, place_key}`). `null` for a record this build cannot read.
/// `account_json` is the stored account record.
///
/// With `surface = "web"` the plan is the web's
/// (`SigningPlan::on_web`): a `getvela.app` account whose venue is a page
/// signs in Vela, and a custom-domain account is `blocked` with
/// `{"type":"not_on_web"}` — refuse to sign, and report
/// `venue_blocked { block }` to the send/sign core so the sheet says why. See
/// `vela_core::app::Account::signing_plan`.
#[wasm_bindgen(js_name = signingPlan)]
pub fn signing_plan(account_json: &str, surface: Option<String>) -> Option<String> {
    if on_web(surface.as_deref()) {
        vela_core::app::signing_plan_on_web_json(account_json)
    } else {
        vela_core::app::signing_plan_json(account_json)
    }
}

/// Every venue an account on `domain` could pick, as a JSON `VenueChoice[]` —
/// Vela's sheet, the official page, then the saved pages, each reachable or
/// blocked with its reason (spec 102 R1, R2). `active_json` is the account's
/// `signing_venue`; `saved_json` the `SigningPage[]` Settings keeps. `null`
/// when either does not read. With `surface = "web"`, every page row R1 does
/// not already block is blocked `{"type":"not_on_web"}`
/// (`settings.venue.blockedWeb`): the web shows the rows, disabled, with why.
#[wasm_bindgen(js_name = signingVenueChoices)]
pub fn signing_venue_choices(
    domain: &str,
    active_json: &str,
    saved_json: &str,
    surface: Option<String>,
) -> Option<String> {
    let active = serde_json::from_str(active_json).ok()?;
    let saved: Vec<vela_core::signing_venue::SigningPage> =
        serde_json::from_str(saved_json).ok()?;
    let rows = if on_web(surface.as_deref()) {
        vela_core::signing_venue::venue_choices_on_web(domain, &active, &saved)
    } else {
        vela_core::signing_venue::venue_choices(domain, &active, &saved)
    };
    serde_json::to_string(&rows).ok()
}

/// The hand-off card's compact fee + speed row (spec 102, D4) as a
/// `HandoffFee` JSON, or `undefined` for no row. The web opens no page and
/// draws no hand-off card; exported for the gallery's boards, which draw the
/// apps' card. See `vela_core::app::sign_confirm::handoff_fee`.
#[wasm_bindgen(js_name = handoffFeeRow)]
#[must_use]
pub fn handoff_fee_row(fee_json: Option<String>, speed_json: Option<String>) -> Option<String> {
    vela_core::app::sign_confirm::handoff_fee_json(fee_json.as_deref(), speed_json.as_deref())
}

/// `{{time}}` in the integrity line's "… · checked {{time}}" (spec 102) — the
/// one rule every shell draws: the clock time in the person's format when the
/// check ran today, else the date and the time. `utc_offset_minutes` is
/// `-new Date().getTimezoneOffset()`; `date_format` / `time_format` the
/// person's presets as stored, `auto` resolved; `language` the page's.
#[wasm_bindgen(js_name = signerIntegrityTime)]
#[must_use]
pub fn signer_integrity_time(
    checked_at_ms: f64,
    now_ms: f64,
    utc_offset_minutes: i32,
    date_format: &str,
    time_format: &str,
    language: &str,
) -> String {
    vela_core::trusted_signer::launch::checked_time(
        ms(checked_at_ms),
        ms(now_ms),
        utc_offset_minutes,
        date_format,
        time_format,
        language,
    )
}

/// Does a check made at `checked_at_ms` still vouch for a page at `now_ms`
/// (spec 102: a day)?
#[wasm_bindgen(js_name = signerCheckFresh)]
#[must_use]
pub fn signer_check_fresh(checked_at_ms: f64, now_ms: f64) -> bool {
    vela_core::trusted_signer::launch::is_fresh_at(ms(checked_at_ms), ms(now_ms))
}

/// Is a background check of a page due (spec 102)? `checked_at_ms`: its last
/// admitting check (`undefined`: none); `last_attempt_ms`: when a check last
/// started. See `vela_core::trusted_signer::launch::refresh_due`.
#[wasm_bindgen(js_name = signerCheckRefreshDue)]
#[must_use]
pub fn signer_check_refresh_due(
    checked_at_ms: Option<f64>,
    last_attempt_ms: Option<f64>,
    now_ms: f64,
) -> bool {
    vela_core::trusted_signer::launch::refresh_due(
        checked_at_ms.map(ms),
        last_attempt_ms.map(ms),
        ms(now_ms),
    )
}

/// The headers a signing page's check sends (spec 102), as a JSON
/// `[[name, value], …]` — a browser's navigation `Accept`.
#[wasm_bindgen(js_name = signerPageCheckHeaders)]
#[must_use]
pub fn signer_page_check_headers() -> String {
    serde_json::to_string(vela_core::trusted_signer::launch::CHECK_HEADERS)
        .unwrap_or_else(|_| "[]".to_owned())
}

/// A JS clock reading (whole milliseconds, well inside u64) as the core's;
/// NaN and negatives read as 0.
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::allow_attributes
)]
fn ms(value: f64) -> u64 {
    if value.is_finite() && value > 0.0 {
        value.floor() as u64
    } else {
        0
    }
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

/// A venue row's words as JSON (`{title_key, line_key, line_name}`) —
/// `"in_vela"`, `"page"`, or `"signing_page"` (the choosers' "Use a trusted
/// signing page", D6; `"own_page"` still reads) — or `null` for a name the
/// core does not know. See `vela_core::app::method_words::venue_words`.
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

/// The words of a venue refusal (spec 102), as JSON `{key, vars}`: the corpus
/// key `VenueBlock::key()` names and the values its line takes by name
/// (`domain`, `pageDomain`) — translate `key` with `vars` and the sentence is
/// the core's whole. `null` for something that is not a `VenueBlock`.
#[wasm_bindgen(js_name = venueBlockLine)]
#[must_use]
pub fn venue_block_line(block_json: &str) -> Option<String> {
    let block: vela_core::signing_venue::VenueBlock = serde_json::from_str(block_json).ok()?;
    let vars: serde_json::Map<String, serde_json::Value> = block
        .vars()
        .into_iter()
        .map(|(name, value)| (name.to_owned(), serde_json::Value::String(value)))
        .collect();
    Some(serde_json::json!({ "key": block.key(), "vars": vars }).to_string())
}

/// The message a dApp reads when signing ended in the error the core named
/// `kind` (a `SignErrorKind`'s wire name), with the notice's `detail` — the
/// core's `dapp_rpc::sign_error_words`. `null` for a kind it does not know.
/// For the developer, not the person: EIP-1193 messages are not UI.
#[wasm_bindgen(js_name = signErrorWords)]
#[must_use]
pub fn sign_error_words(kind: &str, detail: Option<String>) -> Option<String> {
    let kind: vela_core::app::sign_request::SignErrorKind =
        serde_json::from_value(serde_json::Value::String(kind.to_owned())).ok()?;
    Some(vela_core::app::dapp_rpc::sign_error_words(
        kind,
        detail.as_deref(),
    ))
}
