//! A reverse-resolved name is not a name until it resolves back (spec 081,
//! FR-010).
//!
//! Every shell asked one question — *what name does this address claim?* — and
//! drew the answer beside somebody's money. `addr.reverse` is writable by the
//! address itself and by nobody else, which sounds like a guarantee and is the
//! opposite of one: it means an attacker who controls an address controls the
//! string the wallet shows for it. Set the reverse record of a freshly funded
//! address to `vitalik.eth`, `binance.eth`, or the name of the contact somebody
//! is about to pay, and four wallets called it that.
//!
//! ENS has always said the record is a *claim*: a reverse name counts only when
//! resolving that name FORWARD lands on the same address. Nothing anywhere in
//! this repository did that, so this module does it, once, for all four shells
//! (`docs/ARCHITECTURE.md`: business rules live in the core).
//!
//! ## The shape is the one the registry walk already uses
//!
//! [`step`] is a pure function of the transcript so far, exactly like
//! [`crate::registry_lookup::step`] and [`crate::registry_resolve::key_step`]:
//! a shell calls it with the answers it has, performs the `eth_call`s it is
//! handed, appends what came back and calls again until [`VerifyStep::Done`].
//! It reuses that walk's [`LookupRequest`] / [`LookupAnswer`] vocabulary on
//! purpose — every shell already owns a transport that speaks it, so adding
//! this rule costs a call site rather than a new boundary.
//!
//! ## What is asked
//!
//! 1. `registry.resolver(namehash(name))` — who answers for this name;
//! 2. `resolver.addr(namehash(name))` — the address it answers with;
//! 3. and, when the name's own node has no resolver, ENSIP-10: the nearest
//!    ANCESTOR's resolver is asked `resolve(dnsEncode(name), addr(node))`.
//!    Most Basenames and every "wildcard" subdomain resolve only this way, so
//!    skipping it would silently refuse names that are perfectly real.
//!
//! Then one comparison, case-insensitive, against the address the reverse
//! record came from.
//!
//! ## Nothing but `Verified` may be shown
//!
//! [`ForwardState::Mismatch`] is the attack above and [`ForwardState::Unavailable`]
//! is an RPC that did not answer; both mean the shell shows the bare address,
//! which is a state it already has copy for. Failing open — showing the
//! unverified name when the network is down — would hand the attacker a way to
//! *create* the state that suits them, so the timeout case is the one that has
//! to fail closed.
//!
//! A CCIP-read resolver reverts its `eth_call` with `OffchainLookup` rather
//! than answering; a shell reports that as [`LookupOutcome::Failed`] and the
//! verdict is `Unavailable`. That is the honest answer: without following the
//! gateway we do not know, and a name we do not know is not shown.

use serde::{Deserialize, Serialize};

use crate::primitives::keccak256;
use crate::registry_lookup::{LookupAnswer, LookupOutcome, LookupRequest};

/// `resolver(bytes32)` on an ENS-compatible registry.
const SEL_RESOLVER: &str = "0178b8bf";
/// `addr(bytes32)` on a resolver (EIP-137).
const SEL_ADDR: &str = "3b3b57de";
/// `resolve(bytes,bytes)` — the ENSIP-10 extended resolver interface.
const SEL_RESOLVE: &str = "9061b923";

/// How far up the name a wildcard resolver is looked for. `a.b.c.d.eth` is
/// already further than any name a reverse record realistically holds, and the
/// cap is what keeps a crafted 60-label name from costing sixty `eth_call`s.
const MAX_ANCESTORS: usize = 4;

/// The longest name worth a round trip. A DNS name cannot exceed 255 bytes and
/// no honest reverse record approaches it; a longer one is a payload, not a
/// name.
const MAX_NAME_BYTES: usize = 255;

/// Whether the name a reverse record gave has been shown to belong to the
/// address it came from.
///
/// The display rule is one line and lives with every consumer: a name is drawn
/// only for [`Verified`](ForwardState::Verified). Everything else shows the
/// address alone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForwardState {
    /// Nobody asked. A Vela registry wallet name is a LABEL somebody registered
    /// beside their own founding key, not a record anything resolves, so it has
    /// no forward direction to check and keeps this state — the shells never
    /// put those names to this machine.
    Unchecked,
    /// The name resolves forward to exactly this address.
    Verified,
    /// The name resolves forward to a different address, to none at all, or is
    /// not a resolvable name in the first place. Somebody claimed a name that
    /// is not theirs — or the record is junk. Either way it is not shown.
    Mismatch,
    /// Nobody answered: an RPC failure, a timeout, a resolver that reverts into
    /// CCIP-read. Not a verdict — ask again next time, and meanwhile show the
    /// address.
    Unavailable,
}

/// The next thing to do. `Ask` carries `eth_call`s in the same shape the
/// registry walk uses, so a shell's existing transport performs them unchanged.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VerifyStep {
    /// Fetch all of these (together, if the shell can), append the answers to
    /// the transcript, and call [`step`] again.
    Ask { requests: Vec<LookupRequest> },
    Done {
        forward_state: ForwardState,
        /// The name exactly as it was PROVEN, normalised — `Some` only when
        /// `forward_state` is [`ForwardState::Verified`].
        ///
        /// Shells draw this string rather than the one their reverse lookup
        /// returned, so what is displayed and what was checked cannot drift:
        /// `Vitalik.ETH` and `vitalik.eth` are one ENS name but two pixels, and
        /// the one that was verified is the one worth showing.
        name: Option<String>,
    },
}

fn done(forward_state: ForwardState) -> VerifyStep {
    VerifyStep::Done {
        forward_state,
        name: None,
    }
}

fn verified(name: String) -> VerifyStep {
    VerifyStep::Done {
        forward_state: ForwardState::Verified,
        name: Some(name),
    }
}

fn ask(request: LookupRequest) -> VerifyStep {
    VerifyStep::Ask {
        requests: vec![request],
    }
}

/// The address a reverse record was read for, lowercased bare hex, or `None`
/// when it is not an address anybody could hold.
fn askable(address: &str) -> Option<String> {
    let hex = address.strip_prefix("0x").unwrap_or(address).to_lowercase();
    if hex.len() != 40 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    // The zero address is a mint/burn counterparty, not somebody with a name.
    hex.bytes().any(|b| b != b'0').then_some(hex)
}

/// The name in the form it will be hashed and shown in, or `None` when the
/// record is not a resolvable name at all.
///
/// Lowercasing is the part of UTS-46 that decides ordinary names; a name whose
/// normalisation needs more than that will simply fail to resolve forward and
/// therefore not be shown, which is the safe direction for a rule whose whole
/// job is to refuse strings it cannot vouch for.
fn normalize(name: &str) -> Option<String> {
    let name = name.trim().to_lowercase();
    if name.is_empty() || name.len() > MAX_NAME_BYTES {
        return None;
    }
    // A name is drawn in a list beside other people's names. Whitespace,
    // control characters and the invisible formatting codepoints are how one
    // row is made to look like two, or like a row it is not — a right-to-left
    // override turns `alice.eth` into something that reads as another name
    // entirely. ENSIP-15 disallows every one of them, so a record carrying one
    // is not a name this machine will vouch for.
    if name.chars().any(is_never_in_a_name) {
        return None;
    }
    // A single label is not a name anything resolves: `alice` has no registry
    // to ask. Empty labels (`a..eth`, `.eth`, `alice.`) are not names either.
    let labels: Vec<&str> = name.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|label| label.is_empty()) {
        return None;
    }
    Some(name)
}

/// Characters no normalised name contains, and that a hostile record uses to
/// make one name look like another: C0/C1 controls, every flavour of space,
/// and the invisible formatting blocks (zero width, bidi overrides, variation
/// selectors, the interlinear annotations).
fn is_never_in_a_name(c: char) -> bool {
    c.is_control()
        || c.is_whitespace()
        || matches!(c,
            '\u{200B}'..='\u{200F}'
            | '\u{202A}'..='\u{202E}'
            | '\u{2060}'..='\u{206F}'
            | '\u{FE00}'..='\u{FE0F}'
            | '\u{FFF9}'..='\u{FFFB}'
            | '\u{FEFF}')
}

/// EIP-137 namehash, as bare hex: `namehash("")` is 32 zero bytes, then the
/// labels fold in right to left.
fn namehash(name: &str) -> String {
    let mut node = vec![0u8; 32];
    if !name.is_empty() {
        for label in name.split('.').rev() {
            let mut combined = node;
            combined.extend_from_slice(&keccak256(label.as_bytes()));
            node = keccak256(&combined);
        }
    }
    hex(&node)
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// DNS wire format, as ENSIP-10 wants it: each label length-prefixed, a zero
/// byte at the end. `None` when a label will not fit in its length byte.
fn dns_encode(name: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(name.len() + 2);
    for label in name.split('.') {
        let bytes = label.as_bytes();
        if bytes.is_empty() || bytes.len() > 255 {
            return None;
        }
        out.push(u8::try_from(bytes.len()).ok()?);
        out.extend_from_slice(bytes);
    }
    out.push(0);
    Some(out)
}

/// A 32-byte ABI word as 64 hex characters, left-padded.
fn word(bytes: &[u8]) -> String {
    let mut padded = vec![0u8; 32usize.saturating_sub(bytes.len())];
    padded.extend_from_slice(bytes);
    hex(&padded)
}

/// `eth_call` for `registry.resolver(namehash(name))`.
fn resolver_request(id: String, chain_id: u32, registry: &str, name: &str) -> LookupRequest {
    LookupRequest::EthCall {
        id,
        chain_id,
        to: registry.to_owned(),
        data: format!("0x{SEL_RESOLVER}{}", namehash(name)),
    }
}

/// `eth_call` for `resolver.addr(namehash(name))`.
fn addr_request(id: String, chain_id: u32, resolver: &str, name: &str) -> LookupRequest {
    LookupRequest::EthCall {
        id,
        chain_id,
        to: format!("0x{resolver}"),
        data: format!("0x{SEL_ADDR}{}", namehash(name)),
    }
}

/// `eth_call` for ENSIP-10 `resolver.resolve(dnsEncode(name), addr(node))` —
/// two dynamic `bytes` arguments, so two offsets then the two payloads.
fn resolve_request(
    id: String,
    chain_id: u32,
    resolver: &str,
    name: &str,
) -> Option<LookupRequest> {
    let dns = dns_encode(name)?;
    let inner = format!("{SEL_ADDR}{}", namehash(name));
    let dns_len = dns.len();
    // head: offset(name) = 0x40, offset(data) = 0x40 + 32 + padded(name)
    let padded_dns = dns_len.next_multiple_of(32);
    let mut data = String::from("0x");
    data.push_str(SEL_RESOLVE);
    data.push_str(&word(&64u32.to_be_bytes()));
    data.push_str(&word(&u32::try_from(96 + padded_dns).ok()?.to_be_bytes()));
    data.push_str(&word(&u32::try_from(dns_len).ok()?.to_be_bytes()));
    data.push_str(&format!("{:0<width$}", hex(&dns), width = padded_dns * 2));
    // The inner call is 36 bytes: selector + one word.
    data.push_str(&word(&36u32.to_be_bytes()));
    data.push_str(&format!("{inner:0<128}"));
    Some(LookupRequest::EthCall {
        id,
        chain_id,
        to: format!("0x{resolver}"),
        data,
    })
}

/// What one `eth_call` said, once the outcome and the hex are both accounted
/// for.
enum Said<T> {
    /// A readable answer.
    Ok(T),
    /// The other side said "nothing here" — a zero word, an empty return.
    Nothing,
    /// Nobody said anything, or said something this build cannot read.
    Silent,
}

/// The raw hex an answer carries, if it carries one.
fn body(answer: &LookupAnswer) -> Said<String> {
    if answer.outcome != LookupOutcome::Ok {
        return Said::Silent;
    }
    match answer.body.as_deref() {
        Some(hex) => {
            let hex = hex.strip_prefix("0x").unwrap_or(hex).to_lowercase();
            if !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                // An RPC that answers with prose is not answering.
                return Said::Silent;
            }
            // A bare `0x` is a contract that is not there, or a call that
            // returned nothing. Neither is a resolver and neither is a fault.
            if hex.is_empty() {
                Said::Nothing
            } else {
                Said::Ok(hex)
            }
        }
        None => Said::Silent,
    }
}

/// The address in the first 32-byte word of a return value.
fn address_word(hex: &str) -> Said<String> {
    let Some(first) = hex.get(..64) else {
        // Shorter than one word: not an address, and not an answer we can use.
        return Said::Silent;
    };
    let address = first.get(24..).unwrap_or_default().to_owned();
    if address.bytes().all(|b| b == b'0') {
        // A zero address is the registry's way of saying "no record".
        Said::Nothing
    } else {
        Said::Ok(address)
    }
}

/// The address inside the `bytes` an ENSIP-10 `resolve` call returns: a dynamic
/// return, so offset, length, then the inner `addr` word.
fn address_in_resolve_return(hex: &str) -> Said<String> {
    let Some(offset) = hex.get(..64).and_then(|w| usize::from_str_radix(w, 16).ok()) else {
        return Said::Silent;
    };
    let Some(length) = offset
        .checked_mul(2)
        .and_then(|at| hex.get(at..at.checked_add(64)?))
        .and_then(|w| usize::from_str_radix(w, 16).ok())
    else {
        return Said::Silent;
    };
    // An `addr(bytes32)` answer is exactly one word. An empty one is the
    // resolver saying the name has no address.
    if length == 0 {
        return Said::Nothing;
    }
    if length < 32 {
        return Said::Silent;
    }
    match offset
        .checked_mul(2)
        .and_then(|at| at.checked_add(64))
        .and_then(|at| hex.get(at..at.checked_add(64)?))
    {
        Some(payload) => address_word(payload),
        None => Said::Silent,
    }
}

/// The transcript id for the resolver of the name's `depth`-th ancestor; `0` is
/// the name itself.
fn resolver_id(depth: usize) -> String {
    format!("resolver:{depth}")
}

/// The name `depth` labels up from `name`, or `None` past the root.
fn ancestor(name: &str, depth: usize) -> Option<&str> {
    let mut rest = name;
    for _ in 0..depth {
        rest = rest.split_once('.')?.1;
    }
    // A single label has no registry entry worth asking about.
    rest.contains('.').then_some(rest)
}

/// The next thing to do, given everything learned so far.
///
/// `registry` is the ENS-compatible registry the shell read the reverse record
/// from, on `chain_id`; `address` is the address it was read FOR and `name`
/// what it claimed. `answers` is the whole transcript, in any order; an id
/// answered twice reads its first answer, and unknown ids are ignored.
#[must_use]
pub fn step(
    chain_id: u32,
    registry: &str,
    address: &str,
    name: &str,
    answers: &[LookupAnswer],
) -> VerifyStep {
    let Some(wanted) = askable(address) else {
        return done(ForwardState::Mismatch);
    };
    // A record that is not a resolvable name has nothing to resolve forward to,
    // and that is not a technicality: `Vitalik Buterin` in a reverse record is
    // precisely the attack, dressed as a display name.
    let Some(name) = normalize(name) else {
        return done(ForwardState::Mismatch);
    };
    let answer = |id: &str| answers.iter().find(|a| a.id == id);

    // --- Who answers for this name -----------------------------------------
    for depth in 0..=MAX_ANCESTORS {
        let Some(asking_about) = (if depth == 0 {
            Some(name.as_str())
        } else {
            ancestor(&name, depth)
        }) else {
            // Walked past the root without finding a resolver: nobody resolves
            // this name, so nobody can have proven it.
            return done(ForwardState::Mismatch);
        };
        let id = resolver_id(depth);
        let Some(said) = answer(&id) else {
            return ask(resolver_request(id, chain_id, registry, asking_about));
        };
        let resolver = match body(said).and_then(|hex| address_word(hex)) {
            Said::Ok(resolver) => resolver,
            // The registry said "no resolver here". For the name's own node
            // that is ENSIP-10's cue to try its ancestors; for an ancestor it
            // is the next rung of the same ladder.
            Said::Nothing => continue,
            Said::Silent => return done(ForwardState::Unavailable),
        };

        // --- And what address it answers with ------------------------------
        //
        // A resolver found at the name's OWN node answers `addr(node)`; one
        // found at an ancestor is a wildcard resolver and must be asked through
        // ENSIP-10's `resolve`, because `addr(node)` on it is about the
        // ancestor, not about this name.
        if depth == 0 {
            let Some(said) = answer("addr") else {
                return ask(addr_request("addr".to_owned(), chain_id, &resolver, &name));
            };
            match body(said).and_then(|hex| address_word(hex)) {
                Said::Ok(found) => return verdict(&found, &wanted, name),
                // The name exists and has no address record: it is not this
                // address's name.
                Said::Nothing => return done(ForwardState::Mismatch),
                // An extended resolver may implement `resolve` and nothing
                // else, so a malformed `addr` answer is worth one more
                // question before giving up on the name.
                Said::Silent => {}
            }
        }

        let Some(said) = answer("resolve") else {
            let Some(request) = resolve_request("resolve".to_owned(), chain_id, &resolver, &name)
            else {
                return done(ForwardState::Mismatch);
            };
            return ask(request);
        };
        return match body(said).and_then(|hex| address_in_resolve_return(hex)) {
            Said::Ok(found) => verdict(&found, &wanted, name),
            Said::Nothing => done(ForwardState::Mismatch),
            Said::Silent => done(ForwardState::Unavailable),
        };
    }
    done(ForwardState::Mismatch)
}

impl<T> Said<T> {
    fn and_then<U>(self, f: impl FnOnce(&T) -> Said<U>) -> Said<U> {
        match self {
            Said::Ok(value) => f(&value),
            Said::Nothing => Said::Nothing,
            Said::Silent => Said::Silent,
        }
    }
}

/// The one comparison the whole module exists for. Both sides are bare
/// lowercase hex by construction, so this is case-insensitive without anybody
/// having to remember to make it so — EIP-55 checksum casing is a display
/// convention, and two spellings of one address must not read as two addresses.
fn verdict(found: &str, wanted: &str, name: String) -> VerifyStep {
    if found == wanted {
        verified(name)
    } else {
        done(ForwardState::Mismatch)
    }
}

/// [`step`] over JSON, for the bindings: `answers_json` is a `LookupAnswer[]`,
/// the return a [`VerifyStep`]. A transcript that does not parse is read as
/// empty — verification starts over rather than failing, and a serialisation
/// that somehow fails reads as "nobody answered", never as a verified name.
#[must_use]
pub fn step_json(
    chain_id: u32,
    registry: &str,
    address: &str,
    name: &str,
    answers_json: &str,
) -> String {
    let answers: Vec<LookupAnswer> = serde_json::from_str(answers_json).unwrap_or_default();
    serde_json::to_string(&step(chain_id, registry, address, name, &answers)).unwrap_or_else(|_| {
        r#"{"type":"done","forward_state":"unavailable","name":null}"#.to_owned()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// EIP-137's own worked examples, so the hash the forward call is built on
    /// is the hash the registry indexes by.
    #[test]
    fn namehash_matches_the_specification() {
        assert_eq!(namehash(""), "0".repeat(64));
        assert_eq!(
            namehash("eth"),
            "93cdeb708b7545dc668eb9280176169d1c33cfd8ed6f04690a0bcc88a93fc4ae"
        );
        assert_eq!(
            namehash("foo.eth"),
            "de9b09fd7c5f901e23a3f19fecc54828e9c848539801e86591bd9801b019f84f"
        );
    }

    /// ENSIP-10 hands the resolver a DNS-encoded name, not a dotted one.
    #[test]
    fn dns_encoding_is_length_prefixed_and_terminated() {
        assert_eq!(
            dns_encode("foo.eth").map(|b| hex(&b)),
            Some("03666f6f03657468 00".replace(' ', ""))
        );
        assert_eq!(dns_encode("a..eth"), None, "an empty label has no length");
    }

    /// The strings that must never reach the network, and never be shown.
    #[test]
    fn a_record_that_is_not_a_name_is_refused_before_any_call() {
        assert_eq!(normalize("vitalik.eth"), Some("vitalik.eth".to_owned()));
        assert_eq!(normalize("Vitalik.ETH"), Some("vitalik.eth".to_owned()));
        assert_eq!(normalize(""), None);
        assert_eq!(normalize("alice"), None, "a single label resolves nowhere");
        assert_eq!(normalize(".eth"), None);
        assert_eq!(normalize("alice."), None);
        assert_eq!(normalize("a..eth"), None);
        assert_eq!(
            normalize("Vitalik Buterin"),
            None,
            "the attack, dressed as a display name"
        );
        assert_eq!(normalize("a\u{202e}b.eth"), None, "a control character");
        assert_eq!(normalize(&format!("{}.eth", "x".repeat(300))), None);
    }

    #[test]
    fn ancestors_walk_up_and_stop_at_the_root() {
        assert_eq!(ancestor("a.b.base.eth", 1), Some("b.base.eth"));
        assert_eq!(ancestor("a.b.base.eth", 2), Some("base.eth"));
        assert_eq!(ancestor("a.b.base.eth", 3), None, "`eth` is not a name");
        assert_eq!(ancestor("a.b.base.eth", 9), None);
    }
}
