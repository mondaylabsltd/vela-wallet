/**
 * The transaction-store writer (spec 026 D20): a batch's siblings are ONE
 * atomic write (a per-record write raced the read-modify-write on Expo and
 * collapsed a batch to a single Activity row), patches are atomic too, and the
 * store stays de-duped, newest-first and capped.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { LocalTransaction } from './transactions-model';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));

import {
	loadTransactions,
	readTransactionHistory,
	saveTransaction,
	saveTransactions,
	updateTransaction,
	updateTransactions
} from './records';

const tx = (id: string, over: Partial<LocalTransaction> = {}): LocalTransaction => ({
	id,
	userOpHash: '0xop',
	txHash: '',
	from: '0xme',
	to: '0xthem',
	value: '1',
	symbol: 'ETH',
	decimals: 18,
	chainId: 1,
	timestamp: 1_700_000_000,
	status: 'pending',
	type: 'send',
	...over
});

beforeEach(() => kv.clear());

describe('the writer', () => {
	it('a batch lands whole: three siblings in one write, newest first', async () => {
		await saveTransactions([tx('a'), tx('b'), tx('c')]);
		expect((await loadTransactions()).map((t) => t.id)).toEqual(['a', 'b', 'c']);
	});

	it('concurrent writes never drop a record (the withTxLock reason)', async () => {
		await Promise.all([
			saveTransaction(tx('a')),
			saveTransaction(tx('b')),
			saveTransaction(tx('c'))
		]);
		expect((await loadTransactions()).map((t) => t.id).sort()).toEqual(['a', 'b', 'c']);
	});

	it('de-dupes by id — a resubmitted op replaces its record, never doubles it', async () => {
		await saveTransaction(tx('a', { value: '1' }));
		await saveTransaction(tx('a', { value: '2' }));
		const rows = await loadTransactions();
		expect(rows).toHaveLength(1);
		expect(rows[0].value).toBe('2');
	});

	it('caps the store at 200, keeping the newest', async () => {
		await saveTransactions(Array.from({ length: 210 }, (_, i) => tx(`t${i}`)));
		const rows = await loadTransactions();
		expect(rows).toHaveLength(200);
		expect(rows[0].id).toBe('t0');
	});

	it('patches one record; a missing id is a no-op, not a new row', async () => {
		await saveTransaction(tx('a'));
		await updateTransaction('a', { status: 'confirmed', txHash: '0xhash' });
		await updateTransaction('ghost', { status: 'failed' });
		const rows = await loadTransactions();
		expect(rows).toHaveLength(1);
		expect(rows[0]).toMatchObject({ status: 'confirmed', txHash: '0xhash' });
	});

	it("patches a batch's siblings together on their shared receipt", async () => {
		await saveTransactions([tx('a'), tx('b'), tx('c')]);
		await updateTransactions(['a', 'c'], { status: 'confirmed', txHash: '0xhash' });
		const byId = new Map((await loadTransactions()).map((t) => [t.id, t]));
		expect(byId.get('a')?.status).toBe('confirmed');
		expect(byId.get('c')?.txHash).toBe('0xhash');
		expect(byId.get('b')?.status).toBe('pending');
	});

	it('empty inputs touch nothing', async () => {
		await saveTransactions([]);
		await updateTransactions([], { status: 'failed' });
		expect(await loadTransactions()).toEqual([]);
	});
});

describe('the strict reader (round 3)', () => {
	it('nothing stored is an empty history; what is stored is read as it is', async () => {
		expect(await readTransactionHistory()).toEqual([]);
		await saveTransactions([tx('a')]);
		expect((await readTransactionHistory()).map((t) => t.id)).toEqual(['a']);
	});

	it('a store that cannot be read, or bytes that are no list, throw — never an empty history', async () => {
		const { getItem } = await import('$lib/services/storage');
		vi.mocked(getItem).mockRejectedValueOnce(new Error('IndexedDB blocked'));
		await expect(readTransactionHistory()).rejects.toThrow('IndexedDB blocked');
		kv.set('vela.transactionHistory', '{"not":"a list"}');
		await expect(readTransactionHistory()).rejects.toThrow();
		kv.set('vela.transactionHistory', '[{"id"');
		await expect(readTransactionHistory()).rejects.toThrow();
		// The lenient reader still reads both as empty.
		expect(await loadTransactions()).toEqual([]);
	});
});
