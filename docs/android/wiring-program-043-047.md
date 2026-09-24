> **History (2026-09-23).** This is the plan as it stood at the close of spec 042, written in the
> present tense about an Android shell that could not yet send, sign or open a dApp. **Specs
> 043–049 delivered all of it**, each verified on the device: Android now drives `send`,
> `fee_policy`, `fee_speed`, `tx_tracker`, `sign_request`, `clear_signing`, `approval_guard`,
> `dapp_permissions`, `explore_sites`, `browser_history`, `batch_import`, `contacts_io`, the
> scanner and the simulation deltas — see `app-android/vela-wallet/app/src/main/java/app/getvela/wallet/`
> and `docs/PARITY-2026-09-21.md`. The machine counts below are also out of date (the core now has
> 33 modules under `rust/crates/vela-core/src/app/`). Kept as the record of the program's shape and
> its device rule, which still stands.

# The Android wiring program, 043–047

**Written**: 2026-09-12, at the close of 042 (the merge with `main`).
**Owner**: the Android line (`/Volumes/data/production/vela-wallet-android`).
**Rule of the program** (founder, 2026-09-12): every feature is verified on the
connected Xiaomi (`alioth`, serial `9d5f42fb`) before it is called done.
"没有验证，容易写出错误的不符合需求的东西." A phase ends with a screenshot,
not with a green test.

## Where Android stands

The core has 26 machines under `rust/crates/vela-core/src/app/`. Android
drives 13 of them (040: `display_currency`, `network_admin`, `contacts`;
041: `rpc_pool`, `balance_dashboard`, `activity_feed`, `token_trust`,
`receive_watch`, `payment_request`; onboarding: `create_wallet`, `login`,
`session`; plus a `manage_tokens` adapter nothing instantiates). The web drives
every one it needs; the desktop reached the same point through seven specs,
030–037, and 038 for the first run.

What Android does not drive, grouped by what a person cannot do yet:

| A person cannot… | Machines absent on Android | Shell capability it needs |
| --- | --- | --- |
| send money | `send` (40 events / 18 ops), `fee_policy`, `tx_tracker`, `manage_tokens` (finish) | passkey assertion (exists: `PasskeyExecutor.assert`), relay HTTP, timers, notification |
| sign for a dApp | `sign_request`, `clear_signing`, `approval_guard` | a transport that carries requests in and answers out |
| use a dApp in the app | `dapp_permissions`, `explore_sites`, `browser_history` | an in-app WebView with the provider script injected |
| pay many, import a book | `batch_import`; `contacts_io` events | a file picker (SAF), share sheet |
| point the camera at a QR | `send::OpenScanner` / `ScanResolved`; `network_admin::AddByChainIdRequested` | camera + barcode decoder |
| see what a signature will do | `token_trust::SimDeltasComputed` | `eth_simulateV1` through the pool (no new capability) |

Two machines are excluded on purpose: `ext_cache` (Safari App Group, no Android
counterpart) and `dapp_session` (WalletPair; the founder ruled it out for the
desktop on 2026-09-08 — the same ruling is assumed here and recorded as a
question, not a task).

Drawn but not live (from 021/022/023, listed in `VelaNavHost.kt`): every send
screen and sheet (`SendPick/Form/Confirm/Receipt`, `FeeToken`, `ContactPick`,
`AddToken`, `BatchImport`, `Scan`, `Share`), the 探索 tab and the `EXPLORE`
route, the dApp signing sheet, the settings rows that need a network (RPC
health pills, storage figures, relayer panel), the language/format sheets, the
`IMPORT` placeholder route. Not drawn at all: the contact add/edit form and the
favourite control (040 debt).

## The five specs

Each mirrors a web/desktop spec so its plan can be lifted rather than invented;
each is independently shippable; each ends with a device pass. Order follows
what a person needs first, and what the founder's 041 pointer named: send,
sign, browser.

### 043 — Android Money Wiring (mirrors 026 phases 1–4/6 and 032 phases 1–4/6)

A signed-in person sends one token to one address and watches it confirm.

- **Enabling first**: the parallel space on Android — `vela-core`'s fixed
  keyset (`dev_fixtures.rs`, the same scalars web and desktop use) behind a
  debug-only gate, so every later phase can be driven end to end by `adb`
  without a finger on the sensor. The real passkey path is the same code with
  one substitution, verified once by the founder.
- `send` in single mode: pick token → form → `Continue` (quote ∥ treasury
  pre-check) → confirm → assertion → `SubmitUserOp` → `PersistTxRecords` →
  `TrackSubmitted`. `fee_policy` for the quote and the fee-token sheet.
  `tx_tracker` for the receipt and `NotifyConfirmed`. The feed shows the row
  at submit, not at confirm (026's pending-at-submit rule).
- `manage_tokens` instantiated: the AddToken sheet live. `ContactPick` live on
  the contacts machine. The two `// live in 042` arms backfilled
  (`ResolveIdentity` ENS waterfall, `ClearBundlerCache`).
- Not here: split/sweep/batch, the scanner, dApp signing.
- **Device**: dust moves from the Xiaomi's wallet — the parallel-space Safe
  first, then the founder's own passkey once.

### 044 — Android dApp Browser and Signing (mirrors 027 + 032 phases 13–14 + the signing half of 026)

A dApp opened inside the app can connect, read, switch chain, and sign — and
the person sees what they sign.

- An in-app `WebView` with 027's inpage provider script injected (EIP-6963 /
  1193, MetaMask-compat), a transport that carries requests to the core and
  answers back; reads proxied through the person's own pool.
- `dapp_permissions` (consent sheet, connections list, revoke, account/chain
  change events), `explore_sites` live in the 探索 tab (favourites, groups,
  tabs), `browser_history`.
- `sign_request` + `clear_signing` + `approval_guard` on the drawn signing
  sheet, submitting through 043's passkey → relay → tracker spine. 027's
  unmet SC-304 ("connect but cannot sign") is the failure this spec exists to
  not repeat.
- **Device**: a real swap on a real dApp in the built-in browser (memory:
  PancakeSwap took ~12 s on the Expo build), the consent sheet naming the
  signed-in account, an unlimited approval refused by the guard.

### 045 — Android Send Parity, Batch and Contacts Parity (mirrors 033 + 034 + 026 US3 + 028's batch/contacts)

Everything the web can do with money and people that the phone still cannot.

- Split (1 token → N people, editable rows), sweep (N tokens → 1 address),
  `batch_import` from a picked xlsx/CSV (SAF; the parse stays in the core),
  the relay-treasury sheet, `Requote` on a stale fee.
- Contacts: import/export through SAF and the share sheet (`contacts_io`),
  group membership both directions, recipient inspection on open — and the
  add/edit form and favourite control, which must be **drawn** (018's Penpot
  contacts spec is the source) before they can be wired.
- **Device**: a two-row split lands as one UserOp; a CSV round-trips through
  the share sheet and back.

### 046 — Android Signing Depth: Simulation, Message Signing, Scanner (mirrors 035 + 036 + 037)

- `eth_simulateV1` through the pool → `token_trust::SimDeltasComputed` → the
  Balances block on the signing sheet and on Send confirm (never a write from
  a simulation — founder ruling).
- `personal_sign` / `eth_sign` → the MessageSign surface (SIWE, domain
  binding, danger class).
- The scanner: camera + barcode decode for `send::OpenScanner` /
  `ScanResolved` (EIP-681 with both refusals), a picked image, add-network by
  QR (`AddByChainIdRequested` is the scan path; the list path is
  `ChainSelected` — 041's finding).
- **Device**: a QR held to the lens resolves; a SIWE sign-in on a real site;
  the balance-change block agrees with the receipt.

### 047 — Android Port Completion and First-Run Parity (mirrors 028 + 038)

The residue, then the rulers.

- Settings rows that still read fixtures (RPC health and latency pills,
  storage figures, relayer panel, language and format sheets, accounts-sheet
  rows, per-network RPC override selection); the `IMPORT` placeholder; the
  receive share card as a PNG through the share sheet; `/pay` and
  `velawallet://` deep links; erase device; the first-run items 038 settled
  (launch gate, panic sheet, offline/version line).
- The two rulers, re-run: the "web as the checklist" event sweep to zero
  strong diffs, and the "dropped judgement" grep (view fields the core
  computes that Android never reads).
- A full device pass of 040–047, `docs/KNOWN-BUGS.md` and the takeover docs
  updated, the Android install/verify loop written down for the next person.

## Sizing, from the mirrors

| Spec | Nearest mirrors | Their size | Expected here |
| --- | --- | --- | --- |
| 043 | 026 (47 tasks), 032 ph 1–6 | large | large: one 40-event machine |
| 044 | 027 (37 tasks) + 032 ph 13–14 | large | large: WebView + three machines |
| 045 | 033 + 034 (3 + 3 phases) + 028 batch | medium | medium, plus drawing |
| 046 | 035 + 036 + 037 (1 + 2 + 4 phases) | medium | medium: camera is new on Android |
| 047 | 028 (67 tasks) + 038 (106 tasks) | large | medium: much of 038 landed in 019/020 |

## Standing rules, carried from 040–042

- A machine costs an executor and a display-model builder; the road in
  `core/crux/` does not change. Kotlin that starts deciding is a bug.
- Views may be subsets; operations, results and closed error families must be
  exhaustive (`CoreWireDriftTest`). A new core variant that Android cannot
  decode is an exception at runtime, not a wrong message.
- Any Rust edit — `cargo fmt` included — moves the web artefact's fingerprint;
  rebuild it last. Kotlin bindings regenerate from the merged core.
- Gradle runs under the Android Studio JBR; the shell's default JDK has no
  `jlink`, and a daemon started under it must be `--stop`ped.
- "A drawn screen is not a wired screen": tap every tappable thing on the
  device. 040 and 041 each found surfaces this way that no test could.
