/**
 * fetchBundlerAccountInfo — settlementRecipient mapping (vela-relay vault mode,
 * docs/pool-queue-architecture.md Stage 2).
 *
 * The bundler tells the wallet where the in-band gas reimbursement should go:
 * normally the per-Safe EOA, the treasury in vault mode. The wallet must map the
 * field when present and well-formed, and degrade to the depositAddress fallback
 * (old bundlers / corrupted field) — sendUserOpTempo picks
 * `info.settlementRecipient ?? info.depositAddress` as the fee collector.
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

import { fetchBundlerAccountInfo } from '@/services/bundler-service';

const EOA = '0x1111111111111111111111111111111111111111';
const TREASURY = '0xcccccccccccccccccccccccccccccccccccccccc';

function mockAccountResponse(extra: Record<string, unknown>) {
  return jest.fn(async () => ({
    ok: true,
    json: async () => ({
      activeDepositAddress: EOA,
      onchainBalance: '0x0',
      spendableBalance: '0x0',
      status: 'ACTIVE',
      ...extra,
    }),
  })) as any;
}

describe('fetchBundlerAccountInfo settlementRecipient', () => {
  const realFetch = global.fetch;
  afterEach(() => {
    global.fetch = realFetch;
  });

  test('maps a well-formed settlementRecipient (vault mode: the treasury)', async () => {
    global.fetch = mockAccountResponse({ settlementRecipient: TREASURY });
    // Distinct chainId per test — the module caches per (chainId, safe) for 30s.
    const info = await fetchBundlerAccountInfo(84531, '0x' + 'aa'.repeat(20));
    expect(info?.settlementRecipient).toBe(TREASURY);
    expect(info?.depositAddress).toBe(EOA); // funding address never moves
    expect(info?.settlementRecipient ?? info?.depositAddress).toBe(TREASURY);
  });

  test('absent on old bundlers → undefined, fee collector falls back to the EOA', async () => {
    global.fetch = mockAccountResponse({});
    const info = await fetchBundlerAccountInfo(84532, '0x' + 'aa'.repeat(20));
    expect(info?.settlementRecipient).toBeUndefined();
    expect(info?.settlementRecipient ?? info?.depositAddress).toBe(EOA);
  });

  test('a malformed settlementRecipient is rejected, not passed to the fee leg', async () => {
    global.fetch = mockAccountResponse({ settlementRecipient: 'not-an-address' });
    const info = await fetchBundlerAccountInfo(84533, '0x' + 'aa'.repeat(20));
    expect(info?.settlementRecipient).toBeUndefined();
    expect(info?.settlementRecipient ?? info?.depositAddress).toBe(EOA);
  });
});
