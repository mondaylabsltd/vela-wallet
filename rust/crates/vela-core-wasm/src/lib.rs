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
#[wasm_bindgen(js_name = walletKeysStep)]
#[must_use]
pub fn wallet_keys_step(address: &str, device_keys_json: &str, answers_json: &str) -> String {
    vela_core::wallet_keys::step_json(address, device_keys_json, answers_json)
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

    let user_op = user_op::UserOperation {
        sender: op.sender,
        nonce: op.nonce,
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
        signature: Vec::new(),
    };
    user_op::calculate_safe_op_hash(&user_op, chain_id)
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
