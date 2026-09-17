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
) -> String {
    vela_core::registry_backup::step_json(&address, &founding_public_key_hex, &answers_json)
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
    Other { message: String },
}

/// The relay's sentence, classified the way `classifySubmit` does.
#[uniffi::export]
pub fn classify_relay_rejection(message: String) -> RelayRejection {
    match vela_core::user_op::classify_relay_rejection(&message) {
        vela_core::user_op::RelayRejection::RelayerUnavailable => {
            RelayRejection::RelayerUnavailable
        }
        vela_core::user_op::RelayRejection::BundlerUnderfunded => {
            RelayRejection::BundlerUnderfunded
        }
        vela_core::user_op::RelayRejection::Other(message) => RelayRejection::Other { message },
    }
}

/// `parseBundlerError`: the JSON-RPC `error` member as one sentence.
#[uniffi::export]
pub fn relay_error_message(error_json: String) -> String {
    vela_core::user_op::relay_error_message(&error_json)
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
