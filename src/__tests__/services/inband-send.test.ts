/**
 * Generic in-band gas settlement — wallet side (vela-bundler
 * docs/inband-gas-settlement.md).
 *
 * Every sponsored UserOp on an in-band chain signs maxFeePerGas = 0 and batches
 * an in-band transfer (native value, or a whitelisted-stablecoin transfer) to
 * the bundler's recipient, sized by the bundler's own quote
 * (vela_getInBandGasQuote). Under test:
 *
 *   1. fetchInBandGasQuote — quote parse + the fail-to-null guards (error
 *      response, malformed recipient, zero amount).
 *   2. isInBandChain — Tempo is always in-band without probing; other chains
 *      learn capability from a 1-wei probe quote, cached.
 *   3. estimateTransactionFee — the in-band branch returns the QUOTED amount
 *      (feeAsset erc20/native, maxFeePerGas 0n) and falls through to the legacy
 *      estimate when the quote fails.
 *   4. buildInBandFeeLeg — the exact MultiSendCall shapes of the fee leg the
 *      bundler's reimbursement parser must count.
 */

// Mock react-native + storage so the modules import cleanly.
jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));

const rpcCallMock = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({
  rpcCall: (...args: any[]) => rpcCallMock(...args),
}));

// bundler-service reaches the bundler through the rpc-pool — poolBundlerCall is
// the seam the in-band quote rides on, so it MUST be under test control.
const poolBundlerCallMock = jest.fn();
jest.mock('@/services/rpc-pool', () => ({
  getActiveBundlerBaseUrl: jest.fn(async () => 'https://bundler.test'),
  getChainRpcUrl: jest.fn(async () => null),
  isUsingBuiltinBundler: jest.fn(async () => true),
  poolRpcCall: jest.fn(),
  poolBundlerCall: (...args: any[]) => poolBundlerCallMock(...args),
  getBuiltinBundlerUrl: jest.fn(() => 'https://bundler.test'),
}));

import {
  fetchInBandGasQuote,
  isInBandChain,
  _resetInBandSupportCache,
  _resetInBandQuoteCache,
} from '@/services/bundler-service';
import {
  buildInBandFeeLeg,
  estimateInBandBasisGas,
  estimateTransactionFee,
  requoteInBandFee,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';

const SAFE = '0x' + 'aa'.repeat(20);
const RECIPIENT = '0x1111111111111111111111111111111111111111';
const USDC = '0x2222222222222222222222222222222222222222';

// Module-level caches key per chainId (gas price, isDeployed, in-band support) —
// every test uses a fresh chain id so no test reads another's cached state.
let nextChain = 910_000;
const freshChain = () => ++nextChain;

beforeEach(() => {
  rpcCallMock.mockReset();
  poolBundlerCallMock.mockReset();
  _resetInBandSupportCache();
  _resetInBandQuoteCache();
});

// ---------------------------------------------------------------------------
// fetchInBandGasQuote
// ---------------------------------------------------------------------------

describe('fetchInBandGasQuote', () => {
  test('parses a happy native quote', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: {
        recipient: RECIPIENT,
        asset: 'native',
        requiredAmount: '0xde0b6b3a7640000', // 1e18
        markupX: 3,
      },
    });

    const q = await fetchInBandGasQuote(freshChain(), SAFE, 123_456n);
    expect(q).not.toBeNull();
    expect(q!.recipient).toBe(RECIPIENT);
    expect(q!.asset).toBe('native');
    expect(q!.feeToken).toBeNull();
    expect(q!.requiredAmount).toBe(1_000_000_000_000_000_000n);
    expect(q!.markupX).toBe(3);

    // The request carries the hex native cost and NO feeToken key for native.
    const [method, params] = poolBundlerCallMock.mock.calls[0];
    expect(method).toBe('vela_getInBandGasQuote');
    expect(params[0].safeAddress).toBe(SAFE);
    expect(params[0].nativeCost).toBe('0x' + (123_456n).toString(16));
    expect('feeToken' in params[0]).toBe(false);
  });

  test('passes the requested stablecoin and parses an erc20 quote', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: {
        recipient: RECIPIENT,
        asset: 'erc20',
        feeToken: USDC,
        requiredAmount: '0x2710', // 10000 = $0.01 at 6 decimals
        decimals: 6,
        markupX: 3,
      },
    });

    const q = await fetchInBandGasQuote(freshChain(), SAFE, 1n, USDC);
    expect(q).not.toBeNull();
    expect(q!.asset).toBe('erc20');
    expect(q!.feeToken).toBe(USDC);
    expect(q!.requiredAmount).toBe(10_000n);
    expect(q!.decimals).toBe(6);
    expect(poolBundlerCallMock.mock.calls[0][1][0].feeToken).toBe(USDC);
  });

  test('an error response (chain not enabled) → null', async () => {
    poolBundlerCallMock.mockResolvedValue({
      error: { code: -32601, message: 'in-band settlement not enabled on this chain' },
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE, 1n)).resolves.toBeNull();
  });

  test('a malformed recipient → null (never batch a transfer to garbage)', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: { recipient: 'not-an-address', asset: 'native', requiredAmount: '0x64', markupX: 3 },
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE, 1n)).resolves.toBeNull();
  });

  test('a zero/absent requiredAmount → null', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: { recipient: RECIPIENT, asset: 'native', requiredAmount: '0x0', markupX: 3 },
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE, 1n)).resolves.toBeNull();
  });

  test('a transport failure → null', async () => {
    poolBundlerCallMock.mockRejectedValue(new Error('All bundler endpoints failed'));
    await expect(fetchInBandGasQuote(freshChain(), SAFE, 1n)).resolves.toBeNull();
  });
});

// ---------------------------------------------------------------------------
// isInBandChain
// ---------------------------------------------------------------------------

describe('isInBandChain', () => {
  test('Tempo chains are always in-band, no probe fired', async () => {
    await expect(isInBandChain(4217, SAFE)).resolves.toBe(true);
    await expect(isInBandChain(42431, SAFE)).resolves.toBe(true);
    expect(poolBundlerCallMock).not.toHaveBeenCalled();
  });

  test('a successful probe marks the chain in-band and is cached', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: { recipient: RECIPIENT, asset: 'native', requiredAmount: '0x64', markupX: 3 },
    });
    const chain = freshChain();
    await expect(isInBandChain(chain, SAFE)).resolves.toBe(true);
    await expect(isInBandChain(chain, SAFE)).resolves.toBe(true);
    expect(poolBundlerCallMock).toHaveBeenCalledTimes(1); // second read hit the cache
  });

  test('a definitive "not enabled" is cached as false', async () => {
    poolBundlerCallMock.mockResolvedValue({
      error: { code: -32601, message: 'not enabled' },
    });
    const chain = freshChain();
    await expect(isInBandChain(chain, SAFE)).resolves.toBe(false);
    await expect(isInBandChain(chain, SAFE)).resolves.toBe(false);
    expect(poolBundlerCallMock).toHaveBeenCalledTimes(1);
  });
});

// ---------------------------------------------------------------------------
// estimateTransactionFee — in-band branch
// ---------------------------------------------------------------------------

/** Healthy bundler price quote: 20 gwei user price over a 10 gwei network cost. */
const HEALTHY_QUOTE = {
  maxFeePerGas: '0x4a817c800',
  maxPriorityFeePerGas: '0x4a817c800',
  networkFeePerGas: '0x2540be400',
  relayerFeePerGas: '0x2540be400',
};

/** Route the chain-side RPC mocks the generic estimate path reads. */
function routeRpc() {
  rpcCallMock.mockImplementation((method: string) => {
    switch (method) {
      case 'pimlico_getUserOperationGasPrice':
        return Promise.resolve({ result: { slow: HEALTHY_QUOTE, standard: HEALTHY_QUOTE, fast: HEALTHY_QUOTE } });
      case 'eth_gasPrice':
        return Promise.resolve({ result: '0x3b9aca00' }); // 1 gwei
      case 'eth_getBlockByNumber':
        return Promise.resolve({ result: { baseFeePerGas: '0x3b9aca00' } });
      case 'eth_maxPriorityFeePerGas':
        return Promise.resolve({ result: '0x3b9aca00' });
      case 'eth_getCode':
        // Safe + splitter both read as deployed.
        return Promise.resolve({ result: '0x6080' });
      case 'eth_estimateUserOperationGas':
        // Bundler estimation unavailable → static gas fallback (small calldata).
        return Promise.reject(new Error('estimation unavailable'));
      default:
        return Promise.reject(new Error(`unmocked method ${method}`));
    }
  });
}

describe('estimateTransactionFee — in-band branch', () => {
  test('stablecoin quote → feeAsset erc20 with the QUOTED amount, maxFee 0', async () => {
    routeRpc();
    // The capability probe (no feeToken) answers native; the real quote echoes
    // the requested stablecoin.
    poolBundlerCallMock.mockImplementation(async (_m: string, params: any[]) => {
      const p = params[0];
      if (p.feeToken) {
        return {
          result: { recipient: RECIPIENT, asset: 'erc20', feeToken: p.feeToken, requiredAmount: '0x2710', decimals: 6, markupX: 3 },
        };
      }
      return { result: { recipient: RECIPIENT, asset: 'native', requiredAmount: '0x64', markupX: 3 } };
    });

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard', undefined, undefined, USDC);
    expect(est.inBand).toBe(true);
    expect(est.maxFeePerGas).toBe(0n); // the op signs maxFee = 0
    expect(est.totalWei).toBe(0n); // native display not applicable
    expect(est.feeAsset).toEqual({ kind: 'erc20', token: USDC, decimals: 6, amount: 10_000n });
    // Compatibility fields stay populated off the bundler price quote.
    expect(est.networkFeePerGas).toBe(10_000_000_000n);
    expect(est.totalGas).toBeGreaterThan(0n);
  });

  test('native quote → totalWei is the QUOTED amount, feeAsset native', async () => {
    routeRpc();
    poolBundlerCallMock.mockResolvedValue({
      result: { recipient: RECIPIENT, asset: 'native', requiredAmount: '0x38d7ea4c68000', markupX: 3 }, // 1e15
    });

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard');
    expect(est.inBand).toBe(true);
    expect(est.maxFeePerGas).toBe(0n);
    expect(est.totalWei).toBe(1_000_000_000_000_000n);
    expect(est.feeAsset).toEqual({ kind: 'native' });
    // Sign-what-displayed: the estimate carries the recipient + basis so the submit path
    // signs exactly this quote and a fee-token switch can re-quote with one RPC.
    expect(est.feeRecipient).toBe(RECIPIENT);
    expect(est.nativeCostWei).toBeGreaterThan(0n);
  });

  test('in-band chain but the sizing quote fails → legacy estimate unchanged', async () => {
    routeRpc();
    // The 1-wei capability probe succeeds; the real (larger) quote fails.
    poolBundlerCallMock.mockImplementation(async (_m: string, params: any[]) => {
      if (params[0].nativeCost === '0x1') {
        return { result: { recipient: RECIPIENT, asset: 'native', requiredAmount: '0x64', markupX: 3 } };
      }
      return { error: { code: -32000, message: 'rate unavailable' } };
    });

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard');
    expect(est.inBand).toBeUndefined();
    expect(est.feeAsset).toBeUndefined();
    expect(est.maxFeePerGas).toBe(20_000_000_000n); // legacy: the bundler's quoted user price
    expect(est.totalWei).toBe(est.totalGas * est.maxFeePerGas);
  });

  test('chain not in-band at all → legacy estimate, only the probe hits the bundler', async () => {
    routeRpc();
    poolBundlerCallMock.mockResolvedValue({ error: { code: -32601, message: 'not enabled' } });

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard');
    expect(est.inBand).toBeUndefined();
    expect(est.maxFeePerGas).toBe(20_000_000_000n);
    expect(poolBundlerCallMock).toHaveBeenCalledTimes(1); // the capability probe only
  });

  test('requested stablecoin but the bundler quotes a different asset → legacy fallback', async () => {
    routeRpc();
    // Bundler ignores the request and answers native — the wallet must NOT
    // display a fee in an asset the send path would refuse to pay.
    poolBundlerCallMock.mockResolvedValue({
      result: { recipient: RECIPIENT, asset: 'native', requiredAmount: '0x64', markupX: 3 },
    });

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard', undefined, undefined, USDC);
    expect(est.inBand).toBeUndefined();
    expect(est.feeAsset).toBeUndefined();
    expect(est.maxFeePerGas).toBe(20_000_000_000n);
  });
});

// ---------------------------------------------------------------------------
// estimateInBandBasisGas — the precise charge basis (real batch calldata, not the
// padded model that over-charged ~8× on Arbitrum)
// ---------------------------------------------------------------------------

describe('estimateInBandBasisGas', () => {
  const NATIVE_LEG = { to: RECIPIENT, value: '0x1', data: new Uint8Array(0) };

  test('sums the bundler UN-PADDED estimate (verification + call + preVerification)', async () => {
    rpcCallMock.mockImplementation((method: string) => {
      if (method === 'eth_call') return Promise.resolve({ result: '0x' + '00'.repeat(31) + '07' }); // getNonce = 7
      if (method === 'eth_estimateUserOperationGas') {
        return Promise.resolve({ result: {
          verificationGasLimit: '0x186a0', // 100000
          callGasLimit: '0xc350',          // 50000
          preVerificationGas: '0x4e20',    // 20000
        }});
      }
      return Promise.reject(new Error(`unmocked ${method}`));
    });
    const gas = await estimateInBandBasisGas(SAFE, [NATIVE_LEG], null, freshChain());
    expect(gas).toBe(170_000n); // 100000 + 50000 + 20000
  });

  test('returns null when the bundler estimate is unavailable (caller keeps the rough basis)', async () => {
    rpcCallMock.mockImplementation((method: string) => {
      if (method === 'eth_call') return Promise.resolve({ result: '0x' + '00'.repeat(32) });
      if (method === 'eth_estimateUserOperationGas') return Promise.reject(new Error('estimation unavailable'));
      return Promise.reject(new Error(`unmocked ${method}`));
    });
    const gas = await estimateInBandBasisGas(SAFE, [NATIVE_LEG], null, freshChain());
    expect(gas).toBeNull();
  });

  test('prices an N-leg batch off its own calldata (the D fix — batch modes were rough)', async () => {
    const dicts: any[] = [];
    rpcCallMock.mockImplementation((method: string, params: any[]) => {
      if (method === 'eth_call') return Promise.resolve({ result: '0x' + '00'.repeat(31) + '01' });
      if (method === 'eth_estimateUserOperationGas') {
        dicts.push(params[0]); // capture the userOp the batch produced
        return Promise.resolve({ result: { verificationGasLimit: '0x1', callGasLimit: '0x1', preVerificationGas: '0x1' } });
      }
      return Promise.reject(new Error(`unmocked ${method}`));
    });
    const twoLegs = [
      { to: USDC, value: '0', data: '0xa9059cbb' + '00'.repeat(64) },
      { to: RECIPIENT, value: '0x64', data: new Uint8Array(0) },
    ];
    await estimateInBandBasisGas(SAFE, twoLegs as any, USDC, freshChain());
    // The estimate ran against a real multi-leg callData (a placeholder fee leg is appended),
    // so its calldata is longer than an empty op — the whole point of the precise basis.
    expect(dicts).toHaveLength(1);
    expect(String(dicts[0].callData).length).toBeGreaterThan(10);
  });
});

// ---------------------------------------------------------------------------
// requoteInBandFee — the fee-token-switch fast path (one bundler RPC, no re-estimate)
// ---------------------------------------------------------------------------

describe('requoteInBandFee', () => {
  const base: TransactionFeeEstimate = {
    totalWei: 3_000n, maxFeePerGas: 0n, networkFeePerGas: 1n, relayerFeePerGas: 0n,
    bundlerGasPrice: 1n, totalGas: 300_000n, deployed: true, tier: 'fast', quoted: true,
    inBand: true, feeRecipient: RECIPIENT, nativeCostWei: 1_000n, feeAsset: { kind: 'native' },
  };

  test('switch native → stablecoin re-quotes with ONE call, reusing the known gas basis', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: { recipient: RECIPIENT, asset: 'erc20', feeToken: USDC, requiredAmount: '0x2710', decimals: 6, markupX: 3 },
    });
    const next = await requoteInBandFee(base, freshChain(), SAFE, USDC);
    expect(next).not.toBeNull();
    expect(next!.feeAsset).toEqual({ kind: 'erc20', token: USDC, decimals: 6, amount: 10_000n });
    expect(next!.totalWei).toBe(0n);
    expect(next!.feeRecipient).toBe(RECIPIENT);
    // ONE bundler RPC (the re-quote) — no capability probe, no gas re-estimate pipeline.
    expect(poolBundlerCallMock).toHaveBeenCalledTimes(1);
  });

  test('asset mismatch (asked stable, got native) → null so the caller falls back', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: { recipient: RECIPIENT, asset: 'native', requiredAmount: '0x64', markupX: 3 },
    });
    expect(await requoteInBandFee(base, freshChain(), SAFE, USDC)).toBeNull();
  });

  test('a non-in-band estimate (no basis) → null (nothing to fast-path)', async () => {
    const legacy = { ...base, inBand: undefined, nativeCostWei: undefined };
    expect(await requoteInBandFee(legacy, freshChain(), SAFE, USDC)).toBeNull();
    expect(poolBundlerCallMock).not.toHaveBeenCalled();
  });
});

// ---------------------------------------------------------------------------
// buildInBandFeeLeg — the fee-leg MultiSendCall shapes
// ---------------------------------------------------------------------------

describe('buildInBandFeeLeg', () => {
  test('native leg: plain value transfer to the recipient, empty calldata', () => {
    const leg = buildInBandFeeLeg(null, RECIPIENT, 1_000_000_000_000_000n);
    expect(leg.to).toBe(RECIPIENT);
    // value is hex (MultiSendCall format — buildMultiSendExecuteCallData parses hex).
    expect(BigInt(leg.value)).toBe(1_000_000_000_000_000n);
    expect(leg.value).toBe('0x38d7ea4c68000');
    expect(leg.data).toBeInstanceOf(Uint8Array);
    expect(leg.data.length).toBe(0);
  });

  test('erc20 leg: token transfer(recipient, amount), zero value', () => {
    const leg = buildInBandFeeLeg(USDC, RECIPIENT, 10_000n);
    expect(leg.to).toBe(USDC);
    expect(BigInt('0x' + leg.value.replace(/^0x/, ''))).toBe(0n);
    // transfer(address,uint256): 4-byte selector + two 32-byte args.
    expect(leg.data.length).toBe(68);
    const hex = Array.from(leg.data).map((b) => b.toString(16).padStart(2, '0')).join('');
    expect(hex).toBe(
      'a9059cbb' +
      RECIPIENT.slice(2).toLowerCase().padStart(64, '0') +
      (10_000n).toString(16).padStart(64, '0'),
    );
  });
});
