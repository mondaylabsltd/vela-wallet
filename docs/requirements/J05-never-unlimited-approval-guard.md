# J05 · Never-Unlimited Approval Guard & Editing UX

| | |
|---|---|
| **Epic** | J — Simulation & Safety Guards |
| **Status** | ✅ Shipped |
| **Owner** | Shelchin |
| **Depends on** | I06 (ABI/typed-data decoding) |
| **Related** | I01 (clear signing), K06 (SIWE), K07 (batch calls), H08 (payroll) |

## 1. Summary

Vela **refuses to sign an unbounded token approval**. A descriptor-independent, unbypassable
submit-time guard (`enforceNoUnlimited`) re-scans every outgoing request's raw calldata / typed data
and throws on unbounded ERC-20 or Permit2 approvals. The UI offers **no "Max / Unlimited" preset
anywhere** and forces the user to pick a finite cap (or revoke). Together these turn the single most
common drainer vector into a dead end.

## 2. Background & context

Unlimited approvals are how most wallet-drain exploits actually steal funds long after the "harmless"
signature. Descriptor lookup (I02) is exactly what fails on novel/hostile contracts, so the final
guard must **not** depend on a resolved descriptor — it reads raw bytes. The guard runs on both single
`eth_sendTransaction` and every leg of an EIP-5792 batch (K07), so a batch can't smuggle an unbounded
approval past the per-tx check.

## 3. Users & stories

- As a **user connecting to a dApp**, I want unlimited approvals to be impossible to grant by accident, so that a malicious or buggy dApp can't drain me later.
- As a **power user**, I want to set a finite allowance for exactly the amount I'm using, so that my exposure is bounded.
- As a **cautious signer**, I want a boolean "approve-all" (e.g. `setApprovalForAll`) to look dangerous and default to *no*, so that I never grant it on autopilot.

## 4. Functional requirements

- **FR-1** — `enforceNoUnlimited(method, params)` runs at **submit time**, reading raw calldata / typed
  data (not a descriptor), and throws for any unbounded amount.
- **FR-2** — "Unbounded" is bit-width aware: `UNLIMITED_CAP_256 = 1<<200` for ERC-20 `uint256`;
  `UNLIMITED_CAP_160 = 1<<152` for Permit2 `uint160`. Any amount ≥ the cap is rejected.
- **FR-3** — Detection covers ERC-20 `approve`, Permit2 on-chain `approve(address,address,uint160,uint48)`
  (token is the **first arg**, not the tx `to`), and Permit2 typed-data `PermitSingle` / `PermitBatch`.
- **FR-4** — Every leg of an EIP-5792 batch is guarded individually (K07); one unbounded leg fails the batch.
- **FR-5** — `EditableApproveCard` presents **no Max/Unlimited preset**; an unbounded incoming request is
  forced into `custom` mode with confirm **disabled** until a finite amount is entered.
- **FR-6** — Boolean grants (`setApprovalForAll`, DAI-style permit) render as a **danger** card defaulting to no selection (an explicit "Grant all anyway" tap).
- **FR-7** — Re-encoding a chosen finite cap is byte-surgical and self-verifying (`assertOnlyWordChanged`) — exactly one calldata word changes.

## 5. Non-functional requirements

- **NFR-1** — The guard is **unbypassable**: no code path submits a signing request without passing through it.
- **NFR-2** — Zero network dependency — pure calldata/typed-data inspection, so it holds even when descriptor/RPC services are down.
- **NFR-3** — Re-encode never alters any field other than the amount word (verified, not assumed).

## 6. UX / flow notes

Approve requests show a finite-cap input, a "revoke (0)" option, and — for booleans — a red danger card.
There is intentionally no shortcut to "unlimited." Error thrown by the guard surfaces as a clear
"Unlimited approvals are disabled — choose a finite amount," never a silent failure.

## 7. Acceptance criteria

- [ ] **AC-1** — An `approve(spender, 2^256-1)` request is rejected by `enforceNoUnlimited`, before signing.
- [ ] **AC-2** — A Permit2 `uint160` max amount is rejected via the 160-bit cap.
- [ ] **AC-3** — A batch containing one unbounded approval leg fails; an all-finite batch passes.
- [ ] **AC-4** — `EditableApproveCard` exposes no "Max/Unlimited" control; confirm stays disabled until a finite amount is chosen.
- [ ] **AC-5** — Re-encoding a finite cap changes exactly one calldata word (`assertOnlyWordChanged` holds).

## 8. Out of scope / non-goals

- Human-readable rendering of the approval intent — see **I01 / I06**.
- Revoking *existing* on-chain allowances (this guards new grants, not a revoke dashboard).

## 9. Dependencies, risks & open questions

- **Depends on:** calldata/typed-data decoding (I06).
- **Risk:** a novel approval encoding not matched by `detectApproval` would bypass the *editor* — but the
  submit-time guard reads raw bytes and is the backstop. Keep detection and the cap constants in sync with new standards.
- **Open question:** whether to also warn on "large but finite" approvals far exceeding the simulated need — currently only unbounded is blocked.

## 10. Source anchors

- `src/services/approval-guard.ts:14` (guard role), `:32-43` (caps + `isUnboundedAmount`), `:116` (`detectApproval`), `:139-208` (calldata + Permit2 detect), `:210` (typed-data detect), `:282-320` (`rewriteApprovalParams`, unlimited-disabled throw).
- `src/hooks/use-dapp-signing.ts:322` (per-tx guard), `:367` (per-batch-leg guard).
- `src/components/signing/EditableApproveCard.tsx:4-6,62` (no Max preset, forced custom mode).
- `docs/CONTENT-SOURCE-100-CLUES.md` — clues 23, 70.
