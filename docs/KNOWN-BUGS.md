# Known bugs

## R22-PLAN-1 (planned, feature, every surface; from review 1-12, 2026-11-07) — no allowance list, no revoke screen

**Platform:** every surface. **Gap:** Vela shows and caps an approval when a site asks
for one (§37: carried approvals, witness permits, a waiver per leg, unlimited from
2^128), but nowhere lists the allowances an account has already granted, or offers to
revoke one. A reviewer expects one: a standing unlimited approval from months ago is
still a drainer's way in. **Status:** a feature, so out of the frozen Round 22; planned
— one core machine that reads `Approval` / `ApprovalForAll` logs and Permit2's
`allowance` per token and spender, through the wallet's own RPC (never a service's
word), and builds the revoke as an ordinary send (`approve(spender, 0)`,
`setApprovalForAll(operator, false)`, Permit2 `lockdown`), with a row per platform.
**Workaround:** revoke from the token's own site, or a block explorer's token-approval
page.

## R20-S1 (✅ FIXED 2026-11-08, §34; found 2026-11-07, iOS, Arc Testnet) — a deploying account's Max charged a fee larger than shown

**Platform:** found on iOS (the rule was the core's, so every shell). **Symptom:**
"R20 iOS ZH" (0.3 USDC, never deployed — its first send deployed it), zh: Send →
USDC → Max → payee → Continue; the confirm read 0.0675 USDC with 预估手续费 0.2325
USDC; the chain took 0.0058875 to the payee and a 0.29411249999 fee leg (tx
`0x5c832ead9782777a38e92ca41b50ca36b07004a30c14cde91a1ee3f989052357`) — 0.0616 more
fee than shown. The receipt said "已发送 0.005888 USDC". Seen 1 in 3. **Severity:**
S1 (charged ≠ shown). **Cause:** a re-quote landed during the press's own reads
(credential, the relay's float); the confirm drew it beside the amount pressed,
the submit found the review moved and returned the button without a word, the
Max was re-derived from the new quote, and the next press signed it.
**Fix (§34):** the press freezes the fee and amount drawn (marked
`send.feeSigning`) until the signature, a cancel or a stop; a quote that moved by
the signature signs nothing and is said beside the confirm with the new figures
(`send.feeMoved` / `send.feeMovedMax`); the next press signs exactly those. Proved
in the core (vector `press_freeze`), on every shell, and on the fork harness.

## SEC-PARENT-1 (✅ FIXED 2026-11-07, §31; reported 2026-09-29) — a site could make the account act for a multisig it is a member of

**Reported** in the 2026-09-29 disclosure package (`APPENDIX-PARENT.md`): the
self-call guard refused Safe-admin calls only when their target was the signing
account, so a connected site could make the account act for a Safe it owns — a
Vela multisig it belongs to — through `parent.execTransaction(… v = 1)` (a
1-of-N takeover), `parent.approveHash(h)`, owner/module/guard changes (nested in
a MultiSend too), or a `SafeMessage` / `SafeTx` signature under the parent's
domain (a blind "Sign anyway"). **Fix:** the parent-Safe guard
(`vela_core::app::parent_safe_guard`, docs/multisig-machines.md §31): refused
with no "anyway" for this device's multisigs and for any Safe whose
`isOwner(account)` reads true, "couldn't check" when that read cannot be made;
again at submit; independently on the signing page (2c649b76). Proved on the
fork harness (`multisig_fork.rs` `parent::…`).

## R20b-1 (open, S4, 2026-11-08, web, Chrome extension, Safari extension, desktop) — found in the R20b re-verification of §34

**Platform:** web, Chrome extension, Safari extension, desktop. **Symptom:** A red "Insufficient USDC to cover amount + gas fees" can show under a frozen press whose own figures fit (computed against the newer, unshown quote, or the balance re-read after the operation landed). The press either stops or sends correctly; no money effect. **Severity:** S4. **Workaround:** Trust the frozen "Signing at this fee and amount…" line.

## R20b-2 (open, S4, 2026-11-08, iOS) — found in the R20b re-verification of §34

**Platform:** iOS. **Symptom:** A sweep's Max confirm says "Max is now 0.05 USDC (it was 0.014)" when the form already showed 0.05 at Continue. Charged = shown. **Severity:** S4. **Workaround:** Cosmetic.

## R20b-3 (open, S4, 2026-11-08, web (stop logic shared)) — found in the R20b re-verification of §34

**Platform:** web (stop logic shared). **Symptom:** A multi-token sweep's stop says only that the fee changed, though the USDC line moved too (0.28 → 0.244). **Severity:** S4. **Workaround:** Read the lines before confirming again.

## R20b-4 (open, S4, 2026-11-08, web) — found in the R20b re-verification of §34

**Platform:** web. **Symptom:** The stop sentence can be replaced about 2 s later by the next re-quote's "Max is now…" line. Nothing was signed; the next press charges what is drawn. **Severity:** S4. **Workaround:** Read the figures before pressing again.

## R20-2 (open, S3, 2026-11-08, Android) — found in the R20 release-candidate acceptance

**Platform:** Android. **Symptom:** Testnet, fresh install, first switch from Mainnet: "No activity yet" beside "Can't reach 2 networks right now" from about 21 s to 39 s (r18-1 fixed on Mainnet, web and iOS, not here). **Severity:** S3. **Workaround:** Wait for the networks to answer, or pull to refresh.

## R20-3 (open, S4, 2026-11-08, desktop) — found in the R20 release-candidate acceptance

**Platform:** desktop. **Symptom:** The parent-Safe refusal names the Safe in lowercase ("0xd276…1ef9"); the desktop twin of r18-23. **Severity:** S4. **Workaround:** Compare the address case-insensitively.

## R20-4 (open, S4, 2026-11-08, desktop) — found in the R20 release-candidate acceptance

**Platform:** desktop. **Symptom:** An Activity row can keep the language it was recorded in (a zh row in the English UI); intermittent. **Severity:** S4. **Workaround:** Open the row; the sheet is in the current language.

## R20-5 (open, S4, 2026-11-08, desktop) — found in the R20 release-candidate acceptance

**Platform:** desktop. **Symptom:** In the dark theme, Settings → Account's "Create New Account" button is white on light grey (about 1.2:1 contrast). **Severity:** S4. **Workaround:** Use the switcher's Create New Account, or the light theme.

## R20-6 (open, S4, 2026-11-08, desktop) — found in the R20 release-candidate acceptance

**Platform:** desktop. **Symptom:** Approve and execute pressed under a refused Arc draws the review's "can't check" line, not "nothing was sent" beside the button (r18-13 partly fixed). Nothing is signed. **Severity:** S4. **Workaround:** Check again once the network answers.

## R20-7 (open, S4, 2026-11-08, iOS) — found in the R20 release-candidate acceptance

**Platform:** iOS. **Symptom:** Settings → Change members shows the plan line "2 of 0 → 2 of 0" under a correct header; the review sheet after the press is correct. **Severity:** S4. **Workaround:** Read the rule on the proposal's review sheet before approving.

## R20-8 (open, S4, 2026-11-08, iOS) — found in the R20 release-candidate acceptance

**Platform:** iOS. **Symptom:** The deep4 and delegate member sheets have no Reject (R18-15's twin). Vela never signs these. **Severity:** S4. **Workaround:** Reject from web or desktop, or replace it with another proposal at the same number.

## R20-9 (open, S4, 2026-11-08, iOS) — found in the R20 release-candidate acceptance

**Platform:** iOS. **Symptom:** Change members still says "Proposed on Arc Testnet" right after the change executed. **Severity:** S4. **Workaround:** Reopen Settings.

## R20-10 (open, S4, 2026-11-08, iOS) — found in the R20 release-candidate acceptance

**Platform:** iOS. **Symptom:** A deployed multisig's home keeps the first-run cards ("Get it ready / Add funds…"). **Severity:** S4. **Workaround:** Dismiss them with Got it.

## R20-11 (open, S4, 2026-11-08, Safari Web Extension) — found in the R20 release-candidate acceptance

**Platform:** Safari Web Extension. **Symptom:** After declining a multisig dApp request and switching the front account, the request tab stays on "Opening the transaction…". **Severity:** S4. **Workaround:** Press Done.

## R20-12 (open, S4, 2026-11-08, iOS) — found in the R20 release-candidate acceptance

**Platform:** iOS. **Symptom:** After creating an account, Enter wallet lands on the previous front account. **Severity:** S4. **Workaround:** Switch accounts in the header.

## R20-13 (open, S4, 2026-11-08, Android) — found in the R20 release-candidate acceptance

**Platform:** Android. **Symptom:** For 1.0–1.7 s after an execution the chip says "Executing…" over an "Executed" body (r18-16 partly fixed). **Severity:** S4. **Workaround:** Transient.

## R20-14 (open, S4, 2026-11-08, Android (test builds only)) — found in the R20 release-candidate acceptance

**Platform:** Android (test builds only). **Symptom:** After a §28-refused create, the home line says "Vela keeps trying" though nothing is retried. **Severity:** S4. **Workaround:** Point the build at a local service; use Share again.

## R20-15 (open, S4, 2026-11-08, Android) — found in the R20 release-candidate acceptance

**Platform:** Android. **Symptom:** A press stop reads "That didn't go through: Network fee ≈ …", naming the fee line as the reason. Nothing is signed. **Severity:** S4. **Workaround:** Press Check again, then Execute.

## R20-16 (open, S4, 2026-11-08, Android) — found in the R20 release-candidate acceptance

**Platform:** Android. **Symptom:** Wording: the zh Send form says the relay "couldn't test-run… retrying" before anything is typed; the unchecked-owner refusal says "when you're online" while the device is online; the hash copy button is announced "Copy address". **Severity:** S4. **Workaround:** Cosmetic.

## R20-17 (open, S4, 2026-11-08, Android) — found in the R20 release-candidate acceptance

**Platform:** Android. **Symptom:** A cold start flashes a deployed multisig's "Get it ready" card for about 0.2 s before the settled home. **Severity:** S4. **Workaround:** Clears itself.

## R20-18 (open, S4, 2026-11-08, web) — found in the R20 release-candidate acceptance

**Platform:** web. **Symptom:** The approvals heading says "Executed" while "Executing…" is still in the sheet for 0.7–1.1 s. **Severity:** S4. **Workaround:** Transient.

## R20-19 (open, S4, 2026-11-08, outside /connect page, zh) — found in the R20 release-candidate acceptance

**Platform:** outside /connect page, zh. **Symptom:** The approval title doubles the ordinal: "批准第 第 5 号 号". **Severity:** S4. **Workaround:** Cosmetic.

## R20-T1 (open, test only, 2026-11-08, web) — `FeedbackBody` "keeps Tab inside itself" fails under full-suite load

**Platform:** web client vitest. **Symptom:** fails once under a fully loaded host, passes 3/3 alone. **Severity:** test flake, no product effect.

## R18-11 (open, S4, 2026-11-07, desktop) — a site's request to the connected multisig flashes a "passkey prompt was closed" sheet first

**Platform:** desktop. **Symptom:** a request arriving as the multisig (in-app browser, a site connected to a multisig) shows a sheet that says the passkey prompt was closed, with live Decline / Try again, for 0.7–2 s before the parent-Safe refusal replaces it (Round 18, desk-2). Nothing is signed; the refusal and the site's `-32603` are right. **Severity:** S4. Not fixed in Round 19 (scope frozen).

## R18-12 (open, S4, 2026-11-07, desktop) — after a multisig's parent refusal the tab's banner blames the relay

**Platform:** desktop. **Symptom:** after a connected multisig's request is refused in the parent-Safe words, the tab's status banner says "Vela's relay couldn't take it · eth_sendTransaction" — the Vela-account case says "Vela refused this request" (Round 18, desk-3, shot desktop-61). **Severity:** S4.

## R18-15 (open, S4, 2026-11-07, Android) — the deep4 and delegate member sheets have no Reject

**Platform:** Android. **Symptom:** a member's sheet for a transaction Vela refuses to sign (deep4, delegate) offers no Reject, where the web, iOS, desktop and the outside page (r17, `can_reject` on Blocked) do. **Severity:** S4.

## R18-16 (open, S4, 2026-11-07, Android) — "Executing…" over an "Executed" body after an execution

**Platform:** Android. **Symptom:** for about 2.2 s after an execution lands, the chip says "Executing…" over a body that already says "Executed", with a transient "isn't recorded" line (r17-6's family). **Severity:** S4.

## R18-17 (open, S4, 2026-11-07, Android) — after a locale change an "Opening the transaction…" sheet never resolves

**Platform:** Android. **Symptom:** a proposal sheet opened after switching the app's language stays on "Opening the transaction…" and never shows the transaction. **Severity:** S4.

## R18-19 (open, S4, 2026-11-07, iOS) — the executing member's fee has no History row

**Platform:** iOS. **Symptom:** the member who executed a multisig transaction paid its network fee (B 0.248201775, A 0.12 in Round 18), but that member's own History shows no row for it — only "+0.5 Received", still an hour later. **Severity:** S4.

## R18-20 (open, S4, 2026-11-07, iOS, web) — "Couldn't check whether it would run" lingers after the network recovers

**Platform:** iOS, web. **Symptom:** after an outage lifts and one Try again recovers the review, the fee line keeps "Couldn't check whether it would run…" for 23 s on iOS and 7–28 s on the web before the priced line returns. No "… anyway" is drawn meanwhile. **Severity:** S4.

## R18-21 (open, S4, 2026-11-07, iOS, Android) — a refusal aimed at the connected multisig's own address offers a Try again that can't help

**Platform:** iOS, Android. **Symptom:** a site's request refused because it targets the connected multisig's own address shows Try again, which can only meet the same refusal. **Severity:** S4. Related to the §31 open item below (the self-call rule for a connected multisig's own address).

## R18-22 (open, S4, 2026-11-07, desktop, dev-only) — a parallel-space keyset replaced an existing fixture account

**Platform:** desktop (dev-fixtures builds only). **Symptom:** entering the parallel space with a keyset re-created a fixture account over an existing one in the profile, instead of refusing or adding beside it. Release builds have no parallel space. **Severity:** S4.

## SEC-PARENT-2 (✅ FIXED 2026-11-08, §35; found 2026-11-07, every platform; §31 left open) — a connected multisig's own address is not held to the self-call rule

**Platform:** every wallet surface. **Symptom:** a site connected to a multisig is held to the parent-Safe rule (§31), but not to the self-call list for the multisig's own address: a site may propose an owner, threshold or module change of the multisig itself. Each such request is still a proposal every member reviews before it can run (the review names the member change), so nothing is signed or executed by the site alone. **Severity:** S4 (recorded in §31 "Left open").
**Fix (§35, `docs/security/hardening.md` §5):** the account's self-call rule holds for the multisig's own address — refused at arrival (nothing drafted) and again at the proposal step on the very transaction about to be signed, in the same words (`componentsUi.signing.multisigSelfCallBlocked`, `data.reason = "self_call"`). Vector `multisig-self-call.json` (13 cases) replayed by the core and through the real core at every shell's multisig dApp entry.

## SEC-PARENT-3 (✅ FIXED 2026-11-08, §36; found 2026-11-07, every platform; §31 left open) — an unlisted Safe's ownership answer comes from one RPC

**Platform:** every wallet surface. **Symptom:** for a Safe that is not on this device's multisig list, whether the account owns it is one `isOwner(account)` `eth_call` answered by the request chain's RPC pool; a lying or compromised node answering `false` would clear the request (a Safe on the list never depends on the read; a node that fails or answers junk refuses "couldn't check"). The signing page stays stricter (it refuses these shapes on any Safe but the account). **Severity:** S4 (recorded in §31 "Left open").
**Fix (§36, `docs/security/hardening.md` §12):** the read is corroborated — the pool asks a second, independent provider (each checked for its chain id), and "not an owner" / "not a Safe" clears the request only when both agree (`parent_safe_guard::classify_corroborated`), or when the person's own node answered alone ("Only use my node"). One node's "no", or two that differ, refuses "couldn't check". The same holds for a multisig's dApp flow. Vector `signing-truth.json` `owner` (8 cases).

## ANDROID-1 (open, 2026-09-12, spec 047) — the parallel-space flag was found off at 047's first launch

**Symptom:** after 046's last device run (badge on, fixture account active), 047's first
launch showed the founder's real account and the light theme; `shared_prefs/vela.parallel.xml`
read `active=false`. No sign-out was tapped and no launch carried `vela.parallelSpace=false`.
**Status:** not reproduced since; re-entered with `--ez vela.parallelSpace true`. Watch for a
second occurrence before hunting; `DebugParallelSpace.leave()` is the only writer.

## ANDROID-2 (open, 2026-09-12, spec 045) — the sweep pick opened during the first launch after install once showed no rows

**Symptom:** tapping 发送多个代币 before the balances had arrived, on the first launch after
an install, left the pick empty for the wait's length; the plain pick and later cold starts
showed rows in ~2 s. Not reproduced.

## ANDROID-3 (fixed 2026-09-12, spec 046) — SIWE verdicts printed the corpus placeholders

**Symptom:** 044's signing sheet showed `已验证登录到 {{domain}}` raw. **Fix:** the words take
the domain and the origin (`SigningLive.messageBlocks`).

## BUG-7 (✅ FIXED 2026-07-09, fund-path) — an extension sign could be double-submitted on a re-launch

**Symptom / risk:** the App-Group sign mailbox had no lifecycle — `sign-req-<rid>.json`
was never consumed, had no TTL, and the app never checked whether a result already
existed for the rid. So a **cold relaunch of the same rid** (the app killed right after
submitting, or the user tapping "返回 Vela" while the result was still in transit)
re-read the still-present sign-req and re-rendered `SigningRequestModal` → a second
approve could submit the **same transaction twice**. Violated the §12.5 GO/NO-GO gate
(d) "no double-submission across the concurrent/reload matrix".

**Fix (`src/services/extension-bridge-transport.ts`):** `connect()` now reads
`sign-result-<rid>.json` FIRST; if a result already exists the sign is done — it
**replays the prior outcome and never re-emits the request** (no second modal, no
re-sign; `alreadySettled` drives `sign.tsx` straight to the settled state). Added a
5-minute request-payload TTL (§12.1.4) so a stale/leaked rid is refused, never signed.
Covered by 6 unit tests (`src/__tests__/extension-bridge-transport.test.ts`). Related
hardening the same day: funding retry is now request-id-bound (can't replay old params
under a new id), and the extension sign reconciles to the origin's GRANTED account
before signing (§12.1.6 — never silently sign from the wrong account).

## BUG-1 (✅ FIXED 2026-07-06, fund-path) — a send that needs gas-account funding silently did nothing

**Discovered:** 2026-07-06, while device-testing `eth_sendTransaction` from the Safari
extension on Gnosis (fixture Safe "Parallel One").

**Symptom:** on the signing sheet for an `eth_sendTransaction` (or `wallet_sendCalls`),
tapping **授权 / Approve** produces **no visible response at all** — no spinner, no error,
no modal. The transaction never submits. Reproduced by the founder with a real tap
(so it is NOT a test/automation artifact).

**Root cause (confirmed from Metro logs):**
1. Vela's gas model gives each Safe a **dedicated gas-account EOA per chain**
   (`bundler-service.ts` header) that pays the UserOp's gas, separate from the Safe's
   own balance. The bundler normally auto-sponsors new users from its treasury; if that
   is unavailable the gas account is empty.
2. On approve, `approveRequest` (`src/models/dapp-connection.tsx`) runs a proactive
   `checkBundlerFunding` pre-check. With the gas account empty it returns "deposit
   needed", so approveRequest does `setFundingNeeded(funding); return;` — i.e. it
   ABORTS the send and asks the UI to show a top-up prompt.
   ```
   [BundlerFunding] deposit=0x…154d03 balance=0 status=INSUFFICIENT_BALANCE
   [BundlerFunding] threshold=… spendable=0 sufficient=false
   ```
3. **The top-up prompt never appears.** `BundlerFundingModal`
   (`src/components/ui/BundlerFundingModal.tsx`) is built on `AppModal`, which is a
   **native iOS `<Modal presentationStyle="pageSheet">`** (`src/components/ui/AppModal.tsx`).
   The signing sheet is ALSO an `AppModal`. `BundlerFundingModal` is rendered as a
   sibling **stacked on top of** the signing sheet's `AppModal`
   (`src/components/SigningRequestModal.tsx:700`). iOS does not present a second native
   modal over an already-presented one (this app has hit modal-over-modal issues before,
   see [[project_native_readiness_audit]]) → the funding modal renders **invisibly behind
   the sheet**. So the user taps 授权 and sees nothing; the send is silently blocked.

**Why it matters:** this is not fixture-specific. ANY real user whose gas account isn't
auto-sponsored (treasury empty / not eligible) will tap Approve on a send and get NOTHING
— a fund-path dead end with no feedback. (Separately worth checking on the bundler side
why Gnosis auto-sponsorship didn't fund the gas account.)

**Secondary bug (same path):** the `checkBundlerFunding` pre-check in `approveRequest`
runs for up to 15s BEFORE `setIsSigning(true)`, so even on the happy path tapping 授权
gives no feedback (no spinner) during that window — it "looks dead" for up to 15s.

**Fix (shipped 2026-07-06) — the single-modal content swap:**
- **Primary (invisible modal):** the funding UI body was extracted into a wrapper-less
  `BundlerFundingView` (`src/components/ui/BundlerFundingModal.tsx`); the standalone
  `BundlerFundingModal` (still used by the Send screen) is now a thin `<AppModal>` around
  it. `SigningRequestModal` no longer renders a *second* `AppModal` — it renders **one**
  sheet whose CONTENT swaps: `fundingNeeded ? <BundlerFundingView/> : <SigningSheet/>`.
  One native modal is only ever presented, so the funding UI is always visible + tappable.
  Swipe-to-dismiss over the funding view routes to `handleFundingCancel` (matches its 取消).
- **Secondary (no feedback):** in `approveRequest` the `setIsSigning(true)` /
  `setSignError(null)` / `setPendingOpHash(null)` were moved **above** the ≤15s gas
  pre-check, so Approve shows the spinner instantly; the funding-needed branch does
  `setIsSigning(false)` before handing off to the in-sheet funding view.
- **Hygiene:** `handleIncoming` clears `fundingNeeded` on every new signing request (so a
  stale funding view can't hijack the next sheet via the content-swap); `rejectRequest` /
  `dismissRequest` also clear it. `handleFundingComplete` now clears the bundler cache for
  the **request's** chain (`incomingRequest?.__chainId ?? chainIdRef.current`) so an
  extension sign on a non-global chain doesn't loop the funding prompt.
- **Dev harness:** `vela.forceFunding(chain|'all')` (`src/services/dev/fault-injection.ts`)
  makes `checkBundlerFunding` report "deposit needed" so the funding UX is reproducible
  without draining a gas account (dev-only; always false in prod).

**Verification:** typecheck clean; full jest 1058/1058; 4-lens adversarial review (fund-safety
/ react-lifecycle / ios-modal / send-regression) found **no defect introduced by the change**.
**Device confirmation still recommended** (the native-modal visibility is a device-only
property): on the DEV build, trigger a dApp/extension `eth_sendTransaction` whose gas account
is empty (or the natural repro from discovery) → the funding sheet must now appear **visible
and interactive in place of** the sign sheet, and tapping Approve must show a spinner
immediately. Old behavior = silent dead tap.

**Parallel-space test env (免费激活 can't work there):** the parallel space uses the REAL
bundler, but its fixture Safes were seeded into storage (never through the create flow), so
their fixed-key P-256 pubkeys were never uploaded to the bundler's index → free sponsorship
is always denied (`no_passkey_registered` → "免费激活需要通行密钥。"). **Founder decision
2026-07-06:** keep sponsorship real; **self-fund the fixture gas EOAs manually** (like
Parallel One's gas account `0xb32a3965c4823ea426de52c7e869dd0cfe154d03` funded 0.01 xDAI on
Gnosis). So in the parallel space the funding view now **skips the "choose" step and leads
with self-fund** (`BundlerFundingView` gates the initial `step` on `__VELA_PARALLEL__`) —
fund the gas-account EOA shown, the balance poll detects it, Continue → the sign proceeds.
(Prod is unchanged: choose step → real sponsorship → denial falls back to self-fund.)

**Files:** `src/components/ui/BundlerFundingModal.tsx` (BundlerFundingView split + test-env
self-fund lead), `src/components/SigningRequestModal.tsx` (single-modal content swap),
`src/models/dapp-connection.tsx` (approveRequest ordering + funding hygiene + chain fix),
`src/services/dev/fault-injection.ts` + `src/services/bundler-service.ts` (forceFunding seam).

---

## BUG-2 (✅ FIXED 2026-07-06, fund-path) — a reject/swipe during an in-flight approve still submitted the tx

**Surfaced:** 2026-07-06 by the adversarial review of the BUG-1 fix (pre-existing; BUG-1
didn't introduce it). `approveRequest` captured `base = incomingRequest` and never
re-validated after its awaits, and `AppModal` swipe-dismiss is ungated by `isSigning` (unlike
the Reject button). A swipe → `rejectRequest` (4001) while the in-flight approve kept going →
a "rejected" tx that still broadcasts **plus** a contradictory success response for the same id.
Two distinct windows: the ≤15s gas **pre-check** (before any auth) and the **submit** window
(after the passkey resolves, before `onSubmitted` — the bundler round-trip).

**Fix (shipped) — two complementary guards, both windows closed on all platforms:**
- **Pre-check window** — `signCancelledRef`: `rejectRequest` sets it; `approveRequest` resets
  it at entry and, right after the pre-check `await` (before `handleDAppRequest`), **aborts
  if set**. So a reject before auth truthfully 4001s and never submits.
- **Submit window** — a reactive `isSubmitting` flag: set true right before the passkey/submit
  (the tx is now committed once authenticated), reset in `finally`; `SigningRequestModal`'s
  `onClose` routes to `dismissRequest` (not reject) when `signError || pendingOpHash ||
  isSubmitting`. So a swipe during submit **dismisses** — the op proceeds and its real result
  is delivered; no 4001, no contradiction. (`pendingOpHash` covers the same span from submit
  onward — doubly covered.)
- **Web parity** — `AppModal`'s web `DragHandle` created its `PanResponder` once, capturing a
  stale `onClose`; now reads the latest via `onCloseRef` (the pattern AndroidSheet already used),
  so a web drag-dismiss during submit also routes to dismiss, not the stale reject.

**Verified:** typecheck + jest 1058/1058; a follow-up adversarial verify confirmed CLOSED on
iOS/Android (no stuck state, pre-check reject still a real reject, funding flow intact) and the
web `DragHandle` gap is the one it flagged — now also fixed. **Files:** `dapp-connection.tsx`
(signCancelledRef + isSubmitting + approveInFlightRef), `SigningRequestModal.tsx` (onClose
routing), `components/ui/AppModal.tsx` (web DragHandle onCloseRef).

## BUG-3 (✅ FIXED 2026-07-06, fund-path polish) — plain-tap approve buttons had no debounce

**Surfaced:** 2026-07-06 by the adversarial review (pre-existing + systemic). The ordinary
Approve `VelaButton` and the funded **继续/Continue** `Pressable` had no synchronous re-entry
guard — `disabled`/`loading` ride async `isSigning`, which hasn't flipped on a same-tick second
tap — so a double-tap fired two concurrent `approveRequest` calls → two passkey prompts.
(Capped by ERC-4337 nonce enforcement: at most one UserOp lands; worst case was a contradictory
dApp response, not lost funds.)

**Fix (shipped):** a synchronous re-entrancy lock at the single submit path — `approveInFlightRef`
in `approveRequest`: acquired at entry (a second concurrent call returns immediately), released
on **every** exit (the funding hand-off, the cancel-abort, and the `finally`). This covers ALL
approve entry points (Approve button, funding Continue, any future one) at the point that
matters — the submit function — rather than per-button. Verified: typecheck + jest 1058/1058;
the adversarial deadlock lens found no path that strands the lock (`serializeAssetSim` can't
throw; the pre-check is `try`-wrapped; all returns release). **File:** `dapp-connection.tsx`.

## BUG-4 (✅ FIXED 2026-07-06 — WalletPair) — pairing threw "crypto.getRandomValues must be defined"

> 2026-09-11: the runtime this bug lived in — the React Native / Expo app, its
> module-evaluation order under Metro and the `index.js` polyfill entry — was retired in spec
> 039, together with WalletPair itself (ruled out for the web in spec 027).
> Kept as the record of the class of bug (a module capturing `globalThis.crypto`
> once at evaluation); nothing below can recur in the current shells.

**Discovered:** 2026-07-06, building the concurrent-session DEVICE harness
(`e2e/safari/check_concurrent.py` — the first time a REAL WalletPair session was
established on the device; WalletPair has ≈0 real dApp adoption, which is *why* the
Safari extension exists, so this path was never device-tested before).

**Symptom:** on the device (parallel space, DEV build), entering a valid pairing URI
on the Connect screen → **"连接失败 · crypto.getRandomValues must be defined"**.
`WalletPairTransport.prepare()` → `session.prepareJoin()` → X25519 keygen throws.
Reproduces on a FRESH app start (not stale state). The Node peer + the real relay work
fine — the failure is entirely app-side crypto.

**Root cause:** `@noble/hashes/crypto` captures `globalThis.crypto` **once at module
evaluation** (`@noble/hashes/utils.js`: `const crypto_1 = require('@noble/hashes/crypto')`,
then at call time `crypto_1.crypto.getRandomValues`). If that module evaluates BEFORE
`react-native-get-random-values` installs `globalThis.crypto`, noble holds `undefined`
forever. The pod IS linked (`ios/Podfile.lock` has `react-native-get-random-values 1.11.0`)
and `src/polyfills.ts` imports it as `_layout.tsx`'s first line — yet on device it still
loses the race (some module imports @noble before `_layout`'s polyfills line, or the native
RNG isn't installing `getRandomValues` at import). `polyfills.ts`'s own comment already warns
"Without these, WalletPair (Vela Connect) pairing throws on the first scan."

**Fix approach (needs a native rebuild to verify):** guarantee `react-native-get-random-values`
runs before ANY `@noble/*` module — e.g. a custom entry `index.js` (`import '@/polyfills';
import 'expo-router/entry';`) set as `package.json` `main`, instead of relying on `_layout`
import order; OR at the top of `polyfills.ts` assert `globalThis.crypto?.getRandomValues` is a
function after the import and install a fallback (e.g. `expo-crypto`) if not. Then re-run
`e2e/safari/check_concurrent.py` — the harness is ready and drives the full flow (parallel arm →
Connect screen → URI entry → connectToWalletPair → fingerprint → concurrent extension sign →
assert WP survives + no leak); it currently stops at this crypto throw.

**Fix (shipped + DEVICE-VERIFIED 2026-07-06):** a custom entry `index.js`
(`import './src/polyfills'; import 'expo-router/entry';`) set as `package.json` `main` — so
the RNG polyfill installs `globalThis.crypto` before ANY `@noble/*` module (and thus before
the module-load capture) unconditionally. `polyfills.ts` also fails loud now if
`crypto.getRandomValues` is still missing after the import. Verified on the device (ABC iPhone
11): after `main→index.js` + a Metro restart (`iOS Bundled index.js`), the polyfill "MISSING"
warning does NOT fire, and WalletPair pairing no longer throws — the fingerprint-verify screen
renders and Confirm proceeds (the crypto crash is gone). NO native rebuild needed — the entry
change is picked up by re-bundling.

**Note:** the two-slot ROUTING isolation (F2/F3/F4) is independently proven headless
(`src/__tests__/concurrent-session.test.ts`) and is unaffected by this.

## BUG-5 (✅ RESOLVED 2026-07-06 — was a harness mis-declaration, NOT an app bug) — WalletPair peer rejected the app's join

**Resolution:** this was a pre-v1 protocol harness problem. The current dApp peer
(`e2e/safari/wp_peer.mjs`) implements the published relay/encryption/Ethereum protocol directly;
there is no capability negotiation or legacy `wallet_*` method map. **The app's WalletPair works.**
(BUG-4 crypto + BUG-6 stale-session were the real app bugs on this path; both fixed.) The device
concurrent proof now gets `wp_connected` ✓, `wp_survived` ✓ (WP still connected
after the extension sign — two-slot survival), `no_leak` ✓ (the WP peer received only its own
`accountsChanged`/`chainChanged`, never the extension signature — F2), with `ext_real_sig` the only
flaky step (independently 4/4 in `check_real_sign.py`; chained after the WP setup it flakes on
device). All four criteria have passed across runs → the two-slot design is proven on hardware.

--- original mischaracterization kept for the trail ---

## (former) BUG-5 (relay flakiness) — WalletPair join lost when the relay drops an idle pre-pair connection

**Discovered:** 2026-07-06, right after fixing BUG-4, running `e2e/safari/check_concurrent.py`.
With BUG-4 fixed the wallet now joins the relay (app phase `waiting_accept`), but the session
never reaches `connected`: the dApp peer's own protocol disconnect log is
`{"side":"dapp","kind":"transport_close","code":1000,"reason":"closed","phase":"waiting","willReconnect":true}`
— the **CF-Worker relay CLOSES the peer's idle pre-pair WebSocket** (normal 1000), the peer
auto-reconnects, but the wallet's join arrives during that gap and is LOST → the wallet's
`waiting_accept` then times out as `peer_closed`. This is exactly the "the relay may silently
drop the join message (e.g. CF Worker hibernation), leaving both sides stuck in waiting_accept"
case that `src/services/walletpair-transport.ts`'s `confirmFingerprint` already anticipates.

**★ It is APP-SIDE, not the relay (isolated 2026-07-06):** a pure **Node↔Node** pairing over the
same real relay
`wss://relay.walletpair.org/v1` — **pairs in ~2s, fingerprints match, both reach `connected`**.
And a lone peer sits in `waiting` for 38s+ with NO disconnect. So the relay + protocol are fine; the
peer only drops **when the APP's wallet joins**. The one variable is the wallet-side WebSocket:
Node's `ws` vs React-Native's `WebSocket` (Hermes). Something about the app's RN WalletPair
connection makes the relay close the peer as the app joins (candidate causes: RN `WebSocket`
handshake/`?ch=` routing differences; a stale persisted `K_walletpairSession` reconnecting and
colliding; the relay treating the RN connection as a replacement on the channel). WalletPair's
≈0 real-dApp adoption is why this app-side breakage went unnoticed.

**★★ Deep trace via a local WS proxy (`e2e/safari/wp_proxy.mjs`, forwards to the real relay +
logs every frame) — the CHAIN, 2026-07-06:**
1. The app's RN wallet DOES connect (`ws://` passes ATS — the DEV build already talks to
   `http://<lan>` for Metro + testdapp) and sends a proper `{t:"join", body:{sealed_join:…}}`.
2. But the dApp **peer never accepts** it — it stays `phase:waiting`, never emits `walletJoined`
   (in Node↔Node the peer gets it and goes `pending_accept → connected` in ~2s). So the peer
   isn't processing the app's join (candidate: the app's `sealed_join` is malformed vs the Node
   `WalletSession`'s — an RN-side sealing/serialization/crypto difference; the join frame reaches
   the relay fine).
3. The wallet, stuck in `waiting_accept`, **RETRIES the join** (observed 4× in the proxy log,
   first with a real `sealed_join`, then `sealed_join:null`). Each retry is a fresh connection,
   and the relay's per-channel connection handling **closes the peer** (`code 1000 willReconnect`)
   as the extra connections pile on → the peer drops and the join is lost → `peer_closed`.
4. A separate confound found + FIXED along the way: **BUG-6** — the app restored a STALE persisted
   WalletPair session every launch and, on `reconnect()` failing with `channel_not_found`, never
   cleared the snapshot (dapp-connection.tsx:865 restore block) → it restore-looped a dead channel
   and its live reconnect collided with fresh pairings. Fixed: `dropIfDead()` drops the transport +
   `clearWalletPairSession()` when a restored session isn't live shortly after reconnect.

**Next step (crypto/serialization deep-dive, fixable in THIS repo):** hook the peer's raw inbound
frames (or the DAppSession unseal path) to see whether the app's `sealed_join` is REJECTED (unseal
fails → no `walletJoined`) vs never delivered; compare the app's `sealed_join` bytes to the Node
`WalletSession`'s for the same channel. If it's malformed, the bug is in the app's WalletPair
sealing on Hermes (X25519/ChaCha directional keys — the same crypto area as BUG-4). Until then the
device concurrent proof is **2/4** (extension real-signature ✓, no-leak ✓; wp-connected / survived
blocked). Harness ready → 4/4 once the app's wallet holds a session. Keepalive workaround is
impossible (`DAppSession.ping()` no-ops unless `connected`, dapp-session.ts:294; the drop isn't
idle-based — a lone peer waits 38s+ clean).

## ANDROID-4 (fixed 2026-09-12, spec 047) — 探索 did nothing on 通讯录 and 设置

The tab bars of the two routes pushed over the wallet answered only 钱包; 探索 (and the other pushed route) were dead taps — no state change, no frame. The wallet section state was `rememberSaveable` inside the wallet route, unreachable from a pushed route. Fixed by hoisting `section` to the NavHost and giving the pushed routes a real tab handler.

## ANDROID-5 (fixed 2026-09-12, spec 047) — a double 钱包 tap left a blank app under the badge

A bare `popBackStack()` on 钱包 from 通讯录/设置: the fading route still takes taps for ~700 ms, so a second tap popped the wallet itself and emptied the NavHost. With predictive back moving the task to the back and `singleTop`, only a swipe-kill recovered. Fixed with `popBackStack(WALLET, inclusive = false)` and `popUnlessRoot()`.

## ANDROID-6 (fixed 2026-09-12, spec 047) — a `/pay` link was received and then nothing

`LaunchedEffect(payLink)` cleared its own trigger first, which re-keyed the effect to null and cancelled the coroutine waiting for the core's verdict. The link is consumed when the work is done; the verdict read is the one this dispatch produced.

## ANDROID-7 (closed by design 2026-09-12, spec 047) — no offline line from the connectivity callback

The manifest refuses `ACCESS_NETWORK_STATE` (recorded 2026-08-25: a connectivity check lies behind captive portals and on VPNs), so a default-network callback never fires. The offline line comes from `NetHealth`: three consecutive calls that never reached a server.

## WEB-LOGIN-1 (fixed 2026-09-12, spec 048) — sign-in signed and then nothing, until the browser's storage was cleared

Since `wallet.getvela.app` moved to the SvelteKit shell (2026-09-11) the site reads the retired Expo client's `vela.accounts` at the same origin — written in camelCase (`publicKeyHex`, `createdAt`, `keys[].credentialId`). The core's `Account` read snake_case only; the wasm bridge refused the answer; the login and session effect loops had no error handler, so the machine stayed in `LoadingAccounts` with the button busy. Fixed on three layers: the core reads both spellings (hand-written readers; `#[serde(alias)]` trips ts-rs), the web normalises and rewrites the list once and tolerates absent `keys`, and a refused answer is now answered to the machine as its failure (`storage_failed` / `accounts_unavailable`) with a visible prompt: sign in again, or reset this browser's copy.

## DESKTOP-1 (fixed 2026-11-05; opened 2026-09-12, spec 048) — an unreadable account record is skipped silently

The desktop's own store (`wallet.json`) is written only by the current core, so the old spelling never reaches it; but `load_accounts` skipped a record it could not read without saying so — a wallet that "looks signed out". Fixed, mirroring ANDROID-8's hardening: `storage::read_accounts` counts the records it cannot read (a `vela.accounts` value that is not a list counts as one such record, never as an empty list) and logs their positions and sizes — never their contents — as the count changes; every write (`save_account`, `remove_account`) keeps those records where they were, as they were, so a list shortened by what could not be read is never written back, and each write logs its before/after counts as Android's upsert does; the welcome says the stored accounts could not be read (`onboarding.storage.unreadableTitle`/`unreadableBody`, also when the core reports `accounts_unreadable`) instead of looking signed out, and the account switcher says it under the list; the parallel-space seed refuses a store holding an unreadable record. The words are a device-neutral pair since r13 native adoption (`onboarding.storage.deviceUnreadable{Title,Body}`, docs/multisig-machines.md §17): the web's pair said "this browser". A `wallet.json` that is not JSON at all still reads as empty (web parity, `a_corrupt_file_reads_as_empty_rather_than_failing`) and the next write replaces it.

## ANDROID-8 (fixed 2026-11-05, §18 item 2; opened 2026-09-13, spec 048) — the test phone's own account record was dropped from `vela.accounts`

At 22:42:21 UTC on 2026-09-12 the Xiaomi's stored account list was rewritten to the parallel-space fixture alone (its `created_at_iso` is that instant — `DebugParallelSpace.enter()` re-created it), and the founder's account (0x7687…D141) is no longer in the list. The app had just been launched by the scripted pass while the phone was dozing. No logcat from that moment survives. The mechanism that fits: a read of the list that came back empty (an unreadable value maps to an empty array in `AccountStore.readList`, and a thrown read maps to `accounts_unavailable` → `Empty`), followed by the fixture add, whose upsert merged with the same empty read and wrote the list back — so a transient read failure became a permanent loss. Signing in again with the passkey rebuilds the record (any key logs in and restores the account). Hardened in 048: an unreadable stored list is refused, never treated as empty (`AccountStore.readList` throws); every upsert logs its before/after counts (`accounts upsert`); the parallel-space hook refuses to add the fixture — and does not enter — when the stored list cannot be read or the session shows fewer records than the store holds (`parallel enter refused`).

Fixed in the follow-ups after Round 13 (docs/multisig-machines.md §18 item 2), on Android, iOS and the web alike: the account list is read and written through ONE rule in the core (`app::stored_accounts`, UniFFI `stored_accounts_read/upsert/remove`, wasm `storedAccountsRead/Upsert/Remove`). A record this build cannot read — or a stored value that is no list — is counted (positions and sizes logged, never contents) and kept byte for byte on every write; one bad record no longer makes the others unavailable (Android's `loadAccounts` threw, so the session saw nothing and "Sign in again" failed with `storage_failed`; iOS read an unreadable list as empty and the next save wrote `[account]` over it). The welcome says it (`onboarding.storage.deviceUnreadable{Title,Body}`; the web keeps its own browser-worded prompt), the account switcher says it under the list, the parallel-space seed refuses, and signing in with the passkey writes the account back beside what is kept. The lost record of 2026-09-12 itself is not recoverable from the device; the passkey sign-in rebuilt it.

## WEB-CLEAR-LOCALE-1 (fixed 2026-10-03 by spec 097 E, confirmed 2026-11-05 in §18 item 4; opened 2026-09-13, spec 049) — the web's clear-signing sheet formats in a constant locale

`sheet.svelte.ts:98` hands the core `toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' })` with a zero UTC offset, whatever the person chose under 数字格式 / 日期格式 / 时间格式 — so a dApp's amounts and deadlines on the signing sheet ignore the presets every other screen honours, and a deadline is judged against UTC. Android fixed its copy in 049 (`ClearLocale.fromFormats(Formats.current)` from the resolved presets and the phone's offset); the web should call `toClearLocale(resolvedFormatKeys())` at the same point. Recorded for the web owner, not changed here.

Fixed (spec 097 E, `14429c8ac`): `sheet.svelte.ts` now hands the core `toClearLocale(resolvedFormatKeys())` — the person's resolved number / date / time presets — for typed data, a transaction and a batch alike, and `toClearLocale` sets `tz_offset_minutes` to the device's offset (`-new Date().getTimezoneOffset()`, sampled per request; it always did, so the "zero UTC offset" above was never true of that function). The Chrome extension packages the same sheet, so it has the same fix. Confirmed in §18 item 4: iOS (`SigningController.defaultLocale`: `Formats.resolve(Formats.current…)` and `TimeZone.current.secondsFromGMT()`) and the desktop (`signing_host::clear_locale`: `format_prefs::current()` and `local_utc_offset_minutes()`) already used the person's presets and the device's offset. Test: `src/lib/signing/core/sheet-locale.test.ts`.
