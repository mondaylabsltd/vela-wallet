/**
 * The person's "Delete record" (spec 103 round 6b, owner's report): a folded
 * batch row is named by its operation's hash, so every record of it goes; and
 * what the person deleted stays deleted when a scan finds the same transfer.
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
	forgetTransaction,
	loadTransactions,
	mergeTransactions,
	saveTransactions
} from './records';

const tx = (id: string, over: Partial<LocalTransaction> = {}): LocalTransaction => ({
	id,
	userOpHash: '',
	txHash: '',
	from: '0xme',
	to: '0xthem',
	value: '1',
	symbol: 'ETH',
	decimals: 18,
	chainId: 1,
	timestamp: 1_700_000_000,
	status: 'confirmed',
	type: 'send',
	...over
});

beforeEach(() => kv.clear());

describe('deleting a record', () => {
	it('takes every record of a batch, by the operation’s hash its row is named by', async () => {
		await saveTransactions([
			tx('a', { userOpHash: '0xBATCH' }),
			tx('b', { userOpHash: '0xBATCH' }),
			tx('c', { userOpHash: '0xother' })
		]);
		await forgetTransaction('0xbatch');
		expect((await loadTransactions()).map((t) => t.id)).toEqual(['c']);
	});

	it('takes one record by its own id', async () => {
		await saveTransactions([
			tx('a', { userOpHash: '0xBATCH' }),
			tx('b', { userOpHash: '0xBATCH' })
		]);
		await forgetTransaction('a');
		expect((await loadTransactions()).map((t) => t.id)).toEqual(['b']);
	});

	it('keeps a deleted incoming transfer deleted when the next scan finds it again', async () => {
		const incoming = tx('1-0xtx-0', { type: 'receive' });
		expect(await mergeTransactions([incoming])).toBe(1);
		await forgetTransaction(incoming.id);
		expect(await mergeTransactions([incoming]), 'not merged back').toBe(0);
		expect(await loadTransactions()).toEqual([]);
		// Another transfer still arrives.
		expect(await mergeTransactions([tx('1-0xtx-1', { type: 'receive' })])).toBe(1);
	});

	it('does nothing, and remembers nothing, for an id it does not have', async () => {
		await saveTransactions([tx('a')]);
		await forgetTransaction('nope');
		expect((await loadTransactions()).map((t) => t.id)).toEqual(['a']);
		expect(kv.has('vela.deletedTransactions')).toBe(false);
	});
});
