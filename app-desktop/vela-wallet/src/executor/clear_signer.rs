//! The Clear Signer on the desktop (spec 071): the request in a URL
//! fragment, the answer on a loopback callback.
//!
//! The fourth "Sign with" is a page, not a key. `app-web/clearsigning`,
//! opened in the person's default browser, decodes the request from the
//! operation's own bytes, derives the digest itself, runs the passkey
//! ceremony and sends the assertion back. This file is the desktop's end of
//! that conversation and nothing more (contract §3):
//!
//! - the request is the core's (`clear_signer::request`), deflated into the
//!   URL's fragment beside a one-time token (`url_launch`) — a fragment never
//!   reaches a server, and the page wipes it from history at once;
//! - the page answers by navigating to `http://127.0.0.1:<port>/vela?t=…&result=…`
//!   or, when its tab closes unanswered, with a beacon `POST …&error=…`;
//! - which request is the callback (`callback_query`), whose token it
//!   carries (`parse_callback`) and whether the assertion is a user-verified
//!   signature over the digest THIS wallet computed by one of ITS keys
//!   (`verify`) are the core's. What is left here is a socket, a clock and
//!   the sentence the browser tab is left showing.
//!
//! ## Who may talk to the listener (research R4)
//!
//! Bound to 127.0.0.1 on a port the OS picks — never another interface — and
//! one listener per ceremony. Anything that is not the callback gets a 404,
//! and so does a callback carrying another token: the ceremony goes on
//! waiting, so a stray program or an old tab cannot end it. Every connection
//! is read on its own thread with a size cap and a deadline, so one that
//! never finishes its request head cannot hold the real answer up behind it.
//!
//! ## What the screen sees
//!
//! [`Channel`] is what a signing screen and its ceremony say to each other:
//! whether this request goes to the Clear Signer and to which page, that the
//! page is waiting, the URL to hand the browser (the first time, and again on
//! "Open the page again"), "Cancel", and how the last ceremony ended without
//! a signature. The screen words that ending; the core hears it as a
//! cancelled passkey, so the request stays open to be signed another way
//! (contract §5).

use std::io::{self, Read as _, Write as _};
use std::net::{Ipv4Addr, Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::time::{Duration, Instant};

use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use serde_json::Value;

use vela_core::app::network_admin::BUILTIN_CHAINS;
use vela_core::app::{Assertion, FailureKind};
use vela_core::clear_signer::{
    self, CALLBACK_PATH, ClearSignerError, RequestInput, Verified, callback_query, parse_callback,
    verify,
};
use vela_core::primitives::{to_base64url, to_hex};
use vela_core::user_op::{MultiSendCall, UserOperation, WalletKey};

use crate::executor::passkey::{self, PasskeyFailure};
use crate::loc::Loc;

/// How long the page has to answer. The core has no clock; this is the
/// shell's (research R4).
pub const TIMEOUT: Duration = Duration::from_secs(5 * 60);
/// A request head longer than this is not the page's callback. The real one
/// carries an assertion of a few kilobytes in its query.
const HEAD_CAP: usize = 64 * 1024;
/// How long one connection may take to send its request head.
const HEAD_DEADLINE: Duration = Duration::from_secs(5);
/// Connections read at once. Past this a connection is noise, dropped unread.
const READERS: usize = 8;
/// How often the wait looks at the clock and the screen's buttons.
const POLL: Duration = Duration::from_millis(50);

// ---------------------------------------------------------------------------
// What the person is told
// ---------------------------------------------------------------------------

/// Why a ceremony ended without a signature, as the sheet words it (contract
/// §5). Nothing was signed and nothing was submitted, whichever it is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// The person closed the page, declined on it, or stopped waiting.
    Closed,
    /// The page's own rules would not sign it — not the person.
    Refused,
    /// An answer came back that is not this wallet's signature over this
    /// request.
    Mismatch,
    /// Nothing came back in time.
    TimedOut,
}

impl Refusal {
    /// The core's verdict, in the four sentences a person can act on.
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

/// The sentences the browser tab can be left showing, in the person's
/// language. The listener speaks no other words.
#[derive(Clone, Debug)]
pub struct TabWords {
    pub signed: String,
    pub closed: String,
    pub refused: String,
    pub mismatch: String,
}

impl TabWords {
    #[must_use]
    pub fn resolve(loc: &Loc) -> Self {
        let t = |key: &str| loc.t(key).to_string();
        Self {
            signed: t("componentsUi.signing.clearSignerDoneTab"),
            closed: t(Refusal::Closed.key()),
            refused: t(Refusal::Refused.key()),
            mismatch: t(Refusal::Mismatch.key()),
        }
    }

    fn for_outcome(&self, outcome: &Result<Verified, ClearSignerError>) -> &str {
        match outcome.as_ref().map_err(Refusal::of) {
            Ok(_) => &self.signed,
            Err(Refusal::Closed) => &self.closed,
            Err(Refusal::Refused) => &self.refused,
            Err(Refusal::Mismatch | Refusal::TimedOut) => &self.mismatch,
        }
    }
}

// ---------------------------------------------------------------------------
// The screen and the ceremony
// ---------------------------------------------------------------------------

#[derive(Default)]
struct State {
    /// The page this request goes to, when the person chose the Clear Signer
    /// for it.
    chosen: Option<String>,
    /// The URL a ceremony is waiting on — `Some` exactly while one waits.
    waiting: Option<String>,
    /// That URL is to be handed to the browser (again).
    open: bool,
    /// The person stopped this wait.
    cancelled: bool,
    /// The screen is gone: no wait continues, none starts.
    closed: bool,
    /// How the last ceremony ended unsigned, until the person has read it.
    ended: Option<Refusal>,
}

/// What one signing screen and its Clear Signer ceremonies say to each other.
///
/// Shared, because the screen's context is cloned into the executor when a
/// request opens and the choice, the "open it again" and the "cancel" all
/// happen afterwards. Every change is announced on the stream [`Self::new`]
/// hands back, which is what opens the page and redraws the screen — no
/// polling.
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
    /// ceremony's sentence — it answered a question nobody is asking now.
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

    /// A ceremony is waiting for the page's answer.
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
    /// listener still takes its answer.
    pub fn reopen(&self) {
        self.with(|state| state.open = state.waiting.is_some());
        self.announce();
    }

    /// "Cancel" on the waiting sheet: the person declined (contract §2).
    pub fn cancel(&self) {
        self.with(|state| state.cancelled = true);
        self.announce();
    }

    /// How the last ceremony ended without a signature, until [`Self::forget`].
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
    /// starts — a ceremony nobody can see must not hold a port for minutes.
    pub fn close(&self) {
        self.with(|state| state.closed = true);
        self.announce();
    }

    // -- the ceremony's half --------------------------------------------------

    /// A ceremony starts waiting on `url`. `false` when the screen is gone.
    fn begin(&self, url: String) -> bool {
        let began = self.with(|state| {
            if state.closed {
                return false;
            }
            state.waiting = Some(url);
            state.open = true;
            state.cancelled = false;
            state.ended = None;
            true
        });
        self.announce();
        began
    }

    fn stopped(&self) -> bool {
        self.with(|state| state.cancelled || state.closed)
    }

    fn end(&self, ended: Option<Refusal>) {
        self.with(|state| {
            state.waiting = None;
            state.open = false;
            state.ended = ended;
        });
        self.announce();
    }
}

// ---------------------------------------------------------------------------
// The request, and one ceremony
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

/// One ceremony, start to finish: listen, hand the page to the screen, wait
/// for the callback, verify it against `digest` and `keys`.
///
/// A refusal of any kind is told to the screen ([`Channel::ended`]) and
/// answered as a cancelled passkey, which is what keeps the request open. A
/// listener the OS would not give is the one real failure.
pub fn sign(
    request: &Value,
    page: &str,
    digest: &[u8],
    keys: &[WalletKey],
    channel: &Channel,
) -> Result<Assertion, PasskeyFailure> {
    let loopback = Loopback::bind().map_err(|error| PasskeyFailure {
        kind: FailureKind::Other,
        message: Some(format!(
            "The Clear Signer could not listen on this computer: {error}"
        )),
    })?;
    let token = to_base64url(&passkey::random(16));
    let url = clear_signer::url_launch(page, request, &loopback.callback(), &token);
    let cancelled = || PasskeyFailure {
        kind: FailureKind::Cancelled,
        message: None,
    };
    if !channel.begin(url) {
        return Err(cancelled());
    }
    let expect = Expect {
        token,
        digest: digest.to_vec(),
        keys: keys.to_vec(),
        words: TabWords::resolve(&Loc::from_env()),
    };
    let outcome = loopback.serve(&expect, Instant::now() + TIMEOUT, || channel.stopped());
    channel.end(outcome.as_ref().err().copied());
    match outcome {
        Ok(verified) => Ok(assertion_of(verified)),
        Err(refusal) => {
            eprintln!("[vela-wallet] clear signer: ended unsigned ({refusal:?})");
            Err(cancelled())
        }
    }
}

/// A verified answer in the shape the passkey's takes, so the envelope that
/// follows cannot tell them apart.
fn assertion_of(verified: Verified) -> Assertion {
    Assertion {
        credential_id: verified.credential_id_hex,
        signature_der_hex: to_hex(&verified.signature_der, false),
        authenticator_data_hex: to_hex(&verified.authenticator_data, false),
        client_data_json_hex: to_hex(&verified.client_data_json, false),
        user_id_hex: None,
        // The page's browser knows; the page does not say.
        authenticator_attachment: String::new(),
    }
}

// ---------------------------------------------------------------------------
// The listener
// ---------------------------------------------------------------------------

/// What makes an answer this request's: its token, the digest the wallet
/// computed, the wallet's keys — and the words the tab is left with.
#[derive(Clone, Debug)]
pub struct Expect {
    pub token: String,
    pub digest: Vec<u8>,
    pub keys: Vec<WalletKey>,
    pub words: TabWords,
}

/// The loopback listener one ceremony answers on.
pub struct Loopback {
    listener: TcpListener,
    port: u16,
}

impl Loopback {
    /// A port the OS picks, on this machine's loopback and nowhere else.
    pub fn bind() -> io::Result<Self> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        // Accepting never blocks: between connections the wait looks at the
        // clock and at the screen's buttons.
        listener.set_nonblocking(true)?;
        let port = listener.local_addr()?.port();
        Ok(Self { listener, port })
    }

    /// `url_launch`'s callback: this listener's `/vela`.
    #[must_use]
    pub fn callback(&self) -> String {
        format!("http://127.0.0.1:{}{CALLBACK_PATH}", self.port)
    }

    /// Take connections until one carries this request's answer, `stopped`
    /// says the person gave up, or `deadline` passes.
    ///
    /// Any connection that proves itself with the token decides — a
    /// signature, a refusal, or an answer that fails verification. Nothing
    /// else does, so the page opened again (same port, same token) is
    /// answered on the same listener.
    pub fn serve(
        &self,
        expect: &Expect,
        deadline: Instant,
        stopped: impl Fn() -> bool,
    ) -> Result<Verified, Refusal> {
        let expect = Arc::new(expect.clone());
        let (decided, decisions) = mpsc::channel();
        let reading = Arc::new(AtomicUsize::new(0));
        loop {
            if let Ok(outcome) = decisions.try_recv() {
                return decide(outcome);
            }
            if stopped() {
                return Err(Refusal::Closed);
            }
            if Instant::now() >= deadline {
                return Err(Refusal::TimedOut);
            }
            match self.listener.accept() {
                Ok((stream, _)) => {
                    if reading.fetch_add(1, Ordering::SeqCst) >= READERS {
                        reading.fetch_sub(1, Ordering::SeqCst);
                        continue;
                    }
                    let (expect, decided, done) =
                        (Arc::clone(&expect), decided.clone(), Arc::clone(&reading));
                    let spawned = std::thread::Builder::new()
                        .name("clear-signer-callback".to_owned())
                        .spawn(move || {
                            if let Some(outcome) = answer(stream, &expect) {
                                let _ = decided.send(outcome);
                            }
                            done.fetch_sub(1, Ordering::SeqCst);
                        });
                    if spawned.is_err() {
                        reading.fetch_sub(1, Ordering::SeqCst);
                    }
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    if let Ok(outcome) = decisions.recv_timeout(POLL) {
                        return decide(outcome);
                    }
                }
                Err(error) => {
                    eprintln!("[vela-wallet] clear signer: accept failed: {error}");
                    std::thread::sleep(POLL);
                }
            }
        }
    }
}

fn decide(outcome: Result<Verified, ClearSignerError>) -> Result<Verified, Refusal> {
    outcome.map_err(|error| {
        if !matches!(error, ClearSignerError::Declined) {
            eprintln!("[vela-wallet] clear signer: answer not accepted: {error}");
        }
        Refusal::of(&error)
    })
}

/// Read one connection's request head and answer it: `Some` when it was this
/// request's callback — whatever it said — and `None` for anything else,
/// which gets a 404 and changes nothing.
fn answer(mut stream: TcpStream, expect: &Expect) -> Option<Result<Verified, ClearSignerError>> {
    let head = read_head(&mut stream)?;
    let outcome = callback_query(&head).map(|query| {
        parse_callback(query, &expect.token)
            .and_then(|result| verify(&result, &expect.digest, &expect.keys))
    });
    match outcome {
        // Not the callback (`/favicon.ico`), or another request's answer.
        None | Some(Err(ClearSignerError::WrongToken)) => {
            respond(&mut stream, None);
            None
        }
        Some(outcome) => {
            respond(&mut stream, Some(expect.words.for_outcome(&outcome)));
            Some(outcome)
        }
    }
}

/// The request head, up to its blank line — or `None` for a connection that
/// sends too much, too slowly, or nothing at all.
fn read_head(stream: &mut TcpStream) -> Option<String> {
    // Accepted from a non-blocking listener, the socket may have inherited it.
    stream.set_nonblocking(false).ok()?;
    let deadline = Instant::now() + HEAD_DEADLINE;
    let mut head = Vec::with_capacity(4096);
    let mut chunk = [0u8; 4096];
    loop {
        let left = deadline
            .checked_duration_since(Instant::now())
            .filter(|left| !left.is_zero())?;
        stream.set_read_timeout(Some(left)).ok()?;
        let read = stream.read(&mut chunk).ok()?;
        if read == 0 {
            return None;
        }
        head.extend_from_slice(&chunk[..read]);
        match head.windows(4).position(|window| window == b"\r\n\r\n") {
            Some(end) if end <= HEAD_CAP => {
                head.truncate(end);
                return Some(String::from_utf8_lossy(&head).into_owned());
            }
            Some(_) => return None,
            None if head.len() > HEAD_CAP => return None,
            None => {}
        }
    }
}

/// `200` with the tab's sentence, or a bare `404`. Either way the connection
/// ends here.
fn respond(stream: &mut TcpStream, sentence: Option<&str>) {
    let response = match sentence {
        Some(sentence) => {
            let body = tab_page(sentence);
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\n\
                 Content-Length: {}\r\nCache-Control: no-store\r\n\
                 Content-Security-Policy: default-src 'none'; style-src 'unsafe-inline'\r\n\
                 X-Content-Type-Options: nosniff\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        }
        None => {
            "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_owned()
        }
    };
    let _ = stream.set_write_timeout(Some(HEAD_DEADLINE));
    let _ = stream.write_all(response.as_bytes());
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Write);
}

/// The whole page the browser tab is left showing: one sentence.
fn tab_page(sentence: &str) -> String {
    let mut escaped = String::with_capacity(sentence.len());
    for c in sentence.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            other => escaped.push(other),
        }
    }
    format!(
        "<!doctype html><html><head><meta charset=\"utf-8\">\
         <meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
         <meta name=\"color-scheme\" content=\"light dark\"><title>Vela</title></head>\
         <body style=\"margin:0;padding:40px;font:16px/1.5 system-ui,sans-serif\">\
         <p>{escaped}</p></body></html>"
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    use std::sync::atomic::AtomicBool;

    use p256::ecdsa::signature::hazmat::PrehashSigner as _;
    use p256::ecdsa::{Signature, SigningKey};
    use serde_json::json;
    use vela_core::primitives::{from_base64url, sha256};
    use vela_core::webauthn::webauthn_signing_hash;

    const DIGEST: [u8; 32] = [0xab; 32];
    const CREDENTIAL: [u8; 3] = [0x11, 0x22, 0x33];

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

    /// What the page returns for `digest` — a user-verified `webauthn.get`
    /// signed by `signing` as `credential`, in the page's own field shapes.
    pub(crate) fn page_result(signing: &SigningKey, credential: &[u8], digest: &[u8]) -> Value {
        let mut authenticator_data = sha256(b"getvela.app");
        authenticator_data.push(0x05);
        authenticator_data.extend_from_slice(&[0, 0, 0, 7]);
        let client = format!(
            r#"{{"type":"webauthn.get","challenge":"{}","origin":"https://sign.getvela.app","crossOrigin":false}}"#,
            to_base64url(digest)
        );
        let prehash = webauthn_signing_hash(&authenticator_data, client.as_bytes());
        let signature: Signature = signing
            .sign_prehash(&prehash)
            .unwrap_or_else(|e| unreachable!("{e}"));
        json!({
            "credentialId": to_base64url(credential),
            "signature": to_hex(&signature.to_bytes(), true),
            "authenticatorData": to_hex(&authenticator_data, true),
            "clientDataJSON": to_hex(client.as_bytes(), true),
            "userVerified": true,
        })
    }

    fn words() -> TabWords {
        TabWords {
            signed: "signed <ok>".to_owned(),
            closed: "closed".to_owned(),
            refused: "refused".to_owned(),
            mismatch: "mismatch".to_owned(),
        }
    }

    fn expect(token: &str) -> Expect {
        Expect {
            token: token.to_owned(),
            digest: DIGEST.to_vec(),
            keys: vec![
                wallet_key(&signing_key(9), &[0x99]),
                wallet_key(&signing_key(7), &CREDENTIAL),
            ],
            words: words(),
        }
    }

    /// A listener serving one ceremony on its own thread, and the lever
    /// that stops it.
    struct Serving {
        port: u16,
        stop: Arc<AtomicBool>,
        outcome: std::thread::JoinHandle<Result<Verified, Refusal>>,
    }

    fn serve(expect: Expect) -> Serving {
        let loopback = Loopback::bind().unwrap_or_else(|e| unreachable!("{e}"));
        let port = loopback.port;
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = Arc::clone(&stop);
        let outcome = std::thread::spawn(move || {
            loopback.serve(&expect, Instant::now() + Duration::from_secs(20), || {
                stopped.load(Ordering::SeqCst)
            })
        });
        Serving {
            port,
            stop,
            outcome,
        }
    }

    impl Serving {
        fn outcome(self) -> Result<Verified, Refusal> {
            self.outcome
                .join()
                .unwrap_or_else(|_| unreachable!("the listener panicked"))
        }
    }

    /// One request, as a browser sends it, and the whole response.
    pub(crate) fn http(port: u16, method: &str, target: &str) -> String {
        let mut stream =
            TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap_or_else(|e| unreachable!("{e}"));
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let head = format!(
            "{method} {target} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\n\
             User-Agent: Mozilla/5.0\r\nContent-Length: 0\r\n\r\n"
        );
        // A listener that stops reading a head it will not take may reset
        // the connection under the write; the response says what happened.
        let _ = stream.write_all(head.as_bytes());
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
    }

    pub(crate) fn result_query(token: &str, result: &Value) -> String {
        format!(
            "{CALLBACK_PATH}?t={token}&result={}",
            to_base64url(result.to_string().as_bytes())
        )
    }

    /// The page's navigation back with a signature over THIS digest, by one
    /// of THIS wallet's keys: accepted, and the tab is told it can close.
    #[test]
    fn the_pages_signed_answer_is_accepted_and_the_tab_is_told() {
        let serving = serve(expect("tok"));
        let result = page_result(&signing_key(7), &CREDENTIAL, &DIGEST);
        let response = http(serving.port, "GET", &result_query("tok", &result));
        assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
        assert!(response.contains("text/html; charset=utf-8"));
        // The sentence, escaped: it is text on a page, never markup.
        assert!(response.contains("<p>signed &lt;ok&gt;</p>"), "{response}");
        let verified = serving.outcome().unwrap_or_else(|e| unreachable!("{e:?}"));
        assert_eq!(verified.credential_id_hex, "112233");
        assert_eq!(
            verified.signature_der[0], 0x30,
            "DER, as the envelope takes"
        );
    }

    /// An answer carrying another request's token is not this one's: a 404,
    /// and the ceremony goes on waiting for the real one.
    #[test]
    fn another_token_is_turned_away_and_the_wait_goes_on() {
        let serving = serve(expect("tok"));
        let result = page_result(&signing_key(7), &CREDENTIAL, &DIGEST);
        let stray = http(serving.port, "GET", &result_query("old", &result));
        assert!(stray.starts_with("HTTP/1.1 404"), "{stray}");
        assert!(!serving.outcome.is_finished());
        let real = http(serving.port, "GET", &result_query("tok", &result));
        assert!(real.starts_with("HTTP/1.1 200"), "{real}");
        assert!(serving.outcome().is_ok());
    }

    /// The tab closed unanswered: its beacon is a decline, not an error.
    #[test]
    fn the_closing_tabs_beacon_is_a_decline() {
        let serving = serve(expect("tok"));
        let response = http(
            serving.port,
            "POST",
            &format!("{CALLBACK_PATH}?t=tok&error=user_rejected"),
        );
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(response.contains("<p>closed</p>"));
        assert_eq!(serving.outcome(), Err(Refusal::Closed));
    }

    /// The page's own rules refused it — its own sentence, not the person's.
    #[test]
    fn the_pages_refusal_is_its_own() {
        let serving = serve(expect("tok"));
        let response = http(
            serving.port,
            "GET",
            &format!("{CALLBACK_PATH}?t=tok&error=refused"),
        );
        assert!(response.contains("<p>refused</p>"));
        assert_eq!(serving.outcome(), Err(Refusal::Refused));
    }

    /// What a browser asks a listener for besides the callback is a 404 and
    /// changes nothing.
    #[test]
    fn the_favicon_is_a_404_and_the_wait_goes_on() {
        let serving = serve(expect("tok"));
        let favicon = http(serving.port, "GET", "/favicon.ico");
        assert!(favicon.starts_with("HTTP/1.1 404"), "{favicon}");
        let bare = http(serving.port, "GET", "/");
        assert!(bare.starts_with("HTTP/1.1 404"), "{bare}");
        assert!(!serving.outcome.is_finished());
        let result = page_result(&signing_key(7), &CREDENTIAL, &DIGEST);
        http(serving.port, "GET", &result_query("tok", &result));
        assert!(serving.outcome().is_ok());
    }

    /// A signature over another digest, or by a key this wallet does not
    /// hold, proves the token and ends the ceremony — as a mismatch, with
    /// nothing to submit.
    #[test]
    fn a_signature_over_something_else_is_a_mismatch() {
        let serving = serve(expect("tok"));
        let other = page_result(&signing_key(7), &CREDENTIAL, &[0xcd; 32]);
        let response = http(serving.port, "GET", &result_query("tok", &other));
        assert!(response.contains("<p>mismatch</p>"), "{response}");
        assert_eq!(serving.outcome(), Err(Refusal::Mismatch));

        let serving = serve(expect("tok"));
        let foreign = page_result(&signing_key(8), &[0x44], &DIGEST);
        http(serving.port, "GET", &result_query("tok", &foreign));
        assert_eq!(serving.outcome(), Err(Refusal::Mismatch));
    }

    /// A local connection that never finishes its request head cannot hold
    /// the real answer up behind it.
    #[test]
    fn a_silent_connection_does_not_wedge_the_ceremony() {
        let serving = serve(expect("tok"));
        let _silent = TcpStream::connect((Ipv4Addr::LOCALHOST, serving.port))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let mut trickle = TcpStream::connect((Ipv4Addr::LOCALHOST, serving.port))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let _ = trickle.write_all(b"GET /vela?t=tok");
        let started = Instant::now();
        let result = page_result(&signing_key(7), &CREDENTIAL, &DIGEST);
        let response = http(serving.port, "GET", &result_query("tok", &result));
        assert!(response.starts_with("HTTP/1.1 200"), "{response}");
        assert!(serving.outcome().is_ok());
        assert!(
            started.elapsed() < HEAD_DEADLINE,
            "the answer waited behind a silent connection"
        );
    }

    /// A head past the cap is dropped unanswered, and the wait goes on.
    #[test]
    fn an_oversized_head_is_dropped() {
        let serving = serve(expect("tok"));
        let huge = format!("{CALLBACK_PATH}?t=tok&pad={}", "a".repeat(HEAD_CAP + 10));
        let response = http(serving.port, "GET", &huge);
        assert!(response.is_empty(), "{response}");
        assert!(!serving.outcome.is_finished());
        serving.stop.store(true, Ordering::SeqCst);
        assert_eq!(serving.outcome(), Err(Refusal::Closed));
    }

    /// The person's "Cancel", and the shell's clock.
    #[test]
    fn a_cancel_is_closed_and_the_clock_is_a_timeout() {
        let serving = serve(expect("tok"));
        serving.stop.store(true, Ordering::SeqCst);
        assert_eq!(serving.outcome(), Err(Refusal::Closed));

        let loopback = Loopback::bind().unwrap_or_else(|e| unreachable!("{e}"));
        assert_eq!(
            loopback.serve(&expect("tok"), Instant::now(), || false),
            Err(Refusal::TimedOut)
        );
    }

    /// The listener is this machine's loopback and nothing else.
    #[test]
    fn the_listener_is_loopback_only() {
        let loopback = Loopback::bind().unwrap_or_else(|e| unreachable!("{e}"));
        let local = loopback
            .listener
            .local_addr()
            .unwrap_or_else(|e| unreachable!("{e}"));
        assert!(local.ip().is_loopback());
        assert_eq!(
            loopback.callback(),
            format!("http://127.0.0.1:{}/vela", local.port())
        );
    }

    /// The URL a ceremony hands the screen: its callback port and its token,
    /// read back out of the fragment the page will read.
    pub(crate) fn callback_of(url: &str) -> (u16, String) {
        let fragment = url.split_once('#').map_or("", |(_, f)| f);
        let field = |name: &str| {
            fragment
                .split('&')
                .find_map(|pair| pair.strip_prefix(&format!("{name}=")))
                .unwrap_or_default()
                .to_owned()
        };
        let callback =
            String::from_utf8(from_base64url(&field("cb")).unwrap_or_default()).unwrap_or_default();
        let port = callback
            .trim_start_matches("http://127.0.0.1:")
            .trim_end_matches(CALLBACK_PATH)
            .parse()
            .unwrap_or_default();
        (port, field("t"))
    }

    /// Wait for a ceremony on `channel` to hand the screen its page.
    pub(crate) fn page_of(channel: &Channel) -> String {
        for _ in 0..400 {
            if let Some(url) = channel.take_page() {
                return url;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        unreachable!("the ceremony never asked for its page")
    }

    /// The whole ceremony as the spine runs it: the screen is handed the
    /// page (and again on "Open the page again", same port and token), the
    /// callback's signature comes back as the assertion the envelope takes.
    #[test]
    fn a_ceremony_hands_over_its_page_and_returns_the_assertion() {
        let (channel, _changed) = Channel::new();
        let keys = expect("unused").keys;
        let request =
            json!({ "intent": { "method": "personal_sign" }, "context": { "chainId": 100 } });
        let ceremony = {
            let (channel, keys, request) = (Arc::clone(&channel), keys.clone(), request.clone());
            std::thread::spawn(move || {
                sign(
                    &request,
                    "https://sign.getvela.app/",
                    &DIGEST,
                    &keys,
                    &channel,
                )
            })
        };
        let url = page_of(&channel);
        assert!(url.starts_with("https://sign.getvela.app/sign.html?ch=url#i="));
        assert!(channel.waiting());
        assert_eq!(channel.take_page(), None, "handed over once");
        channel.reopen();
        assert_eq!(channel.take_page().as_deref(), Some(url.as_str()));

        let (port, token) = callback_of(&url);
        assert_eq!(token.len(), 22, "16 random bytes, base64url");
        let result = page_result(&signing_key(7), &CREDENTIAL, &DIGEST);
        http(port, "GET", &result_query(&token, &result));
        let assertion = ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the ceremony panicked"))
            .unwrap_or_else(|failure| unreachable!("{failure:?}"));
        assert_eq!(assertion.credential_id, "112233");
        assert!(assertion.signature_der_hex.starts_with("30"));
        assert!(!channel.waiting());
        assert_eq!(channel.ended(), None);
    }

    /// "Cancel" on the waiting sheet ends the ceremony as the person
    /// declining: a cancelled passkey to the core, "closed" to the sheet —
    /// and a screen that is gone starts no ceremony at all.
    #[test]
    fn a_cancelled_wait_is_a_cancelled_passkey_and_a_sentence() {
        let (channel, _changed) = Channel::new();
        let keys = expect("unused").keys;
        let ceremony = {
            let (channel, keys) = (Arc::clone(&channel), keys.clone());
            std::thread::spawn(move || {
                sign(
                    &json!({}),
                    "https://sign.getvela.app/",
                    &DIGEST,
                    &keys,
                    &channel,
                )
            })
        };
        page_of(&channel);
        channel.cancel();
        let failure = ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the ceremony panicked"))
            .err()
            .unwrap_or_else(|| unreachable!("a cancelled wait signed"));
        assert_eq!(failure.kind, FailureKind::Cancelled);
        assert_eq!(channel.ended(), Some(Refusal::Closed));
        channel.forget();
        assert_eq!(channel.ended(), None);

        channel.close();
        let refused = sign(
            &json!({}),
            "https://sign.getvela.app/",
            &DIGEST,
            &keys,
            &channel,
        );
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
        let keys = expect("unused").keys;
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
}
