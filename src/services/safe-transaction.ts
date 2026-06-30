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
import { fetchBundlerAccountInfo } from './bundler-service';
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
): Promise<SubmitResult> {
  if (isTempoChain(chainId)) {
    // Tempo has no native coin; a native send is unusual but routed for consistency
    // (gas is paid in the default stablecoin, not the value being moved).
    return sendUserOpTempo(from, [{ to, value: valueWei, data: new Uint8Array(0) }], TEMPO_DEFAULT_FEE_TOKEN, chainId, publicKeyHex, signFn);
  }
  const callData = buildExecuteCallData(to, valueWei, new Uint8Array(0));
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

  const callData = buildExecuteCallData(tokenAddress, '0', transferData);
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
): Promise<SubmitResult> {
  if (isTempoChain(chainId)) {
    // dApp / contract call: pay gas in the default stablecoin (pathUSD).
    return sendUserOpTempo(from, [{ to, value: valueWei, data }], TEMPO_DEFAULT_FEE_TOKEN, chainId, publicKeyHex, signFn);
  }
  const callData = buildExecuteCallData(to, valueWei, data);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn, maxFeeOverride);
}

/** Send batched calls atomically via Safe MultiSend (EIP-5792 wallet_sendCalls). */
export async function sendBatchCalls(
  from: string,
  calls: Array<{ to: string; value: string; data: string }>,
  chainId: number,
  publicKeyHex: string,
  signFn: SignFn,
): Promise<SubmitResult> {
  const byteCalls: MultiSendCall[] = calls.map(c => ({
    to: c.to,
    value: c.value,
    data: c.data && c.data !== '0x' ? fromHex(stripHexPrefix(c.data)) : new Uint8Array(0),
  }));

  if (isTempoChain(chainId)) {
    return sendUserOpTempo(from, byteCalls, TEMPO_DEFAULT_FEE_TOKEN, chainId, publicKeyHex, signFn);
  }

  const callData = buildMultiSendExecuteCallData(byteCalls);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn);
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
): Promise<TransactionFeeEstimate> {
  const [deployed, { gasPrice }] = await Promise.all([
    isDeployed(from, chainId),
    getGasPrices(chainId),
  ]);

  // Mirror sendUserOpTempo's pricing: the displayed fee is the reimbursement, priced off
  // the REALISTIC gas (tempoExpectedGas) for a simple send (1 transfer + 1 reimbursement
  // = 2 sub-calls), NOT the padded UserOp limits. Keeps the quote == what's charged.
  const expectedGas = tempoExpectedGas(deployed, 2);
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
): Promise<TransactionFeeEstimate> {
  // Tempo pays gas in a stablecoin, not the native coin — separate model.
  if (isTempoChain(chainId)) return estimateTempoFee(from, chainId, tier);

  // The bundler is the single source of truth for the price. getBundlerGasQuote
  // throws GasQuoteTooHighError (propagated here = refuse) if the quote is abusive,
  // and returns null only when the bundler can't quote (then we fall back locally).
  const [deployed, { gasPrice }, quote] = await Promise.all([
    isDeployed(from, chainId),
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
  // ERC-20-sized dummy. Built identically to sendContractCall so the displayed
  // estimate matches what's actually submitted.
  const estCallData = tx?.to
    ? buildExecuteCallData(
        tx.to,
        stripHexPrefix(tx.value ?? '0') || '0',
        tx.data && tx.data !== '0x' ? fromHex(stripHexPrefix(tx.data)) : new Uint8Array(0),
      )
    : buildExecuteCallData(from, '0', new Uint8Array(68));

  // Try to get accurate gas estimates from the bundler. This catches high-gas chains
  // (e.g. Monad) where actual gas usage is 3-10x higher than the static defaults below.
  let totalGas: bigint | null = null;
  try {
    const verificationGas = deployed ? VERIFICATION_GAS_DEPLOYED : VERIFICATION_GAS_UNDEPLOYED;
    const dummySig = buildDummySignature();
    const dummyOp: UserOperation = {
      sender: from,
      nonce: '0x0',
      initCode: new Uint8Array(0),
      callData: estCallData,
      verificationGasLimit: verificationGas,
      callGasLimit: CALL_GAS_LIMIT,
      preVerificationGas: PRE_VERIFICATION_GAS,
      maxFeePerGas: userOpMaxFee,
      maxPriorityFeePerGas: userOpMaxFee,
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

/** Format wei to a human-readable ETH-like string. */
export function formatWeiToEth(wei: bigint): string {
  const eth = Number(wei) / 1e18;
  if (eth === 0) return '0';
  if (eth < 0.000001) return '< 0.000001';
  if (eth < 0.001) return eth.toFixed(6);
  if (eth < 1) return eth.toFixed(4);
  return eth.toFixed(3);
}

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
    getNonce(safeAddress, chainId).catch(() => '0x0'),
    getGasPrices(chainId),
  ]);

  // Build initCode if needed
  const initCode: Uint8Array = deployed
    ? new Uint8Array(0)
    : buildInitCode(publicKeyHex);

  // Use fetched nonce for deployed wallets, 0 for undeployed
  const nonce: string = deployed ? nonceResult : '0x0';

  // Guard: maxFeeOverride is typed bigint, but the type is erased at runtime.
  // A mis-wired caller (e.g. onPress={approveRequest} passing a gesture event)
  // could hand us a non-bigint, which would serialize to "0x[object Object]"
  // and blow up both bundler estimation and the SafeOp hash (BigInt parse).
  const maxFee =
    typeof maxFeeOverride === 'bigint'
      ? maxFeeOverride
      : calcMaxFeePerGas(gasPrices.gasPrice);
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
    const existingHashMatch = errMsg.match(/\[existingHash:(0x[0-9a-fA-F]+)\]/);
    if (existingHashMatch) {
      console.log(`[UserOp] Previous op pending (${existingHashMatch[1]}), polling for receipt...`);
      return {
        userOpHash: existingHashMatch[1],
        waitForTxHash: () => waitForReceipt(existingHashMatch[1], chainId, 60_000),
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

  // The bundler's per-Safe EOA pays the outer 0x76 gas and is the reimbursement
  // recipient. Resolve it first so the batched transfer targets the right address.
  const info = await fetchBundlerAccountInfo(chainId, safeAddress);
  const feeCollector = info?.depositAddress;
  if (!feeCollector || !/^0x[0-9a-fA-F]{40}$/.test(feeCollector)) {
    throw new Error('The Tempo gas relayer is unavailable right now. Please try again.');
  }

  _gasPriceCache.delete(chainId);
  const [deployed, nonceResult, gasPrices] = await Promise.all([
    isDeployed(safeAddress, chainId),
    getNonce(safeAddress, chainId).catch(() => '0x0'),
    getGasPrices(chainId),
  ]);

  const initCode: Uint8Array = deployed ? new Uint8Array(0) : buildInitCode(publicKeyHex);
  const nonce: string = deployed ? nonceResult : '0x0';

  // Build the batch with a placeholder reimbursement (the transfer VALUE doesn't
  // affect gas), estimate, then bake the real amount derived from that estimate.
  const buildBatch = (reimbursement: bigint): Uint8Array =>
    buildMultiSendExecuteCallData([
      ...innerCalls,
      { to: feeToken, value: '0', data: encodeErc20Transfer(feeCollector, reimbursement) },
    ]);

  // The MultiSend batch executes innerCalls + the appended reimbursement transfer.
  // Floor callGasLimit per sub-call: TIP-20 transfers meter high and the bundler's
  // estimate under-reports (handleOps swallows the inner OOG), so a 100k floor lets
  // the atomic batch run out of gas and revert. See services/tempo.ts.
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

  try {
    const est = await estimateGas(userOp, chainId);
    userOp.verificationGasLimit = deployed
      ? bigintMax((est.verificationGasLimit * 15n) / 10n, VERIFICATION_GAS_DEPLOYED)
      : bigintMax((est.verificationGasLimit * 15n) / 10n, TEMPO_VERIFICATION_GAS_UNDEPLOYED);
    userOp.callGasLimit = bigintMax((est.callGasLimit * 15n) / 10n, callGasFloor);
    userOp.preVerificationGas = est.preVerificationGas + 10_000n;
  } catch (err) {
    console.error('[Tempo] Gas estimation failed, using defaults:', err instanceof Error ? err.message : String(err));
  }

  // Price the reimbursement off the REALISTIC gas the 0x76 will burn — NOT the padded
  // UserOp limits (callGasLimit/verificationGasLimit stay high for OOG safety, but the
  // user shouldn't pay 2–4× for that headroom). The batch runs innerCalls + 1 transfer.
  const expectedGas = tempoExpectedGas(deployed, innerCalls.length + 1);
  const reimbursement = tempoReimbursement(expectedGas, gasPrices.gasPrice, TEMPO_FEE_TOKEN_DECIMALS);
  userOp.callData = buildBatch(reimbursement);
  console.log(`[Tempo] feeToken=${feeToken} reimbursement=${reimbursement} expectedGas=${expectedGas} collector=${feeCollector}`);

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
    const existingHashMatch = errMsg.match(/\[existingHash:(0x[0-9a-fA-F]+)\]/);
    if (existingHashMatch) {
      return {
        userOpHash: existingHashMatch[1],
        waitForTxHash: () => waitForReceipt(existingHashMatch[1], chainId, 60_000),
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
function buildExecuteCallData(
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
interface MultiSendCall {
  to: string;
  value: string;
  data: Uint8Array;
}

/**
 * Encode Safe.executeUserOp(MultiSend, 0, multiSend(packedCalls), DELEGATECALL),
 * batching N sub-calls atomically. Each sub-call is a CALL (operation 0).
 */
function buildMultiSendExecuteCallData(calls: MultiSendCall[]): Uint8Array {
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

/** Encode ERC-20 transfer(address,uint256) calldata. */
function encodeErc20Transfer(to: string, amount: bigint): Uint8Array {
  return concatBytes(
    functionSelector('transfer(address,uint256)'),
    abiEncodeAddress(to),
    abiEncodeUint256Hex(amount.toString(16)),
  );
}

// ---------------------------------------------------------------------------
// InitCode
// ---------------------------------------------------------------------------

function buildInitCode(publicKeyHex: string): Uint8Array {
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

  try {
    const response = await rpcCall('eth_getCode', [address, 'latest'], chainId);
    if (response.error) {
      console.error('[UserOp] eth_getCode RPC error:', JSON.stringify(response.error));
      return true;
    }
    const result = response.result as string | undefined;
    const deployed = !!result && result !== '0x' && result.length > 2;
    console.log('[UserOp] isDeployed:', deployed, 'code length:', result?.length ?? 0);
    if (deployed) _deployedCache.set(key, true);
    return deployed;
  } catch (err) {
    console.error('[UserOp] eth_getCode failed:', err instanceof Error ? err.message : String(err));
    return true;
  }
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

  const result = response.result as string | undefined;
  const nonce = result ?? '0x0';
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
const _gasPriceCache = new Map<number, { gasPrice: bigint; at: number }>();
const GAS_PRICE_CACHE_TTL = 15_000; // 15s

/**
 * Fetch raw on-chain gas price (no markup applied).
 * The tier-based markup is applied by callers (estimateTransactionFee, sendUserOp).
 */
async function getGasPrices(
  chainId: number,
): Promise<{ gasPrice: bigint }> {
  const cached = _gasPriceCache.get(chainId);
  if (cached && Date.now() - cached.at < GAS_PRICE_CACHE_TTL) {
    return { gasPrice: cached.gasPrice };
  }

  try {
    // Fetch eth_gasPrice and latest block baseFee in parallel.
    // On some chains (Gnosis), eth_gasPrice returns an absurdly low value
    // that doesn't reflect the actual baseFee. Use max(eth_gasPrice, baseFee)
    // to ensure the UserOp gas price covers at least the base fee.
    const [gasPriceRes, blockRes] = await Promise.all([
      rpcCall('eth_gasPrice', [], chainId),
      rpcCall('eth_getBlockByNumber', ['latest', false], chainId).catch(() => null),
    ]);
    const ethGasPrice = parseHexUInt64(gasPriceRes.result as string | undefined);
    const baseFee = blockRes?.result?.baseFeePerGas
      ? parseHexUInt64(blockRes.result.baseFeePerGas as string)
      : 0n;
    const gasPrice = ethGasPrice > baseFee ? ethGasPrice : baseFee;
    if (gasPrice > 0n) {
      console.log(`[UserOp] Gas: ethGasPrice=${ethGasPrice} baseFee=${baseFee} using=${gasPrice}`);
      _gasPriceCache.set(chainId, { gasPrice, at: Date.now() });
      return { gasPrice };
    }
  } catch {
    // Use defaults
  }

  const fallback = { gasPrice: 5_000_000_000n }; // 5 gwei
  _gasPriceCache.set(chainId, { ...fallback, at: Date.now() });
  return fallback;
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
  let chainGasPrice: bigint;
  try {
    const [r, gp] = await Promise.all([
      rpcCall('pimlico_getUserOperationGasPrice', [], chainId),
      getGasPrices(chainId),
    ]);
    resp = r;
    chainGasPrice = gp.gasPrice;
  } catch (err) {
    console.log(
      '[Gas] Bundler gas-price quote unavailable, using local estimate:',
      err instanceof Error ? err.message : String(err),
    );
    return null;
  }

  const t = resp.result?.[tier];
  if (resp.error || !t?.maxFeePerGas) return null;

  const maxFeePerGas = parseHexUInt64(t.maxFeePerGas as string);
  const maxPriorityFeePerGas = parseHexUInt64((t.maxPriorityFeePerGas ?? t.maxFeePerGas) as string);
  // Vela extension fields; derive them if a generic bundler omits them.
  const networkFeePerGas = t.networkFeePerGas
    ? parseHexUInt64(t.networkFeePerGas as string)
    : chainGasPrice;
  const relayerFeePerGas = t.relayerFeePerGas
    ? parseHexUInt64(t.relayerFeePerGas as string)
    : maxFeePerGas > networkFeePerGas
      ? maxFeePerGas - networkFeePerGas
      : 0n;

  // Client-side hard cap — refuse, don't silently fall back.
  if (chainGasPrice > 0n && maxFeePerGas > chainGasPrice * MAX_QUOTE_VS_CHAIN_MULTIPLE) {
    console.warn(
      `[Gas] Bundler quote ${maxFeePerGas} exceeds ${MAX_QUOTE_VS_CHAIN_MULTIPLE}× chain price ${chainGasPrice} — refusing.`,
    );
    throw new GasQuoteTooHighError(maxFeePerGas, chainGasPrice);
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
  console.log('[UserOp] Submitting:', JSON.stringify({
    sender: dict.sender,
    nonce: dict.nonce,
    factory: dict.factory ?? '(none)',
    factoryDataLen: dict.factoryData?.length ?? 0,
    callDataLen: dict.callData?.length ?? 0,
    signatureLen: dict.signature?.length ?? 0,
    verificationGasLimit: dict.verificationGasLimit,
    callGasLimit: dict.callGasLimit,
    maxFeePerGas: dict.maxFeePerGas,
  }));

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
  let interval = 1000; // Start fast (1s), then back off

  // Track whether we ever got a clean "not ready yet" answer vs. only ever hit
  // RPC/bundler errors. The distinction drives the final message: "submitted but
  // not confirmed" (the bundler answered, op just hasn't landed) is very different
  // from "bundler unreachable, status unknown" (we never got an answer at all).
  let sawCleanResponse = false;
  let rpcFailures = 0;

  while (Date.now() - start < timeout) {
    // Caller cancelled (e.g. the send screen unmounted) — stop polling.
    if (signal?.aborted) throw makeAbortError();

    try {
      const response = await rpcCall(
        'eth_getUserOperationReceipt',
        [userOpHash],
        chainId,
      );

      if (response.error) {
        // The bundler responded with a JSON-RPC error (transient server issue).
        // Don't abandon the op — it may still land; keep polling.
        rpcFailures++;
      } else {
        sawCleanResponse = true;
        const result = response.result as
          | { success?: boolean; receipt?: { transactionHash?: string } }
          | undefined;
        if (result?.receipt?.transactionHash) {
          // Check if the UserOp was marked as failed (e.g. tx dropped from mempool)
          if (result.success === false) {
            throw new Error('Transaction was dropped from the network. Try again with a higher gas price.');
          }
          return result.receipt.transactionHash;
        }
      }
    } catch (err) {
      // A genuine drop (success === false) is final — rethrow it.
      if (err instanceof Error && /dropped from the network/.test(err.message)) throw err;
      // All bundler endpoints failed this round (network blip). Previously this
      // aborted the whole wait; instead keep polling, since the op may still land.
      rpcFailures++;
    }

    await sleep(interval);
    // Adaptive backoff: 1s → 1.5s → 2s → 2.5s → 3s (cap)
    interval = Math.min(interval + 500, 3000);
  }

  const shortOp = `${userOpHash.slice(0, 10)}…`;
  if (!sawCleanResponse && rpcFailures > 0) {
    // We never reached the bundler — the op's fate is genuinely unknown, not a
    // confirmed pending. Mark it as such so the caller can reconcile/retry later.
    throw new Error(
      `Couldn't reach the bundler to confirm transaction ${shortOp}; its status is unknown. ` +
      `Check the explorer in a few minutes before retrying.`,
    );
  }
  // Submitted and accepted by the bundler, but no on-chain receipt in time. The op
  // is NOT lost — it may still land (or the bundler's gas account couldn't fund the
  // bundle). Say so, and surface the hash, instead of implying outright failure.
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

function parseHexUInt64(value: string | undefined): bigint {
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
