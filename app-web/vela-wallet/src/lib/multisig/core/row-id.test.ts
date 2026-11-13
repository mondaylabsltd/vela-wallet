/** r4a-4: one id scheme for a multisig transaction's Activity rows, and nothing else parses. */
import { describe, expect, it } from 'vitest';
import { executedRowId, legacyRowId, transactionOfRow, waitingRowId } from './row-id';

const HASH = `0x${'Ab'.repeat(32)}`;

describe('the Activity row id scheme (r4a-4)', () => {
	it('a waiting row and an executed row both name the transaction and its network', () => {
		expect(waitingRowId(5042002, HASH)).toBe(`multisig:5042002:${HASH.toLowerCase()}`);
		expect(executedRowId(5042002, HASH, 1)).toBe(`multisig:5042002:${HASH.toLowerCase()}:1`);
		expect(transactionOfRow(executedRowId(5042002, HASH, 1))).toEqual({
			chainId: 5042002,
			safeTxHash: HASH.toLowerCase(),
			index: 1
		});
		expect(transactionOfRow(waitingRowId(100, HASH))).toEqual({
			chainId: 100,
			safeTxHash: HASH.toLowerCase(),
			index: null
		});
	});

	it('anything else is no transaction — the old clash above all', () => {
		for (const id of [
			`multisig:${HASH}:0`, // the old executed row: Number("0x…") was an "integer"
			`multisig:0:${HASH}`,
			`multisig:100:${HASH.slice(0, 40)}`,
			`multisig:100:${HASH}:x`,
			`multisig:100:${HASH}:0:1`,
			`multisig-network:100`,
			'multisig-retry',
			'',
			undefined
		]) {
			expect(transactionOfRow(id), String(id)).toBeNull();
		}
	});

	it('a row from before the scheme is recognised, to be migrated', () => {
		expect(legacyRowId(`multisig:${HASH}:2`)).toEqual({ safeTxHash: HASH.toLowerCase(), index: 2 });
		expect(legacyRowId(waitingRowId(100, HASH))).toBeNull();
	});
});
