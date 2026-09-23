//! The Clear Signer on the desktop: a **loopback WebSocket** on this device
//! (specs 071 and 075).
//!
//! The fourth passkey route is a page, not a key. `app-web/clearsigning`,
//! opened in the person's default browser, is told a request, derives what it
//! signs itself, runs the ceremony and answers. This file is the desktop's end
//! of that conversation and nothing more:
//!
//! - the request is the core's (`clear_signer::request` for a signature,
//!   `clear_signer::ceremony::request` for a create / sign-in / proof);
//! - the wire is the core's too — [`ws::Connection`] frames every byte of the
//!   loopback socket;
//! - whether an answer is this wallet's signature over this request
//!   (`clear_signer::verify`) or a ceremony the page was allowed to run
//!   (`ceremony::verify`) is the core's.
//!
//! What is left here is a listener, a socket, a clock, and the screen.
//!
//! ## Spec 075: the desktop moved off the URL fragment
//!
//! 071 put the request in the URL's fragment and took the answer on a loopback
//! HTTP callback. That carried exactly one request per page visit, which a
//! create (key, then member proof) and a sign-in (assertion, then recovery's
//! two proofs) cannot use: each would open a new tab and ask for the passkey
//! again. The owner's call — "我觉得任何端都能接入 websocket 吧" — is the phones'
//! channel for every shell, so the desktop speaks it too: **one page visit per
//! flow**, several requests in turn over one socket, `bye` at the end. The page
//! still supports the fragment; nothing here uses it.
//!
//! ## Who may talk to the listener
//!
//! Bound to 127.0.0.1 on a port the OS picks — never another interface — one
//! listener per page visit. The core's [`ws::Connection`] refuses any upgrade
//! whose `Origin` is not the signer page's, and then any `hello` that does not
//! carry this visit's one-time token: such a connection is closed and has no
//! outcome, so a stray program on this machine cannot end a flow.
//!
//! The token belongs to the VISIT, not to a request — which is what makes
//! "Open the page again" work, and means an earlier tab of the same visit can
//! still answer. That is deliberate; what it must not do is cost the person
//! their signature, so an answer always beats a close in the same breath
//! ([`keep_best`]) and the tab in front of them is never the one hung up on
//! ([`Loopback::accept`]). Connections are read without blocking, so one that
//! never finishes its handshake cannot hold the real answer up behind it.
//!
//! ## What the screen sees
//!
//! [`Channel`] is what a screen and its ceremonies say to each other: whether
//! this request goes to the Clear Signer and to which page, where the person
//! keeps it (this device or another), the URL to hand the browser, the pairing
//! link and the six-digit code to confirm, that a page is being waited on,
//! "Cancel", and how the last attempt ended without an answer. The screen
//! words the ending; the core hears a cancelled passkey, so the request stays
//! open to be answered another way.

use std::io::{self, Read as _, Write as _};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde_json::Value;

use vela_core::app::network_admin::BUILTIN_CHAINS;
use vela_core::app::shell::ShellOperation;
use vela_core::app::{Assertion, FailureKind, RegistryPublishMember};
use vela_core::clear_signer::{
    self, ClearSignerError, RequestInput, Verified, ceremony as core_ceremony, verify, ws,
};
use vela_core::primitives::{to_base64url, to_hex};
use vela_core::user_op::{MultiSendCall, UserOperation, WalletKey};

use crate::executor::passkey::{self, PasskeyFailure};

/// How long a page has to answer one request. The core has no clock; this is
/// the shell's, and it matches the page's own idle timeout.
pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How often a wait looks at the socket, the clock and the screen's buttons.
pub(crate) const POLL: Duration = Duration::from_millis(50);
/// Connections held open at once on one visit's listener. Past this the
/// oldest is hung up on — see [`Loopback::accept`].
const CANDIDATES: usize = 4;
/// How long one write to a socket on this machine may take before it counts
/// as gone.
const WRITE_DEADLINE: Duration = Duration::from_secs(1);

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
    pub fn of(error: &ClearSignerError) -> Self {
        match error {
            ClearSignerError::Declined => Self::Closed,
            ClearSignerError::Refused(_) => Self::Refused,
            ClearSignerError::Malformed(_)
            | ClearSignerError::WrongChallenge
            | ClearSignerError::NotVerified
            | ClearSignerError::ForeignKey
            | ClearSignerError::BadSignature
            | ClearSignerError::WrongToken => Self::Mismatch,
        }
    }

    /// The corpus key it is said with.
    #[must_use]
    pub fn key(self) -> &'static str {
        match self {
            Self::Closed => "componentsUi.signing.clearSignerClosed",
            Self::Refused => "componentsUi.signing.clearSignerRefused",
            Self::Mismatch => "componentsUi.signing.clearSignerMismatch",
            Self::TimedOut => "componentsUi.signing.clearSignerTimeout",
        }
    }
}

// ---------------------------------------------------------------------------
// The screen and the attempt
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    /// The page this request goes to, when the person chose the Clear Signer
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

/// What one screen and its Clear Signer attempts say to each other.
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

    /// This request's "Sign with": `Some(page)` routes it to the Clear Signer
    /// at that page, `None` to a passkey. A new choice clears the last
    /// attempt's sentence — it answered a question nobody is asking now.
    pub fn choose(&self, page: Option<String>) {
        self.with(|state| {
            state.chosen = page;
            state.ended = None;
        });
        self.announce();
    }

    /// The page this request goes to, when it goes to the Clear Signer.
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
            .name("clear-signer-bye".to_owned())
            .spawn(move || open.line.end());
        if let Err(error) = farewell {
            eprintln!("[vela-wallet] clear signer: could not say goodbye: {error}");
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
                "[vela-wallet] clear signer: a second attempt asked for a screen \
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

/// The Clear Signer page from Settings, or the official one.
#[must_use]
pub fn signer_url() -> String {
    crate::executor::storage::read_value(crate::executor::storage::KEY_CLEAR_SIGNER_URL)
        .ok()
        .flatten()
        .as_ref()
        .and_then(Value::as_str)
        .and_then(|text| clear_signer::signer_url(text).ok())
        .unwrap_or_else(|| clear_signer::DEFAULT_SIGNER_URL.to_owned())
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
    let Some(query) = url
        .strip_prefix(clear_signer::CALLBACK_URL)
        .and_then(|rest| rest.strip_prefix('?'))
    else {
        return false;
    };
    let Some(token) = query
        .split('&')
        .find_map(|pair| pair.strip_prefix("t="))
        .map(str::to_owned)
    else {
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
            token: to_base64url(&passkey::random(16)),
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
        self.url = clear_signer::url_launch(
            &self.page.clone(),
            request,
            clear_signer::CALLBACK_URL,
            &self.token,
        );
        // Nothing left over from a previous visit may be read as this one's.
        forget_answer(&self.token);
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
                break clear_signer::parse_callback(&query, &self.token).map_err(|error| {
                    if !matches!(error, ClearSignerError::Declined) {
                        eprintln!("[vela-wallet] clear signer: answer not accepted: {error}");
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
// This device: the loopback WebSocket
// ---------------------------------------------------------------------------

/// The wallet's listener for one page visit, and the page's socket on it.
pub struct Loopback {
    listener: TcpListener,
    page: String,
    token: String,
    url: String,
    /// Connections that have not closed themselves. The one that answers
    /// becomes the session's and the rest are dropped.
    wires: Vec<Wire>,
    /// The request a connection arriving now is to be given — the first of the
    /// visit, or the one after a page that had to be reopened.
    pending: Option<(String, Value)>,
    seq: u64,
}

struct Wire {
    stream: TcpStream,
    conn: ws::Connection,
}

impl Loopback {
    /// A port the OS picks, on this machine's loopback and nowhere else.
    pub fn open(page: &str) -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        // Accepting never blocks: between connections the wait looks at the
        // clock and at the screen's buttons.
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        let token = to_base64url(&passkey::random(16));
        let url = clear_signer::ws_launch(page, port, &token);
        Ok(Self {
            listener,
            page: page.to_owned(),
            token,
            url,
            wires: Vec::new(),
            pending: None,
            seq: 0,
        })
    }

    /// Take every connection waiting. A connection arriving when nothing is
    /// being asked for is not this visit's and is dropped.
    ///
    /// **Past the cap the OLDEST goes, not the newest.** Refusing the newest
    /// was the wrong way round: nothing in this list has answered (a wire that
    /// answers ends the request), so the newest is the likeliest to be the tab
    /// the person is looking at. A person who presses "Open the page again"
    /// four times — the natural thing to do when nothing seems to happen —
    /// would have filled every slot with abandoned tabs and had the fifth, the
    /// live one, hung up on.
    fn accept(&mut self) {
        while let Ok((stream, _)) = self.listener.accept() {
            let Some((id, request)) = self.pending.clone() else {
                let _ = stream.shutdown(Shutdown::Both);
                continue;
            };
            if stream.set_nonblocking(true).is_err() {
                let _ = stream.shutdown(Shutdown::Both);
                continue;
            }
            while self.wires.len() >= CANDIDATES {
                let stale = self.wires.remove(0);
                let _ = stale.stream.shutdown(Shutdown::Both);
            }
            self.wires.push(Wire {
                stream,
                conn: ws::Connection::new(&self.page, &self.token, &id, &request),
            });
        }
    }

    /// Read every wire once. `Some` when one of them settled this request.
    fn pump(&mut self) -> Option<Result<Value, ClearSignerError>> {
        let mut settled: Option<(usize, Result<Value, ClearSignerError>)> = None;
        let mut dead: Vec<usize> = Vec::new();
        for (index, wire) in self.wires.iter_mut().enumerate() {
            let mut buffer = [0u8; 8192];
            loop {
                match wire.stream.read(&mut buffer) {
                    Ok(0) => {
                        if let Some(error) = wire.conn.closed() {
                            keep_best(&mut settled, index, Err(error));
                        }
                        dead.push(index);
                        break;
                    }
                    Ok(read) => {
                        let step = wire.conn.feed(&buffer[..read]);
                        if !write_all(&mut wire.stream, &step.write) || step.close {
                            dead.push(index);
                        }
                        if let Some(outcome) = step.outcome {
                            keep_best(&mut settled, index, outcome);
                        }
                        if step.close {
                            break;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        if let Some(error) = wire.conn.closed() {
                            keep_best(&mut settled, index, Err(error));
                        }
                        dead.push(index);
                        break;
                    }
                }
            }
        }
        match settled {
            // The wire that answered is the session's; the others — an old tab
            // still holding a socket after "Open the page again" — are dropped.
            Some((index, outcome)) => {
                let kept = self.wires.swap_remove(index);
                self.wires.clear();
                self.wires.push(kept);
                self.pending = None;
                Some(outcome)
            }
            None => {
                // A wire that closed before proving itself changes nothing —
                // UNLESS it was the only one left, in which case the page
                // really did go away and the next ask reopens it.
                dead.sort_unstable();
                dead.dedup();
                for index in dead.into_iter().rev() {
                    if index < self.wires.len() {
                        self.wires.swap_remove(index);
                    }
                }
                None
            }
        }
    }
}

/// Which of two verdicts in one pump is this request's.
///
/// **An answer beats a refusal, whatever order they arrive in.** Two wires
/// exist exactly when the person pressed "Open the page again", and both were
/// handed the same request — so the tab they signed on can answer in the same
/// 50 ms in which the tab they abandoned closes. A single slot, overwritten,
/// threw the signature away and told them they had closed the page. For a
/// create that is a passkey minted in the browser and then dropped.
///
/// Between two of a kind the first wins: whoever got there is the answer.
fn keep_best(
    settled: &mut Option<(usize, Result<Value, ClearSignerError>)>,
    index: usize,
    outcome: Result<Value, ClearSignerError>,
) {
    let better = matches!(
        (settled.as_ref(), &outcome),
        (None, _) | (Some((_, Err(_))), Ok(_))
    );
    if better {
        *settled = Some((index, outcome));
    }
}

/// Write every byte, or say the socket is gone.
fn write_all(stream: &mut TcpStream, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    // The socket is non-blocking for reads; a short write on a loopback
    // socket carrying a few kilobytes is rare but not impossible. A second is
    // already an eternity for a write to this machine — and `end` walks every
    // wire, so a longer wait would be paid several times over on a teardown.
    let deadline = Instant::now() + WRITE_DEADLINE;
    let mut at = 0;
    while at < bytes.len() {
        match stream.write(&bytes[at..]) {
            Ok(0) => return false,
            Ok(wrote) => at += wrote,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    return false;
                }
                std::thread::sleep(POLL);
            }
            Err(_) => return false,
        }
    }
    let _ = stream.flush();
    true
}

impl Line for Loopback {
    fn next_id(&mut self) -> String {
        self.seq += 1;
        format!("d{}", self.seq)
    }

    fn ask(
        &mut self,
        id: &str,
        request: &Value,
        channel: &Channel,
        deadline: Instant,
    ) -> Result<Value, Refusal> {
        // Asked before a byte goes out: a person who pressed Cancel in the
        // breath before this would otherwise get a passkey prompt on the page
        // for a request the wallet has already given up on.
        if channel.stopped() {
            return Err(Refusal::Closed);
        }
        // A session already up takes the next request on the same socket; the
        // page leaves its done card for the new one without being reopened.
        if let Some(wire) = self.wires.first_mut() {
            let step = wire.conn.send(id, request);
            let delivered = !step.write.is_empty() && write_all(&mut wire.stream, &step.write);
            if !delivered {
                self.wires.clear();
            }
        }
        let reopening = self.wires.is_empty();
        if reopening {
            self.pending = Some((id.to_owned(), request.clone()));
        }
        if !channel.begin(self.url.clone(), reopening) {
            return Err(Refusal::Closed);
        }
        loop {
            if channel.stopped() {
                return Err(Refusal::Closed);
            }
            if Instant::now() >= deadline {
                return Err(Refusal::TimedOut);
            }
            self.accept();
            if let Some(outcome) = self.pump() {
                channel.rest();
                return outcome.map_err(|error| {
                    if !matches!(error, ClearSignerError::Declined) {
                        eprintln!("[vela-wallet] clear signer: answer not accepted: {error}");
                    }
                    Refusal::of(&error)
                });
            }
            std::thread::sleep(POLL);
        }
    }

    fn end(&mut self) {
        for wire in &mut self.wires {
            let step = wire.conn.end();
            write_all(&mut wire.stream, &step.write);
            let _ = wire.stream.shutdown(Shutdown::Both);
        }
        self.wires.clear();
        self.pending = None;
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
        clear_signer::request(&RequestInput {
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

/// One signature through the Clear Signer, start to finish: ask where the page
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
            eprintln!("[vela-wallet] clear signer: answer not accepted: {error}");
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
    eprintln!("[vela-wallet] clear signer: ended without an answer ({refusal:?})");
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
pub use vela_core::clear_signer::ceremony::Answer;

/// One passkey ceremony through the Clear Signer — a create, a sign-in, a
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
    let request = core_ceremony::request(ceremony, &id, wallet_name, registry);
    let answered = visit.line.ask(&id, &request, channel, deadline);
    let outcome = answered.and_then(|answer| {
        core_ceremony::verify(ceremony, &answer, page, expected_member_challenge).map_err(|error| {
            eprintln!("[vela-wallet] clear signer: ceremony not accepted: {error}");
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
            "the Clear Signer answered a member proof with a new key",
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

    /// The page's answer to a CEREMONY: an assertion over the challenge the
    /// page derived, in the ceremony reply's field shapes (§10.3).
    pub(crate) fn page_assertion(
        signing: &SigningKey,
        credential: &[u8],
        challenge: &[u8],
    ) -> Value {
        let (authenticator_data, client, signature) = signed(signing, challenge, "webauthn.get");
        json!({
            "credentialId": to_base64url(credential),
            "signatureDer": to_hex(&signature, false),
            "authenticatorData": to_hex(&authenticator_data, false),
            "clientDataJSON": to_hex(client.as_bytes(), false),
            "userHandle": Value::Null,
            "authenticatorAttachment": "platform",
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

    // -- the page, as a test drives it ---------------------------------------

    /// The port and the token out of a `ch=ws` launch URL's fragment.
    pub(crate) fn launch_of(url: &str) -> (u16, String) {
        assert!(url.contains("sign.html?ch=ws#"), "{url}");
        let fragment = url.split_once('#').map_or("", |(_, f)| f);
        let field = |name: &str| {
            fragment
                .split('&')
                .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
                .unwrap_or_default()
                .to_owned()
        };
        (field("p").parse().unwrap_or_default(), field("t"))
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
                clear_signer::CALLBACK_URL
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
                clear_signer::CALLBACK_URL
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

    /// A stand-in signer page on the wallet's loopback socket: the RFC 6455
    /// client half, and the session's messages.
    pub(crate) struct FakePage {
        stream: TcpStream,
        inbox: Vec<u8>,
    }

    impl FakePage {
        /// Connect as the page does — the signer page's `Origin`, and the
        /// token out of the fragment.
        pub(crate) fn connect(port: u16, token: &str, origin: &str) -> Self {
            let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
            let mut page = Self {
                stream,
                inbox: Vec::new(),
            };
            let head = format!(
                "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\n\
                 Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                 Sec-WebSocket-Version: 13\r\nOrigin: {origin}\r\n\r\n"
            );
            page.write(head.as_bytes());
            let response = page.read_head();
            assert!(
                response.starts_with("HTTP/1.1 101"),
                "the wallet refused the upgrade: {response}"
            );
            page.say(&json!({ "v": 1, "t": "hello", "token": token }));
            page
        }

        /// The upgrade attempt alone, for a page the wallet must turn away.
        pub(crate) fn refused(port: u16, origin: &str) -> String {
            let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port))
                .unwrap_or_else(|e| unreachable!("{e}"));
            let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
            let mut page = Self {
                stream,
                inbox: Vec::new(),
            };
            let head = format!(
                "GET / HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nUpgrade: websocket\r\n\
                 Connection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\n\
                 Sec-WebSocket-Version: 13\r\nOrigin: {origin}\r\n\r\n"
            );
            page.write(head.as_bytes());
            page.read_head()
        }

        fn write(&mut self, bytes: &[u8]) {
            let _ = self.stream.write_all(bytes);
            let _ = self.stream.flush();
        }

        fn read_head(&mut self) -> String {
            let mut chunk = [0u8; 4096];
            loop {
                if let Some(end) = self
                    .inbox
                    .windows(4)
                    .position(|window| window == b"\r\n\r\n")
                {
                    let head = String::from_utf8_lossy(&self.inbox[..end]).into_owned();
                    self.inbox.drain(..end + 4);
                    return head;
                }
                match self.stream.read(&mut chunk) {
                    Ok(0) | Err(_) => return String::from_utf8_lossy(&self.inbox).into_owned(),
                    Ok(read) => self.inbox.extend_from_slice(&chunk[..read]),
                }
            }
        }

        /// One text message, masked as every client frame must be (§5.1).
        pub(crate) fn say(&mut self, message: &Value) {
            let payload = message.to_string().into_bytes();
            let mut frame = vec![0x81];
            match payload.len() {
                n if n < 126 => frame.push(0x80 | n as u8),
                n if n <= 0xFFFF => {
                    frame.push(0x80 | 126);
                    frame.extend_from_slice(&(n as u16).to_be_bytes());
                }
                n => {
                    frame.push(0x80 | 127);
                    frame.extend_from_slice(&(n as u64).to_be_bytes());
                }
            }
            let mask = [0x37, 0xfa, 0x21, 0x3d];
            frame.extend_from_slice(&mask);
            frame.extend(
                payload
                    .iter()
                    .enumerate()
                    .map(|(i, byte)| byte ^ mask[i % 4]),
            );
            self.write(&frame);
        }

        /// The next message the wallet sent, as JSON. `None` when the socket
        /// closed first.
        pub(crate) fn next(&mut self) -> Option<Value> {
            let mut chunk = [0u8; 8192];
            loop {
                if let Some(message) = self.take_frame() {
                    return message;
                }
                match self.stream.read(&mut chunk) {
                    Ok(0) | Err(_) => return None,
                    Ok(read) => self.inbox.extend_from_slice(&chunk[..read]),
                }
            }
        }

        /// A whole server frame off the front of the inbox, if one is there:
        /// `Some(Some(json))` for text, `Some(None)` for a close.
        fn take_frame(&mut self) -> Option<Option<Value>> {
            if self.inbox.len() < 2 {
                return None;
            }
            let opcode = self.inbox[0] & 0x0F;
            let (length, at) = match self.inbox[1] & 0x7F {
                126 => (
                    u16::from_be_bytes(self.inbox.get(2..4)?.try_into().ok()?) as usize,
                    4,
                ),
                127 => (
                    u64::from_be_bytes(self.inbox.get(2..10)?.try_into().ok()?) as usize,
                    10,
                ),
                short => (short as usize, 2),
            };
            let payload = self.inbox.get(at..at + length)?.to_vec();
            self.inbox.drain(..at + length);
            match opcode {
                0x1 => Some(serde_json::from_slice(&payload).ok()),
                0x8 => Some(None),
                // A ping or a pong is not a message.
                _ => self.take_frame(),
            }
        }

        pub(crate) fn hang_up(&mut self) {
            let _ = self.stream.shutdown(Shutdown::Both);
        }
    }

    // -- the listener --------------------------------------------------------








    /// "Cancel" ends the attempt as the person declining: a cancelled passkey
    /// to the core, "closed" to the sheet — and a screen that is gone starts
    /// no attempt at all.
    #[test]
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
        assert!(url.contains("#i="), "the request must ride in the fragment: {url}");
        let (_, fragment) = url.split_once('#').unwrap_or_default();
        assert!(fragment.contains("cb="), "the callback is named: {url}");
        assert!(fragment.contains("t="), "the one-time token travels with it: {url}");
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
            clear_signer::CALLBACK_URL
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
            clear_signer::CALLBACK_URL
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
        crate::executor::storage::tests::with_temp_state("clear-signer-proof-page", || {
            use vela_core::app::KeyMethod;
            use vela_core::app::shell::ProofPurpose;
            let proof = |origin: Option<&str>| ShellOperation::SignProof {
                credential_id: "aabb".to_owned(),
                transports: String::new(),
                method: KeyMethod::ClearSigner,
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
                method: KeyMethod::ClearSigner,
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
        crate::executor::storage::tests::with_temp_state("clear-signer-hosted-path", || {
            const HOSTED: &str = "http://localhost:8140/clearsigning/";
            let _ = crate::executor::storage::write_value(
                crate::executor::storage::KEY_CLEAR_SIGNER_URL,
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
                method: KeyMethod::ClearSigner,
                purpose: ProofPurpose::Verify,
                // What a key minted on that page actually records.
                signer_origin: Some("http://localhost:8140".to_owned()),
            };
            let page = signer_page_for(&proof);
            assert_eq!(page, HOSTED, "the path was dropped");

            let loopback = Loopback::open(&page).unwrap_or_else(|e| unreachable!("{e}"));
            assert!(
                loopback
                    .url
                    .starts_with("http://localhost:8140/clearsigning/sign.html?ch=ws#p="),
                "the launch URL is a 404: {}",
                loopback.url
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

    /// A registration the page could really have returned: `fmt:"none"`, an
    /// authData carrying the credential id and the key as a COSE_Key, over a
    /// `webauthn.create` client data from the signer page's origin. The core
    /// parses the attestation to a P-256 key before it accepts anything, so a
    /// hand-waved blob would not do.
    fn page_registration(signing: &SigningKey, credential: &[u8]) -> Value {
        let point = signing.verifying_key().to_encoded_point(false);
        let (x, y) = (
            point.x().unwrap_or_else(|| unreachable!("x")),
            point.y().unwrap_or_else(|| unreachable!("y")),
        );
        let mut cose = vec![0xa5, 0x01, 0x02, 0x03, 0x26, 0x20, 0x01, 0x21, 0x58, 0x20];
        cose.extend_from_slice(x);
        cose.extend_from_slice(&[0x22, 0x58, 0x20]);
        cose.extend_from_slice(y);

        let mut auth_data = sha256(b"getvela.app");
        // UP | UV | AT
        auth_data.push(0x45);
        auth_data.extend_from_slice(&[0, 0, 0, 1]);
        auth_data.extend_from_slice(&[0u8; 16]);
        auth_data.extend_from_slice(&[
            (credential.len() >> 8) as u8,
            u8::try_from(credential.len()).unwrap_or(0),
        ]);
        auth_data.extend_from_slice(credential);
        auth_data.extend_from_slice(&cose);

        let cbor_text = |text: &str| {
            let mut out = vec![0x60 | u8::try_from(text.len()).unwrap_or(0)];
            out.extend_from_slice(text.as_bytes());
            out
        };
        let mut attestation = vec![0xa3];
        attestation.extend(cbor_text("fmt"));
        attestation.extend(cbor_text("none"));
        attestation.extend(cbor_text("attStmt"));
        attestation.push(0xa0);
        attestation.extend(cbor_text("authData"));
        attestation.extend_from_slice(&[0x59, (auth_data.len() >> 8) as u8, auth_data.len() as u8]);
        attestation.extend_from_slice(&auth_data);

        let client = format!(
            r#"{{"type":"webauthn.create","challenge":"{}","origin":"https://sign.getvela.app","crossOrigin":false}}"#,
            to_base64url(&[0x5c; 32])
        );
        json!({
            "credentialId": to_base64url(credential),
            "attestationObject": to_hex(&attestation, false),
            "clientDataJSON": to_hex(client.as_bytes(), false),
            "authenticatorAttachment": "platform",
            "transports": "internal",
        })
    }



}
