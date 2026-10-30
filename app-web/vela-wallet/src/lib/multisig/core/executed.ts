/**
 * An executed multisig transaction, as it was when it ran (r4a-8): whose
 * approvals counted, how many were needed, who sent it — never re-counted
 * against today's owners (an owner removed since did approve it; a threshold
 * raised since did not apply to it; an executor that approved by sending it
 * counts once).
 *
 * Read from the chain first: the receipt of the transaction that executed
 * it holds Safe L2's `SafeMultiSigTransaction`, with the signatures
 * `execTransaction` checked and the threshold and sender then — the core
 * reads them (`executionFacts`). A Safe that is not an L2 one emits none;
 * for one executed from this device, what this device recorded when it
 * executed it stands in. Otherwise nothing is claimed.
 */
import { poolRpcCall } from '$lib/services/rpc-pool';
import { executionFacts, sameAddress, type ReceiptLog } from './kernel';

export interface FrozenFacts {
	/** Who sent it; `null` when the record does not say. */
	executor: string | null;
	threshold: number;
	/** The members whose approvals counted. */
	approvers: string[];
	/** The executor approved by sending it (it is among `approvers`). */
	executorApproved: boolean;
	/** Read from its receipt, or what this device recorded when it executed it. */
	source: 'chain' | 'device';
	/**
	 * When it ran (Unix ms): its block's time, read with its receipt, else
	 * when this device saw its execution land; `null` when neither says.
	 */
	atMs: number | null;
}

const KEY = 'vela.multisig.executed';
/** How many executions this device keeps the facts of. */
const KEEP = 200;

const keyOf = (chainId: number, safe: string, safeTxHash: string) =>
	`${chainId}:${safe.toLowerCase()}:${safeTxHash.toLowerCase()}`;

function load(): Record<string, Omit<FrozenFacts, 'source' | 'atMs'> & { at: number }> {
	try {
		return JSON.parse(localStorage.getItem(KEY) ?? '{}');
	} catch {
		return {};
	}
}

/** What this device executed: the approvals it sent, the threshold then, itself. */
export function rememberExecution(
	chainId: number,
	safe: string,
	safeTxHash: string,
	facts: Omit<FrozenFacts, 'source' | 'atMs'>
): void {
	try {
		const all = load();
		all[keyOf(chainId, safe, safeTxHash)] = { ...facts, at: Date.now() };
		const kept = Object.entries(all)
			.sort(([, a], [, b]) => b.at - a.at)
			.slice(0, KEEP);
		localStorage.setItem(KEY, JSON.stringify(Object.fromEntries(kept)));
	} catch {
		// no storage: the receipt is still read
	}
}

/**
 * The facts an execution from this device leaves, as Safe counts them: the
 * approvals it sends and, when it approves by sending it, the executor itself
 * (its `msg.sender` approves; its own approval is not sent twice) — in
 * ascending address order, the order Safe requires, the first `threshold` of
 * them. `approvals`: the ones its calls carry (`planExecution`), never every
 * one the mailbox holds — a given approval it did not carry did not count.
 */
export function factsOfExecution(
	approvals: string[],
	executor: string,
	executorApproves: boolean,
	threshold: number
): Omit<FrozenFacts, 'source' | 'atMs'> {
	const all = executorApproves ? [...approvals, executor] : [...approvals];
	const unique = all
		.filter((a, i) => all.findIndex((b) => sameAddress(a, b)) === i)
		.sort((a, b) => (a.toLowerCase() < b.toLowerCase() ? -1 : 1));
	const approvers = unique.slice(0, threshold);
	return {
		executor,
		threshold,
		approvers,
		executorApproved: executorApproves && approvers.some((a) => sameAddress(a, executor))
	};
}

/** A block's time (Unix ms), or `null` when it could not be read. */
async function blockTimeMs(chainId: number, block: string | undefined): Promise<number | null> {
	if (!block) return null;
	const answer = await poolRpcCall('eth_getBlockByNumber', [block, false], chainId).catch(
		() => null
	);
	const time = (answer?.result as { timestamp?: string } | null)?.timestamp;
	return typeof time === 'string' && /^0x[0-9a-fA-F]+$/.test(time)
		? Number(BigInt(time)) * 1000
		: null;
}

const read = new Map<string, Promise<FrozenFacts | null>>();

/**
 * The frozen facts of `safeTxHash`, executed by `txHash`; `null` when neither
 * its receipt nor this device says them.
 */
export function frozenFacts(
	chainId: number,
	safe: string,
	safeTxHash: string,
	txHash: string
): Promise<FrozenFacts | null> {
	const key = keyOf(chainId, safe, safeTxHash);
	let hit = read.get(key);
	if (!hit) {
		hit = (async () => {
			try {
				const answer = await poolRpcCall('eth_getTransactionReceipt', [txHash], chainId);
				const receipt = answer.result as { logs?: ReceiptLog[]; blockNumber?: string } | null;
				const logs = receipt?.logs;
				if (Array.isArray(logs)) {
					const facts = executionFacts(
						chainId,
						safe,
						safeTxHash,
						logs.map((l) => ({ address: l.address, topics: l.topics, data: l.data }))
					);
					if (facts) {
						return {
							executor: facts.executor,
							threshold: facts.threshold,
							approvers: facts.approvers,
							executorApproved: facts.executorApproved,
							source: 'chain' as const,
							atMs: await blockTimeMs(chainId, receipt?.blockNumber)
						};
					}
				}
			} catch {
				// the chain could not say: what this device recorded, if anything
			}
			const kept = load()[key];
			if (!kept) return null;
			return {
				executor: kept.executor,
				threshold: kept.threshold,
				approvers: kept.approvers,
				executorApproved: kept.executorApproved,
				source: 'device' as const,
				atMs: typeof kept.at === 'number' ? kept.at : null
			};
		})();
		// A read that found nothing is asked again next time (the receipt may come).
		void hit.then((facts) => {
			if (facts === null) read.delete(key);
		});
		read.set(key, hit);
	}
	return hit;
}
