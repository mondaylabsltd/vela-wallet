/**
 * dApp signing-history records — pure helpers that turn an approved dApp request
 * into a LocalTransaction for the Connections panel.
 *
 * Kept free of React / native deps so the "every approved method is recorded"
 * guarantee can be unit-tested directly. The set of methods handled here MUST
 * stay in sync with isSigningMethod() in hooks/use-dapp-signing.ts — anything
 * that gets approved but isn't recorded silently vanishes from history.
 */
import { nativeSymbol } from '@/models/network';
import { MAX_SIGNED_CONTENT, type LocalTransaction } from '@/services/storage';

/** Cap stored payloads so a huge typed-data blob can't bloat history. */
function cap(s: string | undefined): string | undefined {
  if (!s) return undefined;
  return s.length > MAX_SIGNED_CONTENT ? `${s.slice(0, MAX_SIGNED_CONTENT)}…` : s;
}

/** Decode a hex message to readable text; keep hex if it decoded to binary. */
export function decodeSignMessage(raw: unknown): string | undefined {
  if (typeof raw !== 'string') return undefined;
  const clean = raw.startsWith('0x') ? raw.slice(2) : raw;
  if (clean.length === 0 || clean.length % 2 !== 0 || !/^[0-9a-fA-F]+$/.test(clean)) {
    return raw; // already plain text (some dApps don't hex-encode)
  }
  try {
    const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map((b) => parseInt(b, 16)));
    const decoded = new TextDecoder().decode(bytes);
    // Keep hex if it decoded to binary (control chars / invalid UTF-8); else show text.
    return /[\x00-\x08\x0E-\x1F�]/.test(decoded) ? raw : decoded;
  } catch {
    return raw;
  }
}

/** Pull the human-meaningful signed payload out of a dApp request. */
export function extractSignedContent(method: string, params: unknown[] | undefined): string | undefined {
  if (!params) return undefined;
  // personal_sign → [message, address]; eth_sign → [address, data] (reversed).
  if (method === 'personal_sign') return cap(decodeSignMessage(params[0]));
  if (method === 'eth_sign') return cap(decodeSignMessage(params[1]));
  if (method.includes('signTypedData')) {
    const rawData = params[1] ?? params[0];
    try {
      const obj = typeof rawData === 'string' ? JSON.parse(rawData) : rawData;
      return cap(JSON.stringify(obj, null, 2));
    } catch {
      return cap(typeof rawData === 'string' ? rawData : undefined);
    }
  }
  if (method === 'eth_sendTransaction') {
    const tx = params[0] as Record<string, string> | undefined;
    return tx?.data && tx.data !== '0x' ? cap(tx.data) : undefined;
  }
  if (method === 'wallet_sendCalls') {
    try { return cap(JSON.stringify(params[0], null, 2)); } catch { return undefined; }
  }
  return undefined;
}

export interface SigningRecordInput {
  method: string;
  params: unknown[] | undefined;
  /** Return value of handleDAppRequest — a tx hash string for transactions. */
  result: unknown;
  from: string;
  chainId: number;
  dappOrigin: string;
  /** Millisecond timestamp; drives both the unique id and the display time. */
  nowMs: number;
}

/**
 * Build the history record for an approved request. Returns a record for ANY
 * method (the fallback is a message signature) so an approved request is never
 * dropped. Transactions (`eth_sendTransaction`, `wallet_sendCalls`) carry the
 * recipient/value/hash; everything else is a signature.
 */
export function buildSigningRecord(input: SigningRecordInput): LocalTransaction {
  const { method, params, result, from, chainId, dappOrigin, nowMs } = input;
  const now = Math.floor(nowMs / 1000);
  const signedContent = extractSignedContent(method, params);
  const base = {
    userOpHash: '', from, chainId, timestamp: now,
    status: 'confirmed' as const, dappOrigin, signedContent,
  };

  if (method === 'eth_sendTransaction' || method === 'wallet_sendCalls') {
    const tx = (Array.isArray(params) ? params[0] : undefined) as Record<string, string> | undefined;
    return {
      ...base, id: `dapp-${nowMs}-tx`, txHash: typeof result === 'string' ? result : '',
      to: tx?.to ?? '', value: tx?.value ?? '0x0', symbol: nativeSymbol(chainId), decimals: 18, type: 'dapp_tx',
    };
  }
  if (method.includes('signTypedData')) {
    return { ...base, id: `dapp-${nowMs}-typed`, txHash: '', to: '', value: '0', symbol: '', decimals: 0, type: 'sign_typed_data' };
  }
  // personal_sign, eth_sign, and any other approved signing method.
  return { ...base, id: `dapp-${nowMs}-msg`, txHash: '', to: '', value: '0', symbol: '', decimals: 0, type: 'sign_message' };
}
