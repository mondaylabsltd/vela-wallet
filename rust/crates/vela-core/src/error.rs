use thiserror::Error;

/// Single flat error for the whole core.
///
/// Variant set is part of the FFI contract (`contracts/core-api.md`); binding
/// shells map it 1:1 (Kotlin sealed `CoreException`, Swift `CoreError`, TS
/// `{ code, message }`). Messages carry detail; variants carry classification.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CoreError {
    #[error("invalid hex: {0}")]
    InvalidHex(String),
    #[error("invalid base64url: {0}")]
    InvalidBase64Url(String),
    #[error("invalid quantity: {0}")]
    InvalidQuantity(String),
    #[error("invalid address: {0}")]
    InvalidAddress(String),
    #[error("invalid signature encoding: {0}")]
    InvalidSignature(String),
    #[error("invalid CBOR: {0}")]
    InvalidCbor(String),
    #[error("invalid COSE key: {0}")]
    InvalidCoseKey(String),
    #[error("invalid WebAuthn client data: {0}")]
    InvalidClientData(String),
    #[error("invalid P-256 public key: {0}")]
    InvalidPublicKey(String),
    #[error("ABI signature parse failed: {0}")]
    AbiParse(String),
    #[error("ABI calldata decode failed: {0}")]
    AbiDecode(String),
    #[error("EIP-712 payload parse failed: {0}")]
    Eip712Parse(String),
    #[error("EIP-712 domain type is not the canonical EIP712Domain: {0}")]
    Eip712NonCanonicalDomain(String),
    #[error("identicon seed is unrenderable: {0}")]
    InvalidIdenticonSeed(String),
    // i18n (spec 004-rust-i18n, data-model.md "CoreError extension"). Every variant
    // carries a String so the FFI contract stays uniform — `t([])` has no detail of
    // its own, so it carries caller context instead of becoming the enum's first
    // payload-free variant.
    #[error("t() called with an empty key list: {0}")]
    I18nEmptyKeyList(String),
    #[error("i18n count is not a finite number: {0}")]
    I18nInvalidCount(String),
    #[error("i18n option is not supported by this engine: {0}")]
    I18nUnsupportedOption(String),
    #[error("i18n catalog is not available for locale: {0}")]
    I18nCatalogUnavailable(String),
    #[error("i18n catalog JSON is malformed: {0}")]
    I18nCatalogParse(String),
    #[error("registry metadata is invalid: {0}")]
    RegistryMetadata(String),
    #[error("registry group proof could not be built: {0}")]
    RegistryProof(String),
    #[error("multisig: {0}")]
    Multisig(String),
    #[error("internal invariant violated: {0}")]
    Internal(String),
}

impl CoreError {
    /// Stable machine-readable code, identical across all binding surfaces and
    /// used by conformance vectors' `expect.error` field.
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::InvalidHex(_) => "InvalidHex",
            CoreError::InvalidBase64Url(_) => "InvalidBase64Url",
            CoreError::InvalidQuantity(_) => "InvalidQuantity",
            CoreError::InvalidAddress(_) => "InvalidAddress",
            CoreError::InvalidSignature(_) => "InvalidSignature",
            CoreError::InvalidCbor(_) => "InvalidCbor",
            CoreError::InvalidCoseKey(_) => "InvalidCoseKey",
            CoreError::InvalidClientData(_) => "InvalidClientData",
            CoreError::InvalidPublicKey(_) => "InvalidPublicKey",
            CoreError::AbiParse(_) => "AbiParse",
            CoreError::AbiDecode(_) => "AbiDecode",
            CoreError::Eip712Parse(_) => "Eip712Parse",
            CoreError::Eip712NonCanonicalDomain(_) => "Eip712NonCanonicalDomain",
            CoreError::InvalidIdenticonSeed(_) => "InvalidIdenticonSeed",
            CoreError::I18nEmptyKeyList(_) => "I18nEmptyKeyList",
            CoreError::I18nInvalidCount(_) => "I18nInvalidCount",
            CoreError::I18nUnsupportedOption(_) => "I18nUnsupportedOption",
            CoreError::I18nCatalogUnavailable(_) => "I18nCatalogUnavailable",
            CoreError::I18nCatalogParse(_) => "I18nCatalogParse",
            CoreError::RegistryMetadata(_) => "RegistryMetadata",
            CoreError::RegistryProof(_) => "RegistryProof",
            CoreError::Multisig(_) => "Multisig",
            CoreError::Internal(_) => "Internal",
        }
    }
}
