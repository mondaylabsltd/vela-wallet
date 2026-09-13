# Implementation Plan: Web Port Completion

**Branch**: `028-web-port-completion` | **Date**: 2026-09-04 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `/specs/028-web-port-completion/spec.md`

## Summary

Fill every surface 024–027 drew and left on fixture data, and port the services
those surfaces need. Nothing here is designed: the screens exist, the machines
are aboard, and the hard technical questions — how to decode a QR from a real
camera, what "erase" must actually delete — were already answered by the Expo
work and are carried rather than re-derived.

One item is a defect rather than a gap and leads the feature: **the receive code
does not encode the address.**

## Technical Context

**Language/Version**: TypeScript strict, SvelteKit 2 / Svelte 5 runes; wasm
(`rust/pkg-web`) unchanged.
**Primary Dependencies**: existing, plus two for decoding only —
`@undecaf/zbar-wasm` (239 KB wasm) and `jsqr` (257 KB), both lazy and both
loaded from our own origin. Encoding needs nothing: Expo's `qrcode.ts` is 554
dependency-free lines.
**Storage**: existing (IndexedDB KV + localStorage). New: preference keys, and
an export/import file that leaves the browser.
**Testing**: vitest (encoder vectors, format presets, import collisions) +
Playwright ×3 engines (persistence, decode round trip) + the parallel space.
**Target Platform**: web (Cloudflare Worker + static prerender ×15), and the
027 extension build, which packages the same pages.
**Constraints**: budgets unchanged (zero-wasm Welcome, wasm-free deploy bundle,
one CORE artifact, artifact bytes); the decoder's wasm is lazy and must never
reach a startup chunk; galleries pixel-unchanged; corpus via the 5-step process;
no business rule added where a core owns one.
**Scale/Scope**: ~2,500 lines of ported services, ~700 lines of live builders
and wiring, two machines gaining their first screen (`manage_tokens`, `send`'s
sweep half), six preference rows, ~7 new e2e suites.

## Constitution Check

| Rule | Status |
| --- | --- |
| One implementation / tokens only / i18n via corpus / generated regenerated / fixtures canon / components pure / core decides | ✅ every surface is already drawn; this feature builds live siblings and injects callbacks |
| One PR one problem (§2) | ⚠️ **six stories in one feature.** They are one problem — "the port is not finished" — and were audited as one list; splitting them into six PRs would multiply the gate runs without changing the work. Seven phases, one commit + gate each, so a rollback is per-story |
| High-risk (§3) | ⚠️ **Medium-high**: the receive code and sweep both touch money. Mitigations: the QR encoder ports with a vector suite and is asserted by DECODING what it renders; sweep is the core's existing `multi_select` fields with 91 send tests behind them; erase is destructive and gets a confirmation plus an explicit exception list |

## Project Structure

### Documentation (this feature)

```text
specs/028-web-port-completion/
├── spec.md · plan.md · research.md (D44–D52) · data-model.md · quickstart.md
├── contracts/erase-scope.md · checklists/requirements.md
├── tasks.md · results.md (ledger, written per phase)
```

### Source Code (app-web/vela-wallet)

```text
src/lib/services/qrcode.ts                    # D44 — encoder, ported verbatim
src/lib/services/qr-decode.ts                 # D45 — zbar + jsQR, lazy
src/lib/services/share-card.ts                # D46
src/lib/services/locale-format.ts             # D47 — explicit presets, no Intl
src/lib/services/avatar-style.ts · preferences.ts   # D48 — shell state
src/lib/services/erase-device.ts              # D49 — namespace sweep + exceptions
src/lib/services/contact-io.ts                # D50 — existing entry wins
src/lib/wallet/qr.ts                          # the live code (fixtures keep the pattern)
src/lib/flows/live-send.ts (+sweep) · live.ts (+scan, +add-token)
src/lib/wallet/core/manage-tokens.svelte.ts   # the session's first screen
src/routes/[locale]/settings/+page.svelte     # preferences + erase
e2e/{receive-code,scan,preferences,sweep,add-token,contacts-io,desktop-send}.e2e.ts
```

**Structure Decision**: unchanged from 024–027 — ports under `services/`, live
builders as siblings of `fixtures.ts`, machine loops under each domain's
`core/`.

## Phases

| # | Phase | Gate |
| --- | --- | --- |
| 1 | Setup — baselines, port-provenance list @ 28d25ae9, the two decoder deps declared, audit list | results.md |
| 2 | **The code is data** (US1a) — port the encoder with vectors; the receive card, the payment-request code and the share card render REAL codes; the decorative pattern stays only where a placeholder is meant | gates + a decode round trip in e2e |
| 3 | **Reading a code** (US1b) — the decode engine (zbar primary, jsQR fallback, the preprocessing ladder `docs/qr-scanner-web.md` proved), the camera surface, the image picker, and every refusal said out loud | gates + decode-from-image e2e |
| 4 | **Preferences and erase** (US2) — theme, language, number/date/time presets, avatar style; erase by namespace sweep with a named exception list | gates + ×3-engine persistence e2e |
| 5 | **Sweep and custom tokens** (US3, US4) — `send`'s multi-select half; `manage_tokens` gets its screen | gates + sweep and add-token e2e |
| 6 | **The book travels, and the desktop sends** (US5, US6) — export/import with existing-wins; desktop send actions | gates + import-collision and desktop-send e2e |
| 7 | Budgets + closeout — decoder wasm proven lazy, artifact bytes, SC verdicts, 029 handoff | full suite + ledger |

## Complexity Tracking

| Violation | Why | Simpler alternative rejected |
| --- | --- | --- |
| Two new dependencies for DECODING | `docs/qr-scanner-web.md` measured it: iOS Safari has no `BarcodeDetector`, and jsQR alone cannot read a real photo. zbar WASM is the only thing that works, with jsQR as the clean-screenshot fallback | `BarcodeDetector` alone (fails on the platform most likely to be scanning); jsQR alone (measured to fail on camera input) |
| A second wasm in the app | the decoder is 239 KB and only exists while a scanner is open | shipping a decoder that does not work |
| Six user stories in one feature | they are one audit and one problem — "the port is not finished" | six PRs, six gate runs, one changed line of substance each |
| Preferences with no core | theme, language, formats and avatar style are shell state; inventing a machine for them would be a machine with no rule in it | adding a Rust machine to hold four enums |
