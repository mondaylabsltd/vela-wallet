// Ported from src/services/wallet-state-core/feed-executor.ts @ c13e89d4 (spec 025).
/**
 * The only place the `activity_feed` core touches the outside world.
 *
 * Eight operations, one existing service call each — the vocabulary the core
 * declares (`ReadTxStore` / `ScanIncomingTransfers` / `DeleteTxRecord` /
 * `ResolveRecipientIdentity` / `Timer` / `Haptic`, and since PR 3
 * `ReadReceiveTime` / `WriteReceiveTime`). No branching on business
 * meaning: the dedupe, the batch fold, the tombstone filter, the celebration
 * gate, the "when to read again" choices and the repair of a receipt's time
 * (which records, how many a round, when to ask again) all live in Rust.
 *
 * Wire vs stored shape — the two things this file must get exactly right:
 *
 * - **`day_start_ms`.** The core groups by it and can never compute it: the
 *   device timezone is the shell's. It is `dayStartMs(timestamp)`, the same
 *   local-midnight key `useHomeController.ts:554` grouped on, so a record
 *   written at 23:30 local still heads its own day.
 * - **`timestamp` stays the stored epoch SECONDS**, and `usd` stays the legacy
 *   pre-formatted string exactly as persisted — the core parses it once, the way
 *   `txUsdValue` does.
 * - **`time_verified` is the stored `timeVerified` mark, and absent stays
 *   absent** (PR 3): no mark is what tells the core a receipt was written
 *   when its time could still be the clock's, and so is to be read again.
 *
 * A receipt's time (PR 3) is read here and never made here: `ReadReceiveTime`
 * answers the time of the transaction's own block, through the RPC pool, or
 * `null` — no clock, no guess, no retry (the core asks again when it means
 * to); `WriteReceiveTime` rewrites that one record behind the store's lock.
 *
 * A record whose `type` is present but not one of the six known values is
 * dropped rather than guessed at: it can be neither a feed item nor an
 * account-scoped transfer today (`loadActivityItems` and
 * `loadActivityTransactions` both fall through to "neither send nor receive"),
 * so dropping it is the same nothing — and inventing a `kind` for it would be a
 * lie the core would act on. Numeric fields are coerced fail-closed for the same
 * reason `session-executor.web.ts` clamps its index: `loadTransactions()` is an
 * unvalidated JSON parse, and a `chain_id` serde could not accept would fault
 * the core into a feed that never loads.
 *
 * Failure contract (shared effect loop): nothing rejects. Every rejection is
 * converted into the result variant that operation answers with.
 */

import { dayStartMs, syncReceivedTransfers } from '$lib/services/activity';
import { hapticSuccess } from '$lib/services/platform';
import { deleteTransaction, loadTransactions, rewriteReceiveTime } from '$lib/services/records';
import { poolRpcCall } from '$lib/services/rpc-pool';
import { resolveRecipientIdentity } from '$lib/services/recipient-identity';
import type { LocalTransaction } from '$lib/services/transactions-model';

import type { DappAction } from '$lib/core/generated/DappAction';
import type { DappSummary } from '$lib/core/generated/DappSummary';
import type { DappToken } from '$lib/core/generated/DappToken';
import type { FeedShellResult } from '$lib/core/generated/FeedShellResult';
import type { FeedTxKind } from '$lib/core/generated/FeedTxKind';
import type { FeedTxRecord } from '$lib/core/generated/FeedTxRecord';
import type { FeedTxStatus } from '$lib/core/generated/FeedTxStatus';
import type { TrackFailure } from '$lib/core/generated/TrackFailure';
import type { TrackMove } from '$lib/core/generated/TrackMove';
import type { TrackSettlement } from '$lib/core/generated/TrackSettlement';
import type { TrustSimJudgment } from '$lib/core/generated/TrustSimJudgment';
import type { FeedEffect, FeedOwnAccount, FeedRecordSink } from './feed-types';

const KINDS: FeedTxKind[] = [
	'send',
	'receive',
	'dapp_tx',
	'sign_message',
	'sign_typed_data',
	'connect'
];
const STATUSES: FeedTxStatus[] = ['pending', 'confirmed', 'failed'];
/** The kinds a dApp wrote (spec 093): a transaction, and its two signatures. */
const DAPP_KINDS: FeedTxKind[] = ['dapp_tx', 'sign_message', 'sign_typed_data'];
const ACTIONS: DappAction[] = [
	'call',
	'batch',
	'approve',
	'permit',
	'sign_in',
	'message',
	'typed_data',
	'blind_sign'
];
const FAILURES: TrackFailure[] = ['reverted', 'refused', 'not_sent'];
/** `u32::MAX` — the widest count or decimals serde takes. */
const U32_MAX = 4_294_967_295;

const asString = (value: unknown): string => (typeof value === 'string' ? value : '');

/** A non-negative safe integer, or 0 — a `u32` field serde must be able to take. */
function asCount(value: unknown): number {
	return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0 ? value : 0;
}

/** A finite f64, or 0 — `NaN`/`Infinity` serialize as `null` and would fault serde. */
function asNumber(value: unknown): number {
	return typeof value === 'number' && Number.isFinite(value) ? value : 0;
}

/**
 * A stored `value`. A dApp record keeps the page's own figure, and a page may
 * send it as a JSON number — which must not read as nothing (083 H2 review).
 * The number IS the figure the page sent, whatever rounding its own parse did,
 * so it is written out exactly (`BigInt`), in the digits the core reads —
 * never `String()`, which turns 1e21 wei into "1e+21". 0.01 of a coin is
 * 1e16 wei, already past 2^53, so a safe-integer rule would drop the common
 * case. Anything but a non-negative integer is no figure.
 */
function asValue(value: unknown): string {
	return typeof value === 'number' && Number.isInteger(value) && value >= 0
		? BigInt(value).toString()
		: asString(value);
}

const isRecord = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);

/** A `u32` serde accepts, or `undefined` when it is not one. */
const asU32 = (value: unknown): number | undefined =>
	typeof value === 'number' && Number.isInteger(value) && value >= 0 && value <= U32_MAX
		? value
		: undefined;

/**
 * The stored summary (`dappSummary`, spec 093), handed back exactly as the
 * core wrote it — or `null` when it is not one. The store is an unvalidated
 * JSON parse and one field serde cannot take would fault the whole feed, so a
 * summary with ANY field of the wrong shape is dropped whole, never repaired:
 * the record then reads by its kind, as one from before 093 does. Repairing a
 * field (dropping a bad `unlimited`, say) would let the shell change what the
 * row states.
 */
export function storedSummary(value: unknown): DappSummary | null {
	if (!isRecord(value) || !ACTIONS.includes(value.action as DappAction)) return null;
	const summary: DappSummary = { action: value.action as DappAction, calls: 0 };
	if (value.calls !== undefined) {
		const calls = asU32(value.calls);
		if (calls === undefined) return null;
		summary.calls = calls;
	}
	for (const key of [
		'contract',
		'spender',
		'token',
		'symbol',
		'amount',
		'signin_domain',
		'primary_type',
		'contract_name',
		'owner',
		'fee_token',
		'fee_amount'
	] as const) {
		const field = value[key];
		if (field === undefined || field === null) continue;
		if (typeof field !== 'string') return null;
		summary[key] = field;
	}
	if (value.decimals !== undefined && value.decimals !== null) {
		const decimals = asU32(value.decimals);
		if (decimals === undefined) return null;
		summary.decimals = decimals;
	}
	for (const key of ['unlimited', 'revoke'] as const) {
		const field = value[key];
		if (field === undefined) continue;
		if (typeof field !== 'boolean') return null;
		summary[key] = field;
	}
	if (value.expires_at !== undefined && value.expires_at !== null) {
		if (typeof value.expires_at !== 'number' || !Number.isFinite(value.expires_at)) return null;
		summary.expires_at = value.expires_at;
	}
	if (value.tokens !== undefined) {
		// Spec 097: the coins the sheet's reading named.
		if (!Array.isArray(value.tokens)) return null;
		const tokens: DappToken[] = [];
		for (const token of value.tokens) {
			const decimals = isRecord(token) ? asU32(token.decimals) : undefined;
			if (!isRecord(token) || decimals === undefined) return null;
			if (typeof token.address !== 'string' || typeof token.symbol !== 'string') return null;
			tokens.push({ address: token.address, symbol: token.symbol, decimals });
		}
		summary.tokens = tokens;
	}
	return summary;
}

/**
 * The stored settlement (`settlement`, spec 097), handed back exactly as the
 * tracker wrote it — or `null` when any part of it is not one. Dropped whole
 * for the reason `storedSummary` gives: a list of what moved with a line
 * missing would be a different account of it.
 */
export function storedSettlement(value: unknown): TrackSettlement | null {
	if (!isRecord(value)) return null;
	const settlement: TrackSettlement = {};
	if (value.moved !== undefined && value.moved !== null) {
		if (!Array.isArray(value.moved)) return null;
		const moved: TrackMove[] = [];
		for (const line of value.moved) {
			if (!isRecord(line) || typeof line.delta !== 'string') return null;
			if (line.token !== undefined && line.token !== null && typeof line.token !== 'string') {
				return null;
			}
			moved.push({ token: typeof line.token === 'string' ? line.token : null, delta: line.delta });
		}
		settlement.moved = moved;
	}
	if (value.failure !== undefined && value.failure !== null) {
		if (!FAILURES.includes(value.failure as TrackFailure)) return null;
		settlement.failure = value.failure as TrackFailure;
	}
	return settlement;
}

/**
 * The stored balance changes (`balanceChanges`, 083 F1), handed back exactly
 * as the sheet judged them — or `null` when any line is not one a judgment
 * can be. Dropped whole for the reason `storedSummary` gives: a list with a
 * line missing would be a different account of what moved.
 */
export function storedJudgments(value: unknown): TrustSimJudgment[] | null {
	if (!Array.isArray(value) || value.length === 0) return null;
	const judgments: TrustSimJudgment[] = [];
	for (const line of value) {
		if (!isRecord(line) || typeof line.delta !== 'string') return null;
		switch (line.type) {
			case 'native':
				judgments.push({ type: 'native', delta: line.delta });
				break;
			case 'erc20_trusted': {
				const decimals = asU32(line.decimals);
				if (typeof line.token !== 'string' || typeof line.symbol !== 'string') return null;
				if (decimals === undefined) return null;
				if (line.in_trusted_set !== undefined && typeof line.in_trusted_set !== 'boolean') {
					return null;
				}
				judgments.push({
					type: 'erc20_trusted',
					token: line.token,
					delta: line.delta,
					symbol: line.symbol,
					decimals,
					...(line.in_trusted_set === true ? { in_trusted_set: true } : {})
				});
				break;
			}
			case 'erc20_unverified':
				if (line.token !== null && line.token !== undefined && typeof line.token !== 'string') {
					return null;
				}
				judgments.push({
					type: 'erc20_unverified',
					token: typeof line.token === 'string' ? line.token : null,
					delta: line.delta
				});
				break;
			default:
				return null;
		}
	}
	return judgments;
}

/**
 * One stored record in the core's vocabulary, or `null` when it is not a record
 * this machine can speak about. `kind: null` is the legacy untyped row the core
 * reads as `send`, exactly as `t.type ?? 'send'` does.
 */
export function toFeedRecord(tx: LocalTransaction): FeedTxRecord | null {
	const rawKind = tx.type;
	if (rawKind !== undefined && !KINDS.includes(rawKind as FeedTxKind)) return null;
	const timestamp = asNumber(tx.timestamp);
	// What a dApp's row says beyond its money (083 H2, spec 093) — for its
	// transactions and its signatures alike: the origin it came from, the
	// intent it recorded, the core's summary and the sheet's balance changes,
	// each as stored. `dappUrl` only — `dappOrigin` holds the dApp's own name
	// when it gave one, and the core names the site from what it is handed.
	const dapp = DAPP_KINDS.includes(rawKind as FeedTxKind)
		? {
				dapp_url: typeof tx.dappUrl === 'string' && tx.dappUrl !== '' ? tx.dappUrl : null,
				intent: typeof tx.intent === 'string' && tx.intent !== '' ? tx.intent : null,
				summary: storedSummary(tx.dappSummary),
				balance_changes: storedJudgments(tx.balanceChanges),
				// Spec 097: how its operation ended, as the tracker proved it.
				settlement: storedSettlement(tx.settlement)
			}
		: {};
	return {
		id: asString(tx.id),
		user_op_hash: asString(tx.userOpHash),
		tx_hash: asString(tx.txHash),
		from: asString(tx.from),
		to: asString(tx.to),
		to_name: typeof tx.toName === 'string' ? tx.toName : null,
		value: asValue(tx.value),
		symbol: asString(tx.symbol),
		decimals: asCount(tx.decimals),
		logo_urls: Array.isArray(tx.logoUrls) ? tx.logoUrls.map(asString) : null,
		chain_id: asCount(tx.chainId),
		timestamp,
		// The grouping key the core cannot compute — the device timezone is ours.
		day_start_ms: dayStartMs(timestamp),
		status: STATUSES.includes(tx.status) ? tx.status : 'confirmed',
		kind: (rawKind as FeedTxKind | undefined) ?? null,
		usd: typeof tx.usd === 'string' ? tx.usd : null,
		// PR 3: the stored mark, as stored. Absent stays absent — that is what
		// marks a record for the core's repair — and anything but a boolean is
		// no mark (serde takes nothing else here, and a refused record would
		// fault the whole feed).
		...(typeof tx.timeVerified === 'boolean' ? { time_verified: tx.timeVerified } : {}),
		...dapp,
		// The call's data, for a dApp transaction (spec 082 RJ16): the core
		// decodes a plain token transfer's real recipient from it, and names
		// any other call's `to` as the contract it went to.
		call_data: rawKind === 'dapp_tx' ? firstCallData(tx.signedRequest) : null
	};
}

/**
 * The first call's `data` of the stored request (spec 082 RJ16): an
 * `eth_sendTransaction`'s own, or a `wallet_sendCalls` batch's first leg.
 * `null` when there is none, or it is not hex — the stored request is an
 * unvalidated parse, and the core is handed only what it can read.
 */
export function firstCallData(request: LocalTransaction['signedRequest']): string | null {
	const params = request?.params;
	if (!Array.isArray(params) || params.length === 0) return null;
	const first = params[0] as { data?: unknown; calls?: unknown } | null;
	if (!first || typeof first !== 'object') return null;
	const call =
		request?.method === 'wallet_sendCalls' && Array.isArray(first.calls)
			? (first.calls[0] as { data?: unknown } | undefined)
			: first;
	const data = call?.data;
	return typeof data === 'string' && /^0x[0-9a-fA-F]*$/.test(data) && data.length > 2 ? data : null;
}

/**
 * One receipt sweep per account at a time.
 *
 * `ScanIncomingTransfers` is a multi-chain `eth_getLogs` sweep, and the shell
 * issues it from three places that can overlap (the account hand-off, the focus
 * tick, the 10s Activity poll). `fetchTokens` solves the same problem for
 * balances with an in-flight promise; this is that, one layer up.
 *
 * A follower answers `0`, and that is not a fudge — it is the literal truth the
 * operation is defined to report: `new_count` is how many receipts THIS scan
 * persisted, and the run already in flight is the one persisting them. Handing
 * the leader's count to both callers would make the core believe two separate
 * batches landed and celebrate a backlog it already spent.
 */
const inFlightScans = new Map<string, Promise<number>>();

async function scanOnce(address: string): Promise<number> {
	const running = inFlightScans.get(address);
	if (running) {
		await running.catch(() => 0);
		return 0;
	}
	const scan = syncReceivedTransfers(address);
	inFlightScans.set(address, scan);
	try {
		return await scan;
	} finally {
		inFlightScans.delete(address);
	}
}

/** A whole hex quantity (`0x…`) as a number, or `null` when it is not one. */
function hexQuantity(value: unknown): number | null {
	if (typeof value !== 'string' || !/^0x[0-9a-fA-F]+$/.test(value)) return null;
	const n = Number.parseInt(value, 16);
	return Number.isSafeInteger(n) ? n : null;
}

/**
 * The time of the block that holds `txHash`, in Unix seconds — the chain's
 * own word, through the RPC pool: the transaction's receipt names its block,
 * and the block its time. `null` whenever either read gave no usable answer
 * (no receipt yet, an RPC error, a pool that gave up, a hex that does not
 * read). Never a clock's time, never a guess, and asked once: when to ask
 * again is the core's.
 */
export async function readReceiveTime(chainId: number, txHash: string): Promise<number | null> {
	try {
		const receipt = await poolRpcCall('eth_getTransactionReceipt', [txHash], chainId);
		if (receipt?.error) return null;
		const blockNumber = (receipt?.result as { blockNumber?: unknown } | null | undefined)
			?.blockNumber;
		if (hexQuantity(blockNumber) === null) return null;
		const block = await poolRpcCall('eth_getBlockByNumber', [blockNumber, false], chainId);
		if (block?.error) return null;
		return hexQuantity((block?.result as { timestamp?: unknown } | null | undefined)?.timestamp);
	} catch {
		return null;
	}
}

/** A cancellable sleep — the core's toast `Timer`, nothing more. */
function delay(ms: number, signal: AbortSignal): Promise<void> {
	return new Promise<void>((resolve) => {
		const timer = setTimeout(resolve, ms);
		signal.addEventListener(
			'abort',
			() => {
				clearTimeout(timer);
				resolve();
			},
			{ once: true }
		);
	});
}

export function createFeedExecutor(ownAccounts: () => FeedOwnAccount[], records: FeedRecordSink) {
	async function execute(effect: FeedEffect, signal: AbortSignal): Promise<FeedShellResult> {
		const operation = effect.operation;
		switch (operation.type) {
			case 'read_tx_store': {
				// The WHOLE store, unfiltered: the core owns the account filter (and
				// needs the un-owned rows to fold a batch's siblings).
				const stored = await loadTransactions();
				// The detail sheet reads the raw rows; `FeedTxRecord` is a lossy
				// projection, so they are kept here rather than round-tripped.
				records.storeLoaded(stored);
				return {
					type: 'store_loaded',
					records: stored
						.map(toFeedRecord)
						.filter((record): record is FeedTxRecord => record !== null),
					now_ms: Date.now(),
					// Echoed so the core can tell WHICH read this answers. A tick issues
					// the read and the scan together; without the echo a stale read
					// consumes the celebration the sync earned and a real receipt lands
					// with no toast, glow or haptic.
					read_id: operation.read_id
				};
			}
			case 'scan_incoming_transfers':
				// `syncReceivedTransfers` runs the whole discovery + persist pipeline
				// (token admission via the `token_trust` core on web, through
				// `incoming-transfers.web.ts`) and already answers 0 on any failure.
				return { type: 'sync_completed', new_count: asCount(await scanOnce(operation.address)) };
			case 'delete_tx_record':
				await deleteTransaction(operation.id);
				return { type: 'delete_committed', id: operation.id };
			case 'resolve_recipient_identity': {
				// The user's OWN accounts first — a local name, no network at all
				// (`useHomeController.ts:432-434`). `addr` arrives lowercased.
				const own = ownAccounts().find(
					(account) => account.address.toLowerCase() === operation.addr
				);
				if (own) return { type: 'alias_resolved', addr: operation.addr, name: own.name };
				// Then the identity waterfall (passkey index → name services);
				// `null` is the core's "no identity anywhere", never an invented name.
				const identity = await resolveRecipientIdentity(operation.addr);
				return { type: 'alias_resolved', addr: operation.addr, name: identity?.name ?? null };
			}
			case 'timer':
				await delay(operation.ms, signal);
				return { type: 'toast_expired', generation: operation.generation };
			case 'haptic':
				hapticSuccess();
				return { type: 'haptic_played' };
			case 'read_receive_time':
				// PR 3: the block's own time, or `null`. Transport only.
				return {
					type: 'receive_time_read',
					id: operation.id,
					timestamp_sec: await readReceiveTime(operation.chain_id, operation.tx_hash)
				};
			case 'write_receive_time': {
				// PR 3: that one record, behind the store's own lock. A time that
				// is not one is never written — `NaN` would be stored as `null`.
				const sec = operation.timestamp_sec;
				const ok =
					Number.isFinite(sec) && sec > 0
						? await rewriteReceiveTime(operation.id, sec).catch(() => false)
						: false;
				return { type: 'receive_time_written', id: operation.id, ok };
			}
		}
	}

	function toFailure(effect: FeedEffect, error: unknown): FeedShellResult {
		const operation = effect.operation;
		switch (operation.type) {
			case 'read_tx_store':
				// `loadTransactions().catch(() => [])` — ported verbatim: the store is
				// the source of truth even about emptiness.
				return {
					type: 'store_loaded',
					records: [],
					now_ms: Date.now(),
					read_id: operation.read_id
				};
			case 'scan_incoming_transfers':
				// The TS `catch { return 0 }`: a failed scan is a scan that found
				// nothing, so the feed never flickers behind it.
				return { type: 'sync_completed', new_count: 0 };
			case 'delete_tx_record':
				// The `.catch(warn).finally(drop the tombstone)` port — the row comes
				// back on the next reload, because it really is still in storage.
				console.warn('[Home] activity delete failed', error);
				return { type: 'delete_failed', id: operation.id };
			case 'resolve_recipient_identity':
				// Best effort, exactly like today's `catch { /* ignore */ }`; the core
				// remembers the address as attempted either way, so it is never re-asked.
				return { type: 'alias_resolved', addr: operation.addr, name: null };
			case 'timer':
				return { type: 'toast_expired', generation: operation.generation };
			case 'haptic':
				return { type: 'haptic_played' };
			case 'read_receive_time':
				// Not read: the record keeps its time and the core asks later.
				return { type: 'receive_time_read', id: operation.id, timestamp_sec: null };
			case 'write_receive_time':
				// Not written: the same, and the core's round is not left waiting.
				return { type: 'receive_time_written', id: operation.id, ok: false };
		}
	}

	return { execute, toFailure };
}
