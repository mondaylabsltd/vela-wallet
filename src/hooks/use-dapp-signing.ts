/**
 * Shared dApp request signing logic.
 * Used by both BLE (native) and WalletConnect (web) connect screens.
 */
import type { Account } from '@/models/types';
import * as Passkey from '@/modules/passkey';
import { derSignatureToRaw } from '@/services/attestation-parser';
import { keccak256 } from '@/services/eth-crypto';
import { fromHex, stripHexPrefix, toHex } from '@/services/hex';
import * as PublicKeyIndex from '@/services/public-key-index';
import { rpcCall } from '@/services/rpc-adapter';
import { sendContractCall, sendNative } from '@/services/safe-transaction';
import { findAccountByCredentialId } from '@/services/storage';

export interface DAppRequest {
  id: string;
  method: string;
  params: any[];
  origin?: string;
}

/**
 * Handle a personal_sign request.
 * Returns "0x" + raw_signature + "00" (P256 contract sig).
 */
export async function handlePersonalSign(
  request: DAppRequest,
  account: Account,
): Promise<string> {
  const hexMsg = request.params[0] as string;
  const clean = stripHexPrefix(hexMsg);
  const msgBytes = fromHex(clean);

  const prefix = new TextEncoder().encode(`\x19Ethereum Signed Message:\n${msgBytes.length}`);
  const combined = new Uint8Array(prefix.length + msgBytes.length);
  combined.set(prefix);
  combined.set(msgBytes, prefix.length);
  const dataToSign = keccak256(combined);

  const assertion = await Passkey.sign(toHex(dataToSign), account.id);
  const rawSig = derSignatureToRaw(fromHex(assertion.signatureHex));
  if (!rawSig) throw new Error('Failed to convert signature');

  return '0x' + toHex(rawSig) + '00';
}

/**
 * Handle an eth_signTypedData_v4 request.
 */
export async function handleSignTypedData(
  request: DAppRequest,
  account: Account,
): Promise<string> {
  const jsonStr = JSON.stringify(request.params);
  const jsonBytes = new TextEncoder().encode(jsonStr);
  const dataToSign = keccak256(jsonBytes);

  const assertion = await Passkey.sign(toHex(dataToSign), account.id);
  const rawSig = derSignatureToRaw(fromHex(assertion.signatureHex));
  if (!rawSig) throw new Error('Failed to convert signature');

  return '0x' + toHex(rawSig) + '00';
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
    const record = await PublicKeyIndex.queryRecord(Passkey.RELYING_PARTY, account.id);
    publicKeyHex = record.publicKey;
  }

  if (!publicKeyHex) throw new Error('Public key not found');

  const signFn = async (challenge: Uint8Array) => {
    const assertion = await Passkey.sign(toHex(challenge), account.id);

    const { verifySafeWebAuthn } = await import('@/services/webauthn-verify');
    const compat = verifySafeWebAuthn(assertion);
    if (!compat.ok) {
      throw new Error(
        'Your passkey provider is not compatible with Vela Wallet. ' +
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

  return txResult.txHash;
}

/**
 * Handle a generic sign request.
 */
export async function handleGenericSign(
  request: DAppRequest,
  account: Account,
): Promise<string> {
  const jsonStr = JSON.stringify(request.params);
  const jsonBytes = new TextEncoder().encode(jsonStr);
  const dataToSign = keccak256(jsonBytes);

  const assertion = await Passkey.sign(toHex(dataToSign), account.id);
  return '0x' + assertion.signatureHex;
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
  } else if (method === 'personal_sign') {
    return handlePersonalSign(request, account);
  } else if (method.includes('signTypedData')) {
    return handleSignTypedData(request, account);
  } else {
    return handleGenericSign(request, account);
  }
}

/**
 * Check if a method is a signing method that needs user approval.
 */
export function isSigningMethod(method: string): boolean {
  return method === 'eth_sendTransaction' ||
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
