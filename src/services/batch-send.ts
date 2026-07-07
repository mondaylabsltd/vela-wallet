/**
 * batch-send — pure helpers for building Safe MultiSend call batches for the two
 * advanced send modes, both of which settle in ONE UserOp (one signature, one
 * gas payment) via `sendBatchCalls`:
 *
 *   ① split — one token  → many recipients   (一币多人)
 *   ② multiSelect — many tokens → one recipient      (多币一人, "empty the wallet")
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

/**
 * Max recipients in one split/batch UserOp. A payroll run settles as a SINGLE
 * MultiSend (one signature, one gas payment), so this is bounded by the UserOp's
 * gas ceiling, not the UI. 60 keeps the call-gas comfortably under bundler limits
 * on every supported chain; the batch importer warns and trims past it, and the
 * final gas is still estimated on the confirm step (BundlerFundingModal covers a
 * shortfall). Larger payrolls are split across a couple of sends.
 */
export const BATCH_MAX_RECIPIENTS = 60;

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

// ── ② multiSelect: many tokens → one recipient ────────────────────────────────────

/** A token line in multiSelect mode. `amount` is a human decimal string (usually full balance). */
export interface MultiTokenSpec extends TokenRef {
  amount: string;
}

export function buildMultiTokenCalls(recipient: string, tokens: MultiTokenSpec[]): BatchCall[] {
  if (tokens.length === 0) throw new BatchSendError('no tokens');
  return tokens.map((tk) => buildTransferCall(tk.tokenAddress, recipient, toBaseUnits(tk.amount, tk.decimals)));
}

/**
 * Whether a token is worth offering in the multiSelect picker. A positive, non-spam
 * balance is the floor; `requireValue` additionally gates on a known USD value
 * — the predicate behind "全选有价值代币" (select all valuable).
 */
export function isMultiSelectable(tok: APIToken, requireValue = false): boolean {
  if (tok.spam) return false;
  if (tokenBalanceDouble(tok) <= 0) return false;
  if (requireValue && tokenUsdValue(tok) <= 0) return false;
  return true;
}

/**
 * Bridge a multiSelect selection (full APITokens) into `MultiTokenSpec` specs — each token
 * sends its full balance to the one recipient. Native vs ERC-20 is derived from
 * the token (no address ⇒ native). Native gas-reserve is NOT applied here: Vela
 * settles gas via the bundler/gas-account, not the Safe's native balance, so a
 * full-balance native multiSelect is valid; the caller may still trim it if a given
 * chain needs an EntryPoint prefund.
 */
export function toMultiTokenSpecs(tokens: APIToken[]): MultiTokenSpec[] {
  return tokens.map((tk) => ({
    tokenAddress: isNativeToken(tk) ? null : tk.tokenAddress,
    decimals: tk.decimals,
    amount: tk.balance || '0',
  }));
}

/** The tokens a "全选有价值代币" tap selects: held, non-spam, with a known USD value. */
export function selectAllValuable(tokens: APIToken[]): APIToken[] {
  return tokens.filter((t) => isMultiSelectable(t, true));
}

/**
 * Trim a multiSelect's native-coin line by `reserveWei` so the Safe keeps enough to
 * fund the EntryPoint gas prefund. On non-Tempo chains there's no paymaster, so
 * the prefund is paid from the Safe's native balance — sending it ALL would
 * leave nothing to prefund and the UserOp would revert (AA21). ERC-20 lines are
 * untouched; the native line is dropped entirely if nothing remains after the
 * reserve. A non-positive reserve (e.g. Tempo, where prefund is a no-op) is a
 * no-op.
 */
export function reserveNativeGas(tokens: MultiTokenSpec[], reserveWei: bigint): MultiTokenSpec[] {
  if (reserveWei <= 0n) return tokens;
  return tokens.flatMap((tk) => {
    if (tk.tokenAddress !== null) return [tk]; // ERC-20 — gas isn't paid in it
    const left = toBaseUnits(tk.amount, tk.decimals) - reserveWei;
    if (left <= 0n) return []; // native balance can't even cover the gas reserve
    return [{ ...tk, amount: fromBaseUnits(left, tk.decimals) }];
  });
}

/**
 * Trim a multiSelect's Tempo FEE-TOKEN line (pathUSD) by `reserveUnits` so the Safe keeps
 * enough to pay the batched gas reimbursement. Tempo has no native coin — gas is settled by an
 * in-band feeToken.transfer, drawn from the SAME balance being swept. reserveNativeGas can't do
 * this: pathUSD is a TIP-20 with a non-null address, so it looks like any other ERC-20 and is
 * left untouched — sweeping it at full balance would leave nothing for the reimbursement and the
 * op would revert. Only the line whose address matches `feeTokenAddress` is trimmed; other TIP-20
 * lines (gas isn't paid in them) pass through. The fee-token line is dropped entirely if its
 * whole balance is needed for gas. A non-positive reserve is a no-op.
 */
export function reserveTempoFeeToken(
  tokens: MultiTokenSpec[],
  feeTokenAddress: string,
  reserveUnits: bigint,
): MultiTokenSpec[] {
  if (reserveUnits <= 0n) return tokens;
  const fee = feeTokenAddress.toLowerCase();
  return tokens.flatMap((tk) => {
    if (tk.tokenAddress?.toLowerCase() !== fee) return [tk]; // not the gas token
    const left = toBaseUnits(tk.amount, tk.decimals) - reserveUnits;
    if (left <= 0n) return []; // whole fee-token balance is needed for gas — don't sweep it
    return [{ ...tk, amount: fromBaseUnits(left, tk.decimals) }];
  });
}
