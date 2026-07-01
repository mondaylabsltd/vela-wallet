# G01 · UserOp Construction & Counterfactual Deployment (v0.7)

| | |
|---|---|
| **Epic** | G — Transaction Engine (ERC-4337) |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | B06, B07 |
| **Related** | G02, G03, G09, K07, H01 |

## 1. Summary

Vela builds **ERC-4337 v0.7 UserOperations** in the split form (`factory` + `factoryData`,
`verificationGasLimit`/`callGasLimit` as uint128). If the account isn't yet deployed, `buildInitCode`
**prepends `SafeProxyFactory.createProxyWithNonce(...)`** so the first UserOp **atomically deploys and
executes** — the counterfactual account (B07) becomes real on its first transaction, paid from its own
balance.

## 2. Background & context

The counterfactual model (B07) means most users' first send must also deploy. Bundling deployment into
the first op avoids a separate "activate" transaction and keeps the address stable. Undeployed ops need
a higher verification-gas floor (G03) because deployment is included.

## 3. Users & stories

- As a **new user**, I want my first send to just work, so that I don't do a separate deploy step.
- As a **developer**, I want standard v0.7 UserOps, so that any compliant bundler/EntryPoint handles them.

## 4. Functional requirements

- **FR-1** — Construct UserOps in v0.7 split form (`factory`/`factoryData`, uint128 gas limits) against EntryPoint v0.7.
- **FR-2** — For an undeployed account, `buildInitCode` prepends `SafeProxyFactory.createProxyWithNonce(...)` so deploy+execute is atomic.
- **FR-3** — Encode the call(s): single call for Send (H01), `multiSend` for batch/split/sweep (K07/H07).
- **FR-4** — Undeployed ops use a **2,000,000-gas verification floor** (G03).
- **FR-5** — Hand off to hashing/signing (G02) and submission (G09).

## 5. Non-functional requirements

- **NFR-1** — The built op is deterministic given inputs; deploy inclusion depends only on on-chain deployment state.
- **NFR-2** — Works identically across all supported chains (B06).

## 6. UX / flow notes

No direct UI; the user sees one confirmation whether or not a deploy is bundled. First-tx-on-a-chain is transparently a deploy+execute.

## 7. Acceptance criteria

- [ ] **AC-1** — A first send on a chain produces a UserOp that deploys and executes atomically.
- [ ] **AC-2** — A subsequent send omits initCode.
- [ ] **AC-3** — The op validates against EntryPoint v0.7 (split-form fields correct).

## 8. Out of scope / non-goals

- Hashing/signing — **G02**; gas estimation — **G03**; receipts — **G09**.

## 9. Dependencies, risks & open questions

- **Risk:** `deployer-api.ts` is a mock — production deploy facts come from the bundler path here, not that file.
- **Open question:** None.

## 10. Source anchors

- `src/services/safe-transaction.ts:835-863` (`buildInitCode`), `:172-191` (multiSend call encoding).
- `src/services/deployer-api.ts:140` (EntryPoint v0.7 — note: file is a mock).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 37, 39.
