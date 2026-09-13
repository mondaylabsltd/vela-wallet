//! The parallel space's fixed keyset, for debug Android builds only.
//!
//! Every Vela client has a test environment that is the real app with one
//! substitution: where a passkey would sign, `vela-core`'s fixed P-256 keyset
//! signs (`vela_core::dev_fixtures`, the same scalars the web's
//! `/parallel` and the desktop's `VELA_PARALLEL_SPACE=1` use). Android reaches
//! it through THIS library, not through `vela-core-uniffi`, for one reason:
//! uniffi's Kotlin bindings verify every exported function against the `.so`
//! they load, so one bindings file cannot serve a `.so` with the keyset and a
//! `.so` without it. A second library with its own bindings, both in the debug
//! source set, is what lets a release APK carry neither (spec 043 FR-001).
//!
//! Thin by design: mirror records and `#[uniffi::export]` wrappers, no logic.

uniffi::setup_scaffolding!();

use vela_core::dev_fixtures as fixtures;

#[derive(Debug, thiserror::Error, uniffi::Error)]
pub enum FixtureError {
    #[error("{0}")]
    Failed(String),
}

impl From<vela_core::CoreError> for FixtureError {
    fn from(error: vela_core::CoreError) -> Self {
        FixtureError::Failed(error.to_string())
    }
}

/// One account of the keyset: its position, its stable credential id (the
/// hex of `vela-fixture-0N`, no `0x`), its public point, its single-key Safe.
/// The scalar stays on the Rust side.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FixtureAccountRecord {
    pub index: u32,
    pub credential_id_hex: String,
    pub name: String,
    pub public_key_hex: String,
    pub address: String,
}

/// An assertion minted for a fixture key — the onboarding `Assertion` shape.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FixtureAssertionRecord {
    pub credential_id_hex: String,
    pub signature_der_hex: String,
    pub authenticator_data_hex: String,
    pub client_data_json_hex: String,
}

/// A registration minted for a fixture key — what a create ceremony answers.
#[derive(Debug, Clone, uniffi::Record)]
pub struct FixtureRegistrationRecord {
    pub credential_id_hex: String,
    pub attestation_object_hex: String,
    pub client_data_json_hex: String,
}

fn account_of(account: &fixtures::FixtureAccount) -> FixtureAccountRecord {
    FixtureAccountRecord {
        index: u32::try_from(account.index).unwrap_or(u32::MAX),
        credential_id_hex: account.credential_id_hex.clone(),
        name: account.name.to_owned(),
        public_key_hex: account.public_key_hex.clone(),
        address: account.address.clone(),
    }
}

/// Every account of the keyset, in index order.
#[uniffi::export]
pub fn fixture_accounts() -> Result<Vec<FixtureAccountRecord>, FixtureError> {
    Ok(fixtures::accounts()?.iter().map(account_of).collect())
}

/// The multi-key Safe every account of the keyset co-owns — the golden
/// address the other clients' parallel spaces send from.
#[uniffi::export]
pub fn fixture_multi_address() -> Result<String, FixtureError> {
    Ok(fixtures::multi_address()?)
}

/// Sign `challenge` as a fixture key would: the signer is resolved the way
/// an authenticator resolves an allow-list (`allow_credential_ids`, empty =
/// any), with `preferred` picking among several the way the web's
/// `vela.parallel.signWith(n)` does.
#[uniffi::export]
pub fn fixture_assert(
    challenge: Vec<u8>,
    allow_credential_ids: Vec<String>,
    preferred: Option<u32>,
) -> Result<FixtureAssertionRecord, FixtureError> {
    let account =
        fixtures::resolve_signer(&allow_credential_ids, preferred.map(|index| index as usize))?;
    let signed =
        fixtures::build_assertion(&account, &challenge, fixtures::RP_ID, fixtures::ORIGIN)?;
    Ok(FixtureAssertionRecord {
        credential_id_hex: signed.credential_id_hex,
        signature_der_hex: signed.signature_der_hex,
        authenticator_data_hex: signed.authenticator_data_hex,
        client_data_json_hex: signed.client_data_json_hex,
    })
}

/// A registration for account `index` — what a create ceremony would answer.
#[uniffi::export]
pub fn fixture_registration(index: u32) -> Result<FixtureRegistrationRecord, FixtureError> {
    let account = fixtures::account(index as usize)?;
    let minted = fixtures::build_registration(&account, fixtures::RP_ID, fixtures::ORIGIN)?;
    Ok(FixtureRegistrationRecord {
        credential_id_hex: minted.credential_id_hex,
        attestation_object_hex: minted.attestation_object_hex,
        client_data_json_hex: minted.client_data_json_hex,
    })
}
