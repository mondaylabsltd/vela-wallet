# 085 results — EIP-712 typed data: what you see is what you sign

Branch `085-eip712-wysiwys`, from `main` @ `4f2f8b633`. Audit and executed proofs: [audit.md](audit.md).
Redoes PR #337 (web/extension `eth_signTypedData_v4` only; the owner closes it) for every surface.

## Verdicts (audit, before the fix)

| Surface | Verdict |
|---|---|
| Shared Rust core | **Confirmed** — no shape check; SafeTx refusal bypassed by v4 `[benign, SafeTx]`; legacy Permits and Permit2 SignatureTransfer not flagged |
| Chrome extension / web | **Confirmed** — v4 `[benign, malicious]`: the sheet shows benign, the passkey signs malicious |
| iOS | **Confirmed** — legacy `[malicious, benign]`: shows benign, signs malicious (`0x943a0f4c…`) |
| Android | **Confirmed** — same |
| Desktop | **Confirmed** — same; `_v1`/`_v3` had no clear-signing preview |
| Trusted Signer page | **Confirmed** — legacy: preview `params[1]`, digest `params[0]` |

## After the fix

| Check | Result |
|---|---|
| Both malicious shapes, one param, wrong order, `_v2`: refused before any sheet | core machine test `the_malicious_shapes_are_refused_before_the_sheet` (`-32602`, sheet hidden); extension boundary `protocol.test.ts`; signer page `typed-shape-test.mjs` (preview AND digest refuse) |
| Another account than the granted one | `4100` (`a_document_for_another_account_is_refused_4100`) |
| Every method: the sheet's document hashes to the signed digest | desktop `the_sheet_decodes_the_one_document_the_core_signs`; iOS `theSheetDecodesTheOneDocumentTheCoreSigns`; web `typed-data-request.test.ts`; signer page `typed-shape-test.mjs` |
| The sheet holds only the canonical params | `the_sheet_holds_only_the_canonical_params` |
| SafeTx anywhere refused; legacy Permit flagged; Permit2 SignatureTransfer flagged | `a_safe_tx_anywhere_is_refused`, `a_well_formed_request_signs_its_one_document_in_every_method`, `permit2_signature_transfer_is_flagged` |

## Gates

| Suite | Result |
|---|---|
| vela-core (`i18n-all,crux`) | 1920 passed, 0 failed; clippy `-D warnings` clean; fmt clean |
| uniffi + wasm crates | pass; `build-web --check` current (wasm `f1a6fb72bd4a`) |
| desktop | 783 passed, 0 failed |
| signer page | every `samples/*-test.mjs` passes; `single-file-test` 12/12 with Chrome for Testing |
| web | 1920 unit tests pass; the 3 failures are `extension/package.test.ts`, which checks a built `extension/dist` absent from a fresh worktree |
| iOS | VelaWalletTests on a cloned iPhone simulator, xcframework built from this tree: 969 tests in 127 suites passed |
| Android | `testDebugUnitTest`: 803 tests, 802 passed; the 1 failure was pre-existing on `main` (below) and is fixed here too |

### After merging `main` with PR #338 (082), 2026-10-01

The only conflicts were the generated stamps (the wasm, `pkg-web`, the signer page's `dist/`, `BUILD_ALLOWED`) and `.specify/feature.json`. The wasm, Swift bindings, TS mirrors and signer page were rebuilt from the merged tree. The merged page is build `0ba8ee8c…` (see Owner steps).

| Suite | Result |
|---|---|
| vela-core + workspace (`i18n-all,dev-fixtures`) | 2240 passed, 0 failed; clippy `-D warnings` clean |
| desktop | 868 passed, 0 failed; fmt clean |
| signer page | safeop, identicon, typed-shape, takeover, unlimited-line, origin-line, plain-send 153/153, fee-leg, hostile 32/32, channels 19/19, ceremony 47/47, slider 19/19, single-file 12/12 |
| web | 2235 unit tests pass, plus `extension/package.test.ts` 10/10 after `pnpm build:extension`; `svelte-check` 0 errors |
| iOS | 1072 tests in 138 suites passed (cloned simulator, xcframework built from this tree) |
| Android | `testDebugUnitTest`: 863 tests, 0 failures |

Pre-existing on `main`, not 085 (085 changes neither file nor the generated mirror):

- `pnpm check` reported one error in `src/routes/[locale]/wallet/+page.svelte:654` (083's
  `FeeFailure::would_fail` is not a `SendEstimateFailure`). The 082 merge (PR #338) fixed it,
  and after merging `main`, `svelte-check` reports 0 errors.
- Android `CoreWireDriftTest.signRequestWiresMatchTheMirrors`: 083 added `SignSubmitOutcome`
  `not_confirmed` and `reverted` to the core; Android's Kotlin mirror lacked them. The two
  variants are added here (the same block as the 082 merge, PR #338, so the two merge cleanly).

## Owner steps

Done 2026-10-01. The owner deployed `app-web/trusted-signer/dist/` and merged #339, and #337 is closed without merging. Checked from this machine with cache-busting queries:

- `https://sign.getvela.app/b/0ba8ee8c…/sign` returns 200 with `cache-control: public, max-age=31536000, immutable`, and the bytes hash to `0ba8ee8c…`.
- All 12 builds in `dist/index.json` still serve 200, each hashing to its name. The root `/sign` serves `0ba8ee8c…` too.
- `LAUNCH` moves to `0ba8ee8c…` in the follow-up PR (branch `signer-launch-0ba8ee8c`).

## Follow-ups

- Permit2 SignatureTransfer is flagged under the existing Permit2 kinds (no new wire value); a
  dedicated kind and wording ("may take up to … once") needs every shell's decoder.
- The signer page draws `PermitTransferFrom` as an allowance; give it transfer wording in the next
  page build.
