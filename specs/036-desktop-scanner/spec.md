# Feature Specification: Desktop Scanner — a QR the app can actually read

**Feature Branch**: `036-desktop-scanner` (stacked on `035-desktop-message-signing`)

**Created**: 2026-09-09

**Status**: Draft

**Input**: Founder: "按推荐来" — take the recommendation. The comparison table
is empty of wiring; the scanner was the largest of the three remaining real
features.

## Why

DS1 has been a picture since spec 021: a viewfinder rectangle, a hint, and two
tool buttons — `componentsUi.scanner.fromGallery` and `flipCamera` — with
nothing behind any of them. `send::OpenScanner` / `CloseScanner` /
`ScanResolved` are three of the last five "strong" differences the web
comparison found, and the only ones left that are not a platform decision.

## Two halves, and this cut takes the one that works everywhere

- **From a file** — a screenshot, a saved share card, a photo somebody sent.
  Needs no camera, works identically on macOS, Windows and Linux, and is
  testable without hardware. **This cut.**
- **From a live camera** — AVFoundation / Media Foundation / v4l2, a preview in
  the viewfinder, a decode per frame. A platform pipeline per OS. **Next.**

## Scope

1. A QR decoder over image bytes (`rqrr`, pure Rust — no zbar, no C toolchain,
   so it cannot be the reason a platform stops building).
2. An `ethereum:` tokenizer, ported from the web's `eip681.ts` **with both of
   its refusals** (a `/transfer`'s `value` is not the token amount; any other
   function is not a payment).
3. The routing the web already documents: inside a live send the core rules
   (`ScanResolved`); from the home the code OPENS a send, prefilled — and
   locked exactly when the request names a chain.

## Out of Scope

- The camera pipeline (next cut).
- Any change to `send.rs`'s rules about locking.
- New corpus keys — the scanner's words have shipped since spec 021.

## Success Criteria

- **SC-361**: a QR this app wrote decodes back to the same payload.
- **SC-362**: the four canonical EIP-681 forms parse; `approve` and a
  `/transfer?value=` are refused; a negative amount is no amount.
- **SC-363**: picking a picture of a payment request opens Send locked to its
  chain with its amount and recipient filled in — verified on the running app.
- **SC-364**: `cargo test` counts strictly increase; the four gates stay green.
