/**
 * The web rpc_pool executor (spec 025 T116): mechanical transport facts in,
 * zero routing decisions. Fetch is mocked at the boundary; the ban codec is
 * pinned to the cross-client bytes.
 */
// The outcome of a thrown fetch is the core's rule (r11-9, `transportOutcomeOf`).
import '$lib/i18n/wasm-init.server';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { RpcPoolCallRegistry, RpcPoolEffect } from './rpc-pool-types';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
// Endpoint collection fans out into config/registry fetches — not this test's
// subject. The collector is exercised through load_pool_config with stubs.
vi.mock('$lib/services/rpc-pool-endpoints', async (importOriginal) => {
	const original = await importOriginal<typeof import('$lib/services/rpc-pool-endpoints')>();
	return {
		...original,
		collectRpcUrls: vi.fn(async () => [{ url: 'https://rpc.one', source: 'default' as const }]),
		collectBundlerUrls: vi.fn(async () => [
			{ url: 'https://relay.example/1', source: 'builtin' as const }
		])
	};
});

import {
	createEndpointSkipper,
	createRpcPoolExecutor,
	readStoredBans,
	SKIP_AFTER_FAILURES,
	SKIP_BASE_MS,
	SKIP_CAP_MS
} from './rpc-pool-executor';

const effect = (operation: RpcPoolEffect['operation']): RpcPoolEffect => ({ id: 1, operation });

function makeRegistry(overrides: Partial<RpcPoolCallRegistry> = {}): RpcPoolCallRegistry & {
	kept: Array<{ callId: string; url: string }>;
	settled: unknown[];
} {
	const kept: Array<{ callId: string; url: string }> = [];
	const settled: unknown[] = [];
	return {
		kept,
		settled,
		payload: () => ({ method: 'eth_blockNumber', params: [] }),
		keepBody: (callId, url) => kept.push({ callId, url }),
		settle: (_callId, verdict) => settled.push(verdict),
		...overrides
	};
}

beforeEach(() => kv.clear());
afterEach(() => vi.unstubAllGlobals());

describe('ban map codec', () => {
	it('round-trips the cross-client bytes under vela.rpc.banned', async () => {
		const executor = createRpcPoolExecutor(makeRegistry());
		await executor.execute(
			effect({
				type: 'persist_bans',
				entries: [{ url: 'https://bad.rpc', banned_at_ms: 123, permanent: true }]
			})
		);
		expect(JSON.parse(kv.get('vela.rpc.banned')!)).toEqual([
			{ url: 'https://bad.rpc', bannedAt: 123, permanent: true }
		]);
		expect(await readStoredBans()).toEqual([
			{ url: 'https://bad.rpc', banned_at_ms: 123, permanent: true }
		]);
	});

	it('junk records coerce or drop; a ban on "" bans nothing', async () => {
		kv.set(
			'vela.rpc.banned',
			JSON.stringify([{ url: '', bannedAt: 1 }, { url: 'https://x', bannedAt: 'soon' }, null])
		);
		expect(await readStoredBans()).toEqual([
			{ url: 'https://x', banned_at_ms: 0, permanent: false }
		]);
	});
});

describe('json_rpc_post outcomes are transport facts', () => {
	it('a JSON body is kept per URL and reported with its error member', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(
				async () =>
					new Response(
						JSON.stringify({
							jsonrpc: '2.0',
							id: 1,
							error: { code: -32005, message: 'rate limited' }
						}),
						{
							headers: { 'content-type': 'application/json' }
						}
					)
			)
		);
		const registry = makeRegistry();
		const executor = createRpcPoolExecutor(registry);
		const result = await executor.execute(
			effect({
				type: 'json_rpc_post',
				call_id: 'c1',
				url: 'https://rpc.one',
				method: 'eth_blockNumber',
				x_rpc_url: null,
				timeout_ms: 1000,
				must_send: false
			})
		);
		expect(result).toMatchObject({
			type: 'post_outcome',
			outcome: { type: 'response', error: { code: -32005 } }
		});
		expect(registry.kept).toEqual([{ callId: 'c1', url: 'https://rpc.one' }]);
	});

	it('a non-2xx reports its status; the meaning (ban vs 429) stays in Rust', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => new Response('nope', { status: 429 }))
		);
		const executor = createRpcPoolExecutor(makeRegistry());
		const result = await executor.execute(
			effect({
				type: 'json_rpc_post',
				call_id: 'c1',
				url: 'https://rpc.one',
				method: 'x',
				x_rpc_url: null,
				timeout_ms: 1000,
				must_send: false
			})
		);
		expect(result).toMatchObject({ outcome: { type: 'http_error', status: 429 } });
	});

	/**
	 * Round 3 (send-5): a server error whose body is a JSON-RPC `error` is
	 * the endpoint's own words — kept beside the status for a submit to hand
	 * on. The status is still what the core routes on; a gateway's page or
	 * a 4xx keeps nothing.
	 */
	it('a 5xx carrying a JSON-RPC error keeps that body; other error bodies are not kept', async () => {
		const post = effect({
			type: 'json_rpc_post',
			call_id: 'c1',
			url: 'https://relay.example/1',
			method: 'eth_sendUserOperation',
			x_rpc_url: null,
			timeout_ms: 1000,
			must_send: false
		});
		const refusal = { jsonrpc: '2.0', id: 1, error: { code: -32603, message: 'not accepted' } };
		const answers = [
			{ body: JSON.stringify(refusal), status: 500, kept: true },
			{ body: 'internal error', status: 500, kept: false },
			{ body: JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x' }), status: 502, kept: false },
			{ body: JSON.stringify(refusal), status: 400, kept: false }
		];
		for (const answer of answers) {
			vi.stubGlobal(
				'fetch',
				vi.fn(async () => new Response(answer.body, { status: answer.status }))
			);
			const bodies: unknown[] = [];
			const registry = makeRegistry({
				payload: () => ({ method: 'eth_sendUserOperation', params: [] }),
				keepBody: (_callId, _url, body) => bodies.push(body)
			});
			const result = await createRpcPoolExecutor(registry).execute(post);
			expect(result).toMatchObject({ outcome: { type: 'http_error', status: answer.status } });
			expect(bodies).toEqual(answer.kept ? [refusal] : []);
		}
	});

	/**
	 * Spec 082 RA1: `fetch` cannot tell a refused connection from a reply lost
	 * after the request went out, so a thrown fetch is `network` (it may have
	 * arrived) — unless the browser is offline before the call: then nothing
	 * left, and it is `not_connected`.
	 */
	it('a thrown fetch is network; offline before the call is not_connected', async () => {
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('Failed to fetch');
			})
		);
		const registry = makeRegistry({
			payload: () => ({ method: 'eth_sendUserOperation', params: [] })
		});
		const executor = createRpcPoolExecutor(registry);
		const post = effect({
			type: 'json_rpc_post',
			call_id: 'c1',
			url: 'https://relay.example/1',
			method: 'eth_sendUserOperation',
			x_rpc_url: null,
			timeout_ms: 1000,
			must_send: false
		});
		vi.stubGlobal('navigator', { onLine: true });
		await expect(executor.execute(post)).resolves.toMatchObject({
			outcome: { type: 'network' }
		});
		vi.stubGlobal('navigator', { onLine: false });
		await expect(executor.execute(post)).resolves.toMatchObject({
			outcome: { type: 'not_connected' }
		});
	});

	it('a vanished caller posts nothing and reports network', async () => {
		const fetchSpy = vi.fn();
		vi.stubGlobal('fetch', fetchSpy);
		const executor = createRpcPoolExecutor(makeRegistry({ payload: () => undefined }));
		const result = await executor.execute(
			effect({
				type: 'json_rpc_post',
				call_id: 'gone',
				url: 'https://rpc.one',
				method: 'x',
				x_rpc_url: null,
				timeout_ms: 1000,
				must_send: false
			})
		);
		expect(fetchSpy).not.toHaveBeenCalled();
		expect(result).toMatchObject({ outcome: { type: 'network' } });
	});

	function fetchOk() {
		return vi.fn(
			async () =>
				new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x1' }), {
					headers: { 'content-type': 'application/json' }
				})
		);
	}
	const headersOf = (spy: ReturnType<typeof fetchOk>) =>
		((spy.mock.calls[0] as unknown[])[1] as { headers: Record<string, string> }).headers;

	it('tells the RELAY the RPC this wallet uses for the chain, under the name it reads (spec 098)', async () => {
		const fetchSpy = fetchOk();
		vi.stubGlobal('fetch', fetchSpy);
		const executor = createRpcPoolExecutor(makeRegistry());
		await executor.execute(
			effect({
				type: 'json_rpc_post',
				call_id: 'c1',
				url: 'https://relay.example',
				method: 'eth_sendUserOperation',
				x_rpc_url: 'https://rpc.one/v2/KEY',
				timeout_ms: 1000,
				must_send: false
			})
		);
		// Key included — ruled (098 §0.1), and said where it is set.
		expect(headersOf(fetchSpy)['x-vela-rpc-url']).toBe('https://rpc.one/v2/KEY');
		// Not the pre-081 name the relay never read.
		expect(Object.keys(headersOf(fetchSpy))).not.toContain('X-Rpc-Url');
	});

	it('never names an RPC to an RPC provider — the core sets it on bundler calls only', async () => {
		const fetchSpy = fetchOk();
		vi.stubGlobal('fetch', fetchSpy);
		const executor = createRpcPoolExecutor(makeRegistry());
		await executor.execute(
			effect({
				type: 'json_rpc_post',
				call_id: 'c1',
				url: 'https://rpc.two',
				method: 'eth_call',
				x_rpc_url: null,
				timeout_ms: 1000,
				must_send: false
			})
		);
		expect(Object.keys(headersOf(fetchSpy))).not.toContain('x-vela-rpc-url');
	});
});

describe('conclude and the failure twin', () => {
	it('conclude settles through the registry', async () => {
		const registry = makeRegistry();
		const executor = createRpcPoolExecutor(registry);
		await executor.execute(
			effect({
				type: 'conclude',
				call_id: 'c1',
				verdict: { type: 'failed', rate_limited: false, maybe_delivered: false }
			})
		);
		expect(registry.settled).toEqual([
			{ type: 'failed', rate_limited: false, maybe_delivered: false }
		]);
	});

	it('every operation has a failure answer; a failed post is one more endpoint failure', () => {
		const executor = createRpcPoolExecutor(makeRegistry());
		expect(
			executor.toFailure(
				effect({
					type: 'json_rpc_post',
					call_id: 'c1',
					url: 'https://rpc.one',
					method: 'x',
					x_rpc_url: null,
					timeout_ms: 1000,
					must_send: false
				}),
				new Error('boom')
			)
		).toMatchObject({ type: 'post_outcome', outcome: { type: 'network' } });
		expect(
			executor.toFailure(effect({ type: 'load_pool_config', chain_id: 1 }), new Error('boom'))
		).toMatchObject({ type: 'pool_config', rpc_endpoints: [] });
	});
});

describe('the relay status method is the core’s spelling (spec 082 RA7, G13)', () => {
	it('USER_OP_STATUS_METHOD is userOpStatusMethod()', async () => {
		await import('$lib/i18n/wasm-init.server');
		const { userOpStatusMethod } = await import('$lib/core/kernels');
		const { USER_OP_STATUS_METHOD } = await import('$lib/services/rpc-adapter');
		expect(USER_OP_STATUS_METHOD).toBe(userOpStatusMethod());
		expect(USER_OP_STATUS_METHOD).toBe('pimlico_getUserOperationStatus');
		// The core's first load, inside the test: 5 s is not enough under a full parallel run.
	}, 30_000);
});

/**
 * home-17: 567 requests to one dead node in 40 s. An endpoint that failed on
 * transport three times in a row has its reads answered `not_connected` —
 * nothing left the device — for a window that doubles, and any answer from
 * it ends that. A write is never held back.
 */
describe('an endpoint that is not answering is skipped a while (home-17)', () => {
	const read = (url = 'https://dead.rpc', method = 'eth_call') =>
		effect({
			type: 'json_rpc_post',
			call_id: 'c1',
			url,
			method,
			x_rpc_url: null,
			timeout_ms: 1000,
			must_send: false
		});

	it('three refusals, then reads go nowhere until the window ends; an answer ends it', async () => {
		const fetchSpy = vi.fn(async () => {
			throw new TypeError('Failed to fetch');
		});
		vi.stubGlobal('fetch', fetchSpy);
		vi.stubGlobal('navigator', { onLine: true });
		const executor = createRpcPoolExecutor(makeRegistry());
		for (let i = 0; i < SKIP_AFTER_FAILURES; i++) {
			await expect(executor.execute(read())).resolves.toMatchObject({
				outcome: { type: 'network' }
			});
		}
		expect(fetchSpy).toHaveBeenCalledTimes(SKIP_AFTER_FAILURES);
		// Skipped: honest about it — nothing was sent.
		await expect(executor.execute(read())).resolves.toMatchObject({
			outcome: { type: 'not_connected' },
			latency_ms: 0
		});
		expect(fetchSpy).toHaveBeenCalledTimes(SKIP_AFTER_FAILURES);
		// Another endpoint is not affected.
		await executor.execute(read('https://other.rpc'));
		expect(fetchSpy).toHaveBeenCalledTimes(SKIP_AFTER_FAILURES + 1);
		// A write to the dead one still goes on the wire.
		await executor.execute(read('https://dead.rpc', 'eth_sendRawTransaction'));
		expect(fetchSpy).toHaveBeenCalledTimes(SKIP_AFTER_FAILURES + 2);
	});

	it('r17-1 / r17-10: the person’s own ask (`must_send`) is never answered without the wire', async () => {
		let up = false;
		const fetchSpy = vi.fn(async () => {
			if (!up) throw new TypeError('Failed to fetch');
			return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x1' }), {
				status: 200,
				headers: { 'content-type': 'application/json' }
			});
		});
		vi.stubGlobal('fetch', fetchSpy);
		vi.stubGlobal('navigator', { onLine: true });
		const executor = createRpcPoolExecutor(makeRegistry());
		for (let i = 0; i < SKIP_AFTER_FAILURES; i++) await executor.execute(read());
		// The outage lifts; a background read is still skipped…
		up = true;
		await expect(executor.execute(read())).resolves.toMatchObject({
			outcome: { type: 'not_connected' }
		});
		const before = fetchSpy.mock.calls.length;
		// …but the person's Try again reaches the node, and its answer ends the window.
		const theirs = effect({
			type: 'json_rpc_post',
			call_id: 'c1',
			url: 'https://dead.rpc',
			method: 'eth_getCode',
			x_rpc_url: null,
			timeout_ms: 1000,
			must_send: true
		});
		await expect(executor.execute(theirs)).resolves.toMatchObject({
			outcome: { type: 'response' }
		});
		expect(fetchSpy.mock.calls.length).toBe(before + 1);
		await executor.execute(read());
		expect(fetchSpy.mock.calls.length).toBe(before + 2);
	});

	it('the window doubles while it keeps failing, caps, and closes on any answer', () => {
		let now = 0;
		const skipper = createEndpointSkipper(() => now);
		const url = 'https://dead.rpc';
		for (let i = 0; i < SKIP_AFTER_FAILURES; i++) skipper.note(url, { type: 'timeout' });
		expect(skipper.skips(url, 'eth_call')).toBe(true);
		now += SKIP_BASE_MS;
		expect(skipper.skips(url, 'eth_call')).toBe(false);
		skipper.note(url, { type: 'network' });
		now += SKIP_BASE_MS;
		expect(skipper.skips(url, 'eth_call')).toBe(true); // doubled
		now += SKIP_BASE_MS;
		expect(skipper.skips(url, 'eth_call')).toBe(false);
		for (let i = 0; i < 20; i++) skipper.note(url, { type: 'network' });
		now += SKIP_CAP_MS;
		expect(skipper.skips(url, 'eth_call')).toBe(false);
		skipper.note(url, { type: 'http_error', status: 503 });
		skipper.note(url, { type: 'network' });
		expect(skipper.skips(url, 'eth_call')).toBe(false);
		// Back online: every endpoint afresh.
		for (let i = 0; i < SKIP_AFTER_FAILURES; i++) skipper.note(url, { type: 'not_connected' });
		expect(skipper.skips(url, 'eth_getBalance')).toBe(true);
		skipper.reset();
		expect(skipper.skips(url, 'eth_getBalance')).toBe(false);
	});
});

/**
 * Owner decision 2026-10-28: the person's "Only use my node" is a fact of
 * config, handed to the core with the collected lists — what it means is
 * the core's (`own_only_pool`).
 */
describe('load_pool_config carries "Only use my node"', () => {
	it('reads the switch from the network config, off when absent', async () => {
		const executor = createRpcPoolExecutor(makeRegistry());
		await expect(
			executor.execute(effect({ type: 'load_pool_config', chain_id: 100 }))
		).resolves.toMatchObject({ type: 'pool_config', own_only: false });
		kv.set(
			'vela.networkConfig',
			JSON.stringify([{ chainId: 100, rpcURL: 'https://mine.example', onlyMine: true }])
		);
		await expect(
			executor.execute(effect({ type: 'load_pool_config', chain_id: 100 }))
		).resolves.toMatchObject({ type: 'pool_config', own_only: true });
	});
});

/**
 * perf-6: a read silent for the hedge delay also asks the endpoint the core
 * will ask next (the view's `pending_urls`); the caller may take that answer
 * when the core says it is one every node gives alike; and when the core
 * reaches that endpoint it gets the hedge's answer, never a second request.
 */
describe('hedged reads (perf-6)', () => {
	const SLOW = 'https://slow.example';
	const NEXT = 'https://next.example';
	const policy = {
		afterMs: () => 1500,
		isHedged: (kind: string) => kind === 'rpc',
		early: (_kind: string, _method: string, url: string, outcome: { type: string }) =>
			outcome.type === 'response'
				? ({ type: 'respond', url, maybe_delivered: false } as const)
				: null
	};
	const post = (url: string) =>
		effect({
			type: 'json_rpc_post',
			call_id: 'c1',
			url,
			method: 'eth_getBalance',
			x_rpc_url: null,
			timeout_ms: 8000,
			must_send: false
		});

	function hedgeRegistry() {
		const early: Array<{ callId: string; url: string }> = [];
		const registry = makeRegistry({
			payload: () => ({ method: 'eth_getBalance', params: [], kind: 'rpc' as const }),
			view: () => ({
				failed_chains: [],
				rate_limited_chains: [],
				banned: [],
				unreached_chains: [],
				fallback_chains: [],
				own_only_chains: [],
				own_node_unreached_chains: [],
				out_of_reach_chains: [],
				silent_chains: [],
				pending_urls: { c1: [NEXT] }
			}),
			answerEarly: (callId, url) => early.push({ callId, url })
		});
		return { registry, early };
	}

	it('asks the next endpoint after the delay, answers the caller early, and the core reuses it', async () => {
		vi.useFakeTimers();
		try {
			let releaseSlow: (r: Response) => void = () => {};
			const fetchSpy = vi.fn((url: string) =>
				url === SLOW
					? new Promise<Response>((resolve) => (releaseSlow = resolve))
					: Promise.resolve(
							new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x5' }), {
								headers: { 'content-type': 'application/json' }
							})
						)
			);
			vi.stubGlobal('fetch', fetchSpy);
			const { registry, early } = hedgeRegistry();
			const executor = createRpcPoolExecutor(registry, policy);

			const slow = executor.execute(post(SLOW));
			await vi.advanceTimersByTimeAsync(1499);
			expect(fetchSpy).toHaveBeenCalledTimes(1);
			await vi.advanceTimersByTimeAsync(1);
			// The hedge went to the core's next endpoint.
			expect(fetchSpy).toHaveBeenCalledTimes(2);
			expect(fetchSpy.mock.calls[1][0]).toBe(NEXT);
			await vi.advanceTimersByTimeAsync(0);
			expect(early).toEqual([{ callId: 'c1', url: NEXT }]);
			expect(registry.kept).toContainEqual({ callId: 'c1', url: NEXT });

			// The slow endpoint fails; the core reaches NEXT — the hedge's answer.
			releaseSlow(new Response('down', { status: 503 }));
			await expect(slow).resolves.toMatchObject({ outcome: { type: 'http_error' } });
			await expect(executor.execute(post(NEXT))).resolves.toMatchObject({
				url: NEXT,
				outcome: { type: 'response', error: null }
			});
			expect(fetchSpy).toHaveBeenCalledTimes(2);
		} finally {
			vi.useRealTimers();
		}
	});

	it('home-20: a hedge answered with a node error passes to the endpoint after it, one at a time', async () => {
		vi.useFakeTimers();
		try {
			const THIRD = 'https://third.example';
			let releaseNext: (r: Response) => void = () => {};
			const fetchSpy = vi.fn((url: string) => {
				if (url === SLOW) return new Promise<Response>(() => {});
				if (url === NEXT) return new Promise<Response>((resolve) => (releaseNext = resolve));
				return Promise.resolve(
					new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x5' }), {
						headers: { 'content-type': 'application/json' }
					})
				);
			});
			vi.stubGlobal('fetch', fetchSpy);
			const early: Array<{ callId: string; url: string }> = [];
			const registry = makeRegistry({
				payload: () => ({ method: 'eth_getBalance', params: [], kind: 'rpc' as const }),
				view: () => ({
					failed_chains: [],
					rate_limited_chains: [],
					banned: [],
					unreached_chains: [],
					fallback_chains: [],
					own_only_chains: [],
					own_node_unreached_chains: [],
					out_of_reach_chains: [],
					silent_chains: [],
					pending_urls: { c1: [NEXT, THIRD] }
				}),
				answerEarly: (callId, url) => early.push({ callId, url })
			});
			// Only a result is an early answer here; a node's error is not.
			const strict = {
				...policy,
				early: (
					_kind: string,
					_method: string,
					url: string,
					outcome: { type: string; error?: unknown }
				) =>
					outcome.type === 'response' && !outcome.error
						? ({ type: 'respond', url, maybe_delivered: false } as const)
						: null
			};
			const executor = createRpcPoolExecutor(registry, strict);

			void executor.execute(post(SLOW));
			await vi.advanceTimersByTimeAsync(1500);
			expect(fetchSpy.mock.calls.map((c) => c[0])).toEqual([SLOW, NEXT]);
			// NEXT still out: nobody else is asked, however long SLOW stays silent.
			await vi.advanceTimersByTimeAsync(3000);
			expect(fetchSpy).toHaveBeenCalledTimes(2);

			releaseNext(
				new Response(
					JSON.stringify({ jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'busy' } }),
					{ headers: { 'content-type': 'application/json' } }
				)
			);
			await vi.advanceTimersByTimeAsync(0);
			expect(fetchSpy.mock.calls.map((c) => c[0])).toEqual([SLOW, NEXT, THIRD]);
			await vi.advanceTimersByTimeAsync(0);
			expect(early).toEqual([{ callId: 'c1', url: THIRD }]);
		} finally {
			vi.useRealTimers();
		}
	});

	it('an answer inside the delay asks nobody else', async () => {
		vi.useFakeTimers();
		try {
			const fetchSpy = vi.fn(async () => new Response('busy', { status: 503 }));
			vi.stubGlobal('fetch', fetchSpy);
			const { registry, early } = hedgeRegistry();
			const executor = createRpcPoolExecutor(registry, policy);
			await executor.execute(post(SLOW));
			await vi.advanceTimersByTimeAsync(5000);
			expect(fetchSpy).toHaveBeenCalledTimes(1);
			expect(early).toEqual([]);
		} finally {
			vi.useRealTimers();
		}
	});

	it('a write, or a call without a kind, is never hedged', async () => {
		vi.useFakeTimers();
		try {
			const fetchSpy = vi.fn(() => new Promise<Response>(() => {}));
			vi.stubGlobal('fetch', fetchSpy);
			const { registry } = hedgeRegistry();
			const executor = createRpcPoolExecutor(
				{ ...registry, payload: () => ({ method: 'eth_sendRawTransaction', params: [] }) },
				{ ...policy, isHedged: () => false }
			);
			void executor.execute(post(SLOW));
			await vi.advanceTimersByTimeAsync(5000);
			expect(fetchSpy).toHaveBeenCalledTimes(1);
		} finally {
			vi.useRealTimers();
		}
	});
});
