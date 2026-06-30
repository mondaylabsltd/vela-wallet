/**
 * Tests for the network-resilience primitives (timeout, abort, classification,
 * backoff, redaction).
 */

import {
  fetchWithTimeout,
  TimeoutError,
  isTimeoutError,
  isAbortError,
  classifyNetError,
  backoffWithJitter,
  parseRetryAfterMs,
  redactUrl,
  shortenUrl,
  NET_TIMEOUTS,
} from '@/services/net';

/** A fetch that never resolves but rejects with an AbortError when its signal aborts. */
function hangingFetchRespectingSignal(): jest.Mock {
  return jest.fn(
    (_url: string, init: any) =>
      new Promise((_resolve, reject) => {
        const signal: AbortSignal = init.signal;
        const onAbort = () => {
          const e = new Error('aborted');
          e.name = 'AbortError';
          reject(e);
        };
        // Mirror real fetch: an already-aborted signal rejects synchronously.
        if (signal.aborted) onAbort();
        else signal.addEventListener('abort', onAbort);
      }),
  );
}

describe('net resilience', () => {
  const realFetch = global.fetch;
  afterEach(() => {
    global.fetch = realFetch;
    jest.restoreAllMocks();
    jest.useRealTimers();
  });

  describe('fetchWithTimeout', () => {
    test('returns the response when it resolves within budget', async () => {
      const resp = { ok: true, status: 200 } as Response;
      global.fetch = jest.fn(async () => resp) as any;
      await expect(fetchWithTimeout('https://x.test', {}, { timeoutMs: 1000 })).resolves.toBe(resp);
    });

    test('throws a TimeoutError when the request exceeds the budget', async () => {
      global.fetch = hangingFetchRespectingSignal() as any;
      const err = await fetchWithTimeout('https://x.test', {}, { timeoutMs: 10 }).catch((e) => e);
      expect(err).toBeInstanceOf(TimeoutError);
      expect(isTimeoutError(err)).toBe(true);
      expect(isAbortError(err)).toBe(false);
    });

    test('a caller abort surfaces as AbortError, not a timeout', async () => {
      global.fetch = hangingFetchRespectingSignal() as any;
      const controller = new AbortController();
      const p = fetchWithTimeout('https://x.test', {}, { timeoutMs: 5000, signal: controller.signal });
      controller.abort();
      const err = await p.catch((e) => e);
      expect(isAbortError(err)).toBe(true);
      expect(isTimeoutError(err)).toBe(false);
    });

    test('an already-aborted caller signal aborts immediately', async () => {
      global.fetch = hangingFetchRespectingSignal() as any;
      const controller = new AbortController();
      controller.abort();
      const err = await fetchWithTimeout('https://x.test', {}, { timeoutMs: 5000, signal: controller.signal }).catch((e) => e);
      expect(isAbortError(err)).toBe(true);
    });
  });

  describe('classifyNetError', () => {
    test('maps timeout / abort / network', () => {
      const abort = new Error('x');
      abort.name = 'AbortError';
      expect(classifyNetError(new TimeoutError(1000))).toBe('timeout');
      expect(classifyNetError(abort)).toBe('aborted');
      expect(classifyNetError(new Error('boom'))).toBe('network');
    });
  });

  describe('backoffWithJitter', () => {
    test('always within [0, cap]', () => {
      for (let attempt = 0; attempt < 12; attempt++) {
        const d = backoffWithJitter(attempt, 100, 1000);
        expect(d).toBeGreaterThanOrEqual(0);
        expect(d).toBeLessThanOrEqual(1000);
      }
    });

    test('ceiling grows exponentially until capped', () => {
      jest.spyOn(Math, 'random').mockReturnValue(0.999999);
      expect(backoffWithJitter(0, 100, 10_000)).toBeLessThan(100);
      expect(backoffWithJitter(1, 100, 10_000)).toBeLessThan(200);
      expect(backoffWithJitter(20, 100, 1_000)).toBeLessThanOrEqual(1_000);
    });
  });

  describe('parseRetryAfterMs', () => {
    test('delta-seconds', () => {
      expect(parseRetryAfterMs('2')).toBe(2000);
      expect(parseRetryAfterMs('0')).toBe(0);
    });
    test('null / garbage → null', () => {
      expect(parseRetryAfterMs(null)).toBeNull();
      expect(parseRetryAfterMs(undefined)).toBeNull();
      expect(parseRetryAfterMs('not-a-date')).toBeNull();
    });
    test('HTTP-date in the future', () => {
      const future = new Date(Date.now() + 5000).toUTCString();
      const ms = parseRetryAfterMs(future);
      expect(ms).not.toBeNull();
      expect(ms!).toBeGreaterThan(2000);
      expect(ms!).toBeLessThanOrEqual(5000);
    });
  });

  describe('log hygiene', () => {
    test('redactUrl strips key/apikey/token query params', () => {
      expect(redactUrl('https://x.test/v2/rpc?apikey=SECRET&n=1')).toBe('https://x.test/v2/rpc?apikey=***&n=1');
      expect(redactUrl('https://x.test/rpc?key=ABC')).toContain('key=***');
      expect(redactUrl('https://x.test/rpc?access_token=ABC')).toContain('access_token=***');
      expect(redactUrl('https://x.test/rpc?apikey=ABC')).not.toContain('ABC');
    });
    test('shortenUrl keeps host+path only (drops query)', () => {
      expect(shortenUrl('https://x.test/a/b?key=SECRET')).toBe('x.test/a/b');
    });
  });

  describe('NET_TIMEOUTS', () => {
    test('has positive, sane defaults', () => {
      for (const v of Object.values(NET_TIMEOUTS)) {
        expect(v).toBeGreaterThan(0);
        expect(v).toBeLessThanOrEqual(30_000);
      }
      // Bundler ops may legitimately take longer than a read.
      expect(NET_TIMEOUTS.bundlerRpc).toBeGreaterThanOrEqual(NET_TIMEOUTS.rpcRead);
    });
  });
});
