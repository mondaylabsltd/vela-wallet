/**
 * Tests for price-service — the Chainlink native-price fallback (Priority 2 after
 * DEX quotes). Focus on resolveChainlinkPrice's symbol aliasing, which is a real
 * footgun: nativeSymbol() returns "POL" for Polygon but the Chainlink feed is keyed
 * "MATIC", and Tempo's native gas symbol "USD" is a $1 peg with no feed at all.
 * The aggregate3 decode path is covered by abi/format tests; here we mock the RPC
 * only to prove the error path degrades to the cache (never throws).
 */
jest.mock('@/services/rpc-pool', () => ({ poolRpcCall: jest.fn() }));

import { resolveChainlinkPrice, fetchChainlinkPrices } from '@/services/price-service';
import { poolRpcCall } from '@/services/rpc-pool';

const PRICES = { ETH: 2500, BNB: 300, MATIC: 0.7, AVAX: 25, DAI: 1 };

describe('resolveChainlinkPrice — symbol → USD with aliasing', () => {
  test('direct symbol hit', () => {
    expect(resolveChainlinkPrice('ETH', PRICES)).toBe(2500);
    expect(resolveChainlinkPrice('BNB', PRICES)).toBe(300);
  });

  test('case-insensitive', () => {
    expect(resolveChainlinkPrice('eth', PRICES)).toBe(2500);
  });

  test('POL aliases to the MATIC feed', () => {
    expect(resolveChainlinkPrice('POL', PRICES)).toBe(0.7);
  });

  test('xDAI aliases to the DAI feed', () => {
    expect(resolveChainlinkPrice('XDAI', PRICES)).toBe(1);
  });

  test('Tempo "USD" native gas is a $1 peg (no feed needed)', () => {
    expect(resolveChainlinkPrice('USD', {})).toBe(1);
  });

  test('unknown symbol → null (no false price)', () => {
    expect(resolveChainlinkPrice('DOGE', PRICES)).toBeNull();
    expect(resolveChainlinkPrice('ETH', {})).toBeNull();
  });
});

describe('fetchChainlinkPrices — degrades on RPC failure', () => {
  test('RPC rejects and no cache yet → returns {} without throwing', async () => {
    (poolRpcCall as jest.Mock).mockRejectedValue(new Error('all endpoints down'));
    await expect(fetchChainlinkPrices()).resolves.toEqual({});
  });
});
