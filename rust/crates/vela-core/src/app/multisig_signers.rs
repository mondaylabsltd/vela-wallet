//! What a multisig member is — a Vela account, an outside wallet, or a smart
//! contract account — and how it signs: ONE rule over facts, used by every
//! machine (the members entry, the accounts machine's create and import,
//! the proposals machine's rows, the outside machines' signers).
//!
//! A port of `core/signers.svelte.ts` (`kindOf`, `evidence`,
//! `plainAccounts`) and `core/member-kind.ts` (`kindChoice`), with the
//! owner's rule for what a Vela account is (2026-10-30):
//!
//! - **a Vela account** when the chain shows Vela's passkey signer
//!   configured for it — the shared WebAuthn signer's
//!   `getConfiguration(address)` is non-empty (read as the passkey-index
//!   lookup reads it, [`registry_lookup::configuration_call`] /
//!   [`registry_lookup::configured`]) — or the passkey index, this device, or
//!   a counted passkey approval says so;
//! - **a smart contract account** when there is any OTHER contract code at
//!   its address (an EIP-7702 delegation is none — a person's wallet, which
//!   signs): it approves from its own app (EIP-1271), which Vela can't
//!   collect yet;
//! - no code and no evidence: as before — "Other wallet" hinted, or asked.
//!
//! A read that failed decides nothing: code with its configuration unread
//! is neither Vela nor contract until the chain answers. The kind kept on
//! this device (the person's word, an invite's `p`/`w`/`c`) is a label that
//! steers what is offered, never authority — evidence beats it.
//!
//! The configuration is read on the multisig's networks and on the passkey
//! index's own chains (`registry_lookup`'s home chain and the others its
//! first hop asks): a Vela account set up anywhere answers there, which is
//! what the index knows of it.

use std::collections::BTreeMap;

use futures::future;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::multisig_shell::{is_contract_code, same_address, Io};
use crate::multisig::Approval;
use crate::registry_lookup;

/// How long one code or configuration read is waited for: past it, that
/// read said nothing (no verdict).
pub const SIGNER_READ_MS: u32 = 8_000;

// ---------------------------------------------------------------------------
// The facts
// ---------------------------------------------------------------------------

/// How a member signs, as this device can tell (ms-11, r4-9, r4a-9, r7).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSignerKind"))]
pub enum SignerKind {
    /// A Vela account on this device.
    Here,
    /// A Vela account elsewhere: it approves in Vela; never an outside wallet.
    Vela,
    /// A plain account: its own wallet signs.
    Wallet,
    /// Nothing says: the Vela way first, the outside wallet's beside it.
    Unsure,
    /// A smart contract account that is no Vela account: it approves from
    /// its own app (EIP-1271), which Vela can't collect yet — neither an
    /// outside wallet nor the Vela link is offered, and that is said.
    Contract,
}

/// What a counted approval says of how an address approved.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideApprovedWith"))]
pub enum ApprovedWith {
    Passkey,
    Wallet,
}

/// What `rows` (`(owner, counts, approval)`, from the proposals this device
/// reads, any network) say of how `address` approved, if it did (r4a-9):
/// only a counted approval is evidence.
pub fn approved_with_in<'a>(
    rows: impl IntoIterator<Item = (&'a str, bool, &'a Approval)>,
    address: &str,
) -> Option<ApprovedWith> {
    let mut wallet = false;
    for (owner, counts, approval) in rows {
        if !counts || !same_address(owner, address) {
            continue;
        }
        match approval {
            Approval::Passkey(_) => return Some(ApprovedWith::Passkey),
            Approval::Ecdsa(_) => wallet = true,
            Approval::OnChain(_) => {}
        }
    }
    wallet.then_some(ApprovedWith::Wallet)
}

/// What is known of one member, read through the shell.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "OutsideSignerFacts"))]
pub struct SignerFacts {
    /// A Vela account on this device.
    pub local: bool,
    pub approved_with: Option<ApprovedWith>,
    /// The passkey index knows it as a Vela account.
    pub indexed: bool,
    /// The passkey index answered for it (either way).
    pub index_answered: bool,
    /// This device's word: `passkey`, `wallet` or `contract`.
    pub kept: Option<String>,
    /// Contract code at its address (an EIP-7702 delegation is none);
    /// `None` while it is read (or the read failed).
    pub code: Option<bool>,
    /// Vela's passkey signer is configured for it (the shared WebAuthn
    /// signer's `getConfiguration(address)` is non-empty): `Some(true)` on
    /// a network read; `Some(false)` every network read answered none;
    /// `None` unread, or a read failed with none configured.
    pub configured: Option<bool>,
}

impl SignerFacts {
    /// The chain or the index shows Vela's passkey signer for it.
    fn shows_vela(&self) -> bool {
        self.indexed || self.configured == Some(true)
    }

    /// The chain answered that Vela's passkey signer is not configured for
    /// it (or the index answered it holds none).
    fn no_vela_anywhere(&self) -> bool {
        self.configured == Some(false) || (self.index_answered && self.configured.is_none())
    }

    fn kept_is(&self, kind: &str) -> bool {
        self.kept.as_deref() == Some(kind)
    }
}

/// How a member signs (`kindOf`); `None` while that is still being read:
/// nothing is offered on a guess.
pub fn signer_kind(f: &SignerFacts) -> Option<SignerKind> {
    if f.local {
        return Some(SignerKind::Here);
    }
    if f.approved_with == Some(ApprovedWith::Passkey) || f.shows_vela() {
        return Some(SignerKind::Vela);
    }
    match f.code {
        // Code, and the chain says Vela's signer is not configured for it:
        // a smart contract account, whatever is kept.
        Some(true) if f.configured == Some(false) => return Some(SignerKind::Contract),
        // Code, its configuration not read yet: the person's word stands
        // (never offered to an outside wallet), else no verdict.
        Some(true) => {
            return match f.kept.as_deref() {
                Some("passkey") => Some(SignerKind::Vela),
                Some("contract") => Some(SignerKind::Contract),
                _ => None,
            };
        }
        _ if f.kept_is("passkey") => return Some(SignerKind::Vela),
        _ if f.kept_is("contract") => return Some(SignerKind::Contract),
        None => return None,
        Some(false) => {}
    }
    // No code there: a wallet's own signature, or this device's word.
    if f.approved_with == Some(ApprovedWith::Wallet) || f.kept_is("wallet") {
        return Some(SignerKind::Wallet);
    }
    // Neither the chain nor the index has answered: no verdict yet.
    if !f.no_vela_anywhere() {
        return None;
    }
    Some(SignerKind::Unsure)
}

/// Of `owners`, those an outside wallet may be asked to sign for (r4C-10):
/// not this device's, no code at their address, not a Vela account (the
/// chain's configuration, the index, a passkey approval, this device's
/// word), not a smart contract account kept as one. A read that failed is no
/// code and no Vela evidence (`plainAccounts`).
pub fn plain_accounts(owners: &[String], facts: impl Fn(&str) -> SignerFacts) -> Vec<String> {
    owners
        .iter()
        .filter(|o| {
            let f = facts(o);
            !f.local
                && f.approved_with != Some(ApprovedWith::Passkey)
                && !f.kept_is("passkey")
                && !f.kept_is("contract")
                && f.code != Some(true)
                && !f.shows_vela()
        })
        .cloned()
        .collect()
}

// ---------------------------------------------------------------------------
// A member's kind, declared then verified (`member-kind.ts`)
// ---------------------------------------------------------------------------

/// A Vela account (approves with its passkey, in Vela), an outside wallet,
/// or a smart contract account — as kept with a member (`MemberInfo.kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigMemberKind"))]
pub enum MemberKind {
    Vela,
    Wallet,
    /// A smart contract account (r7): approves from its own app.
    Contract,
}

impl MemberKind {
    /// How a multisig keeps it (`MemberInfo.kind`).
    pub fn stored(self) -> &'static str {
        match self {
            Self::Vela => "passkey",
            Self::Wallet => "wallet",
            Self::Contract => "contract",
        }
    }

    /// What a kept kind reads as (`passkey`, `wallet`, `contract`).
    pub fn from_stored(kind: &str) -> Option<Self> {
        match kind {
            "passkey" => Some(Self::Vela),
            "wallet" => Some(Self::Wallet),
            "contract" => Some(Self::Contract),
            _ => None,
        }
    }
}

/// What the wallet can tell of a member being added, before anyone's word
/// (`KindEvidence`, r7). `None`: it can't tell (yet) — the person is asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigKindEvidence"))]
pub enum KindEvidence {
    /// An account here, a counted passkey approval, the index, or Vela's
    /// passkey signer configured for it on a chain.
    Vela,
    /// Contract code at its address on one of the networks, and the chain
    /// says Vela's passkey signer is not configured for it.
    Contract,
    /// Only a hint, likely an outside wallet: no code on any of the
    /// networks, and Vela's signer configured nowhere it was read.
    Wallet,
}

/// What the wallet can tell of `f` across the multisig's networks — the
/// add screens' pre-fill of its kind, never authority (`evidence`).
pub fn kind_evidence(f: &SignerFacts) -> Option<KindEvidence> {
    if f.local || f.approved_with == Some(ApprovedWith::Passkey) || f.shows_vela() {
        return Some(KindEvidence::Vela);
    }
    // Neither the chain nor the index has answered: a Vela account is not
    // ruled out — asked.
    if !f.no_vela_anywhere() {
        return None;
    }
    match f.code {
        Some(true) if f.configured == Some(false) => Some(KindEvidence::Contract),
        Some(true) | None => None,
        Some(false) => Some(KindEvidence::Wallet),
    }
}

/// The line under a member's kind choice (`KindChoice.note`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigKindNote"))]
pub enum KindNote {
    /// A Vela account, found.
    Known,
    /// A smart contract account: approves from its own app, which Vela
    /// can't collect yet.
    Contract,
    /// The "Other wallet" hint.
    Likely,
    /// It can't tell: the person is asked.
    Ask,
}

/// What a member's kind choice shows (`KindChoice`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "MultisigKindChoice"))]
pub struct KindChoice {
    /// What is kept (and drawn picked); `None`: nothing — the member stays unsure.
    pub kind: Option<MemberKind>,
    /// What the wallet suggests without keeping it: "Other wallet", drawn as a hint.
    pub suggested: Option<MemberKind>,
    /// Evidence (or a kept contract) says what it is: shown, not asked.
    pub locked: bool,
    /// The line under it; `None`: the person's own word.
    pub note: Option<KindNote>,
}

/// What a member's kind choice shows, from what the wallet can tell and
/// what the person said (here, or on this device before — `said`)
/// (`kindChoice`). Evidence beats the label.
pub fn kind_choice(evidence: Option<KindEvidence>, said: Option<MemberKind>) -> KindChoice {
    match (evidence, said) {
        (Some(KindEvidence::Vela), _) => KindChoice {
            kind: Some(MemberKind::Vela),
            suggested: None,
            locked: true,
            note: Some(KindNote::Known),
        },
        (Some(KindEvidence::Contract), _) | (None, Some(MemberKind::Contract)) => KindChoice {
            kind: Some(MemberKind::Contract),
            suggested: None,
            locked: true,
            note: Some(KindNote::Contract),
        },
        (_, Some(said)) if said != MemberKind::Contract => KindChoice {
            kind: Some(said),
            suggested: None,
            locked: false,
            note: None,
        },
        (Some(KindEvidence::Wallet), _) => KindChoice {
            kind: None,
            suggested: Some(MemberKind::Wallet),
            locked: false,
            note: Some(KindNote::Likely),
        },
        _ => KindChoice {
            kind: None,
            suggested: None,
            locked: false,
            note: Some(KindNote::Ask),
        },
    }
}

// ---------------------------------------------------------------------------
// Reading the chain (`hasContractCode`, the shared signer's configuration)
// ---------------------------------------------------------------------------

/// What the chains said of one address: its code on each network read, and
/// whether Vela's passkey signer is configured for it on each chain read
/// (`None`: that read did not answer).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignerRead {
    pub codes: BTreeMap<u64, Option<bool>>,
    pub configured: BTreeMap<u64, Option<bool>>,
}

impl SignerRead {
    /// Code at the address: on any network read (`Some(true)`), else on
    /// `primary` when given (the network a proposal is on), else on every
    /// network (`None` while one is unread; none read is no verdict).
    pub fn code(&self, primary: Option<u64>) -> Option<bool> {
        if self.codes.values().any(|c| *c == Some(true)) {
            return Some(true);
        }
        match primary {
            Some(id) => self.codes.get(&id).copied().flatten(),
            None if self.codes.is_empty() => None,
            None => self
                .codes
                .values()
                .all(|c| *c == Some(false))
                .then_some(false),
        }
    }

    /// Configured on any chain read; not configured when every read
    /// answered; else no verdict.
    pub fn configured(&self) -> Option<bool> {
        if self.configured.values().any(|c| *c == Some(true)) {
            return Some(true);
        }
        (!self.configured.is_empty() && self.configured.values().all(|c| *c == Some(false)))
            .then_some(false)
    }

    /// These reads as facts, with what this device knows besides.
    pub fn facts(
        &self,
        primary: Option<u64>,
        local: bool,
        approved_with: Option<ApprovedWith>,
        kept: Option<String>,
    ) -> SignerFacts {
        SignerFacts {
            local,
            approved_with,
            kept,
            code: self.code(primary),
            configured: self.configured(),
            ..SignerFacts::default()
        }
    }
}

/// The chains Vela's passkey signer configuration is read on for an
/// address used on `chain_ids`: those, then the passkey index's own (its
/// first hop's), each once.
pub fn configuration_chains(chain_ids: &[u64]) -> Vec<u64> {
    let mut out: Vec<u64> = Vec::new();
    let index = std::iter::once(registry_lookup::HOME_CHAIN)
        .chain(registry_lookup::OTHER_CHAINS)
        .map(u64::from);
    for id in chain_ids.iter().copied().chain(index) {
        if !out.contains(&id) {
            out.push(id);
        }
    }
    out
}

/// Read `address`'s code on each of `chain_ids` and Vela's passkey signer
/// configuration for it ([`configuration_chains`]), all at once, each read
/// bounded by [`SIGNER_READ_MS`]. Through the wallet's own RPC (`Rpc`).
pub async fn read_signer<Ev: Send + 'static>(
    io: &Io<Ev>,
    chain_ids: &[u64],
    address: &str,
) -> SignerRead {
    let codes = future::join_all(chain_ids.iter().map(|id| async move {
        let code = io
            .rpc_within(
                *id,
                "eth_getCode",
                json!([address, "latest"]),
                Some(SIGNER_READ_MS),
            )
            .await
            .ok()
            .map(|c| is_contract_code(&c));
        (*id, code)
    }));
    let call = registry_lookup::configuration_call(address);
    let config_chains = configuration_chains(chain_ids);
    let configured = future::join_all(config_chains.iter().map(|id| {
        let call = call.clone();
        async move {
            let Some((to, data)) = call else {
                return (*id, Some(false));
            };
            let answer = io
                .rpc_within(
                    *id,
                    "eth_call",
                    json!([{ "to": to, "data": data }, "latest"]),
                    Some(SIGNER_READ_MS),
                )
                .await
                .ok();
            let body = answer.as_ref().and_then(Value::as_str);
            (*id, registry_lookup::configured(body))
        }
    }));
    let (codes, configured) = futures::join!(codes, configured);
    SignerRead {
        codes: codes.into_iter().collect(),
        configured: configured.into_iter().collect(),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_read_says_what_its_chains_said() {
        let mut r = SignerRead::default();
        assert_eq!(r.code(None), None, "nothing read: no verdict");
        assert_eq!(r.configured(), None);
        r.codes.insert(1, Some(false));
        r.codes.insert(2, None);
        assert_eq!(r.code(None), None, "a network unread");
        assert_eq!(
            r.code(Some(1)),
            Some(false),
            "the proposal's network answered"
        );
        r.codes.insert(2, Some(true));
        assert_eq!(
            r.code(Some(1)),
            Some(true),
            "code on another of its networks"
        );
        r.configured.insert(1, Some(false));
        r.configured.insert(2, None);
        assert_eq!(r.configured(), None, "a read failed: no verdict");
        r.configured.insert(2, Some(false));
        assert_eq!(r.configured(), Some(false));
        r.configured.insert(3, Some(true));
        assert_eq!(r.configured(), Some(true));
    }

    #[test]
    fn the_configuration_is_read_on_its_networks_then_the_indexs() {
        let chains = configuration_chains(&[5_042_002, 8453]);
        assert_eq!(chains[..2], [5_042_002, 8453]);
        assert!(chains.contains(&100) && chains.contains(&1));
        assert_eq!(
            chains.iter().filter(|c| **c == 8453).count(),
            1,
            "each once"
        );
    }

    #[test]
    fn kinds_read_and_keep_as_the_web_keeps_them() {
        for k in [MemberKind::Vela, MemberKind::Wallet, MemberKind::Contract] {
            assert_eq!(MemberKind::from_stored(k.stored()), Some(k));
        }
        assert_eq!(MemberKind::from_stored("eoa"), None);
    }
}
