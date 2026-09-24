import { beforeEach, describe, expect, it } from 'vitest';
import { DEFAULT_REGISTRY_URL, registryUrl } from './registry';
import { saveServiceEndpoints } from './storage';

/** The same in-memory `Storage` the other tests in this folder install. */
class MemoryStorage implements Storage {
	#map = new Map<string, string>();
	get length() {
		return this.#map.size;
	}
	clear() {
		this.#map.clear();
	}
	getItem(key: string) {
		return this.#map.get(key) ?? null;
	}
	key(index: number) {
		return [...this.#map.keys()][index] ?? null;
	}
	removeItem(key: string) {
		this.#map.delete(key);
	}
	setItem(key: string, value: string) {
		this.#map.set(key, value);
	}
}

/**
 * Spec 081 FR-002. The registry client used to snapshot its base URL into a
 * module global at import time, and the `setRegistryUrl` that could have moved
 * it had no callers — so somebody could point Settings → Service Endpoints at
 * their own public-key index, create a wallet, and have the key registered
 * with ours. These tests fail if the URL ever stops being read per call.
 */
describe('the registry reads the configured index every time', () => {
	beforeEach(() => {
		(globalThis as { localStorage?: Storage }).localStorage = new MemoryStorage();
	});

	it('uses the default when nothing is configured', () => {
		expect(registryUrl()).toBe(DEFAULT_REGISTRY_URL);
	});

	it('follows a change made after the module was imported', () => {
		saveServiceEndpoints({ passkeyIndexURL: 'https://index.example' });
		expect(registryUrl()).toBe('https://index.example');

		// And again, without anything re-importing or re-initialising.
		saveServiceEndpoints({ passkeyIndexURL: 'https://other.example/' });
		expect(registryUrl()).toBe('https://other.example');
	});

	it('treats a blank, whitespace or newline-poisoned value as unset', () => {
		for (const value of ['', '   ', '\n', 'https://index.example\n']) {
			saveServiceEndpoints({ passkeyIndexURL: value });
			const expected = value.trim() ? 'https://index.example' : DEFAULT_REGISTRY_URL;
			expect(registryUrl(), `a stored value of ${JSON.stringify(value)}`).toBe(expected);
		}
	});
});
