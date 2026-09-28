# Contract — the rules 079 puts in vela-core

Every client calls these; none re-implements them. Rust names first, then the exported names
(UniFFI for Kotlin/Swift, wasm for web, the crate directly on desktop).

## 1. `app::browser_load` (new, pure)

```rust
pub enum LoadPlatform { Android, Apple, Probe }

pub enum LoadFailureClass { Offline, Timeout, NotFound, Refused, Certificate, Other }

pub struct LoadFailure {
    pub class: LoadFailureClass,
    /// Corpus key of the panel's reason line, e.g. "explore.loadFailedOffline".
    pub reason_key: &'static str,
    /// false for NotFound and Certificate.
    pub auto_retry: bool,
}

/// `None` = not a failure (a cancelled navigation, a frame interrupted by a new load).
pub fn classify(platform: LoadPlatform, code: i64, domain: Option<&str>, certificate: bool)
    -> Option<LoadFailure>;

/// Delay before automatic attempt `attempt` (1-based); `None` = stop.
/// Offline/Timeout/Refused: 2000, 5000, 10000. Other: 3000 once. NotFound/Certificate: none.
pub fn retry_delay_ms(class: LoadFailureClass, attempt: u32) -> Option<u32>;

pub struct LoadFinished<'a> {
    pub url: &'a str,          // from the page's own script, not the view's current URL
    pub title: &'a str,
    pub icon: Option<&'a str>,
    pub main_frame_failed: bool,
    pub http_status: Option<u16>,
}
pub struct Visit { pub url: String, pub title: Option<String>, pub favicon: Option<String> }

/// The visit to record, or nothing: failed loads, error statuses (>= 400), non-http(s)
/// URLs and engine error pages are never visits.
pub fn visit_to_record(load: LoadFinished) -> Option<Visit>;

/// The avatar letter for a host: the first ASCII alphanumeric after dropping a leading
/// `www.`, `app.` or `m.` (`app.uniswap.org` → "U"); "?" when there is none.
pub fn site_letter(host: &str) -> String;
```

Exported: `browserLoadClassify(platform, code, domain, certificate) -> LoadFailure?`,
`browserLoadRetryDelayMs(class, attempt) -> UInt?`, `browserLoadVisit(...) -> Visit?`,
`browserSiteLetter(host) -> String`.

Tests (`tests/app_browser_load.rs`): every row of research R1's table for both platforms; the
cancelled cases return `None`; the retry schedule for each class; visit rules (failed, 404, 500,
`about:blank`, `chrome-error://`, `data:`, a good page); letters (`app.uniswap.org`,
`www.example.com`, `m.x.io`, `127.0.0.1`, `xn--…`, empty).

## 2. `app::tx_tracker` (changed)

- Receipt cadence past the wait window grows with the record's age:
  `RECONCILE_MIN_INTERVAL_MS` until 10 min, 60 000 ms until 1 h, 300 000 ms until
  `ABANDON_AGE_MS` (24 h, unchanged).
- `TrackEntryView` gains `outcome: TrackOutcome` = `Landing` (in window) | `StillConfirming`
  (`AcceptedNotLanded`, window closed, still polling) | `Unknown` (abandoned) | `Final` (terminal).
- Unchanged invariant: time alone never produces `Dropped`/`Rejected`.

Tests: cadence at 5 min / 30 min / 3 h; `outcome` for each state; no terminal status from age.

## 3. `app::fee_policy` (changed)

```rust
/// Delay before re-quote `attempt` (1-based) after a failure the service may recover from
/// (`QuoteUnavailable`, `FeeTokenUnavailable`): 3000, 6000, 12000, then 15000 for every later one.
/// `None` for failures a retry cannot fix (`MissingPublicKey`, `CalculationFailed`, estimate refusal).
pub fn requote_delay_ms(failure: &FeeFailure, attempt: u32) -> Option<u32>;
```

Exported: `feeRequoteDelayMs(failureJson, attempt) -> UInt?`.

## 4. Corpus keys (new, all 15 locales)

| Key | zh (source) |
|---|---|
| `explore.loadFailedOffline` | 网络连接不稳定，页面没能打开。 |
| `explore.loadFailedTimeout` | 网站响应太慢，页面没能打开。 |
| `explore.loadFailedNotFound` | 找不到这个网站，请检查网址。 |
| `explore.loadFailedRefused` | 网站拒绝了连接，稍后再试。 |
| `explore.loadFailedCertificate` | 这个网站的安全证书有问题，Vela 已阻止打开。 |
| `explore.loadFailedOther` | 页面没能打开。 |
| `explore.loadRetrying` | 正在重试… |
| `explore.chainUnreachable` | {{chain}} 网络暂时连接不上，页面数据可能不完整。 |
| `componentsUi.signing.signed` | 已签名 |
| `componentsUi.signing.submitting` | 正在提交… |
| `componentsUi.signing.stillConfirming` | 已提交，但还没上链。Vela 会继续查看，请不要重复发送。可以先关闭。 |
| `componentsUi.signing.unknownOutcome` | 超过 24 小时仍未确认，请到区块浏览器查看。 |
| `componentsUi.signing.continueToSigner` | 去签名页确认 |
| `componentsUi.signing.signerUnreachable` | 签名页没能打开，请检查网络后重试。 |
| `explore.httpsA11y` | HTTPS |

Reused, not new: `connect.browser.title`, `connect.browser.retry`, `connect.browser.a11yInsecure`
(screen readers, http), `componentsUi.signing.submitted`, `componentsTx.receipt.statusConfirmed`,
`send.feeRefresh`, `componentsUi.funding.denialNetworkError`, `explore.close`.

Removed from visible UI (kept in the corpus until every client stops reading it, then deleted in the
same change that removes the last reader): `explore.secureSite`, `connect.browser.a11ySecure`.

## 5. What each client promises to do with them

- Page loads: `loading` from the request; the panel with `reason_key` + host + Retry; `retrying`
  during an attempt; timers only for the tab in front; `visit_to_record` is the only path to
  `VisitRecorded`.
- Chain notice: the page-in-front's chain ∈ `RpcPoolView.failed_chains` and ∉ `rate_limited_chains`.
- Signing: explicit-only close; the status body from `SignView` + `TrackEntryView.outcome`.
- Fee: the send flow's refresh control; `requote_delay_ms` timers while open and unapproved.
- Lock: `DbrTabView.secure` → closed lock (neutral) or open lock (warning); no visible text.
