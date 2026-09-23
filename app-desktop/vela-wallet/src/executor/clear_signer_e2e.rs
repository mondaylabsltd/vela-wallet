//! The Clear Signer against the REAL page in a real browser (specs 071 and
//! 075).
//!
//! Everything here is `#[ignore]`d: it needs Chrome, so it is a local check
//! and never a CI one. Run it with `scripts/clear-signer-e2e.sh`, which finds
//! Chrome for Testing and says what it needs. Every port is picked by the OS —
//! the page's server, Chrome's DevTools and the wallet's own listener — so
//! several of these can run beside each other and beside anybody else's
//! browser.
//!
//! **Spec 075: the channel is the loopback WebSocket.** The wallet listens on
//! `127.0.0.1:0`, hands the browser `sign.html?ch=ws#p=<port>&t=<token>`, and
//! the page connects back and stays connected — so a create and the sign-in
//! after it run on ONE page visit, which is the whole point of the move off
//! the URL fragment.
//!
//! What is real: the request the core builds (`Ask::request`,
//! `ceremony::request`), the launch URL (`ws_launch`), the wallet's listener
//! and the core's own WebSocket framing (`ws::Connection`), the page itself —
//! decoding, the operation-binding check, the digest and the challenges it
//! derives on its own, the WebAuthn ceremony in a CDP virtual authenticator —
//! and the wallet's verification of what comes back (`clear_signer::verify`,
//! `ceremony::verify`). What is stood in: the person's slide (the page's
//! automation hook), the person's answer to "where is your Clear Signer?", and
//! `cx.open_url` (a CDP `Target` opened on the same URL).
//!
//! The page is served from this repository on `http://localhost:<port>/`, a
//! secure context whose passkeys live under rpId `localhost`. The wallet
//! never checks the rpId hash (research R5), so the key signs for the wallet
//! exactly as it would through the official page — the same stand-in the
//! phones' device pass uses.

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use p256::pkcs8::EncodePrivateKey as _;
use serde_json::{Value, json};

use vela_core::primitives::{to_base64url, to_hex};
use vela_core::user_op::{
    MultiSendCall, UserOperation, WalletKey, build_in_band_fee_leg,
    build_multi_send_execute_call_data, calculate_safe_op_hash,
};

use vela_core::app::KeyMethod;
use vela_core::app::shell::ShellOperation;
use vela_core::clear_signer::ceremony::Answer;

use crate::executor::clear_signer::tests::{page_of_channel, signing_key};
use crate::executor::clear_signer::{self, Ask, Channel, Refusal};
use crate::executor::user_op::{self, Signer};

const SAFE: &str = "0x88cCA0EeDbF2C4426110bbFc998F048689266894";
const CHAIN: u32 = 100;

// ---------------------------------------------------------------------------
// The page, the browser, and the page's tab
// ---------------------------------------------------------------------------

/// `app-web/clearsigning`, served as files on this machine's loopback.
fn serve_page() -> u16 {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../app-web/clearsigning");
    let listener =
        TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap_or_else(|e| unreachable!("{e}"));
    let port = listener.local_addr().map(|a| a.port()).unwrap_or_default();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let root = root.clone();
            std::thread::spawn(move || serve_file(stream, &root));
        }
    });
    port
}

fn serve_file(mut stream: TcpStream, root: &Path) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut head = Vec::new();
    let mut chunk = [0u8; 4096];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        match stream.read(&mut chunk) {
            Ok(0) | Err(_) => return,
            Ok(n) => head.extend_from_slice(&chunk[..n]),
        }
    }
    let head = String::from_utf8_lossy(&head);
    let target = head.split(' ').nth(1).unwrap_or("/");
    let path = target.split(['?', '#']).next().unwrap_or("/");
    let file = (!path.contains(".."))
        .then(|| root.join(path.trim_start_matches('/')))
        .and_then(|file| std::fs::read(&file).ok().map(|body| (file, body)));
    let response = match file {
        Some((file, body)) => {
            let kind = match file.extension().and_then(|e| e.to_str()) {
                Some("html") => "text/html; charset=utf-8",
                Some("js") => "text/javascript; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("json") => "application/json",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                _ => "application/octet-stream",
            };
            let mut response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {kind}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .into_bytes();
            response.extend(body);
            response
        }
        None => {
            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()
        }
    };
    let _ = stream.write_all(&response);
}

/// Headless Chrome with its DevTools on a port it picks, in a profile of its
/// own that goes with it.
struct Browser {
    child: Child,
    profile: PathBuf,
    port: u16,
}

impl Browser {
    /// `None` without `CHROME_BIN` — the test then says so and passes.
    fn launch() -> Option<Self> {
        let bin = std::env::var("CHROME_BIN").ok()?;
        let base = std::env::var("CLEAR_SIGNER_E2E_DIR")
            .map_or_else(|_| std::env::temp_dir(), PathBuf::from);
        let profile = base.join(format!(
            "clear-signer-e2e-{}",
            to_hex(&crate::executor::passkey::random(6), false)
        ));
        let child = Command::new(bin)
            .args([
                "--headless=new",
                "--remote-debugging-port=0",
                &format!("--user-data-dir={}", profile.display()),
                // A system proxy would carry `localhost` somewhere else.
                "--no-proxy-server",
                "--host-resolver-rules=MAP localhost 127.0.0.1",
                "--no-first-run",
                "--no-default-browser-check",
                "about:blank",
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap_or_else(|e| unreachable!("CHROME_BIN would not start: {e}"));
        // `--remote-debugging-port=0` writes the port it chose here.
        let active = profile.join("DevToolsActivePort");
        let deadline = Instant::now() + Duration::from_secs(20);
        let port = loop {
            if let Some(port) = std::fs::read_to_string(&active)
                .ok()
                .and_then(|text| text.lines().next()?.trim().parse().ok())
            {
                break port;
            }
            assert!(Instant::now() < deadline, "Chrome never opened DevTools");
            std::thread::sleep(Duration::from_millis(100));
        };
        Some(Self {
            child,
            profile,
            port,
        })
    }

    /// One plain HTTP exchange with DevTools' own endpoints.
    fn devtools(&self, method: &str, path: &str) -> String {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
        let request = format!(
            "{method} {path} HTTP/1.1\r\nHost: 127.0.0.1:{}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
            self.port
        );
        let _ = stream.write_all(request.as_bytes());
        let mut response = String::new();
        let _ = stream.read_to_string(&mut response);
        response
            .split_once("\r\n\r\n")
            .map(|(_, body)| body.to_owned())
            .unwrap_or_default()
    }

    /// A new tab on `url` — what `cx.open_url` does with the default browser.
    fn open(&self, url: &str) -> Tab {
        let target: Value =
            serde_json::from_str(&self.devtools("PUT", &format!("/json/new?{}", encode(url))))
                .unwrap_or_else(|e| unreachable!("DevTools did not open a tab: {e}"));
        let ws_url = target["webSocketDebuggerUrl"].as_str().unwrap_or_default();
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port))
            .unwrap_or_else(|e| unreachable!("{e}"));
        let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
        let (socket, _) =
            tungstenite::client(ws_url, stream).unwrap_or_else(|e| unreachable!("CDP socket: {e}"));
        let mut tab = Tab {
            socket,
            next: 0,
            id: target["id"].as_str().unwrap_or_default().to_owned(),
        };
        tab.call("Runtime.enable", json!({}));
        tab
    }

    fn close(&self, tab: &Tab) {
        self.devtools("GET", &format!("/json/close/{}", tab.id));
    }
}

impl Drop for Browser {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

struct Tab {
    socket: tungstenite::WebSocket<TcpStream>,
    next: u64,
    id: String,
}

impl Tab {
    /// One CDP command and its result; events on the way are skipped.
    fn call(&mut self, method: &str, params: Value) -> Value {
        self.next += 1;
        let id = self.next;
        let command = json!({ "id": id, "method": method, "params": params });
        self.socket
            .send(tungstenite::Message::text(command.to_string()))
            .unwrap_or_else(|e| unreachable!("{method}: {e}"));
        loop {
            let message = self
                .socket
                .read()
                .unwrap_or_else(|e| unreachable!("{method}: {e}"));
            let Ok(text) = message.to_text() else {
                continue;
            };
            let reply: Value = serde_json::from_str(text).unwrap_or_default();
            if reply["id"] == id {
                assert!(reply.get("error").is_none(), "{method}: {reply}");
                return reply["result"].clone();
            }
        }
    }

    fn eval(&mut self, expression: &str) -> Value {
        self.call(
            "Runtime.evaluate",
            json!({ "expression": expression, "returnByValue": true, "userGesture": true }),
        )["result"]["value"]
            .clone()
    }

    fn wait_for(&mut self, expression: &str) -> bool {
        let deadline = Instant::now() + Duration::from_secs(20);
        while Instant::now() < deadline {
            if self.eval(expression) == json!(true) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(150));
        }
        false
    }

    /// An empty platform authenticator on this tab — what a create needs: a
    /// vault that can mint a key, holding none yet.
    ///
    /// A CDP virtual authenticator belongs to the TARGET that added it, and
    /// survives navigation inside that target. That is what lets one tab carry
    /// a create and then a sign-in that finds the key the create just made.
    fn add_authenticator(&mut self) -> Value {
        self.call("WebAuthn.enable", json!({}));
        self.call(
            "WebAuthn.addVirtualAuthenticator",
            json!({ "options": {
                "protocol": "ctap2", "transport": "internal",
                "hasResidentKey": true, "hasUserVerification": true,
                "isUserVerified": true, "automaticPresenceSimulation": true,
            }}),
        )["authenticatorId"]
            .clone()
    }

    /// The same authenticator, already holding the wallet's key under
    /// `localhost` — the automated "this person enrolled last week", for the
    /// signing cases where the page never creates anything.
    fn add_passkey(&mut self, key: &p256::ecdsa::SigningKey, credential: &[u8]) {
        let authenticator = self.add_authenticator();
        let pkcs8 = key.to_pkcs8_der().unwrap_or_else(|e| unreachable!("{e}"));
        self.call(
            "WebAuthn.addCredential",
            json!({
                "authenticatorId": authenticator,
                "credential": {
                    "credentialId": base64(credential),
                    "isResidentCredential": true,
                    "rpId": "localhost",
                    "privateKey": base64(pkcs8.as_bytes()),
                    "userHandle": base64(b"vela-e2e"),
                    "signCount": 0,
                },
            }),
        );
    }
}

/// CDP's base64 is the standard alphabet, padded.
fn base64(bytes: &[u8]) -> String {
    let mut text = to_base64url(bytes).replace('-', "+").replace('_', "/");
    while !text.len().is_multiple_of(4) {
        text.push('=');
    }
    text
}

/// The whole URL as one query value — DevTools' `/json/new?` would drop the
/// fragment, and the request lives in the fragment.
fn encode(url: &str) -> String {
    url.bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            other => format!("%{other:02X}"),
        })
        .collect()
}

// ---------------------------------------------------------------------------
// One ceremony, driven
// ---------------------------------------------------------------------------

struct Rig {
    browser: Browser,
    page: String,
    key: p256::ecdsa::SigningKey,
    credential: Vec<u8>,
    keys: Vec<WalletKey>,
    channel: Arc<Channel>,
}

impl Rig {
    fn new() -> Option<Self> {
        let Some(browser) = Browser::launch() else {
            eprintln!("CHROME_BIN is not set — scripts/clear-signer-e2e.sh sets it");
            return None;
        };
        let key = signing_key(7);
        let credential = crate::executor::passkey::random(16);
        let keys = vec![
            // Another founding key first, so the one that signs is not simply
            // the only one there is.
            crate::executor::clear_signer::tests::wallet_key(&signing_key(9), &[0x99]),
            crate::executor::clear_signer::tests::wallet_key(&key, &credential),
        ];
        let channel = Channel::new().0;
        Some(Self {
            browser,
            page: format!("http://localhost:{}/", serve_page()),
            key,
            credential,
            keys,
            channel,
        })
    }

    /// The launch URL the attempt asked the screen to open, opened — with the
    /// wallet's key already in the browser.
    fn open_page(&self) -> Tab {
        let url = self.launch();
        let mut tab = self.browser.open(&url);
        tab.add_passkey(&self.key, &self.credential);
        tab
    }

    /// The launch URL, once the attempt has handed it over. It carries the
    /// listener's port and its one-time token in the fragment, which the page
    /// wipes from history the moment it reads it.
    fn launch(&self) -> String {
        let url = page_of_channel(&self.channel);
        assert!(url.contains("sign.html?ch=ws#p="), "{url}");
        url
    }

    /// Stops the ceremony if the driving side fails, so a failed assertion
    /// ends the test now rather than when the five minutes run out.
    fn guard(&self) -> Stop<'_> {
        Stop(&self.channel)
    }

    /// Wait for the attempt to stop waiting; a stuck one is cancelled so the
    /// test ends with a failure rather than five minutes later.
    fn settle(&self) {
        let deadline = Instant::now() + Duration::from_secs(30);
        while self.channel.waiting() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(100));
        }
        if self.channel.waiting() {
            self.channel.cancel();
            unreachable!("the page never answered the socket");
        }
    }
}

struct Stop<'a>(&'a Channel);

impl Drop for Stop<'_> {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.0.close();
        }
    }
}

/// The page's own words for why it will not sign, for a failure message.
fn why(tab: &mut Tab) -> String {
    tab.eval(
        "[location.href, JSON.stringify(window.__velaState || null), \
          document.getElementById('status') ? document.getElementById('status').textContent : '', \
          ...[...document.querySelectorAll('.warning-text')].map(n => n.textContent)].join(' | ')",
    )
    .as_str()
    .unwrap_or_default()
    .to_owned()
}

/// The wallet's own send, as the spine assembles it: the person's calls,
/// then the in-band fee leg, in one MultiSend.
fn assembled(calls: &[MultiSendCall]) -> UserOperation {
    let mut legs = calls.to_vec();
    legs.push(
        build_in_band_fee_leg(None, "0x4d2C7a3B1e9F0a8b7C6d5E4f3A2b1C0d9E8f7A6b", 420_000)
            .unwrap_or_else(|e| unreachable!("{e}")),
    );
    UserOperation {
        sender: SAFE.to_owned(),
        nonce: "0x7".to_owned(),
        init_code: vec![],
        call_data: build_multi_send_execute_call_data(&legs)
            .unwrap_or_else(|e| unreachable!("{e}")),
        verification_gas_limit: 300_000,
        call_gas_limit: 200_000,
        pre_verification_gas: 110_000,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        paymaster_and_data: vec![],
        signature: vec![],
    }
}

fn dust(to: &str) -> MultiSendCall {
    MultiSendCall {
        to: to.to_owned(),
        value_hex: "0x38d7ea4c68000".to_owned(),
        data: vec![],
    }
}

/// The wallet's own send: the page decodes the operation the wallet
/// assembled, derives the SafeOp hash from it, signs — and the wallet takes
/// the answer only because that hash is the one it computed.
#[test]
#[ignore = "needs a real browser: scripts/clear-signer-e2e.sh"]
fn the_page_signs_the_operation_the_wallet_assembled() {
    let Some(rig) = Rig::new() else { return };
    let calls = vec![dust("0x031d7D57c99CAF891e1C250554691Fd12D84772b")];
    let op = assembled(&calls);
    let digest =
        calculate_safe_op_hash(&op, u64::from(CHAIN)).unwrap_or_else(|e| unreachable!("{e}"));
    let request =
        Ask::own(Some("E2E".to_owned())).request(CHAIN, SAFE, &rig.keys, Some((&op, &calls)));
    let signed = std::thread::scope(|scope| {
        let ceremony = scope
            .spawn(|| clear_signer::sign(&request, &rig.page, &digest, &rig.keys, &rig.channel));
        let _stop = rig.guard();
        let mut tab = rig.open_page();
        assert!(
            tab.wait_for("!!window.__slider"),
            "the page did not offer to sign"
        );
        tab.eval("window.__slider.__confirm()");
        rig.settle();
        ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the ceremony panicked"))
    });
    let assertion = signed.unwrap_or_else(|failure| unreachable!("not accepted: {failure:?}"));
    assert_eq!(assertion.credential_id, to_hex(&rig.credential, false));
    assert_eq!(rig.channel.ended(), None);
}

/// A dApp's `personal_sign`: the page wraps EIP-191 in the Safe's
/// `SafeMessage` exactly as the wallet does, and the answer comes back as
/// the EIP-1271 signature a passkey's would.
#[test]
#[ignore = "needs a real browser: scripts/clear-signer-e2e.sh"]
fn the_page_signs_a_message_as_the_wallet_hashes_it() {
    let Some(rig) = Rig::new() else { return };
    let params = json!(["0x68656c6c6f", SAFE]).to_string();
    let original = crate::executor::sign_request::message_hash("personal_sign", &params)
        .unwrap_or_else(|| unreachable!("hello hashes"));
    let ask = Ask {
        method: "personal_sign".to_owned(),
        params: serde_json::from_str(&params).unwrap_or_default(),
        origin: "https://app.example".to_owned(),
        account_name: None,
    };
    let signed = std::thread::scope(|scope| {
        let ceremony = scope.spawn(|| {
            user_op::sign_message(
                CHAIN,
                SAFE,
                &original,
                &rig.keys,
                Signer::ClearSigner {
                    ask: &ask,
                    page: &rig.page,
                    channel: &rig.channel,
                },
            )
        });
        let _stop = rig.guard();
        let mut tab = rig.open_page();
        assert!(
            tab.wait_for("!!window.__slider"),
            "the page did not offer to sign"
        );
        tab.eval("window.__slider.__confirm()");
        rig.settle();
        ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the ceremony panicked"))
    });
    let signature = signed.unwrap_or_else(|failure| unreachable!("not accepted: {failure:?}"));
    assert!(
        signature.starts_with("0x") && signature.len() > 400,
        "{signature}"
    );
}

/// The tab closed without a slide: its beacon is the person declining, and
/// the request stays open with "closed" to say.
#[test]
#[ignore = "needs a real browser: scripts/clear-signer-e2e.sh"]
fn a_tab_closed_unsigned_is_a_decline() {
    let Some(rig) = Rig::new() else { return };
    let calls = vec![dust("0x031d7D57c99CAF891e1C250554691Fd12D84772b")];
    let op = assembled(&calls);
    let digest =
        calculate_safe_op_hash(&op, u64::from(CHAIN)).unwrap_or_else(|e| unreachable!("{e}"));
    let request = Ask::own(None).request(CHAIN, SAFE, &rig.keys, Some((&op, &calls)));
    let outcome = std::thread::scope(|scope| {
        let ceremony = scope
            .spawn(|| clear_signer::sign(&request, &rig.page, &digest, &rig.keys, &rig.channel));
        let _stop = rig.guard();
        let mut tab = rig.open_page();
        assert!(tab.wait_for("!!window.__slider"));
        rig.browser.close(&tab);
        rig.settle();
        ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the ceremony panicked"))
    });
    assert!(outcome.is_err(), "a closed tab signed");
    assert_eq!(rig.channel.ended(), Some(Refusal::Closed));
}

/// The operation does not carry what the page was told it carries — a
/// swapped recipient. The page refuses on its own rules, and on a session
/// channel it says so AT ONCE (PROTOCOL.md §11) rather than waiting for its
/// tab to close: the wallet hears "refused", not "closed", and the request is
/// still there to be signed another way.
#[test]
#[ignore = "needs a real browser: scripts/clear-signer-e2e.sh"]
fn an_operation_that_is_not_the_request_is_refused_by_the_page() {
    let Some(rig) = Rig::new() else { return };
    let asked = vec![dust("0x031d7D57c99CAF891e1C250554691Fd12D84772b")];
    let op = assembled(&[dust("0x9A8b7C6d5E4F3a2B1c0D9e8F7a6B5c4D3e2F1a09")]);
    let digest =
        calculate_safe_op_hash(&op, u64::from(CHAIN)).unwrap_or_else(|e| unreachable!("{e}"));
    let request = Ask::own(None).request(CHAIN, SAFE, &rig.keys, Some((&op, &asked)));
    let outcome = std::thread::scope(|scope| {
        let ceremony = scope
            .spawn(|| clear_signer::sign(&request, &rig.page, &digest, &rig.keys, &rig.channel));
        let _stop = rig.guard();
        let mut tab = rig.open_page();
        let refused = tab.wait_for("!!window.__refused");
        assert!(
            refused,
            "the page offered to sign a swap: {}",
            why(&mut tab)
        );
        assert_eq!(tab.eval("!!window.__slider"), json!(false));
        rig.settle();
        rig.browser.close(&tab);
        ceremony
            .join()
            .unwrap_or_else(|_| unreachable!("the ceremony panicked"))
    });
    assert!(outcome.is_err());
    assert_eq!(rig.channel.ended(), Some(Refusal::Refused));
}

/// **A wallet created through the Clear Signer, on ONE page visit** (spec 075
/// US1): the page mints the passkey the create machine asked for, and then —
/// on the same socket, with the same tab and no second launch URL — answers
/// the sign-in that finds it again.
///
/// This is the case the URL fragment could not carry, and it is the reason the
/// desktop moved: two ceremonies, one page visit, one `bye`.
///
/// Both challenges are the PAGE's own. The create's is 32 random bytes it
/// generated; the sign-in's is the `vela-signin-<ms>-<hex>` it derived from its
/// own clock, which the core checks the form of before it accepts anything —
/// so a "sign-in" cannot be a transaction hash in disguise.
///
/// The member proof that finishes a real create is NOT here: the page fetches
/// its own challenge from a registry, which means standing one up
/// (`app-web/clearsigning/samples/mock-registry.mjs`, Node). The verification
/// of that answer is covered by
/// `executor::clear_signer::tests::a_create_and_its_member_proof_share_one_page_visit`.
#[test]
#[ignore = "needs a real browser: scripts/clear-signer-e2e.sh"]
fn the_page_creates_a_key_and_then_signs_in_with_it_on_one_visit() {
    let Some(rig) = Rig::new() else { return };
    // A create has no key yet, so it goes to the page SETTINGS names — the
    // real read (`vela.clearSignerUrl`), pointed at this checkout's own copy
    // of the page rather than at the official one.
    let state = crate::executor::storage::tests::state_dir("clear-signer-e2e-create");
    let _ = crate::executor::storage::write_value(
        crate::executor::storage::KEY_CLEAR_SIGNER_URL,
        Value::String(rig.page.clone()),
    );
    assert_eq!(
        clear_signer::signer_url(),
        rig.page,
        "the create would have opened the official page"
    );
    let register = ShellOperation::RegisterPasskey {
        name: "E2E wallet".to_owned(),
        exclude_credential_ids: vec![],
        method: KeyMethod::ClearSigner,
    };
    let sign_in = ShellOperation::AuthenticatePasskey {
        method: KeyMethod::ClearSigner,
    };
    let registry = "https://p256-index-v2.getvela.app";

    let (created, found) = std::thread::scope(|scope| {
        let flow = scope.spawn(|| {
            let created =
                clear_signer::run_ceremony(&register, None, registry, &rig.channel, false)
                    .unwrap_or_else(|| unreachable!("a create is a ceremony"));
            let found = clear_signer::run_ceremony(&sign_in, None, registry, &rig.channel, true)
                .unwrap_or_else(|| unreachable!("a sign-in is a ceremony"));
            (created, found)
        });
        let _stop = rig.guard();

        // An EMPTY vault: the page has to mint the key itself.
        let url = rig.launch();
        let mut tab = rig.browser.open(&url);
        tab.add_authenticator();

        assert!(
            tab.wait_for(
                "window.__velaState && window.__velaState.phase === 'card' \
                 && window.__velaState.kind === 'create'"
            ),
            "the page did not offer to create a key: {}",
            why(&mut tab)
        );
        tab.eval("window.__slider.__confirm()");

        // The SAME tab and the SAME socket carry the next request: the page
        // answered one and took a second without being reopened. Its own
        // counters say so, and the wallet never asked for another page.
        assert!(
            tab.wait_for(
                "window.__velaState.kind === 'signIn' && window.__velaState.received === 2                  && window.__velaState.answered === 1"
            ),
            "the sign-in card never came up on the same visit: {}",
            why(&mut tab)
        );
        assert_eq!(
            rig.channel.take_page(),
            None,
            "the wallet asked for a second page visit"
        );
        tab.eval("window.__slider.__confirm()");
        rig.settle();

        // The wallet said goodbye, so the page is on its done screen.
        assert!(tab.wait_for("window.__velaState.phase === 'ended'"));
        rig.browser.close(&tab);
        flow.join()
            .unwrap_or_else(|_| unreachable!("the flow panicked"))
    });

    let registration = match created.unwrap_or_else(|failure| unreachable!("create: {failure:?}")) {
        Answer::Registration(registration) => registration,
        Answer::Assertion(_) => unreachable!("a create returns a key"),
    };
    assert!(
        !registration.credential_id.is_empty(),
        "no credential came back"
    );
    // The key remembers the page it was made behind: from here on, `auto`
    // routes its signatures there and nowhere else.
    assert_eq!(
        registration.signer_origin.as_deref(),
        Some(rig.page.trim_end_matches('/')),
        "the key did not record its page"
    );
    // The core had already parsed the attestation to a P-256 key before it
    // accepted this answer at all (`ceremony::verify`); the key it extracted
    // is what the wallet's address will be derived from.
    let attestation = vela_core::primitives::from_hex(&registration.attestation_object_hex)
        .unwrap_or_else(|e| unreachable!("the attestation is not hex: {e}"));
    assert!(
        vela_core::webauthn::extract_attestation_public_key(&attestation).is_ok(),
        "the attestation is not a P-256 key"
    );

    let assertion = match found.unwrap_or_else(|failure| unreachable!("sign in: {failure:?}")) {
        Answer::Assertion(assertion) => assertion,
        Answer::Registration(_) => unreachable!("a sign-in returns a signature"),
    };
    assert_eq!(
        assertion.credential_id, registration.credential_id,
        "the sign-in found a different key than the create made"
    );
    assert_eq!(rig.channel.ended(), None);
    drop(state);
}
