//! R22 (review 1-2, KNOWN-BUGS SEC-PARENT-3): one RPC never decides what
//! the signer believes.
//!
//! Every read a "plain" verdict stands on — the simulation, the state-diff
//! guard, `eth_getCode`, a multisig's owners, threshold, modules, guard and
//! nonce, an unlisted Safe's `isOwner` — is asked of two independent sources
//! ([`crate::app::rpc_pool`]'s corroborated calls) and believed only when
//! they agree. A shell hands the core both answers in one envelope:
//!
//! ```text
//! { "result" | "error" | "unreachable": …,     the first node's answer
//!   "witness": { "result" | "error": … },       a second, independent node's
//!   "ownNode": true }                            "Only use my node": trusted alone
//! ```
//!
//! Pure. What "independent" means ([`provider_of`]), how two answers are
//! compared for an exact read ([`same_answer`]) and what a read's standing is
//! ([`standing_exact`]) are here; a simulation's and a state diff's answers are
//! compared by what they MEAN (`sim_outcome::verdict_of_envelope`,
//! `multisig_review`), never byte for byte — two honest clients differ in
//! gas, block hashes and the accounts a prestate diff lists.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

/// The envelope's member carrying the second node's answer.
pub const WITNESS_KEY: &str = "witness";
/// The envelope's member saying the person's own node answered alone, by
/// their choice ("Only use my node").
pub const OWN_NODE_KEY: &str = "ownNode";

/// Who stands behind a read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "ReadStanding"))]
pub enum Standing {
    /// Two independent nodes said the same.
    Corroborated,
    /// The person's own node, alone, by their choice ("Only use my node").
    OwnNode,
    /// One node answered; nobody independent confirmed it.
    Single,
    /// Two independent nodes said different things.
    Disagreed,
}

impl Standing {
    /// May a plain verdict stand on it?
    #[must_use]
    pub fn believed(self) -> bool {
        matches!(self, Self::Corroborated | Self::OwnNode)
    }

    /// The reason word a could-not-check carries (`uncorroborated`,
    /// `disagreed`); `None` for a believed read.
    #[must_use]
    pub fn unread_reason(self) -> Option<&'static str> {
        match self {
            Self::Corroborated | Self::OwnNode => None,
            Self::Single => Some(UNCORROBORATED),
            Self::Disagreed => Some(DISAGREED),
        }
    }
}

/// The reason of a check nobody independent confirmed.
pub const UNCORROBORATED: &str = "uncorroborated";
/// The reason of a check two independent nodes answered differently.
pub const DISAGREED: &str = "disagreed";

// ---------------------------------------------------------------------------
// Independence
// ---------------------------------------------------------------------------

/// Registrable-domain suffixes under which every name is someone else's
/// (the Public Suffix List's private section, the hosts a public RPC is
/// likely to sit behind): `a.workers.dev` and `b.workers.dev` are two owners.
const SHARED_SUFFIXES: &[&str] = &[
    "workers.dev",
    "pages.dev",
    "vercel.app",
    "netlify.app",
    "herokuapp.com",
    "github.io",
    "onrender.com",
    "fly.dev",
    "deno.dev",
    "railway.app",
];

/// Two-label country suffixes (`co.uk`): the owner is the label before them.
const SECOND_LEVEL: &[&str] = &[
    "co", "com", "net", "org", "gov", "edu", "ac", "or", "ne", "go",
];

/// Who runs `url`, as far as the URL can say: its registrable domain
/// (`eth-mainnet.g.alchemy.com` and `polygon.g.alchemy.com` are both
/// `alchemy.com`; `rpc.ankr.com` is `ankr.com`), lowercased. A loopback or an
/// IP address is the address with its port: on a developer's machine every
/// node is `127.0.0.1`, and two nodes on two ports are two processes the
/// person started. Anything unparseable is the whole string — never equal to
/// anything else by accident.
#[must_use]
pub fn provider_of(url: &str) -> String {
    let trimmed = url.trim();
    let rest = trimmed.split_once("://").map_or(trimmed, |(_, rest)| rest);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or("");
    let authority = authority.rsplit('@').next().unwrap_or(authority);
    let (host, port) = split_port(authority);
    let host = host.trim_end_matches('.').to_ascii_lowercase();
    if host.is_empty() {
        return trimmed.to_ascii_lowercase();
    }
    let is_ip = host.starts_with('[') || host.parse::<std::net::Ipv4Addr>().is_ok();
    if is_ip || host == "localhost" || host.ends_with(".localhost") {
        return match port {
            Some(port) => format!("{host}:{port}"),
            None => host,
        };
    }
    let labels: Vec<&str> = host.split('.').collect();
    let take = |n: usize| labels[labels.len().saturating_sub(n)..].join(".");
    if labels.len() >= 3 {
        let two = take(2);
        if SHARED_SUFFIXES.contains(&two.as_str()) {
            return take(3);
        }
        let second = labels[labels.len() - 2];
        let tld = labels[labels.len() - 1];
        if tld.len() == 2 && SECOND_LEVEL.contains(&second) {
            return take(3);
        }
    }
    take(2)
}

fn split_port(authority: &str) -> (&str, Option<&str>) {
    if let Some(end) = authority.find(']') {
        // `[::1]:8545`
        let host = &authority[..=end];
        let port = authority[end + 1..].strip_prefix(':');
        return (host, port);
    }
    match authority.rsplit_once(':') {
        Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
            (host, Some(port))
        }
        _ => (authority, None),
    }
}

/// Are `a` and `b` run by different providers ([`provider_of`])?
#[must_use]
pub fn independent(a: &str, b: &str) -> bool {
    provider_of(a) != provider_of(b)
}

// ---------------------------------------------------------------------------
// The envelope
// ---------------------------------------------------------------------------

/// A node's JSON-RPC response body as an envelope: `{result}` or `{error}`.
/// A body that is neither is an answer nobody can read — an error with no
/// code, never a result.
#[must_use]
pub fn envelope_of_body(body: &Value) -> Value {
    if let Some(error) = body.get("error").filter(|e| !e.is_null()) {
        return json!({ "error": error });
    }
    if body.get("unreachable").and_then(Value::as_bool) == Some(true) {
        return json!({ "unreachable": true });
    }
    match body.get("result") {
        Some(result) => json!({ "result": result }),
        None => json!({ "error": { "code": null, "message": null } }),
    }
}

/// The one envelope a shell hands the core for a corroborated read: the
/// first node's body (`None`: the pool gave up — `{"unreachable": true}`),
/// the witness's body when a second, independent node answered, and whether
/// the pool holds the person's own node alone by their choice.
#[must_use]
pub fn corroborated_envelope(
    primary: Option<&Value>,
    witness: Option<&Value>,
    own_node: bool,
) -> Value {
    let mut envelope = match primary {
        Some(body) => envelope_of_body(body),
        None => return json!({ "unreachable": true }),
    };
    if let Some(witness) = witness {
        envelope[WITNESS_KEY] = envelope_of_body(witness);
    }
    if own_node {
        envelope[OWN_NODE_KEY] = Value::Bool(true);
    }
    envelope
}

/// [`corroborated_envelope`] over bodies as JSON text (UniFFI, wasm). Text
/// that is not JSON is an answer nobody can read.
#[must_use]
pub fn corroborated_envelope_json(
    primary_json: Option<&str>,
    witness_json: Option<&str>,
    own_node: bool,
) -> String {
    let parse = |text: &str| {
        serde_json::from_str::<Value>(text)
            .unwrap_or_else(|_| json!({ "error": { "code": null, "message": null } }))
    };
    let primary = primary_json.map(parse);
    let witness = witness_json.map(parse);
    corroborated_envelope(primary.as_ref(), witness.as_ref(), own_node).to_string()
}

/// The witness's envelope, when one rides in `envelope`.
#[must_use]
pub fn witness_of(envelope: &Value) -> Option<&Value> {
    envelope.get(WITNESS_KEY).filter(|w| w.is_object())
}

/// Did the person's own node answer alone, by their choice?
#[must_use]
pub fn own_node(envelope: &Value) -> bool {
    envelope.get(OWN_NODE_KEY).and_then(Value::as_bool) == Some(true)
}

/// `envelope` without its witness and own-node members: what one node said.
#[must_use]
pub fn first_answer(envelope: &Value) -> Value {
    let mut e = envelope.clone();
    if let Some(map) = e.as_object_mut() {
        map.remove(WITNESS_KEY);
        map.remove(OWN_NODE_KEY);
    }
    e
}

/// Do two envelopes say exactly the same thing? Two results equal as JSON
/// (hex strings compared without regard to case), or two errors with the same
/// code and `data` (a revert's payload). Silence agrees with nothing.
#[must_use]
pub fn same_answer(a: &Value, b: &Value) -> bool {
    match (a.get("result"), b.get("result")) {
        (Some(x), Some(y)) => return same_json(x, y),
        (Some(_), None) | (None, Some(_)) => return false,
        (None, None) => {}
    }
    match (
        a.get("error").filter(|e| !e.is_null()),
        b.get("error").filter(|e| !e.is_null()),
    ) {
        (Some(x), Some(y)) => {
            x.get("code") == y.get("code")
                && same_json(
                    x.get("data").unwrap_or(&Value::Null),
                    y.get("data").unwrap_or(&Value::Null),
                )
        }
        _ => false,
    }
}

fn same_json(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::String(x), Value::String(y)) => x.eq_ignore_ascii_case(y),
        (Value::Array(x), Value::Array(y)) => {
            x.len() == y.len() && x.iter().zip(y).all(|(x, y)| same_json(x, y))
        }
        (Value::Object(x), Value::Object(y)) => {
            x.len() == y.len()
                && x.iter()
                    .all(|(k, v)| y.get(k).is_some_and(|w| same_json(v, w)))
        }
        _ => a == b,
    }
}

/// The standing of an exact read's envelope ([`same_answer`]).
#[must_use]
pub fn standing_exact(envelope: &Value) -> Standing {
    if own_node(envelope) {
        return Standing::OwnNode;
    }
    match witness_of(envelope) {
        None => Standing::Single,
        Some(witness) if same_answer(&first_answer(envelope), witness) => Standing::Corroborated,
        Some(_) => Standing::Disagreed,
    }
}
