/**
 * Which bundler endpoints a chain offers, and in what tier.
 *
 * The pool is the only thing that decides where a user operation lands, so
 * this list is where "the relay I configured in Settings" is either true or
 * quietly untrue.
 */
import { afterEach, describe, expect, it, vi } from 'vitest';
import { collectBundlerUrls, NEVER_BANNED } from './rpc-pool-endpoints';
import { saveServiceEndpoints } from '$lib/onboarding/core/storage';

const local = new Map<string, string>();
Object.defineProperty(globalThis, 'localStorage', {
	configurable: true,
	value: {
		get length() {
			return local.size;
		},
		key: (i: number) => [...local.keys()][i] ?? null,
		getItem: (k: string) => local.get(k) ?? null,
		setItem: (k: string, v: string) => void local.set(k, v),
		removeItem: (k: string) => void local.delete(k),
		clear: () => local.clear()
	}
});

const config = vi.hoisted(() => ({ value: null as { bundlerURL?: string } | null }));
vi.mock('./records', () => ({
	getNetworkConfig: async () => config.value,
	getRpcProviderKeys: async () => ({})
}));

afterEach(() => {
	local.clear();
	config.value = null;
});

describe("a built-in chain's bundler", () => {
	it('is the relay configured in Settings › Service nodes', async () => {
		saveServiceEndpoints({ bundlerServiceURL: 'https://my-relay.example' });
		expect(await collectBundlerUrls(100, NEVER_BANNED)).toEqual([
			{ url: 'https://my-relay.example/100', source: 'builtin' }
		]);
	});

	it('is the shipped relay when nothing is configured', async () => {
		expect(await collectBundlerUrls(100, NEVER_BANNED)).toEqual([
			{ url: 'https://vela-relay-cf.getvela.app/100', source: 'builtin' }
		]);
	});

	// The bundler is not editable per built-in network, so a `bundlerURL`
	// stored against one is a snapshot an RPC edit left behind — today's
	// default, or a host that stopped being the default releases ago. Reading
	// it back as a `user` tier outranks the configured relay, and Settings
	// goes on showing a green badge for a URL nothing calls.
	it('ignores a stale override snapshot, whatever host it names', async () => {
		saveServiceEndpoints({ bundlerServiceURL: 'https://my-relay.example' });
		config.value = { bundlerURL: 'https://vela-relay.getvela.app/100' };
		expect(await collectBundlerUrls(100, NEVER_BANNED)).toEqual([
			{ url: 'https://my-relay.example/100', source: 'builtin' }
		]);
	});
});

describe("a custom chain's bundler", () => {
	it('keeps the one the person recorded, above the relay', async () => {
		saveServiceEndpoints({ bundlerServiceURL: 'https://my-relay.example' });
		config.value = { bundlerURL: 'https://my-own-bundler.example/31337' };
		expect(await collectBundlerUrls(31_337, NEVER_BANNED)).toEqual([
			{ url: 'https://my-own-bundler.example/31337', source: 'user' },
			{ url: 'https://my-relay.example/31337', source: 'builtin' }
		]);
	});
});
