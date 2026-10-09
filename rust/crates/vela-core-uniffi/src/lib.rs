//! uniffi 0.32 shell over vela-core → Kotlin (Android) + Swift (iOS).
//!
//! Thin by design: FFI mirror types + `#[uniffi::export]` wrappers, zero logic.
//! The mirrors convert 1:1 from vela-core's types; the recursive `AbiValue`
//! relies on uniffi 0.32's cycle detection (recursion is through `Vec`, so the
//! generated Swift struct/Kotlin data class need no special indirection).
//! Surface contract: specs/001-rust-core-bindings/contracts/core-api.md.

uniffi::setup_scaffolding!();

// The Crux state machines (spec 019-onboarding-live-wiring), exported with the
// same JSON surface the web gets from `vela-core-wasm`.
mod ctap_bridge;
/// Multicall3 encoding for the native read path (spec 051).
mod multicall;
mod onboarding_bridge;
mod settings_bridge;
mod trusted_signer_bridge;

pub use onboarding_bridge::{CreateWalletCore, LoginCore, SessionCore};

// ---------------------------------------------------------------------------
// Error (flat: foreign side sees variant + Display message)
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error, uniffi::Error)]
#[uniffi(flat_error)]
pub enum CoreError {
    #[error("{0}")]
    InvalidHex(String),
    #[error("{0}")]
    InvalidBase64Url(String),
    #[error("{0}")]
    InvalidQuantity(String),
    #[error("{0}")]
    InvalidAddress(String),
    #[error("{0}")]
    InvalidSignature(String),
    #[error("{0}")]
    InvalidCbor(String),
    #[error("{0}")]
    InvalidCoseKey(String),
    #[error("{0}")]
    InvalidClientData(String),
    #[error("{0}")]
    InvalidPublicKey(String),
    #[error("{0}")]
    AbiParse(String),
    #[error("{0}")]
    AbiDecode(String),
    #[error("{0}")]
    Eip712Parse(String),
    #[error("{0}")]
    Eip712NonCanonicalDomain(String),
    #[error("{0}")]
    InvalidIdenticonSeed(String),
    #[error("{0}")]
    I18nEmptyKeyList(String),
    #[error("{0}")]
    I18nInvalidCount(String),
    #[error("{0}")]
    I18nUnsupportedOption(String),
    #[error("{0}")]
    I18nCatalogUnavailable(String),
    #[error("{0}")]
    I18nCatalogParse(String),
    #[error("{0}")]
    RegistryMetadata(String),
    #[error("{0}")]
    RegistryProof(String),
    #[error("{0}")]
    Internal(String),
}

impl From<vela_core::CoreError> for CoreError {
    fn from(e: vela_core::CoreError) -> Self {
        use vela_core::CoreError as E;
        let msg = e.to_string();
        match e {
            E::InvalidHex(_) => CoreError::InvalidHex(msg),
            E::InvalidBase64Url(_) => CoreError::InvalidBase64Url(msg),
            E::InvalidQuantity(_) => CoreError::InvalidQuantity(msg),
            E::InvalidAddress(_) => CoreError::InvalidAddress(msg),
            E::InvalidSignature(_) => CoreError::InvalidSignature(msg),
            E::InvalidCbor(_) => CoreError::InvalidCbor(msg),
            E::InvalidCoseKey(_) => CoreError::InvalidCoseKey(msg),
            E::InvalidClientData(_) => CoreError::InvalidClientData(msg),
            E::InvalidPublicKey(_) => CoreError::InvalidPublicKey(msg),
            E::AbiParse(_) => CoreError::AbiParse(msg),
            E::AbiDecode(_) => CoreError::AbiDecode(msg),
            E::Eip712Parse(_) => CoreError::Eip712Parse(msg),
            E::Eip712NonCanonicalDomain(_) => CoreError::Eip712NonCanonicalDomain(msg),
            E::InvalidIdenticonSeed(_) => CoreError::InvalidIdenticonSeed(msg),
            E::I18nEmptyKeyList(_) => CoreError::I18nEmptyKeyList(msg),
            E::I18nInvalidCount(_) => CoreError::I18nInvalidCount(msg),
            E::I18nUnsupportedOption(_) => CoreError::I18nUnsupportedOption(msg),
            E::I18nCatalogUnavailable(_) => CoreError::I18nCatalogUnavailable(msg),
            E::I18nCatalogParse(_) => CoreError::I18nCatalogParse(msg),
            E::RegistryMetadata(_) => CoreError::RegistryMetadata(msg),
            E::RegistryProof(_) => CoreError::RegistryProof(msg),
            E::Internal(_) => CoreError::Internal(msg),
        }
    }
}

// ---------------------------------------------------------------------------
// Records / enums (mirrors of vela_core::types + AbiValue)
// ---------------------------------------------------------------------------

/// Recursive decoded-calldata tree (uniffi 0.32 auto-detects the cycle).
#[derive(Debug, Clone, uniffi::Record)]
pub struct AbiValue {
    pub kind: String,
    pub name: String,
    pub value: String,
    pub children: Vec<AbiValue>,
}

impl From<vela_core::AbiValue> for AbiValue {
    fn from(v: vela_core::AbiValue) -> Self {
        AbiValue {
            kind: v.kind,
            name: v.name,
            value: v.value,
            children: v.children.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct P256PublicKey {
    pub x: Vec<u8>,
    pub y: Vec<u8>,
}

impl From<vela_core::P256PublicKey> for P256PublicKey {
    fn from(k: vela_core::P256PublicKey) -> Self {
        P256PublicKey { x: k.x, y: k.y }
    }
}

impl From<P256PublicKey> for vela_core::P256PublicKey {
    fn from(k: P256PublicKey) -> Self {
        vela_core::P256PublicKey { x: k.x, y: k.y }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct SafeAddressInfo {
    pub address: String,
    pub salt_nonce: Vec<u8>,
    pub setup_data: Vec<u8>,
    pub init_code_hash: Vec<u8>,
}

impl From<vela_core::SafeAddressInfo> for SafeAddressInfo {
    fn from(i: vela_core::SafeAddressInfo) -> Self {
        SafeAddressInfo {
            address: i.address,
            salt_nonce: i.salt_nonce,
            setup_data: i.setup_data,
            init_code_hash: i.init_code_hash,
        }
    }
}

#[derive(Debug, Clone, uniffi::Record)]
pub struct WebAuthnAssertion {
    pub authenticator_data: Vec<u8>,
    pub client_data_json: Vec<u8>,
    pub signature_der: Vec<u8>,
}

impl From<WebAuthnAssertion> for vela_core::WebAuthnAssertion {
    fn from(a: WebAuthnAssertion) -> Self {
        vela_core::WebAuthnAssertion {
            authenticator_data: a.authenticator_data,
            client_data_json: a.client_data_json,
            signature_der: a.signature_der,
        }
    }
}

#[derive(Debug, Clone, Copy, uniffi::Enum)]
pub enum ClientDataKind {
    Create,
    Get,
}

impl From<ClientDataKind> for vela_core::ClientDataKind {
    fn from(k: ClientDataKind) -> Self {
        match k {
            ClientDataKind::Create => vela_core::ClientDataKind::Create,
            ClientDataKind::Get => vela_core::ClientDataKind::Get,
        }
    }
}

// ---------------------------------------------------------------------------
// primitives
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn keccak256(data: Vec<u8>) -> Vec<u8> {
    vela_core::primitives::keccak256(&data)
}

#[uniffi::export]
pub fn sha256(data: Vec<u8>) -> Vec<u8> {
    vela_core::primitives::sha256(&data)
}

#[uniffi::export]
pub fn to_hex(data: Vec<u8>, prefixed: bool) -> String {
    vela_core::primitives::to_hex(&data, prefixed)
}

#[uniffi::export]
pub fn from_hex(s: String) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::primitives::from_hex(&s)?)
}

#[uniffi::export]
pub fn to_quantity(value: String) -> Result<String, CoreError> {
    Ok(vela_core::primitives::to_quantity(&value)?)
}

#[uniffi::export]
pub fn checksum_address(address_hex: String) -> Result<String, CoreError> {
    Ok(vela_core::primitives::checksum_address(&address_hex)?)
}

#[uniffi::export]
pub fn function_selector(signature: String) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::primitives::function_selector(&signature)?)
}

#[uniffi::export]
pub fn create2_address(
    deployer_hex: String,
    salt: Vec<u8>,
    init_code_hash: Vec<u8>,
) -> Result<String, CoreError> {
    Ok(vela_core::primitives::create2_address(
        &deployer_hex,
        &salt,
        &init_code_hash,
    )?)
}

#[uniffi::export]
pub fn to_base64url(data: Vec<u8>) -> String {
    vela_core::primitives::to_base64url(&data)
}

#[uniffi::export]
pub fn from_base64url(s: String) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::primitives::from_base64url(&s)?)
}

#[uniffi::export]
pub fn abi_encode_address(address_hex: String) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::primitives::abi_encode_address(&address_hex)?)
}

#[uniffi::export]
pub fn abi_encode_uint256(value_hex: String) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::primitives::abi_encode_uint256(&value_hex)?)
}

#[uniffi::export]
pub fn abi_encode_bytes32(data: Vec<u8>) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::primitives::abi_encode_bytes32(&data)?)
}

// ---------------------------------------------------------------------------
// abi
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn canonicalize_signature(sig: String) -> Result<String, CoreError> {
    Ok(vela_core::abi::canonicalize_signature(&sig)?)
}

#[uniffi::export]
pub fn compute_selector(sig: String) -> Result<String, CoreError> {
    Ok(vela_core::abi::compute_selector(&sig)?)
}

#[uniffi::export]
pub fn match_selector(sig: String, calldata: Vec<u8>) -> Result<bool, CoreError> {
    Ok(vela_core::abi::match_selector(&sig, &calldata)?)
}

#[uniffi::export]
pub fn decode_calldata(sig: String, calldata: Vec<u8>) -> Result<AbiValue, CoreError> {
    Ok(vela_core::abi::decode_calldata(&sig, &calldata)?.into())
}

// ---------------------------------------------------------------------------
// eip712
// ---------------------------------------------------------------------------

/// What a site's message request asks the account to sign, before the
/// Safe's `SafeMessage` wrap: EIP-191 for `personal_sign` / `eth_sign`, the
/// EIP-712 digest for typed data, picked where each method carries it.
/// `None` when there is nothing to sign. One rule for every shell and the
/// Trusted Signer's page.
#[uniffi::export]
pub fn sign_message_hash(method: String, params_json: String) -> Option<Vec<u8>> {
    vela_core::sign_message::original_hash(&method, &params_json)
}

/// The ONE document a typed-data request is read as — what a sheet decodes,
/// the same bytes [`sign_message_hash`] covers. `None` when the request is
/// not a valid typed-data request (the core refuses it at arrival). The audit
/// of 2026-10-01: each shell's own pick previewed one document and signed
/// another.
#[uniffi::export]
pub fn typed_data_document(method: String, params_json: String) -> Option<String> {
    vela_core::typed_data_request::document_json_of(&method, &params_json)
}

/// The calls a dApp transaction request sends — `params[0].calls` of a
/// `wallet_sendCalls`, else `params[0]` — every one or none, value in
/// DECIMAL wei. `None` when any call is unreadable, has no recipient, or
/// there are none. One rule for every shell (spec 096 F1): each used to read
/// `value` its own way, and one request could be signed as different amounts.
#[uniffi::export]
pub fn dapp_request_calls(method: String, params_json: String) -> Option<Vec<UserOpCall>> {
    vela_core::tx_request::calls_of(&method, &params_json).map(|calls| {
        calls
            .into_iter()
            .map(|call| UserOpCall {
                to: call.to,
                value: call.value,
                data: call.data,
            })
            .collect()
    })
}

#[uniffi::export]
pub fn hash_typed_data(typed_data_json: String) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::eip712::hash_typed_data(&typed_data_json)?)
}

#[uniffi::export]
pub fn encode_type(typed_data_json: String) -> Result<String, CoreError> {
    Ok(vela_core::eip712::encode_type(&typed_data_json)?)
}

// ---------------------------------------------------------------------------
// safe
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn parse_public_key(hex: String) -> Result<P256PublicKey, CoreError> {
    Ok(vela_core::safe::parse_public_key(&hex)?.into())
}

#[uniffi::export]
pub fn compute_safe_address(x: Vec<u8>, y: Vec<u8>) -> Result<SafeAddressInfo, CoreError> {
    Ok(vela_core::safe::compute_safe_address(&x, &y)?.into())
}

#[uniffi::export]
pub fn compute_safe_address_multi(keys: Vec<P256PublicKey>) -> Result<SafeAddressInfo, CoreError> {
    let keys: Vec<vela_core::P256PublicKey> = keys.into_iter().map(Into::into).collect();
    Ok(vela_core::safe::compute_safe_address_multi(&keys)?.into())
}

#[uniffi::export]
pub fn compute_webauthn_signer_address(x: Vec<u8>, y: Vec<u8>) -> Result<String, CoreError> {
    Ok(vela_core::safe::compute_webauthn_signer_address(&x, &y)?)
}

#[uniffi::export]
pub fn compute_splitter_address(treasury_hex: String) -> Result<String, CoreError> {
    Ok(vela_core::safe::compute_splitter_address(&treasury_hex)?)
}

#[uniffi::export]
pub fn encode_splitter_deploy_call(treasury_hex: String) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::safe::encode_splitter_deploy_call(&treasury_hex)?)
}

#[uniffi::export]
pub fn safe_proxy_runtime_code() -> Result<String, CoreError> {
    Ok(vela_core::safe::safe_proxy_runtime_code()?)
}

// ---------------------------------------------------------------------------
// webauthn
// ---------------------------------------------------------------------------

#[uniffi::export]
pub fn extract_attestation_public_key(
    attestation_object: Vec<u8>,
) -> Result<P256PublicKey, CoreError> {
    Ok(vela_core::webauthn::extract_attestation_public_key(&attestation_object)?.into())
}

#[uniffi::export]
pub fn der_signature_to_raw_low_s(der: Vec<u8>) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::webauthn::der_signature_to_raw_low_s(&der)?)
}

#[uniffi::export]
pub fn validate_client_data(
    kind: ClientDataKind,
    client_data_json: Vec<u8>,
    authenticator_data: Vec<u8>,
) -> Result<(), CoreError> {
    Ok(vela_core::webauthn::validate_client_data(
        kind.into(),
        &client_data_json,
        &authenticator_data,
    )?)
}

#[uniffi::export]
pub fn webauthn_signing_hash(authenticator_data: Vec<u8>, client_data_json: Vec<u8>) -> Vec<u8> {
    vela_core::webauthn::webauthn_signing_hash(&authenticator_data, &client_data_json)
}

#[uniffi::export]
pub fn recover_public_key_from_assertions(
    a: WebAuthnAssertion,
    b: WebAuthnAssertion,
) -> Result<Option<P256PublicKey>, CoreError> {
    let result = vela_core::webauthn::recover_public_key_from_assertions(&a.into(), &b.into())?;
    Ok(result.map(Into::into))
}

// ---------------------------------------------------------------------------
// identicon (spec 003-rust-identicon, contracts/identicon-api.md)
// ---------------------------------------------------------------------------

/// Flattened `IdenticonParams` — colours plus the four artwork fragments, matching
/// the shape the JS library returns so migrating call sites stay recognisable.
#[derive(Debug, Clone, uniffi::Record)]
pub struct IdenticonParams {
    pub main: String,
    pub background: String,
    pub accent: String,
    pub top: String,
    pub sides: String,
    pub face: String,
    pub bottom: String,
}

impl From<vela_core::identicon::IdenticonParams> for IdenticonParams {
    fn from(p: vela_core::identicon::IdenticonParams) -> Self {
        IdenticonParams {
            main: p.colors.main.to_owned(),
            background: p.colors.background.to_owned(),
            accent: p.colors.accent.to_owned(),
            top: p.sections.top.to_owned(),
            sides: p.sections.sides.to_owned(),
            face: p.sections.face.to_owned(),
            bottom: p.sections.bottom.to_owned(),
        }
    }
}

/// **The wallet's identicon.** Circular variant, no SVG ids — safe to render many
/// instances into one document.
#[uniffi::export]
pub fn identicon_svg_circular(seed: String) -> Result<String, CoreError> {
    Ok(vela_core::identicon::identicon_svg_circular(&seed)?)
}

/// The library's stock hexagonal output.
#[uniffi::export]
pub fn identicon_svg(seed: String) -> Result<String, CoreError> {
    Ok(vela_core::identicon::identicon_svg(&seed)?)
}

/// Stock output as a `data:image/svg+xml;base64,…` URI.
#[uniffi::export]
pub fn identicon_data_uri(seed: String) -> Result<String, CoreError> {
    Ok(vela_core::identicon::identicon_data_uri(&seed)?)
}

#[uniffi::export]
pub fn identicon_params(seed: String) -> Result<IdenticonParams, CoreError> {
    Ok(vela_core::identicon::identicon_params(&seed)?.into())
}

#[uniffi::export]
pub fn identicon_make_hash(seed: String) -> String {
    vela_core::identicon::make_hash(&seed).as_str().to_owned()
}

/// Case- and length-normalises a seed. Every platform must call this rather than
/// lowercasing locally — that is how the platforms drift apart.
#[uniffi::export]
pub fn identicon_normalize_seed(seed: String) -> String {
    vela_core::identicon::normalize_seed(&seed).into_owned()
}

/// **The wallet's identicon as PNG bytes** (`size_px` × `size_px`), rasterized
/// from the same circular SVG every platform shares (spec 015, research.md D1).
/// Kotlin decodes with `BitmapFactory`, Swift with `UIImage(data:)`. Normalize
/// the seed first, exactly as with the SVG entry points. `size_px` is capped at
/// 1024.
#[uniffi::export]
pub fn identicon_png(seed: String, size_px: u32) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::identicon_raster::identicon_png(&seed, size_px)?)
}

/// **A passkey provider's mark as PNG bytes** (`size_px` × `size_px`), from the
/// vendored AAGUID catalog. `None` when the catalog does not know the model —
/// hardware keys and attestation-less registrations both land there — and the
/// caller then shows what it showed before this existed.
///
/// The lookup is offline by construction: asking a directory service would tell
/// it which vault holds a Vela wallet's key.
#[uniffi::export]
pub fn passkey_provider_png(
    aaguid: String,
    dark: bool,
    size_px: u32,
) -> Result<Option<Vec<u8>>, CoreError> {
    Ok(vela_core::identicon_raster::passkey_provider_png(
        &aaguid, dark, size_px,
    )?)
}

/// Sign-in's first question through the three layers — index, Gnosis,
/// Ethereum. See `vela_core::registry_resolve`.
#[uniffi::export]
#[must_use]
pub fn registry_resolve_key_step(public_key_hex: String, answers_json: String) -> String {
    vela_core::registry_resolve::key_step_json(&public_key_hex, &answers_json)
}

/// Sign-in's second question, with an index answer PROVED against the chain.
/// `source` is the token the key step returned.
#[uniffi::export]
#[must_use]
pub fn registry_resolve_unit_step(unit_id: u32, source: String, answers_json: String) -> String {
    vela_core::registry_resolve::unit_step_json(u64::from(unit_id), &source, &answers_json)
}

/// "Sign with": which credential a ceremony is pinned to and how it is reached,
/// for the method the person chose — `None` for `auto`. See
/// `vela_core::wallet_keys::sign_route`.
#[uniffi::export]
#[must_use]
pub fn sign_route(device_keys_json: String, method: String) -> Option<String> {
    vela_core::wallet_keys::sign_route_json(&device_keys_json, &method)
}

/// Which browser tabs to let go of now (spec 099 R2): an `EngineInput` JSON in
/// (tabs, selected, recent, busy, live, pressure), an `EnginePlan` JSON out
/// (`{"suspend":[…]}`). `None` for input that does not read. See
/// `vela_core::app::browser_tabs`.
#[uniffi::export]
#[must_use]
pub fn browser_engine_plan(input_json: String) -> Option<String> {
    vela_core::app::browser_tabs::plan_engines_json(&input_json)
}

/// Which tabs a batch close takes (spec 099 — "close other tabs", "close tabs
/// to the right", "close all tabs"): the strip's tabs (`ExploreView.tabs`
/// JSON) and a `TabCloseScope` JSON (`{"type":"others","keep":…}`,
/// `{"type":"right","of":…}`, `{"type":"all"}`) in, the ids out (JSON array).
/// The shell closes each one's engine and tells the browser machine, then
/// sends `explore_sites`' `tabs_closed` with them. See
/// `vela_core::app::explore_sites::tabs_closed_by`.
#[uniffi::export]
#[must_use]
pub fn explore_tabs_closed_by(tabs_json: String, scope_json: String) -> Option<String> {
    vela_core::app::explore_sites::tabs_closed_by_json(&tabs_json, &scope_json)
}

/// What Explore shows when somebody enters it: the explore view (JSON — only
/// `tabs`, `selected_tab` and `recent_tabs` are read), what brought it up
/// (`"section"` from another section, `"reselect"` chosen again while up,
/// `"page_opened"` a page opened from outside — asked once that open is in
/// the view) and the tab whose request waits on the person
/// (`browser_waiting_tab`) in; an `ExploreLanding` JSON out —
/// `{"type":"home"}` or `{"type":"tab","id":…}`. `None` for input that does
/// not read. See `vela_core::app::browser_tabs::explore_landing`.
#[uniffi::export]
#[must_use]
pub fn explore_landing(
    view_json: String,
    entry: String,
    waiting: Option<String>,
) -> Option<String> {
    vela_core::app::browser_tabs::explore_landing_json(&view_json, &entry, waiting.as_deref())
}

/// Which tab an open goes into — never over a live dApp from the home: the
/// explore view (JSON), the tab whose page is in the view, whether that page
/// is on screen (`false` for anything asked from the home or from outside),
/// the address, and how it was asked for (`"address"` typed or handed in,
/// `"site"` a favourite, recent or featured tile picked) in; an
/// `ExploreOpenTarget` JSON out — `{"type":"load","id":…}` (send
/// `tab_selected` when it is not the selected tab, then `tab_navigated`, and
/// load it there), `{"type":"resume","id":…}` (a tab already on that site:
/// `tab_selected`, shown as it was left) or `{"type":"new_tab"}`
/// (`tab_opened`; never for a full strip, whose open loads in a start-page
/// tab, else the tab used longest ago — never the dApp just left while
/// another tab will do). `None` for input that does not read.
/// See `vela_core::app::browser_tabs::open_target`.
#[uniffi::export]
#[must_use]
pub fn browser_open_target(
    view_json: String,
    shown: Option<String>,
    on_page: bool,
    url: String,
    kind: String,
) -> Option<String> {
    vela_core::app::browser_tabs::open_target_json(
        &view_json,
        shown.as_deref(),
        on_page,
        &url,
        &kind,
    )
}

/// The tab a strip or switcher marks as "this tab": the page's tab while a
/// page is on screen; over the home, the selected tab only when it is a
/// start-page tab — a tab waiting unlit is not "this tab" under a home page.
/// `None` when nothing is lit or the view does not read. See
/// `vela_core::app::browser_tabs::lit_tab`.
#[uniffi::export]
#[must_use]
pub fn browser_lit_tab(view_json: String, shown: Option<String>, on_page: bool) -> Option<String> {
    vela_core::app::browser_tabs::lit_tab_json(&view_json, shown.as_deref(), on_page)
}

/// The tab whose request is in front of the person — the browser machine's
/// consent, signature or add-network sheet — from a `DbrView` JSON: what
/// `explore_landing` takes as `waiting`. `None` while nothing waits. See
/// `vela_core::app::browser_tabs::waiting_tab`.
#[uniffi::export]
#[must_use]
pub fn browser_waiting_tab(dapp_view_json: String) -> Option<String> {
    vela_core::app::browser_tabs::waiting_tab_json(&dapp_view_json)
}

/// May the signing confirm be tapped, and if not why (spec 099 R7): the sign, guard,
/// clear-signing and fee views as last rendered (JSON; `fee_json` `None` with
/// no fee session) and the speed in force (`"fast"`…, `None` with no speed
/// control). A `ConfirmState` JSON out — `{enabled, block, key}`; `None` when
/// a view does not read, and the confirm stays disabled. See
/// `vela_core::app::sign_confirm`.
#[uniffi::export]
#[must_use]
pub fn sign_confirm_state(
    sign_json: String,
    guard_json: String,
    clear_json: String,
    fee_json: Option<String>,
    speed_tier: Option<String>,
) -> Option<String> {
    vela_core::app::sign_confirm::confirm_state_json(
        &sign_json,
        &guard_json,
        &clear_json,
        fee_json.as_deref(),
        speed_tier.as_deref(),
    )
}

/// The landing's countdown (spec 099 R6), counted from when the relay put the
/// bundle on the network (`TrackEntryView.relay_sent_at_ms`): a `LandingPace`
/// JSON — `{line, seconds, progress}`. See `vela_core::app::tx_tracker`.
#[uniffi::export]
#[must_use]
pub fn landing_pace(sent_at_ms: Option<f64>, typical_s: Option<u16>, now_ms: f64) -> String {
    vela_core::app::tx_tracker::landing_pace_json(sent_at_ms, typical_s, now_ms)
}

/// A key-method row's words (087 F01, F02): the title's corpus key and the
/// line under it — `line_key` a corpus key to translate, or `line_name` a
/// product name ("Face ID") drawn as it is; exactly one of the two is set.
/// See `vela_core::app::method_words`.
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct KeyMethodWords {
    pub title_key: String,
    pub line_key: Option<String>,
    pub line_name: Option<String>,
}

/// The words of one key-method row in the create or sign-in chooser, decided
/// once in the core for every shell. All three are wire names: `method`
/// (`"platform"`, `"hybrid"`, `"security_key"`, `"trusted_signer"`),
/// `chooser` (`"create"`, `"sign_in"`) and `unlock` — what unlocks a passkey
/// on this device as far as the shell can tell (`"face_id"`, `"touch_id"`,
/// `"windows_hello"`, `"other"`). `None` for a name the core does not know.
#[uniffi::export]
#[must_use]
pub fn key_method_words(method: String, chooser: String, unlock: String) -> Option<KeyMethodWords> {
    let words = vela_core::app::method_words::method_words(&method, &chooser, &unlock)?;
    Some(KeyMethodWords {
        title_key: words.title_key.to_owned(),
        line_key: words.line_key().map(str::to_owned),
        line_name: words.line_name().map(str::to_owned),
    })
}

/// Where an account's signatures go (founder, 2026-09-26): the key it was
/// created or signed in with, over the route that reached it — `None` for a
/// record written before that existed, which signs as it always did.
/// `account_json` is the stored account record. See
/// `vela_core::app::Account::sign_in_route`.
#[uniffi::export]
#[must_use]
pub fn sign_in_route(account_json: String) -> Option<String> {
    vela_core::app::sign_in_route_json(&account_json)
}

/// Which passkeys control the wallet at `address` — the Settings keys view
/// (spec 062). `sign_in_credential` is the account's sign-in route credential
/// (`sign_in_route`), empty for none: its row is marked `signs_here`. See
/// `vela_core::wallet_keys`.
#[uniffi::export]
#[must_use]
pub fn wallet_keys_step(
    address: String,
    device_keys_json: String,
    answers_json: String,
    sign_in_credential: String,
) -> String {
    vela_core::wallet_keys::step_json(
        &address,
        &device_keys_json,
        &answers_json,
        &sign_in_credential,
    )
}

/// **Signing in when the index is gone** (spec 062): the registry contract's
/// side of the index's two read questions, answered in the index's own JSON
/// shapes so a shell's existing parsing runs unchanged. `…Plan` = the chains to
/// try (in order) and the two `eth_call`s to make on one of them; the matching
/// function turns the two raw results into the body, or nothing when that
/// chain did not really answer (the shell then tries the next). Unit ids are
/// per deployment: ask about a key's units on the chain that listed them.
#[uniffi::export]
pub fn registry_chain_key_plan(public_key_hex: String) -> Option<String> {
    vela_core::registry_chain::key_status_plan_json(&public_key_hex)
}

/// See [`registry_chain_key_plan`].
#[uniffi::export]
pub fn registry_chain_key_status(has_entry_hex: String, groups_hex: String) -> Option<String> {
    vela_core::registry_chain::key_status_json(&has_entry_hex, &groups_hex)
}

/// See [`registry_chain_key_plan`].
#[uniffi::export]
pub fn registry_chain_unit_plan(unit_id: u32) -> Option<String> {
    vela_core::registry_chain::unit_plan_json(u64::from(unit_id))
}

/// See [`registry_chain_key_plan`].
#[uniffi::export]
pub fn registry_chain_unit(unit_id: u32, unit_hex: String, members_hex: String) -> Option<String> {
    vela_core::registry_chain::unit_json(u64::from(unit_id), &unit_hex, &members_hex)
}

/// **Backing the founding record up to Ethereum — the next step of the walk**
/// (spec 062). Server-free: every request is an `eth_call` against the
/// registry contract, on Gnosis (where the record lives) or Ethereum (where
/// the backup goes). `answers_json` is the transcript (`LookupAnswer[]`, the
/// same shape `registryNameStep` uses); the return is a `BackupStep` as JSON —
/// requests to perform, or the verdict and, when the wallet is not backed up,
/// the one call that would do it. No passkey is involved at any point.
#[uniffi::export]
pub fn registry_backup_step(
    address: String,
    founding_public_key_hex: String,
    answers_json: String,
    target_chain: Option<u32>,
) -> String {
    vela_core::registry_backup::step_json(
        &address,
        &founding_public_key_hex,
        target_chain,
        &answers_json,
    )
}

/// **The registered name behind an address — the next step of the lookup.**
///
/// The v2 passkey index cannot be asked about an address, so the name is
/// reached through the chain (`vela_core::registry_lookup`). `answers_json` is
/// the transcript so far (`LookupAnswer[]`); the return is a `LookupStep` as
/// JSON: requests to perform (an `eth_call`, a GET against the configured
/// index), or the verdict and how long it may be remembered. The shell owns
/// the transport; every rule is the core's.
#[uniffi::export]
pub fn registry_name_step(address: String, answers_json: String) -> String {
    vela_core::registry_lookup::step_json(&address, &answers_json)
}

/// **Does this name actually belong to this address — the next step of the
/// check.**
///
/// A reverse record (`addr.reverse`) is written by the address itself, so it is
/// a CLAIM: anyone who funds an address can name it after whoever their victim
/// is about to pay. `vela_core::app::name_verify` resolves the claimed name
/// forward and compares, and nothing but `verified` may be drawn (spec 081,
/// FR-010).
///
/// `answers_json` is the transcript so far (`LookupAnswer[]`, the same shape
/// `registry_name_step` uses); the return is a `VerifyStep` as JSON: `eth_call`s
/// to perform, or the verdict plus the name exactly as it was proven. The shell
/// owns the transport; every rule is the core's.
#[uniffi::export]
pub fn verified_name_step(
    chain_id: u32,
    registry: String,
    address: String,
    name: String,
    answers_json: String,
) -> String {
    vela_core::app::name_verify::step_json(chain_id, &registry, &address, &name, &answers_json)
}

/// **Where to ask about a model the compiled catalog cannot name**, or `None`
/// when there is nothing to ask: a malformed or all-zero AAGUID, or one the
/// catalog already answers offline.
///
/// The shells own the transport; the core owns the contract, so four clients
/// cannot ask four different questions or trust four different answers.
#[uniffi::export]
pub fn passkey_directory_url(aaguid: String) -> Option<String> {
    vela_core::passkey::directory_lookup_url(&aaguid)
}

/// **Read a directory answer.** `None` unless the body is about the AAGUID that
/// was asked about and carries a usable name; the icon URL is present only when
/// the path is the service's own shape.
#[uniffi::export]
pub fn passkey_directory_entry(
    aaguid: String,
    json: String,
    dark: bool,
) -> Option<PasskeyDirectoryEntry> {
    vela_core::passkey::directory_entry(&aaguid, &json, dark).map(Into::into)
}

/// What the directory said about a model.
#[derive(uniffi::Record)]
pub struct PasskeyDirectoryEntry {
    pub name: String,
    pub icon_url: Option<String>,
}

impl From<vela_core::passkey::DirectoryEntry> for PasskeyDirectoryEntry {
    fn from(entry: vela_core::passkey::DirectoryEntry) -> Self {
        Self {
            name: entry.name,
            icon_url: entry.icon_url,
        }
    }
}

/// **The security-key fallback mark as PNG bytes**, for a key whose AAGUID the
/// catalog cannot name. `None` when the row deserves no mark of this kind — a
/// platform authenticator, which the client already draws its own way.
///
/// The three colours are the caller's tokens: the artwork ships in one theme,
/// and one vendor's greys are not this app's greys in either.
#[uniffi::export]
pub fn passkey_fallback_png(
    authenticator_attachment: String,
    transports: String,
    chose_security_key: bool,
    strong: String,
    soft: String,
    hole: String,
    size_px: u32,
) -> Result<Option<Vec<u8>>, CoreError> {
    Ok(vela_core::identicon_raster::passkey_fallback_png(
        &authenticator_attachment,
        &transports,
        chose_security_key,
        vela_core::passkey::MarkPalette {
            strong: &strong,
            soft: &soft,
            hole: &hole,
        },
        size_px,
    )?)
}

/// The provider's brand name, or an empty string when the catalog has no entry.
/// The create view already carries this for its own key rows; this is for every
/// other surface that holds an AAGUID.
#[uniffi::export]
pub fn passkey_provider_name(aaguid: String) -> String {
    vela_core::passkey::provider_name(&aaguid)
        .unwrap_or_default()
        .to_owned()
}

/// The shared placeholder artwork as PNG bytes — what platforms show for an
/// invalid or empty seed instead of crashing or rendering blank.
#[uniffi::export]
pub fn identicon_placeholder_png(size_px: u32) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::identicon_raster::identicon_placeholder_png(
        size_px,
    )?)
}

/// Rasterize app-authored SVG markup (the spec 015 lucide icon corpus) to a
/// square PNG. For platforms without an SVG renderer; callers pass constant
/// markup with the tint pre-substituted (or white, tinted as a template image).
#[uniffi::export]
pub fn rasterize_svg_png(svg: String, size_px: u32) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::identicon_raster::rasterize_svg_png(
        &svg, size_px,
    )?)
}

// ---------------------------------------------------------------------------
// i18n (spec 004-rust-i18n, contracts/i18n-api.md §1.3 / §2.3)
// ---------------------------------------------------------------------------
//
// ONE record per call, ONE crossing per call. The measured FFI cost is 0.605 us
// per string-returning round trip, so a chatty per-option API — one crossing to
// set `count`, another for each variable — would cost roughly 12.7 ms for a
// 500-key screen, two orders of magnitude past SC-007's 0.5 ms budget. The record
// is the whole reason this surface looks like a struct rather than a builder.

/// Per-call translation options, mirroring the i18next object literal.
#[derive(Debug, Clone, Default, uniffi::Record)]
pub struct TOptions {
    /// Plural selector. `None` means no plural handling at all — which is also
    /// what a *string* count means upstream, so a caller that has a string should
    /// leave this unset rather than parsing it.
    pub count: Option<f64>,
    pub context: Option<String>,
    pub default_value: Option<String>,
    /// Per-call language override. **Not** the same code path as
    /// `change_language`: `zh_TW` resolves to `zh` through the latter and falls
    /// through to English here. That asymmetry is upstream's, and it is pinned by
    /// the conformance corpus.
    pub lng: Option<String>,
    pub ordinal: bool,
    /// Interpolation variables, already stringified by the caller.
    pub vars: Vec<TVar>,
}

/// One interpolation variable.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TVar {
    pub name: String,
    /// `None` renders as the empty string — matching an own property whose value
    /// is `undefined`. Omitting the entry entirely is different: the placeholder
    /// stays on screen as the literal `{{name}}`.
    pub value: Option<String>,
}

impl TOptions {
    fn to_owned_options(&self) -> vela_core::i18n::OwnedOptions {
        vela_core::i18n::OwnedOptions {
            count: self.count.map(vela_core::i18n::Count::Num),
            context: self.context.clone(),
            default_value: self.default_value.clone(),
            lng: self.lng.clone(),
            ordinal: self.ordinal,
            vars: self
                .vars
                .iter()
                .map(|v| {
                    let value = match &v.value {
                        Some(s) => vela_core::i18n::OwnedVar::Str(s.clone()),
                        None => vela_core::i18n::OwnedVar::Undefined,
                    };
                    (v.name.clone(), value)
                })
                .collect(),
            ..Default::default()
        }
    }
}

/// The resolve state after a language change.
#[derive(Debug, Clone, uniffi::Record)]
pub struct LanguageState {
    pub language: String,
    pub resolved_language: Option<String>,
    pub languages: Vec<String>,
}

/// A translation engine.
///
/// Wraps `RwLock` because `#[uniffi::export]` methods take `&self` while
/// `change_language` and `load_catalog` need `&mut`. Lock poisoning maps to
/// `CoreError::Internal` — never `unwrap()` a `LockResult`, which the crate lint
/// would reject anyway.
#[derive(uniffi::Object)]
pub struct I18n {
    inner: std::sync::RwLock<vela_core::i18n::I18n>,
}

fn lock_err<T>(_: T) -> CoreError {
    CoreError::Internal("i18n engine lock poisoned".to_owned())
}

#[uniffi::export]
impl I18n {
    /// Build an engine from the `en` fallback catalog, supplied as JSON bytes.
    ///
    /// JSON rather than a compiled-in catalog because that is the on-demand route
    /// (FR-015): a build carries the engine, and each locale arrives when the user
    /// picks it.
    #[uniffi::constructor]
    pub fn new(fallback_json: Vec<u8>) -> Result<Self, CoreError> {
        let en = vela_core::i18n::Catalog::from_json("en", &fallback_json)?;
        let engine = vela_core::i18n::I18n::new(en)?;
        Ok(Self {
            inner: std::sync::RwLock::new(engine),
        })
    }

    /// Build an engine pinned to the LEGACY plural rule — i18next's `dummyRule`,
    /// which is what a host without `Intl.PluralRules` silently falls back to.
    /// Exposed so the conformance corpus can replay MODE B here too; production
    /// code should never call it.
    #[uniffi::constructor]
    pub fn new_with_legacy_plurals(fallback_json: Vec<u8>) -> Result<Self, CoreError> {
        let en = vela_core::i18n::Catalog::from_json("en", &fallback_json)?;
        let engine =
            vela_core::i18n::I18n::new(en)?.with_plural_mode(vela_core::i18n::PluralMode::Legacy);
        Ok(Self {
            inner: std::sync::RwLock::new(engine),
        })
    }

    /// First key that resolves wins; all-missing returns the **last** key.
    pub fn t_first(&self, keys: Vec<String>, opts: TOptions) -> Result<String, CoreError> {
        let owned = opts.to_owned_options();
        let mut scratch = vela_core::i18n::Scratch::default();
        let borrowed = owned.as_options(&mut scratch);
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        Ok(self
            .inner
            .read()
            .map_err(lock_err)?
            .t_first(&refs, &borrowed)?)
    }

    /// Resolve `key`. Returns the key itself when nothing matches — i18next's
    /// behaviour, and not an error.
    pub fn t(&self, key: String, opts: TOptions) -> Result<String, CoreError> {
        let owned = opts.to_owned_options();
        let mut scratch = vela_core::i18n::Scratch::default();
        let borrowed = owned.as_options(&mut scratch);
        Ok(self.inner.read().map_err(lock_err)?.t(&key, &borrowed)?)
    }

    /// Whether `key` resolves to anything. A branch node counts as present.
    pub fn exists(&self, key: String, opts: TOptions) -> Result<bool, CoreError> {
        let owned = opts.to_owned_options();
        let mut scratch = vela_core::i18n::Scratch::default();
        let borrowed = owned.as_options(&mut scratch);
        Ok(self.inner.read().map_err(lock_err)?.exists(&key, &borrowed))
    }

    /// Set the active language. Does **not** load a catalog — the core has no I/O.
    pub fn change_language(&self, lng: String) -> Result<LanguageState, CoreError> {
        let s = self.inner.write().map_err(lock_err)?.change_language(&lng);
        Ok(LanguageState {
            language: s.language,
            resolved_language: s.resolved_language,
            languages: s.languages,
        })
    }

    /// Make `lang`'s catalog the active one, replacing whatever was active.
    pub fn load_catalog(&self, lang: String, json: Vec<u8>) -> Result<(), CoreError> {
        let catalog = vela_core::i18n::Catalog::from_json(&lang, &json)?;
        self.inner.write().map_err(lock_err)?.load_catalog(catalog);
        Ok(())
    }

    /// Release `lang` if it is the active catalog. Releasing `en` is not
    /// expressible — it is a field, not a slot.
    pub fn release_catalog(&self, lang: String) -> Result<bool, CoreError> {
        Ok(self
            .inner
            .write()
            .map_err(lock_err)?
            .release_catalog(&lang)
            .is_some())
    }

    pub fn resident_locales(&self) -> Result<Vec<String>, CoreError> {
        Ok(self
            .inner
            .read()
            .map_err(lock_err)?
            .resident_locales()
            .into_iter()
            .map(str::to_owned)
            .collect())
    }

    pub fn resident_bytes(&self) -> Result<u64, CoreError> {
        #[allow(clippy::cast_possible_truncation, clippy::allow_attributes)]
        Ok(self.inner.read().map_err(lock_err)?.resident_bytes() as u64)
    }

    pub fn language(&self) -> Result<String, CoreError> {
        Ok(self.inner.read().map_err(lock_err)?.language().to_owned())
    }

    /// Text direction of the active language, `"ltr"` or `"rtl"`.
    pub fn dir(&self) -> Result<String, CoreError> {
        Ok(self
            .inner
            .read()
            .map_err(lock_err)?
            .dir()
            .as_str()
            .to_owned())
    }

    /// The core's compact relative time in the active language — `"now"`,
    /// `"2m"`, `"3h"`, a short weekday under a week, else the date — for the
    /// home's "Updated <ago>" and anything else that says how long ago.
    ///
    /// `ts_seconds` is the moment in WHOLE SECONDS (`floor(at_ms / 1000)`;
    /// handing it milliseconds reads as the future, which is "now");
    /// `now_ms` the clock in milliseconds; `utc_offset_minutes` what to add
    /// to UTC for local time at that moment; `date_format` the person's date
    /// preset as stored (`ymd_slash`, `mdy_slash`, `dmy_slash`, `dmy_dot`,
    /// `iso`) with `auto` already resolved — an unknown word is `mdy_slash`.
    pub fn format_relative_time(
        &self,
        ts_seconds: i64,
        now_ms: i64,
        utc_offset_minutes: i32,
        date_format: String,
    ) -> Result<String, CoreError> {
        Ok(self.inner.read().map_err(lock_err)?.format_relative_time(
            ts_seconds,
            now_ms,
            utc_offset_minutes,
            vela_core::l10n::date_preset_of(&date_format),
        )?)
    }
}

// -- plural rules, exposed standalone so a platform can check a category --------

/// Interpolate a template in isolation, without a key lookup.
#[uniffi::export]
pub fn i18n_interpolate(template: String, opts: TOptions) -> Result<String, CoreError> {
    let owned = opts.to_owned_options();
    let mut scratch = vela_core::i18n::Scratch::default();
    let borrowed = owned.as_options(&mut scratch);
    Ok(vela_core::i18n::interpolate(&template, &borrowed)?)
}

#[uniffi::export]
pub fn i18n_plural_suffix(locale: String, count: f64) -> String {
    vela_core::i18n::plural_suffix(&locale, count)
}

#[uniffi::export]
pub fn i18n_plural_suffixes(locale: String) -> Vec<String> {
    vela_core::i18n::plural_suffixes(&locale)
}

/// What "follow the system" resolves to: the first of the platform's
/// preferred languages (most preferred first; BCP-47 or POSIX) a shipped
/// locale serves, else `en` (spec 095 — the rule iOS and Android each kept a
/// copy of).
#[uniffi::export]
pub fn i18n_system_language(preferred: Vec<String>) -> String {
    vela_core::i18n::system_language(&preferred).to_owned()
}

/// The Apple localization codes for the shipped locales, in their order
/// (spec 095) — what the iPhone app's `CFBundleLocalizations` must list.
#[uniffi::export]
pub fn i18n_apple_localizations() -> Vec<String> {
    vela_core::i18n::apple_localizations()
        .into_iter()
        .map(str::to_owned)
        .collect()
}

#[uniffi::export]
pub fn i18n_plural_suffix_legacy(count: f64) -> String {
    vela_core::i18n::plural_suffix_legacy(count)
}

#[uniffi::export]
pub fn i18n_plural_suffixes_legacy() -> Vec<String> {
    vela_core::i18n::plural_suffixes_legacy()
}

#[uniffi::export]
pub fn i18n_text_direction(lng: String) -> String {
    vela_core::l10n::text_direction(&lng).as_str().to_owned()
}

// -- amount fields (spec 073) --------------------------------------------------

/// An amount field's text as the core reads it — ASCII digits and one `.` —
/// or `None` for a paste with no reading as one figure (the field keeps
/// `previous`). `number` is the resolved preset key (`comma_dot`…);
/// `previous` the field's text before this edit; `pasted` `None` when the
/// shell cannot tell (a native field). `vela_core::l10n::amount_text` says why.
#[uniffi::export]
pub fn amount_text_clean(
    raw: String,
    number: String,
    previous: Option<String>,
    pasted: Option<bool>,
) -> Option<String> {
    use vela_core::l10n::amount_text;
    amount_text::clean(
        &raw,
        amount_text::preset_of(&number),
        amount_text::Entry::from_pasted(pasted),
        previous.as_deref(),
    )
}

/// Where the caret belongs in `clean`, having been at `caret` in `raw`
/// (UTF-16 units).
#[uniffi::export]
pub fn amount_text_caret(raw: String, clean: String, caret: u32) -> u32 {
    let at = vela_core::l10n::amount_text::caret_after_clean(&raw, &clean, caret as usize);
    u32::try_from(at).unwrap_or(u32::MAX)
}

// -- registry proofs (spec 019) -----------------------------------------------
//
// Returned as JSON strings rather than as uniffi records, deliberately. Both
// consumers of these values want JSON and want it in the SAME shape: the core
// takes the member proof back as part of a `member_proof_signed` shell result,
// and the registry's HTTP API takes it as a camelCase request body. A mirror
// record would mean two more FFI types on both platforms and a hand-written
// re-serialization on each — three ways for the field names to drift apart on
// the one payload where a wrong name means the server rejects a wallet the
// person has already minted every key for.

/// The uncompressed public key (`04‖x‖y` hex) of the one-time group key a
/// 32-byte `seed_hex` derives. Needed before the group's challenge can be
/// requested, because the contract binds the challenge to this key.
#[uniffi::export]
pub fn registry_group_public_key_from_seed(seed_hex: String) -> Result<String, CoreError> {
    Ok(vela_core::registry_proof::group_public_key_from_seed(
        &seed_hex,
    )?)
}

/// The group's closing proof, as `{"groupPublicKey": …, "proof": { … }}`.
#[uniffi::export]
pub fn registry_build_group_proof(
    seed_hex: String,
    rp_id: String,
    challenge_hex: String,
) -> Result<String, CoreError> {
    let proof = vela_core::registry_proof::build_group_proof(&seed_hex, &rp_id, &challenge_hex)?;
    serde_json::to_string(&proof)
        .map_err(|error| CoreError::Internal(format!("could not serialize group proof: {error}")))
}

/// One member's possession proof, as the registry's camelCase object.
#[uniffi::export]
pub fn registry_build_member_proof(
    authenticator_data_hex: String,
    client_data_json_hex: String,
    signature_der_hex: String,
) -> Result<String, CoreError> {
    let proof = vela_core::registry_proof::build_member_proof(
        &authenticator_data_hex,
        &client_data_json_hex,
        &signature_der_hex,
    )?;
    serde_json::to_string(&proof)
        .map_err(|error| CoreError::Internal(format!("could not serialize member proof: {error}")))
}

// ---------------------------------------------------------------------------
// Native-coin pricing (spec 041)
// ---------------------------------------------------------------------------
//
// These two are RULES, and they were reachable from the web (`vela-core-wasm`)
// but not from Swift or Kotlin — so each native client was one convenient
// afternoon away from writing its own price ladder, and two wallets would have
// disagreed about what a coin is worth. The desktop handover asked for exactly
// this promotion rather than a third copy.
//
// The shell still owns the multicall and the decoding; what crosses here is the
// judgement: which quote is deepest, and which source to believe.

/// One stable quote token's successful multicall outputs.
#[derive(uniffi::Record)]
pub struct NativeQuoteGroup {
    /// Quote outputs in THIS stable's base units, as decimal strings. Failed
    /// calls are simply absent — the shell drops them, it does not zero them.
    pub amounts_out: Vec<String>,
    /// This stable's `decimals()` read; `None` = the read failed (the core
    /// defaults it, and that default is its business).
    pub quote_decimals: Option<u32>,
}

/// The chosen price and where it came from.
#[derive(uniffi::Record)]
pub struct NativePriceChoice {
    /// `None` = nothing could price this coin. **Not zero, and not one.**
    pub price: Option<f64>,
    /// `dex` | `chainlink_sanity` | `chainlink_local` | `chainlink_eth` | `none`.
    pub source: String,
}

/// The deepest pool across every stable quote.
#[uniffi::export]
pub fn best_native_dex_price(groups: Vec<NativeQuoteGroup>) -> Option<f64> {
    let groups: Vec<vela_core::app::balance_dashboard::NativeQuoteGroup> = groups
        .into_iter()
        .map(
            |group| vela_core::app::balance_dashboard::NativeQuoteGroup {
                amounts_out: group.amounts_out,
                quote_decimals: group.quote_decimals,
            },
        )
        .collect();
    vela_core::app::balance_dashboard::best_native_dex_price(&groups)
}

/// The first usable price across quote groups — the CUSTOM-token rule.
///
/// Deliberately not [`best_native_dex_price`]: that one takes the deepest pool
/// across every stable, because a near-empty pool would otherwise price a
/// chain's own coin. This one walks the stables in the shell's preference order
/// and takes the first that answers, because for an arbitrary token the
/// preferred venue is the trustworthy one and a deeper pool elsewhere may be a
/// different asset wearing a similar ticker.
///
/// Each group is scaled by its OWN `decimals()`. Mixing them is a 10^12
/// mispricing on any chain carrying both a 6-decimal and an 18-decimal stable,
/// which is most of them.
#[uniffi::export]
pub fn first_grouped_quote_price(groups: Vec<NativeQuoteGroup>) -> Option<f64> {
    let groups: Vec<vela_core::app::balance_dashboard::NativeQuoteGroup> = groups
        .into_iter()
        .map(
            |group| vela_core::app::balance_dashboard::NativeQuoteGroup {
                amounts_out: group.amounts_out,
                quote_decimals: group.quote_decimals,
            },
        )
        .collect();
    vela_core::app::balance_dashboard::first_grouped_quote_price(&groups)
}

/// Whether the chain's "wrapped" native at `address` is the native itself
/// (spec 038, the founder's Celo report): on Celo the GoldToken IS the coin,
/// so a balance walk that lists both counts one holding twice — CELO 6.96 and
/// WCELO 6.96. Every shell asks here before adding the wrapped slot; the rule
/// lives in the core so four shells cannot drift on which chains it names.
#[uniffi::export]
pub fn wrapped_native_is_the_native(chain_id: u32, address: String) -> bool {
    vela_core::app::balance_dashboard::wrapped_native_is_the_native(chain_id, &address)
}

/// One balance a chain's read covers (spec 082 RE9).
#[derive(Debug, Clone, uniffi::Record)]
pub struct BalanceReadSlot {
    /// `native` (`eth_getBalance`, priced as the native), `stable` (a registry
    /// stablecoin: `balanceOf` + `decimals()`, worth `peg_usd`), `wrapped`
    /// (`balanceOf` + `decimals()`, priced as the native) or `custom` (the
    /// person's token: `balanceOf`, its saved decimals, priced by DEX).
    pub kind: String,
    /// The token contract; `None` for the native coin.
    pub contract: Option<String>,
    /// The registry's or the person's symbol; empty for `native` and
    /// `wrapped`, which the shell names from its own chain data.
    pub symbol: String,
    /// The person's saved name for a custom token (or one that overrode a
    /// registry entry); the symbol for a registry stablecoin; empty otherwise.
    pub name: String,
    /// `Some`: known, no `decimals()` read. `None`: read it on chain (the
    /// native coin takes the chain data's native decimals).
    pub known_decimals: Option<u32>,
    /// `Some(1.0)` for a registry stablecoin.
    pub peg_usd: Option<f64>,
}

/// Which balances one chain's read covers, in order: the native coin (unless
/// the chain has none — Tempo), the registry stablecoins, the wrapped native
/// (unless it IS the native), then the person's custom tokens; one slot per
/// contract, custom metadata winning. `stables_json` is the chain data's
/// `stables[]` (`[{"symbol","contract"}]`, other fields ignored);
/// `custom_json` this chain's saved tokens (`[{"contract","symbol","name",
/// "decimals"}]`). Blank text is an empty list; anything else that is not
/// that shape is an error.
#[uniffi::export]
pub fn balance_read_plan(
    chain_id: u32,
    stables_json: String,
    wrapped_native: Option<String>,
    custom_json: String,
) -> Result<Vec<BalanceReadSlot>, CoreError> {
    use vela_core::app::balance_dashboard::{read_plan, StableRef, TokenRef};
    fn list<T: serde::de::DeserializeOwned>(what: &str, json: &str) -> Result<Vec<T>, CoreError> {
        if json.trim().is_empty() {
            return Ok(Vec::new());
        }
        json_in(what, json)
    }
    let stables: Vec<StableRef> = list("stables_json", &stables_json)?;
    let custom: Vec<TokenRef> = list("custom_json", &custom_json)?;
    Ok(
        read_plan(chain_id, &stables, wrapped_native.as_deref(), &custom)
            .into_iter()
            .map(|slot| BalanceReadSlot {
                kind: snake_name(&slot.kind),
                contract: slot.contract,
                symbol: slot.symbol,
                name: slot.name,
                known_decimals: slot.known_decimals.map(u32::from),
                peg_usd: slot.peg_usd,
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// The submit spine (spec 043): a draft, its hash, its signature, its wire form
// ---------------------------------------------------------------------------
//
// Kotlin drives the ORDER (nonce and deployment reads, the estimate, the
// assertion, the submit and its retry) and passes the operation back and
// forth as a record; every byte of the operation is assembled here. No shell
// computes a hash, a leg, a limit or a signature.

/// One call of the batch: base units as a DECIMAL string, `0x`-hex data.
#[derive(Debug, Clone, uniffi::Record)]
pub struct UserOpCall {
    pub to: String,
    pub value: String,
    pub data: String,
}

/// One founding key of the wallet — `04‖x‖y` hex — with the credential that owns it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct WalletKeyRecord {
    pub credential_id: String,
    pub public_key_hex: String,
}

/// The operation as the shell carries it between steps. Gas fields are
/// DECIMAL strings; bytes are bytes. Opaque to the shell by contract.
#[derive(Debug, Clone, uniffi::Record)]
pub struct UserOpDraft {
    pub sender: String,
    pub nonce: String,
    pub init_code: Vec<u8>,
    pub call_data: Vec<u8>,
    pub verification_gas_limit: String,
    pub call_gas_limit: String,
    pub pre_verification_gas: String,
    pub max_fee_per_gas: String,
    pub max_priority_fee_per_gas: String,
    pub paymaster_and_data: Vec<u8>,
    pub signature: Vec<u8>,
}

/// What the batch's fee leg is: none (an estimate of the bare calls), the
/// in-band leg (native value or a stablecoin `transfer` to the relay's
/// recipient), or Tempo's stablecoin reimbursement to its collector.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum UserOpFeeMode {
    EstimateOnly,
    InBand {
        gas_fee_token: Option<String>,
        amount: String,
        recipient: String,
    },
    Tempo {
        fee_token: String,
        collector: String,
        reimbursement: String,
    },
}

/// The gas floors a path starts from (decimal strings).
#[derive(Debug, Clone, uniffi::Record)]
pub struct GasFloorsRecord {
    pub verification: String,
    pub call: String,
}

fn u128_of(text: &str, what: &str) -> Result<u128, CoreError> {
    text.trim().parse::<u128>().map_err(|_| {
        CoreError::InvalidQuantity(format!("{what} is not a base-unit integer: `{text}`"))
    })
}

fn floors_of(record: &GasFloorsRecord) -> Result<vela_core::user_op::GasFloors, CoreError> {
    Ok(vela_core::user_op::GasFloors {
        verification: u128_of(&record.verification, "verification floor")?,
        call: u128_of(&record.call, "call floor")?,
    })
}

fn draft_of(op: &vela_core::user_op::UserOperation) -> UserOpDraft {
    UserOpDraft {
        sender: op.sender.clone(),
        nonce: op.nonce.clone(),
        init_code: op.init_code.clone(),
        call_data: op.call_data.clone(),
        verification_gas_limit: op.verification_gas_limit.to_string(),
        call_gas_limit: op.call_gas_limit.to_string(),
        pre_verification_gas: op.pre_verification_gas.to_string(),
        max_fee_per_gas: op.max_fee_per_gas.to_string(),
        max_priority_fee_per_gas: op.max_priority_fee_per_gas.to_string(),
        paymaster_and_data: op.paymaster_and_data.clone(),
        signature: op.signature.clone(),
    }
}

fn op_of(draft: &UserOpDraft) -> Result<vela_core::user_op::UserOperation, CoreError> {
    Ok(vela_core::user_op::UserOperation {
        sender: draft.sender.clone(),
        nonce: draft.nonce.clone(),
        init_code: draft.init_code.clone(),
        call_data: draft.call_data.clone(),
        verification_gas_limit: u128_of(&draft.verification_gas_limit, "verificationGasLimit")?,
        call_gas_limit: u128_of(&draft.call_gas_limit, "callGasLimit")?,
        pre_verification_gas: u128_of(&draft.pre_verification_gas, "preVerificationGas")?,
        max_fee_per_gas: u128_of(&draft.max_fee_per_gas, "maxFeePerGas")?,
        max_priority_fee_per_gas: u128_of(&draft.max_priority_fee_per_gas, "maxPriorityFeePerGas")?,
        paymaster_and_data: draft.paymaster_and_data.clone(),
        signature: draft.signature.clone(),
    })
}

fn inner_calls(calls: &[UserOpCall]) -> Result<Vec<vela_core::user_op::MultiSendCall>, CoreError> {
    calls
        .iter()
        .map(|call| vela_core::user_op::to_multi_send_call(&call.to, &call.value, &call.data))
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into)
}

fn batch_for(
    inner: &[vela_core::user_op::MultiSendCall],
    fee: &UserOpFeeMode,
) -> Result<Vec<vela_core::user_op::MultiSendCall>, CoreError> {
    Ok(match fee {
        UserOpFeeMode::EstimateOnly => inner.to_vec(),
        UserOpFeeMode::InBand {
            gas_fee_token,
            amount,
            recipient,
        } => vela_core::user_op::in_band_batch(
            inner,
            gas_fee_token.as_deref(),
            recipient,
            u128_of(amount, "fee amount")?,
        )?,
        UserOpFeeMode::Tempo {
            fee_token,
            collector,
            reimbursement,
        } => vela_core::user_op::tempo_batch(
            inner,
            fee_token,
            collector,
            u128_of(reimbursement, "reimbursement")?,
        )?,
    })
}

/// The floors for a chain and a deployment state: the in-band pair, or
/// Tempo's (its call floor grows with the sub-call count — the person's
/// calls plus the reimbursement leg).
#[uniffi::export]
pub fn user_op_floors(chain_id: u32, deployed: bool, sub_calls: u32) -> GasFloorsRecord {
    let floors = if vela_core::app::fee_policy::is_tempo_chain(chain_id) {
        vela_core::user_op::GasFloors::tempo(
            deployed,
            vela_core::app::fee_policy::tempo_call_gas_limit(sub_calls),
        )
    } else {
        vela_core::user_op::GasFloors::in_band(deployed)
    };
    GasFloorsRecord {
        verification: floors.verification.to_string(),
        call: floors.call.to_string(),
    }
}

/// A draft operation: the batch (calls + the fee leg `fee` names) as MultiSend
/// calldata, the floors as limits, the estimation dummy as signature, and —
/// for an undeployed account — the initCode for its founding keys.
#[uniffi::export]
pub fn user_op_draft(
    sender: String,
    nonce: String,
    deployed: bool,
    key_hexes: Vec<String>,
    calls: Vec<UserOpCall>,
    fee: UserOpFeeMode,
    floors: GasFloorsRecord,
) -> Result<UserOpDraft, CoreError> {
    let init_code = if deployed {
        Vec::new()
    } else {
        vela_core::user_op::build_init_code_for_keys(&key_hexes)?
    };
    let inner = inner_calls(&calls)?;
    let batch = batch_for(&inner, &fee)?;
    let op = vela_core::user_op::draft_operation(
        &sender,
        &nonce,
        init_code,
        &batch,
        floors_of(&floors)?,
    )?;
    Ok(draft_of(&op))
}

/// The relay's raw estimate onto the draft: ×1.5 on the two limits, each held
/// to its floor, +10,000 on preVerificationGas.
#[uniffi::export]
pub fn user_op_apply_estimate(
    draft: UserOpDraft,
    verification_gas_limit: String,
    call_gas_limit: String,
    pre_verification_gas: String,
    floors: GasFloorsRecord,
) -> Result<UserOpDraft, CoreError> {
    let mut op = op_of(&draft)?;
    vela_core::user_op::apply_estimate(
        &mut op,
        vela_core::user_op::GasEstimate {
            verification_gas_limit: u128_of(&verification_gas_limit, "verificationGasLimit")?,
            call_gas_limit: u128_of(&call_gas_limit, "callGasLimit")?,
            pre_verification_gas: u128_of(&pre_verification_gas, "preVerificationGas")?,
        },
        floors_of(&floors)?,
    );
    Ok(draft_of(&op))
}

/// The batch with the SETTLED fee leg onto a draft whose gas the estimate
/// already sized: only the calldata changes.
#[uniffi::export]
pub fn user_op_with_calls(
    draft: UserOpDraft,
    calls: Vec<UserOpCall>,
    fee: UserOpFeeMode,
) -> Result<UserOpDraft, CoreError> {
    let mut op = op_of(&draft)?;
    let inner = inner_calls(&calls)?;
    vela_core::user_op::replace_calls(&mut op, &batch_for(&inner, &fee)?)?;
    Ok(draft_of(&op))
}

/// The SafeOp EIP-712 hash — the challenge the passkey signs.
#[uniffi::export]
pub fn user_op_safe_op_hash(draft: UserOpDraft, chain_id: u32) -> Result<Vec<u8>, CoreError> {
    Ok(vela_core::user_op::calculate_safe_op_hash(
        &op_of(&draft)?,
        u64::from(chain_id),
    )?)
}

/// The EntryPoint v0.7 `getUserOpHash` of the draft on `chain_id`, 0x-lower-case
/// (spec 082 RA6). The signature is not part of it, so it is known before the
/// passkey signs: a client computes it before the first submit POST and
/// follows the operation under it when the relay's reply is lost. When the
/// relay answers, the relay's hash wins.
#[uniffi::export]
pub fn user_op_hash(draft: UserOpDraft, chain_id: u32) -> Result<String, CoreError> {
    Ok(vela_core::user_op::user_op_hash(
        &op_of(&draft)?,
        u64::from(chain_id),
    )?)
}

/// The assertion as the operation's signature: compatibility-checked, DER →
/// raw low-S, the client-data fields cut out, the verifier named by the
/// credential that signed. A credential outside `keys` is an error.
#[uniffi::export]
pub fn user_op_sign(
    draft: UserOpDraft,
    assertion: WebAuthnAssertion,
    credential_id: String,
    keys: Vec<WalletKeyRecord>,
) -> Result<UserOpDraft, CoreError> {
    let keys: Vec<vela_core::user_op::WalletKey> = keys
        .into_iter()
        .map(|key| vela_core::user_op::WalletKey {
            credential_id: key.credential_id,
            public_key_hex: key.public_key_hex,
        })
        .collect();
    let mut op = op_of(&draft)?;
    op.signature = vela_core::user_op::envelope_signature(
        &assertion.authenticator_data,
        &assertion.client_data_json,
        &assertion.signature_der,
        &credential_id,
        &keys,
    )?;
    Ok(draft_of(&op))
}

/// The v0.7 JSON-RPC dictionary the relay takes (`factory`/`factoryData`
/// split out), plus Tempo's `feeToken` when given.
#[uniffi::export]
pub fn user_op_relay_json(
    draft: UserOpDraft,
    fee_token: Option<String>,
) -> Result<String, CoreError> {
    let op = op_of(&draft)?;
    let extra: Vec<(&str, &str)> = fee_token
        .as_deref()
        .map(|token| vec![("feeToken", token)])
        .unwrap_or_default();
    Ok(vela_core::user_op::user_op_to_json(&op, &extra).to_string())
}

/// Whether any call is more than a plain transfer — then a failed estimate
/// must refuse rather than submit with the defaults.
#[uniffi::export]
pub fn user_op_has_contract_call(calls: Vec<UserOpCall>) -> Result<bool, CoreError> {
    Ok(vela_core::user_op::batch_has_contract_call(&inner_calls(
        &calls,
    )?))
}

/// The calls a shell must measure on their own (`eth_estimateGas` from the
/// Safe's address) before submitting: every one that is more than a plain
/// transfer, by index into `calls`. Empty = nothing to measure.
#[uniffi::export]
pub fn user_op_calls_to_measure(calls: Vec<UserOpCall>) -> Result<Vec<u32>, CoreError> {
    Ok(inner_calls(&calls)?
        .iter()
        .enumerate()
        .filter(|(_, call)| !vela_core::user_op::is_plain_transfer_call(&call.data))
        .map(|(index, _)| index as u32)
        .collect())
}

/// The inner calls' own gas floor (`vela_core::user_op::inner_calls_gas_floor`):
/// `measured` are the shell's `eth_estimateGas` figures for the calls
/// `user_op_calls_to_measure` named, as decimal strings; `call_count` is every
/// inner call. The draft's `call_gas_limit` is raised to the result when it is
/// higher — an undeployed Safe's first contract call must not go out with the
/// bundler's trivial "no code here" estimate. `None` = nothing to raise.
#[uniffi::export]
pub fn user_op_raise_call_gas(
    draft: UserOpDraft,
    measured: Vec<String>,
    call_count: u32,
) -> Result<UserOpDraft, CoreError> {
    let measured: Vec<u128> = measured
        .iter()
        .map(|gas| u128_of(gas, "measured gas"))
        .collect::<Result<_, _>>()?;
    let Some(floor) = vela_core::user_op::inner_calls_gas_floor(&measured, call_count as usize)
    else {
        return Ok(draft);
    };
    let mut op = op_of(&draft)?;
    if floor > op.call_gas_limit {
        op.call_gas_limit = floor;
    }
    Ok(draft_of(&op))
}

/// A displayed quote is usable when it is positive and names a real address.
#[uniffi::export]
pub fn quoted_fee_usable(amount: String, recipient: String) -> bool {
    amount
        .trim()
        .parse::<u128>()
        .is_ok_and(|amount| vela_core::user_op::quoted_fee_usable(amount, &recipient))
}

/// The relay's `[existingHash:0x…]` marker: a previous operation is still
/// pending, and this is its hash to poll instead of failing.
#[uniffi::export]
pub fn parse_existing_user_op_hash(message: String) -> Option<String> {
    vela_core::user_op::parse_existing_user_op_hash(&message)
}

/// `parseBundlerUnderfunded`: the relay saying the per-Safe gas account is short.
#[uniffi::export]
pub fn is_bundler_underfunded(message: String) -> bool {
    vela_core::user_op::is_bundler_underfunded(&message)
}

/// Why the relay refused a submit, on the axis the send machine speaks.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum RelayRejection {
    RelayerUnavailable,
    BundlerUnderfunded,
    /// Another operation of the account holds the nonce (083): its hash is
    /// never this request's answer.
    NonceHeld {
        user_op_hash: String,
    },
    Other {
        message: String,
    },
}

impl From<vela_core::user_op::RelayRejection> for RelayRejection {
    fn from(rejection: vela_core::user_op::RelayRejection) -> Self {
        use vela_core::user_op::RelayRejection as R;
        match rejection {
            R::RelayerUnavailable => RelayRejection::RelayerUnavailable,
            R::BundlerUnderfunded => RelayRejection::BundlerUnderfunded,
            R::NonceHeld { user_op_hash } => RelayRejection::NonceHeld { user_op_hash },
            R::Other(message) => RelayRejection::Other { message },
        }
    }
}

/// The relay's sentence, classified the way `classifySubmit` does.
#[uniffi::export]
pub fn classify_relay_rejection(message: String) -> RelayRejection {
    vela_core::user_op::classify_relay_rejection(&message).into()
}

/// `parseBundlerError`: the JSON-RPC `error` member as one sentence.
#[uniffi::export]
pub fn relay_error_message(error_json: String) -> String {
    vela_core::user_op::relay_error_message(&error_json)
}

// -- the submit verdict (spec 082 RA1, contract §1) ---------------------------
//
// One rule for every client's submit loop: POST → OR the pool's
// `maybe_delivered` into a sticky flag → `user_op_submit_step` → wait and POST
// the SAME operation again, or done. The local nonce advances only on
// `Accepted`; `MaybeSent` is followed under the local hash and is never
// "try again" (owner ruling 1).

/// What one `eth_sendUserOperation` POST came back with, as the pool
/// concluded it.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum UserOpSubmitReply {
    /// A JSON-RPC `result` — the relay's operation hash.
    Hash { hash: String },
    /// A JSON-RPC `error` member, as JSON text.
    RpcError { error_json: String },
    /// The pool gave up: no endpoint answered with JSON.
    NoAnswer,
}

impl From<UserOpSubmitReply> for vela_core::user_op::SubmitReply {
    fn from(reply: UserOpSubmitReply) -> Self {
        match reply {
            UserOpSubmitReply::Hash { hash } => Self::Hash(hash),
            UserOpSubmitReply::RpcError { error_json } => Self::Error(error_json),
            UserOpSubmitReply::NoAnswer => Self::NoAnswer,
        }
    }
}

/// One step of the submit loop: a verdict, or wait `delay_ms` and POST the
/// identical operation again.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum UserOpSubmitStep {
    /// The relay holds the operation; its hash wins over the local one.
    Accepted {
        user_op_hash: String,
    },
    /// A request may have reached the relay and its answer never came back:
    /// followed under the LOCAL hash (`OpSubmitted{maybe_sent: true}`).
    MaybeSent {
        user_op_hash: String,
    },
    /// Nothing left the device (`rejection` `None`: the relay was never
    /// reached — the dApp's detail is [`user_op_not_sent_detail`]), or the
    /// relay refused it and no earlier attempt can have delivered it.
    NotSent {
        rejection: Option<RelayRejection>,
    },
    RetryAfter {
        delay_ms: u32,
    },
}

impl From<vela_core::user_op::SubmitStep> for UserOpSubmitStep {
    fn from(step: vela_core::user_op::SubmitStep) -> Self {
        use vela_core::user_op::{SubmitStep, SubmitVerdict};
        match step {
            SubmitStep::RetryAfter { delay_ms } => UserOpSubmitStep::RetryAfter { delay_ms },
            SubmitStep::Done(SubmitVerdict::Accepted { user_op_hash }) => {
                UserOpSubmitStep::Accepted { user_op_hash }
            }
            SubmitStep::Done(SubmitVerdict::MaybeSent { user_op_hash }) => {
                UserOpSubmitStep::MaybeSent { user_op_hash }
            }
            SubmitStep::Done(SubmitVerdict::NotSent { rejection }) => UserOpSubmitStep::NotSent {
                rejection: rejection.map(Into::into),
            },
        }
    }
}

/// Decide one submit POST. `attempt` is the 0-based count of POSTs of this
/// operation, the one just answered included; `maybe_delivered` the OR, over
/// every POST of it so far, of the pool verdict's `maybe_delivered`;
/// `local_hash` is [`user_op_hash`] of the operation. See
/// `vela_core::user_op::submit_step` for the rules.
#[uniffi::export]
pub fn user_op_submit_step(
    reply: UserOpSubmitReply,
    attempt: u32,
    maybe_delivered: bool,
    local_hash: String,
) -> UserOpSubmitStep {
    vela_core::user_op::submit_step(&reply.into(), attempt, maybe_delivered, &local_hash).into()
}

/// The dApp's `-32603` detail when nothing was sent and the relay gave no
/// refusal to quote (RA10): a fixed sentence, never the pool's raw text.
#[uniffi::export]
pub fn user_op_not_sent_detail() -> String {
    vela_core::user_op::NOT_SENT_DAPP_DETAIL.to_owned()
}

/// The dApp's `-32603` detail for a request that could not go out behind
/// another of the account's operations (083, `RelayRejection::NonceHeld`) —
/// never that operation's hash as this one's answer.
#[uniffi::export]
#[must_use]
pub fn user_op_previous_pending_detail() -> String {
    vela_core::user_op::PREVIOUS_PENDING_DETAIL.to_owned()
}

/// The dApp's `-32603` detail when the relay refused the operation (spec 082
/// RJ3): the tracker's `rejected`, or a submit-time not-sent with a rejection
/// that is not "relayer unavailable". A fixed sentence.
#[uniffi::export]
pub fn user_op_refused_dapp_detail() -> String {
    vela_core::user_op::REFUSED_DAPP_DETAIL.to_owned()
}

/// How long a shell waits for the core's `ClearToPost` after `OpSigned`
/// before it gives up without POSTing (spec 082 RJ1), in ms.
#[uniffi::export]
pub fn user_op_write_ahead_wait_ms() -> u32 {
    vela_core::user_op::WRITE_AHEAD_WAIT_MS
}

/// A failed relay gas estimate, as the core reads it (spec 082 RJ19).
#[derive(Debug, Clone, uniffi::Record)]
pub struct UserOpEstimateFailure {
    /// `reverts` (the relay simulated the call and it reverts: warn with
    /// `componentsUi.signing.simWillFail` / `simWillFailReason`) or
    /// `unavailable` (nothing is known about the call).
    pub kind: String,
    /// The decoded, sanitised revert reason, when `reverts` carried one.
    pub reason: Option<String>,
}

/// Classify the relay's answer to a failed gas estimate: `error_json` is the
/// JSON-RPC `error` member (or the whole body); anything else is no answer.
/// See `vela_core::user_op::estimate_failure`.
#[uniffi::export]
pub fn user_op_estimate_failure(error_json: String) -> UserOpEstimateFailure {
    use vela_core::user_op::{estimate_failure, EstimateFailure};
    match estimate_failure(&error_json) {
        EstimateFailure::Reverts { reason } => UserOpEstimateFailure {
            kind: "reverts".to_owned(),
            reason,
        },
        EstimateFailure::Unavailable => UserOpEstimateFailure {
            kind: "unavailable".to_owned(),
            reason: None,
        },
    }
}

/// The relay method the tracker's `PollStatus` asks
/// (`pimlico_getUserOperationStatus`, RA7).
#[uniffi::export]
pub fn user_op_status_method() -> String {
    vela_core::app::tx_tracker::USER_OP_STATUS_METHOD.to_owned()
}

/// One parsed answer of [`user_op_status_method`] — the fields of the
/// tracker's `Status` shell result.
#[derive(Debug, Clone, uniffi::Record)]
pub struct TrackStatusAnswer {
    /// The `TrackLifecycle` wire name: `not_found | queued | not_submitted |
    /// submitted | rejected | included | failed`.
    pub status: String,
    /// The executor stage that last touched the op (`last_executor_stage`).
    pub stage: Option<String>,
    /// The bundle transaction the relay names, when it has one.
    pub tx_hash: Option<String>,
}

/// The relay's status answer — its `result`, or the whole JSON-RPC body — or
/// `None` when it is not one: an error body, a missing or unknown `status`.
/// The one parser every client uses; an unknown status is never guessed at.
#[uniffi::export]
pub fn parse_user_op_status(json: String) -> Option<TrackStatusAnswer> {
    vela_core::app::tx_tracker::parse_user_op_status(&json).map(|answer| TrackStatusAnswer {
        status: snake_name(&answer.status),
        stage: answer.stage,
        tx_hash: answer.tx_hash,
    })
}

/// The origin of a page's URL, normalised the way the browser normalises
/// it (spec 044): the key a grant is stored under, and the one fact about
/// a page the shell attaches to every request. `None` for anything that
/// is not an http(s) URL.
#[uniffi::export]
pub fn dapp_origin_of(url: String) -> Option<String> {
    vela_core::app::dapp_permissions::origin_of(&url)
}

/// Is this provider method one that asks for a signature? The routing
/// table's first question (spec 044), answered by the core so the shell's
/// allowlist and the machine's own notion of "a signing method" cannot drift.
#[uniffi::export]
pub fn dapp_is_signing_method(method: String) -> bool {
    vela_core::app::sign_request::is_signing_method(&method)
}

// -- the ending of a request (spec 082 RA8, RA12, contract §4) -----------------
//
// JSON in and out, deliberately: the ending travels from `sign_ending_of` to
// `sign_ending_state` unchanged, and both it and the state are tagged enums
// the hand-written `SignWire` mirrors already decode (with a wire round-trip
// test), in the same serde shape the web reads over wasm.

fn json_in<T: serde::de::DeserializeOwned>(what: &str, json: &str) -> Result<T, CoreError> {
    serde_json::from_str(json).map_err(|error| CoreError::Internal(format!("{what}: {error}")))
}

fn json_out<T: serde::Serialize>(what: &str, value: &T) -> Result<String, CoreError> {
    serde_json::to_string(value)
        .map_err(|error| CoreError::Internal(format!("could not serialize {what}: {error}")))
}

/// What the answer that went to the page stands for — a `SignEnding` as JSON
/// (`{"type":"signed"}`, `{"type":"landed","tx_hash",…,"user_op_hash"}`,
/// `{"type":"still_confirming","user_op_hash"}`) — or `None` when there is
/// nothing to show (a refusal, an empty answer). `payload_json` is the
/// `SignResponsePayload` the core's `Respond` carried; `submitted_user_op` the
/// op this request handed the tracker. Replaces each shell's own derivation.
#[uniffi::export]
pub fn sign_ending_of(
    method: String,
    payload_json: String,
    submitted_user_op: Option<String>,
) -> Result<Option<String>, CoreError> {
    let payload: vela_core::app::sign_request::SignResponsePayload =
        json_in("payload_json", &payload_json)?;
    vela_core::app::sign_request::ending_of(&method, &payload, submitted_user_op.as_deref())
        .map(|ending| json_out("the sign ending", &ending))
        .transpose()
}

/// What the sheet draws for an ending once the tracker has had its say — a
/// `SignEndingState` as JSON (`signed | confirmed | reverted | not_sent |
/// following{user_op_hash, outcome, fee_held}`). `ending_json` is what
/// [`sign_ending_of`] returned; `track_entry_json` the tracker's
/// `TrackEntryView` for the op, `None` (or `null`) while the tracker has not
/// taken it — that reads `following` / `landing`. A landed or still-confirming
/// request is never drawn confirmed until the tracker says so (W3).
#[uniffi::export]
pub fn sign_ending_state(
    ending_json: String,
    track_entry_json: Option<String>,
) -> Result<String, CoreError> {
    use vela_core::app::{sign_request, tx_tracker::TrackEntryView};
    let ending: sign_request::SignEnding = json_in("ending_json", &ending_json)?;
    let entry: Option<TrackEntryView> = match track_entry_json.as_deref() {
        Some(json) => json_in("track_entry_json", json)?,
        None => None,
    };
    json_out(
        "the sign ending state",
        &sign_request::ending_state(&ending, entry.as_ref()),
    )
}

/// A dApp record's stored request as Technical details shows it (spec 093):
/// typed data pretty-printed, a message as its text (or its hex), call data
/// pretty-printed. `content` is the detail's `content` word (`"call_data"`,
/// `"typed_data"`, `"message"`); `stored_request` the params' JSON text as the
/// record kept it. `None` when the record kept nothing, or for a content word
/// this build does not know. One rule for every client's detail.
#[uniffi::export]
pub fn dapp_request_display(content: String, stored_request: String) -> Option<String> {
    let content = serde_json::from_value(serde_json::Value::String(content)).ok()?;
    vela_core::app::dapp_activity::request_display(content, &stored_request)
}

/// How long to wait for the receipt when the submit answered `elapsed_ms`
/// after the approve tap: what is left of the 120 s answer window, never less
/// than 10 s (RA12). One number for every client's dApp wait.
#[uniffi::export]
pub fn dapp_receipt_wait_ms(elapsed_ms: f64) -> f64 {
    vela_core::app::sign_request::dapp_receipt_wait_ms(elapsed_ms)
}

/// The receipt verdict a tracker entry stands for, for the send machine's
/// `ReceiptUpdate` — a `SendReceiptOutcome` as JSON (`confirmed{tx_hash}`,
/// `failed{rejected, not_sent}`, `fee_held`, `acknowledged`) — or `None` while
/// there is nothing new to say: a slow, unreachable, maybe-sent or 24 h-old op
/// sends nothing, so only a definitive drop, rejection or never-sent is a
/// failure. `track_entry_json` is the tracker's `TrackEntryView`. The one
/// mapping (`vela_core::app::send::receipt_outcome_of`), where each shell had
/// its own.
#[uniffi::export]
pub fn send_receipt_outcome_of(track_entry_json: String) -> Result<Option<String>, CoreError> {
    let entry: vela_core::app::tx_tracker::TrackEntryView =
        json_in("track_entry_json", &track_entry_json)?;
    vela_core::app::send::receipt_outcome_of(&entry)
        .map(|outcome| json_out("the receipt outcome", &outcome))
        .transpose()
}

// -- what a simulation says (spec 082 RG6, RG8, contract §8) ------------------

/// A simulation's outcome and the line the signing sheet draws for it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct SimOutcomeRecord {
    /// `deltas | reverts | not_offered | unreachable`. Only `deltas` means
    /// the node checked the transaction.
    pub kind: String,
    /// The user's signed per-asset moves (`TrustAssetDelta[]` JSON, the input
    /// `token_trust` takes). `[]` with `kind == "deltas"` is "checked, nothing
    /// of theirs moves"; every other kind carries `[]` too and means nothing.
    pub deltas_json: String,
    /// The sanitised `Error(string)` of a revert, ≤ 64 characters — the
    /// `{{reason}}` of `notice_key`. Untrusted text made safe to print.
    pub revert_reason: Option<String>,
    /// The notice's tone, a `ClearRisk` wire name (`danger` for a revert,
    /// `caution` for could-not-check); `None` for `deltas`.
    pub notice_risk: Option<String>,
    /// The notice's corpus key; `None` for `deltas`.
    pub notice_key: Option<String>,
}

/// What the pool's `eth_simulateV1` answer means for `user` (the signing
/// account). `reply_json` is the JSON-RPC envelope as it came —
/// `{"result": …}` or `{"error": {"code", "message"}}` — or
/// `{"unreachable": true}` when the pool gave up; anything else is a node
/// answer nobody can read ("could not check", never "nothing moves"). A
/// revert is a danger and a node that could not check is a caution, on every
/// client.
#[uniffi::export]
pub fn sim_outcome(user: String, reply_json: String) -> SimOutcomeRecord {
    use vela_core::app::sim_outcome::{classify, notice, SimOutcome, SimReply};
    let outcome = classify(SimReply::from_json(&reply_json), &user);
    let deltas: &[vela_core::app::token_trust::TrustAssetDelta] = match &outcome {
        SimOutcome::Deltas { deltas } => deltas,
        _ => &[],
    };
    // A list that cannot be written out is an answer nobody can read: the
    // could-not-check line, never an empty list that reads "nothing moves".
    let (outcome, deltas_json) = match serde_json::to_string(deltas) {
        Ok(json) => (outcome, json),
        Err(_) => (SimOutcome::NotOffered, "[]".to_owned()),
    };
    let notice = notice(&outcome);
    SimOutcomeRecord {
        kind: serde_json::to_value(&outcome)
            .ok()
            .and_then(|value| value.get("kind")?.as_str().map(str::to_owned))
            .unwrap_or_default(),
        deltas_json,
        revert_reason: match &outcome {
            SimOutcome::Reverts { reason } => reason.clone(),
            _ => None,
        },
        notice_risk: notice.as_ref().map(|n| snake_name(&n.risk)),
        notice_key: notice.map(|n| n.key.to_owned()),
    }
}

/// The whole document-start script an in-app browser injects (spec 070):
/// THE provider (`vela-core/provider/inpage.js`, the extension's too) and
/// the one bridge. `host` is `"android"`, `"ios"` or `"desktop"` — the only
/// difference is how the bridge hands a string to native code.
/// `debug_mode` is Settings' (spec 091): with it on, the script also offers
/// the wallet to http pages on this device's own network.
#[uniffi::export]
pub fn dapp_provider_script(host: String, debug_mode: bool) -> String {
    use vela_core::app::dapp_rpc::{provider_script, ProviderHost};
    provider_script(
        match host.as_str() {
            "ios" => ProviderHost::Ios,
            "desktop" => ProviderHost::Desktop,
            _ => ProviderHost::Android,
        },
        debug_mode,
    )
}

/// Whether a page at `origin` is offered the wallet (spec 091): a secure
/// context, or — with debug mode on — http on this device's own network.
/// The rule the browser machine's gate and the injected script follow.
#[uniffi::export]
pub fn dapp_offers_wallet(origin: String, debug_mode: bool) -> bool {
    vela_core::app::dapp_permissions::offers_wallet(&origin, debug_mode)
}

/// Address-bar text → the URL to load: `https://` for a host, `http://` only
/// for loopback / private-network hosts, a DuckDuckGo search for anything
/// else (spec 070 research R6). `None` for blank input.
#[uniffi::export]
pub fn dapp_browser_input(text: String) -> Option<String> {
    vela_core::app::dapp_rpc::browser_input(&text)
}

/// A page another app or site asked the wallet's browser to open
/// (`velawallet://open?url=…`, spec 088): the host the person is asked about
/// before it loads, or `None` when the link is not opened at all (only an
/// `https` page with a plain host qualifies).
#[uniffi::export]
pub fn dapp_external_page_host(url: String) -> Option<String> {
    vela_core::app::dapp_rpc::external_page_host(&url)
}

/// A main-frame load failure as every browser shell draws it (spec 079):
/// `class` is one of `offline | timeout | not_found | refused | certificate |
/// other | proxy` (`proxy` since spec 082: the proxy itself could not be used,
/// `explore.loadProxy`), `reason_key` the corpus key of the panel's sentence.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BrowserLoadFailure {
    pub class: String,
    pub reason_key: String,
    pub auto_retry: bool,
}

/// A visit for Recents, from one document (spec 079).
#[derive(Debug, Clone, uniffi::Record)]
pub struct BrowserVisit {
    pub url: String,
    pub title: Option<String>,
    pub favicon: Option<String>,
}

fn snake<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

fn snake_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}

impl From<vela_core::app::browser_load::LoadFailure> for BrowserLoadFailure {
    fn from(failure: vela_core::app::browser_load::LoadFailure) -> Self {
        BrowserLoadFailure {
            class: snake_name(&failure.class),
            reason_key: failure.reason_key,
            auto_retry: failure.auto_retry,
        }
    }
}

/// The platform's raw load error → the one failure every shell shows, or
/// `None` when it is not a failure (a cancelled navigation). `platform` is
/// `"android"`, `"apple"`, `"probe"` or `"webview2"` (spec 083); `domain` is
/// the `NSError` domain on
/// Apple; `certificate` is set when the failure came from a certificate
/// callback rather than an error code. Apple `kCFErrorDomainCFNetwork`
/// 306–310, Android -5 and probe code 6 are `proxy`; a proxy that answered
/// for the host keeps the host's class.
#[uniffi::export]
pub fn browser_load_classify(
    platform: String,
    code: i64,
    domain: Option<String>,
    certificate: bool,
) -> Option<BrowserLoadFailure> {
    use vela_core::app::browser_load::{classify, LoadPlatform};
    let platform: LoadPlatform = snake(&platform)?;
    classify(platform, code, domain.as_deref(), certificate).map(Into::into)
}

/// The wait before automatic attempt `attempt` (1-based) of a failed page
/// load of `class`, or `None` to stop.
#[uniffi::export]
pub fn browser_load_retry_delay_ms(class: String, attempt: u32) -> Option<u32> {
    use vela_core::app::browser_load::{retry_delay_ms, LoadFailureClass};
    retry_delay_ms(snake::<LoadFailureClass>(&class)?, attempt)
}

/// The visit a finished load is, or `None` (a failed load, an error status,
/// an engine document). All fields from ONE read of the page itself.
#[uniffi::export]
pub fn browser_load_visit(
    url: String,
    title: String,
    icon: Option<String>,
    main_frame_failed: bool,
    http_status: Option<u16>,
) -> Option<BrowserVisit> {
    use vela_core::app::browser_load::{visit_to_record, LoadFinished};
    visit_to_record(LoadFinished {
        url,
        title,
        icon,
        main_frame_failed,
        http_status,
    })
    .map(|visit| BrowserVisit {
        url: visit.url,
        title: visit.title,
        favicon: visit.favicon,
    })
}

/// A site avatar's letter: `app.uniswap.org` → "U".
#[uniffi::export]
pub fn browser_site_letter(host: String) -> String {
    vela_core::app::browser_load::site_letter(&host)
}

// -- balance rounds (spec 092) --

/// How long one chain's balance read may take before the round gives up on
/// it and counts that chain failed — `balance_dashboard::CHAIN_READ_DEADLINE_MS`.
#[uniffi::export]
pub fn balance_chain_read_deadline_ms() -> u32 {
    vela_core::app::balance_dashboard::CHAIN_READ_DEADLINE_MS
}

// -- page loads, the address bar, a site named once (spec 082, contract §10) --

/// A load that has not committed by now is given up on, on every client,
/// unless the engine shows it getting somewhere (20 000 ms).
#[uniffi::export]
pub fn browser_load_give_up_ms() -> u32 {
    vela_core::app::browser_load::GIVE_UP_MS
}

/// Whether a load under way for `elapsed_ms` is given up on: the give-up
/// time or more, nothing committed, and the engine's `progress` (0.0 – 1.0:
/// iOS `estimatedProgress`, Android `getProgress() / 100`) never past 0.15 —
/// a slow but answering site is never cut.
#[uniffi::export]
pub fn browser_load_should_give_up(elapsed_ms: u32, committed: bool, progress: f64) -> bool {
    vela_core::app::browser_load::should_give_up(elapsed_ms, committed, progress)
}

/// What a given-up load is: `timeout`, the network sentence and the network
/// retry schedule, whichever client timed it.
#[uniffi::export]
pub fn browser_load_stalled() -> BrowserLoadFailure {
    vela_core::app::browser_load::stalled().into()
}

/// Whether a failed page of `class` is loaded again when the network comes
/// back (`net_health_step`'s `came_back`): `offline | timeout | refused |
/// other | proxy`. A name that does not resolve, a wrong certificate and a
/// class this core does not know are not. A load the page itself started is
/// retried by hand only, whatever its class.
#[uniffi::export]
pub fn browser_load_retry_when_network_returns(class: String) -> bool {
    use vela_core::app::browser_load::{retry_when_network_returns, LoadFailureClass};
    snake::<LoadFailureClass>(&class).is_some_and(retry_when_network_returns)
}

/// What the address bar shows.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BrowserAddressBar {
    /// The address share, copy, favourite and the edit field act on; empty
    /// when the bar names nothing.
    pub url: String,
    /// Lower-case host, a non-default port kept; empty when nothing.
    pub host: String,
    /// `closed` (https, or http on loopback / a private network), `open`
    /// (public http) or `none` (a failure panel, a pending load, an empty tab).
    pub lock: String,
}

/// What the bar names (RE1, G28), first match: a failure panel is up → the
/// failed host, no lock; a committed document → its host and lock; a load
/// pending in an EMPTY tab → the pending host, no lock; else nothing. A
/// pending load never renames a tab that shows a document. `shown` is the
/// committed document (the commit callback), never the engine's current URL.
#[uniffi::export]
pub fn browser_address_bar(
    shown: Option<String>,
    pending: Option<String>,
    failed: Option<String>,
) -> BrowserAddressBar {
    let bar = vela_core::app::browser_load::address_bar(
        shown.as_deref(),
        pending.as_deref(),
        failed.as_deref(),
    );
    BrowserAddressBar {
        url: bar.url,
        host: bar.host,
        lock: snake_name(&bar.lock),
    }
}

/// A site's name and the host line under it.
#[derive(Debug, Clone, uniffi::Record)]
pub struct BrowserSiteLabel {
    pub name: String,
    /// `None`: the name already is the host — say it once.
    pub host_line: Option<String>,
}

/// A name that is its host is said once (RE7): an empty title, or one equal to
/// the host ignoring ASCII case, → the host alone; otherwise the title over
/// the host. Recents, the signing header and the consent sheet.
#[uniffi::export]
pub fn browser_site_label(title: String, host: String) -> BrowserSiteLabel {
    let label = vela_core::app::browser_load::site_label(&title, &host);
    BrowserSiteLabel {
        name: label.name,
        host_line: label.host_line,
    }
}

/// The title a page is pinned under as a favourite (spec 086, issue #329):
/// `last_good`'s title when it is the same site as `url` — the bar's address,
/// the failed one under a failure panel — else `None`, and the favourite takes
/// its host. `last_good` is the last visit `browser_load_visit` made: an
/// engine's error page never is one.
#[uniffi::export]
pub fn browser_pinned_title(url: String, last_good: Option<BrowserVisit>) -> Option<String> {
    let visit = last_good.map(|visit| vela_core::app::browser_load::Visit {
        url: visit.url,
        title: visit.title,
        favicon: visit.favicon,
    });
    vela_core::app::browser_load::pinned_title(&url, visit.as_ref())
}

// -- network health and logo misses (spec 082 RE3, RE10, contract §11) --------

/// The network count so far (spec 082 RE3, RJ14) — the shell keeps it and
/// hands it back on every call. A fresh app: [`net_health_fresh`].
#[derive(Debug, Clone, PartialEq, uniffi::Record)]
pub struct NetHealthState {
    /// Calls in a row that never reached a server.
    pub misses: u32,
    pub online: bool,
    /// The distinct chains the current run of misses came from.
    pub sources: Vec<u32>,
    /// Misses in the current run that named no chain (each its own source).
    pub unsourced: u32,
    /// When a call last reached a server (epoch ms).
    pub last_reach_ms: Option<f64>,
    /// When the current run of misses began (epoch ms).
    pub run_started_ms: Option<f64>,
}

impl From<vela_core::app::net_health::NetHealth> for NetHealthState {
    fn from(state: vela_core::app::net_health::NetHealth) -> Self {
        NetHealthState {
            misses: state.misses,
            online: state.online,
            sources: state.sources,
            unsourced: state.unsourced,
            last_reach_ms: state.last_reach_ms,
            run_started_ms: state.run_started_ms,
        }
    }
}

impl From<NetHealthState> for vela_core::app::net_health::NetHealth {
    fn from(state: NetHealthState) -> Self {
        vela_core::app::net_health::NetHealth {
            misses: state.misses,
            online: state.online,
            sources: state.sources,
            unsourced: state.unsourced,
            last_reach_ms: state.last_reach_ms,
            run_started_ms: state.run_started_ms,
        }
    }
}

/// One call's outcome applied to the network count.
#[derive(Debug, Clone, uniffi::Record)]
pub struct NetHealthStep {
    /// The next state — hand it to the next call.
    pub state: NetHealthState,
    /// The edge this call crossed: `went_offline` (three misses in a row from
    /// at least two sources, with nothing answering for 10 s) or `came_back`
    /// (the first answer after it); `None` while the state holds.
    pub edge: Option<String>,
}

/// A fresh app's network count: online, no misses.
#[uniffi::export]
pub fn net_health_fresh() -> NetHealthState {
    vela_core::app::net_health::NetHealth::default().into()
}

/// Feed one call: `reached` is any answer from a server, whatever its status;
/// a miss is a call that never reached one (for a pooled read: every endpoint
/// swept, none answered, not throttled — timeouts included). `source` is the
/// chain the call read (`None` for a call with no chain); `now_ms` the clock
/// when it ended. One failing chain is its own notice, never "offline" (spec
/// 082 RJ14). On `came_back` a shell retries its failed page, clears
/// transient logo misses and re-reads the balance.
#[uniffi::export]
pub fn net_health_step(
    state: NetHealthState,
    reached: bool,
    source: Option<u32>,
    now_ms: f64,
) -> NetHealthStep {
    let (next, edge) =
        vela_core::app::net_health::net_health_step(state.into(), reached, source, now_ms);
    NetHealthStep {
        state: next.into(),
        edge: edge.map(|edge| snake_name(&edge)),
    }
}

/// How long a remote logo that failed to load stays failed: `None` for the
/// session (asking again will not help), `Some(ms)` for a miss that may heal.
/// With an HTTP `status` the miss is that status's class (404/410, 401/403,
/// a 2xx whose bytes do not draw → the session; 429, 408, 5xx → 60 s);
/// without one it is `kind`, a `MarkMiss` wire name (`transport`,
/// `not_an_image`, `unknown`…), and a name this core does not know is
/// `unknown`.
#[uniffi::export]
pub fn mark_miss_ttl_ms(kind: String, status: Option<u16>) -> Option<u32> {
    use vela_core::app::remote_mark::{self as mark, MarkMiss};
    let miss = match status {
        Some(status) => mark::mark_miss_of_status(status),
        None => snake::<MarkMiss>(&kind).unwrap_or(MarkMiss::Unknown),
    };
    mark::mark_miss_ttl_ms(miss)
}

/// What a circle standing for a token or a network wears
/// (`vela_core::app::remote_mark::MarkView`): draw `glyph`, then the first of
/// `logo_urls` that loads over it; the corner badge only when
/// `badge_chain_id` is set (its dot, `badge_logo_url` over it).
#[derive(Debug, Clone, PartialEq, Eq, uniffi::Record)]
pub struct MarkView {
    /// The ticker's first three characters, upper-cased: always drawn.
    pub glyph: String,
    /// Logo candidates, best first. Empty = the glyph alone.
    pub logo_urls: Vec<String>,
    /// The chain the badge names; `None` = no badge.
    pub badge_chain_id: Option<u32>,
    /// The badge chain's logo; `None` whenever there is no badge.
    pub badge_logo_url: Option<String>,
}

impl From<vela_core::app::remote_mark::MarkView> for MarkView {
    fn from(mark: vela_core::app::remote_mark::MarkView) -> Self {
        Self {
            glyph: mark.glyph,
            logo_urls: mark.logo_urls,
            badge_chain_id: mark.badge_chain_id,
            badge_logo_url: mark.badge_logo_url,
        }
    }
}

/// A COIN's mark (`remote_mark::token_mark`): `ethereum_data_url` is the
/// person's chain-data endpoint ("" = the built-in one), `token_address`
/// `None` for the chain's native coin, `named` the logo URLs an index already
/// gave (tried first). A native coin wears its home chain's logo, and its
/// badge is hidden on that chain.
#[uniffi::export]
pub fn token_mark(
    ethereum_data_url: String,
    chain_id: u32,
    symbol: String,
    token_address: Option<String>,
    named: Vec<String>,
) -> MarkView {
    vela_core::app::remote_mark::token_mark(
        &ethereum_data_url,
        chain_id,
        &symbol,
        token_address.as_deref(),
        &named,
    )
    .into()
}

/// A NETWORK drawn as itself (`remote_mark::chain_mark`): network rows and
/// facts, chain-locking notices, receive rows, the QR centre, chips. Its own
/// logo, never a badge.
#[uniffi::export]
pub fn chain_mark(ethereum_data_url: String, chain_id: u32, native_symbol: String) -> MarkView {
    vela_core::app::remote_mark::chain_mark(&ethereum_data_url, chain_id, &native_symbol).into()
}

/// `{base}/chainlogos/eip155-{chain_id}.png` on the person's chain-data
/// endpoint ("" = the built-in one); `None` for chain 0, which names no
/// network.
#[uniffi::export]
pub fn chain_logo_url(ethereum_data_url: String, chain_id: u32) -> Option<String> {
    vela_core::app::remote_mark::chain_logo_url(&ethereum_data_url, chain_id)
}

/// A shipped network's usual time to include an operation, in seconds; `None`
/// for a network Vela does not ship (the receipt then circles instead of
/// drawing a promise). The dApp signing sheet's wait reads the same number as
/// the send receipt (spec 079; the web's dApp receipt reads it over wasm).
#[uniffi::export]
pub fn network_typical_inclusion_s(chain_id: u32) -> Option<u32> {
    vela_core::app::network_admin::typical_inclusion_s(chain_id).map(u32::from)
}

/// A `FeeFailure` from the shell: its wire name (`"quote_unavailable"`), or,
/// for a failure that carries data, its JSON (`{"chain_read":{"rate_limited":
/// true}}`, spec 082 RJ13).
fn fee_failure_of(failure: &str) -> Option<vela_core::app::fee_policy::FeeFailure> {
    snake(failure).or_else(|| serde_json::from_str(failure).ok())
}

/// The wait before automatic fee re-quote `attempt` (1-based) after
/// `failure` (see [`fee_failure_of`]), or `None` when no retry can fix it
/// (spec 079 FR-008): 3 s, 6 s, then every 8 s (spec 082 RJ12).
#[uniffi::export]
pub fn fee_requote_delay_ms(failure: String, attempt: u32) -> Option<u32> {
    vela_core::app::fee_policy::requote_delay_ms(fee_failure_of(&failure)?, attempt)
}

/// The bound on each automatic fee re-quote, in ms (spec 082 RJ12): a re-ask
/// not answered in this time is a failure again.
#[uniffi::export]
pub fn fee_requote_timeout_ms() -> u32 {
    vela_core::app::fee_policy::REQUOTE_TIMEOUT_MS
}

/// The corpus key of the reason line under a failed fee (spec 082 RJ13), or
/// `None` for no reason line. `explore.chainDown` takes `{{chain}}`, the
/// chain's name. `failure` as for [`fee_requote_delay_ms`].
#[uniffi::export]
pub fn fee_failure_reason_key(failure: String) -> Option<String> {
    vela_core::app::fee_policy::failure_reason_key(fee_failure_of(&failure)?).map(str::to_owned)
}

/// A signed balance change from signed base units (`"-1000"`, `"+2100…"`):
/// the token ladder, a dust figure written exactly (never `−0`), U+2212 for
/// a minus, `+` for a plus; `None` for zero or unreadable text (spec 082
/// RJ15). `preset` is the number preset's wire name (`comma_dot`,
/// `dot_comma`, `space_comma`, `indian`; anything else is `comma_dot`).
#[uniffi::export]
pub fn format_signed_token_amount(
    delta_base_units: String,
    decimals: u32,
    preset: String,
) -> Option<String> {
    use vela_core::l10n::{format_signed_token_amount as format, NumberPreset};
    format(
        &delta_base_units,
        decimals,
        snake::<NumberPreset>(&preset).unwrap_or_default(),
    )
}

/// The Safe message hash a passkey signs for EIP-1271 verification (spec
/// 044): `SafeMessage(bytes message)` under the SAFE's own domain, so a
/// page's `personal_sign` / typed-data signature verifies on chain.
#[uniffi::export]
pub fn safe_message_hash(
    original_hash: Vec<u8>,
    chain_id: u64,
    safe_address: String,
) -> Result<Vec<u8>, CoreError> {
    vela_core::user_op::compute_safe_message_hash(&original_hash, chain_id, &safe_address)
        .map_err(Into::into)
}

/// The assertion as an EIP-1271 signature (spec 044): the user-operation
/// envelope's checks and encoding, without the validity window.
#[uniffi::export]
pub fn eip1271_signature(
    assertion: WebAuthnAssertion,
    credential_id: String,
    keys: Vec<WalletKeyRecord>,
) -> Result<Vec<u8>, CoreError> {
    let keys: Vec<vela_core::user_op::WalletKey> = keys
        .into_iter()
        .map(|key| vela_core::user_op::WalletKey {
            credential_id: key.credential_id,
            public_key_hex: key.public_key_hex,
        })
        .collect();
    vela_core::user_op::eip1271_envelope_signature(
        &assertion.authenticator_data,
        &assertion.client_data_json,
        &assertion.signature_der,
        &credential_id,
        &keys,
    )
    .map_err(Into::into)
}

/// The EntryPoint every Vela operation is submitted against.
#[uniffi::export]
pub fn entry_point_address() -> String {
    vela_core::safe::ENTRY_POINT.to_owned()
}

/// Issue 212: how long a chain's fee signals may be held, in ms — the one
/// number every shell's cache used to carry its own copy of.
#[uniffi::export]
pub fn fee_signals_cache_ttl_ms() -> u32 {
    vela_core::app::fee_policy::FEE_SIGNALS_CACHE_TTL_MS
}

/// Issue 212: may this gas-signal read be held? Decimal wei in; see
/// `fee_policy::gas_signals_cacheable`.
#[uniffi::export]
pub fn gas_signals_cacheable(
    eth_gas_price: Option<String>,
    block_answered: bool,
    want_tip: bool,
    priority_fee: Option<String>,
) -> bool {
    vela_core::app::fee_policy::gas_signals_cacheable(
        eth_gas_price.as_deref(),
        block_answered,
        want_tip,
        priority_fee.as_deref(),
    )
}

/// Issue 212: may the relay's gas quote for one tier be held?
#[uniffi::export]
pub fn bundler_quote_cacheable(max_fee_per_gas: String) -> bool {
    vela_core::app::fee_policy::bundler_quote_cacheable(&max_fee_per_gas)
}

/// One size a still image is decoded at (spec 090), in pixels.
#[derive(uniffi::Record)]
pub struct QrScanSize {
    pub width: u32,
    pub height: u32,
}

/// The sizes to read a picked or dropped image's QR code at, in order: the
/// image as it is, then shrunk so its longest side is 1024, 640, 400 (only
/// rungs smaller than the image). Stop at the first hit. A screenshot of
/// Vela's own receive code draws modules ~24 px across, wider than ZXing's
/// local binarizer window; the rule lives in `vela_core::qr_scan`.
#[uniffi::export]
pub fn still_qr_sizes(width: u32, height: u32) -> Vec<QrScanSize> {
    vela_core::qr_scan::still_qr_sizes(width, height)
        .into_iter()
        .map(|(width, height)| QrScanSize { width, height })
        .collect()
}

/// The $1 peg for a native gas coin that IS a dollar stablecoin — Tempo's
/// `USD`, Arc's `USDC`. `None` means "not pegged": the caller falls through to
/// the Chainlink/DEX ladder unchanged.
///
/// This exists so the rule is written once. It used to be a `symbol == "USD"`
/// literal in each of the four shells, which is four chances to disagree about
/// what a coin is worth.
#[uniffi::export]
pub fn pegged_native_usd(symbol: String) -> Option<f64> {
    vela_core::app::balance_dashboard::pegged_native_usd(&symbol)
}

/// The source ladder and its sanity band: a DEX price that disagrees with
/// Chainlink by too much loses to Chainlink.
#[uniffi::export]
pub fn choose_native_price(
    dex: Option<f64>,
    chainlink_local: Option<f64>,
    chainlink_eth: Option<f64>,
) -> NativePriceChoice {
    use vela_core::app::balance_dashboard::NativePriceSource as Source;
    match vela_core::app::balance_dashboard::choose_native_price(
        dex,
        chainlink_local,
        chainlink_eth,
    ) {
        Some(chosen) => NativePriceChoice {
            price: Some(chosen.price),
            source: match chosen.source {
                Source::Dex => "dex",
                Source::ChainlinkSanity => "chainlink_sanity",
                Source::ChainlinkLocal => "chainlink_local",
                Source::ChainlinkEth => "chainlink_eth",
            }
            .to_owned(),
        },
        None => NativePriceChoice {
            price: None,
            source: "none".to_owned(),
        },
    }
}

/// Does this chain have no native coin?
///
/// Tempo's gas is a TIP-20 stablecoin, so it has nothing to read a native
/// balance from — and its RPC answers the SAME constant for every address,
/// while its native symbol is `USD`. A shell that queries it anyway and lets a
/// stablecoin peg price that constant at a dollar puts something like
/// 4 × 10^57 dollars into a person's total. The desktop found exactly that
/// (spec 031) and fixed it by reading this predicate rather than inventing a
/// "that number looks too big" threshold.
///
/// It was reachable only by a shell that links Rust directly. Android and iOS
/// could not ask, which left them one plausible-looking constant away from the
/// same bug — so it crosses the bridge now, for the same reason the price
/// ladder does.
#[uniffi::export]
pub fn is_chain_without_native_coin(chain_id: u32) -> bool {
    vela_core::app::fee_policy::is_tempo_chain(chain_id)
}

/// Where to watch for money arriving when a wallet holds nothing yet.
///
/// `token_trust` already falls back to this list when it is handed an empty
/// set of held chains, so the core is the owner. The shell needs the same
/// chain ids slightly earlier than the core does — it must fetch each chain's
/// registry document to build the allowlist BEFORE the poll starts, and a poll
/// that begins with no allowlist scans nothing.
///
/// The web keeps its own copy of these six for that reason. Copying them again
/// here would make a brand-new wallet's very first receipt — the one that
/// matters most — depend on two lists agreeing.
#[uniffi::export]
pub fn default_monitor_chains() -> Vec<u32> {
    vela_core::app::token_trust::DEFAULT_MONITOR_CHAINS.to_vec()
}

// ---------------------------------------------------------------------------
// Spec 082 exports: the wrappers keep the core's rules and wire names
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests_082 {
    use super::*;
    use alloy_dyn_abi::{DynSolType, DynSolValue};
    use serde_json::{json, Value};

    const LOCAL: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";
    const RELAY: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";

    /// The two Gnosis operations the core pins (T011): their `handleOps`
    /// calldata and the EntryPoint's own `userOpHash`.
    const FIXTURE: &str = include_str!("../../vela-core/tests/fixtures/userop-hash-gnosis.json");

    /// Spec 093: the stored request's display is the core's, by content word.
    #[test]
    fn the_request_display_is_the_core_s() {
        assert_eq!(
            dapp_request_display(
                "message".to_owned(),
                r#"["0x68656c6c6f","0x1111111111111111111111111111111111111111"]"#.to_owned()
            )
            .as_deref(),
            Some("hello")
        );
        assert_eq!(
            dapp_request_display("call_data".to_owned(), String::new()),
            None
        );
        assert_eq!(
            dapp_request_display("haiku".to_owned(), "[]".to_owned()),
            None
        );
    }

    fn halves(word: &[u8]) -> (u128, u128) {
        let mut high = [0u8; 16];
        let mut low = [0u8; 16];
        high.copy_from_slice(&word[..16]);
        low.copy_from_slice(&word[16..32]);
        (u128::from_be_bytes(high), u128::from_be_bytes(low))
    }

    /// The one operation of a `handleOps` calldata, as the FFI record a
    /// phone carries: decimal gas strings, bytes as bytes.
    fn draft_from_handle_ops(input: &str) -> UserOpDraft {
        let calldata = vela_core::primitives::from_hex(input).unwrap();
        let params: DynSolType =
            "((address,uint256,bytes,bytes,bytes32,uint256,bytes32,bytes,bytes)[],address)"
                .parse()
                .unwrap();
        let DynSolValue::Tuple(top) = params.abi_decode_params(&calldata[4..]).unwrap() else {
            panic!("params are a tuple")
        };
        let DynSolValue::Array(ops) = &top[0] else {
            panic!("ops array")
        };
        assert_eq!(ops.len(), 1);
        let DynSolValue::Tuple(f) = &ops[0] else {
            panic!("op tuple")
        };
        let bytes = |v: &DynSolValue| match v {
            DynSolValue::Bytes(b) => b.clone(),
            other => panic!("not bytes: {other:?}"),
        };
        let word = |v: &DynSolValue| match v {
            DynSolValue::FixedBytes(w, 32) => w.to_vec(),
            other => panic!("not bytes32: {other:?}"),
        };
        let uint = |v: &DynSolValue| match v {
            DynSolValue::Uint(n, 256) => *n,
            other => panic!("not uint256: {other:?}"),
        };
        let DynSolValue::Address(sender) = &f[0] else {
            panic!("sender")
        };
        let (verification, call) = halves(&word(&f[4]));
        let (priority, max_fee) = halves(&word(&f[6]));
        assert!(bytes(&f[7]).is_empty(), "the vectors carry no paymaster");
        UserOpDraft {
            sender: sender.to_checksum(None),
            nonce: format!("0x{:x}", uint(&f[1])),
            init_code: bytes(&f[2]),
            call_data: bytes(&f[3]),
            verification_gas_limit: verification.to_string(),
            call_gas_limit: call.to_string(),
            pre_verification_gas: uint(&f[5]).to_string(),
            max_fee_per_gas: max_fee.to_string(),
            max_priority_fee_per_gas: priority.to_string(),
            paymaster_and_data: Vec::new(),
            signature: bytes(&f[8]),
        }
    }

    #[test]
    fn user_op_hash_through_the_record_is_the_entry_point_s_own() {
        let fixture: Value = serde_json::from_str(FIXTURE).unwrap();
        let vectors = fixture["vectors"].as_array().unwrap();
        assert_eq!(vectors.len(), 2);
        for vector in vectors {
            let draft = draft_from_handle_ops(vector["input"].as_str().unwrap());
            let want = vector["user_operation_event"]["topics"][1]
                .as_str()
                .unwrap();
            assert_eq!(user_op_hash(draft.clone(), 100).unwrap(), want);
            // Another chain is another operation.
            assert_ne!(user_op_hash(draft, 10200).unwrap(), want);
        }
    }

    #[test]
    fn the_submit_step_keeps_the_three_verdicts_apart() {
        let step = |reply, attempt, maybe| user_op_submit_step(reply, attempt, maybe, LOCAL.into());
        assert!(matches!(
            step(UserOpSubmitReply::Hash { hash: RELAY.into() }, 0, true),
            UserOpSubmitStep::Accepted { user_op_hash } if user_op_hash == RELAY
        ));
        assert!(matches!(
            step(UserOpSubmitReply::NoAnswer, 0, true),
            UserOpSubmitStep::MaybeSent { user_op_hash } if user_op_hash == LOCAL
        ));
        assert!(matches!(
            step(UserOpSubmitReply::NoAnswer, 0, false),
            UserOpSubmitStep::NotSent { rejection: None }
        ));
        let busy = || UserOpSubmitReply::RpcError {
            error_json: r#"{"code":-32000,"message":"currently processing"}"#.into(),
        };
        assert!(matches!(
            step(busy(), 0, false),
            UserOpSubmitStep::RetryAfter { delay_ms: 3_000 }
        ));
        let aa25 = || UserOpSubmitReply::RpcError {
            error_json: r#"{"code":-32500,"message":"AA25 invalid account nonce"}"#.into(),
        };
        assert!(matches!(
            step(aa25(), 1, false),
            UserOpSubmitStep::NotSent {
                rejection: Some(RelayRejection::Other { .. })
            }
        ));
        // A refusal after a lost reply proves nothing about the first POST.
        assert!(matches!(
            step(aa25(), 1, true),
            UserOpSubmitStep::MaybeSent { user_op_hash } if user_op_hash == LOCAL
        ));
        let marker = |hash: &str| UserOpSubmitReply::RpcError {
            error_json: format!(r#"{{"code":-32000,"message":"pending [existingHash:{hash}]"}}"#),
        };
        // This op's own hash: the relay already holds it.
        assert!(matches!(
            step(marker(LOCAL), 0, false),
            UserOpSubmitStep::Accepted { user_op_hash } if user_op_hash == LOCAL
        ));
        // Another op holds the nonce (083): never this one's hash.
        assert!(matches!(
            step(marker(RELAY), 0, false),
            UserOpSubmitStep::NotSent {
                rejection: Some(RelayRejection::NonceHeld { user_op_hash })
            } if user_op_hash == RELAY
        ));
        assert_eq!(
            user_op_not_sent_detail(),
            "relay unreachable; nothing was sent"
        );
    }

    #[test]
    fn the_status_parser_speaks_the_tracker_s_words() {
        assert_eq!(user_op_status_method(), "pimlico_getUserOperationStatus");
        let answer = parse_user_op_status(
            json!({"jsonrpc": "2.0", "id": 1, "result": {
                "status": "not_submitted",
                "last_executor_stage": "in_band_settlement_hold",
                "transactionHash": RELAY,
            }})
            .to_string(),
        )
        .unwrap();
        assert_eq!(answer.status, "not_submitted");
        assert_eq!(answer.stage.as_deref(), Some("in_band_settlement_hold"));
        assert_eq!(answer.tx_hash.as_deref(), Some(RELAY));
        assert!(parse_user_op_status(r#"{"status":"pending"}"#.into()).is_none());
        assert!(parse_user_op_status(r#"{"error":{"code":-32601}}"#.into()).is_none());
    }

    fn entry(status: &str, outcome: &str, tx_hash: Option<&str>) -> String {
        json!({
            "user_op_hash": LOCAL, "chain_id": 100, "record_ids": ["r1"],
            "status": status, "tx_hash": tx_hash, "polling": false,
            "submitted_at_ms": 1.0, "outcome": outcome,
        })
        .to_string()
    }

    #[test]
    fn an_ending_is_never_confirmed_before_the_tracker_says_so() {
        let ok = |result: &str| json!({"type": "ok", "result": result}).to_string();
        let landed = sign_ending_of("eth_sendTransaction".into(), ok(RELAY), Some(LOCAL.into()))
            .unwrap()
            .unwrap();
        let state = |ending: &str, entry: Option<String>| -> Value {
            serde_json::from_str(&sign_ending_state(ending.into(), entry).unwrap()).unwrap()
        };
        // No tracker entry yet: following, landing.
        assert_eq!(
            state(&landed, None),
            json!({"type": "following", "user_op_hash": LOCAL, "outcome": "landing", "fee_held": false, "relay_funding": false})
        );
        assert_eq!(state(&landed, Some("null".into()))["type"], "following");
        // It landed and reverted: the W3 fault, now a cross.
        assert_eq!(
            state(&landed, Some(entry("dropped", "final", Some(RELAY)))),
            json!({"type": "reverted", "tx_hash": RELAY})
        );
        // The wait ran out: the page got the op hash; a lost reply follows.
        let still = sign_ending_of("wallet_sendCalls".into(), ok(LOCAL), Some(LOCAL.into()))
            .unwrap()
            .unwrap();
        assert_eq!(
            state(&still, Some(entry("pending", "maybe_sent", None)))["outcome"],
            "maybe_sent"
        );
        assert_eq!(
            state(&still, Some(entry("not_sent", "final", None))),
            json!({"type": "not_sent"})
        );
        // A message is signed; a refusal is nothing to show.
        let signed = sign_ending_of("personal_sign".into(), ok("0xsig"), None)
            .unwrap()
            .unwrap();
        assert_eq!(state(&signed, None), json!({"type": "signed"}));
        let refused =
            json!({"type": "err", "code": 4001, "kind": "user_rejected", "message": null});
        assert_eq!(
            sign_ending_of("eth_sendTransaction".into(), refused.to_string(), None).unwrap(),
            None
        );
        assert!(sign_ending_of("eth_sendTransaction".into(), "{".into(), None).is_err());
        assert!(sign_ending_state(landed, Some("{}".into())).is_err());
    }

    #[test]
    fn the_receipt_wait_and_the_send_verdict_are_the_core_s() {
        assert!((dapp_receipt_wait_ms(0.0) - 120_000.0).abs() < f64::EPSILON);
        assert!((dapp_receipt_wait_ms(46_000.0) - 74_000.0).abs() < f64::EPSILON);
        assert!((dapp_receipt_wait_ms(200_000.0) - 10_000.0).abs() < f64::EPSILON);
        let outcome = |status, outcome| -> Option<Value> {
            send_receipt_outcome_of(entry(status, outcome, Some(RELAY)))
                .unwrap()
                .map(|json| serde_json::from_str(&json).unwrap())
        };
        assert_eq!(
            outcome("not_sent", "final"),
            Some(json!({"type": "failed", "rejected": false, "not_sent": true}))
        );
        assert_eq!(
            outcome("confirmed", "final"),
            Some(json!({"type": "confirmed", "tx_hash": RELAY}))
        );
        // A maybe-sent op says nothing yet — never a failure.
        assert_eq!(outcome("pending", "maybe_sent"), None);
    }

    #[test]
    fn a_simulation_that_could_not_check_is_a_caution_and_a_revert_a_danger() {
        let user = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
        let crashed = sim_outcome(
            user.into(),
            json!({"error": {"code": -32603, "message": "method handler crashed"}}).to_string(),
        );
        assert_eq!(crashed.kind, "not_offered");
        assert_eq!(crashed.notice_risk.as_deref(), Some("caution"));
        assert_eq!(
            crashed.notice_key.as_deref(),
            Some("componentsUi.signing.simUnavailableWarning")
        );
        let unreachable = sim_outcome(user.into(), r#"{"unreachable":true}"#.into());
        assert_eq!(unreachable.kind, "unreachable");
        assert_eq!(unreachable.notice_risk.as_deref(), Some("caution"));

        let reason = format!(
            "0x08c379a0{:0>64}{:0>64}{:0<64}",
            "20",
            "4",
            "6e6f7065" // "nope"
        );
        let reverts = sim_outcome(
            user.into(),
            json!({"result": [{"calls": [{"status": "0x0", "returnData": reason, "logs": []}]}]})
                .to_string(),
        );
        assert_eq!(reverts.kind, "reverts");
        assert_eq!(reverts.revert_reason.as_deref(), Some("nope"));
        assert_eq!(reverts.notice_risk.as_deref(), Some("danger"));
        assert_eq!(
            reverts.notice_key.as_deref(),
            Some("componentsUi.signing.simWillFailReason")
        );
        assert_eq!(reverts.deltas_json, "[]");

        let token = "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913";
        let transfer = json!({
            "address": token,
            "topics": [
                "0xddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef",
                format!("0x{:0>64}", "1234"),
                format!("0x{:0>64}", user[2..].to_lowercase()),
            ],
            "data": format!("0x{:0>64}", "5"),
        });
        let checked = sim_outcome(
            user.into(),
            json!({"result": [{"calls": [{"status": "0x1", "logs": [transfer]}]}]}).to_string(),
        );
        assert_eq!(checked.kind, "deltas");
        assert_eq!(checked.notice_risk, None);
        assert_eq!(checked.notice_key, None);
        let deltas: Value = serde_json::from_str(&checked.deltas_json).unwrap();
        assert_eq!(
            deltas,
            json!([{"kind": "erc20", "token": token, "delta": "5"}])
        );
    }

    #[test]
    fn the_browser_rules_cross_with_their_wire_names() {
        let proxy = browser_load_classify("probe".into(), 6, None, false).unwrap();
        assert_eq!(proxy.class, "proxy");
        assert_eq!(proxy.reason_key, "explore.loadProxy");
        let cf = browser_load_classify(
            "apple".into(),
            306,
            Some("kCFErrorDomainCFNetwork".into()),
            false,
        )
        .unwrap();
        assert_eq!(cf.class, "proxy");
        assert_eq!(
            browser_load_classify("android".into(), -5, None, false)
                .unwrap()
                .class,
            "proxy"
        );

        assert_eq!(browser_load_give_up_ms(), 20_000);
        assert!(!browser_load_should_give_up(19_999, false, 0.1));
        assert!(browser_load_should_give_up(20_000, false, 0.1));
        assert!(!browser_load_should_give_up(20_000, true, 0.1));
        assert!(!browser_load_should_give_up(20_000, false, 0.4));
        let stalled = browser_load_stalled();
        assert_eq!(stalled.class, "timeout");
        assert_eq!(stalled.reason_key, "explore.loadOffline");
        assert!(stalled.auto_retry);
        for class in ["offline", "timeout", "refused", "other", "proxy"] {
            assert!(
                browser_load_retry_when_network_returns(class.into()),
                "{class}"
            );
        }
        for class in ["not_found", "certificate", "bogus", ""] {
            assert!(
                !browser_load_retry_when_network_returns(class.into()),
                "{class}"
            );
        }

        // The G28 spoof shape: a pending load never renames a shown page.
        let bar = browser_address_bar(
            Some("https://jumper.exchange/swap".into()),
            Some("https://app.uniswap.org/".into()),
            None,
        );
        assert_eq!(
            (bar.host.as_str(), bar.lock.as_str()),
            ("jumper.exchange", "closed")
        );
        let failed = browser_address_bar(None, None, Some("https://gone.example/x".into()));
        assert_eq!(
            (failed.host.as_str(), failed.lock.as_str()),
            ("gone.example", "none")
        );
        let public_http = browser_address_bar(Some("http://example.com/".into()), None, None);
        assert_eq!(public_http.lock, "open");
        let empty = browser_address_bar(None, None, None);
        assert_eq!((empty.url.as_str(), empty.lock.as_str()), ("", "none"));

        let once = browser_site_label("127.0.0.1:8137".into(), "127.0.0.1:8137".into());
        assert_eq!(
            (once.name.as_str(), once.host_line),
            ("127.0.0.1:8137", None)
        );
        let twice = browser_site_label("Uniswap".into(), "app.uniswap.org".into());
        assert_eq!(twice.name, "Uniswap");
        assert_eq!(twice.host_line.as_deref(), Some("app.uniswap.org"));
    }

    #[test]
    fn three_misses_from_two_chains_after_ten_quiet_seconds_go_offline() {
        let fresh = net_health_fresh();
        assert!(fresh.online && fresh.misses == 0 && fresh.sources.is_empty());
        let first = net_health_step(fresh, false, Some(100), 0.0);
        let second = net_health_step(first.state, false, Some(1), 4_000.0);
        assert_eq!(second.edge, None);
        assert_eq!(second.state.sources, vec![100, 1]);
        // One chain only, however long: that chain's notice.
        let mut one = net_health_fresh();
        for i in 0..10 {
            let step = net_health_step(one, false, Some(100), f64::from(i) * 5_000.0);
            assert_eq!(step.edge, None);
            one = step.state;
        }
        let third = net_health_step(second.state, false, Some(100), 11_000.0);
        assert!(!third.state.online);
        assert_eq!(third.edge.as_deref(), Some("went_offline"));
        let fourth = net_health_step(third.state, false, None, 12_000.0);
        assert_eq!(fourth.edge, None);
        let back = net_health_step(fourth.state, true, Some(8453), 13_000.0);
        assert_eq!((back.state.misses, back.state.online), (0, true));
        assert_eq!(back.state.last_reach_ms, Some(13_000.0));
        assert_eq!(back.edge.as_deref(), Some("came_back"));
    }

    #[test]
    fn the_round_2_exports() {
        assert_eq!(
            user_op_refused_dapp_detail(),
            "the network refused this transaction; nothing was sent"
        );
        assert_eq!(user_op_write_ahead_wait_ms(), 5_000);
        assert_eq!(balance_chain_read_deadline_ms(), 18_000);
        let reverts = user_op_estimate_failure(
            r#"{"code":-32500,"message":"UserOperation simulation failed","data":"Safe execution failed: the target call in executeUserOp reverted"}"#.into(),
        );
        assert_eq!((reverts.kind.as_str(), reverts.reason), ("reverts", None));
        assert_eq!(user_op_estimate_failure("null".into()).kind, "unavailable");
        assert_eq!(fee_requote_timeout_ms(), 6_000);
        assert_eq!(
            fee_requote_delay_ms("quote_unavailable".into(), 3),
            Some(8_000)
        );
        let chain = r#"{"chain_read":{"rate_limited":false}}"#;
        assert_eq!(fee_requote_delay_ms(chain.into(), 1), Some(3_000));
        assert_eq!(
            fee_failure_reason_key(chain.into()).as_deref(),
            Some("explore.chainDown")
        );
        assert_eq!(
            fee_failure_reason_key("quote_unavailable".into()).as_deref(),
            Some("componentsUi.gas.reasonQuote")
        );
        assert_eq!(fee_failure_reason_key("missing_public_key".into()), None);
        assert_eq!(fee_failure_reason_key("bogus".into()), None);
        assert_eq!(
            format_signed_token_amount("-1000".into(), 18, "comma_dot".into()).as_deref(),
            Some("\u{2212}0.000000000000001")
        );
        assert_eq!(
            format_signed_token_amount("0".into(), 18, "comma_dot".into()),
            None
        );
    }

    #[test]
    fn a_mark_crosses_the_ffi_whole() {
        let eth_on_base = token_mark(String::new(), 8453, "ETH".into(), None, Vec::new());
        assert_eq!(eth_on_base.glyph, "ETH");
        assert_eq!(
            eth_on_base.logo_urls,
            vec!["https://ethereum-data.getvela.app/chainlogos/eip155-1.png".to_owned()]
        );
        assert_eq!(eth_on_base.badge_chain_id, Some(8453));
        assert_eq!(
            eth_on_base.badge_logo_url.as_deref(),
            Some("https://ethereum-data.getvela.app/chainlogos/eip155-8453.png")
        );
        let base = chain_mark("https://data.example/".into(), 8453, "ETH".into());
        assert_eq!(
            base.logo_urls,
            vec!["https://data.example/chainlogos/eip155-8453.png".to_owned()]
        );
        assert_eq!(base.badge_chain_id, None);
        assert_eq!(chain_logo_url(String::new(), 0), None);
    }

    /// The phones' "Updated 2m" is the core's sentence in the active
    /// language, the weekday and the stored date word included.
    #[test]
    fn a_relative_time_crosses_the_ffi_in_the_active_language() {
        // 2026-06-13 13:45:00.999 UTC, a Saturday.
        const NOW_MS: i64 = 1_781_358_300_999;
        let asset = |lng: &str| {
            let path = format!(
                "{}/../../../assets/i18n/{lng}.json",
                env!("CARGO_MANIFEST_DIR")
            );
            std::fs::read(&path).unwrap_or_else(|e| unreachable!("{path}: {e}"))
        };
        let i18n = I18n::new(asset("en")).unwrap_or_else(|e| unreachable!("{e}"));
        let ago = |ts_seconds: i64, date: &str| {
            i18n.format_relative_time(ts_seconds, NOW_MS, 0, date.into())
                .unwrap_or_else(|e| unreachable!("{e}"))
        };
        let now_s = NOW_MS / 1000;
        assert_eq!(ago(now_s - 44, "iso"), "now");
        assert_eq!(ago(now_s - 90, "iso"), "2m");
        assert_eq!(ago(now_s - 5_400, "iso"), "2h");
        assert_eq!(ago(now_s - 3 * 86_400, "iso"), "Wed");
        assert_eq!(ago(now_s - 30 * 86_400, "iso"), "2026-05-14");
        assert_eq!(ago(now_s - 30 * 86_400, "dmy_dot"), "14.05.2026");
        // Milliseconds where seconds belong are the future: "now", never a
        // date thousands of years out.
        assert_eq!(ago(NOW_MS - 30 * 86_400_000, "iso"), "now");

        i18n.load_catalog("zh".into(), asset("zh"))
            .unwrap_or_else(|e| unreachable!("{e}"));
        i18n.change_language("zh".into())
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(ago(now_s, "ymd_slash"), "刚刚");
        assert_eq!(ago(now_s - 120, "ymd_slash"), "2分钟前");
        assert_eq!(ago(now_s - 3 * 86_400, "ymd_slash"), "周三");
        assert_eq!(ago(now_s - 30 * 86_400, "ymd_slash"), "2026/05/14");
    }

    #[test]
    fn a_missing_logo_stays_missed_and_a_bad_minute_heals() {
        assert_eq!(mark_miss_ttl_ms("unknown".into(), None), Some(60_000));
        assert_eq!(mark_miss_ttl_ms("transport".into(), None), Some(60_000));
        assert_eq!(mark_miss_ttl_ms("not_an_image".into(), None), None);
        assert_eq!(mark_miss_ttl_ms("bogus".into(), None), Some(60_000));
        // With a status, the status decides.
        assert_eq!(mark_miss_ttl_ms("unknown".into(), Some(404)), None);
        assert_eq!(mark_miss_ttl_ms("unknown".into(), Some(403)), None);
        assert_eq!(mark_miss_ttl_ms("unknown".into(), Some(503)), Some(60_000));
        assert_eq!(mark_miss_ttl_ms("unknown".into(), Some(429)), Some(60_000));
    }

    #[test]
    fn the_read_plan_counts_the_stablecoins_and_skips_a_coinless_chain() {
        let usdc = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
        let stables = json!([{"symbol": "USDC", "contract": usdc, "type": "usd"}]).to_string();
        let plan = balance_read_plan(
            8453,
            stables.clone(),
            Some("0x4200000000000000000000000000000000000006".into()),
            String::new(),
        )
        .unwrap();
        let kinds: Vec<&str> = plan.iter().map(|slot| slot.kind.as_str()).collect();
        assert_eq!(kinds, ["native", "stable", "wrapped"]);
        assert_eq!(plan[1].contract.as_deref(), Some(usdc));
        assert_eq!(plan[1].peg_usd, Some(1.0));
        assert_eq!(plan[1].known_decimals, None);

        // The person's entry for the same contract lends its metadata.
        let custom = json!([{"contract": usdc.to_lowercase(), "symbol": "USDC.b",
                             "name": "Mine", "decimals": 6}])
        .to_string();
        let plan = balance_read_plan(8453, stables, None, custom).unwrap();
        assert_eq!(plan.len(), 2);
        assert_eq!(plan[1].kind, "stable");
        assert_eq!(plan[1].symbol, "USDC.b");
        assert_eq!(plan[1].known_decimals, Some(6));

        let tempo = balance_read_plan(4217, "[]".into(), None, "[]".into()).unwrap();
        assert!(tempo.iter().all(|slot| slot.kind != "native"));
        assert!(balance_read_plan(8453, "{".into(), None, String::new()).is_err());
    }
}
