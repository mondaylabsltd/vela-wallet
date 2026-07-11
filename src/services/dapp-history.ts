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
import type { StoredAssetSim } from '@/services/tx-simulation';

/** Cap stored payloads so a huge typed-data blob can't bloat history. */
function cap(s: string | undefined): string | undefined {
  if (!s) return undefined;
  return s.length > MAX_SIGNED_CONTENT ? `${s.slice(0, MAX_SIGNED_CONTENT)}…` : s;
}

/**
 * Budget for the stored original request (method + params) used to re-render the
 * signing panel from history. Larger than MAX_SIGNED_CONTENT because params carry
 * the full tx object; still bounded so a deploy's bytecode can't bloat the store.
 */
const MAX_REPLAY_REQUEST = 24000;

/** Clip every string value in a structure to `capLen` chars (deep, immutable). */
function clipStrings(v: unknown, capLen: number): unknown {
  if (typeof v === 'string') return v.length > capLen ? v.slice(0, capLen) : v;
  if (Array.isArray(v)) return v.map((x) => clipStrings(x, capLen));
  if (v && typeof v === 'object') {
    const o: Record<string, unknown> = {};
    for (const k of Object.keys(v as Record<string, unknown>)) o[k] = clipStrings((v as Record<string, unknown>)[k], capLen);
    return o;
  }
  return v;
}

/**
 * Capture the original request so its signing panel can be replayed read-only.
 * The serialized payload is bounded by MAX_REPLAY_REQUEST so a deploy's bytecode —
 * or a fat EIP-5792 batch of many medium calls — can't bloat history. When over
 * budget, string values (typically calldata) are progressively clipped until the
 * TOTAL fits, and `truncated` is set; the panel still resolves the intent + the
 * surviving calldata prefix (selector + early params), flagged as truncated.
 */
export function capRequest(
  method: string,
  params: unknown[] | undefined,
): { signedRequest: { method: string; params: unknown[] }; requestTruncated: boolean } {
  const safeParams = Array.isArray(params) ? params : [];
  const sizeOf = (p: unknown[]) => JSON.stringify({ method, params: p }).length;

  try {
    if (sizeOf(safeParams) <= MAX_REPLAY_REQUEST) {
      return { signedRequest: { method, params: safeParams }, requestTruncated: false };
    }
  } catch {
    // Non-serializable params — store nothing replayable rather than throw.
    return { signedRequest: { method, params: [] }, requestTruncated: true };
  }

  // Over budget — clip strings progressively (a near-budget cap preserves a single
  // large field; tighter caps bound a many-field batch) until the TOTAL fits.
  for (const capLen of [MAX_REPLAY_REQUEST - 2000, 8000, 2000, 500, 0]) {
    const clipped = clipStrings(safeParams, capLen) as unknown[];
    if (sizeOf(clipped) <= MAX_REPLAY_REQUEST) {
      return { signedRequest: { method, params: clipped }, requestTruncated: true };
    }
  }
  // Pathological field COUNT (structure alone exceeds budget) — drop the params.
  return { signedRequest: { method, params: [] }, requestTruncated: true };
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
  /**
   * Lifecycle status. Defaults to 'confirmed'. A tx is first recorded 'pending'
   * the moment it's submitted (so closing the sheet can't lose it), then patched
   * to 'confirmed'/'failed' once the on-chain receipt resolves.
   */
  status?: 'pending' | 'confirmed' | 'failed';
  /** UserOp hash, kept on the pending record so receipt polling can resume. */
  userOpHash?: string;
  /** Sign-time asset-change simulation (predicted balance changes), JSON-safe. */
  assetChanges?: StoredAssetSim;
  /**
   * ERC-7730 / best-effort clear-signing intent (e.g. "Swap", "Approve", "Permit"),
   * captured at approve time. Persisted so the Connections list + detail view show a
   * meaningful operation label instead of the generic "Contract interaction". The
   * signing sheet re-derives intent live at replay time; this is the recorded copy.
   */
  intent?: string;
}

/**
 * Build the history record for an approved request. Returns a record for ANY
 * method (the fallback is a message signature) so an approved request is never
 * dropped. Transactions (`eth_sendTransaction`, `wallet_sendCalls`) carry the
 * recipient/value/hash; everything else is a signature.
 */
export function buildSigningRecord(input: SigningRecordInput): LocalTransaction {
  const { method, params, result, from, chainId, dappOrigin, nowMs, status = 'confirmed', userOpHash = '', assetChanges, intent } = input;
  const now = Math.floor(nowMs / 1000);
  const signedContent = extractSignedContent(method, params);
  const { signedRequest, requestTruncated } = capRequest(method, params);
  const base = {
    userOpHash, from, chainId, timestamp: now,
    status, dappOrigin, signedContent, signedRequest, requestTruncated, assetChanges, intent,
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

/**
 * Build the "Connected to <app>" audit record for a dApp session grant. Unlike a
 * signing record this carries no signature/tx — it just marks that a connection to
 * `dappOrigin` was authorized at `nowMs`, so every surface has a tappable session
 * trail (the in-app browser previously left only a silent grant). Written once per
 * user-approved connection (the consent moment is naturally deduped), never on an
 * auto-reconnect. Reuses the single transactionHistory store.
 */
export function buildConnectionRecord(input: {
  from: string;
  chainId: number;
  dappOrigin: string;
  nowMs: number;
}): LocalTransaction {
  const { from, chainId, dappOrigin, nowMs } = input;
  return {
    id: `dapp-${nowMs}-connect`,
    userOpHash: '',
    txHash: '',
    from,
    to: '',
    value: '0',
    symbol: '',
    decimals: 0,
    chainId,
    timestamp: Math.floor(nowMs / 1000),
    status: 'confirmed',
    type: 'connect',
    dappOrigin,
  };
}
