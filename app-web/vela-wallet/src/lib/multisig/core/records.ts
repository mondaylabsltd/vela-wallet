/**
 * A multisig's executed transactions, in the activity — executed on this
 * device or on any other member's (found on chain by their
 * `ExecutionSuccess`). Every kind writes a row (r4a-3): a transfer one per
 * transfer, `from` the multisig — the same store and shape a Vela account's
 * own sends are written to — and anything else (a contract call, a batch of
 * calls, a member change, a rename, a cancellation, ms-9) one row titled as
 * its proposal was, moving nothing it does not move. Row ids are the one
 * scheme of `row-id.ts`, so a tap opens the transaction's sheet (r4a-4).
 *
 * The chain's history is read in windows (r4b-2): a node that refuses a
 * window as too wide (Arc's public node, drpc's 10 000 blocks) is asked
 * again with a narrower one — the pool's own rule, `getLogsRangeCap` — and
 * the search is bounded by when the proposal was made. A history that
 * cannot be read is said as that ({@link HistoryUnreadableError}), never
 * read as "nothing happened".
 */
import { fillTemplate } from '$lib/i18n/fill';
import { loadTransactions, mergeTransactions, saveTransactions } from '$lib/services/records';
import { getLogsRangeCap, poolRpcCall } from '$lib/services/rpc-pool';
import type { LocalTransaction } from '$lib/services/transactions-model';
import type { MultisigMessages } from '../messages';
import { actionTitle, tokenInfo } from './display';
import { transfersOf } from './kernel';
import { executedRowId, legacyRowId } from './row-id';
import { plainUnits } from './units';
import type { ProposalItem } from './proposals.svelte';

/**
 * The page's words for a row's title — the proposal's own (`actionTitle`).
 * The multisig host hands them over ({@link useRecordWords}); until then a
 * cancellation keeps its fallback title and anything else its kind's.
 */
let words: MultisigMessages | null = null;
let cancelTitle = 'Cancel transaction #{{nonce}}';

/** The page's words for the rows' titles. */
export function useRecordWords(m: MultisigMessages): void {
	words = m;
	if (m.action.cancel) cancelTitle = m.action.cancel;
}

/** The page's words for a cancellation's row (`{{nonce}}`). */
export function useCancelTitle(template: string): void {
	if (template) cancelTitle = template;
}

async function titleOf(item: ProposalItem): Promise<string> {
	const action = item.review.action;
	if (action?.kind === 'cancel' && (!words || words.action.cancel === '')) {
		return fillTemplate(cancelTitle, { nonce: item.nonce.toString() });
	}
	if (!words) return '';
	return actionTitle(words, item.chainId, action, item.nonce).catch(() => '');
}

/** The rows executing `item` writes: one per transfer, and one for whatever else it does. */
export async function executionRows(
	multisig: string,
	item: ProposalItem,
	txHash: string,
	userOpHash: string,
	at: number
): Promise<LocalTransaction[]> {
	const action = item.review.action;
	const transfers = transfersOf(action);
	const rows: LocalTransaction[] = [];
	for (const [i, t] of transfers.entries()) {
		const info = await tokenInfo(item.chainId, t.token).catch(() => ({ symbol: '', decimals: 18 }));
		rows.push({
			id: executedRowId(item.chainId, item.safeTxHash, i),
			userOpHash,
			txHash,
			from: multisig,
			to: t.to,
			value: plainUnits(t.amount, info.decimals),
			symbol: info.symbol,
			decimals: info.decimals,
			// Round 5: which token it was, for Activity's lookalike note.
			...(t.token ? { token: t.token } : {}),
			chainId: item.chainId,
			timestamp: at,
			status: 'confirmed',
			type: 'send'
		});
	}
	// A transfer, or a batch of transfers alone, is said by its transfers.
	const onlyTransfers =
		action?.kind === 'transfer' ||
		(action?.kind === 'batch' && action.calls.every((c) => c.action.kind === 'transfer'));
	if (onlyTransfers && rows.length > 0) return rows;
	// Anything else — a call, calls in a batch, a member change, a rename, a
	// cancellation — is one row titled as its proposal, moving only the coin
	// a call sends.
	const to =
		action?.kind === 'call'
			? action.to
			: action?.kind === 'batch'
				? (item.tx?.to ?? multisig)
				: multisig;
	const sends = action?.kind === 'call' ? BigInt(action.value) : 0n;
	const coin =
		sends > 0n
			? await tokenInfo(item.chainId, null).catch(() => ({ symbol: '', decimals: 18 }))
			: { symbol: '', decimals: 18 };
	rows.push({
		id: executedRowId(item.chainId, item.safeTxHash, rows.length),
		userOpHash,
		txHash,
		from: multisig,
		to,
		value: sends > 0n ? plainUnits(sends.toString(), coin.decimals) : '0',
		symbol: coin.symbol,
		decimals: coin.decimals,
		chainId: item.chainId,
		timestamp: at,
		status: 'confirmed',
		type: 'dapp_tx',
		intent: await titleOf(item),
		dappSummary: {
			action: 'call',
			calls: action?.kind === 'batch' ? action.calls.length : 1,
			contract: to.toLowerCase()
		}
	});
	return rows;
}

/** Executed from this device: its rows, now. */
export async function recordExecution(
	multisig: string,
	item: ProposalItem,
	txHash: string,
	userOpHash: string,
	/** Unix seconds of its block, when known; else now. */
	at?: number
): Promise<void> {
	const rows = await executionRows(
		multisig,
		item,
		txHash,
		userOpHash,
		at ?? Math.floor(Date.now() / 1000)
	);
	await saveTransactions(rows);
	remember(item.chainId, multisig, item.safeTxHash, { txHash, block: null });
}

// -- the chain's history, in windows ------------------------------------------------

/** `keccak256("ExecutionSuccess(bytes32,uint256)")`: its first topic is the safeTxHash. */
export const EXECUTION_SUCCESS =
	'0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e';

/** The widest window first asked for: what most public nodes take (drpc's free tier). */
export const FIRST_LOG_SPAN = 10_000;
/** The most windows one search reads: past that, the history is said as unreadable. */
export const MAX_LOG_WINDOWS = 400;
/** Before the proposal was made, by this much: its time is the mailbox's word. */
const CREATED_MARGIN_MS = 60 * 60 * 1000;

/** The chain's history could not be read: nothing can be said of what it holds. */
export class HistoryUnreadableError extends Error {
	constructor(
		readonly chainId: number,
		detail: string
	) {
		super(`the history of chain ${chainId} could not be read: ${detail}`);
		this.name = 'HistoryUnreadableError';
	}
}

/** The widest window each chain's nodes were seen to take, this session. */
const spans = new Map<number, number>();

const hex = (n: bigint) => `0x${n.toString(16)}`;

async function rpc(chainId: number, method: string, params: unknown[]) {
	try {
		return await poolRpcCall(method, params, chainId);
	} catch (error) {
		throw new HistoryUnreadableError(
			chainId,
			error instanceof Error ? error.message : String(error)
		);
	}
}

interface Log {
	transactionHash: string;
	blockNumber: string;
}

/**
 * The newest log matching `filter` between `from` and `to` (inclusive),
 * newest window first, or `null` when there is none. A window a node refuses
 * as too wide is asked again narrower — at the width it names, else half;
 * any other failure throws {@link HistoryUnreadableError}.
 */
export async function searchLogs(
	chainId: number,
	filter: { address: string; topics: (string | null)[] },
	from: bigint,
	to: bigint
): Promise<Log | null> {
	let hi = to;
	let span = BigInt(spans.get(chainId) ?? FIRST_LOG_SPAN);
	let windows = 0;
	while (hi >= from) {
		windows += 1;
		if (windows > MAX_LOG_WINDOWS) throw new HistoryUnreadableError(chainId, 'too many windows');
		const lo = hi - span + 1n > from ? hi - span + 1n : from;
		const answer = await rpc(chainId, 'eth_getLogs', [
			{ ...filter, fromBlock: hex(lo), toBlock: hex(hi) }
		]);
		if (answer.error) {
			const cap = getLogsRangeCap(answer.error);
			if (cap === null || span <= 1n) {
				throw new HistoryUnreadableError(chainId, answer.error.message ?? 'eth_getLogs failed');
			}
			const named = BigInt(Math.floor(cap));
			span = named > 0n && named < span ? named : span / 2n > 0n ? span / 2n : 1n;
			spans.set(chainId, Number(span));
			windows -= 1;
			continue;
		}
		if (!Array.isArray(answer.result)) {
			throw new HistoryUnreadableError(chainId, 'eth_getLogs answered no list');
		}
		const logs = answer.result as Log[];
		if (logs.length > 0) return logs[logs.length - 1];
		hi = lo - 1n;
	}
	return null;
}

async function blockOf(chainId: number, tag: string) {
	const answer = await rpc(chainId, 'eth_getBlockByNumber', [tag, false]);
	const block = answer.result as { number?: string; timestamp?: string } | null | undefined;
	if (answer.error || !block?.number || !block.timestamp) {
		throw new HistoryUnreadableError(chainId, answer.error?.message ?? `no block ${tag}`);
	}
	return { number: BigInt(block.number), time: Number(BigInt(block.timestamp)) };
}

/**
 * A block at or before `ms` (Unix ms), and the head: estimated from the
 * chain's own pace over its last window, checked against that block's time
 * and moved back until it is not later — a few reads, not a search.
 */
export async function blockBefore(
	chainId: number,
	ms: number
): Promise<{ from: bigint; head: bigint }> {
	const head = await blockOf(chainId, 'latest');
	const at = Math.floor(ms / 1000);
	if (at >= head.time) return { from: head.number, head: head.number };
	const back = head.number > BigInt(FIRST_LOG_SPAN) ? BigInt(FIRST_LOG_SPAN) : head.number;
	if (back === 0n) return { from: 0n, head: head.number };
	const sample = await blockOf(chainId, hex(head.number - back));
	const perBlock = Math.max(0.001, (head.time - sample.time) / Number(back));
	let distance = BigInt(Math.ceil(((head.time - at) / perBlock) * 1.1)) + 100n;
	for (let tries = 0; tries < 6; tries += 1) {
		if (distance >= head.number) return { from: 0n, head: head.number };
		const block = await blockOf(chainId, hex(head.number - distance));
		if (block.time <= at) return { from: block.number, head: head.number };
		distance *= 2n;
	}
	return { from: 0n, head: head.number };
}

/**
 * The multisig's nonce at `block`, or `-1n` when the node cannot say (its
 * old state pruned): only a nonce read there can show a search began late.
 */
async function nonceAt(chainId: number, multisig: string, block: bigint): Promise<bigint> {
	const answer = await poolRpcCall(
		'eth_call',
		[{ to: multisig, data: '0xaffed0e0' }, hex(block)],
		chainId
	).catch(() => null);
	const raw = answer?.result;
	return typeof raw === 'string' && /^0x[0-9a-fA-F]{1,64}$/.test(raw) ? BigInt(raw) : -1n;
}

/** What a lookup found: the executing transaction, or none (final once its number was used). */
type Found = { txHash: string; block: bigint | null } | null;

const LOOKUPS_KEY = 'vela.multisigExecutions';
const LOOKUPS_CAP = 500;

function lookupKey(chainId: number, multisig: string, safeTxHash: string): string {
	return `${chainId}:${multisig.toLowerCase()}:${safeTxHash.toLowerCase()}`;
}

/** Lookups already answered for good: an execution is found once; a voided number stays voided. */
function readLookups(): Record<string, string> {
	try {
		return JSON.parse(localStorage.getItem(LOOKUPS_KEY) ?? '{}') as Record<string, string>;
	} catch {
		return {};
	}
}

/** The same, this session: kept where storage is not (a private window, a test). */
const memo = new Map<string, string>();

function remember(chainId: number, multisig: string, safeTxHash: string, found: Found): void {
	memo.set(lookupKey(chainId, multisig, safeTxHash), found ? found.txHash : 'none');
	try {
		const all = readLookups();
		all[lookupKey(chainId, multisig, safeTxHash)] = found ? found.txHash : 'none';
		const keys = Object.keys(all);
		for (const old of keys.slice(0, Math.max(0, keys.length - LOOKUPS_CAP))) delete all[old];
		localStorage.setItem(LOOKUPS_KEY, JSON.stringify(all));
	} catch {
		// best effort: it is looked up again next time
	}
}

/** Searches in flight, shared: the sheet and the activity ask for the same one at once. */
const searching = new Map<string, Promise<Found>>();

/**
 * The transaction that executed `safeTxHash` on `chainId`: searched from the
 * head back to just before `sinceMs` (when its proposal was made). `final`:
 * its number was already used when the search began, so "none" is the
 * chain's last word (voided), and is kept — once the history read is seen to
 * reach a block where the number was used. Throws
 * {@link HistoryUnreadableError}.
 */
export async function lookupExecution(
	multisig: string,
	chainId: number,
	safeTxHash: string,
	{ sinceMs = 0, final = false, nonce }: { sinceMs?: number; final?: boolean; nonce?: bigint } = {}
): Promise<Found> {
	const key = lookupKey(chainId, multisig, safeTxHash);
	const kept = memo.get(key) ?? readLookups()[key];
	if (kept === 'none') return null;
	if (kept) return { txHash: kept, block: null };
	const running = searching.get(key);
	if (running) return running;
	const search = (async () => {
		const { from, head } =
			sinceMs > 0
				? await blockBefore(chainId, Math.max(0, sinceMs - CREATED_MARGIN_MS))
				: { from: 0n, head: (await blockOf(chainId, 'latest')).number };
		const log = await searchLogs(
			chainId,
			{ address: multisig, topics: [EXECUTION_SUCCESS, safeTxHash] },
			from,
			head
		);
		const found: Found = log
			? { txHash: log.transactionHash, block: BigInt(log.blockNumber) }
			: null;
		// The start is the mailbox's word on when it was proposed: a number
		// already used there means the search began too late — not "none".
		if (
			!found &&
			nonce !== undefined &&
			from > 0n &&
			(await nonceAt(chainId, multisig, from)) > nonce
		) {
			throw new HistoryUnreadableError(chainId, 'its number was used before the search began');
		}
		// Round 5: "none" is the chain's last word only where the history read
		// reached a block at which the number was used. A node behind the one
		// that read the nonce (or another chain's, behind a pool) holds no
		// `ExecutionSuccess` yet, and its "none" once said a successful
		// execution was replaced — kept for good. Checked at the head the
		// search read to: used there, or it cannot say.
		if (
			!found &&
			final &&
			nonce !== undefined &&
			(await nonceAt(chainId, multisig, head)) <= nonce
		) {
			throw new HistoryUnreadableError(chainId, 'the history read is behind the nonce');
		}
		if (found || final) remember(chainId, multisig, safeTxHash, found);
		return found;
	})();
	searching.set(key, search);
	try {
		return await search;
	} finally {
		searching.delete(key);
	}
}

/**
 * Rows written before the one id scheme (`multisig:<safeTxHash>:<i>`),
 * renamed once to it with their own network — never left to be mistaken
 * for something else when tapped.
 */
async function migrateLegacyRows(): Promise<void> {
	const rows = await loadTransactions();
	const legacy = rows.filter((t) => legacyRowId(t.id) !== null);
	if (legacy.length === 0) return;
	await saveTransactions(
		legacy.map((t) => {
			const old = legacyRowId(t.id)!;
			return { ...t, id: executedRowId(t.chainId, old.safeTxHash, old.index) };
		})
	);
	const { deleteTransactions } = await import('$lib/services/records');
	await deleteTransactions(legacy.map((t) => t.id));
}

/** What the activity's background reading of other members' executions came to. */
export interface RemoteRecording {
	/** New rows were written: whoever shows the activity reads it again. */
	wrote: boolean;
	/** The networks whose history could not be read now (said, with Retry). */
	unreadable: number[];
}

/**
 * Transactions executed somewhere else (another member's device): found by
 * their `ExecutionSuccess` on chain and written to the activity, once — every
 * kind. A voided transaction has no `ExecutionSuccess` and writes nothing. A
 * chain whose history could not be read is named in `unreadable`.
 */
export async function recordRemoteExecutions(
	multisig: string,
	items: ProposalItem[]
): Promise<RemoteRecording> {
	const done = items.filter((i) => i.status === 'done');
	if (done.length === 0) return { wrote: false, unreadable: [] };
	await migrateLegacyRows().catch(() => {});
	const known = new Set((await loadTransactions()).map((t) => t.id));
	const unreadable = new Set<number>();
	let wrote = false;
	for (const item of done) {
		if (known.has(executedRowId(item.chainId, item.safeTxHash, 0))) continue;
		if (unreadable.has(item.chainId)) continue;
		try {
			const found = await lookupExecution(multisig, item.chainId, item.safeTxHash, {
				sinceMs: item.createdMs,
				final: true,
				nonce: item.nonce
			});
			if (!found) continue;
			let at: number | undefined;
			if (found.block !== null) {
				at = await blockOf(item.chainId, hex(found.block))
					.then((b) => b.time)
					.catch(() => undefined);
			}
			const rows = await executionRows(
				multisig,
				item,
				found.txHash,
				'',
				at ?? Math.floor(Date.now() / 1000)
			);
			if ((await mergeTransactions(rows)) > 0) wrote = true;
		} catch (error) {
			if (error instanceof HistoryUnreadableError) unreadable.add(item.chainId);
			else throw error;
		}
	}
	return { wrote, unreadable: [...unreadable] };
}

/**
 * The transaction that executed `safeTxHash` (its `ExecutionSuccess`): its
 * hash; `null` when the chain shows none (not executed — for a proposal whose
 * number is used, voided by another transaction); `undefined` when the chain
 * could not say. Never "none" for a read that failed: a dApp told its
 * transaction was cancelled, or a member told it was voided, because a node
 * did not answer, would be told something false.
 */
export async function findExecution(
	multisig: string,
	chainId: number,
	safeTxHash: string,
	options: { sinceMs?: number; final?: boolean; nonce?: bigint } = {}
): Promise<string | null | undefined> {
	try {
		return (await lookupExecution(multisig, chainId, safeTxHash, options))?.txHash ?? null;
	} catch {
		return undefined;
	}
}

/**
 * The transaction that executed `safeTxHash`, or `null` while the chain
 * shows none or cannot say — what a dApp that asked for it is answered with.
 */
export async function executionTxHash(
	multisig: string,
	chainId: number,
	safeTxHash: string
): Promise<string | null> {
	return (await findExecution(multisig, chainId, safeTxHash)) ?? null;
}

/** For tests: forget the session's windows and the kept lookups. */
export function resetHistoryForTests(): void {
	spans.clear();
	searching.clear();
	memo.clear();
	try {
		localStorage.removeItem(LOOKUPS_KEY);
	} catch {
		// no storage
	}
}
