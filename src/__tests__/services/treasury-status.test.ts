/**
 * fetchTreasuryStatus — the per-chain treasury bootstrap signal
 * (GET /v1/treasury/:chainId, vela-relay Stage 2).
 *
 * bootstrapNeeded means the relayer can't operate on this chain until someone funds the
 * treasury directly (a NON-REFUNDABLE operator-float contribution). The wallet surfaces the
 * bootstrap sheet on that signal, so the parse must be strict: a malformed address or a
 * non-ok response → null (fall back to the normal funding/error surface, never a false ask).
 */

jest.mock('react-native', () => ({}));
jest.mock('@/models/network', () => ({ nativeSymbol: () => 'ETH' }));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/services/storage', () => ({ loadServiceEndpoints: jest.fn(async () => ({})) }));
jest.mock('@/services/tempo', () => ({ isTempoChain: () => false, TEMPO_DEFAULT_FEE_TOKEN: '0x0' }));
jest.mock('@/services/rpc-pool', () => ({
  getActiveBundlerBaseUrl: jest.fn(async () => 'https://bundler.test'),
  getChainRpcUrl: jest.fn(async () => null),
  isUsingBuiltinBundler: jest.fn(async () => true),
  poolRpcCall: jest.fn(),
  getBuiltinBundlerUrl: jest.fn(() => 'https://bundler.test'),
}));

import { fetchTreasuryStatus } from '@/services/bundler-service';

const TREASURY = '0xcccccccccccccccccccccccccccccccccccccccc';

function mockResponse(status: number, body: unknown) {
  return jest.fn(async () => ({ ok: status >= 200 && status < 300, status, json: async () => body })) as any;
}

describe('fetchTreasuryStatus', () => {
  const realFetch = global.fetch;
  afterEach(() => { global.fetch = realFetch; });

  test('parses a well-formed native status (hex balances → bigint)', async () => {
    global.fetch = mockResponse(200, {
      chainId: 42161, address: TREASURY, asset: 'native',
      balance: '0x' + (10n ** 15n).toString(16), floor: '0x' + (10n ** 16n).toString(16),
      bootstrapNeeded: true,
    });
    const s = await fetchTreasuryStatus(42161);
    expect(s).not.toBeNull();
    expect(s!.address).toBe(TREASURY);
    expect(s!.asset).toBe('native');
    expect(s!.balance).toBe(10n ** 15n);
    expect(s!.floor).toBe(10n ** 16n);
    expect(s!.bootstrapNeeded).toBe(true);
  });

  test('pathUSD asset passes through; bootstrapNeeded false parses', async () => {
    global.fetch = mockResponse(200, {
      chainId: 4217, address: TREASURY, asset: 'pathUSD',
      balance: '0x1', floor: '0x2', bootstrapNeeded: false,
    });
    const s = await fetchTreasuryStatus(4217);
    expect(s!.asset).toBe('pathUSD');
    expect(s!.bootstrapNeeded).toBe(false);
  });

  test('a malformed treasury address → null (no false bootstrap ask)', async () => {
    global.fetch = mockResponse(200, { address: 'not-an-address', asset: 'native', balance: '0x1', floor: '0x2', bootstrapNeeded: true });
    expect(await fetchTreasuryStatus(84531)).toBeNull();
  });

  test('a non-ok HTTP response → null', async () => {
    global.fetch = mockResponse(503, { error: 'unavailable' });
    expect(await fetchTreasuryStatus(84532)).toBeNull();
  });

  test('an unknown asset defaults to native; bootstrapNeeded is strictly the boolean', async () => {
    global.fetch = mockResponse(200, { address: TREASURY, balance: '0x1', floor: '0x2', bootstrapNeeded: 'yes' });
    const s = await fetchTreasuryStatus(84533);
    expect(s!.asset).toBe('native'); // absent/unknown → native
    expect(s!.bootstrapNeeded).toBe(false); // only a real `true` counts
  });

  test('a network throw → null (caller keeps its default surface)', async () => {
    global.fetch = jest.fn(async () => { throw new Error('network down'); }) as any;
    expect(await fetchTreasuryStatus(84534)).toBeNull();
  });
});
