/**
 * Builds, signs, and submits ERC-4337 UserOperations for Safe wallets.
 * TypeScript port of SafeTransactionService.swift.
 */

import {
  abiEncodeAddress,
  abiEncodeBytes32,
  abiEncodeUint256,
  abiEncodeUint256Hex,
  functionSelector,
  keccak256,
} from './eth-crypto';

import { concatBytes, fromHex, stripHexPrefix, toHex } from './hex';

import {
  ENTRY_POINT,
  MULTI_SEND,
  SAFE_4337_MODULE,
  SAFE_PROXY_FACTORY,
  SAFE_SINGLETON,
  WEBAUTHN_SIGNER,
  calculateSaltNonce,
  encodeMultiSendTx,
  encodeSetupData,
  parsePublicKey
} from './safe-address';

import { derSignatureToRaw } from './attestation-parser';
import { rpcCall } from './rpc-adapter';
import { requestUserOpReceipt, USER_OP_RECEIPT_POLL_INTERVAL_MS } from './tx-reconciler';
import { gasQuoteShouldZero } from './dev/fault-injection';
import {
  fetchBundlerAccountInfo,
  fetchInBandGasQuotes,
  findInBandGasQuote,
  isInBandChain,
  type InBandGasQuote,
} from './bundler-service';
import {
  isTempoChain,
  tempoReimbursement,
  tempoCallGasLimit,
  tempoExpectedGas,
  TEMPO_DEFAULT_FEE_TOKEN,
  TEMPO_FEE_TOKEN_DECIMALS,
  TEMPO_VERIFICATION_GAS_UNDEPLOYED,
} from './tempo';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const VERIFICATION_GAS_DEPLOYED = 300_000n;
// Undeployed: sendUserOp uses bigintMax(estimated, 2_000_000n), so estimate must match.
const VERIFICATION_GAS_UNDEPLOYED = 2_000_000n;
const CALL_GAS_LIMIT = 200_000n;  // 200k — simple transfers; bundler estimation may increase
const PRE_VERIFICATION_GAS = 100_000n; // 100k — must exceed bundler's calculated preVerificationGas
// Above this callData size the static defaults can't be trusted (a deploy's
// preVerificationGas scales with calldata; its callGasLimit is unknown). If live
// estimation fails for such an op we refuse rather than submit a doomed one.
// Simple transfers are ~160-260 bytes, so this only catches real contract calls.
const ESTIMATION_REQUIRED_CALLDATA = 1024;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

interface UserOperation {
  sender: string;
  nonce: string;
  initCode: Uint8Array;
  callData: Uint8Array;
  verificationGasLimit: bigint;
  callGasLimit: bigint;
  preVerificationGas: bigint;
  maxFeePerGas: bigint;
  maxPriorityFeePerGas: bigint;
  paymasterAndData: Uint8Array;
  signature: Uint8Array;
}

export interface TransactionResult {
  userOpHash: string;
  txHash: string;
}

export interface SubmitResult {
  userOpHash: string;
  /** Resolves to txHash once the receipt is available. */
  waitForTxHash: () => Promise<string>;
}

interface GasEstimate {
  verificationGasLimit: bigint;
  callGasLimit: bigint;
  preVerificationGas: bigint;
}

type SignFn = (challenge: Uint8Array) => Promise<{
  signature: Uint8Array;
  authenticatorData: Uint8Array;
  clientDataJSON: Uint8Array;
}>;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/** Send native token (ETH, POL, BNB, etc.) */
export async function sendNative(
  from: string,
  to: string,
  valueWei: string,
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
  maxFeeOverride?: bigint,
  // In-band chains only: pay gas in this whitelisted stablecoin (null/omitted = native).
  gasFeeToken?: string | null,
  // In-band chains only: the displayed fee (amount + recipient) — signed verbatim.
  quotedFee?: QuotedInBandFee,
): Promise<SubmitResult> {
  if (isTempoChain(chainId)) {
    // Tempo has no native coin; a native send is unusual but routed for consistency
    // (gas is paid in the default stablecoin, not the value being moved).
    return sendUserOpTempo(from, [{ to, value: valueWei, data: new Uint8Array(0) }], TEMPO_DEFAULT_FEE_TOKEN, chainId, publicKeyHex, signFn);
  }
  if (await isInBandChain(chainId, from)) {
    // In-band settlement: gas is repaid by a batched transfer, not a prefunded gas account.
    return sendUserOpInBand(from, [{ to, value: valueWei, data: new Uint8Array(0) }], gasFeeToken ?? null, chainId, publicKeyHex, signFn, quotedFee);
  }
  const callData = buildNativeCallData([{ to, value: valueWei, data: new Uint8Array(0) }], false);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn, maxFeeOverride);
}

/** Send ERC-20 token. */
export async function sendERC20(
  from: string,
  tokenAddress: string,
  to: string,
  amountWei: string,
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
  maxFeeOverride?: bigint,
  // In-band chains only: pay gas in this whitelisted stablecoin (null/omitted = native).
  gasFeeToken?: string | null,
  // In-band chains only: the displayed fee (amount + recipient) — signed verbatim.
  quotedFee?: QuotedInBandFee,
): Promise<SubmitResult> {
  const transferSelector = functionSelector('transfer(address,uint256)');
  const transferData = concatBytes(
    transferSelector,
    abiEncodeAddress(to),
    abiEncodeUint256Hex(amountWei),
  );

  if (isTempoChain(chainId)) {
    // Gas is paid in pathUSD (the canonical Tempo fee token). Tempo requires the gas
    // payer to PRE-HOLD the fee token (a 0-balance account can't submit), so standardising
    // on one token keeps the bundler gas account to a single float; the Safe needs a small
    // pathUSD balance, like ETH for gas. See services/tempo.ts.
    return sendUserOpTempo(from, [{ to: tokenAddress, value: '0', data: transferData }], TEMPO_DEFAULT_FEE_TOKEN, chainId, publicKeyHex, signFn);
  }

  if (await isInBandChain(chainId, from)) {
    return sendUserOpInBand(from, [{ to: tokenAddress, value: '0', data: transferData }], gasFeeToken ?? null, chainId, publicKeyHex, signFn, quotedFee);
  }

  const callData = buildNativeCallData([{ to: tokenAddress, value: '0', data: transferData }], false);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn, maxFeeOverride);
}

/** Send arbitrary contract call (e.g. dApp interaction like swap). */
export async function sendContractCall(
  from: string,
  to: string,
  valueWei: string,
  data: Uint8Array,
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
  maxFeeOverride?: bigint,
  // In-band chains only: pay gas in this whitelisted stablecoin (null/omitted = native).
  gasFeeToken?: string | null,
  // In-band chains only: the displayed fee (amount + recipient) — signed verbatim.
  quotedFee?: QuotedInBandFee,
): Promise<SubmitResult> {
  if (isTempoChain(chainId)) {
    // dApp / contract call: pay gas in the default stablecoin (pathUSD).
    return sendUserOpTempo(from, [{ to, value: valueWei, data }], TEMPO_DEFAULT_FEE_TOKEN, chainId, publicKeyHex, signFn);
  }
  if (await isInBandChain(chainId, from)) {
    return sendUserOpInBand(from, [{ to, value: valueWei, data }], gasFeeToken ?? null, chainId, publicKeyHex, signFn, quotedFee);
  }
  const callData = buildNativeCallData([{ to, value: valueWei, data }], false);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn, maxFeeOverride);
}

/** Send batched calls atomically via Safe MultiSend (EIP-5792 wallet_sendCalls). */
export async function sendBatchCalls(
  from: string,
  calls: Array<{ to: string; value: string; data: string }>,
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
  // The per-gas price the confirm screen quoted & displayed (feeEstimate.maxFeePerGas,
  // a tip-inclusive bundler quote at the user's tier). Threaded through so the batch
  // submits EXACTLY the price it showed — same as single sends. Omitted → sendUserOp
  // re-derives from getGasPrices (also tip-inclusive now), so this never underprices.
  maxFeeOverride?: bigint,
  // In-band chains only: pay gas in this whitelisted stablecoin (null/omitted = native).
  gasFeeToken?: string | null,
  // In-band chains only: the displayed fee (amount + recipient) — signed verbatim.
  quotedFee?: QuotedInBandFee,
): Promise<SubmitResult> {
  const byteCalls: MultiSendCall[] = calls.map(c => ({
    to: c.to,
    value: c.value,
    data: c.data && c.data !== '0x' ? fromHex(stripHexPrefix(c.data)) : new Uint8Array(0),
  }));

  if (isTempoChain(chainId)) {
    // Tempo signs maxFee=0 (gas paid in stablecoin) — the override doesn't apply.
    return sendUserOpTempo(from, byteCalls, TEMPO_DEFAULT_FEE_TOKEN, chainId, publicKeyHex, signFn);
  }

  if (await isInBandChain(chainId, from)) {
    // In-band also signs maxFee=0 — the override doesn't apply.
    return sendUserOpInBand(from, byteCalls, gasFeeToken ?? null, chainId, publicKeyHex, signFn, quotedFee);
  }

  const callData = buildNativeCallData(byteCalls, true);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn, maxFeeOverride);
}

// ---------------------------------------------------------------------------
// Prefetch (public) — warm caches before user confirms
// ---------------------------------------------------------------------------

/**
 * Prefetch all RPC data needed for sendUserOp so that when the user taps
 * "Confirm & Send", the calls resolve instantly from cache.
 * Call this when entering the confirm screen.
 */
export function prefetchForSend(safeAddress: string, chainId: number): void {
  // Fire-and-forget — warm caches in parallel
  verifyChainReady(chainId).catch(() => {});
  isDeployed(safeAddress, chainId).catch(() => {});
  getNonce(safeAddress, chainId).catch(() => {});
  getGasPrices(chainId).catch(() => {});
}

// ---------------------------------------------------------------------------
// Gas Estimation (public)
// ---------------------------------------------------------------------------

/**
 * Gas speed tiers — multiplier over chain gas price for outer tx speed.
 * Higher tier → bundler bids higher gas → faster inclusion.
 *
 * maxFeePerGas = gasPrice × speedTier × BUNDLER_MARGIN
 * Bundler derives: outerGasPrice = maxFeePerGas / BUNDLER_MARGIN = gasPrice × speedTier
 * Margin is always BUNDLER_MARGIN - 1 = 50%, independent of tier.
 * User cost scales with tier (faster = more expensive).
 */
export type GasTier = 'slow' | 'standard' | 'rapid' | 'fast';

export const GAS_TIER_MULTIPLIERS: Record<GasTier, { num: bigint; den: bigint; label: string }> = {
  slow:     { num: 11n, den: 10n, label: 'Slow' },       // ×1.1 (just above market)
  standard: { num: 12n, den: 10n, label: 'Standard' },   // ×1.2 (comfortable buffer)
  rapid:    { num: 15n, den: 10n, label: 'Rapid' },       // ×1.5 (priority)
  fast:     { num: 20n, den: 10n, label: 'Fast' },        // ×2.0 (high priority)
};

/**
 * Local-fallback relayer margin, used by calcMaxFeePerGas ONLY when the bundler
 * can't quote a price (pimlico_getUserOperationGasPrice). The bundler is the
 * source of truth; this must match its WALLET_GAS_MARGIN_PERCENT (default 100)
 * so a fallback op isn't under-priced and rejected.
 * E.g. 100 → 2x markup → relayer fee ≈ network fee.
 */
const BUNDLER_MARGIN_PERCENT = 100;
const BUNDLER_MARGIN_NUM = BigInt(100 + BUNDLER_MARGIN_PERCENT);  // 150
const BUNDLER_MARGIN_DEN = 100n;

/** Detailed gas fee estimate for display and max-send calculation. */
export interface TransactionFeeEstimate {
  /** Total estimated cost in wei (totalGas × maxFeePerGas). */
  totalWei: bigint;
  /** What the user pays per gas — quoted by the bundler (single source of truth). */
  maxFeePerGas: bigint;
  /** Per-gas on-chain network cost (the part that goes to validators). */
  networkFeePerGas: bigint;
  /** Per-gas Vela relayer fee (maxFeePerGas − networkFeePerGas). */
  relayerFeePerGas: bigint;
  /** Intended outer tx gas price (≈ networkFeePerGas). */
  bundlerGasPrice: bigint;
  /** Total gas units (verification + call + preVerification). */
  totalGas: bigint;
  /** Whether the wallet is already deployed on this chain. */
  deployed: boolean;
  /** Which tier was used for this estimate. */
  tier: GasTier;
  /** True when the price came from the bundler quote (vs a local fallback). */
  quoted: boolean;
  /** In-band settlement estimate (generic chains once the bundler enables them). */
  inBand?: boolean;
  /** The asset the gas fee is paid in. Absent → native via the legacy model. */
  feeAsset?: { kind: 'native' } | { kind: 'erc20'; token: string; decimals: number; amount: bigint };
  /** In-band: the quote's transfer recipient. The submit path signs THIS quote
   *  (amount + recipient) verbatim — what the confirm slide shows is what executes. */
  feeRecipient?: string;
}

/** In-band quoted fee as displayed by the confirm UI — the submit path signs exactly
 *  this (amount + recipient); the bundler's gate re-verifies and rejects loudly if
 *  stale, so the user re-confirms a NEW number instead of silently paying one. */
export interface QuotedInBandFee {
  amount: bigint;
  recipient: string;
}

const INBAND_MARKUP = 3n;
const USD_PRICE_DECIMALS = 8;
const USD_PRICE_SCALE = 10n ** BigInt(USD_PRICE_DECIMALS);
const STABLE_MIN_USD_SCALED = USD_PRICE_SCALE / 100n; // $0.01

function ceilDiv(numerator: bigint, denominator: bigint): bigint {
  return (numerator + denominator - 1n) / denominator;
}

/** Convert the decimal strings returned by the bundler to a fixed USD scale.
 * Rounding native up and the fee token down ensures conversion never undercharges. */
function usdPriceScaled(value: string, roundUp: boolean): bigint | null {
  const match = /^(\d+)(?:\.(\d+))?$/.exec(value.trim());
  if (!match) return null;
  const integer = BigInt(match[1]);
  const fraction = match[2] ?? '';
  const kept = (fraction.slice(0, USD_PRICE_DECIMALS)).padEnd(USD_PRICE_DECIMALS, '0');
  let scaled = integer * USD_PRICE_SCALE + BigInt(kept);
  if (roundUp && /[1-9]/.test(fraction.slice(USD_PRICE_DECIMALS))) scaled += 1n;
  return scaled;
}

/** Calculate the exact in-band reimbursement from the transaction's gas basis.
 * `requiredAmount` from the RPC is intentionally not used: it is only the
 * bundler's minimum threshold and will be removed from that endpoint. */
export function calculateInBandFeeAmount(
  totalGas: bigint,
  gasPrice: bigint,
  feeAsset: Pick<InBandGasQuote, 'asset' | 'decimals' | 'usdPrice'>,
  nativeAsset: Pick<InBandGasQuote, 'asset' | 'decimals' | 'usdPrice'>,
): bigint | null {
  if (totalGas < 0n || gasPrice < 0n || nativeAsset.asset !== 'native') return null;
  const nativeUnit = 10n ** BigInt(nativeAsset.decimals);
  // Minimum is 0.00001 native coin. For an unusual native precision below 5,
  // one base unit is the smallest representable safe floor.
  const nativeMinimum = nativeAsset.decimals >= 5
    ? 10n ** BigInt(nativeAsset.decimals - 5)
    : 1n;
  const nativeAmount = bigintMax(totalGas * gasPrice * INBAND_MARKUP, nativeMinimum);
  if (feeAsset.asset === 'native') return nativeAmount;

  const nativeUsdPrice = usdPriceScaled(nativeAsset.usdPrice, true);
  const feeTokenUsdPrice = usdPriceScaled(feeAsset.usdPrice, false);
  if (!nativeUsdPrice || !feeTokenUsdPrice) return null;
  const feeTokenUnit = 10n ** BigInt(feeAsset.decimals);
  const convertedAmount = ceilDiv(
    nativeAmount * nativeUsdPrice * feeTokenUnit,
    nativeUnit * feeTokenUsdPrice,
  );
  const stableMinimum = ceilDiv(STABLE_MIN_USD_SCALED * feeTokenUnit, feeTokenUsdPrice);
  return bigintMax(convertedAmount, stableMinimum);
}

/** Re-quote an existing in-band estimate for a different fee asset. The bundler
 * returns all assets in one address-only response; its short-lived shared cache
 * normally makes a chip switch a local lookup. */
export async function requoteInBandFee(
  prev: TransactionFeeEstimate,
  chainId: number,
  safeAddress: string,
  gasFeeToken: string | null,
): Promise<TransactionFeeEstimate | null> {
  if (!prev.inBand) return null;
  const quotes = await fetchInBandGasQuotes(chainId, safeAddress);
  if (!quotes) return null;
  const q = findInBandGasQuote(quotes, gasFeeToken);
  const nativeQuote = findInBandGasQuote(quotes);
  if (!q || !nativeQuote) return null;
  const amount = calculateInBandFeeAmount(prev.totalGas, prev.networkFeePerGas, q, nativeQuote);
  if (amount === null) return null;
  return {
    ...prev,
    totalWei: q.asset === 'native' ? amount : 0n,
    maxFeePerGas: 0n,
    feeRecipient: q.recipient,
    feeAsset: q.asset === 'erc20'
      ? { kind: 'erc20', token: q.feeToken!, decimals: q.decimals, amount }
      : { kind: 'native' },
  };
}

/**
 * The bundler's gas-account balance check uses the raw chain gasPrice, but a fee
 * estimate's `totalWei` bakes in the user-facing tier markup (gasPrice × tier).
 * Divide the markup back out so a funding pre-check compares against what the
 * bundler will actually require — otherwise we'd over-prompt for funding.
 * Shared by the Send flow and the dApp signing flow.
 */
export function rawBundlerGasCost(fee: TransactionFeeEstimate): bigint {
  const m = GAS_TIER_MULTIPLIERS[fee.tier];
  return (fee.totalWei * m.den) / m.num;
}

/** Fetch fresh on-chain gas price (bypasses cache). */
export async function refreshGasPrice(chainId: number): Promise<bigint> {
  _gasPriceCache.delete(chainId);
  const { gasPrice } = await getGasPrices(chainId);
  return gasPrice;
}

/**
 * Fee estimate for Tempo, denominated in the fee-token for display. The UserOp itself
 * pays 0 gas; the user is charged the stablecoin reimbursement baked into the batch.
 * `totalWei` is that reimbursement scaled to attodollars (USD×1e-18) so the existing
 * USD display path (totalWei / 1e18) renders it correctly. Divide totalWei by 1e12 to
 * recover the fee-token amount (6 dec) for a balance check.
 */
async function estimateTempoFee(
  from: string,
  chainId: number,
  tier: GasTier,
  // The actual call (dApp tx). Omitted for simple transfers. Passing the real call makes the
  // DISPLAYED fee accurate for contract calls, whose gas dwarfs a transfer's flat estimate.
  tx?: { to: string; value?: string; data?: string },
): Promise<TransactionFeeEstimate> {
  const [deployed, { gasPrice }] = await Promise.all([
    isDeployed(from, chainId),
    getGasPrices(chainId),
  ]);

  // Mirror sendUserOpTempo's pricing: the displayed fee is the reimbursement, priced off the
  // REALISTIC gas, NOT the padded UserOp limits. A simple send is 1 transfer + 1 reimbursement
  // (2 sub-calls); a dApp call adds its own sub-call (→ 3, allowing the treasury-split transfer).
  const hasCall = !!tx?.to && !!tx.data && tx.data !== '0x';
  let expectedGas = tempoExpectedGas(deployed, hasCall ? 3 : 2);

  // For a contract call on a DEPLOYED Safe, refine off the bundler's own estimate of the REAL
  // call (same basis sendUserOpTempo prices against) so the quote tracks what's charged instead
  // of showing a transfer-sized fee. Undeployed → the deploy cost dominates; keep the static model.
  if (hasCall && deployed) {
    try {
      // This field is part of the final UserOp. Never replace a failed read
      // with nonce 0 in a simulation.
      const nonce = await getNonce(from, chainId);
      const innerCall: MultiSendCall = {
        to: tx!.to,
        value: stripHexPrefix(tx!.value ?? '0') || '0',
        data: tx!.data && tx!.data !== '0x' ? fromHex(stripHexPrefix(tx!.data)) : new Uint8Array(0),
      };
      // Representative batch: the call + two placeholder reimbursement transfers (EOA + treasury,
      // the split case — so the quote doesn't under-count when splitting; value doesn't affect
      // gas). estimateGas returns un-padded limits ≈ the bundler's simGas.
      const callData = buildMultiSendExecuteCallData([
        innerCall,
        { to: TEMPO_DEFAULT_FEE_TOKEN, value: '0', data: encodeErc20Transfer(from, 1n) },
        { to: TEMPO_DEFAULT_FEE_TOKEN, value: '0', data: encodeErc20Transfer(from, 1n) },
      ]);
      const dummyOp: UserOperation = {
        sender: from,
        nonce,
        initCode: new Uint8Array(0),
        callData,
        verificationGasLimit: VERIFICATION_GAS_DEPLOYED,
        callGasLimit: tempoCallGasLimit(3),
        preVerificationGas: PRE_VERIFICATION_GAS,
        maxFeePerGas: 0n,
        maxPriorityFeePerGas: 0n,
        paymasterAndData: new Uint8Array(0),
        signature: buildDummySignature(),
      };
      const est = await estimateGas(dummyOp, chainId);
      expectedGas = bigintMax(expectedGas, est.verificationGasLimit + est.callGasLimit + est.preVerificationGas);
    } catch (err) {
      // A contract call we can't estimate: surface it (the confirm screen shows "couldn't
      // estimate" + retry) instead of a misleading transfer-sized fee that sendUserOpTempo would
      // then reject/overcharge. Consistent with the native estimate + the submit-side guard.
      throw err instanceof Error ? err : new Error('Gas estimation failed');
    }
  }

  const reimbursement = tempoReimbursement(expectedGas, gasPrice, TEMPO_FEE_TOKEN_DECIMALS);
  const totalWei = reimbursement * 10n ** BigInt(18 - TEMPO_FEE_TOKEN_DECIMALS);

  return {
    totalWei,
    maxFeePerGas: gasPrice,
    networkFeePerGas: gasPrice,
    relayerFeePerGas: 0n,
    bundlerGasPrice: gasPrice,
    totalGas: expectedGas,
    deployed,
    tier,
    quoted: false,
  };
}

/** Estimate the total gas fee in wei for a transaction. */
export async function estimateTransactionFee(
  from: string,
  chainId: number,
  tier: GasTier = 'standard',
  // The actual call being made (dApp tx). When omitted (simple transfers) a small
  // dummy is used. Passing the real tx makes the estimate accurate for contract
  // calls and deploys, whose callGasLimit/preVerificationGas dwarf a transfer's.
  tx?: { to: string; value?: string; data?: string },
  // An EIP-5792 batch (wallet_sendCalls) — estimate against the WHOLE MultiSend of
  // every call, the same bundle sendBatchCalls submits, so the batch fee is accurate.
  // Takes precedence over `tx` when non-empty.
  batchCalls?: { to: string; value?: string; data?: string }[],
  // In-band chains only: quote the gas fee in this whitelisted stablecoin instead of
  // native (must match the gasFeeToken the send path will use). Ignored elsewhere.
  gasFeeToken?: string | null,
  // Required to construct the real initCode when this Safe is not yet deployed.
  // Without it we deliberately skip the live UserOp simulation rather than send
  // a draft which cannot match the final operation.
  publicKeyHex?: string,
): Promise<TransactionFeeEstimate> {
  // Tempo pays gas in a stablecoin, not the native coin — separate model. Forward the tx so a
  // dApp contract call is quoted off its REAL gas, not a transfer-sized default.
  if (isTempoChain(chainId)) return estimateTempoFee(from, chainId, tier, tx);

  // The bundler is the single source of truth for the price. getBundlerGasQuote
  // throws GasQuoteTooHighError (propagated here = refuse) if the quote is abusive,
  // and returns null only when the bundler can't quote (then we fall back locally).
  const [deployed, nonce, { gasPrice }, quote] = await Promise.all([
    // These are UserOperation correctness fields, not best-effort preview
    // data. If either lookup fails, propagate the error and do not estimate.
    isDeployed(from, chainId),
    getNonce(from, chainId),
    getGasPrices(chainId),
    getBundlerGasQuote(chainId, tier),
  ]);

  let userOpMaxFee: bigint;
  let networkFeePerGas: bigint;
  let relayerFeePerGas: bigint;
  let bundlerGasPrice: bigint;
  const quoted = quote !== null;

  if (quote) {
    userOpMaxFee = quote.maxFeePerGas;
    networkFeePerGas = quote.networkFeePerGas;
    relayerFeePerGas = quote.relayerFeePerGas;
    bundlerGasPrice = quote.networkFeePerGas;
  } else {
    // Local fallback: bundler doesn't support pimlico_getUserOperationGasPrice.
    userOpMaxFee = calcMaxFeePerGas(gasPrice, tier);
    const m = GAS_TIER_MULTIPLIERS[tier];
    bundlerGasPrice = (gasPrice * m.num) / m.den;
    if (bundlerGasPrice < 1n) bundlerGasPrice = 1n;
    networkFeePerGas = bundlerGasPrice;
    relayerFeePerGas = userOpMaxFee > bundlerGasPrice ? userOpMaxFee - bundlerGasPrice : 0n;
  }

  // Estimate against the REAL call when we have it (dApp tx); otherwise a minimal
  // ERC-20-sized dummy. Routed through buildNativeCallData — the SAME builder the real send
  // uses, so the estimate matches the final native-chain call shape.
  const innerEstCall: MultiSendCall = tx?.to
    ? {
        to: tx.to,
        value: stripHexPrefix(tx.value ?? '0') || '0',
        data: tx.data && tx.data !== '0x' ? fromHex(stripHexPrefix(tx.data)) : new Uint8Array(0),
      }
    : { to: from, value: '0', data: new Uint8Array(68) };
  // A batch estimates against ALL its calls (the real MultiSend); otherwise the single call.
  const estCalls: MultiSendCall[] = batchCalls && batchCalls.length > 0
    ? batchCalls.map((c) => ({
        to: c.to,
        value: stripHexPrefix(c.value ?? '0') || '0',
        data: c.data && c.data !== '0x' ? fromHex(stripHexPrefix(c.data)) : new Uint8Array(0),
      }))
    : [innerEstCall];
  const estCallData = buildNativeCallData(estCalls, false);

  // Build this before the estimate RPC's fallback handling: account context is
  // mandatory. A static fee must never mask a missing nonce or initCode.
  const estimateAccount = deployed
    ? { nonce, initCode: new Uint8Array(0) }
    : publicKeyHex
      ? { nonce: '0x0', initCode: buildInitCode(publicKeyHex) }
      : null;
  if (!estimateAccount) {
    throw new Error('Could not load the passkey public key required to build the UserOperation initCode');
  }

  // Try to get accurate gas estimates from the bundler. This catches high-gas chains
  // (e.g. Monad) where actual gas usage is 3-10x higher than the static defaults below.
  let totalGas: bigint | null = null;
  try {
    // The preview operation must have the same account context as the one we
    // eventually sign; its signature is intentionally a dummy value.
    // A Safe that does not exist yet has nonce 0 and its real initCode. A
    // deployed Safe carries its current EntryPoint nonce.
    const verificationGas = deployed ? VERIFICATION_GAS_DEPLOYED : VERIFICATION_GAS_UNDEPLOYED;
    const dummySig = buildDummySignature();
    const dummyOp: UserOperation = {
      sender: from,
      nonce: estimateAccount.nonce,
      initCode: estimateAccount.initCode,
      callData: estCallData,
      verificationGasLimit: verificationGas,
      callGasLimit: CALL_GAS_LIMIT,
      preVerificationGas: PRE_VERIFICATION_GAS,
      // In-band settlement has no EntryPoint native prefund. Keep both values
      // at zero in the simulated draft as requested.
      maxFeePerGas: 0n,
      maxPriorityFeePerGas: 0n,
      paymasterAndData: new Uint8Array(0),
      signature: dummySig,
    };
    const est = await estimateGas(dummyOp, chainId);
    const estVgl = deployed
      ? bigintMax((est.verificationGasLimit * 15n) / 10n, VERIFICATION_GAS_DEPLOYED)
      : bigintMax((est.verificationGasLimit * 15n) / 10n, 2_000_000n);
    const estCgl = bigintMax((est.callGasLimit * 15n) / 10n, 100_000n);
    const estPvg = est.preVerificationGas + 10_000n;
    totalGas = estVgl + estCgl + estPvg;
    console.log(`[FeeEstimate] Bundler gas: vgl=${estVgl} cgl=${estCgl} pvg=${estPvg} total=${totalGas}`);
  } catch (err) {
    // For a large/complex op the static fallback would show a misleading number and
    // the submit would be refused anyway (see sendUserOp). Surface the failure so the
    // confirm UI shows "couldn't estimate" + retry instead of a wrong fee. Small ops
    // keep the static fallback — it's accurate enough for transfers.
    if (estCallData.length > ESTIMATION_REQUIRED_CALLDATA) {
      throw err instanceof Error ? err : new Error('Gas estimation failed');
    }
    console.log(`[FeeEstimate] Bundler estimation unavailable, using defaults:`, err instanceof Error ? err.message : String(err));
  }

  // Fallback to static gas constants if bundler estimation failed
  if (totalGas === null) {
    const verificationGas = deployed
      ? VERIFICATION_GAS_DEPLOYED
      : VERIFICATION_GAS_UNDEPLOYED;
    totalGas = verificationGas + CALL_GAS_LIMIT + PRE_VERIFICATION_GAS;

    // L2 rollup data fee adjustments
    const ARBITRUM_CHAIN_IDS = [42161, 421614];
    const OP_STACK_CHAIN_IDS = [10, 8453, 11155420, 84532];
    if (ARBITRUM_CHAIN_IDS.includes(chainId)) {
      totalGas += 600_000n;
    } else if (OP_STACK_CHAIN_IDS.includes(chainId)) {
      totalGas += 150_000n;
    }
  }

  // In-band chains: the signed op pays maxFeePerGas = 0. The wallet derives reimbursement from
  // the displayed gas basis (totalGas × network gas price × 3), applying the native/stable floors;
  // the address-only quote supplies only the selectable assets, balances, recipient and USD prices.
  if (await isInBandChain(chainId, from)) {
    const quotes = await fetchInBandGasQuotes(chainId, from);
    const inBandQuote = quotes ? findInBandGasQuote(quotes, gasFeeToken) : null;
    const nativeQuote = quotes ? findInBandGasQuote(quotes) : null;
    const feeAmount = inBandQuote && nativeQuote
      ? calculateInBandFeeAmount(totalGas, networkFeePerGas, inBandQuote, nativeQuote)
      : null;
    if (inBandQuote && feeAmount !== null) {
      // feeRecipient rides along so the submit path signs EXACTLY this quote (displayed = signed
      // — never a silent mismatch).
      const common = {
        networkFeePerGas, relayerFeePerGas, bundlerGasPrice, totalGas, deployed, tier, quoted,
        inBand: true, feeRecipient: inBandQuote.recipient,
      };
      if (inBandQuote.asset === 'erc20' && inBandQuote.feeToken) {
        return {
          ...common,
          totalWei: 0n, // native display not applicable — the fee rides in feeAsset
          maxFeePerGas: 0n,
          feeAsset: {
            kind: 'erc20',
            token: inBandQuote.feeToken,
            decimals: inBandQuote.decimals,
            amount: feeAmount,
          },
        };
      }
      if (inBandQuote.asset === 'native') {
        return {
          ...common,
          totalWei: feeAmount,
          maxFeePerGas: 0n,
          feeAsset: { kind: 'native' },
        };
      }
    }
  }

  const totalWei = totalGas * userOpMaxFee;

  return {
    totalWei,
    maxFeePerGas: userOpMaxFee,
    networkFeePerGas,
    relayerFeePerGas,
    bundlerGasPrice,
    totalGas,
    deployed,
    tier,
    quoted,
  };
}

// formatWeiToEth was duplicated byte-for-byte across 4+ files; it now lives in
// ./format-eth and is re-exported here to keep the public API (and its tests) stable.
export { formatWeiToEth } from './format-eth';

// ---------------------------------------------------------------------------
// Core UserOp Flow
// ---------------------------------------------------------------------------

async function sendUserOp(
  safeAddress: string,
  callData: Uint8Array,
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
  maxFeeOverride?: bigint,
): Promise<SubmitResult> {
  // 0. Pre-check: verify critical contracts exist on this chain (cached after first success)
  await verifyChainReady(chainId);

  // 1-4. Fetch deployment status, nonce, and gas prices in parallel
  // Clear gas price cache to get fresh values — stale prices cause
  // "gas price too low" rejections on chains with volatile gas (e.g. Gnosis).
  _gasPriceCache.delete(chainId);
  const [deployed, nonceResult, gasPrices] = await Promise.all([
    isDeployed(safeAddress, chainId),
    getNonce(safeAddress, chainId).catch(() => null),
    getGasPrices(chainId),
  ]);

  // Build initCode if needed
  const initCode: Uint8Array = deployed
    ? new Uint8Array(0)
    : buildInitCode(publicKeyHex);

  // Use fetched nonce for deployed wallets, 0 for undeployed. A failed nonce fetch
  // is only tolerable for an undeployed wallet (its nonce IS 0); for a deployed one,
  // submitting 0x0 would burn a passkey prompt on an op the bundler must reject
  // (AA25 invalid nonce) — fail fast with a retryable error before signing instead.
  if (deployed && nonceResult === null) {
    throw new Error('Could not fetch the account nonce — the network may be unstable. Please try again.');
  }
  const nonce: string = deployed ? (nonceResult as string) : '0x0';

  // Price the op. Priority order:
  //  1. Caller-supplied override (the confirm screen's displayed bundler quote).
  //     Guard: maxFeeOverride is typed bigint, but the type is erased at runtime — a
  //     mis-wired caller (e.g. onPress={approveRequest} passing a gesture event) could
  //     hand us a non-bigint, which would serialize to "0x[object Object]" and blow up
  //     bundler estimation and the SafeOp hash. And a zero/negative override (a
  //     degenerate upstream quote leaking through the confirm screen) would sign an
  //     op the bundler MUST reject ("maxFeePerGas must be > 0") — so validate, don't
  //     just type-check, and re-derive the price instead of trusting it.
  //  2. The bundler's OWN quote (tip-inclusive, the same source that accepts/rejects).
  //     This covers override-less callers — notably dApp wallet_sendCalls — so they
  //     never under-price on chains where the wallet's per-chain RPC drops the tip.
  //  3. Local estimate off getGasPrices(), only if the bundler can't quote.
  let maxFee: bigint;
  if (isUsableFeeOverride(maxFeeOverride)) {
    maxFee = maxFeeOverride;
  } else {
    const quote = await getBundlerGasQuote(chainId).catch(() => null);
    maxFee = quote?.maxFeePerGas ?? calcMaxFeePerGas(gasPrices.gasPrice);
  }
  const maxPriority = maxFee;

  // 5. Initial gas estimates
  const verificationGas = deployed
    ? VERIFICATION_GAS_DEPLOYED
    : VERIFICATION_GAS_UNDEPLOYED;

  // 6. Build dummy UserOp for gas estimation
  const dummySig = buildDummySignature();
  const userOp: UserOperation = {
    sender: safeAddress,
    nonce,
    initCode,
    callData,
    verificationGasLimit: verificationGas,
    callGasLimit: CALL_GAS_LIMIT,
    preVerificationGas: PRE_VERIFICATION_GAS,
    maxFeePerGas: maxFee,
    maxPriorityFeePerGas: maxPriority,
    paymasterAndData: new Uint8Array(0),
    signature: dummySig,
  };

  // 7. Estimate gas via bundler
  // Skip estimation only for deployed wallets with small calldata (simple transfers).
  // DApp transactions have large/complex calldata that needs accurate gas estimation,
  // especially for preVerificationGas which scales with calldata size.
  const needsEstimation = !deployed || callData.length > 200;
  if (needsEstimation) {
    try {
      const estimated = await estimateGas(userOp, chainId);
      console.log('[UserOp] Gas estimate:', {
        verificationGasLimit: estimated.verificationGasLimit.toString(),
        callGasLimit: estimated.callGasLimit.toString(),
        preVerificationGas: estimated.preVerificationGas.toString(),
      });
      const estVerification = (estimated.verificationGasLimit * 15n) / 10n;
      // 2M floor is only needed for undeployed wallets (account creation).
      // For deployed wallets, use the estimated value — inflating to 2M
      // causes the bundler's balance check to require ~4x more gas reserve.
      userOp.verificationGasLimit = deployed
        ? bigintMax(estVerification, VERIFICATION_GAS_DEPLOYED)
        : bigintMax(estVerification, 2_000_000n);
      userOp.callGasLimit = bigintMax(
        (estimated.callGasLimit * 15n) / 10n,
        100_000n,
      );
      userOp.preVerificationGas = estimated.preVerificationGas + 10_000n;
    } catch (err) {
      console.error('[UserOp] Gas estimation failed:', err instanceof Error ? err.message : String(err));
      // For a large/complex op the static defaults below can't cover the real
      // callGasLimit/preVerificationGas — submitting anyway yields an op the bundler
      // accepts but can't land, i.e. a silent 2-minute receipt timeout. Refuse so the
      // user gets an immediate, retryable error. Small ops keep the known-good defaults.
      if (callData.length > ESTIMATION_REQUIRED_CALLDATA) {
        throw new Error('Could not estimate gas for this transaction. The network may be busy — please try again.');
      }
      console.error('[UserOp] Falling back to default gas limits (small calldata).');
    }
  }
  console.log('[UserOp] Final gas:', {
    verificationGasLimit: userOp.verificationGasLimit.toString(),
    callGasLimit: userOp.callGasLimit.toString(),
    preVerificationGas: userOp.preVerificationGas.toString(),
    maxFeePerGas: userOp.maxFeePerGas.toString(),
  });

  // 8. Calculate SafeOp hash (EIP-712)
  const safeOpHash = calculateSafeOpHash(userOp, chainId);

  // 9. Sign with passkey
  const assertion = await signFn(safeOpHash);

  // 10. Build real signature
  const rawSig = derSignatureToRaw(assertion.signature);
  if (!rawSig) {
    throw new Error('Failed to create signature: DER to raw conversion failed');
  }

  const clientDataFields = extractClientDataFields(assertion.clientDataJSON);

  const sigR = rawSig.slice(0, 32);
  const sigS = rawSig.slice(32);

  const realSig = buildUserOpSignature(
    assertion.authenticatorData,
    clientDataFields,
    sigR,
    sigS,
  );
  userOp.signature = realSig;

  // 11. Submit to bundler
  let userOpHash: string;
  try {
    userOpHash = await submitUserOp(userOp, chainId);
  } catch (err) {
    // If bundler says a previous UserOp is already pending (replacement or duplicate),
    // extract the existing hash and poll for its receipt instead of failing.
    const errMsg = err instanceof Error ? err.message : String(err);
    const existingHash = parseExistingUserOpHash(errMsg);
    if (existingHash) {
      console.log(`[UserOp] Previous op pending (${existingHash}), polling for receipt...`);
      return {
        userOpHash: existingHash,
        waitForTxHash: () => waitForReceipt(existingHash, chainId, 60_000),
      };
    }
    throw err;
  }

  // 12. Optimistically increment nonce so concurrent sends don't collide
  incrementNonceCache(safeAddress, chainId);

  // Return immediately — caller can await txHash separately
  return {
    userOpHash,
    waitForTxHash: () => waitForReceipt(userOpHash, chainId),
  };
}

// ---------------------------------------------------------------------------
// Tempo UserOp Flow (no native coin — gas paid in a stablecoin)
// ---------------------------------------------------------------------------

/**
 * True if a sub-call is a plain value/token transfer — a native send (no data) or a standard
 * ERC-20 `transfer(address,uint256)` (4-byte selector 0xa9059cbb + two 32-byte args = 68 bytes).
 *
 * Used to decide whether an un-estimatable Tempo batch is safe to submit on the flat per-sub-call
 * gas floor (transfers meter predictably) or must be refused (a contract call whose real gas the
 * floor can't bound). Keys off the call SHAPE, not calldata SIZE: a heavy call can have tiny
 * calldata (`claim()` = 4 bytes, `deposit(uint256)` = 36 bytes), which a length threshold would
 * wrongly wave through onto the too-low floor.
 */
export function isPlainTransferCall(c: { data: Uint8Array }): boolean {
  if (c.data.length === 0) return true; // native value transfer
  return (
    c.data.length === 68 &&
    c.data[0] === 0xa9 && c.data[1] === 0x05 && c.data[2] === 0x9c && c.data[3] === 0xbb
  );
}

/**
 * Send a UserOperation on Tempo. Tempo has no native coin, so:
 *   - the UserOp is signed with maxFeePerGas = maxPriorityFeePerGas = 0 (EntryPoint's
 *     native prefund/refund becomes a no-op — avoids AA21), and
 *   - a `feeToken.transfer(bundlerEOA, reimbursement)` call is batched in so the
 *     bundler — which submits handleOps inside a 0x76 paying gas in `feeToken` — is
 *     repaid in-band. The bundler verifies the reimbursement covers its cost.
 *
 * Same Safe + passkey + EntryPoint + cross-chain address as every other chain; only
 * gas settlement differs. See services/tempo.ts.
 */
async function sendUserOpTempo(
  safeAddress: string,
  innerCalls: MultiSendCall[],
  feeToken: string,
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
): Promise<SubmitResult> {
  await verifyChainReady(chainId);

  // The reimbursement recipient: the bundler's per-Safe EOA (which pays the outer
  // 0x76 gas), or the bundler's treasury when its vault mode is on — the bundler
  // tells us which via settlementRecipient (absent on old bundlers → EOA fallback).
  // MUST come from the same bundler that submits the op (see fetchBundlerAccountInfo).
  const info = await fetchBundlerAccountInfo(chainId, safeAddress);
  const feeCollector = info?.settlementRecipient ?? info?.depositAddress;
  if (!feeCollector || !/^0x[0-9a-fA-F]{40}$/.test(feeCollector)) {
    throw new Error('The Tempo gas relayer is unavailable right now. Please try again.');
  }

  _gasPriceCache.delete(chainId);
  const [deployed, nonceResult, gasPrices] = await Promise.all([
    isDeployed(safeAddress, chainId),
    getNonce(safeAddress, chainId).catch(() => null),
    getGasPrices(chainId),
  ]);

  // A deployed wallet MUST sign its real nonce — a 0x0 fallback is rejected as AA25. Fail fast.
  if (deployed && nonceResult === null) {
    throw new Error('Could not fetch the account nonce — the network may be unstable. Please try again.');
  }
  const initCode: Uint8Array = deployed ? new Uint8Array(0) : buildInitCode(publicKeyHex);
  const nonce: string = deployed ? (nonceResult as string) : '0x0';

  // Static realistic-gas model — a floor/fallback for pricing the reimbursement.
  // The one reimbursement transfer goes directly to the active bundler recipient.
  const staticGas = tempoExpectedGas(deployed, innerCalls.length + 1);

  // Build the batch with a placeholder reimbursement (the transfer value does
  // not affect gas), estimate it, then bake in the real amount.
  const buildBatch = (reimbursement: bigint): Uint8Array =>
    buildMultiSendExecuteCallData([
      ...innerCalls,
      { to: feeToken, value: '0', data: encodeErc20Transfer(feeCollector, reimbursement) },
    ]);

  // Floor callGasLimit per sub-call: TIP-20 transfers meter high and the bundler's estimate
  // under-reports (handleOps swallows the inner OOG), so a per-sub-call floor lets the atomic
  // batch run out of gas and revert. Includes the reimbursement transfer. See services/tempo.ts.
  const callGasFloor = tempoCallGasLimit(innerCalls.length + 1);

  const userOp: UserOperation = {
    sender: safeAddress,
    nonce,
    initCode,
    callData: buildBatch(1n),
    verificationGasLimit: deployed ? VERIFICATION_GAS_DEPLOYED : TEMPO_VERIFICATION_GAS_UNDEPLOYED,
    callGasLimit: callGasFloor,
    preVerificationGas: PRE_VERIFICATION_GAS,
    maxFeePerGas: 0n, // Tempo: zero native gas accounting (avoids AA21)
    maxPriorityFeePerGas: 0n,
    paymasterAndData: new Uint8Array(0),
    signature: buildDummySignature(),
  };

  // The bundler's own un-padded estimate (verification + call + preVerification) tracks the
  // gas the bundler will actually simulate (`simGas`) far more tightly than the static model.
  // Capturing it is what stops the reimbursement drifting below the bundler's cost — for both
  // a Safe DEPLOY (whose real gas the flat model under-counted → the reported rejection) and a
  // heavy dApp contract call (whose real gas dwarfs the flat per-sub-call estimate).
  // A batch carrying a real contract call (not just transfers) can burn far more than the
  // per-sub-call floor. `tempoCallGasLimit`'s flat 380k/sub-call is sized for TIP-20 transfers;
  // if estimation fails for such a call we must NOT submit a doomed op that OOGs on-chain
  // (wasting a passkey prompt and the bundler's gas). Pure-transfer batches keep the floor
  // fallback — the floor covers them. Classify by call SHAPE (isPlainTransferCall), so a heavy
  // call with tiny calldata (claim()/deposit()) is still caught. Mirrors native's large-calldata guard.
  const hasContractCall = innerCalls.some((c) => !isPlainTransferCall(c));
  let estActualGas: bigint | null = null;
  try {
    const est = await estimateGas(userOp, chainId);
    estActualGas = est.verificationGasLimit + est.callGasLimit + est.preVerificationGas;
    userOp.verificationGasLimit = deployed
      ? bigintMax((est.verificationGasLimit * 15n) / 10n, VERIFICATION_GAS_DEPLOYED)
      : bigintMax((est.verificationGasLimit * 15n) / 10n, TEMPO_VERIFICATION_GAS_UNDEPLOYED);
    userOp.callGasLimit = bigintMax((est.callGasLimit * 15n) / 10n, callGasFloor);
    userOp.preVerificationGas = est.preVerificationGas + 10_000n;
  } catch (err) {
    console.error('[Tempo] Gas estimation failed, using defaults:', err instanceof Error ? err.message : String(err));
    if (hasContractCall) {
      throw new Error('Could not estimate gas for this transaction. The network may be busy — please try again.');
    }
  }

  // Price the reimbursement off the realistic gas the 0x76 will burn — the bundler's estimate
  // when we have it, else the static model, whichever is higher (never
  // under-price the bundler). This is still NOT the padded UserOp limits
  // (callGasLimit/verificationGasLimit stay high for OOG safety, but the user shouldn't pay 2–4×
  // for that headroom). `bigintMax` guards the case where estimateGas throws (estActualGas null).
  const realisticGas = estActualGas !== null ? bigintMax(staticGas, estActualGas) : staticGas;
  const reimbursement = tempoReimbursement(realisticGas, gasPrices.gasPrice, TEMPO_FEE_TOKEN_DECIMALS);
  userOp.callData = buildBatch(reimbursement);
  console.log(`[Tempo] feeToken=${feeToken} reimbursement=${reimbursement} realisticGas=${realisticGas} staticGas=${staticGas} est=${estActualGas ?? 'n/a'} collector=${feeCollector}`);

  // Sign the SafeOp (over the FINAL callData) and submit, telling the bundler which
  // stablecoin to charge for the outer 0x76.
  const safeOpHash = calculateSafeOpHash(userOp, chainId);
  const assertion = await signFn(safeOpHash);
  const rawSig = derSignatureToRaw(assertion.signature);
  if (!rawSig) {
    throw new Error('Failed to create signature: DER to raw conversion failed');
  }
  const clientDataFields = extractClientDataFields(assertion.clientDataJSON);
  userOp.signature = buildUserOpSignature(
    assertion.authenticatorData,
    clientDataFields,
    rawSig.slice(0, 32),
    rawSig.slice(32),
  );

  let userOpHash: string;
  try {
    userOpHash = await submitUserOp(userOp, chainId, { feeToken });
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err);
    const existingHash = parseExistingUserOpHash(errMsg);
    if (existingHash) {
      return {
        userOpHash: existingHash,
        waitForTxHash: () => waitForReceipt(existingHash, chainId, 60_000),
      };
    }
    throw err;
  }

  incrementNonceCache(safeAddress, chainId);
  return {
    userOpHash,
    waitForTxHash: () => waitForReceipt(userOpHash, chainId),
  };
}

// ---------------------------------------------------------------------------
// In-band UserOp Flow (generic chains — gas settled by a batched transfer)
// ---------------------------------------------------------------------------

/**
 * The single fee leg batched into an in-band UserOp: a plain native-value transfer to the
 * bundler's recipient, or a whitelisted-stablecoin `transfer`. Pure + exported for tests.
 * The shape must stay a plain CALL to the exact recipient — that is what the bundler's
 * reimbursement parser counts (vela-bundler docs/inband-gas-settlement.md anti-drain guards).
 * `value` is hex (MultiSendCall's format — abiEncodeUint256Hex parses it as hex).
 */
export function buildInBandFeeLeg(
  gasFeeToken: string | null,
  recipient: string,
  amount: bigint,
): MultiSendCall {
  if (gasFeeToken) {
    return { to: gasFeeToken, value: '0', data: encodeErc20Transfer(recipient, amount) };
  }
  return { to: recipient, value: '0x' + amount.toString(16), data: new Uint8Array(0) };
}

/**
 * The CHARGE BASIS for an in-band fee quote: the bundler's UN-PADDED gas estimate for
 * the actual batch (user calls + a placeholder self-transfer fee leg). The padded
 * display model (verification/call constants + L2 adders) over-prices several-fold —
 * measured 8× on Arbitrum, because the +600k adder double-counts the L1 data fee that
 * eth_estimateUserOperationGas already folds into preVerificationGas. Deployed
 * accounts only (an undeployed draft would need initCode). Missing nonce or a
 * failed bundler simulation is an error; callers must not price a malformed op.
 */
export async function estimateInBandBasisGas(
  safeAddress: string,
  innerCalls: MultiSendCall[],
  gasFeeToken: string | null,
  chainId: number,
): Promise<bigint> {
  // A nonce failure is not permission to simulate as nonce 0.
  const nonce = await getNonce(safeAddress, chainId);
  const userOp: UserOperation = {
    sender: safeAddress,
    nonce,
    initCode: new Uint8Array(0),
    callData: buildMultiSendExecuteCallData([
      ...innerCalls,
      buildInBandFeeLeg(gasFeeToken, safeAddress, 1n), // self-transfer placeholder — never reverts
    ]),
    verificationGasLimit: VERIFICATION_GAS_DEPLOYED,
    callGasLimit: CALL_GAS_LIMIT,
    preVerificationGas: PRE_VERIFICATION_GAS,
    // Match the in-band UserOperation: native EntryPoint fee accounting is
    // zero and the reimbursement leg covers the relayer cost.
    maxFeePerGas: 0n,
    maxPriorityFeePerGas: 0n,
    paymasterAndData: new Uint8Array(0),
    signature: buildDummySignature(),
  };
  const est = await estimateGas(userOp, chainId);
  return est.verificationGasLimit + est.callGasLimit + est.preVerificationGas;
}

/**
 * Send a UserOperation on a generic in-band chain (vela-bundler
 * docs/inband-gas-settlement.md — the Tempo model generalized to all EVM chains):
 *   - the UserOp is signed with maxFeePerGas = maxPriorityFeePerGas = 0 (EntryPoint's
 *     native prefund/refund becomes a no-op), and
 *   - an in-band transfer to the bundler's recipient is batched in — native value, or a
 *     whitelisted-stablecoin `transfer` when the user chose `gasFeeToken`. The exact
 *     amount comes from the bundler's own quote (vela_getInBandGasQuote, 3× its cost);
 *     the bundler re-verifies reimbursed ≥ required at submit.
 *
 * Same Safe + passkey + EntryPoint as every other chain; only gas settlement differs.
 * Unlike Tempo there is no feeToken envelope extension and no splitter deploy — the
 * outer tx is a plain native EIP-1559 raw tx and the recipient does its own split.
 */
async function sendUserOpInBand(
  safeAddress: string,
  innerCalls: MultiSendCall[],
  gasFeeToken: string | null, // null = pay gas in native; else whitelisted stablecoin address
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
  /** The fee the confirm UI displayed (amount in the fee asset's own units +
   *  recipient). Signed VERBATIM — what the user saw is what executes. */
  quotedFee?: QuotedInBandFee,
): Promise<SubmitResult> {
  await verifyChainReady(chainId);

  _gasPriceCache.delete(chainId);
  const [deployed, nonceResult] = await Promise.all([
    isDeployed(safeAddress, chainId),
    getNonce(safeAddress, chainId).catch(() => null),
  ]);

  // A deployed wallet MUST sign its real nonce — falling back to 0x0 burns a passkey prompt on
  // an op the bundler rejects (AA25). Fail fast instead. Undeployed → nonce IS 0x0.
  if (deployed && nonceResult === null) {
    throw new Error('Could not fetch the account nonce — the network may be unstable. Please try again.');
  }
  const initCode: Uint8Array = deployed ? new Uint8Array(0) : buildInitCode(publicKeyHex);
  const nonce: string = deployed ? (nonceResult as string) : '0x0';

  // The batch: the user's calls + one fee leg (native value or stablecoin transfer).
  // No additional deployment call is prepended.
  const buildBatch = (amount: bigint, recipient: string): Uint8Array =>
    buildMultiSendExecuteCallData([...innerCalls, buildInBandFeeLeg(gasFeeToken, recipient, amount)]);

  // Build the UserOp with a PLACEHOLDER fee leg (the transfer VALUE doesn't affect gas;
  // the SAFE ITSELF stands in for the recipient — NOT address(0): standard ERC-20s
  // revert `transfer(0x0, …)`, which would poison the estimation run for every
  // stablecoin-fee send. A self-transfer always succeeds and has identical calldata
  // shape/length to the real leg), estimate, then bake the real quote.
  const verificationGas = deployed ? VERIFICATION_GAS_DEPLOYED : VERIFICATION_GAS_UNDEPLOYED;
  const userOp: UserOperation = {
    sender: safeAddress,
    nonce,
    initCode,
    callData: buildBatch(1n, safeAddress),
    verificationGasLimit: verificationGas,
    callGasLimit: CALL_GAS_LIMIT,
    preVerificationGas: PRE_VERIFICATION_GAS,
    maxFeePerGas: 0n, // in-band: zero native gas accounting (prefund no-op)
    maxPriorityFeePerGas: 0n,
    paymasterAndData: new Uint8Array(0),
    signature: buildDummySignature(),
  };

  // A batch carrying a real contract call (not just transfers) can burn far more than the
  // static defaults; if estimation fails for such a call we must NOT submit a doomed op
  // that OOGs on-chain. Pure-transfer batches keep the defaults — they cover them.
  // Classify by call SHAPE (isPlainTransferCall), same as the Tempo path.
  const hasContractCall = innerCalls.some((c) => !isPlainTransferCall(c));
  try {
    const est = await estimateGas(userOp, chainId);
    userOp.verificationGasLimit = deployed
      ? bigintMax((est.verificationGasLimit * 15n) / 10n, VERIFICATION_GAS_DEPLOYED)
      : bigintMax((est.verificationGasLimit * 15n) / 10n, VERIFICATION_GAS_UNDEPLOYED);
    userOp.callGasLimit = bigintMax((est.callGasLimit * 15n) / 10n, CALL_GAS_LIMIT);
    userOp.preVerificationGas = est.preVerificationGas + 10_000n;
  } catch (err) {
    console.error('[InBand] Gas estimation failed, using defaults:', err instanceof Error ? err.message : String(err));
    if (hasContractCall) {
      throw new Error('Could not estimate gas for this transaction. The network may be busy — please try again.');
    }
  }

  // The fee the user pays: EXACTLY what the confirm UI displayed when it is threaded
  // through (签什么执行什么 — the signature covers the displayed amount + recipient, and
  // the bundler's own 2×-real-cost gate rejects a stale quote LOUDLY so the UI shows a
  // NEW number; there is never a silent display/charge mismatch). Only programmatic
  // callers without a confirm UI fall back to a fresh send-time quote.
  let feeAmount: bigint;
  let feeRecipient: string;
  if (quotedFee && quotedFee.amount > 0n && /^0x[0-9a-fA-F]{40}$/.test(quotedFee.recipient)) {
    feeAmount = quotedFee.amount;
    feeRecipient = quotedFee.recipient;
    console.log(`[InBand] signing DISPLAYED quote: feeToken=${gasFeeToken ?? 'native'} amount=${feeAmount} recipient=${feeRecipient}`);
  } else {
    const quotes = await fetchInBandGasQuotes(chainId, safeAddress);
    const quote = quotes ? findInBandGasQuote(quotes, gasFeeToken) : null;
    const nativeQuote = quotes ? findInBandGasQuote(quotes) : null;
    if (!quote || !nativeQuote) {
      if (gasFeeToken) {
        throw new Error('The gas relayer cannot accept the selected fee token right now. Please pick a different gas asset.');
      }
      throw new Error('The gas relayer is unavailable right now. Please try again.');
    }
    const outerQuote = await getBundlerGasQuote(chainId, 'fast').catch(() => null);
    const gasPrice = outerQuote?.networkFeePerGas ?? (await getGasPrices(chainId)).gasPrice;
    const totalGas = userOp.verificationGasLimit + userOp.callGasLimit + userOp.preVerificationGas;
    const amount = calculateInBandFeeAmount(totalGas, gasPrice, quote, nativeQuote);
    if (amount === null) {
      throw new Error('Could not calculate the selected gas fee. Please try again.');
    }
    feeAmount = amount;
    feeRecipient = quote.recipient;
    console.log(`[InBand] feeToken=${gasFeeToken ?? 'native'} amount=${feeAmount} recipient=${feeRecipient} gas=${totalGas} gasPrice=${gasPrice}`);
  }

  userOp.callData = buildBatch(feeAmount, feeRecipient);

  // Sign the SafeOp (over the FINAL callData) and submit. No feeToken extension — that
  // is Tempo-envelope-specific; the generic outer tx is a native raw tx.
  const safeOpHash = calculateSafeOpHash(userOp, chainId);
  const assertion = await signFn(safeOpHash);
  const rawSig = derSignatureToRaw(assertion.signature);
  if (!rawSig) {
    throw new Error('Failed to create signature: DER to raw conversion failed');
  }
  const clientDataFields = extractClientDataFields(assertion.clientDataJSON);
  userOp.signature = buildUserOpSignature(
    assertion.authenticatorData,
    clientDataFields,
    rawSig.slice(0, 32),
    rawSig.slice(32),
  );

  let userOpHash: string;
  try {
    userOpHash = await submitUserOp(userOp, chainId);
  } catch (err) {
    const errMsg = err instanceof Error ? err.message : String(err);
    const existingHash = parseExistingUserOpHash(errMsg);
    if (existingHash) {
      return {
        userOpHash: existingHash,
        waitForTxHash: () => waitForReceipt(existingHash, chainId, 60_000),
      };
    }
    throw err;
  }

  incrementNonceCache(safeAddress, chainId);
  return {
    userOpHash,
    waitForTxHash: () => waitForReceipt(userOpHash, chainId),
  };
}

// ---------------------------------------------------------------------------
// CallData
// ---------------------------------------------------------------------------

/** Encode Safe.executeUserOp(address to, uint256 value, bytes data, uint8 operation) */
export function buildExecuteCallData(
  to: string,
  value: string,
  data: Uint8Array,
): Uint8Array {
  const selector = functionSelector(
    'executeUserOp(address,uint256,bytes,uint8)',
  );
  const toEncoded = abiEncodeAddress(to);
  const valueEncoded = abiEncodeUint256Hex(value);
  const dataOffset = abiEncodeUint256(128n); // 4 * 32 bytes
  const operation = abiEncodeUint256(0n); // CALL
  const dataLen = abiEncodeUint256(BigInt(data.length));
  const paddingLen = (32 - (data.length % 32)) % 32;
  const dataPadding = new Uint8Array(paddingLen);

  return concatBytes(
    selector,
    toEncoded,
    valueEncoded,
    dataOffset,
    operation,
    dataLen,
    data,
    dataPadding,
  );
}

// ---------------------------------------------------------------------------
// MultiSend batching (shared by sendBatchCalls and the Tempo path)
// ---------------------------------------------------------------------------

/** A single call for MultiSend batching. `value` is a hex string (0x optional). */
export interface MultiSendCall {
  to: string;
  value: string;
  data: Uint8Array;
}

/**
 * Encode Safe.executeUserOp(MultiSend, 0, multiSend(packedCalls), DELEGATECALL),
 * batching N sub-calls atomically. Each sub-call is a CALL (operation 0).
 */
export function buildMultiSendExecuteCallData(calls: MultiSendCall[]): Uint8Array {
  const encodedTxs = calls.map(c => {
    const valueClean = stripHexPrefix(c.value) || '0';
    const toBytes = fromHex(stripHexPrefix(c.to));
    const operationByte = new Uint8Array([0]); // CALL
    const value = abiEncodeUint256Hex(valueClean);
    const lenBytes = abiEncodeUint256(c.data.length);
    return concatBytes(operationByte, toBytes, value, lenBytes, c.data);
  });
  const packed = concatBytes(...encodedTxs);

  const multiSendSelector = functionSelector('multiSend(bytes)');
  const msPadding = (32 - (packed.length % 32)) % 32;
  const multiSendPayload = concatBytes(
    multiSendSelector,
    abiEncodeUint256(32),              // offset
    abiEncodeUint256(packed.length),   // length
    packed,
    new Uint8Array(msPadding),         // padding
  );

  const selector = functionSelector('executeUserOp(address,uint256,bytes,uint8)');
  const dataPadding = (32 - (multiSendPayload.length % 32)) % 32;
  return concatBytes(
    selector,
    abiEncodeAddress(MULTI_SEND),
    abiEncodeUint256(0n),
    abiEncodeUint256(128n),  // data offset (4 * 32)
    abiEncodeUint256(1n),    // DELEGATECALL
    abiEncodeUint256(BigInt(multiSendPayload.length)),
    multiSendPayload,
    new Uint8Array(dataPadding),
  );
}

/**
 * Build the UserOp callData for a native-chain send. A lone call stays a single
 * executeUserOp (unless `alwaysMultiSend`); a batch stays a MultiSend. This is
 * intentionally local-only and makes no bundler REST calls.
 */
function buildNativeCallData(
  innerCalls: MultiSendCall[],
  alwaysMultiSend: boolean,
): Uint8Array {
  if (innerCalls.length === 1 && !alwaysMultiSend) {
    const only = innerCalls[0]!;
    return buildExecuteCallData(only.to, only.value, only.data);
  }
  return buildMultiSendExecuteCallData(innerCalls);
}

/** Encode ERC-20 transfer(address,uint256) calldata. */
export function encodeErc20Transfer(to: string, amount: bigint): Uint8Array {
  return concatBytes(
    functionSelector('transfer(address,uint256)'),
    abiEncodeAddress(to),
    abiEncodeUint256Hex(amount.toString(16)),
  );
}

// ---------------------------------------------------------------------------
// InitCode
// ---------------------------------------------------------------------------

export function buildInitCode(publicKeyHex: string): Uint8Array {
  const { x, y } = parsePublicKey(publicKeyHex);
  const setupData = encodeSetupData(x, y);
  const saltNonce = calculateSaltNonce(x, y);

  // createProxyWithNonce(address singleton, bytes initializer, uint256 saltNonce)
  const selector = functionSelector(
    'createProxyWithNonce(address,bytes,uint256)',
  );
  const singletonEncoded = abiEncodeAddress(SAFE_SINGLETON);
  const dataOffset = abiEncodeUint256(96n); // 3 * 32
  const saltEncoded = abiEncodeBytes32(saltNonce);
  const dataLen = abiEncodeUint256(BigInt(setupData.length));
  const paddingLen = (32 - (setupData.length % 32)) % 32;
  const dataPadding = new Uint8Array(paddingLen);

  const createData = concatBytes(
    selector,
    singletonEncoded,
    dataOffset,
    saltEncoded,
    dataLen,
    setupData,
    dataPadding,
  );

  const factoryBytes = fromHex(stripHexPrefix(SAFE_PROXY_FACTORY));
  return concatBytes(factoryBytes, createData);
}

// ---------------------------------------------------------------------------
// SafeOp Hash (EIP-712)
// ---------------------------------------------------------------------------

function calculateSafeOpHash(
  userOp: UserOperation,
  chainId: number,
): Uint8Array {
  const encoder = new TextEncoder();

  const typeHash = keccak256(
    encoder.encode(
      'SafeOp(address safe,uint256 nonce,bytes initCode,bytes callData,uint128 verificationGasLimit,uint128 callGasLimit,uint256 preVerificationGas,uint128 maxPriorityFeePerGas,uint128 maxFeePerGas,bytes paymasterAndData,uint48 validAfter,uint48 validUntil,address entryPoint)',
    ),
  );

  const structHash = keccak256(
    concatBytes(
      typeHash,
      abiEncodeAddress(userOp.sender),
      abiEncodeUint256Hex(userOp.nonce),
      keccak256(userOp.initCode), // hash of dynamic
      keccak256(userOp.callData), // hash of dynamic
      abiEncodeUint256(userOp.verificationGasLimit),
      abiEncodeUint256(userOp.callGasLimit),
      abiEncodeUint256(userOp.preVerificationGas),
      abiEncodeUint256(userOp.maxPriorityFeePerGas),
      abiEncodeUint256(userOp.maxFeePerGas),
      keccak256(userOp.paymasterAndData), // hash of dynamic
      abiEncodeUint256(0n), // validAfter
      abiEncodeUint256(0n), // validUntil
      abiEncodeAddress(ENTRY_POINT),
    ),
  );

  // Domain separator
  const domainTypeHash = keccak256(
    encoder.encode('EIP712Domain(uint256 chainId,address verifyingContract)'),
  );
  const domainSeparator = keccak256(
    concatBytes(
      domainTypeHash,
      abiEncodeUint256(BigInt(chainId)),
      abiEncodeAddress(SAFE_4337_MODULE),
    ),
  );

  // Final hash: keccak256(0x1901 || domainSeparator || structHash)
  return keccak256(
    concatBytes(new Uint8Array([0x19, 0x01]), domainSeparator, structHash),
  );
}

// ---------------------------------------------------------------------------
// Safe Message Hash (EIP-1271)
// ---------------------------------------------------------------------------

const SAFE_MSG_TYPEHASH = keccak256(
  new TextEncoder().encode('SafeMessage(bytes message)'),
);

const EIP712_DOMAIN_TYPEHASH = keccak256(
  new TextEncoder().encode('EIP712Domain(uint256 chainId,address verifyingContract)'),
);

/**
 * Compute the Safe message hash that the passkey must sign for EIP-1271 verification.
 *
 * Safe4337Module.isValidSignature wraps the original hash in a SafeMessage EIP-712
 * structure before passing it to checkSignatures/WebAuthn signer:
 *
 *   messageHash   = keccak256(abi.encode(originalHash))
 *   structHash    = keccak256(abi.encode(SAFE_MSG_TYPEHASH, messageHash))
 *   domainSep     = keccak256(abi.encode(DOMAIN_TYPEHASH, chainId, safeAddress))
 *   safeMessageHash = keccak256(0x1901 || domainSep || structHash)
 */
export function computeSafeMessageHash(
  originalHash: Uint8Array,
  chainId: number,
  safeAddress: string,
): Uint8Array {
  // messageHash = keccak256(abi.encode(bytes32 originalHash))
  const messageHash = keccak256(abiEncodeBytes32(originalHash));

  // structHash = keccak256(abi.encode(SAFE_MSG_TYPEHASH, messageHash))
  const structHash = keccak256(concatBytes(SAFE_MSG_TYPEHASH, messageHash));

  // domainSep = keccak256(abi.encode(DOMAIN_TYPEHASH, chainId, safeAddress))
  const domainSep = keccak256(
    concatBytes(
      EIP712_DOMAIN_TYPEHASH,
      abiEncodeUint256(BigInt(chainId)),
      abiEncodeAddress(safeAddress),
    ),
  );

  // safeMessageHash = keccak256(0x1901 || domainSep || structHash)
  return keccak256(
    concatBytes(new Uint8Array([0x19, 0x01]), domainSep, structHash),
  );
}

// ---------------------------------------------------------------------------
// WebAuthn Signature
// ---------------------------------------------------------------------------

/**
 * Extract clientDataFields from clientDataJSON.
 *
 * clientDataJSON format:
 *   {"type":"webauthn.get","challenge":"<b64url>","origin":"https://...","crossOrigin":false}
 *
 * clientDataFields = everything after challenge's closing `",` up to (but not including) final `}`
 *   e.g.: "origin":"https://getvela.app","crossOrigin":false
 *
 * The contract template already includes `,"` before this, so we must NOT include the leading comma.
 */
export function extractClientDataFields(clientDataJSON: Uint8Array): string {
  const decoder = new TextDecoder();
  const json = decoder.decode(clientDataJSON);

  // Find "challenge":"
  const key = '"challenge":"';
  const keyIndex = json.indexOf(key);
  if (keyIndex === -1) return '';

  // Find the closing quote of the challenge value
  const valueStart = keyIndex + key.length;
  let searchIndex = valueStart;
  while (searchIndex < json.length) {
    if (json[searchIndex] === '"') break;
    searchIndex++;
  }
  if (searchIndex >= json.length) return '';

  // Skip 2 chars: closing `"` and `,` -> start at the next field
  const skipIndex = searchIndex + 2;
  // Take everything up to the final `}`
  const endIndex = json.length - 1; // skip `}`
  if (skipIndex >= endIndex) return '';

  return json.slice(skipIndex, endIndex);
}

/**
 * Build contract signature for SafeWebAuthnSharedSigner (4337 UserOp context).
 *
 * Format: validAfter(6) + validUntil(6) + r(32) + s(32) + v(1) + dataLength(32) + dynamicData
 * Where r = signer address padded, s = 65 (offset), v = 0x00 (contract sig type)
 * dynamicData = abi.encode(bytes authenticatorData, string clientDataFields, uint256 sigR, uint256 sigS)
 */
export function buildUserOpSignature(
  authenticatorData: Uint8Array,
  clientDataFields: string,
  sigR: Uint8Array,
  sigS: Uint8Array,
): Uint8Array {
  // Validity window: validAfter(6) + validUntil(6) = 12 bytes of zeros (4337 SafeModule only)
  const validityPadding = new Uint8Array(12);
  const core = buildContractSignatureCore(authenticatorData, clientDataFields, sigR, sigS);
  return concatBytes(validityPadding, core);
}

/**
 * Build contract signature for EIP-1271 isValidSignature (dApp signing context).
 *
 * Same as buildUserOpSignature but WITHOUT the 12-byte validity padding,
 * since isValidSignature calls checkNSignatures directly without the
 * Safe4337Module validity window prefix.
 *
 * Format: r(32) + s(32) + v(1) + dataLength(32) + dynamicData
 */
export function buildEip1271Signature(
  authenticatorData: Uint8Array,
  clientDataFields: string,
  sigR: Uint8Array,
  sigS: Uint8Array,
): Uint8Array {
  return buildContractSignatureCore(authenticatorData, clientDataFields, sigR, sigS);
}

/** Shared core: r(32) + s(32) + v(1) + dataLength(32) + dynamicData */
function buildContractSignatureCore(
  authenticatorData: Uint8Array,
  clientDataFields: string,
  sigR: Uint8Array,
  sigS: Uint8Array,
): Uint8Array {
  // Contract signature header: r(32) + s(32) + v(1)
  const rField = abiEncodeAddress(WEBAUTHN_SIGNER); // r = signer address
  const sField = abiEncodeUint256(65n); // s = offset to dynamic data (after r+s+v)
  const vField = new Uint8Array([0x00]); // v = 0x00 = contract signature

  // Dynamic data: abi.encode(bytes, string, uint256, uint256)
  const dynamicData = abiEncodeWebAuthnSig(
    authenticatorData,
    clientDataFields,
    sigR,
    sigS,
  );
  const dataLength = abiEncodeUint256(BigInt(dynamicData.length));

  return concatBytes(
    rField,
    sField,
    vField,
    dataLength,
    dynamicData,
  );
}

/**
 * ABI encode: (bytes authenticatorData, string clientDataFields, uint256 r, uint256 s)
 * Matches: encodeAbiParameters([{type:'bytes'},{type:'string'},{type:'uint256'},{type:'uint256'}], ...)
 */
function abiEncodeWebAuthnSig(
  authenticatorData: Uint8Array,
  clientDataFields: string,
  r: Uint8Array,
  s: Uint8Array,
): Uint8Array {
  const encoder = new TextEncoder();
  const clientFieldsBytes = encoder.encode(clientDataFields);

  // Head: 4 slots (offsets for dynamic types, inline for static types)
  // slot 0: offset to authenticatorData (bytes) = 4 * 32 = 128
  // slot 1: offset to clientDataFields (string) = calculated after authData
  // slot 2: r (uint256, inline)
  // slot 3: s (uint256, inline)

  // Tail parts
  // authenticatorData: length(32) + padded data
  const authPadLen = (32 - (authenticatorData.length % 32)) % 32;
  const authTail = concatBytes(
    abiEncodeUint256(BigInt(authenticatorData.length)),
    authenticatorData,
    new Uint8Array(authPadLen),
  );

  // clientDataFields: length(32) + padded data
  const clientPadLen = (32 - (clientFieldsBytes.length % 32)) % 32;
  const clientTail = concatBytes(
    abiEncodeUint256(BigInt(clientFieldsBytes.length)),
    clientFieldsBytes,
    new Uint8Array(clientPadLen),
  );

  const authDataOffset = 128n; // 4 * 32
  const clientDataOffset = authDataOffset + BigInt(authTail.length);

  return concatBytes(
    abiEncodeUint256(authDataOffset),
    abiEncodeUint256(clientDataOffset),
    abiEncodeBytes32(r),
    abiEncodeBytes32(s),
    authTail,
    clientTail,
  );
}

/** Build a dummy signature for gas estimation. */
function buildDummySignature(): Uint8Array {
  const validityPadding = new Uint8Array(12);
  const rField = abiEncodeAddress(WEBAUTHN_SIGNER);
  const sField = abiEncodeUint256(65n);
  const vField = new Uint8Array([0x00]);

  const fakeAuthData = concatBytes(
    new Uint8Array([0x01]),
    new Uint8Array(36), // 37 bytes total, right-padded
  );
  const fakeClientFields =
    '"origin":"https://getvela.app","crossOrigin":false';
  const fakeR = new Uint8Array(32);
  fakeR[31] = 0x01;
  const fakeS = new Uint8Array(32);
  fakeS[31] = 0x01;

  const dynamicData = abiEncodeWebAuthnSig(
    fakeAuthData,
    fakeClientFields,
    fakeR,
    fakeS,
  );
  const dataLength = abiEncodeUint256(BigInt(dynamicData.length));

  return concatBytes(
    validityPadding,
    rField,
    sField,
    vField,
    dataLength,
    dynamicData,
  );
}

// ---------------------------------------------------------------------------
// Bundler RPC Calls
// ---------------------------------------------------------------------------

// Cache: once deployed, a contract stays deployed (irreversible)
const _deployedCache = new Map<string, true>();

async function isDeployed(
  address: string,
  chainId: number,
): Promise<boolean> {
  const key = `${chainId}:${address.toLowerCase()}`;
  if (_deployedCache.has(key)) return true;

  // Deployment status is CORRECTNESS-CRITICAL: it decides whether the UserOp carries
  // initCode. Guessing "deployed" on a transient RPC failure ships an op with EMPTY
  // initCode for a fresh account → bundler rejects with "AA20 account not deployed"
  // (every new user's first send fails). Guessing "undeployed" for an already-deployed
  // account attaches initCode → "AA10 sender already constructed". Neither guess is safe,
  // so on any INDETERMINATE result we fail fast with a retryable error and let the caller
  // retry — same philosophy as the deployed-nonce guard in sendUserOp. Only a DEFINITIVE
  // answer ('0x' = not deployed, or real code = deployed) is trusted.
  let response;
  try {
    response = await rpcCall('eth_getCode', [address, 'latest'], chainId);
  } catch (err) {
    console.error('[UserOp] eth_getCode failed:', err instanceof Error ? err.message : String(err));
    throw new Error('Could not verify the account deployment status — the network may be unstable. Please try again.');
  }
  if (response.error) {
    console.error('[UserOp] eth_getCode RPC error:', JSON.stringify(response.error));
    throw new Error('Could not verify the account deployment status — the network may be unstable. Please try again.');
  }
  const result = response.result as string | undefined;
  if (typeof result !== 'string') {
    console.error('[UserOp] eth_getCode returned no result:', JSON.stringify(response));
    throw new Error('Could not verify the account deployment status — the network may be unstable. Please try again.');
  }
  const deployed = result !== '0x' && result.length > 2;
  console.log('[UserOp] isDeployed:', deployed, 'code length:', result.length);
  if (deployed) _deployedCache.set(key, true);
  return deployed;
}

// Cache: nonce is valid briefly (invalidated after each tx)
const _nonceCache = new Map<string, { nonce: string; at: number }>();
const NONCE_CACHE_TTL = 10_000; // 10s

async function getNonce(
  safeAddress: string,
  chainId: number,
): Promise<string> {
  const key = `${chainId}:${safeAddress.toLowerCase()}`;
  const cached = _nonceCache.get(key);
  if (cached && Date.now() - cached.at < NONCE_CACHE_TTL) return cached.nonce;

  const selector = toHex(functionSelector('getNonce(address,uint192)'));
  const addressEncoded = toHex(abiEncodeAddress(safeAddress));
  const keyEncoded = toHex(abiEncodeUint256(0n));
  const callData = '0x' + selector + addressEncoded + keyEncoded;

  const response = await rpcCall(
    'eth_call',
    [{ to: ENTRY_POINT, data: callData }, 'latest'],
    chainId,
  );

  // Same fail-open trap as isDeployed: coercing a missing/errored result to '0x0' makes a
  // DEPLOYED wallet sign nonce 0 → bundler "AA25 invalid account nonce". A real "no nonce yet"
  // account returns a valid 0x00..00 result, NOT an error, so throwing here only fires on a
  // genuine RPC failure. Send paths guard `deployed && nonce===null`; undeployed callers still
  // safely fall back to 0x0 via their own .catch (an undeployed account's nonce IS 0).
  if (response.error || typeof response.result !== 'string') {
    console.error('[UserOp] getNonce RPC error:', JSON.stringify(response.error ?? response));
    throw new Error('Could not fetch the account nonce — the network may be unstable. Please try again.');
  }
  const nonce = response.result as string;
  _nonceCache.set(key, { nonce, at: Date.now() });
  return nonce;
}

/**
 * Optimistically increment the cached nonce after submitting a UserOp.
 * Prevents concurrent transactions from reusing the same nonce.
 * If the cache is stale or missing, the next getNonce() will fetch fresh.
 */
function incrementNonceCache(safeAddress: string, chainId: number): void {
  const key = `${chainId}:${safeAddress.toLowerCase()}`;
  const cached = _nonceCache.get(key);
  if (!cached) return;
  const currentNonce = BigInt(cached.nonce);
  const nextNonce = '0x' + (currentNonce + 1n).toString(16);
  _nonceCache.set(key, { nonce: nextNonce, at: Date.now() });
}

// Cache: gas prices are stable enough for 15s
/**
 * The chain signals that set a UserOp's gas price, plus the derived network price.
 * Callers apply the tier markup on top of `gasPrice`.
 */
export interface ChainGasPrice {
  /** Network price = max(eth_gasPrice, baseFee + priorityFee). */
  gasPrice: bigint;
  /** Latest-block base fee (0n on non-EIP-1559 chains / when unavailable). */
  baseFee: bigint;
  /** Suggested priority tip (eth_maxPriorityFeePerGas), or derived from eth_gasPrice. */
  priorityFee: bigint;
  /**
   * True only when `priorityFee` came from a real eth_maxPriorityFeePerGas read — NOT
   * derived from eth_gasPrice, defaulted, or skipped (Tempo). When false the wallet has
   * no trustworthy independent tip, so the abuse cap (isQuoteAbusive) fail-opens and
   * defers to the bundler rather than false-reject an honest quote (→ blank "—" fee).
   */
  tipMeasured: boolean;
}

/**
 * Derive the network gas price from the raw chain signals, using the SAME formula the
 * bundler prices from (vela-bundler shared/simulation/index.ts getGasPrices +
 * shared/gas/fee-model.ts): networkPrice = max(eth_gasPrice, baseFee + tip).
 *
 * The priority tip is LOAD-BEARING on chains where the base fee is a rounding error but
 * validators still demand a real tip — Gnosis is the canonical case (baseFee≈0, the tip
 * is essentially the whole gas price). The old max(eth_gasPrice, baseFee) dropped the
 * tip, under-pricing a Gnosis UserOp ~40×, so the bundler rejected it ("derived outer
 * price 26 < 20% of chain rate 1219") AND its honest tip-inclusive quote tripped the
 * wallet's own 3× sanity cap, rendering the fee as "—".
 *
 * When the tip is 0 (the chain didn't answer eth_maxPriorityFeePerGas, or it's a Tempo
 * chain we deliberately don't tip-query) we recover it from eth_gasPrice exactly like
 * the bundler does, so the result never regresses below today's price. `tipMeasured`
 * (default: whether a positive tip was supplied) records whether that tip is a real
 * reading — the caller passes it explicitly to distinguish a measured 0 (L2s) from a
 * failed/absent read. Pure + tested.
 */
export function deriveChainGasPrice(signals: {
  ethGasPrice: bigint;
  baseFee: bigint;
  priorityFee: bigint;
  tipMeasured?: boolean;
}): ChainGasPrice {
  const { ethGasPrice, baseFee } = signals;
  const priorityFee = signals.priorityFee > 0n
    ? signals.priorityFee
    : ethGasPrice > baseFee ? ethGasPrice - baseFee : 0n;
  const withTip = baseFee + priorityFee;
  const gasPrice = ethGasPrice > withTip ? ethGasPrice : withTip;
  const tipMeasured = signals.tipMeasured ?? signals.priorityFee > 0n;
  return { gasPrice, baseFee, priorityFee, tipMeasured };
}

const _gasPriceCache = new Map<number, ChainGasPrice & { at: number }>();
const GAS_PRICE_CACHE_TTL = 15_000; // 15s

/**
 * Fetch the on-chain network gas price (no tier markup — callers add that). Reads the
 * same three signals the bundler reads (eth_gasPrice, block baseFee,
 * eth_maxPriorityFeePerGas) so the wallet's price basis matches the bundler's.
 *
 * Tempo is EXCLUDED from the tip query: there gas is denominated in attodollars and
 * eth_maxPriorityFeePerGas is meaningless, so feeding it in would corrupt the stablecoin
 * reimbursement. On Tempo the tip isn't measured, so `gasPrice` equals the legacy
 * max(eth_gasPrice, baseFee) and `tipMeasured` is false. The block/tip reads degrade
 * gracefully — only an eth_gasPrice failure falls through to the 5-gwei default; a
 * failed tip read leaves tipMeasured=false so the abuse cap defers to the bundler.
 */
async function getGasPrices(chainId: number): Promise<ChainGasPrice> {
  const cached = _gasPriceCache.get(chainId);
  if (cached && Date.now() - cached.at < GAS_PRICE_CACHE_TTL) {
    return { gasPrice: cached.gasPrice, baseFee: cached.baseFee, priorityFee: cached.priorityFee, tipMeasured: cached.tipMeasured };
  }

  const wantTip = !isTempoChain(chainId);
  try {
    const [gasPriceRes, blockRes, tipRes] = await Promise.all([
      rpcCall('eth_gasPrice', [], chainId),
      rpcCall('eth_getBlockByNumber', ['latest', false], chainId).catch(() => null),
      wantTip
        ? rpcCall('eth_maxPriorityFeePerGas', [], chainId).catch(() => null)
        : Promise.resolve(null),
    ]);
    const ethGasPrice = parseHexUInt64(gasPriceRes.result as string | undefined);
    const baseFee = blockRes?.result?.baseFeePerGas
      ? parseHexUInt64(blockRes.result.baseFeePerGas as string)
      : 0n;
    // A present result (even "0x0" on L2s) is a real measurement; null = failed/skipped.
    const tipMeasured = wantTip && tipRes?.result != null;
    const tip = tipRes?.result ? parseHexUInt64(tipRes.result as string) : 0n;
    const derived = deriveChainGasPrice({ ethGasPrice, baseFee, priorityFee: tip, tipMeasured });
    if (derived.gasPrice > 0n) {
      console.log(`[UserOp] Gas: ethGasPrice=${ethGasPrice} baseFee=${baseFee} tip=${derived.priorityFee} measured=${derived.tipMeasured} using=${derived.gasPrice}`);
      _gasPriceCache.set(chainId, { ...derived, at: Date.now() });
      return derived;
    }
  } catch {
    // Use defaults
  }

  const fallback: ChainGasPrice = { gasPrice: 5_000_000_000n, baseFee: 5_000_000_000n, priorityFee: 0n, tipMeasured: false }; // 5 gwei
  _gasPriceCache.set(chainId, { ...fallback, at: Date.now() });
  return fallback;
}

/**
 * A caller-supplied maxFee override is only usable when it is a REAL positive
 * bigint. The type is erased at runtime (a mis-wired caller can pass anything),
 * and a zero/negative value — e.g. a degenerate 0x0 upstream quote echoed back
 * through the confirm screen — would sign a UserOp the bundler must reject
 * ("maxFeePerGas must be > 0"). Unusable → the send path re-derives the price.
 */
export function isUsableFeeOverride(v: unknown): v is bigint {
  return typeof v === 'bigint' && v > 0n;
}

/**
 * Calculate UserOp maxFeePerGas = gasPrice × speedTier × BUNDLER_MARGIN.
 *
 * Speed tier controls how much the bundler bids for on-chain inclusion.
 * BUNDLER_MARGIN_PERCENT is the fixed profit margin (50 → 1.5x).
 * Bundler derives outerGasPrice = maxFeePerGas / (1 + BUNDLER_MARGIN_PERCENT/100).
 *
 * Example (standard, 10 gwei, margin=50%):
 *   maxFee = 10 × 1.2 × 1.5 = 18 gwei
 *   bundler outerGasPrice = 18 / 1.5 = 12 gwei
 *   margin = 50%
 */
export function calcMaxFeePerGas(gasPrice: bigint, tier: GasTier = 'standard'): bigint {
  const m = GAS_TIER_MULTIPLIERS[tier];
  // gasPrice × speedTier × BUNDLER_MARGIN
  let maxFee = (gasPrice * m.num * BUNDLER_MARGIN_NUM) / (m.den * BUNDLER_MARGIN_DEN);
  if (maxFee < 1n) maxFee = 1n;
  return maxFee;
}

/**
 * Hard client-side cap: refuse any bundler quote above this multiple of the
 * chain's own gas price. The relayer policy is ~2× the on-chain cost; 3× leaves
 * headroom for base-fee volatility between our query and the bundler's, while
 * still blocking an abusive or misconfigured (e.g. third-party) bundler.
 */
export const MAX_QUOTE_VS_CHAIN_MULTIPLE = 3n;

/** Thrown when a bundler quotes a gas price above the client-side sanity cap. */
export class GasQuoteTooHighError extends Error {
  constructor(quoted: bigint, chainGasPrice: bigint) {
    super(
      `The relayer quoted an abnormally high gas price ` +
        `(${(Number(quoted) / Number(chainGasPrice || 1n)).toFixed(1)}× the network rate). ` +
        `For your safety, Vela won't submit this transaction. Try again, or switch bundler.`,
    );
    this.name = 'GasQuoteTooHighError';
  }
}

/**
 * Per-tier priority-tip scaling the bundler applies when it quotes
 * (pimlico_getUserOperationGasPrice → quote(100|150|200); see vela-bundler
 * shared/rpc/handlers.ts). The wallet mirrors it so the sanity cap below compares a
 * quote against the honest network price AT THE SAME TIER. Without this a fast-tier
 * quote (tip×2) looks abusive next to a slow-tier (tip×1) baseline on a tip-dominated
 * chain like Gnosis, and the fee falsely renders as "—". The bundler only quotes
 * slow/standard/fast; 'rapid' falls back to a local estimate and never reaches the cap.
 */
const BUNDLER_QUOTE_TIP_PERCENT: Record<GasTier, bigint> = {
  slow: 100n, standard: 150n, fast: 200n, rapid: 200n,
};

/**
 * True when a bundler's quoted maxFeePerGas is abusively high.
 *
 * Judged PRIMARILY by the bundler's OWN reported network cost (its markup) — the same
 * authoritative source that decides accept/reject at submit — so the check is reliable and
 * consistent across chains and NEVER depends on the wallet's per-chain RPC. That RPC is
 * unreliable on cheap-gas chains (Gnosis eth_gasPrice ≈ 0, and providers disagree on the
 * priority tip); letting it veto the bundler's honest quote is exactly what blanked the fee
 * to "—" and forced an under-priced local fallback (the chronic "gas price too low 33 <
 * 1225" on Gnosis). Honest Vela markup is 2× (userPrice = 2 × networkPrice); the cap is 3×
 * for head-room.
 *
 * A generic bundler that omits the networkFee field (reportedNetworkFeePerGas = 0n) gives no
 * markup signal, so we cross-check against the wallet's INDEPENDENT price — but only when it
 * was reliably measured (tipMeasured); otherwise we trust the bundler (fail-open). Pure + tested.
 */
export function isQuoteAbusive(
  quotedMaxFeePerGas: bigint,
  reportedNetworkFeePerGas: bigint,
  chain: ChainGasPrice,
  tier: GasTier,
): boolean {
  // Preferred: the bundler's own markup — user price vs its reported network cost.
  if (reportedNetworkFeePerGas > 0n) {
    return quotedMaxFeePerGas > reportedNetworkFeePerGas * MAX_QUOTE_VS_CHAIN_MULTIPLE;
  }
  // Generic bundler, no network-cost signal → cross-check the wallet's own price, but only
  // when trustworthy. Never let an unreliable per-chain RPC veto the bundler's quote.
  if (!chain.tipMeasured) return false;
  const tipMul = BUNDLER_QUOTE_TIP_PERCENT[tier] ?? 150n;
  const scaledNetwork = chain.baseFee + (chain.priorityFee * tipMul) / 100n;
  const expectedNetwork = chain.gasPrice > scaledNetwork ? chain.gasPrice : scaledNetwork;
  if (expectedNetwork <= 0n) return false;
  return quotedMaxFeePerGas > expectedNetwork * MAX_QUOTE_VS_CHAIN_MULTIPLE;
}

export interface GasQuoteTier {
  maxFeePerGas: bigint;
  maxPriorityFeePerGas: bigint;
  networkFeePerGas: bigint;
  relayerFeePerGas: bigint;
}

/**
 * Ask the bundler for the authoritative gas price (pimlico_getUserOperationGasPrice).
 * The bundler is the single source of truth for the price; the wallet uses its
 * quote and displays it — it never marks the price up on its own.
 *
 * Returns null when the bundler doesn't support the method (older / generic
 * bundler) so the caller can fall back to a local estimate. Throws
 * GasQuoteTooHighError when the quote exceeds the client-side sanity cap — that
 * is a refusal, not a fallback, so an abusive bundler can't overcharge silently.
 */
export async function getBundlerGasQuote(
  chainId: number,
  tier: GasTier = 'standard',
): Promise<GasQuoteTier | null> {
  let resp;
  let chain: ChainGasPrice;
  try {
    const [r, gp] = await Promise.all([
      rpcCall('pimlico_getUserOperationGasPrice', [], chainId),
      getGasPrices(chainId),
    ]);
    resp = r;
    chain = gp;
  } catch (err) {
    console.log(
      '[Gas] Bundler gas-price quote unavailable, using local estimate:',
      err instanceof Error ? err.message : String(err),
    );
    return null;
  }

  // Dev fault seam (vela.zeroGasQuote): replay the degenerate 0x0 quote that
  // produced "预估费用 ~0 ETH" + a signed op the bundler rejects, so the guard
  // below stays verifiable in the test env. Always false in production.
  if (gasQuoteShouldZero(chainId)) {
    resp = { result: { [tier]: { maxFeePerGas: '0x0', maxPriorityFeePerGas: '0x0', networkFeePerGas: '0x0', relayerFeePerGas: '0x0' } } };
  }

  const t = resp.result?.[tier];
  if (resp.error || !t?.maxFeePerGas) return null;

  const maxFeePerGas = parseHexUInt64(t.maxFeePerGas as string);
  // A zero quote is degenerate, not authoritative: "0x0" is a truthy string (it
  // passes the check above) but pricing an op at 0 is self-refuting — the bundler's
  // own validation rejects maxFeePerGas = 0. Seen when a quote endpoint sits on a
  // zero-gas chain/fork or a broken RPC. Treat as "can't quote" → local fallback
  // (which floors at a real price) instead of displaying ~0 and signing a doomed op.
  if (maxFeePerGas <= 0n) {
    console.warn(`[Gas] Bundler quoted ${tier} maxFeePerGas = 0 on chain ${chainId} — ignoring quote, using local estimate.`);
    return null;
  }
  const maxPriorityFeePerGas = parseHexUInt64((t.maxPriorityFeePerGas ?? t.maxFeePerGas) as string);
  // The bundler's OWN network-cost basis (Vela extension). 0n when a generic bundler omits it.
  const reportedNetworkFee = t.networkFeePerGas ? parseHexUInt64(t.networkFeePerGas as string) : 0n;
  // Display/return value; fall back to the wallet's chain price when the field is absent.
  const networkFeePerGas = reportedNetworkFee > 0n ? reportedNetworkFee : chain.gasPrice;
  const relayerFeePerGas = t.relayerFeePerGas
    ? parseHexUInt64(t.relayerFeePerGas as string)
    : maxFeePerGas > networkFeePerGas
      ? maxFeePerGas - networkFeePerGas
      : 0n;

  // Client-side hard cap — judged by the BUNDLER's own reported markup (maxFee vs its
  // networkFee), NOT the wallet's per-chain RPC, which must never veto the authoritative
  // quote (that veto is what blanked the fee to "—" and under-priced Gnosis). See isQuoteAbusive.
  if (isQuoteAbusive(maxFeePerGas, reportedNetworkFee, chain, tier)) {
    console.warn(
      `[Gas] Bundler quote ${maxFeePerGas} exceeds ${MAX_QUOTE_VS_CHAIN_MULTIPLE}× its reported ${tier} ` +
      `network cost ${networkFeePerGas} — refusing.`,
    );
    throw new GasQuoteTooHighError(maxFeePerGas, networkFeePerGas);
  }

  return { maxFeePerGas, maxPriorityFeePerGas, networkFeePerGas, relayerFeePerGas };
}

async function estimateGas(
  userOp: UserOperation,
  chainId: number,
): Promise<GasEstimate> {
  const dict = userOpToDict(userOp);
  console.log('[UserOp] Estimating gas, sender:', dict.sender, 'nonce:', dict.nonce);

  const response = await rpcCall(
    'eth_estimateUserOperationGas',
    [dict, ENTRY_POINT],
    chainId,
  );

  if (response.error) {
    console.error('[UserOp] Estimation RPC error:', JSON.stringify(response.error));
    throw new Error(response.error.message ?? 'Gas estimation failed');
  }

  const result = response.result as Record<string, string> | undefined;
  if (!result) {
    console.error('[UserOp] Estimation returned empty result:', JSON.stringify(response));
    throw new Error('Failed to estimate gas — empty result');
  }

  return {
    verificationGasLimit: parseHexUInt64(result.verificationGasLimit),
    callGasLimit: parseHexUInt64(result.callGasLimit),
    preVerificationGas: parseHexUInt64(result.preVerificationGas),
  };
}

async function submitUserOp(
  userOp: UserOperation,
  chainId: number,
  extra?: Record<string, string>,
): Promise<string> {
  const dict = userOpToDict(userOp, extra);
  const initCodePresent = userOp.initCode.length >= 20;
  console.log('[UserOp] Submitting:', JSON.stringify({
    sender: dict.sender,
    nonce: dict.nonce,
    initCodePresent,                        // ← HOP 1 evidence: did the wallet attach the deploy?
    factory: dict.factory ?? '(none)',
    factoryDataLen: dict.factoryData?.length ?? 0,
    callDataLen: dict.callData?.length ?? 0,
    signatureLen: dict.signature?.length ?? 0,
    verificationGasLimit: dict.verificationGasLimit,
    callGasLimit: dict.callGasLimit,
    maxFeePerGas: dict.maxFeePerGas,
  }));

  // Structural AA20 guard. An undeployed sender + empty initCode is a GUARANTEED
  // "AA20 account not deployed" on-chain — and worse, it strands funds silently. Rather
  // than submit a doomed op, verify against the chain and refuse with a clear, retryable
  // error. This catches a wrong "deployed" read from ANY cause (RPC hiccup, a
  // mis-configured custom-network RPC pointing at the wrong chain, a stale cache) — the
  // last line of defence behind isDeployed's fail-fast. On a truly deployed account the
  // read is cache-hot (the send path already resolved it), so this adds no latency there.
  if (!initCodePresent) {
    const deployed = await isDeployed(userOp.sender, chainId); // throws (retryable) if indeterminate
    if (!deployed) {
      console.error(`[UserOp] ABORT pre-submit: sender ${userOp.sender} has NO on-chain code but the op carries NO initCode → would revert AA20. Refusing to submit.`);
      throw new Error('This account is not deployed on this network yet and the transaction is missing its deployment step. Please try again.');
    }
  }

  // Retry on transient bundler errors (e.g. EOA busy processing another bundle).
  const MAX_RETRIES = 3;
  const RETRY_DELAY = 3_000;

  for (let attempt = 0; attempt <= MAX_RETRIES; attempt++) {
    const response = await rpcCall(
      'eth_sendUserOperation',
      [dict, ENTRY_POINT],
      chainId,
    );

    const result = response.result as string | undefined;
    if (result) return result;

    const errorMsg = parseBundlerError(response.error);
    const isRetryable = errorMsg.includes('currently processing') || errorMsg.includes('Retry later');
    if (!isRetryable || attempt === MAX_RETRIES) {
      throw new Error(errorMsg);
    }

    console.log(`[UserOp] Bundler busy, retry ${attempt + 1}/${MAX_RETRIES} in ${RETRY_DELAY}ms...`);
    await sleep(RETRY_DELAY);
  }

  throw new Error('Bundler unavailable after retries');
}

export async function waitForReceipt(
  userOpHash: string,
  chainId: number,
  timeout: number = 120_000,
  signal?: AbortSignal,
): Promise<string> {
  const start = Date.now();

  // Track whether we ever got a clean "not ready yet" answer vs. only ever hit
  // RPC/bundler errors. The distinction drives the final message: "submitted but
  // not confirmed" (the bundler answered, op just hasn't landed) is very different
  // from "bundler unreachable, status unknown" (we never got an answer at all).
  let sawCleanResponse = false;
  let rpcFailures = 0;
  let polls = 0;

  while (Date.now() - start < timeout) {
    // Caller cancelled (e.g. the send screen unmounted) — stop polling.
    if (signal?.aborted) throw makeAbortError();
    polls++;

    try {
      // The submitted-receipt screen and this background waiter can both be active.
      // They share this request (and its 3-second rate limit), so opening the screen never
      // doubles eth_getUserOperationReceipt traffic to the bundler.
      const outcome = await requestUserOpReceipt(userOpHash, chainId);
      if (!outcome.reachedBundler) {
        rpcFailures++;
      } else {
        sawCleanResponse = true;
        const result = outcome.resolution;
        if (result?.txHash) {
          // Check if the UserOp was marked as failed (e.g. tx dropped from mempool)
          if (result.failed) {
            throw new Error('Transaction was dropped from the network. Try again with a higher gas price.');
          }
          console.log('[UserOp] Receipt landed', {
            userOpHash: `${userOpHash.slice(0, 10)}…`,
            chainId,
            txHash: result.txHash,
            afterMs: Date.now() - start,
            polls,
          });
          return result.txHash;
        }
      }
    } catch (err) {
      // A genuine drop (success === false) is final — rethrow it.
      if (err instanceof Error && /dropped from the network/.test(err.message)) throw err;
      // All bundler endpoints failed this round (network blip). Previously this
      // aborted the whole wait; instead keep polling, since the op may still land.
      rpcFailures++;
    }

    // Receipt production is asynchronous; a fixed cadence is friendlier to the bundler and
    // matches the submitted-receipt countdown. The shared poller also coalesces any UI poll.
    await sleep(USER_OP_RECEIPT_POLL_INTERVAL_MS);
  }

  const shortOp = `${userOpHash.slice(0, 10)}…`;
  if (!sawCleanResponse && rpcFailures > 0) {
    // We never reached the bundler — the op's fate is genuinely unknown, not a
    // confirmed pending. Mark it as such so the caller can reconcile/retry later.
    console.warn('[UserOp] Bundler UNREACHABLE — status unknown', {
      userOpHash: shortOp, chainId, afterMs: Date.now() - start, polls, rpcFailures,
    });
    throw new Error(
      `Couldn't reach the bundler to confirm transaction ${shortOp}; its status is unknown. ` +
      `Check the explorer in a few minutes before retrying.`,
    );
  }
  // Submitted and accepted by the bundler, but no on-chain receipt in time. The op
  // is NOT lost — it may still land (or the bundler's gas account couldn't fund the
  // bundle). Say so, and surface the hash, instead of implying outright failure.
  // NOTE: "bundler answered cleanly for the full window but never produced a receipt"
  // is the signature of a bundler that ACCEPTS the op but never lands the bundle on
  // this chain (e.g. an unfunded/misconfigured chain-56 gas account) — a bundler-side
  // condition, NOT a wallet bug. Grep this line to tell it apart from the reach case.
  console.warn('[UserOp] ACCEPTED but NOT landed within timeout — bundler is not settling this chain', {
    userOpHash: shortOp, chainId, timeoutMs: timeout, polls, rpcFailures, sawCleanResponse,
  });
  throw new Error(
    `Transaction submitted (${shortOp}) but not confirmed within ${Math.round(timeout / 1000)}s. ` +
    `It may still land on-chain — check the explorer before retrying.`,
  );
}

// ---------------------------------------------------------------------------
// UserOp Serialization
// ---------------------------------------------------------------------------

/**
 * Convert UserOperation to JSON-RPC format.
 * ERC-4337 v0.7 uses individual fields + factory/factoryData split.
 */
function userOpToDict(
  userOp: UserOperation,
  extra?: Record<string, string>,
): Record<string, string> {
  const dict: Record<string, string> = {
    sender: userOp.sender,
    nonce: userOp.nonce,
    callData: '0x' + toHex(userOp.callData),
    callGasLimit: '0x' + userOp.callGasLimit.toString(16),
    verificationGasLimit: '0x' + userOp.verificationGasLimit.toString(16),
    preVerificationGas: '0x' + userOp.preVerificationGas.toString(16),
    maxFeePerGas: '0x' + userOp.maxFeePerGas.toString(16),
    maxPriorityFeePerGas: '0x' + userOp.maxPriorityFeePerGas.toString(16),
    signature: '0x' + toHex(userOp.signature),
  };

  // v0.7: split initCode into factory + factoryData
  if (userOp.initCode.length >= 20) {
    dict.factory = '0x' + toHex(userOp.initCode.slice(0, 20));
    dict.factoryData = '0x' + toHex(userOp.initCode.slice(20));
  }

  // v0.7: split paymasterAndData
  if (userOp.paymasterAndData.length >= 20) {
    dict.paymaster = '0x' + toHex(userOp.paymasterAndData.slice(0, 20));
    dict.paymasterData = '0x' + toHex(userOp.paymasterAndData.slice(20));
    dict.paymasterVerificationGasLimit = '0x0';
    dict.paymasterPostOpGasLimit = '0x0';
  }

  // Vela extension fields (e.g. Tempo `feeToken`). The bundler reads these and
  // strips them before building the standard PackedUserOperation.
  if (extra) Object.assign(dict, extra);

  return dict;
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/**
 * Parse the bundler's "[existingHash:0x…]" marker out of a submit error. When a
 * previous UserOp for the account is still pending, the bundler rejects the new one
 * but reports the in-flight hash so we can poll its receipt instead of failing.
 * Returns the hash, or null when the error isn't this case. (Same recovery is used
 * by both the standard and Tempo send paths — this is their shared parser.)
 */
export function parseExistingUserOpHash(errMsg: string): string | null {
  const m = errMsg.match(/\[existingHash:(0x[0-9a-fA-F]+)\]/);
  return m ? m[1] : null;
}

export function parseHexUInt64(value: string | undefined): bigint {
  if (!value) return 0n;
  const clean = value.startsWith('0x') ? value.slice(2) : value;
  if (!clean) return 0n;
  return BigInt('0x' + clean);
}

function bigintMax(a: bigint, b: bigint): bigint {
  return a > b ? a : b;
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/** An Error tagged like a standard AbortError so callers can detect cancellation. */
function makeAbortError(): Error {
  const e = new Error('Aborted');
  e.name = 'AbortError';
  return e;
}

/**
 * Verify that critical contracts are deployed on this chain.
 * Caches results per chain to avoid repeated checks.
 */
const _chainReadyCache = new Map<number, boolean>();

async function verifyChainReady(chainId: number): Promise<void> {
  if (_chainReadyCache.get(chainId)) return;

  // Check EntryPoint as a quick proxy — if it's there, the chain was set up
  const entryPointDeployed = await isDeployed(ENTRY_POINT, chainId);
  if (!entryPointDeployed) {
    throw new Error(
      'This network is not ready yet. Required smart contracts (EntryPoint) ' +
      'are not deployed. Please activate this network in Settings → Transaction Services.',
    );
  }

  _chainReadyCache.set(chainId, true);
}

/** Parse bundler error into a human-readable message. */
function parseBundlerError(error: any): string {
  if (!error) return 'Transaction failed: unknown error';

  const msg = error.message ?? error.data ?? '';

  // Common bundler errors
  if (msg.includes('insufficient funds') || msg.includes('balance too low'))
    return 'Insufficient balance to cover gas fees. Please fund your account.';
  if (msg.includes('could not load bundle') || msg.includes('simulation failed'))
    return 'Transaction simulation failed. The network may be congested or the transaction parameters are invalid. Please try again.';
  if (msg.includes('AA21') || msg.includes('didn\'t pay prefund'))
    return 'Insufficient gas funds. The bundler account needs more balance on this network.';
  if (msg.includes('AA10') || msg.includes('sender already constructed'))
    return 'Wallet deployment conflict. Please try again.';
  if (msg.includes('AA13') || msg.includes('initCode failed'))
    return 'Wallet deployment failed. Required contracts may not be deployed on this network.';
  if (msg.includes('AA23') || msg.includes('reverted'))
    return 'Transaction reverted during simulation. Check recipient address and amount.';
  if (msg.includes('AA25') || msg.includes('invalid account nonce'))
    return 'Transaction nonce mismatch. Please try again.';
  if (msg.includes('rate limit') || msg.includes('429'))
    return 'Bundler rate limit reached. Please wait a moment and try again.';

  // Fallback: show the actual message, cleaned up
  const cleanMsg = msg.replace(/^execution reverted:\s*/i, '').trim();
  if (cleanMsg) return `Transaction failed: ${cleanMsg}`;

  // Last resort
  return `Transaction failed: ${JSON.stringify(error).slice(0, 200)}`;
}
