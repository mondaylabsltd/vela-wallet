// The `balance_dashboard` core (Rust/wasm) driven through the WEB shell.
//
// The machine's own rules (the merge, `max(live, cached)`, the retry budget,
// the notice gate) are covered by the Rust suite. What only exists on THIS side
// is the executor, and three of its jobs are load-bearing in ways a
// hand-written double would hide:
//
//   - The `APIToken` ⇄ `BalanceToken` round trip. The wire shape carries a
//     `chain_id`; the Assets tab, the balance-detail sheet and the token-detail
//     route all read `network` (the API slug) and `chainName`. Getting the
//     reconstruction wrong silently renames every holding — or, with the slug
//     wrong, sends the token-detail route to the wrong chain.
//   - The streamed `onProgress` snapshots, which are the ONLY reason the hero
//     doesn't drop to $0 while a slow chain is still answering. They do not ride
//     the operation's result; they are dispatched separately, and a sink wired
//     to the wrong session would simply never arrive.
//   - The classification snapshot taken at settle. `rate_limited_chain_ids` now
//     comes from the rpc_pool machine's view (`rpc-pool.web.ts`), and the whole
//     point of it is that the "fix your RPC" banner stays quiet for a limit that
//     lifts on its own.
//
// So all three are asserted against the real core, over the real executor.
const mockStorage = new Map<string, string>();
jest.mock('@react-native-async-storage/async-storage', () => ({
  getItem: jest.fn(async (key: string) => mockStorage.get(key) ?? null),
  setItem: jest.fn(async (key: string, val: string) => { mockStorage.set(key, val); }),
  removeItem: jest.fn(async (key: string) => { mockStorage.delete(key); }),
}));

// The rpc_pool projection the executor snapshots at settle (G8 landed the real
// one; here it is the seam, so the test can drive the classification directly).
const rateLimited = new Set<number>();
jest.mock('@/services/rpc-pool', () => ({
  getRateLimitedChains: () => rateLimited,
  getFailedRpcChains: () => new Set<number>(),
}));

// The transport. Everything else in `wallet-api.ts` (the 5-minute cache, the
// per-chain 18s cap) is shell machinery the core deliberately never sees.
const fetchTokensMock = jest.fn();
jest.mock('@/services/wallet-api', () => ({
  fetchTokens: (...args: unknown[]) => fetchTokensMock(...args),
}));

// Load-bearing, and easy to get wrong: jest lists no `.web.ts` in
// `moduleFileExtensions`, so a bare `@/services/vela-core` resolves the NATIVE
// index and the wasm is never initialized. Importing the web entry by explicit
// path first runs `initSync` on the planted bytes.
import '@/services/vela-core';
import { toApiToken } from '@/services/wallet-state-core/balance-executor';
import * as resident from '@/services/wallet-state-core/balance-resident';
import { createBalanceSession } from '@/services/wallet-state-core/balance-session';
import type { BalanceToken } from '@/services/wallet-state-core/generated/BalanceToken';
import type { BalanceView } from '@/services/wallet-state-core/generated/BalanceView';
import type { APIToken } from '@/models/types';
import type { FetchTokensOptions } from '@/services/wallet-api';

const CACHE_KEY = 'vela.balanceCache';
const PRIVACY_KEY = 'vela.balanceHidden';

/**
 * One account per test. `balance-cache.ts` hydrates its in-memory map ONCE per
 * process and is authoritative from then on — exactly as it behaves in the app —
 * so tests share that map and must not share addresses.
 */
const acct = (n: number) => `0x${n.toString(16).padStart(40, '0')}`;
const ADDRESS = acct(1);
const OTHER = acct(2);
const EXPIRED = acct(3);
const FRESH = acct(4);
const COMPLETE = acct(5);
const RATE_LIMITED = acct(6);
const THREW = acct(7);
const RETRY = acct(8);
const PRIVACY = acct(9);
const SWITCHER = acct(10);
const SWITCHER_OTHER = acct(11);
const POLLED = acct(12);
const STREAMED = acct(13);

/** Exactly what `wallet-api.ts` produces (`logo` is always null there). */
function apiToken(over: Partial<APIToken> = {}): APIToken {
  return {
    network: 'matic-mainnet',
    chainName: 'Polygon',
    symbol: 'USDT',
    balance: '100',
    decimals: 6,
    logo: null,
    name: 'Tether USD',
    tokenAddress: '0xc2132d05d31c914a87c6611c10748aeb04b58e8f',
    priceUsd: 1,
    spam: false,
    ...over,
  };
}

const INITIAL: BalanceView = {
  address: null, display_total_usd: null, balance_unknown: true, balance_partial: false, unreachable: false,
  notice: null, hidden: false, refreshing: false, last_refreshed_at_ms: null,
  tokens: [], unpriced_tokens: [], failed_chain_ids: [], rate_limited_chain_ids: [],
  banner_chain_ids: [], holdings_loading: false, cached_total_usd: null,
  switcher: { open: false, loading: false, balances: [] },
};

/** Let the effect loop's storage / fetch round-trips settle. */
const settle = async () => {
  for (let i = 0; i < 8; i += 1) await new Promise<void>((r) => setTimeout(r, 0));
};

const streamed: { address: string; tokens: BalanceToken[] }[] = [];

function open() {
  const faults: unknown[] = [];
  let view: BalanceView = INITIAL;
  const session = createBalanceSession({
    onView: (next) => { view = next; },
    onError: (error) => { faults.push(error); },
    stream: {
      chainAssetsArrived: (address, tokens) => {
        streamed.push({ address, tokens });
        session.dispatch({ type: 'chain_assets_arrived', address, tokens });
      },
    },
  });
  session.start({ type: 'app_focused' });
  return { session, faults, latest: () => view };
}

/** Read back what `balance-cache.ts` persisted. */
function storedCache(): Record<string, { usd: number; at: number }> {
  return JSON.parse(mockStorage.get(CACHE_KEY) ?? '{}');
}

// Planted BEFORE the first read, because the persisted totals are hydrated once
// per process (`ensureLoaded`) — a later plant would never be seen, in the app
// exactly as here.
beforeAll(() => {
  const now = Date.now();
  mockStorage.set(CACHE_KEY, JSON.stringify({
    // 25 hours old — past `balance-cache.ts`'s 24h TTL, which the SHELL applies
    // because it owns the clock.
    [EXPIRED.toLowerCase()]: { usd: 500, at: now - 25 * 60 * 60 * 1000 },
    [FRESH.toLowerCase()]: { usd: 500, at: now },
    [THREW.toLowerCase()]: { usd: 42, at: now },
    [SWITCHER_OTHER.toLowerCase()]: { usd: 77, at: now },
  }));
});

beforeEach(() => {
  mockStorage.delete(PRIVACY_KEY);
  rateLimited.clear();
  streamed.length = 0;
  fetchTokensMock.mockReset();
  fetchTokensMock.mockResolvedValue([]);
});

describe('balance_dashboard core (web shell)', () => {
  test('the APIToken round trip loses nothing the Assets tab renders', async () => {
    const native = apiToken({ symbol: 'POL', name: 'Polygon', tokenAddress: null, balance: '12.5', priceUsd: 0.5 });
    const erc20 = apiToken();
    fetchTokensMock.mockResolvedValue([erc20, native]);

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: ADDRESS });
    await settle();

    // USD-sorted by the core: 100 × $1 beats 12.5 × $0.50.
    const view = h.latest();
    expect(view.tokens.map((t) => t.symbol)).toEqual(['USDT', 'POL']);
    // …and each one reconstructs into exactly the object `fetchTokens` produced,
    // slug and display name included — not "Chain 137", not `chain-137`.
    expect(view.tokens.map(toApiToken)).toEqual([erc20, native]);
    expect(view.display_total_usd).toBeCloseTo(106.25);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('a streamed chain snapshot paints before the fetch settles, and merges by chain', async () => {
    const polygon = apiToken({ balance: '10' });
    const gnosis = apiToken({ network: 'gnosis-mainnet', chainName: 'Gnosis', symbol: 'GNO', balance: '2', priceUsd: 100, tokenAddress: '0x9c58bacc331c9aa871afd802db6379a98e80cedb' });
    let progress: ((tokens: APIToken[]) => void) | undefined;
    fetchTokensMock.mockImplementation(async (_address: string, options: FetchTokensOptions) => {
      progress = options.onProgress;
      // Polygon lands first…
      options.onProgress?.([polygon]);
      await new Promise<void>((r) => setTimeout(r, 0));
      // …then Gnosis, and the accumulated snapshot carries both.
      options.onProgress?.([polygon, gnosis]);
      return [polygon, gnosis];
    });

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: STREAMED });
    await settle();

    expect(streamed.map((s) => s.tokens.length)).toEqual([1, 2]);
    expect(progress).toBeDefined();
    expect(h.latest().tokens.map((t) => t.symbol)).toEqual(['GNO', 'USDT']);
    expect(h.latest().display_total_usd).toBeCloseTo(210);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('a stale account\'s stream can never paint the new account', async () => {
    fetchTokensMock.mockResolvedValue([]);
    const h = open();
    h.session.dispatch({ type: 'account_changed', address: ADDRESS });
    await settle();
    h.session.dispatch({ type: 'account_changed', address: OTHER });
    await settle();

    h.session.dispatch({
      type: 'chain_assets_arrived',
      address: ADDRESS,
      tokens: [{ chain_id: 137, symbol: 'USDT', name: 'Tether USD', balance: '999', decimals: 6, token_address: null, price_usd: 1, spam: false }],
    });
    await settle();
    expect(h.latest().tokens).toEqual([]);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('an expired persisted total reads as nothing cached, so the hero keeps its skeleton', async () => {
    fetchTokensMock.mockImplementation(() => new Promise(() => {})); // never settles

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: EXPIRED });
    await settle();
    expect(h.latest().cached_total_usd).toBeNull();
    // Never a fake $0 that later jumps to the real value (invariant ②).
    expect(h.latest().balance_unknown).toBe(true);
    expect(h.latest().display_total_usd).toBeNull();

    h.session.dispose();
  });

  test('a fresh persisted total paints the hero instantly, and a partial fetch never undercuts it', async () => {
    // One held token with NO price source ⇒ the live sum is incomplete.
    fetchTokensMock.mockResolvedValue([apiToken({ priceUsd: null })]);

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: FRESH });
    await settle();

    expect(h.latest().balance_partial).toBe(true);
    // `max(live, cached)` — never the confidently-wrong smaller number.
    expect(h.latest().display_total_usd).toBe(500);
    // And the partial total must NOT poison the last-known-good floor.
    expect(storedCache()[FRESH.toLowerCase()].usd).toBe(500);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('a complete fetch becomes the new last-known-good total on disk', async () => {
    fetchTokensMock.mockResolvedValue([apiToken({ balance: '7', priceUsd: 3 })]);
    const h = open();
    h.session.dispatch({ type: 'account_changed', address: COMPLETE });
    await settle();

    expect(h.latest().balance_partial).toBe(false);
    expect(h.latest().display_total_usd).toBeCloseTo(21);
    expect(storedCache()[COMPLETE.toLowerCase()].usd).toBeCloseTo(21);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('a rate-limited chain is kept out of the fix-your-RPC banner', async () => {
    rateLimited.add(137);
    fetchTokensMock.mockImplementation(async (_address: string, options: FetchTokensOptions) => {
      options.onFailedChains?.([137, 100]);
      return [];
    });

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: RATE_LIMITED });
    await settle();

    expect(h.latest().failed_chain_ids).toEqual([137, 100]);
    // Snapshotted from the rpc_pool view at settle…
    expect(h.latest().rate_limited_chain_ids).toEqual([137]);
    // …and the banner nags only for the failure that will NOT lift on its own.
    expect(h.latest().banner_chain_ids).toEqual([100]);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('a fetch that throws keeps the last-known total and just closes the skeleton', async () => {
    fetchTokensMock.mockRejectedValue(new Error('offline'));

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: THREW });
    await settle();

    expect(h.latest().balance_unknown).toBe(false);
    expect(h.latest().display_total_usd).toBe(42);
    // A rejection must never reach the loop as a rejection (it is converted).
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('an incomplete result is force-refetched silently before the notice is allowed', async () => {
    fetchTokensMock.mockImplementation(async (_address: string, options: FetchTokensOptions) => {
      options.onFailedChains?.([100]);
      return [];
    });

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: RETRY });
    await settle();
    // Incomplete, but the notice stays hidden while the grace budget lasts.
    expect(h.latest().balance_partial).toBe(true);
    expect(h.latest().notice).toBeNull();
    const afterFirst = fetchTokensMock.mock.calls.length;

    // The first backoff is 1500ms, slept by the executor and answered with the
    // SAME timer id, which is the only id the core will act on.
    await new Promise<void>((r) => setTimeout(r, 1700));
    await settle();
    expect(fetchTokensMock.mock.calls.length).toBeGreaterThan(afterFirst);
    // …and the retry is forced (it must bypass the 5-minute token cache) but not
    // a pull, so the spinner never shows for it.
    const retry = fetchTokensMock.mock.calls[afterFirst] as [string, FetchTokensOptions];
    expect(retry[1].forceRefresh).toBe(true);
    expect(h.latest().refreshing).toBe(false);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  }, 15_000);

  test('the eye tap persists through the privacy store, and withholds the number itself', async () => {
    fetchTokensMock.mockResolvedValue([apiToken()]);
    const h = open();
    h.session.dispatch({ type: 'account_changed', address: PRIVACY });
    await settle();
    expect(h.latest().display_total_usd).toBeCloseTo(100);

    h.session.dispatch({ type: 'privacy_toggled' });
    await settle();
    expect(h.latest().hidden).toBe(true);
    // Withheld by construction, not masked downstream (invariant ⑧).
    expect(h.latest().display_total_usd).toBeNull();
    // The core's `WritePrivacy` is the ONE writer of this byte on web; the three
    // other masking surfaces (holdings, balance detail, switcher) read `hidden`
    // off this same view through `use-balance-privacy.web.ts`.
    expect(mockStorage.get(PRIVACY_KEY)).toBe('1');

    // A hydrate that lands after the tap must not undo it.
    h.session.dispatch({ type: 'privacy_hydrated', hidden: false });
    await settle();
    expect(h.latest().hidden).toBe(true);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('the switcher opens on cached rows, with the active row matching the hero', async () => {
    fetchTokensMock.mockResolvedValue([apiToken({ balance: '5', priceUsd: 2 })]);

    const h = open();
    h.session.dispatch({ type: 'account_changed', address: SWITCHER });
    await settle();
    expect(h.latest().display_total_usd).toBeCloseTo(10);

    h.session.dispatch({ type: 'switcher_opened', addresses: [SWITCHER, SWITCHER_OTHER] });
    await settle();
    expect(h.latest().switcher.open).toBe(true);
    const rows = new Map(h.latest().switcher.balances.map((b) => [b.address, b.usd]));
    expect(rows.get(SWITCHER)).toBeCloseTo(10);
    expect(rows.get(SWITCHER_OTHER)).toBeCloseTo(10); // refreshed by its own fetch
    expect(h.latest().switcher.loading).toBe(false);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });

  test('a backgrounded poll is dropped, a forced one is not', async () => {
    fetchTokensMock.mockResolvedValue([]);
    const h = open();
    h.session.dispatch({ type: 'account_changed', address: POLLED });
    await settle();
    const base = fetchTokensMock.mock.calls.length;

    h.session.dispatch({ type: 'app_backgrounded' });
    h.session.dispatch({ type: 'refresh_requested', force: false, pull: false });
    await settle();
    expect(fetchTokensMock.mock.calls.length).toBe(base);

    h.session.dispatch({ type: 'refresh_requested', force: true, pull: true });
    await settle();
    expect(fetchTokensMock.mock.calls.length).toBe(base + 1);
    expect(h.faults).toEqual([]);
    h.session.dispose();
  });
});

// The resident singleton — declared last on purpose: it boots ONCE per process
// and is never disposed (that is the point), so it must not be woken before the
// suites above have had the mocked services to themselves.
describe('balance_dashboard resident (web)', () => {
  test('boots once, and keeps the holdings array stable across an unrelated view change', async () => {
    const held = apiToken({ balance: '4', priceUsd: 25 });
    fetchTokensMock.mockResolvedValue([held]);

    const seen: unknown[] = [];
    const unsubscribe = resident.subscribeBalanceDashboard((view) => seen.push(view));
    resident.dispatchBalance({ type: 'account_changed', address: acct(20) });
    await settle();

    expect(resident.balanceView().display_total_usd).toBeCloseTo(100);
    const tokens = resident.balanceTokens();
    expect(tokens).toEqual([held]);

    // A privacy toggle changes the view but not the holdings. `HoldingsList` is
    // a FlatList and the Home header's entrance animation is one-shot, so the
    // array must keep its identity (design language rule 10).
    const before = seen.length;
    resident.dispatchBalance({ type: 'privacy_toggled' });
    await settle();
    expect(resident.balanceView().hidden).toBe(true);
    expect(resident.balanceTokens()).toBe(tokens);
    expect(seen.length).toBeGreaterThan(before);

    // A view that did not change must not wake the screen: a re-dispatch of the
    // same account is a whole no-op in the core and stops here.
    const settled = seen.length;
    resident.dispatchBalance({ type: 'account_changed', address: acct(20) });
    await settle();
    expect(seen.length).toBe(settled);

    // And a second ensure is inert — the session booted once for the process.
    expect(resident.ensureBalanceDashboard()).toBe(resident.ensureBalanceDashboard());
    unsubscribe();
  });
});
