# Implementation Plan: Android Port Completion and First-Run Parity

**Branch**: `047-android-port-completion` | **Date**: 2026-09-12 | **Spec**: [spec.md](spec.md)

## Summary
The residue after 040–046, then the rulers. A shell-side `Preferences`
store (the web's keys) feeds the language runtime, the formatters, the text
scale and the avatar style; the settings sheets stop being inert; storage,
erase, relayer, feedback, about, the RPC banner and the pills read what the
device actually holds; the receive share card becomes a PNG through the
documents port; `velawallet://` and `/pay` reach the core's `payment_request`
grammar; offline, version and panic surfaces mirror 038; the two rulers run
as scripts whose tables land in results.md; a scripted device pass and the
papers close the program.

## Technical Context
Kotlin/Compose; no Rust change expected. New: `core/data/Preferences.kt`,
`core/format/Formats.kt` (the web's `locale-format` presets), a
`CrashReport` handler, `ConnectivityWatch`, `DeviceStorage` (key → item
mapping + sizes), `ShareCardRenderer` (GraphicsLayer → PNG). Tests: presets
(parity with the web's separators/date shapes), storage mapping, erase
sweep (FakeStore), pay-link routing, preferences round trip.

## Constitution Check
Rules in the core (the `/pay` grammar, currency, networks); preferences
have no machine (028's ruling, kept); device verification; corpus words only.

## Project Structure
```text
core/data/Preferences.kt · core/format/Formats.kt · core/diagnostics/CrashReport.kt
core/net/ConnectivityWatch.kt · feature/settings/core/DeviceStorage.kt
feature/settings/SettingsLive.kt (+ prefs, storage, about, feedback, relayer, banner, accounts)
feature/flows/ShareCardRenderer.kt · MainActivity (intents, launchMode)
navigation/VelaNavHost.kt (settings wiring, deep-link routing, offline line, panic sheet)
scripts/check-android-event-parity.mjs · scripts/check-android-dropped-judgement.mjs
docs/android/install-verify-loop.md · docs/KNOWN-BUGS.md · docs/project-takeover/02
```
