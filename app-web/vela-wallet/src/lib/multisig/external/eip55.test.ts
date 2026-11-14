import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import * as wasm from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { eip55 } from './eip55';

describe("eip55 — the core's checksumAddress, on every surface", () => {
	beforeAll(() => {
		wasm.initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
	});

	// The EIP-55 specification's own test vectors.
	const vectors = [
		'0x5aAeb6053F3E94C9b9A09f33669435E7Ef1BeAed',
		'0xfB6916095ca1df60bB79Ce92cE3Ea74c37c5d359',
		'0xdbF03B407c01E7cD3CBea99509d93f8DDDC8C6FB',
		'0xD1220A0cf47c7B9Be7A2E6BA89F429762e7b9aDb'
	];

	it('gives the checksummed case of a lower-cased address', () => {
		for (const v of vectors) expect(eip55(v.toLowerCase())).toBe(v);
	});

	it('leaves what is not an address as it came', () => {
		expect(eip55('0x123')).toBe('0x123');
		expect(eip55('vitalik.eth')).toBe('vitalik.eth');
	});
});
