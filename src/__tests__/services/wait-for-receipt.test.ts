/**
 * Tests for waitForReceipt's resilience: it must keep polling through transient
 * bundler blips, distinguish "unreachable / unknown" from "submitted but not
 * confirmed", honour an abort signal, and surface a genuine drop.
 */

jest.mock('react-native', () => ({}));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));

const rpcCall = jest.fn();
jest.mock('@/services/rpc-adapter', () => ({
  rpcCall: (...args: any[]) => rpcCall(...args),
}));

import { waitForReceipt } from '@/services/safe-transaction';

describe('waitForReceipt', () => {
  beforeEach(() => rpcCall.mockReset());
  afterEach(() => jest.useRealTimers());

  test('returns the tx hash on the first successful poll', async () => {
    rpcCall.mockResolvedValueOnce({ result: { success: true, receipt: { transactionHash: '0xabc' } } });
    await expect(waitForReceipt('0xhash', 1)).resolves.toBe('0xabc');
  });

  test('throws a "dropped" error when the bundler reports success=false', async () => {
    rpcCall.mockResolvedValueOnce({ result: { success: false, receipt: { transactionHash: '0xdead' } } });
    await expect(waitForReceipt('0xhash', 1)).rejects.toThrow(/dropped from the network/i);
  });

  test('rejects immediately when the abort signal is already aborted (no poll)', async () => {
    const c = new AbortController();
    c.abort();
    await expect(waitForReceipt('0xhash', 1, 120_000, c.signal)).rejects.toMatchObject({ name: 'AbortError' });
    expect(rpcCall).not.toHaveBeenCalled();
  });

  test('keeps polling through a transient bundler error, then succeeds', async () => {
    jest.useFakeTimers();
    rpcCall
      .mockRejectedValueOnce(new Error('All bundler endpoints failed'))
      .mockResolvedValueOnce({ result: { success: true, receipt: { transactionHash: '0xok' } } });
    const p = waitForReceipt('0xhash', 1, 30_000);
    const assertion = expect(p).resolves.toBe('0xok');
    await jest.advanceTimersByTimeAsync(3_000);
    await assertion;
    expect(rpcCall).toHaveBeenCalledTimes(2);
  });

  test('final error says status is UNKNOWN when the bundler was never reachable', async () => {
    jest.useFakeTimers();
    rpcCall.mockRejectedValue(new Error('All bundler endpoints failed'));
    const p = waitForReceipt('0xhash', 1, 3_000);
    const assertion = expect(p).rejects.toThrow(/unknown|reach the bundler/i);
    await jest.advanceTimersByTimeAsync(10_000);
    await assertion;
  });

  test('final error says "not confirmed" when the bundler answered but no receipt landed', async () => {
    jest.useFakeTimers();
    rpcCall.mockResolvedValue({ result: null }); // clean response, not ready yet
    const p = waitForReceipt('0xhash', 1, 3_000);
    const assertion = expect(p).rejects.toThrow(/not confirmed within/i);
    await jest.advanceTimersByTimeAsync(10_000);
    await assertion;
  });
});
