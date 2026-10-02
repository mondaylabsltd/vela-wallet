# Implementation Plan: 097 — dApp pass two

**Branch**: `097-signing-readable` (part A) | **Date**: 2026-10-03 | **Spec**: [spec.md](spec.md) · evidence [findings.md](findings.md)

This file holds part A (User Story 1: N1, N2, N3, N6, N8 sheet part). Parts B (`097-dapp-activity-amounts`) and C (`097-refusal-after-submit`) plan their own work on their branches.

## Summary

Every rule lands once in `rust/crates/vela-core/src/app/clear_signing.rs`; the shells draw it. The new wire fact is one optional field, `ClearSignField.bound` (`min` / `max`). No corpus keys are added: every word the sheets need already exists.

## Technical context

- **Core**: Rust (`vela-core`, Crux `ClearSigning` machine). `alloy-primitives` `U256` for the 160-bit test and the `makerTraits` bits.
- **Shells**: web + extension (SvelteKit, wasm), desktop (gpui, links the core), iOS (SwiftUI, JSON wire through UniFFI), Android (Compose, JSON wire through UniFFI).
- **Testing**: `cargo test` (new `tests/app_clear_signing_dapp097.rs` over the pass's real requests in `tests/fixtures/dapp097/`); web vitest with the real wasm core; desktop `signing::live` tests with the real core; iOS `SigningLiveTests` and Android `SigningLiveTest` with the real core over UniFFI.
- **Constraints**: ja+en residency budget 141,800 (no new keys); minimal diffs; parts B/C own `activity_feed.rs`, `dapp_activity.rs`, Activity rows, `approveOptsOf`, the extension request lifecycle and the receipt states.

## Design (part A)

| Finding | Rule (core) | Where |
|---|---|---|
| N1 address | An `addressName` field (or a `tokenPath`) whose value is a `uint256` is the address it holds when it fits in 160 bits; a wider number is shown whole, never cut to an address. | `uint_as_address`, `token_ref`, `format_address`, `format_token_amount`, `collect_token_addrs` |
| N1 amount | An amount with no token the reading can name (not an address, not the native sentinel) is unverified, like one whose token never answered `decimals()` — the em dash, never a figure at a guessed 18. Any unverified amount makes the reading `partial` ("Incomplete"). | `format_token_amount`, `token_decimals`, `incomplete` |
| N1 role | "Beneficiary" is a recipient (drawn as a party: short + full address). | `infer_field_roles` |
| N2 | A borrow (intent or label, not "repay") is money in: `ReceiveAmount`. | `infer_field_roles`, `borrows` |
| N3 | An amount whose label names a minimum or maximum (whole words `min`/`minimum`/`max`/`maximum`) carries `bound`. Web's hero, which drops labels, captions the line with the label; the other shells draw labelled rows already. | `bound_of`, `ClearSignField.bound`, web `amountLine` |
| N6 | A built-in typed reading of 1inch LOP v4 `Order` on the Aggregation Router V6: pay `makingAmount`, receive `takingAmount` ("(min)" when `NO_PARTIAL_FILLS`, else pay is "(max)"), the unwrap flag makes the received coin the chain's own, a zero receiver is the maker, the expiry from `makerTraits` bits 80..120. Layout from 1inch's `MakerTraitsLib.sol` (cited in code). | `read_oneinch_order`, `oneinch_order_descriptor`, `local_typed_descriptor`, `eip712_context` |
| N8 | Curve's router on BNB Chain is a known contract (chain 56 only — the same bytes are `crypto_calc` elsewhere). A token named by the registry (`KNOWN_TOKENS`) stands alone; one named only by its own `symbol()` carries its short address — on Token rows and as a batch call's "Interacting with". | `KNOWN_CONTRACTS`, `token_symbol`, `token_name`, `batch_call_view` |

## Shell work

- **Web**: `amountLine` caption from `bound`; tone caution for an unverified amount; `partial` says `partialWarning` (was the best-effort sentence) and an unverified amount says `unverifiedWarning`, as the other three shells do. New `SigningMessages.warnPartial`.
- **Desktop / iOS / Android**: nothing new to draw — they draw labelled rows (the "(min)" is in the label), the party/row with `address`, `to_name`, `partial` and `unverified` already. Each gets a wiring test that runs the real core on the pass's requests.

## Regeneration

`cargo fmt` → `gen:i18n` → `build-web` + `sync-wasm` → `gen-core-types` (new `ClearAmountBound.ts`, `ClearSignField.bound`) → Swift bindings (no API change) → checks.
