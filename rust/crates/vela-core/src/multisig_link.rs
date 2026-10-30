//! The multisig's links and the small readings around them, as pure rules
//! every shell shares: what a link says, what one that does not read says
//! instead, and what the shell keeps of it.
//!
//! Ported from the web's TypeScript (`app-web/vela-wallet/src/lib/multisig/`),
//! which stays the reference and keeps its own copy (the web reads a link
//! before any wasm loads); the vectors in `tests/vectors/multisig/links.json`
//! pin the two to the same answers.
//!
//! - **The proposal link** (`core/link.ts`) —
//!   `…/wallet#ms=<chainId>.<multisig>.<safeTxHash>`: names a transaction to
//!   approve, nothing more ([`proposal_link`], [`parse_proposal_link`]).
//! - **The invite** (`core/link.ts`, `core/invite.ts`) —
//!   `…/wallet#msa=<chainId>,<chainId>:<multisig>&a=<member>,<member>`: the
//!   multisig, the networks it is used on, and what the inviter knows of its
//!   members — labels only ([`add_link`], [`parse_add_link`],
//!   [`add_link_fault`], [`invited_members`], [`invite_link`]).
//! - **Damaged links** — a hash meant as one of these that does not read is
//!   said, never ignored ([`broken_link`], [`broken_link_text`]), and an
//!   address whose letter case is not its EIP-55 checksum is refused
//!   ([`checksum_holds`]).
//! - **A link kept through onboarding** (`core/kept-link.ts`) — the record a
//!   shell stores and how it reads it back ([`keep_link`], [`take_kept_link`]).
//! - **The connect page's link** (`external/connect-link.ts`) —
//!   `/connect#<invite>&for=<member>,…&ms=<tx>` ([`connect_link`],
//!   [`read_connect_hash`], [`is_expected`], [`connect_boot_reads_tx`]).
//! - **Activity row ids** (`core/row-id.ts`), **amounts** (`core/units.ts`)
//!   and **the approvals line** (`core/progress.ts`).
//!
//! Every reading here follows the TypeScript to the character: the
//! ECMAScript `trim` white-space set, `decodeURIComponent`'s refusals,
//! `encodeURIComponent`'s unreserved set, string lengths in UTF-16 code units
//! and `Number.isSafeInteger` for a chain id. No I/O; [`call_json`] is the
//! JSON surface a wasm or uniffi shell calls.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;
use crate::primitives;

#[cfg(feature = "bindings")]
use ts_rs::TS;

fn err(message: impl Into<String>) -> CoreError {
    CoreError::Multisig(message.into())
}

/// The wallet's public web address (`$lib/site`'s `SITE_ORIGIN`).
pub const SITE_ORIGIN: &str = "https://wallet.getvela.app";

/// `Number.MAX_SAFE_INTEGER`: a chain id above it does not read.
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

// ---------------------------------------------------------------------------
// ECMAScript string rules
// ---------------------------------------------------------------------------

/// ECMAScript `WhiteSpace` and `LineTerminator` — what `String.prototype.trim`
/// and a regex's `\s` take. Not Rust's `char::is_whitespace`: that has U+0085
/// and lacks U+FEFF.
fn is_js_space(c: char) -> bool {
    matches!(
        c,
        '\u{9}' | '\u{a}' | '\u{b}' | '\u{c}' | '\u{d}' | ' ' | '\u{a0}' | '\u{1680}' | '\u{2000}'
            ..='\u{200a}'
                | '\u{2028}'
                | '\u{2029}'
                | '\u{202f}'
                | '\u{205f}'
                | '\u{3000}'
                | '\u{feff}'
    )
}

fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_space)
}

/// `s.length`: UTF-16 code units.
fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// `s.slice(0, n)` in UTF-16 code units, or `None` when the cut splits a
/// surrogate pair (the half `encodeURIComponent` would then throw on).
fn js_head(s: &str, n: usize) -> Option<&str> {
    let mut units = 0;
    for (at, c) in s.char_indices() {
        if units == n {
            return Some(&s[..at]);
        }
        units += c.len_utf16();
        if units > n {
            return None;
        }
    }
    Some(s)
}

/// `s.slice(n)` in UTF-16 code units (a split pair's half becomes U+FFFD).
fn js_tail(s: &str, n: usize) -> String {
    let units: Vec<u16> = s.encode_utf16().skip(n).collect();
    String::from_utf16_lossy(&units)
}

/// `decodeURIComponent`, or `None` where it throws: a `%` not followed by two
/// hex digits, or bytes that are not UTF-8 (a surrogate, an overlong form).
fn decode_uri_component(s: &str) -> Option<String> {
    if !s.contains('%') {
        return Some(s.to_owned());
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let hex = bytes.get(i + 1..i + 3)?;
            let text = std::str::from_utf8(hex).ok()?;
            if !text.bytes().all(|b| b.is_ascii_hexdigit()) {
                return None;
            }
            out.push(u8::from_str_radix(text, 16).ok()?);
            i += 3;
        } else {
            out.push(bytes[i]);
            i += 1;
        }
    }
    String::from_utf8(out).ok()
}

/// `encodeURIComponent`: everything but `A–Z a–z 0–9 - _ . ! ~ * ' ( )`
/// escaped as UTF-8, upper-case hex.
fn encode_uri_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(char::from(b));
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

fn starts_with_ignore_ascii_case(s: &str, prefix: &str) -> bool {
    s.len() >= prefix.len() && s.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
}

/// `/^0x[0-9a-fA-F]{n}$/`.
fn is_hex_of(s: &str, n: usize) -> bool {
    s.len() == n + 2 && s.starts_with("0x") && s.as_bytes()[2..].iter().all(u8::is_ascii_hexdigit)
}

/// `/^0x[0-9a-fA-F]{40}$/`.
fn is_address(s: &str) -> bool {
    is_hex_of(s, 40)
}

/// `/^[1-9]\d{0,max}$/`.
fn is_positive_decimal(s: &str, max_digits: usize) -> bool {
    let b = s.as_bytes();
    !b.is_empty()
        && b.len() <= max_digits
        && (b'1'..=b'9').contains(&b[0])
        && b.iter().all(u8::is_ascii_digit)
}

/// A chain id as a link writes it: `/^[1-9]\d{0,15}$/`, and a safe integer.
fn chain_id_of(s: &str) -> Option<u64> {
    if !is_positive_decimal(s, 16) {
        return None;
    }
    s.parse::<u64>().ok().filter(|n| *n <= MAX_SAFE_INTEGER)
}

/// `sameAddress`: both given, equal ignoring case.
fn same_address(a: &str, b: &str) -> bool {
    !a.is_empty() && !b.is_empty() && a.to_lowercase() == b.to_lowercase()
}

// ---------------------------------------------------------------------------
// The proposal link
// ---------------------------------------------------------------------------

/// The transaction a proposal link names (`ProposalLink`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ProposalLink {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub multisig: String,
    pub safe_tx_hash: String,
}

const PREFIX: &str = "ms=";
const ADD_PREFIX: &str = "msa=";
/// Founding members in the genesis's order, no addresses (links from before r6).
const MEMBERS: &str = "m=";
/// r4D-5: every member the inviter knows, each by its address.
const ADDRESSED: &str = "a=";

/// The origin a link made here is for (`shareOrigin`): `origin` when it is a
/// web page anyone can open (`http://` / `https://`), else the wallet's public
/// web address — an extension's `chrome-extension://…` opens nowhere else.
pub fn share_origin(origin: &str) -> String {
    if origin.starts_with("http://") || origin.starts_with("https://") {
        origin.to_owned()
    } else {
        SITE_ORIGIN.to_owned()
    }
}

/// `<origin>/<locale>/wallet#ms=<chainId>.<multisig>.<safeTxHash>`, as given.
pub fn proposal_link(origin: &str, locale: &str, link: &ProposalLink) -> String {
    format!(
        "{origin}/{locale}/wallet#{PREFIX}{}.{}.{}",
        link.chain_id, link.multisig, link.safe_tx_hash
    )
}

/// A hash as the link meant it (`linkBody`): the `#` and the white space a
/// chat app or mail client put around it dropped, and `#ms%3D…` / `#msa%3D…`
/// (an encoded `=`, any case) decoded whole. Anything else as given.
fn link_body(hash: &str) -> String {
    let body = js_trim(hash.strip_prefix('#').unwrap_or(hash));
    if !starts_with_ignore_ascii_case(body, "ms%3d")
        && !starts_with_ignore_ascii_case(body, "msa%3d")
    {
        return body.to_owned();
    }
    decode_uri_component(body).unwrap_or_else(|| body.to_owned())
}

/// The proposal link in a location hash, or `None`. Exactly three parts; the
/// addresses and hash keep the case they came in.
pub fn parse_proposal_link(hash: &str) -> Option<ProposalLink> {
    let body = link_body(hash);
    let rest = body.strip_prefix(PREFIX)?;
    let parts: Vec<&str> = rest.split('.').collect();
    if parts.len() > 3 {
        return None;
    }
    let chain_id = chain_id_of(parts.first().copied().unwrap_or(""))?;
    let multisig = parts.get(1).copied().unwrap_or("");
    let safe_tx_hash = parts.get(2).copied().unwrap_or("");
    if !is_address(multisig) || !is_hex_of(safe_tx_hash, 64) {
        return None;
    }
    Some(ProposalLink {
        chain_id,
        multisig: multisig.to_owned(),
        safe_tx_hash: safe_tx_hash.to_owned(),
    })
}

/// Which link a damaged hash was meant as (`brokenLink`'s answer).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum BrokenLink {
    Proposal,
    Add,
}

/// ms-22: a hash meant as one of these links (`#ms=` / `#msa=`) that does not
/// read — cut short, or damaged on the way. Said, never ignored.
pub fn broken_link(hash: &str) -> Option<BrokenLink> {
    let body = link_body(hash);
    if body.starts_with(PREFIX) && parse_proposal_link(hash).is_none() {
        return Some(BrokenLink::Proposal);
    }
    if body.starts_with(ADD_PREFIX) && parse_add_link(hash).is_none() {
        return Some(BrokenLink::Add);
    }
    None
}

/// r4b-17: the hash to carry to the wallet when a multisig link opened another
/// page (`/en#msa=…`): the link itself (decoded as [`parse_proposal_link`]
/// reads it), or `""`.
pub fn carried_link_hash(hash: &str) -> String {
    let body = link_body(hash);
    if is_multisig_link(&body) {
        format!("#{body}")
    } else {
        String::new()
    }
}

/// Whether `hash` is meant as one of these links at all, readable or not.
pub fn is_multisig_link(hash: &str) -> bool {
    let body = link_body(hash);
    body.starts_with(PREFIX) || body.starts_with(ADD_PREFIX)
}

/// What a damaged link held, for the add sheet to show beside what is wrong
/// with it: the body after `ms=` / `msa=`, decoded when it decodes.
pub fn broken_link_text(hash: &str) -> String {
    let body = link_body(hash);
    let rest = body
        .strip_prefix(ADD_PREFIX)
        .or_else(|| body.strip_prefix(PREFIX))
        .unwrap_or(&body);
    decode_uri_component(rest).unwrap_or_else(|| rest.to_owned())
}

// ---------------------------------------------------------------------------
// EIP-55
// ---------------------------------------------------------------------------

/// `address` in its EIP-55 checksummed case, or as given when it is not
/// `0x` and 40 hex digits (`external/eip55.ts`).
pub fn eip55(address: &str) -> String {
    if !is_address(address) {
        return address.to_owned();
    }
    primitives::checksum_address(address).unwrap_or_else(|_| address.to_owned())
}

/// Whether `address` is written as an address is (r5reg-22): all one case, or
/// its EIP-55 checksum. Anything that is not `0x` and 40 hex digits holds
/// (the TypeScript's `eip55` hands it back unchanged) — callers check the
/// shape first.
pub fn checksum_holds(address: &str) -> bool {
    if !is_address(address) {
        return true;
    }
    primitives::address_entry(address) == primitives::AddressEntry::Valid
}

// ---------------------------------------------------------------------------
// The invite
// ---------------------------------------------------------------------------

/// The longest member label a link carries, in UTF-16 code units (the core's
/// name rule bounds it again).
pub const MEMBER_NAME_MAX: usize = 64;

/// A member's kind: a Vela account (a passkey), an outside wallet, or (r7)
/// a smart contract account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum MemberKind {
    Passkey,
    Wallet,
    /// r7: `c` — another Safe or a contract wallet; `p` and `w` read as
    /// they always did.
    Contract,
}

impl MemberKind {
    fn code(self) -> char {
        match self {
            MemberKind::Passkey => 'p',
            MemberKind::Wallet => 'w',
            MemberKind::Contract => 'c',
        }
    }
}

/// What the inviter knows of a member (`InvitedMember`): its address (absent
/// in a link from before they rode along), a label and its kind. Labels only:
/// who may act is the genesis's and each chain's owners.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct InvitedMember {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub address: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub kind: Option<MemberKind>,
}

/// An invite (`AddLink`, r4D-4, r4D-5).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct AddLink {
    /// As the link wrote it (all one case, or its checksum).
    pub multisig: String,
    /// Its networks, the first used first; empty in a link from before they
    /// rode along (the person is asked then).
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub chain_ids: Vec<u64>,
    /// Each member the inviter knows — by address, or (before r6) one per
    /// founding member in the genesis's order; `None` when the link carries
    /// none, or none that reads (labels are dropped, never guessed at).
    pub members: Option<Vec<InvitedMember>>,
}

/// One member as the link writes it: 40 lowercase hex (when addressed), the
/// kind's letter (`p`, `w`, `-`), then the label trimmed, cut to
/// [`MEMBER_NAME_MAX`] and URI-encoded. A cut that splits a surrogate pair is
/// refused, as `encodeURIComponent` throws on it.
fn member_code(member: &InvitedMember, with_address: bool) -> Result<String, CoreError> {
    let kind = member.kind.map_or('-', MemberKind::code);
    let name = js_trim(member.name.as_deref().unwrap_or(""));
    let name = js_head(name, MEMBER_NAME_MAX)
        .ok_or_else(|| err("URI malformed: a member's label is cut inside a character"))?;
    let at = match member.address.as_deref() {
        Some(address) if with_address && !address.is_empty() => js_tail(address, 2).to_lowercase(),
        _ => String::new(),
    };
    Ok(format!("{at}{kind}{}", encode_uri_component(name)))
}

/// `m=` (by the genesis's order) or, `addressed`, `a=` (each by its address).
/// One member that does not read drops them all.
fn read_members(raw: &str, addressed: bool) -> Option<Vec<InvitedMember>> {
    let mut out = Vec::new();
    for whole in raw.split(',') {
        let mut part = whole;
        let mut member = InvitedMember::default();
        if addressed {
            let hex = part.as_bytes().get(..40)?;
            if !hex
                .iter()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
            {
                return None;
            }
            member.address = Some(eip55(&format!("0x{}", &part[..40])));
            part = &part[40..];
        }
        let kind = match part.as_bytes().first() {
            Some(b'p') => Some(MemberKind::Passkey),
            Some(b'w') => Some(MemberKind::Wallet),
            Some(b'c') => Some(MemberKind::Contract),
            Some(b'-') => None,
            _ => return None,
        };
        let decoded = decode_uri_component(&part[1..])?;
        let name = js_trim(&decoded);
        if js_len(name) > MEMBER_NAME_MAX {
            return None;
        }
        if !name.is_empty() {
            member.name = Some(name.to_owned());
        }
        member.kind = kind;
        out.push(member);
    }
    Some(out)
}

/// The link that adds a multisig (`addLink`, r4D-4):
/// `<origin>/<locale>/wallet#msa=<chainId>,<chainId>:<multisig>&a=<member>,…`.
///
/// `chain_ids` are deduplicated, first kept, and only positive safe integers
/// ride. When every member has an address they ride as `a=` (address, kind,
/// label); otherwise, when any has a label or a kind, as `m=` in the order
/// given, without addresses; otherwise no members at all.
///
/// Errs only where the TypeScript throws: a label whose 64-unit cut splits a
/// surrogate pair.
pub fn add_link(
    origin: &str,
    locale: &str,
    multisig: &str,
    chain_ids: &[u64],
    members: &[InvitedMember],
) -> Result<String, CoreError> {
    let mut chains: Vec<u64> = Vec::new();
    for id in chain_ids {
        if *id > 0 && *id <= MAX_SAFE_INTEGER && !chains.contains(id) {
            chains.push(*id);
        }
    }
    let mut body = String::from(ADD_PREFIX);
    if !chains.is_empty() {
        let list: Vec<String> = chains.iter().map(u64::to_string).collect();
        body.push_str(&list.join(","));
        body.push(':');
    }
    body.push_str(multisig);
    let addressed = !members.is_empty()
        && members
            .iter()
            .all(|m| m.address.as_deref().is_some_and(|a| !a.is_empty()));
    let labelled = members
        .iter()
        .any(|m| m.name.as_deref().is_some_and(|n| !js_trim(n).is_empty()) || m.kind.is_some());
    if addressed || labelled {
        let codes = members
            .iter()
            .map(|m| member_code(m, addressed))
            .collect::<Result<Vec<_>, _>>()?;
        body.push('&');
        body.push_str(if addressed { ADDRESSED } else { MEMBERS });
        body.push_str(&codes.join(","));
    }
    Ok(format!("{origin}/{locale}/wallet#{body}"))
}

/// Why an invite does not read (`addLinkFault`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum AddLinkFault {
    /// Its address's letters were changed on the way (r5reg-22).
    Checksum,
    /// Part of it is missing or wrong.
    Damaged,
}

/// Why an invite does not read, or `None` when it does (or is no invite).
pub fn add_link_fault(hash: &str) -> Option<AddLinkFault> {
    let body = link_body(hash);
    if !body.starts_with(ADD_PREFIX) || parse_add_link(hash).is_some() {
        return None;
    }
    let head = body[ADD_PREFIX.len()..].split('&').next().unwrap_or("");
    let address = head.rfind(':').map_or(head, |at| &head[at + 1..]);
    if is_address(address) && !checksum_holds(address) {
        Some(AddLinkFault::Checksum)
    } else {
        Some(AddLinkFault::Damaged)
    }
}

/// The invite a location hash holds, or `None` (`parseAddLink`). A link from
/// before the networks rode along (`#msa=<multisig>`) reads, naming none.
/// Members that do not read are dropped; the multisig still opens. An address
/// whose letter case is not its checksum does not read.
pub fn parse_add_link(hash: &str) -> Option<AddLink> {
    let body = link_body(hash);
    let rest_all = body.strip_prefix(ADD_PREFIX)?;
    let mut parts = rest_all.split('&');
    let head = parts.next().unwrap_or("");
    let rest: Vec<&str> = parts.collect();
    let colon = head.rfind(':');
    let address = colon.map_or(head, |at| &head[at + 1..]);
    if !is_address(address) || !checksum_holds(address) {
        return None;
    }
    let mut chain_ids: Vec<u64> = Vec::new();
    if let Some(at) = colon {
        for id in head[..at].split(',') {
            let id = chain_id_of(id)?;
            if !chain_ids.contains(&id) {
                chain_ids.push(id);
            }
        }
    }
    let addressed = rest.iter().find(|p| p.starts_with(ADDRESSED));
    let listed = rest.iter().find(|p| p.starts_with(MEMBERS));
    let members = match (addressed, listed) {
        (Some(raw), _) => read_members(&raw[ADDRESSED.len()..], true),
        (None, Some(raw)) => read_members(&raw[MEMBERS.len()..], false),
        (None, None) => None,
    };
    Some(AddLink {
        multisig: address.to_owned(),
        chain_ids,
        members,
    })
}

// ---------------------------------------------------------------------------
// What an invite says of the members (`core/invite.ts`)
// ---------------------------------------------------------------------------

/// What this device knows of one member: the name it shows for it — only a
/// name, never the shortened address shown in its place — and its kind.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct MemberInfo {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "bindings", ts(optional))]
    pub kind: Option<MemberKind>,
}

/// The multisig an invite is for, as the shell's store has it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct InviteAccount {
    pub address: String,
    /// The founding members, in the genesis's order.
    pub owners: Vec<String>,
    /// Its networks, the first used first.
    #[cfg_attr(feature = "bindings", ts(type = "number[]"))]
    pub chain_ids: Vec<u64>,
}

/// What this device knows of each member, by address (`invitedMembers`):
/// the founding members (`owners`) in the genesis's order, then each of
/// `now` — the members now on a network the multisig is deployed on
/// (`membersAnywhere`) — not already among them (ignoring case). Each with
/// the name and kind `known` holds for it (keys matched ignoring case).
///
/// The shell fills `known` from its names: `names.label(owner)` only when it
/// is not `shortenAddress(owner)`, and `names.kindOf(owner)`.
pub fn invited_members(
    owners: &[String],
    now: &[String],
    known: &BTreeMap<String, MemberInfo>,
) -> Vec<InvitedMember> {
    let mut all: Vec<&String> = owners.iter().collect();
    for member in now {
        if !all.iter().any(|a| same_address(a, member)) {
            all.push(member);
        }
    }
    all.into_iter()
        .map(|owner| {
            let info = known
                .get(owner)
                .or_else(|| {
                    known
                        .iter()
                        .find(|(k, _)| same_address(k, owner))
                        .map(|(_, v)| v)
                })
                .cloned()
                .unwrap_or_default();
            InvitedMember {
                address: Some(owner.clone()),
                name: info.name,
                kind: info.kind,
            }
        })
        .collect()
}

/// The invite a multisig's members are sent (`inviteLink`): [`add_link`] with
/// its networks and [`invited_members`].
///
/// What stays in the shell (`keepInvited`, async, the store): when a device
/// adds the multisig from an invite, it keeps the invite's labels only for
/// members — an addressed member that is a founding owner at once; one that
/// is not, only once a network the invite names and the account is on reads
/// it as an owner now (a network that cannot be read keeps nothing of
/// those); a link from before r6 (no addresses) by the genesis's order, and
/// only when its count matches the owners'. Its own names stay over the
/// link's (`nameMembers`). Labels only, never who may act.
pub fn invite_link(
    origin: &str,
    locale: &str,
    account: &InviteAccount,
    now: &[String],
    known: &BTreeMap<String, MemberInfo>,
) -> Result<String, CoreError> {
    add_link(
        origin,
        locale,
        &account.address,
        &account.chain_ids,
        &invited_members(&account.owners, now, known),
    )
}

// ---------------------------------------------------------------------------
// A link kept through onboarding (`core/kept-link.ts`)
// ---------------------------------------------------------------------------

/// Where the shell keeps a link (`localStorage` on the web).
pub const KEPT_LINK_KEY: &str = "vela.multisigKeptLink";
/// How long a kept link waits for the wallet to be there: a day.
pub const KEPT_LINK_MS: f64 = 86_400_000.0;
/// Where the accounts machine keeps a proposal link waiting for a network
/// this device lacks (ms-22) — the same `{"hash":…,"at":…}` record, the
/// same day: a reload (the Add network wizard, another tab) loses nothing.
pub const PENDING_LINK_KEY: &str = "vela.multisigPendingLink";

/// The hash a proposal link is opened from (`#ms=<chainId>.<multisig>.<safeTxHash>`).
pub fn proposal_hash(link: &ProposalLink) -> String {
    format!(
        "#{PREFIX}{}.{}.{}",
        link.chain_id, link.multisig, link.safe_tx_hash
    )
}

/// The record to store under [`KEPT_LINK_KEY`] for `hash` at `now_ms`
/// (`keepLink`): `{"hash":…,"at":…}`, or `None` when `hash` is no multisig
/// link (`#ms=` / `#msa=`, readable or not) and nothing is kept.
pub fn keep_link(hash: &str, now_ms: f64) -> Option<String> {
    if !is_multisig_link(hash) {
        return None;
    }
    let at = if now_ms.is_finite() {
        ryu_js::Buffer::new().format_finite(now_ms).to_owned()
    } else {
        "null".to_owned()
    };
    let hash = serde_json::Value::String(hash.to_owned()).to_string();
    Some(format!("{{\"hash\":{hash},\"at\":{at}}}"))
}

/// The link a stored record holds at `now_ms` (`takeKeptLink`): `None` when
/// none is stored, it does not read, it waited longer than
/// [`KEPT_LINK_MS`], it is dated more than a minute ahead, or it is no
/// multisig link. Taken once: the shell removes the key after reading it
/// (the web does whenever the record parses).
pub fn take_kept_link(stored: Option<&str>, now_ms: f64) -> Option<String> {
    let kept: serde_json::Value = serde_json::from_str(stored.unwrap_or("null")).ok()?;
    let kept = kept.as_object()?;
    let hash = kept.get("hash")?.as_str()?;
    let at = kept.get("at")?.as_f64()?;
    if now_ms - at > KEPT_LINK_MS || now_ms < at - 60_000.0 {
        return None;
    }
    is_multisig_link(hash).then(|| hash.to_owned())
}

// ---------------------------------------------------------------------------
// The connect page's link (`external/connect-link.ts`)
// ---------------------------------------------------------------------------

const FOR: &str = "for=";
const TX: &str = "ms=";

/// `/^wss?:\/\/[^#]+#[A-Za-z0-9_-]{86}$/`: a PeerConnect room's invite.
fn is_invite(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("wss://").or_else(|| s.strip_prefix("ws://")) else {
        return false;
    };
    let Some((relay, secret)) = rest.split_once('#') else {
        return false;
    };
    !relay.is_empty()
        && secret.len() == 86
        && secret
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// What a connect page's fragment holds (`ConnectHash`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct ConnectHash {
    /// The room's invite, or `None` (a page reopened, or none that reads).
    pub invite: Option<String>,
    /// The members Vela asked for, lowercase; empty when the link named none.
    pub members: Vec<String>,
    /// The transaction to approve from the page itself; `None` for an older link.
    pub tx: Option<ProposalLink>,
    /// The link carried a room's invite that does not read — dropped, and said.
    pub damaged: bool,
}

/// `readTx`: the first three `.`-parts as a transaction, lowercased.
fn read_tx(body: &str) -> Option<ProposalLink> {
    let mut parts = body.split('.');
    let chain_id = chain_id_of(parts.next().unwrap_or(""))?;
    let multisig = parts.next().unwrap_or("");
    let safe_tx_hash = parts.next().unwrap_or("");
    if !is_address(multisig) || !is_hex_of(safe_tx_hash, 64) {
        return None;
    }
    Some(ProposalLink {
        chain_id,
        multisig: multisig.to_lowercase(),
        safe_tx_hash: safe_tx_hash.to_lowercase(),
    })
}

fn push_unique(list: &mut Vec<String>, item: String) {
    if !list.contains(&item) {
        list.push(item);
    }
}

/// The connect page to open (`connectLink`):
/// `<page>#<invite>&for=<member>,…&ms=<chainId>.<multisig>.<safeTxHash>` —
/// the invite URI-encoded (none for a room paired before, or an approve
/// link), the members that are addresses lowercased and deduplicated, and
/// the transaction lowercased when it reads (never half-written). `page`
/// alone when there is nothing to carry.
pub fn connect_link(
    page: &str,
    invite: Option<&str>,
    members: &[String],
    tx: Option<&ProposalLink>,
) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(invite) = invite.filter(|i| !i.is_empty()) {
        parts.push(encode_uri_component(invite));
    }
    let mut valid: Vec<String> = Vec::new();
    for member in members.iter().filter(|a| is_address(a)) {
        push_unique(&mut valid, member.to_lowercase());
    }
    if !valid.is_empty() {
        parts.push(format!("{FOR}{}", valid.join(",")));
    }
    if let Some(tx) = tx {
        if read_tx(&format!(
            "{}.{}.{}",
            tx.chain_id, tx.multisig, tx.safe_tx_hash
        ))
        .is_some()
        {
            parts.push(format!(
                "{TX}{}.{}.{}",
                tx.chain_id,
                tx.multisig.to_lowercase(),
                tx.safe_tx_hash.to_lowercase()
            ));
        }
    }
    if parts.is_empty() {
        page.to_owned()
    } else {
        format!("{page}#{}", parts.join("&"))
    }
}

/// What a connect page's fragment holds (`readConnectHash`). Anything that
/// does not read is dropped, never guessed; a later part of one kind wins.
pub fn read_connect_hash(hash: &str) -> ConnectHash {
    let raw = hash.strip_prefix('#').unwrap_or(hash);
    let mut invite: Option<String> = None;
    let mut members: Vec<String> = Vec::new();
    let mut tx: Option<ProposalLink> = None;
    let mut damaged = false;
    if !raw.is_empty() {
        for part in raw.split('&') {
            if let Some(list) = part.strip_prefix(FOR) {
                members = list
                    .split(',')
                    .filter(|a| is_address(a))
                    .map(str::to_lowercase)
                    .collect();
                continue;
            }
            if let Some(body) = part.strip_prefix(TX) {
                tx = read_tx(body);
                continue;
            }
            match decode_uri_component(part) {
                Some(decoded) => {
                    if is_invite(&decoded) {
                        invite = Some(decoded);
                    } else if decoded.starts_with("ws://") || decoded.starts_with("wss://") {
                        // A relay's address with a secret that does not read.
                        damaged = true;
                    }
                }
                None => {
                    // Not even a fragment that decodes: damaged when it was
                    // meant as the invite (`/^wss?(:|%3A)/i`).
                    let lower = part.to_ascii_lowercase();
                    let after = lower
                        .strip_prefix("wss")
                        .or_else(|| lower.strip_prefix("ws"));
                    if after.is_some_and(|a| a.starts_with(':') || a.starts_with("%3a")) {
                        damaged = true;
                    }
                }
            }
        }
    }
    let mut unique = Vec::new();
    for m in members {
        push_unique(&mut unique, m);
    }
    ConnectHash {
        damaged: damaged && invite.is_none(),
        invite,
        members: unique,
        tx,
    }
}

/// Whether `account` is one of `members` (lowercase) — any account, when the
/// link named none.
pub fn is_expected(members: &[String], account: &str) -> bool {
    members.is_empty() || members.contains(&account.to_lowercase())
}

/// The connect page's first frame (`app.html`'s `connect-boot` block): it
/// says "Reading the transaction…" at once when the page is `/connect` (or
/// `/connect/`), its fragment carries `ms=` as a part, and no room's invite
/// (`wss:` / `ws:`, `:` possibly encoded, any case).
pub fn connect_boot_reads_tx(pathname: &str, hash: &str) -> bool {
    let on_connect = pathname.ends_with("/connect") || pathname.ends_with("/connect/");
    let starts_part = |needle: &str, ignore_case: bool| {
        let at_start = |s: &str| {
            if ignore_case {
                starts_with_ignore_ascii_case(s, needle)
            } else {
                s.starts_with(needle)
            }
        };
        hash.strip_prefix('#').is_some_and(at_start)
            || hash
                .match_indices('&')
                .any(|(i, _)| at_start(&hash[i + 1..]))
    };
    let has_tx = starts_part("ms=", false);
    let has_invite = ["wss%3a", "wss:", "ws%3a", "ws:"]
        .iter()
        .any(|p| starts_part(p, true));
    on_connect && has_tx && !has_invite
}

// ---------------------------------------------------------------------------
// Activity row ids (`core/row-id.ts`)
// ---------------------------------------------------------------------------

/// What every multisig Activity row id starts with.
pub const ROW_PREFIX: &str = "multisig:";

/// A waiting transaction's row: `multisig:<chainId>:<safeTxHash>`, the hash
/// lowercased.
pub fn waiting_row_id(chain_id: u64, safe_tx_hash: &str) -> String {
    format!("{ROW_PREFIX}{chain_id}:{}", safe_tx_hash.to_lowercase())
}

/// The `index`-th row an executed transaction wrote:
/// `multisig:<chainId>:<safeTxHash>:<index>`.
pub fn executed_row_id(chain_id: u64, safe_tx_hash: &str, index: u64) -> String {
    format!("{}:{index}", waiting_row_id(chain_id, safe_tx_hash))
}

/// The transaction a row id names.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct RowTransaction {
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub chain_id: u64,
    pub safe_tx_hash: String,
    /// The row's index for an executed row; `None` for a waiting one.
    #[cfg_attr(feature = "bindings", ts(type = "number | null"))]
    pub index: Option<u64>,
}

/// A row written before the scheme, to migrate it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct LegacyRow {
    pub safe_tx_hash: String,
    #[cfg_attr(feature = "bindings", ts(type = "number"))]
    pub index: u64,
}

/// `/^(0|[1-9]\d{0,3})$/`.
fn row_index(s: &str) -> Option<u64> {
    if s == "0" || is_positive_decimal(s, 4) {
        s.parse().ok()
    } else {
        None
    }
}

/// The transaction a row id names, or `None` for anything that is not
/// exactly `multisig:<chainId>:<0x+64 hex>` with an optional `:<index>`.
pub fn transaction_of_row(id: Option<&str>) -> Option<RowTransaction> {
    let rest = id?.strip_prefix(ROW_PREFIX)?;
    let mut parts = rest.split(':');
    let chain = parts.next()?;
    let hash = parts.next()?;
    let index = parts.next();
    if parts.next().is_some() || !is_hex_of(hash, 64) {
        return None;
    }
    let chain_id = chain_id_of(chain)?;
    let index = match index {
        Some(i) => Some(row_index(i)?),
        None => None,
    };
    Some(RowTransaction {
        chain_id,
        safe_tx_hash: hash.to_lowercase(),
        index,
    })
}

/// A row from before the scheme (`multisig:<safeTxHash>:<index>`), or `None`.
pub fn legacy_row_id(id: &str) -> Option<LegacyRow> {
    let rest = id.strip_prefix(ROW_PREFIX)?;
    let (hash, index) = rest.split_once(':')?;
    if !is_hex_of(hash, 64) {
        return None;
    }
    Some(LegacyRow {
        safe_tx_hash: hash.to_lowercase(),
        index: row_index(index)?,
    })
}

// ---------------------------------------------------------------------------
// Amounts (`core/units.ts`)
// ---------------------------------------------------------------------------

/// `BigInt(text)` as sign and decimal magnitude (no leading zeros), or an
/// error where it throws. Arbitrary size, like `BigInt`.
fn big_int(text: &str) -> Result<(bool, String), CoreError> {
    let bad = || err(format!("Cannot convert {text} to a BigInt"));
    let t = js_trim(text);
    if t.is_empty() {
        return Ok((false, "0".to_owned()));
    }
    let b = t.as_bytes();
    let radix = match (b.first(), b.get(1)) {
        (Some(b'0'), Some(b'x' | b'X')) => 16,
        (Some(b'0'), Some(b'o' | b'O')) => 8,
        (Some(b'0'), Some(b'b' | b'B')) => 2,
        _ => 10,
    };
    if radix != 10 {
        let digits = &t[2..];
        if digits.is_empty() || !digits.chars().all(|c| c.is_digit(radix)) {
            return Err(bad());
        }
        return Ok((false, to_decimal(digits, radix)));
    }
    let (neg, digits) = match b[0] {
        b'-' => (true, &t[1..]),
        b'+' => (false, &t[1..]),
        _ => (false, t),
    };
    if digits.is_empty() || !digits.bytes().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    let mag = strip_leading_zeros(digits);
    Ok((neg && mag != "0", mag))
}

fn strip_leading_zeros(digits: &str) -> String {
    let s = digits.trim_start_matches('0');
    if s.is_empty() {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// Digits in `radix` as a decimal string, in base-10⁹ limbs.
fn to_decimal(digits: &str, radix: u32) -> String {
    const LIMB: u64 = 1_000_000_000;
    let mut limbs: Vec<u64> = vec![0];
    for c in digits.chars() {
        let mut carry = u64::from(c.to_digit(radix).unwrap_or(0));
        for limb in &mut limbs {
            let v = *limb * u64::from(radix) + carry;
            *limb = v % LIMB;
            carry = v / LIMB;
        }
        while carry > 0 {
            limbs.push(carry % LIMB);
            carry /= LIMB;
        }
    }
    let mut out = String::new();
    for (i, limb) in limbs.iter().rev().enumerate() {
        if i == 0 {
            out.push_str(&limb.to_string());
        } else {
            out.push_str(&format!("{limb:09}"));
        }
    }
    strip_leading_zeros(&out)
}

/// `value / 10^decimals` and `(value % 10^decimals).toString()`, as `BigInt`
/// has them: truncated toward zero, the remainder carrying the sign.
fn split_units(neg: bool, mag: &str, decimals: usize) -> (String, String) {
    let (whole, fraction) = if mag.len() > decimals {
        mag.split_at(mag.len() - decimals)
    } else {
        ("0", mag)
    };
    let whole = strip_leading_zeros(whole);
    let fraction = strip_leading_zeros(fraction);
    let signed = |s: String| if neg && s != "0" { format!("-{s}") } else { s };
    (signed(whole), signed(fraction))
}

fn pad_start(s: &str, len: usize) -> String {
    let n = js_len(s);
    if n >= len {
        s.to_owned()
    } else {
        format!("{}{s}", "0".repeat(len - n))
    }
}

fn group_thousands(whole: &str) -> String {
    let (sign, digits) = whole.strip_prefix('-').map_or(("", whole), |d| ("-", d));
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    format!("{sign}{out}")
}

/// An amount in base units as a person reads it (`formatUnits`): grouped,
/// at most six decimals, no trailing zero, and `<0.000001` for an amount too
/// small to show — never `0`. Errs where `BigInt(amount)` throws.
pub fn format_units(amount: &str, decimals: u32) -> Result<String, CoreError> {
    let (neg, mag) = big_int(amount)?;
    let (whole, fraction) = split_units(neg, &mag, decimals as usize);
    let fraction = pad_start(&fraction, decimals as usize);
    let six: String = fraction.chars().take(6).collect();
    let shown = six.trim_end_matches('0');
    if shown.is_empty() && whole == "0" && !neg && mag != "0" {
        return Ok("<0.000001".to_owned());
    }
    let whole = group_thousands(&whole);
    Ok(if shown.is_empty() {
        whole
    } else {
        format!("{whole}.{shown}")
    })
}

/// What a person typed, in base units (`parseUnits`), as a decimal string —
/// or `None` when it is not an amount (white space trimmed, `,` dropped) or
/// is finer than `decimals`.
pub fn parse_units(text: &str, decimals: u32) -> Option<String> {
    let clean: String = js_trim(text).chars().filter(|c| *c != ',').collect();
    let mut dots = 0;
    if !clean.chars().all(|c| {
        if c == '.' {
            dots += 1;
            dots == 1
        } else {
            c.is_ascii_digit()
        }
    }) || clean.is_empty()
        || clean == "."
    {
        return None;
    }
    let (whole, fraction) = clean.split_once('.').unwrap_or((&clean, ""));
    let decimals = decimals as usize;
    if fraction.len() > decimals {
        return None;
    }
    let whole = if whole.is_empty() { "0" } else { whole };
    Some(strip_leading_zeros(&format!(
        "{whole}{fraction}{}",
        "0".repeat(decimals - fraction.len())
    )))
}

/// Base units as a plain decimal string, every digit kept (`plainUnits`, the
/// records' `value`). Errs where `BigInt(amount)` throws.
pub fn plain_units(amount: &str, decimals: u32) -> Result<String, CoreError> {
    let (neg, mag) = big_int(amount)?;
    let (whole, fraction) = split_units(neg, &mag, decimals as usize);
    let fraction = pad_start(&fraction, decimals as usize);
    let fraction = fraction.trim_end_matches('0');
    Ok(if fraction.is_empty() {
        whole
    } else {
        format!("{whole}.{fraction}")
    })
}

// ---------------------------------------------------------------------------
// The approvals line (`core/progress.ts`)
// ---------------------------------------------------------------------------

/// `fillTemplate`: each `{{ name }}` with a value in `params` replaced by it;
/// an unknown one left as written, so a missing value shows.
fn fill_template(template: &str, params: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    while i < template.len() {
        let rest = &template[i..];
        if let Some((name, len)) = placeholder(rest) {
            match params.iter().find(|(k, _)| *k == name) {
                Some((_, v)) => out.push_str(v),
                None => out.push_str(&rest[..len]),
            }
            i += len;
        } else {
            let c = rest.chars().next().unwrap_or(' ');
            out.push(c);
            i += c.len_utf8();
        }
    }
    out
}

/// `/\{\{\s*(\w+)\s*\}\}/` at the start of `s`: the name and the match's length.
fn placeholder(s: &str) -> Option<(&str, usize)> {
    let inner = s.strip_prefix("{{")?;
    let after_space = inner.trim_start_matches(is_js_space);
    let name_len = after_space
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
        .count();
    if name_len == 0 {
        return None;
    }
    let name = &after_space[..name_len];
    let tail = after_space[name_len..].trim_start_matches(is_js_space);
    let tail = tail.strip_prefix("}}")?;
    Some((name, s.len() - tail.len()))
}

/// "2/2 approved", never "3/2" (r4b-17, `progressLine`): `template` (the
/// locale's `status.progress`, `{{approved}}` / `{{threshold}}`) filled with
/// the approvals that count — up to the threshold — and the threshold.
pub fn progress_line(template: &str, approved: u64, threshold: u64) -> String {
    fill_template(
        template,
        &[
            ("approved", approved.min(threshold).to_string()),
            ("threshold", threshold.to_string()),
        ],
    )
}

// ---------------------------------------------------------------------------
// The JSON surface
// ---------------------------------------------------------------------------

/// The chain ids a TypeScript caller passes (`number[]`): only positive safe
/// integers ride (`addLink` drops the rest).
fn safe_chain_ids(ids: &[f64]) -> Vec<u64> {
    ids.iter()
        .filter(|id| id.fract() == 0.0 && **id > 0.0 && **id <= MAX_SAFE_INTEGER as f64)
        .map(|id| *id as u64)
        .collect()
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct AddLinkExtra {
    #[serde(default)]
    chain_ids: Option<Vec<f64>>,
    #[serde(default)]
    members: Option<Vec<InvitedMember>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InviteAccountArg {
    address: String,
    owners: Vec<String>,
    #[serde(default)]
    chain_ids: Vec<f64>,
}

/// One call by name, JSON in and out — the surface a wasm or uniffi shell
/// calls. Method names are the TypeScript functions'; `args_json` is an
/// object of their arguments by name; the answer is the function's value as
/// JSON (`null` where the TypeScript answers `null`).
///
/// Methods: `shareOrigin`, `proposalLink`, `parseProposalLink`,
/// `brokenLink`, `carriedLinkHash`, `isMultisigLink`, `brokenLinkText`,
/// `eip55`, `checksumHolds`, `memberNameMax`, `addLink`, `addLinkFault`,
/// `parseAddLink`, `invitedMembers`, `inviteLink`, `keepLink`,
/// `takeKeptLink`, `connectLink`, `readConnectHash` (also
/// `parseConnectLink`), `isExpected`, `connectBootReadsTx`, `waitingRowId`,
/// `executedRowId`, `transactionOfRow`, `legacyRowId`, `formatUnits`,
/// `parseUnits`, `plainUnits`, `progressLine`.
pub fn call_json(method: &str, args_json: &str) -> Result<String, CoreError> {
    let args: serde_json::Value =
        serde_json::from_str(args_json).map_err(|e| err(format!("arguments: {e}")))?;
    fn arg<T: serde::de::DeserializeOwned>(
        args: &serde_json::Value,
        name: &str,
    ) -> Result<T, CoreError> {
        serde_json::from_value(args.get(name).cloned().unwrap_or(serde_json::Value::Null))
            .map_err(|e| err(format!("argument {name}: {e}")))
    }
    fn s(args: &serde_json::Value, name: &str) -> Result<String, CoreError> {
        arg::<String>(args, name)
    }
    /// A `bigint` or `string` amount: a JSON string, or an integer.
    fn amount(args: &serde_json::Value) -> Result<String, CoreError> {
        match args.get("amount") {
            Some(serde_json::Value::String(text)) => Ok(text.clone()),
            Some(serde_json::Value::Number(n)) if n.is_i64() || n.is_u64() => Ok(n.to_string()),
            _ => Err(err("argument amount: a string or an integer")),
        }
    }
    fn to_json<T: Serialize>(value: &T) -> Result<String, CoreError> {
        serde_json::to_string(value).map_err(|e| err(format!("answer: {e}")))
    }
    match method {
        "shareOrigin" => to_json(&share_origin(&s(&args, "origin")?)),
        "proposalLink" => to_json(&proposal_link(
            &s(&args, "origin")?,
            &s(&args, "locale")?,
            &arg(&args, "link")?,
        )),
        "parseProposalLink" => to_json(&parse_proposal_link(&s(&args, "hash")?)),
        "brokenLink" => to_json(&broken_link(&s(&args, "hash")?)),
        "carriedLinkHash" => to_json(&carried_link_hash(&s(&args, "hash")?)),
        "isMultisigLink" => to_json(&is_multisig_link(&s(&args, "hash")?)),
        "brokenLinkText" => to_json(&broken_link_text(&s(&args, "hash")?)),
        "eip55" => to_json(&eip55(&s(&args, "address")?)),
        "checksumHolds" => to_json(&checksum_holds(&s(&args, "address")?)),
        "memberNameMax" => to_json(&MEMBER_NAME_MAX),
        "addLink" => {
            let extra = arg::<Option<AddLinkExtra>>(&args, "extra")?.unwrap_or_default();
            to_json(&add_link(
                &s(&args, "origin")?,
                &s(&args, "locale")?,
                &s(&args, "multisig")?,
                &safe_chain_ids(&extra.chain_ids.unwrap_or_default()),
                &extra.members.unwrap_or_default(),
            )?)
        }
        "addLinkFault" => to_json(&add_link_fault(&s(&args, "hash")?)),
        "parseAddLink" => to_json(&parse_add_link(&s(&args, "hash")?)),
        "invitedMembers" => to_json(&invited_members(
            &arg::<Vec<String>>(&args, "owners")?,
            &arg::<Option<Vec<String>>>(&args, "now")?.unwrap_or_default(),
            &arg::<Option<BTreeMap<String, MemberInfo>>>(&args, "known")?.unwrap_or_default(),
        )),
        "inviteLink" => {
            let account = arg::<InviteAccountArg>(&args, "account")?;
            to_json(&invite_link(
                &s(&args, "origin")?,
                &s(&args, "locale")?,
                &InviteAccount {
                    address: account.address,
                    owners: account.owners,
                    chain_ids: safe_chain_ids(&account.chain_ids),
                },
                &arg::<Option<Vec<String>>>(&args, "now")?.unwrap_or_default(),
                &arg::<Option<BTreeMap<String, MemberInfo>>>(&args, "known")?.unwrap_or_default(),
            )?)
        }
        "keepLink" => to_json(&keep_link(&s(&args, "hash")?, arg(&args, "now")?)),
        "takeKeptLink" => to_json(&take_kept_link(
            arg::<Option<String>>(&args, "stored")?.as_deref(),
            arg(&args, "now")?,
        )),
        "connectLink" => to_json(&connect_link(
            &s(&args, "page")?,
            arg::<Option<String>>(&args, "invite")?.as_deref(),
            &arg::<Vec<String>>(&args, "members")?,
            arg::<Option<ProposalLink>>(&args, "tx")?.as_ref(),
        )),
        "readConnectHash" | "parseConnectLink" => to_json(&read_connect_hash(&s(&args, "hash")?)),
        "isExpected" => to_json(&is_expected(
            &arg::<Vec<String>>(&args, "members")?,
            &s(&args, "account")?,
        )),
        "connectBootReadsTx" => to_json(&connect_boot_reads_tx(
            &s(&args, "pathname")?,
            &arg::<Option<String>>(&args, "hash")?.unwrap_or_default(),
        )),
        "waitingRowId" => to_json(&waiting_row_id(
            arg(&args, "chainId")?,
            &s(&args, "safeTxHash")?,
        )),
        "executedRowId" => to_json(&executed_row_id(
            arg(&args, "chainId")?,
            &s(&args, "safeTxHash")?,
            arg(&args, "index")?,
        )),
        "transactionOfRow" => to_json(&transaction_of_row(
            arg::<Option<String>>(&args, "id")?
                .as_deref()
                .filter(|id| !id.is_empty()),
        )),
        "legacyRowId" => to_json(&legacy_row_id(&s(&args, "id")?)),
        "formatUnits" => to_json(&format_units(&amount(&args)?, arg(&args, "decimals")?)?),
        "parseUnits" => to_json(&parse_units(&s(&args, "text")?, arg(&args, "decimals")?)),
        "plainUnits" => to_json(&plain_units(&amount(&args)?, arg(&args, "decimals")?)?),
        "progressLine" => to_json(&progress_line(
            &s(&args, "template")?,
            arg(&args, "approved")?,
            arg(&args, "threshold")?,
        )),
        other => match crate::multisig_rules::call_json(other, &args) {
            Some(answer) => answer,
            None => Err(err(format!("unknown multisig link method: {other}"))),
        },
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    const SAFE: &str = "0x0cb9aEF9713c4a2eA2995FCa68d7355Ce5E27d83";
    const HASH: &str = "0xed2132b0c8f16940fb0fa92a8fbaed407bed1ed264f61bd9c38e84d56ed0d8aa";

    /// `new URL(link)`'s parts, for the links made here (no `?`, nothing the
    /// URL parser re-encodes in a fragment).
    fn url_parts(link: &str) -> (String, String, String) {
        let (before, hash) = link.split_once('#').map_or((link, ""), |(b, h)| (b, h));
        let after_scheme = before.split_once("://").unwrap().1;
        let path = after_scheme.find('/').map_or("/", |at| &after_scheme[at..]);
        let search = path.find('?').map_or("", |at| &path[at..]);
        let hash = if hash.is_empty() {
            String::new()
        } else {
            format!("#{hash}")
        };
        (path.to_owned(), search.to_owned(), hash)
    }

    #[test]
    fn js_trim_is_the_ecmascript_set() {
        assert_eq!(js_trim("\u{feff} x \u{3000}"), "x");
        // U+0085 is Rust white space, not ECMAScript's.
        assert_eq!(js_trim("\u{85}x"), "\u{85}x");
    }

    #[test]
    fn decode_uri_component_refuses_as_ecmascript_does() {
        assert_eq!(decode_uri_component("a%20b").as_deref(), Some("a b"));
        assert_eq!(decode_uri_component("%E0%A4%95").as_deref(), Some("क"));
        assert_eq!(decode_uri_component("%E0%A4%A"), None);
        assert_eq!(decode_uri_component("%"), None);
        assert_eq!(decode_uri_component("%zz"), None);
        // A surrogate and an overlong form.
        assert_eq!(decode_uri_component("%ED%A0%80"), None);
        assert_eq!(decode_uri_component("%C0%80"), None);
        assert_eq!(decode_uri_component("%2C"), Some(",".to_owned()));
    }

    #[test]
    fn encode_uri_component_keeps_the_unreserved_set() {
        assert_eq!(
            encode_uri_component("Bob, the treasurer!~*'()-_."),
            "Bob%2C%20the%20treasurer!~*'()-_."
        );
        assert_eq!(encode_uri_component("é"), "%C3%A9");
        assert_eq!(encode_uri_component("wss://r/a#b"), "wss%3A%2F%2Fr%2Fa%23b");
    }

    #[test]
    fn the_links_made_here_carry_everything_after_the_hash() {
        let link = add_link(
            "https://wallet.getvela.app",
            "en",
            SAFE,
            &[5_042_002, 100, 5_042_002],
            &[
                InvitedMember {
                    name: Some("Alice".into()),
                    kind: Some(MemberKind::Passkey),
                    ..Default::default()
                },
                InvitedMember {
                    name: Some("Bob, the treasurer".into()),
                    kind: Some(MemberKind::Wallet),
                    ..Default::default()
                },
                InvitedMember::default(),
            ],
        )
        .unwrap();
        let (path, search, hash) = url_parts(&link);
        assert_eq!(search, "");
        assert_eq!(path, "/en/wallet");
        assert_eq!(
            hash,
            format!("#msa=5042002,100:{SAFE}&m=pAlice,wBob%2C%20the%20treasurer,-")
        );
        let proposal = proposal_link(
            "https://wallet.getvela.app",
            "zh",
            &ProposalLink {
                chain_id: 100,
                multisig: SAFE.into(),
                safe_tx_hash: HASH.into(),
            },
        );
        let (_, _, hash) = url_parts(&proposal);
        assert_eq!(parse_proposal_link(&hash).unwrap().chain_id, 100);
    }

    #[test]
    fn a_label_cut_inside_a_character_is_refused_as_the_web_throws() {
        // 63 units, then a pair: the 64-unit cut lands inside it.
        let name = format!("{}😀", "a".repeat(63));
        let refused = add_link(
            "https://x",
            "en",
            SAFE,
            &[],
            &[InvitedMember {
                name: Some(name),
                ..Default::default()
            }],
        );
        assert!(refused.is_err());
        // 62 + the pair = 64 units: whole.
        let name = format!("{}😀", "a".repeat(62));
        let link = add_link(
            "https://x",
            "en",
            SAFE,
            &[],
            &[InvitedMember {
                name: Some(name.clone()),
                ..Default::default()
            }],
        )
        .unwrap();
        let read = parse_add_link(&url_parts(&link).2).unwrap();
        assert_eq!(
            read.members.unwrap()[0].name.as_deref(),
            Some(name.as_str())
        );
    }

    #[test]
    fn the_invite_reads_back_the_members_it_was_made_with() {
        const ISAFE: &str = "0x710311c8d71CC405c5B7782BE1367eAFA3d78937";
        const ALICE: &str = "0xe331F2Ef7A6f4ED426529c8C10642b68084771A2";
        const CAROL: &str = "0xa1D99c2066BF6C4664981529FfB788c1bc5104F1";
        const EVE: &str = "0x34a0f6Ed36F63cc20B462803eBE13FF5C12c1e19";
        const BOB: &str = "0x412f2165f299Dfc2363ED08F8Fc6DE4353E81fF6";
        let known: BTreeMap<String, MemberInfo> = [
            (ALICE, "Alice", MemberKind::Passkey),
            (BOB, "R5 Bob", MemberKind::Passkey),
            (EVE, "Eve MetaMask", MemberKind::Wallet),
        ]
        .into_iter()
        .map(|(a, n, k)| {
            (
                a.to_lowercase(),
                MemberInfo {
                    name: Some(n.into()),
                    kind: Some(k),
                },
            )
        })
        .collect();
        let owners: Vec<String> = [ALICE, CAROL, EVE].map(String::from).to_vec();
        let now: Vec<String> = [ALICE, CAROL, EVE, BOB].map(String::from).to_vec();
        let link = invite_link(
            "https://x",
            "en",
            &InviteAccount {
                address: ISAFE.into(),
                owners,
                chain_ids: vec![5_042_002],
            },
            &now,
            &known,
        )
        .unwrap();
        let members = parse_add_link(&url_parts(&link).2)
            .unwrap()
            .members
            .unwrap();
        assert_eq!(
            members[3],
            InvitedMember {
                address: Some(BOB.into()),
                name: Some("R5 Bob".into()),
                kind: Some(MemberKind::Passkey),
            }
        );
        assert_eq!(members[1].address.as_deref(), Some(CAROL));
        assert_eq!(members[1].name, None);
    }

    #[test]
    fn a_kept_link_is_taken_once_by_the_shell() {
        let invite = format!("#msa=5042002:{SAFE}&m=pAlice,wBob");
        let stored = keep_link(&invite, 1_000.0).unwrap();
        assert_eq!(stored, format!("{{\"hash\":\"{invite}\",\"at\":1000}}"));
        assert_eq!(take_kept_link(Some(&stored), 2_000.0), Some(invite));
        // The shell removed it: nothing the second time.
        assert_eq!(take_kept_link(None, 3_000.0), None);
        // Dated ahead by more than a minute: not trusted.
        assert_eq!(take_kept_link(Some(&stored), 1_000.0 - 60_001.0), None);
        assert_eq!(take_kept_link(Some("{not json"), 2_000.0), None);
        assert_eq!(take_kept_link(Some("[1]"), 2_000.0), None);
        assert_eq!(
            take_kept_link(Some("{\"hash\":\"#flow=x\",\"at\":1}"), 2.0),
            None
        );
    }

    #[test]
    fn the_connect_page_first_frame_reads_links_as_connect_link_makes_them() {
        let tx = ProposalLink {
            chain_id: 5_042_002,
            multisig: SAFE.into(),
            safe_tx_hash: format!("0x{}", "ab".repeat(32)),
        };
        let member = vec!["0x70997970C51812dc3A010C7d01b50e0d17dc79C8".to_owned()];
        let invite = format!("wss://relay.peerconnect.dev/#{}", "A".repeat(86));
        let page = "https://getvela.app/en/connect";
        let boot = |link: String| {
            let (path, _, hash) = url_parts(&link);
            connect_boot_reads_tx(&path, &hash)
        };
        assert!(boot(connect_link(page, None, &member, Some(&tx))));
        assert!(boot(connect_link(
            &format!("{page}/"),
            None,
            &[],
            Some(&tx)
        )));
        assert!(!boot(connect_link(page, Some(&invite), &member, Some(&tx))));
        assert!(!boot(connect_link(page, Some(&invite), &member, None)));
        assert!(!boot(connect_link(page, None, &member, None)));
        assert!(!boot(connect_link(
            "https://getvela.app/en/wallet",
            None,
            &[],
            Some(&tx)
        )));
    }

    #[test]
    fn big_int_reads_what_bigint_reads() {
        assert_eq!(big_int(" 0x10 ").unwrap(), (false, "16".to_owned()));
        assert_eq!(big_int("0b101").unwrap(), (false, "5".to_owned()));
        assert_eq!(big_int("0o17").unwrap(), (false, "15".to_owned()));
        assert_eq!(big_int("").unwrap(), (false, "0".to_owned()));
        assert_eq!(big_int("-0").unwrap(), (false, "0".to_owned()));
        assert_eq!(big_int("+007").unwrap(), (false, "7".to_owned()));
        assert!(big_int("-0x5").is_err());
        assert!(big_int("1.5").is_err());
        assert!(big_int("0x").is_err());
        assert!(big_int("1e3").is_err());
        assert_eq!(
            to_decimal(
                "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff",
                16
            ),
            "115792089237316195423570985008687907853269984665640564039457584007913129639935"
        );
    }

    #[test]
    fn the_approvals_line_counts_up_to_the_threshold() {
        let t = "{{approved}}/{{threshold}} approved";
        assert_eq!(progress_line(t, 3, 2), "2/2 approved");
        assert_eq!(progress_line(t, 1, 2), "1/2 approved");
        assert_eq!(
            progress_line("{{ approved }} of {{threshold}}", 0, 3),
            "0 of 3"
        );
        // An unknown placeholder stays, so a missing value shows.
        assert_eq!(progress_line("{{approved}} {{who}}", 1, 1), "1 {{who}}");
        assert_eq!(
            progress_line("{{}} {{a b}} {approved}", 1, 1),
            "{{}} {{a b}} {approved}"
        );
        assert_eq!(progress_line("{{{approved}}}", 1, 1), "{1}");
    }

    #[test]
    fn call_json_refuses_an_unknown_method_and_bad_arguments() {
        assert!(call_json("nope", "{}").is_err());
        assert!(call_json("parseProposalLink", "{}").is_err());
        assert!(call_json("parseProposalLink", "not json").is_err());
    }
}
