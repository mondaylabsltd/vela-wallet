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
import { findAccountByCredentialId } from '@/services/storage';

export interface DAppRequest {
  id: string;
  method: string;
  params: any[];
  origin?: string;
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
  const hexMsg = request.params[0] as string;
  const clean = stripHexPrefix(hexMsg);
  const msgBytes = fromHex(clean);

  const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
  const combined = new Uint8Array(prefix.length + msgBytes.length);
  combined.set(prefix);
  combined.set(msgBytes, prefix.length);
  const originalHash = keccak256(combined);

  // Wrap in Safe message hash — Safe4337Module.isValidSignature wraps the
  // original hash before passing it to the WebAuthn signer for verification
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
  // EIP-712: params[1] is the typed data JSON string (or object)
  const typedDataRaw = request.params[1] ?? request.params[0];
  const typedData: TypedData = typeof typedDataRaw === 'string'
    ? JSON.parse(typedDataRaw)
    : typedDataRaw;
  const originalHash = hashTypedData(typedData);

  // Wrap in Safe message hash — Safe4337Module.isValidSignature wraps the
  // original hash before passing it to the WebAuthn signer for verification
  const safeHash = computeSafeMessageHash(originalHash, chainId, safeAddress);
  console.log('[DEBUG signTypedData] originalHash:', toHex(originalHash), 'safeHash:', toHex(safeHash), 'chainId:', chainId, 'safeAddress:', safeAddress);
  const assertion = await Passkey.sign(toHex(safeHash), account.id);
  console.log('[DEBUG signTypedData] signatureHex:', assertion.signatureHex.slice(0, 40) + '...');
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
): Promise<string> {
  const txDict = request.params[0] as Record<string, string>;
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
    txResult = await sendNative(safeAddress, to, valueClean, chainId, publicKeyHex, signFn);
  } else {
    const txData = fromHex(stripHexPrefix(dataHex));
    txResult = await sendContractCall(safeAddress, to, valueClean, txData, chainId, publicKeyHex, signFn);
  }

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
): Promise<any> {
  const { method } = request;

  if (method === 'eth_sendTransaction') {
    return handleSendTransaction(request, account, safeAddress, chainId);
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
    calls: Array<{ to: string; value?: string; data?: string }>;
    chainId?: string;
    from?: string;
  };

  const calls = payload.calls ?? [];
  if (calls.length === 0) throw new Error('No calls provided');

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
      txResult = await sendNative(safeAddress, to, valueClean, chainId, publicKeyHex, signFn);
    } else {
      const txData = fromHex(stripHexPrefix(dataHex));
      txResult = await sendContractCall(safeAddress, to, valueClean, txData, chainId, publicKeyHex, signFn);
    }
    return await txResult.waitForTxHash();
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
    chainId,
    publicKeyHex,
    signFn,
  );
  return await txResult.waitForTxHash();
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
 * Handle a read-only RPC method. Returns the result or null if not handled.
 */
export async function handleReadOnlyRPC(
  method: string,
  params: any[],
  address: string,
  chainId: number,
): Promise<{ handled: true; result: any } | { handled: false }> {
  switch (method) {
    case 'eth_accounts':
    case 'eth_requestAccounts':
      return { handled: true, result: [address] };
    case 'eth_chainId':
      return { handled: true, result: '0x' + chainId.toString(16) };
    case 'net_version':
      return { handled: true, result: String(chainId) };
    case 'wallet_getPermissions':
    case 'wallet_requestPermissions':
      return { handled: true, result: [{ parentCapability: 'eth_accounts' }] };
    case 'wallet_addEthereumChain':
      return { handled: true, result: null };
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
