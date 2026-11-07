//! What a transaction's sheet draws from the proposals machine's view, and
//! which of its presses it offers — the web's `ProposalSheet.svelte`
//! drawing rules, gpui-free so they are tested here. Nothing is decided
//! about the transaction: who may approve or execute (`can_approve`,
//! `can_execute`), each executor's look (`previews`), who one press
//! approves and executes for (`approve_execute`), how the remaining members
//! sign (`signers`) and whether a signature may be given at all (the
//! review's state and its `sign`, through the core's one door —
//! `multisig_review_state::door`) are the core's. This file only picks what
//! to draw of them, as the web does.

use serde_json::Value;
use vela_core::app::multisig_proposals::{self as ms_proposals, ItemSigners, ProposalStatus};
use vela_core::app::multisig_review_state::{
    self as review_state, ReviewShows, ReviewState, SignDoor,
};
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
    /// The setup was read and refused here: the look's JSON.
    Refused(Value),
    /// r14-4: the setup could not be checked here (its Safe contracts went
    /// unread): the look's JSON. Execute waits; never said as a finding.
    Unverified(Value),
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
            "refused" => Self::Refused(value.clone()),
            "unverified" => Self::Unverified(value.clone()),
            "spent" => Self::Spent,
            "unpriced" => Self::Unpriced,
            "unchecked" => Self::Unchecked,
            _ => return None,
        })
    }

    /// r15 (fee moved): the fee this look draws (base units, decimal) — what
    /// a press under it carries (`shown_fee`), so the core signs exactly the
    /// figure the person pressed on, or stops with `fee_moved`.
    #[must_use]
    pub fn shown_fee(&self) -> Option<String> {
        match self {
            Self::Priced { fee, .. } if !fee.is_empty() => Some(fee.clone()),
            _ => None,
        }
    }

    /// It cannot run by this executor: it would fail, it is refused, its
    /// number is used, or the executor cannot pay the fee.
    #[must_use]
    pub fn blocks(&self) -> bool {
        matches!(
            self,
            Self::Fails(_)
                | Self::Refused(_)
                | Self::Unverified(_)
                | Self::Spent
                | Self::Priced { can_pay: false, .. }
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

/// r18-5 (§32): a press is out on `executor`'s look (the core serves it
/// `"signing": true`, from the press to its signature, cancel or stop): its
/// figure is the one being signed and is not re-quoted, so the fee line says
/// so (`proposal.feeSigning`).
#[must_use]
pub fn look_signing(
    previews: &std::collections::BTreeMap<String, Value>,
    chain_id: u64,
    hash: &str,
    executor: &str,
) -> bool {
    let key = look_key(chain_id, hash, executor);
    previews
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        .is_some_and(|(_, v)| v.get("signing").and_then(Value::as_bool) == Some(true))
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
    let find =
        |who: Option<&str>| who.and_then(|w| candidates.iter().find(|c| same(c, w)).cloned());
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

/// Whether the execute press can be pressed: the review's door is open
/// (read, and its rule lets a signature be given), the look is in, and a
/// press that approves first is one the machine offers (`approve_execute`).
#[must_use]
pub fn execute_enabled(
    door: &SignDoor,
    look: Option<&Look>,
    approve_first: bool,
    one_tap: bool,
) -> bool {
    door.enabled && look.is_some() && !(approve_first && !one_tap)
}

/// §12: where the review of `<chainId>:<hash>` stands, as the machine holds
/// it (`review_states`; `None`: not asked yet — the door draws it loading).
#[must_use]
pub fn state_of<'a>(
    states: &'a std::collections::BTreeMap<String, ReviewState>,
    chain_id: u64,
    hash: &str,
) -> Option<&'a ReviewState> {
    let key = press_key(chain_id, hash);
    states
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        .map(|(_, v)| v)
}

/// The review the machine holds for `<chainId>:<hash>` (`reviews`).
#[must_use]
pub fn review_of<'a>(
    reviews: &'a std::collections::BTreeMap<String, Value>,
    chain_id: u64,
    hash: &str,
) -> Option<&'a Value> {
    let key = press_key(chain_id, hash);
    reviews
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&key))
        .map(|(_, v)| v)
}

/// The sign door every sign, propose, approve and execute press follows:
/// the core's one rule (`multisig_review_state::door`) over the review's
/// state and the review. `drawable`: this sheet could draw that review — one
/// it cannot draw is not one the person read, so it is passed as no review
/// at all (a `ready` without a review is shut and said unreadable).
#[must_use]
pub fn sign_door(state: Option<&ReviewState>, review: Option<&Value>, drawable: bool) -> SignDoor {
    review_state::door(state, review.filter(|_| drawable))
}

/// The press's words and fill, as the door says them: "… anyway" only
/// through an open door that asks for it (`door.anyway`).
#[must_use]
pub fn press_rule(door: &SignDoor) -> SignRule {
    SignRule {
        allowed: door.enabled,
        anyway: door.anyway,
    }
}

/// D1 (real-device pass 2): the door draws the review, but neither its
/// state-change check nor its simulation was read — the core's danger line
/// (`reason_key`) goes above it with Try again (`retry`), and every press is
/// "… anyway" (`anyway`, [`press_rule`]).
pub fn unchecked(door: &SignDoor) -> bool {
    door.shows == ReviewShows::Review && door.reason_key.is_some()
}

/// A failed review's reason in the person's words: the core's key, its
/// network named where the words do (r14-13: "Couldn't reach {{network}} to
/// read this transaction." — never "… anyway").
#[must_use]
pub fn failed_reason_words(loc: &crate::loc::Loc, reason_key: &str, network: &str) -> String {
    loc.t_texts(reason_key, &[("network", network)]).to_string()
}

/// "Try again" under a failed review: the machine's `review_again` — for a
/// proposal (`Some((chain_id, hash))`) its review, else the draft's.
#[must_use]
pub fn review_again(proposal: Option<(u64, &str)>) -> ms_proposals::Event {
    ms_proposals::Event::ReviewAgain {
        chain_id: proposal.map(|(c, _)| c),
        safe_tx_hash: proposal.map(|(_, h)| h.to_owned()),
    }
}

/// Whether an open sheet asks the machine for its review again on its own
/// timer: only one already read, or one never asked. While it is read
/// (`loading`) the machine is at it; once it `failed`, asking again is the
/// person's "Try again" — never a quiet loop that turns a failure said on
/// screen back into "Reading…".
#[must_use]
pub fn asks_review_again(state: Option<&ReviewState>) -> bool {
    matches!(state, None | Some(ReviewState::Ready))
}

/// r16-19: whether an open sheet asks for its review now. Only once the
/// machine holds the transaction (`listed`: its list, or a link's read-only
/// one) — a review asked before lands `failed {gone}` in the core, said as
/// "no longer in the list" until Try again; then as
/// [`asks_review_again`] allows, at most every `every` ms since this sheet
/// last asked (`last`; `None`: never).
#[must_use]
pub fn asks_review_now(
    listed: bool,
    state: Option<&ReviewState>,
    last: Option<f64>,
    now: f64,
    every: f64,
) -> bool {
    listed && asks_review_again(state) && last.is_none_or(|at| now - at >= every)
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

/// r16-1 (iOS `pressFailures(at:)`, §23 r15-5): where the last Approve /
/// Execute is said when it did not go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailurePlace {
    /// In the sheet's pinned foot: right under the presses, or at the foot's
    /// top when none is drawn — on screen at the press, whatever the scroll.
    Foot,
    /// At the sheet's head: it has no foot (done).
    Head,
}

/// [`FailurePlace`] for a transaction in `status`: the foot, unless the
/// sheet draws none (done).
#[must_use]
pub fn failure_place(status: ProposalStatus) -> FailurePlace {
    if status == ProposalStatus::Done {
        FailurePlace::Head
    } else {
        FailurePlace::Foot
    }
}

/// Whether the foot draws a press for `kind` (its failure is then said
/// right under it): Approve, Execute or the one press — and, where the look
/// blocks the execution, Approve alone when the actor can still approve.
#[must_use]
pub fn presses_drawn(kind: &FootKind, has_actor: bool) -> bool {
    has_actor
        && match kind {
            FootKind::Nothing => false,
            FootKind::Approve => true,
            FootKind::Execute {
                look,
                approve_first,
                can_approve_only,
                ..
            } => {
                !look.as_ref().is_some_and(Look::blocks)
                    || (*approve_first && *can_approve_only && look != &Some(Look::Spent))
            }
        }
}

/// r16-1: what the last Approve and Execute (or the one press, kept on
/// `execute:` alone — §26 r16-4) of `hash` came to when they did not go,
/// worded by `words` from the view's
/// `actions` as they are NOW — never words kept from the moment it failed:
/// the core keeps a stop's figure on the executor's current look. Approve's
/// first; one failure both carry is said once.
pub fn press_failure_lines(
    actions: &std::collections::BTreeMap<String, ms_proposals::ActionState>,
    hash: &str,
    words: impl Fn(&ms_proposals::ActionState) -> Option<String>,
) -> Vec<String> {
    let hash = hash.to_ascii_lowercase();
    let mut lines: Vec<String> = Vec::new();
    for press in ["approve", "execute"] {
        if let Some(line) = actions.get(&format!("{press}:{hash}")).and_then(&words)
            && !lines.contains(&line)
        {
            lines.push(line);
        }
    }
    lines
}

/// r17-21 (§26 r16-4's way on): an Execute — plain, or the one press —
/// the look stopped because the fee could not be worked out or checked
/// (`stopped {unpriced | unchecked}`) offers **Check again** beside its
/// stop line: a new look of the execution (`proposal.feeRetry`), never
/// "Try again" (that re-asks a read that failed). The stop replaces the fee
/// line, whose own Check again is then not drawn, so this is the one. A
/// stop for any other reason (would fail, a setup refused) has no look to
/// take again.
#[must_use]
pub fn stop_offers_check_again(
    actions: &std::collections::BTreeMap<String, ms_proposals::ActionState>,
    hash: &str,
) -> bool {
    matches!(
        actions.get(&format!("execute:{}", hash.to_ascii_lowercase())),
        Some(ms_proposals::ActionState::Stopped { preview })
            if matches!(
                preview.get("kind").and_then(Value::as_str),
                Some("unpriced" | "unchecked")
            )
    )
}

#[cfg(test)]
mod tests {
    /// r17-21: a fee stop of an Execute (plain or the one press — both keep
    /// it on `execute:`) offers Check again; no stop, or a stop with no
    /// look to take again, does not.
    #[test]
    fn a_fee_stop_offers_check_again_and_nothing_else_does() {
        use vela_core::app::multisig_proposals::ActionState;
        let stop = |kind: &str| ActionState::Stopped {
            preview: json!({ "kind": kind }),
        };
        let at = |state: ActionState| {
            let mut actions = BTreeMap::new();
            actions.insert("execute:0xabc".to_owned(), state);
            actions
        };
        assert!(stop_offers_check_again(&at(stop("unpriced")), "0xABC"));
        assert!(stop_offers_check_again(&at(stop("unchecked")), "0xabc"));
        assert!(!stop_offers_check_again(&at(stop("fails")), "0xabc"));
        assert!(!stop_offers_check_again(&at(stop("refused")), "0xabc"));
        assert!(!stop_offers_check_again(&BTreeMap::new(), "0xabc"));
        let mut approve = BTreeMap::new();
        approve.insert("approve:0xabc".to_owned(), stop("unpriced"));
        assert!(!stop_offers_check_again(&approve, "0xabc"));
    }

    use std::collections::BTreeMap;

    use serde_json::json;

    use super::*;

    /// Round 14 (r14-13): a review whose network answered none of its reads
    /// is failed, said with the network's name — never a raw key, never a
    /// placeholder.
    #[test]
    fn a_review_its_network_never_answered_names_the_network() {
        let loc = crate::loc::Loc::for_tag("en");
        let line = failed_reason_words(
            &loc,
            vela_core::app::multisig_review_state::REASON_CHAIN_UNREAD,
            "Arc Testnet",
        );
        assert_eq!(line, "Couldn't reach Arc Testnet to read this transaction.");
        let door = review_state::door(
            Some(&ReviewState::failed(
                vela_core::app::multisig_review_state::REASON_CHAIN_UNREAD,
            )),
            None,
        );
        assert!(!door.enabled && !door.anyway && door.retry);
    }

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
        let f = facts(
            ProposalStatus::Pending,
            1,
            &approved_by,
            &execs,
            &execs,
            &previews,
            &offered,
        );
        let foot = foot(&f, None);
        assert_eq!(foot.actor.as_deref(), Some(DANA));
        let FootKind::Execute {
            look,
            approve_first,
            can_approve_only,
            one_tap,
        } = foot.kind
        else {
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
        let open = ready_door(json!({"sign": {"allowed": true, "anyway": false}}));
        assert!(execute_enabled(
            &open,
            look.as_ref(),
            approve_first,
            one_tap
        ));
        assert_eq!(
            execute_label(approve_first, press_rule(&open)),
            "proposal.approveAndExecute"
        );
    }

    /// No look yet, or a one press the machine does not offer: the press
    /// waits (never "Approve and execute" from a guess).
    #[test]
    fn no_one_press_before_the_look_or_the_offer() {
        let allowed = ready_door(json!({"sign": {"allowed": true, "anyway": false}}));
        assert!(!execute_enabled(&allowed, None, false, false));
        assert!(!execute_enabled(
            &allowed,
            Some(&Look::Unchecked),
            true,
            false
        ));
        assert!(execute_enabled(
            &allowed,
            Some(&Look::Unchecked),
            true,
            true
        ));
        // The review refuses: nothing at all.
        let refused = ready_door(json!({"sign": {"allowed": false, "anyway": false}}));
        assert!(!execute_enabled(
            &refused,
            Some(&Look::Unchecked),
            false,
            false
        ));
        // No review yet.
        let unread = sign_door(None, None, true);
        assert!(!execute_enabled(
            &unread,
            Some(&Look::Unchecked),
            false,
            false
        ));
    }

    fn ready_door(review: Value) -> SignDoor {
        sign_door(Some(&ReviewState::Ready), Some(&review), true)
    }

    /// D1 (real-device pass 2): a review neither of whose safety checks was
    /// read is drawn under the danger line with Try again, and every press —
    /// Approve, Approve and execute (the one press), Execute — says
    /// "… anyway". One check unread alone stays plain.
    #[test]
    fn a_review_nothing_checked_is_drawn_in_danger_and_every_press_is_anyway() {
        let door = ready_door(json!({
            "sign": {"allowed": true, "anyway": true},
            "concerns": [{"kind": "stateUnread", "reason": "unreachable"}, {"kind": "unsimulated"}, {"kind": "unchecked"}]
        }));
        assert!(unchecked(&door) && door.retry);
        assert_eq!(
            door.reason_key.as_deref(),
            Some(review_state::UNCHECKED_KEY)
        );
        let rule = press_rule(&door);
        assert!(rule.allowed && rule.anyway);
        assert_eq!(execute_label(true, rule), "review.approveExecuteAnyway");
        assert_eq!(execute_label(false, rule), "review.executeAnyway");
        assert_eq!(approve_label(rule), "review.approveAnyway");
        assert!(execute_enabled(&door, Some(&Look::Unchecked), true, true));
        let one = ready_door(json!({
            "sign": {"allowed": true, "anyway": false},
            "concerns": [{"kind": "unsimulated"}]
        }));
        assert!(!unchecked(&one) && !one.retry);
        assert_eq!(
            execute_label(true, press_rule(&one)),
            "proposal.approveAndExecute"
        );
    }

    /// §12: the shared door vectors (`tests/vectors/multisig/review-state.json`)
    /// through the door this sheet's presses follow — the core's one rule —
    /// and through the core's link door (`review.door`).
    #[test]
    fn the_shared_door_vectors_open_the_sheets_presses() {
        let file: Value = serde_json::from_str(include_str!(
            "../../../../rust/crates/vela-core/tests/vectors/multisig/review-state.json"
        ))
        .expect("the vectors");
        let cases = file["door"].as_array().expect("door cases");
        assert!(cases.len() >= 10, "{}", cases.len());
        for case in cases {
            let name = case["name"].as_str().unwrap_or_default();
            let args = &case["args"];
            let state: Option<ReviewState> = args
                .get("state")
                .filter(|s| !s.is_null())
                .map(|s| serde_json::from_value(s.clone()).expect("a state"));
            let review = args.get("review").filter(|r| !r.is_null());
            let door = sign_door(state.as_ref(), review, true);
            assert_eq!(
                serde_json::to_value(&door).expect("a door"),
                case["expect"],
                "{name}"
            );
            // The press follows the door, and nothing else.
            assert_eq!(
                execute_enabled(&door, Some(&Look::Unchecked), false, false),
                door.enabled,
                "{name}"
            );
            assert_eq!(press_rule(&door).anyway, door.anyway, "{name}");
            let out = review_state::door_json(args).expect("review.door");
            assert_eq!(out, case["expect"], "{name}: the link door");
        }
    }

    /// r17-8 (seen once, zh): during a press with Arc refused, the sheet
    /// briefly showed enabled "仍然批准 / 仍然批准并执行" under "Vela 未能
    /// 检查这笔交易的结果". Nothing is signable transiently: a review kept
    /// from before (one that allowed signing, even "… anyway") opens no
    /// press while the core says the review is being read again or failed —
    /// the door is the core's state AND rule, never the kept review alone,
    /// and a shut door's press is never worded "… anyway".
    #[test]
    fn a_kept_review_opens_nothing_while_the_review_is_loading_or_failed() {
        let kept = json!({
            "sign": {"allowed": true, "anyway": true},
            "reason_key": "home.multisig.review.unchecked",
        });
        for state in [
            ReviewState::Loading,
            ReviewState::failed(review_state::REASON_UNREADABLE),
        ] {
            let door = sign_door(Some(&state), Some(&kept), true);
            assert!(!door.enabled, "{state:?}");
            assert!(!door.anyway, "{state:?}");
            let sign = press_rule(&door);
            assert!(!sign.allowed && !sign.anyway, "{state:?}");
            assert_eq!(approve_label(sign), "proposal.approve");
            assert_eq!(execute_label(true, sign), "proposal.approveAndExecute");
            for one_tap in [false, true] {
                for look in [None, Some(Look::Unchecked), Some(Look::Unpriced)] {
                    assert!(
                        !execute_enabled(&door, look.as_ref(), true, one_tap),
                        "{state:?}"
                    );
                    assert!(!execute_enabled(&door, look.as_ref(), false, one_tap));
                }
            }
        }
        // Not asked yet (no state): shut too.
        assert!(!sign_door(None, Some(&kept), true).enabled);
        // Read: the rule decides — here, "… anyway".
        let ready = sign_door(Some(&ReviewState::Ready), Some(&kept), true);
        assert!(ready.enabled && ready.anyway);
    }

    /// A review this sheet cannot draw is never signed on: a `ready` with a
    /// review that would allow it, undrawable, is shut and said unreadable.
    #[test]
    fn a_review_the_sheet_cannot_draw_opens_nothing() {
        let review = json!({"sign": {"allowed": true, "anyway": false}});
        let door = sign_door(Some(&ReviewState::Ready), Some(&review), false);
        assert!(!door.enabled && door.retry);
        assert_eq!(
            door.reason_key.as_deref(),
            Some(review_state::REASON_UNREADABLE)
        );
        assert!(sign_door(Some(&ReviewState::Ready), Some(&review), true).enabled);
    }

    /// Loading: shut. Failed: shut, with "Try again" — the machine's
    /// `review_again` for that proposal (or the draft). Ready after it:
    /// open. Only a read (or never asked) review is asked again on the
    /// sheet's own timer.
    #[test]
    fn loading_and_failed_are_shut_and_try_again_reads_it_again() {
        let review = json!({"sign": {"allowed": true, "anyway": false}});
        let loading = sign_door(Some(&ReviewState::Loading), Some(&review), true);
        assert!(!loading.enabled && !loading.retry);
        let failed = ReviewState::failed(review_state::REASON_TIMED_OUT);
        let shut = sign_door(Some(&failed), Some(&review), true);
        assert!(!shut.enabled && shut.retry);
        assert!(!asks_review_again(Some(&failed)));
        assert!(!asks_review_again(Some(&ReviewState::Loading)));
        assert!(asks_review_again(Some(&ReviewState::Ready)));
        assert!(asks_review_again(None));
        assert!(matches!(
            review_again(Some((100, HASH))),
            ms_proposals::Event::ReviewAgain {
                chain_id: Some(100),
                safe_tx_hash: Some(ref h),
            } if h == HASH
        ));
        assert!(matches!(
            review_again(None),
            ms_proposals::Event::ReviewAgain {
                chain_id: None,
                safe_tx_hash: None
            }
        ));
        assert!(sign_door(Some(&ReviewState::Ready), Some(&review), true).enabled);
        // The machine's keys, whatever the hash's case.
        let mut states = BTreeMap::new();
        states.insert(format!("100:{}", HASH.to_ascii_lowercase()), failed.clone());
        assert_eq!(state_of(&states, 100, HASH), Some(&failed));
        assert_eq!(state_of(&states, 8453, HASH), None);
    }

    /// r8-1 (the shell's half): the review's `anyway` is the press's words.
    #[test]
    fn the_review_rule_names_the_press() {
        let anyway = SignRule {
            allowed: true,
            anyway: true,
        };
        assert_eq!(execute_label(true, anyway), "review.approveExecuteAnyway");
        assert_eq!(execute_label(false, anyway), "review.executeAnyway");
        assert_eq!(approve_label(anyway), "review.approveAnyway");
        assert_eq!(
            approve_label(SignRule {
                allowed: true,
                anyway: false
            }),
            "proposal.approve"
        );
    }

    /// The executor that can pay is picked over the first one; the
    /// person's pick over both.
    #[test]
    fn the_payer_is_picked_unless_the_person_picks() {
        let execs = vec![ALICE.to_owned(), DANA.to_owned()];
        let mut previews = BTreeMap::new();
        previews.insert(
            look_key(5_042_002, HASH, ALICE),
            json!({"kind": "priced", "fee": {"amount": "10"}, "canPay": false}),
        );
        previews.insert(
            look_key(5_042_002, HASH, DANA),
            json!({"kind": "priced", "fee": {"amount": "10"}, "canPay": true}),
        );
        let none = BTreeMap::new();
        let approved_by = vec![ALICE.to_owned(), DANA.to_owned()];
        let f = facts(
            ProposalStatus::Ready,
            2,
            &approved_by,
            &[],
            &execs,
            &previews,
            &none,
        );
        assert_eq!(foot(&f, None).actor.as_deref(), Some(DANA));
        assert_eq!(foot(&f, Some(ALICE)).actor.as_deref(), Some(ALICE));
        let FootKind::Execute {
            look,
            approve_first,
            ..
        } = foot(&f, Some(ALICE)).kind
        else {
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
        let f = facts(
            ProposalStatus::Pending,
            0,
            &[],
            &approvers,
            &[],
            &none,
            &nobody,
        );
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
        let odd = ItemSigners {
            outside: vec![DANA.into(), EVE.into()],
            ..signers.clone()
        };
        assert_eq!(member_way(Some(&odd), DANA, true), MemberWay::Waiting);
        assert_eq!(
            outside_members(Some(&odd), &|a| same(a, DANA)),
            vec![EVE.to_owned()]
        );
        // Enough approved: nobody is asked.
        let enough = ItemSigners {
            enough: true,
            ..signers
        };
        assert_eq!(member_way(Some(&enough), EVE, false), MemberWay::Waiting);
        assert!(outside_members(Some(&enough), &|_| false).is_empty());
    }

    /// r8-13: "Get it ready" while just created here, until it is set up.
    #[test]
    fn get_ready_until_it_is_set_up() {
        assert!(get_ready_shown(Some(true), true, false));
        assert!(
            get_ready_shown(Some(true), false, true),
            "set_up is the followed one's"
        );
        assert!(!get_ready_shown(Some(true), true, true));
        assert!(!get_ready_shown(None, true, false));
    }

    /// r8-13: an executed sheet counts what counted when it ran — the
    /// executor's approval by sending it too — not the list after.
    #[test]
    fn an_executed_one_counts_what_counted_when_it_ran() {
        let frozen: vela_core::app::multisig_presence::FrozenFacts =
            serde_json::from_value(json!({
                "executor": DANA,
                "threshold": 2,
                "approvers": [ALICE, DANA],
                "executorApproved": true,
                "source": "chain",
                "atMs": null
            }))
            .expect("frozen facts");
        assert_eq!(approvals_shown(true, Some(&frozen), 1, 2), (2, 2));
        assert_eq!(
            approvals_shown(false, Some(&frozen), 1, 2),
            (1, 2),
            "open: the list's own"
        );
        assert_eq!(approvals_shown(true, None, 1, 2), (1, 2));
        assert_eq!(approvals_shown(false, None, 3, 2), (2, 2));
    }

    /// r16-1: a press's failure is said in the sheet's pinned foot, right
    /// under the presses — never in the scrolling body, where the fee-moved
    /// stop sat under Approvals, off screen at the press. Only a sheet with
    /// no foot (done) says it at its head.
    #[test]
    fn a_press_failure_is_said_in_the_foot_beside_the_presses() {
        assert_eq!(failure_place(ProposalStatus::Pending), FailurePlace::Foot);
        assert_eq!(failure_place(ProposalStatus::Ready), FailurePlace::Foot);
        assert_eq!(failure_place(ProposalStatus::Queued), FailurePlace::Foot);
        assert_eq!(failure_place(ProposalStatus::Done), FailurePlace::Head);
        // Beside the presses when one is drawn; else at the foot's top.
        let priced = Some(Look::Priced {
            fee: "1".into(),
            can_pay: true,
            balance: None,
        });
        let execute = |look: Option<Look>, approve_first, can_approve_only| FootKind::Execute {
            look,
            approve_first,
            can_approve_only,
            one_tap: approve_first,
        };
        assert!(presses_drawn(&FootKind::Approve, true));
        assert!(
            !presses_drawn(&FootKind::Approve, false),
            "no actor, no press"
        );
        assert!(presses_drawn(&execute(priced, true, true), true));
        assert!(
            presses_drawn(&execute(None, false, false), true),
            "still checking: Execute, shut"
        );
        // Blocked: Approve alone stands, when it can — never over a spent number.
        assert!(!presses_drawn(
            &execute(Some(Look::Spent), true, true),
            true
        ));
        assert!(presses_drawn(
            &execute(Some(Look::Fails(Value::Null)), true, true),
            true
        ));
        assert!(!presses_drawn(
            &execute(Some(Look::Fails(Value::Null)), false, false),
            true
        ));
        assert!(!presses_drawn(&FootKind::Nothing, true));
    }

    /// r16-1: the words are read from the view at every render — the stop's
    /// figure is whatever the core's `fee_moved` detail says now (the core
    /// keeps it on the executor's current look), never the figure of the
    /// moment it failed — and one failure both presses carry is said once.
    #[test]
    fn a_press_failure_is_worded_from_the_view_at_each_render() {
        use vela_core::app::multisig_proposals::{ActionState, fee_moved_detail};
        let loc = crate::loc::Loc::for_tag("en");
        let words = |state: &ActionState| match state {
            ActionState::Failed { detail } => Some(crate::wallet::page::multisig::failure_words(
                &loc,
                detail,
                100,
                &|_| "Parallel One".to_owned(),
            )),
            _ => None,
        };
        let key = |press: &str| format!("{press}:{}", HASH.to_ascii_lowercase());
        let mut actions = BTreeMap::new();
        actions.insert(
            key("execute"),
            ActionState::Failed {
                detail: fee_moved_detail(100, DANA, 99_144_000_000_000_000),
            },
        );
        let lines = press_failure_lines(&actions, HASH, words);
        assert_eq!(lines.len(), 1, "{lines:?}");
        assert!(lines[0].contains("0.099144"), "{}", lines[0]);
        assert_eq!(lines[0].matches("That didn't go through").count(), 1);
        // The core moved the stop's figure with the look: the next render
        // says the new one, never the old.
        actions.insert(
            key("execute"),
            ActionState::Failed {
                detail: fee_moved_detail(100, DANA, 119_750_000_000_000_000),
            },
        );
        let lines = press_failure_lines(&actions, HASH, words);
        assert!(lines[0].contains("0.11975"), "{}", lines[0]);
        assert!(!lines[0].contains("0.099144"), "{}", lines[0]);
        // Approve (the person's own press) and Execute failed alike: said
        // once. (The one press never fails both: it writes `execute:` only,
        // and approves nothing unless it executes — §26 r16-4.)
        let failed = actions[&key("execute")].clone();
        actions.insert(key("approve"), failed);
        assert_eq!(press_failure_lines(&actions, HASH, words).len(), 1);
        // Working: nothing.
        actions.clear();
        actions.insert(key("execute"), ActionState::Working);
        assert!(press_failure_lines(&actions, HASH, words).is_empty());
    }

    /// r16-19: a sheet opened at launch, before the list holding its
    /// transaction was read, asked for the review at once; the core found
    /// nothing to review and landed it `failed {gone}` ("This transaction is
    /// no longer in the list.") — and a failed review is read again only on
    /// the person's Try again (r15-13). The review is asked only once the
    /// machine holds the transaction; until then the sheet says Opening….
    #[test]
    fn the_review_is_asked_only_once_the_transaction_is_listed() {
        const EVERY: f64 = 30_000.;
        let now = 100_000.;
        assert!(
            !asks_review_now(false, None, None, now, EVERY),
            "not listed yet"
        );
        assert!(
            asks_review_now(true, None, None, now, EVERY),
            "listed: asked at once"
        );
        assert!(
            !asks_review_now(true, None, Some(now - 1_000.), now, EVERY),
            "asked a moment ago"
        );
        assert!(asks_review_now(
            true,
            Some(&ReviewState::Ready),
            Some(now - EVERY),
            now,
            EVERY
        ));
        let gone = ReviewState::failed(vela_core::app::multisig_review_state::REASON_GONE);
        assert!(
            !asks_review_now(true, Some(&gone), None, now, EVERY),
            "a failed review waits for Try again"
        );
        assert!(!asks_review_now(
            true,
            Some(&ReviewState::Loading),
            None,
            now,
            EVERY
        ));
    }
}
