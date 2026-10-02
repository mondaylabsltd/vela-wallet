/**
 * The history record an approved dApp transaction leaves — the row Activity
 * draws (083 H2). Only what was submitted may reach it.
 */
import { describe, expect, it } from 'vitest';
import { buildSigningRecord } from './dapp-history';

const FROM = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const TO = '0xbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb';

describe('buildSigningRecord', () => {
	it('a single transaction keeps the recipient and figure it submitted', () => {
		const record = buildSigningRecord({
			method: 'eth_sendTransaction',
			params: [{ to: TO, value: '0x2386f26fc10000' }],
			storedRequest: JSON.stringify([{ to: TO, value: '0x2386f26fc10000' }]),
			requestTruncated: false,
			result: '0xhash',
			from: FROM,
			chainId: 100,
			dappOrigin: 'Uniswap',
			dappUrl: 'https://app.uniswap.org',
			nowMs: 1_757_000_000_000
		});
		expect(record).toMatchObject({
			type: 'dapp_tx',
			to: TO,
			value: '0x2386f26fc10000',
			dappUrl: 'https://app.uniswap.org'
		});
	});

	// 083 H2 review: a batch submits its `calls`; a top-level `to`/`value`
	// beside them is the page's to write and the sheet never shows it. Kept,
	// Activity drew a recipient and −1,208,925 xDAI that never moved.
	it('a batch keeps no recipient or figure the page wrote beside its calls', () => {
		const record = buildSigningRecord({
			method: 'wallet_sendCalls',
			params: [
				{
					version: '2.0.0',
					chainId: '0x64',
					calls: [{ to: TO, value: '0x0' }],
					to: '日本語日本語',
					value: '0xffffffffffffffffffff'
				}
			],
			storedRequest: '[]',
			requestTruncated: true,
			result: '0xbatchid',
			from: FROM,
			chainId: 100,
			dappOrigin: 'https://app.uniswap.org',
			nowMs: 1_757_000_000_000
		});
		// The batch's own first leg names the recipient (spec 082 RC7) and its
		// legs the figure — never the page's top-level words beside them.
		expect(record).toMatchObject({ type: 'dapp_tx', to: TO, value: '0x0' });
	});

	// Spec 093: what the record keeps of the request is the core's — its cut
	// (`stored_request`, parsed back into params), whether it was cut, its
	// summary and the sheet's balance changes — each stored as it came. This
	// side neither clips nor reads any of it.
	it("keeps the core's cut of the request, its summary and its balance changes verbatim", () => {
		const full = [{ to: TO, value: '0x0', data: '0x' + 'ab'.repeat(9000) }];
		const cut = [{ to: TO, value: '0x0', data: '0x' + 'ab'.repeat(100) }];
		const summary = { action: 'call' as const, calls: 1, contract: TO.toLowerCase() };
		const balanceChanges = [{ type: 'native' as const, delta: '-1000' }];
		const record = buildSigningRecord({
			method: 'eth_sendTransaction',
			params: full,
			storedRequest: JSON.stringify(cut),
			requestTruncated: true,
			summary,
			balanceChanges,
			result: '0xhash',
			from: FROM,
			chainId: 1,
			dappOrigin: 'Uniswap',
			dappUrl: 'https://app.uniswap.org',
			nowMs: 1_757_000_000_000
		});
		expect(record.signedRequest).toEqual({ method: 'eth_sendTransaction', params: cut });
		expect(record.requestTruncated).toBe(true);
		expect(record.dappSummary).toEqual(summary);
		expect(record.balanceChanges).toEqual(balanceChanges);
		expect(record.txHash).toBe('0xhash');
	});

	it('a signature keeps no result, and a stored request that will not read is none', () => {
		const record = buildSigningRecord({
			method: 'personal_sign',
			params: ['0x48656c6c6f', FROM],
			storedRequest: 'not json',
			requestTruncated: true,
			summary: { action: 'message', calls: 0 },
			result: '',
			from: FROM,
			chainId: 1,
			dappOrigin: 'https://app.uniswap.org',
			dappUrl: 'https://app.uniswap.org',
			nowMs: 1_757_000_000_000
		});
		expect(record).toMatchObject({ type: 'sign_message', txHash: '', requestTruncated: true });
		expect(record.signedRequest).toEqual({ method: 'personal_sign', params: [] });
		expect(record.dappSummary).toEqual({ action: 'message', calls: 0 });
		// No simulation, no stored lines — not an empty list.
		expect(record).not.toHaveProperty('balanceChanges');
		// What the message said is still the Connections list's.
		expect(record.signedContent).toBe('Hello');
	});

	// The core keeps `""` when the request's shape alone is past its cut.
	it('a request the core kept nothing of is stored as no params', () => {
		const record = buildSigningRecord({
			method: 'wallet_sendCalls',
			params: [{ calls: [] }],
			storedRequest: '',
			requestTruncated: true,
			result: '',
			from: FROM,
			chainId: 1,
			dappOrigin: 'https://app.example',
			nowMs: 1_757_000_000_000
		});
		expect(record.signedRequest).toEqual({ method: 'wallet_sendCalls', params: [] });
		expect(record.requestTruncated).toBe(true);
	});
});
