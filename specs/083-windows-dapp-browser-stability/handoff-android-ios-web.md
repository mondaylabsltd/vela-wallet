# Hand-off: check the 083 findings on Android, iOS and web

- **Readers:** first a new Claude Code session on a machine that can reach Android and iOS devices and run the web wallet and its Chrome extension; second, the owner.
- **Source:** spec 083 (the Windows dApp browser, plus real Uniswap swaps on Base). Branch `083-windows-dapp-browser-stability`; the code reviewed is `c0694047`, 2026-09-29.
- **How the verdicts were reached:** this file was written on the Windows PC and is committed on the branch on top of `c0694047`. Every verdict comes from READING the code at `c0694047`. Nothing was built or run on Android, iOS or web. Treat each verdict as a prediction that a device run must confirm or overturn.
- **Background:** `specs/083-windows-dapp-browser-stability/` (spec.md, research.md, results.md, quickstart.md), and `git log main..083-windows-dapp-browser-stability`.

## How to read this

The file is long (about 2,980 lines, over 256 KB): a whole-file Read fails, so read it by line range (below). A session that checks ONE platform reads only these parts:

| Platform | Read | Skip |
|---|---|---|
| Android | §1–§6 (Purpose, Before you start, Ground rules, Verdicts, Summary matrix, Commits to mirror), §7 Android, §10 "Android" table, §11 (Not easily checkable, owner decisions), §12 (Order of work). For A-S3b's second client only: §9.1 "Build and run" and "Test account" (A-S3b's step 2 also names the iPhone and the Windows desktop) | §8, §9 (except as just noted), the iOS and Web tables in §10 |
| iOS | §1–§6, §8 iOS, §10 "iOS" table, §11, §12. From §7 only: §7.1 "What the page offers" (the test page), and the **Windows:** line of each A- item an I- item points to ("see A-S2") | §9, the rest of §7, the Android and Web tables in §10 |
| Web | §1–§6, §9 Web, §10 "Web" table, §11, §12. From §7 only: §7.1 "What the page offers", and the **Windows:** line of each A- item a W- item points to | §8, the rest of §7, the Android and iOS tables in §10 |

Line ranges in this version (after any edit, `grep -n '^##' <this file>` gives the current ones):

| Part | Lines |
|---|---|
| Top, §1–§6 (How to read … Commits to mirror) | 1–280 |
| §7 Android: §7.1 Setup 287–406 (its "What the page offers" 352–365); §7.2 Checklist 407–1228 | 281–1228 |
| §8 iOS: §8.1 Setup 1235–1356; §8.2 Checklist 1357–2128 | 1229–2128 |
| §9 Web: §9.1 Setup 2133–2254; §9.2 Checklist 2255–2841 | 2129–2841 |
| §10 Also noticed: Android 2846–2863, iOS 2864–2882, Web 2883–2901 | 2842–2901 |
| §11 Not easily checkable, owner decisions | 2902–2961 |
| §12 Order of work | 2962–2983 |

- Most checklist items have four parts: **Windows** (what failed), **Why here** (the code), **Check** (what to do and what GOOD/BAD look like), **Fix** (for the later fix spec). The "handled" P3 items have only **Why** and **Check**. For a check pass, read Windows (where present) and Check; Why here tells you what to look at if the result surprises you.
- The account is shared by all platforms (§3). Only the session the owner names runs money steps (§3 Money); every other session closes its sheets with ✕.

---

## 1. Purpose

- On Windows, 083 found and fixed about 40 failures, among them:
  - a reverted swap reported to the dApp as done;
  - the userOpHash handed to a dApp that waits for a transaction hash;
  - the fee coin drained by a Max swap;
  - dApp transactions missing from 活动;
  - Edge's own error pages showing instead of Vela's.
- Most of those fixes are in `app-desktop` and reach no other client. A fix in `rust/crates/vela-core` reaches a client only if all three of these hold:
  1. the client's core is rebuilt from this branch;
  2. its shell feeds the core the new events and results;
  3. its shell draws the new fields.
- **Your job** (the owner asked for the findings to be CHECKED):
  1. Check every item below on real Android and iOS devices, and in the web wallet with its extension.
  2. Record what you see (§3), and show the owner the results table.
  3. Only then, and only for the fix groups (§12) the owner approves: open a new speckit spec and change code. The check pass has already made its folder `specs/084-mobile-web-083-parity/` and branch (§3 Evidence); create the spec in that same folder with `.specify/scripts/bash/create-new-feature.sh --number 84 --short-name mobile-web-083-parity --allow-existing-branch '<description>'`, or `/speckit.specify` if installed (give it the same number and name). Mirror the desktop and core commits named on each item. The **Fix** lines below are the plan for that spec, not work for the check pass.

## 2. Before you start

### 2.1 Getting the code

- The branch `083-windows-dapp-browser-stability` sits on `main` = `origin/main` = `de93634f` (the merge of PR #327). The code reviewed is 47 commits, `c2a137c0` through `c0694047`. This file is committed on the branch on top of them on the Windows PC before the branch leaves it, so it travels with the branch (in any push or bundle). The branch head is therefore the commit that added this file (or a later one), not `c0694047`, and `git log main..083-windows-dapp-browser-stability` lists 48 commits or more.
  - Many of the cited commits come before `c97e0b89`: dbf5a48c, 885867f4, 9e0fe081, 5c9b42a4, 25ed87f6, d5661249, 9fb9d5ce, 4bc67c85, 3cd27313, 3d22bcd1, 545621d7, 4198ec6b, e6285fc9, a6b1b2fb, 84e09838, 24578479. Review with `git log main..083-windows-dapp-browser-stability`, never with `c97e0b89..c0694047`.
- **Getting it onto this machine.** Whether the branch is pushed is the owner's decision (not made when this was written). Do not push it yourself.
  - **Preferred:** the owner pushes the branch (or approves pushing it). Here: `git fetch origin && git checkout 083-windows-dapp-browser-stability`.
  - **Alternative, offline:** a git bundle made on the Windows PC (after this file is committed there). Its base is `de93634f` (= origin/main), so this machine must have that commit first.
    - On the Windows PC: `git bundle create 083.bundle origin/main..083-windows-dapp-browser-stability`
    - Here: `git fetch origin`, then `git bundle verify 083.bundle` (it must say the bundle is okay), then `git fetch 083.bundle 083-windows-dapp-browser-stability:083-windows-dapp-browser-stability && git checkout 083-windows-dapp-browser-stability`.
- Confirm: `git log --oneline -1 c0694047` works, and `git log --oneline -1 -- specs/083-windows-dapp-browser-stability/handoff-android-ios-web.md` names a commit on the branch (a tracked file, not a loose copy).

### 2.2 Which core each client runs

This decides what "present" means for each item.

| Client | How the core gets in | Is the 083 core in a build from this branch? | How to prove it |
|---|---|---|---|
| Android | `cargoNdkBuild` compiles `rust/crates` into `jniLibs` during `assembleDebug` | **Yes**, provided you do not pass `-PvelaSkipRustBuild` | Not Settings › About: it shows only the HEAD commit. Grep the APK's `.so` for an 083-only string (§7.1 Build step 5) |
| iOS | A gitignored xcframework built by `rust/scripts/build-ios-xcframework.sh` | **Only after you run that script** | `rust/scripts/check-ios-core-fresh.sh` exits 0 |
| Web + extension | The committed wasm `assets/wasm/vela_core_bg.55d58f879149.wasm` (`rust/pkg-web/build-info.json` source `55d58f87…`), last rebuilt at `42acc794` on main, **before 083** | **No**, until `npm --prefix scripts run build:wasm` | `node rust/scripts/build-web.mjs --check` prints "rust/pkg-web is current" |

- **Core-only items show pre-083 behaviour on a stale build.** These are R2, D1b, W20, H2's rows and the fee machine's U1 input. With every observation, record which core was running.
- **Wire variants added by 083:**
  - `SignSubmitOutcome`: `reverted`, `not_confirmed`
  - `FeeGasOutcome`: `refused`
  - `FeeFailure`: `would_fail`
  - `FeeEvent`: `balance_changes_measured`
  - new `dapp` feed fields

  A pre-083 core cannot deserialize them: the 079 iOS core logs `sign_request fault`, and the web's committed wasm does not have them. So any shell change that emits one must ship together with the rebuilt core.
- **The web branch will fail CI** until the wasm is rebuilt and committed: CI runs `node rust/scripts/build-web.mjs --check` (`.github/workflows/ci.yml:281`). The local `pnpm check` does not catch this.
- **083 helpers that are NOT exported** through `vela-core-uniffi` (phones) or `vela-core-wasm` (web). Several fixes need these exports first:
  - `user_op::user_op_hash`, `user_op::existing_op` (`rust/crates/vela-core/src/user_op.rs:703, 719`)
  - `sign_request::reverted_transaction`, `REVERTED_MESSAGE`, `PAGE_WAIT_CAP_MS` (`sign_request.rs:136, 116`)
  - `tx_tracker::op_execution_failed` / `user_op_outcome_in_logs` (`tx_tracker.rs:1057`)
  - `rpc_pool` `is_hedged_read`, `early_verdict`, `HEDGE_AFTER_MS`. `RpcPoolView.pending_urls` is `#[serde(skip)]` by design (`rpc_pool.rs:950`), so it needs an accessor rather than a type regeneration.
  - Only `parse_existing_user_op_hash` (`rust/crates/vela-core-uniffi/src/lib.rs:1497`) and `relay_error_message` are exported today. 083's only change to `vela-core-uniffi` is a doc comment (9e0fe081), so there is nothing to regenerate for the phones yet.

### 2.3 Tests already red on this branch (expected)

- Android JVM: `CoreWireDriftTest.signRequestWiresMatchTheMirrors` fails. Kotlin `SignSubmitOutcome` (`SignWire.kt:224`) has 5 variants; the generated TS has 7. Porting S2/S3 turns it green.
- iOS: `CoreWireDriftTests` / `BrowserWireDriftTests` decode views from the REAL core, so after the xcframework rebuild they may flag any new 083 wire field or operation, not only S2/S3's. Porting S2/S3 alone may not turn them green; read the field or operation the failure names.
- Web, local: `pnpm check` fails until `pnpm sync:wasm` has run (it runs `sync-wasm.mjs --check`; `static/` is gitignored and can be stale).
- Web, CI: `node rust/scripts/build-web.mjs --check` (`.github/workflows/ci.yml:281`) fails until the wasm is rebuilt AND committed (§2.2). Do not commit it during a check run (§3).

### 2.4 The relay "refusal" rule (used by U1b on all three clients)

At HEAD the desktop treats a relay error as a refusal ("this transaction would fail"), not an outage, by this rule in `app-desktop/vela-wallet/src/executor/relay.rs:559` `is_simulation_refusal`:

- the message does **not** name an EntryPoint code (`AA` plus two digits standing as a word), **and**
- lower-cased, it contains `simulation failed`, `execution reverted` or `reverted during simulation`.

So `AA23 reverted` stays `simulation_failed`. The earlier 380d9014 version ("contains simulation failed or revert") was tightened in 5825e443; mirror the HEAD rule.

---

## 3. Ground rules

- **Ask the owner first** before any step that needs a fingerprint, Face ID, Windows Hello, a passkey on another phone, or UAC. Items that need the owner's own account or devices are marked **[owner]**.
- **Money:**
  - **Ask first.** Before the first money step of a session, list for the owner every spend you plan: check id, chain, amount, expected fee, and the top-up swap if one is needed. Wait for a yes. Spend nothing outside that list without asking again. Only the session the owner names may run money steps; every other session runs sheet-only checks (close the sheet with ✕).
  - Real funds, small amounts only, and only on:
    - **Gnosis**: any test-dApp transaction, each under $0.01: Send dust (0.001 xDAI to the founder address plus the fee), a capped Approve (A-H2), Bundle (B5792), and the wallet_sendCalls probes (A-REC, I-REC, W-REC, B5792).
    - **Base**: Uniswap swaps (about $0.08 fee each) and the optional S2 deadline-revert router call (a few cents).
  - Never use Ethereum mainnet or any other chain.
  - **Before any Send dust** (or other test-page transaction): press Connect, then Switch to Gnosis, then Chain. `#out` must show `eth_chainId` → `0x64`, and the sheet must name Gnosis and xDAI. On the phones a new site starts on Ethereum (the core's `DEFAULT_CHAIN_ID = 1`, `rust/crates/vela-core/src/app/dapp_browser.rs:84`); in the extension it starts on the wallet's current network (`app-web/vela-wallet/extension/background.js` `chainOf`). On Ethereum, Send dust moves 0.001 ETH on mainnet. If the sheet names Ethereum or ETH, close it with ✕.
  - "Send dust" on **Base** sends 0.001 **ETH**, which is not dust. Do not press it on Base.
  - Never slide an unlimited approve as the page asks: cap it first. The one exception is Bundle, whose second leg resets the allowance to 0.
  - Check balances before every money step. Most checks cost nothing if you close the sheet with ✕.
- **Test account:**
  - The parallel space (debug builds on all three clients) is the MultiTest Safe `0x88cCA0EeDbF2C4426110bbFc998F048689266894`, the account the Windows pass swapped with.
  - The fixture keys are public (the web's parallel page says "These private keys are public. Never send real money to these addresses."), so anyone can move what MultiTest holds. It holds only a few cents, which 083 used for its swaps. Never fund it beyond what one pass needs, and ask the owner before any top-up.
  - **It is the SAME Safe on Android, iOS, web and the Windows desktop.** Balances and the nonce are shared.
    - Never run money steps on two clients at once, except deliberately for S3b.
    - Let each op reach 已确认 before another client sends.
    - A Max swap, or a fee paid in USDC, on one client drains USDC for all of them.
  - Fixture keys sign for it, so there is no biometric prompt and a passkey cannot be cancelled or made to fail there.
  - **Balances** as of 2026-09-29. Check again before any money step.
    - **Base:** about **0.035 USDC and 0.00044 ETH** (after the last 083 swap, 03:37; Uniswap's own display and the on-chain receipt).
    - **Gnosis:** about 0.184 xDAI and 0 USDC (read with eth_getBalance / balanceOf while this file was written). Read it again before the first Gnosis step (wei, hex):
      ```
      curl -s https://rpc.gnosischain.com -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"eth_getBalance","params":["0x88cCA0EeDbF2C4426110bbFc998F048689266894","latest"]}'
      ```
      Each Send dust needs 0.001 xDAI plus the fee. Below 0.01 xDAI, ask the owner to fund it before starting.
  - **The Base budget:**
    - 0.035 USDC is below EVERY USDC→ETH step, not only U1: no 0.05 USDC swap (H2/F1/F3) can run until you top up. Top up with one ETH→USDC 0.0001 swap. It yields about 0.27 USDC and costs 0.0001 ETH plus about 0.000031 ETH in fees (results.md). Its fee is in ETH only while USDC is below the USDC fee (about 0.085), as it is now. A-U8's ETH→USDC swap can be this top-up, and A-F1 accepts an ETH→USDC row.
    - While USDC can pay it, a USDC→ETH swap's fee is auto-picked in USDC (about 0.085; results.md's 0.1 swap paid 0.084558), so each 0.05 USDC→ETH swap costs about 0.135 USDC. One top-up (about 0.305 USDC) funds only TWO such swaps across ALL platforms, and leaves about 0.035 again. Do ONE swap per platform and use it for H2, F1, F3 and U8 together; U8 never needs a swap of its own. If Android's swap is the A-U8 top-up itself, iOS and web need one USDC→ETH swap each, and that one top-up covers both.
    - ETH is scarce too. A top-up costs about 0.00013 ETH, and every swap whose fee is paid in ETH costs about 0.00003 ETH. Any funding beyond the current balances needs the owner. Ask before the balance runs out, not after.
  - USDC is already Permit2-approved from the Windows swaps.
  - Right after a top-up, and before any 0.05 swap, run the Max sheet check (U1, no slide; it costs nothing) on every platform this session covers: U1's precondition (USDC at least about 0.2) holds only between a top-up and the first 0.05 swap after it. Then run each platform's single swap. A SLID Max (I-U1) is the last money step; it drains the USDC, and it proves something only while U1's precondition holds, so plan it (and any top-up it needs) with the owner.
- **The owner's real wallet:**
  - On a phone that holds it, never uninstall, reinstall, `pm clear`, erase the app or clear its site data. Any of these wipes the wallet.
  - Installing OVER the app (`adb install -r`, `xcrun devicectl device install app`) keeps its data; uninstalling does not.
  - If `adb install` reports `INSTALL_FAILED_UPDATE_INCOMPATIBLE`, stop and ask. The same on iOS: if `devicectl` refuses to install over the Vela already there (e.g. an App Store or TestFlight build, or another team's signature), stop and ask. Never delete the app to get past it.
  - WebView-provider experiments (Android W1) run on an emulator only.
  - iOS may ask for local-network permission the first time a LAN page loads. Allow it, and tell the owner.
- **Git:**
  - Work on the check branch `084-mobile-web-083-parity`, made from this branch (Evidence, below).
  - Never push, open a PR or force anything without asking.
  - Do not commit regenerated artifacts during a test run. That covers web `rust/pkg-web` + `assets/wasm`, and the iOS diffs in `vela_core_uniffi.swift` and `Dev/vela_dev_fixtures.swift` (§8.1 Core). Commit them only as a deliberate part of a fix, and say so.
- **Evidence:**
  - For the check pass, first check that 084 is free: `ls specs; git ls-remote origin '*084*'` (specs/082 is missing here, so numbers are also taken on branches this checkout may not have). Then, from this branch: `git switch -c 084-mobile-web-083-parity && mkdir -p specs/084-mobile-web-083-parity/evidence/{android,ios,web}`, and create `results.md` there. Nothing else yet: its spec.md, plan, tasks and any product-code change wait until the owner approves a fix group (§1).
  - When the owner approves a fix group, create the spec in that same folder: `.specify/scripts/bash/create-new-feature.sh --number 84 --short-name mobile-web-083-parity --allow-existing-branch '<description>'`. Without `--number 84` the script numbers past the highest specs/ prefix and makes 085; without `--allow-existing-branch` it refuses the existing folder ("Feature directory … already exists"). It creates no git branch.
  - **Exception — test probes.** The PRIMARY/DECISIVE checks (A-S3b, A-S2, A-S3, I-S3b, I-S2, I-S3 and the other JVM/XCTest checks) need new test code. Write it only in the test source sets (`app-android/vela-wallet/app/src/test/…`, `app-ios/VelaWallet/VelaWalletTests/`). Change no production file. Commit it on the check branch as `test(084): 083 parity probes` (never pushed without asking), or leave it uncommitted, and list each file in results.md so the owner sees it with the results table.
  - Save screenshots to `specs/<new spec>/evidence/{android,ios,web}/<ITEM>-<before|after>-<what>.png`, e.g. `android/S3-before-page-got-op-hash.png`. Save logs as `.txt` next to them.
  - For every check, record:
    - the build commit and which core ran (§2.2);
    - the device, OS and WebView or browser version;
    - the steps taken;
    - the on-screen text, verbatim;
    - the verdict (present / absent / not reachable);
    - every tx hash that touched a chain.
  - Collect the results in a table in `specs/<new spec>/results.md`, in the style of 083's results.md.
- **Prefer the deterministic checks** marked PRIMARY or DECISIVE (JVM, XCTest, the web fetch shim). Several device repros are racy.

## 4. Verdicts and priorities

| Verdict | Meaning |
|---|---|
| **likely** | The Windows failure's mechanism exists here (likely present) |
| **partially** | Part of it exists, or a closely related gap |
| **handled** | The shell already does the right thing; one quick device confirmation |
| **via core** | The 083 core fixes it. It is present on a stale core, so it needs a build from this branch plus a device check |
| **n.a.** | Cannot occur on this client (one line each at the end of each platform) |
| **unknown** | Not assessed; the item says what the device run must show |

| Priority | Meaning |
|---|---|
| P0 | A dApp or the person is told something false about money (success, another op's hash). Fix first |
| P1 | A flow is blocked, a false state persists, or a security gap |
| P2 | Wrong or misleading UI, or slow |
| P3 | Cosmetic, or confirm only |

Priorities and confidence (high/medium/low) come from the code review. Item ids are prefixed per platform: `A-` Android, `I-` iOS, `W-` web.

## 5. Summary matrix

Rows are sorted by the highest priority across the three platforms. **Bold** marks P0.

| ID | Finding | Android | iOS | Web |
|---|---|---|---|---|
| S3b | Another operation's hash answered to the dApp | **likely P0** | **likely P0** | **likely P0** |
| S2 | A reverted dApp operation answered as success | **likely P0** | **likely P0** | partially P1 |
| U1 | Max USDC→ETH: the fee coin is drained by the swap | likely P1 | **likely P0** | likely P1 |
| S3 | The userOpHash answered after a timeout | likely P1 | likely P1 | likely P1 |
| R3 | A revert first reads "couldn't be submitted — funds safe" | likely P1 | likely P1 | partially P2 |
| U1b | A relay refusal drawn as "check your network" | likely P1 | likely P1 | likely P1 |
| U1c | Fee coin list unusable over a failed quote | likely P1 | likely P1 | likely P1 |
| H2 | A dApp transaction is not in 活动 | partially P1 | likely P1 | partially P1 |
| REC | dApp record integrity (page-written to/value) | partially P1 | likely P1 | handled P3 |
| D1b | Dismissed sheet + cancelled passkey never answered | via core P1 | via core P1 | handled P3 |
| R2 | A bundle neighbour's ExecutionFailure fails our op | via core P2 | via core P1 | partially P1 |
| U8 | Fee about 14× the on-chain cost (owner decision) | likely P1 | likely P2 | likely P2 |
| W20 | iPhone caBLE: lower-case tunnel ids rejected | via core P1 | via core P1 | n.a. |
| W4 | Certificate error / insecure origin can still sign or mislead | partially P2 | handled P3 | likely P1 |
| W5 | Renderer crash / page gone leaves a live request | handled P3 | handled P3 | likely P1 |
| F1 | A dApp row says nothing about what it moved | likely P2 | likely P2 | likely P2 |
| F3 | The called contract labelled 接收方 | likely P2 | likely P2 | likely P2 |
| H4a | A failed MESSAGE signature shows the transaction-failure sentence | likely P2 | likely P2 | likely P2 |
| H4b | Passkey/transport failure → -32603 with raw platform text | partially P2 | likely (low) P2 | handled P3 |
| B5792 | wallet_sendCalls answer / wallet_getCapabilities | partially P2 | partially P2 | likely P2 |
| W13 | A dApp read waits a full timeout per silent node (H6) | likely P2 | likely P2 | likely P2 |
| H1 | Sign-in sheet copy ("…创建", "Touch ID 或 Windows Hello") | likely P2 | likely P2 | likely P3 |
| W11 | Wrong progress label around the biometric prompt | likely P2 | partially P3 | handled P3 |
| W10 | A plain native-coin transfer drawn as a contract interaction | handled P3 | handled P2 | handled P3 |
| W1 | Browser engine / core cannot start and nothing says so | likely P2 | n.a. | likely P3 |
| W9 | Demo host / fixture menus in a live session | likely P2 | handled P3 | n.a. |
| W3 | Engine error pages, no panel for a silent host, no recovery | partially P2 | handled P3 | n.a. |
| W2 | Address bar names the previous site; typing appends | partially P3 | partially P2 | n.a. |
| H8 | After a failed load the tab keeps the previous site | handled P3 | likely P2 | n.a. |
| W6 | window.open / target=_blank | handled P3 | partially P2 | n.a. |
| W7 | External schemes and downloads | handled P3 | partially P2 | n.a. |
| W14 | Connect consent without account and network | handled P3 | handled P3 | likely P2 |
| W19 | Phone-held key: QR, 90 s spinner, raw -32603 | partially P2 | handled P3 | handled P3 |
| H5 | "Check your phone" after the scan; Cancel; handshake wait | unknown | likely P2 | n.a. (inferred) |
| EXE | Sheet headline is the English "Execute" | likely P3 | likely P3 | likely P3 |
| F2 | Transaction hash runs off the detail panel | handled P3 | handled P3 | handled P3 |
| DNS | Lookup failures read as generic | handled P3 | handled P3 | n.a. |
| D1 | An accidental dismiss rejects a signing request | handled P3 | handled P3 | handled P3 |
| W15 | Back crosses tabs / enabled on a new tab | handled P3 | handled P3 | n.a. |
| U1d | "Would fail" says you would still pay gas | n.a. | n.a. | n.a. |
| D3 | One timeout flips all traffic to Direct | n.a. | n.a. | n.a. |
| W12 | Menus over the page hide the whole page | n.a. | n.a. | n.a. |

Counts per platform (42 rows each):

| | likely | partially | via core | handled | n.a. | unknown |
|---|---|---|---|---|---|---|
| Android | 17 | 8 | 3 | 10 | 3 | 1 (H5) |
| iOS | 19 | 5 | 3 | 11 | 4 | 0 |
| Web | 17 | 4 | 0 | 8 | 13 | 0 |

Notes on the matrix:

- **H5 was not in the Windows inventory.** The iOS review added it (desktop fixes a22e1b30, 95b8e573, 1063909f). Neither the Android "unknown" nor the Web cell was reviewed: Android was not assessed, and the Web "n.a. (inferred)" follows from W-W19/W-W20 (web has no Vela caBLE).
- **The "Also noticed" section (§10) holds five extra security-relevant items:** the stray `calls` key on Android and iOS (A-1, I-1; the Android review rated it P0/P1), an iOS page leaving the app with no tap and a cross-origin iframe navigating the tab (I-10, I-11), and the web side panel serving only one tab (W-10). The reviews gave W-10 no priority; "P1" for it is this doc's suggestion. Handle all five together with the P0/P1 rows.

## 6. The 083 commits to mirror

| Commit(s) | Where | What |
|---|---|---|
| ddb52da8 | core + desktop | S2/S3/S3b: answer once and truthfully (tx hash, `Reverted`, `NotConfirmed` after `PAGE_WAIT_CAP_MS`, `existing_op` ThisOne/Another) — desktop `executor/landing.rs`, `executor/user_op.rs`, `relay.rs` `SubmitError::Occupied` |
| 2448738a | core + desktop | R2 op-scoped logs (`tx_tracker::op_execution_failed`); R3 revert drawn at once (`signing/status.rs`) |
| 380d9014, 5825e443 | core + desktop | U1 `BalanceChangesMeasured`; U1b `Refused` → `would_fail`; U1c list over a failed quote; U1d first clause only (`signing/mod.rs` `first_clause`) |
| ffc9ac9f | docs | U8: the fee math (README) |
| 43fd67a4, ff660c2d | core + desktop + web | H2 dApp rows in 活动; REC record integrity (batch stores `""`/`"0x0"`, char-based shortening, `dapp_url` = request origin) |
| 48b3e931, 9004bba3, b2430a4e | core + desktop | F1 row figure from the approved simulation, "≈"; F3 合约 label; trust flags |
| 99b15ced | desktop | F2 short hash + copy |
| 25ed87f6, d5661249 | desktop | W10 transfer rows; a stray `calls` key never becomes the headline; single-leg rule |
| 3707e196, 1618a9f6, 4d4c76c4 | web | H3: W10 and W11 on web (`nativeSendOf`, preparing label, `input` as calldata) |
| 545621d7 | desktop | W19 QR card, D1 Esc never answers, W11 preparing label |
| 4198ec6b | core + desktop | D1b core safety net (one 4001), `page_gone` |
| a22e1b30, 95b8e573, 1063909f | desktop | H4 (phone stops told apart; a failed message is not a failed transaction), H5 (check-your-phone, Cancel, 15 s handshake) |
| 24578479 | core | W20 upper-case caBLE tunnel ids |
| dbf5a48c, 672d1dbb | desktop | W1 engine-failure panel, write probe; W9 no demo host |
| 885867f4 | desktop | W2 address bar follows the requested URL |
| 9e0fe081 | core + desktop | W3–W5 Vela panels instead of the engine's, `LoadPlatform` table, watchdog |
| 9fb9d5ce, 3cd27313 | desktop | DNS codes on Windows |
| 0ff7fbed | desktop | H8 tab named by the failed address |
| e6285fc9, a6b1b2fb, 84e09838 | desktop | W6 new tab on a gesture; W7 mailto/tel only on a tap; W14 consent rows; W15 per-tab back floor |
| 8872b915, e6b7469e | desktop | W13/H6 hedged reads (`executor/pool.rs`) |
| d9ab6e80, 5c79c6b3 | desktop | H1 sign-in copy (`hardware::method_line`, `Chooser::SignIn`) |
| 4bc67c85, 3cd27313, 3d22bcd1 | desktop | D3 per-host route (nothing to mirror) |
| 5c9b42a4 | desktop | W12 region hole (nothing to mirror) |

---

## 7. Android

Android is `app-android/vela-wallet`: Kotlin and Compose, with an in-app dApp browser on `android.webkit.WebView`.

**Paths in this section** are relative to `app-android/vela-wallet/app/src/main/java/app/getvela/wallet/`, except those that start with `rust/`, `assets/`, `app-android/`, `app-desktop/`, `docs/`, `specs/` or `scripts/`.

### 7.1 Setup

**Toolchain**
- JDK 17 or newer: point `JAVA_HOME` at Android Studio's bundled `jbr` (JDK 21 in current releases; on macOS `/Applications/Android Studio.app/Contents/jbr/Contents/Home`). Run `./gradlew --stop` after using a wrong JVM.
- `ANDROID_HOME`: an SDK with platform 36 and an NDK. The scripts fall back to `~/Library/Android/sdk`, the macOS path. Put `$ANDROID_HOME/platform-tools` on PATH for `adb`.
- Rust: the repo pins 1.97.1 (`rust/rust-toolchain.toml`), and the build scripts `cd rust` before building, so add the targets to THAT toolchain. (Run from the repo root, `rustup target add` adds them to the default toolchain instead, and cargo ndk then fails with "target may not be installed".)
  - `(cd rust && rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android)`
  - `cargo install cargo-ndk`
- python3, and Google Chrome (chrome://inspect).

**Build** (from the repo root, on the branch). `rust/bindings/kotlin`, `rust/bindings/kotlin-dev` and both `jniLibs` folders are gitignored, so they must be generated.

1. Kotlin bindings:
   ```
   cd rust && cargo build --release -p vela-core-uniffi && cargo run --release -p vela-uniffi-bindgen --bin uniffi-bindgen -- generate --library target/release/libvela_core_uniffi.dylib --language kotlin --out-dir bindings/kotlin --no-format && cd ..
   ```
   - The library file is `.dylib` on macOS, `.so` on Linux, and `vela_core_uniffi.dll` (no `lib` prefix) on Windows.
   - specs/040's quickstart names the wrong bindgen package. The correct one is `vela-uniffi-bindgen`.
2. `bash rust/scripts/build-android.sh`
   - It builds 3 ABIs at `--platform 29` into `app-android/vela-wallet/app/src/main/jniLibs`.
   - It then runs `rust/scripts/build-dev-fixtures.sh`, which builds the parallel-space library and `bindings/kotlin-dev`.
   - On Windows Git Bash that script looks for a `.so`. Run its bindgen line by hand with `target/release/vela_dev_fixtures.dll` instead.
3. `cd app-android/vela-wallet && ./gradlew :app:assembleDebug`
   - `cargoNdkBuild` re-runs step 2 whenever `rust/crates` changed.
   - Never pass `-PvelaSkipRustBuild` after pulling Rust changes: the 083 core must be inside the APK (R2, D1b, W20, H2).
4. `adb install -r app/build/outputs/apk/debug/app-debug.apk`
   - The applicationId is `app.getvela.wallet` in both variants.
   - If install fails with `INSTALL_FAILED_UPDATE_INCOMPATIBLE`, stop: see §3.
   - MIUI asks for confirmation on the phone.
5. Prove the core. Settings › About shows `BuildConfig.GIT_COMMIT`, the 7-character HEAD Gradle saw (`app/build.gradle.kts:36-47`; expect the commit that added this file, or later). That proves the Kotlin build, not the core: an APK built with `-PvelaSkipRustBuild` over pre-083 jniLibs shows the same commit. From `app-android/vela-wallet`, as step 4:
   ```
   unzip -p app/build/outputs/apk/debug/app-debug.apk lib/arm64-v8a/libvela_core_uniffi.so | strings | grep -c 'The transaction was included but reverted'
   ```
   It must print 1 or more: that string (`sign_request.rs` `REVERTED_MESSAGE`, added in ddb52da8) exists only from 083 on. 0 means a pre-083 `.so`.

**JVM tests**
1. Build the host libraries (Build step 1's `rust/bindings/kotlin` must exist too): `(cd rust && cargo build --release -p vela-core-uniffi && bash scripts/build-dev-fixtures.sh --host)`
2. Run the suite: `(cd app-android/vela-wallet && ./gradlew :app:testDebugUnitTest -PvelaSkipRustBuild)`; for one probe add `--tests '<YourProbeTest>'`. The subshells keep the shell in the repo root. The tests load the libraries through `jna.library.path = rust/target/release`.
3. Expect `CoreWireDriftTest.signRequestWiresMatchTheMirrors` to fail (§2.3).

No existing test drives a signed submit end to end: SignSwitchGuardTest and SendControllerTest pass `error("no signing")` as the signer. The PRIMARY checks for S2, S3 and S3b therefore need new scaffolding (test source set only, §3 Evidence):
- **Signer:** a test `UserOpSigner` that returns `uniffi.vela_dev_fixtures.fixtureAssert(challenge, listOfNotNull(credentialIdHex), null)` mapped to `Assertion`, as the private `FixtureUserOpSigner` does (`app/src/debug/java/app/getvela/wallet/dev/ParallelSpaceBinding.kt:174`). The debug source set carries these bindings, so `testDebugUnitTest` can call them.
- **Account:** the address is `fixtureMultiAddress()` (0x88cC…6894, MultiTest). Its `WalletKeyRecord`s are EVERY `fixtureAccounts()` record, the first one pinned: the Safe is founded on the whole keyset (`ParallelSpaceBinding.kt:86-100`).
- **Relay:** `FakeRelayPort` answers `eth_getCode` with `FakeRelayPort.body("0x")` (undeployed: nonce 0x0, no nonce read), `eth_estimateUserOperationGas` with the limits (`verificationGasLimit` and `callGasLimit` `0x30d40`, `preVerificationGas` `0xc350`), then `eth_sendUserOperation` and `eth_getUserOperationReceipt` as each check says.
- **Fee:** pass `quoted_fee = SignQuotedFee(amount = "1000", recipient = <safe>)`. Without a usable quote the spine refuses before signing (`UserOpSpine.kt:281`).
- The rest as SendParityBridgeTest builds it (`FeedExecutor(store = FakeStore(), ownAccounts = { emptyList() })`). SignExecutor waits up to 5 s for the record (`SignExecutor.kt:178`) unless the test persists it first.
- After a run, read `fake.calls` (`<kind>:<method>`, in order). A method with no scripted answer gets `Failed`, so script every method it lists, and assert that `eth_sendUserOperation` is among them: a run that never sent measured nothing.

**Test account (debug builds)**
- Enter: `adb shell am force-stop app.getvela.wallet && adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.parallelSpace true`
  - The extra is read only in onCreate, so force-stop first.
  - It opens "平行空间 #0 0x88cC…6894".
- Leave with `--ez vela.parallelSpace false`. The choice is remembered across relaunches.
- These items need the owner's own account (fingerprint): W11, H4b, D1b, W19, W20, H5. W19, W20 and H5 also need the owner's iPhone. H4a falls back to the owner's account if its parallel-space trigger is refused.

**Test dApp**
1. From the repo root (`cd X && A & B &` would start B in the wrong folder):
   ```
   D=app-android/vela-wallet/dev/testdapp; python3 -m http.server 8137 --directory "$D" & python3 -m http.server 8138 --directory "$D" &
   ```
   - The page frames an "attacker" iframe from port+1 (`index.html:110-111`), hence the second server.
   - Verify: `curl -sI http://127.0.0.1:8138/attacker.html` → 200. A 404 breaks every cross-origin iframe check (I-11, 070's A-series).
2. `adb reverse tcp:8137 tcp:8137 && adb reverse tcp:8138 tcp:8138`
3. Open it with `adb shell am start -n app.getvela.wallet/.MainActivity --es vela.openUrl http://127.0.0.1:8137/`, or type the URL in 探索.

What the page offers:
- Buttons: Connect, Chain, Block number, Switch to Gnosis, Sign (personal_sign), **Send dust**, Approve unlimited / Approve 2^254, **Bundle**, Verify sign, SIWE, Add Base, and others.
  - Send dust = eth_sendTransaction of 0.001 of the chain's coin to the founder address `0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141`.
  - Approve unlimited / 2^254 target Gnosis USDC `0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83`.
  - Bundle = wallet_sendCalls with an unlimited approve plus a reset.
- Links: a `mailto:` link and a `target=_blank` link to https://example.com/.
- `window.__ask(method, params)` sends any request and records `{ok, result}` or `{ok:false, code, message}` in `#out`. `me()` is the connected address. `GNOSIS_USDC` and `FOUNDER` are page globals you can use in the console.
- **`#out` cannot show a missing answer or count answers.** It is `JSON.stringify(state)`: one entry per method, overwritten by the next answer and kept until the page reloads.
  - Reload the test page (then tap Connect, which answers at once for a granted site) before every check whose answer you read in `#out`.
  - For "no answer" or "exactly once", paste this logger in the chrome://inspect console first and count its lines:
    ```
    const r=ethereum.request.bind(ethereum);ethereum.request=async a=>{console.log('→',a.method,JSON.stringify(a.params));try{const v=await r(a);console.log('←',a.method,v);return v}catch(e){console.log('✗',a.method,e.code,e.message);throw e}}
    ```
- Debug builds allow cleartext only to 127.0.0.1 and localhost.

**Debug seams**
- `--ez vela.crashRenderer true` loads chrome://crash in the page in front. It arrives through onNewIntent, so the app must already be open on the page.
- `--es vela.startDestination flows-gallery|settings-gallery|gallery` opens the fixture galleries.

**Inspect, logs and screens**
- WebView DevTools: chrome://inspect/#devices (debug builds; `BrowserController.kt:399`). Alternatively: `adb forward tcp:9222 localabstract:webview_devtools_remote_$(adb shell pidof app.getvela.wallet)`.
- Logs: `adb logcat -s VelaLog`, or `adb pull /sdcard/Android/data/app.getvela.wallet/files/logs/`.
  - Scopes: browser.load, browser.nav, browser.renderer, browser.external, browser.inject, rpc.post (host, outcome, ms), rpc.call slow, net, cable, cable.tunnel, cable.ctap, passkey.*, sign.submit, relay.submit, relay.rpc, tracker.patch, send.fee, userop.submit.
- A successful submit does **not** log the op hash:
  - sign.submit logs only refusals;
  - relay.submit logs sender, nonce and tier;
  - relay.rpc logs method and outcome.

  You see the op hash as the page's result in S3, or in `userop.submit previous op pending hash=` in S3b. That log line holds only the first 12 characters (`0x` + 10 hex, `UserOpSpine.kt:362`): compare prefixes, or read the full hash from the page's result.
- Record `adb shell dumpsys webviewupdate` once per device.
- Screens:
  - screenshot: `adb exec-out screencap -p > x.png`
  - recording: `adb shell screenrecord /sdcard/x.mp4`
  - on-screen text: `adb shell uiautomator dump /sdcard/ui.xml && adb shell cat /sdcard/ui.xml | grep -o 'text="[^"]*"'`

**Fault proxy** (`scripts/device/chaos-proxy.py`)

Setup:
1. `python3 scripts/device/chaos-proxy.py chaos.log`, in the background or its own terminal. Put `CHAOS_UPSTREAM=host:port` in front only if this computer itself needs a proxy to reach the internet.
2. `adb reverse tcp:8899 tcp:8899; adb shell settings put global http_proxy 127.0.0.1:8899`
3. Force-stop the app, because OkHttp pools connections.

Control:
- `curl 'http://127.0.0.1:8899/__chaos?mode=drop|blackhole|latency&latency=6000&match=<host regex>'`; `mode=pass` clears the fault.
- Modes:
  - drop closes the CONNECT at once;
  - blackhole holds it for 600 s;
  - latency delays the upstream connect.
- Faults match by host. Switching one on also cuts the live tunnels it matches.
- A host the proxy cannot resolve gets a 502, so DNS tests run **without** the proxy.

Restore: `adb shell settings put global http_proxy :0`.

**Earlier recipes**: specs/043-android-money-wiring, specs/044-android-dapp-browser-signing, specs/070-dapp-browser-core/quickstart.md (A1–A19), specs/079-android-dapp-browser-stability/quickstart.md (L1–L6, S1–S8, U1–U7, T1–T5), specs/040 (but see the bindgen note above). The previous device was a Xiaomi alioth, adb serial 9d5f42fb.

### 7.2 Checklist (priority order)

#### A-S3b · P0 · likely (high) — Another operation's hash answered
- **Windows:** the relay's "already pending [existingHash:…]" answer (or AA25) made the desktop answer THIS request with the OTHER op's hash. Uniswap then called a swap done when only its approval had landed.
- **Why here:**
  - `feature/send/core/UserOpSpine.kt:361` — `parseExistingUserOpHash(message)?.let { existing -> … return existing }`. Any marker is returned as this op's hash, never compared with this op's own.
  - `feature/signing/core/SignExecutor.kt:175` — `opSubmitted(op.id, hash)` keys the record and the tracker to the other op. Line 183 waits for that op's receipt and answers with its tx hash.
  - `feature/send/core/SendExecutor.kt:399` — the wallet's own Send uses the same `spine.submit`, so a person's transfer can also be recorded under another op's hash.
  - `rust/crates/vela-core/src/user_op.rs:703` — `existing_op(message, own)` (ThisOne/Another) and `user_op_hash` are not exported to Kotlin.
  - `rust/crates/vela-core/src/user_op.rs:1714` — `relay_error_message` returns early for AA25 / 'invalid account nonce', so a marker in such an error is lost before parsing.
- **Check — PRIMARY (JVM, scaffolding from §7.1):**
  - Setup: `FakeRelayPort` answers `eth_sendUserOperation` with `{error:{code:-32602,message:'already pending [existingHash:0x<64 hex OTHER>]'}}`, and `eth_getUserOperationReceipt` for OTHER with `success:true`.
  - BAD (expected): `Submit(Succeeded(<OTHER's tx>))`, and `Ports.opSubmitted` receives OTHER.
  - GOOD: an error, or a wait followed by a resend of the same signed op. Never OTHER.
- **Device race** (parallel space, Gnosis, under $0.01 each):
  0. Put the page on Gnosis (§3 Money): Connect, Switch to Gnosis, Chain → `0x64`. The sheet must name Gnosis and xDAI.
  1. Tap Send dust in the test dApp and wait for the fee.
  2. From a second client on the same Safe, send 0.001 xDAI to 0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141 on Gnosis. Either:
     - the iPhone's parallel space (launch with `VELA_PARALLEL_SPACE=1`, §8.1 "Launch with logs", then 首页 发送); or
     - the hosted web wallet: `cd app-web/vela-wallet && pnpm install && pnpm dev`, open http://localhost:5173/zh/parallel, press "Enter (seed fixture wallet)", then make "Parallel Multi" (0x88cC…6894) the active account in the account switcher (Enter selects the first single-key account), then 发送; or
     - the Windows desktop's parallel space.
  3. Slide on the phone within about 2 s of that client's accept.
  - BAD: VelaLog `userop.submit previous op pending hash=…` (12 characters only); the page's result equals the other client's tx hash; 活动 holds two rows for one landed op.
  - GOOD: a clear "another transaction … pending" error, or this op's own hash after a wait.
- **Fix:** mirror ddb52da8 (desktop `executor/user_op.rs`, `relay.rs` `SubmitError::Occupied`).
  1. Export `user_op_hash` and `existing_op` through uniffi.
  2. In `UserOpSpine.submit`, compute the signed op's own hash and read the marker from the RAW error JSON, before `relayErrorMessage` rewords it.
  3. ThisOne → return it.
  4. Another → wait for it to land (about 2 min), re-read the nonce and resend; otherwise fail clearly. Never return Another's hash.

  This fixes Send and dApp signing together.

#### A-S2 · P0 · likely (high) — A reverted dApp operation answered as success
- **Windows:** eth_sendTransaction was answered with the bundle tx hash although `UserOperationEvent.success` was false. The bundle's status is 0x1 either way. The record closed Confirmed, and Uniswap and the sheet said done while nothing moved.
- **Why here:**
  - `feature/signing/core/SignExecutor.kt:227` — `if (answer is RelayClient.ReceiptAnswer.Resolved) return answer.txHash` drops `Resolved.confirmed`.
  - `feature/send/core/RelayClient.kt:449` — `confirmed = !(success == false)`. Only `TrackerExecutor.kt:91` reads it.
  - `SignExecutor.kt:242` — any receipt becomes `Succeeded(txHash)`. `rust/crates/vela-core/src/app/sign_request.rs:2346` then answers Ok(result) and closes the record Confirmed.
  - `feature/signing/core/SignWire.kt:224` — Kotlin `SignSubmitOutcome` has 5 variants, without reverted/not_confirmed, so the core's Reverted arm (`sign_request.rs:2433`) is unreachable.
  - `feature/signing/SigningAftercare.kt:41` — a result that is not the op hash is shown as Landed: a 已确认 tick.
  - `feature/wallet/core/FeedExecutor.kt:287` — `patchRecords` overwrites status unconditionally. The tracker's `failed` and the sign path's `confirmed` race, and the last write wins (§10 A-4).
  - `feature/browser/core/BrowserController.kt:458` — `userOpTxHash` translates an op hash to the bundle tx even when `confirmed=false`.
  - This is the live path: `VelaWalletApplication.kt:651` builds SigningController with the default 120 s wait, and `SigningController.kt:150` builds the SignExecutor.
- **Check — PRIMARY (JVM):**
  - Drive `SignExecutor.perform(SignOperation.SignAndSubmit(method='eth_sendTransaction', …))`.
  - `FakeRelayPort` answers `eth_sendUserOperation` with `{result:'0x<op>'}`, and `eth_getUserOperationReceipt` with `{result:{success:false, sender:<safe>, receipt:{transactionHash:'0xabc…', logs:[]}}}`.
  - BAD (expected): `SignShellResult.Submit(Succeeded('0xabc…'))`.
  - GOOD: `Reverted(user_op_hash, tx_hash)`, once the Kotlin wire has it.
- **Optional device repro.** Parallel space, Base, real funds. It is racy and may produce a relay refusal or a pending op instead. Only an on-chain `UserOperationEvent.success = 0` counts. On Windows this was found by review, not on chain.
  1. Open app.uniswap.org in 探索 and connect on Base. Paste the §7.1 logger in chrome://inspect, start a swap (0.0001 ETH→USDC will do: a USDC swap needs more USDC than the account holds, §3), read `to` from the `→ eth_sendTransaction` line (selector `0x3593564c`; on Base expect 0xd614…9c40, results.md F3), then close the sheet with ✕. Do not slide. If no `→` line appears, Uniswap is using its EIP-6963 provider: use 0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40 after confirming it with eth_getCode on Base. Then run `ROUTER='<captured>'` (no `const`, so the line can be re-run).
  2. In the console run:
     ```
     d=Math.floor(Date.now()/1000)+25; ethereum.request({method:'eth_sendTransaction',params:[{from:(await ethereum.request({method:'eth_accounts'}))[0],to:ROUTER,value:'0x0',data:'0x3593564c'+[0x60,0x80,d,0,0].map(n=>n.toString(16).padStart(64,'0')).join('')}]}).then(r=>console.log('OK',r),e=>console.log('ERR',e.code,e.message))
     ```
  3. Slide about 3 s before the deadline.
  - BAD: console `OK 0x<tx>`, the sheet shows 已确认 with a tick, and the 活动 detail says 已确认.
  - GOOD: `ERR -32603 The transaction was included but reverted (0x…)`, and the sheet shows the failure with the hash and an explorer link.
  - Verify on chain:
    ```
    curl -s https://mainnet.base.org -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionReceipt","params":["<tx>"]}'
    ```
    In the log from 0x0000000071727De22E5E9d8BAf0edAc6f37da032 with topic0 0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f, the second data word (success) must be 0.
- **Fix:** mirror ddb52da8 and 2448738a (desktop `executor/landing.rs`).
  1. `SignWire.kt`: add `SignSubmitOutcome.Reverted(user_op_hash, tx_hash)` and `NotConfirmed(user_op_hash)`. This also turns CoreWireDriftTest green.
  2. `SignExecutor.awaitReceipt`: decide landed vs reverted from `Resolved.confirmed` AND the core's own-logs rule. Export `op_execution_failed` / `user_op_outcome_in_logs` through uniffi.
  3. `BrowserController.userOpTxHash`: return null unless confirmed.
  4. `FeedExecutor.patchRecords`: never downgrade failed to confirmed, or let only the tracker close dApp records.

  Ship together with A-R3.

#### A-S3 · P1 · likely (high) — The userOpHash answered after a timeout
- **Windows:** after 90 s the page got the userOpHash, which no node knows, and the site waited on pending forever. The desktop now waits up to the core's `PAGE_WAIT_CAP_MS` (10 min), then answers "not confirmed yet". It never answers the op hash.
- **Why here:**
  - `SignExecutor.kt:41` — `receiptWaitMs = 120_000L`. `VelaWalletApplication.kt:651` keeps SigningController's default (`SigningController.kt:87`).
  - `SignExecutor.kt:242` — no receipt gives `ReceiptPending(userOpHash)`. `sign_request.rs:2402` then answers Ok(user_op_hash) and the record stays pending.
  - `sign_request.rs:116` — `PAGE_WAIT_CAP_MS` = `SLOW_POLL_AFTER_MS` = 10 min (`tx_tracker.rs:78`). Nothing on Android uses it.
  - `rust/crates/vela-core/src/app/dapp_browser.rs:1486` — the op hash is translated only for receipt lookups made through the wallet.
  - `feature/signing/SigningAftercare.kt:38` — an op-hash answer is shown as StillConfirming.
- **Check — PRIMARY (JVM, scaffolding from §7.1):** `SignExecutor(…, receiptWaitMs = 300, receiptPollMs = 50)`; `eth_sendUserOperation` → `FakeRelayPort.body("0x<op>")`, `eth_getUserOperationReceipt` → `FakeRelayPort.body(null)` (pending).
  - BAD (expected): `Submit(ReceiptPending("0x<op>"))`.
  - GOOD (after the port): `NotConfirmed("0x<op>")` at the cap, never the op hash as a result.
- **Device check** (parallel space, test dApp on Gnosis, under $0.01):
  0. Put the page on Gnosis (§3 Money): Connect, Switch to Gnosis, Chain → `0x64`.
  1. Start the chaos proxy in `mode=pass` and point the phone at it (§7.1).
  2. Arm the trigger from logcat, in the background or its own terminal. By hand the window is only about 3–5 s, too late.
     ```
     adb logcat -c; adb logcat -s VelaLog | { grep -m1 'relay.rpc.*eth_sendUserOperation'; curl -s 'http://127.0.0.1:8899/__chaos?mode=blackhole&match=vela-relay'; }
     ```
     The curl fires as soon as grep matches. (With `grep … && curl`, bash waits for `adb logcat` to exit first, which happens only on its next write, so the blackhole could land after the first receipt poll.) The relay.rpc line is written after the submit answer arrives, so the accept is not cut.
  3. Tap Send dust and slide.
  4. Wait 125 s, then read the `#out` entry for eth_sendTransaction.
  - BAD (expected): `{ok:true, result:R}`, the aftercare shows 已提交 / 仍在确认, and this returns `result:null` (R is an op hash):
    ```
    curl -s https://rpc.gnosischain.com -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionByHash","params":["R"]}'
    ```
  - GOOD: no answer before about 10 min, then `{ok:false, code:-32603, message:'The transaction was submitted but is not confirmed yet…(user operation 0x…)'}`.
  - Restore `mode=pass` and `adb shell settings put global http_proxy :0`. The tracker should then close the 活动 detail as 已确认.
- **Fix:** mirror ddb52da8.
  1. Export `PAGE_WAIT_CAP_MS` through uniffi.
  2. `awaitReceipt` waits up to the cap at the tracker's cadence, and returns `NotConfirmed(user_op_hash)` at the cap. It never returns `ReceiptPending` for a page.
  3. `SigningAftercare.of`: a not-confirmed error keeps following the op (the core's `answer_still_landing`).

#### A-R3 · P1 · likely (high) — A revert reads "couldn't be submitted — your funds are safe"
- **Windows:** an included op, which may have paid a fee, first read 交易未能提交…资金安全. The desktop now draws the revert at once: failedHint, tx hash and explorer link (2448738a).
- **Why here:**
  - `feature/signing/SigningLive.kt:441` — any non-reject error after approval (`pending_op_hash != null` or SubmitFailed) shows 失败 + `send.txErrorGeneric`.
  - `SigningLive.kt:548` — aftercare StillConfirming plus a tracker Dropped/Rejected shows the same sentence.
  - `rust/crates/vela-core/src/app/tx_tracker.rs:577` — a ReceiptFailed entry is shown as `TrackStatus::Dropped`.
  - `assets/i18n/zh.json:322` — txErrorGeneric = 交易未能提交。您的资金安全无虞——请重试。
  - Today a revert inside the 120 s wait reads 已确认 (S2). The false sentence shows only when the wait ran out and the tracker then reports Dropped. Once S2 is fixed, every revert lands on line 441 and shows it at once. So R3 must ship with S2.
- **Check — prefer a UI unit check:** call `SigningLive.aftercareReceipt(StillConfirming(op), …)` with `ctx.track.status = Dropped`.
  - BAD (expected): captions contain `send.txErrorGeneric`.
  - GOOD: `componentsTx.receipt.statusFailed` + `componentsTx.receipt.failedHint`, the tx hash, and 在浏览器中查看.
- **On the device:** hard to stage. Combine S2's deadline trick with S3's logcat-triggered blackhole, then restore `mode=pass` so the tracker reads success=false.
  - BAD: 失败 · 交易未能提交。您的资金安全无虞——请重试。
- **Fix:** mirror 2448738a. The desktop draws `statusFailed` + `failedHint` (`app-desktop/vela-wallet/src/signing/mod.rs:302-303`).
  1. Export `sign_request::reverted_transaction` (and `REVERTED_MESSAGE`) through uniffi.
  2. Draw `statusFailed` + `failedHint` + hash + explorer:
     - in `SigningLive.receipt`, when `reverted_transaction(sign.error.detail)` is Some;
     - in `aftercareReceipt`, for Dropped.
  3. Keep `txErrorGeneric` only for failures before submission.

#### A-U1 · P1 · likely (medium) — Max USDC → ETH: the fee coin is drained by the swap
- **Windows:** the fee was auto-picked in USDC, and the fee leg runs after the swap, which took every USDC. The relay's simulation failed and the sheet said 无法连接 Vela 服务 forever. Fixed by core `Event::BalanceChangesMeasured` (the fee coin pays from what the op LEAVES), fed by the shell (380d9014, 5825e443).
- **Why here:**
  - `feature/signing/core/SigningController.kt:303` — `speedControl.ask(…, calls = feeCalls, autoFeeToken = true)`: the fee machine weighs the calls alone.
  - `SigningController.kt:309` — the simulation's judgments go only to `_sim`. No `BalanceChangesMeasured` is ever sent; a grep for balance_changes / BalanceChangesMeasured / FeeBalanceChange in app-android `src/main` finds nothing.
  - `rust/crates/vela-core/src/app/fee_policy.rs:558` — `EstimateUserOpGas` simulates "user calls + fee leg".
  - `feature/send/core/FeeExecutor.kt:139` — a relay refusal becomes SimulationFailed, never Refused.
  - `fee_policy.rs:2872` — a large op (calldata over 1024 bytes, line 119) plus SimulationFailed gives EstimateFailed. Only Refused tries the next coin.
  - Confidence is medium only because the auto-pick depends on that day's balances.
- **Precondition — without it a GOOD proves nothing.** The auto-pick weighs each coin against a static fee BEFORE simulating (`fee_policy.rs:2258-2270`, `auto_pick` at 2578). With too little USDC, ETH is picked anyway and the run looks GOOD.
  - Before Max, hold USDC well above the USDC fee the sheet quotes (about 0.085 USDC on Base): aim for at least 0.2. results.md's Max run used 0.271741.
  - On 2026-09-29 the account held only about 0.035 USDC (§3), so top up first: one ETH→USDC 0.0001 swap adds about 0.27 USDC for about 0.000031 ETH. More funding needs the owner.
  - Control: on the same balance, a 0.05 USDC→ETH sheet (do not slide) must auto-pick USDC. If it picks ETH, the balance is too low.
- **Check** (parallel space, Base, app.uniswap.org; the precondition above holds):
  1. Start USDC→ETH with **Max**. Do not slide.
  2. Watch the fee row and VelaLog.
  - BAD (expected):
    - the fee coin is USDC;
    - the row reads 点击重试 with 无法连接 Vela 服务 — 请检查网络，稍后会自动重试。;
    - the slide stays disabled;
    - VelaLog repeats `send.fee estimate refused why=…simulation failed…` (3/3/6/12/15 s).
  - GOOD (desktop): the fee is auto-picked in ETH (about 0.000031 ETH), and the slide is live by about 9 s.
- **Fix:** mirror 380d9014 and 5825e443 (desktop `wallet/signing_host.rs`, `speed_control.rs`, `executor/relay.rs`).
  1. `FeeWire.kt`: add `FeeEvent.BalanceChangesMeasured` and `FeeBalanceChange`, mirroring `app-web/vela-wallet/src/lib/core/generated/FeeBalanceChange.ts`.
  2. `SigningController`: when `_sim` becomes Ready, hand its deltas to every fee session through SpeedControl.
  3. `FeeExecutor`: map a relay refusal to `FeeGasOutcome.Refused` (A-U1b).

#### A-U1b · P1 · likely (high) — A relay refusal drawn as "check your network"
- **Windows:** EstimateFailed ("simulation failed") was shown as 无法连接 Vela 服务 — 请检查网络, and retried forever. Fixed: core `FeeGasOutcome::Refused` becomes `FeeFailure::WouldFail`, and the desktop has a sentence for it.
- **Why here:**
  - `FeeExecutor.kt:137` — `EstimateAnswer.Refused` and `Unreachable` (line 141) both become `SimulationFailed`.
  - `RelayClient.kt:372` — Refused already carries the relay's raw message, so the fix is small.
  - `SigningLive.kt:839` — every failure except MissingPublicKey, CalculationFailed and WouldFail shows `componentsUi.funding.denialNetworkError`. The value (line 805) is `componentsUi.gas.estimateFailed` 点击重试.
  - `SigningController.kt:340` — `feeRequoteDelayMs(estimate_failed)` is Some (`fee_policy.rs:759-770`), so the quote is re-asked forever while the sheet is up.
  - 083 already added the Kotlin wire: `FeeGasOutcome.Refused` and `FeeFailure.WouldFail` exist, SendController maps WouldFail to EstimateFailed, and SigningLive keeps WouldFail off the network sentence. Nothing produces Refused yet.
- **Check:** the A-U1 run (Max USDC→ETH on Base, do not slide).
  - BAD: 点击重试 + 无法连接 Vela 服务 — 请检查网络，稍后会自动重试。, with the spinner cycling and repeated `relay.rpc eth_estimateUserOperationGas` lines while the network is fine.
  - GOOD: 这笔交易预计会失败。 with no auto-retry.
  - Negative control: Send dust on Gnosis with the proxy black-holing vela-relay must keep the network sentence (079).
- **Fix:** mirror 380d9014 and 5825e443.
  1. `FeeExecutor.estimate`: a Refused answer that passes the refusal rule of §2.4 becomes `FeeGasOutcome.Refused`. Anything else stays SimulationFailed.
  2. `SigningLive.feeModel`: WouldFail shows the first clause of `componentsUi.signing.simWillFail` (see U1d), not denialNetworkError.
  3. Re-quoting then stops by itself: `requote_delay_ms(WouldFail)` is None.

#### A-U1c · P1 · likely (high) — Fee coin list unusable over a failed quote
- **Windows:** the fee row's ">" did nothing once the quote had failed. The desktop now opens the list whenever another coin can be chosen (380d9014).
- **Why here:**
  - `SigningController.kt:248` — feeTapped does `view.failed != null -> speedControl.refresh()`, so the list toggle on line 249 is unreachable over a failed quote.
  - `navigation/VelaNavHost.kt:733` — `onFee = controller.feeTapped()` is the row's tap. Refresh has its own `onRefreshFee` (line 734).
  - `SigningLive.kt:847` — the chevron shows when the core has options. The core fills options even when failed (`fee_policy.rs:3635` option_views), so a ">" is drawn that cannot open.
  - `fee_policy.rs:3465` — `select_fee_asset` ignores a pick unless the phase is Quoted or Failed(WouldFail). Under Android's EstimateFailed a picked coin does nothing, so U1c works only together with U1b.
- **Check:** the A-U1 run with the failed quote. Tap the fee row and its ">" 3 times, then dump the UI.
  - BAD (expected): only the refresh spinner. uiautomator shows no ETH option row with a balance.
  - GOOD (after U1b + U1c): the list opens, ETH can be chosen, and picking it re-quotes and opens the slide. Do not slide unless you mean to spend about $0.08.
- **Fix:** mirror 380d9014.
  1. `feeTapped`: toggle feeOpen when `options.size > 1`, even if the quote failed. Refresh stays on onRefreshFee.
  2. `SigningLive.feeModel`: build the options whether or not the quote failed.
  3. Land together with U1b.

#### A-U8 · P1 · likely (high) — Fee about 14× the on-chain cost (owner decision)
- **Windows:** on Base, $0.098 was charged against $0.0068 of real cost:
  - ×3 `INBAND_MARKUP`;
  - ×3.02 from pricing on padded limits;
  - ×1.57 from the default Fast speed.

  A 0.1 USDC swap paid 0.085 USDC. Not fixed; this is the owner's decision (§11.2).
- **Why here:** pricing is the shared core's, and Android feeds it the same inputs.
  - `fee_policy.rs:91` — `INBAND_MARKUP = 3`, applied at line 927.
  - `fee_policy.rs:2852` — the gas limits are ×1.5, raised to the measured inner floor, and the fee is priced on them.
  - `rust/crates/vela-core/src/app/fee_tier_pref.rs:57` — `FACTORY_DEFAULT = FeeTier::Fast`.
  - `VelaWalletApplication.kt:659` — `preferredTier = settings.feeTier.value.tier`.
  - `FeeExecutor.kt:78` — MeasureInnerCalls is answered like the desktop's.
- **Check** (parallel space, Base, about $0.08; Settings' default speed should read 快):
  1. Swap 0.0001 ETH→USDC on app.uniswap.org. This can be the §3 top-up, and its row serves A-H2, A-F1 and A-F3 too. Screenshot the sheet's fee (expect about 0.00003 ETH ≈ $0.08) and the speed chip.
  2. After it lands, take the tx hash from the aftercare. Read `UserOperationEvent.actualGasCost` and the fee transfer to the relay's fee recipient on basescan, or with `cast receipt <tx> --rpc-url https://mainnet.base.org`.
  - Expected: fee ≈ 14× actualGasCost. Report both numbers if the ratio differs materially.
- **Fix:** none on Android. It is a core `fee_policy` change per the owner's decision; ffc9ac9f documents the math.

#### A-H2 · P1 · partially (high) — A dApp transaction is not in 活动
- **Windows:** dapp_tx records existed but the feed showed only sends and receives. Fixed: core `activity_feed` emits dapp_tx rows with FeedDapp (43fd67a4, ff660c2d). The web maps them.
- **Why here:** the core half reaches Android in a build from this branch (the row appears). The shell half is missing.
  - `rust/crates/vela-core/src/app/activity_feed.rs:951` — the core emits every dapp_tx as a row. Android stores dapp_tx records (`SignExecutor.kt:288`), and `FeedExecutor.kt:183` maps the type.
  - `FeedExecutor.kt:156` — `record()` maps no dappUrl, intent, balance_changes or calldata, although recordRow stores `intent` (`SignExecutor.kt:311`).
  - `feature/wallet/core/FeedWire.kt:124` — FeedItem has no `dapp`, and FeedTxRecord (line 70) lacks dapp_url/intent/balance_changes/calldata. The core's FeedDapp is dropped by `ignoreUnknownKeys` (`core/crux/Wire.kt:47`).
  - `feature/wallet/WalletLive.kt:173` — every outgoing row is titled `history.labelSent` 已发送, with the subtitle `history.toName` '至 0x…'.
  - `feature/wallet/WalletModels.kt:62` — ActivityRowModel has no status field, so a row can never read "pending". Only the detail's status chip does, and it comes from tx_hash (§10 A-4).
  - `SignWire.kt:160` — the Kotlin SignRecord lacks dapp_url and balance_changes, which the core sends (`sign_request.rs:328-363`).
- **Check:** build from this branch and prove the core (§7.1 Build step 5). In the parallel space, use this platform's single Uniswap swap on Base (§3; the A-U8 top-up will do), or on Gnosis use the test dApp's Approve unlimited, cap it to 1 and slide (under $0.01).
  1. Right after submission, open 首页 活动.
  2. After it lands, open the row.
  - BAD (expected):
    - the row reads 已发送 · 至 0x…(router/USDC);
    - no site is named;
    - a no-value call shows no amount;
    - the detail's title is '已发送 ' with a bare '−'.

    An APK built from main shows no row at all.
  - GOOD:
    - the title is the intent (兑换/授权) or 合约交互;
    - the subtitle is app.uniswap.org / 127.0.0.1;
    - the detail's chip reads 处理中 until VelaLog `tracker.patch confirmed`, then 已确认.
- **Fix:** mirror 43fd67a4 and ff660c2d (web `feed-executor.ts`; desktop `wallet/live.rs`, `flows/live.rs`).
  1. `SignWire.SignRecord`: add `dapp_url` and `balance_changes`.
  2. `SignExecutor.recordRow`: store dappUrl from `record.dapp_url`.
  3. `FeedExecutor.record`: map dappUrl → dapp_url and intent → intent.
  4. `FeedWire`: add the FeedTxRecord fields, `FeedItem.dapp`, FeedDapp and FeedDappChange, mirroring the generated TS files.
  5. `WalletLive.activityRow`: title from `dapp.intent_term` (`componentsUi.signing.*`), else `componentsUi.signing.intentContractCall`. Subtitle `dapp.site`.
  6. `FlowLive.txDetail`: add an app row.

#### A-REC · P1 · partially (high) — dApp record integrity
- **Windows:** several ways a page could corrupt the record (fixed in core, desktop and web, ff660c2d):
  - any text in wallet_sendCalls' top-level `to` became the counterparty;
  - a byte-based shortener panicked on 日本語 at every launch;
  - the site came from the dApp's self-declared name;
  - the web cut a numeric value.
- **Why here:** the crash half and the self-declared-name half are safe:
  - `WalletLive.kt:204` shortens by characters;
  - `SigningController.kt:286` sends RequestArrived with `dapp = null`, so the origin is the browser's.

  Still present:
  - `SignExecutor.kt:288` — a DappTx record keeps `params[0].to` and `params[0].value` for wallet_sendCalls too. Both are page-authored and never shown on the sheet.
  - `activity_feed.rs:1063` — with no balance_changes the figure is the stored value, scaled: 0xffff…ffff becomes about 1,208,925 xDAI. Line 1100 drops only a non-address `to`.
  - `FeedExecutor.kt:152` — `hasSentTo` counts any dapp_tx `to`, so a page can switch off the first-time-recipient (address-poisoning) tag for an address of its choice.
- **Check** (parallel space, test dApp console on Gnosis; harmless, under $0.01 — it sets the USDC allowance for 0x1111… to 0):
  ```
  __ask('wallet_sendCalls',[{version:'2.0.0',from:me(),chainId:'0x64',to:'0x000000000000000000000000000000000000dEaD',value:'0xffffffffffffffffffff',calls:[{to:'0xDDAfbb505ad214D7b80b1f830fcCc89B60fb7A83',data:'0x095ea7b3'+'1111111111111111111111111111111111111111'.padStart(64,'0')+'0'.repeat(64),value:'0x0'}]}])
  ```
  Slide, then:
  1. Open 活动.
     - BAD (expected): a row −1,208,925.8… XDAI 至 0x0000…dEaD.
     - GOOD: no figure and no counterparty.
  2. Open 转账 to 0x…dEaD.
     - BAD: no first-time-recipient tag.
     - GOOD: the tag shows.
  3. Repeat with `to:'日本語日本語'`, then force-stop and relaunch.
     - Expected: no crash, and the row has no counterparty.
- **Fix:** mirror ff660c2d (desktop `executor/sign_request.rs`, web `dapp-history.ts`).
  1. `recordRow`: for wallet_sendCalls, store to `""` and value `"0x0"`.
  2. Store dappUrl (add dapp_url to SignRecord).
  3. Exclude page-written batch rows from `hasSentTo`.

#### A-D1b · P1 · via core (medium) — Dismissed sheet + cancelled passkey never answered **[owner]**
- **Windows:** a request whose sheet was dismissed, and whose passkey was then cancelled, never answered the page. The core now has a safety net that answers one 4001 (core safety net 4198ec6b; 545621d7 touches no core file).
- **Why here:** Android feeds the core exactly the inputs the safety net needs.
  - `sign_request.rs:2331` — PasskeyCancelled while off the sheet, with the page not gone and no op hash, answers one 4001. Its comment names the phones' case: ✕, then a dismissed passkey. Line 2066: a dismiss past the commitment point clears the sheet but keeps the pipeline.
  - `SignExecutor.kt:187` — a cancelled ceremony reports PasskeyCancelled (line 209 for messages).
  - `VelaNavHost.kt:726` — ✕ calls `controller.swipeDismissed()`, i.e. `SignEvent.SwipeDismissed`. That is not cancel/TransportDropped, so `page_gone` stays false.
  - `VelaWalletApplication.kt:643` — while an unanswered controller lives, every later page request gets -32002 'Another request is open'.
- **Check** (owner's account; the passkey is cancelled, so no fingerprint and no funds; test dApp Send dust on Gnosis; reload the page and paste the §7.1 logger first):
  1. Slide.
  2. During 提交至网络… and before the Credential Manager sheet appears (about 1–2 s), tap ✕ / 在后台关闭.
  3. Press Back on the passkey sheet.
  - GOOD: the logger shows exactly one `✗ eth_sendTransaction 4001`, and a second Send dust opens a fresh sheet.
  - BAD (main, or an older core): no result ever, and the next request gets -32002 'Another request is open'.
- **Fix:** none in the shell. Ship a core from this branch.

#### A-W20 · P1 · via core (high) — iPhone caBLE: lower-case tunnel ids **[owner + iPhone]**
- **Windows:** Apple's cable.auth.com closed the tunnel with 1008 "Policy violation". The core's `cable/session.rs` `nibble()` is upper-case since 24578479.
- **Why here:**
  - `feature/onboarding/core/HybridCeremony.kt:141` — the tunnel URL is the core's `cableConnectUrl(session.staticSeed, session.qrSecret, hit.advert.plaintext)`, passed to `WebSocketCableConn.connect(url)`. The shell builds no URL itself.
  - `rust/crates/vela-core-uniffi/src/ctap_bridge.rs:701` — `cable_connect_url` calls `CableInitiator::connect_url`.
  - `rust/crates/vela-core/src/cable/session.rs:155` — upper-case since 24578479.
  - `app-android/vela-wallet/app/build.gradle.kts:134` — cargoNdkBuild recompiles jniLibs unless `-PvelaSkipRustBuild` is passed.
  - An advert with a PSM goes over L2CAP and needs no URL.
- **Check:** build WITHOUT `-PvelaSkipRustBuild`, with the owner's account on the Android device and the owner's iPhone.
  1. In the test dApp tap Sign, slide, scan with the iPhone, confirm with Face ID.
  2. Watch `adb logcat -s VelaLog | grep -iE 'cable'`.
  - GOOD: "advert has no PSM — WebSocket tunnel", then "cable.tunnel WebSocket tunnel established", then cable.ctap lines; a signature; Verify sign → `valid:true`.
  - BAD: the tunnel closes right after the first frame, or `#out` shows -32603.
  - Optional A/B: a build from before 24578479.
- **Fix:** none. Just make sure jniLibs are rebuilt.

#### A-R2 · P2 · via core (high) — A bundle neighbour's ExecutionFailure fails our op
- **Windows:** receipt logs cover the whole bundle, so any Safe ExecutionFailure topic anywhere in it failed our op. The core now scopes the check to the op's own execution logs (2448738a).
- **Why here:**
  - Android has no execution-failure rule of its own: a grep for 23428b18, ExecutionFailure, safeExecutionFailed and opExecutionFailed in `src/main` finds nothing.
  - `feature/send/core/TrackerExecutor.kt:92` — a confirmed receipt with logs becomes `ReceiptWithLogs(hash, txHash, now, logs)`.
  - `RelayClient.kt:445` — the logs are the whole bundle's, with address/topics/data kept, so the EntryPoint boundaries reach the core.
  - `tx_tracker.rs:687` — the core runs `op_execution_failed(&logs, &user_op_hash)`.
  - The page-answer path (S2) reads no logs. Whoever fixes S2 must reuse this rule, not a whole-receipt topic scan.
- **Check:** not stageable on a device, because a neighbour's failure cannot be arranged on the shared relay.
  - Run `cd rust && cargo test -p vela-core --test app_tx_tracker`.
  - Device smoke test: after any parallel-space swap or send on a build from this branch, VelaLog shows `tracker.patch confirmed` for its ids, not `failed`.
- **Fix:** none in the shell; ship the core. For S2, route SignExecutor's receipt through the same rule (desktop `executor/landing.rs::executed`).

#### A-F1 · P2 · likely (high) — A dApp row says nothing about what it moved
- **Windows:** Uniswap swap rows read 合约交互 with no amount, although the sheet showed 余额变化. Fixed:
  - the shell passes the sheet's simulated balance changes with the approval;
  - the core carries `balance_changes` with trust flags on SignRecord and FeedTxRecord;
  - the row reads "≈ −0.05 USDC / ≈ +0.000019 ETH".

  Commits 48b3e931, 9004bba3, b2430a4e.
- **Why here:**
  - `SigningController.kt:439` — approveOpts sends no balance_changes, although `_sim` (line 309) holds what the sheet drew.
  - `SignWire.kt:148` — SignApproveOpts has no balance_changes, and SignRecord (line 160) has none either.
  - `feature/wallet/core/TrustWire.kt:301` — Erc20Trusted lacks `in_trusted_set` (`token_trust.rs:708`), which the core's figure rule needs.
  - `activity_feed.rs:1063` — with no recorded changes the figure falls back to the call's value. A no-value call has none.
  - `feature/flows/FlowLive.kt:270` — the detail's hero is '−<value> <symbol>': a bare '−' for a no-value call.
- **Check:** use this platform's single Uniswap swap on Base (§3; the A-U8 ETH→USDC top-up will do). Screenshot 余额变化 before sliding, then compare it with the 活动 row and the detail.
  - BAD (expected):
    - USDC→ETH has no figure and a bare '−' hero;
    - ETH→USDC shows only −0.0001 ETH;
    - the detail has no 余额变化.
  - GOOD:
    - the row reads ≈ −X USDC / ≈ +Y ETH;
    - the detail lists the sheet's lines;
    - a failed op shows no simulated figure.
- **Fix:** mirror 48b3e931, 9004bba3, b2430a4e (desktop `executor/sign_request.rs`, `wallet/live.rs`, `flows/live.rs`). Needs A-H2's wire first.
  1. Add `in_trusted_set` to the Kotlin Erc20Trusted.
  2. Send `_sim`'s judgments in `SignApproveOpts.balance_changes`; the core copies them into `SignRecord.balance_changes`.
  3. Store them in recordRow, and map them back into `FeedTxRecord.balance_changes` (a TrustSimJudgment list).
  4. Draw `FeedDapp.changes` / `received` / `estimated` with '≈'.

#### A-F3 · P2 · likely (high) — The called contract labelled 接收方
- **Windows:** the Universal Router was shown as 接收方. The desktop now labels a call that carries calldata 合约 (`tokenDetail.labelContract`) (48b3e931, 9004bba3).
- **Why here:**
  - `FlowLive.kt:234` — the counterparty label is only DETAIL_FROM or DETAIL_TO. `componentsTx.detail.to` = 接收方 (`assets/i18n/zh.json:1128`).
  - `FlowLive.kt:262` — a router call is titled `history.txLabelSent` '已发送 {{symbol}}'.
  - `SignExecutor.kt:285` — recordRow writes no calldata flag, so `FeedDapp.contract_call` can never be true.
- **Check:** open the detail of a Uniswap swap's row (or a test dApp Approve's row).
  - BAD (expected): 接收方 0x… (the router or the USDC contract).
  - GOOD: 合约 0x… for a call with calldata, while a plain Send dust keeps 接收方.
- **Fix:**
  1. In recordRow, record `calldata` from the full request (callsOf, before clipping), and map it into `FeedTxRecord.calldata`.
  2. `FlowLive.txDetail`: use `tokenDetail.labelContract` when `item.dapp.contract_call` is true.

#### A-W11 · P2 · likely (medium) — Wrong progress label around the fingerprint prompt **[owner]**
- **Windows:** 等待生物识别… showed for seconds while network steps ran, before the passkey had asked anything. The desktop now says 正在准备交易… until the prompt appears (545621d7).
- **Why here:**
  - `VelaWalletApplication.kt:690` — `override fun signingStarted() = Unit`: the spine's "prompt is about to open" signal (`UserOpSpine.kt:330`) is thrown away.
  - `SigningLive.kt:481` — while is_submitting (deploy/nonce/estimate reads, the passkey prompt, the relay submit) the sheet shows `send.txSubmitting` 提交至网络...
  - `SigningLive.kt:491` — is_signing alone shows `send.txSigning` 等待生物识别..., only during the instant precheck.
  - `sign_request.rs:1238` — is_signing / is_submitting states. Android answers precheck and sponsorship instantly (`SignExecutor.kt:100, 104`).
  - `feature/send/SendLive.kt:1035` — Android's own Send already uses `send.txPreparing` / `txPreparingBiometric`. The dApp sheet does not.
- **Check** (owner's account with fingerprint; test dApp Send dust on Gnosis, under $0.01):
  1. Start `adb shell screenrecord /sdcard/w11.mp4`.
  2. Slide and complete the fingerprint.
  3. Pull the recording and step through it frame by frame.
  - BAD (expected): 提交至网络… (with 关闭此页交易会在后台继续) from about 0.1 s after the slide, both before and under the fingerprint sheet.
  - GOOD: 正在准备交易… until the prompt; 等待生物识别… while it is up; 提交至网络… after.
- **Fix:** mirror 545621d7 (desktop `signing/status.rs`).
  1. Implement `Ports.signingStarted` as a StateFlow the sheet reads.
  2. Add a "prompt answered" hook after `ceremony()` in `UserOpSpine.submit` and `signMessage`.
  3. `SigningLive.receipt`: txPreparing before the prompt, txSigning during it, txSubmitting after.

#### A-H4a · P2 · likely (high) — A failed MESSAGE signature shows the transaction-failure sentence
- **Windows:** a personal_sign failure showed "交易未能提交…请重试" with only 完成. The desktop now says `error_off_chain` 链下签名 — 未向链上发送任何内容。 (a22e1b30).
- **Why here:**
  - `SigningLive.kt:427` — onChain is only checked for the signing and submitting states.
  - `SigningLive.kt:441` — any SubmitFailed error shows 失败 + `send.txErrorGeneric` + 完成, for messages too.
  - `SignExecutor.kt:198` — an unhashable message gives Failed. Line 210: a non-cancel passkey failure gives Failed. The core then runs `fail_inflight(SubmitFailed)` (`sign_request.rs:2499`).
  - `rust/crates/vela-core/src/sign_message.rs:32` — `original_hash` returns None for an empty personal_sign payload or for typed data that cannot be hashed. That gives a deterministic trigger.
  - `assets/i18n/zh.json:955` — `connect.detail.offChainNote` exists, unused here.
- **Check** (parallel space, no funds). In the test dApp console run either of these, then slide:
  - `__ask('personal_sign',['', me()])`
  - `__ask('eth_signTypedData_v4',[me(),'{"primaryType":"X","types":{},"domain":{},"message":{}}'])`

  If the core refuses before the sheet (a -32602, or no sheet), use the owner's account and make the passkey fail without cancelling it.
  - BAD (expected): 失败 · 交易未能提交。您的资金安全无虞——请重试。 with 完成.
  - GOOD: 失败 · 链下签名 — 未向链上发送任何内容。
- **Fix:** mirror a22e1b30 (desktop `signing/status.rs` `a_failed_message_is_not_a_failed_transaction`). In SigningLive.receipt's error branch, show `connect.detail.offChainNote` instead of txErrorGeneric when `!onChain`.

#### A-H4b · P2 · partially (medium) — Passkey failures reach the page as -32603 with raw text **[owner]**
- **Windows:** a phone or authenticator transport failure answered the dApp -32603 with raw English. The desktop now keeps the request open under Retry/Close (a22e1b30, 95b8e573, 1063909f). Owner decision 2 in §11.2 applies.
- **Why here:** Android has no caBLE of its own for Credential Manager's hybrid. A cancel or a hybrid timeout usually arrives as NotAllowed, i.e. cancelled, which keeps the request open, as the desktop does now. But:
  - `feature/onboarding/core/PasskeyExecutor.kt:925` — classifyGet: Cancellation and DOM NotAllowed become Cancelled; NoCredential and everything else become `Other(describe(error))`, the raw exception message. This is the signing path (getPinned, lines 627/631).
  - `UserOpSpine.kt:150` — Cancelled becomes `Refused(PasskeyCancelled)`; anything else becomes `Failure.Other(message)`.
  - `SignExecutor.kt:190` — that becomes `SignSubmitOutcome.Failed(message)`, and the page gets -32603 with that text.
  - How a hybrid timeout is classified depends on the vendor, hence medium confidence.
- **Check** (owner's account, plus a second device for hybrid; test dApp Sign, no funds):
  - (a) Slide, then press Back on the Credential Manager sheet. Expected: back on the sheet, slide usable, and no personal_sign result yet.
  - (b) Slide, choose 使用其他设备 (hybrid QR), then let it time out or close the other phone's prompt. Read the page's result and the VelaLog lines `passkey.assert refused … type=` and `sign.submit refused why=`.
  - BAD: `{ok:false, code:-32603, message: <androidx/English exception text>}` plus 交易未能提交.
  - GOOD (per owner decision 2): the request stays open with Retry/Close, and the page gets exactly one answer.
- **Fix:** get owner decision 2 first, then mirror a22e1b30, 95b8e573, 1063909f.
  1. Classify `GetCredentialInterruptedException` and hybrid/transport failures as retryable.
  2. Put any -32603 detail in plain words, not the exception's debug text.

#### A-B5792 · P2 · partially (medium) — wallet_sendCalls answer / capabilities
- **Windows:** the desktop answers wallet_getCapabilities with 4200, so Uniswap uses approve + Permit2 + swap. The web returned a batch id as a "confirmed" record under an op hash; fixed to receipt_pending (ff660c2d).
- **Why here:**
  - `rust/crates/vela-core/src/app/dapp_rpc.rs:172` — wallet_getCapabilities and wallet_getCallsStatus appear nowhere in vela-core's routes, so they are Unsupported and get 4200, as on the desktop.
  - `SignExecutor.kt:268` — wallet_sendCalls is submitted as one op. The answer is a bare tx-hash string (or the op hash after 120 s, S3), never `{id}`.
  - `SignExecutor.kt:183` — the record closes only after a receipt, so the web's "confirmed under an op hash" does not happen.
  - So a site that uses wallet_sendCalls gets a bare hash, cannot ask wallet_getCallsStatus, gets a reverted batch answered as success (S2), and has its page-set to/value stored (REC).
- **Check** (parallel space, test dApp console on Gnosis). Record all four answers:
  1. `__ask('wallet_getCapabilities',[me()])` → expect `{ok:false, code:4200}`.
  2. Tap Bundle (approve + reset, under $0.01) and slide → expect a plain '0x…' tx hash (check it on gnosisscan), not `{id}`.
  3. `__ask('wallet_getCallsStatus',[<that hash>])` → 4200.
  4. The 活动 detail shows 处理中, then 已确认.
- **Fix:** a product decision on EIP-5792: the core's dapp_rpc allowlist plus a tracker-backed getCallsStatus that reports reverted too. The Android part rides on S2/S3 and REC.

#### A-W1 · P2 · likely (medium) — WebView cannot start: the app dies or says nothing **[emulator only]**
- **Windows:** the WebView2 profile folder sat beside the exe, the engine failed with 0x80070005, and the browser stayed blank forever, retried on every paint. The desktop now shows an EngineFailure panel with the code, Retry and "open in system browser" (dbf5a48c, 672d1dbb).
- **Why here:** the Android analog is a missing, disabled or updating WebView provider, where `WebViewFactory` throws `MissingWebViewPackageException`.
  - `feature/browser/core/BrowserController.kt:125` — `val webView: WebView = WebView(context)` is a property initialiser with no try/catch.
  - `BrowserController.kt:620` — reconcile's `engines.getOrPut(selected.id) { newEngine(...).also { it.load(url) } }` is unguarded, and so is `open()` (line 745). reconcile runs on entering 探索 when the selected tab has a saved URL.
  - `BrowserController.kt:399` — debug builds call `WebView.setWebContentsDebuggingEnabled` in the controller's init (debuggable = BuildConfig.DEBUG, `VelaWalletApplication.kt:485`).
  - `navigation/VelaNavHost.kt:638` — the lazy controller is built when the signed-in WALLET route composes. `MainActivity.kt:318/443` also build it for vela.openUrl.
  - `core/diagnostics/CrashReport.kt:23` — the global handler records the crash and passes it on, so the process dies.
  - `feature/browser/core/ProviderBridge.kt:40` — without DOCUMENT_START_SCRIPT / WEB_MESSAGE_LISTENER it logs `browser.inject provider unavailable`, and the page loads with no wallet. `BrowserController.kt:146` sets `EngineState.wallet`, but nothing reads it.
- **Check — EMULATOR ONLY.** Never touch WebView on the owner's phone.
  1. Use a Google APIs AVD (x86_64). Install the debug APK, enter the parallel space, open the test dApp once (this saves a tab with a URL), then force-stop.
  2. `adb shell dumpsys webviewupdate` names the provider. Disable it with `adb shell pm disable-user --user 0 <pkg>`. If that is refused, use `adb shell pm uninstall -k --user 0 <pkg>`.
  3. Launch the app and watch `adb logcat *:E | grep -iE 'FATAL|WebView'`.
     - BAD (expected on debug): FATAL EXCEPTION … MissingWebViewPackageException (or "No WebView installed") as the wallet home appears, with a stack through `BrowserController.<init>` → `setWebContentsDebuggingEnabled`.
     - GOOD: the wallet opens, 探索 shows a Vela panel 无法加载此页面 with the reason, 重试 and 在系统浏览器中打开, and the app keeps running.
  4. The release-only path (`BrowserEngine.<init>` from reconcile/open) cannot be reached in a debug build, because init throws first. That path is confirmed by code reading only. A release APK needs hand signing and a real sign-in, so it is optional.
  5. Restore with `adb shell pm enable <pkg>`, or `adb shell cmd package install-existing <pkg>` after an uninstall.
  6. Optional: on an API 29 image with its stock, never-updated WebView, open the test dApp. If logcat shows `browser.inject provider unavailable`, the dApp says it found no wallet. BAD: Vela says nothing.
- **Fix:** mirror dbf5a48c and 672d1dbb.
  1. Wrap `setWebContentsDebuggingEnabled` and BrowserEngine construction (newEngine/reconcile/open) in runCatching.
  2. On failure keep a per-tab engine-failure state, and draw BrowserNotice with:
     - `connect.browser.loadFailed` plus the platform reason;
     - `connect.browser.retry`;
     - `explore.openInSystemBrowser`.

     These are the keys dbf5a48c reused. Retry only when the person asks.
  3. Show a one-line notice when `EngineState.wallet == false`.

#### A-W9 · P2 · likely (high) — Demo host and fixture menus in a live session
- **Windows:** the address bar showed "🔒 app.uniswap.org" with nothing loaded, and a fresh profile had demo tabs. The desktop now shows only the host or tab being opened (dbf5a48c).
- **Why here:**
  - `navigation/VelaNavHost.kt:1200` — the live 探索 model starts from `ExploreFixtures.buildState(ExploreScreenState.E2, strings).withIdentity(...)`, and withIdentity swaps only the account.
  - `feature/browser/ExploreLive.kt:195` — `host = engine?.host?.ifBlank { null } ?: tab?.origin?.substringAfter("://") ?: fallback.browser.host`. A new engine has host "", and the core creates the DbrTabView only at NavigationStarted (`rust/crates/vela-core/src/app/dapp_browser.rs:518`). Until then the bar shows "app.uniswap.org". `secure` (line 196) is false, so the bar draws the orange open lock (`feature/explore/components/BrowserChrome.kt:142`).
  - `ExploreLive.kt:194` — `url = engine?.url ?: fallback.browser.url`: on a crashed tab the pill's editor opens on "app.uniswap.org" (`BrowserChrome.kt:110`).
  - `ExploreLive.kt:213` and `:226-244` — `connection ?: fallback.connection` and `siteMenuSheet ?: fallback.siteMenuSheet`. The fixture site menu (`feature/explore/ExploreFixtures.kt:117`) is Uniswap with 安全站点 and 断开连接; line 132 gives "安全站点 · 已连接".
  - `feature/explore/ExploreScreen.kt:264` / `:245` — the account chip and ⋯ reach these on the crash panel. `BrowserController.kt:517` `revoke()` targets the REAL crashed origin, while the menu names app.uniswap.org.
  - `VelaNavHost.kt:1291` — share / copy / system act on `pageUrl = engineState.url`, which is "" with no engine, so they silently do nothing.
- **Check, part A** (the host before the first commit):
  1. Chaos proxy with `mode=latency&latency=6000&match=pancake`.
  2. 探索 ▸ tabs ▸ + (start page). Type pancakeswap.finance and press Go.
  3. Take screencaps at about 0.5 s, 2 s and 4 s.
  - BAD (expected): "app.uniswap.org" with an orange open lock.
  - GOOD: pancakeswap.finance, or an empty bar.
  4. Set `mode=pass`.
- **Check, part B** (the crash path):
  1. Open the test dApp and Connect.
  2. `adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.crashRenderer true`. The panel 此页面已停止运行 appears.
  3. Tap ⋯. BAD: the sheet names app.uniswap.org and says 安全站点.
  4. Tap the account chip. BAD: "app.uniswap.org", "安全站点 · 已连接", or a network other than the dApp's.
  5. Tap the address pill. BAD: the editor holds "app.uniswap.org".
  - GOOD for steps 3–5: 127.0.0.1:8137, its real network, and no 安全站点.
  - Do not tap 断开连接 unless you want to see it revoke 127.0.0.1:8137 (then check Settings ▸ 连接).
- **Fix:** mirror dbf5a48c. In `ExploreLive.home`, never fall back to `fallback.browser.url/host/secure` on a live route.
  1. Before a commit, use `ExploreView.tabs[selected].url/host`, with no lock until a DbrTabView exists.
  2. When `engine == null`, build siteMenuSheet (filter 断开连接 by `connected`, statusLine "") and connection from the DbrTabView origin and the tab's saved URL.
  3. Feed share / copy / system from the tab's URL.

#### A-W3 · P2 · partially (medium) — No panel for a silent host; panel depends on callback order
- **Windows:** Edge's ERR_EMPTY_RESPONSE / ERR_TIMED_OUT pages replaced Vela's panel, and nothing recovered. The desktop now uses webview2_events.rs, the core's `LoadPlatform::WebView2` table and a no-commit watchdog (9e0fe081).
- **Why here:** the panel and the core classification exist; 079 SC-004 passed for certificate, offline and not-found:
  - `BrowserController.kt:177` — a main-frame onReceivedError goes through `browserLoadClassify("android", code, …)` to `failed()`;
  - `BrowserController.kt:260` — the core's retry schedule of 2/5/10 s for Offline/Timeout/Refused, while attached;
  - `ExploreScreen.kt:235` — BrowserNotice is drawn over the page;
  - `rust/crates/vela-core/src/app/browser_load.rs:103` — the Android table.

  The gaps:
  - There is no no-commit watchdog. It was a desktop decision (`specs/079-android-dapp-browser-stability/research.md:129`), and a grep finds none. A black-holed host shows the old or blank page with the 10% hairline until Chromium gives up.
  - `BrowserController.kt:151` — onPageStarted resets navFailed, and onPageFinished (line 158) clears the failure only when navFailed is false. The panel therefore depends on onPageStarted arriving BEFORE onReceivedError. The Xiaomi's WebView had the safe order; other WebView versions are unverified.
- **Check.** Record `adb shell dumpsys webviewupdate` first. Use the chaos proxy, and keep `adb shell screenrecord /sdcard/w3.mp4` running during each case.
  - (a) `mode=drop&match=uniswap`, then open https://app.uniswap.org.
    - GOOD: within about 1 s the panel 无法加载此页面 / 网络不稳定，页面没能打开。 / app.uniswap.org / 重试; 正在重试… at about 2, 5 and 10 s; logcat `browser.load main frame failed code=-1` (or -6).
    - BAD: any frame of Chrome's 网页无法打开 / net::ERR_… page.
    - Then set `mode=pass` before the third retry. GOOD: Uniswap loads with no tap.
  - (b) `mode=blackhole&match=uniswap`, then open app.uniswap.org and time it.
    - BAD (expected): a blank or previous page with only the hairline, and no browser.load line, until Chromium times out. Note the seconds and the code.
    - GOOD: a panel within about 12 s, as on the desktop.
  - (c) With (a)'s fault on, tap 重试.
    - GOOD: the panel stays and says 正在重试… for the whole attempt.
  - Set `mode=pass` afterwards.
- **Fix:** mirror 9e0fe081 (desktop) and the iOS fix 383af421.
  1. Add a no-commit watchdog to BrowserEngine: if onPageStarted has not arrived about 12–20 s after `requested()`, call `failed()` with the core's Timeout class. Take the panel down only on a clean onPageFinished.
  2. Stop relying on callback order: remember the failing URL in onReceivedError, and do not let onPageStarted for that same URL reset navFailed.

#### A-W4 · P2 · partially (medium) — Certificate failures: in-page links, the lock, and 重试
- **Windows:** Edge's interstitial offered 高级 → continue inside the wallet. The desktop now cancels, shows Vela's panel and a warning lock (9e0fe081).
- **Why here:** the security half is fine. `BrowserController.kt:188` onReceivedSslError always calls `handler.cancel()`, never proceed. The 079 evidence `specs/079-android-dapp-browser-stability/evidence/android-after/us3-l5-certificate.jpg` shows the panel for a load Vela started. The gaps:
  - `BrowserController.kt:189` — the panel is raised only `if (error.url == view.url || view.url.isNullOrBlank())`.
    - For a link tapped inside a page, view.url is still the old page, so no panel is raised.
    - The cancelled load ends as ERR_ABORTED, which WebView keeps out of onReceivedError, so the old page stays.
    - The bar may then name expired.badssl.com over badssl.com's content.
  - `dapp_browser.rs:675` — `DbrTabView.secure` is a scheme check that ignores a certificate failure. `ExploreLive.kt:196` takes the lock from `tab?.secure`. So the bar draws a grey closed lock beside a certificate panel (in a brand-new tab, an orange open lock).
  - `BrowserController.kt:278` — `retry()`/`reload()` call `webView.reload()`, which reloads the last COMMITTED entry. 重试 on a certificate failure that never committed may reload the previous page, or leave 正在重试… up for good.
- **Check:**
  - (a) Open a new tab, type https://expired.badssl.com and press Go.
    - GOOD: 网站证书有问题，Vela 已阻止打开。, with no automatic 正在重试.
    - Look at the bar. BAD: a grey closed lock beside expired.badssl.com. Note it if you see the orange open lock instead.
  - (b) Open https://badssl.com and TAP its "expired" link. Also try self-signed and wrong.host.
    - GOOD: the same certificate panel, naming expired.badssl.com.
    - BAD: nothing visible happens; the bar shows expired.badssl.com over badssl.com's content; a stuck hairline; a blank page.
    - Always BAD: any Chrome interstitial, or 继续前往.
    - `adb logcat -s VelaLog | grep browser.load` shows no line in the BAD case.
  - (c) On (a)'s panel tap 重试.
    - GOOD: the same certificate panel is back within 1–2 s.
    - BAD: 正在重试… never ends, or the previous site loads under the panel.
- **Fix:** mirror 9e0fe081. In BrowserEngine:
  1. Remember the pending main-frame URL: set it in `load()`, and in shouldOverrideUrlLoading when isForMainFrame, including redirects.
  2. In onReceivedSslError, fail when `error.url` matches that URL or its origin, instead of comparing with view.url.
  3. Retry a load that never committed by loading the failed URL again.
  4. In ExploreLive: `secure = tab.secure && failure?.class != Certificate`.

#### A-W13 · P2 · likely (high) — A dApp read waits 8 s per silent node (H6)
- **Windows:** with the first node black-holed, each read took 8.5 s. The desktop now hedges reads after 1.5 s (8872b915, e6b7469e).
- **Why here:**
  - `BrowserController.kt:452` — a page's chain read is `pool.call(chainId, method, …)`, the wallet's own pool.
  - `feature/wallet/core/RpcPool.kt:132` — `call()` awaits the core's single verdict. There is no second ask.
  - `feature/wallet/core/RpcPoolExecutor.kt:292` — each post uses `callTimeout(operation.timeout_ms)`. rpc.post logs host, outcome and ms (line 87).
  - `rust/crates/vela-core/src/app/rpc_pool.rs:161` — `RPC_READ_TIMEOUT_MS = 8_000`; line 167 `HEDGE_AFTER_MS = 1_500`.
  - `rpc_pool.rs:950` — `pending_urls` is `#[serde(skip)]`: "never in the bindings".
  - `dapp_browser.rs:86` — `READS_IN_FLIGHT = 8` per tab.
  - vela-core-uniffi exports none of is_hedged_read / early_verdict / HEDGE_AFTER_MS, and there is no "hedge" anywhere in app-android.
- **Check.** This shows only when the FASTEST node goes silent (§11.1).
  1. Chaos proxy in `mode=pass`; force-stop the app.
  2. In the test dApp tap Connect (then Add Base, if you want Base), and tap Block number 5 times.
  3. `adb logcat -s VelaLog | grep rpc.post` shows which host answers eth_blockNumber.
  4. `curl 'http://127.0.0.1:8899/__chaos?mode=blackhole&match=<that host, regex-escaped>'`
  5. Tap Block number several times, or run in DevTools: `console.time('b');await ethereum.request({method:'eth_blockNumber'});console.timeEnd('b')`
  - BAD (expected): about 8–9 s, with `rpc.post eth_blockNumber host=<held> outcome=Timeout ms≈8000` and `rpc.call slow`. Later reads may be fast, because the core cools the endpoint down for 30 s, doubling each time (`rpc_pool.rs:519/545`).
  - GOOD (after a port): under about 2 s.
  6. Set `mode=pass`.
- **Fix:** the owner must first lift "H6 desktop-first" (§11.2 item 3). Then mirror 8872b915 and e6b7469e:
  1. Export `is_hedged_read`, `early_verdict` and `HEDGE_AFTER_MS` through vela-core-uniffi.
  2. Expose the next URL for a call through an accessor. `pending_urls` is serde-skipped by design, so do not just regenerate types.
  3. In RpcPool.call / RpcPoolExecutor, post the same payload to that URL after 1.5 s.

#### A-H1 · P2 · likely (high) — Sign-in sheet copy
- **Windows:** the SIGN-IN sheet said "手机或平板 — 扫码，用附近设备创建", and "Touch ID 或 Windows Hello". The desktop now says 扫码 for the phone row, and names Windows Hello or Touch ID for this device (d9ab6e80, 5c79c6b3).
- **Why here:**
  - `feature/onboarding/flow/SignInMethodSheet.kt:126` — `signInMethodCopy(method) = methodCopy(method)`: the create copy, for every KeyMethod (line 76).
  - `feature/onboarding/flow/FlowCopy.kt:79` — Platform maps to METHOD_PLATFORM_TITLE/BODY, Hybrid to METHOD_HYBRID_TITLE/BODY.
  - `assets/i18n/zh.json:780` — methodPlatformBody "Touch ID 或 Windows Hello"; line 777 methodHybridBody "扫码，用附近设备创建". The Android build syncs assets/i18n (`build.gradle.kts:165`).
  - `feature/onboarding/flow/CableQrSheet.kt:58` — the caBLE QR sheet, shown for sign-in AND for every phone-key signature, always shows the hybrid create lines.
  - `feature/onboarding/flow/KeysScreen.kt:412` — the add-key picker uses methodCopy too.
  - `VelaNavHost.kt:626` — the account switcher's secondary action pushes WELCOME, so the sign-in sheet can be reached without clearing any data.
- **Check.** Any device will do, including the owner's; nothing is cleared.
  1. 钱包 ▸ tap the account name (switcher) ▸ the secondary button (Welcome) ▸ 登录. Screenshot the sheet.
     - BAD: "这台设备 · Touch ID 或 Windows Hello" and/or "手机或平板 · 扫码，用附近设备创建".
     - GOOD: the phone row says 扫码, and "this device" names an Android authenticator (e.g. 内置通行密钥).
  2. Tap 手机或平板. The QR sheet shows before Bluetooth is asked.
     - BAD: its body reads 扫码，用附近设备创建.
     - The sheet has no Cancel: wait out the 90 s, or force-stop.
  3. Switcher ▸ primary button (创建) ▸ the keys step's method list.
     - BAD: "Touch ID 或 Windows Hello".
  4. Repeat in English. Back out without choosing anything.
- **Fix:** mirror d9ab6e80 and 5c79c6b3. No new keys are needed.
  1. FlowCopy: give sign-in its own mapping:
     - Hybrid → `explore.scan` (扫码);
     - Platform on Android → `onboarding.create.providerPlatform` (内置通行密钥).
  2. CableQrSheet takes a `creating` flag, and uses `explore.scan` outside create.

#### A-W19 · P2 · partially (high) — Phone-held key: QR sheet with no Cancel, English timeout, -32603 **[owner + iPhone]**
- **Windows:** the signing column never drew the caBLE QR for a phone-held passkey; the person saw a 90 s spinner, then a raw -32603. The desktop now draws a QR card in the column, and closing it stops the scan. A scan that runs out counts as cancelled, so the request stays open (545621d7, 4198ec6b).
- **Why here:** the QR IS drawn. The path is `UserOpSpine.kt:95` → `signer().sign` → `PasskeyExecutor.assert` (`UserOpSigner.kt:39`) → `PasskeyExecutor.kt:406` runHybrid → showQr (line 735). `VelaNavHost.kt:2191` hosts CableQrSheet outside the NavHost, so it sits over the signing sheet. But:
  - `feature/onboarding/flow/CableQrSheet.kt:43` — there is no Cancel, and `onDismissRequest = {}` is set on a dismissible sheet: a swipe hides it while the scan runs on.
  - `feature/onboarding/core/HybridCeremony.kt:131` — a scan that runs its 90 s (SCAN_TIMEOUT_MS, line 202) becomes `PasskeyFailure(Other, "No phone answered the code…")`, in English. Only CtapException is caught (line 151).
  - `UserOpSpine.kt:149` — a PasskeyFailure that is not Cancelled becomes `Refused(Other)`, then `SignSubmitOutcome.Failed`, then `sign_request.rs:2499` `fail_inflight(CODE_INTERNAL)`: -32603, and the request is closed.
  - `feature/onboarding/core/CableTransports.kt:205` — a tunnel that will not open throws IOException, which escapes every catch and ends as neutralAnswer `Failed("Signing failed")` (`SignExecutor.kt:143`).
  - Low confidence: after a swipe, the hidden sheet's window may still hold touches, freezing the signing sheet until the scan ends.
- **Check** (owner's account on the Android device, with its key on the owner's iPhone; owner present):
  1. Test dApp ▸ Connect ▸ Sign ▸ slide. GOOD: a QR sheet over the signing sheet.
  2. Do NOT scan. Wait 95 s.
     - BAD (expected): `#out` shows code -32603 with "No phone answered the code…", and the request is gone.
     - GOOD (desktop semantics): the request stays with a scan-again option, and the page gets nothing until ✕ (one 4001).
  3. Sign again. Swipe the QR sheet down, then try the signing sheet's ✕.
     - BAD: there is no way to cancel the scan; ✕ does not respond while the hidden sheet is up; -32603 at 90 s.
  4. Sign again, scan with the iPhone and approve with Face ID.
     - GOOD: a signature, and Verify sign → `valid:true`.
  5. Throughout, run `adb logcat -s VelaLog | grep -E 'cable|passkey|sign.submit'`.
- **Fix:** mirror 545621d7, 4198ec6b, a22e1b30, 95b8e573, 1063909f.
  - HybridCeremony:
    1. On the 90 s timeout, throw `PasskeyFailure(Cancelled)` plus a scan-expired flag (desktop `around_prompt` / `scan_ran_out`), so that PasskeyCancelled keeps the request open.
    2. Catch the tunnel's IOException as a PasskeyFailure the sheet can name, with Retry.
  - CableQrSheet:
    1. Make it `dismissible=false`, with a Cancel that cancels the ceremony coroutine.
    2. Localise the sentence.

#### A-H5 · unknown — "Check your phone" after the scan; Cancel; handshake wait **[owner + iPhone]**
- **Windows:** after the phone scanned, the QR stayed up until the phone's prompt appeared; "查看你的手机" had no Cancel; the handshake waited up to 130 s. The desktop now swaps the QR for 查看你的手机 at the scan, with a Cancel, and waits 15 s per handshake frame (a22e1b30, 95b8e573, 1063909f).
- **Not assessed for Android.** Known so far: CableQrSheet (`CableQrSheet.kt:43`) has no Cancel (see A-W19). iOS has this as P2 (I-H5); give Android the same priority once seen.
- **What the device run must show,** with A-W19's setup, and also from sign-in ▸ 手机或平板:
  1. The time from the other phone's scan until the Vela screen leaves the QR. GOOD: about 1 s or less, replaced by a "check your phone" card.
  2. Whether the QR, and any touch card, has a Cancel that stops the scan.
  3. Scan, then put the other phone in Airplane Mode before approving. Measure how long Vela takes to give up, and record its message.
     - GOOD: seconds, and a retryable sentence.
     - BAD: minutes, or raw transport text.

  Logs: `adb logcat -s VelaLog | grep -E 'cable'`.
- **Fix, if present:** mirror a22e1b30, 95b8e573, 1063909f. See I-H5 for the shape.

#### A-EXE · P3 · likely (high) — Sheet headline is the English "Execute"
- **Windows:** the Universal Router's `execute` has no clear-signing descriptor, so the selector-database name "Execute" became the headline in a Chinese UI. Not fixed.
- **Why here:** clear signing is the shared core; Android runs the same selector lookup (ClearExecutor → the database / 4byte).
  - `SigningLive.kt:688` — the headline is `SigningBlock.Intent(result.intent)`.
  - `SigningLive.kt:153` — localizedTerms swaps only a named intent_term.
  - `SigningLive.kt:579` — summaryOf puts the same intent on the receipt line.
- **Check** (parallel space, zh UI, app.uniswap.org on Base): start any swap, open the sheet, do not slide. Then run `adb shell uiautomator dump /sdcard/ui.xml && adb shell cat /sdcard/ui.xml | grep -o 'text="Execute"'`.
  - BAD (expected): the headline is Execute, with the best-effort caution.
  - GOOD (future): 兑换 0.1 USDC → ETH.
- **Fix:** in the core, a clear-signing descriptor for the Universal Router's execute. It serves every client.

#### A-W2 · P3 · partially (high) — The address bar during a slow load
- **Windows:** after a typed navigation the bar showed the previous URL, and typing appended to it. The cause was specific to gpui (885867f4).
- **Why here:** the gpui cause does not exist on Android:
  - `BrowserChrome.kt:110` — editing starts with the whole URL selected;
  - `BrowserChrome.kt:122` — Go submits once;
  - `rust/crates/vela-core/src/app/dapp_rpc.rs:496` — `browser_input` turns "example.com" into https://example.com.

  What remains is 079's rule against 083's FR-005. This is cosmetic. (A NEW tab's bar before commit is A-W9.)
  - `BrowserController.kt:243` — "The address bar keeps the committed host until then".
  - `BrowserController.kt:290` — url and host change only through `update()`, from onPageStarted, onPageFinished and doUpdateVisitedHistory.
  - `specs/083-windows-dapp-browser-stability/research.md:51` — 083 replaced 079's rule with FR-005 ("the host being opened"), on the desktop only.

  So during a slow load the bar keeps the old host with its lock, and an open editor does not follow a commit.
- **Check:**
  1. Open https://pancakeswap.finance.
  2. Tap the pill. GOOD: the full URL is highlighted.
  3. Type example.com and press Go.
     - GOOD: https://example.com/ loads.
     - BAD: pancakeswap.finance/example.com, or text inserted into the old URL.
  4. Repeat 4 times.
  5. Timing: chaos proxy `mode=latency&latency=6000&match=example`. In a tab showing pancakeswap.finance, type example.com and press Go. Screenshot at 0.5 s, 2 s and 5 s.
     - Expected today: "pancakeswap.finance" with its closed lock until about 6 s. Record which you see.
     - GOOD (083 FR-005, the desktop now): example.com, with no lock, from the first frame.
  6. Set `mode=pass`.
- **Fix:** ask the owner whether to change 079's rule. If yes, mirror 885867f4 (`LoadWatch::named_url`):
  1. BrowserEngine keeps a `requestedUrl`, set in `load()` and in shouldOverrideUrlLoading for main-frame http(s).
  2. ExploreLive shows its host, with no lock, until onPageStarted or a failure.
  3. AddressBar edits the requested URL, and follows a commit while the draft is untouched.

#### A-F2 · P3 · handled (high) — Transaction hash in the detail
- **Why:**
  - `FlowLive.kt:253` — the hash fact is `take(10)…takeLast(6)`, and copyValue is the full hash.
  - `SigningLive.kt:523` — the aftercare receipt shortens too (`take(10)…takeLast(8)`) and copies the full hash.
  - While a row is pending, the "hash" is the record id (§10 A-7).
- **Check:** open a confirmed row's detail (a Gnosis dust send is enough).
  - GOOD: 哈希 reads like 0x7ca7653c…e4f892 on one line, and copy gives all 66 characters.
  - BAD: a clipped or wrapped hash.

#### A-W10 · P3 · handled (high) — A plain native-coin transfer
- **Windows:** a dApp's 0.001 xDAI eth_sendTransaction with no calldata was titled 合约交互, with ⚠ 无法解码 (fixed in 25ed87f6, d5661249).
- **Why:** `SigningLive.kt:644` — a resolved request with no result, no calldata and a `to` draws plainTransferBlocks: 发送, −amount, 接收方. The gaps the desktop closed in d5661249 remain: the stray `calls` key, a multi-leg batch and `input` (§10 A-1, A-2, A-3).
- **Check** (parallel space, test dApp, Send dust):
  - GOOD (expected): 发送, −0.001 XDAI, 接收方 0x7687…D141; no 合约交互 and no ⚠ 无法解码.
  - Reject with ✕, or slide (under $0.01).

#### A-DNS · P3 · handled (high) — Lookup failures
- **Why:**
  - `BrowserController.kt:181` — passes WebView's numeric errorCode, with no locale text.
  - `browser_load.rs:104` — `-2 | -12 | -10 => NotFound`.
  - `browser_load.rs:210` — NotFound and Certificate are never auto-retried.
  - `scripts/device/chaos-proxy.py:120` — the proxy answers 502 for a host it cannot reach, so run this check WITHOUT the proxy.
- **Check:**
  1. `adb shell settings put global http_proxy :0`, and turn off any VPN.
  2. Type no-such-host-vela-083.com and press Go.
     - GOOD: 找不到这个网站，请检查网址。 with the host, no 正在重试…, and logcat `browser.load main frame failed code=-2`.
     - BAD: 网络不稳定 with retries.
  3. Optional: repeat through the owner's VPN, and record which sentence shows.

#### A-W5 · P3 · handled (high) — Renderer crash
- **Why:**
  - `BrowserController.kt:208` — onRenderProcessGone logs and returns true.
  - `BrowserController.kt:673` — rendererGone marks the tab crashed, drops its snapshot, destroys the engine and dispatches `DbrEvent.RendererGone` (`dapp_browser.rs:566`).
  - `ExploreScreen.kt:227` — the crash panel 此页面已停止运行 with 重新加载 (`BrowserController.kt:789`).
  - 079 evidence: `a11-crash.jpg`.
  - The fixture sheets on this path belong to A-W9.
- **Check:**
  1. Open the test dApp and Connect.
  2. With the app open on that page, run `adb shell am start -n app.getvela.wallet/.MainActivity --ez vela.crashRenderer true`.
     - GOOD: the app stays up; the panel reads 此页面已停止运行 / 页面意外关闭。你的钱包不受影响——重新加载即可继续。 / 重新加载; logcat shows `browser.renderer renderer gone`.
     - BAD: the app closes, or a sad or blank page shows.
  3. Run A-W9 part B here.
  4. Tap 重新加载. GOOD: the test dApp is back, and Connect answers with no consent prompt.

#### A-H8 · P3 · handled (high) — The title after a failed load (one residue)
- **Why:**
  - `BrowserController.kt:644` — a failed load dispatches `TabNavigated(url = <the failed URL>, title = null)`.
  - `rust/crates/vela-core/src/app/explore_sites.rs:522` — the title falls back to the host.
  - `BrowserController.kt:350` — no snapshot is taken while failed.
  - The residue:
    - `BrowserController.kt:223` — onReceivedTitle is ignored while navFailed, and `failed()` (line 249) does not reset `EngineState.title`;
    - `BrowserController.kt:807` — toggleFavorite saves the failed URL with the OLD title.
- **Check:**
  1. Open app.uniswap.org and wait for its title, "Uniswap Interface".
  2. Type https://expired.badssl.com and press Go.
  3. Open the tab switcher.
     - GOOD: the card is titled expired.badssl.com, with a letter avatar and no Uniswap picture.
     - BAD: "Uniswap Interface".
  4. Tap ☆, then look at the start page's favourites.
     - Residual BAD: a tile named "Uniswap Interface" that points at expired.badssl.com. Remove it afterwards.
- **Fix:** in `BrowserEngine.failed()`, reset the title to "". Mirror 0ff7fbed.

#### A-W6 · P3 · handled (medium) — New windows open in the same tab, by design
- **Why:**
  - `BrowserController.kt:144` — `setSupportMultipleWindows(false)`, so _blank and window.open load in the same tab. `docs/dapp-browser/ARCHITECTURE.md:91` documents this; window.opener is gone.
  - `BrowserController.kt:196` — a main-frame http(s) navigation is allowed.
  - `VelaWalletApplication.kt:490` — onCancelSigning closes the sheet when the core says the page behind a request is gone.
- **Check:**
  1. Tap the "target=_blank" link.
     - Expected: example.com loads in the SAME tab, the tab count does not change, and system Back returns to the test dApp.
     - BAD: nothing happens, or another browser opens.
  2. In the chrome://inspect console run `window.open('https://example.com')`. This has no user gesture.
     - GOOD: it returns null and nothing navigates.
     - BAD: the page navigates.
  3. While connected, run this in the console, then leave the signing sheet alone:
     ```
     ethereum.request({method:'personal_sign',params:['0x6869',(await ethereum.request({method:'eth_accounts'}))[0]]}).catch(e=>console.log(e.code)); setTimeout(()=>location.href='https://example.com/',4000)
     ```
     - GOOD: when example.com loads, the signing sheet closes by itself.
     - BAD: the sheet stays up for a page that is gone.
  4. Ask the owner whether phones should open a new Vela tab, like the desktop (§11.3).
- **Fix:** only if the owner wants desktop parity. Mirror e6285fc9 and a6b1b2fb:
  1. `setSupportMultipleWindows(true)` plus `WebChromeClient.onCreateWindow`.
  2. Refuse when `!isUserGesture`.
  3. Otherwise take the URL from the transport WebView's first shouldOverrideUrlLoading, and dispatch `ExploreEvent.TabOpened`.

#### A-W7 · P3 · handled (high) — External schemes and downloads
- **Why:**
  - `BrowserController.kt:204` — a non-http(s) scheme leaves the app only `if (request.isForMainFrame && request.hasGesture())`; everything else is refused.
  - `BrowserController.kt:696` — openExternal handles each scheme:
    - `wc:` shows a toast;
    - javascript, file, content, data, blob and about do nothing;
    - `intent:` is sanitised: BROWSABLE, no component or selector, and the fallback URL loads in the tab;
    - anything else goes out as ACTION_VIEW + BROWSABLE. `docs/dapp-browser/ARCHITECTURE.md:83`: phones hand any app scheme to the system on a tap.
  - A cosmetic suspicion: a tapped download link sets loading=true through `requested()` (`BrowserController.kt:198`), and no DownloadListener exists, so the hairline may stay drawn (`BrowserChrome.kt:167`).
- **Check:**
  1. Tap "mailto link".
     - GOOD: the mail app or a chooser opens once.
     - BAD: nothing opens, or it opens twice.
  2. Console: `location='mailto:a@b.c'`.
     - GOOD: nothing opens.
     - BAD: the mail app opens.
  3. Console:
     ```
     document.body.insertAdjacentHTML('beforeend','<p><a id=wc href="wc:abc@2?relay-protocol=irn&symKey=00">wc</a> <a id=dl href="https://proof.ovh.net/files/1Mb.dat" download>dl</a></p>')
     ```
     Then TAP each link on the phone.
     - GOOD: wc shows the toast 暂不支持 WalletConnect 链接…; dl starts no download and saves no file.
     - BAD: another app opens, or a download starts.
     - Cosmetic BAD: the hairline stays drawn after dl.
  4. Optional: tap an `intent://…;S.browser_fallback_url=https%3A%2F%2Fexample.com;end` link. GOOD: the app opens if installed; otherwise example.com loads in the tab.
- **Fix:** none required. For parity: restrict openExternal's else-branch to mailto:/tel:, and add a DownloadListener that clears loading and shows a one-line notice (e6285fc9, a6b1b2fb).

#### A-D1 · P3 · handled (high) — An accidental dismiss
- **Why:** only ✕ answers.
  - `feature/signing/SigningSheet.kt:91` — the signing sheet is `dismissible = false`.
  - `core/designsystem/components/VelaModalSheet.kt:82` — that means no sheet gestures, no dismiss on back press, no dismiss on outside click, and no drag handle.
  - `VelaNavHost.kt:725` — the ✕ is the only close.
  - `ExploreScreen.kt:311` — the connect consent is `dismissible = false` too.
  - 079 results SC-001 on the Xiaomi: 9 attempts of swipe, scrim and Back, 0 closed.
  - Unverified: Android 16's predictive back (targetSdk 36).
- **Check** (reload the test page and paste the §7.1 logger first):
  1. Test dApp ▸ Sign.
  2. Try swipe down ×5, scrim tap ×5 and system Back ×5, with both 3-button and gesture navigation. Use an Android 16 device too if one is available.
     - GOOD: the sheet stays, and the logger shows no `←` / `✗` for personal_sign.
     - BAD: the sheet closes, a 4001 arrives, or Back navigates the page behind the sheet.
  3. Tap ✕. GOOD: exactly one `✗ personal_sign 4001` in the logger.
  4. Repeat for Connect's consent sheet (disconnect first).
- **Fix:** none. This is already the owner's D1 rule. Report the behaviour (§11.2).

#### A-W14 · P3 · handled (high) — Connect consent
- **Why:**
  - `ExploreLive.kt:272` — consent() titles the sheet "连接到 {host}", takes the account from identity, and the network from the consent's chain.
  - `feature/explore/components/ExploreSheets.kt:133` — ConnectionPanel always draws the account row and the network row.
  - `VelaNavHost.kt:1233` — the consent's identity is the session's active account plus `chainNames[c.chain_id]`.
  - `feature/browser/core/DbrWire.kt:36` — the shell ignores the core's `DbrConsentView.address`. This is only visible if the two ever differ.
- **Check:** with the test dApp not connected, tap Connect.
  - GOOD: "连接到 127.0.0.1:8137"; an account row (identicon, name, 0x88cC…6894); a network row (logo and name); a filled 连接 button.
  - BAD: only the site is named.
  - Approve. `#out` prints 0x88cC…6894.
- **Fix:** optional — draw `DbrConsentView.address`, named from `session.accounts` (a6b1b2fb item 7).

#### A-W15 · P3 · handled (high) — Back stays within its tab
- **Why:**
  - `BrowserController.kt:571` — one BrowserEngine, with its own WebView and history, per tab.
  - `BrowserController.kt:774` — `back()` asks only the current engine's `canGoBack()`.
  - `ExploreScreen.kt:163` — system Back walks the page's history, then leaves to the start page.
  - `ExploreLive.kt:197` — the arrows follow canBack/canForward.
- **Check:**
  1. In tab 1, open the test dApp and navigate twice.
  2. Tabs ▸ + ▸ example.com ▸ Go.
     - GOOD: ← and → are dimmed.
     - BAD: ← is enabled, or lands on tab 1's pages.
  3. Press system Back. GOOD: the start page, with tab 1 untouched.
  4. In tab 1, ← walks tab 1's own history.

#### Android — not applicable
- **A-U1d:**
  - The false "you'd still pay gas" clause is never drawn live. simWillFail's only use in app-android `src/main` is a gallery fixture (`feature/signing/SigningFixtures.kt:470`), and the live sheet draws no would-fail sentence (`SigningLive.kt:841`).
  - It becomes relevant with U1b: then use only the first clause (desktop `app-desktop/vela-wallet/src/signing/mod.rs:406` `first_clause`, 5825e443).
  - Quick check during A-U1: `adb shell uiautomator dump /sdcard/ui.xml && adb shell cat /sdcard/ui.xml | grep -o '但仍会扣除 gas'` finds nothing.
- **A-D3:**
  - There is no proxy/direct route switch: `core/net/VelaHttp.kt:33` uses one OkHttpClient on the OS proxy selector.
  - Side note: `core/net/NetHealth.kt:19` counts 3 consecutive socket failures across ALL hosts. This can flip the hero to offline briefly; it is display only. Timeouts do not count (`RpcPoolExecutor.kt:321`).
  - Optional check: chaos `mode=drop` on one RPC host, then pull to refresh on 钱包. BAD: the hero says offline while other hosts answer.
- **A-W12:**
  - There is no native child window: the WebView is a View inside Compose's AndroidView (`feature/explore/components/BrowserPage.kt:30`), and menus are ModalBottomSheet windows.
  - Quick look: tap ⋯, then the account chip. The sheet sits over a dimmed but visible page.
---

## 8. iOS

iOS is `app-ios/VelaWallet` plus `app-ios/VelaCoreKit`, in Swift and SwiftUI. Its in-app browser is a WKWebView driven by the shared dapp_browser core.

**Paths in this section** are relative to `app-ios/VelaWallet/VelaWallet/`, except those that start with `app-ios/`, `rust/`, `assets/` or `app-desktop/`. The tests live in `app-ios/VelaWallet/VelaWalletTests/`.

### 8.1 Setup

**Machine**
- A Mac with Xcode 26. The project's LastUpgradeCheck is 2630, and spec 081 built it with 26.3.
- cargo, plus the rustup targets added to the repo's pinned toolchain (1.97.1, `rust/rust-toolchain.toml`; both iOS scripts `cd rust`): `(cd rust && rustup target add aarch64-apple-ios aarch64-apple-ios-sim)`.
- An Apple Silicon Mac: the xcframeworks carry only `aarch64-apple-ios-sim` for the simulator.
- App facts: deployment target iOS 17.4, bundle id `app.getvela.VelaWallet`, automatic signing with team F9W689P9NE.

**Core.** Every 083 core fix (R2, D1b, W20, dApp feed rows, the new wire variants) reaches the phone only through this step. The xcframework is gitignored at `app-ios/.gitignore:29-30` (`VelaCoreKit/Artifacts/`); the dev-fixtures kit at `:33` (`VelaDevFixturesKit/Artifacts/`).
```
rust/scripts/build-ios-xcframework.sh      # ~5 min; also regenerates app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift
rust/scripts/build-ios-dev-fixtures.sh     # the parallel-space keyset (Debug); without it: "cannot find type 'RustBuffer'"
rust/scripts/check-ios-core-fresh.sh       # run before EVERY device run
```
- `check-ios-core-fresh.sh` exit codes:
  - 1: the build is stale or has no stamp. Rebuild.
  - 2: the xcframework matches HEAD, but the tree has uncommitted `rust/crates` changes. Testing HEAD is fine; say so in the results.
- After the rebuilds, expect diffs in `app-ios/VelaCoreKit/Sources/VelaCore/vela_core_uniffi.swift` (doc comments) and in the committed `app-ios/VelaWallet/VelaWallet/Dev/vela_dev_fixtures.swift` (rewritten by `build-ios-dev-fixtures.sh`). Commit neither during a test run.
- The committed Swift bindings date from 079 (3496d218). A 079 core faults ('sign_request fault') on a `reverted` or `not_confirmed` outcome. So a Swift port of S2/S3 ships with the rebuilt xcframework.

**Hermetic suite** (decisive for S2, S3, S3b, R3 and H4a):
```
cd app-ios/VelaWallet && xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet -destination 'platform=iOS Simulator,id=<sim UDID>' -only-testing:VelaWalletTests test
```
- `-only-testing:VelaWalletTests` keeps the scheme's UI-test target out: those tests launch the app and expect test servers. Add `/<SuiteName>` to run one suite.
- Address the simulator by UDID (`xcrun simctl list devices available` lists them). Spec 052 found that `name`/`OS:latest` did not match, and `name:iPhone 16` does not resolve on Xcode 26.
- Harness pieces that already exist:
  - `ScriptedRelayPort` (`app-ios/VelaWallet/VelaWalletTests/MoneyPlumbingTests.swift:21`) answers by method, bundler calls included, and records every method in `calls`.
  - `ScriptedAccounts` (`MoneyPlumbingTests.swift:50`), `CountingSigner` (`:75`) and `ChallengeCapturingSigner` (`DappSigningTests.swift:257`).
  - SignExecutor's init takes `receiptWaitMs`/`receiptPollMs`, so a test can shrink the 120 s window.
  - `SigningReceiptTests.swift` builds contexts for `SigningLive.receipt` and `aftercareReceipt`.
- **None of the existing signers produces an assertion.** CountingSigner and ChallengeCapturingSigner throw a cancel, so the spine stops before eth_sendUserOperation. For S2, S3 and S3b, sign with `FixtureUserOpSigner(preferred: nil)` (`Dev/ParallelSpaceBinding.swift:151`; Debug only, reachable through `@testable import VelaWallet`; needs `build-ios-dev-fixtures.sh`, above).
  - `golden` = `fixtureMultiAddress()` = `0x88cCA0EeDbF2C4426110bbFc998F048689266894` (MultiTest).
  - `ScriptedAccounts.keyList` = every `fixtureAccounts()` record as `WalletKeyRecord(credentialId: $0.credentialIdHex, publicKeyHex: $0.publicKeyHex)`, the first one pinned: the Safe is founded on the whole keyset (`ParallelSpaceBinding.swift` `enter`).
  - `limits` = `["verificationGasLimit": "0x30d40", "callGasLimit": "0x30d40", "preVerificationGas": "0xc350"]` (as in `DappSigningTests.challenge`).
  - Before reading any verdict, assert `port.calls.contains("eth_sendUserOperation")`: a run that never sent measured nothing.

**Device, build and install**
- Developer Mode on, the phone unlocked, and this Mac trusted.
- Signing: Xcode must be signed in to an Apple ID in team F9W689P9NE (`project.pbxproj` DEVELOPMENT_TEAM). Ask the owner for it; do not change the team. Pass `-allowProvisioningUpdates` to every device xcodebuild (below): without it, a Mac that has never built this app has no development profile and fails with "No profiles for 'app.getvela.VelaWallet' were found".
- Installing over the existing app keeps its data. If `devicectl device install` refuses because the phone holds an App Store or TestFlight Vela (or another signature), stop and ask. Never delete the app (§3).
- Two phones appear in earlier specs: `00008130-001C68C804E1401C` (052) and an iPhone 11, `00008030-001A75961445802E` (052/053). **Ask the owner which one to use.**
- xcodebuild wants the HARDWARE UDID. devicectl may want its own identifier from `xcrun devicectl list devices`: spec 081 found devicectl refused the hardware UDID. If one form fails, try the other.
```
cd app-ios/VelaWallet && xcodebuild -project VelaWallet.xcodeproj -scheme VelaWallet -configuration Debug -destination 'id=<hw-udid>' -derivedDataPath build -allowProvisioningUpdates build
xcrun devicectl device install app --device <devicectl-id> build/Build/Products/Debug-iphoneos/VelaWallet.app
```

**Launch with logs**
- The parallel space (Debug only) is `Dev/ParallelSpaceBinding.swift`. `VELA_PARALLEL_SIGNER=n` picks the fixture key. There is no Face ID in it.
```
xcrun devicectl device process launch --console --terminate-existing --device <devicectl-id> -e '{"VELA_PARALLEL_SPACE":"1","VELA_PARALLEL_SIGNER":"0","VELA_LANG":"zh","VELA_SKIP_LAUNCH_ANIMATION":"1","VELA_URL":"http://<mac-lan-ip>:8000/"}' app.getvela.VelaWallet
```
- `VELA_URL` (Debug) opens 探索 on that page; it is the way to open the live browser. `VELA_PAGE` has no `explore-live` value: `explore` is the only explore override, and it opens the fixture screen (`App/RootView.swift:821`), not the live browser.
- `-e '{"VELA_PARALLEL_SPACE":"0"}'` leaves the space. The space never touches the owner's real account.
- Simulator:
  ```
  SIMCTL_CHILD_VELA_URL=http://127.0.0.1:8000/ SIMCTL_CHILD_VELA_PARALLEL_SPACE=1 xcrun simctl launch --console-pty <sim-id> app.getvela.VelaWallet
  ```
- Shell log lines start with `[vela-wallet] …` or `[vela-cable] …`, e.g. `[vela-wallet] browser load failed: <url> — NSURLErrorDomain -1003 → notFound`. Faults show as `[vela-wallet] sign_request fault / tx_tracker fault / fee_policy fault`.
  - They appear only with `--console` or under the Xcode debugger.
  - Run one devicectl session at a time. A stray `--console` holds the device, and the next run reports "Authentication canceled".
- Items that need the owner's own account and Face ID: D1b, H4b, W11 (Face ID part), W19, W20, H5.
- Balances: check before each money step (§3). A slid Max USDC→ETH (I-U1) runs last.

**Page devtools**
- On the iPhone: 设置 ▸ Safari ▸ 高级 ▸ 网页检查器. On the Mac: enable Safari's developer features, then Safari ▸ 开发 ▸ <device> ▸ the page. Debug builds set `isInspectable` (`Features/Explore/Core/BrowserEngine.swift:159-165`).
- Use a USB cable: Airplane Mode (I-S3) drops a Wi-Fi inspector session.
- WebKit passes a user gesture on to timers of 1 s or less. For every "no gesture" check use `setTimeout(…, 1500)` or longer, and keep the console's "emulate user gesture" off.
- A console logger to paste:
  ```
  const r=ethereum.request.bind(ethereum);ethereum.request=async a=>{console.log('→',a.method,JSON.stringify(a.params));try{const v=await r(a);console.log('←',a.method,v);return v}catch(e){console.log('✗',a.method,e.code,e.message);throw e}}
  ```
  Uniswap may use its EIP-6963 provider object, which this wrapper does not see.
- `eth_sign` is refused with 4200 (`rust/crates/vela-core/src/app/dapp_rpc.rs:754`). Use Sign (personal_sign) for message tests.

**Test dApp**
- The Android test page (§7.1 "What the page offers" lists its buttons, `__ask`, `me()`, `GNOSIS_USDC` and why `#out` needs a reload) is served from the Mac on two ports. The page frames an attacker iframe from port+1. From the repo root:
  ```
  D=app-android/vela-wallet/dev/testdapp; python3 -m http.server 8000 --bind 0.0.0.0 --directory "$D" & python3 -m http.server 8001 --bind 0.0.0.0 --directory "$D" &
  ```
  Verify with `curl -sI http://127.0.0.1:8001/attacker.html` → 200. Open `http://<mac-lan-ip>:8000/`. The Mac firewall must allow Python.
- In the console, `me()` is the connected address. The iOS checks below write `acct`: define it once per page with `const acct=me();` (on app.uniswap.org, which has no `me()`: `const acct=(await ethereum.request({method:'eth_accounts'}))[0];`).
- Info.plist has no `NSLocalNetworkUsageDescription`, so the first LAN load may raise iOS's local-network prompt. Allow it. If it was denied, the page fails as 网络不稳定; fix it in 设置 ▸ 隐私与安全性 ▸ 本地网络 ▸ Vela.
- A private-LAN http origin may sign (`rust/crates/vela-core/src/app/dapp_permissions.rs:687-753`), and the pill shows a CLOSED lock for it. Both are by design.
- The UI-test copy (`app-ios/VelaWallet/VelaWalletUITests/Resources/testdapp.html`) is served on the phone at `127.0.0.1:8137` only while a UI test runs.
- The real dApp: https://app.uniswap.org on Base.

**Chain and stored records**
- A receipt on Base (use a Gnosis RPC for Gnosis):
  ```
  curl -s https://mainnet.base.org -H 'content-type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"eth_getTransactionReceipt","params":["<hash>"]}'
  ```
  - `null` means no node knows the hash.
  - The UserOperationEvent is the log from EntryPoint `0x0000000071727De22E5E9d8BAf0edAc6f37da032` with topic0 `0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f`.
  - topic1 is the op hash. The data words are nonce, success, actualGasCost, actualGasUsed.
- History is ONE JSON string in UserDefaults, key `vela.transactionHistory` (`Core/VelaStore.swift:94, 225-231`). Background or quit the app first, then:
  ```
  xcrun devicectl device copy from --device <id> --domain-type appDataContainer --domain-identifier app.getvela.VelaWallet --source Library/Preferences/app.getvela.VelaWallet.plist --destination ./prefs.plist && /usr/libexec/PlistBuddy -c 'Print :vela.transactionHistory' prefs.plist | python3 -m json.tool
  ```
  Use PlistBuddy, not `plutil -extract`, which reads the dot in the key as a path separator.

**Automated evidence**
```
xcodebuild … -destination 'platform=iOS,id=<hw-udid>' -allowProvisioningUpdates -only-testing:VelaWalletUITests/DappBrowserStabilityProbeTests -resultBundlePath /tmp/probe.xcresult test && xcrun xcresulttool export attachments --path /tmp/probe.xcresult --output-path /tmp/probe
```
- It covers a fresh-tab refused load only (`testProbeALoadThatFails`). Nothing automated covers a failed SECOND navigation (I-H8).
- If automation times out, the phone may need a person to unlock it.

**Fault proxy**
- Start it:
  ```
  CHAOS_BIND=0.0.0.0 python3 scripts/device/chaos-proxy.py chaos.log
  ```
  Add `CHAOS_UPSTREAM=<host:port>` if the Mac itself needs a proxy; otherwise even pass mode fails with FAIL/502 in chaos.log.
- On the iPhone: 设置 ▸ 无线局域网 ▸ (network) ▸ 配置代理 ▸ 手动 ▸ `<mac-lan-ip>:8899`.
- Control it with `curl 'http://127.0.0.1:8899/__chaos?mode=drop|blackhole|pass&match=<host-regex>'`.
- chaos.log lists every host the page and the wallet's RPC pool reach.
- Turn the Wi-Fi proxy Off afterwards.

**Earlier recipes**: specs/052-ios-money-wiring/quickstart.md, specs/053-ios-dapp-browser-signing/quickstart.md, specs/050-ios-live-shell/quickstart.md, specs/070-dapp-browser-core/quickstart.md, specs/081-audit-product-gaps/results.md (iOS traps).

### 8.2 Checklist (priority order)

#### I-S3b · P0 · likely (high) — Another operation's hash answered
- **Windows:** see A-S3b. A relay "already pending [existingHash:…]" made the desktop answer with the OTHER op's hash.
- **Why here:**
  - `Core/UserOpSpine.swift:446` — `if let existing = parseExistingUserOpHash(message: errorJson) ?? parseExistingUserOpHash(message: message) { return existing }`. Any marker is returned as THIS op's hash.
  - `UserOpSpine.swift:437` — the raw JSON is searched first, so even the AA25 wording yields the marker.
  - `Core/RelayClient.swift:654` — `nonce()` reads EntryPoint.getNonce at 'latest'. There is no floor from ops that were submitted but have not landed.
  - `Features/Signing/Core/SignExecutor.swift:238` — the returned hash is reported as op_submitted, recorded, and answered with its receipt's tx hash.
  - `Features/Send/SendExecutor.swift:464` — the wallet's own sends use the same `spine.submit`.
  - `app-ios/VelaWallet/VelaWalletTests/MoneyPlumbingTests.swift:381` — `anAlreadyPendingOperationSurvivesTheTranslation` pins raw-JSON marker reading.
  - `rust/crates/vela-core-uniffi/src/lib.rs:1497` — only `parse_existing_user_op_hash` is exported. `existing_op` and `user_op_hash` (`user_op.rs:703, 719`) are not.
- **Check — DECISIVE (hermetic, VelaWalletTests):**
  - Script `ScriptedRelayPort`:
    - `rpc['eth_getCode'] = .ok('0x')`
    - `rpc['eth_estimateUserOperationGas'] = ok(limits)`, as in `DappSigningTests.challenge`
    - `rpc['eth_sendUserOperation'] = .rpcError(code: -32521, message: 'AA25 invalid account nonce [existingHash:0x1111…1111]')`
  - Call `UserOpSpine.submit(chainId: 100, account: golden, calls: [a plain call], gasFeeToken: nil, quotedFee: Quoted(amount: '1000', recipient: golden))` with the FixtureUserOpSigner set-up from §8.1 (`golden`, `keyList`, `limits`; check `port.calls` first).
  - BAD (today): it returns `'0x1111…1111'`.
  - GOOD: it throws a clear "another transaction from this account was pending" failure, or waits and resubmits. It never returns another op's hash. In the ThisOne case (the marker equals this op's own ERC-4337 hash) it may return it.
- **Device:** not practical with this account. USDC is already Permit2-approved, so Uniswap asks only for a PermitSingle signature and then `execute`; there is no second transaction to collide with. If you try anyway, on Gnosis:
  0. Put the page on Gnosis (§3 Money): Connect, Switch to Gnosis, Chain → `0x64`. The sheet must name Gnosis and xDAI.
  1. Slide a test-dApp Send dust and immediately tap 关闭 · 后台继续.
  2. Within a couple of seconds, send 0.001 xDAI to 0x7687…D141 (`0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141`) from 首页 发送 on Gnosis.
  - BAD: the second send's receipt or record carries the FIRST op's userOpHash/txHash (compare in prefs), and only one transfer happens on chain.
- **Fix:** mirror ddb52da8 (desktop `executor/user_op.rs`, `relay.rs`). In `UserOpSpine.submit`'s rejected branch (`UserOpSpine.swift:431-455`):
  1. Compute `own = user_op_hash(signed op, chain)` and call `existing_op(raw json, own)`. Both need uniffi exports.
  2. ThisOne → return own.
  3. Another → wait for it to land (2 min), resubmit this signed op once, otherwise fail clearly.
  4. Keep a per-account registry of ops that were submitted but have not landed, and floor the nonce at their nonce+1 (`RelayClient.nonce`, line 654).

  This covers SignExecutor and SendExecutor.

#### I-S2 · P0 · likely (high) — A reverted dApp operation answered as success
- **Windows:** see A-S2.
- **Why here:**
  - `Features/Signing/Core/SignExecutor.swift:327` — awaitReceipt's `if case .resolved(_, let txHash, _, _)? = answer` discards `confirmed` (the UserOperationEvent success flag) and the logs.
  - `SignExecutor.swift:376` — afterReceiptWait turns any receipt into `{type: succeeded, result: txHash}`. The shell never produces `reverted`.
  - `Core/RelayClient.swift:551` — `userOpReceipt` computes `confirmed = success != false`. TrackerExecutor uses it; SignExecutor ignores it.
  - `rust/crates/vela-core/src/app/sign_request.rs:2346` — Succeeded answers the page Ok(txHash), then records Confirmed.
  - `Features/Signing/SigningAftercare.swift:57` — an ok result that is not the op hash is shown as `.landed`: 已确认 with the hash, and the tick goes after 2.6 s.
  - `app-ios/VelaWallet/VelaWalletTests/DappSigningTests.swift:92` — the existing test pins any in-time receipt as succeeded.
  - This is the live path: `Features/Signing/Core/SigningController.swift:210` builds the SignExecutor with the app's RelayClient.
  - The tracker's success:false patch races update_record (§10 I-6), so a device run may leave the row either confirmed or failed.
- **Check — DECISIVE (hermetic):**
  - Build `SignExecutor(spine:relay:store:receiptWaitMs: 2_000, receiptPollMs: 100)` over a ScriptedRelayPort:
    - `eth_getCode` → `'0x'`
    - `eth_estimateUserOperationGas` → ok
    - `eth_sendUserOperation` → `ok('0xop')`
    - `eth_getUserOperationReceipt` → `ok({success:false, receipt:{transactionHash:'0xabc', logs:[]}})`
  - Run `perform(['type':'sign_and_submit','id':'r1','method':'eth_sendTransaction','params_json':'[{"to":"0x…","value":"0x0","data":"0x1234"}]','chain_id':100,'address':golden,'quoted_fee':['amount':'1000','recipient':golden]])` with the FixtureUserOpSigner set-up from §8.1. The call waits the 5 s waitForRecord unless you persist first.
  - BAD (today): `{type:'succeeded', result:'0xabc'}`.
  - GOOD: `{type:'reverted', user_op_hash:'0xop', tx_hash:'0xabc'}`.
- **Optional device repro** (parallel space, Base). It is flaky: the relay may refuse or drop the op instead of letting it revert, and each included revert costs a few cents.
  1. Put the page on Base: press Add Base on the test page (or open app.uniswap.org on Base and connect). Check that `eth_chainId` returns `0x2105` (if not, `__ask('wallet_switchEthereumChain',[{chainId:'0x2105'}])`). Then run:
     ```
     const acct=me(), ROUTER='0xd6145b2d3f379919e8cdeda7b97e37c4b2ca9c40';
     ```
     (If `acct` is already defined on this page, declare only `ROUTER`. On app.uniswap.org use the eth_accounts form of `acct` from §8.1 "Test dApp".) The router is results.md F3's 0xd614…9c40 (also `rust/crates/vela-core/tests/app_fee_policy.rs:3849`); confirm it with eth_getCode on Base.
  2. In the Web Inspector console:
     ```
     const w=n=>n.toString(16).padStart(64,'0'); const dl=Math.floor(Date.now()/1000)+40; const data='0x3593564c'+w(0x60)+w(0x80)+w(dl)+w(0)+w(0);
     ethereum.request({method:'eth_sendTransaction',params:[{from:acct,to:ROUTER,value:'0x0',data}]})
     ```
  3. Slide about 3–5 s before `dl`. If you slide after `dl`, the estimate fails with no charge; retry.
  - BAD: `← eth_sendTransaction 0x<tx>` and 已确认, while on chain that tx's UserOperationEvent has success = 0.
  - GOOD: one `✗ -32603 'The transaction was included but reverted (0x…)'`, and the record fails.
- **Fix:** in `SignExecutor.awaitReceipt` / `afterReceiptWait` (`SignExecutor.swift:313-378`), keep `confirmed` and the logs, and return `{type:'reverted', user_op_hash, tx_hash}` when the op did not execute.
  - Minimal version: `success == false`. It is a JSON value the 083 core already deserializes, so no export is needed.
  - Parity version: export `op_execution_failed` (`tx_tracker.rs:1057`) and apply it to the logs, as desktop `executor/landing.rs` does (ddb52da8, 2448738a).
  - Apply the same rule in `resolveUserOp` (`App/RootView.swift:411-415`).
  - Update `DappSigningTests:86-95`.
  - Ship with the rebuilt xcframework (a 079 core faults on `reverted`) and together with I-R3.

#### I-U1 · P0 · likely (high) — Max USDC → ETH: the fee coin is drained by the swap
- **Windows:** see A-U1.
- **Why here:**
  - `Features/Signing/Core/SigningController.swift:409` — `fees.ask(... feeToken: nil, autoFeeToken: true)`: the core auto-picks the fee coin.
  - `SigningController.swift:371` — `simulate()` hands the deltas only to token_trust (`ports.simDeltas`). A grep of app-ios finds `balance_changes_measured` / `BalanceChangesMeasured` nowhere.
  - `rust/crates/vela-core/src/app/fee_policy.rs:2264` — in the auto-pick, `measured_for(ctx, model.measured)` is None without the event. Coins are then weighed on the calls alone, and USDC is picked (it is pulled via Permit2, which the calls do not show).
  - `fee_policy.rs:2868` — for a large op (over 1024 B), SimulationFailed becomes EstimateFailed. Only Refused tries the next coin.
  - `Features/Send/FeeExecutor.swift:201` — the estimate simulates the calls the core supplies, which already include the USDC fee leg (`fee_policy.rs:2285-2286`).
  - U1 and U1b fix the same symptom in two ways: Refused lets the core try ETH on its own; measured changes make ETH the first pick. The desktop does both.
- **Precondition:** the same as A-U1's. Hold USDC well above the USDC fee the sheet quotes (aim for at least 0.2; top up with ETH→USDC 0.0001 first, §3), or ETH is auto-picked on the static fee alone (`fee_policy.rs:2258-2270`) and the run looks GOOD for the wrong reason. Control: a 0.05 USDC→ETH sheet on the same balance (do not slide) must auto-pick USDC.
- **Check** (parallel space, Base, Uniswap; run the sheet part while USDC is high, e.g. right after the top-up; slide a Max only as the LAST money step, after the 0.05 USDC steps; success costs about $0.08, and the BAD case costs nothing):
  0. Launch with `xcrun devicectl device process launch --console …` (§8.1 "Launch with logs"), and paste the §8.1 logger in Web Inspector on app.uniswap.org. If Uniswap uses its EIP-6963 provider and no `✗` line appears, record the sheet's text and the on-chain result instead.
  1. Swap USDC → ETH and tap **Max**.
  2. Watch the fee row.
  - BAD (most likely): the fee value reads 点击重试 with the warning 无法连接 Vela 服务 — 请检查网络，稍后会自动重试。; the slide stays shut; the fee re-quotes at 3/6/12/15 s forever; no fault in the console.
  - Alternative BAD: a USDC fee shows and the slide works, but after the slide the sheet reads 失败 · 交易未能提交…, and the console shows `✗ -32603 'Could not estimate gas for this transaction…'`.
  - GOOD: the fee is auto-picked in ETH, the slide is live, and the swap lands (on-chain UserOperationEvent success = 1).
- **Fix:** mirror 380d9014 and 5825e443.
  1. In `SigningController.simulate` (lines 348-377), as soon as eth_simulateV1 answers and before the trust judging (5825e443 item 6), send the fee_policy event `{type:'balance_changes_measured', changes:[{token: null|contract, delta: signed base units}]}` (FeeBalanceChange, `fee_policy.rs:424`).
  2. Send it to EVERY FeeStore tier session. That needs a new FeeStore method.

#### I-S3 · P1 · likely (high) — The userOpHash answered after a timeout
- **Windows:** see A-S3.
- **Why here:**
  - `SignExecutor.swift:91` — `receiptWaitMs: Double = 120_000`.
  - `SignExecutor.swift:377` — with no receipt in the window it returns `[type: receipt_pending, user_op_hash: hash]`.
  - `sign_request.rs:2413` — ReceiptPending answers the page Ok(user_op_hash). NotConfirmed (line 2458) exists, but iOS never sends it.
  - `SigningController.swift:670` — opHashAnswer tells the browser the answer was an op hash, for receipt translation.
  - `dapp_browser.rs:1484` — only receipts polled THROUGH the wallet are translated; a site reading its own RPC never finds the op hash.
  - `DappSigningTests.swift:86` — `aLateReceiptIsReportedPendingNeverSucceeded` pins receipt_pending.
- **Check — DECISIVE (hermetic):** `DappSigningTests:86-95` already asserts the BAD contract. End to end: build `SignExecutor(..., receiptWaitMs: 300, receiptPollMs: 50)` with the FixtureUserOpSigner set-up from §8.1, and answer `eth_getUserOperationReceipt` with `ok(NSNull())`.
  - BAD: `{type:'receipt_pending', user_op_hash:'0xop'}`.
  - GOOD (after the port): no answer inside the window, and `{type:'not_confirmed', user_op_hash}` at the cap.
- **Optional device repro** (parallel space, Gnosis test dApp Send dust; Web Inspector over USB, logger on):
  1. Slide.
  2. The moment the title reads 交易已提交至网络, turn Airplane Mode on from Control Center. On Base the first poll may already find the op, so use Gnosis.
  3. Wait 125 s, then turn Airplane Mode off.
  - BAD:
    - at about 120 s the console shows `← eth_sendTransaction 0x<h>`;
    - `<h>` equals the record's userOpHash (prefs `vela.transactionHistory`), and `eth_getTransactionReceipt(<h>)` on a public node returns null;
    - the ending reads 还没上链。Vela 会继续查看…, then 已确认 once the tracker sees the op.

    If the console shows a tx hash instead, the receipt came before Airplane Mode; repeat faster.
  - GOOD: no page answer at 120 s. The real tx hash arrives once the op lands, or after 10 min a -32603 '…submitted but is not confirmed yet… (user operation 0x…)'. The page never gets the op hash.
- **Fix:** mirror ddb52da8. In `SignExecutor.swift:91, 313-336, 375-378`:
  1. Wait up to the core's `PAGE_WAIT_CAP_MS` (10 min; export it rather than hard-coding it).
  2. Poll eth_getUserOperationReceipt at the tracker's cadence (3 s, then 12 s), and eth_getUserOperationByHash every 12 s.
  3. At the cap, return `{type:'not_confirmed', user_op_hash}`.

  The core's `answer_still_landing` (`sign_request.rs:2559`) sets no sign_error, so SigningAftercare needs no change. Update `DappSigningTests:86`.

#### I-R3 · P1 · likely (high) — A revert reads "couldn't be submitted — funds safe"
- **Windows:** see A-R3.
- **Why here:**
  - `Features/Signing/SigningLive.swift:514` — aftercareReceipt: a tracker status of 'dropped' (the core's word for a reverted op) or 'rejected' gives `statusFailed` + `send.txErrorGeneric` 交易未能提交。您的资金安全无虞——请重试。, with no hash and no explorer link.
  - `SigningLive.swift:412` — receipt(): any post-approval error gives the same sentence; the core's detail is ignored.
  - `app-ios/VelaWallet/VelaWalletTests/SigningReceiptTests.swift:191` — the existing test pins 'dropped' → `.failed`.
  - `Features/Send/TrackerWire.swift:24` — `dropped` means reverted.
  - `sign_request.rs:136` — `reverted_transaction` has no uniffi export.
  - Rare today, because a revert reads as success (S2). Once S2 is fixed, every revert shows this sentence, so ship it with S2.
- **Check — DECISIVE (extend SigningReceiptTests):**
  - (a) `aftercareReceipt(.stillConfirming, …, context(track: entry('dropped','final')))`.
    - BAD (today): the captions contain `loc.t('send.txErrorGeneric')`, and hash == nil.
    - GOOD: `componentsTx.receipt.failedHint` (转账在链上被回滚 —— …网络费可能仍被扣除…), the tx hash with copy, and viewOnExplorer set.
  - (b) `receipt(sign: sign(error: .submitFailed))` with the detail `'The transaction was included but reverted (0xabc)'`.
    - BAD: the generic sentence.
    - GOOD: the same failed-on-chain ending, with 0xabc.
  - On the device, only after S2 is ported: run I-S2's deadline revert. GOOD: 失败 + failedHint + hash + 在区块浏览器中查看. Never 交易未能提交。您的资金安全无虞.
- **Fix:** mirror 2448738a.
  1. In `SigningLive.receipt` (412-419) and `aftercareReceipt` (514-520), draw `componentsTx.receipt.statusFailed` + `failedHint` + hash + explorer in two cases:
     - the detail is a reverted one (core `reverted_transaction`, exported);
     - the tracker entry is 'dropped' and has a txHash.
  2. Keep `txErrorGeneric` for true pre-chain failures and for 'rejected'.
  3. Update `SigningReceiptTests:113-118` and `:191-194`.

#### I-U1b · P1 · likely (high) — A relay refusal drawn as "check your network"
- **Windows:** see A-U1b.
- **Why here:**
  - `Features/Send/FeeExecutor.swift:214` — `case .refused:` always becomes `{type:'simulation_failed'}`, never 'refused'.
  - `fee_policy.rs:2876` — on a large op this becomes EstimateFailed.
  - `fee_policy.rs:762` — EstimateFailed is re-quoted at 3/6/12/15 s forever; WouldFail is never re-quoted.
  - `SigningLive.swift:1050` — any recoverable failure shows `componentsUi.funding.denialNetworkError`.
  - `SigningLive.swift:1015` — the fee value shows `componentsUi.gas.estimateFailed` 点击重试.
  - The only 083 change on iOS so far: `recoverable()` excludes would_fail (5825e443 item 8).
- **Check:** the I-U1 run.
  - BAD: 无法连接 Vela 服务 — 请检查网络，稍后会自动重试。 under 点击重试, with the spinner cycling, while other pages load fine.
  - GOOD (with U1b ported alone): the core tries ETH by itself and the fee settles in ETH. If every coin is refused, the row reads 这笔交易预计会失败。 or USDC 余额不足以支付 Gas 费, with no network wording and no auto-retry.
- **Fix:** mirror 380d9014 and 5825e443.
  1. `RelayClient.estimateUserOpGas`: keep the relay's words in `.refused`.
  2. `FeeExecutor.simulate` (lines 214-218): return `{type:'refused'}` only when the §2.4 rule holds. Everything else stays simulation_failed.
  3. `SigningLive.feeModel`: draw would_fail as the FIRST clause of `componentsUi.signing.simWillFail`, and a coin with nothing left as `send.warnInsufficientGas`.

#### I-U1c · P1 · likely (high) — Fee coin list unusable over a failed quote
- **Windows:** see A-U1c.
- **Why here:**
  - `SigningController.swift:110` — feeTapped does `if fee.failed != nil { fees.refresh() } else if fee.options.count > 1 { feeOpen.toggle() }`. The list never opens over a failed quote.
  - `SigningLive.swift:1024` — the selector is built only when `feeOpen && options.count > 1`.
  - `SigningLive.swift:277` — `feeChevron = options.count > 1`, so a '>' is drawn whose tap only re-quotes.
  - `fee_policy.rs:3634` — option_views lists the rows even in the Failed phase.
  - `fee_policy.rs:3465` — `select_fee_asset` ignores a pick unless the phase is Quoted or Failed(WouldFail). So land U1c with U1b.
- **Check:** continue the I-U1 run. With the fee row on 点击重试, tap the row and its '>' several times.
  - BAD: only the refresh spinner, then 点击重试 again. No coin list ever appears.
  - GOOD: the coin list opens over the failed quote, and picking ETH re-prices and opens the slide.
- **Fix:** mirror 380d9014.
  1. `feeTapped` (lines 110-118): open the list whenever `options.count > 1`; refresh only when there is no other coin.
  2. Make sure `pickFee` → `fees.chooseFeeToken` works from a failed quote.

#### I-H2 · P1 · likely (high) — A dApp transaction is not in 活动
- **Windows:** see A-H2.
- **Why here:**
  - `Core/TxRecords.swift:158` — toWire maps no dapp_url, intent, balance_changes or calldata into FeedTxRecord.
  - `Features/Wallet/ActivityWire.swift:100` — FeedItemWire has no `dapp` field. Decodable ignores it, so nothing fails to decode.
  - `Features/Wallet/WalletLive.swift:391` — every outgoing row is `history.labelSent` 已发送 / 至 <addr> / '−' + compactAmount(value). A nil value reads '−0'.
  - `SignExecutor.swift:474` — the record stores dappOrigin (on iOS always the browser origin, because request_arrived sends `dapp: null`, `SigningController.swift:303`) and intent, but not dappUrl.
  - `rust/crates/vela-core/src/app/activity_feed.rs:1041` — `dapp_item` exists only in a core built from 43fd67a4 on.
  - `Features/Flows/FlowsLive.swift:262` — txDetail's title is `history.txLabelSent` with the item's symbol, which is '' for a swap.
- **Check** (parallel space; run `check-ios-core-fresh.sh` first and record which core the phone runs):
  1. Do a test-dApp Send dust (Gnosis, 0.001 xDAI; the page on Gnosis first, §3 Money), then a Uniswap 0.05 USDC → ETH swap on Base (about $0.08). That swap is this platform's single swap (§3): I-F1, I-F3 and I-U8 read it too.
  2. Open 首页 活动 and 全部.
  - BAD with a 079 core: no row for either, although prefs `vela.transactionHistory` holds `type:'dapp_tx'` records.
  - BAD with an 083 core: the swap row reads 已发送 · 至 0xd614…9c40 · −0, the dust row 已发送 · 至 0x… · −0.001 xDAI, and neither names the site.
  - GOOD:
    - dust: 发送 · <lan ip>:8000, with −0.001 xDAI;
    - swap: 合约交互 (or its intent) · app.uniswap.org;
    - rows are pending until they land, then confirmed;
    - the detail's first fact is 应用 = the site.
- **Fix:** mirror 43fd67a4 and ff660c2d.
  1. `TxRecords.toWire`: `dapp_url ← dappUrl ?? dappOrigin` (safe on iOS only, where dappOrigin is always the browser origin), and `intent ← intent`.
  2. `SignExecutor.recordRow`: write dappUrl from `record['dapp_url']`.
  3. `ActivityWire.FeedItemWire`: decode `dapp` (FeedDapp: site, intent, intentTerm, changes, received, estimated, contractCall).
  4. `WalletLive.activityRow` and `FlowsLive.txDetail`:
     - title = `componentsUi.signing.<intentTerm>`, else the intent, else `componentsUi.signing.intentContractCall`;
     - subtitle = the site;
     - a detail fact `connect.detail.labelApp`;
     - no '−0' for a row with no figure.

#### I-REC · P1 · likely (high) — dApp record integrity
- **Windows:** see A-REC.
- **Why here:**
  - `SignExecutor.swift:449` — recordRow takes `to = first["to"]` and `value = first["value"]` for EVERY dapp_tx, including wallet_sendCalls, whose top-level to/value are page-written and never shown or signed.
  - `SignExecutor.swift:450` — value is read as a String only, so a JSON-number value is stored as "0x0". The submit path also reads it as 0 (line 419).
  - `activity_feed.rs:1103` — the core drops non-address text, but a forged ADDRESS and a forged value still become the counterparty and the figure.
  - Safe already:
    - `Core/AddressText.swift:37` shortens by Character, so 日本語 cannot crash;
    - `SigningController.swift:303` sends `dapp: null`, so there is no self-declared name.
- **Check** (parallel space, Gnosis, fee only). In the test dApp console:
  ```
  ethereum.request({method:'wallet_sendCalls',params:[{version:'2.0.0',from:acct,chainId:'0x64',to:'0x000000000000000000000000000000000000dEaD',value:'0xffffffffffffffffffff',calls:[{to:acct,value:'0x0'}]}]})
  ```
  Slide, then:
  1. Read prefs with PlistBuddy.
     - BAD: the dapp_tx record has to 0x…dEaD and value 0xffff….
     - GOOD: to "" and value "0x0".
  2. With an 083 core, look at 活动. BAD: a −1,208,925 xDAI row to 0x0000…dEaD.
  3. Repeat with `to:'日本語日本語'` and relaunch. Expected: no crash.
- **Fix:** mirror ff660c2d (desktop `executor/sign_request.rs` persist_record). In `recordRow` (lines 435-480):
  1. Keep to/value only for eth_sendTransaction; a batch stores "" and "0x0".
  2. Read a numeric value properly (desktop `stored_value` / `wei_of`).
  3. Write dappUrl.

#### I-D1b · P1 · via core (medium) — Dismissed sheet + cancelled passkey never answered **[owner, Face ID]**
- **Windows:** see A-D1b.
- **Why here:** iOS sends every input the safety net reads.
  - `sign_request.rs:2314` — the net itself; its comment names the phones' case.
  - `SignExecutor.swift:247` — a cancelled passkey (or a Trusted Signer end) is reported as passkey_cancelled.
  - `SigningLive.swift:450` — during isSubmitting the receipt's button is `send.txCloseBackground` 关闭 · 后台继续 (关闭 under 签名中... for a message).
  - `App/RootView.swift:1524` — closeSigningSheet calls `live.swipeDismissed()`. The core's swipe action during Submitting is Dismiss, which clears the sheet (`sign_request.rs:1340-1352, 1012`).
  - `SigningController.swift:565` — the controller is marked closed only once answered, so it still dispatches the late passkey_cancelled.
  - The fix arrives with an xcframework built from 4198ec6b or later.
- **Check** (owner's account with Face ID; test-dApp Send dust on Gnosis; nothing is spent because Face ID is cancelled; logger on; run `check-ios-core-fresh.sh` first):
  1. Slide.
  2. Immediately tap 关闭 · 后台继续, before the Face ID sheet appears. The nonce read, deploy check and estimate take about 1–2 s.
  3. Cancel Face ID.
  - GOOD (083 core): exactly one ✗ 4001, and a following Sign on the same tab opens normally. A -32603 instead means iOS returned a non-cancel error (see I-H4b); record it — it still answers once.
  - BAD (079 core, or not rebuilt): the promise never settles, and the tab's next request may queue behind it.
- **Fix:** rebuild the xcframework. No shell change.

#### I-R2 · P1 · via core (high) — A bundle neighbour's ExecutionFailure fails our op
- **Windows:** see A-R2.
- **Why here:**
  - `Features/Send/TrackerExecutor.swift:143` — pollReceipt sends success:true as `receipt_with_logs` with the receipt's logs (address, topics, data, in order), and success:false as `receipt_failed`.
  - `Core/RelayClient.swift:549` — logs = the whole bundle's receipt.logs, in order.
  - `tx_tracker.rs:687` — the core runs `op_execution_failed`, scoped by EntryPoint BeforeExecution / UserOperationEvent (`tx_tracker.rs:1019-1066`).
  - `TrackerExecutor.swift:140` — the only ExecutionFailure mention in app-ios is a comment; there is no Swift-side check to remove.
  - The only condition is an xcframework built from 2448738a or later.
- **Check:** no practical device repro.
  1. `rust/scripts/check-ios-core-fresh.sh` exits 0.
  2. `cargo test -p vela-core op_execution` and `cargo test -p vela-core --test app_tx_tracker` pass.
  3. Optional: set a breakpoint in `TrackerExecutor.pollReceipt` during a real swap, and confirm `logs` includes entries from 0x0000000071727De22E5E9d8BAf0edAc6f37da032: the BeforeExecution topic, and the UserOperationEvent whose topic1 is the op hash.
  - BAD would be a landed swap whose 活动 row turns 失败 while its own success = 1.
- **Fix:** rebuild the xcframework. When S2 is ported, reuse this core rule (export `op_execution_failed`); do not add a Swift topic check.

#### I-W20 · P1 · via core (high) — iPhone caBLE: lower-case tunnel ids **[owner, two iPhones]**
- **Windows:** see A-W20.
- **Why here:**
  - `Features/Onboarding/Core/HybridCeremony.swift:143` — a tunnel ceremony (an advert with no PSM, which an iPhone authenticator sends) builds its WebSocket URL with the core's `cableConnectUrl(staticSeed:qrSecret:advertPlaintext:)`.
  - `Features/Onboarding/Core/CableTransports.swift:277` — WebSocketCableConn.connect prints 'opening tunnel: <url>'.
  - `rust/crates/vela-core-uniffi/src/ctap_bridge.rs:701` → `cable/session.rs:155-159` — upper-case since 24578479.
  - It matters for iOS's own sign-in/create "phone or tablet" route with a second iPhone. dApp signatures use Apple's hybrid instead (I-W19).
- **Check:** `check-ios-core-fresh.sh` must pass on 24578479 or later.
  1. On the Vela iPhone: account switcher's sign-in row ▸ 手机或平板.
  2. Scan the QR with the second iPhone's Camera and approve with Face ID.
  3. Read the `--console` output.
  - GOOD:
    - `[vela-cable] advert has no PSM — WebSocket tunnel`;
    - `opening tunnel: wss://cable.auth.com/cable/connect/<UPPER-HEX>/<UPPER-HEX>`;
    - `WebSocket tunnel established (fido.cable)`;
    - the sign-in completes.
  - BAD: lower-case hex in the URL, or the socket closing ('Policy violation', 1008) right after the first frame.
- **Fix:** none in Swift. Rebuild the xcframework.

#### I-U8 · P2 · likely (medium) — Fee about 14× the on-chain cost (owner decision)
- **Windows:** see A-U8.
- **Why here:** the same core `fee_policy` (×3, padded limits, Fast).
  - `App/RootView.swift:1637` — the sheet's speed is `settings.feeTier?.tier ?? "fast"`.
  - `SigningController.swift:192` — preferredTier defaults to "fast".
  - `fee_policy.rs:91` — `INBAND_MARKUP = 3`.
- **Check** (parallel space, Base, about $0.08):
  1. Use this platform's single 0.05 USDC → ETH swap (the I-H2/I-F1 one; no swap of its own, §3). Note the sheet's fee and the speed (快速 unless Settings says otherwise).
  2. After it lands, read the receipt. Compare `UserOperationEvent.actualGasCost` (the 3rd 32-byte data word, in wei) × the ETH price, plus the receipt's l1Fee share, against the fee leg's transfer to the relay's recipient.
  - Expect about 10–15× (Windows: $0.098 charged against $0.0068). Record the tier, fee, actualGasCost and l1Fee.
- **Fix:** the owner's decision; any change goes in the core. iOS's default tier is set at `RootView.swift:1637` and `SigningController.swift:192`.

#### I-F1 · P2 · likely (high) — A dApp row says nothing about what it moved
- **Windows:** see A-F1.
- **Why here:**
  - `SigningController.swift:685` — approveOpts sends no `balance_changes`, so the core records none (`sign_request.rs:1811-1820` keeps only what the approve carries).
  - `SignExecutor.swift:460` — recordRow writes no assetChanges and no calldata flag.
  - `Core/TxRecords.swift:163` — toWire has neither.
  - `SigningLive.swift:628` — the sheet draws `context.sim.judgments` (TrustSimJudgmentWire, from the token_trust store). Those judgments are what the approve should carry.
- **Check:** needs the I-H2 mapping and an 083 core.
  1. Use the I-H2 swap (0.05 USDC → ETH on Uniswap, about $0.08; §3), and note the sheet's 余额变化 before sliding.
  2. Open the row and its detail.
  - BAD: no figure (−0 or blank), no 余额变化 in the detail, and no assetChanges on the record in prefs.
  - GOOD: the row reads '≈ −0.05 USDC / ≈ +0.000019 ETH', the detail repeats the sheet's lines, and an unverified inflow shows no number.
- **Fix:** mirror 48b3e931, 9004bba3, b2430a4e.
  1. `approveOpts.balance_changes` = the judgments the sheet drew, as TrustSimJudgment JSON.
  2. `recordRow` stores assetChanges (the desktop's stored_changes shape), and `calldata` = whether any signed call has calldata. Take it from `callsOf`, not from the 4 KiB-clipped signedRequest.
  3. `toWire` maps both.
  4. Draw `FeedDapp.changes` / `received` / `estimated` with '≈'.

#### I-F3 · P2 · likely (high) — The called contract labelled 接收方
- **Windows:** see A-F3.
- **Why here:**
  - `Features/Flows/FlowsLive.swift:214` — an outgoing counterparty is always labelled `componentsTx.detail.to` 接收方.
  - `SignExecutor.swift:449` — a dApp record's `to` is the called contract.
  - `activity_feed.rs:1103` — with an 083 core the router becomes the counterparty, and `contract_call = (calldata == Some(true))`, a flag iOS never stores.
- **Check** (083 core): open the swap row's detail; the I-H2/I-F1 row will do.
  - BAD: 接收方 0xd614…9c40.
  - GOOD: 合约 0xd614…9c40, while the Send dust row keeps 接收方.
- **Fix:** in `FlowsLive.txDetail` (lines 211-224), use `tokenDetail.labelContract` when `item.dapp.contractCall`. Needs I-F1's calldata flag. Mirror 48b3e931, 9004bba3.

#### I-H4a · P2 · likely (high) — A failed MESSAGE signature shows the transaction sentence
- **Windows:** see A-H4a.
- **Why here:**
  - `SigningLive.swift:412` — `error.kind == .submitFailed` shows `statusFailed` + `send.txErrorGeneric`, with no on-chain check.
  - `SigningLive.swift:369` — statusBlocks adds the same warning for any error.
  - `SignExecutor.swift:274` — signMessage returns failed "<method> carried nothing this wallet could sign" when messageHash is nil. Any non-cancel ceremony error also fails (line 294).
  - `app-ios/VelaWallet/VelaWalletTests/SigningReceiptTests.swift:113` — pins submitFailed → txErrorGeneric.
  - A free trigger exists: the core does not check that typed data can be hashed before showing the sheet, so the failure fires after the slide.
- **Check — DECISIVE:** in SigningReceiptTests, call `SigningLive.receipt(sign: sign(error: .submitFailed, kind: .personalSign), …)`.
  - BAD (today): the captions contain `loc.t('send.txErrorGeneric')`.
  - GOOD: `connect.detail.offChainNote` 链下签名 — 未向链上发送任何内容。
- **On the device** (parallel space, no funds). In the console run the line below, then slide:
  ```
  ethereum.request({method:'eth_signTypedData_v4',params:[acct, JSON.stringify({types:{EIP712Domain:[]},primaryType:'Missing',domain:{},message:{}})]})
  ```
  - BAD: `✗ -32603 'eth_signTypedData_v4 carried nothing this wallet could sign'`, and the sheet shows 失败 + 交易未能提交。您的资金安全无虞——请重试。 + 完成.
  - GOOD: the failure reads as off-chain.
  - If the sheet refuses before a slide is possible, use a real signing failure on the owner's account instead (I-H4b).
- **Fix:** in `SigningLive.receipt` / `statusBlocks`, use `connect.detail.offChainNote` when `sign.request.kind` is personalSign, ethSign or typedData. Mirror a22e1b30.

#### I-H4b · P2 · likely (low) — A non-cancel passkey error fails the request with Apple's text **[owner, Face ID]**
- **Windows:** see A-H4b. Owner decision 2 applies.
- **Why here:**
  - `App/RootView.swift:313` — the signing spine uses `ParallelSpaceHook.signer(passkey: PasskeyExecutor())`: a bare executor with `hybrid == nil` and `smartCard == nil`. dApp signing never runs Vela's own caBLE/CCID; phones and keys go through Apple's system sheet. A person dismissing it gives `.canceled`, which keeps the request open, correctly.
  - `Features/Onboarding/Core/PasskeyExecutor.swift:316` — `.hybrid` and `.securityKey` fall through to systemRequests.
  - `Core/UserOpSpine.swift:600` — only `.cancelled` (and CancellationError) keep the request open; everything else becomes `other(message)`.
  - `PasskeyExecutor.swift:483` — classify maps `.failed`, `.invalidResponse` and `.unknown` to `.other(localizedDescription)`, and `.notHandled` / `.notInteractive` to `.notSupported`.
  - `SignExecutor.swift:260` — `.other` becomes failed. The page gets -32603 with Apple's text, and the sheet shows the transaction failure with only 完成.
  - HybridCeremony's English "No phone answered…" is onboarding-only (`OnboardingModel.swift:206`).
- **Check** (owner's account with Face ID; test dApp Sign, no funds, or Send dust cancelled before signing; logger on). These are attempts, not guaranteed triggers:
  - (a) Slide, then press the Home gesture within about 1 s, before Face ID appears. Return to the app.
  - (b) With a security-key signer: pick the key in the system sheet, then tap a key that does not hold the credential, or pull it out mid-ceremony.
  - BAD: `✗ -32603` with an Apple sentence (e.g. '…AuthorizationError error 1004'), and the sheet shows 失败 + 交易未能提交。您的资金安全无虞——请重试。 with only 完成.
  - GOOD (the desktop's behaviour, pending owner decision 2): the request stays open with 重试/关闭, and ✕ gives one 4001.
  - Control: cancelling Face ID must give no answer, with the form back and the slide available.
  - If neither (a) nor (b) produces a non-cancel error, record I-H4b as "not reachable on this device".
- **Fix:** mirror a22e1b30, 95b8e573, 1063909f; owner decision 2 applies.
  1. Map a non-cancel ceremony failure to a retryable state: passkey_cancelled plus a notice with Retry/Close.
  2. Never pass Apple's `localizedDescription` to the page.

  Touch points: `UserOpSpine.assert` (593-605), `SignExecutor.swift:246-262` and `:288-297`.

#### I-B5792 · P2 · partially (medium) — wallet_sendCalls answer / capabilities
- **Windows:** see A-B5792.
- **Why here:**
  - `dapp_rpc.rs:172` → `dapp_browser.rs:1263` — both methods are Unsupported and get 4200, the same as the desktop.
  - `SignExecutor.swift:409` — wallet_sendCalls is answered like eth_sendTransaction: a bare tx-hash string (or the op hash after 120 s), not `{id}`.
  - `sign_request.rs:2119` — `on_op_submitted` writes a PENDING record for batches, so iOS has no "confirmed under an op hash" bug.
- **Check** (parallel space, test dApp console on Gnosis):
  1. `await ethereum.request({method:'wallet_getCapabilities',params:[acct]})` → expect `✗ 4200`.
  2. Tap Bundle, or send `wallet_sendCalls` v2.0.0 with one call `{to:acct,value:'0x0'}` (fee only). Note the result's shape: today a '0x…' string, where EIP-5792 v2 expects `{id}`.
  3. `wallet_getCallsStatus` with that result → expect `✗ 4200`.
  4. The record in prefs: pending, then confirmed with a txHash.
  - BAD: 'confirmed' under a userOpHash, or a reverted batch reported as success.
- **Fix:** nothing while capabilities stay 4200. If EIP-5792 is adopted: answer `{id}` as receipt_pending (web ff660c2d), and add wallet_getCallsStatus in the core that reports the real outcome, reverted included.

#### I-W13 · P2 · likely (high) — A dApp read waits 8 s per silent node (H6)
- **Windows:** see A-W13.
- **Why here:**
  - `Core/RpcPool.swift:353` — `post()` sends exactly one request to the one URL the core named, and awaits it.
  - `RpcPool.swift:444` — the timeout is the core's `timeout_ms`: 8 000 ms for reads (`rpc_pool.rs:1626-1628`), set as URLRequest's timeoutInterval (an idle timeout).
  - `rpc_pool.rs:167, 1798, 1821, 952` — `HEDGE_AFTER_MS`, `is_hedged_read`, `early_verdict` and `pending_urls` form a contract that the shell must drive. None is exported through uniffi, and `pending_urls` is serde-skipped (line 950), so RpcPoolViewWire cannot decode it.
  - `rpc_pool.rs:519` — a timed-out endpoint is cooled down for 30 s, doubling up to 300 s. So only the first read after a node goes silent (and the first after each cooldown) pays the 8 s.
  - `App/RootView.swift:389` — the dApp browser's poolCall uses the same `RpcPool.call` as the wallet.
- **Check** (chaos proxy set as the iPhone's Wi-Fi proxy; parallel space; test dApp switched to Base, or app.uniswap.org on Base; no funds, no Face ID):
  1. From chaos.log, note the Base RPC host the wallet reaches.
  2. `curl '…/__chaos?mode=blackhole&match=<that-host>'`
  3. Immediately, in Web Inspector:
     ```
     for (let i=0;i<5;i++){const t=performance.now(); await ethereum.request({method:'eth_blockNumber'}); console.log(i, Math.round(performance.now()-t))}
     ```
  - BAD (expected): the first read takes about 8 000–8 500 ms and the rest are fast (the cooldown). To see it again, wait more than 30 s and repeat.
  - GOOD (hedged): every read takes about 2 s or less.
  4. Set `mode=pass`.
- **Fix:** needs the owner's go-ahead (decision 3). Then mirror 8872b915 and e6b7469e:
  1. Export `is_hedged_read`, `early_verdict` and `HEDGE_AFTER_MS`, plus an accessor for a call's next URL (it is serde-skipped, so do not just decode the view).
  2. In `RpcPool.post`, after 1.5 s also post to that URL for hedged reads.
  3. Conclude on the first `early_verdict`, without taking the core's own post slot.

#### I-H1 · P2 · likely (high) — Sign-in sheet copy
- **Windows:** see A-H1.
- **Why here:**
  - `Features/Onboarding/WelcomeScreen.swift:102` — SignInMethodSheet uses `methodCopy(method)`, the CREATE chooser's lines.
  - `Features/Onboarding/FlowCopy.swift:78` — `.hybrid` → methodHybridBody, `.platform` → methodPlatformBody.
  - `assets/i18n/zh.json:777/780` — 扫码，用附近设备创建 / Touch ID 或 Windows Hello. iOS's catalogs are synced from assets/i18n (`Loc.swift:8`).
  - `Features/Onboarding/CableQrSheet.swift:31` — the QR sheet shown under a sign-in scan also uses methodHybridBody. Its comment: "dedicated copy is an i18n-gate follow-up". It has no Cancel, and `interactiveDismissDisabled(true)` (line 52).
  - `App/RootView.swift:3088` — the picker also opens from the account switcher (onAccountSignIn at 1173, 1327, 3088).
  - No iOS code reads `LAContext.biometryType`.
- **Check** (`VELA_LANG=zh`; no sign-out needed). Open the account switcher (钱包's header, or 设置 ▸ 账户) and use its sign-in row, or 我已有钱包 on the Welcome screen.
  - GOOD: 手机或平板 says 扫码 (no 创建), and 这台设备 says Face ID (Touch ID on a Touch ID model), never Windows Hello.
  - BAD: 扫码，用附近设备创建 and Touch ID 或 Windows Hello.
  - Only if you are willing to wait about 90 s: tap 手机或平板 and read the QR sheet's line. There is no Cancel; do not scan, and wait the window out.
  - Also check the create chooser's 这台设备 line (创建钱包).
  - No Face ID or funds needed.
- **Fix:** mirror d9ab6e80, 5c79c6b3.
  1. Give SignInMethodSheet a sign-in variant of methodCopy:
     - hybrid body: `explore.scan`;
     - platform body: the product name from `LAContext().biometryType` (Face ID, Touch ID or Optic ID), kept verbatim as the desktop does.
  2. CableQrSheet takes its line from its caller.

#### I-H5 · P2 · likely (medium) — "Check your phone" after the scan; Cancel; handshake wait **[owner, second phone]**
- **Windows:** after the phone scanned, the QR stayed up until the phone's prompt; 查看你的手机 had no Cancel; a phone that dropped after scanning held the handshake for up to 130 s. The desktop fixes (a22e1b30, 95b8e573, 1063909f):
  - the QR gives way to 查看你的手机 at the scan;
  - a Cancel;
  - 15 s per handshake frame;
  - a 1008 close counts as a link that never came up, not as the phone's cancel.

  This item was not in the Windows inventory; the iOS review added it.
- **Why here:** this is iOS's own "phone or tablet" create and sign-in. dApp signatures do not use it.
  - `App/RootView.swift:3526` — the QR (`onboarding.cableQr`) gives way only to usbTouch, which is raised when the ceremony calls `CableHostBridge.touch` (`Features/Onboarding/Core/HybridCeremony.swift:249`).
  - `Features/Onboarding/CableQrSheet.swift:52` — `interactiveDismissDisabled(true)` and no Cancel ("it times out on its own"). The scan window is 90 s (`HybridCeremony.swift:219`).
  - `Features/Onboarding/UsbCeremonyPrompts.swift:253` — the 查看你的手机 sheet also has no Cancel.
  - `Features/Onboarding/Core/CableTransports.swift:308` — the WebSocket tunnel's readFrame has a 130 s watchdog.
  - `Features/Onboarding/OnboardingModel.swift:203` — this initiator is wired only into onboarding.
  - The final failure is `PasskeyFailure(.other)` carrying the core's detail text (`SmartCardCtapCeremony.swift:249`); the device must show the wording.
  - All timings come from the code, not from measurement.
- **Check** (the owner's second phone, an iPhone or an Android with Google Play; account switcher's sign-in row ▸ 手机或平板):
  1. Scan with the other phone, but do not approve yet. Time how long Vela keeps the QR after the other phone shows its prompt.
     - GOOD: 查看你的手机 within about 1 s of the scan (console `[vela-cable] advert has no PSM — WebSocket tunnel`).
     - BAD: the QR stays until the phone's prompt, or longer.
  2. Look for a Cancel on the QR sheet and on 查看你的手机.
     - BAD (the code today): none, and swipes do nothing.
  3. Scan, then put the other phone into Airplane Mode before approving. Time how long Vela takes to give up, and read the message.
     - GOOD: seconds (the desktop uses 15 s), with a retryable "connection failed" sentence.
     - BAD: about 130 s, or raw text such as 'caBLE transport: …'.
     - Console: `[vela-cable] ← read timed out via …`, `[vela-cable] ceremony failed: …`.
- **Fix:** mirror a22e1b30, 95b8e573, 1063909f.
  1. `HybridCeremony.run`: call a "found" hook when findResponder returns a hit, and have OnboardingModel swap the QR for 查看你的手机 then.
  2. Give CableQrSheet and the remote UsbTouchSheet a Cancel that cancels the scanner and the port (`scanner.cancel()`, `port.close()`) and fails the ceremony as `.cancelled`.
  3. Cut `WebSocketCableConn.readFrame`'s watchdog to about 15 s for the handshake frames.

#### I-H8 · P2 · likely (medium) — After a failed second navigation the tab names the previous site
- **Windows:** the tab said "Uniswap Interface" over an expired.badssl.com failure. The desktop now names the tab by the failed address (0ff7fbed).
- **Why here:**
  - `Features/Explore/Core/BrowserEngine.swift:360` — update() uses `current = liveURL ?? (failure != nil ? failedURL : url)`. failedURL is used only when WebKit reports no URL at all (a fresh tab). After a failed SECOND navigation, url, host and title become the previous site's again.
  - `BrowserEngine.swift:353` — metaChanged runs on URL KVO and sends `onMeta(url, title)`, re-pointing the tab record at the previous page (`Features/Explore/Core/BrowserController.swift:551`).
  - `Components/Explore/BrowserWebView.swift:33` — the team's own note: "the previous page on a failed second navigation".
  - `Features/Explore/ExploreScreen.swift:229` — only the panel's detail line uses failedURL. The pill (line 154) uses engine.host, and the lock (line 156) uses the core's shown_origin.
  - `ExploreScreen.swift:259` — ⋯ 复制链接 / 分享 / 在系统浏览器中打开 act on the previous page's URL.
  - No test covers a used tab: BrowserLoadTests and DappBrowserStabilityProbeTests are fresh-tab only.
  - Confidence is medium because this rests on WebKit reverting webView.url and firing KVO after didFailProvisionalNavigation.
- **Check** (parallel space):
  1. Open https://app.uniswap.org and let it finish loading.
  2. Tap the pill, clear it, type expired.badssl.com and tap 前往.
  3. Then read (a) the pill and its lock, (b) the card title in 标签页, and (c) what ⋯ ▸ 复制链接 pastes.
  4. Repeat with https://vela-083-nohost.invalid/.
  - GOOD (the desktop after 0ff7fbed): the pill names expired.badssl.com with no closed lock; the card is named by the failed host; 复制链接 gives https://expired.badssl.com/.
  - BAD: the pill shows 🔒 app.uniswap.org, the card says 'Uniswap Interface', 复制链接 gives Uniswap's URL, and only the grey line names the failed host.
- **Fix:** mirror 0ff7fbed.
  1. `BrowserEngine.update()`: while failure != nil, use failedURL for url, origin and host, set title = "", and send `onMeta(failedURL, "")`.
  2. `ExploreScreen.browserSecure` returns false while failing.
  3. `pick(menuItem:)` acts on `engine.failedURL` while failing.

#### I-W2 · P2 · partially (medium) — Typing in the address bar appends
- **Windows:** typing "example.com" on PancakeSwap loaded pancakeswap.finance/example.com (a gpui cause, 885867f4).
- **Why here:** the symptom is real on iOS, from a different cause.
  - `Components/Explore/AddressBarView.swift:96` — startEditing() puts the whole URL into a plain SwiftUI TextField with the caret at the END. Nothing is selected and there is no clear button, so typed text is APPENDED.
  - `AddressBarView.swift:63` — onSubmit sends the whole draft to `controller.open` → dappBrowserInput. So 'example.com' typed after 'https://pancakeswap.finance/' becomes https://pancakeswap.finance/example.com.
  - `BrowserEngine.swift:214` — the comment says the bar "keeps the committed host until then". But KVO on webView.url (line 338) → metaChanged → update() sets host, and WebKit reports the pending URL right after load(). So the pill most likely names the NEW host at once; a device must settle this.
  - Side effects: `onMeta(new url, OLD title)` makes the tab card show the new host under the previous site's title until the new title arrives, and the lock stays the previous origin's until commit (§10 I-12).
  - `BrowserController.swift:231` — open() sends `tab_navigated(url, title: null)`, then loads.
- **Check** (parallel space): open https://pancakeswap.finance.
  1. Tap the host pill.
     - GOOD: the whole URL is selected, or the field is empty with the URL as a hint.
     - BAD (expected): the caret sits after the URL.
  2. Without clearing, type example.com and tap 前往.
     - GOOD: example.com loads (`location.href === 'https://example.com/'`).
     - BAD: a pancakeswap URL ending in /example.com.
  3. Clear the field, type app.uniswap.org and submit. Take screenshots at about 0.3 s and 3 s, and open 标签页 during the load.
     - GOOD: the pill shows app.uniswap.org from the first frame.
     - BAD: pancakeswap.finance until the commit.
     - Also record whether the card reads app.uniswap.org under PancakeSwap's title, and whether a closed lock sits next to the new host before the commit.
- **Fix:** mirror 885867f4.
  1. Select all when editing begins. A SwiftUI TextField cannot do this, so wrap a UITextField (selectAll in textFieldDidBeginEditing).
  2. Set `clearButtonMode = .whileEditing`.
  3. While the draft is untouched, let the open editor follow a commit.
  4. Correct the comment at `BrowserEngine.swift:214-216`.

#### I-W6 · P2 · partially (high) — target=_blank / window.open replace the dApp in the same tab
- **Windows:** window.open returned null and target=_blank links were dead. The desktop now opens a new Vela tab, on a user gesture only (e6285fc9, a6b1b2fb).
- **Why here:**
  - `BrowserEngine.swift:705` — createWebViewWith says 'target="_blank" loads in **this** tab': it calls `webView.load(target)` and returns nil. So `window.open()` returns null, and a new-window request REPLACES the dApp.
  - `BrowserEngine.swift:148` — `javaScriptCanOpenWindowsAutomatically = false`: popups without a gesture are refused.
  - `BrowserEngine.swift:711` — there is no sourceFrame check, so a tapped _blank link, or a window.open inside a cross-origin iframe, navigates the whole tab.
  - `rust/crates/vela-core/src/app/explore_sites.rs:491` — TabOpened is dropped silently at `TABS_CAP` (24), so a fix must fall back to the current tab when the strip is full.
  - Back returns to the dApp, reloaded or from WebKit's back-forward cache.
- **Check** (test dApp at `http://<mac-lan-ip>:8000/`):
  1. Tap the target=_blank link.
     - Today: example.com replaces the test dApp, and the tab count does not change.
     - GOOD (desktop parity): a new selected tab on example.com, with the test dApp still intact in 标签页.
  2. In Web Inspector:
     ```
     document.body.insertAdjacentHTML('beforeend','<button id=wo style=font-size:30px>open</button>'); wo.onclick=()=>console.log('open ->', window.open('https://example.com/'))
     ```
     Tap the button on the phone. Record the console value and where example.com opened.
     - GOOD (desktop parity): a new tab, and a window object in the console.
     - Today: null, and the dApp replaced.
  3. Without a gesture, run `setTimeout(()=>console.log(window.open('https://example.com/')),1500)`, and separately `setTimeout(()=>{const a=document.querySelector('a[target=_blank]');a.click()},1500)`.
     - GOOD: null, and nothing navigates.
     - BAD: the tab leaves the test dApp.
  4. On app.uniswap.org, tap a link that Safari would open in a new window (e.g. external docs or help), or the explorer link after the I-H2 swap (no extra swap, §3). Record whether the Uniswap page is lost from the tab.
     - BAD: the Uniswap page is lost from its tab.
- **Fix:** mirror e6285fc9, a6b1b2fb. Ask the owner first whether phones should open new tabs (§11.3). In createWebViewWith, for http(s) with a gesture from the top document (`navigationAction.sourceFrame.isMainFrame`, or the same security origin as the top):
  1. Call a new `onOpenTab(url)`, which sends explore_sites `tab_opened` (this selects the tab).
  2. When the tabs are full, load in the current tab instead.
  3. Keep returning nil.

#### I-W7 · P2 · partially (medium) — Any scheme leaves the app; possibly without a tap; downloads unhandled
- **Windows:** mailto went straight to the OS app picker, and a script's `location='mailto:'` also left the app. The desktop now lets only mailto/tel leave, only on a tap; other schemes and downloads are refused (e6285fc9, a6b1b2fb).
- **Why here:**
  - `BrowserEngine.swift:684` — policy():
    - http(s) is allowed;
    - about, blob and data only in a subframe;
    - javascript, file and empty are cancelled;
    - EVERY other scheme gets `.handToSystem` when `isMainFrame && linkActivated`. That includes tel:, mailto:, sms:, itms-apps:, shortcuts:, metamask: and more.
  - `BrowserEngine.swift:646` — the "tap" test is `navigationType == .linkActivated`. WebKit also reports LinkClicked for a script's `element.click()` on an `<a>`, so this is probably not a user-gesture test. Assigning `location=` is `.other` and is cancelled.
  - `BrowserEngine.swift:661` — `.handToSystem` calls `UIApplication.shared.open` with no Vela confirmation. iOS itself does not ask for most schemes (§10 I-13).
  - `BrowserEngine.swift:642` — `isMainFrame = targetFrame?.isMainFrame ?? true`. A _blank custom-scheme link inside a cross-origin iframe therefore counts as the main frame.
  - `BrowserEngine.swift:543` — decidePolicyFor navigationResponse always returns `.allow`; there is no `.download` and no WKDownload. A body WebKit cannot show fails provisionally:
    - WebKitErrorDomain 102 → None (nothing shown);
    - 100 and others → Other (the load-failed panel over the live dApp, plus one retry).
  - `BrowserEngine.swift:226` — retry/重试 re-requests failedURL, which is the download URL, so the panel comes back and hides the dApp until the person navigates elsewhere.
- **Check** (test dApp):
  1. Tap the mailto link. Record whether Mail opens with no prompt. Either result is acceptable (the desktop allows mailto on a tap), but record it.
  2. Console: `location.href='mailto:a@b.c'`, then `location.href='tel:10086'`. GOOD: nothing happens.
  3. Without a gesture:
     ```
     setTimeout(()=>{const a=document.createElement('a');a.href='itms-apps://apps.apple.com/app/id284882215';document.body.append(a);a.click()},1500)
     ```
     Then the same with 'mailto:a@b.c' and 'sms:10086'.
     - GOOD: nothing leaves the app.
     - BAD (probable today): the App Store, Mail or Messages opens.
  4. A tapped custom scheme:
     ```
     document.body.insertAdjacentHTML('beforeend','<a id=x href="itms-apps://apps.apple.com/app/id284882215" style=font-size:30px>appstore</a>')
     ```
     Tap it; also try `shortcuts://`. Today the App Store or Shortcuts opens, which is BAD against the desktop rule.
  5. Downloads, no internet needed. On the Mac:
     ```
     mkdir -p /tmp/dl && head -c 4096 /dev/urandom > /tmp/dl/a.bin && python3 -m http.server 8002 --bind 0.0.0.0 --directory /tmp/dl
     ```
     Inject `<a href="http://<mac-lan-ip>:8002/a.bin">dl</a>` and tap it; also try a .zip.
     - GOOD: nothing happens, and the dApp stays usable.
     - BAD: 无法加载此页面 over the dApp (console 'WebKitErrorDomain 100 → other', retry at 3 s; record whether 重试 brings the dApp back), or the dApp replaced by a file view.
- **Fix:** mirror e6285fc9, a6b1b2fb. The scheme policy is an owner question (§11.3).
  1. Limit `.handToSystem` to mailto and tel; decide on sms.
  2. Require `sourceFrame.isMainFrame` (or the top origin).
  3. Add a real gesture signal: a touchend listener in the provider world within about 1 s, or always ask with a Vela sheet before `UIApplication.open`.
  4. Cancel everything else.
  5. In decidePolicyFor navigationResponse, return `.cancel` when `!navigationResponse.canShowMIMEType`, or for `Content-Disposition: attachment`.

#### I-W10 · P2 · handled (high) — A plain native-coin transfer
- **Windows:** see A-W10.
- **Why:**
  - `SigningLive.swift:569` — resolved, with no result/message/blindTyped, `dataBytes == 0` and a `to`: plainTransferBlocks.
  - `SigningLive.swift:575` — plainTransferBlocks is intentSend, the amount in the native symbol, and 接收方.
  - `rust/crates/vela-core/src/app/clear_signing.rs:1537` — TxPlain gives ConfirmIntent, so the slide reads 确认发送.
  - It is P2, not P3, because the desktop's single-call and decoy guards and the web's `input` rule are missing (§10 I-1 to I-4).
- **Check** (test dApp on Gnosis, parallel space): tap Send dust, inspect the sheet, then reject (or slide for 0.001 xDAI plus the fee).
  - GOOD: 发送 · −0.001 xDAI · 接收方 0x…, a slide reading 确认发送, and no 无法解码.
  - BAD: 合约交互 with '⚠ 无法解码 — … (0 字节)'.

#### I-W11 · P3 · partially (medium) — Wrong progress label around Face ID **[owner for the Face ID part]**
- **Windows:** see A-W11.
- **Why:**
  - `SigningLive.swift:450` — isSubmitting shows `send.txSubmitting` 提交至网络..., and it is checked BEFORE isSigning (line 458, `send.txSigning` 等待生物识别...).
  - `sign_request.rs:1240` — the nonce read, estimate, passkey and relay submit all run inside Submitting.
  - `SignExecutor.swift:117` — funding/sponsorship are answered at once, so 等待生物识别… only flashes.
  - `SigningController.swift:246` — `signingStarted: {}` is not wired.
  - `send.txPreparing` 正在准备交易... exists, and SendLive uses it (`SendLive.swift:1070`).
  - So the Windows symptom is mostly absent here; the inverse mislabel remains.
- **Check** (owner's account with Face ID; Send dust on Gnosis). Screen-record, slide, and read the receipt title frame by frame.
  - Today: 等待生物识别... for a blink, then 提交至网络... before and during the Face ID sheet, then 交易已提交至网络.
  - GOOD: 正在准备交易... until Face ID appears, 等待生物识别... only while it is up, then 提交至网络....
  - In the parallel space only the preparing/submitting order can be checked.
- **Fix:** mirror 545621d7 and web 3707e196 (signingStatus).
  1. Wire `SignExecutor.Ports.signingStarted` (`SigningController.swift:246`) and the ceremony's end into controller state.
  2. `SigningLive.receipt`: txPreparing until the prompt, txSigning during it, txSubmitting after.

#### I-EXE · P3 · likely (medium) — Sheet headline is the English "Execute"
- **Why:**
  - `SigningLive.swift:748` — the headline is `.intent(text: result.intent)`.
  - `SigningLive.swift:118` — localizedTerms swaps the intent only when the core names an intentTerm.
  - `SigningLive.swift:762` — a best-effort result adds bestEffortWarning.
- **Check** (parallel space, Uniswap on Base, any swap): look, then reject with ✕ (or read it on the I-H2 swap's sheet; no extra swap, §3).
  - BAD: the headline reads 'Execute' in a zh UI, with the best-effort caution, and the slide reads the neutral 确认.
  - GOOD (future): 兑换 0.1 USDC → ETH.
- **Fix:** a core descriptor, shared by all clients.

#### I-F2 · P3 · handled (high) — Transaction hash in the detail
- **Why:**
  - `Features/Flows/FlowsLive.swift:247` — `AddressText.short(hash)`, mono, with copyValue = the full hash.
  - `Core/AddressText.swift:36` — `prefix(6)…suffix(4)` for anything longer than 14.
  - `SigningLive.swift:485` — the receipt shortens too (prefix 10 … suffix 8) and copies the full hash.
- **Check:** open any 活动 row with a hash, then its detail.
  - GOOD: 哈希 0x7ca7…9269 fits on one line, and copy pastes all 66 characters.
  - BAD: the hash is clipped or runs off the panel.

#### I-W9 · P3 · handled (high) — No demo host in the live browser
- **Why:**
  - `Features/Explore/ExploreLive.swift:243` — the live chrome takes host and url from the engine ("" otherwise), and `secure = tab?.secure ?? false`.
  - `ExploreScreen.swift:136` — the view is `.browsing` only while an engine exists.
  - `ExploreScreen.swift:554` — between the address and its engine, the plain background shows. DemoPageView only appears when `controller == nil` (line 560).
  - `App/RootView.swift:822` — the fixture Explore appears only in the VELA_PAGE gallery. The live mount (line 1245) is ExploreLive.home.
  - `BrowserController.swift:96` — pageWanted: a restored tab gets no engine until the person asks (line 514).
  - Residue: `ExploreLive.swift:64` falls back to `ExploreFixtures.uniswap` for the site menu when there is no engine. A person cannot reach it.
- **Check** (parallel space, without VELA_URL; do NOT reinstall on the owner's phone):
  1. 标签页 ▸ 关闭全部标签页; force-quit; relaunch; open 探索.
     - GOOD: 起始页, and no 'app.uniswap.org', 'Uniswap' or 'Polymarket' unless the person favourited or visited it.
  2. Open a site; force-quit; relaunch; open 探索.
     - GOOD: the start page, with the tab waiting in 标签页. It loads only after you tap it (the console shows no 'browser load' before that).
  - BAD: 🔒 app.uniswap.org with nothing loaded, demo tabs, or the restored tab loading at launch.
- **Fix:** optional hygiene. Replace the `ExploreLive.swift:64` fallback with an empty SiteModel (dbf5a48c).

#### I-W3 · P3 · handled (high) — Error pages and recovery
- **Why:**
  - `BrowserEngine.swift:480` — "WebKit draws no error page of its own".
  - `BrowserEngine.swift:613` — fail() classifies with `browserLoadClassify(platform: "apple", …)`.
  - `BrowserEngine.swift:496` — a load that ends on about:blank (behind a proxy) is treated as NetworkConnectionLost, so the panel still shows.
  - `BrowserEngine.swift:276` — the core's retry schedule runs only while the page is on screen and the app active; lines 240 and 263 resume it.
  - `rust/crates/vela-core/src/app/browser_load.rs:201` — offline, timeout and refused retry at 2/5/10 s; 'other' retries once at 3 s; notFound and certificate never retry.
  - `ExploreScreen.swift:541` — the panel is opaque over the web view.
  - Designed gaps:
    - there is no NWPathMonitor, so a network that returns after about 17 s needs 重试 (the desktop behaves the same);
    - a black-holed host waits out the request's own 60 s timeout (§10 I-14).
- **Check** (chaos proxy as the Wi-Fi proxy; use a FRESH tab each time):
  1. `mode=drop&match=uniswap`, then open https://app.uniswap.org.
     - GOOD: 无法加载此页面 / 网络不稳定，页面没能打开。 / app.uniswap.org / 重试; 正在重试… at about 2, 5 and 10 s; console `[vela-wallet] browser load failed: … → offline`.
     - Never a white page, never WebKit's own text.
  2. Start a drop again, and switch to `mode=pass` within 8 s. GOOD: the page loads by itself at the next attempt.
  3. `mode=blackhole&match=pancakeswap`, then open pancakeswap.finance. Record when the panel first appears (the code predicts about 60 s, -1001 → timeout) and whether it stays through 90 s.
  4. After a drop has used up its schedule (more than 20 s), switch to pass and wait 30 s. By design the panel stays until 重试; flag it if the owner wants recovery on its own.
  5. Turn the proxy Off.
- **Fix:** only for parity with the desktop's timings: a 12–15 s provisional-load watchdog, and an NWPathMonitor that retries the on-screen failed tab (9e0fe081 LoadWatch).

#### I-DNS · P3 · handled (medium) — Lookup failures
- **Why:**
  - `browser_load.rs:127` — the Apple codes -1000, -1003, -1006 and -1002 map to NotFound, which never retries (line 219) and says 找不到这个网站，请检查网址。.
  - `BrowserEngine.swift:572` — numeric NSError codes, so the result does not depend on the language.
  - `BrowserEngine.swift:498` — behind a proxy, a first load can end on about:blank and read as Offline.
- **Check:** open https://vela-083-nohost.invalid/ in a fresh tab.
  1. Plain Wi-Fi, no VPN or proxy.
     - GOOD: 找不到这个网站，请检查网址。, no 正在重试…, and console `NSURLErrorDomain -1003 → notFound` (or -1006).
     - BAD: 网络不稳定… with retries.
  2. Repeat with Shadowrocket/Clash (fake-IP), and with the chaos proxy. Record the class and log line. Offline with retries is expected there; report it anyway.
- **Fix:** none for the plain case. The proxy case would need a getaddrinfo probe.

#### I-W4 · P3 · handled (high) — Certificate errors
- **Why:**
  - `BrowserEngine.swift:444` — there is no authentication-challenge handler (a grep for serverTrust / URLAuthenticationChallenge finds none), so default trust evaluation fails the load, and WKWebView has no interstitial.
  - `browser_load.rs:135` — codes -1206…-1200 are Certificate: no retry, and the panel says 网站证书有问题，Vela 已阻止打开。.
  - `rust/crates/vela-core/src/app/dapp_browser.rs:672` — in a fresh tab the pill shows the open amber lock (`AddressBarView.swift:114`); in a used tab, the previous site's closed lock (I-H8).
- **Check:**
  1. 标签页 ▸ 新建标签页 ▸ https://expired.badssl.com/.
     - GOOD: 无法加载此页面 / 网站证书有问题，Vela 已阻止打开。 / expired.badssl.com; no 正在重试…; no way to proceed; the pill shows expired.badssl.com with the open amber lock; console `NSURLErrorDomain -12xx → certificate` (-1202 likely).
     - BAD: the page renders, any proceed control, or a closed lock.
  2. Repeat in fresh tabs for self-signed., wrong.host. and untrusted-root.badssl.com.
  3. In a tab showing app.uniswap.org, go to expired.badssl.com and note the pill. Report it under I-H8.
- **Fix:** none for the blocking. The lock goes with I-H8: browserSecure is false while `engine.failure != nil` (9e0fe081).

#### I-W5 · P3 · handled (high) — Renderer crash
- **Why:**
  - `BrowserEngine.swift:595` — webViewWebContentProcessDidTerminate calls onRendererGone.
  - `BrowserController.swift:548` — sends renderer_gone.
  - `dapp_browser.rs:566` — the core retires the document without delivering anything (`deliver: false`).
  - `dapp_browser.rs:926` — the consent sheet closes, and a signing job gets CancelSigning.
  - `ExploreScreen.swift:541` — BrowserCrashedView: 此页面已停止运行 / 重新加载.
  - The connected dot may remain, because the site's grant is still valid (`dapp_browser.rs:664-671`).
- **Check.** Easiest on the simulator:
  1. Open the test dApp at http://127.0.0.1:8000/ and tap Connect, leaving the consent sheet up (or approve, tap Sign and leave the signing sheet up).
  2. On the Mac, `ps -axo pid,command | grep -i 'CoreSimulator.*com.apple.WebKit.WebContent'`, then `kill -9 <pid>`. Never pkill by name: that also kills the Mac's own Safari tabs.
  - On a device instead, run this in Web Inspector until jetsam kills the page: `const a=[];setInterval(()=>{for(let i=0;i<40;i++)a.push(new Float64Array(1e6))},50)`.
  - GOOD: 此页面已停止运行 with 重新加载; the app stays up; the consent or signing sheet closes by itself; 重新加载 gives a working page where Connect works.
  - BAD: a white page, a sheet left up for a page that no longer exists, or the app relaunching.

#### I-D1 · P3 · handled (high) — Accidental dismiss
- **Why:**
  - `ExploreScreen.swift:371` — the live signing sheet's binding setter does nothing, and `.interactiveDismissDisabled()` is set (line 394). Its comment: "never a swipe (owner ruling)".
  - `ExploreScreen.swift:355` — consent: `.interactiveDismissDisabled(consentOpen)`. Its ✕ goes through onDismiss to consentRejected (lines 337-340), and 拒绝 calls consentRejected directly (line 843). Each gives one 4001.
  - `RootView.swift:1524` — the signing ✕ runs closeSigningSheet, which calls swipeDismissed().
  - The swipeable sheet at `ExploreScreen.swift:357` belongs to the gallery only.
  - An iPhone has no system Back and no Esc.
- **Check:**
  1. Test dApp: Connect, approve, then Sign.
  2. Swipe down hard ×3 and pull from the top edge. GOOD: the sheet stays and there is no answer.
  3. Tap ✕. GOOD: exactly one 4001 'User rejected'.
  4. Revoke from ⋯, tap Connect, and swipe on the consent: nothing happens. Then 拒绝 (or ✕) gives one 4001.
  5. With the signing sheet up, background the app and come back. GOOD: still up, unanswered.

#### I-W14 · P3 · handled (high) — Connect consent
- **Why:**
  - `Components/Explore/ConnectionPanelView.swift:81` — the account row: identicon, name, short address, switch.
  - `ConnectionPanelView.swift:108` — the network row.
  - `ExploreLive.swift:293` — account = consent.address (the core's active_address), chain = consent.chainId, title 连接到 {host}.
  - `ExploreScreen.swift:434` — after the account switcher closes, the consent asks again with the new account.
- **Check:** tap Connect.
  - GOOD: 连接到 <mac-lan-ip>:8000, an account row (0x88cC…6894), and a network row with a logo.
  - BAD: only the site.
  - Then tap the account row and pick another account. The consent returns naming it, and approving gives that address.

#### I-W15 · P3 · handled (high) — Back per tab
- **Why:**
  - `BrowserController.swift:527` — one BrowserEngine, and so one WKWebView, per tab.
  - `Components/Explore/BrowserToolbarView.swift:35` — Back and Forward follow canBack/canForward (`.disabled` at line 103).
  - `ExploreScreen.swift:172` — `canBack = engine.canGoBack`.
  - `BrowserEngine.swift:159` — the edge gestures act on the tab's own list.
- **Check:**
  1. In a new tab, open example.com.
     - GOOD: Back is dimmed, and an edge swipe does nothing.
     - BAD: Back is enabled in the new tab.
  2. In the test dApp tab, tap the _blank link, then Back.
     - GOOD: the test dApp is back, and Connect still answers.
     - BAD: Back does not return to the test dApp, or Connect no longer answers.
  3. With two tabs, use Back in tab 2.
     - GOOD: it never shows tab 1's pages.
     - BAD: tab 1's pages appear in tab 2.
  4. Watch the toolbar and address bar while pages load.
     - GOOD: nothing moves.
     - BAD: the toolbar jumps.

#### I-W19 · P3 · handled (medium) — Phone-held key in dApp signing uses Apple's sheet **[owner]**
- **Why:**
  - `App/RootView.swift:313` — the spine's signer is a fresh `PasskeyExecutor()` with hybrid/smartCard nil, for dApp transactions AND messages (`SignExecutor.swift:224` spine.submit, `:277` spine.signMessage).
  - `PasskeyExecutor.swift:316` — `.hybrid` with nil hybrid goes to the system ASAuthorization request, and the OS draws the QR (line 356).
  - `OnboardingModel.swift:203` — Vela's own HybridCeremony is wired only into onboarding.
  - So the Windows symptom cannot happen. The adjacent gap is §10 I-7.
- **Check** (owner; not in the parallel space). You need an account whose key lives ONLY on a phone that does not share this iPhone's Apple ID / iCloud Keychain, signed in through 手机或平板 (this needs I-W20's core).
  1. Test dApp: Connect, Sign, slide.
  - GOOD:
    - within about 1 s the iOS system passkey sheet appears over the signing sheet and offers "iPhone, iPad or Android device";
    - scanning with the other phone signs, and Verify sign reports valid;
    - no `[vela-cable]` lines appear.
  - BAD: 签名中/等待 with no QR, then an error.
  - Optional: an Android phone without Google Play. Expected unreachable (I-7).

#### iOS — not applicable
- **I-W1** — WKWebView is part of the OS:
  - `BrowserEngine.swift:141` uses the platform-owned `.default()` data store, with a shared WKProcessPool (line 43).
  - `Features/Settings/DeviceStorage.swift:149` clears that platform store; there is no app folder to get wrong.
  - The only engine death is W5.
  - Optional check on an older iPhone: open 6+ heavy dApp tabs and switch between them. Each tab shows its page or Vela's crash panel, never a white tab.
- **I-U1d** — the '…但仍会扣除 gas' clause is used only by a design fixture (`Features/Signing/SigningFixtures.swift:453`), and the live sheet has no would-fail sentence (`SigningLive.swift:1067`). When I-U1b is ported, use only the first clause and update the fixture.
- **I-D3** — `Core/CoreHTTP.swift:47` uses one URLSession that follows the system/VPN proxy on each request, and `RpcPool.swift:88` keeps failure state per chain and per endpoint only. There is no app-wide route switch.
- **I-W12** — the WKWebView is a UIViewRepresentable (`Components/Explore/BrowserWebView.swift:18`); the panels are a ZStack overlay (`ExploreScreen.swift:533`) and the menus are SwiftUI sheets. Quick look: tap ⋯ and the account chip; the page stays visible, dimmed.
---

## 9. Web (hosted wallet + Chrome MV3 extension)

Paths in this section are relative to `app-web/vela-wallet/`, unless they start with `rust/`, `app-desktop/`, `.github/` or `app-android/`.

### 9.1 Setup

**What the web client is**
- The web wallet has **no in-app browser**: 探索 is filtered out (`src/lib/wallet/destinations.ts:15`).
- WalletPair and remote-inject were left out on purpose (`specs/027-web-extension-provider/spec.md:42`), and scanning a `wc:` code is refused.
- dApps reach the web wallet only through the Chrome MV3 extension, built from the same code.
- A request made from a CLICK opens the asking tab's **side panel**. `panel.js` does `location.replace` to `<locale>/wallet.html?panel`, so the wallet is the panel's own document, and it mounts SigningHost with the landing.
- A request with NO user gesture opens a popup **window** instead (`routes/[locale]/request/+page.svelte`). `background.js` `settle()` closes that window the moment the answer goes out, so the window never shows a landing.
- **So run every check from a click.** A request typed into DevTools has no gesture: it lands in the popup window, a separate document your shim does not reach. To send custom params from a click, rebind a test-page button from the TAB console, then click that button. For example:
  ```
  document.getElementById('send').onclick=()=>__ask('eth_sendTransaction',[{from:me(),to:'0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141',value:'0x0',data:'0x'+'ab'.repeat(1200)}])
  ```
- `src/lib/services/dapp-submit.ts` `handleReadOnlyRPC` has **no caller** on the web. The "atomic supported" capabilities, wallet_getCallsStatus and the op-hash → receipt translation in it are all dead code. The extension routes reads through `extension/lib/protocol.js` `classifyMethod`; anything outside it gets 4200.

**0) Two builds: the committed core, then the rebuilt core** (§2.2)
- The committed wasm predates 083. So a web or extension build from this branch carries NONE of 083's core fixes, even though the generated TS types were updated.
- Where an item says so, run each check twice, in this order. `build:wasm` overwrites the tracked `rust/pkg-web/*` and `assets/wasm/vela_core_bg.<hash>.wasm`, so no committed-core build can be made after it.
  - (a) **Committed core:** `cd app-web/vela-wallet && pnpm install && pnpm sync:wasm && pnpm build:extension && cp -R extension/dist /tmp/vela-ext-committed`. Load `/tmp/vela-ext-committed` unpacked, and run the committed passes.
  - (b) **Rebuilt core:** from the repo root `npm --prefix scripts run build:wasm && node rust/scripts/build-web.mjs --check` (it must print "rust/pkg-web is current"; `npm --prefix scripts run verify:wasm` also checks). Then `cd app-web/vela-wallet && pnpm sync:wasm && pnpm build:extension`. Remove the (a) copy on chrome://extensions and load `extension/dist` (the manifest key gives it the same id), press ↻, reopen the side panel, and rerun.
  - (c) Afterwards, `git checkout -- rust/pkg-web assets/wasm` from the repo root, unless a fix needs them (§3).
- The rebuild needs Rust with target `wasm32-unknown-unknown`, wasm-pack 0.15.0 and wasm-opt. Do not commit its output unless you mean to (§3).
- `pnpm build:extension` (`extension/build.mjs`) runs `vite build` directly and does **not** run sync-wasm. The `static/` copy of the wasm is gitignored.
  - So ALWAYS run `pnpm sync:wasm` right before `pnpm build:extension`.
  - Then check that the file under `extension/dist/vela_core_bg.*.wasm` has the same name as `WASM_URL` in `rust/pkg-web/vela_core_wasm_url.js`.
  - The Windows checkout showed exactly this mismatch: `static/` held 97da5febae31 while `WASM_URL` named 55d58f879149.

**Build and run**
- Node 22 (CI uses 22), and `corepack enable` so that pnpm is the pinned 10.11.1 (`package.json` `packageManager`).
- `cd app-web/vela-wallet && pnpm install && pnpm dev` serves http://localhost:5173/zh. It runs sync-wasm itself, and the dev build installs the `window.vela` console.
- Prod-like: `pnpm build && pnpm preview` (wrangler on :4173). There, set `localStorage['vela.dev.console']='1'` to get the console.
- On an Android phone: `adb reverse tcp:5173 tcp:5173`, then Chrome at http://localhost:5173/zh, inspected through chrome://inspect.
- On an iPhone, two options:
  - Safari Web Inspector against https://getvela.app. That runs `main`, without 083's web changes.
  - `pnpm dev --host` over the LAN. That is not a secure context, so no passkeys: use it for copy checks only.
- Gates, in order: `pnpm check && pnpm lint && pnpm test:unit -- --run && pnpm build && pnpm test:e2e`. `pnpm check` fails while `static/` is stale.

**Extension**
- Build: `pnpm sync:wasm && pnpm build:extension` → `extension/dist`.
- Install in desktop Chrome ≥116: chrome://extensions → Developer mode → Load unpacked → `extension/dist`. The manifest key pins the id: `bjbdmnmpgcfkocfcfdocopkioacojkhl` (confirm it on chrome://extensions).
  - Press ↻ on the Vela card after every rebuild, and reopen the side panel.
- **Opening the side panel:** use Chrome's own side-panel button in the toolbar, or click a test-page button (a click opens the asking tab's panel). The Vela toolbar icon opens the wallet in a TAB (`extension/background.js:106`), not the panel. To paste the shim, click Send dust, paste it in the panel console while the sheet waits, then close that sheet with ✕ (4001) and start the check.
- Use a Chrome profile with NO other wallet extension: the provider sets `window.ethereum` only if it is absent.
- For fault runs, start a separate profile with the proxy: on macOS `open -na 'Google Chrome' --args --user-data-dir=/tmp/vela-chrome --proxy-server=http://127.0.0.1:8899`; elsewhere `chrome --user-data-dir=<scratch>/vela-chrome --proxy-server=http://127.0.0.1:8899`.
  - Branded Chrome ≥137 ignores `--load-extension`, so load the extension unpacked by hand in that profile.
  - Chrome bypasses the proxy for localhost.
- Consoles:
  - the service worker: chrome://extensions → Vela → "service worker";
  - the side panel: right-click → Inspect;
  - the request window: right-click → Inspect.
- State: run `chrome.storage.local.get(null)` from any extension page. Keys:
  - `vela.req.<tabId>:<uuid>:<n>`: pending requests;
  - `vela.perm.<origin>`: grants;
  - `vela.chain.<origin>`: the site's chain;
  - `vela.ext.chains`: the endpoint catalog;
  - `vela.ext.cache`: the wallet snapshot the worker answers granted origins from;
  - `vela.ext.surface`: `'panel'` (default) or `'window'`. The e2e harness drives window mode only, so every panel behaviour needs a real Chrome run.

**Test account** (parallel space; no biometrics)
1. In an extension page's console:
   ```
   localStorage.setItem('vela.dev.console','1'); localStorage.setItem('vela.intro.seen', String(Date.now()))
   ```
2. Open `chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl/zh/parallel.html` (or `/zh/parallel` on pnpm dev) and press "Enter (seed fixture wallet)". This is the same flow as `e2e/extension-signing.e2e.ts:46-63`.
3. Switch the active account to MultiTest `0x88cCA0EeDbF2C4426110bbFc998F048689266894` BEFORE connecting, because the grant pins the active account. Enter selects the first single-key fixture (`src/lib/dev/parallel-space.ts:203`), so: open `chrome-extension://bjbdmnmpgcfkocfcfdocopkioacojkhl/zh/wallet.html`, pick "Parallel Multi" (0x88cC…6894, fixture accounts[3]) in the account switcher, and confirm with `chrome.storage.local.get('vela.ext.cache')` that the snapshot's active account is 0x88cC…6894.
- On the web wallet itself: `vela.parallel.enter()`, then reload. `vela.parallel.exit()` leaves.
- Items that need a real WebAuthn prompt use the owner's own wallet: W11, H4a, H4b, D1b, W19. The parallel override returns before `assertSupported` / `navigator.credentials`.
- **The owner's wallet works only inside the extension.** Its rpId is `getvela.app` only on the extension's pages (`src/lib/onboarding/core/passkey.ts:62-99`). On localhost or a LAN IP the rpId is that host, so a passkey ceremony there creates a DIFFERENT, empty wallet.
  - In the extension, open `zh/parallel.html` and press "Leave (restore real wallet)". If no wallet comes back, 登录 with the owner's passkey.
  - The browser's passkey dialog must name getvela.app. If it names anything else, stop.
  - Never create a wallet or sign in on localhost:5173 or a LAN IP. W-H1 only reads the 登录 sheet there: back out without choosing a method.

**Test dApps**
- Serve both pages from the repo root, on the ports the checks below use. First stop the §7.1 servers, which put the Android page on BOTH ports: `pkill -f 'http.server 813'`. Then (on Windows use `python`, or run each line in its own terminal):
  ```
  python3 -m http.server 8138 --bind 127.0.0.1 --directory app-android/vela-wallet/dev/testdapp & python3 -m http.server 8137 --bind 127.0.0.1 --directory app-web/vela-wallet/e2e/testdapp &
  ```
  Verify: `curl -s http://localhost:8137/ | grep -c 'Send dust'` prints 0 (the web e2e page), and the same for 8138 prints 1 (the Android page).
- http://localhost:8138 is the Android test page (§7.1 "What the page offers" lists its buttons, `__ask`, `me()` and `#out`). It is the page every "test page" check below means.
- http://localhost:8137 is the web e2e page: only Connect, Sign (personal_sign) and the blocked-call buttons.
- Always open them as `localhost`, never `127.0.0.1`: the two are different origins with different grants, and the site name shown is the origin you opened.
- `#out` on both pages keeps only the last answer per method until the page reloads. Reload before a check whose answer you read there; for "exactly once", use the "What the page got" wrapper below.
- The real dApp: https://app.uniswap.org on Base.

**What the page got** (paste in the dApp tab's DevTools):
```
(p=>{const r=p.request.bind(p);p.request=async a=>{try{const v=await r(a);console.log('[page<-vela]',a.method,v);return v}catch(e){console.log('[page<-vela ERR]',a.method,e.code,e.message);throw e}}})(window.ethereum)
```

**Wallet logs** (side panel console):
- `[UserOp] Receipt landed` (the op hash is truncated)
- `[UserOp] Relay REJECTED`
- `[UserOp] Previous op pending (0x…full…)`
- `[FeeEstimate] Bundler estimation unavailable:`
- `[sign_request]`

**Dev faults**
- In the side-panel console run `localStorage.setItem('vela.dev.console','1')`, then reload the panel.
- That provides `vela.silentReceipt(chainId)`, `vela.rejectSubmit(chainId)`, `vela.failRelay(chainId)` and `vela.clearFaults()` (`src/lib/services/fault-injection.ts`, `rpc-adapter.ts:54-72`).

**The deterministic relay/receipt shim**
- Paste it in the SIDE PANEL console.
- All wallet RPC goes through the global `fetch` (`src/lib/wallet/core/rpc-pool-executor.ts:96`), so the shim sees everything.
- A panel reload loses it.
- Set `__vf.mode` BEFORE sliding: a resolved receipt is cached per op for the life of the document.
- `__vf.sent` collects every op hash the relay accepted, in full.
```
(()=>{const EF='0x23428b18acfb3ea64b08dc0c1d296ea9c09702c09083ca5272e64d115b687d23',EFlog={address:'0x0000000000000000000000000000000000000001',topics:[EF],data:'0x'+'0'.repeat(128)};window.__vf={mode:null,other:null,sent:[]};const f=window.fetch.bind(window),J=o=>new Response(JSON.stringify(o),{headers:{'content-type':'application/json'}});window.fetch=async(u,i)=>{let q;try{q=JSON.parse(i?.body??'null')}catch{}const m=__vf.mode,me=q?.method;if(m==='pending'&&me==='eth_sendUserOperation')return J({jsonrpc:'2.0',id:q.id,error:{code:-32602,message:'previous op pending [existingHash:'+__vf.other+']'}});if(m==='refuse'&&me==='eth_estimateUserOperationGas')return J({jsonrpc:'2.0',id:q.id,error:{code:-32500,message:'UserOperation simulation failed'}});const r=await f(u,i);if(me==='eth_sendUserOperation')r.clone().json().then(j=>{if(j&&j.result){__vf.sent.push(j.result);console.log('[vf] op sent',j.result)}}).catch(()=>{});if(me==='eth_getUserOperationReceipt'&&m){const j=await r.clone().json(),R=j.result;if(R&&R.receipt){if(m==='revert')R.success=false;const L=R.receipt.logs=R.receipt.logs||[];if(m==='neighbour')L.unshift(EFlog);if(m==='inner'){const k=L.findIndex(l=>(l.topics||[])[0]?.startsWith('0x49628fd1'));L.splice(k<0?L.length:k,0,EFlog)}return J(j)}}return r}})()
```
- **Modes:**
  - `revert` — `success:false`.
  - `neighbour` — an ExecutionFailure at the head of the logs, outside our op.
  - `inner` — an ExecutionFailure just before our UserOperationEvent.
  - `pending` — set `__vf.other` to an earlier op hash of the same account first.
  - `refuse` — the relay answers "simulation failed".
- The receipt modes change only what THIS panel sees, so the local record may end up failed for a send that really landed.
- **About `refuse`:** the core falls back to a static gas model for any op whose MultiSend calldata is 1024 bytes or less (`rust/crates/vela-core/src/app/fee_policy.rs:119, 2868-2881`). A plain Send dust under `refuse` therefore still shows a priced fee. Use a call with more than 1024 bytes of calldata (the rebind above) or the real Uniswap swap.

**Fault proxy** for the extension's own reads:
1. `python3 scripts/device/chaos-proxy.py chaos.log` (`python` on Windows), in the background or its own terminal. Set `CHAOS_UPSTREAM=host:port` only if this machine needs a proxy.
2. Run the scratch Chrome profile with `--proxy-server` (above).
3. Aim a fault: `curl "http://127.0.0.1:8899/__chaos?mode=blackhole&match=mainnet\.base\.org"`. Modes: pass, latency, throttle, drop, blackhole, reset_mid.

### 9.2 Checklist (priority order)

#### W-S3b · P0 · likely (high) — Another operation's result answered
- **Windows:** see A-S3b.
- **Why here:**
  - `src/lib/services/safe-transaction.ts:1800` — a submit error carrying `[existingHash:…]` returns `{ userOpHash: existingHash, waitForTxHash: () => waitForReceipt(existingHash, chainId, 60_000) }`, with no comparison to this op's own hash. The same happens at line 2133 (the in-band path) and line 2407.
  - `safe-transaction.ts:3340` — `submitUserOp` throws `new Error(parseBundlerError(response.error))`, so the catch only ever sees the REWORDED message.
  - `safe-transaction.ts:3656` — `parseBundlerError` rewrites any AA25 / 'invalid account nonce' message (and anything with 'reverted' or 'simulation failed') to a fixed sentence, which drops the marker.
  - `rust/crates/vela-core/src/user_op.rs:1126` — the relay's real wording: 'AA25 invalid account nonce [existingHash:…]'.
  - `src/lib/services/dapp-submit.ts:491` — `onSubmitted(txResult.userOpHash)` sends op_submitted with the OTHER op's hash, and the page is answered with that op's result (line 493).
  - `dapp-submit.ts:764` — wallet_sendCalls' single-call path returns `txResult.userOpHash`, which can be the other op's.
  - `safe-transaction.ts:2930` — the nonce cache is per document, with a 10 s TTL. The side panel and a popup window are separate documents.
  - Scope:
    - For wordings that escape parseBundlerError (e.g. 'previous op pending [existingHash:…]'), the page gets ANOTHER op's result: its tx hash within 60 s, otherwise its op hash. This request's op is never sent.
    - For the AA25 wording, the page gets -32603 'Transaction nonce mismatch. Please try again.' (`parseBundlerError`, passed on verbatim by `sign-types.ts:135-137`).
    - The realistic trigger is Uniswap's approve followed by its swap, with a lagging node.
- **Check** (fetch shim; wrapper in the tab):
  0. Make parallel MultiTest the active account BEFORE connecting (§9.1 "Test account"). Then put the page on Gnosis (§3 Money): test page (http://localhost:8138) → Connect → Switch to Gnosis → Chain → `0x64`. The Send dust sheet must name Gnosis and xDAI.
  1. Open the side panel (§9.1 "Extension": Chrome's side-panel button, not the Vela icon), paste the shim in its console, and set `__vf.mode=null`.
  2. Click Send dust and slide (real: 0.001 xDAI plus the fee). Wait for 已确认. The op hash is `__vf.sent.at(-1)`, and its receipt is now cached in this panel.
  3. `__vf.other=__vf.sent.at(-1); __vf.mode='pending'`, then click Send dust again and slide (the shim fakes the relay's answer, so nothing is sent).
     - BAD: the panel logs `[UserOp] Previous op pending (0x…), polling for receipt...`, and within seconds the tab gets `[page<-vela] eth_sendTransaction 0x<the FIRST send's tx hash>`. The landing re-shows the old op; after the rebuild, a new 活动 row tracks it.
     - GOOD: an error such as 'Another transaction from this account was still pending, so this one was not sent…', or a wait and then this op sent. Never another op's hash.
  4. Variant: change the shim's message to `'AA25 invalid account nonce [existingHash:'+__vf.other+']'`.
     - Today the tab gets `-32603 Transaction nonce mismatch. Please try again.`
     - GOOD: the same wait-and-resend, or a clear error, as in step 3.
  - Real world: a Uniswap swap on Base right after its approve lands. Grep the panel for 'Previous op pending', and compare the swap's answer with the approve's tx hash.
- **Fix:** mirror ddb52da8 (core `user_op.rs` `existing_op`, desktop `relay.rs` `SubmitError::Occupied`).
  1. In the three `parseExistingUserOpHash` catch blocks (`safe-transaction.ts` 1797, 2133, 2407), read the marker from the RAW relay error: have `submitUserOp` throw a typed error that carries `response.error.message`.
  2. Compute this op's own EntryPoint v0.7 hash: export the core's `user_op_hash` / `existing_op` through `rust/crates/vela-core-wasm`, or eth_call `EntryPoint.getUserOpHash`.
  3. Decide ThisOne vs Another the core's way. Another → wait for it, resend this op as signed, otherwise fail clearly.
  4. Floor the next nonce at the last submitted-but-not-landed nonce + 1, per account and across documents.

#### W-S2 · P1 · partially (high) — A revert: wrong words, and an in-op failure answered as success
- **Windows:** see A-S2.
- **Correction to 083's own notes:** results.md and ddb52da8 say the web maps success:false to Succeeded. That is not true on the web's dApp path (§10 W-9).
- **Why here:**
  - `safe-transaction.ts:3426` — `result.failed` (success===false) throws 'Transaction was dropped from the network. Try again with a higher gas price.' It is rethrown at line 3444.
  - `src/lib/services/tx-reconciler.ts:162` — confirmed = `success !== false`. The logs are carried but never judged on this path.
  - `dapp-submit.ts:518` — `receiptStillOutstanding` treats /dropped from the network/ as a verdict, so it is rethrown, not ReceiptPending.
  - `src/lib/signing/core/sign-executor.ts:274` — the catch goes to classifySubmit, which returns `{type:'failed', message}` (line 383). The shell never emits 'reverted'.
  - `src/lib/signing/core/sign-types.ts:135` — submit_failed: the page's message is the detail, verbatim.
  - `rust/crates/vela-core/src/app/sign_request.rs:2499` — one -32603, and the record closes failed.
  - `src/lib/wallet/core/tracker-executor.ts:181` — only the tracker judges a Safe ExecutionFailure under success:true.
  - `rust/pkg-web/build-info.json:3` — the shipped wasm has no Reverted.
  - What is still wrong:
    1. The words: "dropped" and "try a higher gas price" invite a second, paid revert.
    2. An in-op ExecutionFailure under success:true is not checked on the answer path.
    3. The core's `reverted` outcome and `REVERTED_MESSAGE` go unused.
- **Check** (side panel; parallel MultiTest; test page, Switch to Gnosis, Connect; wrapper and shim):
  - (a) `__vf.mode='revert'`, then click Send dust. Real funds: 0.001 xDAI plus the fee, and the op lands.
    - BAD (today): `[page<-vela ERR] eth_sendTransaction -32603 Transaction was dropped from the network. Try again with a higher gas price.`
    - GOOD: `-32603 The transaction was included but reverted (0x<txhash>)`.
  - (b) Reload the panel, re-paste the shim, set `__vf.mode='inner'`, and send dust again.
    - BAD: `[page<-vela] eth_sendTransaction 0x<txhash>` (a success) while the landing turns 失败. After the rebuild, the 活动 row turns 失败 too.
    - GOOD: the page gets an error, and the landing and the record agree.
- **Fix:** mirror ddb52da8 and 2448738a.
  1. Rebuild the wasm first: the shipped core cannot deserialize `reverted`.
  2. In `safe-transaction.ts` waitForReceipt, `dapp-submit.ts` and `sign-executor.ts` classifySubmit, return `{type:'reverted', user_op_hash, tx_hash}` (already in the generated `SignSubmitOutcome.ts`).
  3. Judge "executed" as success AND no ExecutionFailure in the op's OWN logs, in one of two ways:
     - (a) export `tx_tracker::user_op_outcome_in_logs` / `op_execution_failed` through `rust/crates/vela-core-wasm`; or
     - (b) have the dApp wait follow the tracker entry, which applies the rule after the rebuild (as desktop `executor/landing.rs` does).

#### W-S3 · P1 · likely (high) — The userOpHash answered after 120 s
- **Windows:** see A-S3.
- **Why here:**
  - `safe-transaction.ts:3394` — waitForReceipt times out at 120_000, and then throws "…not confirmed within 120s" (line 3527).
  - `dapp-submit.ts:495` — that becomes `DAppReceiptPendingError(txResult.userOpHash)`.
  - `sign-executor.ts:267` — which becomes `{type:'receipt_pending', user_op_hash}`.
  - `sign_request.rs:2402` — which the core answers as Ok(user_op_hash).
  - `extension/lib/protocol.js:130` — eth_getTransactionReceipt is proxied verbatim to the node, so for an op hash it returns null forever.
  - `dapp-submit.ts:987` — the op→tx translation lives in the dead handleReadOnlyRPC.
  - `extension/content.js:50` — every page request has a 300 s deadline.
- **Check** (side panel with the dev console; parallel MultiTest on Gnosis; wrapper in the tab):
  1. In the panel console, `vela.silentReceipt(100)`.
  2. Click Send dust and slide. Real funds: the op lands.
  - BAD: at about 120 s the tab logs `[page<-vela] eth_sendTransaction 0x<66-hex>`, equal to the 操作哈希 the landing showed. And `await ethereum.request({method:'eth_getTransactionReceipt',params:['<hash>']})` returns null, even after `vela.clearFaults()`.
  - GOOD: nothing is answered at 120 s, and the landing says it is still confirming. After 10 min the page gets '…submitted but is not confirmed yet…(user operation 0x…)'. Never the op hash. This is reachable only after the shell port AND a longer content.js deadline. Until then a correct 10-min wait ends at 300 s with 4900 'Vela did not answer in time'. After a wasm rebuild alone, expect the same BAD.
  3. Run `vela.clearFaults()`. The landing, if still open, converges to 已确认; after the rebuild, so does the 活动 row.
- **Fix:** mirror ddb52da8.
  1. Rebuild the wasm first: the shipped core has no NotConfirmed.
  2. In `dapp-submit.ts` handleSendTransaction / waitForReceipt, wait up to `PAGE_WAIT_CAP_MS` (10 min, at the tracker's cadence). Then return `{type:'not_confirmed', user_op_hash}` from sign-executor, instead of receipt_pending.
  3. Extension: content.js's 300 s deadline would cut a 10-min wait, and an MV3 worker that holds one message for more than 5 min can be torn down. Raise the deadline and keep the request alive: a port or a keepalive from the panel, or answer from the worker's stored record.

#### W-R2 · P1 · partially (high) — A bundle neighbour's failure fails our op (stale core)
- **Windows:** see A-R2.
- **Why here:**
  - `src/lib/wallet/core/tracker-executor.ts:181` — the shell hands the core the WHOLE bundle's logs as receipt_with_logs. That is exactly what the fixed core needs.
  - `rust/crates/vela-core/src/app/tx_tracker.rs:1057` — `op_execution_failed` scopes the check to our op (2448738a).
  - `rust/pkg-web/build-info.json:3` — the committed source is 55d58f87… (42acc794). The current fingerprint is 3b09e97a…, so the web ships the pre-083 tracker, which at 42acc794 ran `safe_execution_failed` over every log.
  - `.github/workflows/ci.yml:281` — the CI gate exists.
  - No web shell code does its own ExecutionFailure check.
- **Check** (panel; MultiTest on Gnosis; shim; wrapper):
  1. `__vf.mode='neighbour'`, then Send dust (real).
  - BAD on the committed build: the tab gets `[page<-vela] eth_sendTransaction 0x<txhash>` (a success) while the landing turns 失败.
  - GOOD after the rebuild: the landing reads 已确认, and the 活动 row stays 已确认. To rebuild: `npm --prefix scripts run build:wasm`, `pnpm build`, `pnpm sync:wasm && pnpm build:extension`, then Reload the extension.
  - Control: `__vf.mode='inner'` must still give 失败 after the rebuild.
- **Fix:** rebuild and commit the web core (`rust/pkg-web` + `assets/wasm`) until `node rust/scripts/build-web.mjs --check` prints "current". No shell change is needed.

#### W-U1 · P1 · likely (high) — Max USDC → ETH: the fee coin is drained, and the sheet has no simulation
- **Windows:** see A-U1.
- **Why here:**
  - `src/lib/signing/core/sign-resident.svelte.ts:116` — `setApproveSim` exists, but nothing calls it.
  - `simulateAssetChanges` is called only from `src/lib/flows/core/send-executor.ts:471`, never from the dApp sheet.
  - `src/lib/signing/SigningHost.svelte:361` — `requestQuote({ autoFeeToken: true, … })`.
  - `src/lib/core/generated/FeeEvent.ts` — it has `balance_changes_measured`, but nothing in `src/lib` outside `generated/` sends it.
  - `fee_policy.rs:2868` — only an op with more than 1024 bytes of calldata fails on a failed simulation. A Universal Router swap is over that.
  - The shipped core has no BalanceChangesMeasured, Refused or WouldFail.
  - A small op (1024 B or less) gets a static-gas price and an open slide, and then fails at submit.
- **Precondition:** the same as A-U1's. Hold USDC well above the USDC fee (aim for at least 0.2; top up with ETH→USDC 0.0001 first, §3), or ETH wins on the static fee alone (`fee_policy.rs:2258-2270`) and a GOOD proves nothing. Control: a 0.05 USDC→ETH sheet on the same balance (do not slide) must pick USDC.
- **Check** (panel; MultiTest; Uniswap on Base):
  1. Enter **Max** USDC → ETH and press Swap. If Uniswap asks for a Permit2 approve, that is a real op; the swap sheet itself spends nothing if you stop there.
  - BAD: the fee row shows 点击重试 under 无法连接 Vela 服务 — 请检查网络，稍后会自动重试。, the panel logs '[FeeEstimate] Bundler estimation unavailable: …' every 3–15 s, and the slide never opens.
  - GOOD (desktop): the fee is auto-picked in ETH (about 0.00003 ETH), and the slide opens within about 9 s.
  2. Close with ✕. The page gets 4001.
- **Fix:** mirror 380d9014 and 5825e443 (desktop `wallet/signing_host.rs`, whose `fee_balance_changes` is at lines 102/601, and `speed_control.rs`).
  1. Rebuild the wasm first.
  2. Give the web dApp sheet a simulation: run `src/lib/services/sim/tx-simulation.ts` `simulateAssetChanges` for the request.
  3. Draw its balances block. `BalanceChanges.svelte` is already wired into BlockList.
  4. Dispatch FeeEvent `balance_changes_measured` into the sheet's fee session (`src/lib/flows/core/fee-quote.svelte.ts`).
  5. Call `signRequest.setApproveSim`.

#### W-U1b · P1 · likely (high) — A relay refusal drawn as "check your network"
- **Windows:** see A-U1b.
- **Why here:**
  - `src/lib/flows/core/fee-executor.ts:229` — an estimate failure becomes `{type:'simulation_failed'}` (or context_unavailable), never 'refused'.
  - `safe-transaction.ts:1609` — runUserOpGasSimulation turns ANY estimateGas throw into simulation_failed, including the relay's answered "simulation failed". The text is only logged.
  - `fee_policy.rs:2868` — over 1024 B this becomes EstimateFailed; at 1024 B or less the static model prices it.
  - `fee_policy.rs:759` — requote at 3/6/12/15 s.
  - `src/lib/signing/live.ts:632` — reason(): any failure with a requote delay is drawn as `m.feeNetworkError`, which is 无法连接 Vela 服务 — 请检查网络，稍后会自动重试。 (`componentsUi.json:33`). The fee value is 点击重试 (line 275).
  - This is present on both the committed and the rebuilt core.
- **Check** (costs nothing if you close with ✕):
  1. Panel; MultiTest; test page on Gnosis (Connect, Switch); shim pasted.
  2. In the TAB console, the 1200-byte rebind:
     ```
     document.getElementById('send').onclick=()=>__ask('eth_sendTransaction',[{from:me(),to:'0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141',value:'0x0',data:'0x'+'ab'.repeat(1200)}])
     ```
  3. In the panel set `__vf.mode='refuse'`, then CLICK Send dust.
  - BAD: 点击重试 + 无法连接 Vela 服务 — 请检查网络，稍后会自动重试。, the spinner re-asking every 3–15 s, and '[FeeEstimate] Bundler estimation unavailable: …simulation failed…'.
  - GOOD: 这笔交易预计会失败 (first clause only), no automatic re-asks, and the coin list still choosable.
  - Control: the unmodified Send dust under refuse shows a priced fee. That is the expected static fallback.
- **Fix:** mirror 380d9014 and 5825e443.
  1. Rebuild the wasm first.
  2. `safe-transaction.ts` runUserOpGasSimulation: return a distinct kind when the relay ANSWERED with refusal text. Keep the raw message: estimateGas currently throws through parseBundlerError.
  3. `fee-executor.ts`: map it to `{type:'refused'}` using the §2.4 rule.
  4. `live.ts` feeModel: draw would_fail with the first clause of `componentsUi.signing.simWillFail`, and no network sentence.

#### W-U1c · P1 · likely (high) — Fee coin list unusable over a failed quote
- **Windows:** see A-U1c.
- **Why here:**
  - `src/lib/signing/SigningHost.svelte:562` — onfee: `if (feeShown.failed) fee.requote(); else if (options.length > 1) feeOpen = !feeOpen`.
  - `live.ts:650` — the failed branch returns the row without a `selector`. The list is built only on success (lines 681-698).
  - `live.ts:618` — the chevron is drawn whenever there are more than 1 options.
  - `fee_policy.rs:3465` — the core ignores a pick in Failed(EstimateFailed), so land this with U1b.
- **Check:** reach U1b's state (the 1200-byte rebind, refuse, click) or U1's (Uniswap Max). Tap the fee row, including its '>'. This costs nothing with ✕.
  - BAD: a spinner, another failure, and no list.
  - GOOD: the fee-coin list (xDAI/USDC on Gnosis, ETH/USDC on Base) opens over the failed quote. Picking the other coin re-quotes; on Base with the real swap, the slide opens.
- **Fix:** mirror 380d9014. In SigningHost onfee (lines 559-565) and live.ts feeModel (650-659, 681-698), build and open the selector whenever `fee.options.length > 1`, even when failed. Keep requote for a single coin.

#### W-H2 · P1 · partially (high) — dApp transactions are not in 活动 (stale core; intent never passed)
- **Windows:** see A-H2. The web shell side was done in 083.
- **Why here:**
  - `src/lib/wallet/core/feed-executor.ts:97` — dapp_tx records hand the core `dapp_url` and `intent` (ff660c2d).
  - `src/lib/wallet/live.ts:438` — the rows draw `item.dapp.site` and dappTitle.
  - `rust/pkg-web/build-info.json:3` — the committed core predates 43fd67a4/ff660c2d. At 42acc794 the feed matched only Send and Receive, and a DappTx fell to `_ => None` (`activity_feed.rs:814` there). Unknown keys are ignored, so nothing breaks; the rows are simply dropped.
  - `SigningHost.svelte:475` — approveOpts sends `intent: null`. The core falls back only to `plain_send_intent` ("Send" for a no-calldata eth_sendTransaction; `sign_request.rs:841`).
  - The desktop's rule is in `app-desktop/vela-wallet/src/wallet/signing_host.rs:1383` (recorded_intent).
  - So:
    - today there are no dApp rows at all;
    - after the rebuild, dust sends read 发送 and every decoded call reads 合约交互;
    - rows written before the rebuild name no site.
- **Check** (panel; MultiTest; test page on Gnosis):
  1. Click Send dust (real), then open the extension wallet's 活动.
     - BAD (committed build): no row.
     - GOOD (after the rebuild, `pnpm build`, `pnpm sync:wasm && pnpm build:extension` and a Reload): a row 发送 with subtitle localhost:8138 (your origin), pending and then 已确认; the detail shows 应用 = the site and the real tx hash.
  2. Click Approve 2^254, cap the amount on the sheet (e.g. to 1), and slide (fee only). The sheet reads it as Unlimited, and §3 forbids sliding an unlimited approve as asked. With the intent passed it should read 批准/Approve; today, after the rebuild, it reads 合约交互.
  3. Do this platform's single Uniswap swap on Base (0.05 USDC → ETH, about $0.08; §3). It also serves W-F1, W-F3 and W-U8. The row should sit under app.uniswap.org.
- **Fix:**
  1. Rebuild and commit the web core.
  2. `SigningHost.svelte:475`: pass `signingSheet.clear.result?.intent` when `result.verified || !result.best_effort`, otherwise null (desktop recorded_intent, 43fd67a4).

#### W-W4 · P1 · likely (high) — The extension signs on insecure public origins and past certificate errors
- **Windows:** Edge's interstitial offered 高级 → continue, inside the wallet. The desktop now cancels, and shows Vela's panel with a warning lock (9e0fe081). Every in-app browser refuses signing on public http with 4100.
- **Why here:**
  - `extension/manifest.json:36` — inpage.js (MAIN world) and content.js (line 43) match `*://*/*`. Every public http:// page, and any page reached past Chrome's certificate interstitial, gets a working provider.
  - `rust/crates/vela-core/src/app/dapp_permissions.rs:650` — `decide_popup_request` checks connect, grant and pinned address only. It has no origin-security rule.
  - `src/lib/dapp/request.ts:55` — evaluate() → decidePopupRequest → forward_to_signing → handOffToSigning. sign_request has no secure-origin check either.
  - `rust/crates/vela-core/src/app/dapp_browser.rs:1192` — the in-app browsers answer a sign route on `is_insecure_public_origin` with 4100 "Signing requires a secure origin", and mark tabs secure or insecure (lines 675-677). The extension never reaches this code.
  - `extension/background.js:520` — route() never checks the scheme.
  - `src/lib/dapp/DappRequestHost.svelte:372` — the consent card has no lock and no insecure warning. Neither has `src/lib/signing`.
  - The core exempts loopback and private hosts, so the localhost test dApps keep working under a fix.
- **Check** (parallel space; a profile with no other wallet extension; no funds):
  - (a) Public http:
    1. Open http://neverssl.com; it redirects to an http subdomain.
    2. In its console run `await ethereum.request({method:'eth_requestAccounts'})`. This has no gesture, so a request WINDOW opens. Press 连接.
    3. Then run the line below and slide:
       ```
       await ethereum.request({method:'personal_sign', params:['0x68656c6c6f', (await ethereum.request({method:'eth_accounts'}))[0]]})
       ```
  - (b) Certificate bypass: open https://expired.badssl.com → 高级 → 继续前往 expired.badssl.com（不安全）, and repeat both calls.
  - BAD: 连接到 <host> with no warning, the sheet opens, and a 0x… signature returns.
  - GOOD: personal_sign answers 4100 "Signing requires a secure origin" on public http, and the consent marks the connection insecure.
  - Clean up: revoke both in the wallet's 连接 list, or `chrome.storage.local.remove('vela.perm.<origin>')`.
- **Fix:** this is new policy for the extension; ask the owner (§11.3).
  1. Refuse signing on insecure public origins, either by:
     - exporting `dapp_permissions::is_insecure_public_origin` to wasm (`rust/crates/vela-core-wasm`), and answering 4100 in `src/lib/dapp/request.ts` evaluate() for classify 'sign' before forward_to_signing; or
     - adding an origin argument and a new DpermRejectReason to `decide_popup_request` (then regenerate the wasm and the types).
  2. `DappRequestHost.svelte`: draw an insecure lock/warning on both consent branches, reusing the 079 U1 / 9e0fe081 wording.
  3. Certificate bypass: MV3 exposes neither a tab's security state nor "proceeded past the interstitial". At most, `chrome.webNavigation.onErrorOccurred` with `net::ERR_CERT_*` can taint the tab (this needs the "webNavigation" permission). Document the limit.

#### W-W5 · P1 · likely (medium) — A reloaded, crashed or closed dApp tab keeps its request and sheet
- **Windows:** Edge's crash page, while the chrome kept the green dot and the old title. The desktop now shows a crash panel with reload (9e0fe081), and the core has `page_gone` (4198ec6b).
- **Why here:**
  - `extension/background.js:263` — the comment says a navigation settles 4900, but only `chrome.tabs.onRemoved` is wired (line 277). There is no tabs.onUpdated or webNavigation listener, and no webNavigation permission. A reload, navigation or renderer crash leaves the pending entry and its `vela.req` key.
  - `background.js:235` — nextForPanel returns the OLDEST entry for the tab, so the dead request stays ahead of the reloaded page's new one.
  - `src/lib/dapp/DappRequestHost.svelte:124` — take() returns early while `owing`. So after settleTab removes the key on a tab close, the sheet for the vanished page stays up.
  - `DappRequestHost.svelte:216` — a transport is registered per request and never dropped (§10 W-12). `sign_request.rs:1206` TransportDropped exists even in the committed wasm, but the web never dispatches it.
  - `background.js:156` — settle() swallows a reply to a dead page, but the signature or op already exists.
  - Window mode also keeps its card on reload or crash.
  - Risk: signing for a page that no longer exists. A reloaded dApp may then ask again, and the person acts twice.
- **Check** (parallel space; personal_sign, so no funds):
  1. Tab A: open :8137, Connect, 连接.
  2. Tap Sign, and leave the sheet up.
  3. Then do one of: (a) press F5 in tab A; (b) type chrome://crash in tab A's omnibox; (c) close tab A.
  4. After each, look at the panel, and in the service-worker console run:
     ```
     chrome.storage.local.get(null).then(o=>Object.keys(o).filter(k=>k.startsWith('vela.req.')))
     ```
  5. After (a), click Connect or Sign on the reloaded page.
  6. Repeat (a) in window mode (`vela.ext.surface`='window').
  - BAD:
    - the sheet stays for the dead page, and sliding produces a signature nobody receives;
    - after (a) or (b), the `vela.req.<tabA>:…` key is still listed;
    - after (a), the reloaded page's request does not appear until the old sheet is answered;
    - in window mode, the old window stays after F5.
  - GOOD: within about 1 s the stale sheet or window closes (or says the site reloaded or closed), the key is gone, and the reloaded page's request shows.
  - Only once GOOD is expected, repeat with a transaction: Send dust on **Gnosis** (under $0.01).
- **Fix:** mirror 9e0fe081 and 4198ec6b.
  1. `background.js`: settle a tab's pending requests with CLOSED_WITHOUT_ANSWER on a top-frame navigation or a crash:
     - `chrome.webNavigation.onCommitted` with frameId 0 (add "webNavigation"), or tabs.onUpdated with status 'loading' and a changed URL/document;
     - for crashes, a content.js `runtime.connect` port whose onDisconnect settles.
  2. `DappRequestHost.svelte`: when the owed rid disappears from storage (subscribeRequests already fires), check it with requestDetail. If it is gone:
     - dispatch `{type:'transport_dropped', transport_id}` (this works with the current wasm; page_gone needs the rebuild);
     - clear request, stage and owing;
     - take() again, and unregister the transport.

#### W-R3 · P2 · partially (medium) — The revert landing: no tx hash, no explorer; the popup shows nothing
- **Windows:** see A-R3.
- **Why here:**
  - `src/lib/signing/dapp-receipt.ts:131` — the failed landing is `componentsTx.receipt.failedHint` plus the OP hash only: no tx hash, no explorer. landingFromEntry (line 168) drops `entry.tx_hash` for dropped/rejected.
  - `rust/crates/vela-core/i18n/locales/zh/componentsTx.json:31` — failedHint tells the person to press 「浏览器」, which is not drawn.
  - `live.ts:943` — signingStatus: a non-rejection error with a pending op shows 失败 + `m.status.failedHint`, which is send.txErrorGeneric 交易未能提交。您的资金安全无虞——请重试。 (`src/lib/i18n/engine.server.ts:967`). It is visible where no landing is raised, or after the landing is dismissed.
  - `src/routes/[locale]/request/+page.svelte:19` — the popup window gets no landing, and closes on the answer.
- **Check** (panel; MultiTest on Gnosis; shim):
  1. `__vf.mode='revert'`, then Send dust (real; it lands).
     - BAD: 失败 + 转账在链上被回滚…可点下方「浏览器」, with only a 操作哈希 row and no 浏览器 button. After Done, if the sheet remains: 交易未能提交。您的资金安全无虞——请重试。
     - GOOD (desktop 2448738a): the failure sentence, the TRANSACTION hash and an explorer link, at once.
  2. The popup path: in the TAB console run
     ```
     __ask('eth_sendTransaction',[{from:me(),to:'0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141',value:'0x38d7ea4c68000'}])
     ```
     This has no gesture, so a window opens. It is real: 0.001 xDAI plus the fee. The shim does not reach that window, so this only shows that the window vanishes on the answer, with no outcome drawn.
- **Fix:** mirror 2448738a.
  1. `dapp-receipt.ts`: give the failed state `entry.tx_hash`, and draw the tx hash plus an explorer button.
  2. `live.ts` signingStatus: never show txErrorGeneric once `pending_op_hash` is set; use the core's reverted wording.
  3. Consider keeping the popup window open on a failure.

#### W-U8 · P2 · likely (high) — Fee about 14× the on-chain cost (owner decision)
- **Why here:**
  - `fee_policy.rs:91` — ×3 on padded gas × the tier price.
  - `rust/crates/vela-core/src/app/fee_tier_pref.rs:57` — Fast by default.
  - `SigningHost.svelte:465` — `approveOpts.quoted_fee` is the core's quote, signed verbatim.
  - `safe-transaction.ts:591` — the TS fallback has its own `INBAND_MARKUP = 3n`, used only when no quote is passed.
- **Check** (panel; MultiTest; Uniswap on Base; the same 0.05 USDC → ETH swap as W-H2/W-F1, no swap of its own, §3):
  1. On the sheet, note the fee and the speed (快速). Toggle 标准 and 慢, and note each figure.
  2. Slide to confirm.
  3. On basescan, compare gasUsed × effectiveGasPrice plus the L1 fee with the fee leg (the USDC Transfer to the fee recipient in the same tx).
  - BAD (today, pending the owner): about 14×.
  - GOOD: not defined until the owner picks a ratio (§11.2 item 1). Record the measured ratio.
- **Fix:** the owner's decision, in the core. Also retire the TS fallback's `3n`.

#### W-F1 · P2 · likely (high) — A dApp row says nothing about what it moved
- **Why here:**
  - `src/lib/signing/core/sign-executor.ts:296` — `assetChanges: sim ? serializeAssetSim(sim) : undefined`, and sim is always null on the dApp path.
  - `SigningHost.svelte:455` — approveOpts sets no `balance_changes`. The generated `SignApproveOpts.ts` has `balance_changes?: Array<TrustSimJudgment> | null`, the core's only source for the figure.
  - `feed-executor.ts:97` — maps only dapp_url and intent. The generated `FeedTxRecord.ts` offers balance_changes and calldata, and neither is ever set.
  - `src/lib/wallet/live.ts:449` — the row amount is item.value or the batch count; no `dapp.changes` and no '≈'.
  - The desktop stores `record.balance_changes` (`app-desktop/vela-wallet/src/executor/sign_request.rs:761`).
- **Check** (after the rebuild; panel; MultiTest; Uniswap on Base; the W-H2 swap, 0.05 USDC → ETH, about $0.08; §3). Open 活动.
  - BAD: 合约交互 · app.uniswap.org, with no amount and no 余额变化 in the detail. The sheet showed no 余额变化 before sliding either.
  - GOOD: ≈ −0.05 USDC / ≈ +0.000019 ETH, and a 余额变化 block in the detail.
- **Fix:** mirror 48b3e931, 9004bba3, b2430a4e. It needs the rebuilt wasm and W-U1's sheet simulation.
  1. `approveOpts.balance_changes` = the simulation judged through token_trust (TrustSimJudgment[]; desktop approved_changes, `signing_host.rs:1367`).
  2. Persist `record.balance_changes`.
  3. `feed-executor` maps it to `FeedTxRecord.balance_changes`.
  4. `wallet/live.ts` and `live-detail.ts` draw `dapp.changes` / `received` / `estimated` with '≈'.

#### W-F3 · P2 · likely (high) — The called contract labelled 接收方
- **Why here:**
  - `src/lib/wallet/live-detail.ts:175` — the counterparty label is always from/to.
  - `feed-executor.ts:97` — no calldata flag is sent, so `contract_call` is always false.
  - The desktop decides the flag at persist time from the WHOLE request (`app-desktop/vela-wallet/src/executor/sign_request.rs:764`), not from signedRequest, which is clipped at 8 KB and controlled by the page.
  - Today the row does not exist at all (W-H2).
- **Check** (after the rebuild): open the detail of the W-H2 swap's row (no extra swap, §3).
  - BAD: 接收方 0x…(router).
  - GOOD: 合约 0x…, while Send dust keeps 接收方.
- **Fix:** mirror 48b3e931 and 9004bba3.
  1. `sign-executor.ts` persist_record: store a calldata flag from the whole `params_json` (non-empty data/input on any leg).
  2. `feed-executor.ts` maps it to `FeedTxRecord.calldata`.
  3. `live-detail.ts:173-186`: use `tokenDetail.labelContract` when `item.dapp?.contract_call`.

#### W-H4a · P2 · likely (high) — A failed message signature shows the transaction sentence **[owner's account, no prompt]**
- **Why here:**
  - `live.ts:940` — signingStatus: any non-rejection submit_failed shows 失败 + `m.status.failedHint`. This is checked BEFORE the onChain/message split (line 957).
  - `engine.server.ts:967` — failedHint = send.txErrorGeneric.
  - `sign_request.rs:1988` — fail_inflight keeps the request and sets sign_error, so the failed status is drawn for a message too.
  - `rust/crates/vela-core/i18n/locales/zh/connect.json:79` — offChainNote exists, but the web's signing messages do not resolve it.
- **Check** (the owner's REAL account, because the parallel override returns before assertSupported; test page, Connect):
  1. In the side panel console:
     ```
     Object.defineProperty(window,'PublicKeyCredential',{value:undefined,configurable:true})
     ```
     `assertSupported` reads it at call time (`src/lib/onboarding/core/passkey.ts:517`).
  2. Click Sign and slide.
  - BAD: 失败 + 交易未能提交。您的资金安全无虞——请重试。, and the tab gets `-32603 WebAuthn is not supported in this browser.`
  - GOOD: 链下签名 — 未向链上发送任何内容。, or an equivalent off-chain sentence.
  - No prompt appears and nothing is spent. Reload the panel afterwards.
- **Fix:** mirror a22e1b30.
  1. `engine.server.ts` status: add `offChainNote: k('connect.detail.offChainNote')`.
  2. `live.ts` signingStatus: for personal_sign and typed_data, use it instead of txErrorGeneric.

#### W-B5792 · P2 · likely (high) — wallet_sendCalls is answered at once with the op hash
- **Windows:** see A-B5792.
- **Why here:** the real web gap is S3 applied to batches.
  - `extension/lib/protocol.js:202` — capabilities and getCallsStatus are 'unsupported', answered 4200 (`background.js:545`), like every other client.
  - `sign-executor.ts:245` — wallet_sendCalls is answered with the op hash, as receipt_pending, the moment the relay accepts it.
  - `dapp-submit.ts:764` — handleSendCalls returns `txResult.userOpHash` without waiting for a receipt.
  - `sign_request.rs:489` — the batch should answer "the tx hash as for a transaction".
  - The desktop answers only after landing (`app-desktop/vela-wallet/src/executor/sign_request.rs:540`).
  - `dapp-submit.ts:916` — the "atomic supported" capabilities live in the dead handleReadOnlyRPC.
  - The result: the batch id resolves nowhere, a reverted batch is never reported, and S3b can return another op's hash.
- **Check** (panel; MultiTest; test page on Gnosis; shim; wrapper):
  1. Click Bundle. The guard shows the unlimited leg; keep it or cap it. Slide (fee only).
     - BAD (today): `[page<-vela] wallet_sendCalls 0x<66-hex>` within 1–2 s of the slide, before 已确认. It equals `__vf.sent.at(-1)`, the op hash. eth_getTransactionReceipt on it returns null, and wallet_getCallsStatus returns 4200.
     - GOOD: the answer arrives only after landing, and it is the TRANSACTION hash, which has a receipt.
  2. Repeat with `__vf.mode='revert'` set before clicking Bundle.
     - BAD: the page still got a hash.
     - GOOD: -32603, reverted.
- **Fix:** mirror ddb52da8's after_landing.
  1. Rebuild the wasm.
  2. `sign-executor.ts`: treat wallet_sendCalls like eth_sendTransaction, with the S2/S3 wait.
  3. Optional: answer wallet_getCallsStatus from the tracker for every client, or keep refusing it everywhere.

#### W-W13 · P2 · likely (high) — Extension reads wait 20 s per silent node; wallet reads 8 s (H6)
- **Windows:** see A-W13.
- **Why here:**
  - `extension/background.js:62` — `READ_TIMEOUT_MS = 20_000`.
  - `background.js:429` — forwardRead walks the endpoints strictly one after another, each with `AbortSignal.timeout(20 s)`. No hedge, no cooldown.
  - `src/lib/dapp/core/ext-chains.ts:53` — the catalog is `network.rpcURL` (Base: https://mainnet.base.org, `src/lib/services/chains.ts:121`) followed by PUBLIC_RPCS (`src/lib/services/rpc-pool-endpoints.ts:59`), in a fixed order.
  - `src/lib/wallet/core/rpc-pool-executor.ts:182` — one post per core effect.
  - `rpc_pool.rs:167` — `HEDGE_AFTER_MS`.
  - `rpc_pool.rs:950` — `pending_urls` is serde-skipped.
  - `rpc_pool.rs:545` — the cooldown.
  - So there are two paths:
    - wallet reads go through the core pool: 8 s, then the next endpoint, ranked, with cooldowns;
    - the extension's dApp reads bypass the pool entirely: 20 s per silent node, every time.
- **Check** (no funds):
  - Extension:
    1. Start the chaos proxy. Launch the scratch profile with `--proxy-server`, load the extension, and enter the parallel space.
    2. Open http://localhost:8138 and switch the site to Base:
       ```
       await ethereum.request({method:'wallet_switchEthereumChain',params:[{chainId:'0x2105'}]})
       ```
    3. `curl "http://127.0.0.1:8899/__chaos?mode=blackhole&match=mainnet\.base\.org"`
    4. Run this three times in the dApp console, watching the service worker's Network tab:
       ```
       console.time('r'); await ethereum.request({method:'eth_blockNumber'}); console.timeEnd('r')
       ```
    - BAD: every run takes about 20 s (about 40 s with `match=mainnet\.base\.org|publicnode`).
    - GOOD: every run under about 2 s.
  - Wallet pool:
    1. In the wallet tab's DevTools Network, filter on 'base' and note the host of the balance POSTs.
    2. Black-hole that host and refresh the balances.
    - BAD: the first refresh waits about 8 s before trying another host. Later refreshes within 30 s skip the dead host because of the cooldown.
    - GOOD: another host is asked at about 1.5 s.
  - Set `mode=pass` afterwards.
- **Fix:**
  - Extension (the bigger win, no core change), in `background.js` forwardRead:
    1. Start the next endpoint after 1.5 s; the first real answer wins, and the rest are aborted.
    2. Cap each attempt at about 8 s.
    3. Keep a small cooldown in the worker.
    4. Publish the pool's ordered list, including the user's settings (§10 W-11).
  - Web pool: mirror `app-desktop/vela-wallet/src/executor/pool.rs` (8872b915, e6b7469e) in `rpc-pool-executor.ts` / `src/lib/services/rpc-pool.ts`.
    - It needs wasm exports in `rust/crates/vela-core-wasm` (`wallet_state.rs` RpcPoolCore): `is_hedged_read`, `early_verdict`, and an accessor for a call's pending URLs (e.g. `rpcPoolNextUrls(call_id)`). Do not un-skip the field.
    - The web-pool part waits for the owner to lift "H6 desktop-first". Ask whether the extension part counts too (§11.2).

#### W-W14 · P2 · likely (high) — The connect consent names neither the account nor the network
- **Windows:** Connect named only the site. The desktop now shows account and network rows, like the phones (e6285fc9, a6b1b2fb).
- **Why here:**
  - `src/lib/dapp/DappRequestHost.svelte:372` — the panel's consent is a title, a body, the method and two buttons. No account row, no network row.
  - `DappRequestHost.svelte:397` — the window's consent is the same (`src/routes/[locale]/request/+page.svelte:43-61`).
  - `src/lib/dapp/request.ts:105` — planPopupConnect pins the grant to `wallet.activeAddress` (line 109) on `chainId` (line 111). The person is shown neither.
  - `engine.server.ts:696` — resolveRequestMessages carries only title, body, connect, cancel and preparing.
- **Check** (parallel space, no funds):
  1. Make Parallel Two the active account.
  2. On http://localhost:8138, switch the site to Base with `await ethereum.request({method:'wallet_switchEthereumChain',params:[{chainId:'0x2105'}]})`.
  3. Click Connect.
  4. Repeat with `chrome.storage.local.set({'vela.ext.surface':'window'})`.
  - BAD: only 连接到 localhost:8138, the body text and 'eth_requestAccounts'.
  - GOOD: an account row (Parallel Two) and a network row (Base), in both the panel and the window.
  - Reset the surface to 'panel', and revoke the site.
- **Fix:** mirror e6285fc9 and a6b1b2fb. On both consent branches, add:
  - an account row for the address approve() will pin (name and identicon);
  - a network row for chainId (via chainName).

  Reuse the connection-panel keys, so no new strings are needed.

#### W-W1 · P3 · likely (medium) — The extension surface cannot start its core and says nothing
- **Windows:** the WebView2 engine never started, and nothing said so. The desktop now shows an engine panel (dbf5a48c, 672d1dbb).
- **Why here:**
  - `src/lib/dapp/DappRequestHost.svelte:119` — take() is `try { await session.boot(); … } finally { taking = false }`, with no catch. A failure becomes an unhandled rejection, and stage stays loading.
  - `DappRequestHost.svelte:410` — in window mode only 正在准备钱包… shows; in panel mode, nothing.
  - `DappRequestHost.svelte:144` — the .guard goes up only after `owing` is set, and a wasm failure throws earlier.
  - `src/lib/session/core/session.svelte.ts:47` and `src/lib/signing/core/sign-resident.svelte.ts:121` — both boots cache a REJECTED promise.
  - `src/lib/core/client.ts:168` — loadCore un-caches a failed init (lines 174-176), but the callers keep their cached rejection.
  - `extension/background.js:253` — in window mode, closing the window settles 4900. In panel mode only content.js's 300 s deadline answers (`extension/content.js:50`: 4900 'Vela did not answer in time — check its activity').
  - `extension/build.mjs:72` — the extension build skips sync-wasm, so a stale `static/` ships a 404 for the core.
  - Realistic triggers: an unsynced local build, or Chrome with WebAssembly disabled by a JIT-blocking policy.
- **Check** (no funds):
  1. Build and load the extension (`pnpm sync:wasm && pnpm build:extension`).
  2. Rename ONLY the core wasm:
     - PowerShell: `Get-ChildItem app-web/vela-wallet/extension/dist -Filter 'vela_core_bg.*.wasm' | Rename-Item -NewName {$_.Name + '.bak'}`
     - bash: `for f in app-web/vela-wallet/extension/dist/vela_core_bg.*.wasm; do mv "$f" "$f.bak"; done`
  3. Press ↻ on the extension card.
  4. On :8137 (reload it first), click Connect (a click, so the side panel opens). Time how long `#out` takes to get an eth_requestAccounts entry (`#out` shows the page's JSON state from load on, never 'waiting…'), and watch the panel console.
  5. Window mode: `chrome.storage.local.set({'vela.ext.surface':'window'})`, reload, click Connect. Leave the window for 1 min, then close it.
  - BAD:
    - the panel is stuck loading or blank, and the window shows only 正在准备钱包…;
    - 'Uncaught (in promise)' or a failed wasm fetch in the console;
    - in panel mode, `#out` has no eth_requestAccounts entry for about 300 s, then gets 4900, even after the panel is closed.
  - GOOD: within seconds the surface says Vela could not start (e.g. 无法加载此页面 + 重试), and `#out` gets an error at once.
  6. Restore the .wasm and press ↻.
- **Fix:** mirror dbf5a48c and 672d1dbb.
  1. In take(), add a catch that:
     - sets stage `{kind:'failed'}`;
     - draws `connect.browser.loadFailed` + `connect.browser.retry` (add both to RequestMessages / resolveRequestMessages);
     - answers the request at once (settleOnClose's 4900; if the core itself cannot load, a fixed 4900 through answerRequest);
     - clears owing.
  2. Make Retry reset the cached boots (set `#booted` / `#booting` back to null on rejection), or just call `location.reload()`.
  3. Consider making `extension/build.mjs` run or check sync-wasm.

#### W-H1 · P3 · likely (high) — Sign-in sheet copy
- **Why here:**
  - `src/routes/[locale]/+page.svelte:289` — the 登录 sheet is `<AddMethodPicker … onPick={signIn}>`, the create chooser.
  - `src/lib/ui/onboarding/v2/AddMethodPicker.svelte:103` — every caption is `methodCopy(method).body`.
  - `src/lib/onboarding/core/copy.ts:97` — platform → methodPlatformBody, hybrid → methodHybridBody.
  - `rust/crates/vela-core/i18n/locales/zh/onboarding.json:174/177` — 扫码，用附近设备创建 / Touch ID 或 Windows Hello. English: "Scan a code and create it on a nearby device" / "Touch ID or Windows Hello".
  - `explore.json:5` — 'scan': '扫码'.
- **Check** (no passkey needed; a fresh profile or cleared site data):
  1. Open http://localhost:5173/zh on Windows Chrome, a Mac, Android Chrome (via adb reverse) and iPhone Safari (`pnpm dev --host` is fine for copy).
  2. Press 登录 and read the rows, then back out WITHOUT choosing a method: a ceremony on localhost or a LAN IP makes a different, empty wallet (§9.1). Also check the extension: its toolbar icon takes a signed-out person to the welcome page.
  - BAD: 手机或平板 · 扫码，用附近设备创建, and 这台设备 · Touch ID 或 Windows Hello, on every OS.
  - GOOD: the phone row says 扫码, and "this device" names that machine's authenticator.
- **Fix:** mirror d9ab6e80 and 5c79c6b3.
  1. Give AddMethodPicker a `chooser: 'create' | 'sign_in'` prop.
  2. The sign-in hybrid caption is 'explore.scan'; add it to the welcome page's manifest in `src/lib/i18n/messages.ts`.
  3. Take the platform caption from `navigator.userAgentData?.platform` or the UA.

#### W-EXE · P3 · likely (medium) — Sheet headline "Execute"
- **Why here:**
  - `live.ts:476` — the headline is `result.intent`.
  - `live.ts:813` — only a named intent_term is localized.
  - `src/lib/signing/core/clear-executor.ts:121` — the same 4-byte source as the desktop.
  - `live.ts:1074` — the slide action is 'Execute' too.
- **Check** (panel; MultiTest; Uniswap on Base, 0.05 USDC → ETH; look at the sheet only, no slide).
  - BAD: 'Execute' with the best-effort warning, in a zh UI.
  - GOOD (future): 兑换 0.05 USDC → ETH.
- **Fix:** a core descriptor, shared by every client.

#### W-F2 · P3 · handled (high)
- **Why:** `src/lib/wallet/live-detail.ts:206` shortens the hash with `shortenAddress` and copies the full value (`src/lib/wallet/identity.ts:40`).
- **Check:** open a confirmed tx in the side panel's 活动 at about 360 px wide, and in the hosted wallet's third column.
  - GOOD: 0x7ca765…f89269-style, and copy gives all 66 characters.
  - BAD: the hash is clipped or runs off the panel.

#### W-W10 · P3 · handled (high) — needs a real-browser pass
- **Why:** `live.ts:350` nativeSendOf, and `live.ts:546` blind_transaction + nativeSendOf → nativeSendBlocks (3707e196, 1618a9f6, 4d4c76c4). Verified by unit and e2e tests only.
- **Check** (panel; MultiTest; Switch to Gnosis; Send dust):
  - GOOD: 发送 · −0.001 xDAI · 接收方 0x7687…D141, with no red warning.
  - BAD: 合约交互 + ⚠ 无法解码 — 无 ERC-7730 描述符（0 字节）.
  - Close with ✕ (4001) to spend nothing.

#### W-W11 · P3 · handled (high) **[owner]**
- **Why:**
  - `live.ts:973` — the title is `ceremonyUp ? m.status.signing : m.status.preparing`.
  - `src/lib/signing/approval-progress.ts:43` and `passkey.ts:363` — the 'started' signal fires right before `navigator.credentials.get` (3707e196).
- **Check** (the owner's account; Send dust on Gnosis; slide):
  - GOOD: 正在准备交易… during the funding check, nonce and estimate; 等待生物识别… only once the browser's passkey sheet is visible.
  - BAD: 等待生物识别… for seconds with no prompt.
  - Cancel the prompt to spend nothing.

#### W-H4b · P3 · handled (medium) **[owner]**
- **Why:**
  - `passkey.ts:533` — NotAllowedError and AbortError (a cancel, a timeout, a hybrid attempt that gave up) are 'cancelled'. `sign-executor.ts:346` turns that into passkey_cancelled: no answer, and the request stays open.
  - `passkey.ts:543` — everything else becomes 'other' with the platform's English. These are hard failures the desktop also fails; only the raw text is cosmetic.
  - `src/lib/signing/sign-challenge.ts:55` — the Trusted-Signer-only key is a plain Error. The web has no Trusted Signer, by design.
- **Check** (the owner's account; Send dust):
  - (a) Slide, then Cancel/Esc on the browser's passkey sheet.
    - GOOD: back to the form, and no answer yet. ✕ then gives exactly one 4001.
  - (b) Slide, choose "Use a phone or tablet", and scan with a phone whose Bluetooth is off, so it never connects. Let Chrome give up, or cancel it.
    - GOOD: the same as (a), with no -32603.
  - BAD: -32603, and the request is gone.
- **Fix:** optional — map 'other'/'not_supported' to a fixed English page message.

#### W-REC · P3 · handled (medium) — the core guards arrive with the rebuild
- **Why:**
  - `src/lib/services/dapp-history.ts:201` — only eth_sendTransaction keeps to/value; a batch stores '' / '0x0'.
  - `feed-executor.ts:78` — values are handled as BigInt.
  - `identity.ts:40` — a JS slice, which cannot panic.
  - `DappRequestHost.svelte:252` — `dapp: null`, so the origin comes from the browser.
  - The core does not check `version` or `atomicRequired`, so the probe below is accepted.
- **Check** (after the rebuild; panel; MultiTest; Gnosis). Rebind the button so it is a click:
  ```
  document.getElementById('send').onclick=()=>__ask('wallet_sendCalls',[{version:'2.0.0',chainId:'0x64',from:me(),to:'日本語日本語',value:'0x3635c9adc5dea00000',calls:[{to:'0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141',value:'0x0',data:'0x'}]}])
  ```
  Click Send dust and slide (fee only).
  - GOOD: the row has no 日本語 counterparty and no 1000-xDAI figure, and reloading the panel twice is fine.
  - BAD: a counterparty or figure the sheet never showed, or a broken feed.
- **Fix:** none in the shell. Rebuild and commit the web core.

#### W-D1b · P3 · handled (medium) **[owner]**
- **Why:**
  - `live.ts:966` — before the signature the status is not closable, and `SigningHost.svelte:553` makes the ✕ dead while the passkey is pending. There is no Escape either.
  - `sign-executor.ts:346` — a cancel brings the form back, still owed.
  - `DappRequestHost.svelte:186` — closing the surface with an answer owed gives 4900.
  - `rust/pkg-web/build-info.json:3` — the shipped wasm lacks 4198ec6b.
- **Check** (the owner's account; Send dust):
  1. Slide. While 正在准备交易… or the prompt is up, click ✕. GOOD: nothing happens.
  2. Cancel the prompt. GOOD: the form is back, and there is no answer.
  3. Click ✕. GOOD: exactly one 4001.
  4. Also close the side panel mid-prompt. Expect one 4900 'The browser closed before the request finished'.
- **Fix:** none in the shell. Rebuild the web core.

#### W-D1 · P3 · handled (high) — one caveat about closing the panel
- **Why:**
  - `src/lib/signing/SigningSheet.svelte:13` — the sheet closes only on ✕ (BottomSheet 'explicit'; `src/lib/wallet/ui/BottomSheet.svelte:92`, Escape at line 437).
  - `live.ts:956` — closable only once signed, submitting, and no ceremony is up.
  - `DappRequestHost.svelte:377` — the consent is 'explicit' too, and ✕ / 取消 give 4001.
  - The caveat: `DappRequestHost.svelte:189` sends the 4900 on pagehide, asynchronously, from a page that is unloading. `background.js:253` backs up the WINDOW with windows.onRemoved, but nothing backs up a closed SIDE PANEL.
- **Check** (parallel space; no funds):
  1. On :8137, Connect, then Sign.
  2. Press Esc, click outside the sheet, and drag it down. The sheet stays, and `#out` has no answer.
  3. Close the side panel with Chrome's own panel ✕.
     - GOOD: `#out` gets 4900 at once.
     - BAD: `#out` has no personal_sign entry (reload the page before step 1; or the "What the page got" wrapper logs nothing) until about 300 s, then 4900; or it gets 4001.
  4. In window mode, closing the window gives 4900.
  5. The sheet's own ✕ gives 4001.
- **Fix:** none for the gestures. If step 3 fails, open a `runtime.connect` port from the panel, and settle its owed requests on onDisconnect.

#### W-W19 · P3 · handled (high) **[owner + iPhone]**
- **Why:**
  - `passkey.ts:119` — a hybrid route sets `hints ['hybrid']`; the browser draws its own QR and runs caBLE.
  - `passkey.ts:530` — a cancel or timeout is 'cancelled', and `sign-executor.ts:346` keeps the request open.
  - `live.ts:966` — the status says preparing until the ceremony is up.
- **Check** (the owner's wallet with a phone-held passkey, plus the owner's iPhone; personal_sign, no funds):
  1. In desktop Chrome with the extension, go to :8137, Connect, Sign, slide.
     - Expected: 正在准备… until Chrome's "Use a phone or tablet" QR appears. Scanning with the iPhone signs, and `#out` shows 0x….
  2. Repeat, and this time close Chrome's QR dialog, or let it time out.
     - GOOD: back to the form, with no answer yet.
     - BAD: -32603 or 4001 on the cancel or timeout; or 等待生物识别 before Chrome's dialog appears.

#### Web — not applicable
- **W-U1d** — `engine.server.ts:878` resolves warnWillFail, but it is used only in a fixture (`src/lib/signing/fixtures.ts:610`), and the gallery is pruned from the extension (`extension/build.mjs:62`). When W-U1b is fixed, use the first clause, and fix the fixture.
- **W-W9** — there is no 探索 on the web (`destinations.ts:15`), the live data replaces the fixtures, and the gallery is pruned.
- **W-W2** — the address bar is Chrome's omnibox. The site comes from `sender.origin` for each request (`background.js:523`).
- **W-W3** — Chrome loads the dApp and shows its own error pages. Vela's surfaces are packaged pages (`extension/panel.js:20`). A dead node gives a JSON-RPC -32603 (`background.js:457`).
- **W-DNS** — `fetch` exposes no name-resolution codes (`background.js:454`, `rpc-pool-executor.ts:116`).
- **W-H8** — there are no Vela tabs. Chrome titles its own.
- **W-W6** — new windows are Chrome's. Vela's own explorer link is `target=_blank` (`src/lib/signing/ui/DappReceipt.svelte:58`).
- **W-W7** — the permissions are storage, tabs and sidePanel only (`manifest.json:29`). Chrome handles external schemes and downloads.
- **W-W15** — Back is Chrome's, per tab. The panel's cross-tab bug is §10 W-10.
- **W-D3** — timeouts are per traffic class (`src/lib/services/net.ts:10`). The browser owns proxy vs direct.
- **W-W20** — the wasm exports no caBLE (`rust/pkg-web`), and the browser's WebAuthn runs hybrid itself (`passkey.ts:119`).
- **W-W12** — everything is DOM layers in one document, and the side panel sits beside the tab (`manifest.json:26`).
- **W-H5** — not assessed separately. It follows from W-W19 / W-W20: there is no Vela caBLE on the web.
---

## 10. Also noticed (outside the Windows inventory)

The reviews found these while tracing the items above. Priorities are the reviewers'. Put the security ones (A-1, I-1, I-10, I-11, W-10) next to the P0/P1 rows.

### Android

| # | Finding | Evidence | Why it matters / how to check |
|---|---|---|---|
| A-1 | **A stray `calls` key in eth_sendTransaction sets the sheet's headline, while the top-level call is what gets signed (security, P0/P1)** | `SigningController.kt:464-469` `firstCall` reads `params[0].calls[0]` for every method. `clearKickoff` (471-475) and `SigningLive.model` (`SigningLive.kt:237`, blocks line 246) decode from it. `SignExecutor.callsOf` (`SignExecutor.kt:264-275`) submits the top-level to/data, and the simulation (`SigningController.kt:306`) uses callsOf too, so only the sim and guard blocks tell the truth | The headline, the plain-transfer card and the recorded intent can describe a decoy while an unlimited approve is signed. The desktop fixed exactly this in d5661249: `first_call` follows `calls` only for wallet_sendCalls.<br>**Check (sheet only, reject with ✕):** `__ask('eth_sendTransaction',[{from:me(),to:GNOSIS_USDC,value:'0x0',data:'0x095ea7b3'+'1'.repeat(40).padStart(64,'0')+'0'.repeat(64),calls:[{to:'0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141',value:'0x1'}]}])`<br>BAD: the headline card reads 发送 · 接收方 0x7687….<br>GOOD: the headline describes the signed call, a USDC approval (amount 0) to 0x1111….<br>This probe approves 0, so it is harmless even if slid by mistake |
| A-2 | A multi-leg wallet_sendCalls whose first leg is a plain coin transfer is drawn as a single 发送 card (P1) | `SigningLive.kt:644-645` picks plainTransferBlocks from the first call alone, with no check that the batch has one leg. The desktop's `native_send_of` (`app-desktop/vela-wallet/src/wallet/signing_host.rs:1414-1443`, d5661249) requires exactly one leg | The sheet reads "send 0.001 xDAI to X" for a batch that also does something else |
| A-3 | Calldata sent as `input` is dropped; a zero send reads '−0'; 18 decimals and the settings symbol on any chain (P3) | `SignExecutor.kt:281` and `SigningController.kt:467` read only `data`, so `{to, value, input:'0x…'}` draws plainTransferBlocks (`SigningLive.kt:644`) and is submitted as a bare value transfer, with the call dropped. `plainTransferBlocks` (650-657) always prefixes '−'. The web fixed the first two (4d4c76c4). The desktop requires a built-in chain's symbol and no Tempo (`signing_host.rs:1433-1439`) | The person approves what reads as a simple send, while the dApp's intended call is silently not made |
| A-4 | The transaction detail's status comes from the tx hash, not from the record's status (P1) | `FlowLive.kt:265-269`: 处理中 when tx_hash is blank, 已确认 otherwise. The detail reads `feed.rows` (`FlowLive.kt:216`), never `FeedView.transactions` (`FeedWire.kt:281`). The tracker's failure patch carries no tx hash (`tx_tracker.rs:1104-1113`), but `FeedExecutor.patchRecords` keeps any txHash already stored (`FeedExecutor.kt:288`) | Two failure modes. A dApp record already stamped Confirmed (S2), then marked failed by the tracker, reads 已确认. A failed record without a hash (a dropped Send, or a dApp op that failed after submit, `SignExecutor.kt:116`) reads 处理中 forever |
| A-5 | The Android JVM suite is red on this branch: `CoreWireDriftTest.signRequestWiresMatchTheMirrors` (P2) | `CoreWireDriftTest.kt:899` `assertVariantsExhaustive<SignSubmitOutcome>` requires an exact match (985-994): TS has 7 variants, Kotlin `SignWire.kt:224` has 5. The other 083 mirror changes pass the subset checks. Not run | Whoever runs the Android tests sees this first. S2/S3 turn it green |
| A-6 | The page's receipt lookup of an op hash is translated to a reverted bundle transaction (P1) | `BrowserController.kt:458-459` `userOpTxHash = (relay.userOpReceipt(...) as? Resolved)?.txHash` ignores `confirmed`. The core then reads that tx's receipt (`dapp_browser.rs:1486-1495`) | After an S3 op-hash answer, a site that reads receipts through the wallet sees status 0x1 for an op that reverted: a second route to S2's false success |
| A-7 | A pending row's detail shows the record id as the hash, and links it to the explorer (P3) | `FlowLive.kt:227` `hash = item.tx_hash ?: item.id` feeds the 哈希 fact (253) and `explorerUrl /tx/$hash` (261). A pending dApp record has id `dapp-<ms>-tx` (`sign_request.rs:826-831`) | While a swap is pending, 哈希 shows 'dapp-1727…-tx', and 在浏览器中查看 opens a broken explorer page |
| A-8 | A dApp row with no call value has an empty amount, a bare '−' hero, and the title '已发送 ' (P2/P3) | `WalletLive.kt:213-214` returns '' when value is null. `FlowLive.kt:262` titles it `history.txLabelSent` over an empty symbol; line 270 builds the hero as '−' plus empty strings | Every Uniswap USDC→ETH or approve row looks broken until H2 and F1 are mapped |
| A-9 | No build can open a plain http:// site except loopback, yet the bar has an insecure-http state | Release builds have no network_security_config. The debug config (`app/src/debug/res/xml/network_security_config.xml`) permits only 127.0.0.1/localhost, with targetSdk 36 (`build.gradle.kts:26`). `dapp_rpc.rs:496` loads a typed http:// URL as typed. The blocked load is most likely ERROR_UNKNOWN (-1), which `browser_load.rs:111` classifies as Offline | An http dApp or LAN page probably shows 网络不稳定，页面没能打开。 and retries 3 times, instead of a clear reason.<br>**Check (debug build):** open http://neverssl.com; read the panel and `code=` in logcat.<br>Owner decision (§11.3) |
| A-10 | A WebView too old for the provider loads pages with no wallet, silently (also under A-W1) | `ProviderBridge.kt:38-43` returns false and logs `browser.inject provider unavailable`. `EngineState.wallet` (`BrowserController.kt:146`) is never read | Every dApp says "no wallet detected", and Vela says nothing. Fix: a one-line notice ("update Android System WebView") |
| A-11 | A caBLE tunnel failure escapes as IOException and reaches the page as "Signing failed" -32603 (also under A-W19) | `CableTransports.kt:205-212` throws IOException. `HybridCeremony.kt:151` catches only CtapException, and `UserOpSpine.kt:149` only PasskeyFailure. The failure then goes to `SignExecutor.neutralAnswer` (`:143`) → -32603 | The desktop tells "the link never came up" apart from "the phone hung up", and offers Retry (1063909f, a22e1b30) |
| A-12 | A crashed tab's ⋯ menu, account sheet and address editor are the Uniswap fixtures, and 断开连接 there revokes the real site (also under A-W9) | `ExploreLive.kt:194, 213, 244`; `ExploreFixtures.kt:117-145`; `BrowserController.kt:517` | The action (revoke the real site) and the label (app.uniswap.org) disagree. The 安全站点 text that 079's SC-007 banned is back |
| A-13 | Low confidence: non-http(s) navigations in subframes are cancelled | `BrowserController.kt:194-206` returns true for every non-http(s) scheme in any frame | Widgets that navigate an iframe to data: or blob: may break.<br>**Check:** `document.body.insertAdjacentHTML('beforeend','<iframe src="data:text/html,<h1>hi</h1>"></iframe>')` — does "hi" render? |

### iOS

| # | Finding | Evidence | Why it matters / how to check |
|---|---|---|---|
| I-1 | **A stray `calls` key in eth_sendTransaction sets the sheet's headline and recipient, while the top-level call is signed (security)** | `SigningController.swift:728` `let call = ((first["calls"] as? [[String: Any]])?.first) ?? first` follows `calls` for every method. That call feeds clearKickoff (738-746), the facts/dataBytes (`SigningLive.swift:193-194`) and the plain-transfer branch (`SigningLive.swift:569`). `SignExecutor.callsOf` (409-414) signs `params[0]` itself; the fee quote and the simulation use callsOf. The core does not normalise this | A page can send `{to: USDC, data: transfer(attacker, all), calls:[{to: friend, value:'0x1'}]}`. The sheet reads 发送 0.000…1 xDAI · 接收方 friend with 确认发送, while the USDC transfer is what gets signed. The desktop fixed this in d5661249.<br>**Check (test page on Gnosis; sheet only, reject with ✕):** the same probe as A-1, which approves 0 and so is harmless even if slid: `__ask('eth_sendTransaction',[{from:me(),to:GNOSIS_USDC,value:'0x0',data:'0x095ea7b3'+'1'.repeat(40).padStart(64,'0')+'0'.repeat(64),calls:[{to:'0x76875e38fc6Bc2dEDCaed807cE00782DB5C0D141',value:'0x1'}]}])`<br>BAD: the headline reads 发送 · 接收方 0x7687….<br>GOOD: a USDC approval (amount 0) to 0x1111… |
| I-2 | A multi-call wallet_sendCalls whose first leg is a plain transfer draws as one calm 发送 | `SigningLive.swift:569-571` has no single-call check. firstCall/clearKickoff (`SigningController.swift:722-746`) resolve only the first leg, and guardBlocks (`SigningLive.swift:902-921`) lists only approval legs | [send 1 wei to A, token transfer to B] reads 发送 · 接收方 A. The desktop (d5661249) and the web (1618a9f6) require exactly one call |
| I-3 | Calldata sent as `input` is dropped, so a contract call is submitted as a bare coin transfer | `SignExecutor.swift:421` and `SigningController.swift:731-733` read only `data`, and the core has no `input` normalisation | The dApp's intent is silently changed, and coin sent to a router's receive() may be lost. The web marks `input` as calldata (4d4c76c4) |
| I-4 | A JSON-number `value` is read as 0 | `SignExecutor.swift:419`, `SigningController.swift:731`, recordRow (450) read only strings | A zero-value op, and the sheet shows −0. The desktop reads numbers (`wei_of`, ff660c2d) |
| I-5 | The wallet's own sends share the existingHash path | `Features/Send/SendExecutor.swift:464` → `UserOpSpine.swift:446-449` | A colliding send is tracked and notified as the PREVIOUS op. S3b's fix belongs in the spine, so it covers both paths |
| I-6 | Reverted dApp op: the Activity status races between the sign path (confirmed) and the tracker (failed) | `SignExecutor.swift:149-163` update_record writes confirmed. `TrackerExecutor.swift:87-96` patches failed for the same record id (the handoff is at `SigningController.swift:642-650`). Both write through `TxRecords.patch` | An S2 device run may show either state. Adopting Reverted closes the record failed on both sides |
| I-7 | dApp and Send signatures never use Vela's own caBLE or CCID: a BLE-only phone key can sign in but cannot sign (P1 for that group) | `RootView.swift:313` uses a bare executor. `OnboardingModel.swift:203-209` wires hybrid and smartCard into onboarding only. `PasskeyExecutor.swift:316-325` falls through to the system request. `HybridCeremony.swift:5-8` says Vela's initiator exists for the case Apple's cross-device flow cannot handle (the CTAP 2.3 BLE-only channel) | A person who signed in with an Android phone without Google Play cannot sign anything afterwards. A USB/NFC key skips the app's CCID path that the founder preferred (`PasskeyExecutor.swift:76-81`). Wiring it needs the QR, touch and PIN drawn inside the signing sheet |
| I-8 | The op-hash receipt translation hands a page a "successful" receipt for a reverted op | `RootView.swift:411-415` resolveUserOp ignores `confirmed`. `dapp_browser.rs:1535-1552` then returns the bundle tx's receipt, status 0x1. (The op-hash answers themselves are I-S2/I-S3; they are wired at `BrowserController.swift:451-459` and `RootView.swift:404-414, 1429, 1642`) | A page polling `eth_getTransactionReceipt(opHash)` through Vela sees success. Fix together with S2/S3 |
| I-9 | 083's core helpers are not reachable from Swift yet | §2.2 export list. `user_op_safe_op_hash` (`vela-core-uniffi/src/lib.rs:1386`) is the Safe's hash, NOT the ERC-4337 userOpHash | S3b, R3, and parity-grade S2/S3 need these exports, a `build-ios-xcframework.sh` run and a bindings commit. S2's minimal success==false mapping needs only the rebuilt core |
| I-10 | **A page's script can probably leave the app through a synthetic link click, with no tap** | `BrowserEngine.swift:646` treats `.linkActivated` as the tap. WebKit builds LinkClicked for a script's `element.click()`, then `policy()` (693) returns `.handToSystem` and line 664 opens it | If I-W7 step 3 confirms this, any page, including an ad iframe (via target=_blank, since a nil targetFrame counts as the main frame, line 642), can open Mail, Messages, the App Store or another app with no touch |
| I-11 | **A cross-origin iframe can navigate the whole dApp tab, or launch another app, with one tap** | `BrowserEngine.swift:711-716` createWebViewWith does not check sourceFrame. Lines 642/693: a nil targetFrame counts as the main frame. The test dApp's iframe (served on port+1) is the place to try it | An ad or widget can replace the dApp mid-flow (its open requests settle 4900), or send the person to another app. The desktop (a6b1b2fb) drops these unless they come from the top document's origin |
| I-12 | The pill can show a closed lock beside a host it does not vouch for | `ExploreScreen.swift:156-158` `browserSecure = currentTab?.secure` is judged on doc_origin / shown_origin (`dapp_browser.rs:664-672`). shown_origin is set only at navigation_started, which happens at commit (`BrowserEngine.swift:483-488`). Meanwhile the host text follows webView.url (338, 353-371) | From an https page, typing an http:// address (or one that then fails) shows 🔒 new-host until the commit, or the previous site's closed lock over a certificate panel (I-H8). P3 |
| I-13 | The external-scheme comment overstates the OS | `BrowserEngine.swift:634-635` says "The system still asks the person before it leaves the app", yet line 664 calls `UIApplication.shared.open` directly | Only tel:/facetime: get a system confirmation. mailto:, sms:, itms-apps: and third-party schemes switch apps at once. This bears on I-W7 |
| I-14 | A black-holed site shows no panel for about 60 s | `BrowserEngine.swift:179` `URLRequest(url:)` uses the default 60 s timeout, with no watchdog. The panel appears only on -1001 (`browser_load.rs:128`) | The desktop shows its panel at about 12 s. Check with I-W3 step 3. P2/P3, the owner's call |

### Web

| # | Finding | Evidence | Why it matters / how to check |
|---|---|---|---|
| W-1 | **The web builds with a pre-083 core artifact, so no 083 core fix reaches the web today** | `rust/pkg-web/build-info.json:3` source `55d58f8791496bdc…` and `assets/wasm/vela_core_bg.55d58f879149.wasm` were last changed in 42acc794 (2026-09-28 13:40). c97e0b89 (20:10) and 4198ec6b (18:04) came after. The current source fingerprint is 3b09e97ad4727cbe… (172 files). `pnpm check` compares only `static/` against `assets/wasm`. CI runs `build-web.mjs --check` (`ci.yml:281`) | None of these ship until `npm --prefix scripts run build:wasm` is run and committed: H2 rows, R2, the D1b net, dapp_url, F1/F3 fields, and the new wire variants. Types and binary disagree, and the branch's PR will be red. A device session that skips the rebuild will report pre-083 behaviour |
| W-2 | The web dApp signing sheet shows no balance changes at all | `src/lib/signing/live.ts` never emits a 'balances' block (only `fixtures.ts` does). `sign-resident.svelte.ts:116` setApproveSim has no caller | An undecoded router call shows only 'Execute' and a caution; nothing says how much USDC leaves. This is also the missing input for U1 and F1 |
| W-3 | Web dApp records never carry the decoded intent | `SigningHost.svelte:475` sends `intent: null`. The core falls back only to plain_send_intent (`sign_request.rs:841, 1804-1808`) | Even after the rebuild, every decoded dApp transaction reads 合约交互. The desktop passes the intent (`signing_host.rs:1383`, 43fd67a4) |
| W-4 | A reverted op tells the dApp to "try again with a higher gas price" | `safe-transaction.ts:3429-3431` reaches the page verbatim (`sign-types.ts:135-137`) | The op was included and may have paid the fee; the advice invites a second paid revert. The core's REVERTED_MESSAGE with the tx hash is the right answer |
| W-5 | The extension has no way to resolve an op hash it handed out | `extension/lib/protocol.js:113-131` forwards receipt reads verbatim. The translation (`dapp-submit.ts:987-1004`) sits in the dead handleReadOnlyRPC | After S3, and after every wallet_sendCalls (B5792), the page's receipt polling returns null forever: the "Uniswap waits on pending forever" symptom |
| W-6 | The landing's failure caption points at an explorer button that is not drawn | `dapp-receipt.ts:131-138` (the op hash only); `landingFromEntry` (168) drops `entry.tx_hash`; zh `componentsTx.json:31` says 可点下方「浏览器」 | A person whose swap reverted is told to press a button that is not there, and has no tx hash, although the tracker holds it |
| W-7 | Relay AA25 refusals lose the `[existingHash]` marker | `safe-transaction.ts:3340` throws parseBundlerError(...); lines 3656-3657 rewrite AA25 (and 3652/3654 'simulation failed'/'reverted') before the catch at 1797-1807 | This is the bug ddb52da8 fixed on the desktop. With that wording the web fails with "Transaction nonce mismatch. Please try again." instead of waiting. Other wordings hit S3b |
| W-8 | Popup-window requests close before any outcome can be shown | `extension/background.js` settle() (156-170) runs `chrome.windows.remove`. `routes/[locale]/request/+page.svelte:19-23` says "The window does not get a landing" | A request fired without a gesture signs in a window that disappears on the answer. A revert or failure (R3) is never shown there |
| W-9 | results.md and ddb52da8 overstate the web's S2 gap | results.md "Where the clients differ after 083", and the ddb52da8 message, say the web answers a reverted op as success. In fact `safe-transaction.ts:3426-3431` throws on success:false, and the core answers -32603 and fails the record | Fix the wording and the in-op ExecutionFailure check (W-S2), not the polarity. Correct the text in the new spec, not in 083's files |
| W-10 | **The side panel serves one tab for its whole life: a request from a second dApp tab in the same window is never shown, and waits 5 minutes** | `DappRequestHost.svelte:130` `tabId ??= await panelTabId()` is resolved once, as the active tab (`src/lib/dapp/transport.ts:100-110`). `background.js:235-243` nextForPanel(tabId) skips every other tab's entries. `background.js:187-205` panel.open({tabId}), yet the manifest has only side_panel.default_path (`manifest.json:26-27`) and no per-tab setOptions, so the panel is the window's global one and stays loaded across tab switches (`transport.ts:130-136`). B's entry gets surface 'panel' (`background.js:199`), so no window opens either | Confirmed by code; e2e cannot drive the panel.<br>**Check (no funds):** tab A :8137 → Connect → 连接; then in tab B (:8138, same window) → Connect.<br>BAD: nothing appears in the panel, and B gets 4900 'Vela did not answer in time' after 300 s.<br>GOOD: B's consent appears.<br>Fix: resolve the tab per request (carry the changed `vela.req` key's tabId into take(), re-query panelTabId() on every take(), or use per-tab panels with sidePanel.setOptions before open()) |
| W-11 | The extension's dApp reads ignore the person's own RPC settings, provider keys and everything the pool has learned | `src/lib/dapp/core/ext-chains.ts:48-55` = `network.rpcURL` + PUBLIC_RPCS. The pool's `collectRpcUrls` puts the user's override and the Alchemy/dRPC/Ankr keys first (`rpc-pool-endpoints.ts:93-113`). `background.js:413-460` walks the list in order, 20 s each | Someone whose default node is blocked, and who set a working RPC in Settings, still has every dApp read try the default first.<br>**Check:** set a custom Base RPC, black-hole mainnet.base.org, and read eth_blockNumber from a dApp. BAD: the worker's Network tab tries mainnet.base.org first.<br>Fix: build the catalog from collectRpcUrls, and republish it on change |
| W-12 | The web never tells the signing core that a request's page went away (`transport_dropped` is never dispatched) | Declared in `src/lib/core/generated/SignEvent.ts:18`, never dispatched. `DappRequestHost.svelte:216` registers a transport per request and never unregisters it (the registry evicts the oldest past 32). The core clears the sheet on TransportDropped (`sign_request.rs:1206`), even in the committed wasm | The core keeps treating a dead page's request as live. This underlies W-W5. Dispatching it fixes the sheet part with today's wasm |

---

## 11. Not easily checkable, and open owner decisions

### 11.1 Hard or impossible to stage

- **H6 / W13 hedged reads.** The pool ranks endpoints by measured speed, so a slow user RPC is never asked, and Windows could not reproduce H6 at all.
  - Staging needs the FASTEST node to go silent: black-hole the host the pool actually uses (find it in rpc.post / chaos.log / DevTools).
  - The core then cools that endpoint down for 30 s (doubling), so only the first read after the fault is slow.
- **R2 (a bundle neighbour's failure).** It cannot be arranged on the shared relay.
  - Use `cargo test -p vela-core --test app_tx_tracker`.
  - On the web, the fetch shim's `neighbour` / `inner` modes.
  - On the phones, a smoke test only: `tracker.patch confirmed` after a real swap.
- **S2 on chain.**
  - A real `success:false` needs a deadline race: the relay's fee leg runs inside the same execution, so it may simply refuse to bundle a reverting op.
  - Windows found S2 by review, not on chain.
  - Use the JVM / XCTest / shim checks as the evidence.
- **S3b.** It needs two ops of one account colliding in the relay.
  - The test account's USDC is already Permit2-approved, so Uniswap sends no separate approve transaction to collide with.
  - Use the hermetic checks and the web shim's `pending` mode.
- **R3** needs a revert AND a receipt later than 120 s. Use the UI unit checks.
- **Passkey cancel and failure cases** (D1b, H4a on the web, H4b, W11, W19, W20, H5) cannot be staged in the parallel space, because fixture keys sign without a prompt. They need the owner, the owner's account and, for W19/W20/H5, a second phone.
  - iOS H4b may not be reachable at all. If neither trigger produces a non-cancel error, record "not reachable".
- **Android W1:** emulator only. The release-only path is confirmed by reading code, not by running it.
- **Web side-panel behaviour:** the e2e harness drives window mode only, so W-W5, W-D1 (closing the panel) and W-10 need a real Chrome run.
  - Branded Chrome ≥137 ignores `--load-extension`: load the extension unpacked by hand.
- **Relay faults on the app's own traffic:** on Windows, the app routed around the fault proxy, so S5/S7 were not re-run there. On the phones the proxy is the system proxy, which should work (079 used it on Android).

### 11.2 results.md "Open for the owner" 1–6 (2026-09-29) + Owner decisions D1 (2026-09-28)

Numbered as in results.md.

1. **Fee pricing (U8)** — shared core, so Android, iOS and web all price the same way.
   - Options on the table:
     - price on simulated gas instead of the padded limits (about 30% lower);
     - default speed *standard* on chains whose base fee sits at its floor, e.g. Base (34–45% lower);
     - the ×3 markup itself.
   - Do not change pricing without the owner. Only record the measured ratio per platform.
2. **H4, what a dApp sees when a phone never connected** — the desktop now keeps the request open under 重试 / 关闭, where it used to answer -32603 at once. The decision governs A-H4b, A-W19 and I-H4b.
3. **H6 desktop-first** — hedged reads shipped in the desktop pool only. Porting them to the Android/iOS pool drivers and the web pool (A-W13, I-W13, W-W13) waits for the owner.
   - Ask separately about the extension's own `forwardRead` (20 s per node). It is not the core pool, so arguably outside this decision.
4. **i18n budget** — "would fail" reuses the first clause of `componentsUi.signing.simWillFail`. A sentence of its own needs the residency budget raised. This affects U1b and U1d on all three.
5. **D5, per-user install** — Windows-only; nothing to check here.
6. **H4 remainder** — retrying a relay failure after the transaction was signed could sign the same nonce twice. Decide before any Retry is offered on the phones or the web (A-H4b, A-W19, I-H4b, I-S3b's resubmit).

**D1 (decided 2026-09-28; done on the desktop)** — not an open item, a rule to report against: only the explicit ✕ rejects (results.md "Owner decisions", D1 "Esc never answers a pending request").
- The phones already behave that way by design: Android sheets are non-dismissible (079), and iOS disables swipe on the live sheets. The web sheets are 'explicit'.
- Report what you see (Android 16 predictive back; iOS background/foreground; web panel close = 4900). Do not assume the phones must change.

### 11.3 New questions for the owner raised by this review

- Phones and new windows (A-W6, I-W6): should target=_blank / window.open open a new Vela tab (desktop parity), or keep the documented same-tab design (Android `docs/dapp-browser/ARCHITECTURE.md:91`)?
- iOS external schemes (I-W7, I-10, I-13): which schemes may leave the app, and does it need a real gesture and/or a Vela confirmation?
- Extension security (W-W4): refuse signing on insecure public origins (4100), as the in-app browsers do?
- EIP-5792 (B5792): adopt it (answer `{id}`, implement wallet_getCallsStatus with the real outcome), or keep refusing it everywhere, consistently?
- Android address bar (A-W2): keep 079's "committed host until commit", or adopt 083 FR-005, "the host being opened"?
- Android cleartext (A-9): allow http:// in the browser with the warning lock, or refuse it with its own sentence?
- iOS recovery (I-W3, I-14): add a 12–15 s watchdog and a network monitor, for desktop parity?
- Web build (W-W1): should `extension/build.mjs` run or check sync-wasm?

---

## 12. Suggested order of work

1. **Rebuild every core and prove it.**
   - Web: after the committed-build passes (§9.1 step 0a), `npm --prefix scripts run build:wasm`, then `node rust/scripts/build-web.mjs --check`.
   - iOS: `rust/scripts/build-ios-xcframework.sh`, then `check-ios-core-fresh.sh`.
   - Android: build without `-PvelaSkipRustBuild`, and grep the APK's `.so` for the 083 string (§7.1 Build step 5); About shows only the HEAD commit.
   - Record them in the results table (§3).
2. **Run the deterministic P0/P1 checks first.**
   - Android JVM: new scaffolding for S2, S3, S3b.
   - iOS XCTest: S2, S3, S3b, R3, H4a.
   - Web shim: S2, S3b, R2, B5792, U1b.
3. **Device pass per platform in matrix order** (§5), with evidence (§3). Items marked **[owner]** go into one session with the owner present: fingerprint, Face ID, the second phone.
4. **Record the results and show the owner the results table.** This is where the check pass ends.
5. **Only for the fix groups the owner approves:** open the new spec (e.g. 084), with these findings as its inventory, and change code. Suggested fix groups, each mirroring the named commits:
   1. **The receipts family.** S3b + S2 + S3 + R3 (+ B5792's answer, A-6/I-8/W-5), one change per platform. It needs the uniffi/wasm exports listed in §2.2 first. Also fix A-4 / I-6 so a failed record never reads 已确认.
   2. **Fees.** U1 + U1b + U1c (+ U1d wording).
   3. **Activity.** H2 + F1 + F3 + REC (+ A-7, A-8, W-3).
   4. **Signing-sheet security.** The stray `calls` key and single-leg rule (A-1/A-2, I-1/I-2), `input` (A-3, I-3), and the extension secure-origin rule (W-W4, after the owner decides).
   5. **Browser.** Android W1/W9/W3/W4; iOS H8/W2/W6/W7 (+ I-10/I-11); web W5/W1/W14 + W-10/W-12.
   6. **Copy and labels.** H1, H4a, W11, H5.
   7. **Waiting on the owner.** W13/H6, U8.
6. **Stop before pushing.** Ask the owner before any push or PR, and before committing regenerated core artifacts.
