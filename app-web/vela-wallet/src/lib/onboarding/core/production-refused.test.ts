/**
 * r18-8 (§28, §32): a test build's write to the live key index — the member
 * challenge at creation, the publish — refused by the real guard (the core's
 * `productionWriteVerdict`) before it leaves. The index client says it as
 * its own kind (`refused`, never `network`), the shell answers the core
 * `index_failed { refused: true, network: false }`, and it was asked once.
 * The guard's inner `fetch` answers on this machine: nothing reaches out.
 */
import '$lib/i18n/wasm-init.server';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { productionWriteVerdict } from '../../../../../../rust/pkg-web/vela_core.js';
import { guardFetch, type Decide, type WriteVerdict } from '$lib/services/production-guard';
import { operationFailure } from './executor';
import { RegistryError, memberChallenge, registerGroup } from './registry';

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

/** A test build's `fetch`, guarded by the core: what it judged, and what got past it. */
function guarded() {
	const judged: string[] = [];
	const reached: string[] = [];
	const decide: Decide = async (ask) => {
		judged.push(`${ask.method} ${new URL(ask.url).host}${new URL(ask.url).pathname}`);
		return JSON.parse(productionWriteVerdict(JSON.stringify(ask))) as WriteVerdict;
	};
	const inner = (async (input: RequestInfo | URL) => {
		reached.push(String(input));
		return new Response('{}', { status: 200 });
	}) as typeof fetch;
	vi.stubGlobal(
		'fetch',
		guardFetch(inner, () => ({ testBuild: true, allowProductionWrites: false }), decide)
	);
	return { judged, reached };
}

afterEach(() => vi.unstubAllGlobals());

describe('a test build’s write to the live key index (r18-8)', () => {
	it('is refused before it leaves, once, and answered as refused — never the network', async () => {
		// No endpoint configured: the index is the production default.
		vi.stubGlobal('localStorage', new MemoryStorage());
		const { judged, reached } = guarded();
		const challenge = await memberChallenge({
			rpId: 'getvela.app',
			groupPublicKey: '0x04',
			publicKey: '0x04'
		}).catch((e: unknown) => e);
		expect(challenge).toBeInstanceOf(RegistryError);
		expect((challenge as RegistryError).refused).toBe(true);
		expect((challenge as RegistryError).network).toBe(false);
		expect(judged).toEqual(['POST p256-index-v2.getvela.app/api/challenge']);
		expect(reached).toEqual([]);
		const said = operationFailure(
			{
				id: 1,
				operation: { type: 'sign_member_proof' } as never
			},
			challenge
		);
		expect(said).toMatchObject({ type: 'index_failed', network: false, refused: true });

		const publish = await registerGroup({
			rpId: 'getvela.app',
			groupPublicKey: '0x04',
			groupProof: {} as never,
			members: []
		}).catch((e: unknown) => e);
		expect(
			operationFailure({ id: 2, operation: { type: 'registry_publish' } as never }, publish)
		).toMatchObject({ type: 'index_failed', network: false, refused: true });
		expect(judged).toHaveLength(2);
		expect(reached).toEqual([]);
	});

	it('a network failure is still the network’s, with no `refused`', () => {
		const said = operationFailure(
			{ id: 3, operation: { type: 'registry_publish' } as never },
			new RegistryError('Register failed: Failed to fetch', true)
		);
		expect(said).toEqual({
			type: 'index_failed',
			message: 'Register failed: Failed to fetch',
			network: true
		});
	});
});
