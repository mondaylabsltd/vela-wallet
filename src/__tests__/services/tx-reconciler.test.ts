/**
 * Tests for the pending-transaction reconciler: a pending submission whose
 * receipt landed while the app was closed must converge to confirmed/failed on
 * next run; a transient/null result must leave it pending (never lost, never
 * faked).
 */

jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));

const loadTransactions = jest.fn((..._a: any[]) => Promise.resolve([] as any[]));
const updateTransaction = jest.fn((..._a: any[]) => Promise.resolve());
jest.mock('@/services/storage', () => ({
  loadTransactions: (...a: any[]) => loadTransactions(...a),
  updateTransaction: (...a: any[]) => updateTransaction(...a),
}));

const rpcCall = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({ rpcCall: (...a: any[]) => rpcCall(...a) }));

import { reconcilePendingTransactions } from '@/services/tx-reconciler';

const ADDR = '0xWallet';
function pendingTx(over: Partial<any> = {}) {
  return {
    id: 'op1',
    userOpHash: '0xop1',
    txHash: '',
    from: ADDR,
    to: '0xdest',
    value: '1',
    symbol: 'ETH',
    decimals: 18,
    chainId: 1,
    timestamp: Math.floor(Date.now() / 1000),
    status: 'pending',
    type: 'send',
    ...over,
  };
}

describe('reconcilePendingTransactions', () => {
  beforeEach(() => {
    loadTransactions.mockReset();
    updateTransaction.mockReset().mockResolvedValue(undefined);
    rpcCall.mockReset();
    // Defeat the internal throttle between tests by waiting out / mocking time is
    // overkill — instead each test uses a distinct flow and we rely on the first
    // call per test having a fresh-enough window. Add a tiny delay helper:
  });

  test('confirms a pending submission once the receipt is available', async () => {
    loadTransactions.mockResolvedValue([pendingTx()]);
    rpcCall.mockResolvedValue({ result: { success: true, receipt: { transactionHash: '0xLANDED' } } });
    const n = await reconcilePendingTransactions(ADDR);
    expect(n).toBe(1);
    expect(updateTransaction).toHaveBeenCalledWith('op1', { status: 'confirmed', txHash: '0xLANDED' });
  });

  test('marks failed when the receipt reports success=false', async () => {
    // bypass throttle by advancing the internal clock via a unique address isn't
    // possible; use fake timers to move past MIN_INTERVAL.
    jest.useFakeTimers({ now: Date.now() + 60_000 });
    loadTransactions.mockResolvedValue([pendingTx({ id: 'op2', userOpHash: '0xop2' })]);
    rpcCall.mockResolvedValue({ result: { success: false, receipt: { transactionHash: '0xDROPPED' } } });
    const n = await reconcilePendingTransactions(ADDR);
    jest.useRealTimers();
    expect(n).toBe(1);
    expect(updateTransaction).toHaveBeenCalledWith('op2', { status: 'failed' });
  });

  test('leaves the record pending on a null/transient result (never faked)', async () => {
    jest.useFakeTimers({ now: Date.now() + 120_000 });
    loadTransactions.mockResolvedValue([pendingTx({ id: 'op3', userOpHash: '0xop3' })]);
    rpcCall.mockResolvedValue({ result: null });
    const n = await reconcilePendingTransactions(ADDR);
    jest.useRealTimers();
    expect(n).toBe(0);
    expect(updateTransaction).not.toHaveBeenCalled();
  });

  test('ignores records without a userOpHash or already-confirmed', async () => {
    jest.useFakeTimers({ now: Date.now() + 180_000 });
    loadTransactions.mockResolvedValue([
      pendingTx({ id: 'noHash', userOpHash: '' }),
      pendingTx({ id: 'confirmed', txHash: '0xabc', status: 'confirmed' }),
    ]);
    const n = await reconcilePendingTransactions(ADDR);
    jest.useRealTimers();
    expect(n).toBe(0);
    expect(rpcCall).not.toHaveBeenCalled();
  });
});
