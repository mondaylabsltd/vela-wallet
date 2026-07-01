/**
 * Tests wallet-api's merge-by-chain guarantee (US 2.2 — trusted total). Chains are
 * queried with Promise.allSettled, so ONE chain failing must contribute nothing
 * WITHOUT zeroing a healthy chain's balance. This is what lets the home fall back
 * to a partial-but-correct total instead of dropping to $0 when an RPC is down.
 *
 * We drive two chains: chain 1 returns a real (aggregate3-encoded) native balance,
 * chain 137 fails. Metadata/price paths are stubbed so the native fallback is the
 * only token source and the decode is deterministic.
 */
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: async () => null, setItem: async () => {}, removeItem: async () => {} },
}));
jest.mock('@/services/chain-registry', () => ({ fetchChainInfo: async () => null }));
jest.mock('@/services/chain-tokens', () => ({
  fetchChainTokens: async () => null, // no metadata → native-only fallback
  pickQuoteToken: () => null,
}));
jest.mock('@/services/price-service', () => ({
  fetchChainlinkPrices: async () => ({}),
  resolveChainlinkPrice: () => null,
}));
// Exactly two chains, so the merge is easy to reason about (only chainId is read).
jest.mock('@/models/network', () => {
  const actual = jest.requireActual('@/models/network');
  return { ...actual, getAllNetworksSync: () => [{ chainId: 1 }, { chainId: 137 }] };
});

const mockPoolRpcCall = jest.fn();
jest.mock('@/services/rpc-pool', () => ({
  poolRpcCall: (...a: any[]) => mockPoolRpcCall(...a),
  getFailedRpcChains: () => new Set<number>([137]),
}));

import { fetchTokens, clearTokenCache } from '@/services/wallet-api';

const ADDR = '0x1111111111111111111111111111111111111111';

// aggregate3 result = [(success:true, returnData: uint256(1e18))] — one native coin.
const word = (n: bigint) => n.toString(16).padStart(64, '0');
const ONE_COIN = '0x' + [32n, 1n, 32n, 1n, 64n, 32n, 10n ** 18n].map(word).join('');

beforeEach(() => {
  jest.clearAllMocks();
  clearTokenCache();
});

describe('wallet-api merge-by-chain (US 2.2 trusted total)', () => {
  test('a failing chain does NOT zero a healthy chain — the good balance survives', async () => {
    mockPoolRpcCall.mockImplementation(async (method: string, _p: any[], chainId: number) => {
      if (chainId === 137) throw new Error('rpc down'); // one chain is offline
      if (method === 'eth_call') return { result: ONE_COIN }; // the other returns 1 coin
      return { result: null };
    });

    const tokens = await fetchTokens(ADDR);

    // Chain 137 contributed nothing, but chain 1's balance is intact (not $0).
    expect(tokens).toHaveLength(1);
    expect(parseFloat(tokens[0].balance)).toBeGreaterThan(0);
  });

  test('both chains healthy → both balances merge into the list', async () => {
    mockPoolRpcCall.mockImplementation(async (method: string) => {
      if (method === 'eth_call') return { result: ONE_COIN };
      return { result: null };
    });

    const tokens = await fetchTokens(ADDR);
    expect(tokens).toHaveLength(2);
    expect(tokens.every((t) => parseFloat(t.balance) > 0)).toBe(true);
  });
});
