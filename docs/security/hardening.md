# Hardening: closing the doors (Round 21), making the defences true (Round 22)

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
  reads them too.
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
6. **Round 22 — signing page `e7ee3925…`** (first in `BUILD_ALLOWED`; `LAUNCH` unchanged):
   deploy `app-web/trusted-signer/dist/` with it.
7. **Round 22 — the genesis service** (`sgr-mailbox`): review and commit the uncommitted
   changes on a `mailbox-r22` branch (§10), push, deploy, and pin it for the nightly fork
   job (`SGR_MAILBOX_REF`).
