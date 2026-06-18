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
 * Verification gas for an UNDEPLOYED Safe on Tempo. The Safe deploy (initCode) meters
 * to ~3.9M gas on Tempo — far above the 2M used on EVM chains — so we provision more.
 * Must stay ≤ the bundler's Tempo verification cap (TEMPO_MAX_VERIFICATION_GAS = 8M).
 */
export const TEMPO_VERIFICATION_GAS_UNDEPLOYED = 6_000_000n;

/** Safety margin on the baked-in reimbursement (1.5×) so a slightly-low estimate
 *  isn't rejected by the bundler. Tempo gas price is protocol-fixed (not volatile),
 *  so the only variance is the gas-units estimate, which is already padded 1.5×. */
export const TEMPO_FEE_MARGIN_NUM = 5n;
export const TEMPO_FEE_MARGIN_DEN = 4n;

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
 * transfer. = raw fee-token cost × safety margin. Always ≥ 1 so the transfer is
 * never a no-op the bundler would reject.
 */
export function tempoReimbursement(
  totalGas: bigint,
  gasPriceAtto: bigint,
  decimals: number = TEMPO_FEE_TOKEN_DECIMALS,
): bigint {
  const base = tempoFeeTokenUnits(totalGas, gasPriceAtto, decimals);
  const withMargin = (base * TEMPO_FEE_MARGIN_NUM) / TEMPO_FEE_MARGIN_DEN;
  return withMargin > 0n ? withMargin : 1n;
}
