/**
 * Tests for wallet-api's token-fetch cache + in-flight dedup (US 2.1 — the home
 * paints fast without re-hammering 12 chains). A cache hit within TTL must make
 * NO new RPC, clearTokenCache must force a refetch, and concurrent callers must
 * share a single in-flight multi-chain fetch (not fan out N× the RPC load).
 *
 * The per-chain metadata + price paths are stubbed so every chain resolves fast
 * (empty); we assert on the *number of RPC calls*, which is cache behavior, not
 * balance decoding (covered elsewhere).
 */
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: async () => null, setItem: async () => {}, removeItem: async () => {} },
}));
jest.mock('@/services/chain-registry', () => ({ fetchChainInfo: async () => null }));
jest.mock('@/services/chain-tokens', () => ({
  fetchChainTokens: jest.fn(async () => null), // no metadata → native-only fallback
  pickQuoteToken: () => null,
}));
jest.mock('@/services/price-service', () => ({
  fetchChainlinkPrices: jest.fn(async () => ({})),
  resolveChainlinkPrice: () => null,
}));

const mockPoolRpcCall = jest.fn(async (..._args: any[]) => { throw new Error('rpc down'); });
jest.mock('@/services/rpc-pool', () => ({
  poolRpcCall: (...a: any[]) => mockPoolRpcCall(...a),
  getFailedRpcChains: () => new Set<number>(),
}));

import { fetchTokens, clearTokenCache } from '@/services/wallet-api';

const ADDR = '0x1111111111111111111111111111111111111111';

beforeEach(() => {
  jest.clearAllMocks();
  clearTokenCache();
});

describe('wallet-api token-fetch cache + dedup', () => {
  test('never throws when every chain fails — resolves to an empty list', async () => {
    await expect(fetchTokens(ADDR)).resolves.toEqual([]);
  });

  test('a second fetch within TTL is served from cache (no new RPC)', async () => {
    await fetchTokens(ADDR);
    const afterFirst = mockPoolRpcCall.mock.calls.length;
    expect(afterFirst).toBeGreaterThan(0); // the first fetch did hit the chains

    await fetchTokens(ADDR);
    expect(mockPoolRpcCall.mock.calls.length).toBe(afterFirst); // cached → zero new calls
  });

  test('clearTokenCache forces a fresh fetch', async () => {
    await fetchTokens(ADDR);
    const afterFirst = mockPoolRpcCall.mock.calls.length;

    clearTokenCache(ADDR);
    await fetchTokens(ADDR);
    expect(mockPoolRpcCall.mock.calls.length).toBeGreaterThan(afterFirst);
  });

  test('concurrent callers dedupe to a single in-flight multi-chain fetch', async () => {
    // Measure one fetch's RPC cost…
    await fetchTokens(ADDR);
    const single = mockPoolRpcCall.mock.calls.length;

    // …then fire three at once from a cold cache: they must share one fetch.
    clearTokenCache();
    mockPoolRpcCall.mockClear();
    await Promise.all([fetchTokens(ADDR), fetchTokens(ADDR), fetchTokens(ADDR)]);
    expect(mockPoolRpcCall.mock.calls.length).toBe(single);
  });
});
