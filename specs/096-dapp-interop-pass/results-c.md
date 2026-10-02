# 096 part C — results: fee-coin guard (F2), token-contract recipient (F12), CI flake

Branch `096-fee-coin-guard` from `origin/main` 07a7287c0. Not pushed.

## F2 (S2) — the machine never pays the fee in a coin the operation may spend

Rule, decided once in `rust/crates/vela-core/src/app/fee_policy.rs`:

- **What a call states.** A plain transfer states exactly what it moves: a native `value`, or an ERC-20
  `transfer(to, amount)` (68 bytes). Every other call is *open* (`Outflows::open`). A coin is **spent by
  an amount no call states** (`Outflows::spends_unstated`) when an open call is made **to** its contract
  (`approve`, `permit`, `transferFrom`, anything), or **names** it anywhere in its calldata at a whole byte
  (Permit2 `approve(token, …)`, a router's packed swap path, `supply(asset, …)`). Never the native coin:
  a call can only move it by the `value` it states.
- **Auto pick, no trusted simulation** (`Spend::covers`, `auto_pick`): a coin spent by an unstated amount
  is never a candidate. When the operation holds an open call, the native coin goes first if it covers the
  fee (+ the value sent) — it is the one coin no contract can pull through an earlier allowance (a vault's
  `deposit(amount)` names no token) — then a coin the operation does not touch (larger USD balance). If
  none covers, nothing is picked: the requested coin (native) stands, short, and the sheet says
  "Insufficient BNB for gas fees" with the slide shut. Operations of plain transfers only (every Send)
  keep the old order: a coin not being sent, stablecoin first, larger USD.
- **With a trusted simulation** (`BalanceChangesMeasured`, the desktop's own simulation): a spent coin is
  allowed when what the operation leaves of it covers the fee (inflows at half, spec 083) — unchanged.
- The relay-refusal retry (`next_coin_to_try`) and the landing coin use the same `Spend::usable`, which is
  0 for a coin spent by an unstated amount and not measured — the retry never walks into it either.
- **Manual pick warns, never blocks:** `FeeOptionView.spent_by_operation` (new, `serde(default)`) = the
  operation may spend this coin by an unstated amount and no simulation said how much is left. Shells say
  `componentsUi.gas.feeCoinSpent` under the fee while the selected coin carries it (after the
  insufficient / would-fail sentences, before a network reason). Send never sets it (plain transfers).

Shells (all four have a dApp signing sheet): web `signing/live.ts` (`feeModel` warning), desktop
`signing/live.rs` (`spent_fee_coin_warning`), iOS `SigningLive.feeModel`, Android `SigningLive.feeModel`.
The extension runs the web sheet. Send forms have no such surface (plain transfers only).

## F12 (S3) — Send to a token contract is said before the slide

`SendView.recipient_is_token_contract` (core, `send.rs`): single recipient (one-to-one or sweep, never a
split's rows), on the selected token's network, equals (case aside) the contract of the token being sent
or of any token in the person's list there (the holdings: registry stablecoins / wrapped coin they hold,
tokens they added). Drawn as `send.recipientTokenContract` — first, over a name / "first time" / the
sweep's "same address" — under the recipient field in the warning tone, and as the confirm page's tag.
Warning only; Continue and the slide stay as they were. Shells: web `live-send.ts` + `RecipientField`,
desktop `flows/live.rs` + `panels.rs`, iOS `SendLive` + `FlowBlocks`, Android `SendLive` + `FlowBlocks`.

Not covered (open question): a registry stablecoin the person holds **none** of is not in the holdings
list the Send machine has, so its contract is not recognised; covering it needs the registry's contracts
on `SendChainInfo` from every shell.

## i18n

`componentsUi.gas.feeCoinSpent` ({{sym}}) and `send.recipientTokenContract`, 15 locales (zh-HK in
Cantonese). Paths 1798 → 1800. Runtime JSON ja + en: +511 bytes (ja +289, en +222) → 140,266 against
`SC005_BUDGET` 141,800 (raised from 140,800, owner 2026-10-02); tables ja + en 136,214.

## CI flake — `ChainDeadlineTests.theDeadlineFiresWhileTheMainActorIsHeld`

No wall clock. A `@MainActor` task takes the main actor and spins, never yielding, until the deadline's
answer has arrived (flag under `OSAllocatedUnfairLock`); the read starts only after the hold has begun.
The task returns `true` only if the answer arrived while it held the main actor. A deadline that needed
the main actor could never answer → the hold never ends → the suite's `.timeLimit(.minutes(1))` fails it,
and its cancellation handler releases the hold so nothing else hangs.

## Tests

New tests: core `app_fee_policy.rs` — the real PancakeSwap USDC → BNB batch byte for byte
(`req-pcs-usdc2.json`): BNB pays, never the USDC (the old pick); no BNB → an untouched stablecoin; nothing
else → BNB stands short and a manual USDC pick is flagged; a simulation leaving enough lets USDC pay
unflagged; each of the three legs alone flags USDC; a contract call naming no coin pays native first;
plain sends unchanged (USDC pays its own fee on a part-send, a Max cannot). Eight 083 tests whose premise
was "USDC first for an unmeasured router call" were re-premised (no ETH held, or the roles swapped);
their assertions on the refusal / refresh machinery are unchanged. `app_send.rs` — held token, the
token being sent, case-insensitive, other network, split, wire. Shell tests: web `live.test.ts`,
`live-send.test.ts`; desktop `signing::live` + `flows::live` (and the 083 desktop path test re-premised
to 0 ETH); iOS `SigningLiveTests.aFeeCoinTheTransactionSpendsIsWarned`,
`SendAssetsParityTests.theConfirmSaysTheRecipientIsATokenContract`; Android `SigningLiveTest`,
`SendLiveTest`.

| Suite | Result |
| --- | --- |
| core `cargo test --workspace --features vela-core/i18n-all,vela-core/dev-fixtures` | 2,388 passed, 0 failed |
| core clippy `-D warnings`, `cargo fmt --check`, `build-web --check`, onboarding types `--check` | clean / current |
| web `vitest run` | 2,435 passed, 5 skipped (172 files); `pnpm check` 0 errors |
| desktop `cargo test` | 896 passed, 49 ignored; fmt clean; clippy: no warnings in changed lines |
| Android `testDebugUnitTest` | 955 passed, 0 failed |
| iOS `VelaWalletTests` (cloned iPhone 16 Pro sim) | 1,149 tests in 147 suites passed |
| iOS load recipe (`TEST_RUNNER_VELA_TEST_LOAD_SECONDS=60`, `yes` × 12) | `theDeadlineFiresWhileTheMainActorIsHeld` passed after 6.16 s (the old `< 2.5 s` would have failed) |
| `check-native-reachability`, `check-event-payloads`, `check-dead-controls` | 0 findings |

## Screenshots (scratchpad `shots096c/`, en + zh)

- Web, real wasm core, hermetic stubs (throwaway Playwright run, not committed): `web-fee-auto-*` (the
  PancakeSwap batch on a USDC-holding wallet: the machine pays in ETH), `web-fee-coin-spent-picked-*`
  (USDC chosen by hand: the warning under the fee), `web-send-token-contract-form-*` and `-confirm-*`
  (parallel space, USDC sent to the USDC contract).
- iOS (live builders rendered from a throwaway test): `ios-fee-coin-spent-closed-*`, `-open-*`,
  `ios-send-token-contract-form-*`, `-confirm-*`.
- Desktop and Android: no headless screenshot harness for these surfaces; device/desktop look is the
  lead's.
