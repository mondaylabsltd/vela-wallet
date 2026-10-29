/**
 * An execution sent from this device, followed to its end (r4b-1, r4b-7).
 *
 * The moment the relay has the operation, the execution is written down
 * (`localStorage`): a reload, a closed sheet or a crash never forgets it, and
 * while it may still land the transaction is not offered for execution again
 * — the sheet that opens it resumes watching instead. Watching asks, every
 * few seconds, the chain's nonce, the relay's word on this device's own
 * operation and — once the number is used — the transaction that executed
 * it; the core says what that means (`executionVerdict`): executed here,
 * executed first by another member (and whether this device's attempt paid
 * anything), replaced by another transaction at its number, refused, past
 * its time. Never "Executing…" without end.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';
import { pollUserOpStatus } from '$lib/services/tx-reconciler';
import { readNonce } from './chain';
import { NOT_TAKEN_AFTER_MS } from './execute';
import {
	executionVerdict,
	type ExecutedIn,
	type ExecutionVerdict,
	type OurOp,
	type OurReceipt
} from './kernel';
import { EXECUTION_SUCCESS, findExecution } from './records';

/** How long an execution is watched before it is said as not landed. */
export const EXECUTION_LIMIT_MS = 180_000;
/** How often it is asked about. */
export const EXECUTION_PAUSE_MS = 2_500;
/**
 * How long "the chain can't say" is asked again before it is said: a node a
 * few blocks behind catches up within it; one that is down is said as that.
 */
export const UNKNOWN_GRACE_MS = 30_000;

/** An execution sent from this device, as it is kept. */
export interface InFlight {
	chainId: number;
	multisig: string;
	safeTxHash: string;
	/** The transaction's number, decimal. */
	nonce: string;
	userOpHash: string;
	/** The executor's address. */
	by: string;
	/** The relay's reply was lost: the op may never have been taken. */
	maybeSent: boolean;
	startedMs: number;
	/** When its proposal was made (the history's lower bound). */
	createdMs: number;
}

const KEY = 'vela.multisigExecuting';

function keyOf(chainId: number, multisig: string, safeTxHash: string): string {
	return `${chainId}:${multisig.toLowerCase()}:${safeTxHash.toLowerCase()}`;
}

function load(): Record<string, InFlight> {
	try {
		return JSON.parse(localStorage.getItem(KEY) ?? '{}') as Record<string, InFlight>;
	} catch {
		return {};
	}
}

function save(all: Record<string, InFlight>): void {
	try {
		if (Object.keys(all).length === 0) localStorage.removeItem(KEY);
		else localStorage.setItem(KEY, JSON.stringify(all));
	} catch {
		// best effort: the watch still runs while the page is up
	}
}

class Executions {
	#all = $state<Record<string, InFlight>>(typeof localStorage === 'undefined' ? {} : load());

	/**
	 * The execution of `safeTxHash` in flight from this device, if one may
	 * still land: past its watch (and a minute's grace) it is not.
	 */
	get(chainId: number, multisig: string, safeTxHash: string, now = Date.now()): InFlight | null {
		const flight = this.#all[keyOf(chainId, multisig, safeTxHash)];
		if (!flight) return null;
		return now - flight.startedMs < EXECUTION_LIMIT_MS + 60_000 ? flight : null;
	}

	/** Written the moment the relay has it. */
	start(flight: InFlight): InFlight {
		const all = { ...load(), [keyOf(flight.chainId, flight.multisig, flight.safeTxHash)]: flight };
		save(all);
		this.#all = all;
		return flight;
	}

	/** Its end is known (or it is past its time): it is no longer in flight. */
	end(chainId: number, multisig: string, safeTxHash: string): void {
		const key = keyOf(chainId, multisig, safeTxHash);
		const { [key]: _gone, ...rest } = load();
		void _gone;
		save(rest);
		this.#all = rest;
	}

	/** For tests: forget every one. */
	resetForTests(): void {
		save({});
		this.#all = {};
	}
}

export const executions = new Executions();

/** `keccak256("UserOperationEvent(bytes32,address,address,uint256,bool,uint256,uint256)")`. */
const USER_OPERATION_EVENT = '0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f';

/** A transaction's receipt, as far as the watch reads it. */
export interface Receipt {
	logs?: { address?: string; topics?: string[]; data?: string }[];
}

/** A transaction's receipt, or `null` when no node gave it (not mined there, or no answer). */
async function receiptOf(chainId: number, txHash: string): Promise<Receipt | null> {
	const answer = await poolRpcCall('eth_getTransactionReceipt', [txHash], chainId).catch(
		() => null
	);
	const receipt = answer?.result as Receipt | null | undefined;
	return receipt && Array.isArray(receipt.logs) ? receipt : null;
}

/** Whether `receipt` carried this device's operation (its `UserOperationEvent`). */
export function carriesOp(receipt: Receipt, userOpHash: string): boolean {
	return (receipt.logs ?? []).some(
		(l) =>
			l.topics?.[0]?.toLowerCase() === USER_OPERATION_EVENT &&
			l.topics?.[1]?.toLowerCase() === userOpHash.toLowerCase()
	);
}

/**
 * Whether `receipt` holds the multisig's `ExecutionSuccess` for `safeTxHash`
 * — indexed (Safe 1.4.1) or in its data (1.3.0).
 */
export function executesIn(receipt: Receipt, multisig: string, safeTxHash: string): boolean {
	const hash = safeTxHash.toLowerCase();
	return (receipt.logs ?? []).some(
		(l) =>
			l.address?.toLowerCase() === multisig.toLowerCase() &&
			l.topics?.[0]?.toLowerCase() === EXECUTION_SUCCESS &&
			(l.topics?.[1]?.toLowerCase() === hash || (l.data ?? '').toLowerCase().startsWith(hash))
	);
}

export interface WatchPorts {
	status?: typeof pollUserOpStatus;
	nonce?: (chainId: number, safe: string) => Promise<bigint>;
	lookup?: typeof findExecution;
	receipt?: typeof receiptOf;
	now?: () => number;
}

/** This device's operation, as the relay says it (and as the receipt said it, once read). */
async function ourOp(
	flight: InFlight,
	ports: Required<WatchPorts>,
	own: string | null
): Promise<OurOp> {
	if (own) return { kind: 'landed', txHash: own };
	const answer = await ports.status(flight.userOpHash, flight.chainId);
	if (!answer) return { kind: 'unknown' };
	// A bundle the relay only sent is not on chain yet; one in a block is.
	const inBlock =
		answer.status === 'included' || answer.status === 'failed' || answer.status === 'rejected';
	if (answer.txHash && inBlock) return { kind: 'landed', txHash: answer.txHash };
	if (answer.status === 'rejected') return { kind: 'refused' };
	// A lost reply the relay never heard of, for long enough: never taken.
	if (
		answer.status === 'not_found' &&
		flight.maybeSent &&
		ports.now() - flight.startedMs >= NOT_TAKEN_AFTER_MS
	) {
		return { kind: 'refused' };
	}
	return { kind: 'pending' };
}

/**
 * One round of facts, judged by the core.
 *
 * Round 5: a verdict never contradicts this device's own receipt. A
 * successful execution was once said as "Another transaction ran first":
 * the history was searched on a node behind the one that read the nonce,
 * found no `ExecutionSuccess`, and "none" stood. So the receipt of this
 * device's landed transaction is read first and is the word on it
 * (`receipt`); a lookup's find is believed only once its own receipt is read
 * and shows the execution (a log on one node, gone on another, is nobody's);
 * and a lookup's "none" is final only where the history reached a block at
 * which the number was used (`lookupExecution`'s cross-check).
 */
export async function judgeExecution(
	flight: InFlight,
	ports: Required<WatchPorts>,
	own: string | null = null
): Promise<ExecutionVerdict> {
	const [nonce, op] = await Promise.all([
		ports.nonce(flight.chainId, flight.multisig).catch(() => null),
		ourOp(flight, ports, own)
	]);
	const nonceUsed = nonce === null ? null : nonce > BigInt(flight.nonce);
	const executes = (r: Receipt) => executesIn(r, flight.multisig, flight.safeTxHash);
	let executedIn: ExecutedIn = { kind: 'none' };
	let landed = op;
	let receipt: OurReceipt = 'unread';
	if (op.kind === 'landed') {
		const mine = await ports.receipt(flight.chainId, op.txHash).catch(() => null);
		if (mine) receipt = executes(mine) ? 'executed' : 'notExecuted';
	}
	if (receipt !== 'executed' && (nonceUsed !== false || op.kind === 'landed')) {
		const found = await ports.lookup(flight.multisig, flight.chainId, flight.safeTxHash, {
			sinceMs: flight.createdMs,
			final: nonceUsed === true,
			nonce: BigInt(flight.nonce)
		});
		if (found === undefined) executedIn = { kind: 'unknown' };
		else if (found === null) executedIn = { kind: 'none' };
		else if (op.kind === 'landed' && found.toLowerCase() === op.txHash.toLowerCase()) {
			executedIn = { kind: 'found', txHash: found };
		} else {
			// Whose it is, and that it is one, by its own receipt.
			const theirs = await ports.receipt(flight.chainId, found).catch(() => null);
			if (!theirs || !executes(theirs)) executedIn = { kind: 'unknown' };
			else if (carriesOp(theirs, flight.userOpHash)) {
				// The relay may not have said yet (or named another bundle) that
				// its execution is this device's own.
				landed = { kind: 'landed', txHash: found };
				receipt = 'executed';
				executedIn = { kind: 'found', txHash: found };
			} else executedIn = { kind: 'found', txHash: found };
		}
	}
	return executionVerdict({
		nonceUsed,
		executedIn,
		ourOp: landed,
		receipt,
		elapsedMs: Math.max(0, ports.now() - flight.startedMs),
		limitMs: EXECUTION_LIMIT_MS
	});
}

/**
 * Watch `flight` until the core says where it ended: every
 * {@link EXECUTION_PAUSE_MS}, bounded by {@link EXECUTION_LIMIT_MS}. `own`
 * reads the transaction hash the submit's own wait found, if it did.
 * Resolves `waiting` only when `signal` stops it.
 */
export async function watchExecution(
	flight: InFlight,
	{
		signal,
		own = () => null,
		pauseMs = EXECUTION_PAUSE_MS,
		ports = {}
	}: {
		signal: AbortSignal;
		own?: () => string | null;
		pauseMs?: number;
		ports?: WatchPorts;
	}
): Promise<ExecutionVerdict> {
	const all: Required<WatchPorts> = {
		status: ports.status ?? pollUserOpStatus,
		nonce: ports.nonce ?? readNonce,
		lookup: ports.lookup ?? findExecution,
		receipt: ports.receipt ?? receiptOf,
		now: ports.now ?? Date.now
	};
	let unknownSince: number | null = null;
	while (!signal.aborted) {
		const verdict = await judgeExecution(flight, all, own()).catch(
			() => ({ kind: 'waiting' }) as const
		);
		if (signal.aborted) break;
		// "Can't say" is asked again a while — a node behind catches up —
		// and said only once it lasts (or the watch is past its time).
		if (verdict.kind === 'unknown') {
			unknownSince ??= all.now();
			const late = all.now() - flight.startedMs >= EXECUTION_LIMIT_MS;
			if (late || all.now() - unknownSince >= UNKNOWN_GRACE_MS) return verdict;
		} else if (verdict.kind !== 'waiting') return verdict;
		else unknownSince = null;
		if (all.now() - flight.startedMs >= EXECUTION_LIMIT_MS) return { kind: 'timedOut' };
		await new Promise<void>((done) => {
			const timer = setTimeout(done, pauseMs);
			signal.addEventListener(
				'abort',
				() => {
					clearTimeout(timer);
					done();
				},
				{ once: true }
			);
		});
	}
	return { kind: 'waiting' };
}
