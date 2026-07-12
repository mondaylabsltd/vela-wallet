# Vela Clear Signing — the standard, the gap, and the plan to best-in-class

> Lead-engineer briefing for the Vela founder. Every claim below is grounded in the provided standard (ERC-7730 / clearsigning.org / Ledger / Rabby / MetaMask / Safe-Frame-Rainbow), the code audit of `feat/clear-signing-safety-rework` (commit `b82f508`), and the live e2e defect log (D1–D9). File paths are absolute-in-repo (`src/...`); line numbers are from `b82f508` unless noted.
>
> **⚠️ Editor's correction (verified against `git reflog` on 2026-07-11).** The auto-generated draft claimed the calming work was "stranded / does not ship" on `feat/token-autoadd`. That framing was **overstated and the branch name was wrong**. The verified reality:
>
> - The "confident-simulation-calms-the-alarm" rework = commit `b82f508` (+ favicon `d2d2668`), and it lives on branch **`feat/clear-signing-safety-rework`**, which is **2 commits ahead of `main`** (`b82f508`'s parent = `796266c` = current `main` tip). The work is **not merged, and not lost** — it's a normal unmerged feature branch awaiting a PR/merge.
> - The working tree is **currently on `feat/browser-account-switcher`** (HEAD `796266c` = `main`), which does **not** contain `b82f508`. This session's branch moved underneath us — the reflog shows active parallel work: `clear-signing-safety-rework → main → token-autoadd → main → browser-account-switcher`.
> - **Two real consequences:** (1) The audit agents read a **moving target** — some slices saw the branch *with* the fix, some *without* — so **cross-check every cited line number against `feat/clear-signing-safety-rework` before acting.** (2) If you build/run clear-signing on your **current** branch, you'll see the **old, pre-calming** behavior (loud red on every non-descriptor contract) — which may itself be feeding the "感觉不好 / 警告很烦人" impression.
> - **The live e2e defects I saw (D2 English `确认Send` hero, D3 CTA verbs, D4 left-clip) were observed *on* `feat/clear-signing-safety-rework` (with the calming copy rendering).** So those defects exist **even with `b82f508`** — the P0-2/3/4 fixes below belong **on top of that branch**, not on `browser-account-switcher`.
>
> **So "P0-1" is not an emergency reconcile — it's: get `feat/clear-signing-safety-rework` reviewed & merged to `main`, and do all the P0 fixes below on (a branch off) it.** Everything else in this document stands.

---

> Lead-engineer briefing for the Vela founder. Every claim below is grounded in the provided standard (ERC-7730 / clearsigning.org / Ledger / Rabby / MetaMask / Safe-Frame-Rainbow), a code audit spanning `feat/clear-signing-safety-rework` (`b82f508`) and `main` (`796266c`), and the live e2e defect log (D1–D9). File paths are repo-relative (`src/...`); **verify line numbers against `feat/clear-signing-safety-rework`.**

---

## 1. 真正的 Clear Signing 标准是什么 — the bar

A clear-signing screen has one job: let the signer answer **"who is asking me to do what, to whom, on which network, with what effect on my money — and can I trust this screen?"** without reading hex. The standard below is testable. Treat it as an acceptance checklist.

### Baseline — non-negotiable. If any of these fail, it is not clear signing, it is decorated blind signing.

| # | Criterion | Testable bar |
|---|---|---|
| B1 | **WHO** (`who-origin`) | The sheet names the requesting dApp + its verified domain (`app.uniswap.org`), bound from the transport layer, not from anything the page can claim. Change origin → displayed domain changes. |
| B2 | **WHAT** (`what-intent`) | A plain-language action verb (Swap / Approve / Send), derived from a descriptor/ABI, never a bare 4-byte selector. |
| B3 | **HOW MUCH** (`howmuch-token-amount`) | Value scaled by token decimals + ticker: `1,000 USDC`, `0.19866144 ETH` — never `1000000000` or `0x2c1c…`. |
| B4 | **TO WHOM** (`towhom-recipient`) | EIP-55 checksummed recipient + trusted name when available; the full address always inspectable (not middle-elided only). |
| B5 | **WHICH CHAIN** (`which-chain`) | Network shown by name, derived from the tx `chainId`, not the wallet's last-selected chain. |
| B6 | **FROM** (`sender-account`) | Which of the user's accounts/keys is authorizing. |
| B7 | **FEE** (`network-fee`) | Human-readable gas (fiat and/or native), including *which token* pays when gas ≠ native (Tempo stablecoin gas). |
| B8 | **RAW ALWAYS AVAILABLE** (`raw-data-available`) | Untouched calldata / full EIP-712 always expandable; what you inspect equals what you sign. |
| B9 | **REJECT ACTUALLY CANCELS** (`reject-actually-cancels`) | Reject/dismiss never submits; no path blind-submits an unestimated op. |
| B10 | **BLIND-SIGN WARNING** (`blind-sign-warning`) | When nothing decodes, an explicit blind-sign warning; the CTA is de-emphasized / requires extra acknowledgement. Opaque data is never dressed as understood. |

### Good — this is where a *credible* wallet lives.

| # | Criterion | Testable bar |
|---|---|---|
| G1 | **CLEAR-vs-BLIND indicator** (`clear-vs-blind-indicator`) | A positive "decoded/verified" state visually distinct from "could not decode" — not merely presence/absence of fields. |
| G2 | **LAYERED DECODE** (`decode-layering`) | Prefer, in order: (1) ERC-7730 descriptor → (2) ABI/4byte → (3) simulation deltas → (4) blind. The layer used is discoverable and labeled. |
| G3 | **CONTEXT BINDING VERIFIED** (`context-binding-verified`) | Before applying a descriptor, verify contract chainId+address (or EIP-712 domain). A non-matching descriptor is **not** applied — fall back rather than mislabel. |
| G4 | **BALANCE-CHANGE PREVIEW** (`balance-change-preview`) | Simulation-based "what leaves / what arrives" — outcome, not just parameters. Approvals show *no* balance change. |
| G5 | **APPROVAL = SPENDER + TOKEN** (`approval-spender-token`) | Approvals/permits name the spender and token, labeled as granting spending rights, never reading like a send. |
| G6 | **UNLIMITED FLAGGED** (`unlimited-approval-flag`) | `2^256-1` renders as "Unlimited" with caution, never a 78-digit integer. |
| G7 | **EIP-712 DECODED** (`eip712-decoded`) | Typed data decoded field-by-field with formats + domain surfaced, never raw JSON/hex. |
| G8 | **FIELD-FORMAT FIDELITY** (`field-format-fidelity`) | date→calendar, duration→HH:MM:ss, nft→collection+id, enum→named, unit→SI. No bare integers for semantic values. |

### Best-in-class — the top tier Vela is aiming at.

| # | Criterion | Testable bar |
|---|---|---|
| X1 | **NESTED CALLDATA DECODED** (`nested-calldata-decoded`) | 4337 UserOp `execute`, multicall/EIP-5792 batches each get their own intent + delta, joined with "and" — never one blob. |
| X2 | **METADATA TRUST LABELING** (`metadata-trust-labeling`) | Source/trust of metadata surfaced: verified registry vs community vs curated vs unknown; unknown tokens flagged, not silently trusted. |
| X3 | **ASYMMETRIC SIM TRUST** (`asymmetric-sim-trust`) | Debits/approvals gated strictly; **inbound** token names from a sign-time sim are NOT auto-trusted (spoofable) until confirmed by authenticated receipt logs. |
| X4 | **PHISHING / DOMAIN MISMATCH** (`phishing-domain-mismatch`) | Warn on unknown/mismatched/known-scam origin, and when an EIP-712 `verifyingContract`/domain doesn't match the requesting app. |
| X5 | **CRYPTOGRAPHIC FINGERPRINT** (`cryptographic-fingerprint`) | On forced blind-sign, show a stable hash of the exact payload for cross-check, not an unverifiable hex wall. |
| X6 | **INTERPOLATION CONSISTENCY** (`interpolation-consistency`) | Values in an intent sentence format identically to their standalone fields; interpolation failure falls back to static intent + field list, never raw hex. |
| X7 | **LOCALIZED LABELS** (`localized-labels`) | Descriptor strings (spec = English-only) localized into the user's language while amounts/addresses/decoded values stay unchanged. |
| X8 | **SINGLE RENDER PATH** (`single-render-path`) | Prod signing and test/replay render through the *same* component; past signatures re-openable read-only in that same view. |

**Cross-cutting principle from the benchmark (MetaMask/Rainbow/Ledger converge here):** friction is a scarce resource. A clean, benign simulation should make the screen *quiet and fast*; alarms and hold-to-confirm are reserved for genuine uncertainty. A wallet that warns constantly trains banner-blindness and is *less* safe. This principle governs the whole remediation plan.

---

## 2. Vela 现状体检 — honest scorecard

Vela is not a beginner at this. Several primitives are already at or above best-in-class. The problem is **uneven application + a stranded fix + a coverage cliff**, not a missing foundation. (This matches the standing audit conclusion: top-tier primitives BUILT but applied unevenly → fix by consolidation, not rewrite.)

### Genuine strengths — name them, keep them, market them

| Standard | Vela status | Evidence |
|---|---|---|
| **G6 unlimited-flagged + never-unlimited guard** | **Above best-in-class.** Ledger/MetaMask *render* "Unlimited"; Vela *refuses to pass it unedited* and forces a finite cap. Strictly stronger. | `approval-guard.ts`; `EditableApproveCard.tsx`; `requiresHold` grant branches (SigningRequestModal `470-476`) |
| **X3 asymmetric sim trust** | **Best-in-class.** `enrichDeltas` shows SENT always, gates RECEIVED behind `trustedReceiveSet`/known-token; decimals-unverified → direction-only. Spoofed inbound can't manufacture confidence. | `tx-simulation.ts` `enrichDeltas 257-286` |
| **X8 single render path** | **Best-in-class (architecturally).** One `<SigningSheet>` for prod + harness; read-only replay reuses it. | SigningRequestModal render branches; Connection-activity replay |
| **G4 balance-change preview** | **Good, present.** Shared `BalanceChangePreview`, big fiat hero mirroring Send. | `BalanceChangePreview.tsx`; SigningRequestModal `557` |
| **X4 phishing (partial) + eth_sign** | **Good.** SIWE domain-mismatch *is detected* ("很可能是钓鱼攻击 — 请拒绝") and eth_sign is flagged. | MessageSignView `1150-1183` |
| **B8 raw always available** | **Good.** "readable raw data" expander shipped, collapsed by default. | b82f508 commit |
| **Correct safe-default (isolated)** | `BooleanGrantCard` already does the *right* thing: safe Revoke prominent on top, "grant all anyway" muted below. This is the model the footer should copy. | `EditableApproveCard.tsx 180-231` |
| **Governing principle codified** | The "confident simulation calms the alarms" state machine exists in `b82f508` — the correct direction. | `simConfident 368`; `blindButSimulated` |

### Where it falls short — mapped to defects

| Standard | Gap | Severity | Defect |
|---|---|---|---|
| **X7 localized labels** | Descriptor intents + field labels + CTA render **English inside the Chinese UI**. Hero shows `Send`, field label `Amount`, button `确认Send`. Hits the **most-covered path** (any descriptor tx) and all 13 non-English locales. | **P0** | D2 |
| **B2 what-intent / CTA correctness** | `buttonLabel()` catch-all returns `verbApprove='授权'` for blind sends, contract calls, and eth_sign — a wrong verb on a security surface, contradicting the hero verb. 4 inconsistent verbs total. | **P0** | D3 |
| **G1 clear-vs-blind / D7** | The calming fix (`simConfident`) is **not on mainline**; shipping code shows red `blindDecodeWarning` on every non-descriptor contract. | **P0** | D7 / branch drift |
| **B10 / feel** | **Descriptor coverage cliff.** ~10 local descriptors, ~24 known contracts, ~20 known tokens; **no PancakeSwap** (the device-reported case), Curve, Odos, LiFi, 0x Settler, etc. 4byte lookup is **network-only, in-memory cache lost on restart, bare `fetch`** (not proxy/timeout-aware). On mobile socks5, this drops straight to fully-blind red. **This is the root cause of "这个警告很烦人 / 经常能看到".** | **P0** | D7 root cause |
| **B4 / layout** | **Left-clip bug:** the approval-cap `<input>` has no `minWidth:0`, overflows the ScrollView, autoFocus scrolls the sheet left → `被授权方→皮授权方`, `USDC→JSDC`, addresses clipped. Flagship approve screen unreadable on web. | **P0 (one-line fix)** | D4 |
| **X4 / safe-default (D6)** | Detected SIWE phishing has **no hold-gate** and keeps bright `accent` **Sign** as the dominant CTA. `requiresHold` never sees the mismatch (computed inside the leaf `MessageSignView`). Same gap for token-to-own-contract burn and expired-deadline danger banners. | **P1** | D6 |
| **feel / first impression** | Harness first scenario simulates against a **0-balance mock** → big red "预计会失败" on the flagship demo tile. | **P1** | D5 |
| **G7/G5 permit** | `increaseAllowance` mislabeled as a spending **cap** (shows increment as ceiling); resulting-total row absent from batch path. Off-chain Permit can't be capped and shares the dominant-CTA footer. | **P1** | audit |
| **X6 / hero** | `IntentHeader` has no line-clamp/auto-shrink → long best-effort English intents wrap to a 3–4 line headline while the button truncates to `确认`. | **P1** | audit |
| polish | Amber-hero-under-calmed-banner mismatch; dev-English scenario subtitles; PARALLEL badge z-index over title/modal. | **P2** | D8/D9 |

**Net verdict:** Vela already meets or beats best-in-class on the *hard, safety-critical* items (never-unlimited, asymmetric trust, single render path). It is losing on the *cheap, felt* items (localization, one CSS property, a stranded merge, first-impression demo data) and on the *one genuinely hard* item (descriptor coverage). The gap between "feels unfinished" and "feels best-in-class" is unusually small in effort terms.

---

## 3. 为什么"感觉不好" — why the current screens feel unpolished

The screens feel bad for **six specific, diagnosable reasons** — none of them "the design is wrong." They are correctness and consistency defects that read, subconsciously, as *"this was not finished."*

1. **Half-translated on the highest-trust surface (D2).** The single largest element on the sheet — the hero verb — renders `Send` in English, the amount label reads `Amount`, and the commit button says `确认Send`. `intentSend='发送'` already exists in `componentsUi.json:64/65` but is only wired on the *non-descriptor* fallback. So the **better-decoded** the transaction, the **more English** the screen. A user's gut reads this as "a machine translated most of the app but forgot this screen" — exactly the trust-corrosion you cannot afford where a key is about to sign.

2. **The CTA verb lies (D3).** `buttonLabel()`'s catch-all `return verbApprove ('授权')` (line 460) labels a plain ETH send, a "合约交互" call, and an eth_sign raw-hash all as **"Approve."** The button is the last word before signing; a verb that contradicts the hero ("发送"/"盲签" up top, "授权" on the button) makes the screen feel internally inconsistent and untrustworthy, even to a user who can't articulate why.

3. **The flagship screen is visibly broken (D4).** On the unlimited-approve sheet the spender/token rows clip on the **left**: `被授权方→皮授权方`, `代币→弋币`, `USDC→JSDC`, addresses cut — while the right-side copy/link buttons keep their margin. Root cause is concrete and measured: the cap `<input>` (`EditableApproveCard.tsx:277`) is `flex:1` with **no `minWidth:0`**, so it measures 365px inside a 342px viewport; autoFocus scrolls the whole ScrollView `scrollLeft=20`. Live-applying `min-width:0` dropped scrollWidth 385→342 and fixed it. **An address you can't read defeats the entire clear-signing promise** — and a clipped headline is the most viscerally "broken app" signal there is.

4. **The very first demo is red and scary (D5).** The simplest scenario (ERC-20 transfer of 1,000 USDC) runs against a **0-balance parallel-space mock**, so the real mainnet sim reverts and paints a full-width red "预计会失败：transfer amount exceeds balance — 但仍会扣除 gas。" A clear-signing showcase whose **first tile is a failure** tells every reviewer/teammate "this feature is broken," when the box is actually *correct* — the demo actor is just empty.

5. **The safe action doesn't dominate when it should (D6).** On the one screen where Vela itself has concluded "很可能是钓鱼攻击 — 请拒绝," the bright `accent` **Sign** is still the visually dominant primary and there is **no hold-gate** (the SIWE mismatch is computed inside `MessageSignView` and never reaches the parent `requiresHold`). The screen *says* reject but the *layout* invites signing. This registers as the wallet not meaning its own warnings.

6. **The "无法读取该合约的描述" warning is everywhere, and its ROOT CAUSE is descriptor coverage (D7).** The user's literal complaint — *"经常能看到...这个警告很烦人"* — is not a copy problem, it is a **coverage** problem stacked on top of a **stranded fix**:
   - The negative banner fires whenever there is no ERC-7730 descriptor **and** the 4-byte decode misses. After ~10 local descriptors and the ERC-20/721/1155 standard selectors, **everything else** depends on a fragile external 4byte lookup (`selector-registry.ts:34`, bare `fetch`, 6s timeout, **in-memory cache lost on restart**). On mobile socks5/flaky networks those public APIs frequently fail → straight to fully-blind red.
   - Even when they succeed, best-effort is hard-coded `risk:'caution'` (`clear-signing.ts:466`), so a *provably-safe* "Claim rewards" still shows an orange apology.
   - A **second** apology stacks on top: `unverified-decimals` fires because `KNOWN_TOKENS` is ~20 entries and the on-chain `decimals()` fetch caps at 4s → "无法在链上核实代币数量" on any long-tail token, even on a clean Uniswap swap.
   - And the calming rework that was supposed to fix all this (`b82f508`) **isn't merged**, so the user is seeing the *old, unconditional* red path.
   - Where the calming *does* apply, an **async-sim race** makes it flash: `resolving` only awaits `resolveTransaction`, not `simulateAssetChanges`, so the first paint is `sim===null → calm=false → red`, flipping calm 1–3s later. The scary state is the first impression on every blind call.

The through-line: **the screens feel unfinished because the trustworthy signal (the simulation, the finite cap, the asymmetric-trust deltas) is buried *under* apologies, mislabels, English strings, and a layout bug** — the inverse of best-in-class hierarchy, and much of the correct behavior isn't even on the branch that ships.

---

## 4. 达到 best-in-class 的方案 — prioritized remediation

Grouped P0 (correctness/consistency bugs that break the feel — do these to stop looking broken), P1 (safety-UX + coverage that lift to top-tier), P2 (polish). Each item: **change / file / expected effect**, with the benchmark pattern folded in.

### P0 — stop looking broken (mostly small, enormous felt impact)

| ID | Change | File | Expected effect |
|---|---|---|---|
| **P0-1** | **Reconcile the two clear-signing branches.** Cherry-pick/merge `b82f508` (SigningRequestModal + `simConfident`/`blindButSimulated`/`bestEffortSimulated`/`intentContractCall` + the 3 i18n keys) onto the mainline branch before either merges. Add a test asserting `BlindTransactionView` is calm when `sim.ok===true`. | `SigningRequestModal.tsx`, `locales/{zh,en}/componentsUi.json` | The calming fix actually ships. Everything else in this plan builds on it. *(Folds in MetaMask/Rainbow "confident sim calms alarms.")* |
| **P0-2** | **Localization layer.** Add `canonicalIntent(raw)→key` (send/approve/swap/deposit/withdraw/mint/stake/sign…) and `localizeFieldLabel(label)` (amount/to/spender/recipient/deadline/min-received…) in `clear-signing.ts` at resolve time, keyed on the lowercased ERC-7730 value with English `defaultValue` fallback. Route hero `IntentHeader`, `TokenCard`/`GenericFieldRow` labels, batch leg titles, and `buttonLabel`'s `confirmIntentLabel` through it. Respect the ≤3-segment i18n key-depth gotcha (`componentsUi.signingIntents.send`). | `clear-signing.ts:586/495/686`, `SigningRequestModal.tsx:809/1537/446` | Kills `确认Send`, English hero, `Amount` label on the most-covered path for all 14 locales. Unknown descriptors degrade gracefully to English. *(Ledger ERC-7730 localized verb set 兑换/授权/发送.)* |
| **P0-3** | **Fix the CTA verb.** Drive `buttonLabel()` from a single localized action taxonomy derived from the resolved intent. Replace the `verbApprove` catch-all with neutral `确认` (`confirmLabel`); reserve `授权` for the actual `approval?.editable` branch. | `SigningRequestModal.tsx:443-460` | Button verb matches the hero; no "Approve" on a non-approval. Screen reads internally consistent. |
| **P0-4** | **Fix the left-clip.** Add `minWidth: 0` to `EditableApproveCard.amountInput` (keep `flex:1`). Add `minWidth:0` to `contractInfo`/`tokenInfo`/`dappInfo`/`amountValue`. Give the SigningSheet ScrollView `contentContainerStyle={{minWidth:'100%'}}` and never allow horizontal scroll. For `genValue`, drop `textAlign:'right'` overflow in favor of a middle-ellipsis component. | `EditableApproveCard.tsx:277`, `SigningRequestModal.tsx` style rows | Flagship approve sheet renders full, readable addresses/labels. **Verified live** (385→342, scrollLeft 20→0). One property, unbreaks the showcase. |
| **P0-5** | **Descriptor coverage — seed + persist + proxy-aware** (see §5 for the full sub-plan). Minimum for P0: bundle a static top-N selector→signature table queried *before* the network; route lookups through the app's `fetchWithTimeout`/proxy; persist the selector cache to storage; add PancakeSwap V2/V3, 0x Settler, Paraswap, Odos, CoW, LiFi, Across to `LOCAL_DESCRIPTORS`. | `clear-signing.ts`, `local-descriptors.ts`, `selector-registry.ts` | The "无法读取" warning stops firing on the common dApps — directly answers the user's complaint. |

### P1 — lift to top-tier (safety-UX + coverage)

| ID | Change | File | Expected effect / benchmark |
|---|---|---|---|
| **P1-1** | **Single `screenRisk` → footer + hold-gate.** Lift SIWE `binding==='mismatch'`, `sendingToTokenContract`, expired-deadline, and any field-level `warning`/danger up to the sheet. Compute one `dangerDetected` feeding both `requiresHold` **and** footer arrangement. On detected danger, invert the footer: **Reject = full-width prominent primary on top**, dangerous confirm = muted, slide-gated below (mirror `BooleanGrantCard`). | `SigningRequestModal.tsx:470-476, 620-657, 1150-1183` | Fixes D6. The safe action dominates on phishing/eth_sign/burn/expired. *(Rabby graded-friction engine; MetaMask danger-recolored CTA.)* |
| **P1-2** | **Declarative rules module.** Extract scattered warnings (eth_sign, SIWE-mismatch, first-interaction, never-unlimited, unknown-recipient, token-to-own-contract, expired) into ONE module returning `{id, level: info|caution|danger, i18nKey, detail}` on a single severity scale. Feed the SigningSheet. Wire **Contacts as a whitelist**: send to a never-seen address → `danger` "首次向该地址转账" + inline "add to contacts." | new `signing-rules.ts`, `SigningRequestModal.tsx` | Auditable, open rule list = the exact trust signal a wallet with **no third-party audit** should lean on ("here is every check we run"). *(Rabby named-rule engine; Ledger Transaction-Check as a separate layer.)* Each rule owns one shallow i18n key. |
| **P1-3** | **Tri-state the sim framing** (kills the red-then-calm flash). Track `simPending` alongside `sim`. While in-flight: neutral "预估中…", no alarm. Red danger only after the sim **resolves** unknown/reverting. | `SigningRequestModal.tsx:233-259, 368` | The worst moment (first glance) is calm, not red. |
| **P1-4** | **Thread `simConfident` into `risk`, not just banner text.** When sim is confident (`ok`, non-reverting, changes present) and no field carries `warning`, downgrade best-effort `caution→normal` so the **hero color** goes neutral. Same for `unverified-decimals`-only fields: reuse the sim's already-fetched on-chain metadata to mark decimals verified; only set `unverified` when `decimals()` genuinely failed. | `clear-signing.ts:466, 1168-1176, 1257-1258`, `SigningRequestModal.tsx:846` | Stops the double orange-apology stack on provably-safe txs. The loud, trustworthy signal (the sim) leads. |
| **P1-5** | **Reorder the calm layout to lead with what IS known.** Hoist `BalanceChangePreview` **above** `AdvancedPanel` on no-descriptor/blind branches; demote the descriptor-absence note to a small muted caption *under* it; relabel `未验证合约→合约` when `simConfident`. | `SigningRequestModal.tsx:551/557/1358-1374` | Inverts the current apology-first hierarchy to outcome-first. *(MetaMask/Safe: simulation as the hero.)* |
| **P1-6** | **`IntentHeader` clamp + canonical verb.** `numberOfLines={1}` + `adjustsFontSizeToFit` (native) / max-width ellipsis (web); normalize long best-effort intents to the short canonical verb (same map as P0-2); keep the full signature in Advanced. | `SigningRequestModal.tsx:1549-1556, 1942-1949` | Confident single-verb hero instead of a wrapping 3-line English signature. |
| **P1-7** | **`increaseAllowance` = increment, not cap.** Pass approval kind into `EditableApproveCard`; switch header to "increase by" + "resulting total = current + increment"; move `resultingTotal` into the card so it covers the **batch** path too. Run the never-unlimited guard **per-item** inside batches. | `approval-guard.ts:161-167`, `EditableApproveCard.tsx`, `SigningRequestModal.tsx:998-1016` | Removes the understated-allowance trap; batch approvals get the correction. *(Safe itemized multisend + per-item cap.)* |
| **P1-8** | **Slide-hint i18n.** Add `"slideToConfirm":"滑动确认"` to `componentsUi.signing` in all 14 locales; drop dead `holdToConfirm`. | `locales/*/componentsUi.json` | The friction control on the most consequential screens stops speaking English. |
| **P1-9** | **Fix the demo actor (D5).** Give the harness a curated demo `from` that actually holds USDC/ETH (independent of the passkey signer), so scenario #1 simulates green. Do **not** hide real 0-balance failures. | `ClearSigningTestScreen.tsx:142` | First impression of the feature is clear, not red. |

### P2 — polish

| ID | Change | File | Effect |
|---|---|---|---|
| P2-1 | Amber-hero-under-calmed-banner: pass `simConfident` into `intentColor` in ClearSignView so `bestEffort && simConfident` renders neutral. | `SigningRequestModal.tsx:846` | Consistent color signal within one screen. |
| P2-2 | `blindButSimulated` over-claim: require `sim.changes !== null` for "shows exactly what this does"; softer copy when only the revert pre-check passed (`engine:'none'`). | `BalanceChangePreview.tsx:67-73` | Don't promise a preview the sim didn't compute. |
| P2-3 | Localize/neutralize harness English subtitles (D9); move raw signatures into a locale-neutral code chip. | `locales/*/clear-signing.json` | Consistent language in the shipped harness. |
| P2-4 | PARALLEL badge (D8): pin top-right/bottom-center off the title lane; ensure the signing sheet always stacks above the dev badge. | `ParallelSpaceBadge.tsx:57-63` | Badge stops masking the title/modal. |
| P2-5 | Unbounded-approve editor empty state: replace bare `0` with "设置额度" prompt + one/two finite suggestion chips (e.g. the simulated spend amount). | `EditableApproveCard.tsx:62-65` | Safe finite choice is one tap, not free-typing. |
| P2-6 | Add missing harness scenarios (self-transfer, trusted vs untrusted RECEIVE) and reconcile the 22-vs-24 count. | `clear-signing-scenarios.ts` | Exercises `selfTransfer` + asymmetric-gate paths. |
| P2-7 | Cryptographic fingerprint (X5) on genuinely-blind sign: show a stable payload hash for cross-check. Localize date/duration via Vela's explicit format presets (X7/G8). | SigningSheet | Closes the last two best-in-class gaps. |

---

## 5. The one hard problem: descriptor coverage

Everything else on this list is a bounded bug. **Coverage is the only open-ended one**, and it is the direct cause of the user's loudest complaint. The goal is not "decode everything" — it is **minimize the number of times a user sees "无法读取该合约的描述" on a transaction that is actually fine**, by making a *confident simulation* the primary clarity source and demoting the no-descriptor note from an alarm to a footnote.

### The layered decode strategy (per `decode-layering` G2), with Vela's actual weak links

```
(1) ERC-7730 descriptor      ← richest. Coverage: ~10 local + server/interface. WEAK: no PancakeSwap/Curve/Odos/LiFi/0x/Paraswap.
(2) ABI / 4-byte selector    ← WEAKEST LINK: network-only, in-memory cache lost on restart,
                                bare fetch (no proxy/timeout), 6s. Fails constantly on mobile socks5.
(3) Simulation deltas        ← MOST TRUSTWORTHY & spoof-resistant (asymmetric-trust already sound).
                                Should be the PRIMARY signal when 1&2 miss — currently buried below the apology.
(4) Blind + fingerprint      ← honest last resort. Reserve red for THIS only (unknown address, no sim).
```

### The rollout — four moves, sequenced

**Move A — Bundle a static seed table (ships offline, zero round-trip).**
Add a bundled `selector→signature` table of the top few hundred selectors (`swap*`/`exactInput*`/`multicall`/`execute`/`aggregate`/`mint`/`claim`/`stake`/`deposit`/`withdraw`/`bridge`/`wrap`) queried **before** the network in `lookupSelector`. Expand `LOCAL_DESCRIPTORS` with PancakeSwap V2/V3 (the device-reported miss), 0x Settler, Paraswap, Odos, CoW GPv2, LiFi, Across, Stargate, common staking. *Effect:* the top ~90% of real dApp calls decode richly with **no network at all** — the primary killer of the "constant warning."

**Move B — Make the network layer actually work when it's reached.**
Route `selector-registry.ts` lookups through the app's `fetchWithTimeout`/proxy-aware fetch (not bare `fetch`), and **persist the selector cache to storage** so a decode survives app restart. *Effect:* the long-tail that isn't in the seed table stops silently failing on socks5/flaky mobile networks, and a once-seen contract stays decoded.

**Move C — Make confident simulation the primary clarity source, and demote the note.**
This is the felt-quality lever, and it depends on P0-1 (land `b82f508`) + P1-3/4/5:
- Lead the no-descriptor / best-effort branch with `BalanceChangePreview` **above** everything else (P1-5).
- When `simConfident` (sim `ok`, non-reverting, real `changes`), relabel `未验证合约→合约`, downgrade `caution→normal`, and render the no-descriptor line as a **small muted caption**, not a full-width danger `WarningBanner` (P1-4/5).
- **Tier the framing honestly** (per `metadata-trust-labeling` X2 + `asymmetric-sim-trust` X3):
  - *Contract identified (knownContract/4byte) + confident sim* → **calm**, "已通过模拟显示这笔交易的实际影响。"
  - *Contract identified, sim degraded to revert-only (`engine:'none'`, `changes===null`)* → **caution**, softer copy: "未找到合约描述；该交易预计不会失败。" (P2-2 — don't over-claim).
  - *Fully opaque calldata to an unknown address, no confident sim* → **red danger** + cryptographic fingerprint (X5). This is the *only* place red belongs.
- Keep the asymmetric gate: a simulated **RECEIVE** never auto-adds/auto-trusts a token (spoofable) — mark unverified. Only SENT-side deltas + revert-freeness justify calming.

**Move D — Close the sim-availability holes so the calm path is reachable, not theoretical.**
The calm reframe is only as good as the sim's hit rate:
- **Tri-state the race** (P1-3): pending ≠ failed, so the first paint is neutral, not red.
- **Fix the demo actor** (P1-9): the flagship harness scenario must simulate green or the improvement is undemoable.
- On chains where `eth_simulateV1` is unsupported/rate-limited, show a **neutral "无法预估此交易"** empty state (not an error), and still lead with whatever the 4byte/knownContract layer knows — never fall to false red just because the network sim was unavailable.

**Coverage acceptance test (make it measurable):** take the top 30 dApp contracts across Vela's 12 chains (start with the ones in `docs/CONTENT-SOURCE-100-CLUES.md`). Before: count how many show the red/orange "无法读取" banner. After Moves A–D: **target ≤3/30 showing any apology, and 0/30 showing red danger on a benign, simulatable tx.** Drive each state through the `vela.*` fault-injection harness so the calibration is regression-testable.

---

## The 5 things to do first — biggest felt-quality jump per unit effort

Ranked. Do them in this order.

1. **Land `b82f508` on mainline (P0-1).** Non-negotiable and first, because the entire "confident-sim-calms-the-alarm" improvement — the fix for the user's loudest complaint — **is not in shipping code today.** Reconcile the two clear-signing branches into one render path before either merges, add the calm-when-`sim.ok` regression test. *Without this, items 2–5 are invisible to the user.*

2. **Localization layer + CTA verb fix (P0-2 + P0-3).** One canonical intent/label map kills `确认Send`, the English `Send` hero, the `Amount` label, and the wrong `授权` verb in one stroke — on the **most-covered path**, across all 14 locales. This is the highest-visibility defect on the highest-trust surface, and it's a bounded, well-specified change.

3. **The `minWidth:0` left-clip fix (P0-4).** Literally one CSS property (verified live) unbreaks the flagship never-unlimited approve screen so addresses/labels are readable. Nothing signals "unfinished" harder than a clipped headline; nothing is cheaper to fix.

4. **Demo actor with real balance + calm layout reorder (P1-9 + P1-5 + P1-3).** Make the *first thing anyone sees* — scenario #1 and the first paint of any blind call — calm and outcome-first instead of a red failure box and a red-then-calm flash. First impressions are doing outsized damage right now for near-zero code.

5. **Descriptor coverage Moves A+B (P0-5 seed table + persistent, proxy-aware 4byte).** The root cause of "这个警告很烦人." Bundling the top-N selectors + common routers (PancakeSwap first) and making the network layer survive mobile socks5 + app restart is what actually stops the warning from firing on real dApps — the durable fix behind the reframing.

> After these five, the remaining P1 safety-UX items (single `screenRisk` + declarative rules → Reject-dominant danger footer, `increaseAllowance` correction, tri-state trust labeling) take Vela from "feels finished" to "demonstrably best-in-class on safety," leaning on the two things Vela already does better than Ledger/MetaMask — the **never-unlimited guard** and the **asymmetric-trust simulation** — as the trust story for a wallet that has, and honestly discloses, no third-party audit.