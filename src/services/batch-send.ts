/**
 * batch-send — pure helpers for building Safe MultiSend call batches for the two
 * advanced send modes, both of which settle in ONE UserOp (one signature, one
 * gas payment) via `sendBatchCalls`:
 *
 *   ① split — one token  → many recipients   (一币多人)
 *   ② sweep — many tokens → one recipient      (多币一人, "empty the wallet")
 *
 * These never sign or submit. They only translate already-validated UI state
 * into the `{ to, value, data }[]` array `sendBatchCalls` expects, so the
 * money-shaping logic can be exhaustively unit-tested away from the
 * passkey/bundler stack. The two modes are mutually exclusive by construction
 * upstream (you can't have >1 recipient AND >1 token); this module just builds
 * whichever batch it's handed.
 *
 * Value format matches `buildMultiSendExecuteCallData`: `value` is a 0x-hex wei
 * string ('0x0' for ERC-20 transfers), `data` is 0x-hex calldata ('0x' for a
 * bare native transfer).
 */
import { toBaseUnits, fromBaseUnits } from '@/services/eip681';
import { isAddress, isNativeToken, tokenBalanceDouble, tokenUsdValue, type APIToken } from '@/models/types';

/** One entry in a Safe MultiSend batch, shaped for `sendBatchCalls`. */
export interface BatchCall {
  to: string;
  /** 0x-hex wei. '0x0' for an ERC-20 transfer (the value rides in calldata). */
  value: string;
  /** 0x-hex calldata. '0x' for a plain native-coin transfer. */
  data: string;
}

const ERC20_TRANSFER_SELECTOR = 'a9059cbb';

/** Thrown when a batch is asked to move something malformed — never silently coerced. */
export class BatchSendError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'BatchSendError';
  }
}

function pad32(hexNo0x: string): string {
  if (hexNo0x.length > 64) throw new BatchSendError('value exceeds uint256');
  return hexNo0x.padStart(64, '0');
}

/** `transfer(address,uint256)` calldata as a 0x-hex string. */
export function encodeErc20Transfer(to: string, amount: bigint): string {
  if (!isAddress(to)) throw new BatchSendError(`invalid recipient: ${to}`);
  if (amount < 0n) throw new BatchSendError('amount must be non-negative');
  return '0x' + ERC20_TRANSFER_SELECTOR + pad32(to.slice(2).toLowerCase()) + pad32(amount.toString(16));
}

/**
 * Build one transfer call. `tokenAddress === null` ⇒ the chain's native coin
 * (value carries the amount, no calldata); otherwise an ERC-20 `transfer`.
 */
export function buildTransferCall(tokenAddress: string | null, to: string, amount: bigint): BatchCall {
  if (!isAddress(to)) throw new BatchSendError(`invalid recipient: ${to}`);
  if (amount <= 0n) throw new BatchSendError('amount must be greater than zero');
  if (tokenAddress === null) {
    return { to, value: '0x' + amount.toString(16), data: '0x' };
  }
  if (!isAddress(tokenAddress)) throw new BatchSendError(`invalid token: ${tokenAddress}`);
  return { to: tokenAddress, value: '0x0', data: encodeErc20Transfer(to, amount) };
}

// ── ① split: one token → many recipients ────────────────────────────────────

/** A recipient line in split mode. `amount` is a human decimal string ("1.5"). */
export interface SplitRecipient {
  address: string;
  amount: string;
}

/** Net amount for one token (null tokenAddress ⇒ native). */
export interface TokenRef {
  tokenAddress: string | null;
  decimals: number;
}

export function buildSplitCalls(token: TokenRef, recipients: SplitRecipient[]): BatchCall[] {
  if (recipients.length === 0) throw new BatchSendError('no recipients');
  return recipients.map((r) =>
    buildTransferCall(token.tokenAddress, r.address, toBaseUnits(r.amount, token.decimals)),
  );
}

/** Total of all split amounts in base units — for the "合计" line and balance check. */
export function sumSplitBaseUnits(recipients: SplitRecipient[], decimals: number): bigint {
  return recipients.reduce((sum, r) => sum + toBaseUnits(r.amount, decimals), 0n);
}

// ── ② sweep: many tokens → one recipient ────────────────────────────────────

/** A token line in sweep mode. `amount` is a human decimal string (usually full balance). */
export interface SweepToken extends TokenRef {
  amount: string;
}

export function buildSweepCalls(recipient: string, tokens: SweepToken[]): BatchCall[] {
  if (tokens.length === 0) throw new BatchSendError('no tokens');
  return tokens.map((tk) => buildTransferCall(tk.tokenAddress, recipient, toBaseUnits(tk.amount, tk.decimals)));
}

/**
 * Whether a token is worth offering in the sweep picker. A positive, non-spam
 * balance is the floor; `requireValue` additionally gates on a known USD value
 * — the predicate behind "全选有价值代币" (select all valuable).
 */
export function isSweepable(tok: APIToken, requireValue = false): boolean {
  if (tok.spam) return false;
  if (tokenBalanceDouble(tok) <= 0) return false;
  if (requireValue && tokenUsdValue(tok) <= 0) return false;
  return true;
}

/**
 * Bridge a sweep selection (full APITokens) into `SweepToken` specs — each token
 * sweeps its full balance to the one recipient. Native vs ERC-20 is derived from
 * the token (no address ⇒ native). Native gas-reserve is NOT applied here: Vela
 * settles gas via the bundler/gas-account, not the Safe's native balance, so a
 * full-balance native sweep is valid; the caller may still trim it if a given
 * chain needs an EntryPoint prefund.
 */
export function toSweepTokens(tokens: APIToken[]): SweepToken[] {
  return tokens.map((tk) => ({
    tokenAddress: isNativeToken(tk) ? null : tk.tokenAddress,
    decimals: tk.decimals,
    amount: tk.balance || '0',
  }));
}

/** The tokens a "全选有价值代币" tap selects: held, non-spam, with a known USD value. */
export function selectAllValuable(tokens: APIToken[]): APIToken[] {
  return tokens.filter((t) => isSweepable(t, true));
}

/**
 * Trim a sweep's native-coin line by `reserveWei` so the Safe keeps enough to
 * fund the EntryPoint gas prefund. On non-Tempo chains there's no paymaster, so
 * the prefund is paid from the Safe's native balance — sweeping it ALL would
 * leave nothing to prefund and the UserOp would revert (AA21). ERC-20 lines are
 * untouched; the native line is dropped entirely if nothing remains after the
 * reserve. A non-positive reserve (e.g. Tempo, where prefund is a no-op) is a
 * no-op.
 */
export function reserveNativeGas(tokens: SweepToken[], reserveWei: bigint): SweepToken[] {
  if (reserveWei <= 0n) return tokens;
  return tokens.flatMap((tk) => {
    if (tk.tokenAddress !== null) return [tk]; // ERC-20 — gas isn't paid in it
    const left = toBaseUnits(tk.amount, tk.decimals) - reserveWei;
    if (left <= 0n) return []; // native balance can't even cover the gas reserve
    return [{ ...tk, amount: fromBaseUnits(left, tk.decimals) }];
  });
}
