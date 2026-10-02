// Ported from src/services/wallet-state-core/tx-tracker-executor.ts @ f9bcb278 — RN seams rewritten to the web modules; logic verbatim.
/**
 * The only place the `tx_tracker` core touches the outside world.
 *
 * Nine operations, each one existing service call. No branching on business
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
import { rpcCall } from '$lib/services/rpc-adapter';

import type { TrackEntryView } from '$lib/core/generated/TrackEntryView';
import type { TrackLifecycle } from '$lib/core/generated/TrackLifecycle';
import type { TrackView } from '$lib/core/generated/TrackView';
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
			submitted_at_ms: tx.timestamp * 1000,
			// Spec 082 T182: a may-have-been-sent op stays one across a reload —
			// its MaybeSent outcome, its NotSent end and its find-event. A row
			// from before 082 has neither field: `false` / unknown.
			maybe_sent: tx.maybeSent === true,
			submit_block: asBlock(tx.submitBlock)
		});
	}
	return records;
}

/** A stored block number the core's `u64` can take, else unknown. */
function asBlock(value: unknown): number | null {
	return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : null;
}

/** `eth_blockNumber`'s hex quantity, or `null` for anything else. */
function blockOf(result: unknown): number | null {
	if (typeof result !== 'string' || !/^0x[0-9a-fA-F]+$/.test(result)) return null;
	return asBlock(Number.parseInt(result, 16));
}

/** The chain head through the pool, or `null` when no node answered it. */
async function readHead(chainId: number): Promise<number | null> {
	try {
		const response = await rpcCall('eth_blockNumber', [], chainId);
		return response.error ? null : blockOf(response.result);
	} catch {
		return null;
	}
}

/**
 * The relay-independent landing check's one read (spec 082 ruling 8): the op's
 * own `UserOperationEvent` in a bounded window, through the pool, answered AS
 * IT CAME. A result is `logs_json`; a JSON-RPC error — a range limit the pool
 * now hands back instead of banning the endpoint (T180) — is `error_json`; a
 * pool that reached nobody is neither. Whether an error is a range limit is
 * the core's call (`rpc_pool::is_log_range_error`), never this file's.
 */
async function findOpEvent(
	operation: Extract<TrackEffect['operation'], { type: 'find_op_event' }>
): Promise<{ logs_json: string | null; error_json: string | null; head_block: number | null }> {
	const toHex = (block: number) => '0x' + block.toString(16);
	const logs =
		operation.from_block === null
			? Promise.resolve({ logs_json: null, error_json: null })
			: rpcCall(
					'eth_getLogs',
					[
						{
							address: operation.entry_point,
							topics: [operation.topic0, operation.user_op_hash],
							fromBlock: toHex(operation.from_block),
							toBlock: toHex(operation.to_block ?? operation.from_block)
						}
					],
					operation.chain_id
				).then(
					(response) =>
						response.error
							? { logs_json: null, error_json: JSON.stringify(response.error) }
							: { logs_json: JSON.stringify(response.result ?? null), error_json: null },
					() => ({ logs_json: null, error_json: null })
				);
	const [found, head_block] = await Promise.all([logs, readHead(operation.chain_id)]);
	return { ...found, head_block };
}

/**
 * The relay's bundle tx, read from the CHAIN (spec 082 RJ4, G38): the
 * `eth_getTransactionReceipt` answer through the chain pool, passed on as it
 * came — `"null"` when the tx is not mined, `null` (no answer) when no node
 * answered or it answered an error. Which of its logs are the op's own, and
 * whether the Safe inside executed, is the core's reading, never this file's.
 */
async function readTxReceipt(chainId: number, txHash: string): Promise<string | null> {
	try {
		const response = await rpcCall('eth_getTransactionReceipt', [txHash], chainId);
		return response.error ? null : JSON.stringify(response.result ?? null);
	} catch {
		return null;
	}
}

/** A hash as a log line names it: its first ten characters, never the whole. */
function short(hash: string | null | undefined): string {
	return hash ? hash.slice(0, 10) : '';
}

/**
 * The tracker's own log lines (spec 082 G61, FR-018): one per change of an
 * op's status — `tracker: 0x1234abcd not_sent`, `tracker: 0x1234abcd confirmed
 * tx 0xfeed1234` — so a verdict the panel reached is in the console, not only
 * on the sheet. `previous` is the view before; an op new to the view is
 * logged when it is already past pending.
 */
export function trackerLogLines(previous: TrackView, next: TrackView): string[] {
	const before = new Map<string, TrackEntryView>();
	for (const entry of previous.entries) before.set(entry.user_op_hash, entry);
	const lines: string[] = [];
	for (const entry of next.entries) {
		const was = before.get(entry.user_op_hash);
		if (was?.status === entry.status && was?.tx_hash === entry.tx_hash) continue;
		if (!was && (entry.status === 'pending' || entry.status === 'unreachable')) continue;
		const tx = entry.tx_hash ? ` tx ${short(entry.tx_hash)}` : '';
		lines.push(`tracker: ${short(entry.user_op_hash)} ${entry.status}${tx}`);
	}
	return lines;
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
				// Each step of a not-found streak is a line (G61): two of them past
				// the grace are what ends a may-have-been-sent op "not sent".
				if (status.status === 'not_found') console.log(`tracker: ${short(hash)} not_found`);
				return {
					type: 'status',
					user_op_hash: hash,
					status: status.status as TrackLifecycle,
					stage: status.stage ?? null,
					now_ms,
					// The relay's bundle tx — an explorer link while no receipt has.
					tx_hash: status.txHash ?? null
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
				// Spec 097: how it ended — what the receipt proved it moved, or why
				// it failed — kept with the records verbatim for the feed.
				if (operation.patch.settlement != null) patch.settlement = operation.patch.settlement;
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
				// The balance read is `holdings_moved`'s now (spec 082 RE8): the
				// core asks for it after a confirmation AND after a failed op that
				// spent gas, and never for one that did not land. A confirmation
				// the find-event saw on chain has no receipt poll behind it, so it
				// has no logs here — and no auto-add: token_trust is never handed
				// anything but a receipt's own logs.
				const logs = logsByHash.get(hash);
				logsByHash.delete(hash);
				if (logs && logs.length > 0) {
					const from = await senderOf(hash);
					if (from) ports.receiptLogsConfirmed(from, operation.chain_id, logs);
				}
				fromByHash.delete(hash);
				return { type: 'notified' };
			}

			case 'holdings_moved':
				// An op of ours landed on this chain — confirmed, or failed with
				// gas spent. The hero refetches past the token cache (issue 188).
				ports.confirmed(operation.chain_id);
				return { type: 'notified' };

			case 'find_op_event': {
				const found = await findOpEvent(operation);
				return {
					type: 'op_event',
					user_op_hash: normalize(operation.user_op_hash),
					now_ms: Date.now(),
					...found
				};
			}

			case 'tx_receipt': {
				// No auto-add from these logs: a bundle's receipt carries every op
				// in it, and token_trust is only ever handed an op's own receipt.
				const receipt_json = await readTxReceipt(operation.chain_id, operation.tx_hash);
				return {
					type: 'tx_receipt',
					user_op_hash: normalize(operation.user_op_hash),
					now_ms: Date.now(),
					receipt_json
				};
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
			case 'holdings_moved':
				return { type: 'notified' };
			case 'find_op_event':
				// No answer: the core reads the same window on the next tick.
				return {
					type: 'op_event',
					user_op_hash: normalize(operation.user_op_hash),
					now_ms,
					logs_json: null,
					error_json: null,
					head_block: null
				};
			case 'tx_receipt':
				// No answer: the core asks again at the receipt cadence.
				return {
					type: 'tx_receipt',
					user_op_hash: normalize(operation.user_op_hash),
					now_ms,
					receipt_json: null
				};
		}
	}

	return { execute, toFailure };
}
