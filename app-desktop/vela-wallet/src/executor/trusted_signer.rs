//! The Trusted Signer on the desktop: the page answers over **`velawallet://`**
//! (specs 071, 075 and 076).
//!
//! The fourth passkey route is a page, not a key. `app-web/trusted-signer`,
//! opened in the person's default browser, is told a request, derives what it
//! signs itself, runs the ceremony and answers. This file is the desktop's end
//! of that conversation and nothing more:
//!
//! - the request is the core's (`trusted_signer::request` for a signature,
//!   `trusted_signer::ceremony::request` for a create / sign-in / proof);
//! - it rides out in the launch URL's **fragment** — never its query, which a
//!   server would see and log — and the answer comes back as a
//!   `velawallet://sign-result?…` the OS hands this app
//!   (`trusted_signer::url_launch`, then `parse_callback`);
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
use vela_core::app::{Assertion, FailureKind, RegistryPublishMember};
use vela_core::primitives::{to_base64url, to_hex};
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
        }
    }
}

// ---------------------------------------------------------------------------
// The screen and the attempt
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    /// The page this request goes to, when the person chose the Trusted Signer
    /// for it.
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
    /// An attempt owns this channel's surfaces from the moment it opens a
    /// page until it is done.
    claimed: bool,
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

    /// This request's "Sign with": `Some(page)` routes it to the Trusted Signer
    /// at that page, `None` to a passkey. A new choice clears the last
    /// attempt's sentence — it answered a question nobody is asking now.
    pub fn choose(&self, page: Option<String>) {
        self.with(|state| {
            state.chosen = page;
            state.ended = None;
        });
        self.announce();
    }

    /// The page this request goes to, when it goes to the Trusted Signer.
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
    /// same listener still takes its answer.
    pub fn reopen(&self) {
        self.with(|state| state.open = state.waiting.is_some());
        self.announce();
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

    /// The person read the sentence.
    pub fn forget(&self) {
        self.with(|state| state.ended = None);
        self.announce();
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

/// The Trusted Signer page from Settings, or the official one.
#[must_use]
pub fn signer_url() -> String {
    crate::executor::storage::read_value(crate::executor::storage::KEY_TRUSTED_SIGNER_URL)
        .ok()
        .flatten()
        .as_ref()
        .and_then(Value::as_str)
        .and_then(|text| trusted_signer::signer_url(text).ok())
        .unwrap_or_else(|| trusted_signer::DEFAULT_SIGNER_URL.to_owned())
}

/// Look at the signer page in the background, and remember which version was
/// chosen (spec 076 FR-007).
///
/// **Why a thread and not the launch path.** The launch path must not wait on
/// the network: a person pressing Sign would wait out an HTTP round trip, and
/// the first version of this made three unit tests take five minutes. So the
/// check runs here, off to one side, and `open_url` reads what it left.
///
/// **Why at start and not at signing time.** FR-007 wants the check decoupled
/// in time from signing, so a server cannot tell "a verification request just
/// arrived, the next navigation is the target". Running it when the app opens
/// is the simplest shape of that.
///
/// Nothing here can refuse anything yet: `ENFORCE` is false until the page is
/// published at the official address, so this logs and remembers.
pub fn prime_in_background() {
    std::thread::spawn(|| {
        let base = signer_url();
        let verdict = crate::executor::signer_integrity::check(&base);
        eprintln!(
            "[vela-wallet] {}",
            crate::executor::signer_integrity::describe(&verdict)
        );
    });
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
    /// The signer page this visit opens — already resolved to the version the
    /// check knows about (`signer_integrity::open_url`).
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
        self.url = trusted_signer::url_launch(
            &self.page.clone(),
            request,
            trusted_signer::CALLBACK_URL,
            &self.token,
        );
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
            user_op: operation.map(|(op, _)| op),
            calls: operation.map_or(&[][..], |(_, calls)| calls),
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
    expected_member_challenge: Option<&[u8]>,
    registry: &str,
    channel: &Channel,
    last: bool,
) -> Option<Result<Answer, PasskeyFailure>> {
    let ceremony = core_ceremony::Ceremony::of(operation)?;
    let page = signer_page_for(operation);
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

/// Which page this operation's key lives behind.
///
/// **What a key records is an ORIGIN, not a page** — `verify_registration`
/// stamps `origin_of(signer_origin)`, because an origin is what decides the
/// rpId and therefore which keys the page can reach at all. A launch URL needs
/// more than that: a page served under a path (`http://localhost:8140/
/// clearsigning/`, the shape the core's own tests pin) would be launched at
/// `<origin>/sign.html` and 404.
///
/// So the person's page from Settings wins whenever it is the SAME origin —
/// which is every ordinary case, including a self-hosted one they configured.
/// A key recorded behind some other origin is launched from that origin and
/// nothing else is known about it; see this module's note in the report.
fn signer_page_for(operation: &ShellOperation) -> String {
    let named = match operation {
        ShellOperation::SignProof { signer_origin, .. }
        | ShellOperation::SignMemberProof { signer_origin, .. } => signer_origin.clone(),
        _ => None,
    };
    let settings = signer_url();
    let base = match named.filter(|origin| !origin.is_empty()) {
        None => settings,
        Some(origin) if same_page(&origin, &settings) => settings,
        Some(origin) => origin,
    };
    // Spec 076: open the version that was CHECKED, at its content-addressed
    // path, not whatever the address's root happens to serve. Opening
    // `<base>/sign.html` while the check fetched `<base>/b/<hash>/sign.html`
    // means "verified" would refer to bytes the browser never loaded (owner,
    // 2026-09-23: 「路径缺少了 /b/sha256/」).
    //
    // Falls back to the address as typed when no published version can be
    // chosen — a deployment that serves a single page, or one that publishes
    // nothing this build knows.
    crate::executor::signer_integrity::open_url(&base)
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
/// `signer_origin` is the member's OWN page, which the core now carries on
/// every publish member. A key minted on somebody's own deployment is
/// reachable nowhere else, so falling back to whichever page Settings names
/// would refuse a re-publish that has no other way to run.
pub fn member_proof(
    member: &RegistryPublishMember,
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
    };
    let page = member
        .signer_origin
        .as_deref()
        .filter(|origin| !origin.is_empty())
        .map_or_else(signer_url, str::to_owned);
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
        assert!(url.contains("?ch=url#"), "{url}");
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
    #[test]
    fn a_cancel_does_not_outlive_its_own_attempt() {
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
        .request(100, &op.sender, &keys, Some((&op, &calls)));
        assert_eq!(site["intent"]["method"], "eth_sendTransaction");
        assert_eq!(site["intent"]["origin"], "https://app.uniswap.org");
        assert_eq!(site["intent"]["params"][0]["to"], calls[0].to.as_str());
        assert_eq!(
            site["context"]["operation"]["userOp"]["callData"],
            "0x7bb37428"
        );
    }

    /// Which page a proof goes to: the one the key was found behind, and the
    /// person's own page from Settings for everything else.
    ///
    /// **A recorded `signer_origin` is an origin, and a launch URL is not.**
    /// A key found behind the page Settings names is launched from that page,
    /// PATH AND ALL — an origin alone would send a self-hosted page under a
    /// path to `<origin>/sign.html`, which is a 404 and a five-minute wait.
    #[test]
    fn a_proof_goes_to_the_page_its_key_lives_behind() {
        // Its own store: `signer_url()` reads storage, and another test in this
        // file installs a temp state with a self-hosted address. Without a
        // scope of its own this test read whichever store happened to be
        // installed when it ran — a race that was latent until an unrelated
        // test changed the timing and made it fail. A test that depends on
        // ambient state is not testing what it says it is.
        crate::executor::storage::tests::with_temp_state("trusted-signer-proof-page", || {
            use vela_core::app::KeyMethod;
            use vela_core::app::shell::ProofPurpose;
            let proof = |origin: Option<&str>| ShellOperation::SignProof {
                credential_id: "aabb".to_owned(),
                transports: String::new(),
                method: KeyMethod::TrustedSigner,
                purpose: ProofPurpose::Verify,
                signer_origin: origin.map(str::to_owned),
            };
            // A key behind somebody else's page: all this side knows is the origin.
            assert_eq!(
                signer_page_for(&proof(Some("https://sign.example.test"))),
                "https://sign.example.test"
            );
            // A key behind the page Settings names — the ordinary case — is
            // launched from the Settings URL, which is the one with the path.
            assert_eq!(
                signer_page_for(&proof(Some("https://sign.getvela.app"))),
                signer_url()
            );
            assert_eq!(signer_page_for(&proof(None)), signer_url());
            let anywhere = ShellOperation::AuthenticatePasskey {
                method: KeyMethod::TrustedSigner,
            };
            assert_eq!(signer_page_for(&anywhere), signer_url());
        });
    }

    /// **A self-hosted page under a path.** The core records an ORIGIN on a
    /// key, because an origin is what decides the rpId; a launch URL needs the
    /// path too. `http://localhost:8140/clearsigning/` — the shape the core's
    /// own tests pin — records as `http://localhost:8140`, and launching from
    /// that alone asks for `/sign.html` at a server that serves the page at
    /// `/clearsigning/sign.html`: a 404, then a five-minute wait with nothing
    /// to say. The person's own page wins whenever it is the same origin, and
    /// the visit the create opened is the visit the member proof rides on.
    #[test]
    fn a_self_hosted_page_under_a_path_keeps_its_path() {
        use vela_core::app::KeyMethod;
        use vela_core::app::shell::ProofPurpose;
        crate::executor::storage::tests::with_temp_state("trusted-signer-hosted-path", || {
            const HOSTED: &str = "http://localhost:8140/clearsigning/";
            let _ = crate::executor::storage::write_value(
                crate::executor::storage::KEY_TRUSTED_SIGNER_URL,
                Value::String(HOSTED.to_owned()),
            );
            assert_eq!(
                signer_url(),
                HOSTED,
                "Settings holds the page with its path"
            );

            let proof = ShellOperation::SignProof {
                credential_id: "aabb".to_owned(),
                transports: String::new(),
                method: KeyMethod::TrustedSigner,
                purpose: ProofPurpose::Verify,
                // What a key minted on that page actually records.
                signer_origin: Some("http://localhost:8140".to_owned()),
            };
            let page = signer_page_for(&proof);
            assert_eq!(page, HOSTED, "the path was dropped");

            // The launch URL the visit would open, built by the core that builds
            // it — no socket, no browser. The property under test is the PATH.
            let launch = trusted_signer::url_launch(
                &page,
                &serde_json::json!({ "t": "req" }),
                trusted_signer::CALLBACK_URL,
                "token",
            );
            assert!(
                launch.starts_with("http://localhost:8140/clearsigning/sign.html?ch=url#i="),
                "the launch URL is a 404: {launch}"
            );
            // And it is the SAME visit the create opened, so nothing is torn
            // down between the two ceremonies.
            assert!(same_page(HOSTED, "http://localhost:8140"));
        });
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
