//! Machine — the default transaction speed (spec `068-fee-refresh-and-speed`).
//!
//! ```text
//! refresh ─► LoadingStored ─┬─ an offered name ─► commit {tier, committed}
//!                           └─ absent / anything else ─► the factory default
//! user_chose ─► persist + commit at once
//! ```
//!
//! Shaped on [`super::display_currency`], deliberately: it is this codebase's
//! committed-preference machine, and a second pattern for the same job would
//! be a second set of rules to keep in step. What is simpler here is that a
//! tier needs no second half — a currency is worthless without its rate, a
//! tier is just a name — so the choice commits and persists in one step and
//! there is no seed to race.
//!
//! Three rules paid for the branches below.
//!
//! **The factory default is [`FeeTier::Fast`], not the enum's first variant.**
//! Every shell hard-coded `fast` before this machine existed, so a fresh
//! install — and any device whose storage cannot be read — behaves exactly as
//! it did. A preference that cannot be read is "never chose", never "chose
//! slow": an unreadable key must not make somebody's transaction quietly
//! cheaper and slower than the one they sent yesterday.
//!
//! **A stored name is validated, never trusted.** Storage is a string, and a
//! string that is not one of the three offered names is not a tier. It reads
//! as absent, which is the factory default — the same refusal shape as
//! `display_currency`'s ISO guard.
//!
//! **A per-transaction choice never comes here.** The send screen's picker is
//! one-shot by design (spec 068: "a setting that silently drifts is a setting
//! nobody can trust"), so it changes what one send is priced and submitted at
//! and nothing else. Only Settings dispatches [`Event::UserChose`].
//!
//! The shell owns the storage key (`vela.feeTier`, under the `vela.` prefix
//! that survives sign-out — a speed preference belongs to the person and the
//! device, not to the account) and the words on screen; the core decides what
//! may be stored, and what shows when nothing can be.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

use super::fee_policy::FeeTier;

// ---------------------------------------------------------------------------
// What is offered
// ---------------------------------------------------------------------------

/// The factory default. `fast` because that is what every shell hard-coded
/// before this preference existed — so this feature cannot regress a send
/// nobody asked it to change.
pub const FACTORY_DEFAULT: FeeTier = FeeTier::Fast;

/// The tiers a person may choose, fastest first, in the order the picker
/// draws them.
///
/// [`FeeTier::Rapid`] is deliberately absent. It has a multiplier and a
/// translation, but nothing anywhere constructs it and the relay has never
/// reported it — it is a dead fourth tier (spec 068, "`rapid` is a dead fourth
/// tier"). The owner's ruling is to keep the variant (deleting it would churn
/// the generated enum, 15 locale files and the path-count pins for nothing a
/// person can see) and never offer it, never put it on the wire. If you came
/// here to "finish" it: don't — the relay refuses an unknown tier name with
/// -32602 before any handler runs.
pub const OFFERED: [FeeTier; 3] = [FeeTier::Fast, FeeTier::Standard, FeeTier::Slow];

/// The wire name of a tier — the same token the relay accepts as
/// `eth_sendUserOperation`'s optional third parameter, and the same token
/// [`parse_stored`] reads back.
pub fn tier_name(tier: FeeTier) -> &'static str {
    match tier {
        FeeTier::Slow => "slow",
        FeeTier::Standard => "standard",
        FeeTier::Rapid => "rapid",
        FeeTier::Fast => "fast",
    }
}

/// A stored string as a tier, or `None` when it is not one of the three
/// offered names.
///
/// `None` means "nothing was chosen" and resolves to [`FACTORY_DEFAULT`].
/// "rapid" lands here too: it is a name this build will never offer, so a key
/// somehow holding it reads as unset rather than putting a tier on the wire
/// that the relay refuses.
pub fn parse_stored(raw: &str) -> Option<FeeTier> {
    OFFERED
        .iter()
        .copied()
        .find(|tier| tier_name(*tier) == raw.trim())
}

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Sentences, not I/O.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeeTierPrefOperation"))]
pub enum FeeTierPrefOperation {
    /// Read `vela.feeTier`. Absent ALWAYS means "the person never chose".
    ReadStoredTier,
    /// Persist a choice (best effort — a storage that refuses does not undo
    /// what is on screen; the next launch simply reads the old value).
    WriteStoredTier { tier: String },
}

/// What the shell observed. The stored value arrives as the RAW string,
/// because judging it is the core's job (an unknown name is not a tier).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(
    feature = "bindings",
    derive(TS),
    ts(rename = "FeeTierPrefShellResult")
)]
pub enum FeeTierPrefShellResult {
    StoredTier { raw: Option<String> },
    TierWritten,
}

impl Operation for FeeTierPrefOperation {
    type Output = FeeTierPrefShellResult;
}

#[effect]
pub enum FeeTierPrefEffect {
    Render(RenderOperation),
    Shell(FeeTierPrefOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "FeeTierPrefEvent"))]
pub enum Event {
    /// First mount / screen focus — re-read the preference. Cheap by design:
    /// a second `refresh` while one is in flight is coalesced, so no surface
    /// can double-commit.
    Refresh,
    /// An explicit pick in Settings — and ONLY in Settings. The send screen's
    /// per-transaction picker must never dispatch this: that choice is
    /// one-shot and the next send returns to the default.
    UserChose { tier: FeeTier },
    /// Internal: an effect resolved. `attempt` is captured when the request is
    /// made; a result carrying an older attempt belongs to a superseded run
    /// and is dropped — which is how a slow read cannot land on top of a pick
    /// the person made while it was in flight.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: FeeTierPrefShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    #[default]
    Idle,
    LoadingStored,
}

#[derive(Default)]
pub struct Model {
    /// `None` ⇒ nothing has been read or chosen yet; the view shows the
    /// factory default and says it is not committed.
    committed: Option<FeeTier>,
    phase: Phase,
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct FeeTierPrefView {
    /// The tier every send should START at. Always a real tier — there is no
    /// "unknown speed" to render, and a surface must never have to invent one.
    pub tier: FeeTier,
    /// `false` ⇒ `tier` is the factory default because nothing was stored (or
    /// nothing could be read), not because anybody chose it. Settings uses
    /// this to avoid claiming a choice the person never made.
    pub committed: bool,
    /// The tiers the picker may offer, fastest first. Published so no shell
    /// has to keep its own copy of the list and drift from the one the core
    /// validates against — and so the dead `rapid` variant can never reach a
    /// screen by being enumerated from the enum.
    pub offered: Vec<FeeTier>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct FeeTierPref;

impl App for FeeTierPref {
    type Event = Event;
    type Model = Model;
    type ViewModel = FeeTierPrefView;
    type Effect = FeeTierPrefEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<FeeTierPrefEffect, Event> {
        match event {
            Event::Refresh => {
                // Coalesce, as `display_currency` does: a focus refresh while
                // the first read is in flight must not supersede it. The read
                // in flight commits anyway, which is what the refresher wanted.
                if model.phase != Phase::Idle {
                    return Command::done();
                }
                model.attempt += 1;
                model.phase = Phase::LoadingStored;
                let attempt = model.attempt;
                Command::all([
                    Command::request_from_shell(FeeTierPrefOperation::ReadStoredTier)
                        .then_send(move |result| Event::ShellCompleted { attempt, result }),
                    render(),
                ])
            }
            Event::UserChose { tier } => {
                // Commit and persist together. There is no second half to wait
                // for — unlike a currency, a tier carries no rate — so the row
                // the person just tapped is the row that is selected, with no
                // frame in between showing the old one.
                model.attempt += 1;
                model.phase = Phase::Idle;
                model.committed = Some(tier);
                let attempt = model.attempt;
                Command::all([
                    Command::request_from_shell(FeeTierPrefOperation::WriteStoredTier {
                        tier: tier_name(tier).to_owned(),
                    })
                    .then_send(move |result| Event::ShellCompleted { attempt, result }),
                    render(),
                ])
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A superseded run — most importantly, a stored value that
                    // arrived after the person picked something in Settings.
                    // Dropping it IS the "their choice wins" rule.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> FeeTierPrefView {
        FeeTierPrefView {
            tier: model.committed.unwrap_or(FACTORY_DEFAULT),
            committed: model.committed.is_some(),
            offered: OFFERED.to_vec(),
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: FeeTierPrefShellResult) -> Command<FeeTierPrefEffect, Event> {
    match (model.phase, result) {
        (Phase::LoadingStored, FeeTierPrefShellResult::StoredTier { raw }) => {
            model.phase = Phase::Idle;
            // An unparseable name is not a tier, and not an error either: it
            // reads as "never chose", so the view stays at the factory default
            // and nothing is written back. Rewriting storage here would turn a
            // value this build does not understand into a value it does —
            // which is how a newer build's preference gets silently erased by
            // an older one.
            model.committed = raw.as_deref().and_then(parse_stored);
            render()
        }
        // The best-effort write acknowledged, or a result for a phase that no
        // longer expects it. Neither may change what is on screen.
        _ => Command::done(),
    }
}

impl super::SplitEffect for FeeTierPrefEffect {
    type Op = FeeTierPrefOperation;
    fn into_shell(self) -> Option<crux_core::Request<FeeTierPrefOperation>> {
        match self {
            FeeTierPrefEffect::Render(_) => None,
            FeeTierPrefEffect::Shell(request) => Some(request),
        }
    }
}
