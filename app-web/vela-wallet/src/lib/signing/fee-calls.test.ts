/**
 * What the signing sheet can price (B-3/B-4, owner 2026-09-23).
 *
 * The owner signed a Uniswap swap on Polygon and saw no network fee and no
 * speed switch. Both come from one place: the host asks for a quote only when
 * this returns calls, and it enables the speed control only once a quote has
 * been asked for. So a request this cannot read loses BOTH, silently.
 *
 * The cases below are the shapes dApps actually send, `"0x"` included —
 * `BigInt("0x")` throws, and that throw is what took the fee away.
 */
import { readFileSync } from 'node:fs';
import { beforeAll, describe, expect, it } from 'vitest';
import { initSync } from '../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { feeCallsOf } from './fee-calls';

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

const tx = (call: Record<string, unknown>) => JSON.stringify([call]);
const ROUTER = '0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD';

/**
 * Spec 096 F1: the core's value table (`tx_request` `VALUE_TABLE`), as the fee
 * quote reads it — the same reading the submit sends. `null` = refused.
 */
const VALUE_TABLE: [unknown, string | null][] = [
	['0xaa87bee538000', '3000000000000000'],
	['0xAA87BEE538000', '3000000000000000'],
	['0x0', '0'],
	['0x', '0'],
	['', '0'],
	[null, '0'],
	[undefined, '0'],
	['0', '0'],
	[0, '0'],
	[
		'0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff',
		'115792089237316195423570985008687907853269984665640564039457584007913129639935'
	],
	['0x10000000000000000000000000000000000000000000000000000000000000000', null],
	['1000', null],
	['aa87bee538000', null],
	[1000, null],
	[1.5, null],
	['0X1f', null],
	['  0x2a  ', null],
	['-0x1', null],
	['0xzz', null],
	[{}, null]
];

describe("a call value, read by the core's one rule", () => {
	for (const [value, wei] of VALUE_TABLE) {
		it(`${JSON.stringify(value)} → ${wei ?? 'refused'}`, () => {
			const calls = feeCallsOf('transaction', tx({ to: ROUTER, value, data: '0x' }));
			expect(calls?.[0]?.value ?? null).toBe(wei);
		});
	}
});

describe('the calls a signing request can be priced by', () => {
	it('prices a swap whose value is "0x" — the case that lost the fee', () => {
		expect(
			feeCallsOf('transaction', tx({ from: '0xabc', to: ROUTER, value: '0x', data: '0x3593564c' }))
		).toEqual([{ to: ROUTER, value: '0', data: '0x3593564c' }]);
	});

	it('prices an ordinary swap', () => {
		expect(
			feeCallsOf(
				'transaction',
				tx({ to: ROUTER, value: '0x38d7ea4c68000', data: '0x3593564c', gas: '0x7a120' })
			)
		).toEqual([{ to: ROUTER, value: '1000000000000000', data: '0x3593564c' }]);
	});

	it('a plain transfer keeps its empty calldata', () => {
		expect(feeCallsOf('transaction', tx({ to: ROUTER, value: '0x2a' }))).toEqual([
			{ to: ROUTER, value: '42', data: '0x' }
		]);
	});

	it('prices a batch as its own list of calls', () => {
		const calls = [
			{ to: ROUTER, value: '0x', data: '0x095ea7b3' },
			{ to: ROUTER, value: '0x1', data: '0x3593564c' }
		];
		expect(feeCallsOf('batch', JSON.stringify([{ calls }]))).toEqual([
			{ to: ROUTER, value: '0', data: '0x095ea7b3' },
			{ to: ROUTER, value: '1', data: '0x3593564c' }
		]);
	});

	it('a message has nothing on chain to price', () => {
		expect(feeCallsOf('personal_sign', JSON.stringify(['0xdeadbeef', '0xabc']))).toBeNull();
		expect(feeCallsOf('typed_data', JSON.stringify(['0xabc', '{}']))).toBeNull();
		expect(feeCallsOf('generic', JSON.stringify([]))).toBeNull();
	});

	it('a call with no recipient is not priced as a call to nowhere', () => {
		// A deployment. This wallet does not send them, and a number invented
		// for one would be a lie beside a real request.
		expect(feeCallsOf('transaction', tx({ value: '0x0', data: '0x60806040' }))).toBeNull();
		expect(feeCallsOf('transaction', tx({ to: '', value: '0x0' }))).toBeNull();
	});

	it('an unreadable amount is refused, not read as free', () => {
		expect(feeCallsOf('transaction', tx({ to: ROUTER, value: 'soon' }))).toBeNull();
	});

	it('malformed params are nothing to price, and never a throw', () => {
		expect(feeCallsOf('transaction', 'not json')).toBeNull();
		expect(feeCallsOf('transaction', '{}')).toBeNull();
		expect(feeCallsOf('transaction', '[]')).toBeNull();
		expect(feeCallsOf('batch', JSON.stringify([{}]))).toBeNull();
	});
});
