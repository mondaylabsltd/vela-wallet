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

---

# Round 22: signing truth

Round 22 makes the defences true. The 2026-11-07 review found that a "plain" signing verdict
(no "… anyway") could rest on one source an adversary controls, that several checks failed
open, that an unauthenticated descriptor could grade a call green, and that a site's
transaction was hashed without being checked against what the sheet showed. Items 1-2
(with `KNOWN-BUGS` SEC-PARENT-3), 1-3, 1-4 and 1-5 are closed here. `docs/multisig-machines.md`
§36 is the summary. The parallel lanes are `r22-identity` (key index, ERC-6492/SIWE,
approvals, mailbox) and `r22-supply` (dependency gates, signing-page integrity).

The rule, in one sentence: **a plain verdict needs two independent sources to agree — two
distinct providers, or the person's own node and one default; one source answering, or two
that differ, is a check Vela could not read.**

## 7. One RPC never decides what the signer believes (review 1-2, SEC-PARENT-3)

**Before.** Simulation (`eth_simulateV1`), the state-diff guard (`debug_traceCall`, the
SafeStateProbe), `eth_getCode` and a multisig's authority reads (owners, threshold, modules,
guard, nonce, an unlisted Safe's `isOwner`) were each asked of the RPC pool, and the first
good answer won. A lying or compromised node could answer "No asset changes" for a drain, a
clean diff for a takeover, or "not an owner" to clear a parent-Safe request.

**Decision.**
- **Two independent sources** (`app::corroborate`). A read a verdict stands on is a
  *corroborated read* (`rpc_pool::Event::CallRequested::corroborate`). Once one endpoint
  answers, the pool asks a second one **from another provider** the same question, and
  concludes `Respond { url, witness_url }`. The shell hands both bodies to the core in one
  envelope (`{result|error|unreachable, witness, ownNode}`, built only by
  `corroborated_envelope`). The core compares what the answers MEAN, never their bytes,
  because two honest clients differ in gas, block hashes and which accounts a prestate diff
  lists:
  - simulation: the same set of balances moving, the same direction, figures within 2 %
    (`SIM_AGREE_BPS`). Two nodes a block apart price a busy pool differently. Within the
    bound the sheet shows the **worse** of the two figures (the larger outflow, the smaller
    inflow), so a node that shades a figure gains nothing;
  - state diff and probe: the same `StateVerdict`. An undeclared change or a revert that
    **either** node reads stands alone, so a lying first node cannot hide the honest
    node's refusal;
  - code, storage, `eth_call` (authority reads, `isOwner`): the same answer exactly
    (`same_answer`; hex case-insensitive, an error by code and `data`).
- **Who counts as independent** (`provider_of`). A host's registrable domain: the two
  Alchemy hosts are one provider, `rpc.ankr.com` is `ankr.com`. Shared hosting suffixes
  (`workers.dev`, `vercel.app`, …) count one more label, and so do two-letter country
  suffixes (`co.uk`). A loopback or IP address counts with its port, because a developer's
  nodes are all `127.0.0.1`. This is a URL heuristic. It cannot see that two brands resell
  the same backend, and it is no stronger than the pool's own list. Vectors are in
  `signing-truth.json` `provider`.
- **Disagreement or one source is unread.** Simulation → `SimOutcome::Unconfirmed
  { disagreed }`: a caution, "Only one node answered…" / "Two independent nodes gave
  different answers…", Check again, and the confirm is "… anyway". State diff →
  `StateVerdict::Unavailable { reason: "uncorroborated" | "disagreed" }`. Authority reads
  used by a guard → "couldn't check": the multisig gate is `SigningUnchecked`, and the
  parent-Safe guard refuses `Unchecked`.
- **Where it applies.** The signing sheet's own simulation, every read of the multisig
  review (`gather_review` runs on a corroborating `Io`), the gate before every multisig
  signature (`authorize_signing` and the outside page's `gate_now`: `judge`, members,
  nonce), and the §31 `isOwner` read on the sign sheet and in a multisig's dApp flow.
- **chainId on every source.** Before a corroborated read is answered by an endpoint, that
  endpoint must have reported this chain's id (`ProbeChainId`, once per endpoint until the
  pool reloads). One that cannot say is no source. One that names another chain is
  excluded from **every** read of this chain from then on, ordinary reads included.
  Ordinary display reads (balances, history) are not held behind a probe. They never
  decide a verdict, and holding them would double every cold read.
- **"Only use my node".** With it on, the pool holds the person's node alone. A
  corroborated read is then trusted alone (`own_node`), and the UI says so: "Checked by
  your own node only" under the balance changes (`simOwnNode`). The network setting says
  what it means (`onlyMineTrust`). When a check is unconfirmed, the sheet offers it
  (`ownNodeHint`, which opens the network's RPC settings).
- **Bounded load.** At most two extra POSTs (`WITNESS_MAX_ASKS`) and one chain-id probe per
  new endpoint. A corroborated read is never hedged: it is left out of `pending_urls`,
  because an early answer would skip the witness. Answers already fetched are reused: the
  witness is asked only after the first node answered, and a review read again re-asks
  only its silent checks (r16-3).

**Proof.**
- Vectors `tests/vectors/rules/signing-truth.json`: 13 simulation envelopes (the lie in both
  orders, block drift, a witness that cannot simulate or says it fails, own node, revert,
  unreachable), 8 `isOwner` readings, 12 provider URLs. They are replayed by the core and
  by every shell.
- Core `tests/r22_signing_truth.rs`:
  - the pool's corroborated flow, chain checks, a wrong-chain node never asked again, a
    single provider, own-only and own+default;
  - on the captured fork vectors: one node's word is unread, two honest nodes are plain, a
    lying first or second node is never plain, and a liar cannot hide the honest node's
    Bybit refusal.
- **The lying RPC, on the fork** (`tests/fork_proxy`, a local proxy that rewrites
  `eth_simulateV1` logs to none and a prestate diff to "only the nonce moves", and refuses
  the probe):
  - `tests/signing_truth_fork.rs`: a real WXDAI drain, through the real pool machine and
    real HTTP. Two honest nodes see the outflow, believe it and the confirm is plain. The
    liar alone gives `unconfirmed`, never "No asset changes". The liar beside an honest
    node, first or second, gives `simDisagreed` and "… anyway", held.
  - `tests/multisig_fork.rs` `a_lying_node_never_gives_a_member_change_a_plain_review`: a
    real 2-of-3's member change. Honest + honest is `declared` and plain. With the liar
    first or second the review is refused: the rewritten diff reads as an owners change
    the transaction does not declare, and that refusal stands alone. A lie can only make
    Vela refuse. Run on a fresh harness (ports 9701–9706), 3/3 with the other two fork
    tests (create → propose → approve ×2 → execute; the parent-Safe takeover refused).
    `signing_truth_fork.rs` 1/1 on the same harness.

## 8. Every check Vela could not read is "… anyway" (review 1-3)

**Before.**
- A simulated revert was drawn as a danger, but the confirm stayed a plain press.
- In a multisig review, one of the two safety checks unread still signed plainly.
- A batch leg nobody could read was a caution with a plain Confirm.
- "… anyway" was only a red label: the `holdToConfirm` string existed but was unused.
- Nothing let a person refuse blind signing altogether.

**Decision (one rule, `sign_confirm::posture`, and the multisig door).**
- **"… anyway"** whenever anything Vela checks could not be read:
  - a call or message nobody reads (the blind surfaces);
  - a `wallet_sendCalls` batch with one leg nobody reads (`ClearSigningView::sign_anyway`);
  - for a transaction, a simulation that did not come back (or waited out), that a node did
    not offer, that nobody independent confirmed, or that **says it fails**. A revert is
    "… anyway" and a danger.
  - In the multisig review, `sign.anyway` whenever the state diff OR what moves is unread
    (`Concern::StateUnread | Unsimulated`). Either one alone is still a caution in tone
    (the D1 danger line stays for both unread).
- **Hold to confirm** for every "… anyway" and every danger: `ConfirmState.hold_ms` /
  `SignDoor.hold_ms` = `HOLD_TO_CONFIRM_MS` = **1.5 s**, with a progress fill and "Hold to
  confirm".
  - *Why 1.5 s.* A tap, a double tap or a press made while the sheet is still arriving
    cannot sign. The platforms' long-press threshold (≈0.5 s) is deliberately not enough.
    Past 2 s a hold reads as broken. Hold-to-confirm controls in wallets and in the
    platforms' own destructive confirmations sit at 1–2 s.
  - *Accessible equivalent* (WCAG 2.1 SC 2.5.1, 2.5.7: a held gesture is never the only
    way). With assistive technology in use (VoiceOver, Switch Control, TalkBack, a screen
    reader, the keyboard), the first activation arms the confirm and announces "Press
    again to confirm" (`confirmAgain`). A second within `CONFIRM_AGAIN_WINDOW_MS` (5 s)
    confirms, and past it the confirm disarms. One component per platform serves the sheet,
    the hand-off card's Open and every multisig sign button.
- **"Refuse blind signing"** (`prefs::keys::REFUSE_BLIND_SIGNING`, Settings → Security).
  With it on, every "… anyway" is refused: `ConfirmBlock::BlindRefused`, and
  `home.multisig.review.blindRefused` on the door, each naming where the switch is.
  - **Default: off.** Ledger refuses blind signing by default, but on a device that
    cannot show why. Here every "… anyway" is already held, danger-styled and says its
    reason.
  - Since this round, "… anyway" also covers "only one node answered". A refusing default
    would refuse ordinary transactions on every network whose pool holds a single
    provider, and on every network without `eth_simulateV1`. That trains people to turn
    the protection off (warning fatigue: Akhawe & Felt, USENIX Security 2013).
  - For the audience that wants Ledger's posture — core devs, security researchers — it is
    one switch, named on every refusal and kept per device.

**Proof.** In `r22_signing_truth.rs`:
- red on the old code first (`red-1`): the batch leg, one unread check of two;
- the posture for every simulation kind, blind and eth_sign held, refuse-blind refusing;
- the door's hold and refusal through the link rule shells call.

`review-state.json` gains `hold_ms` on every door case and three `refuseBlind` cases. Its
two "one check unread: plain" cases now say "… anyway".

## 9. Only the app's own words grade a call green (review 1-4)

**Before.** Risk came from a reading's intent words, so a Fetched (unauthenticated)
ERC-7730 descriptor saying "claim" or "deposit" graded **Safe**. A compromised descriptor
service could paint a drain green.

**Decision.** `project_result` floors every reading whose provenance is not verified
(`ClearProvenance::is_verified`: BuiltIn, PinnedMatch) at **Normal**. This covers Fetched,
SelectorDb, Standard and None. Only a description that changing would mean changing the app
may be green. The floor applies on the finished result, so no builder can grant green by
hand.

**Proof.** `a_fetched_descriptor_saying_claim_is_never_graded_safe` (red on the old code:
`Safe`). The built-in Lido and Vela-registry readings stay Safe.

## 10. A site's request is signed as reviewed (review 1-5)

**Before.** A wallet send was bound review → hash (`reviewed_safe_op_hash`, spec 104). A
site's transaction, a `wallet_sendCalls` batch and a typed-data signature were hashed by
every shell with the unchecked hash (desktop `executor/user_op.rs`, iOS `UserOpSpine.swift`,
Android `UserOpSpine.kt`, the web's dApp path). That is the v0.9.5 bug class.

**Decision.** At the confirm the core freezes what the sheet showed:
`signed_review::DappReview`, from the FINAL params, carried on `SignAndSubmit.review`.
- A transaction or batch is `Calls { chain, account, calls[], fee: { recipient } }`. The
  shell may hash only an operation whose decoded calls are exactly these calls in order
  (target, coin, calldata), then at most one in-band fee leg paid to the quoted recipient
  (`dapp_safe_op_hash`). The fee amount stays the fee machine's (bounded by `fee_policy` and
  the settlement-gas cap).
- A signature is `Message { chain, account, digest }`. The shell wraps only the digest the
  sheet read, for that account (`dapp_message_hash`).
- Anything else is refused and nothing is signed. A missing review signs nothing.

**Proof.**
- `r22_signing_truth.rs` `attest`: a drifted calldata, value, chain, a fee leg to another
  recipient, a dropped or swapped batch leg, another typed message and another account are
  each refused. The reviewed ones hash.
- Each shell has a test that a drifted operation prompts no passkey.

## 11. An expired operation says it expired, on every path (Round 21 test, r21-1)

**Before.** A plain Send refused with AA22 said "Not sent — It expired before it was
submitted — nothing was sent." and offered Review again (§1). A multisig Execute refused
the same way said something generic instead. The web said "That didn't go through: The
network didn't take it, so nothing was sent. Try again."; iOS and Android said "Something
went wrong, so nothing was sent. Try again." The relay's `SUBMIT REFUSED by simulation:
AA22` reached the multisig machine as a `not_sent` / `unknown` detail. Someone whose clock
is behind would see this every time, with nothing pointing at the cause.

**Decision.**
- **One kind**, `multisig_failure::FailureKind::Expired`, said in the Send's own words
  (`componentsUi.signing.expired`). Try again signs anew with a fresh window. It covers a
  multisig execute and the one-press approve-and-execute.
- **The rule is the core's.** A shell's `{"kind":"expired"}` maps to it. So does the
  relay's AA22 carried in a `not_sent` or `unknown` detail's words
  (`user_op::is_expired_rejection` over `message` / `detail` / `reason`).
- **The dApp paths too.** A site's transaction, `wallet_sendCalls` and the deploy that
  rides an operation are read the same way: `SignSubmitOutcome::Failed` is expired when its
  message is the core's sentence or the relay's own AA22.

**Proof.** `multisig_failure` `an_expired_operation_is_said_as_expired`. Four new
`review-state.json` `failure` cases, replayed by every shell's `review.failure` replay.

## 12. A fee is never shown below its charge; Max sends what it shows (Round 21 test, r21-2)

**Before.** The web cut fees and amounts to six places through floats, so the chain
charged a few millionths more than the screen showed:

| Shown | Charged |
|---|---|
| 0.192556 | 0.1925565 |
| 0.102365 | 0.10236552 |
| 0.018078 | 0.018078621 |
| Max 0.116999 | 0.116999359 |

**Decision (best practice: a charge is never under-stated, and what is signed is what is
shown).**
- **Fees round UP** at the shown places (`fee_policy::fee_figure`; wasm `feeFigure`,
  UniFFI `fee_figure`). The row's budget is 6 places for a native fee and 4 for a token
  fee. Under the last place a fee keeps two significant digits, also rounded up. The
  figure is cut as text, never through a float. This is the rule `coin_amount_text`
  already applied to a shortfall's `need`.
- **Amounts are shown exactly.** A typed amount is the amount signed.
- **Max is a figure the field can show exactly, and that exact value is sent**
  (`send::max_shown_exact`). It is the sendable amount cut DOWN to the field's ladder: 6
  places below 1, 4 from 1, 2 from 1000. The charge is never more than shown. The cost is
  a remainder under the last shown place (< 0.000001 of the coin), which stays in the
  account. A Max no longer empties an 18-decimal balance to the wei, and that is accepted
  so the screen and the signature are one number.

**Proof.** `a_fee_reads_up_never_below_the_charge` (the four Round 21 pairs) and
`max_sends_exactly_what_it_shows`. `send-fee-cover.json`'s R20 Max now reads and sends
0.005887. Invariant ⑨ now reads "never more than the balance less the fee, less than one
shown place left".

## Left open (Round 22)
- Provider independence is a URL heuristic (above).
- Ordinary display reads are not chain-id probed before use. A proven wrong-chain node is
  excluded from them once any corroborated read or race has met it.
- Recipient facts (is it a contract, first time) and the clear-signing `no_code` probe are
  still one node's word. They only ADD a caution, and never make a verdict plain.
- The signing page (trusted signer) runs offline. It draws "… anyway" as the app's word and
  is unchanged this round. Its own hold is the `r22-supply` lane's call.
