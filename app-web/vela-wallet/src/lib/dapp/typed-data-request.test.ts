import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it } from 'vitest';
import {
	canonicalTypedDataRequest,
	clearTypedDataCommitment,
	commitTypedDataRequest,
	requireCommittedTypedData
} from './typed-data-request';

const address = '0x3333333333333333333333333333333333333333';
const origin = 'https://example.invalid';
const method = 'eth_signTypedData_v4';
const zeroPreview = {
	types: { PermitTransferFrom: [{ name: 'amount', type: 'uint256' }] },
	primaryType: 'PermitTransferFrom',
	domain: { name: 'Permit2', chainId: 1 },
	message: { amount: '0' }
};
const nonzeroBatch = {
	types: { PermitBatchTransferFrom: [{ name: 'amounts', type: 'uint256[]' }] },
	primaryType: 'PermitBatchTransferFrom',
	domain: { name: 'Permit2', chainId: 1 },
	message: { amounts: ['1000000', '2000000'] }
};

afterEach(() => clearTypedDataCommitment('request', origin));

describe('canonical typed-data request', () => {
	it('rejects the reported v4 two-document shape before preview', () => {
		expect(() =>
			canonicalTypedDataRequest(method, [zeroPreview, JSON.stringify(nonzeroBatch)], address)
		).toThrow(/Invalid eth_signTypedData_v4 params/);
	});

	it('normalizes one document and binds its digest through submit', () => {
		const preview = commitTypedDataRequest(
			'request',
			origin,
			method,
			[address, nonzeroBatch],
			address
		);
		const submit = requireCommittedTypedData('request', origin, method, preview.params, address);
		expect(submit.paramsJson).toBe(preview.paramsJson);
		expect(submit.typedDataJson).toBe(preview.typedDataJson);
		expect(submit.digestHex).toBe(preview.digestHex);
	});

	it('fails closed if params change after the preview commitment', () => {
		commitTypedDataRequest('request', origin, method, [address, zeroPreview], address);
		expect(() =>
			requireCommittedTypedData('request', origin, method, [address, nonzeroBatch], address)
		).toThrow(/Preview and signing digest do not match/);
	});

	it('fails closed if signing is reached without a preview commitment', () => {
		expect(() =>
			requireCommittedTypedData('request', origin, method, [address, nonzeroBatch], address)
		).toThrow(/Preview and signing digest do not match/);
	});

	it('requires the v4 address to be the connected account', () => {
		expect(() =>
			canonicalTypedDataRequest(
				method,
				['0x7777777777777777777777777777777777777777', zeroPreview],
				address
			)
		).toThrow(/not the connected account/);
	});
});
