//! The Clear Signer on the desktop: a **loopback WebSocket** on this device,
//! a **relay** to another (specs 071 and 075).
//!
//! The fourth passkey route is a page, not a key. `app-web/clearsigning`,
//! opened in the person's default browser, is told a request, derives what it
//! signs itself, runs the ceremony and answers. This file is the desktop's end
//! of that conversation and nothing more:
//!
//! - the request is the core's (`clear_signer::request` for a signature,
//!   `clear_signer::ceremony::request` for a create / sign-in / proof);
//! - the wire is the core's too — [`ws::Connection`] frames every byte of the
//!   loopback socket, `secure::Session` seals every byte of the relay;
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
//! outcome, so a stray program or a spent tab cannot end a flow. Connections
//! are read without blocking, so one that never finishes its handshake cannot
//! hold the real answer up behind it.
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
use vela_core::app::{Assertion, FailureKind};
use vela_core::clear_signer::{
    self, ClearSignerError, RequestInput, Verified, ceremony as core_ceremony, verify, ws,
};
use vela_core::primitives::{to_base64url, to_hex};
use vela_core::user_op::{MultiSendCall, UserOperation, WalletKey};

use crate::executor::clear_signer_relay as relay;
use crate::executor::passkey::{self, PasskeyFailure};

/// How long a page has to answer one request. The core has no clock; this is
/// the shell's, and it matches the page's own idle timeout.
pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// How often a wait looks at the socket, the clock and the screen's buttons.
pub(crate) const POLL: Duration = Duration::from_millis(50);
/// Connections held open at once on one visit's listener. Past this a
/// connection is noise, accepted and dropped.
const CANDIDATES: usize = 4;

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
    /// Spec 075: the relay could not be reached, or dropped the pairing.
    Unreachable,
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
            Self::Unreachable => "componentsUi.signing.clearSignerRelayDown",
        }
    }
}

/// Where the person keeps their Clear Signer (contract §2). Asked whenever the
/// route is chosen, because both are possible on every desktop.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Place {
    /// This computer's own browser, over the loopback socket.
    ThisDevice,
    /// Another device, paired through the relay.
    OtherDevice,
}

/// A cross-device pairing, while one is on screen: the link the QR carries,
/// the six digits once both ends have derived them, and whether the person has
/// said they match.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pairing {
    /// `<page>sign.html?ch=relay#relay=…&room=…&rk=…&v=1`.
    pub link: String,
    /// The six digits both screens show. `None` until the page has said hello.
    pub code: Option<String>,
    /// The person pressed "the codes match".
    pub confirmed: bool,
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
    /// "Where is your Clear Signer?" is on screen, and a ceremony is blocked
    /// on the answer.
    asking_place: bool,
    /// The answer, until the ceremony takes it.
    place: Option<Place>,
    /// The URL a same-device attempt is waiting on — `Some` while one waits.
    waiting: Option<String>,
    /// That URL is to be handed to the browser (again).
    open: bool,
    /// The cross-device pairing, while one is up.
    pairing: Option<Pairing>,
    /// The person stopped this wait.
    cancelled: bool,
    /// The screen is gone: no wait continues, none starts.
    closed: bool,
    /// How the last attempt ended without an answer, until the person has read
    /// it.
    ended: Option<Refusal>,
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

    /// A page is being waited on — either way.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.with(|state| state.waiting.is_some() || state.pairing.is_some())
    }

    /// "Where is your Clear Signer?" is on screen.
    #[must_use]
    pub fn asking_place(&self) -> bool {
        self.with(|state| state.asking_place)
    }

    /// The person answered it. `None` is a dismissal, which stops the attempt.
    pub fn answer_place(&self, place: Option<Place>) {
        self.with(|state| match place {
            Some(place) => {
                state.place = Some(place);
                state.asking_place = false;
            }
            None => {
                state.cancelled = true;
                state.asking_place = false;
            }
        });
        self.announce();
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

    /// The pairing on screen, if one is up.
    #[must_use]
    pub fn pairing(&self) -> Option<Pairing> {
        self.with(|state| state.pairing.clone())
    }

    /// "The codes match" — the one gate before anything is sent over a relay
    /// (relay.md §2.4).
    pub fn confirm_code(&self) {
        self.with(|state| {
            if let Some(pairing) = state.pairing.as_mut() {
                pairing.confirmed = pairing.code.is_some();
            }
        });
        self.announce();
    }

    /// "Cancel" on a waiting sheet: the person declined.
    pub fn cancel(&self) {
        self.with(|state| {
            state.cancelled = true;
            state.asking_place = false;
        });
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
    /// starts — an attempt nobody can see must not hold a port, or somebody
    /// else's relay room, for minutes. Any page visit a flow was holding open
    /// is ended too.
    pub fn close(&self) {
        self.with(|state| {
            state.closed = true;
            state.asking_place = false;
        });
        end_flow();
        self.announce();
    }

    // -- the attempt's half ---------------------------------------------------

    /// Ask the person where their Clear Signer is, and block until they say.
    ///
    /// Polled rather than parked on a condvar, like every other wait in this
    /// file: the same loop has to watch the clock and the "Cancel" anyway.
    fn ask_place(&self, deadline: Instant) -> Result<Place, Refusal> {
        if self.with(|state| state.closed) {
            return Err(Refusal::Closed);
        }
        self.with(|state| {
            state.asking_place = true;
            state.place = None;
            state.cancelled = false;
            state.ended = None;
        });
        self.announce();
        loop {
            if let Some(place) = self.with(|state| state.place.take()) {
                return Ok(place);
            }
            if self.stopped() {
                self.with(|state| state.asking_place = false);
                return Err(Refusal::Closed);
            }
            if Instant::now() >= deadline {
                self.with(|state| state.asking_place = false);
                return Err(Refusal::TimedOut);
            }
            std::thread::sleep(POLL);
        }
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

    /// A pairing starts: the link on screen as a QR and a copyable address.
    pub(crate) fn pair(&self, link: String) -> bool {
        let began = self.with(|state| {
            if state.closed {
                return false;
            }
            state.pairing = Some(Pairing {
                link,
                code: None,
                confirmed: false,
            });
            state.ended = None;
            true
        });
        self.announce();
        began
    }

    /// Both ends derived the same six digits; the person is asked to check them.
    pub(crate) fn show_code(&self, code: &str) {
        self.with(|state| {
            if let Some(pairing) = state.pairing.as_mut() {
                pairing.code = Some(code.to_owned());
            }
        });
        self.announce();
    }

    pub(crate) fn code_confirmed(&self) -> bool {
        self.with(|state| state.pairing.as_ref().is_some_and(|p| p.confirmed))
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
            state.pairing = None;
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

/// One page visit, whichever way it runs: the loopback socket on this device,
/// or the relay to another. A visit carries **several requests in order** —
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

/// Open a line to `page`, asking the person where their Clear Signer is first.
fn open_line(page: &str, channel: &Channel, deadline: Instant) -> Result<Box<dyn Line>, Refusal> {
    match channel.ask_place(deadline)? {
        Place::ThisDevice => Loopback::open(page)
            .map(|loopback| Box::new(loopback) as Box<dyn Line>)
            .map_err(|error| {
                eprintln!("[vela-wallet] clear signer: could not listen: {error}");
                Refusal::Unreachable
            }),
        Place::OtherDevice => {
            relay::open(page, &relay_url(), relay::Seed::random(), channel, deadline)
                .map(|line| Box::new(line) as Box<dyn Line>)
        }
    }
}

/// The relay from Settings, or the official one.
#[must_use]
pub fn relay_url() -> String {
    crate::executor::storage::read_value(crate::executor::storage::KEY_CLEAR_SIGNER_RELAY)
        .ok()
        .flatten()
        .as_ref()
        .and_then(Value::as_str)
        .and_then(|text| clear_signer::relay_url(text).ok())
        .unwrap_or_else(|| clear_signer::DEFAULT_RELAY_URL.to_owned())
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

    /// The URL the screen hands the browser.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Take every connection waiting, up to the cap. A connection arriving
    /// when nothing is being asked for is not this visit's and is dropped.
    fn accept(&mut self) {
        while let Ok((stream, _)) = self.listener.accept() {
            let Some((id, request)) = self.pending.clone() else {
                let _ = stream.shutdown(Shutdown::Both);
                continue;
            };
            if self.wires.len() >= CANDIDATES || stream.set_nonblocking(true).is_err() {
                let _ = stream.shutdown(Shutdown::Both);
                continue;
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
                            settled = Some((index, Err(error)));
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
                            settled = Some((index, outcome));
                        }
                        if step.close {
                            break;
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => break,
                    Err(_) => {
                        if let Some(error) = wire.conn.closed() {
                            settled = Some((index, Err(error)));
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
                if !matches!(outcome, Err(ClearSignerError::Declined)) || !kept.closed() {
                    self.wires.push(kept);
                }
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

impl Wire {
    /// The socket is gone, so this wire cannot carry the session's next
    /// request.
    fn closed(&self) -> bool {
        self.stream.peer_addr().is_err()
    }
}

/// Write every byte, or say the socket is gone.
fn write_all(stream: &mut TcpStream, bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return true;
    }
    // The socket is non-blocking for reads; a short write on a loopback
    // socket carrying a few kilobytes is rare but not impossible.
    let deadline = Instant::now() + Duration::from_secs(5);
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

static FLOW: Mutex<Option<Flow>> = Mutex::new(None);

fn flow() -> std::sync::MutexGuard<'static, Option<Flow>> {
    FLOW.lock().unwrap_or_else(PoisonError::into_inner)
}

/// End the flow's page visit, if one is open. Called when a flow finishes, when
/// anything refuses, and when the screen goes away.
pub fn end_flow() {
    if let Some(mut open) = flow().take() {
        open.line.end();
    }
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
    let mut line = match open_line(page, channel, deadline) {
        Ok(line) => line,
        Err(refusal) => return Err(gave_up(channel, refusal)),
    };
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

/// Which page this operation's key lives behind: the one the ceremony that
/// found it recorded, or the person's own page from Settings.
fn signer_page_for(operation: &ShellOperation) -> String {
    let named = match operation {
        ShellOperation::SignProof { signer_origin, .. }
        | ShellOperation::SignMemberProof { signer_origin, .. } => signer_origin.clone(),
        _ => None,
    };
    named
        .filter(|origin| !origin.is_empty())
        .unwrap_or_else(signer_url)
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
    // The flow's visit, when it is a visit to this same page; otherwise a new
    // one, and the old one is told goodbye rather than left open.
    let mut open = flow().take();
    if open.as_ref().is_some_and(|flow| flow.page != page) {
        if let Some(mut stale) = open.take() {
            stale.line.end();
        }
    }
    let mut visit = match open {
        Some(visit) => visit,
        None => match open_line(page, channel, deadline) {
            Ok(line) => Flow {
                page: page.to_owned(),
                line,
            },
            Err(refusal) => return Err(gave_up(channel, refusal)),
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
                *flow() = Some(visit);
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
/// The page is the person's from Settings: the core's `RegistryPublish`
/// carries no `signer_origin` per member, so there is nothing else to read.
pub fn member_proof(
    credential_id: &str,
    public_key_hex: &str,
    attestation_hex: &str,
    group_public_key_hex: &str,
    expected_challenge: &[u8],
    registry: &str,
    channel: &Channel,
) -> Result<Assertion, PasskeyFailure> {
    let ceremony = core_ceremony::Ceremony::SignMemberProof {
        credential_id: credential_id.to_owned(),
        public_key_hex: public_key_hex.to_owned(),
        attestation_hex: attestation_hex.to_owned(),
        group_public_key_hex: group_public_key_hex.to_owned(),
    };
    let page = signer_url();
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

    /// Answer the "where is it?" question the way the page's own device would.
    pub(crate) fn here(channel: &Arc<Channel>) -> std::thread::JoinHandle<()> {
        let channel = Arc::clone(channel);
        std::thread::spawn(move || {
            for _ in 0..600 {
                if channel.asking_place() {
                    channel.answer_place(Some(Place::ThisDevice));
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        })
    }

    /// Play the page for ONE request, end to end: say the Clear Signer is on
    /// this device, take the launch URL, connect as the page and answer with
    /// whatever `reply` makes of the intent. The wallet's `bye` is read back,
    /// so the socket is not torn down under it.
    pub(crate) fn answers_once(
        channel: &Arc<Channel>,
        reply: impl FnOnce(&Value) -> Value + Send + 'static,
    ) -> std::thread::JoinHandle<()> {
        let channel = Arc::clone(channel);
        std::thread::spawn(move || {
            for _ in 0..1200 {
                if channel.asking_place() {
                    channel.answer_place(Some(Place::ThisDevice));
                    break;
                }
                std::thread::sleep(Duration::from_millis(5));
            }
            let url = page_of_channel(&channel);
            let (port, token) = launch_of(&url);
            let mut page = FakePage::connect(port, &token, "https://sign.getvela.app");
            let Some(intent) = page.next() else {
                return;
            };
            page.say(&reply(&intent));
            page.next();
        })
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

    /// The URL a visit hands the screen carries this listener's own port and a
    /// token of sixteen random bytes, and the listener is loopback only.
    #[test]
    fn the_launch_url_is_this_listeners_port_on_loopback() {
        let loopback = Loopback::open(PAGE).unwrap_or_else(|e| unreachable!("{e}"));
        let local = loopback
            .listener
            .local_addr()
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(local.ip().is_loopback());
        let (port, token) = launch_of(loopback.url());
        assert_eq!(port, local.port());
        assert_eq!(token.len(), 22, "16 random bytes, base64url");
    }

    /// A page from anywhere but the signer's own origin is refused at the
    /// handshake, and the visit goes on waiting for the real one.
    #[test]
    fn a_page_from_another_origin_never_gets_a_socket() {
        let (channel, _changed) = Channel::new();
        let answered = here(&channel);
        let signing = signing_key(7);
        let request = json!({ "intent": { "method": "personal_sign" }, "context": {} });
        let ceremony = {
            let (channel, request) = (Arc::clone(&channel), request.clone());
            std::thread::spawn(move || sign(&request, PAGE, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        let (port, token) = launch_of(&url);
        let refused = FakePage::refused(port, "https://evil.example");
        assert!(refused.starts_with("HTTP/1.1 403"), "{refused}");

        // The real page, on the same listener, is still taken.
        let mut page = FakePage::connect(port, &token, "https://sign.getvela.app");
        let intent = page.next().unwrap_or_else(|| unreachable!("no intent"));
        assert_eq!(intent["t"], "intent");
        page.say(&json!({
            "v": 1, "t": "result", "n": 1, "id": intent["id"],
            "result": page_result(&signing, &CREDENTIAL, &DIGEST),
        }));
        let assertion = ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the attempt panicked"))
            .unwrap_or_else(|failure| unreachable!("{failure:?}"));
        assert_eq!(assertion.credential_id, "112233");
        let _ = answered.join();
    }

    /// A `hello` carrying another visit's token proves nothing: that socket is
    /// closed and the wait goes on for the real page.
    #[test]
    fn a_spent_token_is_closed_and_the_wait_goes_on() {
        let (channel, _changed) = Channel::new();
        let answered = here(&channel);
        let signing = signing_key(7);
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        let (port, token) = launch_of(&url);
        let mut stray = FakePage::connect(port, "not-the-token", "https://sign.getvela.app");
        assert_eq!(
            stray.next(),
            None,
            "a spent token gets no intent, just a close"
        );
        assert!(channel.waiting(), "the visit is still waiting");

        let mut page = FakePage::connect(port, &token, "https://sign.getvela.app");
        let intent = page.next().unwrap_or_else(|| unreachable!("no intent"));
        page.say(&json!({
            "v": 1, "t": "result", "n": 1, "id": intent["id"],
            "result": page_result(&signing, &CREDENTIAL, &DIGEST),
        }));
        assert!(
            ceremony
                .join()
                .unwrap_or_else(|_| unreachable!("panicked"))
                .is_ok()
        );
        let _ = answered.join();
    }

    /// The whole signature, over the socket: the screen is handed the page
    /// (again on "Open the page again", same port and token), the page's
    /// answer comes back as the assertion the envelope takes, carrying the
    /// page it came from — and the session is said goodbye to.
    #[test]
    fn a_signature_hands_over_its_page_and_returns_the_assertion() {
        let (channel, _changed) = Channel::new();
        let answered = here(&channel);
        let signing = signing_key(7);
        let request =
            json!({ "intent": { "method": "personal_sign" }, "context": { "chainId": 100 } });
        let ceremony = {
            let (channel, request) = (Arc::clone(&channel), request.clone());
            std::thread::spawn(move || sign(&request, PAGE, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        assert!(url.starts_with("https://sign.getvela.app/sign.html?ch=ws#p="));
        assert!(channel.waiting());
        assert_eq!(channel.take_page(), None, "handed over once");
        channel.reopen();
        assert_eq!(channel.take_page().as_deref(), Some(url.as_str()));

        let (port, token) = launch_of(&url);
        let mut page = FakePage::connect(port, &token, "https://sign.getvela.app");
        let intent = page.next().unwrap_or_else(|| unreachable!("no intent"));
        assert_eq!(intent["intent"]["method"], "personal_sign");
        assert_eq!(intent["context"]["chainId"], 100);
        page.say(&json!({
            "v": 1, "t": "result", "n": 1, "id": intent["id"],
            "result": page_result(&signing, &CREDENTIAL, &DIGEST),
        }));
        let assertion = ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the attempt panicked"))
            .unwrap_or_else(|failure| unreachable!("{failure:?}"));
        assert_eq!(assertion.credential_id, "112233");
        assert!(assertion.signature_der_hex.starts_with("30"), "DER");
        assert_eq!(
            assertion.signer_origin.as_deref(),
            Some("https://sign.getvela.app"),
            "the key lives behind the page that signed for it"
        );
        // `bye`, then the close frame: the page leaves its waiting card.
        assert_eq!(
            page.next().as_ref().and_then(|m| m["t"].as_str()),
            Some("bye")
        );
        assert_eq!(page.next(), None);
        assert!(!channel.waiting());
        assert_eq!(channel.ended(), None);
        let _ = answered.join();
    }

    /// A signature over another digest proves the token and ends the attempt
    /// as a mismatch, with nothing to submit.
    #[test]
    fn a_signature_over_something_else_is_a_mismatch() {
        let (channel, _changed) = Channel::new();
        let answered = here(&channel);
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        let (port, token) = launch_of(&url);
        let mut page = FakePage::connect(port, &token, "https://sign.getvela.app");
        let intent = page.next().unwrap_or_else(|| unreachable!("no intent"));
        page.say(&json!({
            "v": 1, "t": "result", "n": 1, "id": intent["id"],
            "result": page_result(&signing_key(7), &CREDENTIAL, &[0xcd; 32]),
        }));
        assert!(
            ceremony
                .join()
                .unwrap_or_else(|_| unreachable!("panicked"))
                .is_err()
        );
        assert_eq!(channel.ended(), Some(Refusal::Mismatch));
        let _ = answered.join();
    }

    /// The page closed its socket without answering: a decline, not an error,
    /// so the request stays open with "closed" to say.
    #[test]
    fn a_page_that_goes_away_unanswered_is_a_decline() {
        let (channel, _changed) = Channel::new();
        let answered = here(&channel);
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        let (port, token) = launch_of(&url);
        let mut page = FakePage::connect(port, &token, "https://sign.getvela.app");
        page.next();
        page.hang_up();
        let failure = ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("panicked"))
            .err()
            .unwrap_or_else(|| unreachable!("a closed page signed"));
        assert_eq!(failure.kind, FailureKind::Cancelled);
        assert_eq!(channel.ended(), Some(Refusal::Closed));
        let _ = answered.join();
    }

    /// The page's own rules refused it — its own sentence, not the person's.
    #[test]
    fn the_pages_refusal_is_its_own() {
        let (channel, _changed) = Channel::new();
        let answered = here(&channel);
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        let url = page_of_channel(&channel);
        let (port, token) = launch_of(&url);
        let mut page = FakePage::connect(port, &token, "https://sign.getvela.app");
        let intent = page.next().unwrap_or_else(|| unreachable!("no intent"));
        page.say(&json!({
            "v": 1, "t": "error", "n": 1, "id": intent["id"], "code": "not_bound",
        }));
        assert!(
            ceremony
                .join()
                .unwrap_or_else(|_| unreachable!("panicked"))
                .is_err()
        );
        assert_eq!(channel.ended(), Some(Refusal::Refused));
        let _ = answered.join();
    }

    /// "Cancel" ends the attempt as the person declining: a cancelled passkey
    /// to the core, "closed" to the sheet — and a screen that is gone starts
    /// no attempt at all.
    #[test]
    fn a_cancelled_wait_is_a_cancelled_passkey_and_a_sentence() {
        let (channel, _changed) = Channel::new();
        let answered = here(&channel);
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
        let _ = answered.join();

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

    /// Dismissing "where is your Clear Signer?" stops the attempt before a
    /// port is bound or a room is taken.
    #[test]
    fn dismissing_the_where_question_stops_the_attempt() {
        let (channel, _changed) = Channel::new();
        let ceremony = {
            let channel = Arc::clone(&channel);
            std::thread::spawn(move || sign(&json!({}), PAGE, &DIGEST, &keys(), &channel))
        };
        for _ in 0..600 {
            if channel.asking_place() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert!(channel.asking_place(), "the person was never asked");
        channel.answer_place(None);
        assert!(
            ceremony
                .join()
                .unwrap_or_else(|_| unreachable!("panicked"))
                .is_err()
        );
        assert!(!channel.asking_place());
        assert_eq!(channel.ended(), Some(Refusal::Closed));
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
    #[test]
    fn a_proof_goes_to_the_page_its_key_lives_behind() {
        use vela_core::app::KeyMethod;
        use vela_core::app::shell::ProofPurpose;
        let behind = ShellOperation::SignProof {
            credential_id: "aabb".to_owned(),
            transports: String::new(),
            method: KeyMethod::ClearSigner,
            purpose: ProofPurpose::Verify,
            signer_origin: Some("https://sign.example.test".to_owned()),
        };
        assert_eq!(signer_page_for(&behind), "https://sign.example.test");
        let anywhere = ShellOperation::AuthenticatePasskey {
            method: KeyMethod::ClearSigner,
        };
        assert_eq!(signer_page_for(&anywhere), signer_url());
    }

    /// Which ceremony closes its flow: a create ends at the member proof, a
    /// recovery at its second signature. A sign-in and a first recovery leave
    /// the page open for what comes next.
    #[test]
    fn only_the_last_ceremony_of_a_flow_says_goodbye() {
        use vela_core::app::KeyMethod;
        use vela_core::app::shell::ProofPurpose;
        let proof = |purpose| ShellOperation::SignProof {
            credential_id: "aabb".to_owned(),
            transports: String::new(),
            method: KeyMethod::ClearSigner,
            purpose,
            signer_origin: None,
        };
        assert!(ends_the_flow(&ShellOperation::SignMemberProof {
            credential_id: "aabb".to_owned(),
            public_key_hex: "04".to_owned(),
            attestation_hex: String::new(),
            transports: String::new(),
            method: KeyMethod::ClearSigner,
            group_public_key_hex: "04".to_owned(),
            signer_origin: None,
        }));
        assert!(ends_the_flow(&proof(ProofPurpose::RecoverSecond)));
        assert!(ends_the_flow(&proof(ProofPurpose::Verify)));
        assert!(!ends_the_flow(&proof(ProofPurpose::RecoverFirst)));
        assert!(!ends_the_flow(&ShellOperation::AuthenticatePasskey {
            method: KeyMethod::ClearSigner,
        }));
    }
}
