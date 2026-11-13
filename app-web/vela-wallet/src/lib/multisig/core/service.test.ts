/**
 * The multisig service in use is one of the service endpoints: the record's
 * `genesisServiceURL` (the old key moved into it once), the default
 * otherwise. Every request and outage is the core's (multisig_shell.rs).
 */
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const storage = new Map<string, string>();
const memory = {
	getItem: (k: string) => storage.get(k) ?? null,
	setItem: (k: string, v: string) => void storage.set(k, v),
	removeItem: (k: string) => void storage.delete(k)
};
vi.stubGlobal('localStorage', memory);

const { LEGACY_GENESIS_SERVICE_KEY, saveServiceEndpoints } =
	await import('$lib/onboarding/core/storage');
const { DEFAULT_GENESIS_SERVICE_URL, genesisService, serviceHost } =
	await import('./service-state.svelte');
const { genesisServiceURL } = await import('./service');

const RECORD = 'vela.serviceEndpoints';
const record = () => JSON.parse(storage.get(RECORD) ?? '{}') as Record<string, string>;

beforeEach(() => storage.clear());
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
