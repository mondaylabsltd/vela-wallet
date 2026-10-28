/**
 * A multisig transfer executed from this device, in the activity: one record
 * per transfer, `from` the multisig, under the transaction that carried it —
 * the same store and shape a Vela account's own sends are written to, so the
 * multisig's activity reads like any account's. An executed cancellation is
 * a row of its own too (ms-9): it moves nothing, but it used a number and
 * voided whatever else waited at it.
 */
import { fillTemplate } from '$lib/i18n/fill';
import { loadTransactions, saveTransactions } from '$lib/services/records';
import { poolRpcCall } from '$lib/services/rpc-pool';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { tokenInfo } from './display';
import { transfersOf } from './kernel';
import { plainUnits } from './units';
import type { ProposalItem } from './proposals.svelte';

/**
 * The title an executed cancellation's row keeps — the proposal's own words
 * (`home.multisig.action.cancel`). The multisig host hands them over with
 * the page's copy ({@link useCancelTitle}); this is only the fallback.
 */
let cancelTitle = 'Cancel transaction #{{nonce}}';

/** The page's words for a cancellation's row (`{{nonce}}`). */
export function useCancelTitle(template: string): void {
	if (template) cancelTitle = template;
}

/** Whether executing `item` leaves a row in Activity: a transfer, or a cancellation. */
function recorded(item: ProposalItem): boolean {
	return item.review.action?.kind === 'cancel' || transfersOf(item.review.action).length > 0;
}

/**
 * ms-9: an executed cancellation, as Activity keeps it — a call from the
 * multisig to itself that moves nothing, titled as its proposal was.
 */
function cancellationRecord(
	multisig: string,
	item: ProposalItem,
	txHash: string,
	userOpHash: string,
	at: number
): LocalTransaction {
	return {
		id: `multisig:${item.safeTxHash}:0`,
		userOpHash,
		txHash,
		from: multisig,
		to: multisig,
		value: '0',
		symbol: '',
		decimals: 18,
		chainId: item.chainId,
		timestamp: at,
		status: 'confirmed',
		type: 'dapp_tx',
		intent: fillTemplate(cancelTitle, { nonce: item.nonce.toString() }),
		dappSummary: { action: 'call', calls: 1, contract: multisig.toLowerCase() }
	};
}

export async function recordExecution(
	multisig: string,
	item: ProposalItem,
	txHash: string,
	userOpHash: string,
	/** Unix seconds of its block, when known; else now. */
	at?: number
): Promise<void> {
	if (item.review.action?.kind === 'cancel') {
		await saveTransactions([
			cancellationRecord(multisig, item, txHash, userOpHash, at ?? Math.floor(Date.now() / 1000))
		]);
		return;
	}
	const transfers = transfersOf(item.review.action);
	const records: LocalTransaction[] = [];
	for (const [i, t] of transfers.entries()) {
		const info = await tokenInfo(item.chainId, t.token).catch(() => ({ symbol: '', decimals: 18 }));
		records.push({
			id: `multisig:${item.safeTxHash}:${i}`,
			userOpHash,
			txHash,
			from: multisig,
			to: t.to,
			value: plainUnits(t.amount, info.decimals),
			symbol: info.symbol,
			decimals: info.decimals,
			chainId: item.chainId,
			timestamp: at ?? Math.floor(Date.now() / 1000),
			status: 'confirmed',
			type: 'send'
		});
	}
	if (records.length > 0) await saveTransactions(records);
}

/** `keccak256("ExecutionSuccess(bytes32,uint256)")`: its first topic is the safeTxHash. */
const EXECUTION_SUCCESS = '0x442e715f626346e8c54381002da614f62bee8d27386535b2521ec8540898556e';
/** How far back an execution is looked for (blocks): a few days on most chains. */
const LOOKBACK = 50_000n;

/**
 * Transfers and cancellations executed somewhere else (another member's
 * device): found by their `ExecutionSuccess` on chain and written to the
 * activity, once. Quiet when the chain cannot say. A voided transaction has
 * no `ExecutionSuccess` and writes nothing.
 */
export async function recordRemoteExecutions(
	multisig: string,
	items: ProposalItem[]
): Promise<boolean> {
	const done = items.filter((i) => i.status === 'done' && recorded(i));
	if (done.length === 0) return false;
	const known = new Set((await loadTransactions()).map((t) => t.id));
	let wrote = false;
	for (const item of done) {
		if (known.has(`multisig:${item.safeTxHash}:0`)) continue;
		try {
			const head = BigInt(
				(await poolRpcCall('eth_blockNumber', [], item.chainId)).result as string
			);
			const from = head > LOOKBACK ? head - LOOKBACK : 0n;
			const logs = (
				await poolRpcCall(
					'eth_getLogs',
					[
						{
							address: multisig,
							topics: [EXECUTION_SUCCESS, item.safeTxHash],
							fromBlock: `0x${from.toString(16)}`,
							toBlock: 'latest'
						}
					],
					item.chainId
				)
			).result as { transactionHash: string; blockNumber: string }[] | undefined;
			const log = logs?.[0];
			if (!log) continue;
			const block = (
				await poolRpcCall('eth_getBlockByNumber', [log.blockNumber, false], item.chainId)
			).result as { timestamp: string } | undefined;
			await recordExecution(
				multisig,
				item,
				log.transactionHash,
				'',
				block ? Number(BigInt(block.timestamp)) : undefined
			);
			wrote = true;
		} catch {
			// the chain could not say; the next refresh asks again
		}
	}
	return wrote;
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
	safeTxHash: string
): Promise<string | null | undefined> {
	try {
		const head = BigInt((await poolRpcCall('eth_blockNumber', [], chainId)).result as string);
		const from = head > LOOKBACK ? head - LOOKBACK : 0n;
		const answer = await poolRpcCall(
			'eth_getLogs',
			[
				{
					address: multisig,
					topics: [EXECUTION_SUCCESS, safeTxHash],
					fromBlock: `0x${from.toString(16)}`,
					toBlock: 'latest'
				}
			],
			chainId
		);
		if (answer.error || !Array.isArray(answer.result)) return undefined;
		const logs = answer.result as { transactionHash?: unknown }[];
		const hash = logs[0]?.transactionHash;
		return typeof hash === 'string' ? hash : null;
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
