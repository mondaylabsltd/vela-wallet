/**
 * Tempo's UI-facing fee estimate is an ordinary ERC-20 fee asset. The only Tempo
 * exception belongs below this boundary, in the 0x76 submission adapter.
 */

jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/services/storage', () => ({ loadServiceEndpoints: jest.fn(async () => ({})) }));

const rpcCallMock = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({ rpcCall: (...args: any[]) => rpcCallMock(...args) }));

jest.mock('@/services/rpc-pool', () => ({
  getActiveBundlerBaseUrl: jest.fn(async () => 'https://relay.test'),
  getChainRpcUrl: jest.fn(async () => null),
  isUsingBuiltinBundler: jest.fn(async () => true),
  poolBundlerCall: jest.fn(),
  poolRpcCall: jest.fn(),
  getBuiltinBundlerUrl: jest.fn(() => 'https://relay.test'),
}));

import { estimateTransactionFee } from '@/services/safe-transaction';
import { TEMPO_DEFAULT_FEE_TOKEN, tempoReimbursement } from '@/services/tempo';

const SAFE = '0x' + 'aa'.repeat(20);
const RECIPIENT = '0x' + 'bb'.repeat(20);
const CUSTOM_USD = '0x' + 'cc'.repeat(20);
const originalFetch = global.fetch;

beforeEach(() => {
  rpcCallMock.mockReset();
  rpcCallMock.mockImplementation((method: string) => {
    switch (method) {
      case 'eth_getCode': return Promise.resolve({ result: '0x6080' });
      case 'eth_gasPrice': return Promise.resolve({ result: '0x4a817c800' }); // 20 gwei attodollars
      case 'eth_getBlockByNumber': return Promise.resolve({ result: { baseFeePerGas: '0x4a817c800' } });
      default: return Promise.reject(new Error(`unmocked ${method}`));
    }
  });
  global.fetch = jest.fn(async () => ({
    ok: true,
    json: async () => ({ settlementRecipient: RECIPIENT }),
  })) as unknown as typeof fetch;
});

afterAll(() => { global.fetch = originalFetch; });

test('Tempo exposes its default fee as a normal pathUSD ERC-20 estimate', async () => {
  const fee = await estimateTransactionFee(SAFE, 4217, 'fast');

  expect(fee.inBand).toBe(true);
  expect(fee.feeAsset).toEqual({
    kind: 'erc20',
    token: TEMPO_DEFAULT_FEE_TOKEN,
    decimals: 6,
    amount: tempoReimbursement(fee.totalGas, fee.networkFeePerGas, 6),
    symbol: 'pathUSD',
  });
  expect(fee.feeRecipient).toBe(RECIPIENT);
});

test('a relay-published Tempo fee token flows through the same ERC-20 estimate contract', async () => {
  const fee = await estimateTransactionFee(SAFE, 4217, 'fast', undefined, undefined, CUSTOM_USD);

  expect(fee.feeAsset).toMatchObject({
    kind: 'erc20', token: CUSTOM_USD, decimals: 6,
    amount: tempoReimbursement(fee.totalGas, fee.networkFeePerGas, 6),
  });
});

test('Tempo batch estimates include every transfer rather than a single-call proxy', async () => {
  const single = await estimateTransactionFee(SAFE, 4217, 'fast');
  const batch = await estimateTransactionFee(SAFE, 4217, 'fast', undefined, [
    { to: RECIPIENT, value: '0x0', data: '0x' },
    { to: CUSTOM_USD, value: '0x0', data: '0x' },
  ]);

  expect(batch.totalGas).toBeGreaterThan(single.totalGas);
  expect(batch.feeAsset?.kind).toBe('erc20');
});
