//! Connecting the AI agent with clientmcp.dev (spec 106): the wallet's side of
//! an OAuth authorization-code exchange with PKCE (RFC 6749, RFC 7636, RFC
//! 8252). The person signs in on the site; the site sends the browser back to
//! the wallet's return address with a one-time code; the wallet trades the
//! code and its PKCE verifier for the bridge address and a client token.
//!
//! Pure, like the rest of the core: the shell hands in random bytes and the
//! time, opens the browser, receives the return address and does the one
//! HTTP request. Everything that decides is here, once, for every shell.
//!
//! [`json`] is the same, JSON in and out, for wasm and uniffi.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The application this wallet is on clientmcp.dev.
pub const APP: &str = "vela";
/// The phones' return address (RFC 8252 §7.1), as registered on the site.
pub const APP_REDIRECT: &str = "app.getvela.wallet:/clientmcp";
/// The path of the web wallet's and the desktop's return address.
pub const RETURN_PATH: &str = "/clientmcp";
/// A sign-in left unfinished this long is forgotten.
pub const PENDING_MS: u64 = 10 * 60 * 1000;
/// The random bytes [`begin`] needs: the verifier's and the state's.
pub const RANDOM_LEN: usize = 64;

/// A sign-in under way: kept by the shell (it is not a secret worth a vault —
/// the verifier is useless without the code, and both expire in minutes)
/// until the browser comes back.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pending {
    /// Where to send the person: the site's `/connect` page, with the request.
    pub authorize_url: String,
    pub site: String,
    pub redirect_uri: String,
    pub state: String,
    pub verifier: String,
    pub started_ms: u64,
}

/// The one request that trades the code: `POST url`, JSON `body`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenRequest {
    pub url: String,
    pub body: String,
}

/// What the wallet is given: applied as the person's own `configure_agent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Linked {
    pub bridge_url: String,
    pub client_token: String,
    pub client_id: String,
    pub login: String,
    /// Seconds since the epoch.
    pub expires_at: u64,
    /// The MCP server address for the person's AI agent, its token in the
    /// path — a credential, kept with the client token. Absent from a site
    /// that does not hand it over.
    #[serde(default)]
    pub agent_url: Option<String>,
}

/// Why connecting did not work, for the person.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "kind", content = "detail", rename_all = "snake_case")]
pub enum LinkError {
    /// The site said no; its words, if it gave any.
    #[error("clientmcp.dev refused: {0}")]
    Refused(String),
    /// The site could not issue a token just now; trying again may work.
    #[error("clientmcp.dev is unavailable")]
    Unavailable,
    /// The sign-in was started more than [`PENDING_MS`] ago.
    #[error("the sign-in expired")]
    Expired,
    /// A return that is not this sign-in's: another state, another address.
    #[error("this is not the sign-in the wallet started")]
    NotOurs,
    /// Something that is not what the protocol says.
    #[error("unexpected answer: {0}")]
    Malformed(String),
}

/// Starts a sign-in. `site` is `https://…` (or a loopback `http://` test
/// site); `redirect_uri` is where the site sends the browser back; `label`
/// is what the person sees this installation called on the site;
/// `installation` is this installation's stable ID ([`installation_id`]).
pub fn begin(
    site: &str,
    redirect_uri: &str,
    label: &str,
    installation: &str,
    random: &[u8],
    now_ms: u64,
) -> Result<Pending, LinkError> {
    let site = site.trim_end_matches('/');
    if !(site.starts_with("https://") || is_loopback_http(site)) || site.contains(['?', '#']) {
        return Err(LinkError::Malformed(format!("site {site}")));
    }
    if random.len() < RANDOM_LEN {
        return Err(LinkError::Malformed("not enough randomness".into()));
    }
    let verifier = URL_SAFE_NO_PAD.encode(&random[..32]);
    let state = URL_SAFE_NO_PAD.encode(&random[32..64]);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let query = [
        ("app", APP),
        ("redirect_uri", redirect_uri),
        ("state", &state),
        ("code_challenge", &challenge),
        ("code_challenge_method", "S256"),
        ("label", label),
        ("installation", installation),
    ]
    .iter()
    .map(|(k, v)| format!("{k}={}", encode(v)))
    .collect::<Vec<_>>()
    .join("&");
    Ok(Pending {
        authorize_url: format!("{site}/connect?{query}"),
        site: site.to_owned(),
        redirect_uri: redirect_uri.to_owned(),
        state,
        verifier,
        started_ms: now_ms,
    })
}

/// The code in the address the browser came back to, if it is this sign-in's.
pub fn returned(pending: &Pending, url: &str, now_ms: u64) -> Result<String, LinkError> {
    if now_ms.saturating_sub(pending.started_ms) > PENDING_MS {
        return Err(LinkError::Expired);
    }
    let url = url.split('#').next().unwrap_or_default();
    let (address, query) = url.split_once('?').unwrap_or((url, ""));
    if address != pending.redirect_uri {
        return Err(LinkError::NotOurs);
    }
    let params = parse_query(query);
    let get = |name: &str| {
        params
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    if get("state") != Some(pending.state.as_str()) {
        return Err(LinkError::NotOurs);
    }
    if let Some(error) = get("error") {
        return Err(LinkError::Refused(
            get("error_description").unwrap_or(error).to_owned(),
        ));
    }
    match get("code") {
        Some(code) if !code.is_empty() => Ok(code.to_owned()),
        _ => Err(LinkError::Malformed("no code".into())),
    }
}

/// The request that trades `code` for the token.
#[must_use]
pub fn exchange(pending: &Pending, code: &str) -> TokenRequest {
    TokenRequest {
        url: format!("{}/api/token", pending.site),
        body: serde_json::json!({
            "grant_type": "authorization_code",
            "code": code,
            "code_verifier": pending.verifier,
            "redirect_uri": pending.redirect_uri,
        })
        .to_string(),
    }
}

/// The site's answer to [`exchange`]'s request.
pub fn accept(status: u16, body: &str) -> Result<Linked, LinkError> {
    let value: serde_json::Value = serde_json::from_str(body).unwrap_or_default();
    let text = |name: &str| value.get(name).and_then(serde_json::Value::as_str);
    if status != 200 {
        if status >= 500 || text("error") == Some("temporarily_unavailable") {
            return Err(LinkError::Unavailable);
        }
        let reason = text("error_description").or(text("error"));
        return Err(LinkError::Refused(
            reason.map_or_else(|| format!("HTTP {status}"), str::to_owned),
        ));
    }
    let field = |name: &str| {
        text(name)
            .filter(|v| !v.is_empty())
            .map(str::to_owned)
            .ok_or_else(|| LinkError::Malformed(format!("no {name}")))
    };
    let linked = Linked {
        bridge_url: field("bridge_url")?,
        client_token: field("client_token")?,
        client_id: field("client_id")?,
        login: field("login")?,
        expires_at: value
            .get("expires_at")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0),
        agent_url: text("agent_url")
            .filter(|v| !v.is_empty())
            .map(str::to_owned),
    };
    if !(linked.bridge_url.starts_with("wss://") || linked.bridge_url.starts_with("ws://")) {
        return Err(LinkError::Malformed("bridge_url".into()));
    }
    // An address an agent is told to send its calls to: encrypted, or this machine.
    if linked
        .agent_url
        .as_deref()
        .is_some_and(|url| !(url.starts_with("https://") || is_loopback_http(url)))
    {
        return Err(LinkError::Malformed("agent_url".into()));
    }
    Ok(linked)
}

/// A new installation ID from 16 random bytes: made once, then kept.
#[must_use]
pub fn installation_id(random: &[u8]) -> String {
    URL_SAFE_NO_PAD.encode(&random[..random.len().min(16)])
}

/// The desktop's loopback return address on `port`.
#[must_use]
pub fn loopback_redirect(port: u16) -> String {
    format!("http://127.0.0.1:{port}{RETURN_PATH}")
}

/// The site's account page, where the person connects their AI agent.
#[must_use]
pub fn account_url(site: &str) -> String {
    format!("{}/account", site.trim_end_matches('/'))
}

fn is_loopback_http(site: &str) -> bool {
    ["http://127.0.0.1:", "http://localhost:"]
        .iter()
        .any(|p| site.starts_with(p))
}

/// Percent-encodes everything but RFC 3986's unreserved characters.
fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(byte as char);
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
            (decode(k), decode(v))
        })
        .collect()
}

fn decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'+' => out.push(b' '),
            b'%' => match (
                bytes.get(i + 1).and_then(hex),
                bytes.get(i + 2).and_then(hex),
            ) {
                (Some(high), Some(low)) => {
                    out.push(high << 4 | low);
                    i += 2;
                }
                _ => out.push(b'%'),
            },
            b => out.push(b),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex(digit: &u8) -> Option<u8> {
    (*digit as char).to_digit(16).map(|d| d as u8)
}

/// The same, JSON in and out: what wasm and uniffi export. Every answer is
/// `{"ok": …}` or `{"error": {"kind": …, "detail": …}}`.
pub mod json {
    use super::{LinkError, Pending};
    use serde::Serialize;

    fn answer<T: Serialize>(result: Result<T, LinkError>) -> String {
        match result {
            Ok(ok) => serde_json::json!({ "ok": ok }).to_string(),
            Err(error) => serde_json::json!({ "error": error }).to_string(),
        }
    }

    fn pending(text: &str) -> Result<Pending, LinkError> {
        serde_json::from_str(text).map_err(|_| LinkError::Malformed("pending".into()))
    }

    #[must_use]
    pub fn begin(
        site: &str,
        redirect_uri: &str,
        label: &str,
        installation: &str,
        random: &[u8],
        now_ms: u64,
    ) -> String {
        answer(super::begin(
            site,
            redirect_uri,
            label,
            installation,
            random,
            now_ms,
        ))
    }

    /// `pending_json` is [`begin`]'s `ok`, as kept by the shell.
    #[must_use]
    pub fn returned(pending_json: &str, url: &str, now_ms: u64) -> String {
        answer(pending(pending_json).and_then(|p| super::returned(&p, url, now_ms)))
    }

    #[must_use]
    pub fn exchange(pending_json: &str, code: &str) -> String {
        answer(pending(pending_json).map(|p| super::exchange(&p, code)))
    }

    #[must_use]
    pub fn accept(status: u16, body: &str) -> String {
        answer(super::accept(status, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SITE: &str = "https://clientmcp.dev";
    const RANDOM: [u8; 64] = {
        let mut r = [0u8; 64];
        let mut i = 0;
        while i < 64 {
            r[i] = i as u8;
            i += 1;
        }
        r
    };

    fn started() -> Pending {
        begin(
            SITE,
            APP_REDIRECT,
            "iPhone 15",
            "inst-0123456789",
            &RANDOM,
            1_000,
        )
        .unwrap()
    }

    #[test]
    fn the_request_carries_an_s256_challenge_of_the_verifier() {
        let p = started();
        // RFC 7636 appendix B's construction, on this verifier.
        let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(p.verifier.as_bytes()));
        assert_eq!(p.verifier.len(), 43);
        assert_eq!(
            p.authorize_url,
            format!(
                "https://clientmcp.dev/connect?app=vela&redirect_uri=app.getvela.wallet%3A%2Fclientmcp&state={}&code_challenge={challenge}&code_challenge_method=S256&label=iPhone%2015&installation=inst-0123456789",
                p.state
            )
        );
        assert_ne!(p.state, p.verifier);
    }

    #[test]
    fn rfc_7636_appendix_b() {
        // The RFC's own verifier and challenge.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn a_site_is_https_or_a_loopback_test_site() {
        assert!(begin(
            "http://clientmcp.dev",
            APP_REDIRECT,
            "x",
            "inst-0123456789",
            &RANDOM,
            0
        )
        .is_err());
        assert!(begin(
            "https://clientmcp.dev?x",
            APP_REDIRECT,
            "x",
            "inst-0123456789",
            &RANDOM,
            0
        )
        .is_err());
        assert!(begin(
            "http://127.0.0.1:18798",
            APP_REDIRECT,
            "x",
            "inst-0123456789",
            &RANDOM,
            0
        )
        .is_ok());
        assert_eq!(
            begin(
                "https://staging.clientmcp.dev/",
                APP_REDIRECT,
                "x",
                "inst-0123456789",
                &RANDOM,
                0
            )
            .unwrap()
            .site,
            "https://staging.clientmcp.dev"
        );
        assert!(begin(SITE, APP_REDIRECT, "x", "i", &RANDOM[..63], 0).is_err());
    }

    #[test]
    fn the_return_gives_the_code_only_to_its_own_sign_in() {
        let p = started();
        let back = |q: &str| format!("{APP_REDIRECT}?{q}");
        let ok = back(&format!("code=abc%2Bd&state={}", p.state));
        assert_eq!(returned(&p, &ok, 2_000).unwrap(), "abc+d");
        assert_eq!(returned(&p, &format!("{ok}#frag"), 2_000).unwrap(), "abc+d");
        assert_eq!(
            returned(&p, &back("code=abc&state=other"), 2_000),
            Err(LinkError::NotOurs)
        );
        assert_eq!(
            returned(
                &p,
                &format!("velawallet://clientmcp?code=abc&state={}", p.state),
                2_000
            ),
            Err(LinkError::NotOurs)
        );
        assert_eq!(
            returned(&p, &ok, 1_000 + PENDING_MS + 1),
            Err(LinkError::Expired)
        );
        assert_eq!(
            returned(&p, &back(&format!("state={}", p.state)), 2_000),
            Err(LinkError::Malformed("no code".into()))
        );
        assert_eq!(
            returned(
                &p,
                &back(&format!(
                    "error=access_denied&error_description=Not+now&state={}",
                    p.state
                )),
                2_000
            ),
            Err(LinkError::Refused("Not now".into()))
        );
    }

    #[test]
    fn the_desktop_returns_to_its_own_port() {
        let p = begin(
            SITE,
            &loopback_redirect(53682),
            "Mac",
            "inst-0123456789",
            &RANDOM,
            0,
        )
        .unwrap();
        assert_eq!(p.redirect_uri, "http://127.0.0.1:53682/clientmcp");
        let state = &p.state;
        assert!(returned(
            &p,
            &format!("http://127.0.0.1:53682/clientmcp?code=c&state={state}"),
            1
        )
        .is_ok());
        assert_eq!(
            returned(
                &p,
                &format!("http://127.0.0.1:53683/clientmcp?code=c&state={state}"),
                1
            ),
            Err(LinkError::NotOurs)
        );
    }

    #[test]
    fn the_exchange_sends_the_verifier_to_the_site() {
        let p = started();
        let request = exchange(&p, "the-code");
        assert_eq!(request.url, "https://clientmcp.dev/api/token");
        let body: serde_json::Value = serde_json::from_str(&request.body).unwrap();
        assert_eq!(body["grant_type"], "authorization_code");
        assert_eq!(body["code"], "the-code");
        assert_eq!(body["code_verifier"], p.verifier.as_str());
        assert_eq!(body["redirect_uri"], APP_REDIRECT);
    }

    #[test]
    fn the_answer_is_the_bridge_and_the_token_or_why_not() {
        let ok = r#"{"token_type":"clientmcp","client_id":"c-abc","client_token":"eyJ.x.y","expires_at":1830000000,"bridge_url":"wss://bridge.clientmcp.dev/connect","mcp_url":"https://bridge.clientmcp.dev/mcp","login":"alice"}"#;
        assert_eq!(
            accept(200, ok).unwrap(),
            Linked {
                bridge_url: "wss://bridge.clientmcp.dev/connect".into(),
                client_token: "eyJ.x.y".into(),
                client_id: "c-abc".into(),
                login: "alice".into(),
                expires_at: 1_830_000_000,
                agent_url: None,
            }
        );
        let with_agent = ok.replace(
            r#""login""#,
            r#""agent_url":"https://bridge.clientmcp.dev/mcp/eyJ.a.b","login""#,
        );
        assert_eq!(
            accept(200, &with_agent).unwrap().agent_url.as_deref(),
            Some("https://bridge.clientmcp.dev/mcp/eyJ.a.b")
        );
        assert!(matches!(
            accept(200, &with_agent.replace("https://bridge", "http://bridge")),
            Err(LinkError::Malformed(_))
        ));
        assert_eq!(
            accept(
                400,
                r#"{"error":"invalid_grant","error_description":"The code is unknown, used or expired."}"#
            ),
            Err(LinkError::Refused(
                "The code is unknown, used or expired.".into()
            ))
        );
        assert_eq!(accept(503, "{}"), Err(LinkError::Unavailable));
        assert_eq!(accept(502, "<html>"), Err(LinkError::Unavailable));
        assert_eq!(accept(403, ""), Err(LinkError::Refused("HTTP 403".into())));
        assert!(matches!(
            accept(200, r#"{"client_token":"x"}"#),
            Err(LinkError::Malformed(_))
        ));
        assert!(matches!(
            accept(200, &ok.replace("wss://", "https://")),
            Err(LinkError::Malformed(_))
        ));
    }

    #[test]
    fn the_json_face_round_trips_the_pending_sign_in() {
        let begun: serde_json::Value = serde_json::from_str(&json::begin(
            SITE,
            APP_REDIRECT,
            "x",
            "inst-0123456789",
            &RANDOM,
            5,
        ))
        .unwrap();
        let pending = begun["ok"].to_string();
        let state = begun["ok"]["state"].as_str().unwrap();
        let code: serde_json::Value = serde_json::from_str(&json::returned(
            &pending,
            &format!("{APP_REDIRECT}?code=k&state={state}"),
            6,
        ))
        .unwrap();
        assert_eq!(code["ok"], "k");
        let refused: serde_json::Value =
            serde_json::from_str(&json::returned(&pending, "nope", 6)).unwrap();
        assert_eq!(refused["error"]["kind"], "not_ours");
        let request: serde_json::Value =
            serde_json::from_str(&json::exchange(&pending, "k")).unwrap();
        assert_eq!(request["ok"]["url"], "https://clientmcp.dev/api/token");
        let answer: serde_json::Value = serde_json::from_str(&json::accept(503, "")).unwrap();
        assert_eq!(answer["error"]["kind"], "unavailable");
        assert_eq!(installation_id(&RANDOM).len(), 22);
    }

    #[test]
    fn decoding_survives_odd_input() {
        assert_eq!(decode("a%2"), "a%2");
        assert_eq!(decode("%"), "%");
        assert_eq!(decode("%zz"), "%zz");
        assert_eq!(decode("%E4%BD%A0"), "你");
        assert_eq!(decode("%你"), "%你");
    }
}
