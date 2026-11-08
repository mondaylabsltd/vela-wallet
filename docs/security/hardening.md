# Hardening: closing the doors (Round 21)

Round 21 closes the code items of the 2026-11-07 core-dev security review that the
earlier P0 work left open: review items 1-6 (relay power), 1-8 (telemetry), 1-1 (web origin
hardening), the 0-1 follow-up (a test RP ID), and SEC-PARENT-2 from `docs/KNOWN-BUGS.md`.
The disclosure process (`SECURITY.md`, `security.txt`, advisories) is a parallel lane.
`docs/multisig-machines.md` §35 is the summary and links here.

Every decision follows the project's rules: the core owns each rule and the shells render it,
the platforms stay at parity, states are honest, and no service is a trust premise.

---

## 1. Every SafeOp expires (review 1-6)

**Before.** Every SafeOp was signed with `validAfter = validUntil = 0`, in the hash
(`user_op.rs calculate_safe_op_hash`) and in the signature's first 12 bytes. A relay could
therefore hold a signed operation and land it at any time: a swap at the worst moment inside
its slippage, or a transfer the person thought had failed, days later.

**Decision.**
- **`validUntil` = now + 30 minutes** (`user_op::SAFE_OP_LIFETIME_SECS`). Every attempt is
  stamped just before its hash, so a re-sign gets its own window.
  - *Long enough* for the slow honest cases: a passkey prompt left open, a hybrid
    (phone-as-key) ceremony, a relay queue on a congested chain, a device clock a few minutes
    behind. Below about 10 minutes, a hybrid ceremony on a busy queue would expire honest
    operations.
  - *Short enough* that "I pressed Send, it did not go, I walked away" cannot become a
    landing hours later. Above an hour the window no longer means anything to a person.
  - The relay waits up to about 35 minutes for fees before it gives up. An operation it holds
    that long now expires in its queue instead of landing late, which is the point.
- **`validAfter` = 0.** A lower bound buys nothing for an operation submitted at once (the
  wallet never signs one for later). It also adds a failure mode: a device clock ahead of the
  chain would make every operation "not due" (AA22) until the chain caught up.
- **One rule.** `user_op::validity_window(now_ms)` and `stamp_validity` are the rule; the
  shells only bring the clock (uniffi `user_op_stamp_validity`, wasm `userOpValidityWindow`,
  desktop by path).
- **Nothing signs an op that never expires.** `signable_safe_op_hash` refuses
  `validUntil = 0`. It sits behind uniffi `user_op_safe_op_hash` /
  `user_op_reviewed_safe_op_hash`, wasm `attestSafeOpHash` / `attestReviewedSafeOpHash`,
  `envelope_signature`, and the signing page's digest. A path that forgot to stamp signs
  nothing, rather than an operation a relay could land at any time. Estimates (the dummy
  signature) keep a zero window: they are never signed, and the relay's simulation must not
  refuse them as expired.
- **Coverage.** Every operation the wallet signs goes through one spine per shell: plain
  sends, a multisig's execution (`SendCalls`), dApp transactions, `wallet_sendCalls`, and
  account deployment (the `initCode` on the first operation).

**The contract, verified in code, not docs.** Safe4337Module v0.3.0 `_getSafeOp` reads
`validAfter = bytes6(signature[0:6])` and `validUntil = bytes6(signature[6:12])` and hashes
both into the SafeOp. `_validateSignatures` returns
`_packValidationData(failed, validUntil, validAfter)`, so the EntryPoint enforces the time
range (`AA22 expired or not due`). The module's NatSpec lists the packed word high bits
first (`validAfter ‖ validUntil ‖ authorizer`). Certora's note that Safe's README example
swaps the two fields was checked the same way: against the code, and on the fork in both
directions (below). The relay (vela-relay) never reads or rewrites the 12 bytes. Admission
only requires a non-empty signature, and execution simulates a real `handleOps`, where the
EntryPoint applies the window.

**Honest state.** The EntryPoint's AA22 is read by the core as `RelayRejection::Expired`:
not sent.
- Send: `SendTxErrorKey::Expired` → "Not sent", calmly, with "It expired before it was
  submitted — nothing was sent." (`componentsUi.signing.expired`), and Review again signs anew.
- The signing sheet says the same calmly and offers Try again (`failure_not_sent`,
  `failure_retryable`). The page hears `EXPIRED_DETAIL`.
- The tracker names a relay's `expired` refusal the same way (`RefusalReason::Expired`).
- The signing page shows "Valid until <date>" in the fee note and refuses an operation with
  no expiry (`refuse.noExpiry`) or an unreadable window (`refuse.badWindow`).

**Proof on the fork** (`rust/crates/vela-core/tests/safe_op_window_fork.rs`, 4/4 on a fresh
harness). Each case runs a fixture account's first operation, which deploys through
`initCode`, signed by the fixture key over the core's hash and enveloped by the core:
1. `Safe4337Module.getOperationHash` equals the core's hash for windows
   `0..0`, `0..1800001800`, `1700000000..1800000000` and `0x010203040506..0x0a0b0c0d0e0f`.
2. The wallet's own window (stamped at the chain's now):
   - one second past `validUntil` → `handleOps` reverts `AA22 expired or not due`, the Safe
     is not deployed, and the core reads the reason as `Expired` / NotSent;
   - at `validUntil` exactly → lands;
   - a minute in → lands.
3. A window that opens in an hour and closes in two:
   - submitted now → AA22;
   - past `validAfter` → lands;
   - past `validUntil` → AA22.
   With the fields swapped it could never be valid, so this proves the order.
4. Through the harness relay (`eth_sendUserOperation` → `handleOps`) the 12 bytes arrive as
   signed and the operation lands.

**Left open.**
- The onchain `send-press` suite (§34) needs a harness of its own: it needs Parallel Three
  undeployed, and `multisig.onchain.ts` deploys it. Run together on one harness, one of
  them fails for the harness's reason. R21 ran each on a fresh harness; both pass.
- The tracker says "expired" only when the relay names `expired` as the
  `rejection_reason`. A relay that reports an expired operation as a simulation failure is
  told as the network's refusal ("nothing was sent"), which is honest but not specific
  (vela-relay go-live item G5).
- A shell clock more than 30 minutes behind the chain makes every operation expire before
  submission. That is said honestly each time, but the wallet does not diagnose the clock.

## 2. The relay's settlementGas is capped (review 1-6)

**Before.** The in-band fee is `settlementGas × inBandFeePerGas`. The relay's
`settlementGas` was checked only for `> 0`: it was shown but never refused.

**Decision** (`fee_policy::settlement_gas_cap`, applied in `accept_gas_outcome` to a
published bill). A quote is refused (`FeeFailure::SettlementGasTooHigh`) when
`settlementGas` exceeds the smaller of:
- the gas limits the operation is signed with: gas it can never use. A legitimate relay's
  bill is `min(used + 15% + 30,000, max(declared, used))`, never above its own declared
  limits. That is a factor of 1.0 with no margin.
- **125% of the wallet's own gas model + 50,000** (`static_limits_for`: the verification
  constant for a deployed or undeployed Safe, the call limit raised to the measured inner
  floor, the pre-verification constant, and the L2 data adders).
  - 1.25 is the relay's documented 15% billing buffer over a simulation it measured within
    about 6% (1.15 × 1.06 ≈ 1.22, rounded up).
  - 50,000 is the relay's 30,000 constant, plus the 10,000 EntryPoint overhead it adds,
    plus slack.

This bounds a relay that inflates its limits along with its bill; the signed limits cost
the person nothing under in-band, so they alone would not. A contract call the wallet could
not measure has no own model, and then only the signed limits bound the bill.

**Recorded quotes still pass** (vector `tests/vectors/rules/settlement-gas-cap.json`,
16 cases, replayed by the core rule and through the fee machine). The real quotes sit at
0.27–0.77 of the wallet's model:

| Quote | settlementGas | Wallet's model |
| --- | ---: | ---: |
| Ethereum send | 208,070 | 600,000 |
| USDT send | 249,371 | 600,000 |
| Swap | 365,061 | 682,863 |
| Undeployed first operation | 610,301 | 2,300,000 |
| Registry backup | 5,831,838 | 7,595,156 |
| Gnosis first send (synthetic native-parity case; no real Gnosis quote is on record) | 150,000 | 2,300,000 |

Arc's relay publishes no settlementGas, so its quote is priced on its limits and the cap
does not apply.

**Words.** `componentsUi.gas.reasonSettlementHigh`: "The relay asked to charge for more gas
than this transaction can use — Vela won't accept that quote. Retrying automatically." The
`…Stopped` form is the same without the last sentence. The quote is asked again on the relay
schedule, because another quote may be honest.

**Left open.** On a relay older than CONTRACT v2 (no settlementGas, `Charge::Marked`), the
fee is the basis × the signed limits, and those limits are the relay's. They are not capped
against the wallet's model here; that is an R22 candidate.

## 3. Usage statistics are opt-in (review 1-8)

**Decision.**
- Product analytics in the web wallet and the extension are **off by default**.
  `analytics/consent.svelte.ts` counts only an explicit `vela.analytics = on`. Before R21 the
  default was on and only `off` was written, so a browser that never chose carries no value,
  gets the new default (off), and is never reported. One that switched it off stays off.
- **Every transaction-outcome event is removed**, so no event, with its network, its time
  and the IP every web request carries, can be matched to an on-chain transaction:
  - `send_submitted`, `send_confirmed`, `send_failed` (the tracker watch that fed them is
    deleted);
  - `backup_keys_started` / `_done`;
  - a multisig proposal's approval;
  - a site's request whose kind becomes a transaction (`send_transaction`, `send_calls`).
  `catalog.ts` `TX_OUTCOME_EVENTS` and `TX_REQUEST_KINDS`, held by `payload.test.ts`.
- Words:
  - the Settings subtitle, fifteen locales: "Off unless you turn it on. Only which screens
    open and which steps finish — never addresses, amounts, names or transactions.";
  - getvela.app's privacy page (opt-in, the real Settings path, nothing about a
    transaction);
  - requirement A03 (the opt-in exception, FR-6, AC-3, AC-4).

The native apps had none and have none.

**Left open.**
- An AI agent's `Preference::Analytics(true)` is still a `Manage`-risk intent that the person
  approves. It is the person's approval, but an R22 review should decide whether an agent
  may ever turn statistics on.
- The whitepaper's Privacy section still mentions only the website's analytics. Its R23
  rewrite should say the wallet's are opt-in.

## 4. Origin hardening (review 1-1)

**wallet.getvela.app** (`app-web/vela-wallet/src/lib/security-headers.ts`, the one list):

| Header | Value and why |
| --- | --- |
| CSP (per page, `<meta>`) | **Hash-based.** SvelteKit `kit.csp` `hash` mode, hosted build only. Every page is prerendered, so a nonce cannot work. Kit hashes its own inline scripts, and `app.html`'s one render-blocking script is hashed from the file at build time, so an edit to it can never ship unhashed. `script-src 'self' 'wasm-unsafe-eval'` + hashes: **no remote script origin**, no `eval`. `'wasm-unsafe-eval'` because the core, PeerConnect and the QR decoder are WebAssembly. `object-src`, `frame-src` `'none'`; `base-uri`, `form-action`, `worker-src`, `manifest-src` `'self'`. `connect-src 'self' https: wss: http://localhost:* http://127.0.0.1:* ws://localhost:* ws://127.0.0.1:*`: a person may point the wallet at their own RPC, relay or index, or accept a site's network, and the core's `is_allowed_endpoint_url` decides which. `img-src 'self' data: blob: https:` (remote logos, wallet icons). `style-src 'self' 'unsafe-inline'`: Svelte renders `style:` bindings into prerendered HTML, and styles cannot run code. |
| `Content-Security-Policy` (header) | `frame-ancestors 'none'; object-src 'none'; base-uri 'self'`. A `<meta>` cannot carry `frame-ancestors`. |
| `X-Frame-Options` | `DENY` (browsers before CSP 2). |
| `Permissions-Policy` | `publickey-credentials-get/create`, `camera` (QR), `clipboard-write`, `web-share`, `screen-wake-lock` (the connect page) for **self**. `microphone`, `geolocation`, `payment`, `usb`, `serial`, `hid`, `bluetooth`, `midi`, `display-capture`, the motion sensors and `browsing-topics` are `()`. |
| `Referrer-Policy` | `no-referrer`. |
| `X-Content-Type-Options` | `nosniff`. |
| COOP | **Not** `same-origin`: the signing page is a window the wallet opens and answers by `postMessage`, which needs `window.opener`. |

The headers are served from `_headers` (static assets and prerendered pages) and from
`hooks.server.ts` (the Worker's responses: kit's own CSP is kept, and an immutable redirect
is rebuilt). The extension's pages carry the manifest's CSP and no page CSP; their inline
scripts are moved to files by `extension/build.mjs`. The hosted parallel space is a
prerendered route on the same origin and gets the same headers.

**getvela.app** (`app-web/getvela.app/src/lib/security-headers.ts`): passkeys are denied on
every page (`publickey-credentials-get=(), publickey-credentials-create=()`, as since spec
080; no page of the site uses WebAuthn). Every page also carries `frame-ancestors 'none'`,
`X-Frame-Options: DENY` and `nosniff`, in `_headers` and on every Worker response (the bug
report's own CSP is kept, with framing appended).

**Checked under the headers.** The whole e2e suite runs against `e2e/preview.mjs` (the
Worker bundle on workerd with `_headers` applied). `e2e/security-headers.e2e.ts` asserts:
- a prerendered page and a Worker response carry the headers;
- the page CSP is hash-based;
- the wallet boots its wasm core with no CSP violation.

**Go-live: the other `*.getvela.app` hosts** (in other repositories; not edited here). Each
serves pages or responses under the passkeys' registrable domain. A script running on any
of them could ask for getvela.app passkeys, so each must send
`Permissions-Policy: publickey-credentials-get=(), publickey-credentials-create=()`. The
anti-framing pair (`frame-ancestors 'none'`, `X-Frame-Options: DENY`) is wise on all of
them too.

| Host | Service | Repository |
| --- | --- | --- |
| `vela-relay-cf.getvela.app` | the relay (bundler, quotes, sponsor) | vela-relay (`vela-relay-cf`) |
| `p256-index-v2.getvela.app` | the passkey key index | p256-index (`p256-index-server`) |
| `ethereum-data.getvela.app` | chain metadata, token lists, logos | ethereum-data |
| `safe-genesis.getvela.app` | the multisig genesis service and mailbox | safe-genesis-registry / sgr-mailbox (`genesis-server`) |
| `vela-currency.getvela.app` | fiat rates (Frankfurter) | vela-currency |
| `sign.getvela.app` | the signing page | **this repo**: `app-web/trusted-signer/dist/_headers` already sends `frame-ancestors 'none'` + `X-Frame-Options: DENY`. It is the one host that USES passkeys, so it must keep `publickey-credentials-*` for self. |

## 5. A connected multisig is held to the self-call rule (SEC-PARENT-2)

**Before.** A site connected to a multisig was held to the parent-Safe rule (§31), but not
to the self-call list for the multisig's own address. It could propose an owner, threshold
or module change of the multisig itself. Members would still review it, but a site has no
reason to ask.

**Decision.** The account's own self-call rule (`self_call_guard`) now applies at the
multisig's address:
- the owner, threshold, module, guard, fallback-handler and setup calls;
- `execTransaction` on itself;
- a delegatecall leg;
- any of these through `multiSend` and `execTransaction`.

A site's request is refused at **arrival** (`HeldStage::Refused`, nothing drafted), and
again at the **proposal step**, on the very transaction about to be signed. A site's
`Calls` draft that reached the proposal step without passing arrival is `SigningBlocked`.
Both use the same words: the sheet's "Vela can't sign this" title over
`componentsUi.signing.multisigSelfCallBlocked`, "This site asked this multisig to change who
controls it ({{function}}). Vela never lets a site do that — members change it from the
multisig's own screen. Nothing was proposed.", with `-32603` and `data {reason: "self_call",
safe, function, leg, key}` to the site. The members' own member changes do not enter this
path and are proposed as before.

Vector `tests/vectors/rules/multisig-self-call.json` (13 cases) is replayed by the core and,
through the real core at each shell's multisig dApp entry, by web, Android, iOS and desktop.

## 6. Test builds have their own passkey domain (review 0-1 follow-up)

**Before.** getvela.app's association files now list only the store apps (0-1). Every
build still asked for `getvela.app`, so a debug or device-test build's ceremony could no
longer succeed, and nothing said why.

**Decision: one core rule** (`service_defaults::relying_party_id`):

| Build | Relying party |
| --- | --- |
| Release | `getvela.app`. The test domain is never read. |
| A test build on a simulator or emulator | `localhost`. Platform passkeys answer no association there; the parallel space's fixture keys sign, as they always have. |
| A test build on a device: Android debug or `devicetest`, the iOS Debug or "Vela Test" build, a desktop debug build | Its **`TEST_RP_DOMAIN`**. With none, `RpIdError::TestDomainUnset` in words that say how to set it, and **never** a fall-back to getvela.app. `getvela.app` itself, a URL, a port, upper case or a bare label is `TestDomainInvalid`. |

The web keeps its rule, which is now the core's (`web_relying_party_id`):
- the extension and `*.getvela.app` → getvela.app;
- `localhost` and IPv4 addresses → themselves (the dev server);
- any other host → its own host (a preview's passkeys are its own).

The vector `tests/vectors/rules/rp-id.json` (14 native and 9 web cases) is replayed by
every shell.

**Setting the test domain.** `TEST_RP_DOMAIN` is a placeholder. It is `None` in
`service_defaults.rs` until the owner picks the domain; then either set the constant or
pass it per build:
- Android: `./gradlew … -PvelaTestRpDomain=<domain>`, or the `TEST_RP_DOMAIN` environment
  variable at build time → `BuildConfig.TEST_RP_DOMAIN`.
- iOS: the `TEST_RP_DOMAIN` build setting → the `VelaTestRpDomain` Info.plist key.
- Desktop: `TEST_RP_DOMAIN=<domain> cargo build`.

The domain must serve its own `/.well-known/assetlinks.json` (the debug, devicetest and
Expo keys that 0-1 removed from getvela.app) and `/.well-known/apple-app-site-association`
(the test bundle ids), and the test entitlements must name it (`webcredentials:<domain>`).

**Release builds are unaffected.** Release passes `test_build = false` and never reads
the domain. The release-binary checks assert it:
- Android `check-release-apk.sh`: the release `BuildConfig.TEST_RP_DOMAIN` is empty, and no
  domain set in the environment is in the dex. A store release forces the setting empty.
- iOS `check-ios-store-binary.sh`: the app's Info.plist carries no `VelaTestRpDomain`. A
  copy with one fails the check.
- Desktop `check-store-binary.sh`: the domain is `option_env!` read only in a test build,
  and a release binary built with `TEST_RP_DOMAIN` set does not contain it. A debug build
  does, so the check catches it.

**Left open.**
- Security keys reached over CTAP (USB, NFC, caBLE) keep `getvela.app` in every build. No
  association file gates CTAP, because every CTAP client names its own relying party
  (review P1, phishing claims). The test RP covers the platform passkeys the association
  files gate.
- The key index records each key's relying party. In a test build that RP follows each
  shell's own reading:
  - iOS records the platform rule's RP;
  - Android switches it by key method;
  - the desktop keeps `getvela.app`, because its replay record carries no key method.
  So a test build's CTAP-minted key can be recorded under the test RP, or the reverse. Only
  test builds are affected (they cannot write to the production index, §28). One core rule,
  "the RP a key was minted under", is the R22 fix.

---

## Go-live checklist (the owner)

1. **Deploy the signing page** `app-web/trusted-signer/dist/` (page `7ec6e9a3`, first in
   `BUILD_ALLOWED`). Check that `curl -I https://sign.getvela.app/b/7ec6e9a3…/sign` returns
   200 and `immutable`, then **move `integrity::LAUNCH` to `7ec6e9a3…` before the apps ship**.
   Until LAUNCH moves, the phones open the old LAUNCH page, which hashes a zero window, and
   the core refuses its transaction signatures as `WrongChallenge`. That fails closed and
   nothing is sent, but page-route transactions on phones do not work until then.
2. **Deploy wallet.getvela.app and getvela.app** with this branch's `_headers` and hooks.
   After the deploy, check with `curl -I` that each carries its headers (§4).
3. **The other `*.getvela.app` hosts** (§4 table) send
   `Permissions-Policy: publickey-credentials-get=(), publickey-credentials-create=()`,
   plus the anti-framing pair.
4. **Pick the test RP domain** (§6). Serve its association files, set `TEST_RP_DOMAIN`, and
   add `webcredentials:<domain>` to the test builds' entitlements.
5. **vela-relay** (G5): name `expired` as the `rejection_reason` when the EntryPoint answers
   AA22, so the tracker says "expired" rather than "refused".
