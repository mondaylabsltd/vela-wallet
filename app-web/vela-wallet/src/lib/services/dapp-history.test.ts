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
			result: '0xbatchid',
			from: FROM,
			chainId: 100,
			dappOrigin: 'https://app.uniswap.org',
			nowMs: 1_757_000_000_000
		});
		expect(record).toMatchObject({ type: 'dapp_tx', to: '', value: '0x0' });
	});
});
