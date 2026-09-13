//! Vela Wallet shared computation core.
//!
//! Pure, deterministic, correctness-critical computation only: parsing, encoding,
//! hashing, big-integer math, data assembly/validation. No I/O, no network, no UI,
//! no randomness. Every fallible function returns `Result<_, CoreError>` — never a
//! default value on bad input.
//!
//! Feature spec: `specs/001-rust-core-bindings/` (contracts/core-api.md is the
//! authoritative surface; conformance vectors in `tests/vectors/` are extracted
//! from the production TypeScript implementations and pin byte-identical behavior).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod abi;
/// Portable onboarding state machines. Feature-gated (`--features crux`) so the
/// uniffi bindings — and therefore the iOS/Android binaries — never link the
/// state-machine framework. See `specs/011-crux-onboarding-state/`.
#[cfg(feature = "crux")]
pub mod app;
/// The hybrid / caBLE v2 initiator, sans-IO: QR payload, key derivations, BLE
/// advert decrypt, tunnel-server domain decode. The radio and the socket
/// belong to a platform shell. See `specs/019-onboarding-live-wiring/`.
pub mod cable;
/// A CTAP2 client with no I/O in it: framing, commands, COSE, PIN/UV. The
/// transport belongs to a platform shell; what is identical on every one of
/// them lives here. See `specs/019-onboarding-live-wiring/research.md` D4.
pub mod ctap;
/// The parallel space's fixed keyset: a software P-256 signer that produces
/// the exact WebAuthn bytes a real authenticator would. Feature-gated
/// (`--features dev-fixtures`), DEFAULT OFF, never enabled by the uniffi or
/// wasm crates — see the module note for why the scalars live here at all.
#[cfg(feature = "dev-fixtures")]
pub mod dev_fixtures;
pub mod eip712;
pub mod error;
pub mod i18n;
mod i18n_catalogs;
pub mod identicon;
mod identicon_features;
#[cfg(feature = "identicon-raster")]
pub mod identicon_raster;
pub mod l10n;
pub mod passkey;
mod passkey_catalog;
pub mod primitives;
pub mod registry_metadata;
pub mod registry_proof;
pub mod safe;
pub mod types;
/// The Safe ERC-4337 user operation — calldata, initCode, the SafeOp hash,
/// the WebAuthn signature envelope, the v0.7 wire shape. Pure assembly; the
/// reads and the submit belong to a shell. Written for the desktop in spec
/// 032; on the web (spec 028 Phase 8) it is the second implementation the
/// shell's TypeScript assembly is checked against before a passkey signs.
pub mod user_op;
pub mod webauthn;

pub use abi::AbiValue;
pub use error::CoreError;
pub use i18n::{
    canonical_tag, plural_category, plural_suffix, plural_suffixes, resolve_language, Catalog,
    Category, Count, Dir, I18n, LanguageState, Lookup, Options, OwnedOptions, OwnedVar, PluralMode,
    Scratch, Var,
};
pub use identicon::{
    identicon_data_uri, identicon_params, identicon_svg, identicon_svg_circular, make_hash,
    normalize_seed, Colors, IdenticonHash, IdenticonParams, Section, Sections,
};
#[cfg(feature = "identicon-raster")]
pub use identicon_raster::{
    identicon_placeholder_png, identicon_png, passkey_provider_png, rasterize_svg_png,
};
pub use types::{ClientDataKind, P256PublicKey, SafeAddressInfo, WebAuthnAssertion};
