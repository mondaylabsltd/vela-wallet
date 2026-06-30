/**
 * Tests for requestGasSponsorship — a non-idempotent treasury write. A timeout
 * must surface as `pending_unknown` (so the caller reconciles by polling the
 * balance) rather than a hard failure, and every request must carry a stable
 * Idempotency-Key so a retry can't double-spend the treasury.
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

import { requestGasSponsorship } from '@/services/bundler-service';

describe('requestGasSponsorship', () => {
  const realFetch = global.fetch;
  afterEach(() => {
    global.fetch = realFetch;
    jest.useRealTimers();
  });

  test('a timeout returns pending_unknown (not a hard network_error)', async () => {
    jest.useFakeTimers();
    global.fetch = jest.fn(
      (_url: string, init: any) =>
        new Promise((_resolve, reject) => {
          init.signal.addEventListener('abort', () => {
            const e = new Error('aborted');
            e.name = 'AbortError';
            reject(e);
          });
        }),
    ) as any;
    const p = requestGasSponsorship(1, '0xSafe', 1000n);
    await jest.advanceTimersByTimeAsync(25_000);
    await expect(p).resolves.toEqual({ sponsored: false, reason: 'pending_unknown' });
  });

  test('sends a stable Idempotency-Key derived from (chain, safe, amount)', async () => {
    let captured: any;
    global.fetch = jest.fn(async (_url: string, init: any) => {
      captured = init;
      return { ok: true, json: async () => ({ sponsored: true }) };
    }) as any;
    const result = await requestGasSponsorship(1, '0xAbCdEf', 1000n);
    expect(result).toEqual({ sponsored: true });
    expect(captured.headers['Idempotency-Key']).toBe('sponsor:1:0xabcdef:0x3e8');
  });

  test('non-timeout network failure still reported as network_error', async () => {
    global.fetch = jest.fn(async () => {
      throw new Error('ECONNRESET');
    }) as any;
    await expect(requestGasSponsorship(1, '0xSafe', 1000n)).resolves.toEqual({
      sponsored: false,
      reason: 'network_error',
    });
  });
});
