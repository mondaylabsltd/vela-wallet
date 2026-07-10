/**
 * Tests for requestGasSponsorship — a non-idempotent treasury write. A timeout
 * must surface as `pending_unknown` (so the caller reconciles by polling the
 * balance) rather than a hard failure, and every request must carry a stable
 * Idempotency-Key so a retry can't double-spend the treasury.
 */

jest.mock('react-native', () => ({}));
jest.mock('@/models/network', () => ({ nativeSymbol: () => 'ETH' }));
jest.mock('@react-native-async-storage/async-storage', () => ({
  __esModule: true,
  default: { getItem: jest.fn(), setItem: jest.fn(), removeItem: jest.fn() },
}));
jest.mock('@/services/storage', () => ({ loadServiceEndpoints: jest.fn(async () => ({})) }));
jest.mock('@/services/tempo', () => ({ isTempoChain: () => false, TEMPO_DEFAULT_FEE_TOKEN: '0x0' }));
jest.mock('@/services/rpc-pool', () => ({
  getActiveBundlerBaseUrl: jest.fn(async () => 'https://bundler.test'),
  getChainRpcUrl: jest.fn(async () => null),
  isUsingBuiltinBundler: jest.fn(async () => true),
  poolRpcCall: jest.fn(),
  getBuiltinBundlerUrl: jest.fn(() => 'https://bundler.test'),
}));

import {
  attemptSilentSponsorship,
  checkBundlerFunding,
  parseBundlerUnderfunded,
  probeGasSponsorship,
  requestGasSponsorship,
  underfundedRequiredWei,
  type FundingNeeded,
} from '@/services/bundler-service';

/** Minimal FundingNeeded for the silent-sponsorship helpers. Vary chainId per
 *  test — attemptSilentSponsorship keeps a module-level denial throttle keyed
 *  by (chainId, safe). */
function makeFunding(chainId: number, thresholdWei = 1_000n): FundingNeeded {
  return {
    depositAddress: '0xdep',
    safeAddress: '0xSafe',
    chainId,
    nativeSym: 'ETH',
    thresholdWei,
    recommendedWei: thresholdWei,
    currentBalance: 0n,
    recommendedFormatted: '0.000001',
    currentFormatted: '0',
  };
}

/** Route the fetch mock: sponsor endpoint vs account-info endpoint. */
function mockEndpoints(opts: {
  sponsor: () => Promise<{ ok: boolean; status?: number; body: any }>;
  spendable?: bigint;
}) {
  return jest.fn(async (url: string) => {
    if (String(url).includes('/v1/sponsor/')) {
      const r = await opts.sponsor();
      return { ok: r.ok, status: r.status ?? (r.ok ? 200 : 500), json: async () => r.body };
    }
    // /v1/account/…
    return {
      ok: true,
      json: async () => ({
        activeDepositAddress: '0xdep',
        onchainBalance: '0x' + (opts.spendable ?? 0n).toString(16),
        spendableBalance: '0x' + (opts.spendable ?? 0n).toString(16),
        status: 'ACTIVE',
      }),
    };
  }) as any;
}

describe('requestGasSponsorship', () => {
  const realFetch = global.fetch;
  afterEach(() => {
    global.fetch = realFetch;
    jest.useRealTimers();
  });

  test('a timeout returns pending_unknown (not a hard network_error)', async () => {
    jest.useFakeTimers();
    global.fetch = jest.fn(
      (_url: string, init: any) =>
        new Promise((_resolve, reject) => {
          init.signal.addEventListener('abort', () => {
            const e = new Error('aborted');
            e.name = 'AbortError';
            reject(e);
          });
        }),
    ) as any;
    const p = requestGasSponsorship(1, '0xSafe', 1000n);
    await jest.advanceTimersByTimeAsync(25_000);
    await expect(p).resolves.toEqual({ sponsored: false, reason: 'pending_unknown' });
  });

  test('sends a stable Idempotency-Key derived from (chain, safe, amount)', async () => {
    let captured: any;
    global.fetch = jest.fn(async (_url: string, init: any) => {
      captured = init;
      return { ok: true, json: async () => ({ sponsored: true }) };
    }) as any;
    const result = await requestGasSponsorship(1, '0xAbCdEf', 1000n);
    expect(result).toEqual({ sponsored: true });
    expect(captured.headers['Idempotency-Key']).toBe('sponsor:1:0xabcdef:0x3e8');
  });

  test('non-timeout network failure still reported as network_error', async () => {
    global.fetch = jest.fn(async () => {
      throw new Error('ECONNRESET');
    }) as any;
    await expect(requestGasSponsorship(1, '0xSafe', 1000n)).resolves.toEqual({
      sponsored: false,
      reason: 'network_error',
    });
  });

  test('a 503 surfaces the server reason (retryable), never a dead-end request_failed', async () => {
    global.fetch = jest.fn(async () => ({
      ok: false,
      status: 503,
      json: async () => ({ sponsored: false, reason: 'passkey_index_unavailable' }),
    })) as any;
    await expect(requestGasSponsorship(1, '0xSafe', 1000n)).resolves.toEqual({
      sponsored: false,
      reason: 'passkey_index_unavailable',
    });
  });

  test('a 503 with an unparseable body still maps to service_unavailable', async () => {
    global.fetch = jest.fn(async () => ({
      ok: false,
      status: 503,
      json: async () => { throw new Error('not json'); },
    })) as any;
    await expect(requestGasSponsorship(1, '0xSafe', 1000n)).resolves.toEqual({
      sponsored: false,
      reason: 'service_unavailable',
    });
  });
});

describe('attemptSilentSponsorship', () => {
  const realFetch = global.fetch;
  afterEach(() => { global.fetch = realFetch; });

  test('grant + verified balance → funded (sponsored: true)', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: true, txHash: '0x1' } }),
      spendable: 2_000n,
    });
    await expect(attemptSilentSponsorship(makeFunding(101))).resolves.toEqual({
      outcome: 'funded',
      sponsored: true,
    });
  });

  test('grant but lagging balance read → confirming, NEVER a denial', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: true, txHash: '0x1' } }),
      spendable: 0n,
    });
    await expect(attemptSilentSponsorship(makeFunding(102))).resolves.toEqual({
      outcome: 'confirming',
    });
  });

  test('pending_unknown → confirming (transfer may have landed; no double-ask)', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: false, reason: 'pending_unknown' } }),
    });
    await expect(attemptSilentSponsorship(makeFunding(103))).resolves.toEqual({
      outcome: 'confirming',
    });
  });

  test('already_funded → funded with sponsored:false (no "covered by Vela" note for money that was already there)', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: false, reason: 'already_funded' } }),
      spendable: 2_000n,
    });
    await expect(attemptSilentSponsorship(makeFunding(105))).resolves.toEqual({
      outcome: 'funded',
      sponsored: false,
    });
  });

  test('already_in_progress → confirming (a concurrent grant is in flight, not a denial)', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: false, reason: 'already_in_progress' } }),
    });
    await expect(attemptSilentSponsorship(makeFunding(106))).resolves.toEqual({
      outcome: 'confirming',
    });
  });

  test('parallel-space test env skips the network entirely and lands on the sheet', async () => {
    const fetchMock = jest.fn();
    global.fetch = fetchMock as any;
    (globalThis as any).__VELA_PARALLEL__ = true;
    try {
      await expect(attemptSilentSponsorship(makeFunding(107))).resolves.toEqual({ outcome: 'denied' });
      await expect(probeGasSponsorship(makeFunding(107))).resolves.toEqual({ outcome: 'denied' });
      expect(fetchMock).not.toHaveBeenCalled();
    } finally {
      delete (globalThis as any).__VELA_PARALLEL__;
    }
  });

  test('denial is cached: an immediate re-ask answers without hitting the network; force bypasses', async () => {
    const fetchMock = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: false, reason: 'no_passkey_registered' } }),
    });
    global.fetch = fetchMock;
    const funding = makeFunding(104);
    await expect(attemptSilentSponsorship(funding)).resolves.toEqual({
      outcome: 'denied',
      denialReason: 'no_passkey_registered',
    });
    const callsAfterFirst = fetchMock.mock.calls.length;
    await expect(attemptSilentSponsorship(funding)).resolves.toEqual({
      outcome: 'denied',
      denialReason: 'no_passkey_registered',
    });
    expect(fetchMock.mock.calls.length).toBe(callsAfterFirst); // throttled — no new request
    await attemptSilentSponsorship(funding, { force: true });
    expect(fetchMock.mock.calls.length).toBeGreaterThan(callsAfterFirst); // force re-asks
  });
});

describe('probeGasSponsorship (dryRun)', () => {
  const realFetch = global.fetch;
  afterEach(() => { global.fetch = realFetch; });

  test('dryRun eligible → eligible (grant deferred to the confirm slide)', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: false, dryRun: true, eligible: true } }),
    });
    await expect(probeGasSponsorship(makeFunding(201))).resolves.toEqual({ outcome: 'eligible' });
  });

  test('dryRun ineligible → denied with the server reason', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({
        ok: true,
        body: { sponsored: false, dryRun: true, eligible: false, reason: 'wallet_balance_too_low' },
      }),
    });
    await expect(probeGasSponsorship(makeFunding(202))).resolves.toEqual({
      outcome: 'denied',
      reason: 'wallet_balance_too_low',
    });
  });

  test('old server without dryRun grants for real → granted', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: { sponsored: true, txHash: '0x1' } }),
    });
    await expect(probeGasSponsorship(makeFunding(203))).resolves.toEqual({ outcome: 'granted' });
  });

  test('transport failure → eligible (probe is advisory; the real grant decides)', async () => {
    global.fetch = jest.fn(async () => { throw new Error('ECONNRESET'); }) as any;
    await expect(probeGasSponsorship(makeFunding(204))).resolves.toEqual({ outcome: 'eligible' });
  });

  test('dryRun reporting already_funded as ineligible still maps to eligible (needs no grant = green light)', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({
        ok: true,
        body: { sponsored: false, dryRun: true, eligible: false, reason: 'already_funded' },
      }),
    });
    await expect(probeGasSponsorship(makeFunding(205))).resolves.toEqual({ outcome: 'eligible' });
  });
});

describe('underfundedRequiredWei', () => {
  test('native amounts pass through unscaled', () => {
    const u = parseBundlerUnderfunded(
      'Insufficient native balance on dedicated bundler gas account. Spendable: 5, required: 1000. Deposit to: 0x1111111111111111111111111111111111111111',
    )!;
    expect(underfundedRequiredWei(u)).toBe(1000n);
  });

  test('Tempo pathUSD 6-dec units are scaled to the 18-dec balance representation', () => {
    const u = parseBundlerUnderfunded(
      'Insufficient pathUSD balance on dedicated bundler gas account. Spendable: 5, required: 2000000. Deposit to: 0x1111111111111111111111111111111111111111',
    )!;
    // 2 pathUSD (6-dec) → 2e18 in the wallet's 18-dec USD representation
    expect(underfundedRequiredWei(u)).toBe(2_000_000n * 10n ** 12n);
  });

  test('returns null when the message carried no required amount', () => {
    const u = parseBundlerUnderfunded(
      'Insufficient native balance on dedicated bundler EOA. Deposit to: 0x1111111111111111111111111111111111111111 required: soon',
    );
    // parse still detects underfunded via the deposit-address signal
    expect(u).not.toBeNull();
    expect(underfundedRequiredWei(u!)).toBeNull();
  });
});

describe('checkBundlerFunding', () => {
  const realFetch = global.fetch;
  afterEach(() => { global.fetch = realFetch; });

  test('underfunded account → FundingNeeded with the account deposit address', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: {} }),
      spendable: 0n,
    });
    const funding = await checkBundlerFunding(301, '0xSafe', 1_000n);
    expect(funding).not.toBeNull();
    expect(funding!.depositAddress).toBe('0xdep');
    expect(funding!.thresholdWei).toBe(1_000n);
    expect(funding!.recommendedWei).toBeGreaterThan(0n);
  });

  test('sufficient balance → null (no funding flow at all)', async () => {
    global.fetch = mockEndpoints({
      sponsor: async () => ({ ok: true, body: {} }),
      spendable: 5_000n,
    });
    await expect(checkBundlerFunding(302, '0xSafe', 1_000n)).resolves.toBeNull();
  });

  test('unreachable bundler → null (let the submit attempt proceed; reactive path is the net)', async () => {
    global.fetch = jest.fn(async () => { throw new Error('ECONNRESET'); }) as any;
    await expect(checkBundlerFunding(303, '0xSafe', 1_000n)).resolves.toBeNull();
  });
});
