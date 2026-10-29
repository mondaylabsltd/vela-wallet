/**
 * The one id scheme a multisig transaction's Activity row carries (r4a-4):
 * `multisig:<chainId>:<safeTxHash>` while it waits, and
 * `multisig:<chainId>:<safeTxHash>:<i>` for the i-th row its execution wrote
 * (a batch writes one per transfer). Both name the transaction and its
 * network, so a tap opens that transaction's sheet. Nothing else parses —
 * a row written before this scheme (`multisig:<safeTxHash>:<i>`) is migrated
 * once ({@link legacyRowId}), never guessed at.
 */

export const ROW_PREFIX = 'multisig:';

const ROW = /^multisig:([1-9]\d{0,15}):(0x[0-9a-fA-F]{64})(?::(0|[1-9]\d{0,3}))?$/;
const LEGACY = /^multisig:(0x[0-9a-fA-F]{64}):(0|[1-9]\d{0,3})$/;

/** A waiting transaction's row. */
export function waitingRowId(chainId: number, safeTxHash: string): string {
	return `${ROW_PREFIX}${chainId}:${safeTxHash.toLowerCase()}`;
}

/** The `index`-th row an executed transaction wrote. */
export function executedRowId(chainId: number, safeTxHash: string, index: number): string {
	return `${waitingRowId(chainId, safeTxHash)}:${index}`;
}

/** The transaction a row id names, or `null` for anything that is not exactly one of these. */
export function transactionOfRow(
	id: string | undefined | null
): { chainId: number; safeTxHash: string; index: number | null } | null {
	const match = id ? ROW.exec(id) : null;
	if (!match) return null;
	const chainId = Number(match[1]);
	if (!Number.isSafeInteger(chainId)) return null;
	return {
		chainId,
		safeTxHash: match[2].toLowerCase(),
		index: match[3] === undefined ? null : Number(match[3])
	};
}

/** A row written before this scheme: its safeTxHash and index, to migrate it. */
export function legacyRowId(id: string): { safeTxHash: string; index: number } | null {
	const match = LEGACY.exec(id);
	return match ? { safeTxHash: match[1].toLowerCase(), index: Number(match[2]) } : null;
}
