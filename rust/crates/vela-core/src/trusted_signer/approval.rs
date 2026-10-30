//! A multisig member's approval on the trusted signing page (2026-10-29).
//!
//! The owner's lesson from Bybit: a signer must be able to review and sign on
//! a surface independent of the wallet app's code. A Vela multisig's members
//! approve a SafeTx with their passkey ([`crate::multisig::approval_challenge`]
//! — `SafeMessage(safeTxHash)` under the MEMBER's domain). When the member's
//! account reviews and signs on a page ([`crate::signing_venue::SigningVenue::
//! Page`]), the approval goes there instead of the app's own passkey prompt:
//!
//! - [`request`] — the page's `{intent, context}`: the method
//!   [`METHOD`], the raw SafeTx, the multisig, the chain, the member, and the
//!   safeTxHash THE CORE computed (the page recomputes it from the fields and
//!   refuses when they differ — it is there to be compared, never to be
//!   signed). The context is the transaction request's ([`super::request`]):
//!   the member's name and keys, the key route, the chain's name and coin —
//!   plus the multisig's name, which the page marks "named by the app".
//! - [`accept`] — the page's answer, accepted only as [`super::verify`]
//!   accepts a transaction's — a user-verified `webauthn.get` over exactly the
//!   approval bytes THIS wallet computed, by one of the member's keys, whose
//!   P-256 signature verifies — and then turned into the approval the genesis
//!   service and the chain take, checked the chain's way
//!   ([`crate::multisig::verify_approval`]). Nothing the page says about what
//!   it signed is read: the bytes are the wallet's.
//! - [`approval_route`] — where a member's approval goes: the one rule for
//!   every signature ([`crate::signing_venue::signing_route`]) for
//!   [`SignatureKind::MultisigApproval`] — its own passkey prompt, the page,
//!   or nowhere and why.
//!
//! Pure: no I/O, no clock.

use serde_json::{json, Value};

use super::{RequestInput, TrustedSignerError};
use crate::multisig::{self, PasskeyApproval, SafeTx};
use crate::signing_venue::{KeyRoute, SignatureKind, SigningPlan, SigningRoute};
use crate::user_op::WalletKey;

/// The page's method for a member's approval.
pub const METHOD: &str = "vela_multisigApprove";

/// What the shell hands over to build an approval request.
#[derive(Clone, Debug)]
pub struct ApprovalInput<'a> {
    pub chain_id: u64,
    pub chain_name: Option<&'a str>,
    pub native_symbol: Option<&'a str>,
    /// The multisig (the Safe whose transaction this is).
    pub multisig: &'a str,
    /// The multisig's name, as this device knows it — a label the page draws
    /// as the app's word.
    pub multisig_name: Option<&'a str>,
    pub tx: &'a SafeTx,
    /// The member approving: a Vela account of this device that owns the
    /// multisig. Its domain is the approval's.
    pub member: &'a str,
    pub member_name: Option<&'a str>,
    /// The member's credential ids, hex (offered when there is no route).
    pub credential_ids_hex: &'a [String],
    pub key_route: Option<&'a KeyRoute>,
}

/// The page's `{intent, context}` for `input` (PROTOCOL.md §12).
///
/// # Errors
///
/// A SafeTx or an address the core cannot hash.
pub fn request(input: &ApprovalInput<'_>) -> Result<Value, crate::error::CoreError> {
    let safe_tx_hash = multisig::safe_tx_hash(input.chain_id, input.multisig, input.tx)?;
    let params = json!([{
        "chainId": input.chain_id,
        "safe": input.multisig,
        "member": input.member,
        "safeTx": input.tx,
        "safeTxHash": safe_tx_hash,
    }]);
    let mut built = super::request(&RequestInput {
        method: METHOD,
        params,
        origin: "",
        chain_id: input.chain_id,
        chain_name: input.chain_name,
        native_symbol: input.native_symbol,
        account: input.member,
        account_name: input.member_name,
        credential_ids_hex: input.credential_ids_hex,
        key_route: input.key_route,
        user_op: None,
        calls: &[],
        origin_seen_by_browser: false,
    });
    if let Some(context) = built.get_mut("context").and_then(Value::as_object_mut) {
        let mut about = serde_json::Map::new();
        if let Some(name) = input.multisig_name.map(str::trim).filter(|n| !n.is_empty()) {
            about.insert("name".into(), json!(name));
        }
        context.insert("multisig".into(), Value::Object(about));
    }
    Ok(built)
}

/// What the page is told about the multisig transaction an executor's own
/// operation executes (round 7, r7-1) — its `context.multisig`: the
/// multisig, its name and how many approvals it needs as this device knows
/// them, the number the transaction runs at, and the safeTxHash THE CORE
/// computes at that number.
///
/// The page reads what runs from the operation's bytes (the `execTransaction`
/// leg); this only says what those bytes do not carry. `execTransaction` has
/// no nonce — the multisig uses its own — so the number, and the hash at it,
/// are the app's word: the page recomputes the hash from the bytes at that
/// number and refuses when the two differ (the app describes one transaction
/// while the operation runs another), and draws the number, the name and the
/// threshold as the app's.
///
/// # Errors
///
/// A SafeTx or an address the core cannot hash.
pub fn execution_context(
    chain_id: u64,
    multisig: &str,
    tx: &SafeTx,
    name: Option<&str>,
    threshold: Option<u64>,
) -> Result<Value, crate::error::CoreError> {
    let safe_tx_hash = multisig::safe_tx_hash(chain_id, multisig, tx)?;
    let mut about = serde_json::Map::new();
    about.insert("address".into(), json!(multisig));
    if let Some(name) = name.map(str::trim).filter(|n| !n.is_empty()) {
        about.insert("name".into(), json!(name));
    }
    if let Some(threshold) = threshold.filter(|t| *t > 0) {
        about.insert("threshold".into(), json!(threshold));
    }
    about.insert("nonce".into(), json!(tx.nonce));
    about.insert("safeTxHash".into(), json!(safe_tx_hash));
    Ok(Value::Object(about))
}

/// The bytes the member's passkey signs for `safe_tx_hash`
/// ([`multisig::approval_challenge`]) — what [`accept`] holds the answer to.
///
/// # Errors
///
/// A member that is not an address, a hash that is not 32 bytes.
pub fn digest(
    chain_id: u64,
    member: &str,
    safe_tx_hash: &str,
) -> Result<Vec<u8>, crate::error::CoreError> {
    crate::primitives::from_hex(&multisig::approval_challenge(
        chain_id,
        member,
        safe_tx_hash,
    )?)
}

/// The page's answer as the member's approval of `safe_tx_hash`, or why not.
///
/// `keys` are the MEMBER's founding keys, as the account record holds them.
/// Accepted only when: the assertion is a user-verified `webauthn.get` over
/// exactly [`digest`] by one of `keys`, with a valid P-256 signature
/// ([`super::verify`]); the approval built from it names `member` (the keys
/// rebuild the member's own account); and it verifies the way the chain will
/// verify it ([`multisig::verify_approval`]).
///
/// # Errors
///
/// [`TrustedSignerError`]: the page's answer is not such an approval.
pub fn accept(
    result: &Value,
    chain_id: u64,
    member: &str,
    safe_tx_hash: &str,
    keys: &[WalletKey],
) -> Result<PasskeyApproval, TrustedSignerError> {
    let bytes = digest(chain_id, member, safe_tx_hash)
        .map_err(|error| TrustedSignerError::Malformed(error.to_string()))?;
    let verified = super::verify(result, &bytes, keys)?;
    let hex = |b: &[u8]| crate::primitives::to_hex(b, false);
    let approval = multisig::approval_from_assertion(
        keys,
        &verified.credential_id_hex,
        &hex(&verified.authenticator_data),
        &hex(&verified.client_data_json),
        &hex(&verified.signature_der),
    )
    .map_err(|error| TrustedSignerError::Malformed(error.to_string()))?;
    if !approval.owner.eq_ignore_ascii_case(member) {
        return Err(TrustedSignerError::ForeignKey);
    }
    multisig::verify_approval(chain_id, safe_tx_hash, &approval)
        .map_err(|_| TrustedSignerError::BadSignature)?;
    Ok(approval)
}

/// Where a multisig member's approval goes — a [`SigningRoute`]. Its JSON is
/// `{type: 'in_vela' | 'page' | 'blocked', url?, block?}`, mirrored by the
/// web's generated `SigningRoute`.
pub type ApprovalRoute = SigningRoute;

/// Where a member's approval goes, from the member's signing plan — the one
/// rule every signature follows ([`crate::signing_venue::signing_route`],
/// owner 2026-10-30), for [`SignatureKind::MultisigApproval`]: a member who
/// chose to review and sign on a page approves there, independently of the
/// wallet's code (the owner's Bybit lesson); one in Vela approves with its
/// own passkey prompt; a blocked plan (a custom-domain account whose page is
/// unknown here) is blocked, with the plan's reason.
#[must_use]
pub fn approval_route(plan: &SigningPlan) -> ApprovalRoute {
    crate::signing_venue::signing_route(plan, SignatureKind::MultisigApproval)
}

/// [`approval_route`] for a stored account record (JSON) — `None` for a
/// record this build cannot read.
#[cfg(feature = "crux")]
#[must_use]
pub fn approval_route_json(account_json: &str) -> Option<String> {
    let account: crate::app::Account = serde_json::from_str(account_json).ok()?;
    serde_json::to_string(&approval_route(&account.signing_plan())).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine as _;
    use p256::ecdsa::signature::hazmat::PrehashSigner as _;
    use p256::ecdsa::{Signature, SigningKey};
    use sha2::{Digest as _, Sha256};

    const CHAIN: u64 = 5_042_002;
    const MULTISIG: &str = "0x4c3FF6Bc7d1a4b7f4e0e93B4C61b8b1a7e5dF2A9";

    fn tx() -> SafeTx {
        SafeTx {
            to: "0x6813Eb9362372EEF6200f3b1dbC3f819671cBA69".into(),
            value: "0xde0b6b3a7640000".into(),
            data: "0x".into(),
            operation: 0,
            safe_tx_gas: "0x0".into(),
            base_gas: "0x0".into(),
            gas_price: "0x0".into(),
            gas_token: "0x0000000000000000000000000000000000000000".into(),
            refund_receiver: "0x0000000000000000000000000000000000000000".into(),
            nonce: "0x3".into(),
        }
    }

    /// A member: one P-256 key, its Vela account address and wallet key.
    fn member(seed: u8) -> (SigningKey, String, WalletKey) {
        let signing = SigningKey::from_slice(&[seed; 32]).unwrap_or_else(|_| unreachable!());
        let point = signing.verifying_key().to_sec1_point(false);
        let key = WalletKey {
            credential_id: format!("{seed:02x}{seed:02x}c0ffee"),
            public_key_hex: crate::primitives::to_hex(point.as_bytes(), false),
        };
        let (address, _) = multisig::vela_account_genesis(std::slice::from_ref(&key))
            .unwrap_or_else(|_| unreachable!());
        (signing, address, key)
    }

    /// What the page would answer: a `webauthn.get` over `challenge` by `key`.
    fn answer(signing: &SigningKey, key: &WalletKey, challenge: &[u8], uv: bool) -> Value {
        let mut auth = Sha256::digest(b"getvela.app").to_vec();
        auth.push(if uv { 0x05 } else { 0x01 });
        auth.extend_from_slice(&[0, 0, 0, 1]);
        let client = format!(
            "{{\"type\":\"webauthn.get\",\"challenge\":\"{}\",\"origin\":\"https://sign.getvela.app\",\"crossOrigin\":false}}",
            URL_SAFE_NO_PAD.encode(challenge)
        );
        let prehash = crate::webauthn::webauthn_signing_hash(&auth, client.as_bytes());
        let signature: Signature = signing
            .sign_prehash(&prehash)
            .unwrap_or_else(|_| unreachable!());
        let id = crate::primitives::from_hex(&key.credential_id).unwrap_or_default();
        json!({
            "credentialId": URL_SAFE_NO_PAD.encode(id),
            "signature": crate::primitives::to_hex(&signature.to_bytes(), false),
            "authenticatorData": crate::primitives::to_hex(&auth, false),
            "clientDataJSON": crate::primitives::to_hex(client.as_bytes(), false),
        })
    }

    #[test]
    fn an_execution_names_its_number_and_the_cores_hash_at_it() {
        let about = execution_context(CHAIN, MULTISIG, &tx(), Some("  Team fund "), Some(2))
            .unwrap_or_else(|_| unreachable!());
        assert_eq!(about["address"], MULTISIG);
        assert_eq!(about["name"], "Team fund");
        assert_eq!(about["threshold"], 2);
        assert_eq!(about["nonce"], "0x3");
        assert_eq!(
            about["safeTxHash"],
            multisig::safe_tx_hash(CHAIN, MULTISIG, &tx()).unwrap_or_default()
        );
        // Nothing this device does not know is said: no name, no threshold.
        let bare = execution_context(CHAIN, MULTISIG, &tx(), Some(" "), Some(0))
            .unwrap_or_else(|_| unreachable!());
        assert!(bare.get("name").is_none() && bare.get("threshold").is_none());
        assert!(execution_context(CHAIN, "not an address", &tx(), None, None).is_err());
    }

    #[test]
    fn the_request_carries_the_safe_tx_and_the_cores_hash() {
        let (_, address, key) = member(7);
        let ids = vec![key.credential_id.clone()];
        let request = request(&ApprovalInput {
            chain_id: CHAIN,
            chain_name: Some("Arc Testnet"),
            native_symbol: Some("USDC"),
            multisig: MULTISIG,
            multisig_name: Some("  Team fund  "),
            tx: &tx(),
            member: &address,
            member_name: Some("Alice"),
            credential_ids_hex: &ids,
            key_route: None,
        })
        .unwrap_or_else(|_| unreachable!());
        assert_eq!(request["intent"]["method"], METHOD);
        assert_eq!(request["intent"]["origin"], "");
        let p = &request["intent"]["params"][0];
        assert_eq!(p["chainId"], CHAIN);
        assert_eq!(p["safe"], MULTISIG);
        assert_eq!(p["member"], address.as_str());
        assert_eq!(p["safeTx"]["value"], "0xde0b6b3a7640000");
        assert_eq!(p["safeTx"]["safeTxGas"], "0x0");
        assert_eq!(
            p["safeTxHash"],
            multisig::safe_tx_hash(CHAIN, MULTISIG, &tx()).unwrap_or_default()
        );
        let context = &request["context"];
        assert_eq!(context["account"], address.as_str());
        assert_eq!(context["chainId"], CHAIN);
        assert_eq!(context["signer"]["name"], "Alice");
        assert_eq!(context["multisig"]["name"], "Team fund");
        assert!(context.get("operation").is_none());
        assert_eq!(
            context["allowCredentials"].as_array().map(Vec::len),
            Some(1)
        );
    }

    #[test]
    fn an_answer_over_the_approval_bytes_is_the_members_approval() {
        let (signing, address, key) = member(9);
        let hash = multisig::safe_tx_hash(CHAIN, MULTISIG, &tx()).unwrap_or_default();
        let bytes = digest(CHAIN, &address, &hash).unwrap_or_default();
        let approval = accept(
            &answer(&signing, &key, &bytes, true),
            CHAIN,
            &address,
            &hash,
            std::slice::from_ref(&key),
        )
        .unwrap_or_else(|error| unreachable!("{error:?}"));
        assert!(approval.owner.eq_ignore_ascii_case(&address));
        assert!(multisig::verify_approval(CHAIN, &hash, &approval).is_ok());
    }

    #[test]
    fn anything_else_is_refused() {
        let (signing, address, key) = member(11);
        let keys = std::slice::from_ref(&key);
        let hash = multisig::safe_tx_hash(CHAIN, MULTISIG, &tx()).unwrap_or_default();
        let bytes = digest(CHAIN, &address, &hash).unwrap_or_default();

        // A signature over the bare safeTxHash (what a page that skipped the
        // member's SafeMessage would sign) is not an approval.
        let bare = crate::primitives::from_hex(&hash).unwrap_or_default();
        assert_eq!(
            accept(
                &answer(&signing, &key, &bare, true),
                CHAIN,
                &address,
                &hash,
                keys
            ),
            Err(TrustedSignerError::WrongChallenge)
        );
        // Another transaction's approval bytes.
        let mut other = tx();
        other.nonce = "0x4".into();
        let other_hash = multisig::safe_tx_hash(CHAIN, MULTISIG, &other).unwrap_or_default();
        let other_bytes = digest(CHAIN, &address, &other_hash).unwrap_or_default();
        assert_eq!(
            accept(
                &answer(&signing, &key, &other_bytes, true),
                CHAIN,
                &address,
                &hash,
                keys
            ),
            Err(TrustedSignerError::WrongChallenge)
        );
        // Not user-verified.
        assert_eq!(
            accept(
                &answer(&signing, &key, &bytes, false),
                CHAIN,
                &address,
                &hash,
                keys
            ),
            Err(TrustedSignerError::NotVerified)
        );
        // Another member's key.
        let (stranger, _, stranger_key) = member(12);
        assert_eq!(
            accept(
                &answer(&stranger, &stranger_key, &bytes, true),
                CHAIN,
                &address,
                &hash,
                keys
            ),
            Err(TrustedSignerError::ForeignKey)
        );
        // The member's key id with another key's signature.
        assert_eq!(
            accept(
                &answer(&stranger, &key, &bytes, true),
                CHAIN,
                &address,
                &hash,
                keys
            ),
            Err(TrustedSignerError::BadSignature)
        );
        // An approval "as" another member, signed with this member's keys.
        let (_, someone, _) = member(13);
        let theirs = digest(CHAIN, &someone, &hash).unwrap_or_default();
        assert_eq!(
            accept(
                &answer(&signing, &key, &theirs, true),
                CHAIN,
                &someone,
                &hash,
                keys
            ),
            Err(TrustedSignerError::ForeignKey)
        );
    }

    #[test]
    fn the_route_follows_the_chosen_venue_and_never_a_blocked_one() {
        use crate::signing_venue::{SigningVenue, VenueBlock};
        let plan = |venue: SigningVenue, blocked: Option<VenueBlock>| SigningPlan {
            domain: "getvela.app".into(),
            venue,
            blocked,
            key: None,
            key_label: crate::signing_venue::KeyLabel::default(),
        };
        assert_eq!(
            approval_route(&plan(SigningVenue::InVela, None)),
            ApprovalRoute::InVela
        );
        assert_eq!(
            approval_route(&plan(SigningVenue::official(), None)),
            ApprovalRoute::Page {
                url: "https://sign.getvela.app/".into()
            }
        );
        let block = VenueBlock::AppCannotReach {
            domain: "sign.example.com".into(),
        };
        assert_eq!(
            approval_route(&plan(SigningVenue::InVela, Some(block.clone()))),
            ApprovalRoute::Blocked { block }
        );
    }
}
