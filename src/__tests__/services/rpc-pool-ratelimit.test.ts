/**
 * Rate-limit classification in the RPC pool (US 2.2 — graceful degradation).
 *
 * Rate-limiting is commonplace on public RPC. When it takes a whole chain down it
 * must be classified as TRANSIENT (self-healing) — the balance keeps its cached
 * value and the UI stays quiet — as opposed to a hard failure, which surfaces the
 * "fix your RPC" banner. HomeScreen reads getRateLimitedChains() to draw that line.
 */
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(async () => null),
    setItem: jest.fn(async () => {}),
    removeItem: jest.fn(async () => {}),
  },
}));
jest.mock('@/services/chain-registry', () => ({ fetchChainInfo: jest.fn(async () => null) }));

const mockFetch = jest.fn();
// eslint-disable-next-line @typescript-eslint/no-explicit-any
global.fetch = mockFetch as any;

import { poolRpcCall, getFailedRpcChains, getRateLimitedChains } from '@/services/rpc-pool';

function jsonResponse(body: object, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    headers: { get: (h: string) => (h.toLowerCase() === 'content-type' ? 'application/json' : null) },
    json: async () => body,
  };
}

beforeEach(() => {
  jest.clearAllMocks();
  mockFetch.mockReset();
});

describe('rpc-pool rate-limit classification', () => {
  test('HTTP 429 on every endpoint → chain is failed AND flagged rate-limited (transient)', async () => {
    mockFetch.mockResolvedValue(jsonResponse({}, 429));
    await expect(poolRpcCall('eth_blockNumber', [], 137)).rejects.toThrow(/All RPC endpoints failed/);
    expect(getFailedRpcChains().has(137)).toBe(true);
    expect(getRateLimitedChains().has(137)).toBe(true);
  });

  test('a JSON-RPC rate-limit error (-32005 "too many requests") also flags rate-limited', async () => {
    mockFetch.mockResolvedValue(
      jsonResponse({ jsonrpc: '2.0', id: 1, error: { code: -32005, message: 'too many requests' } }),
    );
    await expect(poolRpcCall('eth_blockNumber', [], 42161)).rejects.toThrow();
    expect(getRateLimitedChains().has(42161)).toBe(true);
  });

  test('a hard failure (HTTP 500, no rate-limit signal) is failed but NOT rate-limited', async () => {
    mockFetch.mockResolvedValue(jsonResponse({}, 500));
    await expect(poolRpcCall('eth_blockNumber', [], 10)).rejects.toThrow();
    expect(getFailedRpcChains().has(10)).toBe(true);
    expect(getRateLimitedChains().has(10)).toBe(false);
  });

  test('a later success clears both the failed and rate-limited flags', async () => {
    mockFetch.mockResolvedValue(jsonResponse({}, 429));
    await expect(poolRpcCall('eth_blockNumber', [], 8453)).rejects.toThrow();
    expect(getRateLimitedChains().has(8453)).toBe(true);

    mockFetch.mockResolvedValue(jsonResponse({ jsonrpc: '2.0', id: 1, result: '0x1' }));
    const res = await poolRpcCall('eth_blockNumber', [], 8453);
    expect(res.result).toBe('0x1');
    expect(getFailedRpcChains().has(8453)).toBe(false);
    expect(getRateLimitedChains().has(8453)).toBe(false);
  });
});
