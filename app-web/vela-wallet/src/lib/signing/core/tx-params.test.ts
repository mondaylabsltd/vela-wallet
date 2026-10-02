/**
 * The sheet reads a request the way the submit path sends it (spec 082 RC6):
 * the card the person approves and the call that goes out are one call.
 */
import '$lib/i18n/wasm-init.server';
import { describe, expect, it } from 'vitest';
import { ClearSigningCore } from '$lib/core/client';
import type { ClearSigningView } from '$lib/core/generated/ClearSigningView';
import { toClearLocale } from './clear-types';
import { txKickoff, txParams } from './tx-params';

const TO = '0x7687C0bC1dD2B9d7e9a5b1b4e1B0cBd8e0C3D141';

/** What the REAL clear-signing core draws for the params the sheet reads. */
function surfaceFor(paramsJson: string): ClearSigningView {
	const tx = txParams(paramsJson);
	const core = new ClearSigningCore();
	try {
		core.dispatch(
			JSON.stringify({
				type: 'resolve_transaction',
				to: tx?.to ?? null,
				data: tx?.data ?? null,
				value: tx?.value ?? null,
				chain_id: 100,
				locale: toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' })
			})
		);
		return JSON.parse(core.view()) as ClearSigningView;
	} finally {
		core.free();
	}
}

describe('the sheet reads a transaction as the submit path sends it (RC6)', () => {
	it('a JSON null data or value is "not given" — the plain send that is sent, with its amount', () => {
		// dapp-submit sends `data ?? '0x'` and `value ?? '0x0'`: this goes out as
		// a native send of 0.001. Read as the text "null" it drew the red blind
		// card with no amount over it (the desktop and Android read null as None).
		const params = JSON.stringify([{ to: TO, value: '0x38d7ea4c68000', data: null }]);
		expect(txParams(params)).toEqual({ to: TO, data: null, value: '0x38d7ea4c68000' });
		const view = surfaceFor(params);
		expect(view.surface).toBe('plain_send');
		expect(view.plain_send?.amount).toBe('0.001');

		const zero = JSON.stringify([{ to: TO, value: null }]);
		expect(txParams(zero)).toEqual({ to: TO, data: null, value: null });
		expect(surfaceFor(zero).plain_send?.no_value).toBe(true);
	});

	it('a present number is its own text, which the core refuses to print as an amount', () => {
		const params = JSON.stringify([{ to: TO, value: 1000 }]);
		expect(txParams(params)?.value).toBe('1000');
		expect(surfaceFor(params).surface).toBe('blind_transaction');
	});

	it('an eth_sendTransaction is its own top-level call — never a stray `calls` beside it', () => {
		// The submit path sends `params[0]`'s own to/data/value. A harmless
		// `calls` leg next to a malicious top-level call once had the harmless
		// one described while the malicious one was signed (083 review).
		const params = JSON.stringify([
			{
				to: TO,
				data: '0xdeadbeef',
				calls: [{ to: TO, value: '0x1', data: null }]
			}
		]);
		expect(txParams(params)).toEqual({ to: TO, data: '0xdeadbeef', value: null });
		const locale = toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' });
		expect(txKickoff('eth_sendTransaction', params, 100, locale)).toMatchObject({
			type: 'resolve_transaction',
			to: TO,
			data: '0xdeadbeef'
		});
	});

	it('a batch goes to the core whole, so every call of it is read (089 S1)', () => {
		const params = JSON.stringify([
			{
				calls: [
					{ to: TO, value: '0x1', data: null },
					{ to: TO, data: '0xdeadbeef' }
				]
			}
		]);
		const locale = toClearLocale({ number: 'comma_dot', date: 'iso', time: 'h24' });
		expect(txKickoff('wallet_sendCalls', params, 100, locale)).toEqual({
			type: 'resolve_batch',
			params_json: params,
			chain_id: 100,
			locale
		});
		const core = new ClearSigningCore();
		try {
			core.dispatch(JSON.stringify(txKickoff('wallet_sendCalls', params, 100, locale)));
			const view = JSON.parse(core.view()) as ClearSigningView;
			// Call 1 is a plain send and concluded at once; call 2 is still read,
			// so the sheet waits — it never shows call 1 as the request.
			expect(view.surface).toBe('loading');
			expect(view.plain_send).toBeNull();
		} finally {
			core.free();
		}
	});
});
