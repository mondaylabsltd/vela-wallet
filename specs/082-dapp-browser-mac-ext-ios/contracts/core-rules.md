# Contract — what every client depends on after 082

Rust names first, then the exported names (UniFFI for Swift/Kotlin, wasm for the web and the
extension's pages; the desktop uses the crate). Every item says which research decision it comes
from. "Unchanged" items are listed where a client might otherwise re-derive them.

All serde additions are `#[serde(default)]` and additive; new enum variants are **not** additive for
the hand-written Swift and Kotlin wires (`SignWire`, `TrackerWire`, `RpcWire`, `ClearWire`,
`FeedWire`, `ActivityWire`): every client ships in the same change, each with a wire round-trip test.

## 1. `vela_core::user_op` — [RA1, RA6]

```rust
/// EntryPoint v0.7 getUserOpHash; 0x-lowercase; the signature is excluded.
pub fn user_op_hash(op: &UserOperation, chain_id: u64) -> Result<String, CoreError>;

pub const SUBMIT_MAX_RETRIES: u32 = 3;
pub const SUBMIT_RETRY_DELAY_MS: u32 = 3_000;

pub enum SubmitReply { Hash(String), Error(String /* JSON-RPC `error` member as JSON */), NoAnswer }
pub enum SubmitVerdict {
    Accepted { user_op_hash: String },
    MaybeSent { user_op_hash: String },           // the local hash
    NotSent { rejection: Option<RelayRejection> }, // None = relay unreachable, nothing left the device
}
pub enum SubmitStep { Done(SubmitVerdict), RetryAfter { delay_ms: u32 } }

pub fn submit_step(reply: &SubmitReply, attempt: u32, maybe_delivered: bool,
                   local_user_op_hash: &str) -> SubmitStep;

/// The dApp's -32603 detail for NotSent{None}: a fixed sentence, never the pool's text [RA10, T012].
pub const NOT_SENT_DAPP_DETAIL: &str = "relay unreachable; nothing was sent";
/// keccak of `UserOperationEvent(bytes32,address,address,uint256,bool,uint256,uint256)`,
/// the topic §3's find-event filters on [T010].
pub const USER_OPERATION_EVENT_TOPIC: &str;
```

Clients loop: POST → OR `maybe_delivered` from the pool verdict → `submit_step` → retry or done.
Local nonce advances only on `Accepted`. The relay's hash wins; a mismatch logs `userop.hash_mismatch`.

## 2. `app::rpc_pool` — [RA1, RG7, RF1, ruling 8]

```rust
RpcTransportOutcome::NotConnected           // new: nothing left the device; routed like Network
pub fn may_have_delivered(o: &RpcTransportOutcome) -> bool; // Timeout | Network | NonJson | HTTP 500..=599
RpcCallVerdict::Respond { url, #[serde(default)] maybe_delivered: bool }
RpcCallVerdict::Failed  { rate_limited, #[serde(default)] maybe_delivered: bool } // OR over every POST

pub const OPTIONAL_METHODS: &[&str] = &["eth_simulateV1"];
pub fn is_optional_method(method: &str) -> bool;
// optional method + JSON error: rate-limit signal → fail over; else Route::NotServed →
// Respond{url}, clear_chain_failure, no score change, no ban; conclude_failed never classifies the chain.

pub fn is_log_range_error(error: &RpcErrorInfo) -> bool; // the message names a block range or a result cap
// eth_getLogs + JSON error, checked before the ban and fail-over rules [T180]: a rate-limit signal
// (429, "rate limit exceeded") → fail over; is_log_range_error → Respond{url} carrying the error —
// no ban, no score change, no chain classification. Other methods keep today's routing.

RpcPoolView { .., pub unreached_chains: Vec<u32> }  // first pass: every endpoint failed on transport, no rate limit
pub const RPC_READ_TIMEOUT_MS: u32 = 8_000;          // unchanged; now also pinned by the extension (RF2)
pub fn cooldown_ms(consecutive_failures: u64) -> f64; // 30 s · 2^(n−1), cap 300 s (exposed, rule unchanged)
```

Chain notice (every client with browser chrome): `chain ∈ failed_chains ∪ unreached_chains ∧
chain ∉ rate_limited_chains`. The home RPC banner keeps `failed_chains` only.

Why the range rule: §3's find-event halves its window on a range error, so the error must reach the
tracker. Before T180 a message with "exceeded" matched `is_permanent_rpc_error` and banned the
endpoint for every method, and `-32005` failed over until the chain landed in `failed_chains` — a
false chain notice while an op may have been sent.

Shell mapping to `NotConnected` — desktop ureq: `HostNotFound`, `ConnectionFailed`,
`Timeout(Resolve|Connect)`, Io `ConnectionRefused|AddrNotAvailable|HostUnreachable|NetworkUnreachable`,
TLS handshake, proxy CONNECT failure (exact ureq 3.4 variant to be pinned by a test); iOS `URLError`
`.cannotFindHost, .cannotConnectToHost, .dnsLookupFailed, .notConnectedToInternet,
.secureConnectionFailed, .serverCertificate*, .clientCertificate*,
.appTransportSecurityRequiresSecureConnection, .internationalRoamingOff, .dataNotAllowed,
.callIsActive`; Android `UnknownHostException, ConnectException, NoRouteToHostException,
SSLHandshakeException`, `SocketTimeoutException("connect timed out")`; web only when
`navigator.onLine === false` before the call.

## 3. `app::tx_tracker` — [RA4, RA7, RE8, ruling 8]

```rust
pub const USER_OP_STATUS_METHOD: &str = "pimlico_getUserOperationStatus";
pub struct TrackStatusAnswer { pub status: TrackLifecycle, pub stage: Option<String>, pub tx_hash: Option<String> }
pub fn parse_user_op_status(result_json: &str) -> Option<TrackStatusAnswer>; // unknown status → None

pub const NOT_FOUND_GRACE_MS: f64 = 60_000.0;
pub const NOT_FOUND_CONFIRMATIONS: u32 = 2;

Event::Submitted { user_op_hash, record_ids, chain_id, #[serde(default)] maybe_sent: bool,
                   #[serde(default)] submit_block: Option<u64> }  // head read once before the first POST; None = unknown
TrackPendingRecord { .., #[serde(default)] maybe_sent: bool, #[serde(default)] submit_block: Option<u64> }
TrackShellResult::Status { .., #[serde(default)] tx_hash: Option<String> }
TrackStatus::NotSent                  // new terminal; records patched `failed`
TrackOutcome::MaybeSent               // new; maybe_sent ∧ ¬acknowledged ∧ ¬terminal ∧ ¬abandoned
TrackOperation::HoldingsMoved { chain_id: u32 }   // new; answered TrackShellResult::Notified

// The relay-independent landing check [ruling 8, T019]
pub const FIND_OP_MAX_RANGE: u64;        // the widest eth_getLogs window, in blocks (value set in T019)
pub const FIND_OP_LOOKBACK_BLOCKS: u64;  // the scan's start below the head when submit_block is unknown (T019)
TrackOperation::FindOpEvent { chain_id: u32, entry_point: String, user_op_hash: String,
                              from_block: Option<u64>, to_block: Option<u64> } // from_block None: the head only
TrackShellResult::OpEvent { user_op_hash, now_ms, logs_json: Option<String>, error_json: Option<String>,
                            head_block: Option<u64> }  // the pool's answer as it came
```

**Find-event.** For an entry with `maybe_sent ∧ ¬acknowledged ∧ ¬terminal ∧ ¬abandoned` the core
emits `FindOpEvent` on the status-poll cadence. The shell runs `eth_getLogs{address: entry_point,
topics: [USER_OPERATION_EVENT_TOPIC, user_op_hash], fromBlock, toBlock}` and `eth_blockNumber`
through the pool and answers `OpEvent` with the result (`logs_json`), or the JSON error the pool now
returns for a range limit (`error_json`, §2), plus the head; neither means no answer. The shell
judges nothing. The core:

- steps bounded windows (≤ `FIND_OP_MAX_RANGE`) forward from `submit_block`, clipped to the head;
  caught up with the head → waits for the next tick;
- with `submit_block` unknown, asks for the head only first, then scans from
  `head − FIND_OP_LOOKBACK_BLOCKS`;
- on a range error (`rpc_pool::is_log_range_error`, never a shell's call, FR-020) halves the window,
  down to one block; on any other error or no answer retries the same window next tick;
- parses a found log: `success` → `Confirmed` with the log's `transactionHash` (records confirmed,
  then `NotifyConfirmed` and `HoldingsMoved`); failure → `Dropped` (reverted) with that tx hash
  (records failed, then `HoldingsMoved`). A found event wins over any later relay `not_found`.

`HoldingsMoved` follows `UpdateTxRecords` + `NotifyConfirmed` on a confirmed receipt (or found
event), and follows the fail patch alone on a failed receipt (or found event) with a tx hash; never
on pending, unreachable, age or `NotSent`. Invariant kept: time alone never produces a failure.

## 4. `app::sign_request` — [RA2, RA3, RA8, RA9, RA12, RB2]

```rust
Event::OpSubmitted { id, user_op_hash, now_ms, #[serde(default)] maybe_sent: bool,
                     #[serde(default)] submit_block: Option<u64> }
Event::CeremonyStarted { id: String }   // passkey / Trusted Signer prompt opened
Event::CeremonyDone { id: String }      // it returned a signature
Event::TransportDropped { transport_id } // CHANGED behaviour: also stops an inflight of that
                                         // transport in Precheck | Sponsoring | ReactiveSponsoring
SignSubmitOutcome::AskerGone             // new, serde {"type":"asker_gone"}: nothing sent, no answer, no record

SignTrackerHandoff { .., #[serde(default)] maybe_sent: bool, #[serde(default)] submit_block: Option<u64> }
SignRecord { .., #[serde(default)] maybe_sent: bool, #[serde(default)] submit_block: Option<u64> }
pub enum SignPhase { Idle, Preparing, AwaitingSignature, Submitting }
SignView { .., pub phase: SignPhase, pub pending_op_maybe_sent: bool } // is_signing / is_submitting kept until every shell reads phase

pub enum SignEnding { Signed, Landed { tx_hash: String, user_op_hash: Option<String> },
                      StillConfirming { user_op_hash: String } }
pub fn ending_of(method: &str, payload: &SignResponsePayload, submitted_user_op: Option<&str>) -> Option<SignEnding>;
pub enum SignEndingState { Signed, Confirmed { tx_hash: String }, Reverted { tx_hash: String }, NotSent,
                           Following { user_op_hash: String, outcome: TrackOutcome, fee_held: bool } }
pub fn ending_state(ending: &SignEnding, track: Option<&TrackEntryView>) -> SignEndingState;

pub const DAPP_TX_ANSWER_WINDOW_MS: f64 = 120_000.0;   // from ApproveTapped
pub fn dapp_receipt_wait_ms(elapsed_ms: f64) -> f64;   // max(10_000, DAPP_TX_ANSWER_WINDOW_MS − elapsed_ms) [RA12, T024]
pub const EXTENSION_REQUEST_TTL_MS: f64 = 300_000.0;   // unchanged; now exported to the extension
```

Changed: `on_submit` Succeeded with a record answers the page only — no `UpdateRecord Confirmed`;
the tracker closes on-chain records. A reverted-in-window op is answered its tx hash on all four
clients (ruling 9; web: `UserOpRevertedError{txHash}`).

## 5. `app::send` — [RA4, RA10]

```rust
SendShellResult::Submitted { user_op_hash, now_ms, #[serde(default)] maybe_sent: bool,
                             #[serde(default)] submit_block: Option<u64> }
SendOperation::TrackSubmitted { .., maybe_sent: bool, submit_block: Option<u64> }
SendTxRecord { .., #[serde(default)] maybe_sent: bool }
SendReceiptStatus::{ MaybeSent, NotSent }           // new
SendReceiptOutcome::Failed { rejected, #[serde(default)] not_sent: bool }
```

`MaybeSent` never fires the success haptic. `TrackStatus::NotSent` maps to
`Failed{rejected: false, not_sent: true}` (never `fee_rejected`).

## 6. `app::clear_signing` — [RC1–RC5]

```rust
pub enum ClearSurface { None, Loading, ClearSign, EthSign, MessageSign, BlindTypedData, BlindTransaction, PlainSend }
pub struct ClearPlainSend { pub to: String /* EIP-55 */, pub value_wei: String, pub amount: String, pub no_value: bool }
ClearSigningView { .., pub plain_send: Option<ClearPlainSend> }   // Some iff surface == PlainSend
pub fn is_empty_calldata(data: Option<&str>) -> bool;              // None | "" | "0x" | "0X" (trimmed)
pub fn plain_send_of(to: Option<&str>, value: Option<&str>, locale: &ClearLocale) -> Option<ClearPlainSend>;
confirm_of: TxPlain with Some(p) ∧ ¬p.no_value → ConfirmIntent{"send", IntentSend}; else Confirm
```

Unchanged: `Event::ResolveTransaction` wire (no `check-event-payloads` change), `ClearSignResult`,
the descriptor ladder. Shells pass a present non-string `value` as text so the core refuses it.

## 7. `app::activity_feed` — [RG1–RG5]

```rust
FeedTxRecord { .., #[serde(default)] pub dapp_origin: Option<String> }
FeedItem { .., pub kind: FeedTxKind /* Send|Receive|DappTx */, pub status: FeedTxStatus, pub site: Option<String> }
FeedView { .., pub history_empty_key: String, pub home_empty_key: String }
```

`accept()` keeps DappTx with `from == me`; message signatures and connects never become rows.
After `PersistRecord`/`UpdateRecord` every shell dispatches `ReconcileCompleted{resolved_count: 1}`.

## 8. `app::sim_outcome` (new, pure) — [RG6, RG8]

```rust
pub enum SimReply { Result(serde_json::Value), Error { code: Option<i64>, message: Option<String> }, Unreachable }
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SimOutcome { Deltas { deltas: Vec<TrustAssetDelta> }, Reverts { reason: Option<String> }, NotOffered, Unreachable }
pub struct SimNotice { pub risk: ClearRisk, pub key: &'static str, pub reason: Option<String> }
pub fn classify(reply: SimReply, user: &str) -> SimOutcome;
pub fn notice(outcome: &SimOutcome) -> Option<SimNotice>;
pub fn revert_reason(call: &serde_json::Value) -> Option<String>;   // Error(string) only, sanitised, ≤ 64 chars
pub fn derive_deltas(logs: &[&serde_json::Value], user: &str) -> Vec<TrustAssetDelta>; // moved from desktop sim.rs
```

UniFFI `simOutcome(user, replyJson) -> SimOutcomeRecord{kind, deltasJson, revertReason?, noticeRisk?,
noticeKey?}`, where `replyJson` = `{"result": …}` | `{"error": {"code","message"}}` |
`{"unreachable": true}`. No wasm export (the web sheet runs no simulation).

## 9. `app::dapp_permissions` / `app::dapp_browser` and the provider — [RG10]

```rust
pub fn dapp_spelling(address: &str) -> String;   // EIP-55; input unchanged if unparseable
```

Applied in `popup_approved`, `popup_account_switch`, `dapp_browser` `AccountSwitched`,
`sites_listed`. `resolve_granted` is unchanged. `provider/inpage.js` `applyAccounts` keeps the
wallet's spelling; change detection is case-insensitive (embedded via `dapp_rpc::PROVIDER_JS`, so
every client rebuilds).

## 10. `app::browser_load` — [RD3, RD4, RD7, RD9, RE1, RE2, RE3, RE4, RE7]

```rust
pub const WATCHDOG_MS: u32 = 3_000;          // desktop probe start
pub const PROBE_BUDGET_MS: u32 = 5_000;      // desktop probe budget
pub const GIVE_UP_MS: u32 = 20_000;          // every client
pub const ENGINE_LIVE_PROGRESS: f64 = 0.15;
pub fn should_give_up(elapsed_ms: u32, committed: bool, progress: f64) -> bool;
pub fn stalled() -> LoadFailure;             // class Timeout, "explore.loadOffline", auto_retry
pub fn retry_when_network_returns(class: LoadFailureClass) -> bool; // Offline|Timeout|Refused|Other|Proxy

LoadFailureClass::Proxy                      // new; reason "explore.loadProxy"; Offline retry schedule
pub mod probe_code { pub const PROXY: i64 = 6; }
// classify: Probe 6 → Proxy; Apple "kCFErrorDomainCFNetwork" 306..=310 → Proxy (311, an
//           unexpected CONNECT answer, → Refused); Android -5 → Proxy;
//           Apple NSURLErrorDomain -1000 → Offline (was NotFound)
// Proxy = the proxy could not be reached or its PAC could not run; a proxy that answered
//         (502, or a CONNECT closed with no reply) speaks for the host → the host's class.

pub enum BarLock { Closed, Open, None }
pub struct AddressBar { pub url: String, pub host: String, pub lock: BarLock }
pub fn address_bar(shown_url: Option<&str>, pending_url: Option<&str>, failed_url: Option<&str>) -> AddressBar;
pub struct SiteLabel { pub name: String, pub host_line: Option<String> }
pub fn site_label(title: &str, host: &str) -> SiteLabel;

// moved from app-desktop explore/load_watch.rs (desktop uses directly; not exported)
pub enum Asked { Navigation, Retry, AutoRetry, Page }
pub struct EngineSample { pub loading: bool, pub progress: f64, pub url: Option<String> }
pub enum EngineVerdict { Nothing, PageStarted { generation: u64, url: String },
                         StoppedWithoutCommit { generation: u64, url: String } }
pub enum Probed { Ignored, WaitUntil(f64), Failed, Deferred }
pub enum RetryAction { Load(String), EngineStillLoading, NotInFront, Nothing }
pub struct LoadWatch { /* 079 fields */ pub engine_loading: bool, pub engine_live: bool, pub page_initiated: bool }
impl LoadWatch { requested, committed, finished, engine, watchdog, probed, give_up,
                 schedule_retry /* None when page_initiated */, retry_fired, take_due, retry, busy }
pub fn host_of(url: &str) -> String;
```

## 11. `app::net_health`, `app::remote_mark`, `app::balance_dashboard` — [RE3, RE9, RE10]

```rust
pub const MISSES_BEFORE_OFFLINE: u32 = 3;
pub struct NetHealth { pub misses: u32, pub online: bool }
pub enum NetEdge { WentOffline, CameBack }
pub fn net_health_step(state: NetHealth, reached: bool) -> (NetHealth, Option<NetEdge>);

pub enum MarkMiss { NotFound, Refused, NotAnImage, Throttled, ServerError, Transport, Unknown }
pub fn mark_miss_of_status(status: u16) -> MarkMiss;
pub fn mark_miss_ttl_ms(miss: MarkMiss) -> Option<u32>;   // None = session; Some(60_000) transient

pub enum ReadKind { Native, Stable, Wrapped, Custom }
pub struct ReadSlot { pub kind: ReadKind, pub contract: Option<String>, pub symbol: String,
                      pub known_decimals: Option<u8>, pub peg_usd: Option<f64> }
pub fn read_plan(chain_id: u32, stables: &[StableRef], wrapped_native: Option<&str>, custom: &[TokenRef]) -> Vec<ReadSlot>;
```

## 12. Exports

| Surface | New or changed exports |
|---|---|
| UniFFI (`vela-core-uniffi/src/lib.rs`) | `userOpHash(draft, chainId)`, `userOpSubmitStep(reply, attempt, maybeDelivered, localHash)`, `userOpNotSentDetail()`, `userOpStatusMethod()`, `parseUserOpStatus(json)`, `signEndingOf(method, payloadJson, submittedUserOp?)`, `signEndingState(endingJson, trackEntryJson?)`, `dappReceiptWaitMs(elapsedMs)`, `simOutcome(user, replyJson)`, `browserLoadGiveUpMs()`, `browserLoadShouldGiveUp(elapsedMs, committed, progress)`, `browserLoadStalled()`, `browserLoadRetryWhenNetworkReturns(class)`, `browserAddressBar(shown?, pending?, failed?)`, `browserSiteLabel(title, host)`, `netHealthStep(misses, online, reached)`, `markMissTtlMs(kind, status?)`, `balanceReadPlan(chainId, stablesJson, wrappedNative?, customJson)`; `browserLoadClassify` gains class `"proxy"` |
| wasm (`vela-core-wasm/src/lib.rs`) | `userOpHash(opJson, chainId)`, `userOpSubmitStep(replyJson, attempt, maybeDelivered, localHash)`, `userOpNotSentDetail()`, `userOpStatusMethod()`, `parseUserOpStatus(json)`, `signEndingState(endingJson, entryJson)`, `dappReceiptWaitMs(elapsedMs)`, `signRequestTtlMs()`, `rpcReadTimeoutMs()`, `rpcCooldownMs(n)`, `browserSiteLabel(title, host)`, `markMissTtlMs(kind, status?)`, `balanceReadPlan(...)` |
| ts-rs (`app-web/vela-wallet/src/lib/core/generated/`) | regenerate: `SignSubmitOutcome`, `SignView`, `TrackOperation`, `TrackEntryView`, `ClearSurface`, `ClearSigningView`, `ClearPlainSend` (new), `FeedItem`, `FeedTxRecord`, `FeedView`, `RpcPoolView`, `TrackOutcome`, `TrackStatus`, `SendReceiptStatus`, `SendReceiptOutcome`, `SendReceiptView` |
| Rebuilds | `rust/pkg-web` (fingerprint moves) → `extension/dist`; VelaCoreKit xcframework (`check-ios-core-fresh.sh` must print ok); Android `.so` + Kotlin bindings |

## 13. Pinned in JavaScript (the worker cannot load the core)

| JS constant | Pinned to | Test |
|---|---|---|
| `REQUEST_TTL_MS` (`extension/lib/protocol.js`) | `signRequestTtlMs()` | `instant.test.ts` |
| every `SETTLE.*.code` | `popupCloseSettlement().code` (4900), never 4001 | `instant.test.ts` |
| `READ_TIMEOUT_MS` (worker reads) | `rpcReadTimeoutMs()` | `protocol.test.ts` |
| worker endpoint cooldown | `rpcCooldownMs(n)` for n = 1..5 | `protocol.test.ts` |
| web `USER_OP_STATUS_METHOD` | `userOpStatusMethod()` | `rpc-pool-executor.test.ts` |
| port names, `REQUEST_PREFIX`, `SW_COUNTS_KEY` | `protocol.js` / `swlog.js` | `one-surface.test.ts` |

## 14. Extension messages: page ↔ worker ↔ panel — [RB1–RB11, RF3]

**Ports**: `DOC_PORT = "vela.doc"` (content.js ↔ worker, open only while the page owes a sign/connect
answer); `SURFACE_PORT = "vela.surface"` (panel or request window ↔ worker). Constants:
`REQUEST_TTL_MS = 300000`, `CONTENT_GRACE_MS = 5000`, `CLAIM_TIMEOUT_MS = 5000`.

| From → to | Message | Reply / effect |
|---|---|---|
| page → worker (`runtime.sendMessage`, carries the click) | `{type:'rpc', id, method, params, sentAt}` | sign/connect: `{accepted:true}` at once (duplicate id from the same document → `{accepted:true}`); reads: the answer as today |
| page → worker | `{type:'abandon', id}` (content.js deadline passed) | settle `expired`, push `withdrawn` |
| worker → page (`tabs.sendMessage(tabId, msg, {documentId})`) | `{type:'answer', id, result \| error}` | content.js settles the page and replies `{ok:true}` synchronously; a rejection = page gone |
| worker → page | `{type:'claimed', id}` | content.js extends that id's deadline to 5 min from now |
| worker → page | `{type:'alive', ids}` | `{alive: ids[]}` — the ids the document still owns |
| worker → page | `{type:'evt', …}` | as today (accountsChanged, chainChanged) |
| surface → worker | `hello {kind:'panel'\|'window', windowId, rid?}` | register; push `owed` |
| worker → surface | `owed {request}` | draw it; surface sends `shown {rid}` |
| worker → surface | `withdrawn {rid, cause}` | close the card/sheet without words; signing: dispatch `transport_dropped` |
| surface → worker | `claim {rid, phase:'approve'\|'sign'\|'submit', nonce}` | `claimResult {nonce, live, cause?}`; live → `claimed` to the page |
| surface → worker | `answer {rid, result \| error, opHash?: {chainId}}` | `answered {rid, delivered}`; next `owed`; dedicated window closes; `opHash` recorded (RF3) |
| surface → worker | `ping` (every 20 s while owing) | keeps the worker alive |

Fallbacks kept: `requestAnswer` / `requestDetail` by `sendMessage`. Removed: `requestCurrent`,
`nextForPanel`. Pure rules in `extension/lib/request-life.js`: `nextForWindow`, `claimVerdict`,
`recoveryPlan`, `affectedBy`, `surfaceAfterOpen`, `settlement`; `protocol.js`
`droppedChannelAnswer(bucket, method, attempt)` never echoes Chrome's error text.

Web ports: `SignResponder.claim?(id, phase)`, `SignShellPorts.askerLive(id, phase)` (true when the
transport has no claim), `handleDAppRequest(…, beforeSubmit?)` whose signFn wrappers throw
`AskerGoneError`; the executor maps it to `asker_gone`. `lib/dapp/panel-surface.svelte.ts`:
`start()`, `current`, `onWithdrawn(cb)`, `claim()`, `answer()`, `isPanelDocument()` (`?panel` or
sessionStorage `vela.surface.panel`).

## 15. Log lines (FR-018, FR-019) — [RB14, RD12, RE11]

Never logged anywhere: private key material, passkey secrets, seeds, signatures, calldata, request
params or results, full URLs (paths, queries, fragments), wallet addresses.

| Client | Where | Lines (areas) |
|---|---|---|
| Desktop | stderr, `[vela-wallet] HH:MM:SS.mmm area: …` via `vlog!` | `browser:` asked / committed / finished / watchdog / probe verdict=… route=… / failed class=… / retry … (or `skipped (engine still loading)`) / engine stopped without commit / navigation held / navigate asked … view=…; `proxy:` routes for … / `<host:port>` unreachable \| refused tunnel \| timed out / PAC … failed; `rpc:` chain=… host=… method=… outcome=… / gave up after …; `chain notice:` shown / cleared / retry → ok\|failed; `fee:` quote failed reason=… re-quote #n / quote back; `tracker:` op=<short> receipt=… / status unavailable; `relay:` submitting … / submit verdict=<accepted\|maybe_sent\|not_sent> hash=<12> attempts=n / submit answered in=…; `dapp:` read chain=… method=… failed; `trusted signer:` page <host> unreachable; `window:` close held |
| iOS | `os.Logger`, subsystem `app.getvela.VelaWallet`, categories browser, sign, relay, rpc, fee, tracker, balance, net | same events; NSError domain+code; host public in DEBUG, FNV token in Release; ring of 8 "scope: kind" for the bug report |
| Extension worker | console `[vela-sw] <iso> <event> k=v`, ring `vela.sw.log` (200), counters `vela.sw.counts` | `sw.start`, `req.arrived/surface/shown/claim/answered/settled/resumed`, `panel.up/down`, `read.fail/failover/slow/exhausted` (hosts only) |
| Extension panel / web | existing `[UserOp]`, `[sign_request]`, `[tx_tracker]` console lines | + `submit verdict=…` |

## 16. Corpus keys — [RI2]

New (15 locales): `componentsUi.signing.maybeSent`, `explore.loadProxy`, `explore.requestOpen`
(ruling 10: zh "请先完成或取消这个请求").
Reworded: `componentsUi.signing.simUnavailableWarning`. Deleted: `send.txErrorTimeout`. Everything
else reuses existing keys (list in research RI2). Residency after the change ≈ 138,729 of 138,800.

## 17. What each client promises

- **Submit**: loop on `submit_step`; OR `maybe_delivered`; nonce only on Accepted; MaybeSent →
  `OpSubmitted{maybe_sent}` + the same receipt wait → one Ok answer.
- **Status**: `USER_OP_STATUS_METHOD` + `parse_user_op_status`, no local parser.
- **Find-event**: run `FindOpEvent` through the pool and answer `OpEvent` as it came; persist
  `maybe_sent` and `submit_block` with the record and restore both into `TrackPendingRecord`.
- **Signing sheet**: words from `SignView.phase`; ending from `ending_state`; send `CeremonyStarted`
  / `CeremonyDone` around the passkey; never patch a dApp record Confirmed.
- **Extension**: claims at approve / sign / submit; `AskerGone` when not live; never Chrome's text.
- **Plain send**: draw `PlainSend` (intent, amount card with the fee-row symbol, recipient party).
- **Activity**: rows from `FeedItem.kind/status/site`; poke the feed after every record write;
  empty lines from `history_empty_key` / `home_empty_key`.
- **Simulation**: normalise the reply, call `sim_outcome`, draw `notice`.
- **Browser**: bar from `address_bar`; timers from `GIVE_UP_MS` / `should_give_up` / `stalled()`;
  Recents and headers from `site_label`; chain notice from `failed ∪ unreached`; balance read on
  `HoldingsMoved` and on `CameBack`.
- **Logs**: the lines above for every failure shown, nothing secret.
