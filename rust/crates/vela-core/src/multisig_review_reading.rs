//! What each call of a multisig transaction does, read by the core alone
//! (r8-1, r8-2, r8-5) — the part of the one review a signer reads first:
//! "Send 0.01 USDC to …", "Approve 0x… to spend USDC · Unlimited", every leg
//! of every batch, nested ones included.
//!
//! The same reading on every platform, from the SafeTx's bytes and the token
//! facts the core read itself (the wallet's registry, else the contract's own
//! `symbol()` and `decimals()`): nothing here depends on a shell's own
//! decoding. A shell draws [`Reading`]; the review's concerns
//! ([`super::Concern`]) and its sign rule say what must be said and how hard
//! the confirm is.
//!
//! The rules:
//!
//! - **Decimals are never assumed.** A token whose decimals are not known
//!   says its amount raw (`Amount::decimals` `None`, `plain` `None`) and the
//!   review says "decimals unknown"; a friendly amount is never invented.
//! - **An unlimited grant is a danger** (`approve`, `increaseAllowance`,
//!   Permit2 `approve` at or past the guard's line — `approval_guard`'s, the
//!   same one the signing sheet draws — an on-chain EIP-2612 or DAI `permit`,
//!   `setApprovalForAll(…, true)`): the confirm says "… anyway".
//! - **A batch inside a batch** is read leg by leg, every level, down to
//!   [`MAX_NESTING`]; it asks for "… anyway". Deeper than that, nothing is
//!   read and nothing is signed.
//! - **A call the core cannot read** ([`Intent::Call`]) asks for "… anyway"
//!   (the Bybit lesson: no blind signing without the friction).

use std::collections::BTreeMap;

use alloy_primitives::{keccak256, Address, U256};
use serde::{Deserialize, Serialize};

use super::Concern;
use crate::app::approval_guard::{
    detect_calldata_approval, is_unbounded_amount, AmountBits, GuardApprovalKind,
};
use crate::app::multisig_outside::reading::TokenFacts;
use crate::multisig::{Action, Review, SafeTx};

/// The deepest batch inside a batch that is read (the outer batch is level
/// 1). A batch nested deeper is not read: nothing is signed.
pub const MAX_NESTING: u32 = 3;
/// The most calls read in all (a hostile batch of thousands of legs).
pub const MAX_CALLS: usize = 64;

/// An amount of a token: base units, and what can honestly be said of it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    /// Base units, decimal — always shown when `decimals` is unknown.
    pub raw: String,
    /// The token's decimals; `None`: unknown — say `raw` and "decimals
    /// unknown", never a friendly amount.
    pub decimals: Option<u32>,
    /// The exact amount in whole tokens (`"0.01"`, no grouping, nothing cut)
    /// when the decimals are known and it is not unlimited; a shell groups
    /// and shortens it in its own words.
    pub plain: Option<String>,
    /// At or past the line where an allowance is unlimited.
    pub unlimited: bool,
}

/// How an allowance is granted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Grant {
    /// `approve(spender, amount)`: the allowance becomes `amount`.
    Approve,
    /// `increaseAllowance`: `amount` is ADDED to what it is now.
    Increase,
    /// `decreaseAllowance`: it shrinks.
    Decrease,
    /// An on-chain EIP-2612 `permit(owner, spender, value, …)`.
    Permit,
    /// DAI's `permit(holder, spender, nonce, expiry, allowed, …)`: all of
    /// the holder's DAI, or none.
    DaiPermit,
    /// Permit2's `approve(token, spender, amount, expiration)`.
    Permit2,
}

/// What one call does.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum Intent {
    /// The network's coin (`token.token` `None`) or an ERC-20 `transfer`.
    Send {
        token: TokenFacts,
        to: String,
        amount: Amount,
    },
    /// `transferFrom(from, to, amount)`: tokens someone allowed the
    /// multisig to move.
    SendFrom {
        token: TokenFacts,
        from: String,
        to: String,
        amount: Amount,
    },
    /// An allowance: `spender` may then move the token from the multisig.
    Approve {
        token: TokenFacts,
        spender: String,
        grant: Grant,
        amount: Amount,
        /// Unix seconds, when the grant carries one.
        expires: Option<String>,
    },
    /// `setApprovalForAll(operator, granted)`: every token of a collection.
    ApproveAll {
        collection: String,
        operator: String,
        granted: bool,
    },
    /// A batch inside this one (`multiSend` called, not delegatecalled): its
    /// legs run from `runner` — the batch contract, not the multisig.
    Batch { runner: String, calls: Vec<Call> },
    /// A batch nested past [`MAX_NESTING`], or one whose bytes do not read:
    /// nothing is signed.
    Unreadable,
    /// A change to the multisig itself, in a batch (the review's
    /// `memberChanges` says it in full).
    Members { action: Action },
    /// The multisig renames its record in the genesis registry.
    Rename { name: String },
    /// A call the core cannot read: what it calls, with which function
    /// selector (`None`: no data).
    Call { selector: Option<String> },
}

/// One call, read.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Call {
    /// Where it is from the top, each a leg number from 1: `[]` the
    /// transaction's one call, `[2]` the second leg, `[2, 1]` the first leg
    /// of the batch that leg is.
    pub path: Vec<u32>,
    /// The leg's number as every shell says it (r10-10): the path joined by
    /// `.` (`"2.1"`), `None` for the transaction's one call (`path` empty) —
    /// then no number at all, never `review.callNumber` with an empty `n`.
    #[serde(default)]
    pub leg: Option<String>,
    /// The contract (or account) called.
    pub to: String,
    /// The coin sent along with it (a `send` of the coin says it there).
    pub value: Amount,
    pub intent: Intent,
}

/// The transaction's calls, read ([`super::MultisigReview::reading`]).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    /// Its one call, or each leg of its batch; empty for what the review
    /// reads as the multisig's own (a member change, a rename, a
    /// cancellation) or refuses.
    pub calls: Vec<Call>,
    /// A batch is inside a batch somewhere.
    pub nested: bool,
    /// Something could not be read (too deep, cut short, too many legs):
    /// nothing is signed.
    pub unreadable: bool,
    /// Some call grants an unlimited allowance (or all of a collection).
    pub unlimited: bool,
    /// Some call the core cannot read.
    pub blind: bool,
    /// Every token named whose decimals are not known (lowercase).
    pub decimals_unknown: Vec<String>,
}

impl Reading {
    /// Every call at every level, depth first.
    pub fn every(&self) -> Vec<&Call> {
        fn walk<'a>(calls: &'a [Call], out: &mut Vec<&'a Call>) {
            for c in calls {
                out.push(c);
                if let Intent::Batch { calls, .. } = &c.intent {
                    walk(calls, out);
                }
            }
        }
        let mut out = Vec::new();
        walk(&self.calls, &mut out);
        out
    }
}

// ---------------------------------------------------------------------------

fn sel(signature: &str) -> [u8; 4] {
    let h = keccak256(signature.as_bytes());
    [h[0], h[1], h[2], h[3]]
}

fn checksum(a: Address) -> String {
    a.to_checksum(None)
}

fn word(data: &[u8], i: usize) -> Option<U256> {
    let at = 4usize.checked_add(i.checked_mul(32)?)?;
    data.get(at..at.checked_add(32)?).map(U256::from_be_slice)
}

/// A word that holds an address (its high 12 bytes zero).
fn word_address(data: &[u8], i: usize) -> Option<Address> {
    let at = 4 + i * 32;
    let w = data.get(at..at + 32)?;
    if w[..12].iter().any(|b| *b != 0) {
        return None;
    }
    Some(Address::from_slice(&w[12..]))
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    crate::primitives::from_hex(s.trim()).ok()
}

/// `raw` base units in whole tokens, exactly: `10000` at 6 → `"0.01"`.
pub fn plain_units(raw: U256, decimals: u32) -> String {
    let digits = raw.to_string();
    let d = decimals as usize;
    if d == 0 {
        return digits;
    }
    let (whole, frac) = if digits.len() > d {
        let (w, f) = digits.split_at(digits.len() - d);
        (w.to_owned(), f.to_owned())
    } else {
        (
            "0".to_owned(),
            format!("{}{digits}", "0".repeat(d - digits.len())),
        )
    };
    let frac = frac.trim_end_matches('0');
    if frac.is_empty() {
        whole
    } else {
        format!("{whole}.{frac}")
    }
}

/// `raw` of `token` as said: with its decimals if known, never assumed.
pub fn amount(raw: U256, token: &TokenFacts, unlimited: bool) -> Amount {
    Amount {
        raw: raw.to_string(),
        decimals: token.decimals,
        plain: match (unlimited, token.decimals) {
            (false, Some(d)) => Some(plain_units(raw, d)),
            _ => None,
        },
        unlimited,
    }
}

/// What reads the calls: the chain, and the token facts read.
pub struct Reader<'a> {
    pub chain_id: u64,
    pub tokens: &'a BTreeMap<String, TokenFacts>,
    native: TokenFacts,
    count: usize,
}

impl<'a> Reader<'a> {
    pub fn new(chain_id: u64, tokens: &'a BTreeMap<String, TokenFacts>) -> Self {
        Self {
            chain_id,
            tokens,
            native: TokenFacts::of(chain_id, None, tokens),
            count: 0,
        }
    }

    fn token(&self, a: Address) -> TokenFacts {
        TokenFacts::of(self.chain_id, Some(&format!("{a:#x}")), self.tokens)
    }

    fn coin(&self, v: U256) -> Amount {
        amount(v, &self.native, false)
    }

    /// One call to `to` with `value` and `data`, at `path`, `depth` batches
    /// deep (0 for the transaction's own calls).
    pub fn call(
        &mut self,
        path: Vec<u32>,
        to: Address,
        value: U256,
        data: &[u8],
        depth: u32,
    ) -> Call {
        self.count += 1;
        let intent = self.intent(&path, to, value, data, depth);
        Call {
            leg: leg_number(&path),
            path,
            to: checksum(to),
            value: self.coin(value),
            intent,
        }
    }

    fn intent(
        &mut self,
        path: &[u32],
        to: Address,
        value: U256,
        data: &[u8],
        depth: u32,
    ) -> Intent {
        if data.is_empty() {
            return Intent::Send {
                token: self.native.clone(),
                to: checksum(to),
                amount: self.coin(value),
            };
        }
        let s = data.get(..4).unwrap_or_default();
        if s == sel("transfer(address,uint256)") && data.len() == 68 {
            if let (Some(r), Some(n)) = (word_address(data, 0), word(data, 1)) {
                let token = self.token(to);
                return Intent::Send {
                    amount: amount(n, &token, false),
                    token,
                    to: checksum(r),
                };
            }
        }
        if s == sel("transferFrom(address,address,uint256)") && data.len() == 100 {
            if let (Some(f), Some(r), Some(n)) =
                (word_address(data, 0), word_address(data, 1), word(data, 2))
            {
                let token = self.token(to);
                return Intent::SendFrom {
                    amount: amount(n, &token, false),
                    token,
                    from: checksum(f),
                    to: checksum(r),
                };
            }
        }
        if s == sel("permit(address,address,uint256,uint256,uint8,bytes32,bytes32)")
            && data.len() == 4 + 7 * 32
        {
            if let (Some(_), Some(sp), Some(n), Some(dl)) = (
                word_address(data, 0),
                word_address(data, 1),
                word(data, 2),
                word(data, 3),
            ) {
                let token = self.token(to);
                let unlimited = is_unbounded_amount(n, AmountBits::B256);
                return Intent::Approve {
                    amount: amount(n, &token, unlimited),
                    token,
                    spender: checksum(sp),
                    grant: Grant::Permit,
                    expires: Some(dl.to_string()),
                };
            }
        }
        if s == sel("permit(address,address,uint256,uint256,bool,uint8,bytes32,bytes32)")
            && data.len() == 4 + 8 * 32
        {
            if let (Some(_), Some(sp), Some(exp), Some(allowed)) = (
                word_address(data, 0),
                word_address(data, 1),
                word(data, 3),
                word(data, 4),
            ) {
                let token = self.token(to);
                let granted = !allowed.is_zero();
                return Intent::Approve {
                    amount: Amount {
                        raw: if granted {
                            U256::MAX.to_string()
                        } else {
                            "0".into()
                        },
                        decimals: token.decimals,
                        plain: (!granted).then(|| "0".into()),
                        unlimited: granted,
                    },
                    token,
                    spender: checksum(sp),
                    grant: Grant::DaiPermit,
                    expires: Some(exp.to_string()),
                };
            }
        }
        if s == sel("multiSend(bytes)") {
            return match self.batch(path, data, depth + 1) {
                Some(calls) => Intent::Batch {
                    runner: checksum(to),
                    calls,
                },
                None => Intent::Unreadable,
            };
        }
        let hex = crate::primitives::to_hex(data, true);
        if let Some(found) = detect_calldata_approval(Some(&format!("{to:#x}")), Some(&hex)) {
            let spender = found
                .spender
                .parse::<Address>()
                .map(checksum)
                .unwrap_or(found.spender.clone());
            if found.kind == GuardApprovalKind::SetApprovalForAll {
                return Intent::ApproveAll {
                    collection: checksum(to),
                    operator: spender,
                    granted: found.is_unbounded,
                };
            }
            let token_at = found
                .token_address
                .as_deref()
                .and_then(|t| t.parse::<Address>().ok())
                .unwrap_or(to);
            let token = self.token(token_at);
            let raw = found
                .amount_raw
                .as_deref()
                .and_then(|r| U256::from_str_radix(r, 10).ok())
                .unwrap_or_default();
            let grant = match found.kind {
                GuardApprovalKind::IncreaseAllowance => Grant::Increase,
                GuardApprovalKind::DecreaseAllowance => Grant::Decrease,
                GuardApprovalKind::Permit2Single => Grant::Permit2,
                _ => Grant::Approve,
            };
            // The amount as it is (a decrease by 2^256-1 is "all of it");
            // whether it grants is the concern's to say.
            let bits = if found.amount_bits == Some(160) {
                AmountBits::B160
            } else {
                AmountBits::B256
            };
            return Intent::Approve {
                amount: amount(raw, &token, is_unbounded_amount(raw, bits)),
                token,
                spender,
                grant,
                expires: found.deadline,
            };
        }
        Intent::Call {
            selector: Some(crate::primitives::to_hex(s, true)),
        }
    }

    /// A `multiSend(bytes)` payload's legs, `depth` batches deep; `None`
    /// when it does not read, is deeper than [`MAX_NESTING`], or holds more
    /// calls than are read.
    pub fn batch(&mut self, path: &[u32], data: &[u8], depth: u32) -> Option<Vec<Call>> {
        if depth > MAX_NESTING {
            return None;
        }
        if word(data, 0)? != U256::from(0x20) {
            return None;
        }
        let len: usize = word(data, 1)?.try_into().ok()?;
        let packed = data.get(68..68usize.checked_add(len)?)?;
        let mut out = Vec::new();
        let mut i = 0usize;
        while i < packed.len() {
            let leg = packed.get(i..i.checked_add(85)?)?;
            let to = Address::from_slice(&leg[1..21]);
            let value = U256::from_be_slice(&leg[21..53]);
            let n: usize = U256::from_be_slice(&leg[53..85]).try_into().ok()?;
            let d = packed.get(i + 85..(i + 85).checked_add(n)?)?;
            if self.count >= MAX_CALLS {
                return None;
            }
            let mut at = path.to_vec();
            at.push(u32::try_from(out.len() + 1).ok()?);
            if leg[0] != 0 {
                // A delegatecall leg: MultiSendCallOnly refuses it; nobody
                // reads what it would run.
                self.count += 1;
                out.push(Call {
                    leg: leg_number(&at),
                    path: at,
                    to: checksum(to),
                    value: self.coin(value),
                    intent: Intent::Unreadable,
                });
            } else {
                out.push(self.call(at, to, value, d, depth));
            }
            i += 85 + n;
        }
        (!out.is_empty()).then_some(out)
    }
}

/// A leg's number as said (r10-10): `[2, 1]` → `"2.1"`; the transaction's
/// one call (`[]`) has none.
pub fn leg_number(path: &[u32]) -> Option<String> {
    (!path.is_empty()).then(|| {
        path.iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(".")
    })
}

/// The calls of `tx`, as `review` read it, with the token facts read.
pub fn read(
    chain_id: u64,
    tx: &SafeTx,
    review: &Review,
    tokens: &BTreeMap<String, TokenFacts>,
) -> Reading {
    let mut r = Reader::new(chain_id, tokens);
    let leg = |r: &mut Reader, path: Vec<u32>, to: &str, value: &str, data: &str| -> Option<Call> {
        let to = to.parse::<Address>().ok()?;
        let value = U256::from_str_radix(value, 10).ok()?;
        let data = unhex(data)?;
        Some(r.call(path, to, value, &data, 0))
    };
    let calls: Vec<Call> = match &review.action {
        Some(Action::Batch { calls }) => calls
            .iter()
            .enumerate()
            .filter_map(|(i, c)| {
                let path = vec![u32::try_from(i + 1).unwrap_or(u32::MAX)];
                match &c.action {
                    a @ (Action::AddOwner { .. }
                    | Action::RemoveOwner { .. }
                    | Action::SwapOwner { .. }
                    | Action::ChangeThreshold { .. }) => Some(Call {
                        leg: leg_number(&path),
                        path,
                        to: c.to.clone(),
                        value: r.coin(U256::ZERO),
                        intent: Intent::Members { action: a.clone() },
                    }),
                    Action::Rename { name } => Some(Call {
                        leg: leg_number(&path),
                        path,
                        to: c.to.clone(),
                        value: r.coin(U256::ZERO),
                        intent: Intent::Rename { name: name.clone() },
                    }),
                    _ => leg(&mut r, path, &c.to, &c.value, &c.data),
                }
            })
            .collect(),
        Some(Action::Transfer { .. } | Action::Call { .. }) => {
            let value = crate::multisig_review::quantity_dec(&tx.value);
            leg(&mut r, Vec::new(), &tx.to, &value, &tx.data)
                .into_iter()
                .collect()
        }
        _ => Vec::new(),
    };
    let mut reading = Reading {
        calls,
        ..Reading::default()
    };
    let mut unknown: Vec<String> = Vec::new();
    let (mut nested, mut unreadable, mut unlimited, mut blind) = (false, false, false, false);
    for c in reading.every() {
        let token = match &c.intent {
            Intent::Send { token, .. }
            | Intent::SendFrom { token, .. }
            | Intent::Approve { token, .. } => Some(token),
            _ => None,
        };
        if let Some(t) = token {
            if t.decimals.is_none() {
                if let Some(a) = &t.token {
                    if !unknown.contains(a) {
                        unknown.push(a.clone());
                    }
                }
            }
        }
        match &c.intent {
            Intent::Batch { .. } => nested = true,
            Intent::Unreadable => unreadable = true,
            Intent::Approve { amount, grant, .. } => {
                if amount.unlimited && *grant != Grant::Decrease {
                    unlimited = true;
                }
            }
            Intent::ApproveAll { granted: true, .. } => unlimited = true,
            Intent::Call { .. } => blind = true,
            _ => {}
        }
    }
    reading.nested = nested;
    reading.unreadable = unreadable;
    reading.unlimited = unlimited;
    reading.blind = blind;
    reading.decimals_unknown = unknown;
    reading
}

/// What the review says of `reading` (r8-1, r8-2, r8-5), in this order: a
/// batch that does not read (nothing is signed), each unlimited grant, each
/// token sent to its own contract, a nested batch, each token whose decimals
/// are unknown, a call nobody reads.
pub fn concerns(reading: &Reading) -> Vec<Concern> {
    let mut out = Vec::new();
    if reading.unreadable {
        out.push(Concern::UnreadableBatch);
    }
    for c in reading.every() {
        match &c.intent {
            Intent::Approve {
                token,
                spender,
                amount,
                grant,
                ..
            } if amount.unlimited && *grant != Grant::Decrease => {
                out.push(Concern::UnlimitedApproval {
                    token: token.token.clone().unwrap_or_default(),
                    spender: spender.clone(),
                });
            }
            Intent::ApproveAll {
                collection,
                operator,
                granted: true,
            } => out.push(Concern::UnlimitedApproval {
                token: collection.to_ascii_lowercase(),
                spender: operator.clone(),
            }),
            Intent::Send { token, to, .. } | Intent::SendFrom { token, to, .. }
                if token
                    .token
                    .as_deref()
                    .is_some_and(|t| t.eq_ignore_ascii_case(to)) =>
            {
                out.push(Concern::ToOwnToken {
                    token: to.to_ascii_lowercase(),
                });
            }
            _ => {}
        }
    }
    if reading.nested {
        out.push(Concern::NestedBatch);
    }
    for token in &reading.decimals_unknown {
        out.push(Concern::DecimalsUnknown {
            token: token.clone(),
        });
    }
    if reading.blind {
        out.push(Concern::Blind);
    }
    let mut said: Vec<Concern> = Vec::new();
    out.retain(|c| {
        let fresh = !said.contains(c);
        if fresh {
            said.push(c.clone());
        }
        fresh
    });
    out
}

/// Every token a reading of `tx` names that the registry does not (so the
/// core asks its contract), lowercase.
pub fn tokens_to_read(chain_id: u64, tx: &SafeTx, review: &Review) -> Vec<String> {
    let empty = BTreeMap::new();
    let reading = read(chain_id, tx, review, &empty);
    let mut out: Vec<String> = Vec::new();
    for c in reading.every() {
        let token = match &c.intent {
            Intent::Send { token, .. }
            | Intent::SendFrom { token, .. }
            | Intent::Approve { token, .. } => token.token.clone(),
            _ => None,
        };
        if let Some(t) = token {
            let known = TokenFacts::of(chain_id, Some(&t), &empty).known;
            if !known && !out.contains(&t) {
                out.push(t);
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#[path = "multisig_review_reading_tests.rs"]
mod tests;
