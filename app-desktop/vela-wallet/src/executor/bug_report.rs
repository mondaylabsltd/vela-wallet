//! The in-app report, sent (078 S-03) — the web's `services/bug-report.ts`,
//! ported line for line so a report filed from the desktop reads, dedups
//! and falls back exactly as one filed from the web does.
//!
//! ## Two roads
//!
//! The first is `getvela.app/api/bug-report`: the site's own token files the
//! issue, so somebody without a GitHub account can still report a bug. It
//! answers 503 while that token is unprovisioned, 429 past five reports in
//! ten minutes, 413 over 16 000 characters — none of them the person's fault,
//! so every one of them, and every network fault, falls back to the second
//! road: the repository's issue form, prefilled.
//!
//! ## What may never be in a report
//!
//! Addresses, balances, endpoint or RPC URLs (a self-hosted endpoint carries
//! its API key in its path often enough), raw stored values. The payload is
//! ASSEMBLED from [`DeviceFacts`]' named fields and nothing else; [`redact`]
//! is the second line, for the one field a person can name themselves — a
//! custom network's display name reaches the unreachable list.

use std::time::Duration;

/// The site's proxy. The token lives there; nothing here has one.
pub const BUG_REPORT_ENDPOINT: &str = "https://getvela.app/api/bug-report";

/// The repository's issue form, for the fallback road.
pub const GITHUB_ISSUE_FORM: &str = "https://github.com/mondaylabsltd/vela-wallet/issues/new";

/// The endpoint's own cap, honoured before the request leaves.
pub const MAX_REPORT_CHARS: usize = 16_000;

/// The issue form's `area` option a settings-page report answers — the
/// person's own words carry the real area.
pub const AREA_OTHER: &str = "Other (explain above)";

/// The web's `bundlerRest` budget: a report is small and the person waits.
const TIMEOUT: Duration = Duration::from_millis(10_000);

/// Everything about the device a report is allowed to know — by name, so a
/// balance cannot arrive through an index six months from now.
#[derive(Clone, Debug, Default)]
pub struct DeviceFacts {
    pub version: String,
    pub commit: String,
    pub platform: String,
    pub language: String,
    /// Display NAMES of networks whose RPC is unreachable, never their URLs.
    pub unreachable: Vec<String>,
    /// Failure counters' classes, never values.
    pub failures: Vec<String>,
}

/// The corpus labels the preview lines wear.
#[derive(Clone, Debug, Default)]
pub struct EnvironmentLabels {
    pub version: String,
    pub platform: String,
    pub language: String,
    pub rpc: String,
    pub failures: String,
    pub none: String,
}

/// The five fields the endpoint accepts, and the only five sent.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct BugReportPayload {
    pub what: String,
    pub steps: String,
    pub area: String,
    /// The preview lines, joined — identical to what the page showed.
    pub environment: String,
    pub fingerprint: String,
}

/// How a send ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BugReportOutcome {
    /// Filed, as a new issue or a +1 on an open one.
    Filed {
        number: u64,
        url: String,
        deduped: bool,
    },
    /// Not filed; `fallback_url` is the prefilled form, and a caller that
    /// shows anything else instead drops the person's report.
    Fallback { fallback_url: String },
}

/// This machine, coarsely and honestly: the web says `Web · <agent>`.
#[must_use]
pub fn desktop_platform() -> String {
    let os = match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "macOS",
        "linux" => "Linux",
        other => other,
    };
    format!("Desktop · {os} {}", std::env::consts::ARCH)
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The second line of defence, over every generated line: `0x` + 40 hex is
/// an address, anything with a scheme is a URL. Replaced rather than
/// dropped, so a reader can see something was removed. (The web's two
/// regular expressions, spelled out: this crate has no regex engine.)
#[must_use]
pub fn redact(line: &str) -> String {
    let addresses = replace_addresses(line);
    replace_urls(&addresses)
}

/// `/0x[0-9a-fA-F]{40}\b/g` → `[address]`.
fn replace_addresses(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    while i < chars.len() {
        let hex = |j: usize| chars.get(j).is_some_and(char::is_ascii_hexdigit);
        let is_address = chars[i] == '0'
            && chars.get(i + 1) == Some(&'x')
            && (i + 2..i + 42).all(hex)
            && !chars.get(i + 42).is_some_and(|c| is_word(*c));
        if is_address {
            out.push_str("[address]");
            i += 42;
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// `/\b[a-zA-Z][a-zA-Z0-9+.-]*:\/\/\S+/g` → `[url]`.
fn replace_urls(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let mut out = String::with_capacity(line.len());
    let mut i = 0;
    'scan: while i < chars.len() {
        let at_boundary = i == 0 || !is_word(chars[i - 1]);
        if at_boundary && chars[i].is_ascii_alphabetic() {
            let mut j = i + 1;
            while chars
                .get(j)
                .is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '.' | '-'))
            {
                j += 1;
            }
            // The scheme may end anywhere its characters allow: the regex
            // backtracks, so try the longest first and give ground.
            let mut end = j;
            while end > i {
                if chars.get(end) == Some(&':')
                    && chars.get(end + 1) == Some(&'/')
                    && chars.get(end + 2) == Some(&'/')
                    && chars.get(end + 3).is_some_and(|c| !c.is_whitespace())
                {
                    let mut k = end + 3;
                    while chars.get(k).is_some_and(|c| !c.is_whitespace()) {
                        k += 1;
                    }
                    out.push_str("[url]");
                    i = k;
                    continue 'scan;
                }
                end -= 1;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

/// The preview AND the payload's `environment`, as one list — one function,
/// so the consent line ("only what you see is sent") stays literal.
#[must_use]
pub fn environment_lines(labels: &EnvironmentLabels, facts: &DeviceFacts) -> Vec<String> {
    let list = |items: &[String], separator: &str| {
        if items.is_empty() {
            labels.none.clone()
        } else {
            items.join(separator)
        }
    };
    [
        format!("{}: v{} ({})", labels.version, facts.version, facts.commit),
        format!("{}: {}", labels.platform, facts.platform),
        format!("{}: {}", labels.language, facts.language),
        format!("{}: {}", labels.rpc, list(&facts.unreachable, ", ")),
        format!("{}: {}", labels.failures, list(&facts.failures, "; ")),
    ]
    .iter()
    .map(|line| redact(line))
    .collect()
}

/// A stable marker for "the same complaint" — FNV-1a over the UTF-16 units
/// the web hashes, so a desktop report and a web report of the same words
/// on the same build collide, which is the point.
#[must_use]
pub fn fingerprint_of(what: &str, area: &str, version: &str) -> String {
    let words: Vec<u16> = what
        .trim()
        .to_lowercase()
        .encode_utf16()
        .take(120)
        .collect();
    let seed: Vec<u16> = words
        .into_iter()
        .chain(format!("|{area}|{version}").encode_utf16())
        .collect();
    let mut hash: u32 = 0x811c_9dc5;
    for unit in seed {
        hash ^= u32::from(unit);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    format!("{hash:08x}")
}

/// The whole payload, from the allowlist and nothing else.
#[must_use]
pub fn build_bug_report(
    what: &str,
    steps: &str,
    area: &str,
    labels: &EnvironmentLabels,
    facts: &DeviceFacts,
) -> BugReportPayload {
    let what = what.trim().to_owned();
    BugReportPayload {
        fingerprint: fingerprint_of(&what, area, &facts.version),
        what,
        steps: steps.trim().to_owned(),
        area: area.to_owned(),
        environment: environment_lines(labels, facts).join("\n"),
    }
}

/// `application/x-www-form-urlencoded`, as `URLSearchParams` writes it.
fn form_encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'*' | b'-' | b'.' | b'_' => {
                out.push(char::from(byte));
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

/// The prefilled issue form — by the form's FIELD IDS: with
/// `template=bug.yml` GitHub ignores `&body=` and opens an empty form.
#[must_use]
pub fn prefilled_issue_url(payload: &BugReportPayload) -> String {
    let title: String = payload
        .what
        .encode_utf16()
        .take(80)
        .collect::<Vec<u16>>()
        .pipe_utf16();
    let mut params = vec![
        ("template", "bug.yml".to_owned()),
        ("title", format!("[bug] {title}")),
        ("what", payload.what.clone()),
        // The form marks steps required; an empty box beats a missing one.
        ("steps", payload.steps.clone()),
        ("environment", payload.environment.clone()),
    ];
    if !payload.area.is_empty() {
        params.push(("area", payload.area.clone()));
    }
    let query: Vec<String> = params
        .iter()
        .map(|(key, value)| format!("{}={}", form_encode(key), form_encode(value)))
        .collect();
    format!("{GITHUB_ISSUE_FORM}?{}", query.join("&"))
}

/// A UTF-16 prefix back to text — `slice(0, 80)` in the web's units, so
/// both shells title the same issue the same way.
trait PipeUtf16 {
    fn pipe_utf16(self) -> String;
}

impl PipeUtf16 for Vec<u16> {
    fn pipe_utf16(self) -> String {
        String::from_utf16_lossy(&self)
    }
}

/// Where reports go: the site's proxy, or — `VELA_BUG_REPORT_ENDPOINT` — a
/// stand-in, so a verification pass never files a real issue.
#[must_use]
pub fn endpoint() -> String {
    std::env::var("VELA_BUG_REPORT_ENDPOINT").unwrap_or_else(|_| BUG_REPORT_ENDPOINT.to_owned())
}

/// Send it, or hand back the road that still works. Blocking — call it off
/// the frame. Every non-2xx and every fault falls back, deliberately.
#[must_use]
pub fn send_bug_report(payload: &BugReportPayload, endpoint: &str) -> BugReportOutcome {
    let fallback = BugReportOutcome::Fallback {
        fallback_url: prefilled_issue_url(payload),
    };
    let Ok(body) = serde_json::to_string(payload) else {
        return fallback;
    };
    // Checked here as well as there: a 413 round trip costs a spinner to
    // learn what this line knows before the request leaves.
    if body.encode_utf16().count() > MAX_REPORT_CHARS {
        return fallback;
    }
    let agent = crate::executor::proxy::agent(TIMEOUT);
    let response = agent
        .post(endpoint)
        .header("Content-Type", "application/json")
        .send(body.as_bytes());
    let mut response = match response {
        Ok(response) => response,
        Err(error) => {
            eprintln!("[vela-wallet] bug report: {error} — falling back");
            return fallback;
        }
    };
    let Ok(text) = response.body_mut().read_to_string() else {
        return fallback;
    };
    let Ok(filed) = serde_json::from_str::<serde_json::Value>(&text) else {
        return fallback;
    };
    match (
        filed.get("number").and_then(serde_json::Value::as_u64),
        filed.get("url").and_then(serde_json::Value::as_str),
    ) {
        (Some(number), Some(url)) => BugReportOutcome::Filed {
            number,
            url: url.to_owned(),
            deduped: filed.get("deduped").and_then(serde_json::Value::as_bool) == Some(true),
        },
        // A 200 this client cannot read is not a filed report it can point at.
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn labels() -> EnvironmentLabels {
        EnvironmentLabels {
            version: "Version".into(),
            platform: "Platform".into(),
            language: "Language".into(),
            rpc: "Unreachable RPC".into(),
            failures: "Recent failures".into(),
            none: "None".into(),
        }
    }

    /// Addresses and URLs never leave, and the reader can see they were
    /// there.
    #[test]
    fn redact_replaces_addresses_and_urls() {
        assert_eq!(
            redact("to 0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5c now"),
            "to [address] now"
        );
        assert_eq!(
            redact("rpc https://eth.example/v2/KEY123 down"),
            "rpc [url] down"
        );
        // One hex digit too many is not an address the web would match.
        assert_eq!(
            redact("0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5cA"),
            "0x14fB1fB21751E29F7Ec48dC450017552E3D1eA5cA"
        );
        assert_eq!(redact("My net wss://node:8546/x"), "My net [url]");
        assert_eq!(redact("nothing here"), "nothing here");
    }

    /// The five lines, in the web's order, with "None" for empty lists.
    #[test]
    fn environment_lines_are_the_webs_five() {
        let facts = DeviceFacts {
            version: "1.2.3".into(),
            commit: "abc1234".into(),
            platform: "Desktop · Windows x86_64".into(),
            language: "en".into(),
            unreachable: vec!["World Chain".into(), "Zora".into()],
            failures: Vec::new(),
        };
        assert_eq!(
            environment_lines(&labels(), &facts),
            vec![
                "Version: v1.2.3 (abc1234)",
                "Platform: Desktop · Windows x86_64",
                "Language: en",
                "Unreachable RPC: World Chain, Zora",
                "Recent failures: None",
            ]
        );
    }

    /// The same words on the same build hash as the web's
    /// `fingerprintOf` does (values computed with the TS implementation).
    #[test]
    fn fingerprint_matches_the_web() {
        assert_eq!(
            fingerprint_of("  Send FAILED on Base ", AREA_OTHER, "1.0.0"),
            WEB_FP_1
        );
        assert_eq!(fingerprint_of("转账失败", AREA_OTHER, "1.0.0"), WEB_FP_2);
    }

    const WEB_FP_1: &str = "c7e1a5b2";
    const WEB_FP_2: &str = "f7bb6d3d";

    /// The fallback addresses the form's fields, never `body`.
    #[test]
    fn prefilled_url_uses_field_ids() {
        let payload = build_bug_report(
            "It broke & stayed broken",
            "1. open\n2. send",
            AREA_OTHER,
            &labels(),
            &DeviceFacts::default(),
        );
        let url = prefilled_issue_url(&payload);
        assert!(url.starts_with("https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml&title=%5Bbug%5D+It+broke+%26+stayed+broken&what="));
        assert!(url.contains("&steps=1.+open%0A2.+send&"));
        assert!(url.contains("&area=Other+%28explain+above%29"));
        assert!(!url.contains("body="));
    }
}
