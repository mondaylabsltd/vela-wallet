/**
 * Regression: eth_getCode failure must NOT be guessed as "deployed" (AA20 root cause).
 *
 * A fresh smart account's first send builds `initCode = deployed ? empty : buildInitCode()`.
 * The deployment flag came from isDeployed(), which used to `return true` on ANY RPC error or
 * thrown call. That shipped a UserOp with EMPTY initCode for an account with no on-chain code
 * → the bundler's simulateValidation reverts with "AA20 account not deployed", and every new
 * user's first transfer fails. The fix: on an INDETERMINATE eth_getCode result, isDeployed
 * fails fast (throws a retryable error) instead of guessing — so the send never signs/submits a
 * doomed op. A DEFINITIVE '0x' (undeployed) still correctly attaches initCode.
 *
 * isDeployed is private, so we drive the exported sendNative end-to-end (→ sendUserOpInBand) and
 * observe the op the way the bundler sees it: userOpToDict emits `factory`/`factoryData` iff the
 * op carries initCode. A throwing signFn sentinel proves whether the pre-sign flow completed.
 */

jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/modules/passkey', () => ({}));
jest.mock('@/services/storage', () => ({ loadServiceEndpoints: jest.fn(async () => ({})) }));
jest.mock('@/services/tempo', () => ({
  isTempoChain: () => false,
  isTempoFeeToken: () => false,
  TEMPO_DEFAULT_FEE_TOKEN: '0x20c0000000000000000000000000000000000000',
  TEMPO_FEE_TOKEN_DECIMALS: 6,
}));

const rpcCallMock = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({ rpcCall: (...a: any[]) => rpcCallMock(...a) }));

const poolBundlerCallMock = jest.fn();
jest.mock('@/services/rpc-pool', () => ({
  getActiveBundlerBaseUrl: jest.fn(async () => 'https://bundler.test'),
  getChainRpcUrl: jest.fn(async () => null),
  isUsingBuiltinBundler: jest.fn(async () => true),
  poolRpcCall: jest.fn(),
  poolBundlerCall: (...a: any[]) => poolBundlerCallMock(...a),
  getBuiltinBundlerUrl: jest.fn(() => 'https://bundler.test'),
}));

import { sendNative } from '@/services/safe-transaction';

const SAFE = '0x' + 'aa'.repeat(20);
const TO = '0x' + 'bb'.repeat(20);
const TREASURY = '0x' + 'cc'.repeat(20);
// Well-formed uncompressed P-256 key (0x04 ++ 32-byte x ++ 32-byte y) so buildInitCode
// produces a real (factory ++ createProxyWithNonce) deploy for the undeployed case.
const PUBKEY = '0x04' + '11'.repeat(32) + '22'.repeat(32);

let nextChain = 990_000;
const freshChain = () => ++nextChain; // dodge the module-level per-chain caches

const SIGN_SENTINEL = 'SIGN_REACHED';
const throwingSignFn = jest.fn(async () => { throw new Error(SIGN_SENTINEL); });

/** The address-only all-asset quote — native, so sendNative routes in-band. */
function nativeQuote() {
  poolBundlerCallMock.mockResolvedValue({
    result: [{
      recipient: TREASURY, asset: 'native', feeToken: null, balance: '0x100', decimals: 18,
      symbol: 'ETH', usdBalance: '1', usdPrice: '2000',
    }],
  });
}

/**
 * Route every RPC sendUserOpInBand touches. `getCode` is a function so each test decides how
 * eth_getCode(user) behaves while EntryPoint (verifyChainReady) stays healthy.
 */
function routeRpc(getCode: (address: string) => Promise<any>) {
  rpcCallMock.mockImplementation((method: string, params: any[]) => {
    switch (method) {
      case 'eth_getCode': return getCode((params?.[0] ?? '').toLowerCase());
      case 'eth_call': return Promise.resolve({ result: '0x' + '00'.repeat(31) + '07' }); // getNonce = 7
      case 'eth_gasPrice': return Promise.resolve({ result: '0x3b9aca00' });
      case 'eth_getBlockByNumber': return Promise.resolve({ result: { baseFeePerGas: '0x3b9aca00' } });
      case 'eth_maxPriorityFeePerGas': return Promise.resolve({ result: '0x3b9aca00' });
      case 'pimlico_getUserOperationGasPrice':
        return Promise.resolve({ result: { fast: { maxFeePerGas: '0x77359400', maxPriorityFeePerGas: '0x3b9aca00', networkFeePerGas: '0x3b9aca00', relayerFeePerGas: '0x3b9aca00' } } });
      case 'eth_estimateUserOperationGas':
        return Promise.resolve({ result: { verificationGasLimit: '0x30d40', callGasLimit: '0x186a0', preVerificationGas: '0x5208' } });
      case 'eth_sendUserOperation':
        return Promise.resolve({ result: '0x' + 'ee'.repeat(32) });
      default:
        return Promise.reject(new Error(`unmocked ${method}`));
    }
  });
}

/** The op dict as the bundler receives it (captured off the estimate/submit call). */
function submittedOp(): any {
  const call =
    rpcCallMock.mock.calls.find((c) => c[0] === 'eth_estimateUserOperationGas') ??
    rpcCallMock.mock.calls.find((c) => c[0] === 'eth_sendUserOperation');
  return call?.[1]?.[0];
}

beforeEach(() => {
  rpcCallMock.mockReset();
  poolBundlerCallMock.mockReset();
  throwingSignFn.mockClear();
});

test('AA20 core: eth_getCode THROWS on a fresh account → fail-fast, never signs an empty-initCode op', async () => {
  nativeQuote();
  // EntryPoint reads as deployed; only the USER account read fails (isolate the isDeployed(user) path).
  routeRpc((address) =>
    address === SAFE.toLowerCase()
      ? Promise.reject(new Error('RPC down'))
      : Promise.resolve({ result: '0x6080' }),
  );

  await expect(sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn))
    .rejects.toThrow(/try again|unstable|network/i);

  // The old bug: isDeployed returns true → deployed op signed & shipped with no initCode.
  expect(throwingSignFn).not.toHaveBeenCalled();
  expect(rpcCallMock.mock.calls.some((c) => c[0] === 'eth_sendUserOperation')).toBe(false);
});

test('AA20 other branch: eth_getCode returns a JSON-RPC {error} body → fail-fast, no signing', async () => {
  nativeQuote();
  routeRpc((address) =>
    address === SAFE.toLowerCase()
      ? Promise.resolve({ error: { code: -32000, message: 'header not found' } })
      : Promise.resolve({ result: '0x6080' }),
  );

  await expect(sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn))
    .rejects.toThrow(/try again|unstable|network/i);
  expect(throwingSignFn).not.toHaveBeenCalled();
});

test('positive proof: a genuinely undeployed account (eth_getCode "0x") STILL ships initCode', async () => {
  nativeQuote();
  routeRpc((address) =>
    address === SAFE.toLowerCase()
      ? Promise.resolve({ result: '0x' })            // definitively NOT deployed
      : Promise.resolve({ result: '0x6080' }),       // EntryPoint deployed
  );

  await expect(sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn))
    .rejects.toThrow(SIGN_SENTINEL); // reached signing = pre-sign flow completed

  const op = submittedOp();
  expect(op).toBeDefined();
  // v0.7 dict carries the deploy: factory = 20-byte SafeProxyFactory, factoryData non-empty.
  expect(op.factory).toBeDefined();
  expect(op.factory).toMatch(/^0x[0-9a-fA-F]{40}$/);
  expect(op.factoryData && op.factoryData.length).toBeGreaterThan(2);
  expect(op.nonce).toBe('0x0'); // undeployed nonce is 0
});

test('regression: a DEPLOYED account ships NO initCode and signs its real nonce (no AA10)', async () => {
  nativeQuote();
  routeRpc(() => Promise.resolve({ result: '0x6080' })); // user + EntryPoint both deployed

  await expect(sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn))
    .rejects.toThrow(SIGN_SENTINEL);

  const op = submittedOp();
  expect(op).toBeDefined();
  expect(op.factory).toBeUndefined();     // attaching initCode here would be AA10
  expect(op.factoryData).toBeUndefined();
  expect(BigInt(op.nonce)).toBe(7n);      // the real fetched nonce, not a 0x0 fallback
});

test('sibling AA25: eth_call (getNonce) errors for a DEPLOYED wallet → fail-fast, no nonce-0 op', async () => {
  nativeQuote();
  rpcCallMock.mockImplementation((method: string) => {
    switch (method) {
      case 'eth_getCode': return Promise.resolve({ result: '0x6080' });                 // deployed
      case 'eth_call': return Promise.resolve({ error: { code: -32000, message: 'exec' } }); // getNonce fails
      case 'eth_gasPrice': return Promise.resolve({ result: '0x3b9aca00' });
      case 'eth_getBlockByNumber': return Promise.resolve({ result: { baseFeePerGas: '0x3b9aca00' } });
      case 'eth_maxPriorityFeePerGas': return Promise.resolve({ result: '0x3b9aca00' });
      default: return Promise.reject(new Error(`unmocked ${method}`));
    }
  });

  await expect(sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn))
    .rejects.toThrow(/nonce|try again|unstable/i);
  expect(throwingSignFn).not.toHaveBeenCalled(); // never sign a nonce-0 op the bundler rejects as AA25
});
