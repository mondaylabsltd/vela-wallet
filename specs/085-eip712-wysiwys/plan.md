# Implementation Plan: EIP-712 typed data — what you see is what you sign

**Branch**: `085-eip712-wysiwys` | **Date**: 2026-10-01 | **Spec**: [spec.md](spec.md) | **Audit**: [audit.md](audit.md)

## Summary

The audit found seven independent readers of one request. The fix makes the core read a typed-data
request once (`vela_core::typed_data_request`), refuse every other shape at arrival, and carry on
only the two canonical params; every reader — digest, guards, chain pick, account check, each
client's sheet, the web submit path, the extension boundary, the signer page — then takes that one
document. Base: `main` @ `4f2f8b633` (PR #337 is not merged; it is redone here for every surface).

## Technical Context

Rust core (`vela-core`, Crux machines), UniFFI → Swift / Kotlin, wasm-bindgen → web and the
extension's pages, the desktop (gpui) linking the core directly, the Trusted Signer static page
(content-addressed build, `BUILD_ALLOWED` / `LAUNCH` in `trusted_signer/integrity.rs`). House rule:
a rule is decided once in the core, clients only draw (FR-020 of earlier specs).

## Design

| Layer | Change |
|---|---|
| core `typed_data_request.rs` (new) | `TypedDataMethod::of` (exact names); `canonical` (shape), `signable` (shape + hashes), `document_json_of`; `CanonicalTypedData { method, account, document, document_json, digest() }`, `params()` rebuilt in the method's order |
| core `sign_request` arrival | `signable_json` → `-32602` with the reason; account ≠ granted → `4100`; else `params_json` replaced by the canonical two |
| core readers | `sign_message::original_hash`, `approval_guard::detect_approval` + rewrite slot, `self_call_guard` (SafeTx anywhere), `extract_request_chain_id`, `dapp_session` chain context, `dapp_rpc::requested_address`, `method_kind` |
| core `approval_guard` | Permit2 `PermitTransferFrom` → `Permit2Single`, `PermitBatchTransferFrom` → `Permit2Batch`, uint256, `OffChainPermit`, not editable |
| exports | UniFFI `typed_data_document`, wasm `typedDataDocument` (siblings of `sign_message_hash`) |
| iOS / Android / desktop | the sheet's `typedDataOf` / `typed_data_of` → the core's document; desktop routes `_v1`/`_v3` to clear signing too |
| web | `kernels.typedDataDocument`; the sheet uses it; `pickTypedDataParam` and `handleSignTypedData` use it (no fallback) |
| extension | `isTypedDataParams` at `isWellFormedRequest` (the worker cannot run the core) — `-32602 Malformed request` |
| signer page | `resolve.typedDocument` used by the preview and `digest.js`; `refuse.typedShape` (en, zh); new build first in `BUILD_ALLOWED`, not `LAUNCH` |

## Gates

core `cargo test -p vela-core --features i18n-all,crux`, clippy `-D warnings`, fmt; uniffi + wasm
crate tests; `build-web` + `--check`; iOS VelaWalletTests on a cloned simulator (xcframework rebuilt
from this tree); Android `testDebugUnitTest`; desktop `cargo test` + clippy; web `test:unit`,
`check`; signer page `samples/*-test.mjs` (single-file with Chrome for Testing).

## Owner steps (not in this branch)

Deploy `app-web/trusted-signer/dist/` → confirm `/b/8c002ee4…/sign` serves immutable → move `LAUNCH`.
