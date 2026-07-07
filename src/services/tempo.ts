/**
 * Tempo network gas model — the single home for Tempo-specific gas logic.
 *
 * Tempo (chainId 4217) has NO native gas coin. EVM `value`/`balance` are disabled
 * (eth_getBalance returns a sentinel; depositTo{value} reverts), so the canonical
 * ERC-4337 native prefund/refund is impossible — a UserOp with non-zero maxFeePerGas
 * fails `AA21 didn't pay prefund`.
 *
 * Vela keeps the SAME account stack on Tempo — Safe + passkey + EntryPoint, with the
 * IDENTICAL cross-chain address (the CREATE2 address has no chainId input). Only gas
 * SETTLEMENT is adapted:
 *
 *   1. The UserOperation is signed with maxFeePerGas = maxPriorityFeePerGas = 0, so
 *      EntryPoint's native accounting is a no-op (no AA21, refunds nothing).
 *   2. The bundler submits `EntryPoint.handleOps([userOp])` inside a native Tempo 0x76
 *      transaction with `feeToken` = a USD stablecoin, paying the real chain gas in it.
 *   3. To repay the bundler in-band, the wallet batches a
 *      `feeToken.transfer(bundlerEOA, reimbursement)` call into the UserOp's MultiSend.
 *
 * Everything else branches on `isTempoChain(chainId)` exactly once.
 *
 * This module is intentionally dependency-light (only the chain table) so its math is
 * pure and unit-testable. Verified against Tempo testnet: a 50k-gas transfer costs
 * ~$0.001 (= 1000 fee-token units at 6 decimals).
 */

import { chainMeta } from '@/models/chains';

/** Tempo mainnet (4217) + Moderato testnet (42431). */
export const TEMPO_CHAIN_IDS = new Set<number>([4217, 42431]);

/** True for any Tempo chain (stablecoin gas model). */
export function isTempoChain(chainId: number): boolean {
  return TEMPO_CHAIN_IDS.has(chainId) || chainMeta(chainId)?.gasModel === 'tempo';
}

/**
 * True if `tokenAddress` is Tempo's gas/fee token (pathUSD) on a Tempo chain.
 * pathUSD has no native coin to act as "gas", so it plays that role on Tempo —
 * used to categorise it under "Gas" in the token picker.
 */
export function isTempoFeeToken(chainId: number, tokenAddress?: string | null): boolean {
  return (
    isTempoChain(chainId) &&
    !!tokenAddress &&
    tokenAddress.toLowerCase() === TEMPO_DEFAULT_FEE_TOKEN.toLowerCase()
  );
}

/**
 * Canonical default fee token (pathUSD) — used to pay gas when the sent asset is not
 * itself a usable fee token (e.g. a dApp contract call). TIP-20 fee tokens live in the
 * reserved 0x20c0… range; pathUSD is the protocol-default fee token.
 */
export const TEMPO_DEFAULT_FEE_TOKEN = '0x20c0000000000000000000000000000000000000';

/** Every Tempo TIP-20 USD stablecoin uses 6 decimals (microdollar). */
export const TEMPO_FEE_TOKEN_DECIMALS = 6;

/** Tempo's protocol base fee fallback: 20e9 attodollars (USD×1e-18) per gas. */
export const TEMPO_BASE_FEE_ATTO = 20_000_000_000n;

/** Gas the bundler's OUTER 0x76 pays beyond the UserOp limits (EntryPoint + tx base). */
export const TEMPO_OUTER_OVERHEAD_GAS = 150_000n;

/**
 * callGasLimit budget per inner sub-call in a Tempo MultiSend batch. The 0x20c0… TIP-20
 * tokens are extraordinarily gas-heavy: a single `transfer` meters ~308k on-chain (vs
 * ~50k for a vanilla ERC-20). A batch always carries the reimbursement transfer on top
 * of the user's calls, and the bundler's eth_estimateUserOperationGas under-reports
 * (handleOps swallows the inner OOG, so the estimate settles where the inner op failed),
 * so this floor — not the estimate — keeps the atomic batch from reverting "out of gas".
 * 380k = measured ~308k + headroom for cold/first-time recipients and MultiSend's 63/64.
 */
export const TEMPO_CALL_GAS_PER_SUBCALL = 380_000n;

/** callGasLimit floor for a Tempo batch of `subCalls` inner calls (incl. reimbursement). */
export function tempoCallGasLimit(subCalls: number): bigint {
  return BigInt(Math.max(subCalls, 1)) * TEMPO_CALL_GAS_PER_SUBCALL;
}

/**
 * Verification gas for an UNDEPLOYED Safe on Tempo. The Safe deploy (initCode) meters
 * to ~3.9M gas on Tempo — far above the 2M used on EVM chains — so we provision more.
 * Must stay ≤ the bundler's Tempo verification cap (TEMPO_MAX_VERIFICATION_GAS = 8M).
 */
export const TEMPO_VERIFICATION_GAS_UNDEPLOYED = 6_000_000n;

/**
 * Reimbursement margin: charge ~2× the bundler's REAL cost (100% markup). The 2× both
 * gives the bundler a healthy margin AND absorbs estimate error — the op still pays for
 * itself as long as the real gas is ≤ 2× the estimate below. Crucially the charge is
 * priced off `tempoExpectedGas` (realistic), NOT the padded UserOp gas LIMITS, so the
 * user isn't billed for the safety headroom baked into callGasLimit/verificationGasLimit.
 */
export const TEMPO_FEE_MARGIN_NUM = 2n;
export const TEMPO_FEE_MARGIN_DEN = 1n;

/**
 * Realistic gas a Tempo 0x76 actually burns — used ONLY to PRICE the reimbursement, not
 * to set the UserOp gas fields (those stay padded for OOG safety). Measured on Tempo
 * mainnet: a deployed Safe's verification + preVerification ≈ 250k, the Safe deploy
 * (initCode) ≈ 3.9–4.1M, and one in-batch TIP-20 transfer ≈ 90–120k.
 */
export const TEMPO_DEPLOYED_GAS_EST = 250_000n;
export const TEMPO_DEPLOY_GAS_EST = 4_100_000n;
export const TEMPO_PER_SUBCALL_GAS_EST = 95_000n;

/** Realistic total gas for a batch of `subCalls` inner calls (incl. the reimbursement). */
export function tempoExpectedGas(deployed: boolean, subCalls: number): bigint {
  const fixed = deployed ? TEMPO_DEPLOYED_GAS_EST : TEMPO_DEPLOY_GAS_EST;
  return fixed + BigInt(Math.max(subCalls, 1)) * TEMPO_PER_SUBCALL_GAS_EST;
}

/**
 * Convert an attodollar amount (USD×1e-18) to a fee-token's smallest units:
 *   tokenUnits = atto × 10^decimals / 1e18
 */
export function attoToTokenUnits(
  atto: bigint,
  decimals: number = TEMPO_FEE_TOKEN_DECIMALS,
): bigint {
  if (atto <= 0n) return 0n;
  return (atto * 10n ** BigInt(decimals)) / 10n ** 18n;
}

/**
 * Raw gas fee in fee-token units (no margin), incl. outer-tx overhead:
 *   (totalGas + overhead) × gasPriceAtto → fee-token units.
 * `gasPriceAtto` is Tempo's eth_gasPrice (attodollars/gas); falls back to the
 * protocol base fee if a caller passes 0.
 */
export function tempoFeeTokenUnits(
  totalGas: bigint,
  gasPriceAtto: bigint,
  decimals: number = TEMPO_FEE_TOKEN_DECIMALS,
): bigint {
  const price = gasPriceAtto > 0n ? gasPriceAtto : TEMPO_BASE_FEE_ATTO;
  return attoToTokenUnits((totalGas + TEMPO_OUTER_OVERHEAD_GAS) * price, decimals);
}

/**
 * Stablecoin amount to reimburse the bundler, baked into the UserOp as a batched
 * transfer. `expectedGas` should be the REALISTIC gas (see `tempoExpectedGas`), not the
 * padded UserOp limits. = realistic cost × margin. Always ≥ 1 so the transfer is never a
 * no-op the bundler would reject.
 */
export function tempoReimbursement(
  expectedGas: bigint,
  gasPriceAtto: bigint,
  decimals: number = TEMPO_FEE_TOKEN_DECIMALS,
): bigint {
  const price = gasPriceAtto > 0n ? gasPriceAtto : TEMPO_BASE_FEE_ATTO;
  const base = attoToTokenUnits(expectedGas * price, decimals);
  const withMargin = (base * TEMPO_FEE_MARGIN_NUM) / TEMPO_FEE_MARGIN_DEN;
  return withMargin > 0n ? withMargin : 1n;
}

/**
 * Gas buffer the BUNDLER adds to the real simulated gas when computing the cost the EOA
 * reimbursement transfer must clear (bundler rejects if `reimbursed_to_EOA < (gasUsed + this) × price`).
 * MUST match TEMPO_COST_BUFFER_GAS in vela-bundler/shared/tempo.ts.
 */
export const TEMPO_COST_BUFFER_GAS = 80_000n;

/** Extra cushion on the EOA floor over the bundler's buffer, so estimate variance never
 *  causes a rejection when we route the surplus to the treasury. */
export const TEMPO_SPLIT_SAFETY_GAS = 20_000n;

/**
 * Split the Tempo reimbursement between the bundler EOA and the treasury — the Tempo analog of
 * the native VelaGasSettlementSplitter, done atomically in-band (Tempo's fee is an ERC-20, so a
 * receive()-based contract can't split it). The EOA is floored at the bundler's cost
 * (realistic gas + the bundler's buffer + a safety cushion) so the transfer always clears the
 * bundler's accept check; the surplus (the profit) goes to the treasury. When the margin is too
 * thin to cover the floor, everything stays on the EOA (treasury 0) so the tx is never rejected.
 *
 * With the default 2× fee margin this routes ~40–50% of a healthy fee to the treasury, matching
 * the native 50/50 intent while respecting Tempo's hard "EOA must be made whole" constraint.
 */
export function tempoSettlementSplit(
  reimbursement: bigint,
  expectedGas: bigint,
  gasPriceAtto: bigint,
  decimals: number = TEMPO_FEE_TOKEN_DECIMALS,
): { eoa: bigint; treasury: bigint } {
  const price = gasPriceAtto > 0n ? gasPriceAtto : TEMPO_BASE_FEE_ATTO;
  const eoaFloor = attoToTokenUnits(
    (expectedGas + TEMPO_COST_BUFFER_GAS + TEMPO_SPLIT_SAFETY_GAS) * price,
    decimals,
  );
  if (reimbursement <= eoaFloor) return { eoa: reimbursement, treasury: 0n };
  return { eoa: eoaFloor, treasury: reimbursement - eoaFloor };
}
