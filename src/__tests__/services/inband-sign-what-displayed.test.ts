/**
 * sendUserOpInBand — "sign what was displayed" (vela-bundler in-band gas).
 *
 * The confirm slide's fee is signed VERBATIM: when a quotedFee {amount, recipient} is threaded
 * through, the send path must NOT re-quote at submit (no fresh vela_getInBandGasQuote) — the
 * user signs exactly the number they saw. Only a programmatic caller with no quotedFee fetches
 * a send-time quote. And when the user picked a stablecoin but the send-time quote comes back
 * native, the send must REFUSE (never silently pay a different asset).
 *
 * We drive sendNative on an in-band chain with a signFn that throws a sentinel: reaching it
 * proves the pre-sign flow succeeded; whether the address-only quote is reused by the send path
 * is the observable that pins the branch. Full signing/submit is exercised
 * end-to-end by integration; here we pin the money-relevant decision cheaply.
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
const USDC = '0x' + '22'.repeat(20);
const PUBKEY = '0x' + '04'.repeat(33);

let nextChain = 970_000;
const freshChain = () => ++nextChain;

const SIGN_SENTINEL = 'SIGN_REACHED';
const throwingSignFn = jest.fn(async () => { throw new Error(SIGN_SENTINEL); });

/** Route the RPC surface sendUserOpInBand touches before signing. */
function routeRpc() {
  rpcCallMock.mockImplementation((method: string) => {
    switch (method) {
      case 'eth_getCode': return Promise.resolve({ result: '0x6080' }); // EntryPoint + Safe deployed
      case 'eth_call': return Promise.resolve({ result: '0x' + '00'.repeat(31) + '07' }); // getNonce = 7
      case 'eth_gasPrice': return Promise.resolve({ result: '0x3b9aca00' });
      case 'eth_getBlockByNumber': return Promise.resolve({ result: { baseFeePerGas: '0x3b9aca00' } });
      case 'eth_maxPriorityFeePerGas': return Promise.resolve({ result: '0x3b9aca00' });
      case 'pimlico_getUserOperationGasPrice':
        return Promise.resolve({ result: { fast: { maxFeePerGas: '0x77359400', maxPriorityFeePerGas: '0x3b9aca00', networkFeePerGas: '0x3b9aca00', relayerFeePerGas: '0x3b9aca00' } } });
      case 'eth_estimateUserOperationGas':
        return Promise.reject(new Error('estimation unavailable')); // static-gas fallback is fine
      default:
        return Promise.reject(new Error(`unmocked ${method}`));
    }
  });
}

/** New address-only quote response: native plus every accepted stablecoin. */
function quoteImpl(sizing: { asset: 'native' | 'erc20'; feeToken?: string }) {
  return async () => {
    const native = {
      recipient: TREASURY, asset: 'native', feeToken: null, balance: '0x100', decimals: 18,
      symbol: 'ETH', usdBalance: '1', usdPrice: '2000',
    };
    if (sizing.asset === 'erc20') {
      return { result: [native, {
        recipient: TREASURY, asset: 'erc20', feeToken: sizing.feeToken, balance: '0x989680', decimals: 6,
        symbol: 'USDC', usdBalance: '10', usdPrice: '1',
      }] };
    }
    return { result: [native] };
  };
}

beforeEach(() => {
  rpcCallMock.mockReset();
  poolBundlerCallMock.mockReset();
  throwingSignFn.mockClear();
});

const quoteCalls = () => poolBundlerCallMock.mock.calls.length;

test('quotedFee present → signs it VERBATIM and does not repeat the capability quote', async () => {
  routeRpc();
  poolBundlerCallMock.mockImplementation(quoteImpl({ asset: 'native' }));

  const quotedFee = { amount: 1_000_000_000_000_000n, recipient: TREASURY };
  await expect(
    sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn, undefined, null, quotedFee),
  ).rejects.toThrow(SIGN_SENTINEL); // reached signing = the pre-sign flow completed
  // Universal in-band mode does not need a capability probe. The displayed
  // quote is already the complete reimbursement instruction, so it is signed
  // without a second quote request.
  expect(quoteCalls()).toBe(0);
  expect(throwingSignFn).toHaveBeenCalledTimes(1);
});

test('no quotedFee (programmatic caller) reuses the capability quote as its sizing quote', async () => {
  routeRpc();
  poolBundlerCallMock.mockImplementation(quoteImpl({ asset: 'native' }));

  await expect(
    sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn),
  ).rejects.toThrow(SIGN_SENTINEL);
  expect(quoteCalls()).toBe(1);
});

test('stablecoin requested but the send-time quote is native → REFUSE before signing', async () => {
  routeRpc();
  poolBundlerCallMock.mockImplementation(quoteImpl({ asset: 'native' })); // ignores the USDC request

  await expect(
    sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn, undefined, USDC),
  ).rejects.toThrow(/fee token/i); // the asset-mismatch guard, NOT the sign sentinel
  expect(throwingSignFn).not.toHaveBeenCalled(); // never reached signing
});

test('stablecoin requested and the quote matches → signs (reaches signing)', async () => {
  routeRpc();
  poolBundlerCallMock.mockImplementation(quoteImpl({ asset: 'erc20', feeToken: USDC }));

  await expect(
    sendNative(SAFE, TO, '0x64', freshChain(), PUBKEY, throwingSignFn, undefined, USDC),
  ).rejects.toThrow(SIGN_SENTINEL);
  expect(throwingSignFn).toHaveBeenCalledTimes(1);
});
