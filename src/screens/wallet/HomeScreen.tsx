/**
 * HomeScreen (layout A) — payment-first, activity-first single screen.
 *
 *   Header:   account selector · settings (gear)
 *   Balance:  total · hide · currency · Statement → blockscan.com/address
 *   Content:  [ Activity | Connections ] toggle + Network filter
 *               · Activity   = value-transfer feed (received / sent)
 *               · Connections = single active dApp connection + its events
 *   Dock:     Receive · Scan · Send  (WaveDock, full-bleed)
 *
 * Incoming payments play a haptic + row glow. The Activity feed currently uses
 * the interim local-tx adapter; the RPC received-transfer monitor plugs into the
 * same `ActivityItem` shape later.
 */
import { useFocusEffect } from '@react-navigation/native';
import { useRouter } from 'expo-router';
import {
  ArrowRight, ChevronDown, ChevronRight, Eye, EyeOff, Inbox, Plug, RefreshCw, Settings, Trash2, X,
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
import { AppModal } from '@/components/ui/AppModal';
import { AccountSwitcherModal } from '@/components/ui/AccountSwitcherModal';
import { CurrencySheet } from '@/components/ui/CurrencySheet';
import { NetworkFilterButton, NetworkFilterSheet } from '@/components/ui/NetworkFilterSheet';
import { TransactionDetailSheet } from '@/components/ui/TransactionDetailSheet';
import { SegmentedToggle } from '@/components/ui/SegmentedToggle';
import { SigningReplaySheet } from '@/components/ui/SigningReplaySheet';
import { TokenSelector } from '@/components/ui/TokenSelector';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaRefresh } from '@/components/ui/VelaRefresh';
import { WaveDock } from '@/components/ui/WaveDock';
import { RpcTroubleBanner } from '@/components/ui/RpcTroubleBanner';

import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import { useDAppConnection, type ConnectionStatus } from '@/models/dapp-connection';
import { chainName, getAllNetworksSync, type Network } from '@/models/network';
import { shortAddr, isAddress, tokenBalanceDouble, tokenChainId, tokenId, tokenUsdValue, type APIToken } from '@/models/types';
import { useTokenMultiSelect } from '@/hooks/use-token-multi-select';
import { shortAddress, useWallet } from '@/models/wallet-state';
import {
  loadActivityItems, loadActivityTransactions, loadConnectionEvents, relativeTime, syncReceivedTransfers,
  type ActivityItem, type ActivityBatch, type ConnectionEvent,
} from '@/services/activity';
import { reconcilePendingTransactions } from '@/services/tx-reconciler';
import { useLocalePrefs } from '@/services/locale-format';
import { deleteConnectionEvents, deleteTransaction, type LocalTransaction } from '@/services/storage';
import { getAccountBalance, getAccountBalances, setAccountBalance } from '@/services/balance-cache';
import { currencyMeta, formatFiat, getCurrencyCode, getRate, loadCurrency, setCurrency, shouldShowDecimals } from '@/services/currency';
import { parseRemoteInjectURL } from '@/services/dapp-transport';
import { parseEIP681 } from '@/services/eip681';
import { copyToClipboard, hapticLight, hapticSuccess, isAppActive, openURL, showAlert } from '@/services/platform';
import { resolveRecipientIdentity } from '@/services/recipient-identity';
import { fetchTokens } from '@/services/wallet-api';
import { isWalletPairURI } from '@/services/walletpair-transport';

const AUTO_REFRESH_MS = 10 * 60 * 1000;
const LIVE_POLL_MS = 10 * 1000;
const DOCK_CLEARANCE = 112;

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
      size={52}
      minScale={0.55}
      showDecimals={shouldShowDecimals(value, code)}
      style={styles.balanceInt}
      tailStyle={styles.balanceDec}
      containerStyle={styles.balanceFill}
    />
  );
}

// ---------------------------------------------------------------------------
// Screen
// ---------------------------------------------------------------------------

type Tab = 'activity' | 'connections';

export default function HomeScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const { activeAccount, state } = useWallet();
  const conn = useDAppConnection();
  const { connectToWalletPair, connectToBridge } = conn;
  // Same multi-select as the Send picker (shared hook) — filter a network in the
  // assets sheet to pick several tokens, then hand them to Send.
  const multiSelect = useTokenMultiSelect();

  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const [tab, setTab] = useState<Tab>('activity');
  const [tokens, setTokens] = useState<APIToken[]>([]);
  const [failedChainIds, setFailedChainIds] = useState<number[]>([]);
  const [cachedTotal, setCachedTotal] = useState<number | null>(null);
  const [hidden, setHidden] = useState(false);
  const [activity, setActivity] = useState<ActivityItem[]>([]);
  const [connEvents, setConnEvents] = useState<ConnectionEvent[]>([]);
  const [selectedChainId, setSelectedChainId] = useState<number | null>(null);
  const [showNetSheet, setShowNetSheet] = useState(false);
  const [showScanner, setShowScanner] = useState(false);
  const [showSwitcher, setShowSwitcher] = useState(false);
  const [switcherLoading, setSwitcherLoading] = useState(false);
  const [cachedBalances, setCachedBalances] = useState<Map<string, number>>(new Map());
  const [newItemId, setNewItemId] = useState<string | null>(null);
  const [receipt, setReceipt] = useState<{ amount: string; token: string } | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [currencyCode, setCurrencyCode] = useState(getCurrencyCode());
  const [rate, setRate] = useState(1);
  const [showCurrency, setShowCurrency] = useState(false);
  const [showAssets, setShowAssets] = useState(false);
  const [aliasMap, setAliasMap] = useState<Map<string, string>>(new Map());
  const aliasAttempted = useRef<Set<string>>(new Set());
  const [detailTx, setDetailTx] = useState<LocalTransaction | null>(null);
  const [detailBatch, setDetailBatch] = useState<ActivityBatch | null>(null);
  const [eventTx, setEventTx] = useState<LocalTransaction | null>(null);
  const txByIdRef = useRef<Map<string, LocalTransaction>>(new Map());
  const insets = useSafeAreaInsets();
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const loadDataRef = useRef<(() => Promise<void>) | null>(null);
  // Tracks the live address so a slow in-flight load for a previous account
  // can't paint stale balances after the user switches accounts.
  const addressRef = useRef(address);
  useEffect(() => { addressRef.current = address; }, [address]);

  // Swipe-to-delete tombstones. Deletion removes the row from state instantly but
  // writes to storage asynchronously; a background reload that read storage BEFORE
  // that write lands would otherwise repaint the just-deleted row. These sets hold
  // ids mid-delete so any storage-sourced repaint filters them out until the write
  // is committed. `commit*` are the only setters the reload paths should use.
  const pendingDeleteIds = useRef<Set<string>>(new Set());
  const pendingDeleteConnIds = useRef<Set<string>>(new Set());
  const commitActivity = useCallback((items: ActivityItem[]) => {
    const pend = pendingDeleteIds.current;
    setActivity(pend.size ? items.filter((a) => !pend.has(a.id)) : items);
  }, []);
  const commitConnEvents = useCallback((events: ConnectionEvent[]) => {
    const pend = pendingDeleteConnIds.current;
    setConnEvents(pend.size ? events.filter((e) => !pend.has(e.id)) : events);
  }, []);

  // Balance "money in" pulse (cross-platform via shared value).
  const balancePulse = useSharedValue(0);
  const balanceScaleStyle = useAnimatedStyle(() => ({ transform: [{ scale: 1 + balancePulse.value * 0.03 }] }));
  const balanceRingStyle = useAnimatedStyle(() => ({ opacity: balancePulse.value }));

  const networks = useMemo(() => getAllNetworksSync(), []);
  const selectedNetwork = selectedChainId != null ? networks.find((n) => n.chainId === selectedChainId) ?? null : null;
  const connected = conn.status === 'connected' || conn.status === 'reconnecting';
  const currency = currencyMeta(currencyCode);

  // --- balance: derive from streamed tokens, with cache fallback + partial detection ---
  // Never show a confidently-wrong smaller number. If a chain's RPC failed or a
  // held token couldn't be priced, the live sum is incomplete — so we mark it
  // approximate and prefer the last-known-good cached total over the undercount.
  const liveTotal = useMemo(() => tokens.reduce((s, t) => s + tokenUsdValue(t), 0), [tokens]);
  const hasUnpriced = useMemo(() => tokens.some((t) => tokenBalanceDouble(t) > 0 && t.priceUsd == null), [tokens]);
  const hasLiveData = tokens.length > 0;
  const balancePartial = failedChainIds.length > 0 || (hasLiveData && hasUnpriced);
  const displayTotal =
    !hasLiveData && cachedTotal != null ? cachedTotal
    : balancePartial && cachedTotal != null ? Math.max(liveTotal, cachedTotal)
    : liveTotal;

  // --- load currency preference once ---
  useEffect(() => {
    loadCurrency().then((code) => { setCurrencyCode(code); getRate(code).then(setRate); });
  }, []);

  // The feed's amount strings are formatted at load time and cached in state, so
  // a number-format change doesn't re-run the adapter. Re-derive the feed when
  // the format changes (other surfaces re-render via useLocalePrefs directly).
  const localePrefs = useLocalePrefs();
  useEffect(() => {
    const addr = addressRef.current;
    if (addr) loadActivityItems(addr).then(commitActivity).catch(() => {});
  }, [localePrefs, commitActivity]);

  const pickCurrency = useCallback(async (code: string) => {
    await setCurrency(code);
    setCurrencyCode(code);
    setRate(await getRate(code));
  }, []);

  const onRefresh = useCallback(async () => {
    setRefreshing(true);
    // Hold the branded spinner for a beat so a fast fetch never flickers.
    try {
      await Promise.all([
        loadDataRef.current?.(),
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
  const loadData = useCallback(async () => {
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
      // Only trust the total as the new "last known good" when it's complete —
      // no failed chains and every held token priced.
      const unpriced = result.some((t) => tokenBalanceDouble(t) > 0 && t.priceUsd == null);
      if (failed.length === 0 && !unpriced) {
        const usd = result.reduce((s, t) => s + tokenUsdValue(t), 0);
        setAccountBalance(address, usd);
        setCachedTotal(usd);
      }
    } catch { /* ignore — keep last-known tokens + total */ }
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
    let cancelled = false;
    if (address) {
      getAccountBalance(address).then((v) => { if (!cancelled && v != null) setCachedTotal(v); });
    }
    return () => { cancelled = true; };
  }, [address]);

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
    return false;
  }, [connectToWalletPair, connectToBridge]);

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

  const chainFor = (chainId: number): Network | null => networks.find((n) => n.chainId === chainId) ?? null;

  const openDetail = (item: ActivityItem) => {
    // A grouped batch row carries its own breakdown; a normal row resolves to a
    // single stored record by id.
    if (item.batch) { setDetailTx(null); setDetailBatch(item.batch); return; }
    const t = txByIdRef.current.get(item.id);
    if (t) { setDetailBatch(null); setDetailTx(t); }
  };

  // Tap a token in the balance-hero asset sheet → open Send pre-filled to it.
  const openAssetForSend = useCallback((token: APIToken) => {
    setShowAssets(false);
    router.push({ pathname: '/send', params: { preselectedSymbol: token.symbol, preselectedNetwork: token.network } });
  }, [router]);

  // Multi-token confirm: one token → normal send; two+ → hand the selection to
  // Send's multi-token flow (the send/confirm logic lives only there).
  const onAssetsMultiConfirm = useCallback(() => {
    const picked = multiSelect.selectedTokens(tokens);
    if (picked.length === 0) return;
    if (picked.length === 1) { multiSelect.reset(); openAssetForSend(picked[0]); return; }
    setShowAssets(false);
    router.push({ pathname: '/send', params: { preselectedMulti: picked.map(tokenId).join(',') } });
    multiSelect.reset();
  }, [multiSelect, tokens, openAssetForSend, router]);

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

  // Swipe-to-delete on an Activity row prunes the local record(s); on-chain
  // history is untouched and a receipt already past the monitor checkpoint won't
  // reappear. Batch sends collapse N sibling records into one row (id =
  // userOpHash, not a real record id) — delete every sibling via `batch.ids`.
  // Optimistic remove + tombstone (keyed by the row's display id) until the async
  // deletes commit, so an in-flight reload can't resurrect the row.
  const deleteActivityItem = useCallback((item: ActivityItem) => {
    const ids = item.batch?.ids ?? [item.id];
    pendingDeleteIds.current.add(item.id);
    setActivity((prev) => prev.filter((a) => a.id !== item.id));
    Promise.all(ids.map((id) => deleteTransaction(id)))
      .catch((e) => console.warn('[Home] activity delete failed', e))
      .finally(() => { pendingDeleteIds.current.delete(item.id); });
  }, []);

  // Resolved alias for the open detail tx's counterparty.
  const detailAlias = (() => {
    if (!detailTx) return undefined;
    const cp = ((detailTx.type ?? 'send') === 'receive' ? detailTx.from : detailTx.to) ?? '';
    return detailTx.toName ?? aliasMap.get(cp.toLowerCase());
  })();

  // --- renderers ---
  const renderHeader = () => (
    <Animated.View entering={fadeInDown(60, 400)}>
      {/* Balance — hidden on the Connections tab so its list gets the vertical room */}
      {tab === 'activity' && (
      <Animated.View style={balanceScaleStyle}>
        <VelaCard elevated style={styles.balanceCard}>
          <View pointerEvents="none" style={styles.balanceBlob} />
          <Text style={styles.balanceLabel}>{t('home.totalBalance')}</Text>
          <View style={styles.balanceTopRow}>
            {/* Tap the balance to see the assets behind it (reuses the Send picker). */}
            <Pressable style={styles.balanceFill} onPress={() => setShowAssets(true)} disabled={hidden} hitSlop={4}>
              {hidden ? <Text style={styles.balanceHidden}>••••••</Text> : (
                <Balance value={displayTotal * rate} symbol={currency.symbol} code={currencyCode} />
              )}
            </Pressable>
            <Pressable onPress={() => setHidden((h) => !h)} hitSlop={8} style={styles.eyeBtn}>
              {hidden ? <EyeOff size={22} color={color.fg.muted} strokeWidth={2} /> : <Eye size={22} color={color.fg.muted} strokeWidth={2} />}
            </Pressable>
          </View>
          <View style={styles.balanceBottomRow}>
            <Pressable style={styles.currencyChip} onPress={() => setShowCurrency(true)} hitSlop={6}>
              <Text style={styles.currencyText}>{currencyCode}</Text>
              <ChevronDown size={14} color={color.fg.muted} strokeWidth={2.4} />
            </Pressable>
            <Pressable style={styles.manageBtn} onPress={() => { if (address) openURL(`https://blockscan.com/address/${address}`); }} hitSlop={6}>
              <Text style={styles.manageText}>{t('home.statement')}</Text>
              <ChevronRight size={18} color={color.fg.muted} strokeWidth={2.6} />
            </Pressable>
          </View>
          <Animated.View pointerEvents="none" style={[styles.balanceRing, balanceRingStyle]} />
        </VelaCard>
      </Animated.View>
      )}

      {/* RPC failure banner + fix flow (shared with AssetsScreen). */}
      <RpcTroubleBanner
        chainIds={failedChainIds}
        onResolved={(chainId) => {
          setFailedChainIds((prev) => prev.filter((id) => id !== chainId));
          loadData();
        }}
      />

      {/* Toggle + network filter */}
      <View style={styles.navRow}>
        <SegmentedToggle<Tab>
          options={[
            { key: 'activity', label: t('home.tabActivity') },
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
      {receipt && (
        <ReceiptToast amount={receipt.amount} token={receipt.token} top={insets.top + space.md} />
      )}
      <SafeAreaView style={styles.safe} edges={['top']}>
        {/* Header */}
        <Animated.View style={styles.header} entering={fadeIn(0, 400)}>
          <Pressable style={styles.account} onPress={openSwitcher}>
            <View style={styles.avatar}><Text style={styles.avatarText}>{(accountName[0] ?? 'V').toUpperCase()}</Text></View>
            <View style={styles.accountInfo}>
              <Text style={styles.accountName} numberOfLines={1}>{accountName}</Text>
              <Text style={styles.accountAddr} numberOfLines={1}>{shortAddr(address)}</Text>
            </View>
            {state.accounts.length > 1 && (
              <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2.4} />
            )}
          </Pressable>
          <Pressable style={styles.iconBtn} onPress={() => router.navigate('/settings')} hitSlop={6}>
            <Settings size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </Animated.View>

        {/* Content — branded pull-to-refresh (gesture-driven, cross-platform) */}
        {tab === 'activity' ? (
          <VelaRefresh refreshing={refreshing} onRefresh={onRefresh}>
            {(scrollProps) => (
              <Animated.FlatList
                {...scrollProps}
                data={filteredActivity}
                keyExtractor={(item: ActivityItem) => item.id}
                ListHeaderComponent={renderHeader()}
                ListEmptyComponent={renderActivityEmpty()}
                renderItem={({ item, index }: { item: ActivityItem; index: number }) => (
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
                    fiat={item.usdValue > 0 ? formatFiat(item.usdValue * rate, currencyCode, currency.symbol) : undefined}
                    time={relativeTime(item.timestamp)}
                    chain={chainFor(item.chainId)}
                    index={index}
                    isNew={item.id === newItemId}
                    onDelete={() => deleteActivityItem(item)}
                    onPress={() => openDetail(item)}
                  />
                )}
                ItemSeparatorComponent={() => <View style={styles.sep} />}
                contentContainerStyle={styles.listContent}
                showsVerticalScrollIndicator={false}
              />
            )}
          </VelaRefresh>
        ) : (
          <VelaRefresh refreshing={refreshing} onRefresh={onRefresh}>
            {(scrollProps) => (
              <Animated.ScrollView
                {...scrollProps}
                contentContainerStyle={styles.listContent}
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

      {/* Currency picker */}
      <CurrencySheet
        visible={showCurrency}
        selected={currencyCode}
        onSelect={pickCurrency}
        onClose={() => setShowCurrency(false)}
      />

      {/* Transaction detail */}
      <TransactionDetailSheet
        visible={detailTx !== null || detailBatch !== null}
        tx={detailTx}
        batch={detailBatch}
        alias={detailAlias}
        rate={rate}
        currency={currency}
        onResolved={() => loadData()}
        onClose={() => { setDetailTx(null); setDetailBatch(null); }}
      />

      {/* Balance-hero assets — reuses the Send token picker (tap a token → Send). */}
      <AppModal visible={showAssets} onClose={() => { setShowAssets(false); multiSelect.reset(); }}>
        <View style={styles.assetsSheet}>
          <View style={styles.assetsHead}>
            <Text style={styles.assetsTitle}>{t('home.assetsSheetTitle')}</Text>
            <Pressable onPress={() => { setShowAssets(false); multiSelect.reset(); }} hitSlop={8}>
              <X size={22} color={color.fg.base} strokeWidth={2} />
            </Pressable>
          </View>
          <View style={styles.assetsBody}>
            {/* Default to the stablecoin chip; filter a specific network to
                multi-select tokens (shared with the Send picker). */}
            <TokenSelector
              tokens={tokens}
              onSelect={openAssetForSend}
              onAddChanged={loadData}
              defaultCategory="stable"
              initialChainId={multiSelect.chainId}
              multiSelect={{
                selectedIds: multiSelect.selectedIds,
                onToggle: multiSelect.toggle,
                onToggleAll: multiSelect.toggleAll,
                isAllSelected: multiSelect.isAllSelected,
                onNetworkChange: multiSelect.onNetworkChange,
                onConfirm: onAssetsMultiConfirm,
                confirmLabel: multiSelect.count === 1
                  ? t('send.continueBtn')
                  : t('send.multiSendContinue', { n: multiSelect.count, chain: multiSelect.chainId != null ? chainName(multiSelect.chainId) : '' }),
                selectAllLabel: t('send.selectAllValuable'),
              }}
            />
          </View>
        </View>
      </AppModal>

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
        formatSubtitle={(amount, count) => `${t('home.switcherAccountCount', { count })} · ${amount}`}
        balances={cachedBalances}
        loading={switcherLoading}
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
  const [linkInput, setLinkInput] = useState('');
  const submitPaste = () => {
    if (!linkInput.trim()) return;
    onPasteConnect(linkInput);
    setLinkInput('');
  };

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
        <Text style={styles.connEmptySub}>{t('home.connEmptySub')}</Text>

        {/* No dedicated scan button here — the bottom scan FAB already covers it.
            Paste a pairing URI when scanning isn't handy. */}
        <View style={styles.connOrRow}>
          <View style={styles.connOrLine} />
          <Text style={styles.connOrText}>{t('connect.list.orDivider')}</Text>
          <View style={styles.connOrLine} />
        </View>

        <View style={styles.connPasteRow}>
          <TextInput
            style={styles.connPasteInput}
            value={linkInput}
            onChangeText={setLinkInput}
            placeholder={t('connect.list.pastePlaceholder')}
            placeholderTextColor={color.fg.subtle}
            autoCapitalize="none"
            autoCorrect={false}
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
  account: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.lg,
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
    ...shadow.sm,
  },
  avatar: {
    width: 40, height: 40, borderRadius: 13,
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  avatarText: { fontSize: text.lg, ...inter.bold, color: color.accent.base },
  accountInfo: { flex: 1, minWidth: 0 },
  accountName: { fontSize: text.lg, ...inter.bold, color: color.fg.base },
  accountAddr: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle, fontFamily: font.mono },
  iconBtn: {
    width: 58, height: 58, borderRadius: radius.lg,
    backgroundColor: color.bg.raised,
    borderWidth: 1, borderColor: color.border.base,
    alignItems: 'center', justifyContent: 'center',
    ...shadow.sm,
  },
  iconBtnMuted: { backgroundColor: color.bg.sunken },

  // Balance card
  balanceCard: { padding: space['2xl'], marginBottom: space.xl, overflow: 'hidden' },
  balanceBlob: {
    position: 'absolute', top: -60, right: -50,
    width: 150, height: 150, borderRadius: 75,
    backgroundColor: color.accent.soft,
    opacity: 0.55,
  },
  balanceRing: {
    position: 'absolute', top: 0, left: 0, right: 0, bottom: 0,
    borderRadius: radius.xl, borderWidth: 2, borderColor: color.success.base,
  },
  balanceLabel: { fontSize: text.base, ...inter.medium, color: color.fg.muted, letterSpacing: 0.3 },
  balanceTopRow: { flexDirection: 'row', alignItems: 'center', marginTop: space.sm },
  balanceFill: { flex: 1 },
  balanceInt: { fontSize: 52, ...inter.bold, fontFamily: font.display, color: color.fg.base, letterSpacing: -1 },
  balanceDec: { fontSize: 30, ...inter.bold, fontFamily: font.display, color: color.fg.subtle },
  balanceHidden: { fontSize: 52, ...inter.bold, color: color.fg.base, flex: 1, letterSpacing: 2 },
  eyeBtn: { padding: space.xs },
  balanceBottomRow: { flexDirection: 'row', alignItems: 'center', marginTop: space.xl },
  currencyChip: {
    flexDirection: 'row', alignItems: 'center', gap: space.xs,
    backgroundColor: color.bg.sunken, borderRadius: radius.full,
    paddingVertical: space.sm, paddingHorizontal: space.lg,
  },
  currencyText: { fontSize: text.lg, ...inter.bold, color: color.fg.muted },
  manageBtn: { marginLeft: 'auto', flexDirection: 'row', alignItems: 'center', gap: 2 },
  manageText: { fontSize: text.lg, ...inter.semibold, color: color.fg.muted },

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

  // Nav row
  navRow: { flexDirection: 'row', alignItems: 'center', gap: space.md, marginBottom: space.lg },

  // List
  listContent: { paddingHorizontal: space['3xl'], paddingBottom: DOCK_CLEARANCE },
  sep: { height: space.lg },
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

  // Balance-hero asset sheet (Req C)
  assetsSheet: { flex: 1, backgroundColor: color.bg.base },
  assetsHead: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space['3xl'], paddingVertical: space.xl,
    borderBottomWidth: 1, borderBottomColor: color.border.base,
  },
  assetsTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  assetsBody: { flex: 1, paddingHorizontal: space['3xl'], paddingTop: space.lg },

  connEmpty: { alignItems: 'center', paddingTop: space['4xl'], gap: space.md },
  connEmptyIcon: { width: 56, height: 56, borderRadius: 28, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },
  connEmptyTitle: { fontSize: text.xl, ...inter.semibold, color: color.fg.base },
  connEmptySub: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20, paddingHorizontal: space.xl },
  connOrRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg, alignSelf: 'stretch', paddingHorizontal: space.xl, marginTop: space.md },
  connOrLine: { flex: 1, height: 1, backgroundColor: color.border.base },
  connOrText: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  connPasteRow: { flexDirection: 'row', alignItems: 'center', gap: space.md, alignSelf: 'stretch', paddingHorizontal: space.xl },
  connPasteInput: {
    flex: 1, fontSize: text.sm, fontWeight: '500', fontFamily: font.mono,
    color: color.fg.base, padding: space.lg,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base,
  },
  connPasteBtn: { width: 44, height: 44, borderRadius: radius.lg, backgroundColor: color.accent.base, alignItems: 'center', justifyContent: 'center' },
  connPasteBtnDisabled: { backgroundColor: color.bg.sunken },
}));
