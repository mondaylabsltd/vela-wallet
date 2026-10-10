/**
 * Correctness batch item 5: `vela_getInBandGasQuote` publishes the relay's own
 * minimum per row (`minimumAmount`, relay contract §3) and the core floors at
 * it — so the row is read exactly as the relay wrote it, and an older relay
 * that publishes none leaves today's rule standing.
 */
import { describe, expect, it, vi } from 'vitest';

const relay = vi.hoisted(() => ({ rows: [] as unknown[] }));

vi.mock('./rpc-pool', () => ({
	getActiveBundlerBaseUrl: async () => 'https://relay.example',
	getChainRpcUrl: async () => null,
	poolRpcCall: async () => ({ result: '0x' }),
	poolBundlerCall: async () => ({ jsonrpc: '2.0', id: 1, result: relay.rows }),
	getFailedRpcChains: () => new Set<number>(),
	getRateLimitedChains: () => new Set<number>()
}));

const { _resetInBandQuoteCache, fetchInBandGasQuotes } = await import('./bundler-service');

const SAFE = '0x' + '88'.repeat(20);
const native = (minimumAmount?: unknown) => ({
	recipient: '0x' + '22'.repeat(20),
	asset: 'native',
	balance: '0xde0b6b3a7640000',
	decimals: 18,
	symbol: 'ETH',
	usdBalance: '2500.00',
	usdPrice: '2500',
	...(minimumAmount === undefined ? {} : { minimumAmount })
});

async function minimumOf(minimumAmount?: unknown): Promise<string | null | undefined> {
	_resetInBandQuoteCache();
	relay.rows = [native(minimumAmount)];
	const quotes = await fetchInBandGasQuotes(1, SAFE);
	return quotes?.[0]?.minimumAmount;
}

describe('the relay’s minimum on a quote row (item 5)', () => {
	it('is kept verbatim — hex as the relay writes it, a decimal string too', async () => {
		expect(await minimumOf('0x10c6f7a0b5ed8d')).toBe('0x10c6f7a0b5ed8d');
		expect(await minimumOf('4000000000000')).toBe('4000000000000');
	});

	it('is absent from an older relay, and anything that is not a quantity is dropped', async () => {
		expect(await minimumOf()).toBeNull();
		for (const garbage of [null, 12, '', '0x', '-5', '1e9', '0xzz']) {
			expect(await minimumOf(garbage), String(garbage)).toBeNull();
		}
	});
});
