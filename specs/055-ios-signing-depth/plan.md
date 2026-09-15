# Plan — 055

## Constitution check

Same three invariants as 050–054, and they are the gate on every commit:

- **zero lines** under `rust/crates/vela-core/src/app/`;
- **zero corpus delta** — every sentence this cut shows already exists in
  fourteen languages;
- **zero lines** in `app-web/`, `app-desktop/`, `app-android/`,
  `app-browser-extension/`; `vela_core_uniffi.swift` untouched.

Executor contract (`specs/024-web-live-shell/contracts/shell-operations.md`):
every operation answered exactly once; expected failures use the machine's own
failure variant; no business `if` in an executor; exhaustive switch with a loud
`default` and a `neutralAnswer` twin.

## Phases

| | what | done when |
|---|---|---|
| 0 | `SimDeltas`, `SignedDigits`, `Eip681`, `QrDecoder`; `NSCameraUsageDescription` | each has its own tests and the payload matches the desktop's byte for byte |
| 1 | **US1** the balance block: `eth_simulateV1` through `RpcPool` → `TokenTrustStore.simDeltasComputed` → the sheet's `balances` | a contract call shows its moves; an unsimulatable one says so |
| 2 | **US2** message depth: SIWE binding, the phishing warning, `eth_sign`'s danger face | a mismatched domain warns in the core's words |
| 3 | **US3** the camera: preview, torch, flip, library, three refusals | a receive code read on the phone locks the send |
| 4 | closeout: device runs, results, the two owed-to-a-finger items | results.md carries every SC with its verdict |

## Gates, per commit

```bash
cd app-ios/VelaWallet
xcodebuild test -project VelaWallet.xcodeproj -scheme VelaWallet \
  -destination 'id=84146B7B-C679-46AC-8426-41AA42E6F403' -only-testing:VelaWalletTests
xcodebuild test ... -destination 'platform=iOS,id=00008030-001A75961445802E' \
  -only-testing:VelaWalletUITests/SigningDepthAcceptanceTests
node app-ios/scripts/audit-literals.mjs          # baseline 35, the gate is "no new ones"
git diff --stat $(git merge-base origin/main HEAD) -- rust/crates/vela-core/src/app/   # empty
```

## Baselines at the branch point

| | at 055's start |
|---|---|
| hermetic test run | 473 in 58 suites |
| XCUITest methods | 33 |
| literal-audit violations | 35 |
| `vela_core_uniffi.swift` bytes | 353,772 |

## Traps carried in

1. **A `TextEditor`'s newline is a harness fact** — a real keyboard gives one,
   the simulator does not (054 phase 7).
2. **`GestureMask.none` disables every gesture in the subtree**, not just the
   one being attached (054 phase 7).
3. **A `CoreStore` drops events sent before `boot`** — a store whose machine has
   no boot event of its own must boot on its first (054 phase 7).
4. **One `.sheet` per view**, whose content changes.
5. `FlowHost` and `FlowSheetHost` are two structs: a parameter added to one and
   not forwarded from the other renders the DRAWN fallback, which looks exactly
   like the drawing.
