/**
 * The runtime loader's caching contract (spec 024 T011).
 *
 * What is pinned is the PROMISE discipline, not the wasm itself: two callers
 * share one in-flight load, and a FAILED load is forgotten so the next call
 * retries instead of constructing cores over an uninitialized module. In
 * this node environment `fetch`ing the artifact fails by construction, which
 * is exactly the failure path the retry rule exists for.
 */
import { readFileSync } from 'node:fs';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { WASM_URL } from '../../../../../rust/pkg-web/vela_core_wasm_url.js';
import { loadCore, resetCoreForTests, whenCoreLoaded } from './client';

describe('loadCore', () => {
	beforeEach(() => {
		resetCoreForTests();
	});

	it('two immediate callers share one in-flight initialization', () => {
		const first = loadCore();
		const second = loadCore();
		expect(second).toBe(first);
		// Settle the rejection so nothing leaks past the test.
		return first.catch(() => undefined);
	});

	it('a failed load is not cached — the next call is a fresh attempt', async () => {
		const first = loadCore();
		await expect(first).rejects.toBeTruthy();
		const second = loadCore();
		expect(second).not.toBe(first);
		await second.catch(() => undefined);
	});
});

/**
 * perf-1: the root layout waits on `whenCoreLoaded()` above every page,
 * Welcome included — so it must never be what fetches the core, and must
 * settle once somebody else has loaded it. (Kept last: the core stays loaded.)
 */
describe('whenCoreLoaded', () => {
	afterEach(() => {
		vi.unstubAllGlobals();
	});

	it('asks for nothing, outlives a failed load, and settles on the first success', async () => {
		const fetch = vi.fn(
			async () =>
				new Response(readFileSync(`../../assets/wasm${WASM_URL}`), {
					headers: { 'content-type': 'application/wasm' }
				})
		);
		let settled = false;
		void whenCoreLoaded().then(() => (settled = true));
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(settled).toBe(false);

		// A failed load does not count as loaded.
		await loadCore().catch(() => undefined);
		await new Promise((resolve) => setTimeout(resolve, 0));
		expect(settled).toBe(false);

		vi.stubGlobal('fetch', fetch);
		resetCoreForTests();
		await loadCore();
		await whenCoreLoaded();
		expect(settled).toBe(true);
		expect(fetch).toHaveBeenCalledTimes(1);
	});
});
