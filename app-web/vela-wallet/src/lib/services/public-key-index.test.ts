/**
 * The registry name behind an ADDRESS (issue 191): chain → founding key →
 * index units → the unit that names this address. And what is remembered.
 */
import '$lib/i18n/wasm-init.server';
import { beforeEach, describe, expect, it, vi } from 'vitest';

// The REAL core decides every step here — that is what "written once" is
// worth. The server-side init above has already instantiated it; only the
// browser's fetch-and-instantiate is stood down.
vi.mock('$lib/core/client', async (original) => ({
	...(await original<typeof import('$lib/core/client')>()),
	loadCore: async () => {}
}));

const kv = new Map<string, string>();
vi.mock('$lib/services/storage', () => ({
	getItem: vi.fn(async (key: string) => kv.get(key) ?? null),
	setItem: vi.fn(async (key: string, value: string) => void kv.set(key, value))
}));
vi.mock('$lib/services/endpoints', () => ({
	getPasskeyIndexURL: () => 'https://index.test/'
}));

/** chainId → the raw `eth_call` answer; `'down'` = the pool reports an error. */
const chains = new Map<number, string>();
const rpcCalls: number[] = [];
vi.mock('$lib/services/rpc-pool', () => ({
	poolRpcCall: vi.fn(async (_method: string, _params: unknown[], chainId: number) => {
		rpcCalls.push(chainId);
		const answer = chains.get(chainId) ?? '0x' + '0'.repeat(192);
		return answer === 'down'
			? { jsonrpc: '2.0', id: 1, error: { code: -32000, message: 'unreachable' } }
			: { jsonrpc: '2.0', id: 1, result: answer };
	})
}));

/** path+query → JSON body, or a bare status. */
const index = new Map<string, unknown>();
const fetched: string[] = [];
vi.mock('$lib/services/net', () => ({
	NET_TIMEOUTS: { keyIndexRead: 1 },
	fetchWithTimeout: vi.fn(async (url: string) => {
		const path = url.replace('https://index.test', '');
		fetched.push(path);
		const body = index.get(path);
		if (typeof body === 'number') return { ok: false, status: body, text: async () => '{}' };
		if (body === undefined) return { ok: false, status: 404, text: async () => '{}' };
		return { ok: true, status: 200, text: async () => JSON.stringify(body) };
	})
}));

import { queryWalletName } from './public-key-index';

const SAFE = '0x88cCA0EeDbF2C4426110bbFc998F048689266894';
const X = '19'.repeat(32);
const Y = 'fe'.repeat(32);
const KEY = '04' + X + Y;
const CONFIGURED = '0x' + X + Y + '0'.repeat(61) + '100';

function metadataHex(address: string, names: string[]): string {
	const json = JSON.stringify({
		version: 1,
		address,
		wallet_version: 'safe-1.4.1',
		key_names: names,
		created_at_iso: '2026-08-21T00:00:00Z'
	});
	return [...new TextEncoder().encode(json)].map((b) => b.toString(16).padStart(2, '0')).join('');
}
function unit(id: number, address: string, names: string[]) {
	index.set(`/api/query?unitId=${id}&pageSize=1`, {
		unit: { metadata: metadataHex(address, names) }
	});
}

beforeEach(() => {
	kv.clear();
	chains.clear();
	index.clear();
	rpcCalls.length = 0;
	fetched.length = 0;
});

describe('queryWalletName', () => {
	it('chain → founding key → units → the unit that names THIS address', async () => {
		chains.set(100, CONFIGURED);
		index.set(`/api/query?publicKey=${KEY}`, { entry: {}, groups: { unitIds: [12, 10, 8] } });
		// The same key founded another wallet too; its name is not this one's.
		unit(12, '0x306BceFC18cAc6D2ECFb9D18d0b0A495a6C89aD8', ['GateDemo', 'Key 2']);
		unit(10, SAFE, ['Interleave', 'Key 2', 'Key 3']);
		unit(8, SAFE, ['MultiTest', 'Key 2', 'Key 3']);

		expect(await queryWalletName(SAFE)).toEqual({ name: 'Interleave', publicKey: KEY });
		expect(rpcCalls).toEqual([100]); // Gnosis answered: nobody else is asked
		expect(fetched.some((path) => path.includes('walletRef'))).toBe(false);
	});

	it('a hit is kept for good — the second ask touches nothing', async () => {
		chains.set(100, CONFIGURED);
		index.set(`/api/query?publicKey=${KEY}`, { groups: { unitIds: [10] } });
		unit(10, SAFE, ['Interleave']);
		await queryWalletName(SAFE);
		rpcCalls.length = 0;
		fetched.length = 0;

		expect((await queryWalletName(SAFE.toLowerCase()))?.name).toBe('Interleave');
		expect(rpcCalls).toEqual([]);
		expect(fetched).toEqual([]);
	});

	it('a Safe deployed elsewhere is found on the second tier', async () => {
		chains.set(8453, CONFIGURED);
		index.set(`/api/query?publicKey=${KEY}`, { groups: { unitIds: [10] } });
		unit(10, SAFE, ['On Base']);
		expect((await queryWalletName(SAFE))?.name).toBe('On Base');
		expect(rpcCalls[0]).toBe(100);
		expect(rpcCalls).toHaveLength(7);
	});

	it('a plain address is a miss, remembered briefly — and never asks the index', async () => {
		expect(await queryWalletName(SAFE)).toBeNull();
		expect(fetched).toEqual([]);
		rpcCalls.length = 0;
		expect(await queryWalletName(SAFE)).toBeNull();
		expect(rpcCalls).toEqual([]);

		// …briefly: past the window the chain is asked again.
		const stored = [...kv.entries()].find(([k]) => k.startsWith('vela.indexName:'));
		expect(stored).toBeDefined();
		kv.set(stored![0], JSON.stringify({ missAt: Date.now() - 7 * 60 * 60 * 1000 }));
		await queryWalletName(SAFE);
		expect(rpcCalls.length).toBeGreaterThan(0);
	});

	it('a chain that did not answer is not a verdict: nothing is remembered', async () => {
		chains.set(100, 'down');
		expect(await queryWalletName(SAFE)).toBeNull();
		expect(kv.size).toBe(0);
	});

	it('an index that is failing is not a miss either', async () => {
		chains.set(100, CONFIGURED);
		index.set(`/api/query?publicKey=${KEY}`, 503);
		expect(await queryWalletName(SAFE)).toBeNull();
		expect(kv.size).toBe(0);
	});

	it('a key with no unit for this address, a blank name, garbled bytes: no name', async () => {
		chains.set(100, CONFIGURED);
		index.set(`/api/query?publicKey=${KEY}`, { groups: { unitIds: [1, 2, 3] } });
		unit(1, '0x' + '11'.repeat(20), ['Somebody else']);
		unit(2, SAFE, ['   ']);
		index.set('/api/query?unitId=3&pageSize=1', { unit: { metadata: 'fffe' } });
		expect(await queryWalletName(SAFE)).toBeNull();
	});

	it('malformed and zero addresses cost nothing', async () => {
		expect(await queryWalletName('nope')).toBeNull();
		expect(await queryWalletName('0x' + '0'.repeat(40))).toBeNull();
		expect(rpcCalls).toEqual([]);
	});
});
