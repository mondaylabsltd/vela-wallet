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
import { describe, expect, it } from 'vitest';
import { feeCallsOf, weiOf } from './fee-calls';

const tx = (call: Record<string, unknown>) => JSON.stringify([call]);
const ROUTER = '0x3fC91A3afd70395Cd496C647d5a6CC9D4B2b7FAD';

describe('a JSON-RPC quantity as wei', () => {
	it('reads the encodings dApps send for ZERO, "0x" included', () => {
		// The one that threw. `BigInt('0x')` is a SyntaxError, and several dApp
		// libraries write a zero value exactly like this.
		expect(weiOf('0x')).toBe('0');
		expect(weiOf(undefined)).toBe('0');
		expect(weiOf('')).toBe('0');
		expect(weiOf('0x0')).toBe('0');
		expect(weiOf(0)).toBe('0');
	});

	it('reads an amount however it was written', () => {
		expect(weiOf('0x38d7ea4c68000')).toBe('1000000000000000');
		expect(weiOf('1000000000000000')).toBe('1000000000000000');
		expect(weiOf(1000)).toBe('1000');
		expect(weiOf(10n ** 21n)).toBe('1000000000000000000000');
		expect(weiOf('  0x2a  ')).toBe('42');
	});

	it('says NULL for what is not a number, rather than calling it zero', () => {
		// A fee is computed from this. Reading nonsense as 0 would put a wrong
		// number beside a real transaction, which is worse than no number.
		expect(weiOf('later')).toBeNull();
		expect(weiOf('0xzz')).toBeNull();
		expect(weiOf(-1)).toBeNull();
		expect(weiOf(-1n)).toBeNull();
		expect(weiOf(1.5)).toBeNull();
		expect(weiOf({})).toBeNull();
	});
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
