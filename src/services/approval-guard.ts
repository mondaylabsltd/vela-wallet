/**
 * Approval guard — the "unlimited can never leave the wallet" core.
 *
 * Pure, dependency-light, and INDEPENDENT of ERC-7730 descriptors. A token
 * drainer's #1 tool is an unbounded `approve` / `permit` / `setApprovalForAll`,
 * and a descriptor lookup is exactly what fails on novel/hostile contracts — so
 * detection here works straight off the raw calldata / typed-data, not off a
 * resolved descriptor.
 *
 * Three responsibilities, all pure:
 *   1. detectApproval()      — recognise every approval-granting shape.
 *   2. rewriteApprovalParams() — re-encode the request to a user-chosen FINITE
 *                               amount (or revoke), keeping every other byte.
 *   3. enforceNoUnlimited()  — the final, descriptor-independent submit guard:
 *                               re-scan the outgoing request and THROW if it
 *                               would still grant an unbounded allowance.
 *
 * Invariant: no code path may emit an allowance ≥ the per-width cap. The cap
 * sits far above any legitimate amount (largest plausible real approval —
 * total_supply × 10^decimals — is ≈ 2^128) and far below the canonical
 * "unlimited" sentinels (uint256-max, 2^255, uint160-max), so it cleanly
 * separates "a big finite number the user chose" from "unlimited".
 */
import { functionSelector, abiEncodeUint256 } from '@/services/eth-crypto';
import { fromHex, toHex, stripHexPrefix } from '@/services/hex';

// ---------------------------------------------------------------------------
// Caps
// ---------------------------------------------------------------------------

/** uint256 amount fields (ERC-20 approve / increaseAllowance / ERC-2612 value). */
export const UNLIMITED_CAP_256 = 1n << 200n;
/** Permit2 uint160 amount fields (sentinel is 2^160-1 ≈ 2^160). */
export const UNLIMITED_CAP_160 = 1n << 152n;

export type AmountBits = 256 | 160;

export function capForBits(bits: AmountBits): bigint {
  return bits === 160 ? UNLIMITED_CAP_160 : UNLIMITED_CAP_256;
}

/** True when an amount is "effectively unlimited" for its field width. */
export function isUnboundedAmount(amount: bigint, bits: AmountBits = 256): boolean {
  return amount >= capForBits(bits);
}

// ---------------------------------------------------------------------------
// Selectors (computed once)
// ---------------------------------------------------------------------------

const sel = (sig: string) => '0x' + toHex(functionSelector(sig)).toLowerCase();

const SELECTORS = {
  approve: sel('approve(address,uint256)'),                 // 0x095ea7b3 (ERC-20 approve / ERC-721 approve)
  increaseAllowance: sel('increaseAllowance(address,uint256)'),
  decreaseAllowance: sel('decreaseAllowance(address,uint256)'),
  setApprovalForAll: sel('setApprovalForAll(address,bool)'),
} as const;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

export type ApprovalKind =
  | 'erc20-approve'
  | 'increaseAllowance'
  | 'decreaseAllowance'
  | 'setApprovalForAll'
  | 'erc2612-permit'
  | 'dai-permit'
  | 'permit2-single'
  | 'permit2-batch';

export interface DetectedApproval {
  kind: ApprovalKind;
  /** ERC-20 token / NFT collection / Permit2 token address, if known. */
  tokenAddress?: string;
  /** Who is being granted spending power. */
  spender: string;
  /** Granted amount in raw base units. undefined for boolean grants. */
  amountRaw?: bigint;
  /** Field width of the amount (drives the cap + re-encode). */
  amountBits?: AmountBits;
  /** Effectively unlimited (amount ≥ cap) OR a boolean grant-all of `true`. */
  isUnbounded: boolean;
  /** Boolean grant (setApprovalForAll / DAI allowed) — no amount to cap. */
  isBooleanGrant: boolean;
  /** Reduces risk (decreaseAllowance / revoke) — render as safe. */
  isReducing: boolean;
  /** Whether the wallet can safely re-encode a finite amount for this shape. */
  editable: boolean;
  /** Why editing is blocked (when editable === false). */
  blockReason?: string;
  /** Expiry/deadline (unix seconds), when the shape carries one. */
  deadline?: bigint;
  /** Where the amount lives, for the rewrite step. */
  locus:
    | { type: 'calldata-word'; wordIndex: number }
    | { type: 'typed-path'; path: string };
}

export type ApprovalChoice =
  | { type: 'amount'; amountRaw: bigint }   // finite cap
  | { type: 'revoke' }                       // 0 / false
  | { type: 'grant' };                       // keep boolean true (explicit, setApprovalForAll only)

// ---------------------------------------------------------------------------
// Detection
// ---------------------------------------------------------------------------

/**
 * Detect an approval-granting request from raw (method, params).
 * Returns null when the request grants no spending power.
 */
export function detectApproval(method: string, params: any[] | undefined): DetectedApproval | null {
  if (!params || params.length === 0) return null;

  if (method === 'eth_sendTransaction') {
    const tx = params[0];
    if (!tx || typeof tx !== 'object') return null;
    return detectCalldataApproval(tx.to, tx.data);
  }

  if (method.includes('signTypedData')) {
    const raw = params[1] ?? params[0];
    let td: any;
    try {
      td = typeof raw === 'string' ? JSON.parse(raw) : raw;
    } catch {
      return null;
    }
    return detectTypedDataApproval(td);
  }

  return null;
}

function detectCalldataApproval(to: string | undefined, data: string | undefined): DetectedApproval | null {
  if (!data || data === '0x') return null;
  const hex = stripHexPrefix(data).toLowerCase();
  if (hex.length < 8) return null;
  const selector = '0x' + hex.slice(0, 8);
  const word = (i: number): string => hex.slice(8 + i * 64, 8 + (i + 1) * 64);
  const addrFromWord = (w: string): string => '0x' + w.slice(24);
  const bigFromWord = (w: string): bigint => (w ? BigInt('0x' + w.padEnd(64, '0')) : 0n);

  if (selector === SELECTORS.approve) {
    // approve(address spender, uint256 amount). Same selector as ERC-721
    // approve(operator, tokenId) — a tokenId is never ≥ cap, so capping is
    // still safe; metadata resolution upstream refines ERC-20 vs NFT display.
    const spender = addrFromWord(word(0));
    const amountRaw = bigFromWord(word(1));
    return {
      kind: 'erc20-approve', tokenAddress: to?.toLowerCase(), spender, amountRaw, amountBits: 256,
      isUnbounded: isUnboundedAmount(amountRaw, 256), isBooleanGrant: false, isReducing: amountRaw === 0n,
      editable: true, locus: { type: 'calldata-word', wordIndex: 1 },
    };
  }

  if (selector === SELECTORS.increaseAllowance) {
    const spender = addrFromWord(word(0));
    const amountRaw = bigFromWord(word(1));
    return {
      kind: 'increaseAllowance', tokenAddress: to?.toLowerCase(), spender, amountRaw, amountBits: 256,
      isUnbounded: isUnboundedAmount(amountRaw, 256), isBooleanGrant: false, isReducing: false,
      editable: true, locus: { type: 'calldata-word', wordIndex: 1 },
    };
  }

  if (selector === SELECTORS.decreaseAllowance) {
    const spender = addrFromWord(word(0));
    const amountRaw = bigFromWord(word(1));
    return {
      kind: 'decreaseAllowance', tokenAddress: to?.toLowerCase(), spender, amountRaw, amountBits: 256,
      isUnbounded: false, isBooleanGrant: false, isReducing: true,
      editable: true, locus: { type: 'calldata-word', wordIndex: 1 },
    };
  }

  if (selector === SELECTORS.setApprovalForAll) {
    const operator = addrFromWord(word(0));
    const approved = bigFromWord(word(1)) !== 0n;
    return {
      kind: 'setApprovalForAll', tokenAddress: to?.toLowerCase(), spender: operator,
      isUnbounded: approved, isBooleanGrant: true, isReducing: !approved,
      // No finite amount exists; the only safe rewrite is revoke. Granting is
      // allowed but only via an explicit, deliberate confirmation in the UI.
      editable: true, locus: { type: 'calldata-word', wordIndex: 1 },
    };
  }

  return null;
}

function detectTypedDataApproval(td: any): DetectedApproval | null {
  if (!td || typeof td !== 'object') return null;
  const pt: string = td.primaryType ?? '';
  const msg = td.message ?? {};
  const domain = td.domain ?? {};

  // DAI-style permit: Permit(holder, spender, nonce, expiry, allowed)
  if (pt === 'Permit' && msg.allowed !== undefined) {
    const allowed = msg.allowed === true || msg.allowed === 'true' || msg.allowed === 1 || msg.allowed === '1';
    return {
      kind: 'dai-permit', tokenAddress: lc(domain.verifyingContract), spender: lc(msg.spender),
      isUnbounded: allowed, isBooleanGrant: true, isReducing: !allowed,
      editable: true,
      blockReason: allowed ? 'DAI permit grants full-balance access and cannot be capped to an amount.' : undefined,
      deadline: toBig(msg.expiry), locus: { type: 'typed-path', path: 'allowed' },
    };
  }

  // ERC-2612 permit: Permit(owner, spender, value, nonce, deadline)
  if (pt === 'Permit' && msg.value !== undefined) {
    const amountRaw = toBig(msg.value);
    return {
      kind: 'erc2612-permit', tokenAddress: lc(domain.verifyingContract), spender: lc(msg.spender),
      amountRaw, amountBits: 256, isUnbounded: isUnboundedAmount(amountRaw, 256),
      isBooleanGrant: false, isReducing: amountRaw === 0n, editable: true,
      deadline: toBig(msg.deadline), locus: { type: 'typed-path', path: 'value' },
    };
  }

  // Permit2 PermitSingle: { details: {token, amount(uint160), expiration, nonce}, spender, sigDeadline }
  if (pt === 'PermitSingle' && msg.details) {
    const amountRaw = toBig(msg.details.amount);
    return {
      kind: 'permit2-single', tokenAddress: lc(msg.details.token), spender: lc(msg.spender),
      amountRaw, amountBits: 160, isUnbounded: isUnboundedAmount(amountRaw, 160),
      isBooleanGrant: false, isReducing: amountRaw === 0n, editable: true,
      deadline: toBig(msg.details.expiration), locus: { type: 'typed-path', path: 'details.amount' },
    };
  }

  // Permit2 PermitBatch: details[]. Detect for the guard; editing the array is Phase 4.
  if (pt === 'PermitBatch' && Array.isArray(msg.details)) {
    const anyUnbounded = msg.details.some((d: any) => isUnboundedAmount(toBig(d?.amount), 160));
    return {
      kind: 'permit2-batch', spender: lc(msg.spender), amountBits: 160,
      isUnbounded: anyUnbounded, isBooleanGrant: false, isReducing: false,
      editable: false,
      blockReason: 'Batch approvals can\'t be edited yet — review each amount or reject.',
      locus: { type: 'typed-path', path: 'details' },
    };
  }

  return null;
}

// ---------------------------------------------------------------------------
// Rewrite — produce a NEW params array with the chosen finite amount.
// Never mutates the input. Throws if it would emit an unbounded allowance.
// ---------------------------------------------------------------------------

export function rewriteApprovalParams(
  method: string,
  params: any[],
  detected: DetectedApproval,
  choice: ApprovalChoice,
): any[] {
  if (method === 'eth_sendTransaction') {
    const tx = params[0];
    const newData = rewriteCalldata(tx.data, detected, choice);
    return [{ ...tx, data: newData }, ...params.slice(1)];
  }

  if (method.includes('signTypedData')) {
    const idx = typeof params[1] === 'string' || (params[1] && typeof params[1] === 'object') ? 1 : 0;
    const raw = params[idx];
    const wasString = typeof raw === 'string';
    // Deep clone (never mutate the caller's object). Typed data is JSON-safe;
    // structuredClone isn't available under Hermes.
    const td = wasString ? JSON.parse(raw) : JSON.parse(JSON.stringify(raw));
    rewriteTypedData(td, detected, choice);
    const out = params.slice();
    out[idx] = wasString ? JSON.stringify(td) : td;
    return out;
  }

  throw new Error('Cannot rewrite approval for this request');
}

function chosenAmount(detected: DetectedApproval, choice: ApprovalChoice): bigint {
  if (choice.type === 'revoke') return 0n;
  if (choice.type === 'grant') {
    if (!detected.isBooleanGrant) throw new Error('grant choice is only valid for boolean approvals');
    return 1n; // boolean true
  }
  // type === 'amount'
  const bits = detected.amountBits ?? 256;
  if (choice.amountRaw < 0n) throw new Error('amount must be non-negative');
  if (isUnboundedAmount(choice.amountRaw, bits)) {
    throw new Error('Unlimited approvals are disabled — choose a finite amount.');
  }
  return choice.amountRaw;
}

function rewriteCalldata(data: string, detected: DetectedApproval, choice: ApprovalChoice): string {
  if (detected.locus.type !== 'calldata-word') throw new Error('not a calldata approval');
  const hex = stripHexPrefix(data);
  const selector = hex.slice(0, 8);
  const wordIndex = detected.locus.wordIndex;
  const wordStart = 8 + wordIndex * 64;
  const wordEnd = wordStart + 64;
  if (hex.length < wordEnd) throw new Error('calldata too short to rewrite');

  let newWord: string;
  if (detected.kind === 'setApprovalForAll') {
    // boolean: grant → true, revoke → false. No "amount" to cap.
    if (choice.type === 'amount') throw new Error('setApprovalForAll has no amount to set');
    newWord = toHex(abiEncodeUint256(choice.type === 'grant' ? 1n : 0n));
  } else {
    newWord = toHex(abiEncodeUint256(chosenAmount(detected, choice)));
  }

  const out = '0x' + selector + hex.slice(8, wordStart) + newWord + hex.slice(wordEnd);

  // Round-trip safety: re-decode and assert only the intended word changed.
  assertOnlyWordChanged(data, out, wordStart, wordEnd);
  return out;
}

function rewriteTypedData(td: any, detected: DetectedApproval, choice: ApprovalChoice): void {
  if (detected.locus.type !== 'typed-path') throw new Error('not a typed-data approval');
  const path = detected.locus.path;

  if (detected.kind === 'dai-permit') {
    if (choice.type === 'amount') throw new Error('DAI permit has no amount to set');
    setPath(td.message, path, choice.type === 'grant'); // allowed = true/false (boolean)
    return;
  }

  // amount-bearing typed data — store as a DECIMAL STRING (avoid JS number precision loss).
  const amount = chosenAmount(detected, choice);
  setPath(td.message, path, amount.toString());
}

// ---------------------------------------------------------------------------
// enforceNoUnlimited — the independent, descriptor-free final submit guard.
// Throws if the FINAL request would grant an unbounded allowance.
// ---------------------------------------------------------------------------

export class UnlimitedApprovalError extends Error {
  constructor(detail: string) {
    super(`Blocked: this would grant an unlimited approval (${detail}). Set a finite amount and try again.`);
    this.name = 'UnlimitedApprovalError';
  }
}

export function enforceNoUnlimited(method: string, params: any[] | undefined): void {
  const detected = detectApproval(method, params);
  if (!detected) return;
  // Boolean grants (setApprovalForAll true / DAI allowed) are handled by explicit
  // UI consent, not by this amount guard — there is no finite amount to enforce.
  if (detected.isBooleanGrant) return;
  // Reducing the allowance (decreaseAllowance / approve-to-0) never grants — allow.
  if (detected.isReducing) return;
  if (detected.kind === 'permit2-batch') {
    if (detected.isUnbounded) throw new UnlimitedApprovalError('Permit2 batch contains an unlimited amount');
    return;
  }
  if (detected.amountRaw !== undefined && detected.amountBits) {
    if (isUnboundedAmount(detected.amountRaw, detected.amountBits)) {
      throw new UnlimitedApprovalError(`${detected.kind} amount = ${detected.amountRaw}`);
    }
  }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function lc(v: any): string {
  return typeof v === 'string' ? v.toLowerCase() : '';
}

function toBig(v: any): bigint {
  if (v === undefined || v === null) return 0n;
  try {
    if (typeof v === 'bigint') return v;
    if (typeof v === 'number') return BigInt(Math.trunc(v));
    const s = String(v).trim();
    if (s === '') return 0n;
    return BigInt(s.startsWith('0x') ? s : s);
  } catch {
    return 0n;
  }
}

/** Set a dot-path on an object (e.g. "details.amount"), failing loudly if absent. */
function setPath(obj: any, path: string, value: any): void {
  const parts = path.split('.');
  let cur = obj;
  for (let i = 0; i < parts.length - 1; i++) {
    cur = cur?.[parts[i]];
    if (cur == null) throw new Error(`rewrite path not found: ${path}`);
  }
  const leaf = parts[parts.length - 1];
  if (!(leaf in cur)) throw new Error(`rewrite path not found: ${path}`);
  cur[leaf] = value;
}

/** Assert that exactly the [start,end) hex window changed between two calldatas. */
function assertOnlyWordChanged(before: string, after: string, start: number, end: number): void {
  const a = stripHexPrefix(before);
  const b = stripHexPrefix(after);
  if (a.length !== b.length) throw new Error('rewrite changed calldata length');
  for (let i = 0; i < a.length; i++) {
    if (i >= start && i < end) continue;
    if (a[i] !== b[i]) throw new Error(`rewrite altered a byte outside the amount word at ${i}`);
  }
}

/**
 * Parse a human token amount (e.g. "1,234.5") into raw base units for `decimals`.
 * Returns null on invalid input. Pure; no locale assumptions beyond '.'/',' .
 */
export function parseTokenAmount(human: string, decimals: number): bigint | null {
  if (human == null) return null;
  const cleaned = human.replace(/,/g, '').trim();
  if (cleaned === '' || !/^\d*\.?\d*$/.test(cleaned)) return null;
  const [whole = '0', frac = ''] = cleaned.split('.');
  if (frac.length > decimals) return null; // more precision than the token has
  const fracPadded = (frac + '0'.repeat(decimals)).slice(0, decimals);
  try {
    return BigInt(whole || '0') * 10n ** BigInt(decimals) + BigInt(fracPadded || '0');
  } catch {
    return null;
  }
}

/** Format raw base units back to a human string for `decimals` (trims zeros). */
export function formatTokenAmount(raw: bigint, decimals: number, maxFrac = 6): string {
  if (decimals === 0) return raw.toString();
  const base = 10n ** BigInt(decimals);
  const whole = raw / base;
  const frac = raw % base;
  if (frac === 0n) return whole.toString();
  const fracStr = frac.toString().padStart(decimals, '0').slice(0, maxFrac).replace(/0+$/, '');
  return fracStr ? `${whole}.${fracStr}` : whole.toString();
}
