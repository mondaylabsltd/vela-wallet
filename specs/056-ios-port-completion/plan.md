# Plan — 056

## Constitution check

Unchanged from 050–055: zero lines under `rust/crates/vela-core/src/app/`, zero
corpus delta, zero lines in the other four clients, `vela_core_uniffi.swift`
untouched. Executor contract per `specs/024-web-live-shell/contracts/`.

**One rule specific to this cut:** the preferences have no machine, and this cut
does not invent one. They are storage keys with a reader — the same keys, byte
for byte, that web and Android write, so a person's choice survives moving
between their own devices through a backup.

## Phases

| | what | done when |
|---|---|---|
| 0 | `Preferences`, `Formats` | the keys match web's byte for byte, and both have tests |
| 1 | **US1** applied where things SHOW — hero, rows, feed, send, confirm, receipt, detail, signing | a format change moves every figure |
| 2 | **US2** the settings page alive | every row acts |
| 3 | **US3** deep links and the pay-link | a link prefills a send |
| 4 | **US4** the unreachable list | each screen reachable from where a person would look |
| 5 | **US5** two rulers, docs, closeout | zero strong differences |

## Gates

As 055, plus `node app-ios/scripts/check-ios-event-parity.mjs` and
`check-ios-dropped-judgement.mjs` from phase 5.

## Baselines at the branch point

| | at 056's start |
|---|---|
| hermetic test run | 523 in 66 suites |
| XCUITest methods | 35 |
| literal-audit violations | 35 |
| `vela_core_uniffi.swift` bytes | 353,772 |

## Traps carried in

The six in `reference_ios_shell_traps` — `GestureMask.none`, an unbooted
`CoreStore`, two `.sheet`s, `FlowHost`'s type-checker timeout, Vision in the
simulator, and the phone's intermittent refusal of automation mode.
