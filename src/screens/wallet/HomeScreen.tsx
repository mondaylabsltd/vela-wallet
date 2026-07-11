/**
 * HomeScreen (layout A) — payment-first, activity-first single screen.
 *
 *   Header:   account selector · settings (gear)
 *   Balance:  "Total balance · CODE" label + the number (tap = hide/show)
 *   Content:  [ Activity | Assets | Connections ] toggle + Network filter
 *               · Activity    = value-transfer feed (received / sent)
 *               · Assets      = holdings list (HoldingsList → token detail)
 *               · Connections = single active dApp connection + its events
 *   Dock:     Receive · Scan · Send  (WaveDock, full-bleed)
 *
 * The hero is deliberately bare — the number is the only actor. Display
 * currency moved to Settings › Localization (N01 FR-1); balance privacy is the
 * number's own tap (persisted, masks the feed + holdings too, an EyeOff glyph
 * appears only in the masked state).
 *
 * Incoming payments play a haptic + row glow. The Activity feed currently uses
 * the interim local-tx adapter; the RPC received-transfer monitor plugs into the
 * same `ActivityItem` shape later.
 */
import { useFocusEffect } from '@react-navigation/native';
import { useRouter } from 'expo-router';
import {
  AlertTriangle, ArrowRight, ChevronDown, ChevronRight, EyeOff, History, Inbox, Plug, RefreshCw, Settings, Trash2,
} from 'lucide-react-native';
import { Swipeable } from 'react-native-gesture-handler';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Platform, Pressable, Text, TextInput, View } from 'react-native';
import Animated, {
  Easing, useAnimatedStyle, useSharedValue, withRepeat, withSequence, withTiming,
} from 'react-native-reanimated';
import { SafeAreaView, useSafeAreaInsets } from 'react-native-safe-area-context';

import { QRScanner } from '@/components/QRScanner';
import { ConnectionFlowStates } from '@/components/ConnectionFlowStates';
import { ActivityRow } from '@/components/ui/ActivityRow';
import { ConnectionEventDetailSheet } from '@/components/ui/ConnectionEventDetailSheet';
import { AmountText } from '@/components/ui/AmountText';
import { AccountSwitcherModal } from '@/components/ui/AccountSwitcherModal';
import { HoldingsList } from '@/components/ui/HoldingsList';
import { NetworkFilterButton, NetworkFilterSheet } from '@/components/ui/NetworkFilterSheet';
import { TransactionDetailSheet } from '@/components/ui/TransactionDetailSheet';
import { SegmentedToggle } from '@/components/ui/SegmentedToggle';
import { SigningReplaySheet } from '@/components/ui/SigningReplaySheet';
import { BrowserHistorySheet } from '@/components/ui/BrowserHistorySheet';
import { getBrowserHistory } from '@/services/browser-history';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaRefresh } from '@/components/ui/VelaRefresh';
import { WalletAvatar } from '@/components/ui/WalletAvatar';
import { DOCK_BAR_HEIGHT, WaveDock } from '@/components/ui/WaveDock';
import { RpcTroubleBanner, RpcFixModal } from '@/components/ui/RpcTroubleBanner';
import { BalanceDetailSheet } from '@/components/ui/BalanceDetailSheet';
import { getRateLimitedChains } from '@/services/rpc-pool';

import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import { useDAppConnection, type ConnectionStatus } from '@/models/dapp-connection';
import { getAllNetworksSync, type Network } from '@/models/network';
import { shortAddr, isAddress, tokenBalanceDouble, tokenChainId, tokenLogoURLs, tokenUsdValue, type APIToken } from '@/models/types';
import { useBalancePrivacy } from '@/hooks/use-balance-privacy';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { shortAddress, useWallet } from '@/models/wallet-state';
import {
  dayGroupLabel, dayStartMs, loadActivityItems, loadActivityTransactions, loadConnectionEvents, relativeTime, syncReceivedTransfers,
  type ActivityItem, type ActivityBatch, type ConnectionEvent,
} from '@/services/activity';
import { reconcilePendingTransactions } from '@/services/tx-reconciler';
import { useLocalePrefs } from '@/services/locale-format';
import { deleteConnectionEvents, deleteTransaction, type LocalTransaction } from '@/services/storage';
import { getAccountBalance, getAccountBalances, setAccountBalance } from '@/services/balance-cache';
import { currencyMeta, shouldShowDecimals } from '@/services/currency';
import { parseRemoteInjectURL, coerceBrowserUrl } from '@/services/dapp-transport';
import { parseEIP681 } from '@/services/eip681';
import { copyToClipboard, hapticLight, hapticSuccess, isAppActive, showAlert } from '@/services/platform';
import { resolveRecipientIdentity } from '@/services/recipient-identity';
import { fetchTokens } from '@/services/wallet-api';
import { isWalletPairURI } from '@/services/walletpair-transport';

const AUTO_REFRESH_MS = 10 * 60 * 1000;
const LIVE_POLL_MS = 10 * 1000;
// When a fetch settles incomplete (a chain's RPC failed, or a held token has no
// price yet), we don't shout "still updating" immediately — that notice showing
// on a routine hiccup, and surviving repeated pulls, is the bug we're fixing.
// Instead we silently force-refetch a few times with escalating backoff; the
// notice only appears if the balance is STILL incomplete after all of them.
const MAX_PARTIAL_RETRIES = 3;
const PARTIAL_RETRY_DELAYS_MS = [1500, 4000, 8000];

// ---------------------------------------------------------------------------
// Balance — fintech "atomic number" cascade: fit-to-width on one line, drop
// cents when big, fall back to compact notation ($1.23M) before going illegible.
// All handled by <AmountText/>; here we just feed it the value + display prefs.
// ---------------------------------------------------------------------------

function Balance({ value, symbol, code }: { value: number; symbol: string; code: string }) {
  return (
    <AmountText
      value={value}
      symbol={symbol}
      size={56}
      symbolScale={0.58}
      minScale={0.55}
      showDecimals={shouldShowDecimals(value, code)}
      style={styles.balanceInt}
      tailStyle={styles.balanceDec}
      containerStyle={styles.balanceFill}
    />
  );
}

// Shimmer placeholder shown before the first balance is known — a bare "0" there
// reads as a real, wrong value. A light band sweeps across a sunken bar (raised
// on sunken reads as a highlight in BOTH themes), sized to the balance's line box.
const SKELETON_W = 208;
const SKELETON_H = 46;
const SKELETON_BAND_W = 96;
function BalanceSkeleton() {
  const x = useSharedValue(0);
  useEffect(() => {
    x.value = withRepeat(withTiming(1, { duration: 1150, easing: Easing.inOut(Easing.quad) }), -1, false);
  }, [x]);
  const band = useAnimatedStyle(() => ({
    transform: [{ translateX: -SKELETON_BAND_W + x.value * (SKELETON_W + SKELETON_BAND_W) }],
  }));
  return (
    <View style={styles.balanceFill} accessibilityLabel="…" accessibilityRole="progressbar">
      <View style={styles.balanceSkeleton}>
        <Animated.View style={[styles.balanceSkeletonBand, band]} />
      </View>
    </View>
  );
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

type Tab = 'activity' | 'assets' | 'connections';

export default function HomeScreen() {
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

  const networks = useMemo(() => getAllNetworksSync(), []);
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
  // Sum of all accounts' balances (USD), for the switcher header total.

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
  }, [router, connectFromUri]);

  const onPasteConnect = useCallback((uri: string) => {
    if (!connectFromUri(uri)) {
      showAlert(t('connect.list.invalidLinkTitle'), t('connect.list.invalidLinkBody'));
    }
  }, [connectFromUri]);

  const filteredActivity = selectedChainId != null
    ? activity.filter((a) => a.chainId === selectedChainId)
    : activity;

  // Date-first feed: rows carry no per-row time; instead they're grouped under a
  // date header ("Today" / "Yesterday" / "04/07/2026"). Items arrive newest-first,
  // so a header is emitted whenever the calendar day changes. localePrefs is a dep
  // so the header re-derives when the date format preset changes.
  type FeedRow = { kind: 'header'; id: string; label: string } | { kind: 'item'; item: ActivityItem };
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
    const t = txByIdRef.current.get(item.id);
    if (t) { setDetailBatch(null); setDetailTx(t); }
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

  // --- renderers ---
  const renderHeader = () => (
    <Animated.View entering={hasEntered.current ? undefined : fadeInDown(60, 400)}>
      {/* Balance — the hero shows on every tab, Connections included: a constant
          anchor beats reclaiming its vertical room. */}
      <Animated.View style={balanceScaleStyle}>
        <View style={styles.balanceCard}>
          {/* The code in the label keeps the unit unambiguous ($ alone could be
              USD/CAD/AUD…) now that the currency control lives in Settings. */}
          <Text style={styles.balanceLabel}>{`${t('home.totalBalance')} · ${dc.code}`}</Text>
          {/* The number is the hero's only actor: tapping it toggles privacy
              mode (persisted). The EyeOff glyph appears only beside the masked
              value — chrome only when it has something to say. */}
          <Pressable
            style={styles.balanceTopRow}
            onPress={toggleHidden}
            disabled={balanceUnknown}
            hitSlop={8}
            accessibilityRole="button"
            accessibilityLabel={hidden ? t('home.a11yShowBalance') : dc.fmt(displayTotal)}
            accessibilityHint={hidden ? undefined : t('home.a11yHideBalance')}
          >
            {hidden ? (
              <View style={styles.balanceHiddenRow}>
                <View style={styles.balanceDots}>
                  {[0, 1, 2, 3, 4, 5].map((i) => (
                    <View key={i} style={styles.balanceDot} />
                  ))}
                </View>
                <EyeOff size={20} color={color.fg.subtle} strokeWidth={2} />
              </View>
            ) : balanceUnknown ? (
              <BalanceSkeleton />
            ) : (
              <Balance value={displayTotal * dc.rate} symbol={dc.symbol} code={dc.code} />
            )}
          </Pressable>
          {balancePartial && noticeAllowed && (
            // Tappable: the ChevronRight is the "there's more — see exactly what"
            // affordance. Opens a sheet enumerating the culprit networks + tokens.
            <Pressable
              style={({ pressed }) => [styles.balanceStaleRow, pressed && styles.balanceStalePressed]}
              onPress={() => setShowBalanceDetail(true)}
              hitSlop={8}
              accessibilityRole="button"
              accessibilityHint={t('home.balanceDetailViewHint')}
            >
              <AlertTriangle size={12} color={color.warning.base} strokeWidth={2.5} />
              {/* Failed chains are transient ("still updating" is honest — a retry
                  can fix it); a held token with no price source is not going to
                  resolve on its own, so promising an update would lie. */}
              <Text style={styles.balanceStaleText}>
                {t(failedChainIds.length > 0 ? 'home.balanceStale' : 'home.balanceUnpriced')}
              </Text>
              <ChevronRight size={14} color={color.warning.base} strokeWidth={2.5} />
            </Pressable>
          )}
        </View>
      </Animated.View>

      {/* RPC failure banner + fix flow. Rate-limited
          chains are excluded — that's transient and self-healing, so nagging the
          user to swap RPC would be wrong; their balance quietly stays on cache. */}
      <RpcTroubleBanner
        chainIds={failedChainIds.filter((id) => !rateLimitedChainIds.includes(id))}
        onFix={setFixChainId}
      />

      {/* Toggle + network filter */}
      <View style={styles.navRow}>
        <SegmentedToggle<Tab>
          options={[
            { key: 'activity', label: t('home.tabActivity') },
            { key: 'assets', label: t('home.tabAssets') },
            { key: 'connections', label: t('home.tabConnections'), badge: connected ? 1 : 0 },
          ]}
          value={tab}
          onChange={setTab}
        />
        <NetworkFilterButton
          networks={networks}
          selected={selectedNetwork}
          onPress={() => setShowNetSheet(true)}
          onClear={() => setSelectedChainId(null)}
        />
      </View>
    </Animated.View>
  );

  const renderActivityEmpty = () => (
    <View style={styles.empty}>
      <View style={styles.emptyIcon}>
        <Inbox size={28} color={color.fg.subtle} strokeWidth={2} />
      </View>
      <Text style={styles.emptyText}>
        {selectedChainId != null ? t('home.emptyNoActivityNetwork') : t('home.emptyNoActivity')}
      </Text>
      <Text style={styles.emptySub}>{t('home.emptySubtitle')}</Text>
    </View>
  );

  return (
    <View style={styles.root}>
      {/* Suppressed while balance privacy is on — an incoming toast would leak
          exactly the class of number the mask conceals. */}
      {receipt && !hidden && (
        <ReceiptToast amount={receipt.amount} token={receipt.token} top={insets.top + space.md} />
      )}
      <SafeAreaView style={styles.safe} edges={['top']}>
        {/* Header */}
        <Animated.View style={styles.header} entering={hasEntered.current ? undefined : fadeIn(0, 400)}>
          <Pressable
            style={styles.account}
            onPress={openSwitcher}
            accessibilityRole="button"
            accessibilityLabel={t('home.a11ySwitchAccount', { name: accountName })}
          >
            {/* Tapping the identicon itself enlarges it (handled inside
                WalletAvatar); the rest of this button opens the switcher. */}
            <WalletAvatar name={accountName} address={address} size={44} letterSize={text.lg} enlargeable />
            <View style={styles.accountInfo}>
              <View style={styles.accountNameRow}>
                <Text style={styles.accountName} numberOfLines={1}>{accountName}</Text>
                {state.accounts.length > 1 && (
                  <ChevronDown size={15} color={color.fg.subtle} strokeWidth={2.4} />
                )}
              </View>
              <Text style={styles.accountAddr} numberOfLines={1}>{shortAddr(address)}</Text>
            </View>
          </Pressable>
          <Pressable
            style={styles.iconBtn}
            onPress={() => router.navigate('/settings')}
            hitSlop={6}
            accessibilityRole="button"
            accessibilityLabel={t('home.a11yOpenSettings')}
          >
            <Settings size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </Animated.View>

        {/* Content — branded pull-to-refresh (gesture-driven, cross-platform) */}
        {tab === 'activity' ? (
          <VelaRefresh refreshing={refreshing} onRefresh={onRefresh} statusText={refreshStatus}>
            {(scrollProps) => (
              <Animated.FlatList
                {...scrollProps}
                data={activityFeed}
                keyExtractor={(row: FeedRow) => (row.kind === 'header' ? row.id : row.item.id)}
                ListHeaderComponent={renderHeader()}
                ListEmptyComponent={renderActivityEmpty()}
                renderItem={({ item: row, index }: { item: FeedRow; index: number }) => {
                  if (row.kind === 'header') {
                    return <Text style={styles.dayHeader}>{row.label}</Text>;
                  }
                  const item = row.item;
                  // Hairline only between consecutive item rows — never abutting a
                  // day header (the header's own spacing separates groups).
                  const prev = activityFeed[index - 1];
                  return (
                    <>
                      {prev && prev.kind === 'item' ? <View style={styles.sep} /> : null}
                      <ActivityRow
                        direction={item.direction}
                        title={t(item.direction === 'out' ? 'activity.sent' : 'activity.received')}
                        subtitle={
                          item.address
                            ? t(item.direction === 'out' ? 'activity.toAddr' : 'activity.fromAddr', {
                                // Prefer a resolved alias (ENS/.bnb/Vela/own-account), then the
                                // stored name, falling back to the short address. aliasMap is state,
                                // so the row re-renders to the name once it resolves.
                                addr: aliasMap.get(item.address.toLowerCase()) ?? item.alias ?? shortAddress(item.address),
                              })
                            : item.subtitle
                        }
                        amount={item.amount}
                        masked={hidden}
                        fiat={!hidden && item.usdValue > 0 ? dc.fmt(item.usdValue) : undefined}
                        chain={chainFor(item.chainId)}
                        index={index}
                        isNew={item.id === newItemId}
                        onPress={() => openDetail(item)}
                      />
                    </>
                  );
                }}
                contentContainerStyle={listContentStyle}
                showsVerticalScrollIndicator={false}
              />
            )}
          </VelaRefresh>
        ) : tab === 'assets' ? (
          // Keyed by address: an account switch resets the list's local state
          // (zero-balance superset, toggle, search) instead of leaking the
          // previous account's holdings while the new scan streams in.
          <HoldingsList
            key={address ?? 'none'}
            tokens={tokens}
            loading={tokens.length === 0 && (cachedTotal ?? 0) > 0}
            selectedChainId={selectedChainId}
            header={renderHeader()}
            refreshing={refreshing}
            onRefresh={onRefresh}
            refreshStatus={refreshStatus}
            contentContainerStyle={listContentStyle}
          />
        ) : (
          <VelaRefresh refreshing={refreshing} onRefresh={onRefresh} statusText={refreshStatus}>
            {(scrollProps) => (
              <Animated.ScrollView
                {...scrollProps}
                contentContainerStyle={listContentStyle}
                showsVerticalScrollIndicator={false}
              >
                {renderHeader()}
                <ConnectionsView
                  status={conn.status}
                  reconnectStuck={conn.reconnectStuck}
                  dappName={conn.dappInfo?.name ?? null}
                  dappUrl={conn.dappInfo?.url ?? null}
                  events={connEvents}
                  onDisconnect={conn.disconnectBridge}
                  onReconnect={conn.reconnect}
                  onConnect={() => setShowScanner(true)}
                  onPasteConnect={onPasteConnect}
                  onOpenEvent={setEventTx}
                  onClearEvents={clearConnEvents}
                  onDeleteEvent={deleteConnEvent}
                />
              </Animated.ScrollView>
            )}
          </VelaRefresh>
        )}
      </SafeAreaView>

      {/* Bottom dock (full-bleed) */}
      <WaveDock
        onReceive={() => router.push('/receive')}
        onScan={() => setShowScanner(true)}
        onSend={() => router.push('/send')}
      />

      {/* Network filter sheet */}
      <NetworkFilterSheet
        visible={showNetSheet}
        networks={networks}
        selectedChainId={selectedChainId}
        onSelect={setSelectedChainId}
        onClose={() => setShowNetSheet(false)}
        subtitleForChain={(n) => {
          const c = activity.filter((a) => a.chainId === n.chainId).length;
          return c > 0 ? `${c} event${c > 1 ? 's' : ''}` : undefined;
        }}
      />

      {/* Balance-detail sheet (opened by the tappable hero notice) + the single
          shared RPC-fix modal. The banner chips and the sheet's per-chain "Fix"
          rows both drive RpcFixModal, so there's one recovery form, not two. */}
      <BalanceDetailSheet
        visible={showBalanceDetail}
        onClose={() => setShowBalanceDetail(false)}
        failedChainIds={failedChainIds}
        rateLimitedChainIds={rateLimitedChainIds}
        unpricedTokens={unpricedTokens}
        onFixResolved={(chainId) => {
          setFailedChainIds((prev) => prev.filter((id) => id !== chainId));
          loadData();
        }}
        onRetry={() => loadData(true)}
        onTokenPress={(token) => {
          setShowBalanceDetail(false);
          router.push({
            pathname: '/token-detail',
            params: {
              symbol: token.symbol,
              name: token.name,
              network: token.network,
              balance: token.balance,
              decimals: String(token.decimals),
              logos: JSON.stringify(tokenLogoURLs(token)),
              tokenAddress: token.tokenAddress ?? '',
              priceUsd: String(token.priceUsd ?? 0),
              chainName: token.chainName,
            },
          });
        }}
      />
      <RpcFixModal
        chainId={fixChainId}
        onClose={() => setFixChainId(null)}
        onResolved={(chainId) => {
          setFailedChainIds((prev) => prev.filter((id) => id !== chainId));
          loadData();
        }}
      />

      {/* Transaction detail */}
      <TransactionDetailSheet
        visible={detailTx !== null || detailBatch !== null}
        tx={detailTx}
        batch={detailBatch}
        alias={detailAlias}
        rate={dc.rate}
        currency={currency}
        onResolved={() => loadData()}
        onClose={() => { setDetailTx(null); setDetailBatch(null); }}
      />

      {/* dApp signing-record detail. Records that captured their original request
          replay the FULL signing panel (read-only); older ones fall back to the
          metadata detail sheet. */}
      <SigningReplaySheet
        visible={eventTx !== null && !!eventTx?.signedRequest}
        tx={eventTx}
        onClose={() => setEventTx(null)}
      />
      <ConnectionEventDetailSheet
        visible={eventTx !== null && !eventTx?.signedRequest}
        tx={eventTx}
        onClose={() => setEventTx(null)}
      />

      {/* Account switcher (shared component) */}
      <AccountSwitcherModal
        visible={showSwitcher}
        onClose={() => setShowSwitcher(false)}
        title={t('home.switchAccountTitle')}
        formatSubtitle={(amount, count) => `${t('home.switcherAccountCount', { count })}${amount}`}
        balances={cachedBalances}
        loading={switcherLoading}
        showCreateActions
      />

      {showScanner && (
        <QRScanner visible={showScanner} onScan={onScan} onClose={() => setShowScanner(false)} />
      )}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Connections view
// ---------------------------------------------------------------------------

// "立即重连" — the manual reconnect tap. SDK reconnect is fire-and-forget with no
// status of its own, so the button owns its feedback: a haptic + pressed state on
// tap (you felt it register), a continuously spinning icon (work is happening),
// and a brief label flip to "重新连接中…" right after the press to acknowledge it.
function ReconnectButton({ onReconnect }: { onReconnect: () => void }) {
  const { t } = useTranslation();
  const spin = useSharedValue(0);
  const [tapped, setTapped] = useState(false);
  const tapTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    spin.value = withRepeat(withTiming(1, { duration: 900, easing: Easing.linear }), -1, false);
    return () => { if (tapTimer.current) clearTimeout(tapTimer.current); };
  }, [spin]);

  const spinStyle = useAnimatedStyle(() => ({ transform: [{ rotate: `${spin.value * 360}deg` }] }));

  const press = () => {
    hapticLight();
    setTapped(true);
    if (tapTimer.current) clearTimeout(tapTimer.current);
    tapTimer.current = setTimeout(() => setTapped(false), 1400);
    onReconnect();
  };

  return (
    <Pressable style={({ pressed }) => [styles.reconnectBtn, pressed && styles.reconnectBtnPressed]} onPress={press}>
      <Animated.View style={spinStyle}>
        <RefreshCw size={16} color={color.fg.inverse} strokeWidth={2.4} />
      </Animated.View>
      <Text style={styles.reconnectText}>
        {tapped ? t('connect.list.reconnecting') : t('home.connReconnectBtn')}
      </Text>
    </Pressable>
  );
}

function ConnectionsView({
  status, reconnectStuck, dappName, dappUrl, events, onDisconnect, onReconnect, onConnect, onPasteConnect, onOpenEvent, onClearEvents, onDeleteEvent,
}: {
  status: ConnectionStatus;
  reconnectStuck: boolean;
  dappName: string | null;
  dappUrl: string | null;
  events: ConnectionEvent[];
  onDisconnect: () => void;
  onReconnect: () => void;
  onConnect: () => void;
  onPasteConnect: (uri: string) => void;
  onOpenEvent: (tx: LocalTransaction) => void;
  onClearEvents: () => void;
  onDeleteEvent: (id: string) => void;
}) {
  const { t } = useTranslation();
  const router = useRouter();
  const [linkInput, setLinkInput] = useState('');
  const submitPaste = () => {
    if (!linkInput.trim()) return;
    onPasteConnect(linkInput);
    setLinkInput('');
  };

  // Recently-opened dApps — hidden behind the clock icon (the icon only appears once
  // there's history, so a fresh install stays clean).
  const [showHistory, setShowHistory] = useState(false);
  const [historyCount, setHistoryCount] = useState(0);
  const refreshHistoryCount = useCallback(() => {
    void getBrowserHistory().then((h) => setHistoryCount(h.length));
  }, []);
  useEffect(() => { refreshHistoryCount(); }, [refreshHistoryCount]);

  // Pairing in progress (fingerprint / waiting) or failed — shown inline so the
  // user never leaves the Connections panel while connecting.
  if (status === 'connecting' || status === 'error') {
    return <ConnectionFlowStates onScanAgain={onConnect} />;
  }

  if (status === 'disconnected') {
    return (
      <View style={styles.connEmpty}>
        <View style={styles.connEmptyIcon}><Plug size={26} color={color.fg.subtle} strokeWidth={2} /></View>
        <Text style={styles.connEmptyTitle}>{t('home.connEmptyTitle')}</Text>
        {/* One line covers it all — scan (bottom FAB) or paste/type below, for a
            dApp or any site. Keeps the empty state calm instead of stacked text. */}
        <Text style={styles.connEmptySub}>{t('home.connEmptySub')}</Text>

        <View style={[styles.connPasteRow, styles.connPasteRowSpaced]}>
          <TextInput
            style={styles.connPasteInput}
            value={linkInput}
            onChangeText={setLinkInput}
            placeholder={t('connect.list.pastePlaceholder')}
            placeholderTextColor={color.fg.subtle}
            autoCapitalize="none"
            autoCorrect={false}
            // Multiline so a long walletpair link / URL wraps and stays fully visible
            // instead of scrolling out of a cramped one-line field. blurOnSubmit keeps
            // the return key a submit (not a newline) — the ArrowRight button is the
            // primary submit either way.
            multiline
            numberOfLines={3}
            textAlignVertical="top"
            blurOnSubmit
            returnKeyType="go"
            onSubmitEditing={submitPaste}
          />
          <Pressable
            style={[styles.connPasteBtn, !linkInput.trim() && styles.connPasteBtnDisabled]}
            onPress={submitPaste}
            disabled={!linkInput.trim()}
          >
            <ArrowRight size={18} color={!linkInput.trim() ? color.fg.subtle : color.fg.inverse} strokeWidth={2.5} />
          </Pressable>
        </View>

        {/* Recently-opened dApps — one tap to reveal, only shown once there's history. */}
        {historyCount > 0 ? (
          <Pressable
            style={styles.connHistoryBtn}
            onPress={() => setShowHistory(true)}
            accessibilityRole="button"
            accessibilityLabel={t('connect.browser.historyTitle')}
          >
            <History size={15} color={color.fg.muted} strokeWidth={2} />
            <Text style={styles.connHistoryText}>{t('connect.browser.historyOpen')}</Text>
          </Pressable>
        ) : null}

        <BrowserHistorySheet
          visible={showHistory}
          onClose={() => { setShowHistory(false); refreshHistoryCount(); }}
          onOpen={(url) => { setShowHistory(false); router.push({ pathname: '/browser', params: { url } }); }}
        />
      </View>
    );
  }

  // Connected / reconnecting — active session + its signing activity.
  const reconnecting = status === 'reconnecting';
  return (
    <View>
      <VelaCard elevated style={styles.connCard}>
        <View style={styles.connTop}>
          <View style={styles.connDapp}><Text style={styles.connDappText}>{(dappName?.[0] ?? '?').toUpperCase()}</Text></View>
          <View style={styles.connInfo}>
            <Text style={styles.connName} numberOfLines={1}>{dappName ?? t('home.connDefaultName')}</Text>
            {dappUrl ? <Text style={styles.connUrl} numberOfLines={1}>{dappUrl}</Text> : null}
          </View>
          <View style={styles.connStatus}>
            <View style={[styles.connDot, reconnecting && styles.connDotReconnecting]} />
            <Text style={[styles.connStatusText, reconnecting && styles.connStatusTextReconnecting]}>
              {reconnecting ? t('connect.list.reconnecting') : t('home.connActive')}
            </Text>
          </View>
        </View>
        <Text style={[styles.connNote, reconnectStuck && styles.connNoteWarn]}>
          {reconnectStuck ? t('home.connReconnectStuck') : t('home.connNote')}
        </Text>
        {reconnecting && <ReconnectButton onReconnect={onReconnect} />}
        <Pressable style={styles.disconnectBtn} onPress={onDisconnect}>
          <Text style={styles.disconnectText}>{t('home.connDisconnect')}</Text>
        </Pressable>
      </VelaCard>

      <View style={styles.connEventsHeadRow}>
        <Text style={styles.connEventsHead}>{t('home.connEventsHead', { count: events.length })}</Text>
        {events.length > 0 && (
          <Pressable style={styles.connClearBtn} onPress={onClearEvents} hitSlop={8}>
            <Trash2 size={13} color={color.fg.subtle} strokeWidth={2} />
            <Text style={styles.connClearText}>{t('home.connClear')}</Text>
          </Pressable>
        )}
      </View>
      {events.length === 0 ? (
        <Text style={styles.connNoEvents}>{t('home.connNoEvents')}</Text>
      ) : (
        events.map((e) => (
          <Swipeable
            key={e.id}
            overshootRight={false}
            renderRightActions={() => (
              <Pressable style={styles.eventDelete} onPress={() => onDeleteEvent(e.id)}>
                <Trash2 size={18} color={color.fg.inverse} strokeWidth={2.2} />
                <Text style={styles.eventDeleteText}>{t('home.connDelete')}</Text>
              </Pressable>
            )}
          >
            <Pressable style={styles.eventRow} onPress={() => onOpenEvent(e.tx)}>
              <View style={styles.eventInfo}>
                <Text style={styles.eventLabel} numberOfLines={1}>{e.label}</Text>
                <Text style={styles.eventSub} numberOfLines={1}>{e.subtitle}</Text>
              </View>
              {e.status !== 'confirmed' && (
                <View style={[styles.eventPill, e.status === 'failed' ? styles.eventPillFailed : styles.eventPillPending]}>
                  <Text style={[styles.eventPillText, e.status === 'failed' ? styles.eventPillTextFailed : styles.eventPillTextPending]}>
                    {t(e.status === 'failed' ? 'home.connFailed' : 'home.connPending')}
                  </Text>
                </View>
              )}
              <Text style={styles.eventTime}>{relativeTime(e.timestamp)}</Text>
              <ChevronRight size={16} color={color.fg.subtle} strokeWidth={2} />
            </Pressable>
          </Swipeable>
        ))
      )}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Receipt toast — strong "money in" cue, slides in from the top (cross-platform).
// ---------------------------------------------------------------------------

function ReceiptToast({ amount, token, top }: { amount: string; token: string; top: number }) {
  const { t } = useTranslation();
  const v = useSharedValue(0);
  useEffect(() => {
    v.value = withTiming(1, { duration: 320, easing: Easing.out(Easing.quad) });
  }, [v]);
  const style = useAnimatedStyle(() => ({
    opacity: v.value,
    transform: [{ translateY: (1 - v.value) * -24 }],
  }));
  return (
    <Animated.View pointerEvents="none" style={[styles.toast, { top }, style]}>
      <View style={styles.toastDot} />
      <Text style={styles.toastText}>{t('home.toastReceived', { amount, token })}</Text>
    </Animated.View>
  );
}

const styles = createStyles(() => ({
  root: { flex: 1, backgroundColor: color.bg.base },
  safe: { flex: 1 },

  // Header
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingHorizontal: space['3xl'],
    paddingTop: space.md,
    paddingBottom: space.lg,
  },
  // De-boxed header (Wise): account + settings sit openly on the page, no card.
  account: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.xs,
  },
  accountInfo: { flex: 1, minWidth: 0 },
  accountNameRow: { flexDirection: 'row', alignItems: 'center', gap: space.xs },
  accountName: { fontSize: text.lg, ...inter.bold, color: color.fg.base, flexShrink: 1 },
  accountAddr: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle, fontFamily: font.mono },
  iconBtn: {
    width: 44, height: 44,
    alignItems: 'center', justifyContent: 'center',
  },
  iconBtnMuted: {},

  // Balance — OPEN hero (Wise): sits directly on the page, no card. Grouped by
  // space + a section label, not by a box. Premium via generous space + type.
  balanceCard: {
    paddingTop: space.lg,
    paddingBottom: space['2xl'],
  },
  balanceLabel: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle, letterSpacing: 0.6, textTransform: 'uppercase' },
  balanceTopRow: { flexDirection: 'row', alignItems: 'center', marginTop: space.sm },
  balanceFill: { flex: 1 },
  balanceInt: { fontSize: 52, ...inter.bold, fontFamily: font.display, color: color.fg.base, letterSpacing: -1.2 },
  balanceDec: { fontSize: 28, ...inter.bold, fontFamily: font.display, color: color.fg.subtle, letterSpacing: -0.5 },
  // Loading skeleton (sized to the balance line box): a sunken bar with a
  // sweeping raised band — reads as a highlight in both light and dark.
  balanceSkeleton: {
    width: SKELETON_W,
    height: SKELETON_H,
    marginVertical: (63 - SKELETON_H) / 2, // center within the ~63px balance line
    borderRadius: radius.md,
    backgroundColor: color.bg.sunken,
    overflow: 'hidden',
  },
  balanceSkeletonBand: {
    position: 'absolute',
    top: 0,
    bottom: 0,
    width: SKELETON_BAND_W,
    backgroundColor: color.bg.raised,
    opacity: 0.85,
  },
  // Masked state: fixed-size View dots (NOT bullet glyphs — those render wide and
  // wrap to a second line on Android) + the only chrome the hero ever shows (EyeOff
  // glyph). Row height is pinned to the ~63px balance line box so toggling privacy
  // doesn't shift the hero up/down.
  balanceHiddenRow: { flex: 1, flexDirection: 'row', alignItems: 'center', gap: space.md, height: 63 },
  balanceDots: { flexDirection: 'row', alignItems: 'center', gap: 12 },
  balanceDot: { width: 16, height: 16, borderRadius: 8, backgroundColor: color.fg.base },
  // Parity with the holdings view: when a chain read failed or a held token is
  // unpriced, the hero total is an estimate — say so, not a confident number.
  balanceStaleRow: { flexDirection: 'row', alignItems: 'center', gap: space.xs, marginTop: space.md, alignSelf: 'flex-start' },
  balanceStalePressed: { opacity: 0.6 },
  balanceStaleText: { fontSize: text.sm, ...inter.medium, color: color.warning.base },

  // Receipt toast
  toast: {
    position: 'absolute', alignSelf: 'center', zIndex: 50,
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    backgroundColor: color.success.base,
    paddingVertical: space.md, paddingHorizontal: space.xl,
    borderRadius: radius.full, ...shadow.lg,
  },
  toastDot: { width: 8, height: 8, borderRadius: 4, backgroundColor: color.fg.inverse },
  toastText: { fontSize: text.lg, ...inter.bold, color: color.fg.inverse },

  // Nav row — tabs are content-sized (scrollable), so push the network filter
  // to the right edge explicitly.
  navRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: space.md, marginBottom: space.lg },

  // List
  // paddingBottom (dock clearance) is inset-dependent — applied via listContentStyle.
  listContent: { paddingHorizontal: space['3xl'] },
  // Hairline divider between de-boxed rows, inset past the avatar (Apple-Wallet style)
  // so it aligns under the row's text, not the icon.
  sep: { height: 1, backgroundColor: color.border.base, marginLeft: 44 + space.lg + space.xs },
  // Date group header — quiet, uppercase-free date label above each day's rows.
  dayHeader: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
    marginTop: space.xl,
    marginBottom: space.sm,
  },
  empty: { alignItems: 'center', paddingTop: space['5xl'], gap: space.md },
  emptyIcon: {
    width: 64, height: 64, borderRadius: 32,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
    marginBottom: space.sm,
  },
  emptyText: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  emptySub: { fontSize: text.base, ...inter.regular, color: color.fg.subtle, textAlign: 'center', paddingHorizontal: space['3xl'], lineHeight: 20 },


  // Connections
  connCard: { padding: space.xl, marginBottom: space.xl },
  connTop: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  connDapp: { width: 44, height: 44, borderRadius: 13, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },
  connDappText: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  connInfo: { flex: 1, gap: 2 },
  connName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  connUrl: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  connStatus: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  connDot: { width: 7, height: 7, borderRadius: 4, backgroundColor: color.success.base },
  connDotReconnecting: { backgroundColor: color.warning.base, opacity: 0.8 },
  connStatusText: { fontSize: text.sm, ...inter.semibold, color: color.success.base },
  connStatusTextReconnecting: { color: color.warning.base },
  connNote: { fontSize: text.sm, ...inter.medium, color: color.fg.muted, marginTop: space.lg },
  connNoteWarn: { color: color.warning.base },
  reconnectBtn: {
    marginTop: space.lg, flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm,
    paddingVertical: space.lg, borderRadius: radius.lg, backgroundColor: color.accent.base, ...shadow.sm,
  },
  reconnectBtnPressed: { opacity: 0.82, transform: [{ scale: 0.985 }] },
  reconnectText: { fontSize: text.base, ...inter.semibold, color: color.fg.inverse },
  disconnectBtn: {
    marginTop: space.lg, alignItems: 'center',
    paddingVertical: space.lg, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base, backgroundColor: color.bg.raised,
  },
  disconnectText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  connEventsHeadRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: space.md },
  connEventsHead: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle, textTransform: 'uppercase', letterSpacing: 0.8 },
  connClearBtn: { flexDirection: 'row', alignItems: 'center', gap: space.xs, paddingVertical: space.xs, paddingHorizontal: space.sm },
  connClearText: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle },
  connNoEvents: { fontSize: text.base, ...inter.regular, color: color.fg.subtle },
  eventRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.lg, borderBottomWidth: 1, borderBottomColor: color.border.base,
    backgroundColor: color.bg.base,
  },
  eventInfo: { flex: 1, gap: 2 },
  eventLabel: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  eventSub: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  eventTime: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },
  eventDelete: {
    backgroundColor: color.error.base, flexDirection: 'row', alignItems: 'center',
    justifyContent: 'center', gap: space.xs, paddingHorizontal: space.xl,
  },
  eventDeleteText: { fontSize: text.sm, ...inter.semibold, color: color.fg.inverse },
  eventPill: { paddingHorizontal: space.sm, paddingVertical: 2, borderRadius: radius.full },
  eventPillPending: { backgroundColor: color.info.soft },
  eventPillFailed: { backgroundColor: color.error.soft },
  eventPillText: { fontSize: text.xs, ...inter.semibold },
  eventPillTextPending: { color: color.info.base },
  eventPillTextFailed: { color: color.error.base },

  connEmpty: { alignItems: 'center', paddingTop: space['4xl'], gap: space.md },
  connEmptyIcon: { width: 56, height: 56, borderRadius: 28, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },
  connEmptyTitle: { fontSize: text.xl, ...inter.semibold, color: color.fg.base },
  connEmptySub: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20, paddingHorizontal: space.xl },
  connOrRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg, alignSelf: 'stretch', paddingHorizontal: space.xl, marginTop: space.md },
  connOrLine: { flex: 1, height: 1, backgroundColor: color.border.base },
  connOrText: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  connPasteHint: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle, textAlign: 'center', marginBottom: space.sm },
  connPasteRow: { flexDirection: 'row', alignItems: 'flex-end', gap: space.md, alignSelf: 'stretch', paddingHorizontal: space.xl },
  connPasteRowSpaced: { marginTop: space.xl },
  connHistoryBtn: { flexDirection: 'row', alignItems: 'center', gap: space.xs, marginTop: space.xl, paddingVertical: space.xs },
  connHistoryText: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  connPasteInput: {
    // Taller, wider, larger type + multiline wrapping so a long link is readable.
    flex: 1, fontSize: text.base, fontWeight: '500', fontFamily: font.mono, lineHeight: 20,
    color: color.fg.base, paddingHorizontal: space.lg, paddingVertical: space.md,
    minHeight: 56, maxHeight: 108,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base,
  },
  connPasteBtn: { width: 56, height: 56, borderRadius: radius.lg, backgroundColor: color.accent.base, alignItems: 'center', justifyContent: 'center' },
  connPasteBtnDisabled: { backgroundColor: color.bg.sunken },
}));
