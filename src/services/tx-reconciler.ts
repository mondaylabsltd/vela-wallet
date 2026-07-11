/**
 * Pending-transaction reconciler.
 *
 * A UserOp submit returns a userOpHash immediately and the on-chain receipt is
 * resolved in the background. If the app is closed (or the JS context torn down)
 * before that receipt lands, the record is left `pending` forever even though the
 * transaction may have confirmed on-chain. This reconciler re-polls the bundler
 * for any still-pending submission on app launch / Home focus, and flips it to
 * `confirmed` (with the tx hash) or `failed` once there's a definitive receipt.
 *
 * It is the recovery half of the "never lose a pending tx" guarantee: SendScreen
 * and the dApp connection persist the pending record at submit time (so it
 * survives reload/restart); this reads those back and converges them.
 *
 * Conservative by design:
 *   - Only touches records with a userOpHash that haven't confirmed (txHash === '').
 *   - A null/transient result leaves the record pending (retried next run) — a
 *     timeout is never treated as failure.
 *   - Stops re-polling records older than RECONCILE_MAX_AGE_MS to avoid hammering
 *     the bundler forever for an op whose receipt the bundler has since pruned;
 *     such a record stays `pending` (honest "unknown"), surfaced for the user to
 *     check the explorer, rather than being wrongly marked failed.
 */

import { loadTransactions, updateTransaction } from './storage';
import { rpcCall } from './rpc-adapter';
import { autoAddReceivedTokens, type ReceiptLog } from './token-autoadd';

/** Stop re-polling a pending submission after this age (it stays pending = unknown). */
const RECONCILE_MAX_AGE_MS = 24 * 60 * 60 * 1000;
/** Don't reconcile more often than this (Home focus + interval call it a lot). */
const MIN_INTERVAL_MS = 12_000;

let _running = false;
let _lastRunAt = 0;

interface UserOpReceipt {
  success?: boolean;
  receipt?: { transactionHash?: string; logs?: ReceiptLog[] };
}

export interface UserOpResolution {
  confirmed: boolean;
  failed: boolean;
  txHash?: string;
}

/**
 * Poll the bundler once for a single UserOp's definitive receipt. Returns null
 * when there's no definitive answer yet (not landed / transient error) so the
 * caller can retry; never throws. Pure read — the caller decides what to persist.
 * Used by the detail sheet to live-update a still-pending transaction the user is
 * looking at, without waiting for the next Home-focus reconcile sweep.
 */
export async function pollUserOpReceipt(userOpHash: string, chainId: number): Promise<UserOpResolution | null> {
  if (!userOpHash) return null;
  try {
    const res = await rpcCall('eth_getUserOperationReceipt', [userOpHash], chainId);
    if (res.error || !res.result) return null;
    const r = res.result as UserOpReceipt;
    const txHash = r.receipt?.transactionHash;
    if (!txHash) return null;
    return { confirmed: r.success !== false, failed: r.success === false, txHash };
  } catch {
    return null;
  }
}

/**
 * Re-poll the bundler for any still-pending UserOp submissions belonging to
 * `address` and converge their stored status. Returns the number resolved
 * (confirmed or failed) this run, so the caller can refresh the feed only when
 * something actually changed. Safe to call frequently — throttled internally.
 */
export async function reconcilePendingTransactions(address: string): Promise<number> {
  if (!address) return 0;
  if (_running) return 0;
  if (Date.now() - _lastRunAt < MIN_INTERVAL_MS) return 0;
  _running = true;
  _lastRunAt = Date.now();

  let resolved = 0;
  try {
    const lc = address.toLowerCase();
    const txs = await loadTransactions().catch(() => []);
    const now = Date.now();
    const pending = txs.filter(
      (t) =>
        t.status === 'pending' &&
        !!t.userOpHash &&
        t.txHash === '' && // not yet confirmed on-chain
        t.from.toLowerCase() === lc &&
        now - t.timestamp * 1000 < RECONCILE_MAX_AGE_MS,
    );
    if (pending.length === 0) return 0;

    for (const tx of pending) {
      try {
        const res = await rpcCall('eth_getUserOperationReceipt', [tx.userOpHash], tx.chainId);
        // Transient (error) or not-ready-yet (null result): leave pending, retry next run.
        if (res.error || !res.result) continue;
        const r = res.result as UserOpReceipt;
        const txHash = r.receipt?.transactionHash;
        if (!txHash) continue;
        if (r.success === false) {
          await updateTransaction(tx.id, { status: 'failed' }).catch(() => {});
        } else {
          await updateTransaction(tx.id, { status: 'confirmed', txHash }).catch(() => {});
          // Silently list any token this tx net-delivered, from the AUTHENTIC receipt
          // logs (backstop for txs that confirmed while the app was closed). A pure
          // send has no positive delta for `from`, so it adds nothing.
          await autoAddReceivedTokens(tx.from, tx.chainId, r.receipt?.logs).catch(() => {});
        }
        resolved++;
      } catch {
        // Bundler unreachable this round — the op may still land; retry next run.
      }
    }
    console.log(`[Reconcile] ${pending.length} pending → resolved ${resolved}`);
  } finally {
    _running = false;
  }
  return resolved;
}
