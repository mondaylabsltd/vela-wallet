//! wasm-bindgen shell over vela-core → the web shell (app-web/vela-wallet).
//!
//! Thin by design: DTO mirrors + `#[wasm_bindgen]` wrappers, zero logic.
//! Loading is synchronous `initSync` over a base64-embedded module (a shape
//! chosen for the retired Expo web build's bundler — see
//! specs/001-rust-core-bindings/research.md D7 — and kept because every
//! remaining reader depends on it), so the TS facade can call into the core
//! without an async gate.
//!
//! Error contract: every fallible export rejects with `{ code, message }`
//! where `code` is the stable `CoreError` variant name the conformance corpus
//! uses — identical classification to the Kotlin/Swift bindings.

use serde::{Deserialize, Serialize};
use tsify::Tsify;
use wasm_bindgen::prelude::*;

/// The onboarding state machines (spec 011-crux-onboarding-state). Unlike the
/// function exports below — pure kernels the app calls — these are stateful
/// cores the web shell drives with events and effect results.
mod bridge;
mod onboarding;
mod settings;
/// Where an account's signatures go: its sign-in key.
mod signing;
/// Only the REGISTRY's relying party: the web wallet offers no Trusted Signer
/// (owner, 2026-09-23), but it can publish a unit whose keys were minted on
/// one.
mod trusted_signer;
mod wallet_state;

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct CoreErrorJs {
    pub code: String,
    pub message: String,
}

fn err(e: vela_core::CoreError) -> JsValue {
    let payload = CoreErrorJs {
        code: e.code().to_owned(),
        message: e.to_string(),
    };
    serde_wasm_bindgen::to_value(&payload).unwrap_or_else(|_| JsValue::from_str(e.code()))
}

type JsResult<T> = Result<T, JsValue>;

// ---------------------------------------------------------------------------
// DTOs (same shapes as the uniffi records; AbiValue stays recursive)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
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

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct P256PublicKey {
    /// 0x-hex, 32 bytes.
    pub x: String,
    /// 0x-hex, 32 bytes.
    pub y: String,
}

impl From<vela_core::P256PublicKey> for P256PublicKey {
    fn from(k: vela_core::P256PublicKey) -> Self {
        P256PublicKey {
            x: vela_core::primitives::to_hex(&k.x, true),
            y: vela_core::primitives::to_hex(&k.y, true),
        }
    }
}

#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct SafeAddressInfo {
    pub address: String,
    /// 0x-hex, 32 bytes.
    pub salt_nonce: String,
    /// 0x-hex.
    pub setup_data: String,
    /// 0x-hex, 32 bytes.
    pub init_code_hash: String,
}

impl From<vela_core::SafeAddressInfo> for SafeAddressInfo {
    fn from(i: vela_core::SafeAddressInfo) -> Self {
        SafeAddressInfo {
            address: i.address,
            salt_nonce: vela_core::primitives::to_hex(&i.salt_nonce, true),
            setup_data: vela_core::primitives::to_hex(&i.setup_data, true),
            init_code_hash: vela_core::primitives::to_hex(&i.init_code_hash, true),
        }
    }
}

// ---------------------------------------------------------------------------
// primitives
// ---------------------------------------------------------------------------

#[wasm_bindgen]
pub fn keccak256(data: &[u8]) -> Vec<u8> {
    vela_core::primitives::keccak256(data)
}

#[wasm_bindgen]
pub fn sha256(data: &[u8]) -> Vec<u8> {
    vela_core::primitives::sha256(data)
}

#[wasm_bindgen(js_name = toHex)]
pub fn to_hex(data: &[u8], prefixed: bool) -> String {
    vela_core::primitives::to_hex(data, prefixed)
}

#[wasm_bindgen(js_name = fromHex)]
pub fn from_hex(s: &str) -> JsResult<Vec<u8>> {
    vela_core::primitives::from_hex(s).map_err(err)
}

#[wasm_bindgen(js_name = toQuantity)]
pub fn to_quantity(value: &str) -> JsResult<String> {
    vela_core::primitives::to_quantity(value).map_err(err)
}

#[wasm_bindgen(js_name = checksumAddress)]
pub fn checksum_address(address_hex: &str) -> JsResult<String> {
    vela_core::primitives::checksum_address(address_hex).map_err(err)
}

#[wasm_bindgen(js_name = functionSelector)]
pub fn function_selector(signature: &str) -> JsResult<Vec<u8>> {
    vela_core::primitives::function_selector(signature).map_err(err)
}

#[wasm_bindgen(js_name = create2Address)]
pub fn create2_address(deployer_hex: &str, salt: &[u8], init_code_hash: &[u8]) -> JsResult<String> {
    vela_core::primitives::create2_address(deployer_hex, salt, init_code_hash).map_err(err)
}

#[wasm_bindgen(js_name = toBase64Url)]
pub fn to_base64url(data: &[u8]) -> String {
    vela_core::primitives::to_base64url(data)
}

#[wasm_bindgen(js_name = fromBase64Url)]
pub fn from_base64url(s: &str) -> JsResult<Vec<u8>> {
    vela_core::primitives::from_base64url(s).map_err(err)
}

#[wasm_bindgen(js_name = abiEncodeAddress)]
pub fn abi_encode_address(address_hex: &str) -> JsResult<Vec<u8>> {
    vela_core::primitives::abi_encode_address(address_hex).map_err(err)
}

#[wasm_bindgen(js_name = abiEncodeUint256)]
pub fn abi_encode_uint256(value_hex: &str) -> JsResult<Vec<u8>> {
    vela_core::primitives::abi_encode_uint256(value_hex).map_err(err)
}

#[wasm_bindgen(js_name = abiEncodeBytes32)]
pub fn abi_encode_bytes32(data: &[u8]) -> JsResult<Vec<u8>> {
    vela_core::primitives::abi_encode_bytes32(data).map_err(err)
}

// ---------------------------------------------------------------------------
// abi
// ---------------------------------------------------------------------------

#[wasm_bindgen(js_name = canonicalizeSignature)]
pub fn canonicalize_signature(sig: &str) -> JsResult<String> {
    vela_core::abi::canonicalize_signature(sig).map_err(err)
}

#[wasm_bindgen(js_name = computeSelector)]
pub fn compute_selector(sig: &str) -> JsResult<String> {
    vela_core::abi::compute_selector(sig).map_err(err)
}

#[wasm_bindgen(js_name = matchSelector)]
pub fn match_selector(sig: &str, calldata: &[u8]) -> JsResult<bool> {
    vela_core::abi::match_selector(sig, calldata).map_err(err)
}

#[wasm_bindgen(js_name = decodeCalldata)]
pub fn decode_calldata(sig: &str, calldata: &[u8]) -> JsResult<AbiValue> {
    vela_core::abi::decode_calldata(sig, calldata)
        .map(Into::into)
        .map_err(err)
}

// ---------------------------------------------------------------------------
// eip712
// ---------------------------------------------------------------------------

#[wasm_bindgen(js_name = hashTypedData)]
pub fn hash_typed_data(typed_data_json: &str) -> JsResult<Vec<u8>> {
    vela_core::eip712::hash_typed_data(typed_data_json).map_err(err)
}

/// What a site's message request asks the account to sign, before the
/// Safe's `SafeMessage` wrap — the phones' `sign_message_hash`.
#[wasm_bindgen(js_name = signMessageHash)]
pub fn sign_message_hash(method: &str, params_json: &str) -> Option<Vec<u8>> {
    vela_core::sign_message::original_hash(method, params_json)
}

#[wasm_bindgen(js_name = encodeType)]
pub fn encode_type(typed_data_json: &str) -> JsResult<String> {
    vela_core::eip712::encode_type(typed_data_json).map_err(err)
}

// ---------------------------------------------------------------------------
// safe
// ---------------------------------------------------------------------------

#[wasm_bindgen(js_name = parsePublicKey)]
pub fn parse_public_key(hex: &str) -> JsResult<P256PublicKey> {
    vela_core::safe::parse_public_key(hex)
        .map(Into::into)
        .map_err(err)
}

#[wasm_bindgen(js_name = computeSafeAddress)]
pub fn compute_safe_address(x: &[u8], y: &[u8]) -> JsResult<SafeAddressInfo> {
    vela_core::safe::compute_safe_address(x, y)
        .map(Into::into)
        .map_err(err)
}

/// Multi-device Safe: `keys_xy` is a concatenation of 64-byte x‖y blocks,
/// one per key — raw coordinates only, same byte convention as the
/// single-key `computeSafeAddress`. NOT hex strings: a bare-hex form would be
/// ambiguous for keys whose x starts with byte 0x04 (the SEC1-tag strip in
/// `parsePublicKey`). Key 0 drives the shared signer; later keys become
/// factory signer owners, their proxies deployed inside the setup MultiSend.
#[wasm_bindgen(js_name = computeSafeAddressMulti)]
pub fn compute_safe_address_multi(keys_xy: &[u8]) -> JsResult<SafeAddressInfo> {
    if keys_xy.is_empty() || keys_xy.len() % 64 != 0 {
        return Err(err(vela_core::CoreError::InvalidPublicKey(format!(
            "expected a non-empty multiple of 64 bytes of x‖y blocks, got {}",
            keys_xy.len()
        ))));
    }
    let keys: Vec<vela_core::P256PublicKey> = keys_xy
        .chunks(64)
        .map(|block| vela_core::P256PublicKey {
            x: block[..32].to_vec(),
            y: block[32..].to_vec(),
        })
        .collect();
    vela_core::safe::compute_safe_address_multi(&keys)
        .map(Into::into)
        .map_err(err)
}

#[wasm_bindgen(js_name = computeWebauthnSignerAddress)]
pub fn compute_webauthn_signer_address(x: &[u8], y: &[u8]) -> JsResult<String> {
    vela_core::safe::compute_webauthn_signer_address(x, y).map_err(err)
}

#[wasm_bindgen(js_name = computeSplitterAddress)]
pub fn compute_splitter_address(treasury_hex: &str) -> JsResult<String> {
    vela_core::safe::compute_splitter_address(treasury_hex).map_err(err)
}

#[wasm_bindgen(js_name = encodeSplitterDeployCall)]
pub fn encode_splitter_deploy_call(treasury_hex: &str) -> JsResult<Vec<u8>> {
    vela_core::safe::encode_splitter_deploy_call(treasury_hex).map_err(err)
}

#[wasm_bindgen(js_name = safeProxyRuntimeCode)]
pub fn safe_proxy_runtime_code() -> JsResult<String> {
    vela_core::safe::safe_proxy_runtime_code().map_err(err)
}

// ---------------------------------------------------------------------------
// webauthn
// ---------------------------------------------------------------------------

#[wasm_bindgen(js_name = extractAttestationPublicKey)]
pub fn extract_attestation_public_key(attestation_object: &[u8]) -> JsResult<P256PublicKey> {
    vela_core::webauthn::extract_attestation_public_key(attestation_object)
        .map(Into::into)
        .map_err(err)
}

#[wasm_bindgen(js_name = derSignatureToRawLowS)]
pub fn der_signature_to_raw_low_s(der: &[u8]) -> JsResult<Vec<u8>> {
    vela_core::webauthn::der_signature_to_raw_low_s(der).map_err(err)
}

/// `kind` is `"create"` or `"get"` (anything else errors — the caller is
/// choosing which contract-mirrored rule set applies).
#[wasm_bindgen(js_name = validateClientData)]
pub fn validate_client_data(
    kind: &str,
    client_data_json: &[u8],
    authenticator_data: &[u8],
) -> JsResult<()> {
    let kind = match kind {
        "create" => vela_core::ClientDataKind::Create,
        "get" => vela_core::ClientDataKind::Get,
        other => {
            return Err(err(vela_core::CoreError::InvalidClientData(format!(
                "unknown client data kind `{other}`"
            ))))
        }
    };
    vela_core::webauthn::validate_client_data(kind, client_data_json, authenticator_data)
        .map_err(err)
}

#[wasm_bindgen(js_name = webauthnSigningHash)]
pub fn webauthn_signing_hash(authenticator_data: &[u8], client_data_json: &[u8]) -> Vec<u8> {
    vela_core::webauthn::webauthn_signing_hash(authenticator_data, client_data_json)
}

/// Returns `null` when the two assertions do not pin down exactly one key
/// (different credentials, or the same signature twice) — that is a legitimate
/// outcome, not an error.
#[wasm_bindgen(js_name = recoverPublicKeyFromAssertions)]
pub fn recover_public_key_from_assertions(
    a_authenticator_data: &[u8],
    a_client_data_json: &[u8],
    a_signature_der: &[u8],
    b_authenticator_data: &[u8],
    b_client_data_json: &[u8],
    b_signature_der: &[u8],
) -> JsResult<Option<P256PublicKey>> {
    let a = vela_core::WebAuthnAssertion {
        authenticator_data: a_authenticator_data.to_vec(),
        client_data_json: a_client_data_json.to_vec(),
        signature_der: a_signature_der.to_vec(),
    };
    let b = vela_core::WebAuthnAssertion {
        authenticator_data: b_authenticator_data.to_vec(),
        client_data_json: b_client_data_json.to_vec(),
        signature_der: b_signature_der.to_vec(),
    };
    vela_core::webauthn::recover_public_key_from_assertions(&a, &b)
        .map(|opt| opt.map(Into::into))
        .map_err(err)
}

// ---------------------------------------------------------------------------
// p256-index registry (group/member proofs + metadata blob)
// ---------------------------------------------------------------------------

/// Mirror of `registry_proof::RegistryProof` — the WebAuthn-shaped proof the
/// registry contract verifies.
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct RegistryProofJs {
    pub authenticator_data: String,
    #[serde(rename = "clientDataJSON")]
    pub client_data_json: String,
    pub challenge_index: u32,
    pub type_index: u32,
    pub r: String,
    pub s: String,
}

impl From<vela_core::registry_proof::RegistryProof> for RegistryProofJs {
    fn from(proof: vela_core::registry_proof::RegistryProof) -> Self {
        RegistryProofJs {
            authenticator_data: proof.authenticator_data,
            client_data_json: proof.client_data_json,
            challenge_index: proof.challenge_index,
            type_index: proof.type_index,
            r: proof.r,
            s: proof.s,
        }
    }
}

/// Mirror of `registry_proof::GroupProof`.
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct GroupProofJs {
    pub group_public_key_hex: String,
    pub proof: RegistryProofJs,
}

impl From<vela_core::registry_proof::GroupProof> for GroupProofJs {
    fn from(built: vela_core::registry_proof::GroupProof) -> Self {
        GroupProofJs {
            group_public_key_hex: built.group_public_key_hex,
            proof: built.proof.into(),
        }
    }
}

/// Input for `encodeRegistryMetadata`; `version` is supplied by the core.
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
#[serde(rename_all = "camelCase")]
pub struct RegistryMetadataInput {
    pub address: String,
    pub wallet_version: String,
    pub key_names: Vec<String>,
    pub created_at_iso: String,
}

/// The uncompressed public key of the one-time group key a 32-byte seed
/// derives — needed before requesting the group's challenge.
#[wasm_bindgen(js_name = groupPublicKeyFromSeed)]
pub fn group_public_key_from_seed(seed_hex: &str) -> JsResult<String> {
    vela_core::registry_proof::group_public_key_from_seed(seed_hex).map_err(err)
}

/// Derive the one-time group key from a 32-byte seed and build its closing
/// proof over the group's content-hash challenge.
#[wasm_bindgen(js_name = buildGroupProof)]
pub fn build_group_proof(
    seed_hex: &str,
    rp_id: &str,
    challenge_hex: &str,
) -> JsResult<GroupProofJs> {
    vela_core::registry_proof::build_group_proof(seed_hex, rp_id, challenge_hex)
        .map(Into::into)
        .map_err(err)
}

/// Assemble a member passkey's proof from its real WebAuthn assertion.
#[wasm_bindgen(js_name = buildMemberProof)]
pub fn build_member_proof(
    authenticator_data_hex: &str,
    client_data_json_hex: &str,
    signature_der_hex: &str,
) -> JsResult<RegistryProofJs> {
    vela_core::registry_proof::build_member_proof(
        authenticator_data_hex,
        client_data_json_hex,
        signature_der_hex,
    )
    .map(Into::into)
    .map_err(err)
}

/// Encode the wallet's registry metadata blob to `0x`-hex, bounded to the
/// contract's 2048-byte cap.
#[wasm_bindgen(js_name = encodeRegistryMetadata)]
pub fn encode_registry_metadata(input: RegistryMetadataInput) -> JsResult<String> {
    let meta = vela_core::registry_metadata::RegistryMetadata {
        version: vela_core::registry_metadata::REGISTRY_METADATA_VERSION,
        address: input.address,
        wallet_version: input.wallet_version,
        key_names: input.key_names,
        created_at_iso: input.created_at_iso,
    };
    meta.encode_hex().map_err(err)
}

// ---------------------------------------------------------------------------
// identicon (spec 003-rust-identicon, contracts/identicon-api.md)
// ---------------------------------------------------------------------------

/// Flattened `IdenticonParams` — the same shape `getIdenticonsParams` returns in
/// the JS library, so migrating call sites stay recognisable.
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
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

/// **The wallet's identicon.** Circular variant, no SVG ids — several instances can
/// share one DOM without their clip paths colliding.
#[wasm_bindgen(js_name = identiconSvgCircular)]
pub fn identicon_svg_circular(seed: &str) -> JsResult<String> {
    vela_core::identicon::identicon_svg_circular(seed).map_err(err)
}

/// The library's stock hexagonal output.
#[wasm_bindgen(js_name = identiconSvg)]
pub fn identicon_svg(seed: &str) -> JsResult<String> {
    vela_core::identicon::identicon_svg(seed).map_err(err)
}

/// Stock output as a `data:image/svg+xml;base64,…` URI.
#[wasm_bindgen(js_name = identiconDataUri)]
pub fn identicon_data_uri(seed: &str) -> JsResult<String> {
    vela_core::identicon::identicon_data_uri(seed).map_err(err)
}

#[wasm_bindgen(js_name = identiconParams)]
pub fn identicon_params(seed: &str) -> JsResult<IdenticonParams> {
    vela_core::identicon::identicon_params(seed)
        .map(Into::into)
        .map_err(err)
}

#[wasm_bindgen(js_name = identiconMakeHash)]
pub fn identicon_make_hash(seed: &str) -> String {
    vela_core::identicon::make_hash(seed).as_str().to_owned()
}

/// Case- and length-normalises a seed. Every platform must call this rather than
/// lowercasing locally — that is how the platforms drift apart.
#[wasm_bindgen(js_name = identiconNormalizeSeed)]
pub fn identicon_normalize_seed(seed: &str) -> String {
    vela_core::identicon::normalize_seed(seed).into_owned()
}

// ---------------------------------------------------------------------------
// Passkey providers (`vela_core::passkey`)
// ---------------------------------------------------------------------------

/// **A passkey provider's mark**, as an `image/svg+xml` data URI, from the
/// vendored AAGUID catalog. `undefined` when the catalog does not know the
/// model — the caller then shows what it showed before this existed.
///
/// A data URI rather than markup to inline: these marks carry `<style>` blocks
/// and `clipPath` ids, and several of them inlined into one document would
/// fight over both. The lookup is offline by construction — asking a directory
/// service would tell it which vault holds a Vela wallet's key.
#[wasm_bindgen(js_name = passkeyProviderIconDataUri)]
#[must_use]
pub fn passkey_provider_icon_data_uri(aaguid: &str, dark: bool) -> Option<String> {
    vela_core::passkey::provider_icon_data_uri(aaguid, dark)
}

/// **Signing in when the index is gone** (spec 062): the registry contract's
/// side of the index's two read questions, answered in the index's own JSON
/// shapes so a shell's existing parsing runs unchanged. `…Plan` = the chains to
/// try (in order) and the two `eth_call`s to make on one of them; the matching
/// function turns the two raw results into the body, or nothing when that
/// chain did not really answer (the shell then tries the next). Unit ids are
/// per deployment: ask about a key's units on the chain that listed them.
#[wasm_bindgen(js_name = registryChainKeyPlan)]
#[must_use]
pub fn registry_chain_key_plan(public_key_hex: &str) -> Option<String> {
    vela_core::registry_chain::key_status_plan_json(public_key_hex)
}

/// See [`registry_chain_key_plan`].
#[wasm_bindgen(js_name = registryChainKeyStatus)]
#[must_use]
pub fn registry_chain_key_status(has_entry_hex: &str, groups_hex: &str) -> Option<String> {
    vela_core::registry_chain::key_status_json(has_entry_hex, groups_hex)
}

/// See [`registry_chain_key_plan`].
#[wasm_bindgen(js_name = registryChainUnitPlan)]
#[must_use]
pub fn registry_chain_unit_plan(unit_id: u32) -> Option<String> {
    vela_core::registry_chain::unit_plan_json(u64::from(unit_id))
}

/// See [`registry_chain_key_plan`].
#[wasm_bindgen(js_name = registryChainUnit)]
#[must_use]
pub fn registry_chain_unit(unit_id: u32, unit_hex: &str, members_hex: &str) -> Option<String> {
    vela_core::registry_chain::unit_json(u64::from(unit_id), unit_hex, members_hex)
}

/// Sign-in's first question — which groups does this key belong to? — through
/// the three layers: the index, then the registry contract on Gnosis, then on
/// Ethereum. See `vela_core::registry_resolve`.
#[wasm_bindgen(js_name = registryResolveKeyStep)]
#[must_use]
pub fn registry_resolve_key_step(public_key_hex: &str, answers_json: &str) -> String {
    vela_core::registry_resolve::key_step_json(public_key_hex, answers_json)
}

/// Sign-in's second question — who are this group's members? — with an index
/// answer PROVED against the chain (`contentHash`), not merely believed.
/// `source` is the token the key step returned.
#[wasm_bindgen(js_name = registryResolveUnitStep)]
#[must_use]
pub fn registry_resolve_unit_step(unit_id: u32, source: &str, answers_json: &str) -> String {
    vela_core::registry_resolve::unit_step_json(u64::from(unit_id), source, answers_json)
}

/// Which passkeys control the wallet at `address` — the Settings keys view
/// (spec 062). `device_keys_json` is the account record's `keys` array (or a
/// one-element array built from the legacy scalars); the answer is `ask` with
/// `eth_call`s to perform, or `done` with the rows and where they came from.
/// `sign_in_credential` is the account's sign-in route credential
/// (`signInRoute`), empty for none: its row is marked `signs_here`.
#[wasm_bindgen(js_name = walletKeysStep)]
#[must_use]
pub fn wallet_keys_step(
    address: &str,
    device_keys_json: &str,
    answers_json: &str,
    sign_in_credential: &str,
) -> String {
    vela_core::wallet_keys::step_json(address, device_keys_json, answers_json, sign_in_credential)
}

/// **Backing the founding record up to Ethereum — the next step of the walk**
/// (spec 062). Server-free: every request is an `eth_call` against the
/// registry contract, on Gnosis (where the record lives) or Ethereum (where
/// the backup goes). `answers_json` is the transcript (`LookupAnswer[]`, the
/// same shape `registryNameStep` uses); the return is a `BackupStep` as JSON —
/// requests to perform, or the verdict and, when the wallet is not backed up,
/// the one call that would do it. No passkey is involved at any point.
/// `target_chain` is Ethereum when absent; Base (8453) is the operator's
/// rehearsal deployment; anything else is refused.
#[wasm_bindgen(js_name = registryBackupStep)]
#[must_use]
pub fn registry_backup_step(
    address: &str,
    founding_public_key_hex: &str,
    answers_json: &str,
    target_chain: Option<u32>,
) -> String {
    vela_core::registry_backup::step_json(
        address,
        founding_public_key_hex,
        target_chain,
        answers_json,
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
#[wasm_bindgen(js_name = registryNameStep)]
#[must_use]
pub fn registry_name_step(address: &str, answers_json: &str) -> String {
    vela_core::registry_lookup::step_json(address, answers_json)
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
/// `registryNameStep` uses); the return is a `VerifyStep` as JSON: `eth_call`s
/// to perform, or the verdict plus the name exactly as it was proven. The
/// shell owns the transport; every rule is the core's.
#[wasm_bindgen(js_name = verifiedNameStep)]
#[must_use]
pub fn verified_name_step(
    chain_id: u32,
    registry: &str,
    address: &str,
    name: &str,
    answers_json: &str,
) -> String {
    vela_core::app::name_verify::step_json(chain_id, registry, address, name, answers_json)
}

/// **Where to ask about a model the compiled catalog cannot name**, or
/// `undefined` when there is nothing to ask: a malformed or all-zero AAGUID, or
/// one the catalog already answers offline.
///
/// `origin` is the directory node the person's service-endpoint settings
/// name (spec 038 #E4); absent, the crate's default.
#[wasm_bindgen(js_name = passkeyDirectoryUrl)]
#[must_use]
pub fn passkey_directory_url(aaguid: &str, origin: Option<String>) -> Option<String> {
    match origin {
        Some(origin) => vela_core::passkey::directory_lookup_url_at(&origin, aaguid),
        None => vela_core::passkey::directory_lookup_url(aaguid),
    }
}

/// **Read a directory answer.** `undefined` unless the body is about the AAGUID
/// that was asked about and carries a usable name; `iconUrl` is present only
/// when the path is the service's own shape.
#[wasm_bindgen(js_name = passkeyDirectoryEntry)]
pub fn passkey_directory_entry(
    aaguid: &str,
    json: &str,
    dark: bool,
    origin: Option<String>,
) -> JsResult<JsValue> {
    let entry = match origin {
        Some(origin) => vela_core::passkey::directory_entry_at(&origin, aaguid, json, dark),
        None => vela_core::passkey::directory_entry(aaguid, json, dark),
    };
    let Some(entry) = entry else {
        return Ok(JsValue::UNDEFINED);
    };
    serde_wasm_bindgen::to_value(&entry).map_err(|e| JsValue::from_str(&e.to_string()))
}

/// **The security-key fallback mark**, as an `image/svg+xml` data URI, for a
/// key whose AAGUID the catalog cannot name. `undefined` when the row deserves
/// no mark of this kind — a platform authenticator, which the client already
/// draws its own way.
///
/// The three colours are the caller's tokens: the artwork ships in one theme,
/// and one vendor's greys are not this app's greys in either.
#[wasm_bindgen(js_name = passkeyFallbackIconDataUri)]
#[must_use]
pub fn passkey_fallback_icon_data_uri(
    authenticator_attachment: &str,
    transports: &str,
    chose_security_key: bool,
    strong: &str,
    soft: &str,
    hole: &str,
) -> Option<String> {
    let mark = vela_core::passkey::fallback_mark(
        authenticator_attachment,
        transports,
        chose_security_key,
    )?;
    Some(vela_core::passkey::fallback_icon_data_uri(
        mark,
        vela_core::passkey::MarkPalette { strong, soft, hole },
    ))
}

/// The provider's brand name, or an empty string when the catalog has no entry.
#[wasm_bindgen(js_name = passkeyProviderName)]
#[must_use]
pub fn passkey_provider_name(aaguid: &str) -> String {
    vela_core::passkey::provider_name(aaguid)
        .unwrap_or_default()
        .to_owned()
}

// ---------------------------------------------------------------------------
// Native-coin price selection (`vela_core::app::balance_dashboard`)
// ---------------------------------------------------------------------------
//
// The floor under the home hero number and under every per-row fiat value: get
// the native coin's USD price wrong and every holding priced through it is
// wrong too (X Layer's WOKB reads $5 out of a near-empty pool and $81 out of
// the liquid one). The rules — the deepest-pool max within one stable, the
// cross-stable max, the DEX/Chainlink sanity band and the source ladder —
// have always lived in `balance_dashboard.rs`; these three exports are how the
// web shell finally executes them instead of re-deciding in TypeScript.
//
// Pure kernels, not a machine: the shell still owns the multicall, the ABI
// decode and the log line. It hands over decoded numbers and gets a verdict.

/// One stable's DEX quotes for 1 native coin, as the shell decodes them out of
/// the multicall. Each stable is its own group because its own `decimals()`
/// normalizes the amount — USDC (6) and DAI (18) must never be compared under
/// one shared scale.
#[derive(Serialize, Deserialize, Tsify)]
pub struct NativeQuoteGroup {
    /// Successful quote outputs in THIS stable's base units, as decimal
    /// strings (failed calls are simply absent).
    #[serde(rename = "amountsOut")]
    pub amounts_out: Vec<String>,
    /// This stable's `decimals()` read; `null` = the read failed, and the core
    /// applies its own `DEFAULT_QUOTE_DECIMALS`.
    #[serde(rename = "quoteDecimals")]
    pub quote_decimals: Option<u32>,
}

/// Wrapper so the group list crosses the boundary as one value.
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(from_wasm_abi)]
pub struct NativeQuoteGroups {
    pub groups: Vec<NativeQuoteGroup>,
}

/// The chosen price and the rung of the ladder it came from. `source` is the
/// `NativePriceSource` variant name; `"none"` when nothing could price.
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi)]
pub struct NativePriceChoice {
    pub price: Option<f64>,
    pub source: String,
}

fn source_name(source: vela_core::app::balance_dashboard::NativePriceSource) -> &'static str {
    use vela_core::app::balance_dashboard::NativePriceSource as S;
    match source {
        S::Dex => "dex",
        S::ChainlinkSanity => "chainlinkSanity",
        S::ChainlinkLocal => "chainlinkLocal",
        S::ChainlinkEth => "chainlinkEth",
    }
}

/// The deepest pool across ALL stable quotes — `best_native_dex_price`, which
/// folds `best_group_price` over each group.
#[wasm_bindgen(js_name = bestNativeDexPrice)]
pub fn best_native_dex_price(groups: NativeQuoteGroups) -> Option<f64> {
    let groups: Vec<vela_core::app::balance_dashboard::NativeQuoteGroup> = groups
        .groups
        .into_iter()
        .map(|g| vela_core::app::balance_dashboard::NativeQuoteGroup {
            amounts_out: g.amounts_out,
            quote_decimals: g.quote_decimals,
        })
        .collect();
    vela_core::app::balance_dashboard::best_native_dex_price(&groups)
}

/// The lowest gas price a chain accepts, as a decimal string (money crosses
/// this boundary as decimal strings, never as JS numbers).
///
/// `"0"` on every chain but Arc, which discards an underpriced transaction
/// SILENTLY — no error, no trace, a payment that simply never happens. The web
/// shell keeps a TypeScript twin of the gas-price derivation
/// (`safe-transaction.ts`), and it reads the floor from here rather than
/// carrying its own copy of the number.
#[wasm_bindgen(js_name = minGasPriceWei)]
pub fn min_gas_price_wei(chain_id: u32) -> String {
    vela_core::app::fee_policy::min_gas_price_wei(chain_id).to_string()
}

/// Spec 077: how long a submitted operation usually takes to land on a chain,
/// in seconds — `0` where Vela ships no estimate for it.
///
/// The send receipt has had this number since spec 038 (#D3), through the
/// core's own `SendReceiptView`. A dApp transaction lands on the SAME receipt
/// but arrives through `tx_tracker`, whose entries carry no estimate — so the
/// web shell reads it from the core's table here rather than carrying a second
/// copy of twenty-four numbers that would quietly drift.
///
/// `0` rather than `undefined` because that is what the receipt already does
/// with "no estimate": the ring circles instead of filling, which is the honest
/// drawing of a wallet that does not know.
#[wasm_bindgen(js_name = typicalInclusionSeconds)]
#[must_use]
pub fn typical_inclusion_seconds(chain_id: u32) -> u32 {
    vela_core::app::network_admin::typical_inclusion_s(chain_id).map_or(0, u32::from)
}

/// Issue 212: how long a chain's fee signals may be held, in ms — the one
/// number every shell's cache used to carry its own copy of.
#[wasm_bindgen(js_name = feeSignalsCacheTtlMs)]
#[must_use]
pub fn fee_signals_cache_ttl_ms() -> u32 {
    vela_core::app::fee_policy::FEE_SIGNALS_CACHE_TTL_MS
}

/// Issue 212: may this gas-signal read be held? Decimal wei in; see
/// `fee_policy::gas_signals_cacheable`.
#[wasm_bindgen(js_name = gasSignalsCacheable)]
#[must_use]
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
#[wasm_bindgen(js_name = bundlerQuoteCacheable)]
#[must_use]
pub fn bundler_quote_cacheable(max_fee_per_gas: &str) -> bool {
    vela_core::app::fee_policy::bundler_quote_cacheable(max_fee_per_gas)
}

/// The $1 peg for a native gas coin that IS a dollar stablecoin — Tempo's
/// `USD`, Arc's `USDC`. `None` means "not pegged": the caller falls through to
/// the Chainlink/DEX ladder unchanged.
///
/// This exists so the rule is written once. It used to be a `symbol == "USD"`
/// literal in each of the four shells, which is four chances to disagree about
/// what a coin is worth.
#[wasm_bindgen(js_name = peggedNativeUsd)]
pub fn pegged_native_usd(symbol: &str) -> Option<f64> {
    vela_core::app::balance_dashboard::pegged_native_usd(symbol)
}

/// The source ladder and its sanity band — `choose_native_price`.
#[wasm_bindgen(js_name = chooseNativePrice)]
pub fn choose_native_price(
    dex: Option<f64>,
    chainlink_local: Option<f64>,
    chainlink_eth: Option<f64>,
) -> NativePriceChoice {
    match vela_core::app::balance_dashboard::choose_native_price(
        dex,
        chainlink_local,
        chainlink_eth,
    ) {
        Some(chosen) => NativePriceChoice {
            price: Some(chosen.price),
            source: source_name(chosen.source).to_owned(),
        },
        None => NativePriceChoice {
            price: None,
            source: "none".to_owned(),
        },
    }
}

// ---------------------------------------------------------------------------
// i18n (spec 004-rust-i18n, contracts/i18n-api.md §1.3 / §2.3)
// ---------------------------------------------------------------------------
//
// ENGINE ONLY — no catalogs are compiled in (T047). All 15 measured 1,315,023
// wasm bytes against a 1,000,000 ceiling, and even one locale costs more over the
// wire compiled in (+31,862 brotli'd) than fetched as plain JSON (15,353). The web
// route fetches `/i18n/<lng>.json` and hands the bytes to `loadCatalog`.
//
// No lock, unlike the uniffi shell: `wasm_bindgen` exports `&mut self` directly and
// the module is single-threaded.

/// Per-call translation options, shaped so a TS caller writes the i18next object
/// literal verbatim — `{ count: 3, name: 'Alice' }`. The reserved names are typed;
/// everything else falls into `vars` through `#[serde(flatten)]`.
#[derive(Serialize, Deserialize, Tsify, Default)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct TOptions {
    /// Untyped: i18next accepts a number, a string (which silently DISABLES plural
    /// handling), `null`, an object, or a BigInt (which makes it throw). Typing
    /// this as `f64` would reject inputs the oracle accepts.
    ///
    /// Double `Option` because `Option<Value>` collapses an explicit JSON `null`
    /// into `None`, which would make `count: null` indistinguishable from an absent
    /// count — and upstream those differ: `null` still pluralises (`Number(null)`
    /// is 0), while absent does not.
    #[serde(default, deserialize_with = "deserialize_present")]
    pub count: Option<Option<CountValue>>,
    /// Untyped for the same reason — a numeric context is coerced, not rejected.
    #[serde(default)]
    pub context: Option<serde_json::Value>,
    /// Untyped because i18next accepts a string, a number, a boolean, an object
    /// or an array here, and the last two are non-strings this engine rejects
    /// rather than approximates.
    #[serde(default, rename = "defaultValue")]
    pub default_value: Option<serde_json::Value>,
    /// Per-call language override. **Not** `changeLanguage`: `zh_TW` resolves to
    /// `zh` there and falls through to English here.
    #[serde(default)]
    pub lng: Option<String>,
    #[serde(default)]
    pub ordinal: bool,
    /// Per-call namespace override. Anything but `translation` misses.
    #[serde(default)]
    pub ns: Option<String>,
    /// `keySeparator: false` — look the key up as ONE literal property.
    #[serde(default, rename = "keySeparator")]
    pub key_separator: Option<serde_json::Value>,
    /// `nsSeparator: false` — a `:` in the key is not a namespace separator.
    #[serde(default, rename = "nsSeparator")]
    pub ns_separator: Option<serde_json::Value>,
    /// When present and an object, `replace` REPLACES the options as the
    /// interpolation source (`i18next.js:1180`) — a top-level `v` is shadowed
    /// rather than merged.
    #[serde(default)]
    pub replace: Option<ReplaceArg>,
    /// Options i18next answers with a NON-string. A Rust `t()` is string-typed by
    /// construction, so these are typed errors, not silent coercions.
    #[serde(default, rename = "returnObjects")]
    pub return_objects: Option<bool>,
    #[serde(default, rename = "returnDetails")]
    pub return_details: Option<bool>,
    #[serde(default, rename = "joinArrays")]
    pub join_arrays: Option<serde_json::Value>,
    /// Every other key becomes an interpolation variable, so the call site does not
    /// have to know which names are reserved.
    #[serde(flatten)]
    pub vars: std::collections::BTreeMap<String, VarValue>,
}

impl TOptions {
    fn to_owned_options(&self) -> vela_core::i18n::OwnedOptions {
        use vela_core::i18n::{Count, OwnedVar};
        vela_core::i18n::OwnedOptions {
            count: self.count.as_ref().and_then(|present| match present {
                None => Some(Count::Null),
                Some(CountValue::Num(n)) => Some(Count::Num(*n)),
                // A STRING count silently disables plural resolution upstream.
                Some(CountValue::Str(s)) => Some(Count::Str(s.clone())),
                Some(CountValue::Other(v)) => match v {
                    serde_json::Value::Number(n) => {
                        Some(Count::Num(n.as_f64().unwrap_or(f64::NAN)))
                    }
                    serde_json::Value::String(s) => Some(Count::Str(s.clone())),
                    serde_json::Value::Null => Some(Count::Null),
                    serde_json::Value::Bool(b) => Some(Count::Num(if *b { 1.0 } else { 0.0 })),
                    serde_json::Value::Object(o) => {
                        match o.get("__t").and_then(serde_json::Value::as_str) {
                            Some("nan") => Some(Count::Num(f64::NAN)),
                            Some("infinity") => Some(Count::Num(
                                if o.get("sign").and_then(serde_json::Value::as_i64) == Some(-1) {
                                    f64::NEG_INFINITY
                                } else {
                                    f64::INFINITY
                                },
                            )),
                            Some("bigint") => Some(Count::BigInt(
                                o.get("v")
                                    .and_then(serde_json::Value::as_str)
                                    .and_then(|s| s.parse().ok())
                                    .unwrap_or(0),
                            )),
                            // An own property that is `undefined` is NOT a count at all.
                            Some("undefined") => None,
                            _ => Some(Count::Object),
                        }
                    }
                    serde_json::Value::Array(_) => Some(Count::Object),
                },
            }),
            context: self.context.as_ref().map(|v| match v {
                serde_json::Value::String(s) => s.clone(),
                other => other.to_string(),
            }),
            default_value: match &self.default_value {
                Some(serde_json::Value::String(s)) => Some(s.clone()),
                Some(serde_json::Value::Number(n)) => Some(n.to_string()),
                Some(serde_json::Value::Bool(b)) => Some(b.to_string()),
                _ => None,
            },
            // An object or array default is a non-string, so `t()` answers with the
            // branch diagnostic — EXCEPT a tagged `undefined`, which is an absent
            // default and makes the key echo instead.
            default_value_object: match &self.default_value {
                Some(serde_json::Value::Object(o)) => {
                    o.get("__t").and_then(serde_json::Value::as_str) != Some("undefined")
                }
                Some(serde_json::Value::Array(_)) => self.join_arrays.is_none(),
                _ => false,
            },
            unsupported: {
                let mut u = Vec::new();
                if self.return_objects == Some(true) {
                    u.push("returnObjects".to_owned());
                }
                if self.return_details == Some(true) {
                    u.push("returnDetails".to_owned());
                }
                if self.join_arrays.is_some()
                    && matches!(self.default_value, Some(serde_json::Value::Array(_)))
                {
                    u.push("joinArrays".to_owned());
                }
                // A value carrying its own `toString` stringifies through host
                // semantics Rust cannot reach.
                if self.vars.values().any(|v| host_only(&v.as_json())) {
                    u.push("hostOnlyValue".to_owned());
                }
                u
            },
            lng: self.lng.clone(),
            ordinal: self.ordinal,
            ns: self.ns.clone(),
            key_separator_off: self
                .key_separator
                .as_ref()
                .is_some_and(|v| v == &serde_json::Value::Bool(false)),
            ns_separator_off: self
                .ns_separator
                .as_ref()
                .is_some_and(|v| v == &serde_json::Value::Bool(false)),
            vars: match self.replace.as_ref() {
                // An object `replace` REPLACES the options as the interpolation
                // source; anything else leaves the flattened vars in place.
                Some(ReplaceArg::Map(m)) => m,
                _ => &self.vars,
            }
            .iter()
            .flat_map(|(k, v)| {
                // A non-finite number has no `serde_json::Value` form, so it is
                // matched BEFORE dropping to the JSON view (spec 005 FR-024).
                let json = v.as_json();
                let var = match v {
                    VarValue::Num(n) => OwnedVar::Num(*n),
                    // JS string-coercion semantics, so `{{v}}` renders what the
                    // template literal would have.
                    VarValue::Other(j) => match j {
                        serde_json::Value::Null => OwnedVar::Null,
                        serde_json::Value::Bool(b) => OwnedVar::Bool(*b),
                        serde_json::Value::Number(n) => {
                            OwnedVar::Num(n.as_f64().unwrap_or(f64::NAN))
                        }
                        serde_json::Value::String(s) => OwnedVar::Str(s.clone()),
                        // `Array.prototype.join(",")` flattens nested arrays:
                        // `[[1],[2]]` is `"1,2"`, not `"[1],[2]"`.
                        serde_json::Value::Array(_) => OwnedVar::Array(js_join(j)),
                        // The tagged encodings for values JSON cannot carry.
                        serde_json::Value::Object(o) => {
                            match o.get("__t").and_then(serde_json::Value::as_str) {
                                Some("undefined") => OwnedVar::Undefined,
                                Some("nan") => OwnedVar::Num(f64::NAN),
                                Some("infinity") => OwnedVar::Num(
                                    if o.get("sign").and_then(serde_json::Value::as_i64) == Some(-1)
                                    {
                                        f64::NEG_INFINITY
                                    } else {
                                        f64::INFINITY
                                    },
                                ),
                                Some("bigint") => OwnedVar::Str(
                                    o.get("v")
                                        .and_then(serde_json::Value::as_str)
                                        .unwrap_or_default()
                                        .to_owned(),
                                ),
                                _ => OwnedVar::Object,
                            }
                        }
                    },
                };
                // A nested object is BOTH `[object Object]` under its own name
                // and a source of dotted names, so `{{a.b.c}}` resolves.
                let mut out = vec![(k.clone(), var)];
                flatten_dotted(k, json.as_ref(), &mut out);
                out
            })
            .filter(|(k, _)| !k.starts_with("defaultValue_"))
            .collect(),
            default_value_variants: self
                .vars
                .iter()
                .filter_map(|(k, v)| {
                    // `defaultValue_one`, `defaultValue_many`, … arrive through the
                    // flattened map because only the bare `defaultValue` is typed.
                    let cat = k.strip_prefix("defaultValue_")?;
                    // Only a string is a usable variant; a non-finite number has no
                    // JSON form and is not one, so `as_json`'s null stand-in is fine.
                    let text = match v {
                        VarValue::Other(serde_json::Value::String(s)) => s.clone(),
                        _ => String::new(),
                    };
                    Some((cat.to_owned(), text))
                })
                .collect(),
        }
    }
}

/// A `count` as it arrives from JS.
///
/// `serde_json::Value` cannot hold `Infinity` or `NaN` — JSON has no syntax for
/// them, so `serde_wasm_bindgen` turns both into `null`. That silently rendered
/// `{{count}}` as the empty string where i18next renders `"Infinity"`. The
/// committed corpus never caught it, because it encodes those values with a
/// `{"__t":"infinity"}` tag and so never exercises the raw-number path a real
/// caller takes. `scripts/verify-i18n-parity.mjs`'s fuzz pass did.
///
/// Untagged, with `f64` FIRST: a JS number deserialises straight into `f64`,
/// non-finite values included, before the `Value` arm can flatten it.
#[derive(Serialize, Deserialize, Tsify)]
#[serde(untagged)]
pub enum CountValue {
    Num(f64),
    Str(String),
    Other(serde_json::Value),
}

/// An interpolation variable as it arrives from JS.
///
/// Same defect as `CountValue`, same device — and it took a second sighting to
/// notice the fix had been applied to `count` alone. Every OTHER variable still
/// went through `serde_json::Value`, so `t('time.minutesShort', { n: NaN })`
/// rendered `"分前"` where i18next renders `"NaN分前"`. That one is reachable in
/// production: `src/services/activity.ts:116` passes `{ n: Math.round(diff / 60) }`.
///
/// The corpus cannot catch this class at all — it encodes non-finite values as
/// `{"__t":"nan"}` and decodes the tag back on the Rust side, so a vector never
/// crosses the raw-number boundary a live caller crosses (spec 005 FR-024).
#[derive(Serialize, Deserialize, Tsify)]
#[serde(untagged)]
pub enum VarValue {
    /// FIRST, so a non-finite JS number lands here rather than flattening to null.
    Num(f64),
    Other(serde_json::Value),
}

/// `replace`, which when it is an object REPLACES the options as the
/// interpolation source (`i18next.js:1180`). Typed as a map of [`VarValue`] so a
/// non-finite value survives that route too — the 005 adapter deliberately routes
/// through `replace` when normalising an own-but-undefined `count`.
#[derive(Serialize, Deserialize, Tsify)]
#[serde(untagged)]
pub enum ReplaceArg {
    Map(std::collections::BTreeMap<String, VarValue>),
    /// i18next ignores a non-object `replace`.
    Other(serde_json::Value),
}

impl VarValue {
    fn as_json(&self) -> std::borrow::Cow<'_, serde_json::Value> {
        match self {
            VarValue::Num(n) => serde_json::Number::from_f64(*n).map_or_else(
                // Non-finite has no JSON form; the caller only uses this for
                // dotted-path flattening and `defaultValue_*`, neither of which a
                // non-finite number participates in.
                || std::borrow::Cow::Owned(serde_json::Value::Null),
                |n| std::borrow::Cow::Owned(serde_json::Value::Number(n)),
            ),
            VarValue::Other(v) => std::borrow::Cow::Borrowed(v),
        }
    }
}

/// Deserialise a present field into `Some(_)`, so an explicit JSON `null` is
/// distinguishable from an absent key.
fn deserialize_present<'de, D>(d: D) -> Result<Option<Option<CountValue>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    serde::Deserialize::deserialize(d).map(Some)
}

/// `Array.prototype.join(",")` semantics, flattening nested arrays.
fn js_join(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Array(a) => a.iter().map(js_join).collect::<Vec<_>>().join(","),
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Whether a value stringifies through host semantics with no Rust analogue — a
/// Decode per-call options from a raw `JsValue`.
///
/// Deliberately **not** `opts: Option<TOptions>` in the signature, which would be
/// the obvious spelling. wasm-bindgen takes the `&self` borrow *before* it
/// converts the remaining arguments, and tsify's failure path throws out of Rust
/// without unwinding — so a single rejected option leaked the borrow guard and
/// left every `&mut self` method (`changeLanguage`, `loadCatalog`) permanently
/// dead with `recursive use of an object detected`. `t()` kept working, which is
/// what made it so hard to see: the UI pinned to the boot language while
/// `i18n.language` moved (spec 005 FR-023).
///
/// Decoding here returns `Err` through the normal path, so the guard drops.
///
/// The TS parameter widens from `TOptions | null` to `any` as a result. That is
/// no loss: tsify emits `TOptions` as `interface TOptions extends Map<string, Value>`
/// because of the flattened `vars`, which rejects every real object literal at
/// compile time — the type was a lie, and callers cast at their own boundary.
fn parse_options(opts: Option<&JsValue>) -> Result<TOptions, JsValue> {
    let Some(opts) = opts.filter(|v| !v.is_undefined() && !v.is_null()) else {
        return Ok(TOptions::default());
    };
    serde_wasm_bindgen::from_value(opts.clone()).map_err(|e| {
        // Classified as unsupported rather than a new variant: an option the
        // decoder cannot represent is one this engine does not support, and the
        // code is already in the corpus's error vocabulary.
        err(vela_core::CoreError::I18nUnsupportedOption(e.to_string()))
    })
}

/// JS `Date`, a callable, or an object carrying its own `toString`. The dumper
/// tags these, and the check is recursive because the tag can sit one level down.
fn host_only(v: &serde_json::Value) -> bool {
    match v {
        serde_json::Value::Object(o) => {
            matches!(
                o.get("__t").and_then(serde_json::Value::as_str),
                Some("date" | "fn")
            ) || o.values().any(host_only)
        }
        _ => false,
    }
}

/// Expand a nested option object into dotted variable names.
fn flatten_dotted(
    prefix: &str,
    v: &serde_json::Value,
    out: &mut Vec<(String, vela_core::i18n::OwnedVar)>,
) {
    use vela_core::i18n::OwnedVar;
    if let serde_json::Value::Object(map) = v {
        for (k, inner) in map {
            let name = format!("{prefix}.{k}");
            let var = match inner {
                serde_json::Value::String(s) => OwnedVar::Str(s.clone()),
                serde_json::Value::Number(n) => OwnedVar::Num(n.as_f64().unwrap_or(f64::NAN)),
                serde_json::Value::Bool(b) => OwnedVar::Bool(*b),
                serde_json::Value::Null => OwnedVar::Null,
                _ => OwnedVar::Object,
            };
            out.push((name.clone(), var));
            flatten_dotted(&name, inner, out);
        }
    }
}

/// The resolve state after a language change.
#[derive(Serialize, Deserialize, Tsify)]
#[tsify(into_wasm_abi, from_wasm_abi)]
pub struct LanguageState {
    pub language: String,
    #[serde(rename = "resolvedLanguage")]
    pub resolved_language: Option<String>,
    pub languages: Vec<String>,
}

/// A translation engine.
#[wasm_bindgen]
pub struct I18n {
    inner: vela_core::i18n::I18n,
}

#[wasm_bindgen]
impl I18n {
    /// Build from the `en` fallback catalog, supplied as the bytes of
    /// `/i18n/en.json`.
    #[wasm_bindgen(constructor)]
    pub fn new(fallback_json: &[u8]) -> Result<I18n, JsValue> {
        let en = vela_core::i18n::Catalog::from_json("en", fallback_json).map_err(err)?;
        let engine = vela_core::i18n::I18n::new(en).map_err(err)?;
        Ok(I18n { inner: engine })
    }

    /// Build an engine pinned to the LEGACY plural rule — i18next's `dummyRule`,
    /// which is what a host without `Intl.PluralRules` silently falls back to.
    /// Exposed so the conformance corpus can replay MODE B here too; production
    /// code should never call it.
    #[wasm_bindgen(js_name = newWithLegacyPlurals)]
    pub fn new_with_legacy_plurals(fallback_json: &[u8]) -> Result<I18n, JsValue> {
        let en = vela_core::i18n::Catalog::from_json("en", fallback_json).map_err(err)?;
        let engine = vela_core::i18n::I18n::new(en)
            .map_err(err)?
            .with_plural_mode(vela_core::i18n::PluralMode::Legacy);
        Ok(I18n { inner: engine })
    }

    /// First key that resolves wins; all-missing returns the **last** key.
    #[wasm_bindgen(js_name = tFirst)]
    pub fn t_first(&self, keys: Vec<String>, opts: Option<JsValue>) -> Result<String, JsValue> {
        let owned = parse_options(opts.as_ref())?.to_owned_options();
        let mut scratch = vela_core::i18n::Scratch::default();
        let borrowed = owned.as_options(&mut scratch);
        let refs: Vec<&str> = keys.iter().map(String::as_str).collect();
        self.inner.t_first(&refs, &borrowed).map_err(err)
    }

    /// Resolve `key`. Returns the key itself when nothing matches.
    pub fn t(&self, key: &str, opts: Option<JsValue>) -> Result<String, JsValue> {
        let owned = parse_options(opts.as_ref())?.to_owned_options();
        let mut scratch = vela_core::i18n::Scratch::default();
        let borrowed = owned.as_options(&mut scratch);
        self.inner.t(key, &borrowed).map_err(err)
    }

    pub fn exists(&self, key: &str, opts: Option<JsValue>) -> Result<bool, JsValue> {
        let owned = parse_options(opts.as_ref())?.to_owned_options();
        let mut scratch = vela_core::i18n::Scratch::default();
        let borrowed = owned.as_options(&mut scratch);
        Ok(self.inner.exists(key, &borrowed))
    }

    #[wasm_bindgen(js_name = changeLanguage)]
    pub fn change_language(&mut self, lng: &str) -> LanguageState {
        let s = self.inner.change_language(lng);
        LanguageState {
            language: s.language,
            resolved_language: s.resolved_language,
            languages: s.languages,
        }
    }

    /// Make `lang`'s catalog active — the on-demand load.
    #[wasm_bindgen(js_name = loadCatalog)]
    pub fn load_catalog(&mut self, lang: &str, json: &[u8]) -> Result<(), JsValue> {
        let catalog = vela_core::i18n::Catalog::from_json(lang, json).map_err(err)?;
        self.inner.load_catalog(catalog);
        Ok(())
    }

    /// Release `lang` if it is the active catalog. `en` is never releasable.
    #[wasm_bindgen(js_name = releaseCatalog)]
    pub fn release_catalog(&mut self, lang: &str) -> bool {
        self.inner.release_catalog(lang).is_some()
    }

    #[wasm_bindgen(js_name = residentLocales)]
    pub fn resident_locales(&self) -> Vec<String> {
        self.inner
            .resident_locales()
            .into_iter()
            .map(str::to_owned)
            .collect()
    }

    #[wasm_bindgen(js_name = residentBytes)]
    pub fn resident_bytes(&self) -> usize {
        self.inner.resident_bytes()
    }

    pub fn language(&self) -> String {
        self.inner.language().to_owned()
    }

    pub fn dir(&self) -> String {
        self.inner.dir().as_str().to_owned()
    }
}

/// Interpolate a template in isolation, without a key lookup.
#[wasm_bindgen(js_name = i18nInterpolate)]
pub fn i18n_interpolate(template: &str, opts: Option<TOptions>) -> Result<String, JsValue> {
    let owned = opts.unwrap_or_default().to_owned_options();
    let mut scratch = vela_core::i18n::Scratch::default();
    let borrowed = owned.as_options(&mut scratch);
    vela_core::i18n::interpolate(template, &borrowed).map_err(err)
}

#[wasm_bindgen(js_name = i18nPluralSuffix)]
pub fn i18n_plural_suffix(locale: &str, count: f64) -> String {
    vela_core::i18n::plural_suffix(locale, count)
}

#[wasm_bindgen(js_name = i18nPluralSuffixes)]
pub fn i18n_plural_suffixes(locale: &str) -> Vec<String> {
    vela_core::i18n::plural_suffixes(locale)
}

#[wasm_bindgen(js_name = i18nPluralSuffixLegacy)]
pub fn i18n_plural_suffix_legacy(count: f64) -> String {
    vela_core::i18n::plural_suffix_legacy(count)
}

#[wasm_bindgen(js_name = i18nPluralSuffixesLegacy)]
pub fn i18n_plural_suffixes_legacy() -> Vec<String> {
    vela_core::i18n::plural_suffixes_legacy()
}

#[wasm_bindgen(js_name = i18nTextDirection)]
pub fn i18n_text_direction(lng: &str) -> String {
    vela_core::l10n::text_direction(lng).as_str().to_owned()
}

/// An amount field's text as the core reads it, or `undefined` for a paste
/// with no reading as one figure (spec 073; `l10n::amount_text`).
#[wasm_bindgen(js_name = amountTextClean)]
pub fn amount_text_clean(
    raw: &str,
    number: &str,
    previous: Option<String>,
    pasted: Option<bool>,
) -> Option<String> {
    use vela_core::l10n::amount_text;
    amount_text::clean(
        raw,
        amount_text::preset_of(number),
        amount_text::Entry::from_pasted(pasted),
        previous.as_deref(),
    )
}

/// Where the caret belongs in `clean`, having been at `caret` in `raw`
/// (UTF-16 units, as `selectionStart` counts).
#[wasm_bindgen(js_name = amountTextCaret)]
pub fn amount_text_caret(raw: &str, clean: &str, caret: u32) -> u32 {
    let at = vela_core::l10n::amount_text::caret_after_clean(raw, clean, caret as usize);
    u32::try_from(at).unwrap_or(u32::MAX)
}

// ---------------------------------------------------------------------------
// user_op — the second implementation the shell's assembly is checked against
// (spec 028 Phase 8). The shell hands over the operation it built and the
// calls it SHOWED; the core rebuilds the calldata from those calls, refuses if
// the bytes differ, and answers the SafeOp hash the passkey may sign.
// ---------------------------------------------------------------------------

/// One sub-call as the shell built it. `value_hex` and `data_hex` may carry a
/// `0x` or not; empty means zero / no data.
#[derive(Deserialize)]
struct AttestCall {
    to: String,
    value_hex: String,
    data_hex: String,
}

/// The in-band fee leg by its INPUTS. The core builds the leg itself, so a
/// shell that changed the recipient or the amount after the confirm is caught
/// here — not only a shell that changed the bytes.
#[derive(Deserialize)]
struct AttestFeeLeg {
    gas_fee_token: Option<String>,
    recipient: String,
    amount_hex: String,
}

#[derive(Deserialize)]
struct AttestCalls {
    inner: Vec<AttestCall>,
    fee: Option<AttestFeeLeg>,
    always_multi_send: bool,
}

/// The shell's operation, every field as it will be hashed. Gas fields are
/// decimal strings (JavaScript bigints); byte fields are hex.
#[derive(Deserialize)]
struct AttestOp {
    sender: String,
    nonce: String,
    init_code_hex: String,
    call_data_hex: String,
    verification_gas_limit: String,
    call_gas_limit: String,
    pre_verification_gas: String,
    max_fee_per_gas: String,
    max_priority_fee_per_gas: String,
    paymaster_and_data_hex: String,
}

fn attest_bytes(hex: &str) -> Result<Vec<u8>, vela_core::CoreError> {
    let bare = hex.strip_prefix("0x").unwrap_or(hex);
    if bare.is_empty() {
        return Ok(Vec::new());
    }
    vela_core::primitives::from_hex(bare)
}

fn attest_decimal(value: &str, field: &str) -> Result<u128, vela_core::CoreError> {
    value
        .parse::<u128>()
        .map_err(|_| vela_core::CoreError::InvalidQuantity(format!("{field}: {value}")))
}

fn attest_hex_amount(value: &str) -> Result<u128, vela_core::CoreError> {
    let bare = value.strip_prefix("0x").unwrap_or(value);
    if bare.is_empty() {
        return Ok(0);
    }
    u128::from_str_radix(bare, 16)
        .map_err(|_| vela_core::CoreError::InvalidQuantity(format!("fee amount: {value}")))
}

fn attest_safe_op_hash_inner(
    op_json: &str,
    calls_json: &str,
    chain_id: u64,
) -> Result<Vec<u8>, vela_core::CoreError> {
    use vela_core::user_op;
    let op: AttestOp = serde_json::from_str(op_json)
        .map_err(|e| vela_core::CoreError::Internal(format!("attest: operation: {e}")))?;
    let call_data = attest_bytes(&op.call_data_hex)?;

    // An empty description means "hash only": the legacy path hands the core
    // finished calldata and nothing it was built from.
    if !calls_json.is_empty() {
        let calls: AttestCalls = serde_json::from_str(calls_json)
            .map_err(|e| vela_core::CoreError::Internal(format!("attest: calls: {e}")))?;
        let mut legs: Vec<user_op::MultiSendCall> = Vec::with_capacity(calls.inner.len() + 1);
        for call in calls.inner {
            legs.push(user_op::MultiSendCall {
                to: call.to,
                value_hex: call.value_hex,
                data: attest_bytes(&call.data_hex)?,
            });
        }
        if let Some(fee) = calls.fee {
            legs.push(user_op::build_in_band_fee_leg(
                fee.gas_fee_token.as_deref(),
                &fee.recipient,
                attest_hex_amount(&fee.amount_hex)?,
            )?);
        }
        let expected = user_op::build_native_call_data(&legs, calls.always_multi_send)?;
        if expected != call_data {
            return Err(vela_core::CoreError::Internal(
                "attest: the operation's calldata is not the calls that were shown".to_owned(),
            ));
        }
    }

    let user_op = attest_operation(op, call_data)?;
    user_op::calculate_safe_op_hash(&user_op, chain_id)
}

/// The shell's operation as the core's `UserOperation`, the signature left
/// empty (neither hash covers it). `call_data` is `op.call_data_hex`, already
/// decoded by the caller.
fn attest_operation(
    op: AttestOp,
    call_data: Vec<u8>,
) -> Result<vela_core::user_op::UserOperation, vela_core::CoreError> {
    Ok(vela_core::user_op::UserOperation {
        init_code: attest_bytes(&op.init_code_hex)?,
        call_data,
        verification_gas_limit: attest_decimal(&op.verification_gas_limit, "verificationGasLimit")?,
        call_gas_limit: attest_decimal(&op.call_gas_limit, "callGasLimit")?,
        pre_verification_gas: attest_decimal(&op.pre_verification_gas, "preVerificationGas")?,
        max_fee_per_gas: attest_decimal(&op.max_fee_per_gas, "maxFeePerGas")?,
        max_priority_fee_per_gas: attest_decimal(
            &op.max_priority_fee_per_gas,
            "maxPriorityFeePerGas",
        )?,
        paymaster_and_data: attest_bytes(&op.paymaster_and_data_hex)?,
        sender: op.sender,
        nonce: op.nonce,
        signature: Vec::new(),
    })
}

/// The SafeOp hash of `op_json`, on `chain_id` — after checking that its
/// calldata is exactly what `calls_json` describes (pass `""` to skip the
/// calldata check and attest the hash alone).
#[wasm_bindgen(js_name = attestSafeOpHash)]
pub fn attest_safe_op_hash(op_json: &str, calls_json: &str, chain_id: u64) -> JsResult<Vec<u8>> {
    attest_safe_op_hash_inner(op_json, calls_json, chain_id).map_err(err)
}

/// The Safe message hash a passkey signs for EIP-1271 (`SafeMessage(bytes)`
/// under the Safe's own domain) — the core's reading, for comparison.
#[wasm_bindgen(js_name = attestSafeMessageHash)]
pub fn attest_safe_message_hash(
    original_hash: &[u8],
    chain_id: u64,
    safe_address: &str,
) -> JsResult<Vec<u8>> {
    vela_core::user_op::compute_safe_message_hash(original_hash, chain_id, safe_address)
        .map_err(err)
}

// ---------------------------------------------------------------------------
// The submit verdict (spec 082 RA1, RA6, RA10, ruling 8). The web computes the
// operation's hash before the first POST, asks the core what each POST's
// answer means, and — when the reply is lost — follows the op by that hash
// through the relay's status and the chain's `UserOperationEvent`.
// ---------------------------------------------------------------------------

fn user_op_hash_inner(op_json: &str, chain_id: u64) -> Result<String, vela_core::CoreError> {
    let op: AttestOp = serde_json::from_str(op_json)
        .map_err(|e| vela_core::CoreError::Internal(format!("userOpHash: operation: {e}")))?;
    let call_data = attest_bytes(&op.call_data_hex)?;
    let user_op = attest_operation(op, call_data)?;
    vela_core::user_op::user_op_hash(&user_op, chain_id)
}

/// The EntryPoint v0.7 `getUserOpHash` of `op_json` on `chain_id`,
/// 0x-lowercase — `user_op::user_op_hash`. `op_json` is the operation in the
/// `attestSafeOpHash` shape (gas fields decimal strings, byte fields hex); any
/// signature is ignored, so the hash is known before the passkey signs.
#[wasm_bindgen(js_name = userOpHash)]
pub fn user_op_hash(op_json: &str, chain_id: u64) -> JsResult<String> {
    user_op_hash_inner(op_json, chain_id).map_err(err)
}

/// The reply as the core's `SubmitReply`. The core's own JSON is accepted
/// (`{"hash":"0x…"}`, `{"error":"<error member as JSON text>"}`,
/// `"no_answer"`), and so is an `error` member handed over as the object
/// itself, which is what a JSON-RPC client holds.
fn submit_reply_of(
    reply_json: &str,
) -> Result<vela_core::user_op::SubmitReply, vela_core::CoreError> {
    use vela_core::user_op::SubmitReply;
    let bad = |what: &str| vela_core::CoreError::Internal(format!("userOpSubmitStep: {what}"));
    let value: serde_json::Value =
        serde_json::from_str(reply_json).map_err(|e| bad(&format!("reply: {e}")))?;
    if let Some(error) = value.get("error").filter(|error| !error.is_null()) {
        return Ok(SubmitReply::Error(match error {
            serde_json::Value::String(text) => text.clone(),
            other => other.to_string(),
        }));
    }
    serde_json::from_value(value).map_err(|e| bad(&format!("reply: {e}")))
}

fn user_op_submit_step_inner(
    reply_json: &str,
    attempt: u32,
    maybe_delivered: bool,
    local_hash: &str,
) -> Result<String, vela_core::CoreError> {
    let reply = submit_reply_of(reply_json)?;
    let step = vela_core::user_op::submit_step(&reply, attempt, maybe_delivered, local_hash);
    serde_json::to_string(&step)
        .map_err(|e| vela_core::CoreError::Internal(format!("userOpSubmitStep: {e}")))
}

/// One step of the submit loop — `user_op::submit_step` (RA1). `reply_json`
/// is what the POST came back with (see [`submit_reply_of`]); `attempt` is the
/// 0-based count of POSTs of this op, the one just answered included;
/// `maybe_delivered` is the OR over every POST of this op of the pool's
/// `maybe_delivered`; `local_hash` is [`user_op_hash`]'s answer.
///
/// Answers the core's `SubmitStep` as JSON:
/// `{"retry_after":{"delay_ms":3000}}`, or `{"done":<verdict>}` where the
/// verdict is `{"type":"accepted","user_op_hash":…}`,
/// `{"type":"maybe_sent","user_op_hash":…}` or
/// `{"type":"not_sent","rejection":null|"relayer_unavailable"|"bundler_underfunded"|{"other":…}}`.
/// The local nonce advances on `accepted` only.
#[wasm_bindgen(js_name = userOpSubmitStep)]
pub fn user_op_submit_step(
    reply_json: &str,
    attempt: u32,
    maybe_delivered: bool,
    local_hash: &str,
) -> JsResult<String> {
    user_op_submit_step_inner(reply_json, attempt, maybe_delivered, local_hash).map_err(err)
}

/// The dApp's `-32603` detail for a request that was not sent and has no
/// relay refusal to quote (RA10) — a fixed sentence, never the pool's text.
#[wasm_bindgen(js_name = userOpNotSentDetail)]
#[must_use]
pub fn user_op_not_sent_detail() -> String {
    vela_core::user_op::NOT_SENT_DAPP_DETAIL.to_owned()
}

/// The dApp's `-32603` detail for an operation the relay refused (spec 082
/// RJ3) — a fixed sentence.
#[wasm_bindgen(js_name = userOpRefusedDappDetail)]
#[must_use]
pub fn user_op_refused_dapp_detail() -> String {
    vela_core::user_op::REFUSED_DAPP_DETAIL.to_owned()
}

/// How long the page waits for `ClearToPost` after `OpSigned` before it gives
/// up without POSTing (spec 082 RJ1), in ms.
#[wasm_bindgen(js_name = userOpWriteAheadWaitMs)]
#[must_use]
pub fn user_op_write_ahead_wait_ms() -> u32 {
    vela_core::user_op::WRITE_AHEAD_WAIT_MS
}

/// A failed relay gas estimate, classified (spec 082 RJ19): `error_json` is
/// the JSON-RPC `error` member or the whole body. Answers the core's
/// `EstimateFailure` as JSON — `{"type":"reverts","reason":null|"…"}` or
/// `{"type":"unavailable"}`.
#[wasm_bindgen(js_name = userOpEstimateFailure)]
#[must_use]
pub fn user_op_estimate_failure(error_json: &str) -> String {
    serde_json::to_string(&vela_core::user_op::estimate_failure(error_json))
        .unwrap_or_else(|_| r#"{"type":"unavailable"}"#.to_owned())
}

/// A signed balance change from signed base units, or `undefined` for zero
/// or unreadable text: the token ladder, dust written exactly (never `−0`),
/// U+2212 / `+` (spec 082 RJ15). `preset` is the number preset's wire name;
/// anything else is `comma_dot`.
#[wasm_bindgen(js_name = formatSignedTokenAmount)]
#[must_use]
pub fn format_signed_token_amount(
    delta_base_units: &str,
    decimals: u32,
    preset: &str,
) -> Option<String> {
    use vela_core::l10n::{format_signed_token_amount as format, NumberPreset};
    let preset: NumberPreset =
        serde_json::from_value(serde_json::Value::String(preset.to_owned())).unwrap_or_default();
    format(delta_base_units, decimals, preset)
}

/// `topics[0]` of the `eth_getLogs` filter a `FindOpEvent` asks for (ruling
/// 8): the EntryPoint's `UserOperationEvent`; `topics[1]` is the op's hash.
#[wasm_bindgen(js_name = userOpEventTopic)]
#[must_use]
pub fn user_op_event_topic() -> String {
    vela_core::user_op::USER_OPERATION_EVENT_TOPIC.to_owned()
}

/// The relay's lifecycle-status method (RA7) — the only spelling it serves.
#[wasm_bindgen(js_name = userOpStatusMethod)]
#[must_use]
pub fn user_op_status_method() -> String {
    vela_core::app::tx_tracker::USER_OP_STATUS_METHOD.to_owned()
}

/// The status method's answer — its `result`, or the whole JSON-RPC body —
/// as the core's `TrackStatusAnswer` JSON (`{"status","stage","tx_hash"}`),
/// or `undefined` when it is no answer (an error, or a status the core does
/// not know). `tx_tracker::parse_user_op_status`.
#[wasm_bindgen(js_name = parseUserOpStatus)]
#[must_use]
pub fn parse_user_op_status(json: &str) -> Option<String> {
    vela_core::app::tx_tracker::parse_user_op_status(json)
        .and_then(|answer| serde_json::to_string(&answer).ok())
}

// ---------------------------------------------------------------------------
// dapp_rpc — the routing table the extension's service worker mirrors (spec
// 070). The worker cannot run the core on every page load; a web unit test
// replays its JS table against this one so the two cannot drift.
// ---------------------------------------------------------------------------

/// How a request still pending when the request window goes away is settled,
/// as JSON (`{"code":4900,"reason":"browser_closed"}`) — spec 070 T063.
///
/// 4900, never 4001: a dApp treats an explicit "user rejected" as safe to
/// retry, which double-spends a request that may already have landed. The
/// window asks rather than restating it.
#[wasm_bindgen(js_name = dpermSettleOnClose)]
pub fn dperm_settle_on_close() -> String {
    let (code, reason) = vela_core::app::dapp_permissions::settle_on_close();
    serde_json::json!({ "code": code, "reason": reason }).to_string()
}

/// The core's route for `method`, as JSON (`{"type":"read","bundler":true}`).
#[wasm_bindgen(js_name = dappRpcClassify)]
pub fn dapp_rpc_classify(method: &str) -> String {
    serde_json::to_string(&vela_core::app::dapp_rpc::classify(method))
        .unwrap_or_else(|_| "{\"type\":\"unsupported\"}".to_owned())
}

/// The document-start script an in-app browser injects, for `host`
/// (`"android"` / `"ios"` / `"desktop"`) — exported so the web suite can run
/// the real bridge in a real browser.
#[wasm_bindgen(js_name = dappProviderScript)]
pub fn dapp_provider_script(host: &str) -> String {
    use vela_core::app::dapp_rpc::{provider_script, ProviderHost};
    provider_script(match host {
        "ios" => ProviderHost::Ios,
        "desktop" => ProviderHost::Desktop,
        _ => ProviderHost::Android,
    })
}

// ---------------------------------------------------------------------------
// sign_request — how a dApp request ends, and its clocks (spec 082 RA8, RA12,
// RB2). The sheet draws the ending the tracker knows, never its own guess.
// ---------------------------------------------------------------------------

fn sign_ending_of_inner(
    method: &str,
    payload_json: &str,
    submitted_user_op: Option<&str>,
) -> Result<Option<String>, vela_core::CoreError> {
    use vela_core::app::sign_request::{ending_of, SignResponsePayload};
    let payload: SignResponsePayload = serde_json::from_str(payload_json)
        .map_err(|e| vela_core::CoreError::Internal(format!("signEndingOf: payload: {e}")))?;
    ending_of(method, &payload, submitted_user_op)
        .map(|ending| {
            serde_json::to_string(&ending)
                .map_err(|e| vela_core::CoreError::Internal(format!("signEndingOf: {e}")))
        })
        .transpose()
}

/// The ending of a request whose answer to the page was `payload_json` (the
/// core's `SignResponsePayload` JSON) — `sign_request::ending_of`. Answers
/// the `SignEnding` JSON (`{"type":"signed"}`,
/// `{"type":"landed","tx_hash","user_op_hash"}`,
/// `{"type":"still_confirming","user_op_hash"}`), or `undefined` when there is
/// nothing to show. `submitted_user_op` is the op this request handed the
/// tracker.
#[wasm_bindgen(js_name = signEndingOf)]
pub fn sign_ending_of(
    method: &str,
    payload_json: &str,
    submitted_user_op: Option<String>,
) -> JsResult<Option<String>> {
    sign_ending_of_inner(method, payload_json, submitted_user_op.as_deref()).map_err(err)
}

fn sign_ending_state_inner(
    ending_json: &str,
    entry_json: Option<&str>,
) -> Result<String, vela_core::CoreError> {
    use vela_core::app::sign_request::{ending_state, SignEnding};
    use vela_core::app::tx_tracker::TrackEntryView;
    let bad = |what: String| vela_core::CoreError::Internal(format!("signEndingState: {what}"));
    let ending: SignEnding =
        serde_json::from_str(ending_json).map_err(|e| bad(format!("ending: {e}")))?;
    let entry: Option<TrackEntryView> = match entry_json.map(str::trim) {
        None | Some("") => None,
        Some(json) => serde_json::from_str(json).map_err(|e| bad(format!("entry: {e}")))?,
    };
    serde_json::to_string(&ending_state(&ending, entry.as_ref())).map_err(|e| bad(e.to_string()))
}

/// What the sheet draws for `ending_json` (a `SignEnding`) once the tracker
/// has had its say — `sign_request::ending_state`. `entry_json` is the
/// tracker's `TrackEntryView` for the op (`null`, `undefined` or `""` = not
/// taken yet; an entry for another op is ignored). Answers the
/// `SignEndingState` JSON: `{"type":"signed"}`, `{"type":"confirmed","tx_hash"}`,
/// `{"type":"reverted","tx_hash"}`, `{"type":"not_sent"}` or
/// `{"type":"following","user_op_hash","outcome","fee_held"}`.
#[wasm_bindgen(js_name = signEndingState)]
pub fn sign_ending_state(ending_json: &str, entry_json: Option<String>) -> JsResult<String> {
    sign_ending_state_inner(ending_json, entry_json.as_deref()).map_err(err)
}

/// How long to wait for the receipt when the submit answered `elapsed_ms`
/// after the approve tap: what is left of the 120 s answer window, never less
/// than 10 s — `sign_request::dapp_receipt_wait_ms` (RA12).
#[wasm_bindgen(js_name = dappReceiptWaitMs)]
#[must_use]
pub fn dapp_receipt_wait_ms(elapsed_ms: f64) -> f64 {
    vela_core::app::sign_request::dapp_receipt_wait_ms(elapsed_ms)
}

/// A sign request older than this (ms) is never signed —
/// `sign_request::EXTENSION_REQUEST_TTL_MS`. The extension worker pins its
/// `REQUEST_TTL_MS` to it (RB2).
#[wasm_bindgen(js_name = signRequestTtlMs)]
#[must_use]
pub fn sign_request_ttl_ms() -> f64 {
    vela_core::app::sign_request::EXTENSION_REQUEST_TTL_MS
}

// ---------------------------------------------------------------------------
// rpc_pool — the two clocks the extension worker keeps in JavaScript (RF2).
// The worker cannot load the core; its tests pin its copies to these.
// ---------------------------------------------------------------------------

/// The per-endpoint timeout of a chain read, ms — `rpc_pool::RPC_READ_TIMEOUT_MS`.
#[wasm_bindgen(js_name = rpcReadTimeoutMs)]
#[must_use]
pub fn rpc_read_timeout_ms() -> u32 {
    vela_core::app::rpc_pool::RPC_READ_TIMEOUT_MS
}

/// How long an endpoint rests after `consecutive_failures` failures in a
/// row, ms: `30 s · 2^(n−1)`, capped at 300 s, `0` for none —
/// `rpc_pool::cooldown_ms`.
#[wasm_bindgen(js_name = rpcCooldownMs)]
#[must_use]
pub fn rpc_cooldown_ms(consecutive_failures: u32) -> f64 {
    vela_core::app::rpc_pool::cooldown_ms(consecutive_failures)
}

// ---------------------------------------------------------------------------
// Site names, logo misses and the balance read (spec 082 RE7, RE9, RE10)
// ---------------------------------------------------------------------------

/// A site's name and the line under it — `browser_load::site_label` (RE7):
/// `{"name","host_line"}`, `host_line` `null` when the name already is the
/// host, so it is said once.
#[wasm_bindgen(js_name = browserSiteLabel)]
#[must_use]
pub fn browser_site_label(title: &str, host: &str) -> String {
    serde_json::to_string(&vela_core::app::browser_load::site_label(title, host))
        .unwrap_or_default()
}

/// The class of a logo miss: the HTTP status when the miss had one (it is the
/// stronger evidence), else `kind` as the core's `MarkMiss` wire name
/// (`"not_found"`, `"refused"`, `"not_an_image"`, `"throttled"`,
/// `"server_error"`, `"transport"`, `"unknown"`); a name the core does not know
/// is `unknown`.
fn mark_miss_of(kind: &str, status: Option<u16>) -> vela_core::app::remote_mark::MarkMiss {
    use vela_core::app::remote_mark::{mark_miss_of_status, MarkMiss};
    match status {
        Some(status) => mark_miss_of_status(status),
        None => serde_json::from_value(serde_json::Value::String(kind.to_owned()))
            .unwrap_or(MarkMiss::Unknown),
    }
}

/// How long a logo that did not load stays failed, ms — `undefined` for the
/// session (asking again will not help), a number for a miss that may heal
/// (RE10). The web's `<img onerror>` has no status: `markMissTtlMs("unknown")`.
/// See [`mark_miss_of`] for `kind` and `status`.
#[wasm_bindgen(js_name = markMissTtlMs)]
#[must_use]
pub fn mark_miss_ttl_ms(kind: &str, status: Option<u16>) -> Option<u32> {
    vela_core::app::remote_mark::mark_miss_ttl_ms(mark_miss_of(kind, status))
}

fn balance_read_plan_inner(
    chain_id: u32,
    stables_json: &str,
    wrapped_native: Option<&str>,
    custom_json: &str,
) -> Result<String, vela_core::CoreError> {
    use vela_core::app::balance_dashboard::{read_plan, StableRef, TokenRef};
    let bad = |what: String| vela_core::CoreError::Internal(format!("balanceReadPlan: {what}"));
    fn list<T: serde::de::DeserializeOwned>(json: &str) -> Result<Vec<T>, serde_json::Error> {
        if json.trim().is_empty() {
            return Ok(Vec::new());
        }
        serde_json::from_str(json)
    }
    let stables: Vec<StableRef> = list(stables_json).map_err(|e| bad(format!("stables: {e}")))?;
    let custom: Vec<TokenRef> = list(custom_json).map_err(|e| bad(format!("custom: {e}")))?;
    let plan = read_plan(chain_id, &stables, wrapped_native, &custom);
    serde_json::to_string(&plan).map_err(|e| bad(e.to_string()))
}

/// Which balances one chain's read covers, in order —
/// `balance_dashboard::read_plan` (RE9): the native coin, the registry
/// stablecoins, the wrapped native, the person's own tokens, each contract
/// once. `stables_json` is the chain data's `stables[]` (`[{symbol,
/// contract}]`), `custom_json` the person's tokens on this chain
/// (`[{contract, symbol, name?, decimals}]`); `""` is an empty list.
/// Answers a JSON array of `ReadSlot`
/// (`{kind, contract, symbol, name, known_decimals, peg_usd}`).
#[wasm_bindgen(js_name = balanceReadPlan)]
pub fn balance_read_plan(
    chain_id: u32,
    stables_json: &str,
    wrapped_native: Option<String>,
    custom_json: &str,
) -> JsResult<String> {
    balance_read_plan_inner(
        chain_id,
        stables_json,
        wrapped_native.as_deref(),
        custom_json,
    )
    .map_err(err)
}

// ---------------------------------------------------------------------------
// Tests: each 082 export answers what the core function it wraps answers, in
// the JSON shape its doc names. The `_inner` functions carry the fallible
// ones — a `JsValue` cannot be built off wasm.
// ---------------------------------------------------------------------------

#[cfg(test)]
mod core_082_exports {
    use super::*;
    use serde_json::{json, Value};

    /// Round 2 (T196): the new exports answer the core's own values.
    #[test]
    fn the_round_2_exports_are_the_core_s() {
        assert_eq!(
            user_op_refused_dapp_detail(),
            vela_core::user_op::REFUSED_DAPP_DETAIL
        );
        assert_eq!(user_op_write_ahead_wait_ms(), 5_000);
        assert_eq!(
            parse(&user_op_estimate_failure(
                r#"{"code":-32500,"message":"UserOperation simulation failed","data":"Safe execution failed: the target call in executeUserOp reverted"}"#
            )),
            json!({ "type": "reverts", "reason": null })
        );
        assert_eq!(
            parse(&user_op_estimate_failure("")),
            json!({ "type": "unavailable" })
        );
        assert_eq!(signing::fee_requote_timeout_ms(), 6_000);
        assert_eq!(signing::fee_requote_delay_ms("quote_unavailable", 4), Some(8_000));
        let chain = r#"{"chain_read":{"rate_limited":true}}"#;
        assert_eq!(signing::fee_requote_delay_ms(chain, 2), Some(6_000));
        assert_eq!(
            signing::fee_failure_reason_key(chain).as_deref(),
            Some("home.balanceDetailStatusRetrying")
        );
        assert_eq!(signing::fee_failure_reason_key("calculation_failed"), None);
        assert_eq!(
            format_signed_token_amount("-1000", 18, "dot_comma").as_deref(),
            Some("\u{2212}0,000000000000001")
        );
        assert_eq!(format_signed_token_amount("0", 18, "comma_dot"), None);
    }

    const SAFE: &str = "0x1111111111111111111111111111111111111111";
    const OP_HASH: &str = "0x2222222222222222222222222222222222222222222222222222222222222222";

    fn parse(text: &str) -> Value {
        serde_json::from_str(text).unwrap_or_else(|e| unreachable!("{text}: {e}"))
    }

    fn ok<T>(result: Result<T, vela_core::CoreError>) -> T {
        result.unwrap_or_else(|e| unreachable!("{e}"))
    }

    fn op_json() -> String {
        json!({
            "sender": SAFE,
            "nonce": "0x29",
            "init_code_hex": "0x",
            "call_data_hex": "0x541d63c80102",
            "verification_gas_limit": "300000",
            "call_gas_limit": "450000",
            "pre_verification_gas": "60000",
            "max_fee_per_gas": "2000000000",
            "max_priority_fee_per_gas": "1000000000",
            "paymaster_and_data_hex": "0x3333333333333333333333333333333333333333abcd",
            "signature": "0xabab"
        })
        .to_string()
    }

    #[test]
    fn the_op_hash_is_the_core_s_and_ignores_the_signature() {
        let core = ok(vela_core::user_op::user_op_hash(
            &vela_core::user_op::UserOperation {
                sender: SAFE.to_owned(),
                nonce: "0x29".to_owned(),
                init_code: Vec::new(),
                call_data: vec![0x54, 0x1d, 0x63, 0xc8, 0x01, 0x02],
                verification_gas_limit: 300_000,
                call_gas_limit: 450_000,
                pre_verification_gas: 60_000,
                max_fee_per_gas: 2_000_000_000,
                max_priority_fee_per_gas: 1_000_000_000,
                paymaster_and_data: ok(vela_core::primitives::from_hex(
                    "3333333333333333333333333333333333333333abcd",
                )),
                signature: vec![0xcd; 65],
            },
            100,
        ));
        assert_eq!(ok(user_op_hash_inner(&op_json(), 100)), core);
        assert_ne!(ok(user_op_hash_inner(&op_json(), 1)), core);
        assert!(user_op_hash_inner("{}", 100).is_err());
    }

    #[test]
    fn a_submit_reply_becomes_the_core_s_step() {
        let step = |reply: &str, attempt: u32, maybe: bool| {
            parse(&ok(user_op_submit_step_inner(
                reply, attempt, maybe, OP_HASH,
            )))
        };
        let relay_hash = format!("0x{}", "ab".repeat(32));
        assert_eq!(
            step(&json!({ "hash": relay_hash }).to_string(), 0, false),
            json!({"done": {"type": "accepted", "user_op_hash": relay_hash}})
        );
        assert_eq!(
            step("\"no_answer\"", 0, false),
            json!({"done": {"type": "not_sent", "rejection": null}})
        );
        assert_eq!(
            step("\"no_answer\"", 0, true),
            json!({"done": {"type": "maybe_sent", "user_op_hash": OP_HASH}})
        );
        // The error member as the object a client holds, and as the core's
        // JSON text, are one reply.
        let busy = json!({"code": -32000, "message": "currently processing, Retry later"});
        let as_object = json!({ "error": busy }).to_string();
        let as_text = json!({ "error": busy.to_string() }).to_string();
        assert_eq!(
            step(&as_object, 0, false),
            json!({"retry_after": {"delay_ms": vela_core::user_op::SUBMIT_RETRY_DELAY_MS}})
        );
        assert_eq!(step(&as_object, 3, false), step(&as_text, 3, false));
        let existing =
            json!({"error": {"message": format!("AA25 invalid nonce [existingHash:{OP_HASH}]")}});
        assert_eq!(
            step(&existing.to_string(), 1, false),
            json!({"done": {"type": "accepted", "user_op_hash": OP_HASH}})
        );
        let refused = json!({"error": {"message": "The gas relayer is unavailable right now."}});
        assert_eq!(
            step(&refused.to_string(), 0, false),
            json!({"done": {"type": "not_sent", "rejection": "relayer_unavailable"}})
        );
        assert!(user_op_submit_step_inner("{\"other\":1}", 0, false, OP_HASH).is_err());
        assert!(user_op_submit_step_inner("not json", 0, false, OP_HASH).is_err());
        // A null `error` is no error member: not a reply the core can read.
        assert!(user_op_submit_step_inner("{\"error\":null}", 0, false, OP_HASH).is_err());
    }

    #[test]
    fn the_fixed_words_are_the_core_s() {
        assert_eq!(
            user_op_not_sent_detail(),
            vela_core::user_op::NOT_SENT_DAPP_DETAIL
        );
        assert_eq!(
            user_op_event_topic(),
            vela_core::user_op::USER_OPERATION_EVENT_TOPIC
        );
        assert_eq!(user_op_status_method(), "pimlico_getUserOperationStatus");
    }

    #[test]
    fn a_status_answer_is_parsed_once_by_the_core() {
        let answer = parse_user_op_status(
            &json!({"jsonrpc": "2.0", "id": 1, "result": {
                "status": "submitted", "transactionHash": "0xabc", "last_executor_stage": "fee_hold"
            }})
            .to_string(),
        );
        assert_eq!(
            answer.as_deref().map(parse),
            Some(json!({"status": "submitted", "stage": "fee_hold", "tx_hash": "0xabc"}))
        );
        assert_eq!(parse_user_op_status("{\"status\":\"teleported\"}"), None);
        assert_eq!(parse_user_op_status("{\"error\":{\"code\":-32601}}"), None);
    }

    fn entry(status: &str, outcome: &str, tx_hash: Option<&str>) -> String {
        json!({
            "user_op_hash": OP_HASH,
            "chain_id": 100,
            "record_ids": ["r1"],
            "status": status,
            "tx_hash": tx_hash,
            "polling": status == "pending",
            "submitted_at_ms": 1.0,
            "outcome": outcome
        })
        .to_string()
    }

    #[test]
    fn the_sheet_ending_waits_for_the_tracker() {
        let answered = json!({"type": "ok", "result": OP_HASH}).to_string();
        let ending = ok(sign_ending_of_inner(
            "eth_sendTransaction",
            &answered,
            Some(OP_HASH),
        ))
        .unwrap_or_else(|| unreachable!("an op-hash answer has an ending"));
        assert_eq!(
            parse(&ending),
            json!({"type": "still_confirming", "user_op_hash": OP_HASH})
        );
        assert_eq!(
            ok(sign_ending_of_inner(
                "personal_sign",
                &json!({"type": "ok", "result": "0x01"}).to_string(),
                None
            ))
            .as_deref()
            .map(parse),
            Some(json!({"type": "signed"}))
        );
        let refused =
            json!({"type": "err", "code": 4001, "kind": "user_rejected", "message": null});
        assert_eq!(
            ok(sign_ending_of_inner(
                "eth_sendTransaction",
                &refused.to_string(),
                None
            )),
            None
        );

        let state = |entry: Option<&str>| parse(&ok(sign_ending_state_inner(&ending, entry)));
        let following_landing = json!({"type": "following", "user_op_hash": OP_HASH, "outcome": "landing", "fee_held": false});
        assert_eq!(state(None), following_landing);
        assert_eq!(state(Some("")), following_landing);
        assert_eq!(state(Some("null")), following_landing);
        assert_eq!(
            state(Some(&entry("confirmed", "final", Some("0xfeed")))),
            json!({"type": "confirmed", "tx_hash": "0xfeed"})
        );
        assert_eq!(
            state(Some(&entry("dropped", "final", Some("0xdead")))),
            json!({"type": "reverted", "tx_hash": "0xdead"})
        );
        assert_eq!(
            state(Some(&entry("not_sent", "final", None))),
            json!({"type": "not_sent"})
        );
        assert_eq!(
            state(Some(&entry("pending", "maybe_sent", None))),
            json!({"type": "following", "user_op_hash": OP_HASH, "outcome": "maybe_sent", "fee_held": false})
        );
        assert!(sign_ending_state_inner("{\"type\":\"nope\"}", None).is_err());
        assert!(sign_ending_state_inner(&ending, Some("{}")).is_err());
    }

    #[test]
    fn the_clocks_are_the_core_s() {
        assert!((dapp_receipt_wait_ms(0.0) - 120_000.0).abs() < f64::EPSILON);
        assert!((dapp_receipt_wait_ms(46_000.0) - 74_000.0).abs() < f64::EPSILON);
        assert!((dapp_receipt_wait_ms(115_000.0) - 10_000.0).abs() < f64::EPSILON);
        assert!((sign_request_ttl_ms() - 300_000.0).abs() < f64::EPSILON);
        assert_eq!(rpc_read_timeout_ms(), 8_000);
        let cooldowns: Vec<f64> = (0..=6).map(rpc_cooldown_ms).collect();
        assert_eq!(
            cooldowns,
            vec![0.0, 30_000.0, 60_000.0, 120_000.0, 240_000.0, 300_000.0, 300_000.0]
        );
    }

    #[test]
    fn a_site_is_named_once_when_its_name_is_its_host() {
        assert_eq!(
            parse(&browser_site_label("App.Uniswap.org", "app.uniswap.org")),
            json!({"name": "app.uniswap.org", "host_line": null})
        );
        assert_eq!(
            parse(&browser_site_label("Uniswap", "app.uniswap.org")),
            json!({"name": "Uniswap", "host_line": "app.uniswap.org"})
        );
    }

    #[test]
    fn a_logo_miss_heals_unless_the_image_is_not_there() {
        assert_eq!(mark_miss_ttl_ms("unknown", None), Some(60_000));
        assert_eq!(mark_miss_ttl_ms("transport", None), Some(60_000));
        assert_eq!(mark_miss_ttl_ms("not_found", None), None);
        assert_eq!(mark_miss_ttl_ms("teleported", None), Some(60_000));
        // The status is the stronger evidence.
        assert_eq!(mark_miss_ttl_ms("unknown", Some(404)), None);
        assert_eq!(mark_miss_ttl_ms("not_found", Some(503)), Some(60_000));
        assert_eq!(mark_miss_ttl_ms("unknown", Some(200)), None);
    }

    #[test]
    fn the_balance_read_plan_is_the_core_s() {
        let usdc = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";
        let stables = json!([{ "symbol": "USDC", "contract": usdc, "type": "usd" }]).to_string();
        let custom =
            json!([{ "contract": usdc.to_lowercase(), "symbol": "MYUSDC", "decimals": 6 }])
                .to_string();
        let plan = parse(&ok(balance_read_plan_inner(8453, &stables, None, &custom)));
        let core = vela_core::app::balance_dashboard::read_plan(
            8453,
            &[vela_core::app::balance_dashboard::StableRef {
                symbol: "USDC".to_owned(),
                contract: usdc.to_owned(),
            }],
            None,
            &[vela_core::app::balance_dashboard::TokenRef {
                contract: usdc.to_lowercase(),
                symbol: "MYUSDC".to_owned(),
                name: String::new(),
                decimals: 6,
            }],
        );
        assert_eq!(plan, serde_json::to_value(&core).unwrap_or_default());
        assert_eq!(plan[0]["kind"], "native");
        assert_eq!(plan[1]["kind"], "stable");
        assert_eq!(
            parse(&ok(balance_read_plan_inner(8453, "", None, " "))),
            json!([{"kind": "native", "contract": null, "symbol": "", "name": "",
                    "known_decimals": null, "peg_usd": null}])
        );
        assert!(balance_read_plan_inner(8453, "{", None, "").is_err());
    }
}
