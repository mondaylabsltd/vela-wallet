/**
 * All HomeScreen state, refs, effects, and handlers. Extracted verbatim from
 * HomeScreen so the screen file holds only view wiring. Returns everything the
 * header, feed, connections view, sheets, and dock consume.
 *
 * Boundaries are deliberately tangled here (loadData drives the feed, balance,
 * and celebration together; the account switcher reads the derived total) — the
 * same reason SendScreen keeps one controller rather than a spray of micro-hooks.
 */
import { useFocusEffect } from '@react-navigation/native';
import { useRouter } from 'expo-router';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Platform } from 'react-native';
import { Easing, useAnimatedStyle, useSharedValue, withSequence, withTiming } from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { DOCK_BAR_HEIGHT } from '@/components/ui/WaveDock';
import { space } from '@/constants/theme';
import { useAllNetworks } from '@/hooks/use-networks';
import { useBalancePrivacy } from '@/hooks/use-balance-privacy';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { useDAppConnection } from '@/models/dapp-connection';
import { type Network } from '@/models/network';
import { isAddress, tokenBalanceDouble, tokenChainId, tokenUsdValue, type APIToken } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import {
  dayGroupLabel, dayStartMs, loadActivityItems, loadActivityTransactions, loadConnectionEvents, relativeTime, syncReceivedTransfers,
  type ActivityItem, type ActivityBatch, type ConnectionEvent,
} from '@/services/activity';
import { getAccountBalance, getAccountBalances, setAccountBalance } from '@/services/balance-cache';
import { currencyMeta } from '@/services/currency';
import { coerceBrowserUrl, parseRemoteInjectURL } from '@/services/dapp-transport';
import { parseEIP681 } from '@/services/eip681';
import { useLocalePrefs } from '@/services/locale-format';
import { copyToClipboard, hapticLight, hapticSuccess, isAppActive, showAlert } from '@/services/platform';
import { resolveRecipientIdentity } from '@/services/recipient-identity';
import { getRateLimitedChains } from '@/services/rpc-pool';
import { deleteConnectionEvents, deleteTransaction, type LocalTransaction } from '@/services/storage';
import { reconcilePendingTransactions } from '@/services/tx-reconciler';
import { fetchTokens } from '@/services/wallet-api';
import { isWalletPairURI } from '@/services/walletpair-transport';

import { styles } from './HomeScreen.styles';

const AUTO_REFRESH_MS = 10 * 60 * 1000;
const LIVE_POLL_MS = 10 * 1000;
// When a fetch settles incomplete (a chain's RPC failed, or a held token has no
// price yet), we don't shout "still updating" immediately — that notice showing
// on a routine hiccup, and surviving repeated pulls, is the bug we're fixing.
// Instead we silently force-refetch a few times with escalating backoff; the
// notice only appears if the balance is STILL incomplete after all of them.
const MAX_PARTIAL_RETRIES = 3;
const PARTIAL_RETRY_DELAYS_MS = [1500, 4000, 8000];

export type Tab = 'activity' | 'assets' | 'connections';

// Date-first feed: rows carry no per-row time; instead they're grouped under a
// date header ("Today" / "Yesterday" / "04/07/2026").
export type FeedRow = { kind: 'header'; id: string; label: string } | { kind: 'item'; item: ActivityItem };

export function useHomeController() {
  const { t } = useTranslation();
  const router = useRouter();
  const { activeAccount, state } = useWallet();
  const conn = useDAppConnection();
  const { connectToWalletPair, connectToBridge } = conn;

  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const [tab, setTab] = useState<Tab>('activity');
  const [tokens, setTokens] = useState<APIToken[]>([]);
  const [failedChainIds, setFailedChainIds] = useState<number[]>([]);
  // Subset of failedChainIds that failed due to rate-limiting — transient and
  // self-healing, so we keep the cached balance but suppress the "fix your RPC"
  // banner (swapping RPC is the wrong fix for a limit that lifts on its own).
  const [rateLimitedChainIds, setRateLimitedChainIds] = useState<number[]>([]);
  const [cachedTotal, setCachedTotal] = useState<number | null>(null);
  // Gate for the "still updating / couldn't be priced" notice: kept hidden while
  // silent force-retries are still in flight, flipped true only once they're
  // exhausted and the balance is still incomplete. Reset per account.
  const [noticeAllowed, setNoticeAllowed] = useState(false);
  const partialRetriesLeft = useRef(MAX_PARTIAL_RETRIES);
  const partialRetryTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // When balances were last synced — surfaced in the pull-to-refresh caption so
  // the pull's payoff is a glance at freshness, not just a re-fetch.
  const [lastRefreshedAt, setLastRefreshedAt] = useState<number | null>(null);
  // True once the first fetch for this account has settled — lets us tell
  // "balance not known yet" (show a skeleton) from a genuine $0 wallet.
  const [bootstrapped, setBootstrapped] = useState(false);
  // Balance privacy — shared store (hero, feed, holdings, switcher, toast all
  // mask together); persisted, hydrate-race-safe.
  const { hidden, toggle: toggleHidden } = useBalancePrivacy();
  const [activity, setActivity] = useState<ActivityItem[]>([]);
  const [connEvents, setConnEvents] = useState<ConnectionEvent[]>([]);
  const [selectedChainId, setSelectedChainId] = useState<number | null>(null);
  const [showNetSheet, setShowNetSheet] = useState(false);
  // The hero's "still updating / couldn't be priced" line is tappable — it opens
  // a sheet enumerating the exact culprit networks + tokens. `fixChainId` drives
  // the single shared RPC-fix modal (used by both the banner and that sheet).
  const [showBalanceDetail, setShowBalanceDetail] = useState(false);
  const [fixChainId, setFixChainId] = useState<number | null>(null);
  const [showScanner, setShowScanner] = useState(false);
  const [showSwitcher, setShowSwitcher] = useState(false);
  const [switcherLoading, setSwitcherLoading] = useState(false);
  const [cachedBalances, setCachedBalances] = useState<Map<string, number>>(new Map());
  const [newItemId, setNewItemId] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<{ amount: string; token: string } | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [aliasMap, setAliasMap] = useState<Map<string, string>>(new Map());
  const aliasAttempted = useRef<Set<string>>(new Set());
  const [detailTx, setDetailTx] = useState<LocalTransaction | null>(null);
  const [detailBatch, setDetailBatch] = useState<ActivityBatch | null>(null);
  const [eventTx, setEventTx] = useState<LocalTransaction | null>(null);
  const txByIdRef = useRef<Map<string, LocalTransaction>>(new Map());
  const insets = useSafeAreaInsets();
  // Dock clearance depends on the device's bottom inset — static padding either
  // clips the last row (inset > 0) or wastes space (inset = 0).
  const listContentStyle = useMemo(
    () => [styles.listContent, { paddingBottom: DOCK_BAR_HEIGHT + insets.bottom + space['2xl'] }],
    [insets.bottom],
  );
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const loadDataRef = useRef<((forceRefresh?: boolean) => Promise<void>) | null>(null);
  // Tracks the live address so a slow in-flight load for a previous account
  // can't paint stale balances after the user switches accounts.
  const addressRef = useRef(address);
  useEffect(() => { addressRef.current = address; }, [address]);

  // Swipe-to-delete tombstone (Connections only — the activity feed no longer
  // has delete). A per-row delete removes the event from state instantly but
  // writes to storage asynchronously; a background reload that read storage
  // BEFORE that write lands would otherwise repaint the just-deleted event, so
  // this set holds ids mid-delete to filter such repaints. `commit*` are the
  // only setters the reload paths should use.
  const pendingDeleteConnIds = useRef<Set<string>>(new Set());
  const commitActivity = useCallback((items: ActivityItem[]) => {
    setActivity(items);
  }, []);
  const commitConnEvents = useCallback((events: ConnectionEvent[]) => {
    const pend = pendingDeleteConnIds.current;
    setConnEvents(pend.size ? events.filter((e) => !pend.has(e.id)) : events);
  }, []);

  // Entrance animations must play ONCE, on first mount. Opening the account
  // switcher fires a burst of state updates (setCachedBalances/setAccountBalance,
  // one per account, via refreshAllBalances); without this gate the FlatList
  // header re-creates its `entering={fadeInDown}` on each re-render and the whole
  // balance/header appears to slide/flicker behind the modal. After the first
  // paint, entrance props go undefined so re-renders never re-animate.
  const hasEntered = useRef(false);
  useEffect(() => { hasEntered.current = true; }, []);

  // Balance "money in" pulse (cross-platform via shared value).
  const balancePulse = useSharedValue(0);
  const balanceScaleStyle = useAnimatedStyle(() => ({ transform: [{ scale: 1 + balancePulse.value * 0.03 }] }));

  const networks = useAllNetworks();
  const selectedNetwork = selectedChainId != null ? networks.find((n) => n.chainId === selectedChainId) ?? null : null;
  const connected = conn.status === 'connected' || conn.status === 'reconnecting';
  // Display currency — set in Settings › Localization; re-read on focus by the
  // hook, so a change over there lands here without a remount.
  const dc = useDisplayCurrency();
  const currency = currencyMeta(dc.code);

  // --- balance: derive from streamed tokens, with cache fallback + partial detection ---
  // Never show a confidently-wrong smaller number. If a chain's RPC failed or a
  // held token couldn't be priced, the live sum is incomplete — so we mark it
  // approximate and prefer the last-known-good cached total over the undercount.
  const liveTotal = useMemo(() => tokens.reduce((s, t) => s + tokenUsdValue(t), 0), [tokens]);
  const hasUnpriced = useMemo(() => tokens.some((t) => tokenBalanceDouble(t) > 0 && t.priceUsd == null), [tokens]);
  // The concrete tokens behind the "couldn't be priced" line — held, valued at
  // nothing (no price source), spam excluded so the detail sheet mirrors the
  // Assets view rather than dumping airdrop noise on the user.
  const unpricedTokens = useMemo(
    () => tokens.filter((t) => !t.spam && tokenBalanceDouble(t) > 0 && t.priceUsd == null),
    [tokens],
  );
  const hasLiveData = tokens.length > 0;
  const balancePartial = failedChainIds.length > 0 || (hasLiveData && hasUnpriced);
  const displayTotal =
    !hasLiveData && cachedTotal != null ? cachedTotal
    : balancePartial && cachedTotal != null ? Math.max(liveTotal, cachedTotal)
    : liveTotal;
  // Nothing known yet: no live tokens, no cached total, first fetch still in
  // flight → show a skeleton, never a fake "0" that later jumps to the real value.
  const balanceUnknown = !hasLiveData && cachedTotal == null && !bootstrapped;

  // The feed's amount strings are formatted at load time and cached in state, so
  // a number-format change doesn't re-run the adapter. Re-derive the feed when
  // the format changes (other surfaces re-render via useLocalePrefs directly).
  const localePrefs = useLocalePrefs();
  useEffect(() => {
    const addr = addressRef.current;
    if (addr) loadActivityItems(addr).then(commitActivity).catch(() => {});
  }, [localePrefs, commitActivity]);

  // Pull-to-refresh caption: "Updated <relative time>" (the reason to pull is to
  // see freshness). relativeTime is localized + re-evaluates each render.
  const refreshStatus = lastRefreshedAt != null
    ? t('home.lastUpdated', { ago: relativeTime(Math.floor(lastRefreshedAt / 1000)) })
    : undefined;

  const onRefresh = useCallback(async () => {
    setRefreshing(true);
    // A user-initiated pull MUST re-hit RPC — forceRefresh bypasses the 5-min
    // token cache. Without it, pulling within the TTL silently re-served the same
    // (possibly stale/partial) snapshot, so the "still updating" notice never
    // cleared no matter how many times you pulled.
    try {
      await Promise.all([
        loadDataRef.current?.(true),
        new Promise((resolve) => setTimeout(resolve, 650)),
      ]);
    } finally { setRefreshing(false); }
  }, []);

  // Full "money in" feedback: row glow + haptic + toast + balance pulse.
  const celebrateReceipt = useCallback((item: ActivityItem) => {
    setNewItemId(item.id);
    hapticSuccess();
    // item.amount already ends with the symbol (e.g. "+4.84M SNDRA"); strip it so
    // the toast — which appends {{token}} itself — doesn't show the symbol twice.
    const amt = item.amount.replace(/^\+/, '').trim();
    const sp = amt.lastIndexOf(' ');
    setReceipt({ amount: sp > 0 ? amt.slice(0, sp) : amt, token: item.token });
    balancePulse.value = withSequence(
      withTiming(1, { duration: 220, easing: Easing.out(Easing.quad) }),
      withTiming(0, { duration: 1000 }),
    );
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setReceipt(null), 2800);
  }, [balancePulse]);

  // Dev helper (web): call `velaSimulateReceipt(100, 'USDT')` in the console to
  // feel the full "money in" effect (toast + balance pulse + row glow).
  useEffect(() => {
    if (Platform.OS !== 'web') return;
    (globalThis as any).velaSimulateReceipt = (amount: number | string = 1, token = 'USDT') => {
      const item: ActivityItem = {
        id: `sim-${Date.now()}`,
        direction: 'in',
        title: 'Received',
        subtitle: 'from 0xSIMULATED…test',
        amount: `+${amount} ${token}`,
        usd: `$${Number(amount).toFixed(2)}`,
        usdValue: Number(amount),
        token,
        chainId: 137,
        timestamp: Math.floor(Date.now() / 1000),
      };
      setActivity((prev) => [item, ...prev]);
      celebrateReceipt(item);
      return `simulated +${amount} ${token}`;
    };
  }, [celebrateReceipt]);

  // --- data loading ---
  const initializedRef = useRef(false);
  const loadData = useCallback(async (forceRefresh = false) => {
    if (!address) return;

    // 1. Paint the feed instantly from the on-device store (no network). It's
    //    cached locally, so it must appear the moment the screen loads — never
    //    blank behind the slow multi-chain receipt scan in step 2.
    try {
      const [items, events, rawTxs] = await Promise.all([
        loadActivityItems(address),
        loadConnectionEvents(address),
        loadActivityTransactions(address),
      ]);
      if (addressRef.current !== address) return; // account switched mid-load
      txByIdRef.current = new Map(rawTxs.map((t) => [t.id, t]));
      commitActivity(items);
      commitConnEvents(events);
    } catch { /* ignore — keep the last-known feed */ }

    // 1b. Reconcile any pending submissions whose receipt never landed in-session
    //     (app was closed before confirmation). Re-poll the bundler and converge
    //     them to confirmed/failed; re-read the feed if anything changed. This is
    //     what makes a pending send survive an app restart and still resolve.
    try {
      const reconciled = await reconcilePendingTransactions(address);
      if (addressRef.current !== address) return;
      if (reconciled > 0) {
        const [items, rawTxs] = await Promise.all([
          loadActivityItems(address),
          loadActivityTransactions(address),
        ]);
        if (addressRef.current !== address) return;
        txByIdRef.current = new Map(rawTxs.map((t) => [t.id, t]));
        commitActivity(items);
      }
    } catch { /* ignore — pending records stay pending, retried next focus */ }

    // 2. Discover + persist new receipts in the background, then re-read the feed
    //    only when something actually landed (newCount > 0) so the list doesn't
    //    flicker. Don't celebrate the existing backlog on the first pass.
    try {
      const newCount = await syncReceivedTransfers(address).catch(() => 0);
      if (addressRef.current !== address) return; // stale result for a previous account
      const firstPass = !initializedRef.current;
      initializedRef.current = true;
      if (newCount > 0) {
        const [items, rawTxs] = await Promise.all([
          loadActivityItems(address),
          loadActivityTransactions(address),
        ]);
        if (addressRef.current !== address) return;
        txByIdRef.current = new Map(rawTxs.map((t) => [t.id, t]));
        const newestIn = items.find((i) => i.direction === 'in');
        if (!firstPass && newestIn) celebrateReceipt(newestIn);
        commitActivity(items);
      }
    } catch { /* ignore */ }

    try {
      // Stream chains as they arrive and merge by chain, so the total never
      // drops to $0 mid-refresh (slow chains keep their last value).
      let failed: number[] = [];
      const result = await fetchTokens(address, {
        forceRefresh,
        onProgress: (partialTokens) => {
          if (addressRef.current !== address) return; // account switched mid-load
          setTokens((prev) => {
            const fresh = new Set(partialTokens.map((t) => tokenChainId(t)));
            const kept = prev.filter((t) => !fresh.has(tokenChainId(t)));
            return [...kept, ...partialTokens].sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
          });
        },
        onFailedChains: (ids) => { failed = ids; },
      });
      if (addressRef.current !== address) return; // stale result for a previous account
      setTokens(result);
      setFailedChainIds(failed);
      setLastRefreshedAt(Date.now());
      // Snapshot which of those failures are just rate-limiting (transient).
      setRateLimitedChainIds([...getRateLimitedChains()]);
      // Only trust the total as the new "last known good" when it's complete —
      // no failed chains and every held token priced.
      const unpriced = result.some((t) => tokenBalanceDouble(t) > 0 && t.priceUsd == null);
      const isPartial = failed.length > 0 || unpriced;
      if (!isPartial) {
        const usd = result.reduce((s, t) => s + tokenUsdValue(t), 0);
        setAccountBalance(address, usd);
        setCachedTotal(usd);
      }
      // Grace before the notice: an incomplete result gets a few silent
      // force-retries with escalating backoff before we admit "still updating".
      // A clean result resets the budget so a later hiccup gets its own grace.
      if (partialRetryTimer.current) { clearTimeout(partialRetryTimer.current); partialRetryTimer.current = null; }
      if (!isPartial) {
        partialRetriesLeft.current = MAX_PARTIAL_RETRIES;
        setNoticeAllowed(false);
      } else if (partialRetriesLeft.current > 0) {
        const delay = PARTIAL_RETRY_DELAYS_MS[MAX_PARTIAL_RETRIES - partialRetriesLeft.current] ?? 8000;
        partialRetriesLeft.current -= 1;
        setNoticeAllowed(false);
        partialRetryTimer.current = setTimeout(() => {
          if (addressRef.current === address) loadDataRef.current?.(true);
        }, delay);
      } else {
        // Retries exhausted and still incomplete — now the notice is honest.
        setNoticeAllowed(true);
      }
    } catch { /* ignore — keep last-known tokens + total */ }
    // The first fetch has settled (either way) — stop showing the skeleton.
    if (addressRef.current === address) setBootstrapped(true);
  }, [address, celebrateReceipt, commitActivity, commitConnEvents]);
  loadDataRef.current = loadData;

  useFocusEffect(useCallback(() => {
    loadData();
    const timer = setInterval(() => { if (isAppActive()) loadData(); }, AUTO_REFRESH_MS);
    return () => clearInterval(timer);
  }, [loadData]));

  // Near-real-time payment monitoring while viewing Activity (incremental scans
  // are cheap — they only fetch logs since the last checkpoint).
  useEffect(() => {
    if (tab !== 'activity') return;
    const timer = setInterval(() => { if (isAppActive()) loadData(); }, LIVE_POLL_MS);
    return () => clearInterval(timer);
  }, [tab, loadData]);

  // Refresh connection activity the moment a request is handled (approve/reject
  // both clear incomingRequest). The provider awaits the history write before
  // clearing, so storage is already up to date when this fires.
  const hadRequest = useRef(false);
  useEffect(() => {
    const has = conn.incomingRequest !== null;
    if (hadRequest.current && !has) loadData();
    hadRequest.current = has;
  }, [conn.incomingRequest, loadData]);

  // Reset incoming-detection + balance state when the active account changes,
  // then paint the hero instantly from the cached total (never a $0 flash).
  useEffect(() => {
    initializedRef.current = false;
    setNewItemId(null);
    setReceipt(null);
    setTokens([]);
    setFailedChainIds([]);
    setCachedTotal(null);
    setBootstrapped(false); // new account: unknown until its first fetch settles
    // Fresh grace budget for the new account; drop any pending retry from the old one.
    partialRetriesLeft.current = MAX_PARTIAL_RETRIES;
    setNoticeAllowed(false);
    if (partialRetryTimer.current) { clearTimeout(partialRetryTimer.current); partialRetryTimer.current = null; }
    let cancelled = false;
    if (address) {
      getAccountBalance(address).then((v) => { if (!cancelled && v != null) setCachedTotal(v); });
    }
    return () => { cancelled = true; };
  }, [address]);

  // Drop any pending partial-retry timer on unmount.
  useEffect(() => () => { if (partialRetryTimer.current) clearTimeout(partialRetryTimer.current); }, []);

  // Resolve counterparty names (own accounts → ENS/.bnb/Vela/etc.), best-effort + cached.
  useEffect(() => {
    const pending = [...new Set(
      activity.filter((a) => a.address && !a.alias).map((a) => a.address!.toLowerCase()),
    )].filter((a) => !aliasAttempted.current.has(a));
    if (pending.length === 0) return;
    pending.forEach((a) => aliasAttempted.current.add(a));

    let cancelled = false;
    (async () => {
      const resolved: [string, string][] = [];
      for (const addr of pending) {
        const own = state.accounts.find((ac) => ac.address.toLowerCase() === addr);
        if (own) { resolved.push([addr, own.name]); continue; }
        try {
          const id = await resolveRecipientIdentity(addr);
          if (id?.name) resolved.push([addr, id.name]);
        } catch { /* ignore */ }
      }
      if (!cancelled && resolved.length) {
        setAliasMap((prev) => {
          const next = new Map(prev);
          resolved.forEach(([k, v]) => next.set(k, v));
          return next;
        });
      }
    })();
    return () => { cancelled = true; };
  }, [activity, state.accounts]);

  // --- account switcher ---
  // Query every account's balance (not just the active one) and update the rows
  // + total live. fetchTokens is cached (5 min) so reopening won't spam RPCs.
  const refreshAllBalances = useCallback(async () => {
    setSwitcherLoading(true);
    try {
      await Promise.all(state.accounts.map(async (acc) => {
        try {
          const tokens = await fetchTokens(acc.address);
          const usd = tokens.reduce((s, t) => s + tokenUsdValue(t), 0);
          setAccountBalance(acc.address, usd);
          setCachedBalances((prev) => new Map(prev).set(acc.address, usd));
        } catch { /* per-account best effort */ }
      }));
    } finally {
      setSwitcherLoading(false);
    }
  }, [state.accounts]);

  const openSwitcher = useCallback(async () => {
    if (state.accounts.length <= 1) { if (address) await copyToClipboard(address); return; }
    // Paint instantly from cache, then refresh every account in the background.
    if (address) setAccountBalance(address, displayTotal);
    const balances = await getAccountBalances(state.accounts.map((a) => a.address));
    if (address) balances.set(address, displayTotal);
    setCachedBalances(balances);
    setShowSwitcher(true);
    refreshAllBalances();
  }, [address, displayTotal, state.accounts, refreshAllBalances]);

  // --- connect (shared by scanner + pasted URI) ---
  // Returns true if `data` was a recognized pairing link and a connection was
  // kicked off. We surface the whole pairing flow (fingerprint → connected)
  // inline on the Connections tab rather than pushing a separate screen.
  const connectFromUri = useCallback((data: string): boolean => {
    const trimmed = data.trim();
    if (isWalletPairURI(trimmed)) {
      connectToWalletPair(trimmed);
      setTab('connections');
      return true;
    }
    const bridge = parseRemoteInjectURL(trimmed);
    if (bridge) {
      connectToBridge(bridge);
      setTab('connections');
      return true;
    }
    // Any remaining web address (full URL or bare host) → open the in-app browser.
    const browserUrl = coerceBrowserUrl(trimmed);
    if (browserUrl) {
      router.push({ pathname: '/browser', params: { url: browserUrl } });
      return true;
    }
    return false;
  }, [connectToWalletPair, connectToBridge, router]);

  const onScan = useCallback((data: string) => {
    setShowScanner(false);

    // EIP-681 payment request → open Send pre-filled and locked. We need a chain
    // to lock onto; a chainless request degrades to a plain recipient prefill.
    const req = parseEIP681(data);
    if (req && req.chainId != null) {
      const params: Record<string, string> = {
        prefilledRecipient: req.recipient,
        prefilledChainId: String(req.chainId),
        locked: '1',
      };
      if (req.tokenAddress) params.prefilledTokenAddress = req.tokenAddress;
      if (req.amountBaseUnits != null) params.prefilledAmountBase = req.amountBaseUnits.toString();
      router.push({ pathname: '/send', params });
      return;
    }

    const addr = req?.recipient ?? data;
    if (isAddress(addr)) {
      router.push(`/send?prefilledRecipient=${addr}`);
      return;
    }
    if (!connectFromUri(data)) {
      showAlert(t('home.invalidQrTitle'), t('home.invalidQrBody'));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [router, connectFromUri]);

  const onPasteConnect = useCallback((uri: string) => {
    if (!connectFromUri(uri)) {
      showAlert(t('connect.list.invalidLinkTitle'), t('connect.list.invalidLinkBody'));
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [connectFromUri]);

  const filteredActivity = selectedChainId != null
    ? activity.filter((a) => a.chainId === selectedChainId)
    : activity;

  // Date-first feed: items arrive newest-first, so a header is emitted whenever
  // the calendar day changes. localePrefs is a dep so the header re-derives when
  // the date format preset changes.
  const activityFeed = useMemo<FeedRow[]>(() => {
    const rows: FeedRow[] = [];
    let lastDay: number | null = null;
    for (const item of filteredActivity) {
      const day = dayStartMs(item.timestamp);
      if (day !== lastDay) {
        rows.push({ kind: 'header', id: `day-${day}`, label: dayGroupLabel(item.timestamp) });
        lastDay = day;
      }
      rows.push({ kind: 'item', item });
    }
    return rows;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filteredActivity, localePrefs]);

  const chainFor = (chainId: number): Network | null => networks.find((n) => n.chainId === chainId) ?? null;

  const openDetail = (item: ActivityItem) => {
    // A grouped batch row carries its own breakdown; a normal row resolves to a
    // single stored record by id.
    if (item.batch) { setDetailTx(null); setDetailBatch(item.batch); return; }
    const tx = txByIdRef.current.get(item.id);
    if (tx) { setDetailBatch(null); setDetailTx(tx); }
  };

  // Connection-activity clear (whole list) + per-row delete. Both prune the
  // underlying records; on-chain transactions are untouched.
  const clearConnEvents = useCallback(() => {
    if (!address) return;
    showAlert(t('home.connClearTitle'), t('home.connClearBody'), [
      { text: t('home.cancel'), style: 'cancel' },
      {
        text: t('home.connClearConfirm'),
        style: 'destructive',
        onPress: () => { deleteConnectionEvents(address); setConnEvents([]); },
      },
    ]);
  }, [address, t]);

  // Disconnect is irreversible (tears down the transport AND wipes the persisted
  // session, so the dApp must re-pair), so gate it behind a confirm — a bare tap
  // shouldn't silently drop a live connection. Mirrors the in-app browser's
  // confirmDisconnect and the clear-events dialog above. Reuses the already-
  // localized connect.browser.disconnect* strings (no new keys across 14 locales).
  const confirmDisconnect = useCallback(() => {
    hapticLight();
    showAlert(t('connect.browser.disconnectTitle'), t('connect.browser.disconnectBody'), [
      { text: t('home.cancel'), style: 'cancel' },
      { text: t('home.connDisconnect'), style: 'destructive', onPress: conn.disconnectBridge },
    ]);
  }, [t, conn.disconnectBridge]);

  const deleteConnEvent = useCallback((id: string) => {
    // Optimistic remove + tombstone until the async storage write commits, so a
    // concurrent background reload can't repaint the just-deleted event.
    pendingDeleteConnIds.current.add(id);
    setConnEvents((prev) => prev.filter((e) => e.id !== id));
    deleteTransaction(id)
      .catch((e) => console.warn('[Home] connection-event delete failed', e))
      .finally(() => { pendingDeleteConnIds.current.delete(id); });
  }, []);

  // Resolved alias for the open detail tx's counterparty.
  const detailAlias = (() => {
    if (!detailTx) return undefined;
    const cp = ((detailTx.type ?? 'send') === 'receive' ? detailTx.from : detailTx.to) ?? '';
    return detailTx.toName ?? aliasMap.get(cp.toLowerCase());
  })();

  return {
    // identity / nav
    t, router, conn, state, address, accountName, insets,
    // tabs + network filter
    tab, setTab, networks, selectedNetwork, selectedChainId, setSelectedChainId,
    showNetSheet, setShowNetSheet, connected, activity,
    // balance hero
    dc, currency, hidden, toggleHidden, displayTotal, balancePartial, balanceUnknown,
    noticeAllowed, failedChainIds, rateLimitedChainIds, unpricedTokens,
    balanceScaleStyle, hasEntered,
    // balance-detail + rpc-fix
    showBalanceDetail, setShowBalanceDetail, fixChainId, setFixChainId, setFailedChainIds,
    // tokens / assets
    tokens, cachedTotal,
    // activity feed
    activityFeed, aliasMap, newItemId, chainFor, openDetail,
    // refresh
    refreshing, onRefresh, refreshStatus, listContentStyle, loadData,
    // receipt toast
    receipt,
    // connections
    connEvents, confirmDisconnect, onPasteConnect, clearConnEvents, deleteConnEvent, eventTx, setEventTx,
    // scanner
    showScanner, setShowScanner, onScan,
    // account switcher
    openSwitcher, showSwitcher, setShowSwitcher, cachedBalances, switcherLoading,
    // tx detail
    detailTx, detailBatch, detailAlias, setDetailTx, setDetailBatch,
  };
}
