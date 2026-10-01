/**
 * Receipt reads for the operation hashes the extension handed out (spec 082
 * RF3, W12) — the same translation the in-app browsers do
 * (`services/dapp-submit.ts`, the `eth_getTransactionReceipt` case): a hash
 * this wallet handed out is looked up at the bundler and the page's own method
 * is forwarded for the real transaction; any other hash goes out untouched.
 */
import { describe, expect, it } from 'vitest';
import {
	OP_PREFIX,
	OP_TTL_MS,
	expiredOpKeys,
	liveOpEntry,
	opKey,
	opRecord,
	realTxHash,
	receiptLookupHash
} from '../../../extension/lib/op-receipt.js';

const OP = `0x${'Ab'.repeat(32)}`;
const TX = `0x${'cd'.repeat(32)}`;
const NOW = 1_800_000_000_000;

describe('what is remembered when an answer goes out', () => {
	it('an operation hash with its chain, keyed lower-case', () => {
		expect(opRecord(OP, { chainId: 100 }, NOW)).toEqual({
			key: `${OP_PREFIX}${OP.toLowerCase()}`,
			value: { chainId: 100, at: NOW }
		});
	});

	it('nothing for a signature, an address, a batch without a chain or a bad chain', () => {
		expect(opRecord(`0x${'11'.repeat(65)}`, { chainId: 100 }, NOW)).toBeNull();
		expect(opRecord(`0x${'22'.repeat(20)}`, { chainId: 100 }, NOW)).toBeNull();
		expect(opRecord(OP, undefined, NOW)).toBeNull();
		expect(opRecord(OP, { chainId: 0 }, NOW)).toBeNull();
		expect(opRecord(null, { chainId: 100 }, NOW)).toBeNull();
	});
});

describe('which reads are translated', () => {
	it('the two receipt reads, for a 32-byte hash', () => {
		expect(receiptLookupHash('eth_getTransactionReceipt', [OP])).toBe(OP);
		expect(receiptLookupHash('eth_getTransactionByHash', [OP])).toBe(OP);
	});

	it('nothing else', () => {
		expect(receiptLookupHash('eth_call', [OP])).toBeNull();
		expect(receiptLookupHash('eth_getTransactionReceipt', ['0x1234'])).toBeNull();
		expect(receiptLookupHash('eth_getTransactionReceipt', [])).toBeNull();
	});

	it('a remembered hash for 24 hours, then not', () => {
		expect(liveOpEntry({ chainId: 100, at: NOW }, NOW + OP_TTL_MS - 1)).toEqual({
			chainId: 100,
			at: NOW
		});
		expect(liveOpEntry({ chainId: 100, at: NOW }, NOW + OP_TTL_MS)).toBeNull();
		// An unrecorded hash has no entry and is forwarded as it is.
		expect(liveOpEntry(undefined, NOW)).toBeNull();
		expect(liveOpEntry({ chainId: 'x', at: NOW }, NOW)).toBeNull();
	});

	it('prunes only the remembered hashes that are past their day', () => {
		const all = {
			[opKey(OP)]: { chainId: 100, at: NOW - OP_TTL_MS },
			[opKey(TX)]: { chainId: 100, at: NOW },
			'vela.perm.https://a.example': { address: '0x' }
		};
		expect(expiredOpKeys(all, NOW)).toEqual([opKey(OP)]);
	});
});

describe('the bundler’s answer', () => {
	it('names the real transaction once the operation landed', () => {
		expect(realTxHash({ success: true, receipt: { transactionHash: TX, status: '0x1' } })).toBe(TX);
	});

	it('is null while it has not, so the page keeps polling', () => {
		expect(realTxHash(null)).toBeNull();
		expect(realTxHash({ receipt: {} })).toBeNull();
		expect(realTxHash({ receipt: { transactionHash: '0x' } })).toBeNull();
	});
});
