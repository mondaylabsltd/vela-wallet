// Ported from src/services/wallet-state-core/tx-tracker-executor.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * The only place the `tx_tracker` core touches the outside world.
 *
 * Six operations, each one existing service call. No branching on business
 * meaning: every throttle (3 s receipt, 12 s status, 12 s single-flight
 * reconcile), the 120 s window, the 24 h abandon line and every verdict live in
 * Rust. What lives HERE is exactly what the core's module doc assigns to the
 * shell:
 *
 * - **The whole wording/instanceof layer.** `requestUserOpReceipt` already
 *   collapses `rpcCall` into the typed axis the core wants — an RPC error or a
 *   throw is `reachedBundler: false` (→ `ReceiptUnreachable`, NEVER a failure),
 *   no result or no `transactionHash` is `resolution: null` (→ `ReceiptPending`),
 *   `success !== false` is `confirmed` (→ `Receipt`) and `success === false` is
 *   `failed` (→ `ReceiptFailed`). `pollUserOpStatus` answers `null` for a null
 *   result, an RPC error or an older relay that has no such method (→
 *   `StatusUnavailable`). So the regexes that used to *be* the classification
 *   (`/dropped from the network/`, `instanceof UserOpRejectedError`) have no
 *   remaining reader on web.
 * - **The clock.** Every time-bearing result carries `now_ms`; the core owns
 *   cadence but never reads a clock.
 * - **The shared 3 s receipt throttle (invariant ⑤).** Going through
 *   `requestUserOpReceipt` rather than a raw `rpcCall` is load-bearing: the
 *   receipt sheet (`TransactionReceipt.tsx`), the detail sheet and
 *   `safe-transaction.ts`'s own `waitForReceipt` join the SAME in-flight request
 *   and the SAME cooldown, so a hash watched by four surfaces still costs one
 *   `eth_getUserOperationReceipt` every 3 s. The core coalesces its own side
 *   (one in-flight request per hash); this cache coalesces across machines.
 * - **The receipt's own by-products.** `NotifyConfirmed` carries no payload
 *   beyond the hash — the AUTHENTIC logs and the sender are facts the shell
 *   already holds from the poll it just made, and they are what `token_trust`
 *   admits tokens from. They are cached per hash here and consumed once.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with — and for this
 * machine the "failure" variants are all the honest-unknown ones, never a
 * verdict.
 */

import { loadTransactions, updateTransactions } from '$lib/services/records';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { pollUserOpStatus, requestUserOpReceipt } from '$lib/services/tx-reconciler';

import type { TrackLifecycle } from '$lib/core/generated/TrackLifecycle';
import type { TrackPendingRecord } from '$lib/core/generated/TrackPendingRecord';
import type { TrackShellResult } from '$lib/core/generated/TrackShellResult';
import type { TrustReceiptLog } from '$lib/core/generated/TrustReceiptLog';
import type { TrackEffect, TrackShellPorts } from './tracker-types';

/**
 * The two record kinds a UserOp submission produces — `send` (the send screen,
 * one row per batch recipient) and `dapp_tx` (the signing sheet). `type` is
 * optional on older rows and defaults to `send`, exactly as `storage.ts` says.
 * Everything else in the store (receives, signatures, connections) has no
 * userOpHash to converge.
 */
const TRACKED_TYPES = new Set(['send', 'dapp_tx']);

/**
 * Per-hash by-products of the last receipt poll. Bounded because a long-lived
 * tab can watch many ops; the cap is the same order as the sign resident's
 * transport table, and the entries are consumed by their own `NotifyConfirmed`.
 */
const MAX_TRACKED_RECEIPTS = 64;

/** `LocalTransaction.status`, narrowed to what a patch may write. */
type PatchStatus = 'confirmed' | 'failed';

function normalize(hash: string): string {
	return hash.toLowerCase();
}

/** A receipt log, kept only if it has the three fields the netting reads. */
function toTrustLog(log: unknown): TrustReceiptLog | null {
	const candidate = log as { address?: unknown; topics?: unknown; data?: unknown } | null;
	if (!candidate || typeof candidate.address !== 'string') return null;
	if (!Array.isArray(candidate.topics)) return null;
	return {
		address: candidate.address,
		topics: candidate.topics.filter((topic): topic is string => typeof topic === 'string'),
		data: typeof candidate.data === 'string' ? candidate.data : '0x'
	};
}

/**
 * The still-pending submissions the reconcile sweep answers with — the union of
 * the two scans it replaces: `tx-reconciler.ts:217-224` (any `send`) and the
 * dApp startup scan (`dapp-connection.tsx:1038-1040`, `dapp_tx`).
 *
 * Deliberately NOT filtered by account: the TS reconciler filtered on the Home
 * account because it was called with one, and a pending op belonging to another
 * local account was simply never converged. The core keys everything by hash and
 * patches records by id, so the honest superset is the right answer — see the
 * behaviour note in the integration report.
 *
 * The 24 h line is left to the core (invariant ④): it drops records that old
 * itself, so the filter here stays a pure "is this a live submission" read.
 */
function toPendingRecords(txs: LocalTransaction[]): TrackPendingRecord[] {
	const records: TrackPendingRecord[] = [];
	for (const tx of txs) {
		if (tx.status !== 'pending') continue;
		if (!tx.userOpHash) continue;
		if (tx.txHash !== '') continue; // already confirmed on-chain
		if (!TRACKED_TYPES.has(tx.type ?? 'send')) continue;
		records.push({
			record_id: tx.id,
			user_op_hash: tx.userOpHash,
			chain_id: tx.chainId,
			// Stored in SECONDS; the core measures every deadline in epoch ms.
			submitted_at_ms: tx.timestamp * 1000
		});
	}
	return records;
}

export function createTxTrackerExecutor(ports: TrackShellPorts) {
	/** hash → the authentic logs of the receipt that confirmed it. */
	const logsByHash = new Map<string, TrustReceiptLog[]>();
	/** hash → the account that submitted it (`LocalTransaction.from`). */
	const fromByHash = new Map<string, string>();

	function remember<T>(map: Map<string, T>, key: string, value: T): void {
		if (map.size >= MAX_TRACKED_RECEIPTS && !map.has(key)) {
			const oldest = map.keys().next().value;
			if (oldest !== undefined) map.delete(oldest);
		}
		map.set(key, value);
	}

	/**
	 * Who received what this op delivered. The submitting account is
	 * `LocalTransaction.from` — the very value `autoAddReceivedTokens(tx.from, …)`
	 * and `dapp-connection.tsx:786-802` passed. Cached from the reconcile sweep;
	 * otherwise read back from the store, which is where the record was written
	 * before this op could ever be tracked (send: `RecordsPersisted` precedes
	 * `TrackSubmitted`; dApp: §4's record precedes the receipt wait).
	 *
	 * No sender, no auto-add: token_trust is never handed a guess.
	 */
	async function senderOf(hash: string): Promise<string | null> {
		const cached = fromByHash.get(hash);
		if (cached) return cached;
		const txs = await loadTransactions().catch(() => []);
		const match = txs.find((tx) => !!tx.userOpHash && normalize(tx.userOpHash) === hash);
		if (!match) return null;
		remember(fromByHash, hash, match.from);
		return match.from;
	}

	async function execute(effect: TrackEffect): Promise<TrackShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'now':
				return { type: 'clock', now_ms: Date.now() };

			case 'poll_receipt': {
				const hash = normalize(operation.user_op_hash);
				// Never throws: its own try/catch already answers `reachedBundler:false`.
				const outcome = await requestUserOpReceipt(operation.user_op_hash, operation.chain_id);
				const now_ms = Date.now();
				if (!outcome.reachedBundler) {
					// Timeout / network / RPC error. NOT a failure (invariant ①), and
					// honestly distinct from "the bundler answered, nothing yet" (⑧).
					return { type: 'receipt_unreachable', user_op_hash: hash, now_ms };
				}
				const resolution = outcome.resolution;
				if (!resolution) return { type: 'receipt_pending', user_op_hash: hash, now_ms };
				const txHash = resolution.txHash ?? '';
				if (!txHash) return { type: 'receipt_pending', user_op_hash: hash, now_ms };
				if (resolution.failed) {
					// `success === false` — dropped or reverted. The one receipt shape
					// that may fail a record (invariant ③).
					return { type: 'receipt_failed', user_op_hash: hash, tx_hash: txHash, now_ms };
				}
				// Hold the authentic logs for the `NotifyConfirmed` that follows the
				// patch — they are the only thing token_trust may auto-add from.
				const logs = (resolution.logs ?? [])
					.map(toTrustLog)
					.filter((log): log is TrustReceiptLog => log !== null);
				remember(logsByHash, hash, logs);
				// With the authentic logs: the core decides whether the Safe inside
				// the op actually executed (spec 038 #D1 — `ExecutionFailure` under
				// a `success: true` is a failed payment, not a confirmed one).
				return { type: 'receipt_with_logs', user_op_hash: hash, tx_hash: txHash, now_ms, logs };
			}

			case 'poll_status': {
				const hash = normalize(operation.user_op_hash);
				// Never throws either: a null/error/older-relay answer is `null`.
				const status = await pollUserOpStatus(operation.user_op_hash, operation.chain_id);
				const now_ms = Date.now();
				if (!status) return { type: 'status_unavailable', user_op_hash: hash, now_ms };
				return {
					type: 'status',
					user_op_hash: hash,
					status: status.status as TrackLifecycle,
					stage: status.stage ?? null,
					now_ms
				};
			}

			case 'load_pending_txs': {
				// `loadTransactions().catch(() => [])`, verbatim: a store that cannot be
				// read answers an empty sweep, never a fault.
				const txs = await loadTransactions().catch(() => []);
				const records = toPendingRecords(txs);
				for (const tx of txs) {
					if (tx.userOpHash) remember(fromByHash, normalize(tx.userOpHash), tx.from);
				}
				return { type: 'records_loaded', records, now_ms: Date.now() };
			}

			case 'update_tx_records': {
				const status: PatchStatus = operation.patch.status === 'failed' ? 'failed' : 'confirmed';
				const patch: Partial<LocalTransaction> =
					operation.patch.tx_hash != null
						? { status, txHash: operation.patch.tx_hash }
						: { status };
				// ONE atomic read-modify-write for every sibling of a batch, same ids,
				// in place — never a second record (invariant ⑦). Best effort, as every
				// TS call site's `.catch(() => {})` was.
				await updateTransactions(operation.ids, patch).catch(() => {});
				// `activity_feed`'s `ReconcileCompleted`: verdicts landed, so the feed
				// re-reads the store — and deliberately does not celebrate them.
				ports.feedReconciled(operation.ids.length);
				return { type: 'records_patched' };
			}

			case 'notify_confirmed': {
				const hash = normalize(operation.user_op_hash);
				const logs = logsByHash.get(hash);
				logsByHash.delete(hash);
				if (logs && logs.length > 0) {
					const from = await senderOf(hash);
					if (from) ports.receiptLogsConfirmed(from, operation.chain_id, logs);
				}
				fromByHash.delete(hash);
				return { type: 'notified' };
			}
		}
	}

	/**
	 * The defensive tail. Every operation above swallows its own errors, so this
	 * is only ever reached by a fault in the plumbing — and for this machine the
	 * only safe answers are the ones that keep money in flight *pending*.
	 */
	function toFailure(effect: TrackEffect): TrackShellResult {
		const operation = effect.operation;
		const now_ms = Date.now();
		switch (operation.type) {
			case 'now':
				return { type: 'clock', now_ms };
			case 'poll_receipt':
				// Unreachable, never failed (invariant ①).
				return {
					type: 'receipt_unreachable',
					user_op_hash: normalize(operation.user_op_hash),
					now_ms
				};
			case 'poll_status':
				return {
					type: 'status_unavailable',
					user_op_hash: normalize(operation.user_op_hash),
					now_ms
				};
			case 'load_pending_txs':
				return { type: 'records_loaded', records: [], now_ms };
			case 'update_tx_records':
				// The core must still be told, or the entry never leaves its
				// patch-pending state — and the feed is told nothing changed.
				return { type: 'records_patched' };
			case 'notify_confirmed':
				return { type: 'notified' };
		}
	}

	return { execute, toFailure };
}
