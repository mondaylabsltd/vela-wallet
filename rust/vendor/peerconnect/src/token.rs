//! Relay tokens (SPEC §4.1): the credential a relay MAY require, and the
//! per-client generations that make it revocable (`docs/self-serve.md`).
//!
//! A token is an HS256 JSON Web Token signed by the relay's own secret. It
//! says which client connects (`sub`), for which application (`app`), at
//! which generation (`gen`) and until when (`exp`). It carries nothing about
//! rooms, keys or content: it is what lets a relay operator know who uses the
//! relay, never what they say.
//!
//! Pure: times are the caller's.

use std::fmt;

use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::base64;

/// Seconds of clock skew tolerated when checking `exp` and `nbf`.
const LEEWAY: u64 = 60;
/// Shortest secret accepted, in bytes.
pub const MIN_SECRET_LEN: usize = 32;
/// Longest client or application ID accepted, in characters.
pub const MAX_ID_LEN: usize = 128;
/// Longest token accepted, in bytes: a token rides in a URL.
pub const MAX_TOKEN_LEN: usize = 2048;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Claims {
    /// The client: one installation of an application, or one of its users.
    pub sub: String,
    /// The application the client belongs to.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub app: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nbf: Option<u64>,
    /// The client's generation when the token was minted: a relay that keeps
    /// generations accepts only the current one. Absent is generation 0.
    #[serde(rename = "gen", default, skip_serializing_if = "Option::is_none")]
    pub generation: Option<u64>,
}

impl Claims {
    pub fn new(sub: impl Into<String>) -> Claims {
        Claims { sub: sub.into(), app: None, exp: None, nbf: None, generation: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenError {
    Malformed,
    Algorithm,
    Signature,
    Claims,
    Expired,
    NotYetValid,
}

impl fmt::Display for TokenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            TokenError::Malformed => "token is not a JWT",
            TokenError::Algorithm => "token algorithm must be HS256",
            TokenError::Signature => "token signature is invalid",
            TokenError::Claims => "token claims are invalid",
            TokenError::Expired => "token has expired",
            TokenError::NotYetValid => "token is not valid yet",
        })
    }
}

impl std::error::Error for TokenError {}

/// Whether `id` is a valid client or application ID: 1–128 characters from
/// `A–Z a–z 0–9 . _ - : @`.
pub fn is_id(id: &str) -> bool {
    (1..=MAX_ID_LEN).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_alphanumeric() || b".-_:@".contains(&b))
}

/// The `token` parameter of a request URL's query, if any. Tokens are
/// base64url and dots, so they need no percent-decoding.
pub fn from_query(query: &str) -> Option<&str> {
    query.split('&').find_map(|pair| pair.strip_prefix("token=")).filter(|t| !t.is_empty())
}

/// The secrets a relay accepts. The first one signs new tokens.
#[derive(Clone)]
pub struct Keys(Vec<Vec<u8>>);

impl fmt::Debug for Keys {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Keys({} secret(s))", self.0.len())
    }
}

impl Keys {
    /// Parses whitespace-separated secrets, newest first.
    pub fn parse(secrets: &str) -> Result<Keys, String> {
        let keys: Vec<Vec<u8>> = secrets.split_whitespace().map(|s| s.as_bytes().to_vec()).collect();
        if keys.is_empty() {
            return Err("no secret configured".into());
        }
        if keys.iter().any(|k| k.len() < MIN_SECRET_LEN) {
            return Err(format!("secrets must be at least {MIN_SECRET_LEN} bytes"));
        }
        Ok(Keys(keys))
    }

    /// Signs `claims` with the newest secret.
    pub fn sign(&self, claims: &Claims) -> String {
        let header = base64::encode(br#"{"alg":"HS256","typ":"JWT"}"#);
        let payload = base64::encode(&serde_json::to_vec(claims).expect("claims serialize"));
        let input = format!("{header}.{payload}");
        let signature = base64::encode(&mac(&self.0[0], input.as_bytes()).finalize().into_bytes());
        format!("{input}.{signature}")
    }

    /// The claims of `token`, if one of the secrets signed it and it is valid
    /// at `now` (Unix seconds).
    pub fn verify(&self, token: &str, now: u64) -> Result<Claims, TokenError> {
        if token.len() > MAX_TOKEN_LEN {
            return Err(TokenError::Malformed);
        }
        let Some((input, signature)) = token.rsplit_once('.') else {
            return Err(TokenError::Malformed);
        };
        let Some((header, payload)) = input.split_once('.').filter(|(_, p)| !p.contains('.')) else {
            return Err(TokenError::Malformed);
        };
        #[derive(Deserialize)]
        struct Header {
            alg: String,
            #[serde(default)]
            crit: Option<serde_json::Value>,
        }
        let header: Header = decode_json(header)?;
        // Only HS256: never let the token choose a weaker algorithm (RFC 8725 §2.1).
        // `crit` would name extensions we do not understand, so it must be absent.
        if header.alg != "HS256" || header.crit.is_some() {
            return Err(TokenError::Algorithm);
        }
        let signature = base64::decode(signature).ok_or(TokenError::Malformed)?;
        if !self.0.iter().any(|key| mac(key, input.as_bytes()).verify_slice(&signature).is_ok()) {
            return Err(TokenError::Signature);
        }
        let claims: Claims = decode_json(payload).map_err(|_| TokenError::Claims)?;
        if !is_id(&claims.sub) || claims.app.as_deref().is_some_and(|app| !is_id(app)) {
            return Err(TokenError::Claims);
        }
        if claims.exp.is_some_and(|exp| now > exp.saturating_add(LEEWAY)) {
            return Err(TokenError::Expired);
        }
        if claims.nbf.is_some_and(|nbf| now.saturating_add(LEEWAY) < nbf) {
            return Err(TokenError::NotYetValid);
        }
        Ok(claims)
    }
}

fn mac(key: &[u8], data: &[u8]) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC takes any key length");
    mac.update(data);
    mac
}

fn decode_json<T: serde::de::DeserializeOwned>(part: &str) -> Result<T, TokenError> {
    let bytes = base64::decode(part).ok_or(TokenError::Malformed)?;
    serde_json::from_slice(&bytes).map_err(|_| TokenError::Malformed)
}

/// A client's token generation: revocation for stateless tokens
/// (`docs/self-serve.md`, "Tokens on the relay").
///
/// A relay that keeps generations stores one per client. A token is accepted
/// only at the current generation. One generation is one credential: asked
/// for again, it is signed with the same claims and expiry, so it is the same
/// token, not another one.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Generation {
    #[serde(default)]
    pub current: u64,
    /// A deleted client: no token is accepted, none is minted.
    #[serde(default)]
    pub deleted: bool,
    /// New generations in the current issuance window.
    #[serde(default)]
    pub issued: Window,
    /// The expiry the current credential was issued with.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exp: Option<u64>,
}

/// A credential to sign: its generation and expiry, and whether it was issued
/// just now (a new credential) or is the current one shown again.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Credential {
    pub generation: u64,
    pub exp: u64,
    pub new: bool,
}

/// A fixed window: how many events since `started_ms`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Window {
    pub started_ms: u64,
    pub count: u32,
}

impl Window {
    /// Counts one event against `limit` per `window_ms`; refused past it. A
    /// window opens at its first event, not at the epoch.
    pub fn take(&mut self, limit: u32, window_ms: u64, now_ms: u64) -> bool {
        if self.count == 0 || now_ms.saturating_sub(self.started_ms) >= window_ms {
            *self = Window { started_ms: now_ms, count: 0 };
        }
        if self.count >= limit {
            return false;
        }
        self.count += 1;
        true
    }
}

/// Why a rotation, revocation or mint was refused.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The client was deleted.
    Deleted,
    /// More new generations than the issuance limit allows in its window.
    TooMany,
}

impl Generation {
    /// Whether a token at `generation` is accepted.
    pub fn accepts(&self, generation: u64) -> bool {
        !self.deleted && generation == self.current
    }

    /// The current credential: the first time it is asked for at its
    /// generation, with `exp`; every time after, the same one again.
    pub fn credential(&mut self, exp: u64) -> Result<Credential, Refusal> {
        if self.deleted {
            return Err(Refusal::Deleted);
        }
        if let Some(issued) = self.exp {
            return Ok(Credential { generation: self.current, exp: issued, new: false });
        }
        self.exp = Some(exp);
        Ok(Credential { generation: self.current, exp, new: true })
    }

    /// A new generation: every token of the old one stops at once. Counted
    /// against `limit` new generations per `window_ms`.
    pub fn rotate(&mut self, limit: u32, window_ms: u64, now_ms: u64) -> Result<u64, Refusal> {
        if self.deleted {
            return Err(Refusal::Deleted);
        }
        if !self.issued.take(limit, window_ms, now_ms) {
            return Err(Refusal::TooMany);
        }
        self.current += 1;
        self.exp = None;
        Ok(self.current)
    }

    /// The client is gone: no token is accepted again.
    pub fn delete(&mut self) {
        self.deleted = true;
        self.exp = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "0123456789abcdef0123456789abcdef";
    const HOUR: u64 = 60 * 60 * 1000;

    fn keys() -> Keys {
        Keys::parse(SECRET).unwrap()
    }

    fn claims() -> Claims {
        Claims { app: Some("app_x".into()), exp: Some(1_000), generation: Some(2), ..Claims::new("c-alice") }
    }

    #[test]
    fn round_trip() {
        let token = keys().sign(&claims());
        assert_eq!(keys().verify(&token, 900), Ok(claims()));
        let json = String::from_utf8(base64::decode(token.split('.').nth(1).unwrap()).unwrap()).unwrap();
        assert_eq!(json, r#"{"sub":"c-alice","app":"app_x","exp":1000,"gen":2}"#);
    }

    #[test]
    fn expiry_with_leeway() {
        let token = keys().sign(&claims());
        assert!(keys().verify(&token, 1_060).is_ok());
        assert_eq!(keys().verify(&token, 1_061), Err(TokenError::Expired));
        let early = keys().sign(&Claims { nbf: Some(500), ..claims() });
        assert_eq!(keys().verify(&early, 400), Err(TokenError::NotYetValid));
        assert!(keys().verify(&early, 450).is_ok());
    }

    #[test]
    fn rejects_tampering_and_foreign_keys() {
        let token = keys().sign(&claims());
        let other = Keys::parse(&"f".repeat(32)).unwrap();
        assert_eq!(other.verify(&token, 0), Err(TokenError::Signature));
        let forged = base64::encode(br#"{"sub":"mallory"}"#);
        let mut parts: Vec<&str> = token.split('.').collect();
        parts[1] = &forged;
        assert_eq!(keys().verify(&parts.join("."), 0), Err(TokenError::Signature));
        assert_eq!(keys().verify(&"a".repeat(MAX_TOKEN_LEN + 1), 0), Err(TokenError::Malformed));
    }

    #[test]
    fn rejects_other_algorithms() {
        let header = base64::encode(br#"{"alg":"none"}"#);
        let payload = base64::encode(br#"{"sub":"alice"}"#);
        assert_eq!(keys().verify(&format!("{header}.{payload}."), 0), Err(TokenError::Algorithm));
        assert_eq!(keys().verify("a.b", 0), Err(TokenError::Malformed));
    }

    #[test]
    fn rotation_accepts_every_configured_secret() {
        let old = keys().sign(&claims());
        let rotated = Keys::parse(&format!("{} {SECRET}", "n".repeat(32))).unwrap();
        assert!(rotated.verify(&old, 0).is_ok());
        assert!(rotated.verify(&rotated.sign(&claims()), 0).is_ok());
        assert!(keys().verify(&rotated.sign(&claims()), 0).is_err());
    }

    #[test]
    fn validates_secrets_and_ids() {
        assert!(Keys::parse("short").is_err());
        assert!(Keys::parse("  ").is_err());
        assert!(is_id("user:42@app.example-1_x"));
        assert!(!is_id(""));
        assert!(!is_id("a b"));
        assert!(!is_id(&"a".repeat(129)));
        let bad = keys().sign(&Claims::new("a/b"));
        assert_eq!(keys().verify(&bad, 0), Err(TokenError::Claims));
        let bad_app = keys().sign(&Claims { app: Some("a b".into()), ..Claims::new("c") });
        assert_eq!(keys().verify(&bad_app, 0), Err(TokenError::Claims));
    }

    #[test]
    fn finds_the_token_in_a_query() {
        assert_eq!(from_query("token=a.b.c"), Some("a.b.c"));
        assert_eq!(from_query("x=1&token=a.b.c&y=2"), Some("a.b.c"));
        assert_eq!(from_query("x=1"), None);
        assert_eq!(from_query("token="), None);
        assert_eq!(from_query("mytoken=a"), None);
    }

    #[test]
    fn tokens_without_a_generation_are_generation_0() {
        let fresh = Generation::default();
        assert!(fresh.accepts(0));
        assert!(!fresh.accepts(1), "nothing was minted at 1");
    }

    #[test]
    fn a_rotation_stops_the_old_token() {
        let mut g = Generation::default();
        assert_eq!(g.rotate(30, HOUR, 0), Ok(1));
        assert!(!g.accepts(0));
        assert!(g.accepts(1));
    }

    #[test]
    fn showing_a_credential_again_is_not_issuing_one() {
        let mut g = Generation::default();
        assert_eq!(g.credential(5_000), Ok(Credential { generation: 0, exp: 5_000, new: true }));
        for _ in 0..100 {
            assert_eq!(g.credential(9_999), Ok(Credential { generation: 0, exp: 5_000, new: false }));
        }
        assert_eq!(g.issued.count, 0, "nothing counted");
        g.rotate(30, HOUR, 0).unwrap();
        assert_eq!(g.credential(7_000), Ok(Credential { generation: 1, exp: 7_000, new: true }));
    }

    #[test]
    fn a_deleted_client_accepts_nothing_and_mints_nothing() {
        let mut g = Generation::default();
        g.delete();
        assert!(!g.accepts(0));
        assert_eq!(g.rotate(30, HOUR, 0), Err(Refusal::Deleted));
        assert_eq!(g.credential(1), Err(Refusal::Deleted));
    }

    #[test]
    fn rotations_are_limited_per_window() {
        let mut g = Generation::default();
        for _ in 0..3 {
            assert!(g.rotate(3, HOUR, 10).is_ok());
        }
        assert_eq!(g.rotate(3, HOUR, 10 + HOUR - 1), Err(Refusal::TooMany));
        assert_eq!(g.current, 3, "a refused rotation changes nothing");
        assert_eq!(g.rotate(3, HOUR, 10 + HOUR), Ok(4), "a new window");
    }

    #[test]
    fn what_is_stored_reads_back_and_absent_fields_are_zero() {
        let mut g = Generation::default();
        g.rotate(30, HOUR, 5).unwrap();
        g.credential(77).unwrap();
        let text = serde_json::to_string(&g).unwrap();
        assert_eq!(serde_json::from_str::<Generation>(&text).unwrap(), g);
        assert_eq!(serde_json::from_str::<Generation>("{}").unwrap(), Generation::default());
    }
}
