import { describe, expect, it } from 'vitest';
import { decodeFunctionResult, getAddress } from 'viem';
import { REGISTRY_ABI } from './chain';
import { formatRecord, type RawRecord } from './registry';
import { GET_KEYS_RESULT } from './registry.fixture';

/**
 * Decodes a real `getKeysByRpId` response captured from Gnosis (see
 * registry.fixture.ts) and asserts our ABI + formatting reproduce the chain
 * exactly. This catches an ABI drift or a bad walletRef→address derivation
 * without needing a live network call.
 */
describe('registry decode (real chain fixture)', () => {
	const [total, records] = decodeFunctionResult({
		abi: REGISTRY_ABI,
		functionName: 'getKeysByRpId',
		data: GET_KEYS_RESULT
	}) as [bigint, readonly RawRecord[]];

	it('reads the on-chain total', () => {
		expect(Number(total)).toBe(189);
		expect(records.length).toBe(2);
	});

	it('decodes the newest record with name, address, key and time intact', () => {
		const a = formatRecord(records[0]);
		expect(a.name).toBe('v test');
		expect(a.credentialId).toBe('c4fabb9a1f71abed5116d00a2a6ab571');
		expect(a.walletAddress).toBe(getAddress('0xb92359aea2d4933e4450924b90532c8621de3e4b'));
		expect(a.publicKey.startsWith('0x04afbbdc')).toBe(true);
		expect(a.publicKey.length).toBe(2 + 65 * 2); // 0x + 65 bytes
		expect(a.createdAt).toBe(1783414105 * 1000);
	});

	it('decodes a unicode name and its address', () => {
		const b = formatRecord(records[1]);
		expect(b.name).toBe('Mimimi');
		expect(b.walletAddress).toBe(getAddress('0xb0419e72285d4b5f04f194a5c636c3b12b567f3f'));
	});
});
