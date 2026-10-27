/**
 * A multisig transfer executed from this device, in the activity: one record
 * per transfer, `from` the multisig, under the transaction that carried it —
 * the same store and shape a Vela account's own sends are written to, so the
 * multisig's activity reads like any account's.
 */
import { saveTransactions } from '$lib/services/records';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { tokenInfo } from './display';
import type { ProposalItem } from './proposals.svelte';

/** Base units as a plain decimal string (the records' `value`). */
export function plainUnits(amount: string, decimals: number): string {
	const value = BigInt(amount);
	const base = 10n ** BigInt(decimals);
	const fraction = (value % base).toString().padStart(decimals, '0').replace(/0+$/, '');
	return fraction ? `${value / base}.${fraction}` : `${value / base}`;
}

export async function recordExecution(
	multisig: string,
	item: ProposalItem,
	txHash: string,
	userOpHash: string
): Promise<void> {
	const action = item.review.action;
	const transfers = action?.kind === 'transfer' ? [action] : action?.kind === 'batch' ? action.transfers : [];
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
			timestamp: Math.floor(Date.now() / 1000),
			status: 'confirmed',
			type: 'send'
		});
	}
	if (records.length > 0) await saveTransactions(records);
}
