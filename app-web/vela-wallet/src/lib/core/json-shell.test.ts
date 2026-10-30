/**
 * The JSON wire to a wasm core: a result carrying an `i64` the generated
 * types spell as `bigint` (a wallet's EIP-1193 refusal code, a node's error
 * code) reaches the core as the JSON number it is — never a
 * "Do not know how to serialize a BigInt" fault that leaves the machine
 * waiting on that effect forever.
 */
import { describe, expect, it } from 'vitest';
import { createJsonWasmShell, type JsonWasmCore } from './json-shell';

describe('the JSON wire', () => {
	it('sends a bigint in a result as a JSON number', async () => {
		const resolved: string[] = [];
		let first = true;
		const core: JsonWasmCore = {
			dispatch: () =>
				JSON.stringify({ view: {}, effects: first ? [{ id: 1, operation: 'ask' }] : [] }),
			resolve_effect: (_id, result) => {
				first = false;
				resolved.push(result);
				return JSON.stringify({ view: {}, effects: [] });
			},
			view: () => '{}',
			free: () => {}
		};
		const loop = createJsonWasmShell<object, unknown, { id: number }, unknown>(core, {
			onView: () => {},
			execute: async () => ({ type: 'wallet_refused', code: 4001n, message: 'no' }),
			toFailure: () => ({})
		});
		loop.dispatch({ type: 'go' });
		await new Promise((r) => setTimeout(r, 0));
		expect(resolved).toEqual(['{"type":"wallet_refused","code":4001,"message":"no"}']);
		loop.dispose();
	});
});
