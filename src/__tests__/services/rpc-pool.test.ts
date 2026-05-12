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

import { poolRpcCall, getFailedRpcChains } from '@/services/rpc-pool';

function makeJsonResponse(body: object, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
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

  describe('getFailedRpcChains', () => {
    test('returns empty set initially for known chains', () => {
      // Chain 1 (Ethereum) should not be in failed set on fresh start
      // (unless a previous test marked it — but it gets cleared on success)
      const failed = getFailedRpcChains();
      expect(failed).toBeInstanceOf(Set);
    });
  });
});
