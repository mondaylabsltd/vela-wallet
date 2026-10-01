# Research — spec 082

Phase 0 of the plan. Six read-only research passes (A money safety, B extension lifecycle, C plain
transfer, D desktop, E iOS, G 079 leftovers) and an i18n budget pass ran on 2026-09-28 against the
082 worktree: `de93634f` plus `35ac2b07` (G3, G29) and `bea04b57` (`VELA_DEV_PROXY`, chaos `mute`).
Workstream F (chain reads that hang) had no research pass; its decisions (RF*) come from the plan
writer's own read of the lines cited there and of `device-pass-plan.md` §3. Workstream H (how 082
is verified) collects the test-infra parts of every pass.

Nothing here was built or run. "Unverified" marks a claim a test or a device row must confirm.

**Path shorthand.** `core:` = `rust/crates/vela-core/src/app/`; `desktop:` =
`app-desktop/vela-wallet/src/`; `ios:` = `app-ios/VelaWallet/VelaWallet/`; `android:` =
`app-android/vela-wallet/app/src/main/java/app/getvela/wallet/`; `web:` =
`app-web/vela-wallet/src/lib/`; `ext:` = `app-web/vela-wallet/extension/`; `signer:` =
`app-web/trusted-signer/src/lib/`; `relay:` = the vela-relay repo (read, not changed).

## R0 — Open questions and how each is closed

| Question | Where it was open | Closed by |
|---|---|---|
| Should dApp transactions show in Activity? | device-pass-plan §5 Q1 | Owner ruling 7 (in scope) → RG1–RG4 |
| $0.01 native fee floor | §5 Q2 | Owner ruling 2 (no change; relay hand-off) → RG13 |
| Lost submit reply wording and tracking | §5 Q3 | Owner ruling 1 → RA1–RA6, RA10 |
| Desktop proxy policy, PAC | §5 Q4 | Owner ruling 3 → RD2, RD9 |
| Desktop tab switch during a request | §5 Q5 | Owner ruling 4 (hold) → RD1 |
| Fault injection scope | spec ruling 6, this run's request 不能影响设备上的其他流量 | RH1 |
| i18n headroom (50 B) | spec "Decisions left to the owner" | RI1–RI3: net −21 B, no cap raise |
| G23 (d): no stored entry for the new page | B, unexplained by reading | Fixed mechanisms (a)–(c) in RB1/RB8/RB9; RB14 logs settle (d) on the device (quickstart EX8b) |
| G2: first Enter in a restored tab does nothing | D, unexplained by reading | RD5/RD6 remove both suspects; RD6's `browser: navigate asked … view=` line settles it (DX11) |
| PAC file unreachable: fall back to DIRECT? | D risk | No: ruling 3 forbids a silent direct route; reported as a proxy failure (RD2, RD9) |
| Failure panel: lock or no lock | D vs E | No lock (RE1), all clients |
| Apple -1000 class | E (G31) | Offline (RE4) |
| G24: iOS-only stop-gap or core `read_plan` | E | Core, all clients (RE9), house rule |
| Answer to the dApp for a revert inside the window | A (RA8) | tx hash on all four; owner may reverse (plan Q2) |
| Residual double pay after an ignored MaybeSent | A (RA5) | Recorded; chain-log lookup offered as plan Q1 |

## RX — Where the research passes disagree, and the decision

| Topic | Pass says | Other pass says | Decision and why |
|---|---|---|---|
| i18n residency baseline | G: 134,738 | I18N: 138,750 (node replica of the test) | **138,750.** The binding assertion is the runtime-JSON route (`rust/crates/vela-core/tests/i18n_residency.rs:213-274`); 134,738 is the compiled-in route (2-byte en offsets, `i18n_catalogs/mod.rs:54,78`), which never binds. Matches 079 `results.md:127`. |
| Words for a lost reply | A: one sentence `componentsUi.signing.maybeSent` (+185 B) | I18N: short title "May have been sent" (+65 B) + reuse `stillConfirming` as the caption | **A's sentence.** `stillConfirming` opens with "Not on-chain yet", which G21 proved false at the moment it would show (the op landed 42 s before the sheet spoke). The bytes come from deleting the unused `send.txErrorTimeout` (−213 B). |
| `simUnavailableWarning` rewording | G: "couldn't check…" (−191 B, 15 drafts) | I18N: "couldn't preview…" (−199 B, 5 drafts) | **G's.** FR-012 and US4 say "could not check"; G has all 15 locales drafted. |
| `home.emptyNoActivityNetwork` | G, I18N: delete (−101 B, no consumer) | D: the desktop home under a chain filter needs it | **Keep**, chosen by the core (`FeedView.home_empty_key`, RG5) so the choice is not a desktop copy. |
| Lost reply in `sign_request` | A: no new outcome; `maybe_sent` flag on `OpSubmitted` | G: assumed `SignSubmitOutcome::MaybeSent` | **A's.** `ReceiptPending` already persists before answering and hands the record to the tracker (`sign_request.rs:2191-2243`); G's requirement (pending row under the local hash) is met by `on_op_submitted`. |
| Page-load watchdog in core | D: move the whole desktop `LoadWatch` into `browser_load` | E: move constants + `stalled()` only | **Both.** `LoadWatch` moves (desktop uses the crate); the phones take the constants, `stalled()` and one give-up predicate `should_give_up(elapsed, committed, progress)` so "a slow but answering site is never cut" is one rule. iOS reads `estimatedProgress` too. |
| Address bar over a failure panel | D: failed host with its scheme lock (`page.rs:12526-12550`) | E: failed host, no lock | **No lock** (RE1): nothing from that host is on screen. |
| Constant names | D: `WATCHDOG`, `PROBE_BUDGET`, `GIVE_UP` | E: `STALL_*` | `browser_load::{WATCHDOG_MS, PROBE_BUDGET_MS, GIVE_UP_MS, ENGINE_LIVE_PROGRESS}`. |
| `explore.loadProxy` wording | D: 139 B | I18N: "Your proxy isn't responding." 78 B | **I18N's**, shorter; the panel's Retry is the action. |
| What counts as a proxy failure | D (RD2/RD9): every `ProxyFailure`, incl. a refused tunnel, → class `proxy` | E (RE4): iOS -1000 (the per-app proxy refusing CONNECT) → `offline` | **Plan writer: only a proxy that cannot be reached (or a PAC that cannot run) is `proxy`.** A proxy that answered — 502, or a close with no reply (what chaos `drop` does, `scripts/device/chaos-proxy.py:146-149`) — spoke for the host; calling that "your proxy isn't responding" would blame a working proxy for every blocked or missing site, i.e. for most proxy users' ordinary failures. Keeps RD9 and RE4 one rule. |
| `TrackOutcome` ending for a never-admitted op | G (RG4): stays pending until the 24 h abandon | A (RA4): `NotSent` after two `not_found` ≥ 60 s | **A's.** |
| Answer window vs the extension's page deadline | A (RA12): 120 s from approval | B (RB11): content.js 5 min after claim | Compatible; RA12 adopted, so every on-chain answer arrives well inside RB11's deadline. |
| No-click request with the side panel open | today: a window (e2e `extension-live-provider.e2e.ts:343-357`) | B (RB8): the panel | **The panel**; row EX2's expectation changes. |

---

# A — Money safety (G21/W1, G13/W4, W3, G22)

Root causes. All four clients call the pool once for `eth_sendUserOperation` and turn the pool's
give-up into "relay unreachable, try again": desktop `desktop:executor/relay.rs:597-599` →
`executor/user_op.rs:726-729`; iOS `ios:Core/RelayClient.swift:522-523` →
`Core/UserOpSpine.swift:429-430` → `Features/Signing/Core/SignExecutor.swift:258-259`; Android
`android:feature/send/core/RelayClient.kt:409-410` → `UserOpSpine.kt:373`; web
`web:services/rpc-pool.ts:111-123` → `signing/core/sign-executor.ts` classifySubmit. The core pool
cannot say whether bytes left the device: `RpcTransportOutcome` (`core:rpc_pool.rs:272-285`) lumps
DNS/TLS/refused with a reset after the write, and `RpcCallVerdict::Failed` (`:302`) carries only
`rate_limited`. No EntryPoint v0.7 userOpHash exists in core, UniFFI or wasm (only
`calculate_safe_op_hash`, `rust/crates/vela-core/src/user_op.rs:357`). The relay serves only
`pimlico_getUserOperationStatus` (`relay:vela-relay-core/src/wire.rs:196-221`; live probe
`evidence/relay-status-probe.txt`). A Landed ending is drawn "confirmed" without asking the tracker
(iOS `SigningAftercare.swift:47-58`, `SigningLive.swift:505-506`; Android `SigningAftercare.kt:36-44`,
`SigningLive.kt:544`; desktop `desktop:signing/status.rs:91-118`, `353-374`). The sign view has only
`is_signing`/`is_submitting` (`core:sign_request.rs:1080-1102`).

## RA1 — How a submit failure is classified (G21/W1)

- **Decision**: new pure `user_op::submit_step(reply, attempt, maybe_delivered, local_hash) -> SubmitStep`. In order: (1) a result hash → `Accepted` (the relay's hash wins; a mismatch with the local hash logs `userop.hash_mismatch`); (2) an `[existingHash:0x…]` marker, searched in the raw error JSON first, then in `relay_error_message` → `Accepted` with that hash; (3) "currently processing" / "Retry later" with `attempt < SUBMIT_MAX_RETRIES` (3) → `RetryAfter 3000 ms`, the identical op re-POSTed; (4) any error or pool exhaustion while `maybe_delivered` → `MaybeSent(local hash)`; (5) an error with `!maybe_delivered` → `NotSent{Some(classify_relay_rejection(..))}`; (6) exhaustion with `!maybe_delivered` → `NotSent{None}`. `maybe_delivered` is the OR over every POST of the op of `rpc_pool::may_have_delivered(outcome)`: true for Timeout, Network, NonJson and HTTP 5xx; false for the new `NotConnected` (DNS, refused, TLS, proxy CONNECT failure, connect timeout), for any JSON answer and for HTTP 4xx.
- **Rationale**: only proof that nothing left the device, or a refusal no earlier attempt could have caused, makes "not sent — try again" true; an AA25 on attempt 2 after a lost reply on attempt 1 proves nothing. The relay errors only for ops it did not durably queue (`relay:admission.rs:101-126`, `send_user_operation.rs:179-230`) and returns the hash for an identical re-POST (`AlreadyQueued`, `send_user_operation.rs:189-198`). One function replaces four busy loops and their 3/3000 constants (desktop `relay.rs:595-616`, iOS `RelayClient.swift:514-534`, Android `RelayClient.kt:407-421`, web `services/safe-transaction.ts:3314-3354`).
- **Alternatives**: every transport failure = MaybeSent (a plainly refused relay would read "may have been sent" on native clients, where refusal is provable); regex relay errors for deterministic AA codes (fragile; nonce errors are the ambiguous case); keep per-client loops (FR-020).

## RA2 — What the dApp is answered after MaybeSent (exactly one answer)

- **Decision**: `Ok` with a hash, never 4900 or -32603. After MaybeSent the client sends `OpSubmitted{maybe_sent: true}` with the local hash and runs the same receipt wait as for an accepted op, polling that hash. A receipt → `Succeeded{tx_hash}`; otherwise `ReceiptPending{user_op_hash}` (079's still-confirming contract).
- **Rationale**: EIP-1193 has no "outcome unknown" code; dApps read 4900/-32603 as "not sent" and retry — the G21 double payment. The op hash is what 079 already hands dApps after the window and what EIP-5792 returns as a batch id; the in-app browsers translate it for receipt reads (`core:dapp_browser.rs:1482-1497`; `web:services/dapp-submit.ts:987-1008`), and RF3 gives the extension the same translation.
- **Alternatives**: -32603 plus a "may have been sent" sheet (the page and sheet contradict each other); the op hash at once with no wait (loses the real tx hash when the relay was only briefly mute); hold until the tracker resolves (breaks content.js's bounded answer).

## RA3 — The core surface for MaybeSent in `sign_request`

- **Decision**: no new `SignSubmitOutcome` variant. `#[serde(default)] maybe_sent: bool` on `Event::OpSubmitted` (`sign_request.rs:541-545`), `SignTrackerHandoff` (`:320-325`) and `SignRecord` (`:266-285`); `SignView.pending_op_maybe_sent`. `on_op_submitted` (`:1940-1990`) copies the flag into the persisted pending record and the handoff.
- **Rationale**: `ReceiptPending` already does what MaybeSent needs — record before answer, the tracker alone closes it, the rid settles Submitted so it never signs twice, and a swipe after commit is a dismiss, not a 4001 (`committed()`, `:839-850`). Serde-additive, so old shell JSON still decodes.
- **Alternatives**: a `MaybeSent` variant with its own persist/answer branch (duplicates `ReceiptPending`).

## RA4 — How the tracker ends a MaybeSent op (ruling 1: "track to its end")

- **Decision**: `tx_tracker` changes — `Submitted` and `TrackPendingRecord` gain `maybe_sent`; `Entry` gains `maybe_sent`, `acknowledged`, `not_found_streak`. `acknowledged` = any receipt or any relay status other than `not_found`. `TrackEntryView.outcome` is the new `MaybeSent` while `maybe_sent ∧ ¬acknowledged ∧ ¬terminal ∧ ¬abandoned` (Unknown still wins at 24 h). For such entries status polls continue past the 120 s window at `receipt_interval_ms` cadence (today they stop, `tx_tracker.rs:951-969`). The relay answering `not_found` at age ≥ `NOT_FOUND_GRACE_MS` (60 s) `NOT_FOUND_CONFIRMATIONS` (2) times in a row with no receipt → new terminal `EntryStatus::NotSent` / `TrackStatus::NotSent`, records patched `failed` through `fail_records`. `StatusUnavailable` resets nothing; time alone is still never a failure.
- **Rationale**: the relay stores every op it admitted and answers `not_found` only for hashes it never admitted (`relay:wire.rs:597-620`, `task.rs:8-23`). Without this end, a web submit whose connection was merely refused (the web cannot prove refusal) reads "may have been sent" for 24 h. Polls must outlive the window because in exactly this fault the relay is down during the window.
- **Alternatives**: reuse `TrackStatus::Rejected` (Send words it "fees stayed above…", `desktop:wallet/money.rs:935`); a chain `UserOperationEvent` log lookup by hash — relay-independent proof, offered as plan Q1; resolve by chain nonce (not proof: another key of a multi-key account can consume it).

## RA5 — The nonce of a MaybeSent op

- **Decision**: a MaybeSent op never advances the local nonce. Desktop `chain::bump_nonce` (`desktop:executor/user_op.rs:732`) and web `incrementNonceCache` (`safe-transaction.ts:1812, 2143, 2417`) run only for Accepted. iOS and Android read `EntryPoint.getNonce` every time (`RelayClient.swift:654-667`, `RelayClient.kt:534-539`). A new attempt therefore reuses nonce N while N is unconsumed, and the EntryPoint lets at most one land. No blocking reservation in 082.
- **Rationale**: a double payment needs two ops with different nonces — only a local pre-increment, or the first op having already landed (then the receipt poll and "don't send it again" protect).
- **Alternatives**: block signing on (chain, sender) while a MaybeSent op is unresolved (locks a person out up to 24 h when the relay is down); a pre-sign warning (needs a sentence). Residual risk is in plan.md.

## RA6 — Computing the userOpHash locally

- **Decision**: `user_op::user_op_hash(op, chain_id) -> Result<String, CoreError>` (EntryPoint v0.7): `keccak256(abi.encode(keccak256(abi.encode(sender, nonce, keccak(initCode), keccak(callData), bytes32(verificationGasLimit<<128|callGasLimit), preVerificationGas, bytes32(maxPriorityFeePerGas<<128|maxFeePerGas), keccak(packedPaymasterAndData))), ENTRY_POINT, chainId))`; `packedPaymasterAndData` is empty or `paymaster[20] ‖ u128 0 ‖ u128 0 ‖ data`, mirroring `user_op_to_json` (`user_op.rs:629-655`); the signature is excluded. Exported as UniFFI `userOpHash(draft, chainId)` (next to `vela-core-uniffi/src/lib.rs:1386`) and wasm `userOpHash(opJson, chainId)` (AttestOp shape, `vela-core-wasm/src/lib.rs:1537-1630`). The relay's hash always wins.
- **Rationale**: no client has one. The relay's `abi.rs:236-255` / `admission.rs:632-648` is the reference. `paymaster_and_data` is empty today (`user_op.rs:979, 1347`).
- **Alternatives**: none workable (no by-sender lookup API).
- **Test vector**: decode the `handleOps` calldata of Gnosis tx `0xc6f3544f…4dc4` (block 48478729, `evidence/extension/w1-lost-reply-landed.txt`) and assert the hash equals its `UserOperationEvent` `topics[1]`. Capturing it needs one read-only RPC fetch at implementation time (unverified here).

## RA7 — G13: the relay status method and its parser

- **Decision**: `tx_tracker::USER_OP_STATUS_METHOD = "pimlico_getUserOperationStatus"` and `parse_user_op_status(json) -> Option<TrackStatusAnswer{status, stage, tx_hash}>` (unknown status strings → `None`), exported through UniFFI and wasm; they replace desktop `relay.rs:727-759`, iOS `RelayClient.swift:574-587`, Android `RelayClient.kt:467-481`, web `services/tx-reconciler.ts:91-112`. The web keeps a TS constant pinned by vitest to the wasm value. Fix the e2e stub (`e2e/stub-chain.ts:301-302` answers the wrong name and an invalid `pending`) and `RelayClientTest.kt:217`; the desktop live test (`relay.rs:1260-1262`) must fail on `None`. `TrackShellResult::Status` gains an optional `tx_hash` (079 D2 explorer link). Ask the relay owner for an `eth_getUserOperationStatus` alias (hand-off).
- **Rationale**: the shapes already agree; only the name drifted, and -32601 is not a ban (`rpc_pool.rs:601-646`), so nothing else needs undoing.
- **Alternatives**: the method name inside `TrackOperation::PollStatus` (the web's `waitForReceipt`, `safe-transaction.ts:3456`, polls outside the tracker).

## RA8 — W3: the ending of an on-chain dApp request

- **Decision**: (1) pure `sign_request::ending_of(method, payload, submitted_user_op) -> Option<SignEnding>` replaces three copies (desktop `status.rs:91-118`, iOS `SigningAftercare.swift:47-58`, Android `SigningAftercare.kt:36-44`); `Landed` also carries `user_op_hash`. (2) `ending_state(ending, track) -> SignEndingState`: tracker Confirmed → Confirmed; Dropped → Reverted; NotSent/Rejected → NotSent; else Following{outcome, fee_held}; no entry yet → Following(Landing). (3) `on_submit` Succeeded with a record (`sign_request.rs:2143-2162`) stops emitting `UpdateRecord Confirmed` (`:2152-2156`); the tracker alone closes on-chain records. (4) The dApp is answered the tx hash for an op that reverted inside the wait, on all four; the web's `waitForReceipt` throws a typed `UserOpRevertedError{txHash}` instead of "dropped… try again" (`safe-transaction.ts:3427-3431`), which `dapp-submit.ts:503-507` turns into the hash.
- **Rationale**: `TrackStatus::Dropped` + `safe_execution_failed` (`tx_tracker.rs:684-697, 999-1010`) is the only place a revert (including a Safe `ExecutionFailure` inside a successful op) is judged; two writers of one record race. A hash answer does not depend on how fast the revert is seen and matches the desktop's documented intent (`desktop:executor/sign_request.rs:506-509`).
- **Alternatives**: -32603 when the revert is seen inside the window (timing-dependent; owner may still prefer it — plan Q2; the display fix is independent).

## RA9 — G22: a truthful stage before the passkey prompt

- **Decision**: `Event::CeremonyStarted{id}` / `CeremonyDone{id}` (id-guarded; accepted only while that inflight is in `Submitting`); `Inflight.ceremony: NotYet | Up | Done`; `SignView.phase: SignPhase = Idle | Preparing | AwaitingSignature | Submitting`. Precheck, Sponsoring, and Submitting-before-ceremony → Preparing; ceremony Up → AwaitingSignature; Done, ReactiveSponsoring, PersistingResult → Submitting. Words: Preparing → `send.txPreparing`; AwaitingSignature → `send.txSigning` (tx) or `componentsUi.signing.signing` (message); Submitting → `send.txSubmitting` + `send.txBackgroundHint`. Android's no-op dApp `signingStarted` port (`VelaWalletApplication.kt:690`) is wired.
- **Rationale**: parity with Send, which already moves Submitting → Signing on `SigningStarted` (`core:send.rs:1147, 1872-1882`). Today the extension says "Waiting for biometric…" through ~40 s of precheck and relay estimate (`web:signing/live.ts:826-838`). Zero new strings.
- **Alternatives**: shell-derived flags (four copies; Android's port is a no-op).

## RA10 — Words for MaybeSent, NotSent and Reverted

- **Decision**: MaybeSent (sheet, dApp aftercare, Send receipt): title `send.txSubmitting`, caption new `componentsUi.signing.maybeSent` "It may have been sent. Vela keeps checking — don't send it again.", the op hash row, CTA `send.txCloseBackground`, no Retry; after the relay acknowledges, the existing Landing/StillConfirming words. NotSent: `componentsTx.receipt.statusFailed` + `send.txErrorGeneric` (now true). Reverted: `statusFailed` + `componentsTx.receipt.failedHint` + explorer link (desktop already, `status.rs:262-279`; iOS/Android wrongly use `txErrorGeneric` for Dropped, `SigningLive.kt:544-552`). The dApp's -32603 detail for NotSent is a fixed "relay unreachable; nothing was sent", never the pool's raw text (`rpc-pool.ts:119`).
- **Rationale**: ruling 1's wording; see RX for why not `stillConfirming`.
- **Alternatives**: `stillConfirming` as the caption (0 B, not true at that moment).

## RA11 — i18n for workstream A

- **Decision**: add `componentsUi.signing.maybeSent` (15 locales, drafts in RI2); delete `send.txErrorTimeout` in all 15 (no consumer anywhere; only `rust/crates/vela-core/src/i18n/paths.rs:1510`; its "submitted but timed out" framing is what the tracker rules forbid).
- **Rationale / Alternatives**: see RI2.

## RA12 — The dApp answer deadline after a slow submit

- **Decision**: `sign_request::DAPP_TX_ANSWER_WINDOW_MS = 120_000`, measured from `ApproveTapped`; each client's receipt wait is `max(10 s, window − elapsed)`. Replaces desktop `RECEIPT_BUDGET` 90 s (`desktop:executor/sign_request.rs:156`), iOS 120 s (`SignExecutor.swift:91`), Android 120 s (`SignExecutor.kt:41`), web 120 s (`safe-transaction.ts:3386`).
- **Rationale**: MaybeSent is decided after ~30–46 s of relay timeouts (2 passes × 15 s, `rpc_pool.rs:152-178`); a further full 120 s wait answers at ~210 s, near content.js's 300 s limit, and the clients disagree today (90 vs 120 s).
- **Alternatives**: keep per-client waits (the page can wait > 3 min on the extension).

---

# B — Extension request lifecycle, extension/web UI, worker logs (G17, G18, G19, G23, EX6, EX7, G16, G1-web, W23-ext)

Root causes (all verified in code by B). **G17**: the worker settles a request only on
`windows.onRemoved` / `tabs.onRemoved` (`ext:background.js:253-261, 277`; the comment at `:263-266`
claims "or navigated" but nothing handles navigation); the record stays in `pending` (`:137`) and
storage.local (`:139-145`); the panel ignores `delivered:false` (`web:dapp/transport.ts:170-184`,
`dapp/DappRequestHost.svelte:221-222`). **G19**: `ext:content.js:62-63` passes Chrome's error text as
-32603; `sendResponse` dies with the worker and the start sweep deletes every `vela.req.*`
(`background.js:289-298`); the panel's later answer finds no entry (`:156-158`). **G23**: stale records
(a), surface set only after `sidePanel.open` resolves while the panel may ask first (b,
`background.js:197-202, 238`; `transport.ts:146-167`), the panel forgets `?panel` after Wallet →
Settings → Wallet (c, `routes/[locale]/wallet/+page.svelte:238-241`, `settings/+page.svelte:698`), and
(d) unexplained. **G18**: `tabId ??=` (`DappRequestHost.svelte:130`) plus the tab filter
(`background.js:235-243`) under one global panel per window. **EX6**: settle only from an async
`pagehide` send (`DappRequestHost.svelte:195, 262-277`). **G16**: 19 undefined token references
(`DappRequestHost.svelte:437-485` ×15, `signing/ui/SigningBody.svelte:96-98` ×3,
`routes/dev/gallery/+page.svelte:246` ×1). **G1 (wide web)**: `wallet/WalletDesktop.svelte:109-128,
134-148` ignores the section `mode`. **W23**: one worker log line (`background.js:107`). **Core
hole on every client**: `TransportDropped` (`core:sign_request.rs:1057-1068`) clears the sheet but
leaves a pipeline in precheck/sponsoring running; `on_precheck` (`:2014-2033`) then signs.

## RB1 — Who owns an extension request's life

- **Decision**: the service worker. Each request is a record in `chrome.storage.session` under `vela.req.<tabId>:<pageRequestId>` = `{v, rid, id, method, params, origin, tabId, windowId, documentId, sentAt, at, surface, surfaceWindowId, state}`. A record leaves only when answered or settled. At first start the worker also removes leftover `vela.req.*` from storage.local.
- **Rationale**: only the worker sees both the asking page and the answering surface; storage.session survives worker restarts, is cleared with the browser, and is unreadable by content scripts. The in-memory map plus the start sweep caused G19.
- **Alternatives**: storage.local + sweep (G19/G23 as today); panel-owned lifecycle (dies with ✕, cannot see reloads).

## RB2 — What goes into vela-core

- **Decision**: (1) `TransportDropped` also stops a pipeline still before the passkey (Precheck, Sponsoring, ReactiveSponsoring → inflight cleared, attempt bumped, effect aborted); past the commitment point nothing changes. (2) New `SignSubmitOutcome::AskerGone` (nothing sent, no answer, no record, sheet closes). (3) wasm `signRequestTtlMs()` = `EXTENSION_REQUEST_TTL_MS` (`sign_request.rs:80`). The extension's own lifecycle rules live in a new pure `ext:lib/request-life.js`, codes and TTL pinned to the core by vitest (the `instant.test.ts:61-69` pattern).
- **Rationale**: the worker cannot load the ~3.6 MB core per wake (`ext:lib/protocol.js:329-338`); in-app browsers already have the rule (`dapp_browser.rs:926-970` retire_document). (1) closes a real hole on all four clients.
- **Alternatives**: a core `ext_request` machine in the panel (the panel is not alive when the page leaves); wasm in the worker (cost); `transport_id` on `SignOperation::SignAndSubmit` (wire change on four clients).

## RB3 — How the worker knows the asking page is still there

- **Decision**: Chrome's `documentId`. content.js opens a `vela.doc` port only while it owes a sign/connect answer; the port closing while the worker is alive means the page left → settle `page_left`. Answers go by `chrome.tabs.sendMessage(tabId, msg, {documentId})` (rejection = page gone). `tabs.onRemoved` / `onReplaced` settle as a backstop. No new manifest permission.
- **Rationale**: `sender.documentId` and `tabs.sendMessage({documentId})` (Chrome 106+) are the browser's own facts; the manifest already requires 116. Answering by documentId works from a restarted worker (US1 AS6).
- **Alternatives**: `tabs.onUpdated 'loading'` (fires while the old page lives; misses bfcache); `webNavigation` (new permission, re-consent).

## RB4 — Worker restart: resume, don't settle

- **Decision**: content.js sees its port close and reconnects with backoff 0/250/1000/3000 ms, which wakes the new worker; the panel reconnects its port. At start, `recover()` checks each record: past 5 min → 4900 `expired`; panel window with no side panel (`runtime.getContexts SIDE_PANEL`) or dedicated window gone → `surface_closed`; page not answering `alive` by documentId → dropped, panel told; else kept.
- **Rationale**: US1 AS6 — approving after a worker restart still completes and the dApp gets its answer.
- **Alternatives**: settle every open request on restart (breaks AS6; tempts a double send).

## RB5 — Claim: check the request is live before signing

- **Decision**: the panel claims the request at three points — `approve` (before writing a connect grant), `sign` (before the passkey), `submit` (after the passkey, before the relay POST). Live only if the record exists, belongs to that window's surface, is under 5 min (not checked for `submit`), and its page port is attached; `submit` requires state `claimed`. "No", or no answer within 5 s after one reconnect → nothing signed or sent; the core gets `AskerGone`. A live claim tells content.js `claimed`, extending its deadline.
- **Rationale**: exactly the G17/G19 harm. A false "no" costs one re-approval, never money; it also covers parallel-space fixture signing (the check precedes any signer).
- **Alternatives**: check only at Approve (leaves the ~40 s precheck window); rely on pushes (lost when a port is down).

## RB6 — What the dApp is told when a request ends without a decision

- **Decision**: always 4900 (pinned to `popupCloseSettlement()`), never 4001, with plain English in `protocol.js` `SETTLE`: `page_left` "The page navigated away" (as `dapp_browser.rs:945`), `surface_closed` "The browser closed before the request finished", `expired` "Vela did not answer in time — check its activity", `restarted` "Vela restarted before the request finished — check its activity", `updated` "Vela was updated — reload this page and try again". Chrome's `error.message` is never passed on.
- **Rationale**: these go to the dApp's code, not a screen, so they are not corpus strings (`web:dapp/core/dperm-types.ts:39-47`). 4900 means "may have happened", which stops a re-send.
- **Alternatives**: pass Chrome's text (G19); -32603 (dApps offer "try again").

## RB7 — Several tabs (G18)

- **Decision**: one queue per window, oldest first, across the window's tabs; no active-tab filter, no automatic tab switching; the sheet names the site.
- **Rationale**: the manifest declares only `side_panel.default_path` (one global panel per window); `dapp_browser.rs:49-55` serialises the same way.
- **Alternatives**: per-tab panels (a wallet and core per tab; switching hides a sheet mid-passkey); follow the active tab (hides requests).

## RB8 — Where a request is shown

- **Decision**: from a click, `sidePanel.open` synchronously and record `surface: 'panel'` at once. If the open fails or hangs 2 s: a side panel already open in that window → keep `panel` and wake it through the storage.session write; else a window. A saved `window` preference wins.
- **Rationale**: removes the race at `background.js:197-202`; a no-click request no longer pops a window beside an open panel (EX2).
- **Alternatives**: today's behaviour.

## RB9 — The panel stays the panel

- **Decision**: first `?panel` load sets sessionStorage `vela.surface.panel=1`; `inPanel` = `?panel` OR that flag. A panel controller started from `routes/+layout.svelte` keeps the `vela.surface` port and switches to the wallet screen when a request is owed while another screen shows.
- **Rationale**: `goto(walletHref)` drops `?panel` (`settings/+page.svelte:698`, `contacts/+page.svelte:444`).
- **Alternatives**: carry `?panel` on every goto (every future link must remember).

## RB10 — Chrome's panel ✕ (EX6)

- **Decision**: the panel holds its `vela.surface` port (20 s heartbeat while owing); the port closing with the worker alive settles every request of that window's panel with `surface_closed` at once. A panel reload does the same (today's pagehide behaviour); the pagehide settle stays as a backstop.
- **Rationale**: an async send during pagehide is not guaranteed; a port close is. The heartbeat keeps the worker awake while a person decides (Chrome 114+ per docs; not verified on 154).
- **Alternatives**: a 1.5 s grace to tell reload from ✕.

## RB11 — The 5-minute limit (EX7)

- **Decision**: the worker refuses `approve`/`sign` claims at 5 min (`REQUEST_TTL_MS` pinned to `signRequestTtlMs()`), age counted from content.js's `sentAt`. content.js gives up at 5 min + 5 s unclaimed, or 5 min after the claim, then tells the worker (`abandon`), which settles `expired` and withdraws the sheet. `request_ts_ms` stays null in `RequestArrived`.
- **Rationale**: one rule at both ends, the page's deadline strictly after the worker's.
- **Alternatives**: `chrome.alarms` (unneeded).

## RB12 — G16: the consent card's look

- **Decision**: the shared `web:ui/Button.svelte` on the consent card (Cancel secondary, Connect primary with a spinner while busy; Cancel disabled while busy) and for `SigningBody`'s dismiss. Remaining tokens map: `--space-3→--space-lg`, `--space-4→--space-xl`, `--space-6→--space-3xl`, `--color-text-primary→--color-fg-base`, `--color-text-secondary→--color-fg-muted`, `--color-text-tertiary→--color-fg-subtle`, `--color-bg-elevated→--color-bg-raised`, `--color-border-subtle→--color-border-strong`, `--color-accent-on→--color-onAccent`, `--border-width-hairline→--border-hairline`, `--font-size-body→calc(var(--text-lg) * var(--text-scale, 1))`, `--font-size-title-3→calc(var(--text-2xl) * var(--text-scale, 1))` + `--weight-bold`, `--font-weight-semibold→--weight-semibold`, gallery `--layout-galleryRail→--layout-settingsNavW`. The raw method line goes (panel and window). Window mode with no live request closes the window instead of hard-coded English. A vitest gate in `web:tokens/tokens.test.ts` fails on any fallback-less `var(--x)` nothing defines.
- **Rationale**: FR-010; Button already has height, white-on-accent label, busy≠disabled and press feedback (house button rule).
- **Alternatives**: tokens only, keep hand-made buttons.

## RB13 — G1 on the wide web layout

- **Decision**: `WalletDesktop.svelte` draws `activitySection.mode` / `assetsSection.mode` exactly as `WalletHome.svelte:88-145` (skeletons while loading, `EmptyState` when empty); `buildDesktopState` (`wallet/fixtures.ts:592-598`) gets the same `empty` copy as the narrow model (`:409-421`).
- **Rationale**: both layouts already get `mode` from `liveSections` (`wallet/live.ts:595-641`). No new strings.
- **Alternatives**: a desktop-only line (new bytes).

## RB14 — Worker logs (W23)

- **Decision**: new `ext:lib/swlog.js`: `[vela-sw] <iso> <event> k=v…` console lines, a 200-line ring in storage.session `vela.sw.log` and counters in `vela.sw.counts`. Fixed events: `sw.start`, `req.arrived`, `req.surface`, `req.shown`, `req.claim`, `req.answered`, `req.settled`, `req.resumed`, `panel.up/down`, `read.fail`, `read.failover`, `read.slow`, `read.exhausted`. Never params, results, signatures, addresses or URL paths; hosts only. The extension's Settings bug report adds `sw:<event>.<cause> ×N`.
- **Rationale**: FR-018/019; the report allowlist permits counters and classes, not endpoint URLs (`web:services/bug-report.ts:44-56`).
- **Alternatives**: console only (lost on eviction); the full ring in reports (hosts may be private).

## RB15 — Words for a withdrawn request

- **Decision**: none; the sheet or card closes, as desktop/iOS/Android do on `CancelSigning` (`desktop:wallet/page.rs:13418-13446`, `android:…/SigningController.kt:376-381`, `ios:App/RootView.swift:1432-1447`).
- **Alternatives**: a notice (~75 B; not worth the headroom).

---

# C — A dApp's plain value transfer (G14)

Root cause (C): `clear_signing::start_tx` tags empty calldata `ReqKind::TxPlain` with `result = None`
(`core:clear_signing.rs:1567-1573`, comment dating from RN); `surface_of` (`:1497-1519`) maps it to
`BlindTransaction`; two tests pin the bug (`tests/app_clear_signing.rs:337-353`, `:2236`). Desktop
draws the blind rung (`desktop:signing/live.rs:231`, `1044-1067`); web/extension draws red "Blind
signature" (`web:signing/live.ts:418-431`); the phones intercept with their own shell copy (iOS
`SigningLive.swift:559-594`, Android `SigningLive.kt:637-658`). No client checks code on the signing
path.

## RC1 — Where the plain-send verdict lives

- **Decision**: in the core. `start_tx` sets `Model.plain_send` for empty calldata; `surface_of` returns new `ClearSurface::PlainSend` when it is Some; the view carries `plain_send: Option<ClearPlainSend>`. Shells draw it and decide nothing; the phones delete their interception.
- **Rationale**: the one place all four ask; keeps `result = None`, so code keyed on `result` is untouched (desktop `signing_host.rs:925`, web `live.ts:603-629`, iOS tech line `SigningLive.swift:222-224`, `clear_signing.rs:885-898`).
- **Alternatives**: a `ClearSignResult` with intent Send (regresses the phones' card to rows; writes "Send" into records); copying the phones' interception into desktop and web (how it drifted).

## RC2 — Code at `to` does not matter

- **Decision**: empty calldata is a plain send whatever the recipient; no `eth_getCode`.
- **Rationale**: owner ruling in G14; a probe adds a failure mode on bad networks. What `receive()` does is the simulation block's job.
- **Alternatives**: probe and caption contract recipients (new string, network dependency).

## RC3 — Zero value with empty data

- **Decision**: the same card with `no_value = true`: "Send · 0 <coin> · Recipient", no minus, neutral `Confirm` (never "Confirm send"); no static "nothing leaves" line.
- **Rationale**: honest at 0 B; a contract's fallback can still spend earlier allowances, so only a simulation may say nothing leaves.
- **Alternatives**: a new title "Empty call" (~32 B, not needed).

## RC4 — When `value` is readable enough to print

- **Decision**: absent/null/""/"0x" = 0; "0x"+hex = exact U256; anything else (decimal text, "null", signs, non-hex, overflow) → `plain_send = None` and the surface stays `BlindTransaction`.
- **Rationale**: iOS/web/Android submit hex (`dapp-submit.ts:424-466`, `SignExecutor.swift:419-420`, `SignExecutor.kt:279`); desktop also accepts decimal (`executor/sign_request.rs:476-498`). Hex-only display never states a figure a submit path reads differently.
- **Alternatives**: `to_quantity` semantics (decimal "1000" would mean 0x1000 on three clients).

## RC5 — Amount formatting and the coin symbol

- **Decision**: the core scales by 18 exactly (no rounding, trailing zeros trimmed, `ClearLocale` marks via `separators()`/`group_digits`, `clear_signing.rs:521-529, 5402-5434`). The symbol is the shell's fee-row symbol (iOS/Android `nativeSymbol`, web `nativeSymbol(chain_id)` `services/networks.ts:162`, desktop `network_admin::builtin_native_symbol`).
- **Rationale**: scaling is the core's job (desktop test `live.rs:1621-1630`); one symbol source per card avoids "xDAI" beside "XDAI" (`network_admin.rs:241-247`).
- **Alternatives**: a core symbol with shell fallback (a second spelling).

## RC6 — The four first-call readers

- **Decision**: minimal patches so a present non-string `value` reaches the core as text (refused under RC4), never as absent (a calm "0"): desktop `signing_host.rs:1025-1029` (`as_str` drops numbers), Android `optString` → `isNull` check, web passes `String(value)` and reads `calls[0]` (`signing/core/sheet.svelte.ts:56-65`), iOS NSNumber → text. Moving the reader into core (`tx_call_of`, also for submit builders) is a follow-up.
- **Rationale**: without it desktop shows "Send 0" and submits 1000 wei; web's number makes serde reject the whole event.
- **Alternatives**: the full core move now (widens G14 into four submit paths).

## RC7 — `wallet_sendCalls` whose first leg is a plain send

- **Decision**: no special case; the rule applies to leg 1 like every rung (CS26 per-leg panorama is out of scope).
- **Alternatives**: a `call_count` on the event (wire change on four clients).

## RC8 — The trusted-signer page

- **Decision**: `hasCalldata` false for absent/""/"0x"/"0X"; every no-calldata call takes the send path, including value 0 (amount "0", no out-minus, no `ui.simNoOther` line) — `signer:resolve.js:242-287`; the blind ladder (`:348-366`) only sees calldata.
- **Rationale**: the page renders independently (supply-chain boundary) but must reach the same verdict.
- **Alternatives**: leave the page (a 0-value empty call reads level-6 danger).

---

# D — macOS desktop (W14, W6/W8, W7, W18, G2, G6, G7, G30, G1, G11, W16, W17, W23-desktop, G4)

Root causes (D): **G6** the live canvas is `.size_full()` (`desktop:wallet/page.rs:12717-12731`) in a
column whose strip and toolbar are not `flex_none` (`explore/components.rs:230-236, 417-425`).
**G7/G30** gpui turns Enter on a focused element with `on_click` into a keyboard click
(gpui `div.rs:2790-2850`); the bar's `on_click(edit_address)` (`page.rs:12616, 12628-12630`) re-opens
edit mode after Enter. **W14** plus two hidden cancels: re-clicking the shown tab reloads
(`page.rs:12220-12226`), closing a background tab reloads the shown page (`:12248-12262`). **W7**
WebKit's `isLoading`/`estimatedProgress` and wry's `can_go_back/forward/go_back/go_forward` (wry
0.56.1 `lib.rs:2223-2236`) are unused; Back/Forward are JS (`webview.rs:268-278`); Back is always
enabled (`components.rs:668-672`). **W6/W8** a global System→Direct route switch
(`executor/proxy.rs:137-247, 309-333`), no PAC, scutil parsing (`:512-523`).

## RD1 — Tabs while a request is open (W14, ruling 4)

- **Decision**: hold. `browser_host::holds_navigation(view) = view.consent.is_some() || view.signing.is_some() || view.queued_signing > 0` (`core:dapp_browser.rs:453-462`; `signing` stays Some until `SigningAnswered`, `:607-612, 684-688`). Every path that navigates the one webview goes through one funnel `browser_go(Go)`. Held: other tab, +, start-page tab, Enter, favourites, Recents, Reload, "Open in a new tab", Back/Forward. Not held: closing the tab that holds the request (explicit; the core answers 4900), the column's ✕, the dApp's own navigations, leaving Explore. Held controls are drawn at the disabled opacity (`components.rs:357-360`) but still take clicks; a click brings the request's column forward (re-attaching `signing_background` if closed), shows `explore.requestOpen` in the bar's notice slot (`components.rs:459-462`) for 2.5 s, and logs `browser: navigation held`. The typed draft is kept. Re-clicking the shown tab is a no-op; closing a background tab no longer reloads.
- **Rationale**: ruling 4; the bar is the one gpui surface the native WKWebView cannot cover (`webview.rs:11-26`), so a toast would be hidden.
- **Alternatives**: one WKWebView per tab (rewrites `webview.rs`'s single-view model; too big for 082); allow and re-deliver (the core retires the document, `dapp_browser.rs:27-36`); no words (fallback if the owner declines the string).

## RD2 — The desktop wallet's own HTTP follows the system proxy (W6, W8, ruling 3)

- **Decision**: per request, from macOS: `CFNetworkCopySystemProxySettings()` (5 s cache; replaces the scutil subprocess, `proxy.rs:512-523`), `CFNetworkCopyProxiesForURL` (exceptions, simple hostnames), and for PAC `CFNetworkExecuteProxyAutoConfigurationURL/Script` on a `vela-pac` thread with its own run loop (5 s cap; results cached per (scheme, host) 5 min, failures 30 s). Result: ordered `Vec<Route>` (Direct | HttpConnect | Socks5h). Rules: no environment proxies on macOS (`VELA_DEV_PROXY` still wins in dev-fixtures builds, `proxy.rs:254-277`); Direct only when the system list says so; no process-wide state (delete `Candidate`, `Candidates`, `current`, `advance`, `REDERIVE_AFTER`, `proxy.rs:137-247`); within one request, move on only when the TCP connect to the proxy itself failed (never on a Timeout after connecting, TLS, a CONNECT 5xx or a CONNECT closed without an answer — the proxy answered for that host; see RD9); loopback direct (`is_local`, `:111-135`); a failure carries `ProxyFailure{proxy, kind}` and is logged. Linux/Windows keep their sources under the same no-fallback rule; their PAC stays unsupported (logged once). Bindings: a small `extern "C"` block over CFNetwork + `core-foundation` 0.10 / `core-foundation-sys` 0.8 (already in Cargo.lock via gpui). The module only reads system settings; it never calls networksetup or SCPreferences.
- **Rationale**: ruling 3 verbatim; `CFNetworkCopyProxiesForURL` is WebKit's own resolver, so the wallet takes WebKit's route. Per-request state removes W8's global flip and doubled timeouts (RPC 8→16 s, bundler 15→30 s). A PAC failure is a proxy failure, not a silent DIRECT (whether CFNetwork itself falls back is unverified; the log names it either way).
- **Alternatives**: `system-configuration` crate (no PAC); `objc2-cf-network` (not in the lockfile or offline cache); a second PAC JS engine (drifts from WebKit); keep Direct as last resort (ruled out).

## RD3 — WebKit's own load state first (W7, W6)

- **Decision**: a poll of the one WKWebView (250 ms while a load is watched or a panel is up, 500 ms otherwise while Explore is in front) reads `isLoading`, `estimatedProgress`, `URL` (the `view_url` pointer technique, `webview.rs:335-353`) and wry's `can_go_back/forward`, and feeds `LoadWatch::engine`. A failure is declared when the engine stops loading with no commit; the probe then only classifies (probe Ok → Other). The 3 s watchdog probe still shows the panel fast for DNS/refused/connect/timeout/proxy, but never cancels the engine's load. `retry_fired` returns `EngineStillLoading` instead of navigating while `isLoading` is true for the same URL. Give-up at 20 s only if progress never rose above `ENGINE_LIVE_PROGRESS` (0.15; WebKit starts a provisional load at 0.1 — unverified on this build). A probe TLS verdict while the engine loads is `Deferred`. Windows (WebView2) keeps probe-only.
- **Rationale**: wry has no `didFail*` (`wry_navigation_delegate.rs:49-101`), but KVO properties give the same fact. W7's second `loadRequest` at ~10 s comes from `explore/load_watch.rs:238-248` + `page.rs:13270-13281` navigating unconditionally.
- **Alternatives**: fork wry; swizzle its delegate; lengthen the probe budget (still restarts working loads).

## RD4 — The load-watch rules move to the core

- **Decision**: `LoadWatch` (pure) moves from `desktop:explore/load_watch.rs:34-287` into `core:browser_load.rs` with its tests (`load_watch.rs:360-652` minus the two ureq tests) and the RD3/RD7 extensions; ms constants. `probe()`, `probe_code_of()`, `io_code()` stay in `desktop:explore/probe.rs`. No FFI export here; the phones share the constants and `should_give_up` (RX, RE2).
- **Rationale**: the iPhone has no watchdog (W5, G28, G32); moving it now, with one user, is cheap.
- **Alternatives**: keep it desktop-only (a second copy appears with iOS's fix); a Crux machine (heavier than a per-tab value object).

## RD5 — G7, G30: the address bar after Enter

- **Decision**: the bar's `on_click` ignores `event.is_keyboard()` (gpui `interactive.rs:424`); `address_key` returns `Go | Stop | NotMine` and Go (not held) or Stop calls `window.blur()` (gpui `window.rs:2051`), which also invalidates the pending keyboard click (`div.rs:2831-2836`); `edit_address` (`page.rs:12980-13002`) starts from the address the bar shows (`load_watch.url` while loading or failed, else the webview URL); the live bar never falls back to the fixture host `app.uniswap.org` (`page.rs:12510-12513`). Host and lock come from core `address_bar` (RE1). While a failure panel is up, the shown tab's title is the failed host (drawing only).
- **Rationale**: root cause above; explains `zoom-address.png` (G7) and the empty bar in `strip-G29.png` (G30).
- **Alternatives**: `prevent_default` in the key handler (listener order inside gpui unverified).

## RD6 — G2: the restored tab, nav buttons, first Enter

- **Decision**: `shown_tab: Option<String>` = the tab whose page is in the webview (None at launch). The strip lights only `shown_tab` or a selected start-page tab; a restored tab waits unlit (iOS `pageWanted`, `BrowserController.swift:85-96, 514-519`). A visit from the meta handler (`page.rs:13122-13138`) goes to `shown_tab`, else the selected start-page tab, else opens a new tab (the waiting tab is left intact). Nav buttons take `enabled: [bool; 3]` from `webview::engine()` (all off on the start page); Back/Forward call wry's native `go_back/go_forward`. Diagnostics: `browser: navigate asked host=… view=<built|pending> composing=<b>`.
- **Rationale**: today `selected_tab` (`core:explore_sites.rs:641-648`) is drawn as "this tab" over the start page (`page.rs:1228, 12305-12312`) — exactly `d03-explore.png`.
- **Alternatives**: load the restored tab at launch (the owner's dead-127.0.0.1 incident is why the phones keep it dormant); a "tap to load" card (words).

## RD7 — W18: loads the wallet did not start

- **Decision**: the RD3 poll sees `isLoading` go true with no wallet request and a URL other than the committed one → `EngineVerdict::PageStarted` (`Asked::Page`): hairline, watchdog, probe, panel with the failed host; manual Retry only (the original may have been a POST).
- **Rationale**: same poll, no extra cost; `isLoading` tracks the main frame (to be confirmed, DX4).
- **Alternatives**: wry's navigation handler (sees subframes, URL only, cannot tell main-frame loads).

## RD8 — G6: the chrome jump

- **Decision**: the live canvas uses `.flex_1().min_h(px(0.)).w_full()`; `tab_strip_with` and `toolbar` get `.flex_none()`.
- **Rationale**: taffy's default `flex-shrink:1` shares the 94 pt overflow ≈5:7, as measured (`cmp-toolbar.png`); the start page already uses `flex_1 + min_h(0)` (`page.rs:13720-13722`).

## RD9 — "When the proxy fails, say so"

- **Decision**: new core `LoadFailureClass::Proxy` (`probe_code::PROXY = 6` from the desktop; Apple `kCFErrorDomainCFNetwork` 306–310 — HTTP/HTTPS proxy connection failure, bad proxy credentials, PAC file error/auth — unverified on device; Android -5), `reason_key = "explore.loadProxy"`, retried on the Offline schedule. **Proxy means the proxy itself could not be used**: the TCP connect to it failed or timed out, or the PAC could not be run (`ProxyFailureKind::{Unreachable, Timeout, PacFailed}`). A proxy that accepted the connection and then refused the tunnel — a 502, or a close with no answer (`RefusedTunnel`, ureq `ConnectProxyFailed`, Apple 311, chaos `drop`) — answered for that host, so the site keeps its own class (the probe's `refused`/`connect` code), exactly as RE4 treats iOS -1000. RD2's route walk follows the same line: it moves to the next route only on the first kind. Wallet-traffic surfaces keep their wording in 082; the proxy is named in the log.
- **Rationale**: one class in `browser_load::classify` (`browser_load.rs:86-144`) reaches all four clients as a string through UniFFI (`vela-core-uniffi/src/lib.rs:1609-1623`); ruling 3 says to name the proxy.
- **Alternatives**: map to `explore.loadOffline` (says nothing about the proxy); proxy words in `FeeFailure` too (touches fee_policy on four clients; deferred).

## RD10 — G1 on the desktop home

- **Decision**: rows; skeletons while `BalanceView.balance_unknown`; else `empty_state(Inbox, home.emptyNoActivity, home.emptySubtitle)`, or the core's `home_empty_key` (`home.emptyNoActivityNetwork`, no caption) when the sidebar narrows to one chain (RG5).
- **Rationale**: the keys already resolve on desktop (`wallet/mod.rs:172-173`), used only on the gallery board (`page.rs:7624-7630`); iOS draws rows/empty/loading this way (`WalletLive.swift:88-103`).
- **Alternatives**: always show the empty line (says "nothing happened" before anyone looked).

## RD11 — G11: consent shows account and network

- **Decision**: `consent_body` (`page.rs:14000-14072`) adds an account row (`consent.address`) and a changeable network row (existing `open_site_networks → SiteChainPicked`, `page.rs:14320-14343`; the core accepts it for any origin, `dapp_browser.rs:590-594`), both extracted from `connection_body` (`:14127-14285`). The connected panel shows the grant's account (`tab.connected_address`), iOS parity (`ExploreLive.swift:290-318`).
- **Rationale**: `DbrConsentView.address` / `chain_id` already exist (`dapp_browser.rs:411-420`).

## RD12 — Desktop logs (W23, FR-018/019)

- **Decision**: `desktop:diag.rs` with `vlog!(area, …)` → `[vela-wallet] HH:MM:SS.mmm area: …` (libc `localtime_r`, `Cargo.toml:330-331`) and redaction helpers `host_of(url)` (host[:port] only — RPC URLs carry API keys, `pool.rs:733-735`; the Trusted Signer fragment carries the request and token, `trusted_signer.rs:10-13, 37-41`) and `short(hash)`. Areas and lines: browser, proxy, rpc, chain notice, fee, tracker, relay, dapp, signer, window (catalogue in contracts §13).
- **Rationale**: stderr is the only desktop log and the paths under test are silent (device-pass-plan §1).
- **Alternatives**: os_log (better for field reports; needs FFI; follow-up).

## RD13 — W16: the Trusted Signer page could not open

- **Decision**: Android's rule (`TrustedSignerChannel.kt:87-145`): while a wait runs and the window becomes active with no answer (`observe_window_activation`), HEAD the page's scheme+host+path (fragment stripped) over RD2's route within 5 s; a transport failure → `mark_unreachable()` → the card shows `componentsUi.signing.signerDown` with `connect.browser.retry` primary and Cancel; the request and its 5-minute clock stay.
- **Rationale**: no new strings (iOS `TrustedSignerSheets.swift:50-66`); probing only on return keeps a cached page working.
- **Alternatives**: probe at launch (false "could not open" for a cached page).

## RD14 — W17: closing the window during a submit

- **Decision**: `SUBMITS_IN_FLIGHT: AtomicU32` with a drop guard around the relay POST (`relay.rs:578-615`); `on_window_should_close` (gpui `window.rs:5747-5756`) and the Quit action (`main.rs:401`) refuse once while in flight (activate, bring the submitting column forward, log `window: close held`); a second close within 5 s goes through. No new string.
- **Rationale**: `QuitMode::LastWindowClosed` (`main.rs:346`) quits mid-submit; RA3/RG3 cover what happens after a quit anyway.
- **Alternatives**: a confirm dialog (bytes); keep the process alive windowless (Dock reopen broken on macOS 26, `main.rs:337-346`).

## RD15 — G4 (sign-in sheet wording) is not in 082

- **Decision**: deferred. "Scan a code and create it on a nearby device" (`onboarding.create.methodHybridBody`) and "Touch ID or Windows Hello" (`methodPlatformBody`) are create-flow strings reused by desktop sign-in (`desktop:hardware.rs:190-195, 360`); a correct sign-in and per-platform wording needs new keys, and the surface is onboarding, outside the dApp browser.
- **Alternatives**: reuse `onboarding.create.providerPlatform` "Built-in passkey" as the Mac body (0 B) — acceptable as a follow-up if the owner wants it now.

---

# E — iOS browser, signing chrome, diagnostics (G28, G32/W5, G31, G8, G9, G10, G12, G24, G25, G26, W20, W23-iOS), with parity

Root causes (E): **G28** the bar host is rebuilt from `webView.url` by KVO
(`ios:Features/Explore/Core/BrowserEngine.swift:338-340`, `update()` `:360-376`), which is the
provisional URL during any provisional load (WebKit `PageLoadState::activeURL`; not re-verified on
iOS 26), while the lock follows the core's committed document (`ExploreScreen.swift:154-158`);
page-initiated navigations rename the bar the same way (`:652-656`) — a spoofing shape. The core's
origin attribution is commit-based (`:474-488`), so routing is safe. **W5/G32** `URLRequest(url:)`
(`:179`) has a 60 s idle timeout, no Stop, Retry disabled while retrying
(`Components/Explore/BrowserWebView.swift:82-83`), no network-back signal, and attempt 4 asks for
nil (`:268-287`). **G31** `-1000 → NotFound` (`core:browser_load.rs:121`). **G9/G10**
`ConnectionPanelView.swift:34-48`, `ExploreLive.swift:330-336`. **G12** `SigningAtoms.swift:61-87`.
**G25** `TabCardView.swift:40-57`, `ExploreTabsScreen.swift:49-60`. **G26** balances refresh only at
submit (`core:send.rs:5217`); only the web refreshes on confirm (`web:wallet/core/tracker-executor.ts:231-236`).
**G24** iOS reads only the native coin and custom tokens (`BalanceExecutor.swift:160-176`,
`Core/TokenReads.swift:61-130`). **W20** session-long logo misses (`RemoteLogoView.swift:46-53`).
**W23** `print` only; the report omits failures (`SettingsLive.swift:1356-1361`). Android: a fresh
tab's bar falls back to the fixture `app.uniswap.org` with an open lock (`android:feature/browser/ExploreLive.kt:195`,
`navigation/VelaNavHost.kt:1200-1203`); no stall watchdog; Retry dims while busy (`ExploreSheets.kt:621-627`).

## RE1 — G28: what the address bar names

- **Decision**: core `browser_load::address_bar(shown_url, pending_url, failed_url) -> AddressBar{url, host, lock}`: a failure panel up → the failed host, no lock; else a committed document → its host, lock `Closed`/`Open` from `!is_insecure_public_origin(origin)`; else a pending load in an empty tab → the pending host, no lock; else empty. Host from `origin_of` (non-default port kept). Shells pass the committed URL from their commit callback (iOS `didCommit` + same-origin SPA changes while nothing is pending; Android `onPageStarted` when not failed; desktop commit) and the pending URL from load/retry/back/forward/policy-allowed main-frame/`_blank`. Share, copy, favourite and the edit field use `bar.url`.
- **Rationale**: 079's L1 rule as one tested function; closes the page-initiated spoof; also fixes Android's fixture host and desktop G30. One function yields host and lock from the same URL.
- **Alternatives**: `DbrTabView.origin` (after a failed first load iOS reports the failed URL as finished, so a lock would show for nothing); a "正在打开 …" text (new string); an iOS-only fix (Android and desktop have variants).

## RE2 — W5/G32: a load watchdog on the phones

- **Decision**: timings from the core (`GIVE_UP_MS = 20_000`, shared with the desktop) and `browser_load::stalled()` (class Timeout, `explore.loadOffline`, auto-retry). The iOS and Android engines arm a timer at every `requested()`, keyed by a load generation, disarmed by commit, failure, finish, Stop or teardown. On fire, when `should_give_up(elapsed, committed, progress)` holds (iOS `estimatedProgress`, Android `WebView.getProgress()`), the engine calls `stopLoading()`, shows `stalled()` with the pending host, and retries on the 2/5/10 s schedule while in front. The -999 from `stopLoading` is a non-failure that keeps the panel (`BrowserEngine.swift:613-619`; a test pins it). iOS drops the timer when inactive and re-arms with the full budget on return.
- **Rationale**: nothing looks frozen for a minute; 20 s without a response covers a 6–10 s proxy. Same class, words and schedule people already see, 0 B. The phones need no probe: WebKit and Chromium report their own failures.
- **Alternatives**: `URLRequest(timeoutInterval: 20)` (API loads only, idle timer, misses page-initiated loads); the whole `LoadWatch` as a UniFFI object (engines' callbacks differ; the shared decisions move now).

## RE3 — W5: recovery when the network returns

- **Decision**: core `browser_load::retry_when_network_returns(class)` (true for offline, timeout, refused, other, proxy) and pure `app::net_health` (`MISSES_BEFORE_OFFLINE = 3`, `net_health_step`, Android `core/net/NetHealth.kt:18-47` moved). On `CameBack` a shell resets failed engines' attempt counts, retries the tab in front, clears transient logo misses (RE10) and forces a balance read (Android already, `VelaNavHost.kt:530-536`). iOS feeds `net_health_step` from `RpcPool.call` outcomes (unthrottled `.failed` = unreached; any answer = reached) and adds an NWPathMonitor unsatisfied→satisfied edge (500 ms debounce).
- **Rationale**: a hanging proxy node leaves the path "satisfied", so NWPathMonitor alone never fires in the China case; under ruling 6 per-app faults are visible only through call outcomes.
- **Alternatives**: NWPathMonitor only; endless slow retries (FR-003).

## RE4 — G31: Apple -1000

- **Decision**: -1000 moves to Offline (words "网络不稳定，页面没能打开。", retries 2/5/10 s); -1002/-1003/-1006 stay NotFound.
- **Rationale**: every observed -1000 was the per-app proxy refusing CONNECT; the Wi-Fi proxy gave -1004/-1005 for the same shape (SC-006). Vela cannot hand WebKit a malformed URL (`dappBrowserInput` + `URL(string:)`).
- **Alternatives**: Other (generic words, one retry); keep NotFound (false accusation, no retry).

## RE5 — W5: Stop, and a busy Retry

- **Decision**: while loading, the site menu's refresh row becomes Stop (`connect.dapp.stop`, icon close): cancels the pending load and the watchdog, keeps the committed page and bar, no panel. The failure panel's Retry is `VelaButton(loading: retrying, busyTitle: explore.loadRetrying)` — busy, full colour, taps ignored. Android's `BrowserNotice` gets the same busy look.
- **Rationale**: house button rule (busy ≠ disabled); 0 B (the key exists, no reader today). If a copy-parity ruler objects to the namespace, fall back to a new `explore.stop` (~18 B).
- **Alternatives**: a Stop in the bar (no free slot).

## RE6 — G9/G10: the consent sheet

- **Decision**: while a site asks: one header row (avatar + "连接到 {host}" wrapping, lock line, ✕), one sentence `connect.browser.body` above the buttons, no footnote. The connected panel is unchanged (`explore.connectionExplainer` above Disconnect, `explore.autoRequestHint` below). Android merges its title into the row the same way.
- **Rationale**: names the site once (FR-009); the desktop's shape (`page.rs:14000-14040`).
- **Alternatives**: drop the title, keep the host row (079 ruled the title names the asker).

## RE7 — G8 (+ F14): a name that is its host is said once

- **Decision**: core `browser_load::site_label(title, host) -> SiteLabel{name, host_line}`: title empty or equal to the host ignoring ASCII case → `{host, None}`; else `{title, Some(host)}`. Used by Recents on iOS, Android, desktop, and replaces 079's five F14 copies (iOS `SigningAtoms.swift:66-72`, Android `SigningComponents.kt:117`, `SigningLive.kt:254`, desktop `signing/components.rs:130`, web `signing/live.ts:898`). The signer page's L-HOST rule (RG11) uses the same wording in JS.
- **Rationale**: one rule written five times; the spoof test (`desktop:explore/live.rs:253-262`) still holds.

## RE8 — G26: the balance after the app's own transaction

- **Decision**: `tx_tracker` emits new `TrackOperation::HoldingsMoved{chain_id}` (answered `Notified`) with `NotifyConfirmed` on a confirmed receipt, and alone on a failed receipt that carries a tx hash (gas was spent). Shells force a balance read (iOS `wallet.refresh(pull:false)`, Android `refresh(force=true)`, desktop `balance_dashboard::invalidate()`, web `ports.confirmed(chainId)` moved from `notify_confirmed`). A `NotSent` op (no tx hash) never emits it.
- **Rationale**: submit-time invalidation reads before landing (`send.rs:5217`); dApp ops reach the tracker too (`RootView.swift:1644-1647`).
- **Alternatives**: document `NotifyConfirmed` as "refresh" and copy the web's mapping (decision stays in shells; misses failed receipts).

## RE9 — G24: which tokens a balance read covers

- **Decision**: pure `balance_dashboard::read_plan(chain_id, stables, wrapped_native, custom) -> Vec<ReadSlot>`: native (unless the chain has none, e.g. Tempo 4217), registry stablecoins (decimals read on chain, `peg_usd = 1.0`), the wrapped native unless `wrapped_native_is_the_native`, custom tokens; dedupe by lower-case contract, custom metadata wins. iOS adopts it; desktop (`executor/balances.rs:338-400, 595`), Android (`BalanceExecutor.kt:260-300, 452`) and web (`services/wallet-api.ts`) delete their copies.
- **Rationale**: three clients agree already; iOS alone never counted stablecoins (USDC on Base). The iOS hero total will rise on first run — correct, but visible.
- **Alternatives**: port Android's list to Swift (a fourth copy).

## RE10 — W20: how long a failed logo stays failed

- **Decision**: pure `app::remote_mark`: `mark_miss_of_status(u16)`, `mark_miss_ttl_ms(kind)`: NotFound (404/410), Refused (401/403), NotAnImage → session; Throttled, ServerError (5xx/408), Transport, Unknown → 60 000 ms; transient misses also clear on `CameBack`. iOS `LogoStore` keeps expiries and bumps an observable epoch; Android `RemoteLogo.kt` follows; web `<img onerror>` has no status → Unknown.
- **Alternatives**: no miss memory (one 404 per row per frame).

## RE11 — W23: iOS logs and the bug report

- **Decision**: `ios:Core/VelaLog.swift` on `os.Logger` (subsystem `app.getvela.VelaWallet`; categories browser, sign, relay, rpc, fee, tracker, balance, net; `.notice` events and `.error` failures, both kept by `log collect`). Fields public: kinds, classes, chain ids, NSError domain+code, short hashes (10 hex), durations. dApp host public in DEBUG, an FNV token in Release. Never addresses, full URLs, calldata, signatures, credentials. A ring of the last 8 "scope: kind" failures feeds the report's existing "Recent failures" line (`componentsUi.bugReport.previewFailures`) through `SettingsLive.redact`.
- **Rationale**: FR-018/019; `print` reaches only a `devicectl --console` session, which cancels Face ID sheets (050/052), so signing rows had no trace.
- **Alternatives**: `print` + console capture; public hosts in Release (browsing history in the system log).

## RE12 — G25: tab switcher cards

- **Decision**: one skeleton per cell: a preview sized by `Color.clear.aspectRatio(tabCardAspect, .fit)` with content in an overlay, the snapshot top-cropped; a tab with no snapshot shows its avatar (48 pt) and host, not fake page bars; the new-tab tile has the same preview and a caption row `explore.newTab`; the grid is top-aligned. Android `ExploreTabs.kt` gets the same.
- **Alternatives**: persist snapshots to disk (dApp pixels with balances on disk).

## RE13 — G12: the signing header at 375 pt

- **Decision**: the name/host column gets `.layoutPriority(1)`, name and host `lineLimit(2)` without truncation, the chain chip `.fixedSize()`. Android: `maxLines = 2`, no ellipsis (`SigningComponents.kt:107-115`).
- **Alternatives**: middle truncation (hides the part a spoofer controls).

## RE14 — Device verification of iOS under ruling 6

- **Decision**: every iPhone fault row runs on a Debug build launched with `"VELA_DEV_PROXY":"192.168.50.9:8899"` in the devicectl JSON (per-app `ProxyConfiguration`, `ios:Core/DevProxy.swift:74-98`). No Wi-Fi proxy, no Shadowrocket, no airplane mode: IX4 becomes `drop match=''` then `pass`; IX3/IX5 become per-host drop/blackhole/reset_mid. The NWPathMonitor edge is proven by a unit test with a path seam; a hermetic XCUITest uses an in-process never-answering listener.
- **Alternatives**: one owner-approved Wi-Fi toggle row (offered as optional, not assumed).

---

# F — Chain reads that hang (G20, G33, W11, W12, W10)

No research pass covered these; the plan writer read the lines below. Findings: the extension's
worker walks the chain's endpoints in order with a fixed 20 s timeout each
(`ext:background.js:62` `READ_TIMEOUT_MS = 20_000`, `forwardRead` `:414-456`) and remembers nothing
between calls, so a dead Gnosis costs 3 × 20 s on every read (G20: 45–57 s) and ends with Chrome's
raw `Failed to fetch` in the message (`:452-455`, G33). The core pool marks a chain failed only when
every endpoint failed every pass (`core:rpc_pool.rs:1622-1661`: "Pass swept clean" →
`conclude_failed` → `failed_chains.insert`, `:1644`), i.e. up to 3 passes × endpoints × 8 s
(`RPC_READ_TIMEOUT_MS`, `:161`) — the iPhone's missing notice at 60 s (G33). The extension hands a
dApp an op hash that its receipt reads then send to public nodes (W12, device-pass-plan §3).

## RF1 — The chain notice appears after one full pass, not three (G33, W11)

- **Decision**: `rpc_pool` gains a view set `unreached_chains`: an RPC call whose first pass tried every endpoint of the chain and saw only transport failures with no rate-limit signal adds its chain at "Pass swept clean" (`rpc_pool.rs:1622`); any usable answer removes it (`clear_chain_failure`, `:1841-1846`), and the call's conclusion hands over to `failed_chains` as today. The notice rule (079 data-model) becomes `chain ∈ (failed_chains ∪ unreached_chains) ∧ chain ∉ rate_limited_chains`. The home "fix your RPC" banner keeps reading `failed_chains` only.
- **Rationale**: FR-004 asks for a notice while the dApp still waits; a first-pass failure bounds it to endpoints × 8 s (Gnosis ≈ 24 s; desktop no longer doubles it after RD2). Keeping the banner on `failed_chains` avoids a more eager banner on the home screen.
- **Alternatives**: mark `failed_chains` early (changes the banner's meaning and the ported "only the final pass classifies" invariant, `:847`, `:1475`); a timer in each shell (four copies).

## RF2 — The extension's reads: shorter waits, memory of dead nodes, plain words (G20, G33)

- **Decision**: the worker's per-endpoint timeout becomes the core's `RPC_READ_TIMEOUT_MS` (8 s) and a failed endpoint enters a cooldown of `30 s · 2^(n−1)`, capped at 300 s (the core's `COOLDOWN_BASE_MS`/`COOLDOWN_CAP_MS`, `rpc_pool.rs:170-171`), kept in storage.session `vela.ext.endpoints`; endpoints in cooldown are tried last, and a success clears the entry. Both numbers are pinned by vitest to new wasm exports `rpcReadTimeoutMs()` / `rpcCooldownMs(n)`. The final error is `-32603 "Vela could not reach a node for chain <name> (<id>)"` with no engine text. RB14's `read.*` lines log each step. The extension has no chrome of its own over a dApp tab, so there is no notice surface (079's matrix already records F6 "—" for the extension).
- **Rationale**: a dead chain now costs ≤ 3 × 8 s once, then the cooled endpoints are skipped until a live one answers; the first call after recovery no longer pays for the dead ones.
- **Alternatives**: load the core pool in the worker (cost, RB2); race all endpoints at once (triples public-node load); a content-script banner on the dApp's page (intrusive, spoofable).

## RF3 — The extension translates receipt reads for op hashes it handed out (W12)

- **Decision**: when the panel answers an on-chain request with an op hash (ReceiptPending or MaybeSent, RA2), its `answer` message carries `opHash: {chainId}`; the worker records it in storage.session `vela.ext.op.<hash>` (24 h). `forwardRead` for `eth_getTransactionReceipt` / `eth_getTransactionByHash` with a recorded hash first asks the chain's bundler `eth_getUserOperationReceipt`; with a receipt it forwards the same method for the real `transactionHash`, else answers `null` — the web's rule (`web:services/dapp-submit.ts:987-1008`) and the core's for in-app browsers (`core:dapp_browser.rs:1482-1497`). A small pure helper `ext:lib/op-receipt.js` holds the rule, unit-tested on the web's vectors.
- **Rationale**: without it, an extension dApp answered with an op hash polls public nodes forever and says "failed" while the op lands — the resubmission risk A's answer rule depends on avoiding.
- **Alternatives**: answer only real tx hashes (impossible for ReceiptPending/MaybeSent); record every 32-byte answer (a real tx hash would then never be forwarded).

## RF4 — The chain notice's Retry shows it is working (W11)

- **Decision**: desktop and iOS chain-notice Retry use the house busy state (spinner, full colour, taps ignored) until the one `eth_blockNumber` it sends settles; success clears the notice, failure leaves it.
- **Rationale**: device-pass-plan W11: Retry "looks dead" — desktop `retry_chain` fires one `eth_blockNumber` and redraws only after it (`desktop:wallet/browser_host.rs:367-382`, called from `wallet/page.rs:12817`); iOS's Retry is a plain text button (`ios:Features/Explore/ExploreScreen.swift:202-210`). No words.

## RF5 — W10: iOS fee row while the account's deployment cannot be read

- **Decision**: when the deployed-state read fails, the iOS sheet feeds the core the same quote failure the send flow uses, so the row shows the core's reason (`componentsUi.funding.denialNetworkError`) and re-quotes on `fee_policy::requote_delay_ms` (079 R8); a refresh tap cancels the running loop before starting one (`ios:Features/Signing/Core/SigningController.swift:386-401, 432-438`). The exact event name is to be confirmed at implementation.
- **Rationale**: "估算中…" with no reason reads as a hung wallet whenever public RPCs are blocked (IX6).

## RF6 — Not changed in 082

- **W9** (TLS reset -1200 classed Certificate): unverifiable under ruling 6 (it needs a Wi-Fi-wide proxy client); kept as is, logged by RE11, revisited if a chaos `tls_reset` mode is ever added.
- **W21** (address-bar search goes to DuckDuckGo, `core:dapp_rpc.rs:486`): a product choice, not a stability fix; recorded for the owner.
- **W22** (desktop hairline until `didFinish` while trackers hang): cosmetic; RD3's `engine_live` stops it from turning into a false timeout, which was the harmful part.

---

# G — 079 leftovers (L-D3, L-D5, L-D6, L-D7, L-HOST, L-PANEL, L-D2/L-D4, L-SC)

Root causes (G): **L-D3** the feed drops DappTx in the record filter
(`core:activity_feed.rs:633-640`) and the row builder (`:811-832`), and `FeedItem` carries no kind or
status, so shells guess (desktop `wallet/live.rs:1935-1950`, web `wallet/live.ts:430-435`, Android
`FlowLive.kt:265` can never say Failed). Android never re-reads the feed after a record write
(`VelaWalletApplication.kt:691`, `WalletController.kt:722`). **L-D5** three shell parsers collapse
every non-answer to "unavailable" (danger on iOS/desktop, caution on Android) and skip reverts
(desktop `executor/sim.rs:88-94`, iOS `SimDeltas.swift:84-86`); `-32603` on `eth_simulateV1` counts
as transient and can mark Arbitrum failed (`rpc_pool.rs:627-641, 1644`). **L-D6** `inpage.js`
lower-cases (`rust/crates/vela-core/provider/inpage.js:162-170, 337, 411`). **L-D7** web/iOS/Android
copy the fixture's `history.emptyFilter`. **L-HOST** `signer:render.js:490` draws name and host with
no equality check. **L-PANEL** the helper looks for `/request.html` (`e2e/extension-helpers.ts:106-145`).

## RG1 — L-D3: the core decides everything about a row

- **Decision**: `FeedItem` gains `kind: FeedTxKind` (send/receive/dapp_tx; a folded batch is send), `status: FeedTxStatus` (the record's; a batch its first line's), `site: Option<String>` (dapp_tx only); `FeedTxRecord` gains `#[serde(default)] dapp_origin`. `accept()` keeps DappTx where `from == me`; `build_items()` gets a DappTx arm (`dapp_item()`); message signatures and connects stay out. Shells delete their derivations (desktop `kind_of` + status lookup `flows/live.rs:404`; web `feedItemStatus` `wallet/live-detail.ts:61-69`; iOS record lookup `FlowsLive.swift:261`; Android tx-hash heuristic).
- **Rationale**: FR-020; four derivations today, one of which cannot say Failed.
- **Alternatives**: widen the filter and keep shell lookups (four copies, batch-status bug); a separate dApp feed (duplicates grouping and dedupe).

## RG2 — L-D3: what a dApp row says

- **Decision**: title `history.txLabelDappTx`; subtitle `site` verbatim (host[:port]), else the recipient's name or short address, else the chain name; amount only when native value > 0 (`fee_policy::from_base_units` of hex or decimal wei); every row not confirmed is prefixed with `componentsTx.detail.statusPending` / `statusFailed` + " · "; the detail sheet adds a "Requested by" fact (`componentsUi.signing.siweOrigin`). No new strings.
- **Rationale**: US3 (pending, then confirmed or failed, with the site).
- **Alternatives**: status only in the detail chip (fails US3 AS1); a "from {{site}}" string (bytes).

## RG3 — L-D3: the row appears within 5 s

- **Decision**: after `PersistRecord`/`UpdateRecord` every shell dispatches the feed's existing `ReconcileCompleted{resolved_count: 1}`. iOS already does (`RootView.swift:1648` → `ActivityStore.swift:166`); Android `recordsPersisted()` passes 1 (`VelaWalletApplication.kt:691`); desktop hands the count over on the next tick after the background persist (the `tracker.rs:247-256` pattern), never from the tracker handoff; web `persist_record` (`sign-executor.ts:270-293`) calls `feedReconciled(1)` after the save.
- **Rationale**: otherwise the row waits for a 10–30 s tick, or never on Android.
- **Alternatives**: a new `FeedEvent::RecordsWritten` (wire churn on four clients).

## RG4 — L-D3: a MaybeSent op in Activity

- **Decision**: through RA3: `OpSubmitted{maybe_sent: true}` persists a Pending `SignRecord` under the local hash and hands it to the tracker; the row reads "处理中 · <site>" until the tracker patches it (Confirmed, failed on Dropped/NotSent, or untouched at the 24 h abandon).
- **Rationale**: one persistence path for "a hash and no receipt"; the tracker is the only closer.
- **Alternatives**: write-ahead before the POST (a definite refusal would then need a delete).

## RG5 — L-D7: empty History and empty home Activity, chosen by the core

- **Decision**: `FeedView.history_empty_key` = `history.emptyTitle` when `chain_filter` is None, `history.emptyFilter` when Some; `FeedView.home_empty_key` = `home.emptyNoActivity` / `home.emptyNoActivityNetwork` the same way (RX). All four shells already dispatch `ChainFilterChanged` (web `feed.svelte.ts:81`, iOS `ActivityStore.swift:174`, Android `WalletController.kt:727`, desktop `page.rs:4162`). Loading vs empty stays with the shells. Desktop's own branch (`flows/live.rs:219-222`) goes.
- **Rationale**: FR-020 names empty-state choice; the 079 `reason_key` pattern.
- **Alternatives**: fix each shell's builder (a fourth copy).

## RG6 — L-D5: the simulation classifier

- **Decision**: new pure `app::sim_outcome`: `classify(reply, user) -> SimOutcome` — pool no answer → Unreachable; a JSON-RPC error (-32601, -32602, -32603 "method handler crashed", other) or a result that is not a non-empty array of blocks with calls → NotOffered; any call with status 0x0 or an error → `Reverts{reason}`; else `Deltas{derive_deltas(logs, user)}` (empty = checked, nothing moves). `notice(outcome)`: Reverts → Danger `simWillFailReason`/`simWillFail`; NotOffered/Unreachable → Caution `simUnavailableWarning`; Deltas → none. `derive_deltas` and helpers move from `desktop:executor/sim.rs:98-226`; iOS `SimDeltas.swift:76-176` and Android `SimDeltas.kt` parsers go. UniFFI `sim_outcome(user, reply_json)`; no wasm (the web sheet runs no simulation, `setApproveSim` has no caller).
- **Rationale**: FR-012; severity differs today by client (iOS `SigningLive.swift:613-614`, desktop `signing/live.rs:264-268`, Android `SigningLive.kt:757`) and a real revert reads "no change".
- **Alternatives**: change tones in three shells (keeps the revert blind spot); classify inside `token_trust` (bigger machine change).

## RG7 — L-D5: `eth_simulateV1` in the pool

- **Decision**: `rpc_pool::OPTIONAL_METHODS = ["eth_simulateV1"]`: a JSON error to an optional method → new `Route::NotServed` (conclude `Respond{url}`, `clear_chain_failure`, no score change, no ban), except a rate-limit signal still fails over; `conclude_failed` never classifies a chain for an optional method.
- **Rationale**: today a node that cannot simulate can raise the chain notice and banner for Arbitrum (`rpc_pool.rs:1644`), and plan words on it ban the endpoint for every method (`:600-622`).
- **Alternatives**: keep failing over (endpoints × 8 s × passes before the sheet speaks).

## RG8 — L-D5: a revert reason is untrusted text

- **Decision**: `sim_outcome::revert_reason` decodes only `Error(string)` (0x08c379a0), strips what `name_verify::is_never_in_a_name` rejects (`name_verify.rs:186-197`), caps at 64 characters; panics and custom errors → None → plain `simWillFail`.
- **Rationale**: the contract the dApp chose writes it, and it is drawn on a signing sheet (supply-chain signing boundary).

## RG9 — L-D5: the warning's words

- **Decision**: reword `componentsUi.signing.simUnavailableWarning` in 15 locales to "Vela couldn’t check what this transaction does. Review it before you sign." (zh "Vela 未能检查这笔交易的结果，请核对后再签名。"), caution tone; reverts reuse `simWillFail`/`simWillFailReason`. All 15 drafts are in RI2.
- **Rationale**: FR-012; the old text says "(the RPC refused)", false for Unreachable; saves 191 B.
- **Alternatives**: two keys (NotOffered vs Unreachable) — a difference no one can act on.

## RG10 — L-D6: one address spelling

- **Decision**: `inpage.js` `applyAccounts` keeps the wallet's spelling and compares case-insensitively; core `dapp_permissions::dapp_spelling(addr)` (= `primitives::checksum_address`, input unchanged if unparseable, `primitives.rs:96`) at `popup_approved` (`dapp_permissions.rs:478-521`), `popup_account_switch` (`:526-568`), `dapp_browser` `AccountSwitched` (`:493-496`) and `sites_listed` (`:785-789`). `resolve_granted` (`:574-598`) is unchanged (its JS twin `protocol.js:343` cannot checksum; `instant.test.ts:42`). The extension rewrites old lower-case grants once at wallet boot (`normalizeGrantSpelling()` in `web:dapp/follow.ts`, via wasm `checksumAddress`); the worker's case-insensitive compare (`background.js:497-506`) keeps that silent.
- **Rationale**: FR-014/SC-006; the lower-casing is in `inpage.js` alone (evidence `p30-dapp-log.png`).
- **Alternatives**: checksum inside `resolve_granted` (breaks the twin, or keccak in the MV3 worker).

## RG11 — L-HOST: the signing page names the site once

- **Decision**: `signer:resolve.js` `baseView` (`:169-212`) adds `originShown: !!host && host !== ((known && known.name) || '')`; the ceremony view (`:1034-1036`) sets it too; `render.js` draws the host line only when set (`:448-449, 490`). `view.dapp.origin` stays for `warn.claimedOrigin` (`resolve.js:1364-1365`). Released by HANDOVER.md steps: new hash first in `BUILD_ALLOWED`, owner deploys from the 082 tree, `curl -I …/b/<hash>/sign` 200 immutable, then `LAUNCH` moves (`trusted_signer/integrity.rs:135`).
- **Rationale**: the core sets `context.dapp.name = host` for browser requests (`trusted_signer.rs:343-353`); the judgement belongs in resolve, not render. The apps' rule is RE7's `site_label`; the page mirrors it (exact host equality after case-folding).
- **Alternatives**: compare in render.js (a judgement in drawing).

## RG12 — L-PANEL: test the panel the product uses

- **Decision**: the helper finds the view with `pathname.endsWith('/wallet.html') && search.has('panel')` (`ext:panel.js:17-20`), reads the dialog's `aria-label` (`BottomSheet.svelte:534-536`), clicks inside the dialog only; `sidePanelOpen` becomes `sidePanelShowsRequest`; the assertion at `extension-live-provider.e2e.ts:339-341` becomes "the card goes, the panel stays". New test: a MutationObserver in the panel sees `.landing-over [data-testid="dapp-receipt"]` titled "Signed" and gone within 5 s (tick 1.4 s, `web:signing/dapp-receipt.ts:200`).
- **Rationale**: FR-017, SC-009; the helper fails on main too. Whether fixture keys sign inside the panel without a CDP authenticator is unverified (window-mode SC-304 does).

## RG13 — L-D2 / L-D4: hand-offs

- **Decision**: L-D4 no wallet change (ruling 2): the floor stays in `fee_policy.rs:811-868`; hand-off note for the relay repo; re-check on the device that the fee row shows coin + fiat before signing. L-D2(a) Arbitrum in-band never mined: relay executor, hand-off with 079 D2 evidence. L-D2(b) the status method: RA7. Ask the relay owner for the `eth_` alias.

## RG14 — i18n for workstream G

- **Decision**: no new keys; the L-D5 reword saves 191 B; `home.emptyNoActivityNetwork` is kept (RX).

## RG15 — Fault rows for G run through the per-app switch only

- **Decision**: iPhone/desktop `VELA_DEV_PROXY`, extension a separate Chrome with `--proxy-server`; never the Mac's `networksetup` or the iPhone's Wi-Fi proxy that device-pass-plan §1 (step 4, line 109) still describes. See RH1.

---

# H — How 082 is verified (ruling 6 and the device pass)

## RH1 — Faults touch only the app under test

- **Decision**: desktop fault rows run a **dev-fixtures** build (`cargo build --features dev-fixtures`, `app-desktop/vela-wallet/Cargo.toml:20`) launched with `VELA_DEV_PROXY=127.0.0.1:8899` and a scratch `VELA_STATE_DIR` (`desktop:executor/proxy.rs:249-293` makes it the one route, `scripts/device/chaos-proxy.py` header); iPhone fault rows run a **Debug** build with `"VELA_DEV_PROXY":"192.168.50.9:8899"` in the devicectl JSON (`ios:Core/DevProxy.swift:67-98`); extension fault rows run **Chrome for Testing** in its own `--user-data-dir` with `--proxy-server=http://127.0.0.1:8899`. Never `networksetup`, the iPhone Wi-Fi proxy, Shadowrocket toggles, airplane mode, or `adb shell settings put global http_proxy`. device-pass-plan.md §1 step 4, §2 "Switch the Mac to chaos"/DX3, IX3, IX4, IX5 and the chaos-proxy.py header lines for Android/iPhone are superseded by quickstart.md.
- **Rationale**: ruling 6 and this run's request.

## RH2 — chaos-proxy modes

- **Decision**: `mute` (committed, `bea04b57`) reproduces the lost reply (G21); `drop` a refusal before any byte; `blackhole` a hanging CONNECT; `reset_mid` a cut mid-exchange. Add an optional `stall` mode (CONNECT answered 200, upstream never opened) for the TUN-shaped hang the probe never covered (W24). No TLS-reset mode in 082 (RF6).

## RH3 — Android without a per-app switch

- **Decision**: no Android fault rows (a device-wide `http_proxy` breaks ruling 6, and a debug per-app proxy — androidx.webkit `ProxyController` + OkHttp `.proxy()` — does not exist yet). FR-020 parity is covered by JVM tests plus an optional no-fault smoke on the Xiaomi (quickstart §5).

## RH4 — Hermetic iOS probe

- **Decision**: `DappBrowserStabilityProbeTests` gains an in-process loopback listener that accepts and never answers (the stall row without any proxy); run via the copied-`.xctestrun` recipe (UI Automation ON, trusted developer certificate).

## RH5 — Extension e2e

- **Decision**: new `e2e/extension-lifecycle.e2e.ts` (reload during a sheet, second tab, panel ✕, worker stop via CDP `Target.closeTarget` on the SW target, panel identity after Settings) plus RG12's helper fix and signed-tick test, on the suite's own preview port (concurrent sessions share ports).

## RH6 — Parallel space first, the owner's passkey last

- **Decision**: fault and lifecycle rows run in the parallel space (fixed keyset; desktop `VELA_PARALLEL_SPACE=1` in a dev-fixtures build, `desktop:parallel_space.rs:15-50`; iPhone `"VELA_PARALLEL_SPACE":"1"`; CfT parallel profile). One short owner batch at the end covers what fixed keys cannot: Touch ID on the `/Applications` build, the owner's own Chrome profile after an extension reload, Face ID on the iPhone.

## RH7 — Logs are evidence

- **Decision**: every failure row names the log line it must produce (FR-018), and the collected logs pass one secret scan: `rg -n '0x[0-9a-fA-F]{130,}|/v3/[0-9a-f]{20,}|#[A-Za-z0-9_-]{40,}|signature=0x|privateKey|mnemonic|seed'` finds nothing (FR-019).

---

# I18N — the budget

## RI1 — How residency is measured

- **Decision**: the binding figure is `runtime_json_catalogs_fit_the_same_budget` (`tests/i18n_residency.rs:213-274`): en and ja catalogs built the web's way; bytes = every UTF-8 value + 4 × (N_PATHS + 1) offsets + 2 bitmaps of ⌈N/8⌉, per catalog. N_PATHS = 1782 (`i18n/paths.rs:1818`; pins at `scripts/gen-i18n.mjs:452-454`). Today: en 56,230 + ja 82,520 = **138,750** of `SC005_BUDGET = 138_800` (`:48`). One key costs en + ja + 8 bytes; the bitmaps grow only past N = 1784. Command: `cd rust && cargo test -p vela-core --features i18n-all,crux --test i18n_residency -- --nocapture`.

## RI2 — The ledger (no cap raise)

| Change | Key | en / zh (source) | Bytes (en+ja+8) |
|---|---|---|---|
| reword | `componentsUi.signing.simUnavailableWarning` | "Vela couldn’t check what this transaction does. Review it before you sign." / "Vela 未能检查这笔交易的结果，请核对后再签名。" | −191 |
| delete | `send.txErrorTimeout` (no consumer; `paths.rs:1510` only) | — | −213 |
| add | `componentsUi.signing.maybeSent` | "It may have been sent. Vela keeps checking — don't send it again." / "可能已经发出。Vela 会继续查看，请不要重复发送。" | +185 |
| add | `explore.loadProxy` | "Your proxy isn't responding." / "代理没有响应。" | +78 |
| add | `explore.requestOpen` | "Finish or cancel the request first" / "请先完成或取消这个请求" | +120 |
| **net** | | | **−21 → ≈138,729 (71 B headroom)**; N_PATHS 1782 → 1784, bitmaps unchanged |

Drafts, `maybeSent`: zh-TW 可能已經送出。Vela 會繼續查看，請不要重複送出。 · zh-HK 可能已經發出咗。Vela 會繼續睇住，唔好重複發送。 · ja 送信された可能性があります。Vela が確認を続けます。再送信しないでください。 · ko 이미 전송되었을 수 있습니다. Vela가 계속 확인합니다. 다시 보내지 마세요. · de Möglicherweise schon gesendet. Vela prüft weiter – bitte nicht erneut senden. · fr Elle a peut-être été envoyée. Vela continue de vérifier — ne l’envoyez pas à nouveau. · es-MX Es posible que ya se haya enviado. Vela sigue revisando; no la envíes otra vez. · pt-BR Talvez já tenha sido enviada. O Vela continua verificando — não envie de novo. · it Potrebbe essere già stata inviata. Vela continua a controllare: non inviarla di nuovo. · ru Возможно, уже отправлено. Vela продолжает проверять — не отправляйте повторно. · tr Gönderilmiş olabilir. Vela kontrol ediyor — tekrar göndermeyin. · vi Có thể đã được gửi. Vela vẫn đang theo dõi — đừng gửi lại. · id Mungkin sudah terkirim. Vela terus memeriksa — jangan kirim ulang.

Drafts, `simUnavailableWarning`: zh-TW Vela 未能檢查這筆交易的結果，請核對後再簽名。 · zh-HK Vela 查唔到呢筆交易會做乜，請核對清楚先簽。 · ja Vela はこの取引の結果を確認できませんでした。内容を確かめてから署名してください。 · ko Vela가 이 거래의 결과를 확인하지 못했습니다. 내용을 확인한 뒤 서명하세요. · de Vela konnte nicht prüfen, was diese Transaktion bewirkt – bitte vor dem Signieren kontrollieren. · es-MX Vela no pudo comprobar qué hace esta transacción. Revísala antes de firmar. · fr Vela n’a pas pu vérifier ce que fait cette transaction. Relisez-la avant de signer. · id Vela tidak bisa memeriksa hasil transaksi ini. Periksa dulu sebelum menandatangani. · it Vela non ha potuto verificare cosa fa questa transazione. Controllala prima di firmare. · pt-BR A Vela não conseguiu verificar o que esta transação faz. Revise antes de assinar. · ru Vela не удалось проверить, что сделает эта транзакция. Проверьте её перед подписью. · tr Vela bu işlemin ne yapacağını denetleyemedi. İmzalamadan önce gözden geçirin. · vi Vela không kiểm tra được giao dịch này sẽ làm gì. Hãy xem kỹ trước khi ký.

Drafts, `explore.loadProxy` and `explore.requestOpen` (ja): プロキシが応答していません。 / 先にこのリクエストを完了するかキャンセルしてください. The other 12 locales are written at implementation with the same meaning.

Reused, not new (0 B): `send.txPreparing`, `send.txSigning`, `send.txSubmitting`, `send.txBackgroundHint`, `send.txCloseBackground`, `send.txErrorGeneric`, `componentsUi.signing.{signing, stillConfirming, simWillFail, simWillFailReason, intentSend, recipientLabel, confirmSend, confirmLabel, signerDown, siweOrigin}`, `componentsTx.receipt.{statusFailed, failedHint}`, `componentsTx.detail.{statusPending, statusFailed}`, `history.{txLabelDappTx, emptyTitle, emptyFilter}`, `home.{emptyNoActivity, emptySubtitle, emptyNoActivityNetwork}`, `assets.{emptyTitle, emptySubtext}`, `connect.browser.{title, body, connect, cancel, preparing, retry}`, `connect.dapp.stop`, `explore.{loadOffline, loadRetrying, newTab, network, connectionExplainer, autoRequestHint}`, `componentsUi.bugReport.{previewFailures, previewNone}`, `componentsUi.funding.denialNetworkError`.

- **Rationale**: every fix reuses first; the three additions are the ones ruling 1 and ruling 3 ask for in words, plus the hold hint (RD1). The deletions carry no reader.
- **Alternatives**: raise `SC005_BUDGET` (owner); `maybeSent` as a short title (+65 B, see RX); drop `explore.requestOpen` (headroom ≈191 B, plan Q3).

## RI3 — How the corpus change lands

- **Decision**: all corpus edits in **one commit** (adding `maybeSent` alone would reach 138,935 and fail). The path pins in `scripts/gen-i18n.mjs:452-454` move from 1782 paths / 1693 leaves to **1784 / 1695** (89 branches unchanged: every key lands in an existing namespace), with a history comment in the `:436-451` style. Order: `npm --prefix scripts run gen:i18n` → `lint:i18n` → `verify:i18n` → `dump:vectors` → `build:wasm` + `sync:wasm` (fingerprint moves) → `cargo test -p vela-core --features i18n-all,crux` (residency printed) → `check-ios-core-fresh.sh`. The CI `git diff --exit-code` over `paths.rs`, `i18n_catalogs`, `assets/i18n` means the generated files ship with the corpus change. Every new key gets a reader on every client that shows it (desktop `SigningStrings`/`ExploreStrings`; web `engine.server.ts` + `signing/messages.ts`; iOS/Android key constants) and a resolve-without-echo test (the desktop `signing/mod.rs:560-590` pattern).

---

# J — Round 2: the post-fix device pass (2026-09-29)

The desktop and extension were re-run at `fb8c7026` (`evidence/desktop/post-results.md`,
`evidence/extension/post-results.md`) and audited adversarially (`post-audit.md` in both
folders). Findings G34–G73 are in spec.md "Post-fix device pass (2026-09-29)"; the tasks are
tasks.md Phase 9 (T185–T254). RJ1–RJ7 are the orchestrator's decisions, recorded verbatim in
substance; RJ8 onward are the planner's. Every suspected-code line the auditors named was
re-read at `47a20343`; corrections are noted where the auditor was wrong. Nothing here was built.

## RJ1 — Write-ahead: a record exists before the bytes leave (G34, P0; every client)

- **Decision** (orchestrator): before the submit effect is issued, the core persists the dApp-tx
  record (and the wallet Send records) under the locally computed userOpHash with
  `maybe_sent = true` and `submit_block`, and hands it to the tracker. A NotSent verdict then
  withdraws it (S5 still shows no Activity row once the verdict is in); Accepted and MaybeSent
  keep it. A quit, crash or window close at any point after that leaves an in-doubt record the
  tracker resolves on the next launch (FindOpEvent). The wallet's own Send machine gets the same.
- **Why**: DX9 (nonce 50, block 48487627) landed 6 s after the second window close; the only
  writer of the record is `Event::OpSubmitted`, sent after `user_op::submit` returns
  (`desktop:executor/sign_request.rs:392-403`), and `send.rs` persists only on
  `SendShellResult::Submitted` (`core:send.rs:4847-4852` → `accept_submitted`). RD14's rationale
  ("RA3/RG3 cover what happens after a quit") and RG4's rejection of write-ahead are superseded.
- **Mechanics (planner)**:
  - sign_request: new `Event::OpSigned{id, user_op_hash, submit_block, now_ms}` — the shell sends it
    after the passkey, after the local hash and the head read, before any POST. The core builds the
    `SignRecord` (Pending, `maybe_sent: true`, `submit_block`), sets `tracker_handoff`
    (`maybe_sent: true`) and emits `PersistRecord`; on its `RecordPersisted` (stage Submitting,
    write-ahead awaiting) it emits new `SignOperation::ClearToPost{id, user_op_hash}`. The shell
    POSTs only after `ClearToPost` for that id; if none comes within
    `user_op::WRITE_AHEAD_WAIT_MS` (5 000) it does not POST and reports `Failed` (nothing sent).
    The asker check (RB2) runs after the clearance, immediately before the POST.
  - Verdicts: `OpSubmitted{maybe_sent: false}` with the write-ahead hash → no second record;
    `UpdateRecord{close: SignRecordClose::Admitted}` (record's `maybeSent` → false, stays pending)
    and a second handoff with the new `admitted: true` (tracker `acknowledged = true`), so an
    accepted op never reads "may have been sent". `OpSubmitted{maybe_sent: true}` → nothing new.
    A relay hash ≠ the local hash → withdraw the write-ahead record and persist a fresh one under
    the relay's hash (today's path; logged `userop.hash_mismatch`). `Failed` / `Underfunded` /
    `AskerGone` after `OpSigned` and before `OpSubmitted` = proven not sent → new
    `SignOperation::DeleteRecord{record_id}` plus `SignView.tracker_withdraw` (the shell feeds
    tracker `Event::Withdrawn{user_op_hash, record_ids}`), then today's answer. After
    `OpSubmitted` the G21 guard stays (any failure = ReceiptPending).
  - send: the same shape — `Event::OpSigned{user_op_hash, submit_block, now_ms}` during
    `SubmitUserOp`; records (`maybe_sent: true`) → `PersistTxRecords` → on `RecordsPersisted`
    `TrackSubmitted` and new `SendOperation::ClearToPost`; `Submitted{maybe_sent:false}` → new
    `SendOperation::MarkAdmitted{record_ids}` + `TrackSubmitted{admitted: true}`;
    `SubmitFailed` after the write-ahead → new `SendOperation::DeleteTxRecords{ids}` +
    `SendOperation::TrackWithdrawn{user_op_hash, record_ids}`. The receipt screen still waits
    for the verdict.
  - tracker: `admitted` on `Event::Submitted` (serde default) sets `acknowledged`; new
    `Event::Withdrawn{user_op_hash, record_ids}` drops those ids and removes the entry when none
    is left, never patching a record. The existing `new_life` rule (`tx_tracker.rs:896-909`)
    already lets a re-submit of the same hash (S5, DX9 runs 1–2 share `0x7df211ed…`) start fresh.
  - The local nonce still moves only on Accepted (RA5); nothing else changes in `submit_step`.
- **Alternatives**: refuse every quit while a POST is in flight (a crash or force-quit still
  loses the record); write the record from the shell (a second writer of a core rule, FR-020).

## RJ2 — A claimed submit is never settled 4900 on surface loss (G35, P0; extension)

- **Decision** (orchestrator): once the panel's claim for submit carries the op hash, the worker
  answers `ok(op hash)` on `surface_closed`, panel reload, `window_removed` and in
  `recoveryPlan` — may-have-been-sent, ruling 1 + RA2 — never 4900. Before the op hash exists
  (nothing signed or sent) 4900 stays right. This supersedes RB10 for claimed-submit records only.
  The caption 关闭此页交易会在后台继续 then tells the truth.
- **Mechanics (planner)**: the op hash rides the existing `submit` claim (RB5), sent after the
  write-ahead's `ClearToPost` and immediately before the POST, so every record the worker answers
  `ok(op hash)` for has a durable wallet record the tracker resumes on the next boot (T182). The
  worker stores `opHash` and `chainId` on the record (`phase: 'submit'`); `affectedBy` /
  `recoveryPlan` return `{rid, cause, answer: {ok: opHash}}` for such records, and the worker
  also writes `vela.ext.op.<hash>` (RF3) so the page's receipt reads resolve. Verified: today
  `request-life.js:184-193` and `:137-153` ignore `state`, and `background.js:541-553` settles
  every owed record.

## RJ3 — A refused or proven-not-sent op answers an error (G36, P1; every client)

- **Decision** (orchestrator): a relay-Rejected op, or a NotSent the core has proven inside the
  answer window, answers the dApp with an error, not ok + op hash: `-32603` with a fixed
  plain-English message saying the network refused it and nothing was sent. A revert on chain
  stays ruling 9 (tx hash). One answer only. The sheet's 失败 ending for a deterministic reject
  must not say 请重试.
- **Mechanics (planner)**: `user_op::REFUSED_DAPP_DETAIL = "the network refused this
  transaction; nothing was sent"` for a relay rejection (tracker `Rejected`, or a submit-time
  `NotSent{Some(rejection)}` that is not `RelayerUnavailable`); the existing
  `NOT_SENT_DAPP_DETAIL` ("relay unreachable; nothing was sent") for a relay that never had it
  (tracker `NotSent`, submit-time `NotSent{None}`). Kind stays `SignErrorKind::SubmitFailed` (no
  wire change). `SignSubmitOutcome::Failed` gains `#[serde(default)] refused: bool`; the sheet
  reads `SignView.failure_refused`. New `SignEndingState::Refused`: cross, `statusFailed` + the
  new `componentsUi.signing.refused` (RJ6), no Retry words. The auditor's code note (D2) is right:
  `after_receipt_wait` (`desktop:executor/sign_request.rs:512-517`) and the core's G21 guard
  (`core:sign_request.rs:2472-2484`) turn every late verdict into ReceiptPending.

## RJ4 — The answer and the sheet follow what the tracker knows (G37–G39, P1/P2)

- **Decision** (orchestrator): when the tracker has the tx hash (relay status included/failed
  with a tx hash, or the chain lookup found the event), the waiting dApp request is answered at
  once and the sheet shows 已确认; no fall-back to 提交至网络… afterwards; the relay's tx hash is
  used to confirm (receipt by tx hash) instead of waiting for a relay receipt; the 120 s window
  is enforced inside long polls, not only at the loop top.
- **Mechanics (planner)**:
  - sign_request: new `Event::OpTracked{user_op_hash, status: TrackStatus, tx_hash, now_ms}`;
    each shell forwards the tracker's entry for the in-flight op whenever it changes. While the
    inflight is past `OpSubmitted` and unanswered: `Confirmed` / `Dropped` with a tx hash →
    `Ok(tx hash)` (ruling 9 for Dropped); `Rejected` → `Err(-32603, REFUSED_DAPP_DETAIL)`;
    `NotSent` → `Err(-32603, NOT_SENT_DAPP_DETAIL)`; anything else waits. The inflight clears, so
    the shell's later `Submit` result is dropped by the existing guard; shells may stop their
    receipt wait early. The contract is otherwise unchanged (op hash at the window's end).
    Answers follow terminal tracker states only: a relay's `submitted` tx hash can still be
    replaced by a fee bump, and `included` becomes `Confirmed` within one poll through `TxReceipt`.
  - `ending_state`: a `Landed{tx_hash}` ending whose entry still reads `MaybeSent` is drawn
    `Following(Landing)`, not "may have been sent" (DX6: the page had its tx hash while the sheet
    still said "may have been sent").
  - tx_tracker: when a status names a tx hash for a non-terminal entry, new
    `TrackOperation::TxReceipt{chain_id, tx_hash, user_op_hash}` (the chain pool's
    `eth_getTransactionReceipt`), answered `TrackShellResult::TxReceipt{user_op_hash, now_ms,
    receipt_json}`; the core finds the op's own `UserOperationEvent` in the receipt's logs (the
    find-event reader) and runs `safe_execution_failed` over the same logs → Confirmed / Failed,
    `NotifyConfirmed` / `HoldingsMoved` as for a relay receipt. A null receipt keeps polling at
    the receipt cadence. Verified: `tx_tracker.rs:1110-1116` only sets `acknowledged`, and
    FindOpEvent is gated to `in_doubt()` (`:1291-1305`), so EX13's landed op read 还没上链 for
    5 min 49 s.
  - The window inside polls: desktop (`wait_within`), iOS (`SignExecutor.swift:363-387`) and
    Android (`SignExecutor.kt:266-276`) already give each poll only what is left; the web does
    not (`safe-transaction.ts:3525`, `dapp-submit.ts:558`) — it gets an abort at the deadline.
    The web's own status branch in `waitForReceipt` (`:3567-3583`) is a client copy of the
    tracker's rule and goes (the core answers through `OpTracked`).

## RJ5 — Desktop tabs: the minimum, not one WKWebView per tab (G42, P1)

- **Decision** (orchestrator's rule): the address bar never names a host the visible page is not,
  and Back/Forward never cross tabs.
- **Planner's choice: the minimum.** (1) **Veil**: the page keeps `doc_tab`, the tab whose
  document the webview holds (set when a navigation this page asked for the shown tab commits,
  `Load::Started`). While `browsing ∧ shown_tab ≠ doc_tab` the webview is hidden
  (`webview::hide()`), the column draws the page background with the hairline, the failure panel
  when one is up, and no connection chip; the bar shows the pending host with no lock (RE1,
  unchanged). (2) **Per-tab back floor**: `webview::engine()` also reads the WKWebView
  back-list length (`backForwardList.backList.count`, the same `msg_send` technique as
  `engine_sample`); at the shown tab's first commit the page stores `floor = back_len`; Back is
  enabled only while `back_len > floor`. Forward needs no floor: a new load truncates WebKit's
  forward list, so its entries are always this tab's. Same-document (SPA) entries count, which a
  commit counter would miss. A tab shown again starts a new floor; its older history is not
  reachable (confined, not crossed).
- **Why not per-tab views in 082**: `webview.rs` is one thread-local view (`BROWSER`,
  `COMMITTED`, `COMMITTED_URL`, one message sink and one load sink without a tab id,
  `webview.rs:140-186`); `page.rs` calls it from ~40 sites; `browser_host`'s `LoadWatch`, the
  chain notice and the RD1 hold are single-document; and a hidden tab's live page can still send
  provider requests, which the core's routing and the hold would have to learn. Rewriting that
  while RJ1–RJ4 land in the same files is the larger risk. Per-tab views stay a follow-up
  (results.md).
- **Alternatives**: clear WebKit's back list on a switch (no public API); count commits (misses
  SPA history).

## RJ6 — i18n: one new sentence, paid for by a trim (all clients)

- **Decision** (orchestrator): every new user-facing string needs a corpus key; the ja+en budget
  had ~71 B left (138,729 / 138,800). Reuse first; a new sentence is paid for by trimming an
  existing long string in the same change. No key for dApp-facing error text.
- **Ledger (planner)**:

| Change | Key | en / zh | en+ja bytes |
|---|---|---|---|
| add | `componentsUi.signing.refused` | "The network refused it — nothing was sent." / "网络拒绝了这笔交易，什么都没有发出。" (ja ネットワークに拒否されました。何も送信されていません。) | +44 +81 +8 offsets +4 bitmap (N 1784 → 1785) = **+137** |
| trim | `componentsTx.receipt.failedHint` | drop the last sentence ("Open the explorer below for the reason, or go back and try again." / 可点下方「浏览器」查看失败原因,或返回重试。): the explorer link sits right under it, and "try again" is wrong after a revert — the RJ3 point | **−195** (en −66, ja −129) |
| zh-only | `send.txBackgroundHint` | 关闭此页，交易会在后台继续 (zh, zh-TW, zh-HK; the comma D22 asked for) | 0 |
| **net** | | | **≈ −58 B → ≈138,671, ≈129 B headroom** (the residency test prints the figure) |

- Reused, 0 B: `send.txSubmitting` (the close-hold words, RJ17), `home.balanceDetailStatusRetrying`
  and `explore.chainDown` (the fee row's chain words, RJ13), `componentsUi.signing.interactingLabel`
  (a contract counterparty, RJ16), `componentsUi.signing.simWillFail` (the web's revert estimate,
  RJ19), `componentsUi.signing.maybeSent`, `componentsTx.receipt.statusFailed`,
  `send.txErrorGeneric`, `history.deleteRecord`, `connect.browser.clearAllBody`.
- The corpus change is one commit (RI3 order); the pins move to 1785 paths / 1696 leaves.

## RJ7 — Deferral rule

- **Decision** (orchestrator): P3 cosmetic items may be deferred only with a one-line reason in
  results.md; P0/P1/P2 are fixed in 082 unless shown not to be a defect (with evidence). The
  deferred P3s and the not-defects are listed in spec.md's round-2 section.

## RJ8 — The auto-retry race and URL normalisation (G43)

- **Decision**: `browser_load` treats its own retry as its own until the engine commits or fails
  it. `retry_now` records the address it handed out (`own_request`); `engine_started` for that
  address (or while `next_asked` is `AutoRetry` / `Retry`) is never a page-started load, never
  overwrites `next_asked`, and never calls `requested()` (which resets `attempt` and `failure`).
  One pure `same_address(a, b)` compares addresses everywhere the watch compares them: scheme and
  host case-insensitive, default port dropped, an empty path equals `/`, one trailing slash
  ignored, fragment ignored. Verified: `browser_load.rs:619-644` compares `self.url ==
  Some(url)` literally, and the engine reports `https://app.uniswap.org/` for a typed
  `https://app.uniswap.org`.
- **Test vector**: DX14 — fail → attempt 1 → the engine poll sees the trailing-slash URL before
  `Load::Requested` → attempt stays 1, the next is attempt 2 at +5 s, three attempts in all.

## RJ9 — Retry starvation while WebKit's provisional load hangs (G44)

- **Decision**: an attempt that falls due while the engine is still on the same address is given
  back (W7's protection) only while that load is younger than `GIVE_UP_MS` (20 s) or has shown
  live progress (`ENGINE_LIVE_PROGRESS`). Past 20 s with no live progress the attempt is
  `RetryAction::Load(url)`: the new `loadRequest` cancels WebKit's hung provisional load — the
  phones' `should_give_up` rule (RE2), now on the desktop too. The desktop logs the skip once per
  load, not every 2–5 s.
- **Why**: L2 had no attempt for 60 s and waited 49.6 s after `pass` for WebKit's second 60 s
  timeout (`browser_load.rs:780-788`, `browser_host.rs:795-806`). A 6–10 s proxy (DX1) stays
  protected: it commits before 20 s.
- **Alternatives**: `stopLoading` at the first due attempt (re-creates W7 for slow proxies).

## RJ10 — The certificate class in the desktop probe (G45)

- **Decision**: `probe.rs` `io_code` first downcasts the `io::Error`'s inner error to
  `rustls::Error` (as `executor/pool.rs:973-976` does): `InvalidCertificate(_)` and
  `NoCertificatesPresented` → `probe_code::TLS`; any other rustls error keeps its class. Verified:
  ureq surfaces the handshake failure as `ureq::Error::Io` with kind `InvalidData`, which
  `io_code` maps to 0 (`probe.rs:55-83`).

## RJ11 — The desktop chain notice after the first pass (G46)

- **Decision**: while any page read is in flight, the browser host re-reads the pool's health
  every second (`failed_chains`, `unreached_chains`, `rate_limited_chains`), not only after a read
  settles or once something is already down. Verified: `browser_host.rs:239-244` refreshes only
  when a `Work` resolves, and the recheck loop (`:360-375`) starts only once a chain is down. The
  core half (RF1, `rpc_pool.rs:1749`) is right. The quickstart's `outcome=timeout` was wrong: a
  black-holed CONNECT never connects and the desktop logs `outcome=not connected` (fixed there).

## RJ12 — Fee re-quote cadence and `fee:` log lines (G47)

- **Decision**: `fee_policy::requote_delay_ms` becomes 3 s, 6 s, then every 8 s (no 12/15 s
  steps), and new `REQUOTE_TIMEOUT_MS = 6 000` bounds each automatic re-quote, so the fee is back
  within 8 + 6 = 14 s of the relay returning (SC-003). Every client logs `fee: quote failed
  chain=<id> cause=<FeeFailure> re-quote #<n> in <ms> ms` and `fee: quote back chain=<id> after
  <n> re-quotes`. Verified: no `fee:` line exists on the desktop outside `user_op.rs:377`, and the
  panel writes only `[InBand] quote failed … All bundler endpoints failed`.

## RJ13 — The fee row names the chain node, not Vela (G48)

- **Decision**: new `FeeFailure::ChainRead{rate_limited}` for a fee blocked by a chain read (the
  deployment read today), on the re-quote schedule, and `fee_policy::failure_reason_key(failure)`
  so no shell picks the words: rate-limited → `home.balanceDetailStatusRetrying` ("Rate-limited ·
  retrying automatically"); unreachable → `explore.chainDown` with the chain name; relay failures
  keep `componentsUi.funding.denialNetworkError`. Verified: the desktop maps an unanswered
  deployment read to `QuoteUnavailable` (`signing_host.rs:75-78`), and iOS does the same by RF5.

## RJ14 — Network health counts sources, not calls (G53)

- **Decision**: `net_health_step(state, reached, source: Option<u32>, now_ms)`: "went offline"
  needs `MISSES_BEFORE_OFFLINE` misses in a row from at least two distinct sources (chain ids; a
  miss with no chain counts as its own source) **and** no reach from anything for
  `OFFLINE_QUIET_MS` (10 000); any reach resets. Faulted chains while others answer are those
  chains' notices, never "offline". Verified: `executor/pool.rs:872-908` feeds one global counter
  from any chain, and each "came back" invalidates the dashboard. `desk-post-T181.err` has ten
  `net:` lines in two minutes, each "came back" 0.2–0.9 s after "offline" (chains 1, 100 and 480
  were giving up — `1rpc` serves Ethereum too — while the rest answered), so a source count alone
  would still flap; the quiet window is what stops it.
- **Not a defect** (with it): the hero total above the listed assets while 部分余额仍在更新 shows
  is `balance_dashboard::display_total`'s `max(live, cached)` rule (#188,
  `balance_dashboard.rs:1419-1440`). What made it last minutes, and the "24 个网络" banner on
  relaunch, is the flap restarting read rounds (DESK_B's investigation task proves it with a test).

## RJ15 — A signed amount never reads −0 (G49)

- **Decision**: new core `l10n::number::format_signed_token_amount(delta_base_units, decimals,
  preset) -> Option<String>`: `None` for a zero delta (never drawn, RC4/RC6); a non-zero delta
  whose ladder rendering is "0" is written exactly (RC5's scaling, trailing zeros trimmed); the
  minus is U+2212. Desktop `signing/live.rs:387-404` and the phones' delta rows use it.

## RJ16 — What a dApp record's detail says (G52)

- **Decision**: `FeedTxRecord` gains `#[serde(default)] call_data: Option<String>` (the shells
  map it from the stored request); `FeedItem` gains `counterparty_role: Recipient | Contract`.
  `dapp_item`: calldata that is exactly `transfer(address,uint256)` → the decoded recipient,
  Recipient; any other calldata → `to`, Contract (label `componentsUi.signing.interactingLabel`);
  no calldata → `to`, Recipient. `tx_hash` is `None` when it equals the record's `user_op_hash`
  (an op hash is never an explorer link). Clients draw no explorer control without a URL
  (the desktop draws the button always, `flows/panels.rs:893-903`).
- **Not a defect**: the empty amount block for a token transfer is RG2's rule (amount only for a
  native value > 0).

## RJ17 — ⌘W and the close hold's words (G68)

- **Decision**: ⌘W is Close Window (macOS convention for a non-document app; a menu item is
  added) and goes through `on_window_should_close`, so the RD14 hold applies. A held close shows
  `send.txSubmitting` (提交至网络…) in the bar's notice slot for 2.5 s in Explore and brings the
  submitting column or the Send screen forward. The second close within 5 s still quits; with
  RJ1 the record is already on disk.

## RJ18 — The "don't send it again" trace stays in view (G51)

- **Decision**: on a pending record the desktop detail's 删除记录 is a quiet secondary control
  below the explorer, not the full-width danger button (confirmed and failed records keep it).
  The dApp ending (`dapp_landing`) survives a section switch and is shown again in Explore until
  it ends or the person closes it (`page.rs:14102-14130` keeps it only while `panel == Signing`).
  Web, iOS and Android check their pending-record detail for the same prominence.

## RJ19 — A relay estimate that says "reverts" (G57)

- **Decision**: new core `user_op::estimate_failure(error_json) -> EstimateFailure{Reverts{reason},
  Unavailable}` (reason through `sim_outcome::revert_reason`). The web, which runs no simulation
  (RG6), shows `simWillFail` / `simWillFailReason` in the danger tone when the quote's estimate
  says Reverts. The slide stays live (L-D5: a warning informs, never blocks); a submit then ends
  in a relay rejection that RJ3 answers `-32603` refused, with the Refused ending. Verified:
  `isPlainTransferCall` (`safe-transaction.ts:1938`) counts an ERC-20 `transfer` as plain, so the
  catch at `:2284-2294` falls back to defaults silently.

## RJ20 — Extension panel hygiene (G55, G58, G59, G63, G64, G65)

- **Decision**:
  - RB9: `PanelSurface.caller` is `$state`, so the layout effect re-runs when a request is owed.
  - The panel keeps its port only while it owes a request or holds a claimed submit; when idle it
    does not reconnect after Chrome's idle stop, and it reconnects when the worker writes a record
    for its window (`chrome.storage.onChanged`), which RB8 already does.
  - Cooled endpoints are skipped while an un-cooled one is left; with all cooled, only the one
    whose cooldown ends first is tried (one 8 s attempt). An abort by the 8 s timer is logged
    `kind=timeout`.
  - The full-panel 已签名 tick is skipped when another request is already owed; the next card
    shows at once.
  - `panel.js` reads the wallet's pinned language before Chrome's UI language.
  - The account follow and the lower-case-grant rewrite run from the root layout on every route,
    and `followedAddress` lives outside the wallet page, so a remount is not read as a boot.

## RJ21 — Evidence and harness fixes

- The chaos proxy resets `latency` on every mode switch unless the switch names one (the W1b
  artefact). The quickstart gets the desktop's real C1 log words and a correct nonce template
  (`getNonce(address,uint192)` = `0x35567e1a` + the 32-byte padded address + a 32-byte zero
  key). The isolated e2e builds into its own output directory and never rewrites the live
  `extension/dist`.
