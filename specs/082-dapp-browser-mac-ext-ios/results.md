# 082 results — dApp browser on the Mac, the Chrome extension and the iPhone

Branch `082-dapp-browser-mac-ext-ios`. Goal (owner): 在一个不稳定的环境里构建稳定的用户体验 — a
stable experience in an unstable network. Faults reached only the app under test (ruling 6): a
per-app dev proxy on the desktop and the iPhone, Chrome for Testing with its own `--proxy-server`;
no system or phone proxy was ever changed.

## 1. What happened, in order

| Step | Outcome |
|---|---|
| Device pass (2026-09-28) | 33 findings G1–G33 on three clients; owner rulings 1–10 (spec.md) |
| Round 1 (T001–T141) | core + desktop + web/extension + iOS + Android + signer page; each group adversarially reviewed |
| Post-fix device pass (09-29) | desktop 25 confirmed / 9 refuted / 5 unproven; extension 19 / 7 / 2; iPhone 23 / 10 / 9 — two new **P0**s (below) |
| Round 2 (T185–T251, G34+) | write-ahead records (RJ1), panel-close answer (RJ2), refusal answered as an error (RJ3), tracker-driven answers (RJ4), tab veil + back floor (RJ5); core reviewed three times (6 money defects found and fixed), every client reviewed |
| Close-out | client tests brought to the second review's hand-off rule; an Android cancel that never answered the core (would block every later send) fixed |
| iPhone fixes (round 3, iOS only) | X-HISTORY, X-FIRST-TAP, G12 header, the wordless ⚠ line, the unbounded first deployment read |
| Post-fix checks (post3, 09-29 → 10-01) | table in §3 |

## 2. The two P0s found after round 1 — both fixed and checked on the device

| P0 | Before | After |
|---|---|---|
| **G34 / DX9** desktop: the window closed during the submit POST | the payment landed (nonce 50) with no Activity row, no tracker, no dApp answer | the record is on disk before the POST; relaunched with the relay still muted, Activity shows the row and the tracker confirmed it on chain by itself (nonce 62→63, one op) — `evidence/desktop/post3-DX9.txt` |
| **G35** extension: the side panel closed after the submit claim | the page got 4900 while the op landed 13 s later (a retrying dApp pays twice) | a claimed submit with an op hash is answered ok(op hash), never 4900 — e2e G35 ×3 (panel closed / reloaded / dust slid in the panel) in Chromium with the real extension |

## 3. Post-fix checks (post3)

| Client | Row | Result | Evidence |
|---|---|---|---|
| Desktop | DX9 close during POST | ✓ | `desktop/post3-DX9.txt`, `post3-dx9-*.jpg` |
| Desktop | DX-W3 relay refusal | ✓ the page got `-32603 the network refused this transaction; nothing was sent` 0.1 s after the relay's `rejected`; sheet 失败 with no 请重试; nonce unchanged | `desktop/post3-DX-W3.txt`, `post3-w3-*.jpg` |
| Desktop | CLOSE-TAB | ✓ closing a background tab loads nothing | `desktop/post3-closetab.jpg` |
| Extension | lifecycle + provider e2e (isolated, port 4174) | ✓ 15/15 — G35 ×3, G55 (a request while the panel shows Settings), G19 (worker stopped), EX4/EX5/EX6, provider suite. The relay is scripted by the suite: answer semantics, not an on-chain landing | `extension/post3-e2e.txt` |
| iPhone | X-FIRST-TAP | ✓ a row opens its own record (by id) | `ios/post3-X-FIRST-TAP.jpg` |
| iPhone | X-HISTORY | ✓ after the second fix (SwiftUI calls the sheet setter twice; the close is now reported once via `onDismiss`) | `ios/post3-X-HISTORY.jpg` |
| iPhone | E-G12 signing header | ✓ host whole on one line, chain chip under it | `ios/post3-E-G12.jpg` |
| iPhone | G14-zero | ✓ 发送 / 0 ETH, slide 确认 | `ios/post3-E-G12.jpg` |
| iPhone | X-DEADPROXY | ✓ ⚠ 部分余额仍在更新。 (was a wordless ⚠ ›); no direct bypass | `ios/post3-deadproxy.jpg`, `post3-iphone.txt` |
| iPhone | IX6 / RF5 fee under a silent chain | ✓ reason at +15–19 s (was 4 min 35 s of 估算中 with no reason); `[fee] quote failed … cause=chain_read`; back by itself ~16 s after the fault cleared (target 15 s) | `ios/post3-ix6-*.jpg` |

Rows of quickstart §2–§4 not re-run after round 2 (their fixes are unit-tested only): desktop
DX14/L2–L4 retry race, L5 certificate class, C1 timing, S7/S8 15 s, G14-num −0, KEY-FOCUS on an
autofocusing page; extension EX-W1/EX-S5/EX13 against the live relay; iPhone IX-W1/T183 force-quit
under the write-ahead, IX7-probe.

## 4. Gates (last run)

| Suite | Result |
|---|---|
| vela-core (`i18n-all,crux`) | 2118 passed; clippy/fmt clean; i18n ja+en 138,671 / 138,800 |
| uniffi + wasm crates | 25 passed; `build-web --check`, `gen-core-types --check`, event payloads 0 mismatches |
| desktop | 760 passed, 0 failed, 49 ignored |
| iOS VelaWalletTests | 1054 tests in 135 suites passed |
| Android unit | 863 tests; 1 failure only under full-suite load (see §6.4), passes alone 5/5 |
| web | 2225 unit passed; svelte-check 0 errors / 0 warnings; extension e2e 15/15 (isolated) |

## 5. Money (recomputed from chain)

Every op the passes sent matched one UserOperationEvent, none twice, nothing called failed that
landed: desktop Safe 0x88cC…6894 nonces 44–50 and 63; extension Safe 0xD400…130b nonces 23–26;
iPhone (same Safe as the desktop) nonces 51–61. Refused or not-sent ops never landed and were
superseded. Only dust (0.001 xDAI + ~0.01 fee) moved.

## 6. Open — not done in 082

1. **Merge with `main` (083/084, PRs up to #335): 204 conflicting files, and two owner rulings that
   contradict each other** — needs the owner before the merge:
   - lost relay reply: 082 ruling 1 answers the **op hash** within 120 s; 083 (ddb52da8) never
     answers an op hash and keeps the page waiting for the real transaction;
   - on-chain revert: 082 ruling 9 answers the **tx hash**; 083 answers **-32603 "included but
     reverted (tx)"**.
   Several other 083 fixes overlap 082's (dApp tx in 活动, plain coin send, tab close reload, Back
   floor, hash fits the column, "would fail … still pay gas", the bar after Enter).
2. **Shared core rules found on the iPhone, not yet in the core** (FR-020, all four clients):
   the chain notice waits for the end of the first pass (155 s with 19 Gnosis endpoints);
   NSURLError -1200 behind a proxy is classed as a certificate error; a never-landed dApp record
   reads 处理中 forever (resolve it by the Safe's EntryPoint nonce); a dApp `value` given as a JSON
   number or decimal string (reject anything but hex, -32602); **new: an unknown total is drawn as
   ¥0.00 when no chain answered and nothing is cached** (X-DEADPROXY).
3. Desktop: a new tab does not focus the address bar; one WKWebView per tab (RJ5 shipped the
   minimum: veil + per-tab back floor); popups blank the page (G69).
4. Android: `SendMachineTest` split-send case ends `EstimateFailed(Other)` before Confirm under
   full-suite load — check whether a slow device can see Continue's quote superseded.
5. Core reviewer, owner decision: re-sending the identical op while an earlier may-have-been-sent
   POST of it is out, then proven not sent, deletes the earlier op's Activity rows (needs one fact
   from the shells, e.g. a `replaced` flag on `RecordsPersisted`).
6. A chain-found `UserOperationEvent` with success is not checked for a Safe `ExecutionFailure`
   on the `execTransaction` path (module v0.3.0 already sets success = false).
7. P3s deferred in spec.md's round-2 section (copy and layout polish for one design pass).
8. **Owner batch T170–T173** (Touch ID / Face ID / the owner's Chrome) not run; **T142** — deploy
   the signer page `82fae7f2…` — owner only.

## 7. Relay hand-offs (T176)

- The relay marks every op of a mined bundle `rejected` unless its own event succeeded, and stores
  the bundle tx with it (`vela-bundler mark_bundle_confirmed`, `vela-relay-cf lane_do.rs`). The
  wallet now reads such a "rejected" from the chain (an on-chain revert is never "nothing was sent").
- Status polling: the relay serves only `pimlico_getUserOperationStatus`; an
  `eth_getUserOperationStatus` alias would let other bundlers' clients poll it.
- D2/D4: the fee floor on Gnosis and Arc (native coin = stablecoin) is the relay's to update
  (ruling 2).
- The relay refuses ops whose estimate reverts, so an on-chain revert cannot be produced on demand
  in a device pass (DX-W3/EX-W3/IX7 rows).

The iPhone runs this branch's Debug build, back on the owner's wallet (`VELA_PARALLEL_SPACE=0`).
