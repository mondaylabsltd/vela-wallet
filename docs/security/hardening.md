# Hardening: closing the doors (Round 21), making the defences true (Round 22)

*The decision record behind [`docs/SECURITY-MODEL.md`](../SECURITY-MODEL.md), which summarises it for reviewers. Every path, symbol and id cited here is checked to exist by `scripts/ci/check-doc-claims.mjs` in CI (last run clean at commit `dff291256` with the final review's docs and CI fixes, branch `final-docs-ci`, 2026-11-12).*

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
- **Every transaction-outcome event is removed**, so that no event says how a transaction
  ended:
  - `send_submitted`, `send_confirmed`, `send_failed` (the tracker watch that fed them is
    deleted);
  - `backup_keys_started` / `_done`;
  - a multisig proposal's approval;
  - a site's request whose kind becomes a transaction (`send_transaction`, `send_calls`).
  `catalog.ts` `TX_OUTCOME_EVENTS` and `TX_REQUEST_KINDS`, held by `payload.test.ts`.
- **A site's approved request is counted bare** (final review, 2026-11-12):
  `dapp_request_approved` carries no `kind` and no `chain`. An approved signature (a permit,
  an order) is something a site may take on chain itself, and "a typed-data signature on
  chain N at time T" would point at that transaction. A rejection signs nothing, so
  `dapp_request_rejected` keeps both. The catalog drops the fields whatever a call site
  passes (`payload.test.ts`, "a site's approved request says neither its kind nor its
  network").
- **What is still sent with every hit**: its time, and the IP and browser type every web
  request carries. No event carries an address, an amount or a hash.
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
Worker bundle on workerd with `_headers` applied), nightly. `e2e/security-headers.e2e.ts` also
runs in CI's `web` job on every pull request (since the final review; before it, nightly only).
It asserts:
- a prerendered page and a Worker response carry the headers;
- the page CSP is hash-based;
- the wallet boots its wasm core with no CSP violation.

The site's header list is pinned by `app-web/getvela.app/src/lib/security-headers.test.ts`,
which runs in CI's `site` job on every pull request (since the final review; before it, by hand
only) and before every `bun run deploy`.

**Left open: getvela.app runs a third-party script with no `script-src`.** The site loads
Rybbit's script (`https://tj.appsdata.org/api/script.js`, unpinned, no Subresource Integrity) on
every page but `/chain-setup`, and its CSP is `frame-ancestors 'none'` alone. Recommended: remove
it, or self-host a pinned copy with SRI, then add a `script-src` CSP (the site's inline SvelteKit
scripts need kit's hash-mode CSP first, so this is not a one-line change). Leaving the tag off
`/chain-setup` does not protect that page's deployer key, which is in the origin's
`localStorage` (SECURITY-MODEL G27).

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

## 6. Test builds' passkey domain (review 0-1 follow-up; revised 2026-11-14)

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

**Revised 2026-11-14 (owner): a test build on a device uses `getvela.app`.** Testers must
create and sign in exactly as the store app does, so `TEST_RP_DOMAIN` now defaults to
`getvela.app` (`Some(RELEASE_RP_ID)` in `service_defaults.rs`), and a test build may also
name it explicitly. The table's third row now reads: its `TEST_RP_DOMAIN` if it was built
with one, else `getvela.app`; a URL, a port, upper case or a bare label is still
`TestDomainInvalid`, and an ask the core cannot read still has no relying party at all.

This does **not** reopen 0-1: getvela.app's association files are unchanged and still
decide which builds may use its passkeys.
- iOS: the AASA lists the app id `F9W689P9NE.app.getvela.VelaWallet`, which only the
  team's certificates can sign. A Debug build of that id was always allowed by the file;
  only the app's own rule stopped it.
- Android: `assetlinks.json` lists the release certificate only. An Android test build
  can use getvela.app **only when signed with the release (upload) key**; signed with a
  debug key, the system refuses it (§11's words). Adding a debug certificate to
  getvela.app's `assetlinks.json` would let anyone holding that key build an app that asks
  for every person's passkeys, which is what 0-1 closed: don't.

A build can still be given a domain of its own, which then needs association files of
its own:
- Android: `./gradlew … -PvelaTestRpDomain=<domain>`, or the `TEST_RP_DOMAIN` environment
  variable at build time → `BuildConfig.TEST_RP_DOMAIN`.
- iOS: the `TEST_RP_DOMAIN` build setting → the `VelaTestRpDomain` Info.plist key.
- Desktop: `TEST_RP_DOMAIN=<domain> cargo build`.

A test build still never writes to production services (§28): what it creates is not
published to the production key index, and it says so.

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

# Round 22: identity, standards, mailbox

Round 22 ("make the defences true") takes four more items of the same review: 1-7 (the key
index), 1-11 (ERC-6492 and SIWE), 1-12 (approvals) and 1-13 (the mailbox, wallet side).
`docs/multisig-machines.md` §37 is the summary. The same rules hold: the core owns each
rule, the shells render it, the platforms stay at parity, states are honest, and no
service is a trust premise.

## 7. The key index is never the last word on who co-owns a wallet (review 1-7)

**Before.** Signing in asks the key index which group a passkey founded and who its
members are; `registry_resolve` checks the index's answer against the registry contract on
Gnosis (then the Ethereum backup) by recomputing the group's `contentHash`. When no chain
knew the group yet — the index takes a registration before it lands — or no chain answered,
the answer was accepted as `VerifiedBy::None` and the person signed in, and the signing
member was matched by `credential_id` alone. A lying index could return the person's real
credential beside an attacker's keys. The address that set makes is one the attacker
co-owns (a Vela account is 1-of-N), and the wallet showed it to receive at.

**Decision.**
- **Who vouched travels.** `ShellResult::RegistryUnit` carries the resolver's
  `verified_by`. Absent reads `none`, so a shell that does not pass it fails closed.
- **The member is matched by its public key.** The signing member must hold the credential
  id AND one of the two public keys the assertion itself recovers
  (`webauthn::recover_candidates`). The id is a label the index hands back; the key is what
  the signature proves. A group whose member under this id holds another key is refused
  ("registry group does not match its address"), whoever vouched for it.
- **A multi-key group nobody checked is saved unconfirmed** (`Account.identity`,
  `registry_resolve::identity_of`). One key is the person's own whoever answered (its
  address derives from that key alone), and a group a chain proved is confirmed; only
  `members > 1 && verified_by == none` is unconfirmed. Refusing the sign-in would make the
  wallet depend on RPC health, so the person is in, and can sign; what they cannot do is
  receive.
- **No address, no deposits** (`app::receive_standing`, the one rule every shell draws).
  An unconfirmed or refuted account shows no address, no QR code, no copy or share, no
  payment request and no deposit watch — "Not yet confirmed on-chain" with Check again
  (`receive.unconfirmed.*`), or "This isn't your registered wallet"
  (`receive.refuted.*`). Absent on the wire reads `confirmed` (every record before §37, a
  wallet made on the device), and an identity this build cannot read is read as
  `unconfirmed`, never confirmed.
- **Re-checked before first use, on the chain alone** (`registry_resolve::confirm_step`):
  which groups does the registry say this key founded (Gnosis, then the Ethereum backup),
  and does one of them make this very address? Each group needs every member's passkey to
  be registered at all (`_verifyProof`), so a group the chain holds for the key is the
  person's own: any of them making the address confirms it. Every group read and none
  making it refutes it — the index lied or broke. No chain answers, or none holds a group
  for the key yet: still unconfirmed, asked again. The walk runs when Receive opens and
  once at start for an unconfirmed active account.

**Proof.** Vector `rust/crates/vela-core/tests/vectors/rules/lying-index.json` (8 cases:
an honest index, a forged member caught, the forged and the honest group with no chain to
ask, the confirm step's confirmed / Ethereum-backup / refuted / unconfirmed). It is the
core's own answer (`registry_resolve` `the_lying_index_vector_is_the_cores_answer`) and is
replayed by every shell's resolver loop. `app_login`: an unverified multi-key group is
saved unconfirmed and written so; a verified one is confirmed and not written; a credential
id under an attacker's key is refused with or without a chain's word. On the fork
(`lying_index_fork.rs`, the real registry on the Gnosis fork, every index answer a lie):

- the golden key's real group with one member swapped for an attacker's: discarded, the
  chain's own members used (`chain:100`, `gnosis`);
- a group the chain never held (a key the registry never saw beside an attacker's):
  `verified_by: none` → unconfirmed, and the confirm step finds no group: still
  unconfirmed;
- the golden key's real key beside an attacker's: refuted by the groups Gnosis holds for
  it; its real group: confirmed by Gnosis.

1/1 on a fresh harness (2026-11-09).

**Left open.** An unconfirmed account can still sign; a multisig created with it as a
member would carry its address (members' identity is not part of the multisig create
flow — R23). An older build reading an unconfirmed record ignores the field, and re-saving
there drops it.

## 8. Standard signatures: ERC-6492 and a sign-in that binds (review 1-11)

**ERC-6492 — before.** A message signature (`personal_sign`, typed data, a sign-in) is the
account's EIP-1271 signature over its `SafeMessage` hash. An account that has never sent a
transaction is not deployed — Vela accounts are counterfactual until then — so there is no
code for a verifier's `isValidSignature`, and the signature verified nowhere: "Sign in with
Ethereum" failed for every new account.

**Decision.** `user_op::message_signature(signature, keys, account, code)`, called by every
shell after the EIP-1271 envelope is built, with the account's `eth_getCode` on the
request's chain read through the wallet's own RPC:
- code there → the bare EIP-1271 signature, as before (a plain `isValidSignature` caller
  keeps working);
- no code, or the read failed → ERC-6492's envelope
  `abi.encode(factory, factoryCalldata, signature) ‖ 0x6492…6492`, the factory call being
  the account's own `createProxyWithNonce` — exactly the `initCode` of its first operation.
  A 6492-aware verifier simulates that deployment, then asks the account. An unread code
  wraps because 6492 verifiers accept the envelope from a deployed account too, while a bare
  signature from an undeployed one verifies nowhere.
- `erc6492_wrap` refuses keys that do not make the account: an envelope never deploys
  anything but the account that signed.

**Proof on the fork** (`erc6492_fork.rs` builds the signatures exactly as the wallet does,
`scripts/onchain/erc6492/verify.mjs` verifies them with viem's `verifyMessage` /
`verifyTypedData` — universal validation, the deployless UniversalSigValidator):
a `personal_sign`, a typed-data request and a sign-in, each from a counterfactual
account (the envelope) and a deployed one (deployed by its own factory call; the bare
signature): **21/21 checks** with viem 2.54.6 — all six verify, each tampered twin does
not, and each counterfactual account's bare EIP-1271 signature does not (no code to ask).
So the passkey verifier, Safe's `SafeMessage` hash and the 6492 envelope agree with a
verifier the wallet does not control.

**SIWE — before.** A sign-in (EIP-4361) whose domain did not match the requesting site
drew a red warning and "Sign anyway". The URI and the chain were not checked.

**Decision.** Refused at arrival, never "anyway" (`clear_signing::sign_in_refusal`,
`SignBlockedView.sign_in`, `SignErrorKind::SignInRefused`, -32603 to the site), unless:
- the domain is the requesting origin's host — and its port, when the domain names one
  (EIP-4361: the wallet MUST match the domain to the origin);
- the `URI` is present and on that host;
- the `Chain ID` is present and is the request's chain. A smart account's signature is
  bound to one chain — Safe's domain separator carries the chain id — so a sign-in for
  another chain would not even verify there.
A sign-in with no URI or chain id is incomplete; a request whose origin is unknown is
refused too. Each says why in its own words (`componentsUi.signing.signIn.*`). A sign-in
has no honest reason to name another site or network, so there is nothing to sign anyway.

**Proof.** `app_sign_request`: six refusals (another domain, a port the origin does not
serve, another URI, another chain, no URI, no chain id), each with its key, a Refused
confirm and -32603 `sign_in_refused` on close; a sign-in for the site on its network is
signable; a plain message is not a sign-in.

## 9. Approvals: the witness permit, carried calls, a waiver per leg, and where unlimited starts (review 1-12)

**Before.**
- Permit2's `PermitWitnessTransferFrom` — what UniswapX and most intent protocols ask for —
  fell through to blind typed data.
- An approval inside a `multicall` or a `multiSend` was not seen.
- One unlimited waiver (`unlimited_approved`) skipped the submit check for every leg of a
  batch.
- "Unlimited" started at 2^200 (2^152 for Permit2's uint160), so 2^199 read as a figure.

**Decision.**
- **Witness permits** read as the transfer they are: the guard detects
  `PermitWitnessTransferFrom` and `PermitBatchWitnessTransferFrom` as Permit2 approvals
  (spender, token, amount, deadline; off-chain, signed verbatim under consent), the
  built-in descriptor reads them when Permit2 is the verifying contract and the four fields
  Permit2 defines are exactly there (the witness type is the dApp's), and the signing page
  reads `PermitWitnessTransferFrom` the same way. It reads `PermitBatchWitnessTransferFrom`
  (and `PermitBatch`, `PermitBatchTransferFrom`) as generic typed data ("Round 22: left
  open"; SECURITY-MODEL G11).
- **Carried calls** (`approval_guard::calldata_approvals`): an approval inside
  `multicall(bytes[])` (and its deadline / block-hash forms), Multicall3 `aggregate*`, or a
  Safe `multiSend` is detected, to four carriers deep, at most 64 calls a carrier. Whether a
  carrier keeps `msg.sender` (an OpenZeppelin `multicall` delegatecalls itself and does; a
  Multicall3 does not) is its code's to decide, not the calldata's, so every carried
  approval is read as the account's: the guard errs toward seeing a grant. The sheet
  surfaces the broadest one per call; a cap re-encodes its own word in place
  (`GuardDetectedApproval.nested_at`, the call re-read at that offset before it is
  rewritten); the submit guard judges every one.
- **The waiver is per approval, per leg.** Consent (`unlimited_approved`) waives ONE
  unbounded approval — the one the sheet showed — in the call it was given on; a call that
  carries two cannot have shown both. In a batch, only the legs the sheet names
  (`GuardView.unlimited_consented_legs` → `SignApproveOpts.unlimited_approved_legs`) are
  waived, one approval each; the request-wide flag waives no leg.
- **Unlimited from 2^128** for every amount field. 2^128 base units is 3.4 × 10^20 whole
  tokens at 18 decimals, beyond any real supply (SHIB's is below 2^110), so no amount at or
  above it is about what the transaction moves. The cap editor, the sheet's "Unlimited"
  line and the page draw the same line.
- **Unlimited-like at 100× the balance**: an approval for at least a hundred times what the
  account holds of the token (read, non-zero) is said in the danger tone, in its own words
  ("Over 100× your balance — this contract could spend all you hold, and more as it
  arrives", `GuardView.unlimited_warning_key`). It is a number and the sheet shows it, so
  it is never called "unlimited". Lower multiples are common and honest (a round number, an
  approval before a top-up), and a warning on them would teach people to skip it; a zero
  balance flags nothing by this rule (nothing is at stake yet, and every first approval
  would read as a drain).

**Proof.** `app_approval_guard`: the line and the carrier selectors pinned; a carried
approval detected in each carrier and a carrier inside a carrier, and refused at submit; a
carried approval capped in place, the length unchanged; consent waives one approval and
refuses a second; the batch view names its consented leg; 100× the balance is
unlimited-like in its own key, 99× and a zero or unread balance are not; both witness forms
are approvals. `app_sign_request`: a two-leg batch signs with both legs consented and is
refused with one or none. `app_clear_signing`: "Unlimited" at 2^128, a figure below.
`app_batch_id_097`: Uniswap's batch (two unbounded legs) signs with both consented. The
page: `unlimited-line-test.mjs` 15/15.

**Planned, not in this round.** An allowance list with revoke is a feature, out of this
frozen round: `docs/KNOWN-BUGS.md` R22-PLAN-1.

## 10. The mailbox cannot stall or phish a multisig (review 1-13, wallet side)

**Before.** The mailbox takes a post from anyone. A proposal with no counting approval
still advanced `next_nonce_of`, so one stranger's signature at nonce + 1000 made every
member proposal wait behind 1000 numbers nobody could fill. Strangers' posts also counted
as "waiting" items. And `vet_genesis` did not bind a multisig's contract members to their
own genesis, as the registry README requires before a Safe is replayed on a new chain.

**Decision.**
- **(a) The next nonce** comes only from open proposals with at least one approval that
  counts — verified, by a member on that network now (`next_nonce_of`).
- **(b) What no member vouches for** is `ProposalItem.from_non_member`: left out of the
  default list, of `open_count`, `waiting_on_me` and every presence count
  (`multisig_presence::waiting_of`), so it never reads as "needs your approval". The list
  ends with "Show proposals from non-members (n)" (`Event::ShowNonMembers`), and each one
  shown carries "No member of this multisig has approved this…". Once a member approves
  one it is the members' like any other.
- **(c) Contract members are bound** (`multisig::counterfactual_authority`,
  `Refusal::OwnerNotBound`), the README's step 3 and the server's `replay` rule: a member is
  fine when its own genesis — held by this device (its Vela accounts) or brought by an
  approval (`ownerGenesis`) — deploys exactly its address through a canonical Safe factory
  whose code is canonical on the target chain, or when it holds the same code here as on the
  multisig's other networks (and the registry's, Gnosis and Ethereum, where the device has
  them), or no contract code anywhere it was read (a key holder; an EIP-7702 delegation is
  one). A contract elsewhere with nothing here and nothing binding it, other code here than
  there, or code here and none on its other networks is refused: whoever can put code at
  that address would hold its vote (the 2022 Optimism replay). An unanswered read is
  Unknown, never Verified. The deploy gates bring the full binding; a list or page judges a
  member by its code on the target chain alone.

**Proof.** `app_multisig_proposals`: a stranger's post at nonce 1000 (no approval) and
1001 (a non-member's valid signature) leave the next proposal at 1; the default list,
`open_count` and `waiting_on_me` leave it out, the disclosure shows it flagged.
`multisig.rs` `a_contract_member_must_be_bound_to_its_own_genesis` (11 cases). On the fork
(`multisig_fork.rs` `a_strangers_far_proposal_does_not_stall_the_multisig`, the real
genesis service and mailbox): a 2-of-3 created and recorded by the
service; a stranger's post at nonce 1000 is refused by the service (`nonceTooFar`, the
server bound below), one at nonce 2 is taken; the wallet reads it as a non-member's (out
of the list and every count) and the members' next proposal takes nonce 0, not 3. 1/1 on
a fresh harness.

**Server side** (`sgr-mailbox`, see the round's report): changed in `/Volumes/data/production3/sgr-mailbox`, decided by
best practice and tested there (`cargo test --workspace` 124 passed, clippy clean):
- a proposal more than 100 numbers past the Safe's nonce (read where the service reads the
  chain; elsewhere past the highest open nonce a known owner proposed) is refused
  `nonceTooFar` — the wallet's rule no longer depends on it, but the mailbox no longer
  stores what can never run;
- a proposal counts as a stranger's until a known owner approves it; strangers share one
  allowance of 32 per Safe (oldest pushed out), and at the per-chain cap a new proposal
  pushes out the oldest stranger's, so owners can always use the whole list;
- the per-chain list is every open proposal first, then the recent done ones (done rows
  can no longer crowd open ones out);
- `GET /api/safe/{safe}/chains` no longer reads an unread owner as a key holder
  (`ownerUnread` → unverified), and a genesis no longer excuses other code at an owner's
  address on the target chain (blocked);
- privacy (no auth, CORS `*`, the account ↔ Safe ↔ pending-transaction linkage) is
  documented in its README.
This session's tool isolation refused every version-control command aimed at that
repository, so those changes are left uncommitted there for the owner (branch
`mailbox-r22`, two commits — see the round's report). Nothing was pushed.

## 11. A test build's passkey domain, said as such (R21 finding r21-3)

**Before.** On a test build — RP `localhost` on a simulator or emulator, `TEST_RP_DOMAIN` on
a device (§6) — creating a platform passkey fails the platform's association check, as it
must. The sheet then said the release sentence, "This device couldn't confirm that Vela may
use its passkeys. Check your connection and try again — if it keeps happening, update Vela."
That sent a tester after the network.

**Decision.** The shell's error facts carry the same test-build flag it hands
`relying_party_id` (`PasskeyErrorFacts.test_build`; absent = release), and
`passkey_error_words_on` says an association failure in a test build as
`onboarding.common.passkeyError.testDomainNotAssociated`: "This test build's passkey domain
isn't set up on this device (TEST_RP_DOMAIN, or localhost on a simulator or emulator), so
the system won't let it use a passkey. Release builds aren't affected." The kind
(`AppNotAssociated`) and the failure the machines branch on are unchanged, and so are a
release build's words. iOS (1004 "not associated with domain"), Android (50152) and the
macOS desktop pass the flag; the web's test hosts are their own RP, so it keeps one set of
words.

**Proof.** `passkey_error.rs` `a_test_builds_association_failure_names_its_passkey_domain`
(both platforms, both builds, the facts' JSON); iOS `R213TestBuildPasskeyWordsTests` (2),
Android `PasskeyTestDomainWordsTest` (3).

## Round 22: left open

- An unconfirmed account can still sign, and could be made a member of a multisig (its
  address would be the one only the index vouched for). Gating the multisig member flows on
  `identity` is R23.
- iOS: after Receive saves a new identity, the session machine holds its older copy of the
  account, and a later re-save from there writes `unconfirmed` again. Fail-safe — the next
  Receive checks the chain again — but a core event telling the session is R23.
- An older build reading an unconfirmed record ignores `identity`, and a re-save there
  drops it.
- The signing page still reads `PermitBatch`, `PermitBatchTransferFrom` and
  `PermitBatchWitnessTransferFrom` as generic typed data (caution, not blind-green); the
  wallet's own sheet reads both witness forms.
- A batch leg is judged by the 2^128 line alone: no balance is read per leg, so the 100×
  rule is the single approval's.
- A contract member that exists only on a network neither the multisig nor the registry is
  used on is not seen by the binding rule (the same limit the server's replay check has).
- The server changes (§10) are uncommitted in `sgr-mailbox`; the CI's nightly fork job
  still pins `mailbox-any-signer`.

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

## 12. One RPC never decides what the signer believes (review 1-2, SEC-PARENT-3)

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
  parent-Safe guard refuses `Unchecked`. §18 refines this: two that disagree are
  `SigningDisagreed`; one provider where the pool holds no other goes on, in the member's
  own multisig flows only, as "… anyway".
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
- **Bounded load.** At most one extra POST per other provider, within
  `rpc_pool::WITNESS_BUDGET_MS` (§18; Round 22 first asked at most two), and one chain-id
  probe per new endpoint. A corroborated read is never hedged: it is left out of `pending_urls`,
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

## 13. Every check Vela could not read is "… anyway" (review 1-3)

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

## 14. Only the app's own words grade a call green (review 1-4)

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

## 15. A site's request is signed as reviewed (review 1-5)

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

## 16. An expired operation says it expired, on every path (Round 21 test, r21-1)

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

## 17. A fee is never shown below its charge; Max sends what it shows (Round 21 test, r21-2)

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

## 18. Round 22 follow-ups: one provider, a fee that moves (§39)

Round 22's verification on Arc Testnet found that two of §12–§17's rules, taken
together, locked people out or looped. `docs/multisig-machines.md` §39 is the summary
table; this is the reasoning.

### 18.1 One public provider must not lock a multisig (r22-2, S2)

**Found.** Arc Testnet with the default RPC: a member's review loaded ("Only one node
answered…", "Execute anyway"), but the held "Execute anyway" stopped in 150 ms. The
gate's authority reads (owners, threshold, modules, guard, nonce) are corroborated reads
(§12), and no second provider answered. Desktop said "Can't reach Arc Testnet" (it mapped
`ActionState::SigningUnchecked` to a reachability sentence); Android said it could not
check who can act. Adding an own node fixed it. The likely cause of "no second provider":
Arc's chain index lists two hosts that answered nothing from the tester's network ahead of
`arc-testnet.drpc.org`, and the witness asked at most two.

**Decision.**
- **The witness is looked for within a budget, not a count.** `rpc_pool::WITNESS_BUDGET_MS`
  (10 s) from the first answer; one endpoint per OTHER provider (`corroborate::provider_of`
  differs), best-scored first; a chain-id probe that hangs scores as a failure, so the next
  read asks past it. Ten seconds is two hung chain-id probes (3 s each) and an answer, and
  stays under the review's own read bound. The load bound is now one POST and one probe per
  other provider (was two), still never hedged.
- **"The pool holds one provider" is a fact of configuration, said by the pool.**
  `RpcCallVerdict::Respond::sole_provider` is true only when no endpoint of the pool for
  that chain is another provider (a node proven to serve another chain is not this chain's
  provider). A ban, a cooldown or a silent witness is a failure, not a sole provider: Try
  again can work there. The envelope carries it (`corroborate::SOLE_PROVIDER_KEY`).
- **The member's own flows go on, as "… anyway"** (`Io::corroborating_own`,
  `corroborate::authority_read` → `one_node`): approve, execute, propose (a site's draft
  included — it is the members' own review of their own multisig), a member change, the
  outside page's signature. The read is taken, remembered, and the signature is asked
  "… anyway" (held 1.5 s, or the assistive two presses), whatever the review's own rule
  said. The review never says "checked": its state line is
  `home.multisig.review.stateOneNode` ("Only one node could confirm who can act on
  {network} and what running it changes in the multisig. Add your own node in Settings to
  have it checked twice.") with `addOwnNode`, which opens that network's RPC settings.
  "Refuse blind signing" refuses it like any "… anyway".
  - *Why not refuse.* On a single-provider network the refusal is permanent: the member
    cannot sign or execute their own multisig with the wallet's defaults, and the words
    blamed reachability. The core principle forbids a rule that locks a person out with no
    way forward. Ledger-style refusal remains one switch away.
  - *What one lying node can do then.* Hide a module or guard (no risk confirmation), or
    misstate owners in the review. It cannot move funds: the Safe enforces its own owners
    and threshold on chain, the state-diff guard's undeclared-write refusal stands alone
    (§12), and every such signature is held and said as one node's word.
- **Strict where a site is the threat.** §31's `isOwner` and the self-call rule refuse
  when uncorroborated, including on a single-provider network (`authority_read(…, false)`
  → `unchecked`): refusing a site's request locks nobody out of anything.
- **Two that disagree refuse.** `ActionState::SigningDisagreed`
  (`home.multisig.authority.signDisagreed`): a real signal, never "… anyway".
- **Words.** `SigningUnchecked` is `home.multisig.authority.signUnchecked` on every
  shell, never "can't reach".

**Proof.** Vector `signing-truth.json` `authority` (8 cases, own flow and site guard) and
an `owner` case (one provider: a site's guard stays strict); `r22_signing_truth.rs` pool
tests, red on the old two-ask rule; `app_multisig_proposals.rs`
`one_provider_lets_a_member_sign_only_anyway_said_as_one_node`,
`two_disagreeing_nodes_sign_nothing`, `two_agreeing_nodes_sign_plainly`. On the fork
(`tests/multisig_fork.rs`, fresh harness on ports 9741–9746): with no second node at all,
`one_provider_lets_the_members_sign_and_execute_as_anyway` creates a 2-of-3, proposes,
approves twice and executes — the review says `oneNode`, every signature is asked
"… anyway", the transfer lands; `two_disagreeing_providers_refuse_the_member` puts an
authority-lying `fork_proxy` (`Kind::AuthorityLiar`: code and `getOwners` inverted) beside
the honest node, and the proposal is `SigningDisagreed` with no passkey asked.

### 18.2 A fee that moves must not loop, nor be charged above what was shown (r22-3, S2; A1; r22-1, S1)

**Found.** Android's multisig Execute never went through: six presses, each stopped
"That didn't go through: Network fee ≈ X" because the relay's quote moved between the look
and the press (§24's "a new quote that is not the one on screen signs nothing"). A real
network's base fee moves every block, so a stop on ANY difference is a livelock in the
making. Separately, iOS charged a Max's old figures (r22-1), and Android charged a site's
send 0.138 after the sheet was armed at 0.12 (A1).

**Decision.**
- **A quote at or under the figure on screen signs exactly the figure on screen.** The fee
  leg pays it — charged = shown, and the relay receives at least what it now asks. Only a
  quote ABOVE the figure shown stops; the stop names the new figure and the next press pays
  it, and that press too signs its figure when the quote then is at or under it. One rule
  in the core for: Send and Max (`send::quote_within`, the press's quote; a tap drawn on the
  quote before the one now up signs that quote when it charges no more), a multisig execute
  and the one press (`execute_as`), a site's transaction and `wallet_sendCalls`
  (`SignApproveOpts::current_fee` beside `quoted_fee`; `sign_request::fee_rose`,
  `SignErrorKind::FeeRose`).
- **The figure shown is the one the confirm began on.** A site's sheet hands the fee it
  drew when the hold began, or at the assistive path's first press, and holds the fee row
  still while armed or held (A1). Send and multisig already froze the press (§34, §32).
- **Headroom.** A fee priced at the relay's published per-gas price
  (`Charge::Published`) carries `fee_policy::FEE_HEADROOM_BPS` = 12.5 %, rounded up.
  - *Why 12.5 %.* It is the most EIP-1559 lets a base fee rise in one block, and the
    relay's per-gas figure follows the next block's base fee: a quote shown for a block or
    two stays payable however the base fee moves. viem's default is 20 %, MetaMask's
    `maxFeePerGas` is about 2× base; but this headroom is CHARGED (the fee leg is fixed in
    the signature, nothing is refunded), so it is kept to one block.
  - *Consistent with the bounds.* A relay that publishes no price is paid 3× its gas price
    (`INBAND_MARKUP`, the 3× bound) and gets no headroom on top. The settlementGas cap
    (`settlement_gas_cap`, 125 % + 50,000) bounds the gas a relay may bill, before any of
    this; the headroom touches the price, not the gas.
- **Stop words say why** (r22-4, KNOWN-BUGS R20-15): `home.multisig.proposal.feeRose`
  ("The network fee went up to ≈ {{fee}} since you pressed; nothing was signed. Press
  again to pay {{fee}}."), `componentsUi.signing.feeRose`, and `send.feeMoved` /
  `send.feeMovedMax` reworded. Never "That didn't go through: Network fee ≈ X".
- **r22-1.** The Max's move was said from the figure first shown, so a quote that fell
  (said) and came back un-said it and flipped the figures back silently while the person
  had last read the lower fee. Every move is now said from the figure drawn now
  (`send::set_max`); the press signs exactly what is drawn (the shell carries the review
  digest it drew, and §34 freezes it).

**Proof.** `send-fee-cover.json` `press_freeze` +4 cases (red with the rule off);
`a_max_that_moves_and_moves_back_is_said_each_time` (red on the old `set_max`);
`a_press_signs_only_the_fee_it_carries_even_on_a_fresh_look` (the fee leg is the 999
carried over a quote of 1; a figure under the quote stops); `a_site_send_signs_the_fee_shown_or_nothing_when_the_quote_rose`;
`app_fee_eth_mainnet.rs` (the relay's published figures unchanged, shown with the
headroom). The fork relay stand-in moves its gas price on every quote (`relay.py`
`MOVING_GAS`, `/v1/test/gas`) for the shells' onchain proof.

### 18.3 Smaller faults (S3/S4)

- **Desktop keyboard (r22-5).** Tab / Shift-Tab traverse a sheet; Enter or Space on a
  focused hold button arms "Press again to confirm" for 5 s
  (`sign_confirm::CONFIRM_AGAIN_WINDOW_MS`), as on web and Android.
- **A re-check keeps the door (r22-6).** `on_sim_started` ignores a re-check of a verdict
  already on the sheet: the confirm no longer flips between "Sign anyway" and a shut
  "Checking…", and a hold or arm made on it survives a re-check that lands the same.
- **Send's picker under a refused network (r22-7).** The last-known rows with the home's
  "Can't reach …" line, never "No tokens with balance".
- **Sign-in words (r22-8).** A key-index group that does not make its recorded address is
  said `onboarding.login.registryUnreachableBody` (`login::REGISTRY_MISMATCH_WORDS`), never
  the core's English with biometric advice.
- **A site's refusal (r22-9)** no longer asserts a membership the wallet could not check.
- **Hidden stranger proposals** count for nothing — not for a member's "Only one of these
  can run…" note, and a stranger's cancellation voids nothing (`mark_cancellations`).
- **A node never erases a value the wallet decoded** (`sim_outcome::with_decoded_value`):
  with "Only use my node", a checked answer that moves no native coin is drawn with the
  request's own native outflow. Reconciled, not refused: an honest node without
  `traceTransfers` reports no native move at all.
- **A stop that sent the site nothing is still answered.** A `FeeRose` (or a stale Tempo
  quote) holds no answer for the site; a swipe of that sheet returned `Dismiss` and the
  site waited for ever. An error with nothing sent, no answer held and nothing in flight
  now swipes as `Reject` (4001), the spec-081 rule (vector `signing-truth.json`
  `fee_rose`).
- **The desktop's connect request behind the chip (R22v-10)** is not fixed: the Connection
  column opens only on a new consent, and a restored tab's standing one is not raised
  again. Recorded in KNOWN-BUGS with the fix direction; nothing is granted meanwhile.
- **Every unbounded grant is a held danger** (`sign_confirm::guarded_posture`): a
  Multicall3- or multicall-carried approval and a Permit2 witness permit were a plain
  Confirm while a two-leg unlimited batch was held. The approval guard's one verdict
  (`GuardView.unlimited_warning`) now holds the confirm however the grant is nested
  (`every_unbounded_grant_holds_the_confirm_however_it_is_nested`).

### 18.4 Left open

- On a single-provider network a member signs on one node's word about who can act —
  said, held, and one switch ("Refuse blind signing") or one node ("Add your own node")
  away from corroborated. A provider-diverse default pool for Arc is the real fix and is
  the chain index's to list.
- The headroom is paid, not refunded (the in-band fee leg is fixed in the signature).
- A quote that rises more than the headroom between the look and the press still stops
  once — said, and the next press pays it.
- R22v-10 (desktop): a restored tab's connect request waits behind the account chip.
- Dependency advisories published after a round's gates fail CI until the lock moves:
  rustix 1.1.6 and the Windows passkey crate's crypto-bigint / ctutils / wnaf were updated
  here (§39).

## 19. Final review fixes (§41)

The decisions and their evidence are in
[`docs/multisig-machines.md` §41](../multisig-machines.md). What changed in the guarantees:

- **Addresses in one form** (P1-1, G24 closed). Arrival refuses -32602 a transaction `to`, a
  `wallet_sendCalls` leg's `to` or a typed-data `verifyingContract` that is not `0x` and exactly
  40 hex digits (`tx_request::is_canonical_address`); the guards read addresses as the encoders
  do (`self_call_guard::normalise_address`).
- **Safe v1.5.0** (P1-2). A v1.5.0 Safe checks an owner contract with `isValidSignature(bytes32)`,
  the wrap a site's typed data gets; the parent-Safe guard refuses `SafeTx`, `SafeMessage` and
  `SafeOp` for every Safe the account owns, whatever its version (`SelfCallFunction::SafeOpTypedData`).
- **The simulation sees NFTs, grants and what it cannot read** (P2-3): ERC-721 / ERC-1155 moves and
  the account's approvals are changes inside the two-node agreement; an unread asset log is
  "This changes something Vela can't read" and "… anyway" (`sim_outcome::derive_deltas`).
- **Approvals** (P2-4): a single `setApprovalForAll(true)` is a held danger; a carrier past the
  guard's limits is "… anyway" (`GuardView::carrier_unread`); an unbounded grant the simulation
  finds is a held danger whatever carried it (`sim_outcome::grants_unlimited`).
- **The machines hold the gate** (P2-5): `sign_request` re-works the posture at approve time from
  the views shown (`SignApproveOpts::review`) and takes "… anyway" only with `anyway_confirmed` and
  never with Refuse blind signing; the multisig machine does the same in `authorize_signing`. The
  hold's duration stays the shell's.
- **4byte and fetched readings** (P2-6) lift nothing out of "… anyway" without a corroborated
  simulation; **a reading that never started** (P2-7) cannot be confirmed
  (`sign_confirm::is_transaction`).
- **"Your chosen node"** (P2-8): a URL built from a retired provider key is not the person's own
  node (`network_admin::is_provider_key_url`).
- **The §40 creation lookup** is bounded, total and timed, and asks only a Blockscout-style API
  (`multisig_shell::blockscout_api`).

## Left open (Round 22, signing truth)

- Provider independence is a URL heuristic (above).
- Ordinary display reads are not chain-id probed before use. A proven wrong-chain node is
  excluded from them once any corroborated read or race has met it.
- Recipient facts (is it a contract, first time) and the clear-signing `no_code` probe are
  still one node's word. They only ADD a caution, and never make a verdict plain.
- The signing page (trusted signer) runs offline. It draws "… anyway" as the app's word and
  is unchanged this round. Its own hold is the `r22-supply` lane's call.
- The wallet's own requests (the key-record backup) get no exemption. When their simulation
  is one node's word they are "… anyway", held. This is honest, and a candidate for a
  first-party rule.
- While a simulation's verdict is still out, the confirm is shut ("Checking…") and plain.
  It is judged only when the verdict lands, so "Sign anyway" never flashes over a check that
  comes back plain.
- A single local fork has no independent second node. The onchain harnesses (the web's
  `seedNetwork`) therefore run with "Only use my node": trusted alone, and said.
- Hold, the assistive two-press path and the AA22 clock-skew repro are covered by unit and
  device tests on each shell (Android `HoldToConfirmDeviceTest`). They have not been
  walked by a person on real hardware.

---

# Round 22: supply chain and the signing page's integrity

Round 22 makes three defences of the 2026-11-07 review true: dependency hygiene (review 1-10),
reproducible builds (1-9), and the signing page's check-then-open race (0-5, with the honest
account of what the page guarantees from 0-4). `docs/multisig-machines.md` §38 is the summary.
How a reader checks all of it is [`verify.md`](verify.md).

## R22-1. Dependency gates (review 1-10)

**Before.** CI ran no dependency audit (`npm ci --no-audit`), there was no Dependabot or
Renovate, the desktop's `gpui` was a git dependency with no revision, and the vendored
PeerConnect client was a built `dist/` with a source commit that turned out to be wrong.

**Decision.**

- **cargo-deny over both Rust graphs** (`deny.toml`, one file, run once per lockfile:
  `rust/Cargo.lock` and `app-desktop/vela-wallet/Cargo.lock`, plus the Windows passkey crate).
  It covers four areas:
  - *advisories*: vulnerabilities, unmaintained, unsound and yanked crates all fail;
  - *licences*: an explicit allow-list;
  - *sources*: crates.io only, plus four named git repositories, each pinned to a commit
    (`required-git-spec = "rev"`; unknown git is denied);
  - *bans*: openssl and native-tls are banned outside the one place they already are.

  The graph is limited to the targets that ship (wasm32, macOS, Linux, Windows, Android, iOS).
  It uses default features, because the extra features are test seams that no release carries.
  Every exception says why it is safe today and what removes it. "Upstream has no fix" is
  accepted only together with "unreachable" or "our own input".
  - *Installed in CI* with `taiki-e/install-action` at a commit, `cargo-deny@0.20.2`. The action
    checks the release's own sha256 and needs no container.
- **JS audits of what ships.**
  - `app-web/vela-wallet`: `bun audit --prod --audit-level high`, and the whole tree at
    `critical`. SvelteKit bundles its devDependencies (kit, svelte, devalue) into the shipped
    pages, so `--prod` alone would miss them.
  - `app-web/getvela.app`: `bun audit --prod --audit-level=high`. It is a bun project, so its
    own package manager audits it.
  - `scripts/` (tooling): `npm audit --audit-level=high`.
- **Dependabot** (`.github/dependabot.yml`): cargo (`/rust`, the desktop, the Windows crate),
  npm (`/scripts`), bun (the web wallet and getvela.app), gradle (Android), and GitHub Actions. iOS has
  no entry: it has had no remote SwiftPM package since lottie-ios was removed (2026-11-14).
  - Weekly; minor and patch grouped, majors one PR each.
  - A 7-day cooldown (30 days for majors), so a malicious release has time to be found and
    yanked first.
  - uniffi and the commit-pinned git dependencies are ignored, because they must move by hand.
  - The header says that a `rust/Cargo.lock` bump must rebuild the web wasm in the same PR.
- **gpui pinned** to the commit the lockfile already resolved,
  `c97b7c0ea41acf56e9726964eca10c2078e921b0`. This applies to `gpui`, `gpui_platform` and
  `vendor/gpui_macos`'s four zed dependencies (otherwise Cargo sees two sources and a second
  gpui). `dotlottie-rs` moved from a tag to the tag's commit. Nothing that is built changed:
  the same 21 packages at the same commit, and `cargo check --locked` passes both ways.
- **PeerConnect, rebuilt from source.** `VENDOR.json` records the repository, commit,
  toolchain (rustc 1.97.1), wasm-bindgen (0.2.129), the recipe, the local patch and every
  file's sha256. `scripts/check-peerconnect-vendor.mjs` checks it in two ways:
  - the hashes, always (CI `app`);
  - `--rebuild`, which clones the commit, builds it with the paths mapped out, applies
    `vela-local.patch` and compares byte for byte (CI `supply-chain`).

**What the gates found today, and what was done.**

| Finding | Where | Decision |
| --- | --- | --- |
| RUSTSEC-2026-0387 crypto-bigint 0.7.5, secret-dependent timing | core (k256/p256 0.14): the CTAP2 PIN/UV ECDH and the caBLE keys on Android/iOS | **fixed**, 0.7.10 in `rust/Cargo.lock` (shipped with this round's wasm) |
| RUSTSEC-2026-0385 ctutils 0.4.2, non-constant-time masks | core: caBLE AES-GCM, CTAP ECDH | **fixed**, 0.4.8 |
| RUSTSEC-2026-0220 ruint 1.19.0, wrong shift overflow flags | core (alloy-primitives) | **fixed**, 1.20.1. It was not exploitable: the core's one `checked_shl` was checked for every exponent it can hit |
| RUSTSEC-2026-0384 tokio 1.54.0 (and yanked) | core (vela-agent; current-thread runtime, no `Interval`) | **fixed**, 1.54.1 |
| wnaf 0.14.0 yanked | core | **fixed**, 0.14.1 |
| rustls 0285/0346/0348/0349, rustls-webpki 0382, webpki-roots 0381, pulldown-cmark 0414/0433, tokio 1.53.1 (unsound 0386), calamine 0436, zip 8.6 (0367/0368/0369/0429), crypto-bigint 0.7.5, ctutils, chacha20/wnaf/async-executor yanked | desktop | **fixed** by `--precise` bumps in the desktop lockfile, calamine 0.36 → 0.38 |
| RUSTSEC-2026-0387 crypto-bigint 0.5.5 | desktop: p256 0.13, used only to **verify** the updater's signature (public inputs) | allowlisted. Exit: the desktop moves to p256 0.14 |
| zip 2.4.2 (0367/0368/0369/0429) | desktop: came in with dotlottie-rs | **gone** with dotlottie-rs (2026-11-14, the launch animation is drawn by gpui); the allowlist entries are removed |
| lru 0.12.5 unsound (0002/0253) | desktop: came in with rqrr 0.9 (QR decode) | **gone** with rqrr (2026-11-14, the desktop reads QR codes with `rust/vela-qr`, which has no dependencies); the allowlist entries are removed |
| glib 0.18.5 unsound (0429) | in no shipped build (`cargo tree --target` is empty for every target); cargo-deny joins cfg edges across targets | allowlisted |
| unmaintained: paste, proc-macro-error(2), basic-toml (compile time only); bincode 1.3.3 (crux_core, our bytes only); rustls-pemfile (OS cert files); rustybuzz/ttf-parser (system fonts); zvariant/zbus (session bus) | both | allowlisted, each with its exit in `deny.toml` |
| **GPL-3.0-or-later `zlog`, `ztracing`, `ztracing_macro`** | desktop: linked by gpui at the pinned commit (gpui → sum_tree → ztracing → zlog) | **open, a release blocker for the desktop (owner, below).** A version-pinned exception keeps the gate green only because the fix is known: Zed relicensed all three to Apache-2.0 in zed#63573 (`ac5af8b9`, 2026-09-01). Exit: move the gpui pin to `ac5af8b9` or later |
| MPL-2.0 (uniffi ×8, cbindgen, cssparser, selectors, dtoa-short, dwrote, option-ext) | both | allowed per crate. File-level copyleft, and these files are not modified |
| npm: @sveltejs/kit 2.70.1, devalue 5.8.2 (bundled into the wallet's pages) | web wallet | **fixed**, kit 2.70.4, devalue 5.9.4 |
| npm: undici 7.28 and sharp 0.35 (7 + 2 high), vite 8.1.5 (high), vitest, minimatch, cookie (dev only) | web wallet dev tree: wrangler's local simulator, the dev server, tests | reported; Dependabot. None ships, and none is critical, so the whole-tree gate at `critical` passes |
| bun: @vitest/browser 4.1.1 (3 critical, test runner); the site's Worker runs kit 2.55 / svelte ≤5.55.6 / devalue ≤5.9.2 (high/moderate) | getvela.app | the prod audit (13 runtime packages) is clean. **The Worker's kit/svelte/devalue bump is open** (in range, separate PR) |
| npm: yaml 2.8.1 moderate | `scripts/` (parses repository-owned YAML) | reported |
| PeerConnect's README said `f3148cb` | vendored client | **corrected**. `index.js` is `977ec1e` + `vela-local.patch`. The module had been built from an uncommitted tree between the two commits (one panic line number differed), so it matched no commit. It was rebuilt from `977ec1e` with the recipe; the only change is that line and the builder's paths, now mapped out |

## R22-2. Reproducible builds (review 1-9)

**The signing page** was already reproducible: `build-single` concatenates with no minifier,
timestamp or environment, and Bun and Node produce the same bytes. What was missing is that
nothing enforced it. Its `--check` is now the `signing-page` CI job on every PR, under Bun and
under Node; it blocks a merge once branch protection requires it (go-live). It also checks
`dist/sw.js` (R22-3).

**The web core.** The repository used to say the wasm bytes were "not reproducible across
machines", and `build-web.mjs --check` compared a source fingerprint and the ABI only. Two
causes were found and fixed:

- **The builder's paths.** The 0.9.8 module embedded `/Users/<developer>/.rustup/toolchains/
  1.97.1-aarch64-apple-darwin/lib/rustlib/src/rust/library/…` in 27 panic-location strings.
  With the `rust-src` component installed, rustc maps std's virtual `/rustc/<commit>/` paths
  back to local ones. The build now maps the sysroot's `rust-src` back to `/rustc/<commit>`,
  `$CARGO_HOME` to `/cargo` and the checkout to `/vela`. No path of the build machine remains
  (0 strings, measured).
- **The `producers` section** records the tools in their own words. wasm-bindgen's entry
  differs between a CLI built from source (`0.2.126`) and a downloaded one
  (`0.2.126 (21ac804a9)`). Nothing reads it at run time, so it is dropped; the toolchain is
  pinned in the repository instead.

**The pinned builder.** `rust/scripts/web-build.Dockerfile` contains:
- `rust:1.97.1-bookworm` by its index digest;
- wasm-pack 0.15.0 and wasm-bindgen-cli 0.2.126, built `--locked` from crates.io;
- binaryen `version_117` and node 22.22.3, each tarball checked against its pinned sha256.

`rust/scripts/reproduce-web.sh [<commit>]` builds the module from a `git archive` of the commit
inside that image and compares every output byte for byte.

**Measured.**
- Two clean builds on macOS arm64, each from an export of the tree in a fresh target
  directory: **byte-identical**, module and glue (`43e1933b…` for that tree). The module
  carries 0 paths of the build machine; it used to carry 27.
- **Not measured locally: Linux.** The Docker VM here (2 CPUs, 3.8 GB) killed rustc on
  `vela-core` under amd64 emulation (out of memory), and OrbStack had no network.
- The repository's earlier note measured macOS and Linux differing in 92% of their bytes,
  with different function counts. Paths of different lengths (`/Users/…/.rustup/…` against
  `/rustc/<commit>/…`) shift every later offset in the data section, which fits most of that.
  The remap removes them. This is a likely explanation, not a proven one.
- **Decision.** The local builds were reproducible twice from clean, so the CI compare is
  byte-exact and fails the run (it blocks a merge once branch protection requires it). If the
  first CI run shows that the Linux container's bytes differ from a macOS build's, the job
  fails and uploads what it built (`web-core-container-build`).
  The fix is then to commit the container's output (`rust/scripts/reproduce-web.sh`'s tree),
  so the shipped module is the CI-built, attested one, and to make `build:wasm` run in the
  container. That is an owner item for the first CI run.

**CI.** `web-core-reproducible` runs `reproduce-web.sh` on every PR, and fails when the
committed module is not what the commit builds. A failing job stops a merge only if branch
protection lists it as a required check, which is an owner setting (go-live, "Branch
protection"); this repository cannot show or enforce it, and the same holds for `signing-page`
and every other job called blocking here.

On a push to `main`, and only after its rebuild matched the committed bytes, the job also
attests the CI-built module and glue (`actions/attest-build-provenance`), so
`gh attestation verify` can bind wallet.getvela.app's bytes to a workflow run and a commit.
That needs a merge to `main` whose container build matches. `main` (v0.9.8) predates the job,
so no served module is attested today, and with Linux reproducibility unmeasured (G5 in
SECURITY-MODEL) the first such run may instead fail and keep the container's build.

## R22-3. The signing page: check-then-open (review 0-5)

**Before.** Every wallet fetched `b/<h>/sign`, hashed it, ruled on it (`integrity.rs`), and
then had a browser open the same URL (`page-handoff.svelte.ts` `checkPage` → `window.open`;
the apps' system browser). A host that answers the check with the published page and the
navigation with another one was caught by nothing. A check vouched for 24 hours, and the
check's request differed from a navigation in everything but `Accept`.

**Decision: open the bytes that were hashed, wherever the platform allows.**

| Surface | How the page is opened now | Why |
| --- | --- | --- |
| **Web wallet, official page** | **from the checked bytes**: `URL.createObjectURL(new Blob([bytes]))` + `#ch=post&lang=…&b=<h>` | a blob document has the wallet's origin; `wallet.getvela.app` may use `getvela.app` passkeys, so WebAuthn works and the host is never asked again |
| Web wallet, a self-hosted page | the checked address + the Service Worker pin | its keys are its own domain's; the wallet's origin cannot use them, so a blob could not sign |
| Chrome and Safari extensions | the checked address + the pin | an extension origin cannot use `getvela.app` passkeys, and MV3 extension pages allow no inline script |
| iOS, Android, desktop | the checked address in the system browser + the pin + a 10-minute check | see "the natives" below |

**The core decides** (one rule, every shell):
- `integrity::FROM_BYTES` lists the versions that can be opened from bytes. It starts with
  `674b7333`, the first page that reads its place from its origin and fragment. Older pages
  would ask the passkey for an empty relying party.
- `launch::CheckedPage::opens_from_bytes(opener_origin)` says yes when the version is in that
  list and the opener's origin is `https` (or loopback `http`) on the page's signing domain or a
  subdomain of it. That is WebAuthn's own RP ID rule.
- `bytes_launch` writes the fragment.
- The wasm hands it to the web as `trustedSignerPageAdmit(…, openerOrigin).fromBytes`.

`page-handoff` keeps the bytes the check read and opens a fresh object URL from them at the tap.
It is released when the hand-off ends, and a reload re-runs the same bytes. Messages are then
expected from the wallet's own origin.

**The page's integrity is kept, not loosened.**
- The page's own `default-src 'none'` CSP is inside the bytes and still governs a blob document.
- A blob document is also held to the wallet's CSP, so its one inline script must be allowed
  there. `vite.config.ts` reads the versions from the shipped core (`trustedSignerFromBytes`),
  re-hashes each committed page (a mismatch fails the build), and adds exactly that page's
  `script-src` hash to the wallet's policy.
- Measured in Chromium, Firefox and WebKit: without the hash the page loads and runs nothing.
  With it, the page runs under its own CSP (its `fetch` is refused).

**WebAuthn and the channels from a blob origin** (measured; `e2e/page-from-bytes.e2e.ts`):
- The blob's origin is the opener's.
- `navigator.credentials` is present, and `isSecureContext` is true.
- Chromium creates and gets credentials for the origin's RP ID.
- `clientDataJSON.origin` is the wallet's origin (`https://wallet.getvela.app`). The core does
  not bind the page's origin, and neither does Safe's WebAuthn verifier on chain: the origin is
  inside `clientDataFields`.
- `window.opener` and `postMessage` work as before. The page answers only the wallet origins
  the browser reports for its opener.
- The e2e signs a message end to end from a blob: one request for the page's bytes, the
  check's, and none from the window. A host that serves other bytes afterwards changes nothing
  that opens.
- WebKit's and Firefox's WebAuthn could not be driven here, because they have no virtual
  authenticator. Their blob documents report the same origin and a secure context.

**What opening from bytes costs.** The page then runs in the wallet's origin, not on
`sign.getvela.app`, so it is no more separated from the wallet's script than the wallet is from
its own passkeys. For a `getvela.app` account this was already so: the wallet can ask the
passkey directly (`signing_venue.rs`, "What this does NOT claim"). Before this change, a
compromised wallet could also open any page it liked, because on the web the check is the
wallet's own code. What the change gains is that the bytes that run are the bytes that were
hashed, so the host is out of the trust base for the web.

**Where bytes cannot be opened: the Service Worker pin** (spec 076 FR-008, built now).
- The page registers `<deployment>/sw.js` with scope `b/`. Its CSP gains `worker-src 'self'`
  and nothing else; the page still fetches nothing.
- The worker answers every navigation to `b/<h>/sign` from its cache, or fetches, hashes and
  serves the bytes only if they hash to `h`. Anything else gets a 409 page that runs nothing
  and names both hashes.
- It pins the version that registered it, re-fetched and verified, so the next visit is
  answered locally.
- Measured (`samples/sw-pin-test.mjs`, 10/10 checks): after the host swaps the bytes, the
  published page still opens, the host is not even asked, and the response still carries
  `frame-ancestors 'none'`. A version served wrong from the start is refused.
- **Limits, said plainly:**
  - The first visit is trust on first use.
  - The browser updates the worker from the host, so a host serving a hostile `sw.js` defeats
    the pin from the following visit.
  - A browser that keeps no worker (private window, cleared storage) is unpinned.

**The 24-hour vouch window is now 10 minutes for opening.**
- `MAX_CHECK_AGE_MS` = 10 minutes. Every shell re-checks at the tap when the check is older:
  iOS `openable`, Android `admitted`, the desktop's `launchable`, and the web at every
  hand-off. So the check a page opens on is minutes old.
- The line a person reads ("matches · checked 14:32") still stands for a day (`LINE_MAX_AGE_MS`,
  with its time). The background refresh stays at 12 hours, so a phone does not fetch the
  1 MB page every few minutes.
- This narrows the window for a whole-build swap. It does not close check-then-open; only the
  two measures above do.

**Check requests look like navigations, as far as headers go.**
- `launch::CHECK_HEADERS`, which every app's check sends, now carries the whole of a top-level
  navigation's request headers a host can key on: `Accept`, `Upgrade-Insecure-Requests: 1`, and
  `Sec-Fetch-Dest: document`, `-Mode: navigate`, `-Site: none`, `-User: ?1`.
- **What still differs:** the TLS and HTTP/2 fingerprint of URLSession, OkHttp and ureq; the
  `User-Agent` and `Accept-Language`; and the timing (a check, then a navigation). A browser's
  `fetch` may not send `Sec-*`, so the web wallet's fallback check sends `Accept` only. A host
  set on telling them apart can, which is why the web opens the bytes and the page pins itself.

**The natives: an in-app WebView loaded from the verified bytes was considered and not built.**
1. The apps open the page for two kinds of account.
   - A **custom-domain account**: its keys can be used only by pages on its domain. An in-app
     WebView can use a passkey only for an RP the app is associated with (Android's WebView
     WebAuthn checks the app's asset links, and WKWebView needs `webcredentials`). The app is not
     associated with someone else's domain, so a WebView there could not sign.
   - A **`getvela.app` account** that chose the page as its venue. The app can already call that
     passkey natively, so a WebView adds no authority boundary.
2. An in-app WebView is the app's: the app can inject script into it, read it and rewrite it.
   The system browser (SFSafariViewController, Custom Tabs, the desktop's default browser) is
   the one place the app cannot reach. It is also what the page-scoped RP ID below needs: an app
   associated with the page's RP could use those keys itself.

So the natives keep the system browser, now with the pin and the 10-minute check.

### A test build names the page version it opens (R21 finding)

**Before.** `integrity::LAUNCH` is a compile-time constant (`6d7172b4`), and the deployed
`index.json` lists only what is deployed, so a test build always opened a page that was
already published. After R21 the core refuses what an older page signs: `6d7172b4` hashes a
zero validity window, which the core rejects as `WrongChallenge`. So the phones' page path
could not be exercised locally against `7ec6e9a3`, or against this round's page.

**Decision.** There is one core rule, `launch::set_test_launch(version, test_build)`
(uniffi `signer_page_set_test_launch`). In a test build it names the version that `target()`
picks, whatever the index lists.
- It is refused unless `test_build` is true.
- It is refused unless the version is in `BUILD_ALLOWED`. A test build opens a page this build
  accepts, or none.
- The version is still fetched, hashed and ruled on (`admit`), and a version blocked on the
  device stays blocked.

Each shell reads it only in code that a release build does not contain:

| Shell | Input | How it is kept out of release, and the check that proves it |
| --- | --- | --- |
| iOS | env `VELA_PAGE_LAUNCH` (`Dev/DevLaunchInputs.swift`) | `Dev/*.swift` is in Release's `EXCLUDED_SOURCE_FILE_NAMES`; `check-ios-store-binary.sh` refuses any `VELA_*` name from `Dev/` in the binary |
| Android | `adb shell am start … --es vela.pageLaunch <hash>` (debug `ServiceLaunchInputs`) | debug source set only; `check-release-apk.sh` refuses the class and now the `vela.pageLaunch` string |
| Desktop | env `VELA_PAGE_LAUNCH` (`dev_env::var!`) | expands to `None` outside a developer build; `check-store-binary.sh` refuses every `dev_env` name in a release binary |

Measured:
- Core `tests/test_launch_input.rs`: a release build is refused, and so is a version not in
  `BUILD_ALLOWED`. In a test build, `target()` picks the named version with no index, against
  an index that lists only what is deployed, and on a local page; other bytes are still
  refused, and a blocked version still falls back.
- Android `SignerPageChecksTest`: the same rule through the Kotlin bindings.
- iOS `WireParityI1Tests`: the refusals through `DevLaunchInputs`.
- **Release binaries:**
  - iOS Release (simulator) passes `check-ios-store-binary.sh` ("none of 13 developer
    inputs"), with 0 occurrences of `VELA_PAGE_LAUNCH`.
  - Android `assembleRelease -PvelaUnsignedRelease` passes `check-release-apk.sh`.
  - The desktop's release binary was not built this round, for lack of disk. Its gate is
    unchanged: `dev_env::var!` expands to `None`, and `check-store-binary.sh` reads every
    `dev_env` name.

**Not done: an end-to-end page signature on a phone.** The ask was a page-signed send and a
multisig approval through the page, on an emulator or simulator, with the local page on the
newest build. That needs a harness that does not exist yet:
- a passkey on the page's relying party, in the system browser;
- a local chain, relay and genesis service, as the multisig fork suite has for the web;
- an account whose keys were made on that page.

The Android emulator's Chrome holds no WebAuthn credential without a Google account. The iOS
simulator can hold one for `localhost`, but driving SFSafariViewController's Face ID sheet from
XCUITest was not built.

What stands in for it today:
- the page's own suites on `674b7333`, all 23 green. `desktop-demo` drives the URL channel the
  phones use (app → browser page → `velawallet`-style callback → the answer the core
  accepts); `multisig-test` covers the member approval; `answer-test` covers every signature;
- the web wallet's `e2e/page-from-bytes.e2e.ts`, which signs end to end from the checked bytes;
- the phones' own `SignerPageChecks` suites, which cover the check, the 10-minute window and
  the override.

The phone harness is an open item for the R21 tester. With this input it can be built: start
the debug app with `VELA_PAGE_LAUNCH=674b7333…` (Android `--es vela.pageLaunch`) and a
loopback page serving `app-web/trusted-signer/dist`.

### What the page guarantees, per account type (review 0-4 / 0-5)

| Account | The page guarantees | It does not guarantee |
| --- | --- | --- |
| **`getvela.app`** (made in the apps or on the official page) | **display assurance**: it decodes the request with its own code, from bytes that match a published version (from the checked bytes on the web; pinned elsewhere after the first visit), and signs only the digest it computed itself. A tampered or wrong wallet interface cannot make it show one thing and sign another | **authority**: the web wallet's origin, the extensions and the apps can all call the same passkey directly (`signing_venue.rs`). A fully compromised wallet, extension or app does not need the page. On the web the wallet's own code runs the check, so a compromised wallet can also open a page of its choosing |
| **Its own domain** (made on a self-hosted page) | display assurance as above, **and authority**: only pages on that domain can use its keys, so the wallet cannot sign without the page | an honest page host on the first visit (trust on first use), and until the pin is in place; the page's host chooses what `sw.js` it serves |

The Settings copy already says the getvela.app limit in each language
(`settings.venue.pageLimit`: "It can't protect you if this wallet itself is compromised…").

### Owner decision: a page-scoped RP ID (the real fix, not built this round)

A `getvela.app` account's page is display assurance, not authority, because the keys live
under an RP ID that the wallet origins and the app associations can also use. The fix is an
account whose keys live under an RP ID that **only the page** can use: either a separate
registrable domain (for example `getvela-sign.app`), or `sign.getvela.app` itself as the RP ID
with no other origin on it and no app association for it. The wallet and the apps then cannot
call the passkey at all. They can only hand a request to the page, which becomes the one thing
able to sign, the boundary the review asks for.

Trade-offs:
- **Every signature goes through the page.** That means a browser window on every surface,
  every time; no native passkey sheet; and the page must be reachable (or cached by its worker).
- **Recovery and new devices** need the page, and key ceremonies (create, add a key, sign in)
  move to it, as a custom-domain account's already do (spec 102 R3).
- **The domain becomes the root of trust for those accounts.** Its registrar, DNSSEC, CAA and
  HSTS preload all matter more, and the domain can never be given up.
- **Existing accounts cannot move.** A key's RP ID is fixed. It would be a new kind of account
  ("page-only"), offered beside `getvela.app`, with a migration by sending funds.
- **Multisig members** would sign on the page every time. That is the Bybit lesson's ideal, at
  a cost in convenience.
- **The opener question.** The page must still not answer a phishing opener (it answers the
  wallet origins only, PROTOCOL.md §12.3). The wallet still assembles the operation, and the
  page re-derives what it signs.

Recommended: offer it as an opt-in "page-only signing" account type for high-value and multisig
use, on a dedicated registrable domain, after the domain-security work (review 1-1's
DNSSEC/CAA/registry lock) is done. The owner decides; nothing here depends on it.

## Round 22 go-live (the owner)

> **R23 note (checked at `fa6083c87`).** The lanes below each name their own page as "first in
> `BUILD_ALLOWED`". After the merge, the first entry of `integrity::BUILD_ALLOWED` is
> `e95fb59d…`, one page carrying both `674b7333`'s opening from bytes and Service Worker pin and
> `e7ee3925`'s Permit2 witness reading and 2^128 line; `integrity::FROM_BYTES` lists it and
> `674b7333`. Deploy `app-web/trusted-signer/dist/` as built (it holds every listed version), and
> check `e95fb59d…` with the `curl … | shasum -a 256` step. `integrity::LAUNCH` is still
> `6d7172b4…`.

1. **Deploy the signing page** `app-web/trusted-signer/dist/`, including the new `dist/sw.js`,
   with page `674b7333` first in `BUILD_ALLOWED`.
   - Check that `curl -s https://sign.getvela.app/b/674b7333…/sign | shasum -a 256` prints
     `674b7333…`, and that `/sw.js` is served as JavaScript.
   - Until it is deployed, the web wallet opens the newest version the deployed `index.json`
     lists. That version is not in `FROM_BYTES`, so it opens at its address, as before. Nothing
     breaks; the bytes-opening starts once `674b7333` is live.
   - `LAUNCH` is unchanged. Move it with R21's page, once both are deployed.
2. **Branch protection:** add `supply-chain`, `signing-page`, `web-core-reproducible`, `site`
   and `web` to the required checks (and `rust` / `rust-macos`, as ci.yml notes). Until a job is
   required, "blocking on every PR" in these documents means "runs and goes red on every PR";
   a red job does not stop a merge. This repository cannot show which checks are required.
3. **Turn on Dependabot alerts and security updates** (Settings → Code security). The file
   alone only schedules version updates.
4. **Desktop release blocker:** move the gpui pin to zed `ac5af8b9` or later (the GPL-3.0
   crates relicensed to Apache-2.0), refresh `vendor/gpui_macos`, and delete the three
   exceptions in `deny.toml`. Or explicitly accept the licence risk for one release.
5. **getvela.app's Worker**: bump kit, svelte and devalue (bun audit, in range).
6. **Decide on the page-scoped RP ID** (above).
7. **Read the first `web-core-reproducible` run.** If the Linux container's bytes differ from
   the committed (macOS-built) module, commit the container's build, which the job keeps as an
   artifact, and run `build:wasm` in the container from then on (R22-2).
8. **The phones' page path end to end** (the R21 tester): build the harness for a page-signed
   send and a multisig approval on an emulator or simulator, using `VELA_PAGE_LAUNCH` (above).
9. **Round 22 — signing page `e7ee3925…`** (first in `BUILD_ALLOWED`; `LAUNCH` unchanged):
   deploy `app-web/trusted-signer/dist/` with it.
10. **Round 22 — the genesis service** (`sgr-mailbox`): review and commit the uncommitted
    changes on a `mailbox-r22` branch (§10), push, deploy, and pin it for the nightly fork
    job (`SGR_MAILBOX_REF`). Until `vars.SGR_MAILBOX_REF` is set, the nightly `multisig-fork`
    job skips itself, and with it every fork test (`multisig_fork`, `signing_truth_fork`,
    `lying_index_fork`, `safe_op_window_fork`, `erc6492_fork`).
11. **Merge to `main` before pointing anyone at the published links** (final review). These
    link to files under `main` that exist only on `feat/multisig-account`, so they 404 until
    the merge: security.txt's `Policy` (`…/blob/main/SECURITY.md`, served by both sites), the
    audits page's links to SECURITY.md, SECURITY-MODEL.md, `docs/security/advisories` and
    `incidents.md`, and the whitepaper's link to SECURITY-MODEL.md. The URLs are right for after the
    merge and are not changed.
12. **After that merge, read its `web-core-reproducible` run** (item 7). Only a `main` run whose
    rebuild matches attests the web core; until one has, `gh attestation verify` on the
    served module fails, and verify.md says so.
13. **getvela.app's analytics script** (§4, "Left open"): remove it, or self-host a pinned copy
    with Subresource Integrity, and add a `script-src` CSP.
