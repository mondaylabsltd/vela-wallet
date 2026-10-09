// Ported from src/services/tx-reconciler.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
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

import { loadTransactions, updateTransaction } from './records';
import { rpcCall, USER_OP_STATUS_METHOD } from './rpc-adapter';
import { parseUserOpStatus } from '$lib/core/kernels';
import { autoAddReceivedTokens, type ReceiptLog } from './token-autoadd';

/** Stop re-polling a pending submission after this age (it stays pending = unknown). */
const RECONCILE_MAX_AGE_MS = 24 * 60 * 60 * 1000;
/** Don't reconcile more often than this (Home focus + interval call it a lot). */
const MIN_INTERVAL_MS = 12_000;
/** A UserOp receipt is produced by the bundler asynchronously. Multiple surfaces can be
 * interested in the same hash (the send receipt, its background waiter, and Activity), so
 * coalesce them and never repeatedly hit the bundler faster than this. */
export const USER_OP_RECEIPT_POLL_INTERVAL_MS = 3_000;
/**
 * How early an ask may come against that cooldown — the core's
 * `tx_tracker::RECEIPT_TICK_SLACK_MS`, for the same reason (issue 464): the
 * tracker asks on a 3 s timer, and a tick that reads 2 999 ms after the last
 * ask must still reach the relay, or it waits a whole extra tick (6 s).
 */
export const USER_OP_RECEIPT_TICK_SLACK_MS = 250;

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
	/** Authentic receipt logs are only used by the background reconciler. */
	logs?: ReceiptLog[];
}

export interface UserOpReceiptPoll {
	/** A definitive receipt, or null while the UserOp has not landed yet. */
	resolution: UserOpResolution | null;
	/** False only when the bundler could not answer this request. */
	reachedBundler: boolean;
}

/**
 * The relay's view of an op that has no receipt yet. A null receipt is ambiguous —
 * it means "not landed", which covers both "any second now" and "refused, and it is
 * never landing". Only this endpoint separates them.
 */
export type UserOpLifecycle =
	'not_found' | 'queued' | 'not_submitted' | 'submitted' | 'rejected' | 'included' | 'failed';

export interface UserOpStatus {
	status: UserOpLifecycle;
	/** Executor stage that last touched the op, e.g. `in_band_settlement_hold`. */
	stage?: string;
	/** Human-readable diagnostic from that stage — for logs, never for a verdict. */
	detail?: string;
	/** The relay's bundle tx, when it names one (079 D2's explorer link). */
	txHash?: string;
}

/**
 * Ask the relay what became of an op. Never throws: an unreachable or older relay
 * simply yields null, and callers fall back to receipt-only behaviour.
 *
 * The method's name and the reading of its answer are the core's
 * (`tx_tracker::USER_OP_STATUS_METHOD`, `parse_user_op_status` — spec 082 RA7):
 * the web asked a spelling the relay never served (G13), and a status string
 * the core does not know parses to nothing rather than to a guess.
 */
export async function pollUserOpStatus(
	userOpHash: string,
	chainId: number
): Promise<UserOpStatus | null> {
	if (!userOpHash) return null;
	try {
		const res = await rpcCall(USER_OP_STATUS_METHOD, [userOpHash], chainId);
		if (res.error || !res.result || typeof res.result !== 'object') return null;
		const answer = parseUserOpStatus(JSON.stringify(res.result));
		if (!answer) return null;
		// The executor's own words go to the log line that names a refusal;
		// nothing decides on them.
		const detail = (res.result as { last_executor_error?: unknown }).last_executor_error;
		return {
			status: answer.status,
			stage: answer.stage ?? undefined,
			detail: typeof detail === 'string' ? detail : undefined,
			txHash: answer.tx_hash ?? undefined
		};
	} catch {
		return null;
	}
}

interface ReceiptPollCacheEntry {
	/**
	 * When the last request for this op was SENT — the cooldown counts from
	 * here (issue 464). It used to count from the answer: an answer 0.3 s after
	 * its 3 s tick left the next tick 2.7 s out, served from this cache, and
	 * the relay was asked every 6 s, not 3.
	 */
	askedAt?: number;
	outcome?: UserOpReceiptPoll;
	pending?: Promise<UserOpReceiptPoll>;
}

const receiptPollCache = new Map<string, ReceiptPollCacheEntry>();

function receiptPollKey(userOpHash: string, chainId: number): string {
	return `${chainId}:${userOpHash.toLowerCase()}`;
}

/** Test-only reset for the module-level coalescing/throttle state. */
export function _resetUserOpReceiptPollCache(): void {
	receiptPollCache.clear();
}

/**
 * Make (or join) the one permitted receipt request for this UserOp. A pending request is
 * always shared; an unresolved/null answer is retained until three seconds after the ASK
 * that produced it (less the tick slack), then the next caller asks again. This is
 * deliberately a small, local throttle rather than a global one: independent UserOps may
 * still settle concurrently.
 */
export async function requestUserOpReceipt(
	userOpHash: string,
	chainId: number
): Promise<UserOpReceiptPoll> {
	if (!userOpHash) return { resolution: null, reachedBundler: false };
	const key = receiptPollKey(userOpHash, chainId);
	const existing = receiptPollCache.get(key);
	if (existing?.pending) return existing.pending;
	if (existing?.outcome?.resolution) return existing.outcome;
	if (
		existing?.askedAt !== undefined &&
		Date.now() - existing.askedAt < USER_OP_RECEIPT_POLL_INTERVAL_MS - USER_OP_RECEIPT_TICK_SLACK_MS
	) {
		return existing.outcome ?? { resolution: null, reachedBundler: false };
	}

	const entry: ReceiptPollCacheEntry = existing ?? {};
	entry.askedAt = Date.now();
	const request = (async (): Promise<UserOpReceiptPoll> => {
		try {
			const res = await rpcCall('eth_getUserOperationReceipt', [userOpHash], chainId);
			if (res.error) return { resolution: null, reachedBundler: false };
			if (!res.result) return { resolution: null, reachedBundler: true };
			const r = res.result as UserOpReceipt;
			const txHash = r.receipt?.transactionHash;
			if (!txHash) return { resolution: null, reachedBundler: true };
			return {
				reachedBundler: true,
				resolution: {
					confirmed: r.success !== false,
					failed: r.success === false,
					txHash,
					logs: r.receipt?.logs
				}
			};
		} catch {
			return { resolution: null, reachedBundler: false };
		}
	})();
	entry.pending = request;
	receiptPollCache.set(key, entry);
	try {
		const outcome = await request;
		entry.outcome = outcome;
		return outcome;
	} finally {
		entry.pending = undefined;
	}
}

/**
 * Poll the bundler once for a single UserOp's definitive receipt. Returns null
 * when there's no definitive answer yet (not landed / transient error) so the
 * caller can retry; never throws. Pure read — the caller decides what to persist.
 * Used by the detail sheet to live-update a still-pending transaction the user is
 * looking at, without waiting for the next Home-focus reconcile sweep.
 */
export async function pollUserOpReceipt(
	userOpHash: string,
	chainId: number
): Promise<UserOpResolution | null> {
	return (await requestUserOpReceipt(userOpHash, chainId)).resolution;
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
				now - t.timestamp * 1000 < RECONCILE_MAX_AGE_MS
		);
		if (pending.length === 0) return 0;

		for (const tx of pending) {
			try {
				const outcome = await requestUserOpReceipt(tx.userOpHash, tx.chainId);
				// Transient (error) or not-ready-yet (null result): leave pending, retry next run.
				const resolution = outcome.resolution;
				if (!resolution) continue;
				if (resolution.failed) {
					await updateTransaction(tx.id, { status: 'failed' }).catch(() => {});
				} else {
					await updateTransaction(tx.id, { status: 'confirmed', txHash: resolution.txHash! }).catch(
						() => {}
					);
					// Silently list any token this tx net-delivered, from the AUTHENTIC receipt
					// logs (backstop for txs that confirmed while the app was closed). A pure
					// send has no positive delta for `from`, so it adds nothing.
					await autoAddReceivedTokens(tx.from, tx.chainId, resolution.logs).catch(() => {});
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
