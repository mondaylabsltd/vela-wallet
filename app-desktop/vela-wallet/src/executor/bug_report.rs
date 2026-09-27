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
//!
//! ## Screenshots (078 round 3)
//!
//! Up to five, as plain base64 JPEGs in `screenshots` — still ONE
//! `application/json` body, which the endpoint prefers to multipart (its
//! CSRF check refuses a cross-site multipart POST, and a native app sends no
//! Origin). They are the prepared bytes from [`super::screenshot_prep`],
//! never the picked files. Absent, not empty, when there are none; a send
//! with them gets [`SCREENSHOT_TIMEOUT`], and the reply says how many could
//! not be stored (`screenshotsDropped`).

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

/// The endpoint's `client` for this app; the issue's title tag is `[Desktop]`.
pub const CLIENT: &str = "desktop";
/// The tag the fallback form's title opens with, as the endpoint's
/// `CLIENT_TAGS.desktop` spells it.
pub const CLIENT_TAG: &str = "Desktop";

/// The web's `bundlerRest` budget: a report is small and the person waits.
const TIMEOUT: Duration = Duration::from_millis(10_000);

/// With screenshots: up to five ~300 KB images ride in the body — the web's,
/// iOS's and Android's 30 s.
pub const SCREENSHOT_TIMEOUT: Duration = Duration::from_millis(30_000);

/// Everything about the device a report is allowed to know — by name, so a
/// balance cannot arrive through an index six months from now.
#[derive(Clone, Debug, Default)]
pub struct DeviceFacts {
    pub version: String,
    pub commit: String,
    pub platform: String,
    /// The operating system and its version, one short line — "macOS
    /// 15.5", "Windows 11", "Ubuntu 24.04" ([`desktop_os`]).
    pub os: String,
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

/// The five text fields the endpoint accepts, and the screenshots.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct BugReportPayload {
    pub what: String,
    pub steps: String,
    pub area: String,
    /// The preview lines, joined — identical to what the page showed.
    pub environment: String,
    pub fingerprint: String,
    /// Which app sent it — always "desktop" here. The endpoint titles the
    /// issue "[Desktop] …" from it (078 round 3, section E).
    pub client: String,
    /// The operating system as this app reads it ("macOS 15.5").
    pub os: String,
    /// "0.9.4" — no leading v, no commit.
    #[serde(rename = "appVersion")]
    pub app_version: String,
    /// Base64 of each prepared JPEG, in tile order, at most five. ABSENT,
    /// not empty, when there are none, so a text-only report is byte for
    /// byte what it was before screenshots existed.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub screenshots: Vec<String>,
}

/// How a send ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BugReportOutcome {
    /// Filed, as a new issue or a +1 on an open one.
    Filed {
        number: u64,
        url: String,
        deduped: bool,
        /// Filed, but this many screenshots could not be stored — the
        /// person is told, and pointed at the issue page to add them.
        screenshots_dropped: u32,
    },
    /// Not filed; `fallback_url` is the prefilled form, and a caller that
    /// shows anything else instead drops the person's report.
    Fallback {
        fallback_url: String,
        /// The send carried screenshots, which a URL cannot: the fallback
        /// says to add them in the form.
        with_screenshots: bool,
    },
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

/// The whole payload, from the allowlist and nothing else. `screenshots`
/// are base64 JPEGs in tile order; past the fifth they are not sent.
#[must_use]
pub fn build_bug_report(
    what: &str,
    steps: &str,
    area: &str,
    labels: &EnvironmentLabels,
    facts: &DeviceFacts,
    mut screenshots: Vec<String>,
) -> BugReportPayload {
    let what = what.trim().to_owned();
    screenshots.truncate(super::screenshot_prep::MAX_SCREENSHOTS);
    BugReportPayload {
        fingerprint: fingerprint_of(&what, area, &facts.version),
        what,
        steps: steps.trim().to_owned(),
        area: area.to_owned(),
        environment: environment_lines(labels, facts).join("\n"),
        client: CLIENT.to_owned(),
        os: facts.os.clone(),
        app_version: facts.version.clone(),
        screenshots,
    }
}

/// How long a send may take: the REST budget for text, 30 s with images.
#[must_use]
pub fn timeout_for(payload: &BugReportPayload) -> Duration {
    if payload.screenshots.is_empty() {
        TIMEOUT
    } else {
        SCREENSHOT_TIMEOUT
    }
}

/// The report's TEXT, as the endpoint counts it against
/// [`MAX_REPORT_CHARS`]: the screenshots have caps of their own (count and
/// bytes), and five of them would otherwise be "too long" every time.
fn text_chars(payload: &BugReportPayload) -> usize {
    #[derive(serde::Serialize)]
    struct Text<'a> {
        what: &'a str,
        steps: &'a str,
        area: &'a str,
        environment: &'a str,
        fingerprint: &'a str,
        client: &'a str,
        os: &'a str,
        #[serde(rename = "appVersion")]
        app_version: &'a str,
    }
    serde_json::to_string(&Text {
        what: &payload.what,
        steps: &payload.steps,
        area: &payload.area,
        environment: &payload.environment,
        fingerprint: &payload.fingerprint,
        client: &payload.client,
        os: &payload.os,
        app_version: &payload.app_version,
    })
    .map_or(usize::MAX, |text| text.encode_utf16().count())
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
    let mut params = vec![
        ("template", "bug.yml".to_owned()),
        ("title", fallback_title(&payload.what)),
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

/// `[Desktop] <the first line of what happened, ≤ 80 characters>` — the tag
/// the endpoint's own titles carry (section E), in place of bug.yml's
/// default `[bug] `. The cut is in UTF-16 units, the web's `slice(0, 80)`.
#[must_use]
pub fn fallback_title(what: &str) -> String {
    let first = what
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or_default();
    let line = first.split_whitespace().collect::<Vec<_>>().join(" ");
    let cut = line
        .encode_utf16()
        .take(80)
        .collect::<Vec<u16>>()
        .pipe_utf16();
    format!("[{CLIENT_TAG}] {cut}")
}

/// This machine's operating system and version, one short line, as the
/// endpoint wants it for the issue's "Platform:" line: "macOS 15.5",
/// "Windows 11", "Ubuntu 24.04". Never a path, a host name or an id.
#[must_use]
pub fn desktop_os() -> String {
    os_line().unwrap_or_else(|| {
        match std::env::consts::OS {
            "macos" => "macOS",
            "windows" => "Windows",
            "linux" => "Linux",
            other => other,
        }
        .to_owned()
    })
}

#[cfg(target_os = "macos")]
fn os_line() -> Option<String> {
    let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
    let mut line = format!("macOS {}.{}", version.majorVersion, version.minorVersion);
    if version.patchVersion > 0 {
        line.push_str(&format!(".{}", version.patchVersion));
    }
    Some(line)
}

/// Windows 11 still reports itself as 10.0 — the build number is what
/// tells them apart (22000 and up is 11). Read from `ver`, which needs no
/// binding and no manifest; the console it would flash is suppressed.
#[cfg(windows)]
fn os_line() -> Option<String> {
    use std::os::windows::process::CommandExt as _;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    let output = std::process::Command::new("cmd")
        .args(["/C", "ver"])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .ok()?;
    windows_line(&String::from_utf8_lossy(&output.stdout))
}

/// "Microsoft Windows [Version 10.0.22631.4317]" → "Windows 11".
#[cfg_attr(not(any(windows, test)), allow(dead_code))]
fn windows_line(ver: &str) -> Option<String> {
    let version = ver.split("Version ").nth(1)?.trim().trim_end_matches(']');
    let mut parts = version.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let _minor = parts.next()?;
    let build: u32 = parts.next()?.parse().ok()?;
    let name = match (major, build) {
        (10, build) if build >= 22_000 => "11".to_owned(),
        (10, _) => "10".to_owned(),
        (major, _) => major.to_string(),
    };
    Some(format!("Windows {name}"))
}

/// The distribution and its version from `/etc/os-release` — "Ubuntu
/// 24.04", "Fedora Linux 40" — or its pretty name when it has no version.
#[cfg(target_os = "linux")]
fn os_line() -> Option<String> {
    let release = std::fs::read_to_string("/etc/os-release")
        .or_else(|_| std::fs::read_to_string("/usr/lib/os-release"))
        .ok()?;
    linux_line(&release)
}

#[cfg_attr(not(any(target_os = "linux", test)), allow(dead_code))]
fn linux_line(release: &str) -> Option<String> {
    let value = |key: &str| {
        release.lines().find_map(|line| {
            line.strip_prefix(key)
                .and_then(|rest| rest.strip_prefix('='))
                .map(|value| value.trim().trim_matches('"').to_owned())
                .filter(|value| !value.is_empty())
        })
    };
    let line = match (value("NAME"), value("VERSION_ID")) {
        (Some(name), Some(version)) => format!("{name} {version}"),
        _ => value("PRETTY_NAME").or_else(|| value("NAME"))?,
    };
    Some(line.chars().take(120).collect())
}

#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
fn os_line() -> Option<String> {
    None
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
/// the frame. Every non-2xx and every fault falls back, deliberately — the
/// screenshot refusals (400 `too_many_screenshots` / `invalid_screenshot`,
/// 413 `screenshot_too_large`, 415 `unsupported_screenshot`) included.
///
/// Over the proxy chain every other request walks (system, environment,
/// direct), and direct for a loopback stand-in, which is what a
/// verification pass points `VELA_BUG_REPORT_ENDPOINT` at.
#[must_use]
pub fn send_bug_report(payload: &BugReportPayload, endpoint: &str) -> BugReportOutcome {
    let fallback = BugReportOutcome::Fallback {
        fallback_url: prefilled_issue_url(payload),
        with_screenshots: !payload.screenshots.is_empty(),
    };
    // Checked here as well as there: a 413 round trip costs a spinner to
    // learn what this line knows before the request leaves.
    if text_chars(payload) > MAX_REPORT_CHARS {
        return fallback;
    }
    let Ok(body) = serde_json::to_string(payload) else {
        return fallback;
    };
    let text =
        crate::executor::proxy::with_candidates_for(endpoint, timeout_for(payload), |agent| {
            agent
                .post(endpoint)
                .header("Content-Type", "application/json")
                .send(body.as_bytes())?
                .body_mut()
                .read_to_string()
        });
    let text = match text {
        Ok(text) => text,
        Err(failure) => {
            eprintln!("[vela-wallet] bug report: {} — falling back", failure.error);
            return fallback;
        }
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
            screenshots_dropped: dropped_count(&filed),
        },
        // A 200 this client cannot read is not a filed report it can point at.
        _ => fallback,
    }
}

/// `screenshotsDropped`, as the web reads it: a positive finite number, or
/// none dropped.
fn dropped_count(filed: &serde_json::Value) -> u32 {
    filed
        .get("screenshotsDropped")
        .and_then(serde_json::Value::as_f64)
        .filter(|n| n.is_finite() && *n > 0.)
        // Bounded to MAX_SCREENSHOTS by the endpoint; clamped anyway.
        .map_or(0, |n| {
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let count = n.round().min(f64::from(u32::MAX)) as u32;
            count
        })
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
            os: "Windows 11".into(),
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
            Vec::new(),
        );
        let url = prefilled_issue_url(&payload);
        assert!(url.starts_with("https://github.com/mondaylabsltd/vela-wallet/issues/new?template=bug.yml&title=%5BDesktop%5D+It+broke+%26+stayed+broken&what="));
        assert!(url.contains("&steps=1.+open%0A2.+send&"));
        assert!(url.contains("&area=Other+%28explain+above%29"));
        assert!(!url.contains("body="));
    }

    /// The three fields the endpoint titles the issue from (section E), and
    /// the same tag on the fallback form's title.
    #[test]
    fn the_platform_rides_in_the_payload_and_the_title() {
        let facts = DeviceFacts {
            version: "0.9.4".into(),
            os: "macOS 15.5".into(),
            ..DeviceFacts::default()
        };
        let payload = build_bug_report(
            "  \n  Send froze\n  on   Base\n",
            "",
            AREA_OTHER,
            &labels(),
            &facts,
            Vec::new(),
        );
        let json: serde_json::Value = serde_json::to_value(&payload)
            .unwrap_or_else(|error| unreachable!("serialize: {error}"));
        assert_eq!(json["client"], "desktop");
        assert_eq!(json["os"], "macOS 15.5");
        assert_eq!(json["appVersion"], "0.9.4");
        assert_eq!(fallback_title(&payload.what), "[Desktop] Send froze");
        assert!(prefilled_issue_url(&payload).contains("&title=%5BDesktop%5D+Send+froze&"));
        // At most 80 characters of the first line.
        let long = "x".repeat(200);
        assert_eq!(
            fallback_title(&long),
            format!("[Desktop] {}", "x".repeat(80))
        );
        assert_eq!(fallback_title("a   b\tc"), "[Desktop] a b c");
    }

    /// The OS line is one short line naming the system, never more.
    #[test]
    fn the_os_line_is_short_and_named() {
        let os = desktop_os();
        assert!(
            !os.is_empty() && os.len() <= 120 && !os.contains('\n'),
            "{os:?}"
        );
        if cfg!(target_os = "macos") {
            assert!(
                os.starts_with("macOS 1") || os.starts_with("macOS 2"),
                "{os:?}"
            );
        }
        assert_eq!(
            windows_line("\r\nMicrosoft Windows [Version 10.0.22631.4317]\r\n").as_deref(),
            Some("Windows 11")
        );
        assert_eq!(
            windows_line("Microsoft Windows [Version 10.0.19045.3803]").as_deref(),
            Some("Windows 10")
        );
        assert_eq!(windows_line("nonsense"), None);
        assert_eq!(
            linux_line(
                "NAME=\"Ubuntu\"\nVERSION_ID=\"24.04\"\nPRETTY_NAME=\"Ubuntu 24.04.1 LTS\"\n"
            )
            .as_deref(),
            Some("Ubuntu 24.04")
        );
        assert_eq!(
            linux_line("NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\n").as_deref(),
            Some("Arch Linux")
        );
    }

    fn payload_with(screenshots: Vec<String>) -> BugReportPayload {
        build_bug_report(
            "Send froze",
            "",
            AREA_OTHER,
            &labels(),
            &DeviceFacts::default(),
            screenshots,
        )
    }

    /// Text alone is the payload it always was — no `screenshots` key at
    /// all — and keeps the REST budget.
    #[test]
    fn a_text_only_report_carries_no_screenshots_key() {
        let payload = payload_with(Vec::new());
        let json: serde_json::Value = serde_json::to_value(&payload)
            .unwrap_or_else(|error| unreachable!("serialize: {error}"));
        let keys: Vec<&str> = json
            .as_object()
            .map(|map| map.keys().map(String::as_str).collect())
            .unwrap_or_default();
        assert_eq!(
            keys,
            vec![
                "what",
                "steps",
                "area",
                "environment",
                "fingerprint",
                "client",
                "os",
                "appVersion"
            ]
        );
        assert_eq!(timeout_for(&payload), TIMEOUT);
    }

    /// With images: plain base64 strings, in tile order, at most five, and
    /// the 30 s budget. The cap on text does not count them.
    #[test]
    fn screenshots_ride_in_tile_order_and_stop_at_five() {
        let shots: Vec<String> = (1..=7).map(|n| format!("shot{n}")).collect();
        let payload = payload_with(shots);
        assert_eq!(
            payload.screenshots,
            vec!["shot1", "shot2", "shot3", "shot4", "shot5"]
        );
        let json: serde_json::Value = serde_json::to_value(&payload)
            .unwrap_or_else(|error| unreachable!("serialize: {error}"));
        assert_eq!(json["screenshots"][0], "shot1");
        assert_eq!(json["screenshots"][4], "shot5");
        assert_eq!(timeout_for(&payload), SCREENSHOT_TIMEOUT);

        let big = payload_with(vec!["A".repeat(1_000_000)]);
        assert!(text_chars(&big) < MAX_REPORT_CHARS);
    }

    /// A stand-in endpoint on loopback: answers every request with `status`
    /// and `reply`, and hands the request body it received back to the test.
    /// Never the real endpoint.
    fn stub(status: u16, reply: &'static str) -> (String, std::sync::mpsc::Receiver<String>) {
        use std::io::{BufRead as _, Read as _, Write as _};

        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap_or_else(|error| unreachable!("no loopback port: {error}"));
        let port = listener
            .local_addr()
            .map(|addr| addr.port())
            .unwrap_or_else(|error| unreachable!("no port: {error}"));
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let mut reader = std::io::BufReader::new(
                    stream
                        .try_clone()
                        .unwrap_or_else(|error| unreachable!("clone: {error}")),
                );
                let mut length = 0usize;
                let mut line = String::new();
                while reader.read_line(&mut line).unwrap_or(0) > 0 {
                    if line == "\r\n" {
                        break;
                    }
                    if let Some(value) = line.to_lowercase().strip_prefix("content-length:") {
                        length = value.trim().parse().unwrap_or(0);
                    }
                    line.clear();
                }
                let mut body = vec![0u8; length];
                let _ = reader.read_exact(&mut body);
                let _ = tx.send(String::from_utf8_lossy(&body).into_owned());
                let reason = if status == 200 { "OK" } else { "Refused" };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{reply}",
                    reply.len()
                );
            }
        });
        (format!("http://127.0.0.1:{port}/api/bug-report"), rx)
    }

    /// Filed with images the endpoint could not store: the count comes back,
    /// so the page can say so. The body it received is the payload, with the
    /// screenshots in tile order.
    #[test]
    fn a_filed_report_says_how_many_screenshots_were_dropped() {
        let (endpoint, received) = stub(
            200,
            r#"{"number":4242,"url":"https://github.com/mondaylabsltd/vela-wallet/issues/4242","deduped":false,"screenshots":1,"screenshotsDropped":2}"#,
        );
        let payload = payload_with(vec![
            "/9j/AA==".into(),
            "/9j/AQ==".into(),
            "/9j/Ag==".into(),
        ]);
        let outcome = send_bug_report(&payload, &endpoint);
        assert_eq!(
            outcome,
            BugReportOutcome::Filed {
                number: 4242,
                url: "https://github.com/mondaylabsltd/vela-wallet/issues/4242".into(),
                deduped: false,
                screenshots_dropped: 2,
            }
        );
        let body: serde_json::Value = received
            .recv_timeout(std::time::Duration::from_secs(5))
            .ok()
            .and_then(|body| serde_json::from_str(&body).ok())
            .unwrap_or_else(|| unreachable!("the stub received no JSON"));
        assert_eq!(
            body["screenshots"],
            serde_json::json!(["/9j/AA==", "/9j/AQ==", "/9j/Ag=="])
        );
        assert_eq!(body["what"], "Send froze");
    }

    /// A reply from before screenshots existed reads as none dropped.
    #[test]
    fn an_old_reply_drops_nothing() {
        let (endpoint, received) = stub(
            200,
            r#"{"number":7,"url":"https://x.test/7","deduped":true}"#,
        );
        let outcome = send_bug_report(&payload_with(Vec::new()), &endpoint);
        assert_eq!(
            outcome,
            BugReportOutcome::Filed {
                number: 7,
                url: "https://x.test/7".into(),
                deduped: true,
                screenshots_dropped: 0,
            }
        );
        let body = received
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap_or_default();
        assert!(!body.contains("screenshots"), "{body}");
    }

    /// A screenshot refusal is a refusal like any other: the form, which
    /// says the images cannot follow — and only when there were images.
    #[test]
    fn a_refused_report_falls_back_and_remembers_its_screenshots() {
        for (status, reply) in [
            (415, r#"{"error":"unsupported_screenshot"}"#),
            (413, r#"{"error":"screenshot_too_large"}"#),
            (503, r#"{"error":"not_configured"}"#),
        ] {
            let (endpoint, _received) = stub(status, reply);
            match send_bug_report(&payload_with(vec!["/9j/AA==".into()]), &endpoint) {
                BugReportOutcome::Fallback {
                    fallback_url,
                    with_screenshots,
                } => {
                    assert!(with_screenshots, "{status}");
                    assert!(fallback_url.starts_with(GITHUB_ISSUE_FORM));
                }
                other => unreachable!("{status} filed: {other:?}"),
            }
        }
        let (endpoint, _received) = stub(503, r#"{"error":"not_configured"}"#);
        assert!(matches!(
            send_bug_report(&payload_with(Vec::new()), &endpoint),
            BugReportOutcome::Fallback {
                with_screenshots: false,
                ..
            }
        ));
    }

    #[test]
    fn dropped_counts_read_as_the_web_reads_them() {
        let read = |json: &str| dropped_count(&serde_json::from_str(json).unwrap_or_default());
        assert_eq!(read(r#"{"screenshotsDropped":3}"#), 3);
        assert_eq!(read(r#"{"screenshotsDropped":0}"#), 0);
        assert_eq!(read(r#"{"screenshotsDropped":-1}"#), 0);
        assert_eq!(read(r#"{"screenshotsDropped":"2"}"#), 0);
        assert_eq!(read("{}"), 0);
    }
}
