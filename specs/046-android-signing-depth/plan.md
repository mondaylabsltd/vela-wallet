# Implementation Plan: Android Signing Depth

**Branch**: `046-android-signing-depth` | **Date**: 2026-09-12 | **Spec**: [spec.md](spec.md)

## Summary
Three rungs the phone lacks after 044/045: the simulated balance-change
block on the signing sheet (the shell runs `eth_simulateV1`, derives deltas
as the desktop's `executor/sim.rs`, the wallet's `token_trust` host judges
them, the sheet draws `sim.judgments`); `eth_sign` presented as the danger
surface (the desktop's 035 rung) with SIWE binding already drawn by 044;
and the scanner — CameraX + ZXing (no Google services), a picked photo, a
Kotlin EIP-681 tokenizer pinned to the web's, the core deciding through
`scan_resolved`, and the send executor's `AddNetwork` arm finally asking the
settings machine. Every phase on the Xiaomi.

## Technical Context
**Language**: Kotlin/Compose; no Rust change expected (all machines exist:
`token_trust`, `sign_request`, `clear_signing`, `send`, `network_admin`).
**New dependencies**: `androidx.camera:camera-core/camera2/lifecycle/view`
(1.4.x), `com.google.zxing:core` (3.5.x). Manifest: `CAMERA` permission,
`android.hardware.camera.any` not required.
**Testing**: JUnit (`Eip681Test` parity with the web's cases + the desktop's
refusals, `SimDeltasTest` from log fixtures, `SigningLiveTest` balances,
`DappSignMachineTest` eth_sign, `SendMachineTest` scan), device loop.
**Constraints**: never a write from a simulation; camera frames analysed on
a single background executor; the decoder is the same for camera and photo.

## Constitution Check
Rules in the core (scan decisions, trust judgments, message danger classes);
one implementation per platform port (the EIP-681 tokenizer is the phone's
port of the web's, pinned by a test — the same deviation the desktop
recorded in 036); drift gate untouched (no wire change); device verification.

## Project Structure
```text
app-android/vela-wallet/app/src/main/java/app/getvela/wallet/
├── feature/scan/Eip681.kt                 # tokenizer (web parity, both refusals)
├── feature/scan/QrDecoder.kt              # ZXing over a YUV frame / a bitmap
├── feature/scan/CameraScanner.kt          # CameraX preview + analysis, torch, permission
├── feature/signing/core/SimDeltas.kt      # eth_simulateV1 payload + delta derivation
├── feature/signing/core/SigningController.kt  # + simulate on open, sim state
├── feature/signing/SigningLive.kt         # + Balances block
├── feature/browser/core/DappRpc.kt        # eth_sign → Sign (danger surface)
├── feature/send/core/SendController.kt    # + openScanner/closeScanner/scanned
├── feature/send/core/SendExecutor.kt      # AddNetwork → settings
├── feature/wallet/core/WalletController.kt # + judgeSimDeltas
└── navigation/VelaNavHost.kt              # scanner routing, home 扫码
```

## Complexity Tracking
| Deviation | Why | Alternative rejected |
| --- | --- | --- |
| A Kotlin EIP-681 tokenizer beside the web's TS and the desktop's Rust | The core owns every decision but not the tokenizing (Hermes precedent); a uniffi export would be a fourth implementation to keep | Calling the web's TS is impossible on Android |
| `eth_sign` presented in the in-app browser while the extension refuses it | The desktop's 035 rung; the sheet's danger class exists for it | Silent refusal hides a request from the person who could judge it |
