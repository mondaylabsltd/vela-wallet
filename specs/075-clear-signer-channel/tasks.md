# Tasks — 075 The Clear Signer as a passkey route

## A — Core (lead)
- [x] T001 `KeyMethod::ClearSigner` through the wire (ts-rs, Kotlin, Swift mirrors); create + sign-in machines offer it
- [x] T002 Key records: `signer_origin`; `auto` routing follows the key; refusal for a foreign-origin key on another route
- [x] T003 `clear_signer` ceremony requests (create / signIn / proof / memberProof) + `verify_registration` / `verify_ceremony` + tests (each refusal)
- [x] T004 `ws::Connection`: several requests per session; `bye`; idle timeout
- [x] T005 `secure_session` (P-256 ECDH, HKDF, AES-GCM; labels `vela-relay/1`, `vela-ble/1`) + `tests/clear-signer/secure-session.json`
- [x] T006 UniFFI + wasm exports; wasm size gate

## B — Page (agent)
- [x] T010 Request kinds + cards; create only from wallet requesters
- [x] T011 Sessions of several requests (loopback WS, postMessage)
- [x] T012 `secure.js` shared by BLE and relay, against the vectors
- [x] T013 Relay transport, `rk` check, code screen
- [x] T014 Hostile tests: foreign challenge, create from a site, stand-in wallet

## R — Relay (agent)
- [ ] T020 `vela-relay` rules + unit tests
- [ ] T021 `vela-relay-server` + Dockerfile
- [ ] T022 `vela-relay-worker` (Durable Object, hibernation)
- [ ] T023 Conformance on native, Worker (wrangler dev), Docker

## A+ — after the contracts (lead)
- [x] T007 `sign_pref`: the relay is a preference (`vela.clearSignerRelay`), with its own rules and refusals
- [x] T008 i18n: where the signer is, the pairing sheet, the code, the relay row — all fifteen locales (pin 1717 → 1733)
- [x] T009 The page's ceremony suite also judged by the real core (`clearSignerVerifyCeremony`), 61/61

## C — Shells (agents)
- [x] T030 Web: the fourth route in create / sign-in / backup; postMessage sessions; relay pairing sheet
  — `AddMethodPicker` lists four (create's first key, "add another", the sign-in
  sheet); the onboarding executor routes `method = clear_signer` to the page and
  reports the core's verdict; one page visit per flow (create → member proof,
  sign-in → proofs), ended with `bye` when the flow is; the relay requester runs
  on WebCrypto, pinned to `tests/clear-signer/secure-session.json`; the sheet
  asks where, draws the QR + link + the six digits, and sends nothing before the
  confirm; a key with `signer_origin` is signed on ITS page, `auto` included;
  Settings' relay row in both layouts. 1577 unit tests; e2e: create through the
  page, sign in again, a signature for a key behind the page, and the same create
  across two pages over the mock relay.
- [x] T031 Android: same, loopback WS sessions, relay pairing sheet, Settings relay row
  — the fourth row in both choosers and the sign-in sheet, ceremonies routed to
  the page with the core's verdict, Settings' page + relay rows. 662 unit tests.
- [x] T032 iOS: same — `KeyMethod.allCases` puts it in every chooser, one session
  per flow, the relay requester pinned to `tests/clear-signer/secure-session.json`.
  737 → 757 tests; a simulator UI test walks create and sign-in to the where-choice.
- [x] T033 Desktop: same; loopback WS replaces fragment + callback — `ws_launch`
  carries several requests per visit (a create and its member proof cannot use
  one-request-per-visit). 500 → 518 tests, plus 5 Chrome e2e and a real-relay case.

## D — BLE
- [ ] T040 Android peripheral · T041 iOS peripheral · T042 desktop (macOS) peripheral · T043 real-radio pass

## E — Device passes
- [~] T050 SC-002 on the Android phone — run 2026-09-22 on the connected phone
  (build `d1e77fe`), page on `http://localhost:8140/` over `adb reverse`, passkey
  index pointed at a scratch mock so no test key reached the owner's index, and
  a stand-in authenticator installed in the page (Chrome on Android exposes no
  WebAuthn DevTools domain, and a real passkey would need a fingerprint).

  | # | Result |
  |---|---|
  | E1 create through the page | **pass** — the Custom Tab opens it, the card names the wallet, says nothing is signed and that the key will belong to `localhost`; the key comes back and the draft row reads 清晰签名器 |
  | E2 member proof | **pass only with the rpId gap worked around** — see F-1 |
  | E3 the key's row names its page | **fail, then fixed and re-run** — the row now reads 清晰签名器 · 95a0…263d and its detail names `http://localhost:8140`, while a platform key's row is unchanged (核心 `28239a5c`, Android `8f0888e5`) |
  | E4 send from that wallet | not run — the test wallet has no funds and none were moved to it |
  | E5 sign out, sign in through the page | not run — sign-out clears every account on the device (see F-3), and the owner's other passkeys cannot be re-authenticated from here |

  Refusals seen, both honest: with the registry unreachable the page refuses —
  「注册表没有应答，这把钥匙的挑战码无从核对。请回到钱包重试。」 — and the wallet
  says 「清晰签名器拒绝签署这笔请求，原因见它的页面。」 A reply the wallet cannot
  match gives 「清晰签名器的回复与这笔请求不符，什么都没有发出。」

  The whole path then completed: key created on the page → member proof signed
  there → published (the mock recorded the unit) → 钱包已创建, address
  `0x57e9498FbEa4406a01142DF6ff2627E468f7912F`, the key row reading 云同步.
- [ ] T051 SC-003 across devices (relay: native + Worker)

## F — What the device pass found

- **F-1 · a key minted on a self-hosted page cannot finish its member proof.**
  The wallet fetches the member challenge under its OWN rpId while the page
  derives it under the page's host, so the bytes differ and the wallet discards
  its own key's proof. Captured in one exchange, same key, same registry:

  ```
  POST /api/challenge  {"rpId":"getvela.app","publicKey":"0495a0e9…","groupPublicKey":"04305f4f…"}   ← the wallet
  POST /api/challenge  {"rpId":"localhost",  "publicKey":"0495a0e9…","groupPublicKey":"04305f4f…"}   ← the page
  ```

  A page cannot claim another domain's rpId, so a key created on a self-hosted
  page necessarily belongs to that page's host; the registry must learn that
  rpId per member. Android passes `passkey.relyingPartyId` at both call sites
  (`OnboardingExecutor.kt:191`, `:352`); the web reported the same from
  `publish.ts`. The official page shares `getvela.app`, so only self-hosted
  pages — a supported setting — are affected. The fix has two halves: a core
  rule deriving a member's rpId from its `signer_origin` (the core already
  stores it), and a registry `/api/challenge` that takes rpId per member — the
  group mode sends one for the whole set, which cannot serve a mixed set.
- **F-2 · the key list never names the page a key lives behind.** ~~After
  creation the row and its detail caption a Clear Signer key 内置通行密钥 (the
  authenticator's own report — a page reports `platform`), and nothing shows
  `signer_origin`, which the core does store.~~ **Fixed.** The rule moved into
  the core (`28239a5c`): a `signer_origin` outranks the authenticator's report
  in both row builders, and `WalletKeyRow` carries the origin so a row can name
  the page. A shell still has to pass each key's origin INTO the keys view —
  Android did not (`8f0888e5`), and the same one-field gap was handed to the
  other three shells. Re-run on the phone: the row reads 清晰签名器 and its
  detail names the page.
- **F-3 · 退出登录 clears every account on the device.** `clearSignedInWallet()`
  removes `vela.accounts` and `vela.activeAccountIndex` wholesale, while the
  sheet says 「地址不变，交易记录、联系人、代币和各项设置也都还在」 — true of one
  wallet, misleading on a device holding six. There is also no way to remove a
  single account.
- **F-4 · a Settings endpoint change does not reach a flow already in memory.**
  ~~`OnboardingViewModel.init` reads the passkey index once; changing it in
  Settings and creating a wallet without restarting still queries the previous
  index (observed: the page was handed the production index after the change).~~
  **Fixed on Android** (`8f0888e5`): re-read on entry, never mid-flow, so one
  wallet is still never asked of two registries. Worth checking on the other
  three shells.
