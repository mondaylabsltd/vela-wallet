//! What a transaction's sheet draws from the proposals machine's view, and
//! which of its presses it offers — the web's `ProposalSheet.svelte`
//! drawing rules, gpui-free so they are tested here. Nothing is decided
//! about the transaction: who may approve or execute (`can_approve`,
//! `can_execute`), each executor's look (`previews`), who one press
//! approves and executes for (`approve_execute`), how the remaining members
//! sign (`signers`) and whether a signature may be given at all (the
//! review's `sign`) are the core's. This file only picks what to draw of
//! them, as the web does.

use serde_json::Value;
use vela_core::app::multisig_proposals::{ItemSigners, ProposalStatus};
use vela_core::multisig_review::SignRule;

fn same(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// `<chainId>:<hash lowercase>:<executor lowercase>` — the machine's key
/// for one executor's look (`previews`).
#[must_use]
pub fn look_key(chain_id: u64, hash: &str, executor: &str) -> String {
    format!(
        "{chain_id}:{}:{}",
        hash.to_ascii_lowercase(),
        executor.to_ascii_lowercase()
    )
}

/// `<chainId>:<hash lowercase>` — `approve_execute`'s key.
#[must_use]
pub fn press_key(chain_id: u64, hash: &str) -> String {
    format!("{chain_id}:{}", hash.to_ascii_lowercase())
}

/// The executors an open sheet asks the machine to look for (r8-6): every
/// member here who can execute it — also while it is one approval short,
/// so "Approve and execute" and the fee are known before any press (the
/// web's `looked`). Nothing for one done or one Vela cannot read.
#[must_use]
pub fn looked(status: ProposalStatus, readable: bool, can_execute: &[String]) -> Vec<String> {
    if status == ProposalStatus::Done || !readable {
        return Vec::new();
    }
    can_execute.to_vec()
}

/// One executor's look, as the machine keeps it.
#[derive(Clone, Debug, PartialEq)]
pub enum Look {
    Priced {
        /// The fee in the chain's coin, base units (decimal).
        fee: String,
        can_pay: bool,
        /// What the executor holds (decimal), when read.
        balance: Option<String>,
    },
    Fails(Value),
    Refused(Value),
    Spent,
    Unpriced,
    Unchecked,
}

impl Look {
    /// The machine's `ExecutionPreview` JSON (`None`: a kind not known here
    /// — drawn as still checking, never as fine).
    #[must_use]
    pub fn of(value: &Value) -> Option<Self> {
        Some(match value.get("kind")?.as_str()? {
            "priced" => {
                let fee = value
                    .get("fee")
                    .and_then(|f| f.get("amount"))
                    .map(|a| match a {
                        Value::String(s) => vela_core::app::multisig_shell::quantity(s)
                            .map(|v| v.to_string())
                            .unwrap_or_default(),
                        other => other.to_string(),
                    })
                    .unwrap_or_default();
                Self::Priced {
                    fee,
                    can_pay: value.get("canPay").and_then(Value::as_bool).unwrap_or(true),
                    balance: value
                        .get("balance")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                }
            }
            "fails" => Self::Fails(value.get("why").cloned().unwrap_or(Value::Null)),
            "refused" => Self::Refused(value.get("authority").cloned().unwrap_or(Value::Null)),
            "spent" => Self::Spent,
            "unpriced" => Self::Unpriced,
            "unchecked" => Self::Unchecked,
            _ => return None,
        })
    }

    /// It cannot run by this executor: it would fail, it is refused, its
    /// number is used, or the executor cannot pay the fee.
    #[must_use]
    pub fn blocks(&self) -> bool {
        matches!(
            self,
            Self::Fails(_) | Self::Refused(_) | Self::Spent | Self::Priced { can_pay: false, .. }
        )
    }
}

/// The look the machine holds for `executor` (`None`: none yet).
#[must_use]
pub fn look_of(
    previews: &std::collections::BTreeMap<String, Value>,
    chain_id: u64,
    hash: &str,
    executor: &str,
) -> Option<Look> {
    let key = look_key(chain_id, hash, executor);
    previews
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        .and_then(|(_, v)| Look::of(v))
}

/// What the sheet's foot offers, for the member it acts as.
#[derive(Clone, Debug, PartialEq)]
pub struct Foot {
    /// The members here it may act as (executors first, else approvers).
    pub candidates: Vec<String>,
    /// The one it acts as.
    pub actor: Option<String>,
    pub kind: FootKind,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FootKind {
    /// No member here can approve or execute it.
    Nothing,
    /// A member here can approve it, none can execute it.
    Approve,
    /// A member here can execute it (with its approval first, when short).
    Execute {
        /// The actor's look (`None`: still checking).
        look: Option<Look>,
        /// The actor has not approved and it is short of approvals: a
        /// press approves first.
        approve_first: bool,
        /// The actor can approve on its own (Approve beside the one press).
        can_approve_only: bool,
        /// The machine offers this actor the one press (`approve_execute`).
        one_tap: bool,
    },
}

/// The inputs of [`foot`], all the machine's.
pub struct FootFacts<'a> {
    pub chain_id: u64,
    pub hash: &'a str,
    pub status: ProposalStatus,
    pub readable: bool,
    pub approved: u64,
    pub threshold: u64,
    /// The members whose approval counts now.
    pub approved_by: &'a [String],
    pub can_approve: &'a [String],
    pub can_execute: &'a [String],
    pub previews: &'a std::collections::BTreeMap<String, Value>,
    pub approve_execute: &'a std::collections::BTreeMap<String, Vec<String>>,
    pub last_actor: Option<&'a str>,
}

/// The web's actor and foot (r5reg-5, r4-5, core-ms-2): the person's pick;
/// else an executor whose look can pay; else the member used here last;
/// else the first — among the executors here, else the approvers.
#[must_use]
pub fn foot(f: &FootFacts<'_>, picked: Option<&str>) -> Foot {
    let executing = !f.can_execute.is_empty() && f.status != ProposalStatus::Done && f.readable;
    let candidates: Vec<String> = if executing {
        f.can_execute.to_vec()
    } else if f.status != ProposalStatus::Done && f.readable {
        f.can_approve.to_vec()
    } else {
        Vec::new()
    };
    let find = |who: Option<&str>| {
        who.and_then(|w| candidates.iter().find(|c| same(c, w)).cloned())
    };
    let payer = executing
        .then(|| {
            f.can_execute.iter().find(|e| {
                matches!(
                    look_of(f.previews, f.chain_id, f.hash, e),
                    Some(Look::Priced { can_pay: true, .. })
                )
            })
        })
        .flatten()
        .cloned();
    let actor = find(picked)
        .or(payer)
        .or_else(|| find(f.last_actor))
        .or_else(|| candidates.first().cloned());
    let kind = match &actor {
        None => FootKind::Nothing,
        Some(_) if !executing => FootKind::Approve,
        Some(actor) => {
            let actor_approved = f.approved_by.iter().any(|a| same(a, actor));
            let one_tap = f
                .approve_execute
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&press_key(f.chain_id, f.hash)))
                .is_some_and(|(_, who)| who.iter().any(|w| same(w, actor)));
            FootKind::Execute {
                look: look_of(f.previews, f.chain_id, f.hash, actor),
                approve_first: !actor_approved && f.approved < f.threshold,
                can_approve_only: f.can_approve.iter().any(|a| same(a, actor)),
                one_tap,
            }
        }
    };
    Foot {
        candidates,
        actor,
        kind,
    }
}

/// The execute press's words (`anywayLabel`): "Approve and execute" when a
/// press approves first, "… anyway" where the review's rule says so.
#[must_use]
pub fn execute_label(approve_first: bool, sign: SignRule) -> &'static str {
    match (approve_first, sign.anyway) {
        (true, true) => "review.approveExecuteAnyway",
        (true, false) => "proposal.approveAndExecute",
        (false, true) => "review.executeAnyway",
        (false, false) => "proposal.execute",
    }
}

/// The approve press's words.
#[must_use]
pub fn approve_label(sign: SignRule) -> &'static str {
    if sign.anyway {
        "review.approveAnyway"
    } else {
        "proposal.approve"
    }
}

/// Whether the execute press can be pressed: the review is here and lets a
/// signature be given, the look is in, and a press that approves first is
/// one the machine offers (`approve_execute`).
#[must_use]
pub fn execute_enabled(reviewed: bool, sign: SignRule, look: Option<&Look>, approve_first: bool, one_tap: bool) -> bool {
    reviewed && sign.allowed && look.is_some() && !(approve_first && !one_tap)
}

/// A member still to approve, as its row in the approvals list says how it
/// signs (r7-8): the core's split, never this device's own member.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MemberWay {
    /// On this device, or nothing to offer: "Waiting".
    Waiting,
    /// A Vela account elsewhere: approves in Vela from the link.
    InVela,
    /// Nothing says yet: the link, or its own wallet.
    Either,
    /// An outside wallet: "Approve with another wallet".
    Outside,
    /// A smart contract account: approves from its own app.
    Contract,
}

/// How `owner` (still to approve) signs, from the machine's split. A
/// member here is never offered another wallet.
#[must_use]
pub fn member_way(signers: Option<&ItemSigners>, owner: &str, here: bool) -> MemberWay {
    let Some(s) = signers else {
        return MemberWay::Waiting;
    };
    if here || s.enough {
        return MemberWay::Waiting;
    }
    let has = |list: &[String]| list.iter().any(|o| same(o, owner));
    if has(&s.either) {
        MemberWay::Either
    } else if has(&s.outside) {
        MemberWay::Outside
    } else if has(&s.in_vela) {
        MemberWay::InVela
    } else if has(&s.contract) {
        MemberWay::Contract
    } else {
        MemberWay::Waiting
    }
}

/// The outside-wallet link's members: the core's `outside`, never a member
/// here (r7-8).
#[must_use]
pub fn outside_members(signers: Option<&ItemSigners>, here: &dyn Fn(&str) -> bool) -> Vec<String> {
    signers
        .filter(|s| !s.enough)
        .map(|s| s.outside.iter().filter(|o| !here(o)).cloned().collect())
        .unwrap_or_default()
}

/// The approvals line's count: an executed one says what counted when it
/// ran (the core's `frozen`: the executor's approval by sending it
/// included), never the list as it reads after — "1/2 approved" over an
/// executed transaction (r8-13, r8-17). Open: the list's own count.
#[must_use]
pub fn approvals_shown(
    done: bool,
    frozen: Option<&vela_core::app::multisig_presence::FrozenFacts>,
    approved: u64,
    threshold: u64,
) -> (u64, u64) {
    match frozen.filter(|_| done) {
        Some(f) => (f.approvers.len() as u64, f.threshold),
        None => (approved.min(threshold), threshold),
    }
}

/// The home's "Get it ready" card (r4D-8, r8-13): while the multisig is
/// kept as just created here (`first_run`) and the machine has not read it
/// set up on any network (`set_up`, for the multisig it follows).
#[must_use]
pub fn get_ready_shown(first_run: Option<bool>, followed: bool, set_up: bool) -> bool {
    first_run.unwrap_or(false) && !(followed && set_up)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    const HASH: &str = "0xABCDEF";
    const DANA: &str = "0x306b000000000000000000000000000000009ad8";
    const ALICE: &str = "0x7a12000000000000000000000000000000076c6";
    const EVE: &str = "0x9247000000000000000000000000000000046e4";

    fn facts<'a>(
        status: ProposalStatus,
        approved: u64,
        approved_by: &'a [String],
        can_approve: &'a [String],
        can_execute: &'a [String],
        previews: &'a BTreeMap<String, Value>,
        approve_execute: &'a BTreeMap<String, Vec<String>>,
    ) -> FootFacts<'a> {
        FootFacts {
            chain_id: 5_042_002,
            hash: HASH,
            status,
            readable: true,
            approved,
            threshold: 2,
            approved_by,
            can_approve,
            can_execute,
            previews,
            approve_execute,
            last_actor: None,
        }
    }

    /// r8-6: a transaction one approval short, whose approval here makes it
    /// ready, is looked at by its executor — the web asks for every executor
    /// whatever the status; the desktop asked only once it was ready.
    #[test]
    fn a_pending_one_with_an_executor_here_is_looked_at() {
        let execs = vec![DANA.to_owned()];
        assert_eq!(looked(ProposalStatus::Pending, true, &execs), execs);
        assert_eq!(looked(ProposalStatus::Ready, true, &execs), execs);
        assert!(looked(ProposalStatus::Done, true, &execs).is_empty());
        assert!(looked(ProposalStatus::Pending, false, &execs).is_empty());
    }

    /// r8-6: one short, the look priced and the machine offering the one
    /// press: "Approve and execute", pressable, Approve beside it.
    #[test]
    fn the_one_press_is_offered_when_the_machine_says_so() {
        let execs = vec![DANA.to_owned()];
        let mut previews = BTreeMap::new();
        previews.insert(
            look_key(5_042_002, HASH, DANA),
            json!({"kind": "priced", "fee": {"amount": "0x2386f26fc10000"}, "canPay": true, "balance": "400000000000000000"}),
        );
        let mut offered = BTreeMap::new();
        offered.insert(press_key(5_042_002, HASH), vec![DANA.to_owned()]);
        let approved_by = vec![ALICE.to_owned()];
        let f = facts(ProposalStatus::Pending, 1, &approved_by, &execs, &execs, &previews, &offered);
        let foot = foot(&f, None);
        assert_eq!(foot.actor.as_deref(), Some(DANA));
        let FootKind::Execute { look, approve_first, can_approve_only, one_tap } = foot.kind else {
            panic!("an executor here: {foot:?}");
        };
        assert!(approve_first && can_approve_only && one_tap);
        assert_eq!(
            look,
            Some(Look::Priced {
                fee: "10000000000000000".into(),
                can_pay: true,
                balance: Some("400000000000000000".into())
            })
        );
        let allowed = SignRule { allowed: true, anyway: false };
        assert!(execute_enabled(true, allowed, look.as_ref(), approve_first, one_tap));
        assert_eq!(execute_label(approve_first, allowed), "proposal.approveAndExecute");
    }

    /// No look yet, or a one press the machine does not offer: the press
    /// waits (never "Approve and execute" from a guess).
    #[test]
    fn no_one_press_before_the_look_or_the_offer() {
        let allowed = SignRule { allowed: true, anyway: false };
        assert!(!execute_enabled(true, allowed, None, false, false));
        assert!(!execute_enabled(true, allowed, Some(&Look::Unchecked), true, false));
        assert!(execute_enabled(true, allowed, Some(&Look::Unchecked), true, true));
        // The review refuses: nothing at all.
        let refused = SignRule { allowed: false, anyway: false };
        assert!(!execute_enabled(true, refused, Some(&Look::Unchecked), false, false));
        // No review yet.
        assert!(!execute_enabled(false, allowed, Some(&Look::Unchecked), false, false));
    }

    /// r8-1 (the shell's half): the review's `anyway` is the press's words.
    #[test]
    fn the_review_rule_names_the_press() {
        let anyway = SignRule { allowed: true, anyway: true };
        assert_eq!(execute_label(true, anyway), "review.approveExecuteAnyway");
        assert_eq!(execute_label(false, anyway), "review.executeAnyway");
        assert_eq!(approve_label(anyway), "review.approveAnyway");
        assert_eq!(approve_label(SignRule { allowed: true, anyway: false }), "proposal.approve");
    }

    /// The executor that can pay is picked over the first one; the
    /// person's pick over both.
    #[test]
    fn the_payer_is_picked_unless_the_person_picks() {
        let execs = vec![ALICE.to_owned(), DANA.to_owned()];
        let mut previews = BTreeMap::new();
        previews.insert(look_key(5_042_002, HASH, ALICE), json!({"kind": "priced", "fee": {"amount": "10"}, "canPay": false}));
        previews.insert(look_key(5_042_002, HASH, DANA), json!({"kind": "priced", "fee": {"amount": "10"}, "canPay": true}));
        let none = BTreeMap::new();
        let approved_by = vec![ALICE.to_owned(), DANA.to_owned()];
        let f = facts(ProposalStatus::Ready, 2, &approved_by, &[], &execs, &previews, &none);
        assert_eq!(foot(&f, None).actor.as_deref(), Some(DANA));
        assert_eq!(foot(&f, Some(ALICE)).actor.as_deref(), Some(ALICE));
        let FootKind::Execute { look, approve_first, .. } = foot(&f, Some(ALICE)).kind else {
            panic!()
        };
        assert!(!approve_first);
        assert!(look.is_some_and(|l| l.blocks()));
    }

    /// Done, or no executor here: no execute press.
    #[test]
    fn approve_only_and_done() {
        let approvers = vec![DANA.to_owned()];
        let (none, nobody) = (BTreeMap::new(), BTreeMap::new());
        let f = facts(ProposalStatus::Pending, 0, &[], &approvers, &[], &none, &nobody);
        assert_eq!(foot(&f, None).kind, FootKind::Approve);
        let f = facts(ProposalStatus::Done, 2, &[], &[], &[], &none, &nobody);
        assert_eq!(foot(&f, None).kind, FootKind::Nothing);
    }

    /// r7-8: a member on this device is never offered another wallet; an
    /// outside one is; one nothing says yet both ways.
    #[test]
    fn a_member_here_is_never_offered_another_wallet() {
        let signers = ItemSigners {
            rows: Vec::new(),
            outside: vec![EVE.into()],
            in_vela: vec![ALICE.into()],
            either: Vec::new(),
            contract: Vec::new(),
            enough: false,
        };
        assert_eq!(member_way(Some(&signers), DANA, true), MemberWay::Waiting);
        assert_eq!(member_way(Some(&signers), EVE, false), MemberWay::Outside);
        assert_eq!(member_way(Some(&signers), ALICE, false), MemberWay::InVela);
        // Even were the core's list to name it, a member here is not asked.
        let odd = ItemSigners { outside: vec![DANA.into(), EVE.into()], ..signers.clone() };
        assert_eq!(member_way(Some(&odd), DANA, true), MemberWay::Waiting);
        assert_eq!(outside_members(Some(&odd), &|a| same(a, DANA)), vec![EVE.to_owned()]);
        // Enough approved: nobody is asked.
        let enough = ItemSigners { enough: true, ..signers };
        assert_eq!(member_way(Some(&enough), EVE, false), MemberWay::Waiting);
        assert!(outside_members(Some(&enough), &|_| false).is_empty());
    }

    /// r8-13: "Get it ready" while just created here, until it is set up.
    #[test]
    fn get_ready_until_it_is_set_up() {
        assert!(get_ready_shown(Some(true), true, false));
        assert!(get_ready_shown(Some(true), false, true), "set_up is the followed one's");
        assert!(!get_ready_shown(Some(true), true, true));
        assert!(!get_ready_shown(None, true, false));
    }

    /// r8-13: an executed sheet counts what counted when it ran — the
    /// executor's approval by sending it too — not the list after.
    #[test]
    fn an_executed_one_counts_what_counted_when_it_ran() {
        let frozen: vela_core::app::multisig_presence::FrozenFacts = serde_json::from_value(json!({
            "executor": DANA,
            "threshold": 2,
            "approvers": [ALICE, DANA],
            "executorApproved": true,
            "source": "chain",
            "atMs": null
        }))
        .expect("frozen facts");
        assert_eq!(approvals_shown(true, Some(&frozen), 1, 2), (2, 2));
        assert_eq!(approvals_shown(false, Some(&frozen), 1, 2), (1, 2), "open: the list's own");
        assert_eq!(approvals_shown(true, None, 1, 2), (1, 2));
        assert_eq!(approvals_shown(false, None, 3, 2), (2, 2));
    }
}
