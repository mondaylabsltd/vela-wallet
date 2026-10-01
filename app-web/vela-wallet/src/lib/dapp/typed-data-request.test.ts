/**
 * What you see is what you sign — the web wallet and the extension's panel
 * (audit 2026-10-01). The sheet decodes `typedDataDocument`, the submit path
 * signs `pickTypedDataParam`; both are the REAL core (wasm), so they cannot
 * disagree, and neither reads anything from a request that carries two
 * documents.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { hashTypedData, typedDataDocument } from '$lib/core/kernels';
import { extractRequestChainId, pickTypedDataParam } from '$lib/services/dapp-submit';

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

const account = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const mail = {
	types: {
		EIP712Domain: [{ name: 'name', type: 'string' }, { name: 'chainId', type: 'uint256' }],
		Mail: [{ name: 'contents', type: 'string' }]
	},
	primaryType: 'Mail',
	domain: { name: 'Ether Mail', chainId: 100 },
	message: { contents: 'Hello' }
};
const permit = {
	types: {
		EIP712Domain: [{ name: 'name', type: 'string' }, { name: 'chainId', type: 'uint256' }],
		Permit: [{ name: 'spender', type: 'address' }, { name: 'value', type: 'uint256' }]
	},
	primaryType: 'Permit',
	domain: { name: 'USD Coin', chainId: 100 },
	message: { spender: '0x000000000000000000000000000000000000dEaD', value: '1' }
};
const hex = (b: Uint8Array) => Array.from(b, (x) => x.toString(16).padStart(2, '0')).join('');

describe('typed data: one reading for the sheet and the passkey', () => {
	it('every method: the sheet\'s document is the signed one', () => {
		for (const [method, params] of [
			['eth_signTypedData_v4', [account, JSON.stringify(permit)]],
			['eth_signTypedData_v3', [account, permit]],
			['eth_signTypedData', [JSON.stringify(permit), account]],
			['eth_signTypedData_v1', [permit, account]]
		] as const) {
			const shown = typedDataDocument(method, JSON.stringify(params));
			const signed = pickTypedDataParam(method, [...params]);
			expect(shown, method).not.toBeNull();
			expect(signed, method).toBe(shown);
			expect(hex(hashTypedData(JSON.parse(shown as string))), method).toBe(hex(hashTypedData(permit as never)));
			expect(extractRequestChainId(method, [...params]), method).toBe(100);
		}
	});

	it('the audit\'s two shapes give the sheet nothing and the passkey nothing', () => {
		const v4 = [JSON.stringify(mail), JSON.stringify(permit)];
		expect(typedDataDocument('eth_signTypedData_v4', JSON.stringify(v4))).toBeNull();
		expect(pickTypedDataParam('eth_signTypedData_v4', v4)).toBeUndefined();
		const legacy = [JSON.stringify(permit), JSON.stringify(mail)];
		expect(typedDataDocument('eth_signTypedData', JSON.stringify(legacy))).toBeNull();
		expect(pickTypedDataParam('eth_signTypedData_v1', legacy)).toBeUndefined();
		// No fallback to the only element, and no other method name.
		expect(pickTypedDataParam('eth_signTypedData_v4', [JSON.stringify(permit)])).toBeUndefined();
		expect(pickTypedDataParam('eth_signTypedData_v2', [account, JSON.stringify(permit)])).toBeUndefined();
	});
});
