# Tasks: EIP-712 typed data — what you see is what you sign (085)

**Input**: [spec.md](spec.md), [plan.md](plan.md), [audit.md](audit.md)

## Phase 1 — Audit (read-only, `main` @ `4f2f8b633`)

- [x] T001 Map every typed-data reader per client and method; [audit.md](audit.md) §2.
- [x] T002 [P] Core probe `rust/crates/vela-core/tests/audit_typed_data_selectors.rs` (7 tests, all passing on main) — then inverted into T010.
- [x] T003 [P] Desktop probe (`typed_data_of` + `message_hash`): shape B shows Mail, signs Permit `0x943a0f4c…`.
- [x] T004 [P] iOS probe (`SigningController.typedDataOf` + `SignExecutor.messageHash`, simulator): same.
- [x] T005 [P] Android probe (JVM unit test): same.
- [x] T006 [P] Web probe (vitest, real wasm): `isWellFormedRequest` admits shape A; sheet → Mail, passkey → Permit.
- [x] T007 [P] Signer page probe (node, the page's libraries): shape B preview Mail, digest Permit.

## Phase 2 — Core (US1–US3)

- [x] T008 [US1] `rust/crates/vela-core/src/typed_data_request.rs`: `TypedDataMethod`, `canonical`, `signable`, `document_json_of`; unit tests (4).
- [x] T009 [US1] `app/sign_request.rs` arrival: `-32602` / `4100` before any sheet; canonical params carried on.
- [x] T010 [US1–US3] All core readers on the canonical document (`sign_message`, `approval_guard` detect + rewrite, `self_call_guard` SafeTx anywhere, `extract_request_chain_id`, `dapp_session`, `dapp_rpc`, `method_kind`); `tests/typed_data_wysiwys.rs` (8); older tests moved to real accounts / documents. Core: 1920 passed; clippy clean.
- [x] T011 [US3] `approval_guard`: Permit2 `PermitTransferFrom` / `PermitBatchTransferFrom` detected.
- [x] T012 Exports: UniFFI `typed_data_document`, wasm `typedDataDocument`.

## Phase 3 — Clients (US2)

- [x] T013 [P] [US2] iOS `SigningController.typedDataOf(method:paramsJson:)` → the core; test `theSheetDecodesTheOneDocumentTheCoreSigns`.
- [x] T014 [P] [US2] Android `SigningController.typedDataOf(method, paramsJson)` → the core.
- [x] T015 [P] [US2] Desktop `signing_host::typed_data_of(method, …)` → the core; `_v1`/`_v3` routed; test `the_sheet_decodes_the_one_document_the_core_signs`.
- [x] T016 [P] [US2] Web: `kernels.typedDataDocument`; `sheet.svelte.ts`; `dapp-submit.ts` `pickTypedDataParam` + `handleSignTypedData`; `typed-data-request.test.ts`.
- [x] T017 [P] [US1] Extension `protocol.js isTypedDataParams` at `isWellFormedRequest`; `protocol.test.ts`.
- [x] T018 [US4] Signer page `resolve.typedDocument` for preview + digest, `refuse.typedShape`; `samples/typed-shape-test.mjs`; build `8c002ee4…` first in `BUILD_ALLOWED`.

## Phase 4 — Bindings and gates

- [x] T019 `rust/pkg-web` + wasm asset rebuilt from the final core; `build-web --check`; `sync-wasm`.
- [x] T020 iOS xcframework + `vela_core_uniffi.swift` from this tree; VelaWalletTests on a cloned simulator (969 passed).
- [x] T021 Android bindings + `testDebugUnitTest` (802/803; the 1 failure is pre-existing on `main`, see results).
- [x] T022 Desktop `cargo test` + clippy.
- [x] T023 Web `test:unit`, `check`; extension e2e (isolated).

## Phase 5 — Delivery

- [x] T024 `results.md`; PR to `main` (closes the need for #337).
- [ ] T025 👤 Owner: deploy `app-web/trusted-signer/dist/`, confirm `/b/0ba8ee8c…/sign` is 200 + immutable, then move `LAUNCH` to it. This is the build from merging `main` after #338: 085 and 082 in one page.
