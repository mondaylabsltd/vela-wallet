//! One way in for the person's UI and an AI agent (spec 103), the desktop's
//! runner — the web's `intents.svelte.ts`, step for step.
//!
//! Both ask for the same typed [`Action`]; who asks only changes the
//! authorization. A call goes:
//!
//! 1. **check** — the core's shared rules: may this asker ask for this, and is
//!    the input well formed (`intents::check`, the same on every shell);
//! 2. **examine** — does what it names exist, is it already so
//!    (`intent_rules::examine`, against the residents' views);
//! 3. **approve** — an agent's manage-level request waits for the person, in
//!    the wallet (on show in [`Book::approvals`]);
//! 4. **perform** — the same resident calls the UI makes (`perform.rs`);
//! 5. **observe** — `completed` only once the change is SEEN in the views; a
//!    step that ran is not yet a change that held.
//!
//! The runner reaches the wallet only through [`Deps`], so its own rules —
//! approvals, Stop, a call given up — are tested here without a window.

use std::collections::HashMap;

use serde::Serialize;
use vela_core::app::intent_rules::{self, Examined, Verdict, World};
use vela_core::app::intents::{
    self as core_intents, Action, Actor, Approver, Check, Decision, Operation, Reason, ReasonCode,
    Risk, Screen,
};

/// How an intent ended (ClientMCP SPEC §9.2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum Outcome {
    Completed,
    /// It started, and goes on as `operation` (a transfer, followed by its state).
    Accepted {
        operation: Operation,
        /// The finished transfer whose receipt drafting this one closed.
        #[serde(skip_serializing_if = "Option::is_none")]
        closed: Option<String>,
    },
    Rejected {
        reason: Reason,
    },
}

fn rejected(code: ReasonCode, detail: &str) -> Outcome {
    Outcome::Rejected {
        reason: Reason {
            code,
            detail: detail.to_owned(),
        },
    }
}

/// What doing it came to.
pub enum Performed {
    /// Every call was made: the change is for the runner to see.
    Done,
    /// Refused, in words (`refused`) or with its own code.
    Refused(Reason),
    /// It started an operation that goes on.
    Started {
        operation: Operation,
        closed: Option<String>,
    },
}

impl Performed {
    pub fn refused(detail: impl Into<String>) -> Self {
        Performed::Refused(Reason {
            code: ReasonCode::Refused,
            detail: detail.into(),
        })
    }
}

/// An agent's request, waiting for the person.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PendingApproval {
    pub id: u64,
    pub action: Action,
    pub risk: Risk,
    /// Epoch milliseconds.
    pub since: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Approving,
    Performing,
    Observing,
}

/// One of the agent's calls, for the person to see what it did.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ActivityEntry {
    pub id: u64,
    /// The action's `type`.
    pub action: String,
    pub stage: Option<Stage>,
    pub outcome: Option<Outcome>,
}

/// How many of the agent's recent calls are kept on show.
pub const ACTIVITY_KEPT: usize = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Answer {
    Approved,
    Denied,
    Stopped,
}

/// The agent's requests waiting for the person and its recent calls: what
/// the bar and the approval sheet show. Kept by the shell where its screens
/// can watch it (a gpui global on the desktop).
#[derive(Default)]
pub struct Book {
    /// Oldest first.
    pub approvals: Vec<PendingApproval>,
    /// Newest first.
    pub activity: Vec<ActivityEntry>,
    answers: HashMap<u64, Answer>,
    /// Bumped by [`Book::stop`]: an agent's call from an older epoch stops at
    /// its next step.
    epoch: u64,
    next: u64,
}

impl Book {
    /// The person answered an agent's request.
    pub fn answer(&mut self, id: u64, approved: bool) {
        if self.approvals.iter().any(|pending| pending.id == id) {
            self.answers.insert(
                id,
                if approved {
                    Answer::Approved
                } else {
                    Answer::Denied
                },
            );
        }
    }

    /// The person stopped the agent: nothing of its still waits.
    pub fn stop(&mut self) {
        self.epoch += 1;
        for pending in &self.approvals {
            self.answers.insert(pending.id, Answer::Stopped);
        }
    }

    fn log(&mut self, entry: ActivityEntry) {
        self.activity.insert(0, entry);
        self.activity.truncate(ACTIVITY_KEPT);
    }

    fn update(&mut self, id: u64, change: impl FnOnce(&mut ActivityEntry)) {
        if let Some(entry) = self.activity.iter_mut().find(|entry| entry.id == id) {
            change(entry);
        }
    }
}

/// What the runner needs of the wallet.
///
/// Implemented by the shells for their own concrete types, whose futures are
/// then known whole (`Send` included where the shell needs it).
#[allow(async_fn_in_trait)]
pub trait Deps {
    /// The core's shared check, for this asker, with the grants the wallet holds.
    fn check(&mut self, actor: Actor, action: &Action) -> Check;
    /// The residents' views, now.
    fn world(&mut self) -> World;
    /// The same calls the UI makes.
    async fn perform(
        &mut self,
        action: &Action,
        world: &World,
        now_ms: f64,
        actor: Actor,
    ) -> Performed;
    fn now_ms(&mut self) -> f64;
    async fn sleep(&mut self, ms: u64);
    /// Open `screen` for the person, as `show` does.
    async fn reveal(&mut self, screen: &Screen);
    /// The book, to read or change (and have its watchers told).
    fn book<R>(&mut self, change: impl FnOnce(&mut Book) -> R) -> R;
}

/// How long an agent's request may wait for the person: under the bridge's
/// own call timeout (ClientMCP's default is 120 s), so the wallet says how the
/// wait ended before the bridge gives up on it.
pub const APPROVAL_MS: f64 = 100_000.;
/// How long a performed change may take to show.
pub const OBSERVE_MS: f64 = 10_000.;
const POLL_MS: u64 = 50;

/// How an agent's call was given up before it ran: nothing was done.
const CANCELLED: &str = "the agent’s call was cancelled (its connection or the bridge gave up on it) before anything was done";

/// Ask for an action. Never fails: every end is an outcome.
///
/// `gone` is the caller giving up — an agent's call the bridge timed out or
/// its connection lost (`Sync` where the shell's runner must be `Send`). Before anything is done that ends the call: an
/// approval still waiting is withdrawn and nothing is performed, so an agent
/// told "timed out" never finds the change made after all. Once the step has
/// begun it is not undone; the person sees how it ended.
pub async fn invoke<G: Fn() -> bool + ?Sized>(
    deps: &mut impl Deps,
    actor: Actor,
    action: Action,
    gone: &G,
) -> Outcome {
    let agent = actor == Actor::Agent;
    let (id, epoch) = deps.book(|book| {
        book.next += 1;
        let id = book.next;
        if agent {
            book.log(ActivityEntry {
                id,
                action: action.kind().to_owned(),
                stage: None,
                outcome: None,
            });
        }
        (id, book.epoch)
    });
    let outcome = run(deps, id, actor, &action, epoch, gone).await;
    if agent {
        let ended = outcome.clone();
        deps.book(|book| {
            book.update(id, |entry| {
                entry.stage = None;
                entry.outcome = Some(ended);
            });
        });
        // What an agent did is seen where it is (owner, 2026-10-21: an
        // approved change that left the page as it was felt silent). A `show`
        // has opened its screen already.
        let done = matches!(outcome, Outcome::Completed | Outcome::Accepted { .. });
        if done
            && !matches!(action, Action::Show { .. })
            && let Some(screen) = core_intents::shows(&action)
        {
            deps.reveal(&screen).await;
        }
    }
    outcome
}

fn stage(deps: &mut impl Deps, id: u64, stage: Stage) {
    deps.book(|book| book.update(id, |entry| entry.stage = Some(stage)));
}

async fn run<G: Fn() -> bool + ?Sized>(
    deps: &mut impl Deps,
    id: u64,
    actor: Actor,
    action: &Action,
    epoch: u64,
    gone: &G,
) -> Outcome {
    let checked = deps.check(actor, action);
    if let Decision::Deny { reason } = checked.decision {
        return Outcome::Rejected { reason };
    }
    match intent_rules::examine(action, &deps.world()) {
        Examined::Refused { reason } => return Outcome::Rejected { reason },
        Examined::Already => return Outcome::Completed,
        Examined::Go => {}
    }

    if checked.decision
        == (Decision::Approve {
            by: Approver::Wallet,
        })
    {
        stage(deps, id, Stage::Approving);
        let risk = checked.risk.unwrap_or(Risk::Manage);
        match ask(deps, id, action, risk, gone).await {
            Asked::Cancelled => return rejected(ReasonCode::Stopped, CANCELLED),
            Asked::Answered(Answer::Denied) => {
                return rejected(ReasonCode::Denied, "the person did not approve it");
            }
            Asked::TimedOut => {
                return rejected(ReasonCode::ApprovalTimedOut, "nobody approved it in time");
            }
            Asked::Answered(Answer::Stopped) => {
                return rejected(
                    ReasonCode::Stopped,
                    "the person stopped the agent before approving",
                );
            }
            Asked::Answered(Answer::Approved) => {}
        }
        // An approval can take a while: the wallet may have changed under it.
        match intent_rules::examine(action, &deps.world()) {
            Examined::Refused { reason } => return Outcome::Rejected { reason },
            Examined::Already => return Outcome::Completed,
            Examined::Go => {}
        }
    }
    if stopped(deps, actor, epoch) {
        return rejected(ReasonCode::Stopped, "the person stopped the agent");
    }
    if gone() {
        return rejected(ReasonCode::Stopped, CANCELLED);
    }

    stage(deps, id, Stage::Performing);
    let before = deps.world();
    let mut baseline = intent_rules::baseline(action, &before);
    let now = deps.now_ms();
    match deps.perform(action, &before, now, actor).await {
        Performed::Done => {}
        Performed::Refused(reason) => return Outcome::Rejected { reason },
        Performed::Started { operation, closed } => return Outcome::Accepted { operation, closed },
    }

    stage(deps, id, Stage::Observing);
    // A sign-out ends on the session's own sheet: as long as an approval.
    let wait = if matches!(action, Action::SignOut) {
        APPROVAL_MS
    } else {
        OBSERVE_MS
    };
    let deadline = deps.now_ms() + wait;
    loop {
        match intent_rules::verdict(action, &deps.world(), &mut baseline) {
            Verdict::Held => return Outcome::Completed,
            Verdict::Refused { reason } => return Outcome::Rejected { reason },
            Verdict::NotYet => {}
        }
        if stopped(deps, actor, epoch) {
            return rejected(
                ReasonCode::Stopped,
                "the person stopped the agent; the step may already have taken effect",
            );
        }
        if deps.now_ms() >= deadline {
            return rejected(
                ReasonCode::NotObserved,
                "the change was asked for, but did not show in time",
            );
        }
        deps.sleep(POLL_MS).await;
    }
}

/// The person stopped the agent since its call began.
fn stopped(deps: &mut impl Deps, actor: Actor, epoch: u64) -> bool {
    actor == Actor::Agent && deps.book(|book| book.epoch) != epoch
}

enum Asked {
    Answered(Answer),
    TimedOut,
    Cancelled,
}

/// Put the request in front of the person and wait for their answer.
async fn ask<G: Fn() -> bool + ?Sized>(
    deps: &mut impl Deps,
    id: u64,
    action: &Action,
    risk: Risk,
    gone: &G,
) -> Asked {
    let since = deps.now_ms();
    deps.book(|book| {
        book.approvals.push(PendingApproval {
            id,
            action: action.clone(),
            risk,
            since,
        });
    });
    let asked = loop {
        if gone() {
            break Asked::Cancelled;
        }
        if let Some(answer) = deps.book(|book| book.answers.remove(&id)) {
            break Asked::Answered(answer);
        }
        if deps.now_ms() >= since + APPROVAL_MS {
            break Asked::TimedOut;
        }
        deps.sleep(POLL_MS).await;
    };
    deps.book(|book| {
        book.approvals.retain(|pending| pending.id != id);
        book.answers.remove(&id);
    });
    asked
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use futures::executor::block_on;
    use vela_core::app::intents;

    use super::*;

    /// A test's person, answering at each sleep.
    type Person = Box<dyn FnMut(&mut Book, f64)>;
    /// What a perform does to the world.
    type Effect = Box<dyn FnMut(&Action, &mut World) -> Performed>;

    /// A wallet with nothing in it but what a test puts there.
    struct Fake {
        world: World,
        book: Book,
        clock: f64,
        performed: Vec<String>,
        each_sleep: Person,
        effect: Effect,
        revealed: Vec<Screen>,
    }

    impl Fake {
        fn new() -> Fake {
            Fake {
                world: World {
                    agent_enabled: true,
                    ..World::default()
                },
                book: Book::default(),
                clock: 0.,
                performed: Vec::new(),
                each_sleep: Box::new(|_, _| {}),
                effect: Box::new(|_, _| Performed::Done),
                revealed: Vec::new(),
            }
        }
    }

    impl Deps for Fake {
        fn check(&mut self, actor: Actor, action: &Action) -> Check {
            intents::check(
                actor,
                action,
                &vela_core::app::intent_rules::grants(&self.world),
            )
        }
        fn world(&mut self) -> World {
            self.world.clone()
        }
        async fn perform(&mut self, action: &Action, _: &World, _: f64, _: Actor) -> Performed {
            self.performed.push(action.kind().to_owned());
            (self.effect)(action, &mut self.world)
        }
        fn now_ms(&mut self) -> f64 {
            self.clock
        }
        async fn sleep(&mut self, ms: u64) {
            self.clock += ms as f64;
            (self.each_sleep)(&mut self.book, self.clock);
        }
        async fn reveal(&mut self, screen: &Screen) {
            self.revealed.push(screen.clone());
        }
        fn book<R>(&mut self, change: impl FnOnce(&mut Book) -> R) -> R {
            change(&mut self.book)
        }
    }

    fn delete_group() -> Action {
        serde_json::from_value(serde_json::json!({ "type": "delete_group", "id": "g1" })).unwrap()
    }

    fn with_group(fake: &mut Fake) {
        fake.world = serde_json::from_value(serde_json::json!({
            "agent_enabled": true,
            "contacts": { "loaded": true, "contacts": [], "groups": [{ "id": "g1", "name": "Team" }] }
        }))
        .unwrap();
        fake.effect = Box::new(|action, world| {
            if matches!(action, Action::DeleteGroup { .. })
                && let Some(book) = world.contacts.as_mut()
            {
                book.groups.clear();
            }
            Performed::Done
        });
    }

    #[test]
    fn a_manage_request_waits_for_the_person_and_is_done_once_seen() {
        let mut fake = Fake::new();
        with_group(&mut fake);
        fake.each_sleep = Box::new(|book, _| {
            if let Some(id) = book.approvals.first().map(|pending| pending.id) {
                book.answer(id, true);
            }
        });
        let outcome = block_on(invoke(&mut fake, Actor::Agent, delete_group(), &|| false));
        assert_eq!(outcome, Outcome::Completed);
        assert_eq!(fake.performed, ["delete_group"]);
        assert!(fake.book.approvals.is_empty());
        assert_eq!(fake.book.activity[0].outcome, Some(Outcome::Completed));
        assert_eq!(fake.book.activity[0].stage, None);
        // And the person is shown where it happened.
        assert_eq!(fake.revealed, [Screen::Contacts]);
    }

    #[test]
    fn denied_timed_out_stopped_and_given_up_do_nothing() {
        let cases: [(Person, ReasonCode); 3] = [
            (
                Box::new(|book, _| {
                    if let Some(id) = book.approvals.first().map(|p| p.id) {
                        book.answer(id, false);
                    }
                }),
                ReasonCode::Denied,
            ),
            (Box::new(|_, _| {}), ReasonCode::ApprovalTimedOut),
            (Box::new(|book, _| book.stop()), ReasonCode::Stopped),
        ];
        for (person, code) in cases {
            let mut fake = Fake::new();
            with_group(&mut fake);
            fake.each_sleep = person;
            let Outcome::Rejected { reason } =
                block_on(invoke(&mut fake, Actor::Agent, delete_group(), &|| false))
            else {
                panic!("rejected");
            };
            assert_eq!(reason.code, code);
            assert!(fake.performed.is_empty(), "{code:?}: nothing was done");
            assert!(fake.revealed.is_empty(), "{code:?}: nothing to show");
            assert!(fake.book.approvals.is_empty());
        }

        // The bridge gave up on the call while it waited.
        let mut fake = Fake::new();
        with_group(&mut fake);
        let waited = Cell::new(0);
        let gone = || {
            waited.set(waited.get() + 1);
            waited.get() > 3
        };
        let Outcome::Rejected { reason } =
            block_on(invoke(&mut fake, Actor::Agent, delete_group(), &gone))
        else {
            panic!("rejected");
        };
        assert_eq!(reason.code, ReasonCode::Stopped);
        assert_eq!(reason.detail, CANCELLED);
        assert!(fake.performed.is_empty());
        assert!(fake.book.approvals.is_empty(), "the request is withdrawn");
    }

    #[test]
    fn the_person_is_never_asked_and_an_agent_switched_off_is_refused() {
        let mut fake = Fake::new();
        with_group(&mut fake);
        assert_eq!(
            block_on(invoke(&mut fake, Actor::Person, delete_group(), &|| false)),
            Outcome::Completed
        );
        assert!(
            fake.book.activity.is_empty(),
            "the person's own calls are not the agent's activity"
        );
        assert!(fake.revealed.is_empty(), "the person is where they did it");

        let mut fake = Fake::new();
        with_group(&mut fake);
        fake.world.agent_enabled = false;
        let Outcome::Rejected { reason } =
            block_on(invoke(&mut fake, Actor::Agent, delete_group(), &|| false))
        else {
            panic!("rejected");
        };
        assert_eq!(reason.code, ReasonCode::AgentOff);
        assert!(fake.performed.is_empty());
    }

    #[test]
    fn a_change_that_never_shows_is_not_observed() {
        let mut fake = Fake::new();
        with_group(&mut fake);
        fake.effect = Box::new(|_, _| Performed::Done);
        let Outcome::Rejected { reason } =
            block_on(invoke(&mut fake, Actor::Person, delete_group(), &|| false))
        else {
            panic!("rejected");
        };
        assert_eq!(reason.code, ReasonCode::NotObserved);
        assert!(fake.clock >= OBSERVE_MS);
    }

    #[test]
    fn what_does_not_exist_is_refused_before_anyone_is_asked() {
        let mut fake = Fake::new();
        fake.world = serde_json::from_value(serde_json::json!({
            "agent_enabled": true,
            "contacts": { "loaded": true, "contacts": [], "groups": [] }
        }))
        .unwrap();
        let Outcome::Rejected { reason } =
            block_on(invoke(&mut fake, Actor::Agent, delete_group(), &|| false))
        else {
            panic!("rejected");
        };
        assert_eq!(reason.code, ReasonCode::NotFound);
        assert!(fake.performed.is_empty());
        assert!(fake.book.approvals.is_empty(), "nothing to approve");
    }
}
