/**
 * Generic in-band gas settlement — wallet side (vela-bundler
 * docs/inband-gas-settlement.md).
 *
 * Every Vela UserOp signs maxFeePerGas = 0 and batches
 * an in-band transfer (native value, or a whitelisted-stablecoin transfer) to
 * the bundler's recipient, sized by the bundler's own quote
 * (vela_getInBandGasQuote). Under test:
 *
 *   1. fetchInBandGasQuote — quote parse + the fail-to-null guards (error
 *      response, malformed recipient, zero amount).
 *   2. isInBandChain — every supported network is in-band without probing.
 *   3. estimateTransactionFee — the in-band branch returns the QUOTED amount
 *      (feeAsset erc20/native, maxFeePerGas 0n) and fails closed when the
 *      reimbursement quote is unavailable.
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
  fetchInBandGasQuotes,
  isInBandChain,
  _resetInBandSupportCache,
  _resetInBandQuoteCache,
} from '@/services/bundler-service';
import {
  buildInBandFeeLeg,
  calculateInBandFeeAmount,
  estimateInBandBasisGas,
  estimateTransactionFee,
  requoteInBandFee,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';

const SAFE = '0x' + 'aa'.repeat(20);
const RECIPIENT = '0x1111111111111111111111111111111111111111';
const USDC = '0x2222222222222222222222222222222222222222';

/** New address-only RPC response: every accepted asset arrives in one array. */
function inBandQuote(overrides: Record<string, unknown> = {}) {
  const asset = overrides.asset === 'erc20' ? 'erc20' : 'native';
  return {
    recipient: RECIPIENT,
    asset,
    feeToken: asset === 'erc20' ? USDC : null,
    balance: asset === 'erc20' ? '0x989680' : '0xde0b6b3a7640000',
    decimals: asset === 'erc20' ? 6 : 18,
    symbol: asset === 'erc20' ? 'USDC' : 'ETH',
    usdBalance: asset === 'erc20' ? '10' : '1',
    usdPrice: asset === 'erc20' ? '1' : '2000',
    ...overrides,
  };
}

const quoteResponse = (...quotes: Record<string, unknown>[]) => ({ result: quotes });

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
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote()));

    const q = await fetchInBandGasQuote(freshChain(), SAFE);
    expect(q).not.toBeNull();
    expect(q!.recipient).toBe(RECIPIENT);
    expect(q!.asset).toBe('native');
    expect(q!.feeToken).toBeNull();
    expect(q!.balance).toBe(1_000_000_000_000_000_000n);
    expect(q!.usdPrice).toBe('2000');

    // The new request contains only the Safe address; it returns every fee asset.
    const [method, params] = poolBundlerCallMock.mock.calls[0];
    expect(method).toBe('vela_getInBandGasQuote');
    expect(params).toEqual([{ safeAddress: SAFE }]);
  });

  test('selects the requested stablecoin from the shared response', async () => {
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote(), inBandQuote({ asset: 'erc20' })));

    const q = await fetchInBandGasQuote(freshChain(), SAFE, USDC);
    expect(q).not.toBeNull();
    expect(q!.asset).toBe('erc20');
    expect(q!.feeToken).toBe(USDC);
    expect(q!.decimals).toBe(6);
    expect(q!.balance).toBe(10_000_000n);
    expect(poolBundlerCallMock.mock.calls[0][1]).toEqual([{ safeAddress: SAFE }]);
  });

  test('an error response (chain not enabled) → null', async () => {
    poolBundlerCallMock.mockResolvedValue({
      error: { code: -32601, message: 'in-band settlement not enabled on this chain' },
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE)).resolves.toBeNull();
  });

  test('a malformed recipient → null (never batch a transfer to garbage)', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: [inBandQuote({ recipient: 'not-an-address' })],
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE)).resolves.toBeNull();
  });

  test('accepts the response without requiredAmount, which is only a server-side minimum threshold', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: [inBandQuote()],
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE)).resolves.toMatchObject({ asset: 'native' });
  });

  test('does not discard fee choices when optional usdBalance is absent', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: [inBandQuote({ usdBalance: undefined })],
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE)).resolves.toMatchObject({
      asset: 'native',
      usdBalance: '0',
    });
  });

  test('keeps a native fee quote when the network cannot provide a USD price', async () => {
    poolBundlerCallMock.mockResolvedValue({
      result: [inBandQuote({ usdPrice: null })],
    });
    await expect(fetchInBandGasQuote(freshChain(), SAFE)).resolves.toMatchObject({
      asset: 'native',
      usdPrice: null,
    });
  });

  test('a transport failure → null', async () => {
    poolBundlerCallMock.mockRejectedValue(new Error('All bundler endpoints failed'));
    await expect(fetchInBandGasQuote(freshChain(), SAFE)).resolves.toBeNull();
  });
});

describe('calculateInBandFeeAmount', () => {
  const native = { asset: 'native' as const, decimals: 18, usdPrice: '1868.70000000' };
  const usdc = { asset: 'erc20' as const, decimals: 6, usdPrice: '1' };

  test('uses gas × gas price × 3 for native, instead of the RPC requiredAmount', () => {
    expect(calculateInBandFeeAmount(200_000n, 1_000_000_000n, native, native))
      .toBe(600_000_000_000_000n);
    const nativeWithoutPrice = { asset: 'native' as const, decimals: 18, usdPrice: null };
    expect(calculateInBandFeeAmount(200_000n, 1_000_000_000n, nativeWithoutPrice, nativeWithoutPrice))
      .toBe(600_000_000_000_000n);
    // A stablecoin conversion without an oracle price is unsafe; native payment remains the
    // usable default until both conversion prices are available.
    expect(calculateInBandFeeAmount(200_000n, 1_000_000_000n, usdc, nativeWithoutPrice))
      .toBeNull();
  });

  test('converts the native cost to the stablecoin and applies the $0.01 floor', () => {
    // 0.0006 ETH × $1868.70 = $1.12122 = 1.121220 USDC.
    expect(calculateInBandFeeAmount(200_000n, 1_000_000_000n, usdc, native))
      .toBe(1_121_220n);
    // 1 wei × 3 is below the 0.00001 ETH floor; $0.018687 exceeds $0.01.
    expect(calculateInBandFeeAmount(1n, 1n, usdc, native))
      .toBe(18_687n);
    // If the native floor converts below one cent, stablecoin payment still floors at $0.01.
    const lowPriceNative = { asset: 'native' as const, decimals: 18, usdPrice: '100' };
    expect(calculateInBandFeeAmount(1n, 1n, usdc, lowPriceNative))
      .toBe(10_000n);
  });
});

// ---------------------------------------------------------------------------
// isInBandChain
// ---------------------------------------------------------------------------

describe('isInBandChain', () => {
  test('every chain is in-band, including when a quote endpoint is unavailable', async () => {
    poolBundlerCallMock.mockResolvedValue({ error: { code: -32601, message: 'not enabled' } });
    await expect(isInBandChain(4217, SAFE)).resolves.toBe(true);
    await expect(isInBandChain(42431, SAFE)).resolves.toBe(true);
    await expect(isInBandChain(freshChain(), SAFE)).resolves.toBe(true);
    expect(poolBundlerCallMock).not.toHaveBeenCalled();
  });

  test('native and stable quote lookups coalesce into one all-asset request', async () => {
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote(), inBandQuote({ asset: 'erc20' })));
    const chain = freshChain();
    const [all, stable] = await Promise.all([
      fetchInBandGasQuotes(chain, SAFE),
      fetchInBandGasQuote(chain, SAFE, USDC),
    ]);
    expect(all).toHaveLength(2);
    expect(stable?.feeToken).toBe(USDC);
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

describe('estimateTransactionFee — in-band branch', () => {
  test('fails instead of estimating with a fabricated nonce', async () => {
    routeRpc();
    const baseRoute = rpcCallMock.getMockImplementation()!;
    rpcCallMock.mockImplementation((method: string, ...args: any[]) => (
      method === 'eth_call'
        ? Promise.reject(new Error('nonce read failed'))
        : baseRoute(method, ...args)
    ));

    await expect(estimateTransactionFee(SAFE, freshChain(), 'standard'))
      .rejects.toThrow('nonce read failed');
    expect(rpcCallMock.mock.calls.some(([method]) => method === 'eth_estimateUserOperationGas')).toBe(false);
  });

  test('simulates every deployed Safe with its real nonce, reimbursement leg, and zero EntryPoint fee fields', async () => {
    const userOps: Record<string, string>[] = [];
    const nonce = '0x' + '00'.repeat(31) + '07';
    rpcCallMock.mockImplementation((method: string, params: any[]) => {
      switch (method) {
        case 'pimlico_getUserOperationGasPrice':
          return Promise.resolve({ result: { slow: HEALTHY_QUOTE, standard: HEALTHY_QUOTE, fast: HEALTHY_QUOTE } });
        case 'eth_gasPrice':
          return Promise.resolve({ result: '0x3b9aca00' });
        case 'eth_getBlockByNumber':
          return Promise.resolve({ result: { baseFeePerGas: '0x3b9aca00' } });
        case 'eth_maxPriorityFeePerGas':
          return Promise.resolve({ result: '0x3b9aca00' });
        case 'eth_getCode':
          return Promise.resolve({ result: '0x6080' });
        case 'eth_call':
          return Promise.resolve({ result: nonce });
        case 'eth_estimateUserOperationGas':
          userOps.push(params[0]);
          return Promise.resolve({ result: {
            verificationGasLimit: '0x186a0', callGasLimit: '0xc350', preVerificationGas: '0x4e20',
          }});
        default:
          return Promise.reject(new Error(`unmocked method ${method}`));
      }
    });
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote()));

    await estimateTransactionFee(SAFE, freshChain(), 'standard');

    expect(userOps).toHaveLength(1);
    expect(userOps[0]).toMatchObject({
      sender: SAFE,
      nonce,
      maxFeePerGas: '0x0',
      maxPriorityFeePerGas: '0x0',
    });
    // The no-transaction preview carries the same MultiSend reimbursement shape as submit.
    expect(userOps[0].callData).toContain(RECIPIENT.slice(2));
  });

  test('simulates an in-band Safe with the same zero EntryPoint fee fields it will submit', async () => {
    const userOps: Record<string, string>[] = [];
    const nonce = '0x' + '00'.repeat(31) + '08';
    rpcCallMock.mockImplementation((method: string, params: any[]) => {
      switch (method) {
        case 'pimlico_getUserOperationGasPrice':
          return Promise.resolve({ result: { slow: HEALTHY_QUOTE, standard: HEALTHY_QUOTE, fast: HEALTHY_QUOTE } });
        case 'eth_gasPrice':
        case 'eth_maxPriorityFeePerGas':
          return Promise.resolve({ result: '0x3b9aca00' });
        case 'eth_getBlockByNumber':
          return Promise.resolve({ result: { baseFeePerGas: '0x3b9aca00' } });
        case 'eth_getCode':
          return Promise.resolve({ result: '0x6080' });
        case 'eth_call':
          return Promise.resolve({ result: nonce });
        case 'eth_estimateUserOperationGas':
          userOps.push(params[0]);
          return Promise.resolve({ result: {
            verificationGasLimit: '0x186a0', callGasLimit: '0xc350', preVerificationGas: '0x4e20',
          }});
        default:
          return Promise.reject(new Error(`unmocked method ${method}`));
      }
    });
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote()));

    await estimateTransactionFee(SAFE, freshChain(), 'standard');

    expect(userOps).toHaveLength(1);
    expect(userOps[0]).toMatchObject({
      sender: SAFE,
      nonce,
      maxFeePerGas: '0x0',
      maxPriorityFeePerGas: '0x0',
    });
    // In-band submit always uses MultiSend (user calls + reimbursement). The
    // estimate must use that same outer calldata and real fee recipient, not
    // a direct Safe self-call that can fail before execution is simulated.
    expect(userOps[0].callData).toContain(RECIPIENT.slice(2));
  });

  test('simulates an undeployed Safe with the same initCode it will submit', async () => {
    const userOps: Record<string, string>[] = [];
    rpcCallMock.mockImplementation((method: string, params: any[]) => {
      switch (method) {
        case 'pimlico_getUserOperationGasPrice':
          return Promise.resolve({ result: { slow: HEALTHY_QUOTE, standard: HEALTHY_QUOTE, fast: HEALTHY_QUOTE } });
        case 'eth_gasPrice':
          return Promise.resolve({ result: '0x3b9aca00' });
        case 'eth_getBlockByNumber':
          return Promise.resolve({ result: { baseFeePerGas: '0x3b9aca00' } });
        case 'eth_maxPriorityFeePerGas':
          return Promise.resolve({ result: '0x3b9aca00' });
        case 'eth_getCode':
          return Promise.resolve({ result: '0x' });
        case 'eth_call':
          return Promise.resolve({ result: '0x' + '00'.repeat(32) });
        case 'eth_estimateUserOperationGas':
          userOps.push(params[0]);
          return Promise.resolve({ result: {
            verificationGasLimit: '0x186a0', callGasLimit: '0xc350', preVerificationGas: '0x4e20',
          }});
        default:
          return Promise.reject(new Error(`unmocked method ${method}`));
      }
    });
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote()));

    await estimateTransactionFee(
      SAFE, freshChain(), 'standard', undefined, undefined, undefined,
      '04' + '11'.repeat(64),
    );

    expect(userOps).toHaveLength(1);
    expect(userOps[0]).toMatchObject({
      sender: SAFE,
      nonce: '0x0',
      maxFeePerGas: '0x0',
      maxPriorityFeePerGas: '0x0',
    });
    expect(userOps[0].factory).toMatch(/^0x[0-9a-f]+$/i);
    expect(userOps[0].factoryData).toMatch(/^0x[0-9a-f]+$/i);
  });

  test('stablecoin option → feeAsset erc20 with the gas-derived amount, maxFee 0', async () => {
    routeRpc();
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote(), inBandQuote({ asset: 'erc20' })));

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard', undefined, undefined, USDC);
    expect(est.inBand).toBe(true);
    expect(est.maxFeePerGas).toBe(0n); // the op signs maxFee = 0
    expect(est.totalWei).toBe(0n); // native display not applicable
    expect(est.feeAsset).toEqual({ kind: 'erc20', token: USDC, decimals: 6, amount: 36_000_000n });
    // Compatibility fields stay populated off the bundler price quote.
    expect(est.networkFeePerGas).toBe(10_000_000_000n);
    expect(est.totalGas).toBeGreaterThan(0n);
  });

  test('native option → totalWei is the gas-derived amount, feeAsset native', async () => {
    routeRpc();
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote()));

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard');
    expect(est.inBand).toBe(true);
    expect(est.maxFeePerGas).toBe(0n);
    expect(est.totalWei).toBe(18_000_000_000_000_000n);
    expect(est.feeAsset).toEqual({ kind: 'native' });
    // Sign-what-displayed: the estimate carries the recipient from this exact quote.
    expect(est.feeRecipient).toBe(RECIPIENT);
  });

  test('native gas estimation succeeds when the relay omits the native USD price', async () => {
    routeRpc();
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote({ usdPrice: null })));

    const est = await estimateTransactionFee(SAFE, freshChain(), 'standard');
    expect(est.feeAsset).toEqual({ kind: 'native' });
    expect(est.totalWei).toBeGreaterThan(0n);
  });

  test('an unavailable all-asset quote fails closed instead of constructing a legacy UserOp', async () => {
    routeRpc();
    poolBundlerCallMock.mockResolvedValue({ error: { code: -32000, message: 'rate unavailable' } });

    await expect(estimateTransactionFee(SAFE, freshChain(), 'standard'))
      .rejects.toThrow('Could not load the in-band gas quote');
    expect(rpcCallMock.mock.calls.some(([method]) => method === 'eth_estimateUserOperationGas')).toBe(false);
  });

  test('a stale "not enabled" response fails closed rather than selecting legacy gas fees', async () => {
    routeRpc();
    poolBundlerCallMock.mockResolvedValue({ error: { code: -32601, message: 'not enabled' } });

    await expect(estimateTransactionFee(SAFE, freshChain(), 'standard'))
      .rejects.toThrow('Could not load the in-band gas quote');
    expect(poolBundlerCallMock).toHaveBeenCalledTimes(1);
  });

  test('requested stablecoin but the bundler quotes a different asset fails closed', async () => {
    routeRpc();
    // The all-asset response does not contain the requested stable — the wallet must NOT
    // display a fee in an asset the send path would refuse to pay.
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote()));

    await expect(estimateTransactionFee(SAFE, freshChain(), 'standard', undefined, undefined, USDC))
      .rejects.toThrow('cannot quote the selected fee token');
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

  test('rejects when the bundler estimate is unavailable', async () => {
    rpcCallMock.mockImplementation((method: string) => {
      if (method === 'eth_call') return Promise.resolve({ result: '0x' + '00'.repeat(32) });
      if (method === 'eth_estimateUserOperationGas') return Promise.reject(new Error('estimation unavailable'));
      return Promise.reject(new Error(`unmocked ${method}`));
    });
    await expect(estimateInBandBasisGas(SAFE, [NATIVE_LEG], null, freshChain()))
      .rejects.toThrow('estimation unavailable');
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
    inBand: true, feeRecipient: RECIPIENT, feeAsset: { kind: 'native' },
  };

  test('switch native → stablecoin selects it from one all-asset quote response', async () => {
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote(), inBandQuote({ asset: 'erc20' })));
    const next = await requoteInBandFee(base, freshChain(), SAFE, USDC);
    expect(next).not.toBeNull();
    expect(next!.feeAsset).toEqual({ kind: 'erc20', token: USDC, decimals: 6, amount: 20_000n });
    expect(next!.totalWei).toBe(0n);
    expect(next!.feeRecipient).toBe(RECIPIENT);
    // One bundler RPC — no capability probe, gas re-estimate, or per-token query.
    expect(poolBundlerCallMock).toHaveBeenCalledTimes(1);
  });

  test('asset mismatch (asked stable, got native) → null so the caller falls back', async () => {
    poolBundlerCallMock.mockResolvedValue(quoteResponse(inBandQuote()));
    expect(await requoteInBandFee(base, freshChain(), SAFE, USDC)).toBeNull();
  });

  test('a non-in-band estimate → null (nothing to fast-path)', async () => {
    const legacy = { ...base, inBand: undefined };
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
