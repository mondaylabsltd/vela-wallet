//! Machine — the wallet session (spec `016-crux-wallet-state`, session).
//!
//! ```text
//! Boot ─► Restoring ─┬─ accounts ─► migrate addresses ─► clamp index ─► Active
//!                    ├─ none ────────────────────────────────────────► Empty
//!                    └─ read failed ──────────────────────────────────► Empty (unreadable)
//! Active ─ SwitchAccount / AccountEstablished ─► Active (index persisted)
//! Active ─ SignOut ─► CheckPendingUploads ─► confirm dialog ─► SignedOut
//!
//! landing watch (issue #409, ob-2), alongside — started on every restore,
//! every establishment and every SyncPending:
//!   LoadPendingUploads ─► per record:
//!     accepted one-key:  AwaitRegistryLanding(task) ─┬─ landed ─► RemovePendingUpload
//!     publish carried:   PublishPendingUpload       ─┤
//!                                                    └─ not confirmed ─► record kept
//! ```
//!
//! The session is the account source of truth every money flow sits on top of
//! (send / sign / balance), and it is **app-resident**: unlike the onboarding
//! and screen machines, this core is built once per process and outlives every
//! screen — the first extension of the 011 paradigm (inventory.md integration
//! note). It absorbs the `wallet-state.ts` reducer, its two effects (startup
//! restore + index persistence) and the scattered dispatch sites.
//!
//! The rules, one per inventory invariant:
//!
//! - ① `address` is DERIVED from `accounts[active_index]`, never stored, so it
//!   can never disagree with the active account — and an out-of-range switch
//!   is a WHOLE no-op instead of blanking the address.
//! - ② An account whose stored address disagrees with
//!   `computeAddress(publicKeyHex)` is corrected in memory BEFORE it is ever
//!   shown (funds sent to the stored address would reach the wrong Safe); the
//!   corrected record is written back best-effort, and a single failure keeps
//!   that account's old address without blocking the others.
//! - ③ The saved index is clamped into range, and a failed restore lands
//!   `Empty` — never a forever-spinner — and SAYS so
//!   ([`SessionView::accounts_unreadable`]): a stored list that cannot be
//!   read is not a clean sign-out, and must not look like one.
//! - ④ While the restore is in flight the index is NEVER persisted — the
//!   initial 0 would overwrite the user's saved value.
//! - ⑤ Signing out first asks the shell whether un-synced pending uploads
//!   exist; the confirm dialog carries the warning, and the machine offers no
//!   confirm path that skips the check.
//! - ⑥⑧ [`Event::AccountEstablished`] is the single hand-off from the
//!   onboarding machines' `CompleteOnboarding`, reusing [`CompletionMode`] —
//!   the ADD_ACCOUNT / SET_WALLET dual entry unified. An added account always
//!   becomes active.
//! - ⑦ The view rows carry each account's ORIGINAL index so a balance-sorted
//!   switcher can only dispatch the right one.
//! - ⑧ The route guard is a VIEW derivation ([`SessionView::allowed_route`]):
//!   the core says what is allowed, the shell decides when and how to
//!   navigate. `Loading` means "make no redirect judgment yet".
//! - ⑨ ONE WALLET, ONE ROW. A wallet is its ADDRESS, so an establishment
//!   whose address the list already holds ACTIVATES that row instead of
//!   appending a second one, and a restore collapses a pair already on disk
//!   (with the saved index following its wallet, not its old slot). The
//!   credential is not the identity: a multi-key wallet signed into with a
//!   second passkey recovers to the same address under a different credential
//!   id, which is exactly how devices ended up listing one wallet twice.
//!
//! Open question 2 is **decided** (spec `017`), and narrowly: signing out means
//! *stop being signed in on this device*, not *this device is no longer mine*.
//! [`Event::SignOutConfirmed`] emits [`SessionOperation::ClearSignedInWallet`]
//! (plus [`SessionOperation::ClearExtensionCache`]) alongside the in-memory
//! wipe, so a relaunch lands on onboarding instead of silently restoring the
//! session the user just ended — which is the whole user-visible point. Wiping
//! the device is a different feature, and this is not it.
//!
//! What makes the narrow scope self-consistent — and why nothing has to be
//! migrated or reconciled when the user comes back:
//!
//! - The address is DERIVED from the passkey, never merely stored. Signing in
//!   again resolves the same credential → the same public key →
//!   [`super::address_from_public_key_hex`] → the same Safe. So everything keyed
//!   by address (balance caches, receive-confirmation flags, transaction
//!   history) and everything keyed by origin (dApp permissions) lines back up on
//!   its own.
//! - Therefore only the two keys that *constitute being signed in* are cleared;
//!   contacts, history, custom tokens/networks, endpoint and provider settings,
//!   price source and locale preferences all stay. The user gets them back by
//!   authenticating, not by restoring a backup.
//! - The pending-upload outbox is NOT cleared either, for a different reason: a
//!   record there means some public key has not been confirmed on the
//!   registry, and deleting it loses the only trace of that — the sign-out
//!   warning, and the landing watch below. The core cannot enforce a key set
//!   (it names sentences, not storage) —
//!   [`SessionOperation::ClearSignedInWallet`] states it and the shell obeys.
//!
//! Invariant ⑤ is what makes this safe to decide: the confirmation dialog only
//! exists after the pending-upload check has answered, so the destructive
//! button is never reachable without the warning it may need.
//!
//! ## The landing watch (issue #409)
//!
//! A one-key wallet is entered as soon as the registry has ACCEPTED its publish
//! (owner's decision, 2026-10-04); the on-chain landing finishes afterwards,
//! and by then the create screen — and the onboarding core with it — is gone.
//! This machine is the one that is still here, so it confirms the landing:
//! whenever it holds a wallet (a restore into Active, an establishment) it
//! reads the outbox and, for each record waiting only on a landing
//! ([`super::PendingUpload::awaits_landing`] — one key, a task id), asks the
//! shell to wait on that registry task. Landed ⇒ the record is removed.
//! Anything else — the task failed, was not found, or was still pending when
//! the shell's budget ran out — leaves it exactly where it was: the sign-out
//! dialog keeps warning, and the next launch asks again.
//!
//! ob-2 (owner, 2026-10-28) widened it from confirming to finishing. The
//! create flow now saves the account BEFORE its publish and writes the whole
//! publish into the record — every member's creation-time proof and the group
//! key they bind to. A record with no task id but such a publish is sent again
//! ([`SessionOperation::PublishPendingUpload`]), whatever its key count, on
//! every launch (signed in or not — the record needs no account list) and
//! whenever the shell says to try again ([`Event::SyncPending`]: the
//! connection came back, its timer fired). Landed ⇒ removed, as above. The
//! view names what is still owed ([`SessionView::unsynced`]), so erasing the
//! device can say which wallet it would lose.
//!
//! Every step is a READ of the registry, a replay of proofs already made, or
//! a write of the local outbox. There is no passkey operation in this
//! machine's vocabulary at all, so finishing a sync late can never put a
//! prompt in front of anyone. A record from before ob-2 — no task id, no
//! publish — is left exactly where it is: only a sign-in can publish it.
//!
//! Out of scope on purpose: `SET_CONNECTED` (browser connection) belongs to
//! the dapp-connection machine, `shortAddress` and the balance-sorted switcher
//! ordering (`accounts.ts`) are shell-side presentation.

use crux_core::capability::Operation;
use crux_core::macros::effect;
use crux_core::{render::render, render::RenderOperation, App, Command};
use serde::{Deserialize, Serialize};

use super::shell::CompletionMode;
use super::Account;

#[cfg(feature = "bindings")]
use ts_rs::TS;

// ---------------------------------------------------------------------------
// Protocol
// ---------------------------------------------------------------------------

/// What this machine asks the platform to do. Sentences, not I/O — the shell
/// owns AsyncStorage, its keys, and every failure mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SessionOperation"))]
// A wire type: the JSON shape is pinned by the generated TS, and an account
// (spec 102 added its venue and domain) is what most of these carry.
#[allow(clippy::large_enum_variant)]
pub enum SessionOperation {
    /// Read every locally stored account.
    LoadAccounts,
    /// Read the saved active-account index.
    LoadActiveIndex,
    /// Migration write-back of a corrected account (invariant ②). Best
    /// effort: the shell swallows write errors — the TS inner `catch` does the
    /// same, and the in-memory correction stands either way (the reducer never
    /// rolled back a failed migration write; ported verbatim).
    SaveAccount { account: Account },
    /// Persist the active index. Best effort, like today's un-awaited
    /// `saveActiveAccountIndex` call.
    SaveActiveIndex { index: usize },
    /// Are there passkeys whose public key never reached the index server?
    /// (`hasPendingUploads` — the sign-out warning's input, invariant ⑤.)
    CheckPendingUploads,
    /// Stop this device being signed in: drop the stored account list and the
    /// active-account index, and NOTHING else.
    ///
    /// The scope is the decision, not an implementation detail. Those two keys
    /// are what "signed in" *is*; everything else — contacts, transaction
    /// history, custom tokens and networks, endpoint/provider settings, price
    /// source, locale preferences, dApp permissions — belongs to the account,
    /// not to the session, and the account comes back intact because its
    /// address is derived from the passkey rather than restored from disk.
    ///
    /// The pending-upload outbox is excluded for a second, independent reason:
    /// a record there is a public key the registry has not confirmed, and the
    /// landing watch needs no account list to finish the job (issue 409) —
    /// but a deleted record can never be confirmed, and the warning it carries
    /// is gone with it.
    ///
    /// Worded as a sentence because the key list is the shell's to know (016
    /// rule). Best effort, like every other write here: a storage failure
    /// leaves the session signed out in memory regardless.
    ClearSignedInWallet,
    /// Drop ONE account from the stored list, keyed by its address — the
    /// identity a row has (invariant ⑨), never its position, which a
    /// balance-sorted display can reorder under a slow write.
    ///
    /// This is the narrow half of signing out, and it exists because the wide
    /// half was the only thing a person could reach: a device holding six
    /// wallets could sign out of all six or none (owner, 2026-09-23). Removing
    /// an account is NOT deleting a wallet — the passkey still opens it, and
    /// signing in again brings the row back with its address, history and
    /// settings intact, for the same reason [`Self::ClearSignedInWallet`]
    /// gives.
    RemoveAccount { address: String },
    /// Drop the Safari extension's account snapshot (`vela.ext.account.json`).
    /// Same scope as [`Self::ClearSignedInWallet`] and for the same reason: the
    /// snapshot mirrors the signed-in account, so leaving it behind would let
    /// the extension keep answering as an account this device is no longer
    /// signed into. A no-op wherever no extension exists.
    ClearExtensionCache,
    /// Issue 409: read the pending-upload outbox, every record as stored —
    /// the landing watch decides which of them a read can settle.
    LoadPendingUploads,
    /// Issue 409: has this registry task landed? The shell polls the
    /// registry's task status on its own budget — the same interval and limit
    /// the create's publish always polled with — and answers once the task is
    /// terminal or the budget is spent. A read: no passkey, no signature,
    /// nothing written.
    AwaitRegistryLanding { task_id: String },
    /// Issue 409: drop the outbox record whose landing was just confirmed,
    /// keyed by its `id` (the wallet's first credential id). Best effort: a
    /// record that survives a failed write is confirmed again next launch.
    RemovePendingUpload { credential_id: String },
    /// ob-2: send a wallet's publish that never landed — the one its create
    /// would have sent, replayed from the outbox record: every member carries
    /// its creation-time proof and the group key rides along, so NO passkey is
    /// asked. The shell closes the group, registers and waits for the landing
    /// on the same budget as [`Self::AwaitRegistryLanding`], answering
    /// [`SessionShellResult::RegistryLanded`] or
    /// [`SessionShellResult::RegistryLandingUnconfirmed`]. Idempotent by
    /// content hash, so a second send of the same group is harmless.
    PublishPendingUpload { publish: super::PendingPublish },
}

/// What the shell observed.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SessionShellResult"))]
pub enum SessionShellResult {
    AccountsLoaded {
        accounts: Vec<Account>,
    },
    /// The account read threw (corrupt JSON, storage unavailable) — the
    /// `Promise.all(...).catch(LOADED_EMPTY)` branch.
    AccountsUnavailable,
    /// The shell's read already maps missing / garbage / errors to 0
    /// (`parseInt(raw, 10) || 0` inside try/catch), so there is no failure
    /// variant. Unsigned here: the TS clamp (`savedIndex < accounts.length`)
    /// lets a negative stored value through and the reducer would then render
    /// `address: ''` with `hasWallet: true`, which invariant ① forbids — so a
    /// negative value fails closed at the wire instead.
    ActiveIndexLoaded {
        index: usize,
    },
    /// Best-effort write acknowledged (see [`SessionOperation::SaveAccount`]).
    AccountSaved,
    /// The row is gone from storage (best effort, like every write here).
    AccountRemoved,
    ActiveIndexSaved,
    PendingUploads {
        has_pending: bool,
    },
    /// The pending-upload read threw. Ported verbatim: `handleOpenSignOut`
    /// dies before `setShowSignOut(true)`, so the dialog simply never opens —
    /// fail-closed for invariant ⑤ (no unwarned logout path appears).
    PendingUploadsUnavailable,
    /// Acks for the two logout clears. Best effort: the session is already
    /// signed out when they arrive, and neither can put it back.
    SignedInWalletCleared,
    ExtensionCacheCleared,
    /// Issue 409: the outbox as stored. A record this build cannot read
    /// arrives as `null` and costs only itself. A read that fails outright is
    /// [`Self::PendingUploadsUnavailable`], as for the sign-out check.
    PendingUploadsLoaded {
        records: Vec<super::OutboxRecord>,
    },
    /// Issue 409: the task is `done` — the group has a receipt on-chain. Also
    /// the answer to a [`SessionOperation::PublishPendingUpload`] that landed.
    RegistryLanded,
    /// Issue 409: the landing is NOT confirmed — the task failed, is not
    /// known (any more), was still pending when the shell's budget ran out, or
    /// the registry could not be reached — or a replayed publish (ob-2) did
    /// not land. The record stays; `message` is for the log.
    RegistryLandingUnconfirmed {
        message: String,
        /// r18-8 (§28, §32): this test build refused the publish to the live
        /// index before it left — the session never asks again by itself
        /// (the record stays; the person's own sign-in publishes it).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[cfg_attr(feature = "bindings", ts(optional))]
        refused: Option<bool>,
    },
    /// Issue 409: the confirmed record is gone (best effort).
    PendingUploadRemoved,
}

impl Operation for SessionOperation {
    type Output = SessionShellResult;
}

#[effect]
pub enum SessionEffect {
    Render(RenderOperation),
    Shell(SessionOperation),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS), ts(rename = "SessionEvent"))]
pub enum Event {
    /// App start. Single-shot: the restore effect ran once per process under
    /// React (`useEffect([])`), and a second `Boot` is inert here too.
    Boot,
    /// The switcher picked a row. `index` is the position in the ORIGINAL
    /// account list (invariant ⑦) — the view rows carry it so a display
    /// reorder cannot lose it. Negative values cannot cross the wire
    /// (unsigned; the TS reducer treats them as the same no-op).
    SwitchAccount { index: usize },
    /// The onboarding hand-off (`CompleteOnboarding` from the create_wallet /
    /// login machines) — ADD_ACCOUNT and SET_WALLET unified behind the one
    /// [`CompletionMode`] both machines already speak.
    AccountEstablished { mode: CompletionMode },
    /// Drop ONE account from this device, by its position in the original
    /// list (invariant ⑦, as [`Self::SwitchAccount`]).
    ///
    /// Sometimes a person wants out of one wallet and not the other five
    /// (owner, 2026-09-23). Removing the last one signs the device out, which
    /// is the same end state as [`Self::SignOutConfirmed`] and reached the
    /// same way, so there is one definition of "not signed in".
    RemoveAccount { index: usize },
    /// Spec 102 (D1): where the account at `address` reviews and signs, on
    /// this device — keyed by address, the identity a row has (invariant ⑨).
    /// Refused, and nothing written, when the venue cannot reach the
    /// account's keys (R1): Vela's sheet for a custom-domain account, or a
    /// page on another domain.
    SigningVenueChosen {
        address: String,
        venue: crate::signing_venue::SigningVenue,
    },
    /// The settings row: open the sign-out confirmation. Triggers the
    /// pending-upload check first (invariant ⑤).
    SignOut,
    /// The dialog's destructive button.
    SignOutConfirmed,
    /// The dialog's cancel / backdrop dismiss.
    SignOutDismissed,
    /// ob-2: try the outbox again now — the shell's connection came back, its
    /// retry timer fired, or a screen that names unsynced wallets opened. A
    /// round already running is not doubled: this is a no-op until it ends.
    SyncPending,
    /// Internal: an effect resolved. `attempt` is captured by the core when
    /// the request is made; a result carrying an older attempt belongs to a
    /// superseded run and is dropped.
    #[serde(skip)]
    ShellCompleted {
        attempt: u64,
        result: SessionShellResult,
    },
    /// Internal: a landing-watch effect resolved (issue 409). Kept apart from
    /// [`Self::ShellCompleted`] on purpose: `attempt` is bumped by every sign-out,
    /// switch and establishment, and a confirmation in flight must survive all
    /// of them — a key that landed has landed whoever is signed in. `watch` is
    /// the generation of the watch that asked; an older watch's answer is
    /// dropped, the newer one having read the outbox again.
    #[serde(skip)]
    WatchCompleted {
        watch: u64,
        result: SessionShellResult,
    },
}

// ---------------------------------------------------------------------------
// Model
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Phase {
    /// Storage not read yet — `isLoading: true`. The pristine state, so the
    /// splash shows from the first frame exactly as `INITIAL_STATE` does.
    #[default]
    Restoring,
    /// Loaded, no wallet (`LOADED_EMPTY`).
    Empty,
    Active,
    /// Explicit logout this session. Same surface as `Empty` (onboarding),
    /// kept distinct because the two arrive differently: `Empty` is "nothing
    /// was ever stored", `SignedOut` is "the storage clear is on its way".
    SignedOut,
}

#[derive(Default)]
pub struct Model {
    accounts: Vec<Account>,
    active_index: usize,
    phase: Phase,
    boot_started: bool,
    /// Restore gather — both loads run in parallel, the decision needs both.
    loaded_accounts: Option<Vec<Account>>,
    loaded_index: Option<usize>,
    /// A `CheckPendingUploads` is in flight (single-flight, like the async
    /// `handleOpenSignOut`).
    checking_uploads: bool,
    /// `Some(warn)` ⇒ the confirm dialog is open, `warn` = show the
    /// un-synced-passkey warning. Existing only after the check answered is
    /// what makes an unwarned logout unreachable (invariant ⑤).
    sign_out_warning: Option<bool>,
    /// Bumped per user-initiated flow; a result carrying an older attempt is
    /// dropped. This is what keeps a late restore from clobbering a session
    /// already established by onboarding.
    attempt: u64,
    /// The restore's account read failed (settings set-21): `Empty` because
    /// nothing could be READ, not because nothing is stored.
    accounts_unreadable: bool,
    /// The landing watch (issue 409): the generation of the newest one.
    watch: u64,
    /// Records this watch has still to settle, by credential id: a landing to
    /// confirm, or (ob-2) a publish to send.
    watch_queue: std::collections::VecDeque<(String, Settle)>,
    /// The record whose task is being waited on right now.
    watching: Option<String>,
    /// A watch round is between reading the outbox and its last answer.
    watch_running: bool,
    /// r18-8 (§28): this test build refused a publish to the live index this
    /// session — `SyncPending` asks nothing more by itself.
    write_refused: bool,
    /// ob-2: the outbox as the last round read it, less what it has removed
    /// since — what [`SessionView::unsynced`] names.
    outbox: Vec<super::PendingUpload>,
}

/// How the landing watch settles one outbox record.
#[derive(Clone, Debug)]
enum Settle {
    /// Issue 409: wait on the registry task its accepted publish is queued
    /// under.
    Landing(String),
    /// ob-2: send its publish, which needs no prompt.
    Publish(super::PendingPublish),
}

// ---------------------------------------------------------------------------
// ViewModel
// ---------------------------------------------------------------------------

/// Where the app is allowed to be. The shell performs navigation; the core
/// only rules (invariant ⑧). Today's `src/app/index.tsx` verbatim: spinner
/// while loading, `/(tabs)/wallet` with a wallet, `/onboarding` without.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "bindings", derive(TS))]
pub enum SessionRoute {
    /// Storage unread — make NO redirect judgment yet.
    Loading,
    Onboarding,
    Wallet,
}

/// One switcher row. `index` is the account's position in the ORIGINAL list —
/// exactly what [`Event::SwitchAccount`] expects — riding along so a
/// balance-sorted display can never dispatch a display position (invariant ⑦,
/// the `OrderedAccount` contract from `accounts.ts`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SessionAccountRow {
    pub index: usize,
    pub account: Account,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SessionSignOutView {
    /// Un-synced pending uploads exist — the dialog must show the warning and
    /// relabel the button ("Sign out anyway").
    pub pending_upload_warning: bool,
    /// How many wallets this device is signed into, so the dialog can say so.
    ///
    /// The copy was true of one wallet and quietly false of six: "the wallet
    /// is not deleted, everything comes back when you sign in again" says
    /// nothing about signing in SIX times (owner, 2026-09-23). The number is
    /// here rather than counted in each shell so all four say the same one.
    pub account_count: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SessionView {
    /// `isLoading` — true until storage has been read.
    pub loading: bool,
    pub has_wallet: bool,
    /// The active account's address, `""` when there is none — derived, so it
    /// is `accounts[active_index].address` by construction (invariant ①).
    pub address: String,
    pub active_index: usize,
    pub accounts: Vec<SessionAccountRow>,
    pub allowed_route: SessionRoute,
    /// `Some` ⇒ the sign-out confirmation dialog is open.
    pub sign_out: Option<SessionSignOutView>,
    /// The stored account list could not be read (settings set-21). The
    /// route is onboarding as for any empty list, but the shell must say the
    /// accounts could not be read — and offer to rebuild or reset them —
    /// rather than look signed out. Nothing was written: the list is where it
    /// was. Clears once a wallet is established.
    pub accounts_unreadable: bool,
    /// ob-2: the wallets on this device whose registration has not finished —
    /// no other device can find them yet. Named, so the screens that could
    /// lose one (erasing this device) say WHICH; empty once every record has
    /// landed. As of the landing watch's last read of the outbox.
    pub unsynced: Vec<SessionUnsynced>,
}

/// One wallet still owed to the registry (ob-2).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "bindings", derive(TS))]
pub struct SessionUnsynced {
    /// The account's name on this device, else the name the record was made
    /// under.
    pub name: String,
    /// Its address; empty when the record's keys derive none.
    pub address: String,
    /// The session finishes it on its own, with no prompt — when the shell
    /// says to try again ([`Event::SyncPending`]) and on every launch. `false`:
    /// a record from before ob-2, which only a sign-in can publish.
    pub retrying: bool,
}

// ---------------------------------------------------------------------------
// App
// ---------------------------------------------------------------------------

#[derive(Default)]
pub struct Session;

impl App for Session {
    type Event = Event;
    type Model = Model;
    type ViewModel = SessionView;
    type Effect = SessionEffect;

    fn update(&self, event: Event, model: &mut Model) -> Command<SessionEffect, Event> {
        match event {
            Event::Boot => {
                if model.boot_started || model.phase != Phase::Restoring {
                    return Command::done();
                }
                model.boot_started = true;
                model.attempt += 1;
                requests(
                    model,
                    vec![
                        SessionOperation::LoadAccounts,
                        SessionOperation::LoadActiveIndex,
                    ],
                )
            }
            Event::SwitchAccount { index } => switch_account(model, index),
            Event::AccountEstablished { mode } => establish(model, mode),
            Event::SignOut => {
                // Only reachable with a wallet (the settings screen), and
                // single-flight: while a check is running or the dialog is
                // already open, another tap does nothing.
                if model.phase != Phase::Active
                    || model.checking_uploads
                    || model.sign_out_warning.is_some()
                {
                    return Command::done();
                }
                model.checking_uploads = true;
                model.attempt += 1;
                requests(model, vec![SessionOperation::CheckPendingUploads])
            }
            Event::RemoveAccount { index } => remove_account(model, index),
            Event::SigningVenueChosen { address, venue } => choose_venue(model, &address, venue),
            Event::SignOutConfirmed => sign_out_confirmed(model),
            Event::SignOutDismissed => {
                if model.sign_out_warning.take().is_none() {
                    return Command::done();
                }
                render()
            }
            Event::SyncPending => {
                // Not before the restore has decided (its own round starts
                // there), and never a second round over a running one.
                // r18-8 (§28): a publish this test build refused is not asked
                // again by itself.
                if model.phase == Phase::Restoring || model.watch_running || model.write_refused {
                    return Command::done();
                }
                start_watch(model)
            }
            Event::ShellCompleted { attempt, result } => {
                if attempt != model.attempt {
                    // A superseded run — most importantly, a restore that
                    // finished after onboarding already established the
                    // session. Dropping it is what keeps the stale stored
                    // list from clobbering the live one.
                    return Command::done();
                }
                accept(model, result)
            }
            Event::WatchCompleted { watch, result } => {
                if watch != model.watch {
                    return Command::done();
                }
                accept_watch(model, result)
            }
        }
    }

    fn view(&self, model: &Model) -> SessionView {
        // Invariant ① by construction: the address IS the active account's.
        let address = model
            .accounts
            .get(model.active_index)
            .map(|account| account.address.clone())
            .unwrap_or_default();
        let has_wallet = !model.accounts.is_empty();
        SessionView {
            loading: model.phase == Phase::Restoring,
            has_wallet,
            address,
            active_index: model.active_index,
            accounts: model
                .accounts
                .iter()
                .cloned()
                .enumerate()
                .map(|(index, account)| SessionAccountRow { index, account })
                .collect(),
            allowed_route: match model.phase {
                // Invariant ⑧: no redirect judgment while loading.
                Phase::Restoring => SessionRoute::Loading,
                _ if has_wallet => SessionRoute::Wallet,
                // Empty and SignedOut: never linger on a funds surface.
                _ => SessionRoute::Onboarding,
            },
            sign_out: model
                .sign_out_warning
                .map(|pending_upload_warning| SessionSignOutView {
                    pending_upload_warning,
                    account_count: model.accounts.len(),
                }),
            accounts_unreadable: model.phase == Phase::Empty && model.accounts_unreadable,
            unsynced: model
                .outbox
                .iter()
                .map(|record| unsynced_row(model, record))
                .collect(),
        }
    }
}

/// How an outbox record is named (ob-2): by the account it is, when this
/// device holds it — the account's id IS the record's (the first credential).
fn unsynced_row(model: &Model, record: &super::PendingUpload) -> SessionUnsynced {
    let held = model
        .accounts
        .iter()
        .find(|account| account.id == record.id);
    SessionUnsynced {
        name: held.map_or_else(|| record.name.clone(), |account| account.name.clone()),
        address: held
            .map(|account| account.address.clone())
            .or_else(|| record.address())
            .unwrap_or_default(),
        retrying: record.awaits_landing().is_some() || record.replayable_publish().is_some(),
    }
}

// ---------------------------------------------------------------------------
// User-initiated transitions
// ---------------------------------------------------------------------------

fn switch_account(model: &mut Model, index: usize) -> Command<SessionEffect, Event> {
    // Invariant ①: out of range ⇒ the WHOLE action is a no-op (`if (!account)
    // return state`) — the address is never blanked, nothing is persisted.
    // During the restore window the list is still empty, so a premature switch
    // falls in here too and can never persist an index (invariant ④).
    if index >= model.accounts.len() {
        return Command::done();
    }
    if index == model.active_index {
        // Ported quirk: the reducer returns a new state object, but the
        // persist effect's deps (`activeAccountIndex`, `isLoading`,
        // `hasWallet`) are unchanged, so React never re-runs it — a
        // same-index switch re-renders without a write.
        return render();
    }
    model.active_index = index;
    requests(model, vec![SessionOperation::SaveActiveIndex { index }])
}

/// Spec 102: an account's venue, chosen in its settings. The write is best
/// effort, like every write here; the choice stands in memory either way.
fn choose_venue(
    model: &mut Model,
    address: &str,
    venue: crate::signing_venue::SigningVenue,
) -> Command<SessionEffect, Event> {
    if model.phase != Phase::Active {
        return Command::done();
    }
    let Some(account) = model
        .accounts
        .iter_mut()
        .find(|account| account.address.eq_ignore_ascii_case(address))
    else {
        return Command::done();
    };
    if account.signing_venue.same_as(&venue) || account.choose_venue(venue).is_err() {
        return Command::done();
    }
    account.stamp_page_origin();
    let account = account.clone();
    requests(model, vec![SessionOperation::SaveAccount { account }])
}

/// Drop one wallet from this device and stay on the others.
///
/// The wide half — [`sign_out_confirmed`] — takes every row, which on a device
/// holding six was all a person could reach (owner, 2026-09-23). This is the
/// narrow half, and it is NOT deletion: the passkey still opens that wallet,
/// and signing in again brings the row back with everything keyed to its
/// address, exactly as the module doc says of signing out.
///
/// Removing the LAST row is being signed out, so it ends where
/// [`sign_out_confirmed`] does rather than leaving a wallet-less `Active`
/// phase that invariant ① forbids.
fn remove_account(model: &mut Model, index: usize) -> Command<SessionEffect, Event> {
    // Out of range is the whole action's no-op, as in `switch_account`: a
    // display that reordered under a slow write must not remove a stranger.
    if index >= model.accounts.len() {
        return Command::done();
    }
    let removed = model.accounts.remove(index);
    // The row that was active either went, or moved down one.
    let was_active = index == model.active_index;
    if index < model.active_index {
        model.active_index -= 1;
    }

    if model.accounts.is_empty() {
        // The same end state as a confirmed sign-out, reached the same way, so
        // there is one definition of "this device is not signed in".
        model.attempt += 1;
        model.sign_out_warning = None;
        model.checking_uploads = false;
        model.active_index = 0;
        model.phase = Phase::SignedOut;
        return requests(
            model,
            vec![
                SessionOperation::ClearSignedInWallet,
                SessionOperation::ClearExtensionCache,
            ],
        );
    }

    // Another wallet is in front of the person now. `active_index` can be past
    // the end when the last row was the active one.
    model.active_index = model.active_index.min(model.accounts.len() - 1);
    let mut operations = vec![
        SessionOperation::RemoveAccount {
            address: removed.address,
        },
        SessionOperation::SaveActiveIndex {
            index: model.active_index,
        },
    ];
    if was_active {
        // The extension's snapshot mirrors the ACTIVE account, so leaving it
        // behind would let it keep answering as a wallet this device just put
        // away — the reason `ClearExtensionCache` exists at all.
        operations.push(SessionOperation::ClearExtensionCache);
    }
    requests(model, operations)
}

fn establish(model: &mut Model, mode: CompletionMode) -> Command<SessionEffect, Event> {
    // Whatever the boot restore is still doing is now stale: the onboarding
    // hand-off is fresher than any stored list (which was written BEFORE the
    // hand-off fired). Bumping the attempt drops those results by
    // construction.
    model.attempt += 1;
    model.loaded_accounts = None;
    model.loaded_index = None;
    match mode {
        CompletionMode::AddAccount { account } => {
            // ADD_ACCOUNT, and the new account MUST become active (invariant
            // ⑥) — but a wallet this device already holds is ACTIVATED, never
            // appended a second time (invariant ⑨). `login.rs` stops the
            // common case before the record reaches storage; this is the
            // machine's own guarantee, so no shell and no future caller can
            // put one wallet in the list twice by taking another route.
            let existing = model
                .accounts
                .iter()
                .position(|held| held.address.eq_ignore_ascii_case(&account.address));
            model.active_index = match existing {
                Some(index) => index,
                None => {
                    model.accounts.push(account);
                    model.accounts.len() - 1
                }
            };
            model.phase = Phase::Active;
            let saved = requests(
                model,
                vec![SessionOperation::SaveActiveIndex {
                    index: model.active_index,
                }],
            );
            // A wallet created a moment ago may be one whose landing is still
            // finishing (issue #409): its pending record carries the task.
            Command::all([saved, start_watch(model)])
        }
        CompletionMode::SetWallet {
            accounts,
            active_index,
        } => {
            // SET_WALLET verbatim, with one fail-closed correction: the
            // reducer would render `address: ''` for an out-of-range
            // activeIndex (every live dispatch site pre-clamps, so the branch
            // is unreachable today) — invariant ① forbids an empty address
            // beside `hasWallet: true`, so the index is clamped to 0 exactly
            // like the restore path's savedIndex (invariant ③).
            let index = if active_index < accounts.len() {
                active_index
            } else {
                0
            };
            let has_wallet = !accounts.is_empty();
            model.accounts = accounts;
            model.active_index = index;
            model.phase = if has_wallet {
                Phase::Active
            } else {
                Phase::Empty
            };
            if has_wallet {
                let saved = requests(model, vec![SessionOperation::SaveActiveIndex { index }]);
                Command::all([saved, start_watch(model)])
            } else {
                render()
            }
        }
    }
}

fn sign_out_confirmed(model: &mut Model) -> Command<SessionEffect, Event> {
    // Invariant ⑤ structurally: the confirm only exists while the dialog is
    // open, and the dialog only opens after the pending-upload check answered
    // — there is no path to an unwarned logout.
    if model.sign_out_warning.is_none() {
        return Command::done();
    }
    model.attempt += 1;
    model.sign_out_warning = None;
    model.checking_uploads = false;
    model.accounts.clear();
    model.active_index = 0;
    model.phase = Phase::SignedOut;
    // Open question 2, decided: the sign-in leaves the disk too. Memory alone
    // made "sign out" a lie — the next launch restored the very session the
    // user just ended. Narrow on purpose (module doc): the account list and the
    // active index, plus the extension's mirror of them. Nothing that belongs
    // to the ACCOUNT rather than to the session, and never the pending-upload
    // outbox.
    //
    // Both clears are best-effort and their acks are inert: the session is
    // signed out the instant this returns, whatever the disk does. `attempt`
    // was just bumped, so a restore or an upload check still in flight cannot
    // land after this and re-populate the model.
    requests(
        model,
        vec![
            SessionOperation::ClearSignedInWallet,
            SessionOperation::ClearExtensionCache,
        ],
    )
}

// ---------------------------------------------------------------------------
// Shell results
// ---------------------------------------------------------------------------

fn accept(model: &mut Model, result: SessionShellResult) -> Command<SessionEffect, Event> {
    match (model.phase, result) {
        // -- startup restore -------------------------------------------------
        (Phase::Restoring, SessionShellResult::AccountsLoaded { accounts }) => {
            model.loaded_accounts = Some(accounts);
            try_finish_restore(model)
        }
        (Phase::Restoring, SessionShellResult::ActiveIndexLoaded { index }) => {
            model.loaded_index = Some(index);
            try_finish_restore(model)
        }
        // Invariant ③: a failed read lands Empty — `isLoading` MUST clear, or
        // the app is a forever-spinner. The other load's late answer finds the
        // phase changed and is inert.
        (Phase::Restoring, SessionShellResult::AccountsUnavailable) => {
            model.loaded_accounts = None;
            model.loaded_index = None;
            model.phase = Phase::Empty;
            model.accounts_unreadable = true;
            render()
        }

        // -- sign-out check --------------------------------------------------
        (Phase::Active, SessionShellResult::PendingUploads { has_pending })
            if model.checking_uploads =>
        {
            model.checking_uploads = false;
            model.sign_out_warning = Some(has_pending);
            render()
        }
        (Phase::Active, SessionShellResult::PendingUploadsUnavailable)
            if model.checking_uploads =>
        {
            // Ported verbatim: the dialog silently fails to open (the async
            // handler dies before `setShowSignOut`). Fail-closed for ⑤; the
            // user can tap again.
            model.checking_uploads = false;
            Command::done()
        }

        // A best-effort write acknowledged, or a result for a phase that no
        // longer expects it. Neither may change state.
        _ => Command::done(),
    }
}

/// Both loads answered — migrate, clamp, and enter the session. The TS
/// equivalent is the body of the `Promise.all` handler in `WalletProvider`.
fn try_finish_restore(model: &mut Model) -> Command<SessionEffect, Event> {
    if model.loaded_accounts.is_none() || model.loaded_index.is_none() {
        return Command::done();
    }
    let mut accounts = model.loaded_accounts.take().unwrap_or_default();
    let saved_index = model.loaded_index.take().unwrap_or_default();

    if accounts.is_empty() {
        model.phase = Phase::Empty;
        // ob-2: a record needs no account list to be finished — a wallet whose
        // account could not be written, or a device signed out before its
        // landing, still gets its keys onto the registry.
        return Command::all([render(), start_watch(model)]);
    }

    // Invariant ②: fix accounts whose stored address is not what the public
    // key computes to (historically the credentialId landed there — funding
    // that string reaches the wrong Safe). Per account: no key ⇒ skip (the
    // `!acct.publicKeyHex` falsy check — an empty string here means the same);
    // computes differently ⇒ correct in memory and write back; computation
    // fails ⇒ keep the OLD address rather than corrupt storage, and never
    // block the other accounts. Ported verbatim, including the write being
    // fire-and-forget from the session's point of view: a failed write leaves
    // the in-memory correction standing (the TS catch spans the save, but the
    // reducer state was already built from the mutated object).
    let mut writes: Vec<SessionOperation> = Vec::new();
    for account in &mut accounts {
        if account.public_key_hex.is_empty() {
            continue;
        }
        // The full founding key set: a multi-key account must be recomputed
        // from ALL its keys — deriving from keys[0] alone yields a different
        // (single-key) Safe and would rewrite a correct multi-key address.
        // For legacy accounts key_hexes() is the sole scalar key and the
        // multi derivation is byte-identical to the old single-key one.
        let key_hexes = account.key_hexes();
        if let Ok(correct) = super::address_from_public_key_hexes(&key_hexes) {
            if account.address != correct {
                account.address = correct;
                writes.push(SessionOperation::SaveAccount {
                    account: account.clone(),
                });
            }
        }
    }

    // Invariant ⑨, applied to what is ALREADY on disk: one wallet, one row.
    //
    // Records written before `login.rs` matched on address can hold the same
    // wallet twice — a multi-key wallet signed into with a second passkey got
    // a second record under that credential's id. Collapsing them here is
    // what heals a device that already has the pair; the addresses are only
    // comparable AFTER the migration above, which is why this sits below it.
    //
    // The FIRST record wins, and the saved index follows its wallet rather
    // than its old slot — dropping a row above it would otherwise silently
    // select a different account. Storage is left alone on purpose: the
    // machine names sentences, has no "forget this record" among them, and a
    // row nothing reads is not worth a new operation on four shells.
    let mut kept: Vec<Account> = Vec::with_capacity(accounts.len());
    // NOT pre-clamped: an out-of-range saved index must still land on 0 below
    // (invariant ③), and clamping it to the last row here would quietly
    // select a different account instead.
    let mut moved_index = saved_index;
    for (slot, account) in accounts.into_iter().enumerate() {
        let twin = kept
            .iter()
            .position(|held: &Account| held.address.eq_ignore_ascii_case(&account.address));
        match twin {
            Some(first) => {
                if slot == moved_index {
                    moved_index = first;
                } else if slot < moved_index {
                    moved_index -= 1;
                }
            }
            None => kept.push(account),
        }
    }
    let accounts = kept;
    let saved_index = moved_index;

    // Invariant ③: clamp the saved index to the valid range.
    let active_index = if saved_index < accounts.len() {
        saved_index
    } else {
        0
    };
    model.accounts = accounts;
    model.active_index = active_index;
    model.phase = Phase::Active;

    // Invariant ④'s flip side: the persist effect fires exactly once the
    // loading window closes (`isLoading` flipping re-runs it in TS), which is
    // also what durably repairs an out-of-range saved value.
    writes.push(SessionOperation::SaveActiveIndex {
        index: active_index,
    });
    let written = requests(model, writes);
    // The launch is where a landing the last run could not see finish is
    // confirmed (issue #409) — the app closed inside those seconds, or the
    // registry was slow past the shell's budget.
    Command::all([written, start_watch(model)])
}

// ---------------------------------------------------------------------------
// The landing watch (issue #409)
// ---------------------------------------------------------------------------

/// Read the outbox afresh and confirm every landing it is waiting on. A newer
/// watch supersedes an older one: it reads the outbox again, so it sees every
/// record the older one would have, and the older one's answers are dropped.
fn start_watch(model: &mut Model) -> Command<SessionEffect, Event> {
    model.watch += 1;
    model.watch_queue.clear();
    model.watching = None;
    model.watch_running = true;
    watch_request(model, SessionOperation::LoadPendingUploads)
}

fn accept_watch(model: &mut Model, result: SessionShellResult) -> Command<SessionEffect, Event> {
    match result {
        SessionShellResult::PendingUploadsLoaded { records } => {
            model.outbox = records.into_iter().filter_map(|record| record.0).collect();
            model.watch_queue = model
                .outbox
                .iter()
                .filter_map(|record| {
                    let settle = match record.awaits_landing() {
                        Some(task_id) => Settle::Landing(task_id.to_owned()),
                        // ob-2: a publish that never landed, sendable with
                        // no prompt.
                        None => Settle::Publish(record.replayable_publish()?.clone()),
                    };
                    Some((record.id.clone(), settle))
                })
                .collect();
            Command::all([render(), next_landing(model)])
        }
        SessionShellResult::RegistryLanded => match model.watching.take() {
            // Confirmed: only now may the record go.
            Some(credential_id) => {
                model.outbox.retain(|record| record.id != credential_id);
                Command::all([
                    render(),
                    watch_request(
                        model,
                        SessionOperation::RemovePendingUpload { credential_id },
                    ),
                ])
            }
            None => Command::done(),
        },
        // Not confirmed. The record stays — warning at sign-out, asked again
        // next launch — and the watch moves on to the next one.
        SessionShellResult::RegistryLandingUnconfirmed { refused, .. } => {
            model.watching = None;
            if refused == Some(true) {
                model.write_refused = true;
            }
            next_landing(model)
        }
        SessionShellResult::PendingUploadRemoved => next_landing(model),
        // The outbox could not be read: nothing is known, so nothing changes.
        _ => {
            model.watch_queue.clear();
            model.watching = None;
            model.watch_running = false;
            Command::done()
        }
    }
}

/// Settle the next record, one at a time.
fn next_landing(model: &mut Model) -> Command<SessionEffect, Event> {
    let Some((credential_id, settle)) = model.watch_queue.pop_front() else {
        model.watching = None;
        model.watch_running = false;
        return Command::done();
    };
    model.watching = Some(credential_id);
    let operation = match settle {
        Settle::Landing(task_id) => SessionOperation::AwaitRegistryLanding { task_id },
        Settle::Publish(publish) => SessionOperation::PublishPendingUpload { publish },
    };
    watch_request(model, operation)
}

/// Issue one watch operation, its answer tagged with this watch's generation
/// rather than with `attempt` (see [`Event::WatchCompleted`]).
fn watch_request(model: &Model, operation: SessionOperation) -> Command<SessionEffect, Event> {
    let watch = model.watch;
    Command::request_from_shell(operation)
        .then_send(move |result| Event::WatchCompleted { watch, result })
}

// ---------------------------------------------------------------------------
// Plumbing
// ---------------------------------------------------------------------------

/// Issue operations whose answers must match the current attempt.
fn requests(model: &Model, operations: Vec<SessionOperation>) -> Command<SessionEffect, Event> {
    let attempt = model.attempt;
    let mut commands: Vec<Command<SessionEffect, Event>> = operations
        .into_iter()
        .map(|operation| {
            Command::request_from_shell(operation)
                .then_send(move |result| Event::ShellCompleted { attempt, result })
        })
        .collect();
    commands.push(render());
    Command::all(commands)
}

impl super::SplitEffect for SessionEffect {
    type Op = SessionOperation;
    fn into_shell(self) -> Option<crux_core::Request<SessionOperation>> {
        match self {
            SessionEffect::Render(_) => None,
            SessionEffect::Shell(request) => Some(request),
        }
    }
}
