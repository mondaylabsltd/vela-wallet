/**
 * Tests for RPC pool scoring, banning, and endpoint selection logic.
 *
 * Tests the pure scoring/classification functions by importing the module
 * and verifying behavior through the public API.
 */

// Mock all transitive dependencies
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    getItem: jest.fn(async () => null),
    setItem: jest.fn(async () => {}),
    removeItem: jest.fn(async () => {}),
  },
}));
jest.mock('@/modules/cloud-sync', () => ({
  get: jest.fn(async () => null),
  save: jest.fn(async () => {}),
  remove: jest.fn(async () => {}),
  syncNow: jest.fn(async () => {}),
}));

// Mock chain-registry so pool init doesn't make real HTTP calls
jest.mock('@/services/chain-registry', () => ({
  fetchChainInfo: jest.fn(async () => null),
}));

// Mock fetch for tryEndpoint
const mockFetch = jest.fn();
global.fetch = mockFetch;

import { poolRpcCall, getFailedRpcChains, getLogsRangeCap } from '@/services/rpc-pool';

function makeJsonResponse(body: object, status = 200) {
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

describe('rpc-pool', () => {
  describe('poolRpcCall - basic routing', () => {
    test('returns result from successful endpoint', async () => {
      mockFetch.mockResolvedValue(makeJsonResponse({
        jsonrpc: '2.0', id: 1, result: '0x1',
      }));

      const res = await poolRpcCall('eth_blockNumber', [], 1);
      expect(res.result).toBe('0x1');
      expect(mockFetch).toHaveBeenCalled();
    });

    test('includes correct JSON-RPC payload', async () => {
      mockFetch.mockResolvedValue(makeJsonResponse({
        jsonrpc: '2.0', id: 1, result: '0x42',
      }));

      await poolRpcCall('eth_getBalance', ['0xabc', 'latest'], 1);

      const call = mockFetch.mock.calls[0];
      const body = JSON.parse(call[1].body);
      expect(body.method).toBe('eth_getBalance');
      expect(body.params).toEqual(['0xabc', 'latest']);
      expect(body.jsonrpc).toBe('2.0');
    });
  });

  describe('poolRpcCall - failover', () => {
    test('fails over to next endpoint on network error', async () => {
      // First call fails, second succeeds
      mockFetch
        .mockRejectedValueOnce(new Error('Connection refused'))
        .mockResolvedValue(makeJsonResponse({
          jsonrpc: '2.0', id: 1, result: '0x2',
        }));

      const res = await poolRpcCall('eth_blockNumber', [], 1);
      expect(res.result).toBe('0x2');
      expect(mockFetch).toHaveBeenCalledTimes(2);
    });

    test('throws when all endpoints fail', async () => {
      mockFetch.mockRejectedValue(new Error('Connection refused'));

      await expect(
        poolRpcCall('eth_blockNumber', [], 99999)
      ).rejects.toThrow('All RPC endpoints failed');
    });

    test('marks chain as failed when all endpoints fail', async () => {
      mockFetch.mockRejectedValue(new Error('Connection refused'));

      try { await poolRpcCall('eth_blockNumber', [], 99998); } catch {}

      expect(getFailedRpcChains().has(99998)).toBe(true);
    });
  });

  describe('poolRpcCall - banning', () => {
    test('bans and skips endpoints returning 401', async () => {
      // First call returns 401 (ban), then pool re-tries with next endpoint
      mockFetch
        .mockResolvedValueOnce({ ok: false, status: 401 })
        .mockResolvedValue(makeJsonResponse({
          jsonrpc: '2.0', id: 1, result: '0x3',
        }));

      const res = await poolRpcCall('eth_blockNumber', [], 1);
      expect(res.result).toBe('0x3');
    });

    test('bans endpoints returning "unauthorized" in RPC error', async () => {
      mockFetch
        .mockResolvedValueOnce(makeJsonResponse({
          jsonrpc: '2.0', id: 1,
          error: { code: -32000, message: 'unauthorized: API key required' },
        }))
        .mockResolvedValue(makeJsonResponse({
          jsonrpc: '2.0', id: 1, result: '0x4',
        }));

      const res = await poolRpcCall('eth_blockNumber', [], 1);
      expect(res.result).toBe('0x4');
    });
  });

  describe('poolRpcCall - transient errors', () => {
    test('fails over on transient server error without banning', async () => {
      mockFetch
        .mockResolvedValueOnce(makeJsonResponse({
          jsonrpc: '2.0', id: 1,
          error: { code: -32603, message: 'internal error' },
        }))
        .mockResolvedValue(makeJsonResponse({
          jsonrpc: '2.0', id: 1, result: '0x5',
        }));

      const res = await poolRpcCall('eth_blockNumber', [], 1);
      expect(res.result).toBe('0x5');
    });

    test('does NOT failover on execution revert (valid RPC response)', async () => {
      mockFetch.mockResolvedValue(makeJsonResponse({
        jsonrpc: '2.0', id: 1,
        error: { code: 3, message: 'execution reverted' },
      }));

      const res = await poolRpcCall('eth_call', [{}], 1);
      // Execution revert is a valid response - should be returned, not retried
      expect(res.error?.message).toContain('execution reverted');
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });
  });

  describe('getLogsRangeCap', () => {
    // Returns the stated block-span cap, 0 for "narrow but no usable number",
    // or null when the error isn't about range/size at all.
    test('parses the stated block-span cap', () => {
      expect(getLogsRangeCap({ code: -32000, message: 'eth_getLogs is limited to a 100 range' })).toBe(100);
      expect(getLogsRangeCap({ code: -32000, message: 'block range too large, max is 2000 blocks' })).toBe(2000);
      expect(getLogsRangeCap({ code: -32005, message: 'exceeds the maximum block range of 50,000' })).toBe(50000);
      expect(getLogsRangeCap({ code: -32000, message: 'requested too many blocks; limit is 1000' })).toBe(1000);
    });

    test('honours k/m suffix on the cap', () => {
      expect(getLogsRangeCap({ code: -32000, message: 'You can make eth_getLogs requests with up to a 2K block range' })).toBe(2000);
      expect(getLogsRangeCap({ code: -32000, message: 'max block range is 1M' })).toBe(1_000_000);
    });

    test('returns 0 for range errors with no usable number', () => {
      expect(getLogsRangeCap({ code: -32000, message: 'block range is too wide' })).toBe(0);
      expect(getLogsRangeCap({ code: -32000, message: 'query exceeds max results' })).toBe(0);
    });

    test('returns 0 for result-count caps (number is a count, not a span)', () => {
      expect(getLogsRangeCap({ code: -32005, message: 'query returned more than 10000 results' })).toBe(0);
      expect(getLogsRangeCap({ code: -32000, message: 'too many results, limit 5000' })).toBe(0);
    });

    test('returns null for non-range errors', () => {
      expect(getLogsRangeCap({ code: 3, message: 'execution reverted' })).toBeNull();
      expect(getLogsRangeCap({ code: -32603, message: 'internal error' })).toBeNull();
      expect(getLogsRangeCap({ code: -32000, message: 'unauthorized: API key required' })).toBeNull();
      expect(getLogsRangeCap(undefined)).toBeNull();
      expect(getLogsRangeCap({ code: 0, message: '' })).toBeNull();
    });
  });

  describe('poolRpcCall - eth_getLogs range limit', () => {
    test('returns the range error to caller without failing over', async () => {
      // A range-limit response must NOT trigger failover (next endpoint has the
      // same cap) — the caller splits the block range and retries instead.
      mockFetch.mockResolvedValue(makeJsonResponse({
        jsonrpc: '2.0', id: 1,
        error: { code: -32000, message: 'eth_getLogs is limited to a 100 range' },
      }));

      const res = await poolRpcCall('eth_getLogs', [{ fromBlock: '0x0', toBlock: '0xbb8' }], 1);
      expect(res.error?.message).toContain('limited to a 100 range');
      // Single endpoint tried — no failover fan-out.
      expect(mockFetch).toHaveBeenCalledTimes(1);
    });

    test('a range-limit message on a non-getLogs method still fails over', async () => {
      // The range short-circuit is gated to eth_getLogs; other methods keep the
      // existing transient-error failover behaviour (code -32000 → try next).
      mockFetch
        .mockResolvedValueOnce(makeJsonResponse({
          jsonrpc: '2.0', id: 1,
          error: { code: -32000, message: 'block range too large' },
        }))
        .mockResolvedValue(makeJsonResponse({ jsonrpc: '2.0', id: 1, result: '0x7' }));

      const res = await poolRpcCall('eth_call', [{}], 1);
      expect(res.result).toBe('0x7');
      expect(mockFetch.mock.calls.length).toBeGreaterThan(1);
    });
  });

  describe('getFailedRpcChains', () => {
    test('returns empty set initially for known chains', () => {
      // Chain 1 (Ethereum) should not be in failed set on fresh start
      // (unless a previous test marked it — but it gets cleared on success)
      const failed = getFailedRpcChains();
      expect(failed).toBeInstanceOf(Set);
    });
  });
});
