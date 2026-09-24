/**
 * The web rpc_pool executor (spec 025 T116): mechanical transport facts in,
 * zero routing decisions. Fetch is mocked at the boundary; the ban codec is
 * pinned to the cross-client bytes.
 */
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

import { createRpcPoolExecutor, readStoredBans } from './rpc-pool-executor';

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
				timeout_ms: 1000
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
				timeout_ms: 1000
			})
		);
		expect(result).toMatchObject({ outcome: { type: 'http_error', status: 429 } });
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
				timeout_ms: 1000
			})
		);
		expect(fetchSpy).not.toHaveBeenCalled();
		expect(result).toMatchObject({ outcome: { type: 'network' } });
	});

	it('never tells the relay which RPC endpoint this wallet prefers (spec 081)', async () => {
		const fetchSpy = vi.fn(
			async () =>
				new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: '0x1' }), {
					headers: { 'content-type': 'application/json' }
				})
		);
		vi.stubGlobal('fetch', fetchSpy);
		const executor = createRpcPoolExecutor(makeRegistry());
		await executor.execute(
			effect({
				type: 'json_rpc_post',
				call_id: 'c1',
				url: 'https://relay.example',
				method: 'eth_sendUserOperation',
				x_rpc_url: 'https://rpc.one',
				timeout_ms: 1000
			})
		);
		const init = (fetchSpy.mock.calls[0] as unknown[])[1] as { headers: Record<string, string> };
		// FR-007: that URL can carry a provider API key, and the relay reads
		// `x-vela-rpc-url` anyway — so it is never sent, even when the core
		// still names its own fastest pick.
		expect(Object.keys(init.headers ?? {})).not.toContain('X-Rpc-Url');
		expect(JSON.stringify(init.headers ?? {})).not.toContain('rpc.one');
	});
});

describe('conclude and the failure twin', () => {
	it('conclude settles through the registry', async () => {
		const registry = makeRegistry();
		const executor = createRpcPoolExecutor(registry);
		await executor.execute(
			effect({ type: 'conclude', call_id: 'c1', verdict: { type: 'failed', rate_limited: false } })
		);
		expect(registry.settled).toEqual([{ type: 'failed', rate_limited: false }]);
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
					timeout_ms: 1000
				}),
				new Error('boom')
			)
		).toMatchObject({ type: 'post_outcome', outcome: { type: 'network' } });
		expect(
			executor.toFailure(effect({ type: 'load_pool_config', chain_id: 1 }), new Error('boom'))
		).toMatchObject({ type: 'pool_config', rpc_endpoints: [] });
	});
});
