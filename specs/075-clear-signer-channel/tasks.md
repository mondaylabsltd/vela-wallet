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
- [x] T020 `vela-relay` rules + unit tests — the room rules as a crate
  (`crates/vela-relay`, in the workspace, three test modules)
- [x] T021 `vela-relay-server` + Dockerfile — the native host, distroless image
  (`690963d7`)
- [x] T022 `vela-relay-worker` (Durable Object, hibernation) — its own workspace,
  wasm32 through worker-build (`b388e836`, docs `8e4accac`)
- [x] T023 Conformance on native, Worker (wrangler dev), Docker — the desktop's
  `relay_conformance_against_a_real_relay` runs against a real one (`--ignored`)
  **Nothing is deployed**: `sign.getvela.app` and a public relay are the owner's
  to put up, so the cross-device route works against a relay you run, not one
  that already exists.

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

The framing is the core's (`clear_signer::ble`, commit `8da4384e`): three
peripherals speak to one page, and three hand-written reassemblers would be
three chances to disagree about a wrapped `msgId`, a frame that arrived twice,
or a message that never finished. `tests/clear-signer/ble-frames.json` holds six
cases that BOTH sides read — this crate, and `samples/ble-vectors-test.mjs` with
the page's own code.

- [x] T040 Android peripheral — `BluetoothGattServer`, the service advertised
  with the device name in the scan response (a 128-bit uuid eats 18 of the 31
  advertised bytes), notifications serialised on `onNotificationSent`. 663 → 678
  tests. It does NOT rename the Bluetooth adapter: `setName()` renames the phone
  for every app and every paired device.
- [x] T041 iOS peripheral — `CBPeripheralManager`, the same service, the
  foreground rule said on screen rather than left to be discovered (a
  backgrounded iOS app loses its local name and drops into the advertisement's
  overflow area). 758 → 781 tests. Two bugs its own tests found: a request
  marked in flight *after* its frames went out (an answer that arrived instantly
  was dropped), and a discarded `sweep` result (a lost frame left a spinner with
  no clock behind it — this channel has no socket to die).
- [ ] T042 desktop (macOS) peripheral — not started. Lower value than it looks:
  the desktop already reaches the page over the loopback socket on the same
  machine and over the relay across machines, so BLE only adds a third road to
  the same place, and a peripheral role from Rust needs raw `objc2-core-bluetooth`.
  Worth a ruling before anyone spends the day on it.
- [x] T043 real-radio pass — **Android, 2026-09-22**: advertise → Chrome's
  chooser → GATT connect → MTU 517 → handshake → matching six digits →
  request → answer → `verdict` → `core.result passkey_registered`. Five
  defects fell out of it, below.
  **iOS, 2026-09-23** (iPhone 11, iOS 26.5.2, page at `http://localhost:8140`
  in Chrome on the Mac): the phone advertised, Chrome's chooser found it —
  under the system's own name for the phone, not our `Vela · <name>` local
  name, which is worth knowing for anyone driving the chooser — connect,
  handshake and the six digits (`079298`) matched what the phone showed, and
  the create request crossed and drew its card. The run then ended with the
  member proof unanswered (`received: 1, answered: 0, endReason: bye`) because
  the harness's deadline passed between the two cards, not because anything on
  the link failed. The last leg — both cards confirmed in one session, the
  answer back on the phone — is still to run.
  Two stand-ins, as on Android: CDP answers the native device chooser, and the
  page's authenticator is `softauth.js` (a real passkey there would want a
  finger on the Mac). The radio, the MTU, the CCCD subscription, the frames,
  the handshake and the digits are all real.

### What the framing's first users found

- **A notified frame has to fit the link whole** (`1bcb3766`). 244 is the page's
  write size, and 244 + the six-byte header is a 250-byte ATT value that an
  MTU-247 link cannot carry. A central gets away with it — the OS splits an
  oversized write into a long write — but a peripheral's notify cannot be split:
  `updateValue` truncates, and a truncated frame is a message that never
  completes. iOS hit it; Android then found its own ladder had never taken a
  step (`room = mtu - 3` was 244, the framer already sat at 244), so **every
  multi-frame answer it sent would have been truncated on every modern phone**.
  Since answers go out over notify, that is the direction carrying every
  signature. Now `Framer::fit_to_mtu`, once, for all three.
- **The peripheral's own units** (`555e53fc`). `CBCentral.maximumUpdateValueLength`
  is the notification's capacity, not the MTU, and two of the three peripherals
  are CoreBluetooth. `fit_to_value_len` is the door for that number; handing it
  to an MTU-shaped door costs three bytes a frame and breaks nothing, so nothing
  would ever find it.
- **Copy that can be acted on** (`4b9c6d80`, `555e53fc`). The peripherals shipped
  borrowing the dApp flow's "Bluetooth permission is needed", which never said
  which device to pick out of the browser's list — the one thing the pairing step
  cannot work without. Six sentences now, in fifteen locales, including one for a
  device with no peripheral role at all, which was being told to switch Bluetooth
  on.

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

- **F-5 · a route that cannot add to this set was still offered.** Found by the
  owner, 2026-09-23: with a first key on `getvela.app` (this device) and the
  Clear Signer page set to `http://localhost:8140`, the Clear Signer row was
  still live. A fourth key was minted there, and the failure surfaced only at
  the publish — the person was already holding a passkey no unit would accept.
  The rule shipped as "a `getvela.app` set takes every route", which is true of
  the OFFICIAL page and false of the page Settings names. **Fixed:**
  `methods_for(drafts, signer_page)` now asks where the Clear Signer WOULD mint
  (`registry_rp_id` of the configured page) and offers it only when that equals
  the set's committed party; `Event::SignerPageChanged` is how a shell reports
  the setting, and `add_blocked` carries the two facts a sentence needs. All
  four shells dim the row and print it
  (`onboarding.create.methodBlocked{Hint,Signer}`, 15 locales). Pinned by
  `a_signer_page_on_another_domain_is_off_for_a_getvela_set` and by the
  narrowed-picker case in `add-method-picker.svelte.test.ts`; the gallery of
  each shell gained `keys · signer page elsewhere` and `keys · a page's own set`.
- **F-2's tail on the CREATE screen (Android).** The wallet-keys list took the
  core's `kind`; the founding-key list never did — the Kotlin `CreateKeyRow`
  mirror had no such field, so a key minted on a page was drawn with this
  device's vault ("Apple Passwords", 设备绑定) on the one screen where the
  person is deciding what the set is made of. Fixed with the rest of F-5: the
  mirror carries `kind` (defaulting to `method`), and the row's mark, provider
  line and glyph all read it. The gallery fixtures stopped lending a page's key
  an AAGUID at the same time, in all three shells.
