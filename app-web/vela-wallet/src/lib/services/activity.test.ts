import { readFileSync } from 'node:fs';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import type { TrustIncomingView } from '$lib/core/generated/TrustIncomingView';
import type { LocalTransaction } from './transactions-model';

// The store, the wallet's held tokens and the `token_trust` feed — stood in
// for, for the ingestion test at the end. Nothing above it reads them.
const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
vi.mock('./wallet-api', () => ({ fetchTokens: async () => [] }));
vi.mock('./token-metadata', () => ({
	resolveTokenMetadata: async () =>
		new Map([['0xddafbb505ad214d7b80b1f830fccc89b60fb7a83', { symbol: 'USDC', decimals: 6 }]])
}));
const trust = vi.hoisted(() => ({ feed: [] as unknown[] }));
vi.mock('$lib/wallet/core/token-trust-resident', () => ({
	pollIncoming: async () => trust.feed
}));

import { incomingToRecord, syncReceivedTransfers } from './activity';

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

const transfer = (value: bigint) => ({
	id: '56-0xabc-0',
	chainId: 56,
	token: '0x9b00a09492a626678e5a3009982191586c444df9',
	isNative: false,
	from: '0x0000000000000000000000000000000000000000',
	value,
	txHash: '0xabc',
	blockNumber: 1,
	logIndex: 0,
	timestamp: 1
});

const index = new Map([
	[
		'56:0x9b00a09492a626678e5a3009982191586c444df9',
		{ symbol: 'aBnbWBNB', decimals: 18, priceUsd: null }
	]
]);

describe('a received transfer is stored at its exact amount (097 final pass)', () => {
	it('keeps an amount below 1e-6 as a plain decimal', () => {
		// The Aave interest mint the pass saw stored as "1.373924e-12".
		const record = incomingToRecord(transfer(1_373_924n), '0xme', index);
		expect(record.value).toBe('0.000000000001373924');
	});

	it('keeps every digit of a large amount', () => {
		const record = incomingToRecord(transfer(123_456_789_012_345_678_901n), '0xme', index);
		expect(record.value).toBe('123.456789012345678901');
	});
});

/**
 * PR 3 — a receipt's time is its block's time. The `token_trust` feed now
 * withholds a transfer whose block it has not read, so every row it hands
 * over carries the block's own time; the record written from it says so
 * (`timeVerified`), which is what tells it from an older record stamped with
 * the clock — the ones the feed core re-reads and rewrites.
 */
describe('a receipt taken from the trust feed is stored with its block’s time, marked (PR 3)', () => {
	const ME = '0x14fb1f4e2b9c7a5d8e3f6a1b4c7d9e2f5a8b1d1e';
	/** 2026-09-29 08:00:00 UTC — the block's own time. */
	const BLOCK_TIME = 1_790_668_800;
	/** 2026-10-10 08:00:00 UTC — the clock when the scan ran. */
	const NOW_MS = 1_791_619_200_000;
	const row = (over: Partial<TrustIncomingView> = {}): TrustIncomingView => ({
		id: '100-0xaaa-0',
		chain_id: 100,
		token: null,
		is_native: true,
		from: '0x' + 'b1'.repeat(20),
		value: '1000000000000000',
		tx_hash: '0xaaa',
		block_number: 4_242,
		log_index: 0,
		timestamp_sec: BLOCK_TIME,
		symbol: null,
		decimals: null,
		...over
	});
	const stored = (): LocalTransaction[] => JSON.parse(kv.get('vela.transactionHistory') ?? '[]');

	beforeEach(() => {
		kv.clear();
		vi.useFakeTimers({ toFake: ['Date'] });
		vi.setSystemTime(NOW_MS);
	});
	afterEach(() => vi.useRealTimers());

	it('a native receipt and a token’s: the feed’s time, and `timeVerified: true`', async () => {
		trust.feed = [
			row(),
			row({
				id: '100-0xbbb-3',
				tx_hash: '0xbbb',
				log_index: 3,
				is_native: false,
				token: '0xddafbb505ad214d7b80b1f830fccc89b60fb7a83',
				value: '2500000',
				timestamp_sec: BLOCK_TIME + 5,
				symbol: 'USDC',
				decimals: 6
			})
		];
		expect(await syncReceivedTransfers(ME)).toBe(2);
		const records = stored();
		expect(records.map((tx) => [tx.id, tx.type, tx.timestamp, tx.timeVerified])).toEqual([
			['100-0xbbb-3', 'receive', BLOCK_TIME + 5, true],
			['100-0xaaa-0', 'receive', BLOCK_TIME, true]
		]);
		// The clock stood eleven days later the whole time: no record holds it.
		expect(JSON.stringify(records)).not.toContain(String(NOW_MS / 1000));
	});

	it('the mark is the record’s own: the mapper writes it whatever the row says', () => {
		const record = incomingToRecord(
			{
				id: '100-0xaaa-0',
				chainId: 100,
				token: null,
				isNative: true,
				from: '0x' + 'b1'.repeat(20),
				value: 1n,
				txHash: '0xaaa',
				blockNumber: 1,
				logIndex: 0,
				timestamp: BLOCK_TIME
			},
			ME,
			new Map()
		);
		expect(record).toMatchObject({ type: 'receive', timestamp: BLOCK_TIME, timeVerified: true });
	});
});
