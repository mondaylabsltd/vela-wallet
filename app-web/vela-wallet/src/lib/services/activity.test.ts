import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { incomingToRecord } from './activity';

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
	['56:0x9b00a09492a626678e5a3009982191586c444df9', { symbol: 'aBnbWBNB', decimals: 18 }]
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
