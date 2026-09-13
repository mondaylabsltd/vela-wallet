import { describe, expect, it } from 'vitest';
import type { LocalTransaction } from '$lib/services/transactions-model';
import { toHistoryTx } from './contacts-executor';

function tx(partial: Partial<LocalTransaction>): LocalTransaction {
	return {
		id: 'r1',
		userOpHash: '',
		txHash: '0xabc',
		from: '0x' + '11'.repeat(20),
		to: '0x' + 'aa'.repeat(20),
		value: '1',
		symbol: 'ETH',
		decimals: 18,
		chainId: 1,
		timestamp: 1_700_000_000,
		status: 'confirmed',
		...partial
	};
}

describe('toHistoryTx — the stored record as the book reads it', () => {
	it('carries the kind, the recipient, the captured name, and seconds → ms', () => {
		expect(toHistoryTx(tx({ type: 'send', toName: 'vitalik.eth' }))).toEqual({
			kind: 'send',
			to: '0x' + 'aa'.repeat(20),
			to_name: 'vitalik.eth',
			timestamp_ms: 1_700_000_000_000
		});
	});

	it('a legacy record with no type stays untyped — the core decides what that means', () => {
		expect(toHistoryTx(tx({ type: undefined })).kind).toBeNull();
	});

	it('empty strings are absences, not names', () => {
		const out = toHistoryTx(tx({ to: '', toName: '' }));
		expect(out.to).toBeNull();
		expect(out.to_name).toBeNull();
	});
});
