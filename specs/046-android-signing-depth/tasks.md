# Tasks: Android Signing Depth

## Phase 0 — Foundations
- [x] T001 Gradle: `camerax` + `zxing` in `libs.versions.toml` and `app/build.gradle.kts`; manifest `CAMERA` permission
- [x] T002 [P] `feature/scan/Eip681.kt` tokenizer + `Eip681Test` (the web's shapes, the desktop's two refusals, nonsense)
- [x] T003 [P] `feature/signing/core/SimDeltas.kt` (payload builder + `deriveDeltas(logs, user)`) + `SimDeltasTest` (Transfer in/out, sentinel, failed call skipped, zero dropped)

## Phase 1 — US1 Simulation
- [x] T004 `WalletController.judgeSimDeltas(address, chainId, deltas)` (dispatch + wait `sim.ready`)
- [x] T005 `SigningController`: `simulate` port, `sim: StateFlow<SimOutcome?>` set on open for tx methods
- [x] T006 `SigningLive`: Balances block (rows from judgments, note for unverified, no-change, unavailable) + `SigningLiveTest`
- [x] T007 `VelaWalletApplication.openSigning`: the simulate port (pool + derive + judge)
- [x] T008 Device: test dApp transfer → `−0.001 XDAI` block → lands → feed agrees (SC-001)

## Phase 2 — US2 Message signing depth
- [x] T009 `DappRpc`: `eth_sign` → Sign; `DappRpcParityTest` records the deviation
- [x] T010 `SignExecutor`: `eth_sign` hashing/signing; `DappSignMachineTest` eth_sign case
- [x] T011 Test dApp: `siwe`, `siwe-bad`, `ethsign` buttons
- [x] T012 Device: SIWE OK / mismatch / eth_sign danger surface (SC-002)

## Phase 3 — US3 Scanner
- [x] T013 `feature/scan/QrDecoder.kt` (YUV frame + bitmap) + `CameraScanner.kt` (CameraX, torch, first hit) ; `MainActivity.requestCameraPermission`
- [x] T014 `ScanSurface`: preview slot + permission line; `FlowHost` Scan arm live; `SendCallbacks.onScanned/onScanTool`
- [x] T015 `SendController`: `openScanner/closeScanner/scanned(text)`; `SendLive.flowState` S1; home 扫码 → send + scanner
- [x] T016 `SendExecutor` AddNetwork → settings (`Added`/`NotFound`), `SendMachineTest`
- [x] T017 Device: picked QR → locked Send (SC-003); camera opens/closes (SC-004); unknown chain adds (SC-005)

## Phase 4 — Closeout
- [x] T018 Gates (suite, drift, reachability, wasm check, .so size) + results.md + memory
