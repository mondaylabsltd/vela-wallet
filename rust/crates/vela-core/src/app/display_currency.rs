//! Machine — the display currency (spec `016-crux-wallet-state`, US1).
//!
//! ```text
//! refresh ─► LoadingStored ─┬─ stored code ─► ResolvingDisplay ─► commit {code, rate?}
//!                           └─ absent ─► ReadingDevice ─┬─ none/USD ─► commit {USD, 1}
//!                                                       └─ candidate ─► ResolvingSeed
//!                                    rate? ─► RecheckingStored ─► persist + commit
//! ```
//!
//! One rule bought every branch here. The pair is committed **atomically**
//! because flipping the code while the old rate is still applied renders a
//! wrong-magnitude balance for a frame (¥12 instead of ¥1,860). The seed is
//! persisted **only after a real rate resolves** because a seeded currency
//! rendering at the rate-1 fallback (₫78 instead of ₫2,000,000) is strictly
//! worse than staying on USD. And an explicit user choice **always wins** over
//! the async seed, because the seed's rate fetch takes real network time in
//! which the user may have picked something in Settings.
//!
//! The shell owns the rate *sources* (Chainlink → FX endpoint), the currency
//! catalog (names/symbols), formatting, and the storage key — the core only
//! decides what may be paired, persisted and shown.
//!
//! # The withhold rule: no fiat figure before the currency commits
//!
//! Until [`CurrencyView::committed`] is true the view is the USD/1
//! placeholder, which is not the person's currency. **While it is false no
//! fiat figure is drawn — anywhere.** Not "$1,234" that becomes "¥8,876" a
//! few seconds later (the iPhone home, the 102 device run), and not a "$"
//! on one screen while another waits: the rule has no surface it skips.
//!
//! Every surface that draws money in the display currency
//! ([`FIAT_SURFACES`]):
//!
//! - the home total, the holdings' worth, the account switcher;
//! - the token page (its worth, its price), the Assets page;
//! - the balance detail sheet, its unpriced and unreachable lists;
//! - an activity row's fiat, a transfer's and a dApp row's detail;
//! - Send: the coin list, the form's "≈" line under the amount, the review;
//! - the signing sheet: the fee's fiat and every balance change's worth;
//! - Settings: the account's total.
//!
//! What is withheld is the FIAT figure only. A token amount ("0.5 ETH") is
//! not in the display currency and is drawn as always — so Send and the
//! signing sheet stay decidable while the currency is on its way.
//!
//! **Withholding never moves the layout.** A withheld figure keeps the room
//! the figure will take (a skeleton of its line, or an empty line of its
//! height); the figure then lands in place. A row that grows when its "≈"
//! line arrives is the same jump the rule exists to remove.
//!
//! The wait is bounded: every path through the machine ends in a commit —
//! the stored code with its rate (or with none, which formats as USD), or
//! USD itself.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Sentences, not I/O: the shell
/// decides *how* to read a preference or price a currency.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "CurrencyOperation"))]
pub enum CurrencyOperation {
    /// Read `vela.displayCurrency`. Absent ALWAYS means "the user never chose".
    ReadStoredCode,
    /// Persist a choice (best effort — the shell swallows storage errors,
    /// matching today's `setCurrency`).
    WriteStoredCode { code: String },
    /// The device region's currency, from the primary locale only. `None` on
    /// web and for regionless locales.
    ReadDeviceCurrency,
    /// Price one currency: USD→code multiplier, or `None` when no source can
    /// price it right now. `None` is NOT 1 — the seed decision depends on the
    /// difference (research.md D5 / FR-007).
    ResolveRate { code: String },
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "CurrencyShellResult"))]
pub enum CurrencyShellResult {
    StoredCode { code: Option<String> },
    CodeWritten,
    DeviceCurrency { code: Option<String> },
    RateResolved { code: String, rate: Option<f64> },
}

impl Operation for CurrencyOperation {
    type Output = CurrencyShellResult;
}

#[effect]
pub enum CurrencyEffect {
    Render(RenderOperation),
    Shell(CurrencyOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "CurrencyEvent"))]
pub enum Event {
    /// Screen focus / first mount — re-read the preference. Cheap by design:
    /// a second `refresh` while one is in flight supersedes it (attempt bump),
    /// so no screen can double-commit.
    Refresh,
    /// An explicit pick in Settings. Persists immediately and makes every
    /// in-flight seed result stale — the enforcement of "user choice wins".
    UserChose { code: String },
    /// Internal: an effect resolved. `attempt` is captured by the core when
    /// the request is made; a result carrying an older attempt belongs to a
    /// superseded run and is dropped.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: CurrencyShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

/// A display currency and the multiplier that prices it — inseparable.
///
/// `rate: None` is a currency the shell is SHOWING but could not price. It is
/// not rate 1: a rate of 1 is a claim (1 USD = 1 CNY), and the difference is
/// the whole reason `ResolveRate` answers `Option<f64>` instead of a number.
/// A surface may still render in `code` — degrading the fiat figure it prints
/// is a cosmetic loss — but nothing may CONVERT with a `None`, because the
/// conversion is what moves money (`send`'s fiat-denominated amount input
/// divides by exactly this multiplier).
#[derive(Clone, Debug, PartialEq)]
pub struct Pair {
    pub code: String,
    /// USD → `code`. `None` ⇒ no source could price `code`; never 1-by-default.
    pub rate: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq)]
enum Phase {
    #[default]
    Idle,
    LoadingStored,
    /// Pricing a KNOWN preference. Unpriceable commits `{code, None}` — the
    /// currency still shows, but nothing may convert with it.
    ResolvingDisplay {
        code: String,
    },
    ReadingDevice,
    /// Pricing a seed CANDIDATE. Strict: no rate, no seed.
    ResolvingSeed {
        candidate: String,
    },
    /// The seed's rate resolved; re-read storage because the user may have
    /// picked a currency during the fetch. Their choice wins.
    RecheckingStored {
        candidate: String,
        rate: f64,
    },
}

/// Every surface the withhold rule covers (the module doc): where a figure
/// in the display currency is drawn. A shell's own list of the places it
/// formats fiat is held to this one by its tests, so a new surface is added
/// here first — the rule was once applied to five of these and missed the
/// send form, the signing fee and the balance detail sheet.
pub const FIAT_SURFACES: [&str; 12] = [
    "home_total",
    "holdings",
    "account_switcher",
    "token_detail",
    "assets",
    "balance_detail",
    "activity_row",
    "activity_detail",
    "send_coin_list",
    "send_form",
    "signing_sheet",
    "settings_total",
];

#[derive(Default)]
pub struct Model {
    /// The ONLY pair any surface may render. `None` ⇒ USD/1 placeholder —
    /// never a stored code at rate 1.
    committed: Option<Pair>,
    phase: Phase,
    /// The device was already probed this session; don't re-probe on every
    /// focus (matches today's single-flight `_seedPromise`).
    seed_attempted: bool,
    attempt: u64,
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct CurrencyView {
    pub code: String,
    /// USD → `code`, or `null` when no source could price `code` right now.
    ///
    /// `null` is NOT 1. Every shell-side use splits on it: formatting may
    /// degrade (show the USD figure rather than label an unconverted number
    /// with a ¥), converting may not — a fiat-denominated amount multiplied by
    /// a defaulted 1 is a real 7x mispayment, the single-send twin of the
    /// batch importer's refusal to quote a rate it cannot vouch for.
    pub rate: Option<f64>,
    /// `false` ⇒ the USD/1 placeholder is showing. The shell derives the
    /// symbol from its catalog and owns all formatting.
    ///
    /// **While `false`, no fiat figure is drawn — on any surface**
    /// ([`FIAT_SURFACES`], the module doc's withhold rule): each holds the
    /// figure's room and shows nothing in it. The placeholder is not the
    /// person's currency; drawing it put "$1,234" on an iPhone home for a
    /// few seconds before it jumped to "¥8,876" (the 102 device run). The
    /// figure appears once, in the right money, where its room was kept.
    pub committed: bool,
    /// The person's own currency on its way: a stored choice whose rate is
    /// being fetched while nothing is committed yet. A surface that names
    /// its currency apart from the figure may name this one while the figure
    /// waits. `None` once committed, before the preference is read, and
    /// while the first launch's region guess is priced (not a choice yet).
    #[serde(default)]
    pub pending: Option<String>,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct DisplayCurrency;

impl App for DisplayCurrency {
    type Event = Event;
    type Model = Model;
    type ViewModel = CurrencyView;
    type Effect = CurrencyEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<CurrencyEffect, Event> {
        match event {
            Event::Refresh => {
                // Coalesce: while any read/seed/pricing is in flight, a focus
                // refresh must not supersede it — cancelling an in-flight seed
                // would strand the first launch on USD until the next one
                // (today's `_seedPromise` is single-flight for the same
                // reason). The in-flight pipeline always concludes with a
                // commit, which is what the refresher wanted anyway.
                if model.phase != Phase::Idle {
                    return Command::done();
                }
                model.attempt += 1;
                model.phase = Phase::LoadingStored;
                request(model, CurrencyOperation::ReadStoredCode)
            }
            Event::UserChose { code } => {
                // Persist first, price second — the pair still commits
                // atomically when the rate arrives; until then the previous
                // committed pair keeps rendering (no flicker to USD).
                model.attempt += 1;
                model.phase = Phase::ResolvingDisplay { code: code.clone() };
                let attempt = model.attempt;
                Command::all([
                    Command::request_from_shell(CurrencyOperation::WriteStoredCode {
                        code: code.clone(),
                    })
                    .then_send(move |result| Event::ShellCompleted { attempt, result }),
                    Command::request_from_shell(CurrencyOperation::ResolveRate { code })
                        .then_send(move |result| Event::ShellCompleted { attempt, result }),
                    render(),
                ])
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A superseded run — most importantly, a seed whose rate
                    // arrived after the user explicitly chose. Dropping it IS
                    // the "user choice wins" rule.
                    return Command::done();
                }
                accept(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> CurrencyView {
        match &model.committed {
            Some(pair) => CurrencyView {
                code: pair.code.clone(),
                rate: pair.rate,
                committed: true,
                pending: None,
            },
            None => CurrencyView {
                code: "USD".to_owned(),
                // The placeholder is USD, and USD really is 1 against itself —
                // this `Some` is a priced pair, not a default.
                rate: Some(1.0),
                committed: false,
                pending: match &model.phase {
                    Phase::ResolvingDisplay { code } => Some(code.clone()),
                    _ => None,
                },
            },
        }
    }
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: CurrencyShellResult) -> Command<CurrencyEffect, Event> {
    match (&model.phase, result) {
        (Phase::LoadingStored, CurrencyShellResult::StoredCode { code: Some(code) }) => {
            model.phase = Phase::ResolvingDisplay { code: code.clone() };
            request(model, CurrencyOperation::ResolveRate { code })
        }
        (Phase::LoadingStored, CurrencyShellResult::StoredCode { code: None }) => {
            if model.seed_attempted {
                // Already probed this session — a later focus with the key
                // still absent means the seed declined (offline/unpriceable
                // or region USD). Stay where we are; next LAUNCH retries.
                model.phase = Phase::Idle;
                if model.committed.is_none() {
                    commit(model, "USD", Some(1.0));
                }
                return render();
            }
            model.seed_attempted = true;
            model.phase = Phase::ReadingDevice;
            request(model, CurrencyOperation::ReadDeviceCurrency)
        }

        // -- display pricing -------------------------------------------------
        //
        // The pair commits with the rate EXACTLY as the shell observed it —
        // `None` stays `None`. This used to be `rate.unwrap_or(1.0)`, i.e. the
        // `getRate` display fallback baked into the one pair every surface
        // reads; the balance card would rather show a USD figure than a blank,
        // but the same pair is the multiplier `send` divides a fiat-denominated
        // amount by, and there rate-1-by-default is a 7x mispayment behind a
        // green button (the single-send twin of `batch_import`'s
        // `auto_price_per_token`, owner ruling: when the number that moves
        // money is unknown, stop; do not guess). The degradation the card
        // wanted is a FORMATTING choice and now lives where formatting lives —
        // the shell, which can print the USD figure without claiming it is ¥.
        //
        // The `code == priced` guard is the other half: a rate only ever
        // labels the currency it was fetched for.
        (
            Phase::ResolvingDisplay { code },
            CurrencyShellResult::RateResolved { code: priced, rate },
        ) if *code == priced => {
            let code = code.clone();
            // A source answering 0 or a negative number has not priced
            // anything — same refusal as unknown, not a multiplier.
            commit(model, &code, rate.filter(|r| r.is_finite() && *r > 0.0));
            model.phase = Phase::Idle;
            render()
        }

        // -- first-launch seed ----------------------------------------------
        (Phase::ReadingDevice, CurrencyShellResult::DeviceCurrency { code }) => {
            let candidate = code
                .map(|c| c.trim().to_uppercase())
                .filter(|c| is_iso_shape(c) && c != "USD");
            match candidate {
                None => {
                    // No region signal, or the region already is USD.
                    model.phase = Phase::Idle;
                    commit(model, "USD", Some(1.0));
                    render()
                }
                Some(candidate) => {
                    model.phase = Phase::ResolvingSeed {
                        candidate: candidate.clone(),
                    };
                    request(model, CurrencyOperation::ResolveRate { code: candidate })
                }
            }
        }
        (Phase::ResolvingSeed { candidate }, CurrencyShellResult::RateResolved { code, rate })
            if *candidate == code =>
        {
            match rate {
                // Offline or unpriceable: stay USD, key stays absent, next
                // launch retries. Unpriceable is NOT rate 1 (FR-007).
                None => {
                    model.phase = Phase::Idle;
                    commit(model, "USD", Some(1.0));
                    render()
                }
                Some(rate) => {
                    // The fetch took real time — re-read before persisting so
                    // an explicit choice made meanwhile is never overwritten.
                    model.phase = Phase::RecheckingStored {
                        candidate: code,
                        rate,
                    };
                    request(model, CurrencyOperation::ReadStoredCode)
                }
            }
        }
        (
            Phase::RecheckingStored { candidate, rate },
            CurrencyShellResult::StoredCode { code: None },
        ) => {
            let (candidate, rate) = (candidate.clone(), *rate);
            commit(model, &candidate, Some(rate));
            model.phase = Phase::Idle;
            let attempt = model.attempt;
            Command::all([
                Command::request_from_shell(CurrencyOperation::WriteStoredCode { code: candidate })
                    .then_send(move |result| Event::ShellCompleted { attempt, result }),
                render(),
            ])
        }
        (Phase::RecheckingStored { .. }, CurrencyShellResult::StoredCode { code: Some(code) }) => {
            // The user chose during the seed's rate fetch. Their pick wins;
            // the seed is NOT persisted. Price their choice instead.
            model.phase = Phase::ResolvingDisplay { code: code.clone() };
            request(model, CurrencyOperation::ResolveRate { code })
        }

        // A best-effort write acknowledged, or a result for a phase that no
        // longer expects it. Neither may change state.
        _ => Command::done(),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// The ONLY place `committed` changes — code and rate always land together,
/// and `rate: None` (unpriceable) is a value the pair can hold, not a hole to
/// be plugged with 1 on the way in.
fn commit(model: &mut Model, code: &str, rate: Option<f64>) {
    model.committed = Some(Pair {
        code: code.to_owned(),
        rate,
    });
}

/// Three ASCII uppercase letters — the `/^[A-Z]{3}$/` guard on device codes.
fn is_iso_shape(code: &str) -> bool {
    code.len() == 3 && code.bytes().all(|b| b.is_ascii_uppercase())
}

/// Issue one operation whose answer must match the current attempt.
fn request(model: &mut Model, operation: CurrencyOperation) -> Command<CurrencyEffect, Event> {
    let attempt = model.attempt;
    Command::all([
        Command::request_from_shell(operation)
            .then_send(move |result| Event::ShellCompleted { attempt, result }),
        render(),
    ])
}

impl super::SplitEffect for CurrencyEffect {
    type Op = CurrencyOperation;
    fn into_shell(self) -> Option<crux_core::Request<CurrencyOperation>> {
        match self {
            CurrencyEffect::Render(_) => None,
            CurrencyEffect::Shell(request) => Some(request),
        }
    }
}
