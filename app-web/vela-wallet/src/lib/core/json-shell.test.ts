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

describe('answers without an unchanged view (view_host)', () => {
	type View = { a: number; pending?: Record<string, string[]> };
	function leanCore(answers: object[]) {
		const opted: boolean[] = [];
		const core: JsonWasmCore = {
			dispatch: () => JSON.stringify(answers.shift()),
			resolve_effect: () => JSON.stringify(answers.shift()),
			view: () => JSON.stringify({ a: 0 }),
			free: () => {},
			skip_unchanged_views: (includeVolatile) => opted.push(includeVolatile)
		};
		return { core, opted };
	}
	const shellOf = (core: JsonWasmCore, views: View[], optIn: boolean) =>
		createJsonWasmShell<View, unknown, { id: number }, unknown>(
			core,
			{ onView: (v) => views.push(v), execute: async () => ({}), toFailure: () => ({}) },
			optIn ? { skipUnchangedViews: { includeVolatile: true } } : {}
		);

	it('draws a view only when one came, with its volatile fields put back', () => {
		const { core, opted } = leanCore([
			{ view: { a: 1 }, view_version: 1, volatile: { pending: {} }, effects: [] },
			{ view_version: 1, effects: [] },
			{ view_version: 1, volatile: { pending: { c1: ['x'] } }, effects: [] },
			{ view: { a: 2 }, view_version: 2, effects: [] }
		]);
		const views: View[] = [];
		const loop = shellOf(core, views, true);
		expect(opted).toEqual([true]);
		for (let i = 0; i < 4; i++) loop.dispatch({ type: 'go' });
		expect(views).toEqual([
			{ a: 1, pending: {} },
			{ a: 1, pending: { c1: ['x'] } },
			{ a: 2, pending: { c1: ['x'] } }
		]);
		loop.dispose();
	});

	it('leaves a shell that did not opt in on the classic answers', () => {
		const { core, opted } = leanCore([
			{ view: { a: 1 }, effects: [] },
			{ view: { a: 1 }, effects: [] }
		]);
		const views: View[] = [];
		const loop = shellOf(core, views, false);
		loop.dispatch({ type: 'go' });
		loop.dispatch({ type: 'go' });
		expect(opted).toEqual([]);
		expect(views).toEqual([{ a: 1 }, { a: 1 }]);
		loop.dispose();
	});
});
