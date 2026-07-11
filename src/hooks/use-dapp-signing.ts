/**
 * Shared dApp request signing logic.
 * Used by both BLE (native) and WalletConnect (web) connect screens.
 */
import type { Account } from '@/models/types';
import * as Passkey from '@/modules/passkey';
import { derSignatureToRaw } from '@/services/attestation-parser';
import { keccak256 } from '@/services/eth-crypto';
import { hashTypedData, type TypedData } from '@/services/eip712';
import { fromHex, stripHexPrefix, toHex } from '@/services/hex';
import * as PublicKeyIndex from '@/services/public-key-index';
import { rpcCall } from '@/services/rpc-adapter';
import { sendContractCall, sendNative, buildEip1271Signature, extractClientDataFields, computeSafeMessageHash } from '@/services/safe-transaction';
import { enforceNoUnlimited } from '@/services/approval-guard';
import { findAccountByCredentialId } from '@/services/storage';
import { SAFE_PROXY_RUNTIME_CODE } from '@/services/safe-address';
import { getAllNetworksSync } from '@/models/network';

export interface DAppRequest {
  id: string;
  method: string;
  params: any[];
  origin?: string;
}

// ── Chain ID resolution & validation ──────────────────────────────────────

const UNSUPPORTED_CHAIN_ERROR_CODE = 4902; // EIP-3085: unrecognized chain ID
const UNSUPPORTED_CAPABILITY_ERROR_CODE = 5700; // EIP-5792: unsupported non-optional capability

// ── Submitted UserOp → chain tracking ──────────────────────────────────────
//
// Two readers need the chain a userOp was submitted on, even after the wallet
// has since switched networks:
//   • wallet_getCallsStatus — looks up the EIP-5792 batch id (== userOpHash).
//   • eth_getTransactionReceipt / eth_getTransactionByHash — a dApp that polls a
//     receipt by a hash we handed back (a wallet_sendCalls batch id, or an
//     eth_sendTransaction userOpHash) matches nothing on the public RPC and would
//     poll forever; translate it to the real bundle tx via the bundler, on the
//     ORIGINAL chain, not the current one.
// Hashes are opaque, so we remember hash → chainId at submit time.
const userOpChainIds = new Map<string, number>();
const MAX_TRACKED_USEROPS = 200;

function rememberUserOpChain(userOpHash: string, chainId: number): void {
  const key = userOpHash.toLowerCase();
  // Bound memory: drop the oldest entry once we exceed the cap (Map keeps
  // insertion order, so the first key is the oldest).
  if (!userOpChainIds.has(key) && userOpChainIds.size >= MAX_TRACKED_USEROPS) {
    const oldest = userOpChainIds.keys().next().value;
    if (oldest !== undefined) userOpChainIds.delete(oldest);
  }
  userOpChainIds.set(key, chainId);
}

/** The chain a userOp/batch was submitted on, or undefined if we didn't issue it. */
function resolveUserOpChain(userOpHash: string | undefined): number | undefined {
  if (!userOpHash) return undefined;
  return userOpChainIds.get(userOpHash.toLowerCase());
}

/**
 * EIP-5792: request capabilities are assumed REQUIRED unless explicitly marked
 * `{ optional: true }`. This wallet supports no request-level capabilities, so a
 * required capability must be rejected with code 5700 ("unsupported non-optional
 * capability") rather than silently ignored. Optional capabilities are dropped.
 */
function assertNoRequiredCapabilities(payload: {
  capabilities?: Record<string, { optional?: boolean }>;
  calls?: Array<{ capabilities?: Record<string, { optional?: boolean }> }>;
}): void {
  const buckets = [payload.capabilities, ...(payload.calls ?? []).map(c => c.capabilities)];
  const required = new Set<string>();
  for (const caps of buckets) {
    if (!caps) continue;
    for (const [name, value] of Object.entries(caps)) {
      if (value?.optional !== true) required.add(name);
    }
  }
  if (required.size > 0) {
    throw Object.assign(
      new Error(`Unsupported non-optional capabilities: ${[...required].join(', ')}`),
      { code: UNSUPPORTED_CAPABILITY_ERROR_CODE },
    );
  }
}

/**
 * Resolve the effective chain ID from request context.
 * Priority: request-embedded chainId > fallback (component-level chainId).
 */
export function resolveChainId(fallback: number, ...candidates: (string | number | undefined | null)[]): number {
  for (const c of candidates) {
    if (c == null) continue;
    const n = typeof c === 'string'
      ? (c.startsWith('0x') ? parseInt(c, 16) : parseInt(c, 10))
      : c;
    if (!isNaN(n) && n > 0) return n;
  }
  return fallback;
}

/**
 * Assert the wallet supports the given chain ID.
 * Throws with EIP-3085 error code 4902 if unsupported.
 */
export function assertChainSupported(chainId: number): void {
  const supported = getAllNetworksSync().some(n => n.chainId === chainId);
  if (!supported) {
    throw Object.assign(
      new Error(`Unsupported chain: ${chainId}. Add this network in wallet settings.`),
      { code: UNSUPPORTED_CHAIN_ERROR_CODE },
    );
  }
}

/**
 * Extract an embedded chain ID from a dApp request's params, if present.
 * Returns undefined when the request carries no chain hint.
 */
export function extractRequestChainId(method: string, params: any[]): number | undefined {
  try {
    if (method.includes('signTypedData')) {
      const raw = params[1] ?? params[0];
      const typed = typeof raw === 'string' ? JSON.parse(raw) : raw;
      const cid = typed?.domain?.chainId;
      if (cid != null) {
        const n = typeof cid === 'string'
          ? (cid.startsWith('0x') ? parseInt(cid, 16) : parseInt(cid, 10))
          : Number(cid);
        if (!isNaN(n) && n > 0) return n;
      }
    } else if (method === 'eth_sendTransaction') {
      // Same coercion as the submit-side resolveChainId (string hex/dec OR number) —
      // if this pre-check misses a numeric chainId the modal estimates/displays on
      // the wallet's current chain while the submit goes to the tx's chain.
      const tx = params[0] as { chainId?: string | number } | undefined;
      const n = resolveChainId(0, tx?.chainId);
      if (n > 0) return n;
    } else if (method === 'wallet_sendCalls') {
      const payload = params[0] as { chainId?: string | number } | undefined;
      const n = resolveChainId(0, payload?.chainId);
      if (n > 0) return n;
    }
  } catch { /* malformed params — ignore */ }
  return undefined;
}

/**
 * Build a full Safe-compatible EIP-1271 contract signature from a WebAuthn assertion.
 *
 * This encodes the signature in the format Safe's isValidSignature expects:
 * validAfter(6) + validUntil(6) + signerAddr(32) + offset(32) + v=0x00(1) + dataLen(32) + dynamicData
 * where dynamicData = abi.encode(authenticatorData, clientDataFields, r, s)
 */
function buildContractSignature(assertion: Passkey.PasskeyAssertionResult): string {
  const rawSig = derSignatureToRaw(fromHex(assertion.signatureHex));
  if (!rawSig) throw new Error('Failed to convert signature');

  const authenticatorData = fromHex(assertion.authenticatorDataHex);
  const clientDataJSON = fromHex(assertion.clientDataJSONHex);
  const clientDataFields = extractClientDataFields(clientDataJSON);
  const sigR = rawSig.slice(0, 32);
  const sigS = rawSig.slice(32);

  const sig = buildEip1271Signature(authenticatorData, clientDataFields, sigR, sigS);
  return '0x' + toHex(sig);
}

/**
 * Handle a personal_sign request.
 * Returns a full Safe contract signature (EIP-1271 compatible).
 */
export async function handlePersonalSign(
  request: DAppRequest,
  account: Account,
  safeAddress: string,
  chainId: number,
): Promise<string> {
  // personal_sign has no embedded chainId — use the fallback
  assertChainSupported(chainId);

  const hexMsg = request.params[0] as string;
  const clean = stripHexPrefix(hexMsg);
  const msgBytes = fromHex(clean);

  const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
  const combined = new Uint8Array(prefix.length + msgBytes.length);
  combined.set(prefix);
  combined.set(msgBytes, prefix.length);
  const originalHash = keccak256(combined);

  const safeHash = computeSafeMessageHash(originalHash, chainId, safeAddress);
  const assertion = await Passkey.sign(toHex(safeHash), account.id);
  return buildContractSignature(assertion);
}

/**
 * Handle an eth_signTypedData_v4 request.
 * Returns a full Safe contract signature (EIP-1271 compatible).
 */
export async function handleSignTypedData(
  request: DAppRequest,
  account: Account,
  safeAddress: string,
  chainId: number,
): Promise<string> {
  const typedDataRaw = request.params[1] ?? request.params[0];
  const typedData: TypedData = typeof typedDataRaw === 'string'
    ? JSON.parse(typedDataRaw)
    : typedDataRaw;

  const effectiveChainId = resolveChainId(chainId, typedData.domain?.chainId);
  assertChainSupported(effectiveChainId);

  const originalHash = hashTypedData(typedData);
  const safeHash = computeSafeMessageHash(originalHash, effectiveChainId, safeAddress);
  const assertion = await Passkey.sign(toHex(safeHash), account.id);
  return buildContractSignature(assertion);
}

/**
 * Handle an eth_sendTransaction request (full ERC-4337 UserOp).
 */
export async function handleSendTransaction(
  request: DAppRequest,
  account: Account,
  safeAddress: string,
  chainId: number,
  maxFeeOverride?: bigint,
  onSubmitted?: (userOpHash: string) => void,
): Promise<string> {
  const txDict = request.params[0] as Record<string, string>;
  const effectiveChainId = resolveChainId(chainId, txDict.chainId);
  assertChainSupported(effectiveChainId);

  const to = txDict.to ?? '';
  const valueHex = txDict.value ?? '0x0';
  const dataHex = txDict.data ?? '0x';

  // Get public key
  let publicKeyHex: string | undefined;
  const stored = await findAccountByCredentialId(account.id);
  publicKeyHex = stored?.publicKeyHex;

  if (!publicKeyHex) {
    const record = await PublicKeyIndex.queryRecord(Passkey.getRelyingPartyId(), account.id);
    publicKeyHex = record.publicKey;
  }

  if (!publicKeyHex) throw new Error('Public key not found');

  const signFn = async (challenge: Uint8Array) => {
    const assertion = await Passkey.sign(toHex(challenge), account.id);

    const { verifySafeWebAuthn } = await import('@/services/webauthn-verify');
    const compat = verifySafeWebAuthn(assertion);
    if (!compat.ok) {
      throw new Error(
        'Your device\'s identity provider is not compatible with Vela Wallet. ' +
        'Please switch to Google Password Manager.\n\n' + compat.reason,
      );
    }

    return {
      signature: fromHex(assertion.signatureHex),
      authenticatorData: fromHex(assertion.authenticatorDataHex),
      clientDataJSON: fromHex(assertion.clientDataJSONHex),
    };
  };

  const valueClean = stripHexPrefix(valueHex) || '0';

  let txResult;
  if (dataHex === '0x' || dataHex === '') {
    txResult = await sendNative(safeAddress, to, valueClean, effectiveChainId, publicKeyHex, signFn, maxFeeOverride);
  } else {
    const txData = fromHex(stripHexPrefix(dataHex));
    txResult = await sendContractCall(safeAddress, to, valueClean, txData, effectiveChainId, publicKeyHex, signFn, maxFeeOverride);
  }

  // Op is signed + accepted by the bundler here; the receipt wait can take a while.
  // Remember the chain so a later eth_getTransactionReceipt(userOpHash) poll can be
  // translated to the real bundle tx on the ORIGINAL chain (not the current one).
  rememberUserOpChain(txResult.userOpHash, effectiveChainId);
  // Report the hash so the UI can show "submitted, waiting" instead of a blank spin.
  onSubmitted?.(txResult.userOpHash);
  return await txResult.waitForTxHash();
}

/**
 * Handle a generic sign request.
 * Returns a full Safe contract signature (EIP-1271 compatible).
 */
export async function handleGenericSign(
  request: DAppRequest,
  account: Account,
  safeAddress: string,
  chainId: number,
): Promise<string> {
  assertChainSupported(chainId);

  const jsonStr = JSON.stringify(request.params);
  const jsonBytes = new TextEncoder().encode(jsonStr);
  const originalHash = keccak256(jsonBytes);

  const safeHash = computeSafeMessageHash(originalHash, chainId, safeAddress);
  const assertion = await Passkey.sign(toHex(safeHash), account.id);
  return buildContractSignature(assertion);
}

/**
 * Route a request to the appropriate handler.
 * Returns the result to send back to the dApp.
 */
export async function handleDAppRequest(
  request: DAppRequest,
  account: Account,
  safeAddress: string,
  chainId: number,
  maxFeeOverride?: bigint,
  onSubmitted?: (userOpHash: string) => void,
): Promise<any> {
  const { method } = request;

  // Final, descriptor-independent safety net: never sign or submit a request that
  // would grant an unbounded allowance. The UI caps approvals up-front, but this
  // guard catches anything that bypassed it (incl. shapes no descriptor decodes).
  enforceNoUnlimited(method, request.params);

  if (method === 'eth_sendTransaction') {
    return handleSendTransaction(request, account, safeAddress, chainId, maxFeeOverride, onSubmitted);
  } else if (method === 'wallet_sendCalls') {
    return handleSendCalls(request, account, safeAddress, chainId);
  } else if (method === 'personal_sign') {
    return handlePersonalSign(request, account, safeAddress, chainId);
  } else if (method.includes('signTypedData')) {
    return handleSignTypedData(request, account, safeAddress, chainId);
  } else {
    return handleGenericSign(request, account, safeAddress, chainId);
  }
}

/**
 * Handle a wallet_sendCalls request (EIP-5792 batched atomic calls).
 * Executes multiple calls as a single UserOp via the Safe account.
 */
export async function handleSendCalls(
  request: DAppRequest,
  account: Account,
  safeAddress: string,
  chainId: number,
): Promise<string> {
  const payload = request.params[0] as {
    calls: Array<{ to: string; value?: string; data?: string; capabilities?: Record<string, { optional?: boolean }> }>;
    chainId?: string;  // hex chain ID from dApp
    from?: string;
    capabilities?: Record<string, { optional?: boolean }>;
  };

  // EIP-5792: reject any required capability we don't support before touching the
  // wallet (so the dApp gets a clean 5700 rather than a silently-dropped feature).
  assertNoRequiredCapabilities(payload);

  const effectiveChainId = resolveChainId(chainId, payload.chainId);
  assertChainSupported(effectiveChainId);

  const calls = payload.calls ?? [];
  if (calls.length === 0) throw new Error('No calls provided');

  // A batch must not smuggle an unbounded approval past the per-tx guard — check
  // every leg as if it were a standalone transaction.
  for (const c of calls) {
    enforceNoUnlimited('eth_sendTransaction', [{ to: c.to, data: c.data, value: c.value }]);
  }

  // Get public key
  let publicKeyHex: string | undefined;
  const stored = await findAccountByCredentialId(account.id);
  publicKeyHex = stored?.publicKeyHex;

  if (!publicKeyHex) {
    const record = await PublicKeyIndex.queryRecord(Passkey.getRelyingPartyId(), account.id);
    publicKeyHex = record.publicKey;
  }

  if (!publicKeyHex) throw new Error('Public key not found');

  const signFn = async (challenge: Uint8Array) => {
    const assertion = await Passkey.sign(toHex(challenge), account.id);

    const { verifySafeWebAuthn } = await import('@/services/webauthn-verify');
    const compat = verifySafeWebAuthn(assertion);
    if (!compat.ok) {
      throw new Error(
        'Your device\'s identity provider is not compatible with Vela Wallet. ' +
        'Please switch to Google Password Manager.\n\n' + compat.reason,
      );
    }

    return {
      signature: fromHex(assertion.signatureHex),
      authenticatorData: fromHex(assertion.authenticatorDataHex),
      clientDataJSON: fromHex(assertion.clientDataJSONHex),
    };
  };

  // Single call → use existing send logic
  if (calls.length === 1) {
    const call = calls[0];
    const to = call.to ?? '';
    const valueHex = call.value ?? '0x0';
    const dataHex = call.data ?? '0x';
    const valueClean = stripHexPrefix(valueHex) || '0';

    let txResult;
    if (dataHex === '0x' || dataHex === '') {
      txResult = await sendNative(safeAddress, to, valueClean, effectiveChainId, publicKeyHex, signFn);
    } else {
      const txData = fromHex(stripHexPrefix(dataHex));
      txResult = await sendContractCall(safeAddress, to, valueClean, txData, effectiveChainId, publicKeyHex, signFn);
    }
    rememberUserOpChain(txResult.userOpHash, effectiveChainId);
    return txResult.userOpHash;
  }

  // Multiple calls → batch via Safe multiSend
  const { sendBatchCalls } = await import('@/services/safe-transaction');
  const txResult = await sendBatchCalls(
    safeAddress,
    calls.map(c => ({
      to: c.to,
      value: stripHexPrefix(c.value ?? '0x0') || '0',
      data: c.data ?? '0x',
    })),
    effectiveChainId,
    publicKeyHex,
    signFn,
  );
  rememberUserOpChain(txResult.userOpHash, effectiveChainId);
  return txResult.userOpHash;
}

/**
 * Check if a method is a signing method that needs user approval.
 */
export function isSigningMethod(method: string): boolean {
  return method === 'eth_sendTransaction' ||
    method === 'wallet_sendCalls' ||
    method === 'personal_sign' ||
    method === 'eth_sign' ||
    method.includes('signTypedData');
}

/**
 * Read-only methods answered instantly from local wallet state (no network). The
 * dispatch layer skips the concurrency gate for these so a flood of cheap local
 * queries never queues behind network-bound reads.
 */
export const INSTANT_READONLY_METHODS = new Set([
  'eth_accounts',
  'eth_requestAccounts',
  'eth_chainId',
  'net_version',
  'wallet_getPermissions',
  'wallet_requestPermissions',
  'wallet_addEthereumChain',
]);

/**
 * Cache of the wallet's own deployed Safe code, keyed by `${chainId}|${address}`.
 * A deployed Safe's code is immutable, so once observed we answer eth_getCode for
 * our own address without re-querying the RPC (defense-in-depth for older SDKs that
 * still forward eth_getCode for the wallet's own address). Keyed by address as well
 * as chain so switching accounts never serves another account's code. Undeployed
 * accounts are intentionally NOT cached — they may deploy at any time.
 */
const deployedSelfCode = new Map<string, string>();

/** Reset module-level read-only caches. Tests only. */
export function __resetReadOnlyCache(): void {
  deployedSelfCode.clear();
}

/**
 * Handle a read-only RPC method. Returns the result or null if not handled.
 */
export async function handleReadOnlyRPC(
  method: string,
  params: any[],
  address: string,
  chainId: number,
): Promise<{ handled: true; result: any } | { handled: false }> {
  // Counterfactual smart-account override: when a dApp queries eth_getCode for
  // THIS wallet's own address and the Safe proxy isn't deployed yet (real code
  // is empty), return the Safe proxy runtime bytecode so the dApp detects a
  // smart contract wallet (EIP-1271) instead of an EOA. Other addresses pass
  // through to the normal RPC forward below.
  if (method === 'eth_getCode') {
    const target = (params?.[0] as string | undefined)?.toLowerCase();
    if (target && address && target === address.toLowerCase()) {
      const cacheKey = `${chainId}|${target}`;
      const cached = deployedSelfCode.get(cacheKey);
      if (cached) return { handled: true, result: cached };
      try {
        const res = await rpcCall('eth_getCode', params ?? [target, 'latest'], chainId);
        const code = res.result as string | undefined;
        if (code && code !== '0x' && code.length > 2) {
          deployedSelfCode.set(cacheKey, code);
          return { handled: true, result: code };
        }
      } catch { /* fall through to runtime code */ }
      return { handled: true, result: SAFE_PROXY_RUNTIME_CODE };
    }
  }

  switch (method) {
    case 'eth_accounts':
    case 'eth_requestAccounts':
      return { handled: true, result: [address] };
    case 'eth_chainId':
      return { handled: true, result: '0x' + chainId.toString(16) };
    case 'net_version':
      return { handled: true, result: String(chainId) };
    // EIP-2255 compatibility shims — not protocol methods (the WalletPair
    // pairing IS the authorization), but kept so dApps that call them don't
    // break. Not advertised in capabilities.methods.
    case 'wallet_getPermissions':
    case 'wallet_requestPermissions':
      return { handled: true, result: [{ parentCapability: 'eth_accounts' }] };
    case 'wallet_addEthereumChain':
      return { handled: true, result: null };
    // EIP-5792: poll the status of a prior wallet_sendCalls batch by its ID
    // (the userOpHash returned from handleSendCalls).
    case 'wallet_getCallsStatus': {
      const id = params?.[0] as string | undefined;
      // Resolve the chain the batch was submitted on (the wallet may have since
      // switched networks); fall back to the current chain for unknown ids.
      const batchChain = resolveUserOpChain(id) ?? chainId;
      const hexChain = '0x' + batchChain.toString(16);
      const pending = { version: '2.0.0', id: id ?? '0x', chainId: hexChain, status: 100, atomic: true, receipts: [] as unknown[] };
      if (!id) return { handled: true, result: pending };
      try {
        // ID is a userOpHash — query the bundler for the UserOp receipt
        const res = await rpcCall('eth_getUserOperationReceipt', [id], batchChain);
        const opReceipt = res.result as {
          success?: boolean;
          receipt?: {
            status?: string; logs?: unknown[]; blockHash?: string;
            blockNumber?: string; gasUsed?: string; transactionHash?: string;
          };
        } | null;
        if (!opReceipt?.receipt) return { handled: true, result: pending };
        const receipt = opReceipt.receipt;
        const ok = opReceipt.success !== false && receipt.status === '0x1';
        return { handled: true, result: {
          version: '2.0.0',
          id,
          chainId: hexChain,
          status: ok ? 200 : 500,
          atomic: true,
          receipts: [{
            logs: receipt.logs ?? [],
            status: receipt.status ?? '0x0',
            blockHash: receipt.blockHash,
            blockNumber: receipt.blockNumber,
            gasUsed: receipt.gasUsed,
            transactionHash: receipt.transactionHash ?? id,
          }],
        } };
      } catch {
        return { handled: true, result: pending };
      }
    }
    // A dApp polling a receipt by a hash WE handed back (a wallet_sendCalls batch
    // id, or a userOpHash) would otherwise hit the public RPC — where that hash
    // matches no on-chain tx — and poll forever ("submitted but never confirms").
    // Translate our hashes to the real bundle tx via the bundler, on the chain the
    // op was submitted on, and forward the AUTHENTIC on-chain receipt (never a
    // synthesized one) so the dApp gets a complete, spec-shaped result.
    case 'eth_getTransactionReceipt':
    case 'eth_getTransactionByHash': {
      const hash = params?.[0] as string | undefined;
      const opChain = resolveUserOpChain(hash);
      if (hash && opChain !== undefined) {
        try {
          const opRes = await rpcCall('eth_getUserOperationReceipt', [hash], opChain);
          const realHash = (opRes.result as { receipt?: { transactionHash?: string } } | null)
            ?.receipt?.transactionHash;
          // Not landed yet (or a transient bundler blip) — return null so the dApp
          // keeps polling, exactly as it would for an unmined tx.
          if (!realHash) return { handled: true, result: null };
          const real = await rpcCall(method, [realHash], opChain);
          return { handled: true, result: real.result ?? null };
        } catch {
          return { handled: true, result: null };
        }
      }
      // Not one of our hashes — forward untouched on the current chain.
      try {
        const res = await rpcCall(method, params ?? [], chainId);
        return { handled: true, result: res.result ?? null };
      } catch {
        return { handled: false };
      }
    }
    default:
      // Try forwarding as RPC query
      if (!isSigningMethod(method)) {
        try {
          const res = await rpcCall(method, params ?? [], chainId);
          return { handled: true, result: res.result ?? null };
        } catch {
          return { handled: false };
        }
      }
      return { handled: false };
  }
}
