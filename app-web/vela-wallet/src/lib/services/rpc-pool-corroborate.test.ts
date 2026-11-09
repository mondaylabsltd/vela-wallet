/**
 * R22 (review 1-2): a corroborated read through the REAL `rpc_pool` core and
 * this shell's executor and registry — two endpoints of two providers, each
 * asked its chain id first (`ProbeChainId` outside any race, answered), the
 * call resolved with BOTH bodies; an ordinary read unchanged; "Only use my
 * node" trusted alone and said (`ownNode`). Fetch is mocked at the boundary.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value)),
	removeItem: vi.fn(async (key: string) => void kv.delete(key))
}));
vi.mock('$lib/core/client', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));

const world = {
	ownOnly: false,
	endpoints: [
		{ url: 'https://rpc.alpha.example/', source: 'default' as const },
		{ url: 'https://node.beta.example/', source: 'default' as const }
	]
};
vi.mock('$lib/services/rpc-pool-endpoints', async (importOriginal) => ({
	...(await importOriginal<typeof import('$lib/services/rpc-pool-endpoints')>()),
	collectRpcUrls: vi.fn(async () => world.endpoints),
	collectBundlerUrls: vi.fn(async () => [
		{ url: 'https://relay.example/1', source: 'builtin' as const }
	]),
	ownNodeOnly: vi.fn(async () => world.ownOnly)
}));

/** What each endpoint was asked, in order. */
let asked: { url: string; method: string }[] = [];

function node(url: string, method: string): unknown {
	if (method === 'eth_chainId') return '0x64';
	if (method === 'eth_call') return url.includes('alpha') ? '0x01' : '0x02';
	return '0x10';
}

beforeEach(() => {
	kv.clear();
	asked = [];
	vi.stubGlobal(
		'fetch',
		vi.fn(async (input: string | URL | Request, init?: RequestInit) => {
			const url = String(input);
			const { method } = JSON.parse(String(init?.body ?? '{}')) as { method: string };
			asked.push({ url, method });
			return new Response(JSON.stringify({ jsonrpc: '2.0', id: 1, result: node(url, method) }), {
				status: 200,
				headers: { 'content-type': 'application/json' }
			});
		})
	);
});

describe('the pool’s corroborated read (R22)', () => {
	it('asks two providers — each its chain id first — and resolves with both bodies', async () => {
		world.ownOnly = false;
		const { poolCorroboratedCall, poolRpcCall } = await import('./rpc-pool');
		const answer = await poolCorroboratedCall(
			'eth_call',
			[{ to: '0x01', data: '0x' }, 'latest'],
			100
		);
		expect(answer.ownNode).toBe(false);
		const results = [answer.body.result, answer.witnessBody?.result].sort();
		expect(results).toEqual(['0x01', '0x02']);
		// Each endpoint that answered said its chain first — outside any race.
		for (const host of ['alpha', 'beta']) {
			const own = asked.filter((a) => a.url.includes(host));
			expect(own.map((a) => a.method).slice(0, 2), host).toEqual(['eth_chainId', 'eth_call']);
		}
		// An ordinary read is as before: one body, no witness asked for it.
		asked = [];
		const plain = await poolRpcCall('eth_call', [{ to: '0x01', data: '0x' }, 'latest'], 100);
		expect(['0x01', '0x02']).toContain(plain.result);
		expect(asked.filter((a) => a.method === 'eth_call')).toHaveLength(1);
	});

	it('"Only use my node": the person’s node alone, said as such — nobody else asked', async () => {
		world.ownOnly = true;
		world.endpoints = [{ url: 'https://rpc.alpha.example/', source: 'user' as never }];
		const { invalidateAllPools, poolCorroboratedCall } = await import('./rpc-pool');
		// The person switched it on: every pool reads its config again.
		invalidateAllPools();
		const answer = await poolCorroboratedCall(
			'eth_call',
			[{ to: '0x01', data: '0x' }, 'latest'],
			100
		);
		expect(answer.ownNode).toBe(true);
		expect(answer.witnessBody).toBeNull();
		expect(answer.body.result).toBe('0x01');
		expect(asked.every((a) => a.url.includes('alpha'))).toBe(true);
	});
});
