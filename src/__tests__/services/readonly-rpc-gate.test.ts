/**
 * Tests for the read-only RPC gate: in-flight dedupe + concurrency cap.
 */
import { gateReadOnly, readOnlyKey, __resetReadOnlyGate } from '@/services/readonly-rpc-gate';

function deferred<T>() {
  let resolve!: (v: T) => void;
  let reject!: (e: any) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const flush = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  __resetReadOnlyGate();
});

describe('gateReadOnly — dedupe', () => {
  test('collapses identical in-flight keys into one execution', async () => {
    const task = jest.fn().mockResolvedValue('R');
    const a = gateReadOnly('k', task);
    const b = gateReadOnly('k', task);
    expect(await a).toBe('R');
    expect(await b).toBe('R');
    expect(task).toHaveBeenCalledTimes(1);
  });

  test('runs again after the prior call settles (no cross-time caching)', async () => {
    const task = jest.fn().mockResolvedValueOnce('R1').mockResolvedValueOnce('R2');
    expect(await gateReadOnly('k', task)).toBe('R1');
    expect(await gateReadOnly('k', task)).toBe('R2');
    expect(task).toHaveBeenCalledTimes(2);
  });

  test('propagates rejection to all deduped callers and clears the key', async () => {
    const task = jest.fn().mockRejectedValueOnce(new Error('boom')).mockResolvedValueOnce('ok');
    const a = gateReadOnly('k', task);
    const b = gateReadOnly('k', task);
    await expect(a).rejects.toThrow('boom');
    await expect(b).rejects.toThrow('boom');
    expect(await gateReadOnly('k', task)).toBe('ok'); // key cleared → fresh run
    expect(task).toHaveBeenCalledTimes(2);
  });
});

describe('gateReadOnly — concurrency cap', () => {
  test('runs at most 6 concurrently and starts a queued task when one frees a slot', async () => {
    const defs = Array.from({ length: 9 }, () => deferred<string>());
    const tasks = defs.map((d) => jest.fn().mockReturnValue(d.promise));
    const results = defs.map((_, i) => gateReadOnly(`k${i}`, tasks[i]));

    await flush();
    const started = () => tasks.filter((t) => t.mock.calls.length >= 1).length;
    expect(started()).toBe(6); // 6 running, 3 queued

    defs[0].resolve('a');
    await results[0];
    await flush();
    expect(started()).toBe(7); // one slot freed → one queued task started

    defs.forEach((d, i) => {
      if (i > 0) d.resolve(`x${i}`);
    });
    await Promise.all(results);
    expect(started()).toBe(9);
  });

  test('rejects with a retryable -32005 when the queue is saturated, then recovers', async () => {
    const defs: Array<ReturnType<typeof deferred<string>>> = [];
    const pending: Promise<unknown>[] = [];
    // 6 active + 512 queued = exactly at capacity (none overflow)
    for (let i = 0; i < 6 + 512; i++) {
      const d = deferred<string>();
      defs.push(d);
      pending.push(gateReadOnly(`q${i}`, () => d.promise).catch(() => 'rejected'));
    }
    await flush();

    // one more overflows the queue
    await expect(gateReadOnly('overflow', async () => 'x')).rejects.toMatchObject({ code: -32005 });

    // drain → the concurrency count must recover (no leaked slots)
    defs.forEach((d, i) => d.resolve(`r${i}`));
    await Promise.all(pending);
    expect(await gateReadOnly('after', async () => 'ok')).toBe('ok');
  });
});

describe('readOnlyKey', () => {
  test('is stable and case-insensitive on the account address', () => {
    expect(readOnlyKey(1, '0xAbC', 'eth_call', [{ a: 1 }])).toBe(
      readOnlyKey(1, '0xabc', 'eth_call', [{ a: 1 }]),
    );
  });

  test('differs by chain, method, params, and account address', () => {
    const base = readOnlyKey(1, '0x1', 'eth_call', [1]);
    expect(readOnlyKey(2, '0x1', 'eth_call', [1])).not.toBe(base);
    expect(readOnlyKey(1, '0x1', 'eth_getBalance', [1])).not.toBe(base);
    expect(readOnlyKey(1, '0x1', 'eth_call', [2])).not.toBe(base);
    // account address must be part of the key so switching accounts never
    // collides two different accounts' reads onto one shared result
    expect(readOnlyKey(1, '0xAAA', 'eth_call', [1])).not.toBe(readOnlyKey(1, '0xBBB', 'eth_call', [1]));
  });
});
