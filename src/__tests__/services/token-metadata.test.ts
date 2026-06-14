/**
 * Tests for the on-chain ERC-20 metadata resolver.
 *
 * This is what stops a first-time receipt of an unknown token (not in the user's
 * token list) from rendering as "+0 tokens": it reads real symbol/decimals from
 * the contract, validates them, and caches the result (in-memory + persistent)
 * so repeat scans don't re-query. `poolRpcCall` and the ABI batch-decoder are
 * mocked so the resolver's batching/validation/caching logic is exercised
 * directly, on top of the real `decString` / `decU8` decoders.
 */

jest.mock('react-native', () => ({}));

// A fake AsyncStorage backed by a Map so multiSet → multiGet round-trips like
// the real persistent cache would across an app restart.
const mockStore = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: {
    multiGet: jest.fn(async (keys: string[]) => keys.map((k) => [k, mockStore.get(k) ?? null])),
    multiSet: jest.fn(async (pairs: [string, string][]) => { for (const [k, v] of pairs) mockStore.set(k, v); }),
  },
}));

const mockPoolRpcCall = jest.fn();
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall: (...args: any[]) => mockPoolRpcCall(...args) }));

// Keep every real abi helper except decAggregate3, which we stub so each test
// can hand the resolver a precise set of per-call results.
jest.mock('@/services/abi', () => ({
  ...jest.requireActual('@/services/abi'),
  decAggregate3: jest.fn(),
}));

import AsyncStorage from '@react-native-async-storage/async-storage';
import { decAggregate3 } from '@/services/abi';
import { resolveTokenMetadata, clearTokenMetadataCache } from '@/services/token-metadata';

const mockDecAgg = decAggregate3 as unknown as jest.Mock;

const A = '0x' + 'a'.repeat(40);
const B = '0x' + 'b'.repeat(40);
const C = '0x' + 'c'.repeat(40);
const CHAIN = 8453;

/** Encode an ABI `string` return value (offset/length/data). */
function encStr(s: string): string {
  const bytes = Array.from(new TextEncoder().encode(s));
  const dataHex = bytes.map((b) => b.toString(16).padStart(2, '0')).join('');
  const offset = (32).toString(16).padStart(64, '0');
  const length = bytes.length.toString(16).padStart(64, '0');
  return '0x' + offset + length + dataHex.padEnd(Math.ceil(dataHex.length / 64) * 64, '0');
}

/** Encode a uint256 (used for decimals returns). */
function u256(n: number): string {
  return '0x' + n.toString(16).padStart(64, '0');
}

beforeAll(() => {
  mockPoolRpcCall.mockResolvedValue({ result: '0xdeadbeef' }); // decAggregate3 ignores this
});

describe('resolveTokenMetadata', () => {
  test('resolves valid tokens, skips failed calls and out-of-range decimals', async () => {
    mockDecAgg.mockReturnValue([
      { success: true, data: encStr('USDC') }, { success: true, data: u256(6) },   // A: ok
      { success: false, data: '0x' },          { success: false, data: '0x' },      // B: call reverted
      { success: true, data: encStr('WEIRD') }, { success: true, data: u256(99) },  // C: insane decimals
    ]);

    const map = await resolveTokenMetadata(CHAIN, [A, B, C]);

    expect(map.get(A.toLowerCase())).toEqual({ symbol: 'USDC', decimals: 6 });
    expect(map.has(B.toLowerCase())).toBe(false);
    expect(map.has(C.toLowerCase())).toBe(false);
    // Single batched Multicall3 round-trip for all three tokens.
    expect(mockPoolRpcCall).toHaveBeenCalledTimes(1);
    // Only the resolved token is persisted.
    expect(AsyncStorage.multiSet).toHaveBeenCalledTimes(1);
    expect((AsyncStorage.multiSet as jest.Mock).mock.calls[0][0]).toHaveLength(1);
  });

  test('serves repeat lookups from the in-memory cache (no RPC)', async () => {
    mockPoolRpcCall.mockClear();

    const map = await resolveTokenMetadata(CHAIN, [A, B, C]);

    expect(map.get(A.toLowerCase())).toEqual({ symbol: 'USDC', decimals: 6 });
    expect(mockPoolRpcCall).not.toHaveBeenCalled(); // A cached positive, B/C cached negative
  });

  test('serves from the persistent cache after the in-memory cache is dropped', async () => {
    clearTokenMetadataCache(); // simulate an app restart
    mockPoolRpcCall.mockClear();

    const map = await resolveTokenMetadata(CHAIN, [A]);

    expect(map.get(A.toLowerCase())).toEqual({ symbol: 'USDC', decimals: 6 });
    expect(mockPoolRpcCall).not.toHaveBeenCalled(); // hydrated from AsyncStorage
  });

  test('returns an empty map for no addresses without touching the network', async () => {
    mockPoolRpcCall.mockClear();
    const map = await resolveTokenMetadata(CHAIN, []);
    expect(map.size).toBe(0);
    expect(mockPoolRpcCall).not.toHaveBeenCalled();
  });
});
