/**
 * Spec 098 §5: every REST request the web wallet makes to the relay names the
 * RPC this wallet uses for the chain. Without it the relay cannot read a
 * treasury on a network its directory does not reach, and answers 503 — which
 * the wallet routed as transient and sent through (098 §1).
 */
import { afterEach, describe, expect, it, vi } from 'vitest';

vi.mock('./rpc-pool', () => ({
	getActiveBundlerBaseUrl: async () => 'https://relay.example',
	getChainRpcUrl: async (chainId: number) =>
		chainId === 7 ? null : `https://rpc.example/${chainId}/KEY`,
	poolRpcCall: async () => ({ result: '0x' }),
	poolBundlerCall: async () => ({ result: '0x' }),
	getFailedRpcChains: () => new Set<number>(),
	getRateLimitedChains: () => new Set<number>()
}));

const { probeTreasury } = await import('./bundler-service');

function answering(body: unknown, status = 200) {
	return vi.fn(
		async () =>
			new Response(JSON.stringify(body), {
				status,
				headers: { 'content-type': 'application/json' }
			})
	);
}
const headersOf = (spy: ReturnType<typeof answering>) =>
	new Headers(((spy.mock.calls[0] as unknown[])[1] as RequestInit).headers);

afterEach(() => vi.unstubAllGlobals());

describe('the treasury probe names the chain RPC to the relay', () => {
	it('sends x-vela-rpc-url with the URL the wallet uses — key and all', async () => {
		const spy = answering({
			chainId: 1337,
			address: '0x' + '3e'.repeat(20),
			asset: 'native',
			balance: '0x0',
			floor: '0x5af3107a4000',
			bootstrapNeeded: true
		});
		vi.stubGlobal('fetch', spy);
		const probe = await probeTreasury(1337);
		expect(headersOf(spy).get('x-vela-rpc-url')).toBe('https://rpc.example/1337/KEY');
		expect(probe.kind).toBe('low-float');
	});

	it('sends no header when the core names no RPC for the chain', async () => {
		const spy = answering({ error: 'x' }, 503);
		vi.stubGlobal('fetch', spy);
		await probeTreasury(7);
		expect(headersOf(spy).has('x-vela-rpc-url')).toBe(false);
	});
});
