/**
 * Builds, signs, and submits ERC-4337 UserOperations for Safe wallets.
 * TypeScript port of SafeTransactionService.swift.
 */

import {
  keccak256,
  abiEncodeAddress,
  abiEncodeUint256,
  abiEncodeUint256Hex,
  abiEncodeBytes32,
  functionSelector,
} from './eth-crypto';

import { toHex, fromHex, concatBytes, stripHexPrefix } from './hex';

import {
  SAFE_SINGLETON,
  SAFE_PROXY_FACTORY,
  ENTRY_POINT,
  SAFE_4337_MODULE,
  WEBAUTHN_SIGNER,
  SAFE_MODULE_SETUP,
  parsePublicKey,
  encodeSetupData,
  calculateSaltNonce,
} from './safe-address';

import { rpcCall } from './rpc-adapter';
import { derSignatureToRaw } from './attestation-parser';

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const VERIFICATION_GAS_DEPLOYED = 300_000n;
const VERIFICATION_GAS_UNDEPLOYED = 600_000n;
const CALL_GAS_LIMIT = 500_000n;  // 500k — swap/complex calls need more
const PRE_VERIFICATION_GAS = 80_000n;

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
): Promise<TransactionResult> {
  const callData = buildExecuteCallData(to, valueWei, new Uint8Array(0));
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn);
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
): Promise<TransactionResult> {
  const transferSelector = functionSelector('transfer(address,uint256)');
  const transferData = concatBytes(
    transferSelector,
    abiEncodeAddress(to),
    abiEncodeUint256Hex(amountWei),
  );

  const callData = buildExecuteCallData(tokenAddress, '0', transferData);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn);
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
): Promise<TransactionResult> {
  const callData = buildExecuteCallData(to, valueWei, data);
  return sendUserOp(from, callData, chainId, publicKeyHex, signFn);
}

// ---------------------------------------------------------------------------
// Gas Estimation (public)
// ---------------------------------------------------------------------------

/** Estimate the total gas fee in wei for a transaction. */
export async function estimateTransactionFee(
  from: string,
  chainId: number,
): Promise<{ totalWei: bigint; maxFeePerGas: bigint; totalGas: bigint }> {
  const deployed = await isDeployed(from, chainId);
  const { maxFee } = await getGasPrices(chainId);

  const verificationGas = deployed
    ? VERIFICATION_GAS_DEPLOYED
    : VERIFICATION_GAS_UNDEPLOYED;

  const totalGas = verificationGas + CALL_GAS_LIMIT + PRE_VERIFICATION_GAS;
  const totalWei = totalGas * maxFee;

  return { totalWei, maxFeePerGas: maxFee, totalGas };
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
): Promise<TransactionResult> {
  // 0. Pre-check: verify critical contracts exist on this chain
  await verifyChainReady(chainId);

  // 1. Check if deployed
  const deployed = await isDeployed(safeAddress, chainId);

  // 2. Build initCode if needed
  const initCode: Uint8Array = deployed
    ? new Uint8Array(0)
    : buildInitCode(publicKeyHex);

  // 3. Get nonce (0 for undeployed wallets)
  const nonce: string = deployed
    ? await getNonce(safeAddress, chainId)
    : '0x0';

  // 4. Get gas prices
  const { maxFee, maxPriority } = await getGasPrices(chainId);

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
  try {
    const estimated = await estimateGas(userOp, chainId);
    console.log('[UserOp] Gas estimate:', {
      verificationGasLimit: estimated.verificationGasLimit.toString(),
      callGasLimit: estimated.callGasLimit.toString(),
      preVerificationGas: estimated.preVerificationGas.toString(),
    });
    // Trust the bundler's estimate with a safety margin.
    // For undeployed wallets, the verification phase includes CREATE2 + Safe.setup +
    // P256 signature validation — needs significantly more gas than the estimation
    // reports (estimation uses dummy sig which skips P256 verify).
    const estVerification = (estimated.verificationGasLimit * 15n) / 10n;
    userOp.verificationGasLimit = deployed
      ? estVerification
      : bigintMax(estVerification, 450_000n); // floor for first-time deploy (under bundler's 500k MAX)
    userOp.callGasLimit = bigintMax(
      (estimated.callGasLimit * 15n) / 10n,
      100_000n, // reasonable floor
    );
    userOp.preVerificationGas = estimated.preVerificationGas + 10_000n;
  } catch (err) {
    console.error('[UserOp] Gas estimation failed, using defaults:', err instanceof Error ? err.message : String(err));
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
  const userOpHash = await submitUserOp(userOp, chainId);

  // 12. Wait for receipt
  const txHash = await waitForReceipt(userOpHash, chainId);

  return { userOpHash, txHash };
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
function extractClientDataFields(clientDataJSON: Uint8Array): string {
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
 * Build contract signature for SafeWebAuthnSharedSigner.
 *
 * Format: validAfter(6) + validUntil(6) + r(32) + s(32) + v(1) + dataLength(32) + dynamicData
 * Where r = signer address padded, s = 65 (offset), v = 0x00 (contract sig type)
 * dynamicData = abi.encode(bytes authenticatorData, string clientDataFields, uint256 sigR, uint256 sigS)
 */
function buildUserOpSignature(
  authenticatorData: Uint8Array,
  clientDataFields: string,
  sigR: Uint8Array,
  sigS: Uint8Array,
): Uint8Array {
  // Validity window: validAfter(6) + validUntil(6) = 12 bytes of zeros
  const validityPadding = new Uint8Array(12);

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
    validityPadding,
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

async function isDeployed(
  address: string,
  chainId: number,
): Promise<boolean> {
  try {
    const response = await rpcCall('eth_getCode', [address, 'latest'], chainId);
    if (response.error) {
      console.error('[UserOp] eth_getCode RPC error:', JSON.stringify(response.error));
      // On error, assume deployed to avoid sending initCode for existing contracts
      // This is safer than assuming undeployed (which generates initCode that fails)
      return true;
    }
    const result = response.result as string | undefined;
    const deployed = !!result && result !== '0x' && result.length > 2;
    console.log('[UserOp] isDeployed:', deployed, 'code length:', result?.length ?? 0);
    return deployed;
  } catch (err) {
    console.error('[UserOp] eth_getCode failed:', err instanceof Error ? err.message : String(err));
    // Fail safe: assume deployed
    return true;
  }
}

async function getNonce(
  safeAddress: string,
  chainId: number,
): Promise<string> {
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
  return result ?? '0x0';
}

async function getGasPrices(
  chainId: number,
): Promise<{ maxFee: bigint; maxPriority: bigint }> {
  // EIP-1559: try eth_maxPriorityFeePerGas + eth_gasPrice
  try {
    const [gasPriceRes] = await Promise.all([
      rpcCall('eth_gasPrice', [], chainId),
    ]);

    const gasPrice = parseHexUInt64(gasPriceRes.result as string | undefined);
    if (gasPrice > 0n) {
      // ERC-4337 effectivePrice = min(maxFeePerGas, baseFee + maxPriorityFeePerGas).
      // Vela bundler requires: effectivePrice >= (baseFee + bundlerTip) × (1 + margin).
      // With bundlerTip=1.5gwei, margin=20%:
      //   required = (baseFee + 1.5gwei) × 1.2 = baseFee × 1.2 + 1.8gwei
      //
      // Set maxPriority = maxFee so effective = min(maxFee, baseFee+maxFee) = maxFee.
      // Formula: maxFee = gasPrice × 2 + 4gwei (covers tip + margin + buffer at any gas price).
      //
      // Verification:
      //   baseFee=0.5gwei → maxFee=5gwei > required 2.4gwei ✓
      //   baseFee=1gwei   → maxFee=6gwei > required 3.0gwei ✓
      //   baseFee=10gwei  → maxFee=24gwei > required 13.8gwei ✓
      //   baseFee=100gwei → maxFee=204gwei > required 121.8gwei ✓
      const BUNDLER_TIP_BUFFER = 4_000_000_000n; // 4 gwei — covers tip(1.5) × margin(1.2) + headroom
      const maxFee = gasPrice * 2n + BUNDLER_TIP_BUFFER;
      console.log(`[UserOp] Gas price: ${gasPrice} → maxFee=${maxFee} maxPriority=${maxFee}`);
      return {
        maxFee,
        maxPriority: maxFee,
      };
    }
  } catch {
    // Use defaults
  }

  return {
    maxFee: 50_000_000_000n,
    maxPriority: 25_000_000_000n,
  };
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
): Promise<string> {
  const dict = userOpToDict(userOp);
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

  const response = await rpcCall(
    'eth_sendUserOperation',
    [dict, ENTRY_POINT],
    chainId,
  );

  const result = response.result as string | undefined;
  if (!result) {
    const error = response.error;
    throw new Error(parseBundlerError(error));
  }

  return result;
}

async function waitForReceipt(
  userOpHash: string,
  chainId: number,
  timeout: number = 120_000,
): Promise<string> {
  const start = Date.now();

  while (Date.now() - start < timeout) {
    const response = await rpcCall(
      'eth_getUserOperationReceipt',
      [userOpHash],
      chainId,
    );

    const result = response.result as
      | { receipt?: { transactionHash?: string } }
      | undefined;
    if (result?.receipt?.transactionHash) {
      return result.receipt.transactionHash;
    }

    await sleep(1500);
  }

  throw new Error('Transaction timed out waiting for confirmation');
}

// ---------------------------------------------------------------------------
// UserOp Serialization
// ---------------------------------------------------------------------------

/**
 * Convert UserOperation to JSON-RPC format.
 * ERC-4337 v0.7 uses individual fields + factory/factoryData split.
 */
function userOpToDict(userOp: UserOperation): Record<string, string> {
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

  // Common Pimlico / bundler errors
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
