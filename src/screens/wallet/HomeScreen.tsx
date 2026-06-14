/**
 * HomeScreen (layout A) — payment-first, activity-first single screen.
 *
 *   Header:   account selector · voice toggle · settings (gear)
 *   Balance:  total · hide · currency · Manage Assets → AssetsScreen
 *   Content:  [ Activity | Connections ] toggle + Network filter
 *               · Activity   = value-transfer feed (received / sent)
 *               · Connections = single active dApp connection + its events
 *   Dock:     Receive · Scan · Send  (WaveDock, full-bleed)
 *
 * Incoming payments play a voice announcement + haptic + row glow. The Activity
 * feed currently uses the interim local-tx adapter; the RPC received-transfer
 * monitor plugs into the same `ActivityItem` shape later.
 */
import { useFocusEffect } from '@react-navigation/native';
import { useRouter } from 'expo-router';
import {
  ArrowRight, Check, ChevronDown, ChevronRight, Eye, EyeOff, Inbox, Plug, Settings, X,
} from 'lucide-react-native';
import React, { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, FlatList, Platform, Pressable, RefreshControl, ScrollView, Text, TextInput, View } from 'react-native';
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
import { CurrencySheet } from '@/components/ui/CurrencySheet';
import { NetworkFilterButton, NetworkFilterSheet } from '@/components/ui/NetworkFilterSheet';
import { TransactionDetailSheet } from '@/components/ui/TransactionDetailSheet';
import { SegmentedToggle } from '@/components/ui/SegmentedToggle';
import { VelaCard } from '@/components/ui/VelaCard';
import { WaveDock } from '@/components/ui/WaveDock';

import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import { useDAppConnection, type ConnectionStatus } from '@/models/dapp-connection';
import { getAllNetworksSync, type Network } from '@/models/network';
import { shortAddr, tokenUsdValue } from '@/models/types';
import { shortAddress, useWallet } from '@/models/wallet-state';
import {
  loadActivityItems, loadActivityTransactions, loadConnectionEvents, relativeTime, syncReceivedTransfers,
  type ActivityItem, type ConnectionEvent,
} from '@/services/activity';
import type { LocalTransaction } from '@/services/storage';
import { getAccountBalances, setAccountBalance } from '@/services/balance-cache';
import { currencyMeta, formatFiat, getCurrencyCode, getRate, loadCurrency, setCurrency, shouldShowDecimals } from '@/services/currency';
import { parseRemoteInjectURL } from '@/services/dapp-transport';
import { copyToClipboard, hapticSuccess, isAppActive, showAlert } from '@/services/platform';
import { resolveRecipientIdentity } from '@/services/recipient-identity';
import { announcePayment, loadVoicePreference } from '@/services/voice';
import { fetchTokens } from '@/services/wallet-api';
import { isWalletPairURI } from '@/services/walletpair-transport';

const AUTO_REFRESH_MS = 10 * 60 * 1000;
const LIVE_POLL_MS = 30 * 1000;
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
  const { activeAccount, state, dispatch } = useWallet();
  const conn = useDAppConnection();
  const { connectToWalletPair, connectToBridge } = conn;

  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const [tab, setTab] = useState<Tab>('activity');
  const [totalUsd, setTotalUsd] = useState(0);
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
  const [aliasMap, setAliasMap] = useState<Map<string, string>>(new Map());
  const aliasAttempted = useRef<Set<string>>(new Set());
  const [detailTx, setDetailTx] = useState<LocalTransaction | null>(null);
  const [eventTx, setEventTx] = useState<LocalTransaction | null>(null);
  const txByIdRef = useRef<Map<string, LocalTransaction>>(new Map());
  const insets = useSafeAreaInsets();
  const toastTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const loadDataRef = useRef<(() => Promise<void>) | null>(null);

  // Balance "money in" pulse (cross-platform via shared value).
  const balancePulse = useSharedValue(0);
  const balanceScaleStyle = useAnimatedStyle(() => ({ transform: [{ scale: 1 + balancePulse.value * 0.03 }] }));
  const balanceRingStyle = useAnimatedStyle(() => ({ opacity: balancePulse.value }));

  // "Live" indicator pulse.
  const livePulse = useSharedValue(1);
  useEffect(() => { livePulse.value = withRepeat(withTiming(0.3, { duration: 850, easing: Easing.inOut(Easing.quad) }), -1, true); }, [livePulse]);
  const liveDotStyle = useAnimatedStyle(() => ({ opacity: livePulse.value }));

  const networks = useMemo(() => getAllNetworksSync(), []);
  const selectedNetwork = selectedChainId != null ? networks.find((n) => n.chainId === selectedChainId) ?? null : null;
  const connected = conn.status === 'connected' || conn.status === 'reconnecting';
  const currency = currencyMeta(currencyCode);

  // --- load voice + currency preferences once ---
  useEffect(() => { loadVoicePreference(); }, []);
  useEffect(() => {
    loadCurrency().then((code) => { setCurrencyCode(code); getRate(code).then(setRate); });
  }, []);

  const pickCurrency = useCallback(async (code: string) => {
    await setCurrency(code);
    setCurrencyCode(code);
    setRate(await getRate(code));
  }, []);

  const onRefresh = useCallback(async () => {
    setRefreshing(true);
    try { await loadDataRef.current?.(); } finally { setRefreshing(false); }
  }, []);

  // Full "money in" feedback: row glow + haptic + voice + toast + balance pulse.
  const celebrateReceipt = useCallback((item: ActivityItem) => {
    setNewItemId(item.id);
    hapticSuccess();
    const amt = item.amount.replace(/^[+\-]/, '').split(' ')[0] ?? '';
    announcePayment(amt, item.token);
    setReceipt({ amount: item.amount.replace(/^\+/, ''), token: item.token });
    balancePulse.value = withSequence(
      withTiming(1, { duration: 220, easing: Easing.out(Easing.quad) }),
      withTiming(0, { duration: 1000 }),
    );
    if (toastTimer.current) clearTimeout(toastTimer.current);
    toastTimer.current = setTimeout(() => setReceipt(null), 2800);
  }, [balancePulse]);

  // Dev helper (web): call `velaSimulateReceipt(100, 'USDT')` in the console to
  // feel the full "money in" effect (toast + balance pulse + row glow + voice).
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
    try {
      // Discover + persist new receipts first; newCount > 0 means a real new one.
      const newCount = await syncReceivedTransfers(address).catch(() => 0);
      const [items, events, rawTxs] = await Promise.all([
        loadActivityItems(address),
        loadConnectionEvents(address),
        loadActivityTransactions(address),
      ]);

      // Skip the initial load (don't celebrate the existing backlog).
      const newestIn = items.find((i) => i.direction === 'in');
      if (initializedRef.current && newCount > 0 && newestIn) {
        celebrateReceipt(newestIn);
      }
      initializedRef.current = true;

      txByIdRef.current = new Map(rawTxs.map((t) => [t.id, t]));
      setActivity(items);
      setConnEvents(events);
    } catch { /* ignore */ }

    try {
      const result = await fetchTokens(address);
      const usd = result.reduce((s, t) => s + tokenUsdValue(t), 0);
      setTotalUsd(usd);
      setAccountBalance(address, usd);
    } catch { /* ignore */ }
  }, [address, celebrateReceipt]);
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

  // Reset incoming-detection when the active account changes.
  useEffect(() => { initializedRef.current = false; setNewItemId(null); setReceipt(null); }, [address]);

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
  const totalAllUsd = useMemo(
    () => state.accounts.reduce((s, a) => s + (cachedBalances.get(a.address) ?? 0), 0),
    [state.accounts, cachedBalances],
  );

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
    if (address) setAccountBalance(address, totalUsd);
    const balances = await getAccountBalances(state.accounts.map((a) => a.address));
    if (address) balances.set(address, totalUsd);
    setCachedBalances(balances);
    setShowSwitcher(true);
    refreshAllBalances();
  }, [address, totalUsd, state.accounts, refreshAllBalances]);

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
    if (/^0x[0-9a-fA-F]{40}$/.test(data)) {
      router.push(`/send?prefilledRecipient=${data}`);
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

  const openDetail = (id: string) => {
    const t = txByIdRef.current.get(id);
    if (t) setDetailTx(t);
  };

  // Resolved alias for the open detail tx's counterparty.
  const detailAlias = (() => {
    if (!detailTx) return undefined;
    const cp = ((detailTx.type ?? 'send') === 'receive' ? detailTx.from : detailTx.to) ?? '';
    return detailTx.toName ?? aliasMap.get(cp.toLowerCase());
  })();

  // --- renderers ---
  const renderHeader = () => (
    <Animated.View entering={fadeInDown(60, 400)}>
      {/* Balance */}
      <Animated.View style={balanceScaleStyle}>
        <VelaCard elevated style={styles.balanceCard}>
          <View pointerEvents="none" style={styles.balanceBlob} />
          <Text style={styles.balanceLabel}>{t('home.totalBalance')}</Text>
          <View style={styles.balanceTopRow}>
            {hidden ? <Text style={styles.balanceHidden}>••••••</Text> : <Balance value={totalUsd * rate} symbol={currency.symbol} code={currencyCode} />}
            <Pressable onPress={() => setHidden((h) => !h)} hitSlop={8} style={styles.eyeBtn}>
              {hidden ? <EyeOff size={22} color={color.fg.muted} strokeWidth={2} /> : <Eye size={22} color={color.fg.muted} strokeWidth={2} />}
            </Pressable>
          </View>
          <View style={styles.balanceBottomRow}>
            <Pressable style={styles.currencyChip} onPress={() => setShowCurrency(true)} hitSlop={6}>
              <Text style={styles.currencyText}>{currencyCode}</Text>
              <ChevronDown size={14} color={color.fg.muted} strokeWidth={2.4} />
            </Pressable>
            <Pressable style={styles.manageBtn} onPress={() => router.push('/assets')} hitSlop={6}>
              <Text style={styles.manageText}>{t('home.manageAssets')}</Text>
              <ChevronRight size={18} color={color.fg.muted} strokeWidth={2.6} />
            </Pressable>
          </View>
          <Animated.View pointerEvents="none" style={[styles.balanceRing, balanceRingStyle]} />
        </VelaCard>
      </Animated.View>

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

      {/* Live monitoring indicator */}
      {tab === 'activity' && (
        <View style={styles.liveRow}>
          <Animated.View style={[styles.liveDot, liveDotStyle]} />
          <Text style={styles.liveText}>{t('home.liveIndicator')}</Text>
        </View>
      )}
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
            <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2.4} />
          </Pressable>
          <Pressable style={styles.iconBtn} onPress={() => router.navigate('/settings')} hitSlop={6}>
            <Settings size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </Animated.View>

        {/* Content */}
        {tab === 'activity' ? (
          <FlatList
            data={filteredActivity}
            keyExtractor={(item) => item.id}
            ListHeaderComponent={renderHeader()}
            ListEmptyComponent={renderActivityEmpty()}
            renderItem={({ item, index }) => (
              <ActivityRow
                direction={item.direction}
                title={t(item.direction === 'out' ? 'activity.sent' : 'activity.received')}
                subtitle={item.address
                  ? t(item.direction === 'out' ? 'activity.toAddr' : 'activity.fromAddr', { addr: shortAddress(item.address) })
                  : item.subtitle}
                amount={item.amount}
                fiat={item.usdValue > 0 ? formatFiat(item.usdValue * rate, currencyCode, currency.symbol) : undefined}
                time={relativeTime(item.timestamp)}
                alias={item.alias ?? (item.address ? aliasMap.get(item.address.toLowerCase()) : undefined)}
                chain={chainFor(item.chainId)}
                index={index}
                isNew={item.id === newItemId}
                onPress={() => openDetail(item.id)}
              />
            )}
            ItemSeparatorComponent={() => <View style={styles.sep} />}
            contentContainerStyle={styles.listContent}
            showsVerticalScrollIndicator={false}
            refreshControl={
              <RefreshControl refreshing={refreshing} onRefresh={onRefresh} tintColor={color.accent.base} />
            }
          />
        ) : (
          <ScrollView
            contentContainerStyle={styles.listContent}
            showsVerticalScrollIndicator={false}
            refreshControl={
              <RefreshControl refreshing={refreshing} onRefresh={onRefresh} tintColor={color.accent.base} />
            }
          >
            {renderHeader()}
            <ConnectionsView
              status={conn.status}
              dappName={conn.dappInfo?.name ?? null}
              dappUrl={conn.dappInfo?.url ?? null}
              events={connEvents}
              onDisconnect={conn.disconnectBridge}
              onConnect={() => setShowScanner(true)}
              onPasteConnect={onPasteConnect}
              onOpenEvent={setEventTx}
            />
          </ScrollView>
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
        visible={detailTx !== null}
        tx={detailTx}
        alias={detailAlias}
        rate={rate}
        currency={currency}
        onClose={() => setDetailTx(null)}
      />

      {/* dApp signing-record detail (message / typed-data / transaction) */}
      <ConnectionEventDetailSheet
        visible={eventTx !== null}
        tx={eventTx}
        onClose={() => setEventTx(null)}
      />

      {/* Account switcher */}
      <AppModal visible={showSwitcher} onClose={() => setShowSwitcher(false)}>
        <View style={styles.switcher}>
          <View style={styles.switcherHead}>
            <View style={styles.switcherHeadInfo}>
              <Text style={styles.switcherTitle}>{t('home.switchAccountTitle')}</Text>
              <View style={styles.switcherTotalRow}>
                <Text style={styles.switcherTotalLabel}>
                  {t('home.switcherAccountCount', { count: state.accounts.length })}
                </Text>
                <Text style={styles.switcherTotalValue}>{formatFiat(totalAllUsd * rate, currencyCode, currency.symbol)}</Text>
                {switcherLoading && <ActivityIndicator size="small" color={color.fg.subtle} style={styles.switcherSpinner} />}
              </View>
            </View>
            <Pressable onPress={() => setShowSwitcher(false)} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
          </View>
          <ScrollView contentContainerStyle={styles.switcherList}>
            {state.accounts
              .map((account, index) => ({ account, index }))
              .sort((a, b) => (cachedBalances.get(b.account.address) ?? -1) - (cachedBalances.get(a.account.address) ?? -1))
              .map(({ account, index }) => {
                const isActive = index === state.activeAccountIndex;
                const bal = cachedBalances.get(account.address);
                return (
                  <Pressable
                    key={account.id}
                    style={[styles.switcherItem, isActive && styles.switcherItemActive]}
                    onPress={() => { dispatch({ type: 'SWITCH_ACCOUNT', index }); hapticSuccess(); setShowSwitcher(false); }}
                  >
                    <View style={styles.switcherAvatar}><Text style={styles.switcherAvatarText}>{(account.name[0] ?? 'V').toUpperCase()}</Text></View>
                    <View style={styles.switcherInfo}>
                      <Text style={styles.switcherName} numberOfLines={1}>{account.name}</Text>
                      <Text style={styles.switcherAddr}>{shortAddress(account.address)}</Text>
                    </View>
                    <View style={styles.switcherRight}>
                      {bal != null
                        ? <Text style={styles.switcherBal}>{formatFiat(bal * rate, currencyCode, currency.symbol)}</Text>
                        : switcherLoading ? <ActivityIndicator size="small" color={color.fg.subtle} /> : null}
                      {isActive && <Check size={18} color={color.accent.base} />}
                    </View>
                  </Pressable>
                );
              })}
          </ScrollView>
        </View>
      </AppModal>

      {showScanner && (
        <QRScanner visible={showScanner} onScan={onScan} onClose={() => setShowScanner(false)} />
      )}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Connections view
// ---------------------------------------------------------------------------

function ConnectionsView({
  status, dappName, dappUrl, events, onDisconnect, onConnect, onPasteConnect, onOpenEvent,
}: {
  status: ConnectionStatus;
  dappName: string | null;
  dappUrl: string | null;
  events: ConnectionEvent[];
  onDisconnect: () => void;
  onConnect: () => void;
  onPasteConnect: (uri: string) => void;
  onOpenEvent: (tx: LocalTransaction) => void;
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
        <Pressable style={styles.connectBtn} onPress={onConnect}>
          <Text style={styles.connectBtnText}>{t('home.connScanBtn')}</Text>
        </Pressable>

        {/* or — paste a pairing URI when scanning isn't handy */}
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
        <Text style={styles.connNote}>{t('home.connNote')}</Text>
        <Pressable style={styles.disconnectBtn} onPress={onDisconnect}>
          <Text style={styles.disconnectText}>{t('home.connDisconnect')}</Text>
        </Pressable>
      </VelaCard>

      <Text style={styles.connEventsHead}>{t('home.connEventsHead', { count: events.length })}</Text>
      {events.length === 0 ? (
        <Text style={styles.connNoEvents}>{t('home.connNoEvents')}</Text>
      ) : (
        events.map((e) => (
          <Pressable key={e.id} style={styles.eventRow} onPress={() => onOpenEvent(e.tx)}>
            <View style={styles.eventInfo}>
              <Text style={styles.eventLabel} numberOfLines={1}>{e.label}</Text>
              <Text style={styles.eventSub} numberOfLines={1}>{e.subtitle}</Text>
            </View>
            <Text style={styles.eventTime}>{relativeTime(e.timestamp)}</Text>
            <ChevronRight size={16} color={color.fg.subtle} strokeWidth={2} />
          </Pressable>
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

  // Live indicator
  liveRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm, marginBottom: space.lg, paddingHorizontal: space.xs },
  liveDot: { width: 8, height: 8, borderRadius: 4, backgroundColor: color.success.base },
  liveText: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted, letterSpacing: 0.2 },

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

  // Account switcher
  switcher: { flex: 1, backgroundColor: color.bg.base },
  switcherHead: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space['3xl'], paddingVertical: space.xl,
    borderBottomWidth: 1, borderBottomColor: color.border.base,
  },
  switcherHeadInfo: { flex: 1, gap: 2 },
  switcherTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  switcherTotalRow: { flexDirection: 'row', alignItems: 'center' },
  switcherTotalLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },
  switcherTotalValue: { fontSize: text.sm, ...inter.bold, color: color.fg.base },
  switcherSpinner: { marginLeft: space.sm },
  switcherList: { padding: space['3xl'], gap: space.lg },
  switcherItem: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    padding: space.xl, backgroundColor: color.bg.raised,
    borderRadius: radius.xl, borderWidth: 1, borderColor: color.border.base, ...shadow.sm,
  },
  switcherItemActive: { borderColor: color.accent.base, borderWidth: 1.5 },
  switcherAvatar: { width: 40, height: 40, borderRadius: 20, backgroundColor: color.accent.soft, alignItems: 'center', justifyContent: 'center' },
  switcherAvatarText: { fontSize: text.lg, ...inter.semibold, color: color.accent.base },
  switcherInfo: { flex: 1, gap: 2 },
  switcherName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  switcherAddr: { fontSize: text.sm, ...inter.medium, fontFamily: font.mono, color: color.fg.subtle },
  switcherRight: { marginLeft: 'auto', alignItems: 'flex-end', gap: 4 },
  switcherBal: { fontSize: text.sm, ...inter.bold, color: color.fg.base },

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
  disconnectBtn: {
    marginTop: space.lg, alignItems: 'center',
    paddingVertical: space.lg, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base, backgroundColor: color.bg.raised,
  },
  disconnectText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  connEventsHead: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle, textTransform: 'uppercase', letterSpacing: 0.8, marginBottom: space.md },
  connNoEvents: { fontSize: text.base, ...inter.regular, color: color.fg.subtle },
  eventRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.lg, borderBottomWidth: 1, borderBottomColor: color.border.base,
  },
  eventInfo: { flex: 1, gap: 2 },
  eventLabel: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  eventSub: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  eventTime: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },

  connEmpty: { alignItems: 'center', paddingTop: space['4xl'], gap: space.md },
  connEmptyIcon: { width: 56, height: 56, borderRadius: 28, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },
  connEmptyTitle: { fontSize: text.xl, ...inter.semibold, color: color.fg.base },
  connEmptySub: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20, paddingHorizontal: space.xl },
  connectBtn: { marginTop: space.md, backgroundColor: color.accent.base, borderRadius: radius.lg, paddingVertical: space.lg, paddingHorizontal: space['3xl'] },
  connectBtnText: { fontSize: text.base, ...inter.semibold, color: color.fg.inverse },
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
