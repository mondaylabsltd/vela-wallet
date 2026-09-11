//! The public-key registry, over HTTP.
//!
//! A port of `app-web/.../onboarding/core/registry.ts` and `publish.ts` — the
//! same six calls, the same two guards, the same one judgement.
//!
//! **The one judgement**: whether a request reached the server. The core
//! branches on it (`index_failed { network }`), and only a shell can tell a
//! transport failure from a 4xx. Everything else about a registry failure is
//! the core's to interpret, and nothing here decides what happens next.
//!
//! **The two guards** are on `queryUnit`, and both refuse rather than degrade:
//! a group larger than a wallet's 7-key cap is not ours, and a partial page
//! would rebuild the Safe address from a SUBSET of the founding set — a
//! different, wrong, fundable address.

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::json;

use vela_core::app::{RegistryPublishMember, RegistryUnitMember};
use vela_core::registry_proof::{RegistryProof, build_group_proof, build_member_proof};

use super::passkey::{self, Ceremony, RELYING_PARTY};

/// The v2 registry. Overridable so a self-hosted stack is a setting, not a
/// fork.
pub const DEFAULT_REGISTRY_URL: &str = "https://p256-index-v2.getvela.app";

/// The health identities this endpoint accepts — the legacy index and the v2
/// registry, so a wallet can point at either during the migration.
const SERVICE_IDENTITIES: [&str; 2] = [
    "webauthn-p256-publickey-registry",
    "webauthn-p256-publickey-index",
];

const READ_TIMEOUT: Duration = Duration::from_secs(15);
const WRITE_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_TIMEOUT: Duration = Duration::from_secs(120);
const POLL_INTERVAL: Duration = Duration::from_secs(2);

/// A vela wallet's founding set is capped at 7 keys; a larger group is not ours
/// and must never be reconstructed into an account.
const MAX_UNIT_MEMBERS: usize = 7;

/// A request that never reached the server, as opposed to one the server
/// refused. An unreachable index is a transient condition the person can fix by
/// pointing somewhere else; a 4xx is an answer.
#[derive(Debug)]
pub struct RegistryError {
    pub message: String,
    pub network: bool,
    /// `network`, and every route out of this machine refused (spec 038): the
    /// sentence is "check your network", not "the service is down".
    pub local: bool,
}

impl RegistryError {
    fn network(message: impl Into<String>, local: bool) -> Self {
        Self {
            message: message.into(),
            network: true,
            local,
        }
    }

    fn answered(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            network: false,
            local: false,
        }
    }
}

type Result<T> = std::result::Result<T, RegistryError>;

/// The configured endpoint. A `Mutex` rather than a field on the executor
/// because the endpoint-settings surface can change it from a screen while a
/// health probe is already in flight on another thread.
fn endpoint() -> &'static Mutex<String> {
    static ENDPOINT: OnceLock<Mutex<String>> = OnceLock::new();
    ENDPOINT.get_or_init(|| Mutex::new(DEFAULT_REGISTRY_URL.to_owned()))
}

pub fn set_registry_url(url: &str) {
    let cleaned = url
        .trim()
        .replace(['\r', '\n'], "")
        .trim_end_matches('/')
        .to_owned();
    let value = if cleaned.is_empty() {
        DEFAULT_REGISTRY_URL.to_owned()
    } else {
        cleaned
    };
    if let Ok(mut slot) = endpoint().lock() {
        *slot = value;
    }
}

pub fn registry_url() -> String {
    endpoint()
        .lock()
        .map(|slot| slot.clone())
        .unwrap_or_else(|_| DEFAULT_REGISTRY_URL.to_owned())
}

/// This module builds no agent of its own: `proxy` is the app's only HTTP
/// client factory, and every request here walks its candidate chain
/// (`proxy::with_candidates`) so a refused route is retried on the next one
/// rather than reported as the index being down. See that module's note.
use super::proxy::{self, Transport};

/// Turn a transport failure into the two bits of classification the core
/// needs.
///
/// `StatusCode` is the ONLY variant that means the server answered. Everything
/// else — DNS, TLS, a refused connection, a timeout — is a request that never
/// arrived; `local` says whether it never even left the machine.
fn classify(label: &str, failure: Transport) -> RegistryError {
    match failure.error {
        ureq::Error::StatusCode(status) => {
            RegistryError::answered(format!("{label} failed: {status}"))
        }
        other => RegistryError::network(format!("{label} failed: {other}"), failure.local),
    }
}

fn get_json<T: serde::de::DeserializeOwned>(
    path: &str,
    label: &str,
    timeout: Duration,
) -> Result<T> {
    let url = format!("{}{path}", registry_url());
    proxy::with_candidates(timeout, |agent| agent.get(&url).call())
        .map_err(|failure| classify(label, failure))?
        .body_mut()
        .read_json::<T>()
        .map_err(|error| {
            RegistryError::answered(format!("{label} returned unreadable JSON: {error}"))
        })
}

fn post_json<T: serde::de::DeserializeOwned>(
    path: &str,
    body: serde_json::Value,
    label: &str,
    timeout: Duration,
) -> Result<T> {
    let url = format!("{}{path}", registry_url());
    proxy::with_candidates(timeout, |agent| agent.post(&url).send_json(&body))
        .map_err(|failure| classify(label, failure))?
        .body_mut()
        .read_json::<T>()
        .map_err(|error| {
            RegistryError::answered(format!("{label} returned unreadable JSON: {error}"))
        })
}

// ---------------------------------------------------------------------------
// Challenges
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ChallengeValue {
    pub challenge: String,
    #[serde(rename = "publicKey", default)]
    pub public_key: String,
}

#[derive(Debug, Deserialize)]
struct GroupChallenge {
    #[serde(rename = "groupChallenge")]
    group_challenge: ChallengeValue,
    #[serde(default)]
    members: Vec<ChallengeValue>,
}

/// MEMBER-mode challenge: one founding passkey confirming AT CREATION. It binds
/// only (groupPublicKey, own attestation), so it exists before the rest of the
/// set does — which is what makes the interleaved create→confirm flow work.
pub fn member_challenge(
    group_public_key_hex: &str,
    public_key_hex: &str,
    attestation_hex: &str,
) -> Result<String> {
    let value: ChallengeValue = post_json(
        "/api/challenge",
        json!({
            "rpId": RELYING_PARTY,
            "groupPublicKey": group_public_key_hex,
            "publicKey": public_key_hex,
            "attestation": attestation_hex,
        }),
        "Challenge",
        READ_TIMEOUT,
    )?;
    Ok(value.challenge)
}

// ---------------------------------------------------------------------------
// Queries
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct KeyProfile {
    entry: Option<serde_json::Value>,
    #[serde(default)]
    groups: Option<KeyGroups>,
}

#[derive(Debug, Deserialize)]
struct KeyGroups {
    #[serde(rename = "unitIds", default)]
    unit_ids: Vec<i64>,
}

pub struct KeyStatus {
    pub registered: bool,
    pub unit_ids: Vec<u32>,
}

pub fn query_by_public_key(public_key_hex: &str) -> Result<KeyStatus> {
    let profile: KeyProfile = get_json(
        &format!("/api/query?publicKey={}", urlencode(public_key_hex)),
        "Query",
        READ_TIMEOUT,
    )?;
    let raw = profile
        .groups
        .map(|groups| groups.unit_ids)
        .unwrap_or_default();
    let mut unit_ids = Vec::with_capacity(raw.len());
    for id in raw {
        // The core speaks u32 unit ids because the wire is JSON. An id past
        // 2^32 would truncate into a DIFFERENT group, so this fails the query
        // instead of quietly fetching the wrong founding set.
        match u32::try_from(id) {
            Ok(id) => unit_ids.push(id),
            Err(_) => {
                return Err(RegistryError::answered(format!(
                    "Query failed: unit id {id} is out of u32 range"
                )));
            }
        }
    }
    Ok(KeyStatus {
        registered: profile.entry.is_some_and(|entry| !entry.is_null()),
        unit_ids,
    })
}

/// One index record, of which the identity waterfall wants exactly one field.
#[derive(Debug, Deserialize)]
struct WalletRefRecord {
    #[serde(default)]
    name: String,
}

/// The Vela name behind an address, if the index knows one.
///
/// `walletRef` is the address left-padded to 32 bytes, which is how the index
/// stores it (`public-key-index.ts:174`).
///
/// Every failure — unreachable, timed out, 404, unparseable, an empty name — is
/// `None`. This is best-effort enrichment for a badge; it must never be the
/// reason something else does not happen.
pub fn query_by_wallet_ref(address: &str) -> Option<String> {
    let stripped = address.trim_start_matches("0x").to_lowercase();
    if stripped.len() != 40 || !stripped.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // The zero address has no entry, and asking is a doomed 404
    // (`public-key-index.ts:180`).
    if stripped.bytes().all(|b| b == b'0') {
        return None;
    }
    let wallet_ref = format!("0x{stripped:0>64}");
    let record: WalletRefRecord = get_json(
        &format!("/api/query?walletRef={}", urlencode(&wallet_ref)),
        "Query",
        READ_TIMEOUT,
    )
    .ok()?;
    (!record.name.trim().is_empty()).then(|| record.name)
}

#[derive(Debug, Deserialize)]
struct UnitResponse {
    unit: UnitMeta,
    #[serde(default)]
    members: Option<UnitMembers>,
}

#[derive(Debug, Deserialize)]
struct UnitMeta {
    metadata: String,
}

#[derive(Debug, Deserialize)]
struct UnitMembers {
    #[serde(default)]
    total: usize,
    #[serde(default)]
    items: Vec<UnitMember>,
}

#[derive(Debug, Deserialize)]
struct UnitMember {
    #[serde(rename = "publicKey")]
    public_key: String,
    #[serde(rename = "credentialId")]
    credential_id: String,
    #[serde(rename = "authenticatorAttachment", default)]
    authenticator_attachment: String,
    #[serde(default)]
    transports: String,
}

pub struct UnitDetail {
    pub metadata_hex: String,
    pub members: Vec<RegistryUnitMember>,
}

/// One group: the frozen metadata blob and ALL its founding members in
/// ascending order, which IS the canonical founding order the Safe address
/// derivation pins.
pub fn query_unit(unit_id: u32) -> Result<UnitDetail> {
    let detail: UnitResponse = get_json(
        &format!("/api/query?unitId={unit_id}&pageSize={MAX_UNIT_MEMBERS}&order=asc"),
        "Query",
        READ_TIMEOUT,
    )?;
    let members = detail.members.unwrap_or(UnitMembers {
        total: 0,
        items: Vec::new(),
    });
    if members.total > MAX_UNIT_MEMBERS {
        return Err(RegistryError::answered(format!(
            "Query failed: unit {unit_id} has {} members (cap {MAX_UNIT_MEMBERS})",
            members.total
        )));
    }
    if members.items.len() != members.total {
        return Err(RegistryError::answered(format!(
            "Query failed: unit {unit_id} page holds {} of {} members",
            members.items.len(),
            members.total
        )));
    }
    Ok(UnitDetail {
        metadata_hex: detail.unit.metadata,
        members: members
            .items
            .into_iter()
            .map(|member| RegistryUnitMember {
                credential_id: member.credential_id,
                public_key_hex: member.public_key,
                authenticator_attachment: member.authenticator_attachment,
                transports: member.transports,
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize)]
struct Health {
    #[serde(default)]
    service: String,
    #[serde(default)]
    status: String,
}

/// What one health probe found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Probe {
    /// The index answered with its own identity and `status: ok`.
    Reachable,
    /// A route existed and the index did not answer as itself — down, or the
    /// wrong service behind the URL.
    Down,
    /// Nothing left this machine: every route refused (spec 038).
    Local,
}

/// One health probe. Never fails outward: the core asked a yes/no question,
/// and spec 038 added the one refinement a screen can act on differently.
pub fn probe_health() -> Probe {
    // The nonce defeats an intermediary cache: a cached 200 would make an
    // unreachable endpoint look healthy, which is the one answer this probe
    // must never give wrongly.
    let nonce = Instant::now().elapsed().as_nanos();
    match get_json::<Health>(&format!("/api/health?_t={nonce}"), "Health", READ_TIMEOUT) {
        Ok(health) => {
            if SERVICE_IDENTITIES.contains(&health.service.as_str()) && health.status == "ok" {
                Probe::Reachable
            } else {
                Probe::Down
            }
        }
        Err(error) if error.local => Probe::Local,
        Err(_) => Probe::Down,
    }
}

/// The v1 index CONTRACT on Gnosis — the only place a v1-era wallet's display
/// name survives. The v2 server never stored names, so this read is on-chain,
/// exactly like the web client's `queryLegacyName` (public-key-index.ts): an
/// `eth_call` of `getRecord(rpId, credentialId)` with the name decoded out of
/// the returned struct's fifth slot.
const LEGACY_INDEX_CONTRACT: &str = "0xdd93420BD49baaBdFF4A363DdD300622Ae87E9c3";

/// Gnosis RPC endpoints, tried in order. Same set the wallet's rpc pool pins
/// for chain 100; this module keeps its own copy because the onboarding
/// executor has no pool.
const GNOSIS_RPC_URLS: &[&str] = &[
    "https://rpc.gnosischain.com",
    "https://gnosis-rpc.publicnode.com",
    "https://1rpc.io/gnosis",
];

/// The v1 index's display name for a credential. Best effort and read-only; a
/// lost name degrades the label ("Wallet"), never the flow.
pub fn legacy_name(credential_id: &str) -> Option<String> {
    let data = encode_get_record(RELYING_PARTY, credential_id);
    let data_hex = format!("0x{}", vela_core::primitives::to_hex(&data, false));
    for url in GNOSIS_RPC_URLS {
        match eth_call(url, LEGACY_INDEX_CONTRACT, &data_hex) {
            // An empty (reverted) result is a definite "no record" — a v2-era
            // wallet. Do not burn two more RPCs re-asking the same question.
            Ok(result) if result.is_empty() => return None,
            Ok(result) => return decode_record_name(&result),
            Err(_) => continue,
        }
    }
    None
}

/// ABI-encode `getRecord(string,string)`: selector, two head offsets, then the
/// two length-prefixed, 32-padded tails.
fn encode_get_record(rp_id: &str, credential_id: &str) -> Vec<u8> {
    fn tail(bytes: &[u8]) -> Vec<u8> {
        let mut out = [0u8; 32].to_vec();
        out[24..32].copy_from_slice(&(bytes.len() as u64).to_be_bytes());
        out.extend_from_slice(bytes);
        out.resize(out.len() + (32 - bytes.len() % 32) % 32, 0);
        out
    }
    fn word(value: u64) -> [u8; 32] {
        let mut out = [0u8; 32];
        out[24..32].copy_from_slice(&value.to_be_bytes());
        out
    }
    let rp_tail = tail(rp_id.as_bytes());
    let mut out = vela_core::primitives::function_selector("getRecord(string,string)")
        .unwrap_or_else(|_| vec![0; 4]);
    out.extend_from_slice(&word(64)); // offset of rpId (2 head slots)
    out.extend_from_slice(&word(64 + rp_tail.len() as u64)); // offset of credentialId
    out.extend_from_slice(&rp_tail);
    out.extend_from_slice(&tail(credential_id.as_bytes()));
    out
}

/// Decode `PublicKeyRecord.name` out of a `getRecord` return: one struct (via a
/// top-level offset) whose slot 4 holds the offset of the dynamic `name` field,
/// relative to the struct start. Mirrors the web client's `decodeRecordName`.
fn decode_record_name(data: &[u8]) -> Option<String> {
    fn word_at(data: &[u8], at: usize) -> Option<usize> {
        let slice = data.get(at..at + 32)?;
        // Offsets and lengths in a sane return fit far below 2^53; reject the
        // rest instead of wrapping.
        let mut out = 0usize;
        for &byte in slice {
            out = out.checked_mul(256)?.checked_add(byte as usize)?;
        }
        Some(out)
    }
    let struct_start = word_at(data, 0)?;
    let name_offset =
        struct_start.checked_add(word_at(data, struct_start.checked_add(4 * 32)?)?)?;
    let name_length = word_at(data, name_offset)?;
    let name_bytes =
        data.get(name_offset + 32..name_offset.checked_add(32)?.checked_add(name_length)?)?;
    let name = std::str::from_utf8(name_bytes).ok()?.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// One `eth_call` against `url`, through the app's proxy-aware agent. The
/// result is the raw return bytes ( empty when the call reverted — v1's
/// `getRecord` reverts for an unknown credential).
fn eth_call(url: &str, to: &str, data_hex: &str) -> Result<Vec<u8>> {
    #[derive(Debug, Deserialize)]
    struct RpcReply {
        #[serde(default)]
        result: Option<String>,
    }
    let reply: RpcReply = proxy::agent(READ_TIMEOUT)
        .post(url)
        .send_json(serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [{ "to": to, "data": data_hex }, "latest"],
        }))
        .map_err(|error| {
            classify(
                "Legacy name",
                Transport {
                    error,
                    local: false,
                },
            )
        })?
        .body_mut()
        .read_json()
        .map_err(|error| RegistryError::answered(format!("Legacy name: bad JSON: {error}")))?;
    let result = reply.result.unwrap_or_default();
    let stripped = result.strip_prefix("0x").unwrap_or(&result);
    vela_core::primitives::from_hex(stripped)
        .map_err(|error| RegistryError::answered(format!("Legacy name: bad hex: {error}")))
}

// ---------------------------------------------------------------------------
// Publish
// ---------------------------------------------------------------------------

/// One member as the REGISTRY wants it.
///
/// Spelled out rather than derived from `RegistryPublishMember`. The core's
/// wire type is snake_case because it is generated from Rust; the registry's
/// HTTP API is camelCase. Sending `public_key_hex` where the server reads
/// `publicKey` is answered with `members[0]: publicKey is required` — AFTER the
/// person has minted and confirmed every key. The two vocabularies meet here,
/// in one function, and nowhere else.
#[derive(Debug, Serialize)]
struct ApiMember {
    #[serde(rename = "publicKey")]
    public_key: String,
    #[serde(rename = "attestation", skip_serializing_if = "String::is_empty")]
    attestation: String,
    #[serde(rename = "credentialId")]
    credential_id: String,
    #[serde(rename = "authenticatorAttachment")]
    authenticator_attachment: String,
    transports: String,
    proof: RegistryProof,
}

#[derive(Debug, Deserialize)]
struct Accepted {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    status: String,
}

#[derive(Debug, Deserialize)]
struct TaskStatus {
    #[serde(default)]
    status: String,
    #[serde(default)]
    error: Option<String>,
}

/// Publish a wallet's founding key set as one possession-proven group.
///
/// With `seed_hex` set (the interleaved create flow) the members already carry
/// creation-time proofs and this only closes the group — no prompts. Empty (the
/// login re-publish) runs the legacy mechanism: fresh group key, challenges,
/// one assertion per member.
pub fn publish(
    metadata_hex: &str,
    members: &[RegistryPublishMember],
    seed_hex: &str,
    group_public_key_hex: &str,
    method: vela_core::app::KeyMethod,
    ceremony: &Ceremony,
) -> Result<()> {
    if members.is_empty() {
        return Err(RegistryError::answered(
            "registry publish needs at least one member",
        ));
    }

    let (seed_hex, group_public_key) = if seed_hex.is_empty() || group_public_key_hex.is_empty() {
        let seed = vela_core::primitives::to_hex(&passkey::random(32), false);
        let public = vela_core::registry_proof::group_public_key_from_seed(&seed)
            .map_err(|error| RegistryError::answered(format!("group key: {error}")))?;
        (seed, public)
    } else {
        (seed_hex.to_owned(), group_public_key_hex.to_owned())
    };

    let challenge: GroupChallenge = post_json(
        "/api/challenge",
        json!({
            "rpId": RELYING_PARTY,
            "metadata": metadata_hex,
            "groupPublicKey": group_public_key,
            "members": members
                .iter()
                .map(|member| json!({
                    "publicKey": member.public_key_hex,
                    "attestation": member.attestation_hex,
                }))
                .collect::<Vec<_>>(),
        }),
        "Challenge",
        READ_TIMEOUT,
    )?;

    let mut proven = Vec::with_capacity(members.len());
    for member in members {
        let proof = match &member.proof {
            Some(proof) => proof.clone(),
            None => {
                // A member with no creation-time proof signs live. On desktop
                // that means one touch of the security key per such member —
                // which is the login re-publish path, not the create path.
                let derived = challenge
                    .members
                    .iter()
                    .find(|candidate| {
                        candidate
                            .public_key
                            .eq_ignore_ascii_case(&member.public_key_hex)
                    })
                    .ok_or_else(|| {
                        RegistryError::answered(format!(
                            "registry challenge is missing member {}",
                            member.public_key_hex
                        ))
                    })?;
                let challenge_bytes =
                    vela_core::primitives::from_hex(strip_hex(&derived.challenge)).map_err(
                        |error| RegistryError::answered(format!("challenge is not hex: {error}")),
                    )?;
                // The route the person signed in with: a phone credential signs
                // its possession proof over caBLE (a fresh QR), a USB one on the
                // key in the port. Hardcoding SecurityKey here was why a caBLE
                // recovery silently entered the wallet unpublished.
                let assertion = passkey::assert(
                    &challenge_bytes,
                    Some(&member.credential_id),
                    method,
                    ceremony,
                )
                .map_err(|failure| {
                    // A ceremony failure inside a publish is not a
                    // network failure. It is reported as an answered
                    // one so the core does not offer to change the
                    // endpoint over a cancelled touch.
                    RegistryError::answered(
                        failure
                            .message
                            .unwrap_or_else(|| "the signature was refused".to_owned()),
                    )
                })?;
                build_member_proof(
                    &assertion.authenticator_data_hex,
                    &assertion.client_data_json_hex,
                    &assertion.signature_der_hex,
                )
                .map_err(|error| RegistryError::answered(format!("member proof: {error}")))?
            }
        };
        proven.push(ApiMember {
            public_key: member.public_key_hex.clone(),
            attestation: member.attestation_hex.clone(),
            credential_id: member.credential_id.clone(),
            authenticator_attachment: member.authenticator_attachment.clone(),
            transports: member.transports.clone(),
            proof,
        });
    }

    // The group key silently closes over the content hash.
    let group = build_group_proof(
        &seed_hex,
        RELYING_PARTY,
        strip_hex(&challenge.group_challenge.challenge),
    )
    .map_err(|error| RegistryError::answered(format!("group proof: {error}")))?;

    let accepted: Accepted = post_json(
        "/api/register",
        json!({
            "rpId": RELYING_PARTY,
            "metadata": metadata_hex,
            "groupPublicKey": group_public_key,
            "groupProof": group.proof,
            "members": proven,
        }),
        "Register",
        WRITE_TIMEOUT,
    )?;

    // `done` up front means the identical group was already on-chain —
    // idempotent by content hash, and just as landed as a fresh one.
    if accepted.status == "done" {
        return Ok(());
    }
    let id = accepted
        .id
        .ok_or_else(|| RegistryError::answered("register was accepted without a task id"))?;
    await_task(&id)
}

/// Poll until terminal.
///
/// A transient read failure is retried until the budget runs out: the task is
/// already accepted, so giving up on one bad read would report a failure that
/// did not happen.
fn await_task(id: &str) -> Result<()> {
    let deadline = Instant::now() + POLL_TIMEOUT;
    let mut last_error: Option<String> = None;
    while Instant::now() < deadline {
        match get_json::<TaskStatus>(
            &format!("/api/task/{}", urlencode(id)),
            "Task status",
            READ_TIMEOUT,
        ) {
            Ok(task) if task.status == "done" => return Ok(()),
            Ok(task) if task.status == "failed" => {
                return Err(RegistryError::answered(format!(
                    "Register failed: {}",
                    task.error.unwrap_or_else(|| "unknown".to_owned())
                )));
            }
            Ok(_) => {}
            Err(error) if !error.network => return Err(error),
            Err(error) => last_error = Some(error.message),
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    // The server was reached — it answered "not yet" for the whole budget —
    // so this is never a local transport failure.
    Err(RegistryError::network(
        format!(
            "Register timed out after {}s{}",
            POLL_TIMEOUT.as_secs(),
            last_error
                .map(|error| format!(": {error}"))
                .unwrap_or_default()
        ),
        false,
    ))
}

fn strip_hex(value: &str) -> &str {
    value.strip_prefix("0x").unwrap_or(value)
}

/// Percent-encode the characters that can appear in a credential id or a unit
/// id and would otherwise change what is being asked for. Hand-rolled because
/// the alternative is a URL crate for two call sites whose inputs are hex.
fn urlencode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The endpoint is normalised, not trusted: a trailing slash would produce
    /// `//api/health`, and a newline pasted with a URL would be smuggled into
    /// the request line.
    #[test]
    fn the_endpoint_is_cleaned_before_it_is_used() {
        set_registry_url("  https://example.invalid/\n ");
        assert_eq!(registry_url(), "https://example.invalid");
        // Empty is not an endpoint — it falls back rather than producing
        // requests to `/api/health` with no host.
        set_registry_url("   ");
        assert_eq!(registry_url(), DEFAULT_REGISTRY_URL);
    }

    /// Only a status code means the server answered. Everything else is a
    /// request that never arrived, and the core offers a different endpoint for
    /// exactly that case — so misclassifying a 4xx as a network failure would
    /// send someone hunting for a working URL over a refusal.
    #[test]
    fn only_a_status_code_counts_as_an_answer() {
        assert!(
            !classify(
                "Query",
                Transport {
                    error: ureq::Error::StatusCode(404),
                    local: false
                }
            )
            .network
        );
        assert!(
            classify(
                "Query",
                Transport {
                    error: ureq::Error::HostNotFound,
                    local: false
                }
            )
            .network
        );
        // Spec 038: the one new bit — "this machine could not get out".
        let local = classify(
            "Query",
            Transport {
                error: ureq::Error::HostNotFound,
                local: true,
            },
        );
        assert!(local.network && local.local);
    }

    /// A credential id is hex today, but the query string is built from it —
    /// so anything that could change WHICH record is being asked for is
    /// escaped rather than assumed absent.
    #[test]
    fn query_values_are_escaped() {
        assert_eq!(urlencode("abc123"), "abc123");
        assert_eq!(urlencode("a&b=c"), "a%26b%3Dc");
        assert_eq!(urlencode("a b"), "a%20b");
    }

    /// The live registry, reached for real.
    ///
    /// `#[ignore]` because it needs the network: run it with
    /// `cargo test -- --ignored` when changing this file. It is the only check
    /// that the service identity this client accepts is still the one the
    /// deployed server sends — a rename there would make every wallet report
    /// the index unreachable while it is answering perfectly.
    #[test]
    #[ignore = "needs the network"]
    fn the_deployed_registry_answers_its_health_probe() {
        set_registry_url(DEFAULT_REGISTRY_URL);
        assert_eq!(
            probe_health(),
            Probe::Reachable,
            "{DEFAULT_REGISTRY_URL} did not answer"
        );
    }

    /// A key nobody registered comes back as not-registered, not as an error.
    #[test]
    #[ignore = "needs the network"]
    fn an_unknown_public_key_is_simply_unregistered() {
        set_registry_url(DEFAULT_REGISTRY_URL);
        let unknown = format!("04{}", "11".repeat(64));
        match query_by_public_key(&unknown) {
            Ok(status) => assert!(!status.registered),
            Err(error) => unreachable!("{}", error.message),
        }
    }
}
