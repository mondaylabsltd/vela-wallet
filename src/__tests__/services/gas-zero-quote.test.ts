/**
 * Regression tests for the legacy "maxFeePerGas must be > 0" dApp-tx failure:
 * a bundler gas quote of exactly 0x0 used to pass every guard — the sheet
 * displayed "预估费用 ~0 ETH" and the UserOp was signed with maxFeePerGas = 0,
 * which the bundler's own validation then rejected.
 *
 * Guards under test on the legacy quote parser:
 *   1. getBundlerGasQuote treats a 0 quote as "can't quote" (null → local fallback),
 *      instead of returning it as authoritative.
 *   2. isUsableFeeOverride — the legacy submit chokepoint only trusts a positive bigint
 *      override; anything else makes sendUserOp re-derive the price.
 *
 * UserOperation construction is covered by inband-send.test.ts: every supported
 * network now uses in-band settlement and therefore deliberately signs both
 * EntryPoint fee fields as zero.
 */

// Mock react-native + storage so the module imports cleanly under jsdom.
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));

const rpcCallMock = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({
  rpcCall: (...args: any[]) => rpcCallMock(...args),
}));

import {
  getBundlerGasQuote,
  isUsableFeeOverride,
} from '@/services/safe-transaction';

/**
 * Route mocked RPC methods. Chain gas reads are all-zero (the degenerate
 * environment that produced the bug: rate-limited RPC / zero-gas fork).
 */
function routeRpc(quoteTier: Record<string, string> | 'unsupported') {
  rpcCallMock.mockImplementation((method: string) => {
    switch (method) {
      case 'pimlico_getUserOperationGasPrice':
        if (quoteTier === 'unsupported') return Promise.reject(new Error('method not found'));
        return Promise.resolve({ result: { slow: quoteTier, standard: quoteTier, fast: quoteTier } });
      case 'eth_gasPrice':
        return Promise.resolve({ result: '0x0' });
      case 'eth_getBlockByNumber':
        return Promise.resolve({ result: { baseFeePerGas: '0x0' } });
      case 'eth_maxPriorityFeePerGas':
        return Promise.resolve({ result: '0x0' });
      case 'eth_getCode':
        // Safe + splitter both read as deployed.
        return Promise.resolve({ result: '0x6080' });
      case 'eth_call':
        return Promise.resolve({ result: '0x' + '00'.repeat(32) });
      case 'eth_estimateUserOperationGas':
        // Bundler estimation unavailable → static gas fallback (small calldata).
        return Promise.reject(new Error('estimation unavailable'));
      default:
        return Promise.reject(new Error(`unmocked method ${method}`));
    }
  });
}

const ZERO_QUOTE = {
  maxFeePerGas: '0x0',
  maxPriorityFeePerGas: '0x0',
  networkFeePerGas: '0x0',
  relayerFeePerGas: '0x0',
};

const HEALTHY_QUOTE = {
  maxFeePerGas: '0x4a817c800', // 20 gwei (2× markup over network)
  maxPriorityFeePerGas: '0x4a817c800',
  networkFeePerGas: '0x2540be400', // 10 gwei
  relayerFeePerGas: '0x2540be400',
};

const HIGH_QUOTE = {
  maxFeePerGas: '0x60db88400', // 26 gwei (2.6× reported network cost)
  maxPriorityFeePerGas: '0x60db88400',
  networkFeePerGas: '0x2540be400', // 10 gwei
  relayerFeePerGas: '0x3b9aca000',
};

// getGasPrices/isDeployed cache per chainId at module level — every test uses a
// fresh chain id so no test reads another's cached price.
let nextChain = 900_000;
const freshChain = () => ++nextChain;

beforeEach(() => {
  rpcCallMock.mockReset();
});

describe('getBundlerGasQuote — zero-quote guard', () => {
  test('a 0x0 quote is degenerate → null (local fallback), not an authoritative price', async () => {
    routeRpc(ZERO_QUOTE);
    await expect(getBundlerGasQuote(freshChain(), 'standard')).resolves.toBeNull();
  });

  test('a healthy quote still passes through verbatim', async () => {
    routeRpc(HEALTHY_QUOTE);
    const q = await getBundlerGasQuote(freshChain(), 'standard');
    expect(q).not.toBeNull();
    expect(q!.maxFeePerGas).toBe(20_000_000_000n);
    expect(q!.networkFeePerGas).toBe(10_000_000_000n);
  });

  test('a high quote is returned for the confirmation UI to display', async () => {
    routeRpc(HIGH_QUOTE);
    await expect(getBundlerGasQuote(freshChain(), 'standard')).resolves.toMatchObject({
      maxFeePerGas: 26_000_000_000n,
      networkFeePerGas: 10_000_000_000n,
    });
  });
});

describe('isUsableFeeOverride — submit chokepoint', () => {
  test('accepts only a positive bigint', () => {
    expect(isUsableFeeOverride(1n)).toBe(true);
    expect(isUsableFeeOverride(20_000_000_000n)).toBe(true);
  });

  test('rejects zero — a degenerate quote echoed back must not be signed', () => {
    expect(isUsableFeeOverride(0n)).toBe(false);
  });

  test('rejects negatives, non-bigints and absent values', () => {
    expect(isUsableFeeOverride(-1n)).toBe(false);
    expect(isUsableFeeOverride(undefined)).toBe(false);
    expect(isUsableFeeOverride(null)).toBe(false);
    expect(isUsableFeeOverride(5)).toBe(false);
    expect(isUsableFeeOverride('5')).toBe(false);
    expect(isUsableFeeOverride({})).toBe(false);
  });
});
