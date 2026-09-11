/**
 * The one `balance_dashboard` core the web app has — WEB only, and APP-RESIDENT.
 *
 * The integration plan mandates the module-level singleton for this machine
 * (`use-display-currency.web.ts` / `network-admin-resident.web.ts` pattern), and
 * the machine's own contract requires it: the silent-retry budget, the partial
 * grace timer, the last-known-good total and the privacy hydrate are facts about
 * the *account*, not about a mounted screen. A per-mount session would restart
 * the retry budget every time Home regained focus, which is exactly the "still
 * updating" nag the grace exists to prevent.
 *
 * Two properties this module owes its consumers:
 *
 * - **A first frame identical to today's.** `INITIAL_VIEW` mirrors the core's
 *   own pristine projection (no address, no tokens, nothing cached, not
 *   bootstrapped ⇒ `balance_unknown`), so a component that renders before the
 *   first committed view sees the skeleton it has always seen — never a fake $0.
 * - **Reference stability.** The loop commits a view after every dispatch AND
 *   every effect resolution, so a run of best-effort cache writes would
 *   otherwise hand out a fresh (but equal) view each time. Equal views are
 *   dropped here, and the three list-shaped projections (holdings, unpriced
 *   tokens, switcher balances) are rebuilt only when their own contents change —
 *   `HoldingsList` is a `FlatList` and `HomeScreen`'s header re-creates its
 *   `entering=` animation on every re-render (design language rule 10: an
 *   entrance animation must never replay).
 *
 * Imported by explicit `.web` specifier on every side: `tsc` resolves a
 * `.web.ts` file's own imports to the base `.ts` variant, so a bare specifier
 * would type-check against a native module that does not exist.
 */

import AsyncStorage from '@react-native-async-storage/async-storage';

import { toApiToken } from '@/services/wallet-state-core/balance-executor';
import { createBalanceSession } from '@/services/wallet-state-core/balance-session';
import { BALANCE_PRIVACY_KEY } from './balance-types';
import type { BalanceEvent } from './generated/BalanceEvent';
import type { BalanceView } from './generated/BalanceView';
import type { APIToken } from '@/models/types';

/**
 * The persisted privacy byte. On web this module is the ONLY hydrate: the core
 * owns the first-write-wins race, and `use-balance-privacy.web.ts` mirrors the
 * core's `hidden` rather than reading the key a second time.
 */
const PRIVACY_KEY = BALANCE_PRIVACY_KEY;

/** The machine's own initial projection — mirrored until the first view lands. */
const INITIAL_VIEW: BalanceView = {
  address: null,
  display_total_usd: null,
  balance_unknown: true,
  unreachable: false,
  balance_partial: false,
  notice: null,
  hidden: false,
  refreshing: false,
  last_refreshed_at_ms: null,
  tokens: [],
  unpriced_tokens: [],
  failed_chain_ids: [],
  rate_limited_chain_ids: [],
  banner_chain_ids: [],
  holdings_loading: false,
  cached_total_usd: null,
  switcher: { open: false, loading: false, balances: [] },
};

let current: BalanceView = INITIAL_VIEW;
/** Structural key of `current`, so an unchanged view never re-renders Home. */
let currentKey = JSON.stringify(INITIAL_VIEW);

let currentTokens: APIToken[] = [];
let currentTokensKey = '[]';
let currentUnpriced: APIToken[] = [];
let currentUnpricedKey = '[]';
let currentSwitcherBalances = new Map<string, number>();
let currentSwitcherKey = '[]';

const listeners = new Set<(view: BalanceView) => void>();
let session: ReturnType<typeof createBalanceSession> | null = null;

function project(view: BalanceView): void {
  const tokensKey = JSON.stringify(view.tokens);
  if (tokensKey !== currentTokensKey) {
    currentTokensKey = tokensKey;
    currentTokens = view.tokens.map(toApiToken);
  }
  const unpricedKey = JSON.stringify(view.unpriced_tokens);
  if (unpricedKey !== currentUnpricedKey) {
    currentUnpricedKey = unpricedKey;
    currentUnpriced = view.unpriced_tokens.map(toApiToken);
  }
  const switcherKey = JSON.stringify(view.switcher.balances);
  if (switcherKey !== currentSwitcherKey) {
    currentSwitcherKey = switcherKey;
    currentSwitcherBalances = new Map(view.switcher.balances.map((row) => [row.address, row.usd]));
  }
  current = view;
}

export function ensureBalanceDashboard() {
  if (!session) {
    session = createBalanceSession({
      onView: (view: BalanceView) => {
        const key = JSON.stringify(view);
        if (key === currentKey) return;
        currentKey = key;
        project(view);
        listeners.forEach((listener) => listener(view));
      },
      onError: (error) => console.error('[balance-dashboard] core fault:', error),
      stream: {
        // The mid-fetch snapshots. Dispatched from `fetchTokens`'s own
        // `onProgress`, i.e. from a microtask between two effect resolutions —
        // never from inside a `core.dispatch()`, so the core is never re-entered.
        chainAssetsArrived: (address, tokens) =>
          session?.dispatch({ type: 'chain_assets_arrived', address, tokens }),
      },
    });
    // `AppFocused` with no account yet is a whole no-op in the core — it exists
    // only to give `start()` an event so the pristine view is committed (the
    // frame `INITIAL_VIEW` already mirrors).
    session.start({ type: 'app_focused' });
    // The boot-time privacy read, once per process. The core's first-write-wins
    // rule makes a toggle that races this read win, so it is safe to be slow;
    // "key missing" and "read failed" collapse to `hidden: false`, exactly as
    // `use-balance-privacy.ts:28-32` does.
    AsyncStorage.getItem(PRIVACY_KEY)
      .then((raw) => session?.dispatch({ type: 'privacy_hydrated', hidden: raw === '1' }))
      .catch(() => session?.dispatch({ type: 'privacy_hydrated', hidden: false }));
  }
  return session;
}

export function dispatchBalance(event: BalanceEvent): void {
  ensureBalanceDashboard().dispatch(event);
}

/** The latest committed view. Synchronous — that is the whole point. */
export function balanceView(): BalanceView {
  return current;
}

/** Holdings in the `APIToken` shape, reference-stable while unchanged. */
export function balanceTokens(): APIToken[] {
  return currentTokens;
}

/** The detail sheet's "couldn't be priced" list, reference-stable while unchanged. */
export function balanceUnpricedTokens(): APIToken[] {
  return currentUnpriced;
}

/** Switcher rows in the `Map<address, usd>` shape the modal takes. */
export function balanceSwitcherBalances(): Map<string, number> {
  return currentSwitcherBalances;
}

/** Subscribe to every committed view. Returns the unsubscribe. */
export function subscribeBalanceDashboard(listener: (view: BalanceView) => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
