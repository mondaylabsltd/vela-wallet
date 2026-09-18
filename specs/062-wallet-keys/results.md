# 062 — Results

What was ruled (spec §0): no key replacement, no keys screen. The feature is **back the
founding record up to Ethereum, and be able to sign in from it.** Both halves are built on
all four shells.

## What exists

| | core | web | desktop | Android | iOS |
|---|---|---|---|---|---|
| Check + the one `register` call (`registry_backup`) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Settings row (checking / backed up / not backed up / could not check; dark when there is no registry or no registration) | — | ✅ | ✅ | ✅ | ✅ |
| The wallet's own request through the ordinary signing sheet, drawn as "Back up wallet keys" | ✅ | ✅ | ✅ | ✅ | ✅ |
| Inner-call gas floor (an undeployed Safe's first operation) | ✅ | ✅ | ✅ | ✅ | ✅ |
| Sign-in reads fall back to the contract — Gnosis, then Ethereum (`registry_chain`) | ✅ | ✅ | ✅ | ✅ | ✅ |
| A wallet's name by address survives the index (`registry_lookup` sources) | ✅ | ✅ | ✅ | ✅ | ✅ |

The registry is deployed at `0x94fD1A891EB6c5F340622Baf2F3A0cb70A941EA9` on Gnosis, Ethereum
and Base (operator, 2026-09-18). `TARGET_CHAINS = [1, 8453]`; the Settings row offers
Ethereum only.

## How each claim was checked

**Real money, web.** The golden multi-key Safe `0x88cC…6894` backed itself up from the
wallet's own signing sheet: Base `0x69c54f91…5fc73d` (13 s), Ethereum `0x86795d08…84dc5c`
(block 26,002,374, 87 s). Both registries hold its unit; every shell's check now reads
"backed up" for it.

**Device, Android** (Galaxy S22, a real single-key account): the row read "Not backed up
yet"; tapping it opened the sheet on Ethereum — "Back up wallet keys · Wallet jdjd ·
Address 0x94BF3E…EE60e3 · Keys 1 · Vela passkey registry", with a fee. **Not signed** — that
is the account owner's money and passkey.

**Live, no UI:** desktop (`the_golden_safe_reads_backed_up_on_ethereum_and_base`,
`a_dead_index_still_signs_the_golden_wallet_in`, both `#[ignore]`) and iOS (a throwaway
probe through Swift + uniffi, deleted after) read the real chains.

**Recorded bytes, real core:** the sign-in fallback on web, desktop, Android and iOS is
tested against one fixture — `registry-chain.json`, verbatim `eth_call` results from both
chains — so the four shells are pinned to the same contract answers.

Suites at the end: core green; desktop 400; Android 538; iOS 598; web 1045 with the same 3
failures `main` has (explore fixtures, `FLOW_KEYS`, token px audit).

## Not verified — say so rather than imply it

- **iOS has not been seen running.** It compiles, its tests pass, the live probe works; the
  simulator's dev seed is key-less by design (spec 051 FR-010), so the row is dark there.
  Needs a device with a real account.
- **Sign-in fallback on a device, any shell.** It needs a passkey sign-in with the index
  blocked. Covered by recorded-bytes tests and one live desktop test only.
- **The gas floor on Android/iOS/desktop with a real send.** Proven by the web sends; the
  native shells share the core rule and have unit tests.
- **The 15 translations** of the four new strings are machine-written.

## Found on the way, fixed here

1. dApp-path signing resolved the wallet by credential id; one passkey founds many wallets
   → `AA14`. Now by address (`storedWalletFor`).
2. The bundler's `callGasLimit` for an undeployed Safe is 21k + calldata, so a first
   operation that does real work is accepted and never lands. The wallet now measures its
   inner calls itself (`inner_calls_gas_floor`). **The bundler should still be fixed**
   (`vela-bundler shared/simulation/index.ts`); this is the wallet not depending on it.
3. The signing sheet printed `[object Object]` for a tuple and `{{fn}}` in a warning.

## Found on the way, NOT fixed here

- Android's signing/Send fee shows full precision (`~0.00100259302524924 ETH`) and wraps —
  `SendLive.feeParts`, shared, pre-existing.
- The first-party request's origin draws as "getvela.app" with a letter avatar on native; web
  shows the app's name. Cosmetic.
- Only deployed wallets can be named by address: hop 1 reads the Safe's own storage.

## Deferred, with the founder's ruling

Keys screen (Part A) and key replacement (Part B): spec §0.
