//! The trusted signing page on the desktop: the page answers over
//! **`velawallet://`** (specs 071, 075, 076 and 102).
//!
//! Spec 102: the page is where a person REVIEWS and signs — an account's
//! signing venue — not a place a key lives. `app-web/trusted-signer`, opened in
//! the person's default browser, is told a request (and which key to use, R5),
//! derives what it signs itself, runs the ceremony and answers. This file is
//! the desktop's end of that conversation and nothing more:
//!
//! - the request is the core's (`trusted_signer::request` for a signature,
//!   `trusted_signer::ceremony::request` for a create / sign-in / proof);
//! - it rides out in the launch URL's **fragment** — never its query, which a
//!   server would see and log — and the answer comes back as a
//!   `velawallet://sign-result?…` the OS hands this app
//!   (`CheckedPage::url_launch`, then `parse_callback`);
//! - the page opened is the page that was CHECKED (R6): a launch URL is built
//!   only from the integrity check's admitted page
//!   (`signer_integrity::checked_page`), and a page whose check failed — or
//!   never ran and cannot — is not opened at all ([`NotOpened`]);
//! - whether an answer is this wallet's signature over this request
//!   (`trusted_signer::verify`) or a ceremony the page was allowed to run
//!   (`ceremony::verify`) is the core's.
//!
//! What is left here is a URL, a clock, and the screen.
//!
//! ## Spec 076: why there is no socket
//!
//! 071 put the request in the fragment and took the answer on a loopback HTTP
//! callback; 075 replaced both with a loopback WebSocket, so that one page visit
//! could carry a create's two requests. On the PUBLISHED page that cannot work,
//! and it was measured: `default-src 'none'` is inside the bytes the page is
//! addressed by, so the page may not open a socket at all — the wallet's own
//! listener saw no byte arrive. The owner's call was to drop it ("回环 WebSocket
//! 不做呀,现在就是纯 custome schema"), so the desktop speaks what the phones
//! speak. A navigation to a custom scheme is governed by no CSP, and the OS
//! delivers it even when this app is not in front.
//!
//! A URL carries exactly one request, so a flow's several operations are several
//! visits; what makes them one flow is this side, not the channel ([`Flow`]).
//!
//! ## Who may answer
//!
//! A fresh one-time token per REQUEST — random, and forgotten the moment that
//! request ends. Whether a URL is a callback at all, and which attempt it names,
//! are both the core's (`callback_of`, `callback_token`), so the shells that
//! receive one cannot answer those questions three slightly different ways; an
//! answer for a token nothing is waiting on is dropped in silence. Nothing is
//! listened on: no port, no interface, nothing on this machine to connect to.
//!
//! macOS delivers the callback as an Apple Event to the running app
//! (`on_open_urls`); Windows and Linux start the app with the URL as an
//! argument, which `main` reads. A second process started that way hands it
//! to the running wallet — over a pipe on Windows, a Unix socket on Linux
//! (`scheme_relay`).
//!
//! ## What the screen sees
//!
//! [`Channel`] is what a screen and its ceremonies say to each other: whether
//! this request goes to the Trusted Signer and to which page, where the person
//! keeps it (this device or another), the URL to hand the browser, the pairing
//! link and the six-digit code to confirm, that a page is being waited on,
//! "Cancel", and how the last attempt ended without an answer. The screen
//! words the ending; the core hears a cancelled passkey, so the request stays
//! open to be answered another way.

use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde_json::Value;

use vela_core::app::network_admin::BUILTIN_CHAINS;
use vela_core::app::shell::ShellOperation;
use vela_core::app::{Assertion, FailureKind, KeyMethod, RegistryPublishMember};
use vela_core::primitives::{to_base64url, to_hex};
use vela_core::signing_venue::{KeyLabel, KeyRoute};
use vela_core::trusted_signer::launch::{CheckedPage, IntegrityLine};
use vela_core::trusted_signer::{
    self, RequestInput, TrustedSignerError, Verified, ceremony as core_ceremony, verify, ws,
};
use vela_core::user_op::{MultiSendCall, UserOperation, WalletKey};

use crate::executor::passkey::{self, PasskeyFailure};

/// How long a page has to answer one request. The core has no clock; this is
/// the shell's, and it matches the page's own idle timeout.
pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How often a wait looks at the socket, the clock and the screen's buttons.
pub(crate) const POLL: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// What the person is told
// ---------------------------------------------------------------------------

/// Why an attempt ended without an answer, as the sheet words it (071
/// contract §5). Nothing was signed and nothing was submitted, whichever it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The person closed the page, declined on it, or stopped waiting.
    Closed,
    /// The page's own rules would not do it — not the person.
    Refused,
    /// An answer came back that is not this wallet's, for this request.
    Mismatch,
    /// Nothing came back in time.
    TimedOut,
    /// The page was never opened (spec 102): its check did not admit it.
    /// [`Channel::not_opened`] says why, in the words the card draws.
    NotOpened,
}

/// Why a page was not opened at all (spec 102) — said on the card instead of
/// any of the four endings above, because no page was ever there to end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NotOpened {
    /// R6: the page's check did not admit it — the integrity line says why
    /// ("couldn't check the page, so it won't open", "isn't on Vela's
    /// published build list", "Version … is new to Vela. Trust it on this
    /// device?" …) — and `page` is the page it was about, so a question can
    /// be answered where it is said. (Keys out of reach, R1, are not a page
    /// left unopened: the submit ends `VenueBlocked`, and the core says why.)
    Integrity { line: IntegrityLine, page: String },
}

impl Refusal {
    /// The core's verdict, in the sentences a person can act on.
    #[must_use]
    pub fn of(error: &TrustedSignerError) -> Self {
        match error {
            TrustedSignerError::Declined => Self::Closed,
            TrustedSignerError::Refused(_) => Self::Refused,
            TrustedSignerError::Malformed(_)
            | TrustedSignerError::WrongChallenge
            | TrustedSignerError::NotVerified
            | TrustedSignerError::ForeignKey
            | TrustedSignerError::BadSignature
            | TrustedSignerError::WrongToken => Self::Mismatch,
        }
    }

    /// The corpus key it is said with.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Closed => "componentsUi.signing.trustedSignerClosed",
            Self::Refused => "componentsUi.signing.trustedSignerRefused",
            Self::Mismatch => "componentsUi.signing.trustedSignerMismatch",
            Self::TimedOut => "componentsUi.signing.trustedSignerTimeout",
            // Drawn from `Channel::not_opened` whenever it is known; this is
            // the sentence for a channel that lost it.
            Self::NotOpened => "componentsUi.signing.integrity.couldNotCheck",
        }
    }
}

// ---------------------------------------------------------------------------
// The screen and the attempt
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    /// The page this request goes to, when the account's venue is a page.
    chosen: Option<String>,
    /// The wallet's name, for the page's `context.walletName`.
    wallet_name: Option<String>,
    /// The URL this attempt is waiting on — `Some` while one waits.
    waiting: Option<String>,
    /// That URL is to be handed to the browser (again).
    open: bool,
    /// The person stopped this wait.
    cancelled: bool,
    /// The screen is gone: no wait continues, none starts.
    closed: bool,
    /// How the last attempt ended without an answer, until the person has read
    /// it.
    ended: Option<Refusal>,
    /// Why the page was not opened, when that is how it ended.
    not_opened: Option<NotOpened>,
    /// An attempt owns this channel's surfaces from the moment it opens a
    /// page until it is done.
    claimed: bool,
    /// Spec 082 RD13 (W16): the page could not be reached when the person
    /// came back to the wallet with no answer. The wait — the request and its
    /// five-minute clock — goes on; the card says the page did not open and
    /// offers to try again.
    unreachable: bool,
    /// The ceremony this attempt runs on the page, by its title's corpus key
    /// (`Ceremony::title_key`: create / sign in / confirm) — `None` for a
    /// signature, which is the hand-off's "review and sign".
    ceremony_title: Option<&'static str>,
    /// That ceremony's key row (`Ceremony::key_label`): "New key on | This
    /// device" while a key is made, "Confirm with | Phone or tablet" while one
    /// signs in or proves — `None` for a signature.
    ceremony_key: Option<KeyLabel>,
}

/// What one screen and its Trusted Signer attempts say to each other.
///
/// Shared, because the screen's context is cloned into the executor when a
/// request opens and the choice, the "open it again", the pairing confirmation
/// and the "cancel" all happen afterwards. Every change is announced on the
/// stream [`Self::new`] hands back, which is what opens the page and redraws
/// the screen — no polling.
pub struct Channel {
    state: Mutex<State>,
    /// The page visit a ceremony flow holds open between operations (contract
    /// §1.5). Its OWN lock, never `state`'s: a ceremony holds the line for
    /// minutes while the screen reads and writes `state` on every frame.
    flow: Mutex<Option<Flow>>,
    wake: UnboundedSender<()>,
}

impl Channel {
    /// The channel, and the stream that says "look again" whenever it changes.
    #[must_use]
    pub fn new() -> (Arc<Self>, UnboundedReceiver<()>) {
        let (wake, woken) = unbounded();
        (
            Arc::new(Self {
                state: Mutex::default(),
                flow: Mutex::default(),
                wake,
            }),
            woken,
        )
    }

    fn with<T>(&self, change: impl FnOnce(&mut State) -> T) -> T {
        change(&mut self.state.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn announce(&self) {
        // Nobody listening is a screen already gone — not a fault.
        let _ = self.wake.unbounded_send(());
    }

    // -- the screen's half ----------------------------------------------------

    /// Where this request is reviewed and signed (spec 102): `Some(page)` —
    /// the account's venue is that trusted page — or `None`, in Vela. A new
    /// choice clears the last attempt's sentence — it answered a question
    /// nobody is asking now.
    pub fn choose(&self, page: Option<String>) {
        self.with(|state| {
            state.chosen = page;
            state.ended = None;
        });
        self.announce();
    }

    /// The page this request goes to, when the account's venue is a page.
    #[must_use]
    pub fn chosen(&self) -> Option<String> {
        self.with(|state| state.chosen.clone())
    }

    /// The wallet's name, as the page shows it on a ceremony card. The screen
    /// knows it; the executor does not.
    pub fn describe(&self, wallet_name: Option<String>) {
        self.with(|state| state.wallet_name = wallet_name);
    }

    /// A page is being waited on.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.with(|state| state.waiting.is_some())
    }

    /// The URL to hand the browser, once per ask.
    #[must_use]
    pub fn take_page(&self) -> Option<String> {
        self.with(|state| {
            if std::mem::take(&mut state.open) {
                state.waiting.clone()
            } else {
                None
            }
        })
    }

    /// "Open the page again": the same URL — the same port and token, so the
    /// same listener still takes its answer. Also the Retry of a page that
    /// could not be reached (RD13).
    pub fn reopen(&self) {
        self.with(|state| {
            state.open = state.waiting.is_some();
            state.unreachable = false;
        });
        self.announce();
    }

    /// The page this wait is on could not be reached (spec 082 RD13).
    pub fn mark_unreachable(&self) {
        let marked = self.with(|state| {
            let fresh = state.waiting.is_some() && !state.unreachable;
            if fresh {
                state.unreachable = true;
            }
            fresh
        });
        if marked {
            self.announce();
        }
    }

    /// The page could not be reached, and the wait is still on.
    #[must_use]
    pub fn unreachable(&self) -> bool {
        self.with(|state| state.waiting.is_some() && state.unreachable)
    }

    /// The title of the cards this attempt raises (the wait, how it ended):
    /// a ceremony's own — "Create a key on the signing page", "Sign in on
    /// the signing page", "Confirm with the key on the signing page" —
    /// else the hand-off's, for a signature.
    #[must_use]
    pub fn title_key(&self) -> &'static str {
        self.with(|state| {
            state
                .ceremony_title
                .unwrap_or("componentsUi.signing.handoffTitle")
        })
    }

    /// The key row of the ceremony this attempt runs on the page — the
    /// core's (`Ceremony::key_label`), drawn on its waiting card — or `None`
    /// for a signature.
    #[must_use]
    pub fn ceremony_key(&self) -> Option<KeyLabel> {
        self.with(|state| state.ceremony_key.clone())
    }

    /// The URL this wait is on — for the check the window's return runs.
    #[must_use]
    pub fn waiting_on(&self) -> Option<String> {
        self.with(|state| state.waiting.clone())
    }

    /// "Cancel" on a waiting sheet: the person declined.
    pub fn cancel(&self) {
        self.with(|state| state.cancelled = true);
        self.announce();
    }

    /// How the last attempt ended without an answer, until [`Self::forget`].
    #[must_use]
    pub fn ended(&self) -> Option<Refusal> {
        self.with(|state| state.ended)
    }

    /// Why the page was not opened, when the last attempt ended that way.
    #[must_use]
    pub fn not_opened(&self) -> Option<NotOpened> {
        self.with(|state| {
            state
                .not_opened
                .clone()
                .filter(|_| state.ended == Some(Refusal::NotOpened))
        })
    }

    /// The person read the sentence.
    pub fn forget(&self) {
        self.with(|state| {
            state.ended = None;
            state.not_opened = None;
        });
        self.announce();
    }

    /// The page will not be opened, and why — told before the attempt ends,
    /// so the card that ending raises already has its sentence.
    pub(crate) fn refuse_open(&self, why: NotOpened) {
        self.with(|state| state.not_opened = Some(why));
    }

    /// The screen that owned this channel is gone: a wait stops now, and none
    /// starts — an attempt nobody can see must not hold a port for minutes.
    /// Any page visit a flow was holding open is ended too.
    pub fn close(&self) {
        self.with(|state| state.closed = true);
        self.end_flow();
        self.announce();
    }

    /// End the page visit this channel's flow was holding open, if any — the
    /// page leaves its waiting card for its done card and the port goes.
    /// Called when a flow finishes, when anything refuses,
    /// and when the screen goes away.
    ///
    /// **Safe to call from the screen while a ceremony runs**, because a
    /// ceremony TAKES its visit out of here for as long as it is using the
    /// line (see [`Self::take_flow`]): there is never a moment when this
    /// thread and that one could both be writing one socket.
    ///
    /// **And safe to call from the main thread**, because saying goodbye is
    /// socket I/O — a `bye` and a close frame — and
    /// every caller but the ceremony itself is a `Drop` on the thread that
    /// draws the window. It goes on a thread of its own, which is allowed to
    /// outlive this call by the second or two a dead peer costs.
    pub fn end_flow(&self) {
        let Some(mut open) = self.take_flow() else {
            return;
        };
        let farewell = std::thread::Builder::new()
            .name("trusted-signer-bye".to_owned())
            .spawn(move || open.line.end());
        if let Err(error) = farewell {
            eprintln!("[vela-wallet] trusted signer: could not say goodbye: {error}");
        }
    }

    fn take_flow(&self) -> Option<Flow> {
        self.flow
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
    }

    /// Hand a visit back for the flow's next ceremony.
    ///
    /// A visit already here is ENDED rather than dropped. Two ceremonies on
    /// one channel — a create machine and a login machine both live, which
    /// "add another account" makes possible — would each have taken `None` and
    /// opened their own; the one that put its visit back second would
    /// otherwise leave the other's port bound and the other's tab sitting in
    /// front of nobody.
    fn keep_flow(&self, flow: Flow) {
        let displaced = self
            .flow
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .replace(flow);
        if let Some(mut stale) = displaced {
            stale.line.end();
        }
    }

    // -- the attempt's half ---------------------------------------------------

    /// Take this channel's surfaces for one attempt, or `None` when another
    /// already has them.
    ///
    /// **One screen, one attempt.** `waiting` is one slot, and it has to be:
    /// it is what one person is looking at. Two attempts sharing it is one
    /// attempt's page named on the other's card. A channel belongs to one
    /// screen, so this cannot happen in any flow this app offers; it is
    /// refused rather than left to be true by luck.
    fn claim(&self) -> Option<Claim<'_>> {
        let taken = self.with(|state| std::mem::replace(&mut state.claimed, true));
        if taken {
            eprintln!(
                "[vela-wallet] trusted signer: a second attempt asked for a screen \
                 one already has"
            );
            return None;
        }
        // `cancelled` belongs to ONE attempt, and this is where an attempt
        // begins. It used to outlive its own: `ask` checks `stopped()` before a
        // byte goes out — rightly, so that a Cancel pressed in that breath does
        // not still raise a passkey prompt — but nothing ever cleared the flag,
        // so one Cancel ended every later attempt before its page could open,
        // and the screen said "closed without signing" about a page nobody had
        // seen (owner, 2026-09-23: 「取消后再打开 一直报错」).
        //
        // Not `closed`: that means the screen itself is gone, which no new
        // attempt can undo.
        self.with(|state| state.cancelled = false);
        Some(Claim(self))
    }

    fn release(&self) {
        self.with(|state| state.claimed = false);
    }

    /// What this attempt is, for its cards: a ceremony's title key and key
    /// row, or `None` for a signature.
    fn entitle(&self, ceremony: Option<&core_ceremony::Ceremony>) {
        self.with(|state| {
            state.ceremony_title = ceremony.map(core_ceremony::Ceremony::title_key);
            state.ceremony_key = ceremony.map(core_ceremony::Ceremony::key_label);
        });
    }

    /// An attempt starts waiting on `url`; `open` hands it to the browser.
    /// `false` when the screen is gone.
    fn begin(&self, url: String, open: bool) -> bool {
        let began = self.with(|state| {
            if state.closed {
                return false;
            }
            state.waiting = Some(url);
            state.open = open;
            state.ended = None;
            state.not_opened = None;
            state.unreachable = false;
            true
        });
        self.announce();
        began
    }

    pub(crate) fn stopped(&self) -> bool {
        self.with(|state| state.cancelled || state.closed)
    }

    /// The wallet's name for a ceremony card.
    fn wallet_name(&self) -> String {
        self.with(|state| state.wallet_name.clone().unwrap_or_default())
    }

    /// One attempt is over. `ended` is `None` for one that answered.
    fn end(&self, ended: Option<Refusal>) {
        self.with(|state| {
            state.waiting = None;
            state.open = false;
            state.ended = ended;
            state.unreachable = false;
        });
        self.announce();
    }

    /// The waiting sheet stays up between the requests of one session, but
    /// nothing is being asked for right now.
    fn rest(&self) {
        self.with(|state| state.open = false);
        self.announce();
    }
}

// ---------------------------------------------------------------------------
// Could the page open at all (spec 082 RD13, W16)
// ---------------------------------------------------------------------------

/// How long the check may take.
pub const PAGE_CHECK_BUDGET: Duration = Duration::from_secs(5);

/// The page's address as a HEAD asks for it: scheme, host and path. The
/// fragment carries the request and its one-time token and is never sent —
/// a HEAD would not send it anyway, and it is cut here so nothing downstream
/// could.
#[must_use]
pub fn page_address(url: &str) -> String {
    let without_fragment = url.split('#').next().unwrap_or(url);
    without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment)
        .to_owned()
}

/// The person came back to the wallet and the page has not answered: can it
/// be reached at all? One HEAD of the page's address, over the wallet's own
/// routes, within [`PAGE_CHECK_BUDGET`]. A failure to reach it marks the wait
/// (the card then says the page did not open, with Retry); any answer —
/// whatever its status — leaves it alone, so a page the browser has cached
/// keeps working. **Blocks**: run it off the frame. `true` when it marked.
pub fn check_page(channel: &Channel) -> bool {
    let Some(url) = channel.waiting_on() else {
        return false;
    };
    let address = page_address(&url);
    let reached = crate::executor::proxy::with_routes(&address, PAGE_CHECK_BUDGET, |agent| {
        agent.head(&address).call()
    })
    .map_or_else(
        |failure| matches!(failure.error, ureq::Error::StatusCode(_)),
        |_| true,
    );
    if reached {
        return false;
    }
    crate::diag::vlog!(
        "trusted signer",
        "page {} unreachable",
        crate::diag::host_of(&address)
    );
    channel.mark_unreachable();
    true
}

// ---------------------------------------------------------------------------
// The line to the page
// ---------------------------------------------------------------------------

/// One page visit over the loopback socket on this device. A visit carries
/// **several requests in order** —
/// create then member proof, sign-in then recovery's two proofs — and ends on
/// `bye` (contract §1.5).
pub trait Line: Send {
    /// The id the next request of this session carries.
    fn next_id(&mut self) -> String;

    /// Put one request and wait for its answer, in the shape
    /// [`ws::parse_message`] normalises: a signature's `result` object, or a
    /// ceremony's whole envelope.
    fn ask(
        &mut self,
        id: &str,
        request: &Value,
        channel: &Channel,
        deadline: Instant,
    ) -> Result<Value, Refusal>;

    /// `bye`, and the line down.
    fn end(&mut self);
}

/// One attempt's hold on a channel's surfaces, released when it ends however
/// it ends.
struct Claim<'a>(&'a Channel);

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        self.0.release();
    }
}

/// A second attempt on one screen: refused rather than allowed to scribble
/// over the first one's card. Reported as a cancelled passkey, like every
/// other way an attempt can come to nothing, so the request stays open to be
/// answered once the first one is out of the way.
fn busy() -> PasskeyFailure {
    PasskeyFailure {
        kind: FailureKind::Cancelled,
        message: None,
    }
}

/// Open a line to `page` on this computer's own loopback.
///
/// Every refusal is told to the screen and answered as a cancelled passkey —
/// the request stays open to be answered another way. A **listener the OS will
/// not give** is the one real failure: it is not the person refusing, so it
/// carries its own words into the bug report rather than borrowing one.
fn open_line(
    page: &str,
    channel: &Channel,
    _deadline: Instant,
) -> Result<Box<dyn Line>, PasskeyFailure> {
    // The scheme line, not the loopback socket: the published page carries
    // `default-src 'none'` in its hashed bytes and cannot open a WebSocket at
    // all (spec 076, measured — the server saw no byte). It answers by
    // navigating to `velawallet://sign-result`, which no CSP governs and which
    // the OS delivers even when this app is not in front.
    let _ = channel;
    Ok(Box::new(SchemeLine::open(page)) as Box<dyn Line>)
}

/// Check every signing page this device may open, in the background (spec
/// 076 FR-007, spec 102 R6): the official page, the saved ones, and every
/// account's venue — so a person pressing Open finds the page already
/// checked, and Settings and the choosers already have its line.
///
/// Each page is checked only when the core says a refresh is due
/// (`launch::refresh_due`, D-14): never checked, or checked more than half a
/// day ago — and not within ten minutes of an attempt that could not
/// complete. Asked at start, on every return of a window to the front, and
/// hourly while the app runs ([`keep_fresh`]), so Open almost never waits.
///
/// **Why not at signing time.** FR-007 wants the check decoupled in time from
/// signing, so a server cannot tell "a verification request just arrived, the
/// next navigation is the target". A check that is too old by the time a page
/// is opened is still run again first, on the launch path ([`launchable`]).
pub fn prime_in_background() {
    for page in pages_in_use() {
        crate::executor::signer_integrity::check_in_background(&page);
    }
}

/// D-14: ask [`prime_in_background`] again every `launch::REFRESH_POLL_MS`
/// (an hour) for as long as the app runs, on a thread of its own — the
/// question is two numbers compared, and the answer changes twice a day.
pub fn keep_fresh() {
    let polling = std::thread::Builder::new()
        .name("signer-page-refresh".to_owned())
        .spawn(|| {
            let every = Duration::from_millis(vela_core::trusted_signer::launch::REFRESH_POLL_MS);
            loop {
                std::thread::sleep(every);
                prime_in_background();
            }
        });
    if let Err(error) = polling {
        eprintln!("[vela-wallet] signer page: no hourly refresh: {error}");
    }
}

/// The official page, every saved page, and every account's venue, each once.
fn pages_in_use() -> Vec<String> {
    use crate::executor::signer_integrity::key_of;
    let mut pages = vec![key_of(trusted_signer::DEFAULT_SIGNER_URL)];
    let saved = crate::executor::signing_pages::saved();
    let venues = crate::executor::storage::load_accounts()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|account| account.signing_venue.page_url().map(str::to_owned));
    for page in saved.into_iter().map(|page| page.url).chain(venues) {
        let page = key_of(&page);
        if !pages.contains(&page) {
            pages.push(page);
        }
    }
    pages
}

/// The checked page to open for `base` — the version that was checked, at
/// the address that was checked (R6). When nothing fresh vouches for it, it is
/// checked now (this runs on the ceremony's own thread, never the window's);
/// a check that does not admit it opens nothing, and its line says why.
fn launchable(base: &str) -> Result<CheckedPage, IntegrityLine> {
    use crate::executor::signer_integrity;
    let now = || crate::executor::now_ms() as u64;
    if let Some(page) = signer_integrity::checked_page(base, now()) {
        return Ok(page);
    }
    let verdict = signer_integrity::check(base);
    crate::diag::vlog!(
        "signer page",
        "{}",
        signer_integrity::unprefixed(&signer_integrity::describe(&verdict))
    );
    signer_integrity::checked_page(base, now()).ok_or_else(|| signer_integrity::line(base, now()))
}

// ---------------------------------------------------------------------------
// The custom-scheme callback
// ---------------------------------------------------------------------------

/// Callbacks that have arrived, by the one-time token they carry.
///
/// Keyed rather than a single slot: the token is what says WHICH attempt a
/// callback belongs to, so routing by it is both simpler and stricter than
/// assuming only one attempt exists. It also stops two attempts in flight —
/// rare in the product, ordinary in a parallel test run — from taking each
/// other's answers, which is how the single-slot version announced itself.
///
/// An entry nobody claims is dropped when the next attempt on that token
/// starts; there is no other way to reach it, because the token is random and
/// used once.
static ANSWERS: Mutex<Option<std::collections::HashMap<String, String>>> = Mutex::new(None);

fn forget_answer(token: &str) {
    if let Ok(mut slot) = ANSWERS.lock()
        && let Some(map) = slot.as_mut()
    {
        map.remove(token);
    }
}

fn take_delivered_answer(token: &str) -> Option<String> {
    ANSWERS
        .lock()
        .ok()
        .and_then(|mut slot| slot.as_mut().and_then(|map| map.remove(token)))
}

/// A `velawallet://sign-result?…` the OS handed this app.
///
/// **It is an event for a pending request, not a navigation.** Nothing about
/// the screen changes here: no route, no state, and a callback that arrives
/// when nothing is waiting is dropped in silence rather than raising an error
/// a person cannot act on.
///
/// `true` when it was delivered, which is only interesting to a caller that
/// wants to log the other case.
pub fn deliver_callback(url: &str) -> bool {
    // Both questions — is this a callback, and whose — are the core's, so the
    // three shells that receive one cannot answer them three slightly
    // different ways.
    let (Some(query), Some(token)) = (
        trusted_signer::callback_of(url),
        trusted_signer::callback_token(url),
    ) else {
        return false;
    };
    match ANSWERS.lock() {
        Ok(mut slot) => {
            slot.get_or_insert_with(std::collections::HashMap::new)
                .insert(token, query.to_owned());
            true
        }
        Err(_) => false,
    }
}

/// The page visit that answers over `velawallet://` (spec 076).
///
/// The request goes out in the launch URL's FRAGMENT — never its query, which
/// a server would see and log — and the answer comes back as a navigation the
/// OS hands to this app. No socket is listened on, and nothing of the page's
/// code has to reach the network, which is what makes it work under the
/// published page's `default-src 'none'`.
struct SchemeLine {
    /// The signing page this visit opens, as the account or the ceremony
    /// names it — each request launches the version its check admitted
    /// ([`launchable`]).
    page: String,
    url: String,
    token: String,
    /// The request this line was opened for, kept so "open the page again"
    /// hands the browser the same URL.
    seq: u64,
}

impl SchemeLine {
    fn open(page: &str) -> Self {
        Self {
            page: page.to_owned(),
            url: String::new(),
            // Replaced per request in `ask`: a token used for one request is
            // what makes "one-time" literally true, and it is what the phones
            // do. An empty one here would answer nothing, which is the right
            // thing for a line nobody has asked anything of yet.
            token: String::new(),
            seq: 0,
        }
    }
}

impl Line for SchemeLine {
    fn next_id(&mut self) -> String {
        self.seq += 1;
        format!("r{}", self.seq)
    }

    fn ask(
        &mut self,
        _id: &str,
        request: &Value,
        channel: &Channel,
        deadline: Instant,
    ) -> Result<Value, Refusal> {
        // Asked before a byte goes out, as the socket line does: a Cancel in
        // that breath must not still raise a passkey prompt on the page.
        if channel.stopped() {
            return Err(Refusal::Closed);
        }
        // A fresh token per request. Nothing left over from a previous visit
        // can then be read as this one's, because this one's name is new.
        forget_answer(&self.token);
        self.token = to_base64url(&passkey::random(16));
        // R6: the URL is the checked page's, and only an admitted check
        // builds one. A page nothing vouches for is not opened.
        // The page speaks the app's language (`lang=`), not the browser's.
        let launched = launchable(&self.page).and_then(|page| {
            page.url_launch(
                request,
                trusted_signer::CALLBACK_URL,
                &self.token,
                &crate::loc::app_language(),
                crate::executor::now_ms() as u64,
            )
            .map_err(|_| IntegrityLine::checking())
        });
        self.url = match launched {
            Ok(url) => url,
            Err(line) => {
                crate::diag::vlog!(
                    "trusted signer",
                    "page {} not opened: {}",
                    crate::diag::host_of(&self.page),
                    line.key
                );
                channel.refuse_open(NotOpened::Integrity {
                    line,
                    page: self.page.clone(),
                });
                return Err(Refusal::NotOpened);
            }
        };
        if !channel.begin(self.url.clone(), true) {
            return Err(Refusal::Closed);
        }
        let outcome = loop {
            if channel.stopped() {
                break Err(Refusal::Closed);
            }
            if Instant::now() >= deadline {
                break Err(Refusal::TimedOut);
            }
            if let Some(query) = take_delivered_answer(&self.token) {
                channel.rest();
                // The answer settled this request, and the person is looking
                // at the browser that sent it.
                crate::scheme_relay::bring_to_front();
                break trusted_signer::parse_callback(&query, &self.token).map_err(|error| {
                    if !matches!(error, TrustedSignerError::Declined) {
                        eprintln!("[vela-wallet] trusted signer: answer not accepted: {error}");
                    }
                    Refusal::of(&error)
                });
            }
            std::thread::sleep(POLL);
        };
        forget_answer(&self.token);
        outcome
    }

    fn end(&mut self) {
        forget_answer(&self.token);
    }
}

// ---------------------------------------------------------------------------
// The flow's page visit
// ---------------------------------------------------------------------------

/// The visit a ceremony flow holds open between operations (contract §1.5):
/// create → member proof, sign-in → recovery's two proofs. One page visit, one
/// passkey prompt each, rather than a new tab per ceremony.
struct Flow {
    /// The page it is a visit to — a second ceremony for a different page
    /// cannot ride on it.
    page: String,
    line: Box<dyn Line>,
}

// ---------------------------------------------------------------------------
// The request, and one signature
// ---------------------------------------------------------------------------

/// A request as the page is told about it — all but the operation, which the
/// spine assembles later.
#[derive(Clone, Debug, Default)]
pub struct Ask {
    /// The request's own method; empty for the wallet's own send, which the
    /// page is sent as `wallet_sendCalls` of the operation's calls
    /// (`own_send_params`, contract §1).
    pub method: String,
    /// Its params, verbatim — after the wallet's own edits (invariant ⑨).
    pub params: Value,
    /// Who asked, as the transport says it; empty for the wallet itself.
    pub origin: String,
    /// The account's name — it points the person at a passkey.
    pub account_name: Option<String>,
}

impl Ask {
    /// The wallet's own send: no site, the calls are the intent.
    #[must_use]
    pub fn own(account_name: Option<String>) -> Self {
        Self {
            account_name,
            ..Self::default()
        }
    }

    /// The page's `{intent, context}`. `operation` is the ASSEMBLED operation
    /// and the calls before its fee leg — the wallet appends the fee last on
    /// every chain, so its index is their count. `None` for a message.
    #[must_use]
    pub fn request(
        &self,
        chain_id: u32,
        account: &str,
        keys: &[WalletKey],
        key_route: Option<&KeyRoute>,
        operation: Option<(&UserOperation, &[MultiSendCall])>,
    ) -> Value {
        let credential_ids: Vec<String> =
            keys.iter().map(|key| key.credential_id.clone()).collect();
        let chain_name = crate::executor::custom_tokens::network_name(chain_id);
        let native_symbol = BUILTIN_CHAINS
            .iter()
            .find(|chain| chain.chain_id == chain_id)
            .map(|chain| chain.native_symbol);
        trusted_signer::request(&RequestInput {
            method: &self.method,
            params: self.params.clone(),
            origin: &self.origin,
            chain_id: u64::from(chain_id),
            chain_name: Some(&chain_name),
            native_symbol,
            account,
            account_name: self.account_name.as_deref(),
            credential_ids_hex: &credential_ids,
            // R5: the page offers that one key, with its transports and the
            // browser's hints, instead of asking where the passkey is.
            key_route,
            user_op: operation.map(|(op, _)| op),
            calls: operation.map_or(&[][..], |(_, calls)| calls),
            // Spec 079: the desktop's only dApp source is its own browser
            // (group C: no WalletPair), so a site origin here was read from
            // the engine, not claimed by another app.
            origin_seen_by_browser: !self.origin.is_empty(),
        })
    }
}

/// One signature through the Trusted Signer, start to finish: ask where the page
/// is, open it, wait for the answer, verify it against `digest` and `keys`.
///
/// A refusal of any kind is told to the screen ([`Channel::ended`]) and
/// answered as a cancelled passkey, which is what keeps the request open to be
/// signed another way. A listener the OS would not give is the one real
/// failure.
pub fn sign(
    request: &Value,
    page: &str,
    digest: &[u8],
    keys: &[WalletKey],
    channel: &Channel,
) -> Result<Assertion, PasskeyFailure> {
    let deadline = Instant::now() + TIMEOUT;
    let Some(_claim) = channel.claim() else {
        return Err(busy());
    };
    channel.entitle(None);
    let mut line = open_line(page, channel, deadline)?;
    let id = line.next_id();
    let answered = line.ask(&id, request, channel, deadline);
    // A signature is one request: the visit ends either way.
    line.end();
    let outcome = answered.and_then(|answer| {
        verify(&answer, digest, keys).map_err(|error| {
            eprintln!("[vela-wallet] trusted signer: answer not accepted: {error}");
            Refusal::of(&error)
        })
    });
    match outcome {
        Ok(verified) => {
            channel.end(None);
            Ok(assertion_of(verified, page))
        }
        Err(refusal) => Err(gave_up(channel, refusal)),
    }
}

/// Tell the screen how it ended, and the core that a passkey was cancelled.
fn gave_up(channel: &Channel, refusal: Refusal) -> PasskeyFailure {
    channel.end(Some(refusal));
    eprintln!("[vela-wallet] trusted signer: ended without an answer ({refusal:?})");
    PasskeyFailure {
        kind: FailureKind::Cancelled,
        message: None,
    }
}

/// A verified answer in the shape the passkey's takes, so the envelope that
/// follows cannot tell them apart. The page it came from rides along: a key
/// reached through a page lives behind it (contract §1.2).
fn assertion_of(verified: Verified, page: &str) -> Assertion {
    Assertion {
        credential_id: verified.credential_id_hex,
        signature_der_hex: to_hex(&verified.signature_der, false),
        authenticator_data_hex: to_hex(&verified.authenticator_data, false),
        client_data_json_hex: to_hex(&verified.client_data_json, false),
        user_id_hex: None,
        // The page's browser knows; the page does not say.
        authenticator_attachment: String::new(),
        signer_origin: Some(ws::origin_of(page)).filter(|origin| !origin.is_empty()),
    }
}

// ---------------------------------------------------------------------------
// Spec 075: the ceremonies
// ---------------------------------------------------------------------------

/// A verified ceremony answer, in the machines' own types.
pub use vela_core::trusted_signer::ceremony::Answer;

/// One passkey ceremony through the Trusted Signer — a create, a sign-in, a
/// proof, a member proof (contract §1.3).
///
/// `operation` is the machine's own operation; `expected_member_challenge` is
/// the bytes THIS wallet fetched from the registry, which the page's own fetch
/// has to agree with. The page visit is the flow's: a create's member proof and
/// a recovery's second signature ride on the socket the first ceremony opened,
/// and `last` ends it.
pub fn run_ceremony(
    operation: &ShellOperation,
    page: &str,
    expected_member_challenge: Option<&[u8]>,
    registry: &str,
    channel: &Channel,
    last: bool,
) -> Option<Result<Answer, PasskeyFailure>> {
    let ceremony = core_ceremony::Ceremony::of(operation)?;
    let page = trusted_signer::signer_url(page).unwrap_or_else(|_| page.to_owned());
    let wallet_name = match &ceremony {
        // The first key's name IS the wallet's name, and it is the only name
        // this side knows at create time.
        core_ceremony::Ceremony::RegisterPasskey { name, .. } => name.clone(),
        _ => channel.wallet_name(),
    };
    Some(ceremony_on_flow(
        &ceremony,
        &page,
        &wallet_name,
        registry,
        expected_member_challenge,
        channel,
        last,
    ))
}

/// The page a ceremony operation runs on (spec 102 R3): the op's own `page`
/// — `Some` only for a wallet on a custom signing domain, whose keys only its
/// page can mint or use. `None`: the ceremony runs in the app.
///
/// The page is the one the person chose ("Use a trusted signing page"), path
/// and all — a page served under a path (`http://localhost:8140/
/// clearsigning/`) is launched there, never at its origin's root.
#[must_use]
pub fn page_of(operation: &ShellOperation) -> Option<&str> {
    match operation {
        ShellOperation::RegisterPasskey { page, .. }
        | ShellOperation::AuthenticatePasskey { page, .. }
        | ShellOperation::SignProof { page, .. }
        | ShellOperation::SignMemberProof { page, .. } => page.as_deref(),
        _ => None,
    }
    .filter(|page| !page.trim().is_empty())
}

/// Two addresses that reach the same page, as the channel judges it: the
/// WebSocket handshake accepts one `Origin` and the core's verifier compares
/// one origin, so that is the identity a page visit has.
fn same_page(one: &str, other: &str) -> bool {
    let origin = ws::origin_of(one);
    !origin.is_empty() && origin == ws::origin_of(other)
}

fn ceremony_on_flow(
    ceremony: &core_ceremony::Ceremony,
    page: &str,
    wallet_name: &str,
    registry: &str,
    expected_member_challenge: Option<&[u8]>,
    channel: &Channel,
    last: bool,
) -> Result<Answer, PasskeyFailure> {
    let deadline = Instant::now() + TIMEOUT;
    let Some(_claim) = channel.claim() else {
        return Err(busy());
    };
    // A ceremony has nothing to review: its cards say what the person is
    // doing on the page (spec 102 core round 12).
    channel.entitle(Some(ceremony));
    // The flow's visit, when it is a visit to this same page; otherwise a new
    // one, and the old one is told goodbye rather than left open.
    let mut open = channel.take_flow();
    // A visit is to an ORIGIN: `Flow.page` was built from the Settings URL and
    // `page` may have come from a key's recorded origin, so comparing the two
    // strings would have found them different on every second ceremony of
    // every real flow — and torn the page down between a create and its
    // member proof, which is the one thing this whole channel exists to avoid.
    if open
        .as_ref()
        .is_some_and(|flow| !same_page(&flow.page, page))
        && let Some(mut stale) = open.take()
    {
        stale.line.end();
    }
    let mut visit = match open {
        Some(visit) => visit,
        None => Flow {
            page: page.to_owned(),
            line: open_line(page, channel, deadline)?,
        },
    };

    let id = visit.line.next_id();
    // A member proof needs the registry's deployment: the page computes the
    // challenge itself now, and the published page reaches no network to look it
    // up (spec 076 — the create's second step failed at 「注册表没有应答」 until
    // the wallet carried it). Fetched only for the ceremony that needs it, so no
    // other one waits on a round trip.
    let deployment = matches!(ceremony, core_ceremony::Ceremony::SignMemberProof { .. })
        .then(crate::executor::registry::deployment)
        .flatten();
    let request = core_ceremony::request(ceremony, &id, wallet_name, registry, deployment.as_ref());
    let answered = visit.line.ask(&id, &request, channel, deadline);
    let outcome = answered.and_then(|answer| {
        core_ceremony::verify(ceremony, &answer, page, expected_member_challenge).map_err(|error| {
            eprintln!("[vela-wallet] trusted signer: ceremony not accepted: {error}");
            Refusal::of(&error)
        })
    });
    match outcome {
        Ok(answer) => {
            if last {
                visit.line.end();
                channel.end(None);
            } else {
                // The page stays on its waiting card for the next request of
                // the same flow; so does this wallet's sheet.
                channel.keep_flow(visit);
            }
            Ok(answer)
        }
        Err(refusal) => {
            // Nothing more will be asked on a visit whose last request was
            // refused: the page is told goodbye and the flow starts over.
            visit.line.end();
            Err(gave_up(channel, refusal))
        }
    }
}

/// A member proof asked for OUTSIDE the create machine: the re-publish a
/// sign-in does (`registry::publish` signs each member that has no
/// creation-time proof). It is the same `vela_memberProof` ceremony, on the
/// same page visit, so a sign-in that re-publishes three keys opens the page
/// once and shows three cards.
///
/// `expected_challenge` is the bytes THIS wallet was given for this member;
/// the page fetches its own and the core refuses an answer over anything else.
///
/// `page` is the publish's own (`RegistryPublish.page`, spec 102 R3): the
/// custom-domain wallet's page, the only place its keys answer. `method` is
/// where the key lives, told to the page as hints (R5).
pub fn member_proof(
    member: &RegistryPublishMember,
    page: &str,
    method: KeyMethod,
    group_public_key_hex: &str,
    expected_challenge: &[u8],
    registry: &str,
    channel: &Channel,
) -> Result<Assertion, PasskeyFailure> {
    let ceremony = core_ceremony::Ceremony::SignMemberProof {
        credential_id: member.credential_id.clone(),
        public_key_hex: member.public_key_hex.clone(),
        attestation_hex: member.attestation_hex.clone(),
        group_public_key_hex: group_public_key_hex.to_owned(),
        method,
        transports: member.transports.clone(),
    };
    let page = trusted_signer::signer_url(page).unwrap_or_else(|_| page.to_owned());
    let wallet_name = channel.wallet_name();
    // Not the last: a publish signs its members in a row, and the caller ends
    // the visit once they are all in.
    match ceremony_on_flow(
        &ceremony,
        &page,
        &wallet_name,
        registry,
        Some(expected_challenge),
        channel,
        false,
    )? {
        Answer::Assertion(assertion) => Ok(assertion),
        Answer::Registration(_) => Err(PasskeyFailure::other(
            "the Trusted Signer answered a member proof with a new key",
        )),
    }
}

/// Is this the last ceremony of its flow? A create ends with the member proof;
/// a recovery with its second signature. Everything else leaves the page open
/// for what comes next.
#[must_use]
pub fn ends_the_flow(operation: &ShellOperation) -> bool {
    use vela_core::app::shell::ProofPurpose;
    match operation {
        ShellOperation::SignMemberProof { .. } => true,
        ShellOperation::SignProof { purpose, .. } => {
            matches!(purpose, ProofPurpose::RecoverSecond | ProofPurpose::Verify)
        }
        _ => false,
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use p256::ecdsa::signature::hazmat::PrehashSigner as _;
    use p256::ecdsa::{Signature, SigningKey};
    use serde_json::json;
    use vela_core::primitives::sha256;
    use vela_core::webauthn::webauthn_signing_hash;

    const DIGEST: [u8; 32] = [0xab; 32];
    const CREDENTIAL: [u8; 3] = [0x11, 0x22, 0x33];
    pub(crate) const PAGE: &str = "https://sign.getvela.app/";

    pub(crate) fn signing_key(seed: u8) -> SigningKey {
        SigningKey::from_slice(&[seed; 32]).unwrap_or_else(|e| unreachable!("{e}"))
    }

    pub(crate) fn wallet_key(signing: &SigningKey, credential: &[u8]) -> WalletKey {
        WalletKey {
            credential_id: to_hex(credential, false),
            public_key_hex: to_hex(
                signing.verifying_key().to_encoded_point(false).as_bytes(),
                false,
            ),
        }
    }

    pub(crate) fn keys() -> Vec<WalletKey> {
        vec![
            wallet_key(&signing_key(9), &[0x99]),
            wallet_key(&signing_key(7), &CREDENTIAL),
        ]
    }

    /// What the page returns for a SIGNATURE over `digest` — a user-verified
    /// `webauthn.get` signed by `signing` as `credential`, in the page's own
    /// field shapes (071 contract §4).
    pub(crate) fn page_result(signing: &SigningKey, credential: &[u8], digest: &[u8]) -> Value {
        let (authenticator_data, client, signature) = signed(signing, digest, "webauthn.get");
        json!({
            "credentialId": to_base64url(credential),
            "signature": to_hex(&signature, true),
            "authenticatorData": to_hex(&authenticator_data, true),
            "clientDataJSON": to_hex(client.as_bytes(), true),
            "userVerified": true,
        })
    }

    fn signed(signing: &SigningKey, challenge: &[u8], kind: &str) -> (Vec<u8>, String, Vec<u8>) {
        let mut authenticator_data = sha256(b"getvela.app");
        authenticator_data.push(0x05);
        authenticator_data.extend_from_slice(&[0, 0, 0, 7]);
        let client = format!(
            r#"{{"type":"{kind}","challenge":"{}","origin":"https://sign.getvela.app","crossOrigin":false}}"#,
            to_base64url(challenge)
        );
        let prehash = webauthn_signing_hash(&authenticator_data, client.as_bytes());
        let signature: Signature = signing
            .sign_prehash(&prehash)
            .unwrap_or_else(|e| unreachable!("{e}"));
        (authenticator_data, client, signature.to_bytes().to_vec())
    }

    /// The official page, checked and admitted — so an attempt launches it
    /// without reaching the network (spec 102 R6: nothing else launches).
    pub(crate) fn admitted() {
        crate::executor::signer_integrity::admit_for_tests(PAGE);
    }

    /// Wait for an attempt on `channel` to hand the screen its page.
    pub(crate) fn page_of_channel(channel: &Channel) -> String {
        for _ in 0..600 {
            if let Some(url) = channel.take_page() {
                return url;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        unreachable!("the attempt never asked for its page")
    }

    /// Play the page for ONE request, end to end: take the launch URL, connect
    /// Play the page for ONE request: take the launch URL, and answer over the
    /// custom scheme as the page does.
    ///
    /// There is no envelope any more. The socket carried `{v,t,n,id,result}`;
    /// a callback carries the result itself, base64url in `result=`, and
    /// `parse_callback` hands it straight back. So `reply` produces the answer,
    /// not a message wrapping one.
    pub(crate) fn answers_once(
        channel: &Arc<Channel>,
        reply: impl FnOnce() -> Value + Send + 'static,
    ) -> std::thread::JoinHandle<()> {
        let channel = Arc::clone(channel);
        std::thread::spawn(move || {
            let url = page_of_channel(&channel);
            let token = token_of(&url);
            let body = to_base64url(reply().to_string().as_bytes());
            deliver_callback(&format!(
                "{}?t={token}&result={body}",
                trusted_signer::CALLBACK_URL
            ));
        })
    }

    /// Play the page refusing ONE request, as the page does: `error=<code>`.
    pub(crate) fn refuses_once(
        channel: &Arc<Channel>,
        code: &'static str,
    ) -> std::thread::JoinHandle<()> {
        let channel = Arc::clone(channel);
        std::thread::spawn(move || {
            let url = page_of_channel(&channel);
            let token = token_of(&url);
            deliver_callback(&format!(
                "{}?t={token}&error={code}",
                trusted_signer::CALLBACK_URL
            ));
        })
    }

    /// The one-time token out of a launch URL's fragment.
    pub(crate) fn token_of(url: &str) -> String {
        assert!(url.contains("?ch=url"), "{url}");
        let fragment = url.split_once('#').map_or("", |(_, f)| f);
        fragment
            .split('&')
            .find_map(|pair| pair.strip_prefix("t="))
            .unwrap_or_default()
            .to_owned()
    }

    // -- cancel, the launch URL, and the callback -----------------------------

    /// Cancel, then try again. The second attempt must be a real attempt.
    ///
    /// `begin` reset `waiting`, `open` and `ended` but not `cancelled`, and
    /// `stopped()` reads `cancelled` — so one Cancel ended every later attempt
    /// before the page could open, and the screen said "closed without
    /// signing" about a page nobody had seen. Reported from using the app;
    /// no test here covered a SECOND attempt.
    /// Spec 082 RD13 (W16): a page that cannot be reached marks the wait —
    /// which goes on — and a page that answers (a cached one too) is left
    /// alone. The fragment never leaves the machine.
    #[test]
    fn an_unreachable_page_is_said_and_a_reachable_one_is_left_alone() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(1);
        let (channel, _woken) = Channel::new();
        assert!(channel.begin(
            format!("http://127.0.0.1:{closed}/b/abc/sign#req=eyJ0b2tlbiI6InNlY3JldCJ9"),
            true,
        ));
        assert!(check_page(&channel), "nothing listens there");
        assert!(channel.unreachable());
        assert!(channel.waiting(), "the request and its clock stay");
        channel.reopen();
        assert!(!channel.unreachable(), "Retry opens the page again");

        // A page that answers: the HEAD asks for its path, never the fragment.
        let server = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|e| unreachable!("loopback: {e}"));
        let port = server.local_addr().map(|addr| addr.port()).unwrap_or(0);
        let seen = std::thread::spawn(move || {
            use std::io::{Read as _, Write as _};
            let mut line = String::new();
            if let Some(Ok(mut stream)) = server.incoming().next() {
                let mut buf = [0u8; 2048];
                let n = stream.read(&mut buf).unwrap_or(0);
                line = String::from_utf8_lossy(&buf[..n])
                    .lines()
                    .next()
                    .unwrap_or("")
                    .to_owned();
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\ncontent-length: 0\r\n\r\n");
            }
            line
        });
        let (answering, _woken) = Channel::new();
        assert!(answering.begin(
            format!("http://127.0.0.1:{port}/b/abc/sign?x=1#req=secret-token"),
            true,
        ));
        assert!(!check_page(&answering), "an answer, whatever its status");
        assert!(!answering.unreachable());
        let line = seen.join().unwrap_or_default();
        assert_eq!(line, "HEAD /b/abc/sign HTTP/1.1", "no query, no fragment");
        assert_eq!(
            page_address("https://sign.getvela.app/b/abc/sign#req=eyJ"),
            "https://sign.getvela.app/b/abc/sign"
        );
    }

    #[test]
    fn a_cancel_does_not_outlive_its_own_attempt() {
        admitted();
        let (channel, _changed) = Channel::new();
        let first = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        page_of_channel(&channel);
        channel.cancel();
        assert!(
            first
                .join()
                .unwrap_or_else(|_| unreachable!("panicked"))
                .is_err()
        );
        assert!(channel.stopped(), "the cancelled attempt is stopped");
        channel.forget();

        // The next one starts clean: it reaches the point of having a page to
        // open, rather than ending the moment it starts.
        let second = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        let page = page_of_channel(&channel);
        assert!(
            !page.is_empty(),
            "the second attempt never got a page to open"
        );
        assert!(
            !channel.stopped(),
            "a previous cancel is still stopping new attempts"
        );
        channel.cancel();
        let _ = second.join();
    }

    // --- the custom-scheme line (spec 076) ---------------------------------
    //
    // These replace the socket tests that went with `Loopback`. The transport
    // changed because the published page carries `default-src 'none'` in its
    // hashed bytes and cannot open a socket at all; the guarantees that still
    // mean something are re-pinned here on the path actually in use.

    /// The launch URL carries the request in the FRAGMENT and names the
    /// callback — never a query, which a server would see and log.
    #[test]
    fn the_launch_url_hides_the_request_from_the_server() {
        admitted();
        let (channel, _changed) = Channel::new();
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        assert!(url.starts_with(PAGE), "{url}");
        assert!(url.contains("?ch=url"), "{url}");
        assert!(
            url.contains("#i="),
            "the request must ride in the fragment: {url}"
        );
        // Spec 102: the page is told the app's language, in the query — and
        // only that: the request stays in the fragment.
        let (query, _) = url.split_once('#').unwrap_or_default();
        assert!(
            query.ends_with(&format!("&lang={}", crate::loc::app_language())),
            "the app's language is named: {url}"
        );
        let (_, fragment) = url.split_once('#').unwrap_or_default();
        assert!(fragment.contains("cb="), "the callback is named: {url}");
        assert!(
            fragment.contains("t="),
            "the one-time token travels with it: {url}"
        );
        channel.cancel();
        let _ = ceremony.join();
    }

    /// A callback from another attempt cannot answer this one, and the wait
    /// goes on rather than ending on a stranger's word.
    #[test]
    fn a_callback_with_another_token_answers_nothing() {
        admitted();
        let (channel, _changed) = Channel::new();
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        let _ = page_of_channel(&channel);
        let answer = to_base64url(json!({ "signature": "0x30" }).to_string().as_bytes());
        assert!(deliver_callback(&format!(
            "{}?t=not-this-attempt&result={answer}",
            trusted_signer::CALLBACK_URL
        )));
        std::thread::sleep(Duration::from_millis(120));
        assert!(
            channel.ended().is_none(),
            "a stranger's callback ended this attempt"
        );
        channel.cancel();
        let _ = ceremony.join();
    }

    /// A callback is an event for a pending request, never a navigation of
    /// this app: one for a token nobody is waiting on changes nothing, and one
    /// that is not ours at all is not ours to take.
    #[test]
    fn a_callback_for_nobody_changes_nothing() {
        assert!(deliver_callback(&format!(
            "{}?t=nobody-is-waiting-on-this&result=e30",
            trusted_signer::CALLBACK_URL
        )));
        // And something that is not our callback at all is not ours to take.
        assert!(!deliver_callback("velawallet://pay?to=0x1"));
        assert!(!deliver_callback("https://sign.getvela.app/"));
    }

    #[test]
    fn a_cancelled_wait_is_a_cancelled_passkey_and_a_sentence() {
        admitted();
        let (channel, _changed) = Channel::new();
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        page_of_channel(&channel);
        channel.cancel();
        let failure = ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("panicked"))
            .err()
            .unwrap_or_else(|| unreachable!("a cancelled wait signed"));
        assert_eq!(failure.kind, FailureKind::Cancelled);
        assert_eq!(channel.ended(), Some(Refusal::Closed));
        channel.forget();
        assert_eq!(channel.ended(), None);

        channel.close();
        let refused = sign(&json!({}), PAGE, &DIGEST, &keys(), &channel);
        assert!(matches!(
            refused,
            Err(PasskeyFailure {
                kind: FailureKind::Cancelled,
                ..
            })
        ));
        assert_eq!(
            channel.take_page(),
            None,
            "no page for a screen that is gone"
        );
    }

    /// The wallet's own send is told to the page as `wallet_sendCalls` of the
    /// operation's calls, from no site, the fee leg after them; a site's
    /// request keeps its own method, params and origin.
    #[test]
    fn the_request_is_the_sites_own_or_the_wallets_calls() {
        let keys = keys();
        let op = UserOperation {
            sender: "0x88cCA0EeDbF2C4426110bbFc998F048689266894".to_owned(),
            nonce: "0x7".to_owned(),
            init_code: vec![],
            call_data: vec![0x7b, 0xb3, 0x74, 0x28],
            verification_gas_limit: 300_000,
            call_gas_limit: 200_000,
            pre_verification_gas: 110_000,
            max_fee_per_gas: 0,
            max_priority_fee_per_gas: 0,
            paymaster_and_data: vec![],
            signature: vec![],
        };
        let calls = vec![MultiSendCall {
            to: "0x031d7D57c99CAF891e1C250554691Fd12D84772b".to_owned(),
            value_hex: "0x38d7ea4c68000".to_owned(),
            data: vec![],
        }];
        let own = Ask::own(Some("savings".to_owned())).request(
            100,
            &op.sender,
            &keys,
            None,
            Some((&op, &calls)),
        );
        assert_eq!(own["intent"]["method"], "wallet_sendCalls");
        assert_eq!(own["intent"]["origin"], "");
        assert_eq!(own["intent"]["params"][0]["chainId"], "0x64");
        assert_eq!(
            own["intent"]["params"][0]["calls"][0]["value"],
            "0x38d7ea4c68000"
        );
        assert_eq!(own["context"]["operation"]["feeLegIndex"], 1);
        assert_eq!(own["context"]["chainName"], "Gnosis");
        assert_eq!(own["context"]["nativeSymbol"], "xDAI");
        assert_eq!(own["context"]["signer"]["name"], "savings");
        assert_eq!(own["context"]["allowCredentials"][1], "ESIz");

        let site = Ask {
            method: "eth_sendTransaction".to_owned(),
            params: json!([{ "to": calls[0].to, "value": "0x38d7ea4c68000" }]),
            origin: "https://app.uniswap.org".to_owned(),
            account_name: None,
        }
        .request(100, &op.sender, &keys, None, Some((&op, &calls)));
        assert_eq!(site["intent"]["method"], "eth_sendTransaction");
        assert_eq!(site["intent"]["origin"], "https://app.uniswap.org");
        assert_eq!(site["intent"]["params"][0]["to"], calls[0].to.as_str());
        assert_eq!(
            site["context"]["operation"]["userOp"]["callData"],
            "0x7bb37428"
        );
    }

    /// Spec 102 R3: a ceremony runs on the page its op names — and only then.
    /// A `getvela.app` wallet's ops carry none, and run in the app.
    #[test]
    fn a_ceremony_runs_on_the_page_its_op_names() {
        use vela_core::app::shell::ProofPurpose;
        const HOSTED: &str = "http://localhost:8140/clearsigning/";
        let proof = |page: Option<&str>| ShellOperation::SignProof {
            credential_id: "aabb".to_owned(),
            transports: String::new(),
            method: KeyMethod::Hybrid,
            purpose: ProofPurpose::Verify,
            page: page.map(str::to_owned),
        };
        assert_eq!(page_of(&proof(Some(HOSTED))), Some(HOSTED));
        assert_eq!(page_of(&proof(None)), None);
        assert_eq!(page_of(&proof(Some("  "))), None, "an empty page is none");
        let sign_in = ShellOperation::AuthenticatePasskey {
            method: KeyMethod::SecurityKey,
            page: Some(HOSTED.to_owned()),
        };
        assert_eq!(page_of(&sign_in), Some(HOSTED));
        let create = ShellOperation::RegisterPasskey {
            name: "w".to_owned(),
            exclude_credential_ids: Vec::new(),
            method: KeyMethod::Platform,
            page: None,
        };
        assert_eq!(page_of(&create), None);
        assert_eq!(page_of(&ShellOperation::CheckPasskeySupport), None);
    }

    /// **A self-hosted page under a path** is launched under its path, at the
    /// version its check admitted — `http://localhost:8140/clearsigning/` opens
    /// `…/clearsigning/b/<sha256>/sign.html`, never `<origin>/sign.html` (a
    /// 404 and a five-minute wait with nothing to say).
    #[test]
    fn a_self_hosted_page_under_a_path_keeps_its_path() {
        const HOSTED: &str = "http://localhost:8140/clearsigning/";
        crate::executor::signer_integrity::admit_for_tests(HOSTED);
        let (channel, _changed) = Channel::new();
        let attempt = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), HOSTED, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        assert!(
            url.starts_with("http://localhost:8140/clearsigning/b/")
                && url.contains("/sign.html?ch=url&lang="),
            "not the checked page under its path: {url}"
        );
        channel.cancel();
        let _ = attempt.join();
        // And it is the SAME visit, however the origin was written.
        assert!(same_page(HOSTED, "http://localhost:8140"));
    }

    /// R6: a page whose check did not admit it is NOT opened — no URL is
    /// handed to the browser, the attempt ends at once, and the card has the
    /// integrity line that says why.
    #[test]
    fn a_page_whose_check_failed_opens_nothing() {
        // Nothing listens here: the index and the page both fail to fetch.
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(1);
        let base = format!("http://127.0.0.1:{closed}/");
        let (channel, _changed) = Channel::new();
        let failure = sign(&json!({}), &base, &DIGEST, &keys(), &channel)
            .err()
            .unwrap_or_else(|| unreachable!("a page nobody checked signed"));
        assert_eq!(failure.kind, FailureKind::Cancelled);
        assert_eq!(channel.take_page(), None, "a refused page was handed out");
        assert!(!channel.waiting());
        assert_eq!(channel.ended(), Some(Refusal::NotOpened));
        let Some(NotOpened::Integrity { line, page }) = channel.not_opened() else {
            unreachable!("no reason given: {:?}", channel.not_opened())
        };
        assert!(!line.opens);
        assert_eq!(line.key, "componentsUi.signing.integrity.couldNotCheck");
        assert_eq!(page, base, "the refusal names the page it was about");
        // Read, it is gone; the next attempt starts clean.
        channel.forget();
        assert_eq!(channel.not_opened(), None);
    }

    /// Core round 12: a ceremony's cards say what the person is doing on the
    /// page — "Sign in on the signing page" — not the hand-off's "review and
    /// sign"; a signature's say the hand-off's again.
    #[test]
    fn a_ceremonys_cards_carry_its_own_title() {
        let closed = std::net::TcpListener::bind("127.0.0.1:0")
            .and_then(|listener| listener.local_addr())
            .map(|addr| addr.port())
            .unwrap_or(1);
        let base = format!("http://127.0.0.1:{closed}/");
        let (channel, _changed) = Channel::new();
        let sign_in = ShellOperation::AuthenticatePasskey {
            method: KeyMethod::SecurityKey,
            page: Some(base.clone()),
        };
        let ended = run_ceremony(
            &sign_in,
            &base,
            None,
            "https://registry.test",
            &channel,
            true,
        );
        assert!(matches!(ended, Some(Err(_))), "a page nobody checked ran");
        assert_eq!(channel.ended(), Some(Refusal::NotOpened));
        assert_eq!(channel.title_key(), "componentsUi.signing.ceremonySignIn");
        // …and its key row, the core's: the place it signs in with.
        assert_eq!(
            channel.ceremony_key(),
            Some(vela_core::signing_venue::KeyLabel::of_ceremony(
                "security_key",
                false
            ))
        );
        let _ = sign(&json!({}), &base, &DIGEST, &keys(), &channel);
        assert_eq!(channel.title_key(), "componentsUi.signing.handoffTitle");
        assert_eq!(
            channel.ceremony_key(),
            None,
            "a signature's key is the card's"
        );
    }

    /// R5: the request names the key to use and where it lives, so the
    /// browser goes straight to it — and offers only that key.
    #[test]
    fn the_request_names_the_key_and_where_it_lives() {
        let keys = keys();
        let route = KeyRoute::new(&keys[1].credential_id, "security_key", "usb,nfc");
        let request = Ask::own(None).request(100, "0xabc", &keys, Some(&route), None);
        let context = &request["context"];
        assert_eq!(context["keyRoute"]["place"], "security_key", "{context}");
        assert_eq!(context["keyRoute"]["hints"][0], "security-key");
        assert_eq!(
            context["allowCredentials"].as_array().map(Vec::len),
            Some(1),
            "only the sign-in key is offered: {context}"
        );
    }

    /// One page visit is one ORIGIN. The create's launch URL and the origin
    /// its key records differ by a trailing slash, and a flow that compared
    /// them as strings would open a second tab — and, cross-device, a second
    /// QR and a second code — between a create and its member proof.
    #[test]
    fn a_visit_is_the_same_visit_however_its_address_was_written() {
        assert!(same_page(
            "https://sign.getvela.app/",
            "https://sign.getvela.app"
        ));
        assert!(same_page(
            "http://localhost:8140/clearsigning/",
            "http://localhost:8140"
        ));
        assert!(same_page(
            "https://Sign.GetVela.app:443/sign.html?x#y",
            "https://sign.getvela.app"
        ));
        assert!(!same_page(
            "https://sign.getvela.app",
            "https://sign.example.test"
        ));
        assert!(!same_page("not a url", "not a url"));
    }

    // -- the ceremonies ------------------------------------------------------
}
