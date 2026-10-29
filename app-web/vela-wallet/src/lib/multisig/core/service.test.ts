/**
 * The multisig service as the endpoints set it, and as an outage finds it:
 * the record's `genesisServiceURL` is the one used (the old key moved into
 * it once); a service that does not answer is marked away for every network
 * at once, background reads then ask nothing until the next step of a capped
 * schedule, and any answer, a new address or the person's Retry ends it. The
 * health answer is read by the core.
 */
import { readFileSync } from 'node:fs';
import { afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { initSync } from '../../../../../../rust/pkg-web/vela_core.js';
import { WASM_URL } from '../../../../../../rust/pkg-web/vela_core_wasm_url.js';

const storage = new Map<string, string>();
const memory = {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
};
vi.stubGlobal('localStorage', memory);

const { serviceAnswer } = await import('./kernel');
const { LEGACY_GENESIS_SERVICE_KEY, saveServiceEndpoints } =
	await import('$lib/onboarding/core/storage');
const { DEFAULT_GENESIS_SERVICE_URL, RETRY_STEPS_MS, genesisService, serviceHost, serviceOutage } =
	await import('./service-state.svelte');
const { checkService, fetchProposals, genesisServiceURL, servedChains, servedOnly } =
	await import('./service');

const RECORD = 'vela.serviceEndpoints';
const record = () => JSON.parse(storage.get(RECORD) ?? '{}') as Record<string, string>;

beforeAll(() => {
	initSync({ module: readFileSync(`../../assets/wasm${WASM_URL}`) });
});

const HEALTH = JSON.stringify({
	service: 'safe-genesis-registry',
	homeChainId: 5042002,
	chains: [{ chainId: 5042002, name: 'Arc Testnet' }]
});

function answering(status: number, body: string) {
	return vi.fn(async () => new Response(body, { status }));
}

beforeEach(() => {
	storage.clear();
	serviceOutage.reset();
});
afterEach(() => {
	vi.unstubAllGlobals();
	vi.stubGlobal('localStorage', memory);
});

describe('the address (one of the service endpoints)', () => {
	it('is the default until the record holds one', () => {
		expect(genesisServiceURL()).toBe(DEFAULT_GENESIS_SERVICE_URL);
		expect(genesisService.custom).toBeNull();
		storage.set(RECORD, JSON.stringify({ genesisServiceURL: 'http://127.0.0.1:8788/' }));
		expect(genesisServiceURL()).toBe('http://127.0.0.1:8788');
		expect(genesisService.custom).toBe('http://127.0.0.1:8788');
	});

	it('is the default when the record holds one the rule refuses, or a blank', () => {
		for (const refused of ['http://evil.example', 'javascript:alert(1)', '', '  ']) {
			storage.set(RECORD, JSON.stringify({ genesisServiceURL: refused }));
			expect(genesisServiceURL()).toBe(DEFAULT_GENESIS_SERVICE_URL);
			expect(genesisService.custom).toBeNull();
		}
	});

	it('moves the old key into the record once, and the record stands over it', () => {
		storage.set(RECORD, JSON.stringify({ bundlerServiceURL: 'http://127.0.0.1:8790' }));
		storage.set(LEGACY_GENESIS_SERVICE_KEY, 'http://127.0.0.1:8788');
		expect(genesisServiceURL()).toBe('http://127.0.0.1:8788');
		expect(record()).toEqual({
			bundlerServiceURL: 'http://127.0.0.1:8790',
			genesisServiceURL: 'http://127.0.0.1:8788'
		});
		expect(storage.has(LEGACY_GENESIS_SERVICE_KEY)).toBe(false);
		// A test seeding the old key again does not undo what Settings set.
		saveServiceEndpoints({ genesisServiceURL: 'http://127.0.0.1:8787' });
		storage.set(LEGACY_GENESIS_SERVICE_KEY, 'http://127.0.0.1:8788');
		expect(genesisServiceURL()).toBe('http://127.0.0.1:8787');
	});

	it('is named by its host in a line', () => {
		expect(serviceHost('http://127.0.0.1:8788')).toBe('127.0.0.1:8788');
		expect(serviceHost(null)).toBeNull();
	});
});

describe('the health answer (the core reads it)', () => {
	it('is the service, with its chains, or something else, or nothing', async () => {
		expect(serviceAnswer(200, HEALTH)).toEqual({
			kind: 'genesis',
			chains: [{ chainId: 5042002, name: 'Arc Testnet' }],
			homeChainId: 5042002
		});
		vi.stubGlobal('fetch', answering(200, JSON.stringify({ service: 'vela-relay' })));
		expect(await checkService('http://127.0.0.1:1')).toEqual({ kind: 'other' });
		vi.stubGlobal('fetch', answering(404, 'not found'));
		expect(await checkService('http://127.0.0.1:1')).toEqual({ kind: 'other' });
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('Failed to fetch');
			})
		);
		expect(await checkService('http://127.0.0.1:1')).toEqual({ kind: 'unreachable' });
	});

	it("gives the create sheet the service's chains, asked once a minute", async () => {
		storage.set(RECORD, JSON.stringify({ genesisServiceURL: 'http://127.0.0.1:8788' }));
		const fetch = answering(200, HEALTH);
		vi.stubGlobal('fetch', fetch);
		expect(await servedChains()).toEqual(new Set([5042002]));
		expect(await servedChains()).toEqual(new Set([5042002]));
		expect(fetch).toHaveBeenCalledTimes(1);
		expect(fetch.mock.calls[0]).toEqual([
			'http://127.0.0.1:8788/api/health',
			expect.objectContaining({ cache: 'no-store' })
		]);
	});
});

describe("the create sheet's networks", () => {
	const networks = [{ chainId: 100 }, { chainId: 5042002 }, { chainId: 1 }];

	it("are the ones the service serves, by its own table; all while that isn't known", () => {
		expect(servedOnly(networks, new Set([5042002]))).toEqual([{ chainId: 5042002 }]);
		expect(servedOnly(networks, new Set([42]))).toEqual([]);
		expect(servedOnly(networks, null)).toEqual(networks);
	});

	it("are not narrowed by a service that lists no chains, or can't be read", async () => {
		vi.stubGlobal('fetch', answering(200, JSON.stringify({ service: 'safe-genesis-registry' })));
		expect(await servedChains()).toBeNull();
		vi.stubGlobal(
			'fetch',
			vi.fn(async () => {
				throw new TypeError('Failed to fetch');
			})
		);
		storage.set(RECORD, JSON.stringify({ genesisServiceURL: 'http://127.0.0.1:1' }));
		expect(await servedChains()).toBeNull();
	});
});

describe('an outage', () => {
	const unreachable = () =>
		vi.fn(async () => {
			throw new TypeError('ERR_CONNECTION_CLOSED');
		});

	it('is the whole service: once marked, background reads ask nothing', async () => {
		const fetch = unreachable();
		vi.stubGlobal('fetch', fetch);
		// Three networks in one round: each asks once; the retries are not made.
		const reads = await Promise.allSettled(
			[100, 5042002, 1].map((chain) => fetchProposals(chain, '0xabc', { background: true }))
		);
		expect(reads.every((r) => r.status === 'rejected')).toBe(true);
		expect(fetch).toHaveBeenCalledTimes(3);
		expect(serviceOutage.down).toBe(true);
		// The next round, before the next step: no request at all.
		await expect(fetchProposals(100, '0xabc', { background: true })).rejects.toThrow(/away/);
		expect(fetch).toHaveBeenCalledTimes(3);
	});

	it('steps through a capped schedule, one probe at a time', async () => {
		let now = 1_000_000;
		serviceOutage.failed(now);
		expect(serviceOutage.retryAt).toBe(now + RETRY_STEPS_MS[0]);
		// Requests that were already out count once.
		serviceOutage.failed(now + 1);
		expect(serviceOutage.retryAt).toBe(now + RETRY_STEPS_MS[0]);
		for (let i = 1; i < RETRY_STEPS_MS.length + 2; i += 1) {
			now = serviceOutage.retryAt;
			serviceOutage.failed(now);
			const step = RETRY_STEPS_MS[Math.min(i, RETRY_STEPS_MS.length - 1)];
			expect(serviceOutage.retryAt - now).toBe(step);
		}
		// Bounded: five minutes at most between tries.
		expect(Math.max(...RETRY_STEPS_MS)).toBe(300_000);
	});

	it('shares one probe among every network of a round, and ends on its answer', async () => {
		vi.useFakeTimers();
		try {
			serviceOutage.failed();
			const probe = vi.fn(async () => false);
			expect(await serviceOutage.available(probe)).toBe(false);
			expect(probe).not.toHaveBeenCalled();
			vi.advanceTimersByTime(RETRY_STEPS_MS[0]);
			const round = await Promise.all([1, 2, 3].map(() => serviceOutage.available(probe)));
			expect(round).toEqual([false, false, false]);
			expect(probe).toHaveBeenCalledTimes(1);
			// Found away again: the next step is longer.
			expect(serviceOutage.retryAt - Date.now()).toBe(RETRY_STEPS_MS[1]);
			vi.advanceTimersByTime(RETRY_STEPS_MS[1]);
			expect(await serviceOutage.available(async () => true)).toBe(true);
			expect(serviceOutage.down).toBe(false);
		} finally {
			vi.useRealTimers();
		}
	});

	it('ends on any answer, a new address, or the Retry', async () => {
		serviceOutage.failed();
		vi.stubGlobal('fetch', answering(404, JSON.stringify({ error: 'unknown safe' })));
		// A person waiting asks anyway; the service answering ends the outage.
		expect(await fetchProposals(100, '0xabc')).toBeNull();
		expect(serviceOutage.down).toBe(false);

		serviceOutage.failed();
		serviceOutage.reset();
		expect(serviceOutage.blocked()).toBe(false);
	});

	it('is not a chain failing behind the service (an answer is an answer)', async () => {
		vi.stubGlobal('fetch', answering(502, JSON.stringify({ error: 'rpc' })));
		vi.useFakeTimers();
		try {
			const read = fetchProposals(100, '0xabc', { background: true }).catch((e: Error) => e);
			await vi.runAllTimersAsync();
			expect(await read).toBeInstanceOf(Error);
		} finally {
			vi.useRealTimers();
		}
		expect(serviceOutage.down).toBe(false);
	});
});
