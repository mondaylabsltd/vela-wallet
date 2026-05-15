import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { TokenRow } from '@/components/ui/TokenRow';
import { VelaCard } from '@/components/ui/VelaCard';
import { AppModal } from '@/components/ui/AppModal';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, motion, radius, shadow, space, text } from '@/constants/theme';
import { chainName, getAllNetworksSync } from '@/models/network';
import type { Network } from '@/models/network';
import { formatBalance, shortAddr, tokenBalanceDouble, tokenChainId, tokenLogoURLs, tokenUsdValue, type APIToken } from '@/models/types';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { fetchTokens } from '@/services/wallet-api';
import { saveNetworkConfig } from '@/services/storage';
import { refreshPool } from '@/services/rpc-pool';
import { setAccountBalance, getAccountBalances } from '@/services/balance-cache';
import { showAlert, copyToClipboard, hapticSuccess, isAppActive } from '@/services/platform';
import { ChainLogo } from '@/components/ChainLogo';
import { QRScanner } from '@/components/QRScanner';
import { useFocusEffect } from '@react-navigation/native';
import { useRouter } from 'expo-router';
import { ArrowDown, ArrowUp, Check, Clock, Copy, Plus, ChevronDown, Search, X, AlertTriangle, Wifi, RefreshCw, ScanLine } from 'lucide-react-native';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { ActivityIndicator, FlatList, Platform, Pressable, RefreshControl, ScrollView, Text, TextInput, View } from 'react-native';
import Animated, {
  useAnimatedProps,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
  withRepeat,
  Easing,
  cancelAnimation,
} from 'react-native-reanimated';

const AUTO_REFRESH_MS = 10 * 60 * 1000;

const AnimatedTextInput = Animated.createAnimatedComponent(TextInput);

/**
 * Smoothly animates between USD values instead of snapping.
 * Uses Reanimated's useAnimatedProps to drive a TextInput on the UI thread.
 * On web, useAnimatedProps can't set `text` on inputs, so we fall back to plain Text.
 */
function AnimatedBalance({ value }: { value: number }) {
  const fontSize = balanceFontSize(value);

  if (Platform.OS === 'web') {
    return (
      <View style={styles.balanceRow}>
        <Text style={[styles.balanceInt, { fontSize }]}>{formatUsdInt(value)}</Text>
        <Text style={[styles.balanceDec, { fontSize: fontSize * 0.58 }]}>{formatUsdDec(value)}</Text>
      </View>
    );
  }

  return <AnimatedBalanceNative value={value} fontSize={fontSize} />;
}

function AnimatedBalanceNative({ value, fontSize }: { value: number; fontSize: number }) {
  const displayed = useSharedValue(value);

  useEffect(() => {
    displayed.value = withTiming(value, {
      duration: 800,
      easing: Easing.out(Easing.quad),
    });
  }, [value, displayed]);

  const intProps = useAnimatedProps(() => {
    'worklet';
    const v = displayed.value;
    const full = '$' + Math.abs(v).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    const dot = full.indexOf('.');
    return { text: dot === -1 ? full : full.slice(0, dot) } as any;
  });

  const decProps = useAnimatedProps(() => {
    'worklet';
    const v = displayed.value;
    const full = Math.abs(v).toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
    const dot = full.indexOf('.');
    return { text: dot === -1 ? '.00' : full.slice(full.indexOf('.')) } as any;
  });

  return (
    <View style={styles.balanceRow}>
      <AnimatedTextInput
        editable={false}
        underlineColorAndroid="transparent"
        style={[styles.balanceInt, { fontSize }]}
        animatedProps={intProps}
        defaultValue={formatUsdInt(value)}
      />
      <AnimatedTextInput
        editable={false}
        underlineColorAndroid="transparent"
        style={[styles.balanceDec, { fontSize: fontSize * 0.58 }]}
        animatedProps={decProps}
        defaultValue={formatUsdDec(value)}
      />
    </View>
  );
}

function formatUsd(value: number): string {
  return '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function formatUsdInt(value: number): string {
  const full = formatUsd(value);
  const dot = full.indexOf('.');
  return dot === -1 ? full : full.slice(0, dot);
}

function formatUsdDec(value: number): string {
  const full = formatUsd(value);
  const dot = full.indexOf('.');
  return dot === -1 ? '.00' : full.slice(dot);
}

function balanceFontSize(usd: number): number {
  const len = formatUsdInt(usd).length;
  if (len <= 7) return 36;
  if (len <= 9) return 30;
  if (len <= 12) return 26;
  if (len <= 15) return 22;
  return 18;
}

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

function ActionButton({ label, icon: Icon, onPress, accent }: { label: string; icon: React.ComponentType<any>; onPress: () => void; accent?: boolean }) {
  const scale = useSharedValue(1);
  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  return (
    <AnimatedPressable
      style={[styles.actionBtn, accent && styles.actionBtnAccent, animatedStyle]}
      onPress={onPress}
      onPressIn={() => { scale.value = withSpring(0.95, motion.spring); }}
      onPressOut={() => { scale.value = withSpring(1, motion.spring); }}
    >
      <View style={[styles.actionIconWrap, accent && styles.actionIconWrapAccent]}>
        <Icon size={18} color={accent ? color.fg.inverse : color.fg.base} strokeWidth={2.5} />
      </View>
      <Text style={[styles.actionLabel, accent && styles.actionLabelAccent]}>{label}</Text>
    </AnimatedPressable>
  );
}

export default function HomeScreen() {
  const router = useRouter();
  const { activeAccount, state, dispatch } = useWallet();

  const [tokens, setTokens] = useState<APIToken[]>([]);
  const [debugBalance, setDebugBalance] = useState<number | null>(null);
  const [showScanner, setShowScanner] = useState(false);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);

  // Spinning refresh icon for web
  const spinRotation = useSharedValue(0);
  useEffect(() => {
    if (refreshing) {
      spinRotation.value = 0;
      spinRotation.value = withRepeat(
        withTiming(360, { duration: 800, easing: Easing.linear }),
        -1,
      );
    } else {
      cancelAnimation(spinRotation);
      spinRotation.value = withTiming(0, { duration: 200 });
    }
  }, [refreshing, spinRotation]);
  const spinStyle = useAnimatedStyle(() => ({
    transform: [{ rotate: `${spinRotation.value}deg` }],
  }));

  // Web pull-to-refresh state
  const flatListRef = useRef<FlatList>(null);
  const pullStartY = useRef<number | null>(null);
  const pullDistanceRef = useRef(0);
  const [pullDistance, setPullDistance] = useState(0);
  const PULL_THRESHOLD = 60;

  const loadInFlightRef = useRef(false);
  const [failedChainIds, setFailedChainIds] = useState<number[]>([]);
  const [rpcFixChainId, setRpcFixChainId] = useState<number | null>(null);
  const [rpcFixUrl, setRpcFixUrl] = useState('');
  const [rpcFixSaving, setRpcFixSaving] = useState(false);

  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const loadTokens = useCallback(async (silent = false, forceRefresh = false) => {
    if (!address) {
      setLoading(false);
      setRefreshing(false);
      return;
    }
    if (loadInFlightRef.current) {
      if (!silent) setRefreshing(false);
      return;
    }
    loadInFlightRef.current = true;
    if (!silent) setLoading(true);
    try {
      const result = await fetchTokens(address, {
        forceRefresh,
        onProgress: (partial) => {
          // Merge: replace tokens from chains that have new data,
          // keep old tokens from chains that haven't responded yet.
          // This prevents the total from dropping to zero during refresh.
          setTokens(prev => {
            const freshChains = new Set(partial.map(t => tokenChainId(t)));
            const kept = prev.filter(t => !freshChains.has(tokenChainId(t)));
            return [...kept, ...partial].sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
          });
          setLoading(false);
        },
        onFailedChains: (ids) => setFailedChainIds(ids),
      });
      setTokens(result);
      // Cache total USD for this account
      const usd = result.reduce((s, t) => s + tokenUsdValue(t), 0);
      setAccountBalance(address, usd);
    } catch (err) {
      if (!silent) {
        showAlert('Error', 'Failed to load token balances.');
      }
    } finally {
      loadInFlightRef.current = false;
      setLoading(false);
      setRefreshing(false);
    }
  }, [address]);

  useFocusEffect(useCallback(() => {
    loadTokens();
    const timer = setInterval(() => {
      if (isAppActive()) {
        loadTokens(true);
      }
    }, AUTO_REFRESH_MS);
    return () => clearInterval(timer);
  }, [loadTokens]));

  const onRefresh = useCallback(() => {
    setRefreshing(true);
    loadTokens(false, true);
  }, [loadTokens]);

  // Web pull-to-refresh: attach native DOM touch listeners
  useEffect(() => {
    if (Platform.OS !== 'web' || !flatListRef.current) return;
    const node = (flatListRef.current as any)?.getScrollableNode?.()
      ?? (flatListRef.current as any)?._listRef?._scrollRef
      ?? (flatListRef.current as any);
    const el: HTMLElement | null = node instanceof HTMLElement ? node : null;
    if (!el) return;

    const getScrollTop = () => {
      let target: HTMLElement | null = el;
      while (target) {
        if (target.scrollHeight > target.clientHeight) return target.scrollTop;
        target = target.parentElement;
      }
      return 0;
    };

    const handleTouchStart = (e: TouchEvent) => {
      if (getScrollTop() <= 0) {
        pullStartY.current = e.touches[0].clientY;
      }
    };
    const handleTouchMove = (e: TouchEvent) => {
      if (pullStartY.current === null) return;
      const dist = Math.max(0, (e.touches[0].clientY - pullStartY.current) * 0.5);
      pullDistanceRef.current = dist;
      setPullDistance(dist);
    };
    const handleTouchEnd = () => {
      if (pullDistanceRef.current >= PULL_THRESHOLD) {
        onRefresh();
      }
      pullStartY.current = null;
      pullDistanceRef.current = 0;
      setPullDistance(0);
    };

    el.addEventListener('touchstart', handleTouchStart, { passive: true });
    el.addEventListener('touchmove', handleTouchMove, { passive: true });
    el.addEventListener('touchend', handleTouchEnd);
    return () => {
      el.removeEventListener('touchstart', handleTouchStart);
      el.removeEventListener('touchmove', handleTouchMove);
      el.removeEventListener('touchend', handleTouchEnd);
    };
  }, [onRefresh]);

  const totalUsd = tokens.reduce((sum, t) => sum + tokenUsdValue(t), 0);

  // --- RPC fix modal helpers ---
  const failedNetworks = failedChainIds
    .map(id => getAllNetworksSync().find(n => n.chainId === id))
    .filter((n): n is Network => !!n);

  const openRpcFix = (chainId: number) => {
    const net = getAllNetworksSync().find(n => n.chainId === chainId);
    setRpcFixChainId(chainId);
    setRpcFixUrl(net?.rpcURL ?? '');
  };

  const handleRpcFixSave = async () => {
    if (!rpcFixChainId || !rpcFixUrl.trim()) return;
    setRpcFixSaving(true);
    try {
      const net = getAllNetworksSync().find(n => n.chainId === rpcFixChainId);
      await saveNetworkConfig({
        chainId: rpcFixChainId,
        rpcURL: rpcFixUrl.trim(),
        explorerURL: net?.explorerURL ?? '',
        bundlerURL: net?.bundlerURL ?? '',
      });
      await refreshPool(rpcFixChainId);
      setFailedChainIds(prev => prev.filter(id => id !== rpcFixChainId));
      setRpcFixChainId(null);
      loadTokens(true, true);
    } catch {
      showAlert('Error', 'Failed to save RPC URL.');
    } finally {
      setRpcFixSaving(false);
    }
  };

  const [showAccountSwitcher, setShowAccountSwitcher] = useState(false);
  const [cachedBalances, setCachedBalances] = useState<Map<string, number>>(new Map());

  const openAccountSwitcher = useCallback(async () => {
    // Update active account's cache with live data before opening
    if (address) setAccountBalance(address, totalUsd);
    const addrs = state.accounts.map(a => a.address);
    const balances = await getAccountBalances(addrs);
    // Also set the live balance for current account in case cache was stale
    if (address) balances.set(address, totalUsd);
    setCachedBalances(balances);
    setShowAccountSwitcher(true);
  }, [address, totalUsd, state.accounts]);
  const [tokenSearch, setTokenSearch] = useState('');

  const [copied, setCopied] = useState(false);
  const copyAddress = async () => {
    if (!address) return;
    await copyToClipboard(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const navigateToToken = (token: APIToken) => {
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
  };

  const renderHeader = () => (
    <View style={styles.header}>
      {/* Web pull-to-refresh indicator */}
      {Platform.OS === 'web' && pullDistance > 0 && (
        <View style={styles.webPullIndicator}>
          <Animated.View style={pullDistance >= PULL_THRESHOLD ? spinStyle : undefined}>
            <RefreshCw
              size={18}
              color={pullDistance >= PULL_THRESHOLD ? color.accent.base : color.fg.subtle}
              strokeWidth={2.5}
            />
          </Animated.View>
        </View>
      )}
      {/* Account chip */}
      <Animated.View entering={fadeIn(0, 400)}>
        <View style={styles.accountChipWrap}>
          <Pressable
            style={styles.accountChip}
            onPress={state.accounts.length > 1 ? openAccountSwitcher : copyAddress}
            onLongPress={copyAddress}
          >
            <View style={styles.accountAvatar}>
              <Text style={styles.accountAvatarText}>
                {(accountName[0] ?? 'V').toUpperCase()}
              </Text>
            </View>
            <View style={styles.accountTextGroup}>
              <Text style={styles.accountName}>{accountName}</Text>
              <View style={styles.addrRow}>
                <Text style={styles.accountAddr}>{shortAddr(address)}</Text>
                {copied ? (
                  <Check size={10} color={color.accent.base} strokeWidth={3} />
                ) : state.accounts.length > 1 ? (
                  <ChevronDown size={10} color={color.fg.subtle} strokeWidth={2.5} />
                ) : (
                  <Copy size={10} color={color.fg.subtle} strokeWidth={2} />
                )}
              </View>
            </View>
          </Pressable>
        </View>
      </Animated.View>

      {/* Hero balance — long press to cycle mock values (dev only) */}
      <Pressable
        onLongPress={() => {
          if (!__DEV__) return;
          const mocks = [0.42, 9.99, 150.50, 1234.56, 98765.43, 1234567.89, 12345678.90, 123456789.00, 1234567890.00, 12345678901.00, 123456789012.00, 1234567890123.00, 0];
          const cur = mocks.findIndex(v => v === debugBalance);
          setDebugBalance(mocks[(cur + 1) % mocks.length]!);
        }}
        delayLongPress={500}
      >
        <Animated.View style={styles.balanceSection} entering={fadeInDown(100, 500)}>
          <AnimatedBalance value={debugBalance ?? totalUsd} />
        </Animated.View>
      </Pressable>

      {/* Action buttons */}
      <Animated.View style={styles.actionRow} entering={fadeInDown(200, 400)}>
        <ActionButton label="Send" icon={ArrowUp} onPress={() => router.push('/send')} accent />
        <ActionButton label="Scan" icon={ScanLine} onPress={() => setShowScanner(true)} />
        <ActionButton label="Receive" icon={ArrowDown} onPress={() => router.push('/receive')} />
        <ActionButton label="History" icon={Clock} onPress={() => router.push('/history')} />
      </Animated.View>

      {/* Token list header */}
      <View style={styles.tokenListHeader}>
        <Text style={styles.tokenListTitle}>Assets</Text>
        <View style={styles.tokenListActions}>
          {Platform.OS === 'web' && (
            <Pressable
              style={styles.searchToggleBtn}
              onPress={onRefresh}
              hitSlop={8}
            >
              <Animated.View style={spinStyle}>
                <RefreshCw size={14} color={refreshing ? color.accent.base : color.fg.muted} strokeWidth={2.5} />
              </Animated.View>
            </Pressable>
          )}
          <Pressable
            style={styles.addTokenBtn}
            onPress={() => router.push('/add-token')}
            hitSlop={8}
          >
            <Plus size={14} color={color.accent.base} strokeWidth={2.5} />
            <Text style={styles.addTokenText}>Add</Text>
          </Pressable>
        </View>
      </View>

      {/* Search bar — always visible */}
      <View style={styles.searchBar}>
        <Search size={14} color={color.fg.subtle} strokeWidth={2} />
        <TextInput
          style={styles.searchInput}
          placeholder="Search tokens..."
          placeholderTextColor={color.fg.subtle}
          value={tokenSearch}
          onChangeText={setTokenSearch}
          autoCapitalize="none"
          autoCorrect={false}
        />
      </View>

      {/* RPC failure banner */}
      {failedNetworks.length > 0 && (
        <Animated.View entering={fadeInDown(0, 300)} style={styles.rpcBanner}>
          <AlertTriangle size={14} color={'#C07A0A'} strokeWidth={2.5} />
          <View style={styles.rpcBannerContent}>
            <Text style={styles.rpcBannerText}>
              {failedNetworks.length === 1
                ? `${failedNetworks[0].displayName} RPC unavailable`
                : `${failedNetworks.length} networks RPC unavailable`}
            </Text>
            <View style={styles.rpcBannerChips}>
              {failedNetworks.map(net => (
                <Pressable
                  key={net.chainId}
                  style={styles.rpcBannerChip}
                  onPress={() => openRpcFix(net.chainId)}
                >
                  <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />
                  <Text style={styles.rpcBannerChipText}>{net.displayName}</Text>
                  <Text style={styles.rpcBannerFixLink}>Fix</Text>
                </Pressable>
              ))}
            </View>
          </View>
        </Animated.View>
      )}
    </View>
  );

  const renderEmpty = () => {
    if (loading) return null;
    return (
      <Pressable onPress={() => router.push('/receive')}>
        <VelaCard style={styles.emptyCard}>
          <View style={styles.emptyIconWrap}>
            <ArrowDown size={22} color={color.accent.base} strokeWidth={2.5} />
          </View>
          <Text style={styles.emptyTitle}>Deposit your first asset</Text>
          <Text style={styles.emptySubtext}>
            Tap here to see your address and receive tokens
          </Text>
        </VelaCard>
      </Pressable>
    );
  };

  const filteredTokens = tokenSearch
    ? tokens.filter(t =>
        t.symbol.toLowerCase().includes(tokenSearch.toLowerCase()) ||
        t.name.toLowerCase().includes(tokenSearch.toLowerCase())
      )
    : tokens;

  return (
    <ScreenContainer>
      <FlatList
        ref={flatListRef}
        data={filteredTokens}
        keyExtractor={(item) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
        ListHeaderComponent={renderHeader()}
        ListEmptyComponent={renderEmpty()}
        renderItem={({ item, index }) => (
          <TokenRow
            symbol={item.symbol}
            chainLabel={chainName(tokenChainId(item))}
            logoUrls={tokenLogoURLs(item)}
            balance={formatBalance(tokenBalanceDouble(item))}
            usdValue={tokenUsdValue(item) > 0 ? formatUsd(tokenUsdValue(item)) : undefined}
            onPress={() => navigateToToken(item)}
            index={index}
          />
        )}
        refreshControl={
          Platform.OS !== 'web' ? (
            <RefreshControl
              refreshing={refreshing}
              onRefresh={onRefresh}
              tintColor={color.accent.base}
            />
          ) : undefined
        }
        initialNumToRender={10}
        windowSize={5}
        maxToRenderPerBatch={8}
        contentContainerStyle={styles.listContent}
        showsVerticalScrollIndicator={false}
      />

      {/* Account Switcher */}
      <AppModal visible={showAccountSwitcher} onClose={() => setShowAccountSwitcher(false)}>
        <View style={styles.switcherContainer}>
          <View style={styles.switcherHeader}>
            <View>
              <Text style={styles.switcherTitle}>Switch Account</Text>
              {cachedBalances.size > 0 && (
                <Text style={styles.switcherTotal}>
                  Total {formatUsd([...cachedBalances.values()].reduce((s, v) => s + v, 0))}
                </Text>
              )}
            </View>
            <Pressable onPress={() => setShowAccountSwitcher(false)} hitSlop={8}>
              <X size={22} color={color.fg.base} strokeWidth={2} />
            </Pressable>
          </View>
          <ScrollView style={styles.switcherScroll} contentContainerStyle={styles.switcherScrollContent}>
            {state.accounts
              .map((account, index) => ({ account, index }))
              .sort((a, b) => {
                const balA = cachedBalances.get(a.account.address) ?? -1;
                const balB = cachedBalances.get(b.account.address) ?? -1;
                if (balB !== balA) return balB - balA;
                return a.account.name.localeCompare(b.account.name);
              })
              .map(({ account, index }) => {
              const isActive = index === state.activeAccountIndex;
              const bal = cachedBalances.get(account.address);
              return (
                <Pressable
                  key={account.id}
                  style={[styles.switcherItem, isActive && styles.switcherItemActive]}
                  onPress={() => {
                    dispatch({ type: 'SWITCH_ACCOUNT', index });
                    hapticSuccess();
                    setShowAccountSwitcher(false);
                  }}
                >
                  <View style={styles.switcherAvatar}>
                    <Text style={styles.switcherAvatarText}>{(account.name[0] ?? 'V').toUpperCase()}</Text>
                  </View>
                  <View style={styles.switcherInfo}>
                    <Text style={styles.switcherName}>{account.name}</Text>
                    <Text style={styles.switcherAddr}>{shortAddress(account.address)}</Text>
                  </View>
                  <View style={styles.switcherRight}>
                    {bal != null && <Text style={styles.switcherBal}>{formatUsd(bal)}</Text>}
                    {isActive && <Check size={18} color={color.accent.base} />}
                  </View>
                </Pressable>
              );
            })}
            <View style={styles.switcherEndLine} />
          </ScrollView>
        </View>
      </AppModal>

      {/* RPC Fix Modal */}
      <AppModal visible={rpcFixChainId !== null} onClose={() => setRpcFixChainId(null)}>
        {rpcFixChainId !== null && (() => {
          const net = getAllNetworksSync().find(n => n.chainId === rpcFixChainId);
          return (
            <View style={styles.rpcFixContainer}>
              <View style={styles.rpcFixHeader}>
                <Text style={styles.rpcFixTitle}>Fix RPC</Text>
                <Pressable onPress={() => setRpcFixChainId(null)} hitSlop={8}>
                  <X size={22} color={color.fg.base} strokeWidth={2} />
                </Pressable>
              </View>

              <View style={styles.rpcFixBody}>
                <View style={styles.rpcFixChainRow}>
                  {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={32} />}
                  <View>
                    <Text style={styles.rpcFixChainName}>{net?.displayName ?? `Chain ${rpcFixChainId}`}</Text>
                    <Text style={styles.rpcFixChainSub}>Chain ID: {rpcFixChainId}</Text>
                  </View>
                </View>

                <View style={styles.rpcFixWarning}>
                  <Wifi size={14} color={'#C07A0A'} strokeWidth={2.5} />
                  <Text style={styles.rpcFixWarningText}>
                    All RPC endpoints for this network are failing. Enter a working RPC URL to restore connectivity.
                  </Text>
                </View>

                <Text style={styles.rpcFixLabel}>RPC URL</Text>
                <TextInput
                  style={styles.rpcFixInput}
                  value={rpcFixUrl}
                  onChangeText={setRpcFixUrl}
                  placeholder="https://rpc.example.com"
                  placeholderTextColor={color.fg.subtle}
                  autoCapitalize="none"
                  autoCorrect={false}
                  autoFocus
                />

                <Pressable
                  style={[styles.rpcFixBtn, rpcFixSaving && styles.rpcFixBtnDisabled]}
                  onPress={handleRpcFixSave}
                  disabled={rpcFixSaving || !rpcFixUrl.trim()}
                >
                  {rpcFixSaving ? (
                    <ActivityIndicator size={16} color={color.fg.inverse} />
                  ) : (
                    <Text style={styles.rpcFixBtnText}>Save & Retry</Text>
                  )}
                </Pressable>
              </View>
            </View>
          );
        })()}
      </AppModal>
      {showScanner && (
        <QRScanner
          visible={showScanner}
          onScan={(addr) => {
            if (/^0x[0-9a-fA-F]{40}$/.test(addr)) {
              setShowScanner(false);
              router.push(`/send?prefilledRecipient=${addr}`);
            } else {
              showAlert('Invalid QR', 'Please scan a valid Ethereum address (0x...).');
            }
          }}
          onClose={() => setShowScanner(false)}
        />
      )}
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  listContent: {
    paddingBottom: 100,
  },
  webPullIndicator: {
    alignItems: 'center',
    justifyContent: 'center',
    paddingVertical: space.lg,
  },
  header: {
    paddingTop: space.xl,
    marginBottom: space.sm,
  },

  // Account chip
  accountChipWrap: {
    alignItems: 'center',
  },
  accountChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.sm,
    paddingHorizontal: space.lg,
    borderRadius: radius.full,
    backgroundColor: color.bg.sunken,
  },
  accountAvatar: {
    width: 24,
    height: 24,
    borderRadius: 12,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
  },
  accountAvatarText: {
    fontSize: text.xs,
    ...inter.bold,
    color: color.accent.base,
  },
  accountTextGroup: {
    gap: 0,
  },
  accountName: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
  },
  addrRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
  },
  accountAddr: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },

  // Balance
  balanceSection: {
    alignItems: 'center',
    marginTop: space['3xl'],
    marginBottom: space['2xl'],
  },
  balanceLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 1,
    marginBottom: space.sm,
  },
  balanceRow: {
    flexDirection: 'row',
    alignItems: 'baseline',
    justifyContent: 'center',
  },
  balanceInt: {
    fontSize: 36,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
    padding: 0,
  },
  balanceDec: {
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.subtle,
    padding: 0,
  },

  // Actions
  actionRow: {
    flexDirection: 'row',
    justifyContent: 'center',
    gap: space.xl,
    marginBottom: space['3xl'],
  },
  actionBtn: {
    alignItems: 'center',
    gap: space.md,
    minWidth: 72,
  },
  actionBtnAccent: {},
  actionIconWrap: {
    width: 48,
    height: 48,
    borderRadius: 24,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
    ...shadow.sm,
  },
  actionIconWrapAccent: {
    backgroundColor: color.accent.base,
    ...shadow.md,
  },
  actionLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.base,
  },
  actionLabelAccent: {
    ...inter.semibold,
  },

  // Token list header
  tokenListHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: space.md,
    paddingHorizontal: space.sm,
  },
  tokenListTitle: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
  },
  tokenListActions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
  },
  searchToggleBtn: {
    width: 32,
    height: 32,
    borderRadius: 16,
    alignItems: 'center',
    justifyContent: 'center',
  },
  addTokenBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
    paddingVertical: space.sm,
    paddingHorizontal: space.md,
  },
  addTokenText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
  },

  // Search
  searchBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    marginBottom: space.lg,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  searchInput: {
    flex: 1,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    paddingVertical: space.xs,
  },

  // Account Switcher Modal
  switcherContainer: {
    flex: 1,
    backgroundColor: color.bg.base,
  },
  switcherHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space['3xl'],
    paddingVertical: space.xl,
    borderBottomWidth: 1,
    borderBottomColor: color.border.base,
  },
  switcherTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  switcherScroll: {
    paddingHorizontal: space['3xl'],
    paddingTop: space['3xl'],
  },
  switcherScrollContent: {
    paddingBottom: space['3xl'],
  },
  switcherEndLine: {
    height: 1,
    backgroundColor: color.border.base,
    marginTop: space.lg,
    marginBottom: space.xl,
  },
  switcherItem: {
    flexDirection: 'row',
    alignItems: 'center',
    padding: space.xl,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    marginBottom: space.lg,
    gap: space.lg,
    ...shadow.sm,
  },
  switcherItemActive: {
    borderColor: color.accent.base,
    borderWidth: 1.5,
  },
  switcherAvatar: {
    width: 40,
    height: 40,
    borderRadius: 20,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
  },
  switcherAvatarText: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.accent.base,
  },
  switcherInfo: {
    flex: 1,
    gap: 2,
  },
  switcherName: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
  },
  switcherAddr: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.subtle,
  },
  switcherTotal: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
    marginTop: 2,
  },
  switcherRight: {
    marginLeft: 'auto',
    alignItems: 'flex-end',
    gap: 4,
  },
  switcherBal: {
    fontSize: text.sm,
    ...inter.bold,
    color: color.fg.base,
  },

  // Empty
  emptyCard: {
    padding: space['4xl'],
    alignItems: 'center',
    gap: space.md,
  },
  emptyIconWrap: {
    width: 48,
    height: 48,
    borderRadius: 24,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  emptyTitle: {
    fontSize: text.xl,
    ...inter.semibold,
    color: color.fg.muted,
  },
  emptySubtext: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 20,
  },

  // RPC failure banner
  rpcBanner: {
    flexDirection: 'row',
    gap: space.md,
    padding: space.lg,
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.warning.border,
    marginBottom: space.lg,
  },
  rpcBannerContent: {
    flex: 1,
    gap: space.sm,
  },
  rpcBannerText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.warning.base,
  },
  rpcBannerChips: {
    gap: space.sm,
  },
  rpcBannerChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingVertical: space.xs,
  },
  rpcBannerChipText: {
    flex: 1,
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.base,
  },
  rpcBannerFixLink: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
  },

  // RPC Fix Modal
  rpcFixContainer: {
    flex: 1,
    backgroundColor: color.bg.base,
  },
  rpcFixHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space['3xl'],
    paddingVertical: space.xl,
    borderBottomWidth: 1,
    borderBottomColor: color.border.base,
  },
  rpcFixTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  rpcFixBody: {
    padding: space['3xl'],
    gap: space.xl,
  },
  rpcFixChainRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  rpcFixChainName: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
  },
  rpcFixChainSub: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  rpcFixWarning: {
    flexDirection: 'row',
    gap: space.md,
    padding: space.lg,
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.warning.border,
  },
  rpcFixWarningText: {
    flex: 1,
    fontSize: text.sm,
    ...inter.regular,
    color: color.warning.base,
    lineHeight: 18,
  },
  rpcFixLabel: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
  },
  rpcFixInput: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.lg,
    paddingHorizontal: space.lg,
    paddingVertical: space.lg,
    marginTop: -space.sm,
  },
  rpcFixBtn: {
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: color.accent.base,
    borderRadius: radius.lg,
    paddingVertical: space.lg,
    ...shadow.sm,
  },
  rpcFixBtnDisabled: {
    opacity: 0.5,
  },
  rpcFixBtnText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.inverse,
  },
}));
