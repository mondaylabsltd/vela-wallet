import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaRefresh } from '@/components/ui/VelaRefresh';
import { TokenRow } from '@/components/ui/TokenRow';
import { VelaCard } from '@/components/ui/VelaCard';
import { AppModal } from '@/components/ui/AppModal';
import { AmountText } from '@/components/ui/AmountText';
import { RpcTroubleBanner } from '@/components/ui/RpcTroubleBanner';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { shouldShowDecimals } from '@/services/currency';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, motion, radius, shadow, space, text } from '@/constants/theme';
import { useTranslation } from 'react-i18next';
import { chainName } from '@/models/network';
import { shortAddr, isAddress, tokenBalanceDouble, tokenChainId, tokenLogoURLs, tokenUsdValue, type APIToken } from '@/models/types';
import { formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { fetchTokens } from '@/services/wallet-api';
import { setAccountBalance, getAccountBalance, getAccountBalances } from '@/services/balance-cache';
import { showAlert, hapticSuccess, isAppActive } from '@/services/platform';
import { useCopyFeedback } from '@/hooks/use-copy-feedback';
import { QRScanner } from '@/components/QRScanner';
import { useDAppConnection } from '@/models/dapp-connection';
import { isWalletPairURI } from '@/services/walletpair-transport';
import { parseRemoteInjectURL } from '@/services/dapp-transport';
import { useFocusEffect } from '@react-navigation/native';
import { useRouter } from 'expo-router';
import { ArrowDown, ArrowUp, Check, Clock, Plus, Search, X, RefreshCw, ScanLine, AlertTriangle } from 'lucide-react-native';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { Platform, Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
  withRepeat,
  Easing,
  cancelAnimation,
} from 'react-native-reanimated';

const AUTO_REFRESH_MS = 10 * 60 * 1000;

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

export default function AssetsScreen() {
  const router = useRouter();
  const { t } = useTranslation();
  useLocalePrefs(); // re-render when the number format changes
  const { activeAccount, state, dispatch } = useWallet();
  const { connectToWalletPair, connectToBridge } = useDAppConnection();
  const dc = useDisplayCurrency();

  const [tokens, setTokens] = useState<APIToken[]>([]);
  const [allTokens, setAllTokens] = useState<APIToken[]>([]);
  const [showZeroBalance, setShowZeroBalance] = useState(false);
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

  const loadInFlightRef = useRef(false);
  const [failedChainIds, setFailedChainIds] = useState<number[]>([]);
  const [cachedTotal, setCachedTotal] = useState<number | null>(null);

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
      let failed: number[] = [];
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
        onFailedChains: (ids) => { failed = ids; setFailedChainIds(ids); },
      });
      setTokens(result);
      setFailedChainIds(failed);
      // Fetch all tokens (including zero balance) for hidden count
      fetchTokens(address, { includeZeroBalance: true }).then(all => setAllTokens(all)).catch(() => {});
      // Only persist the total as "last known good" when it's complete — no
      // failed chains and every held token priced — so a partial read never
      // poisons the cache that the home screen falls back to.
      const unpriced = result.some(t => tokenBalanceDouble(t) > 0 && t.priceUsd == null);
      if (failed.length === 0 && !unpriced) {
        const usd = result.reduce((s, t) => s + tokenUsdValue(t), 0);
        setAccountBalance(address, usd);
        setCachedTotal(usd);
      }
    } catch (err) {
      if (!silent) {
        showAlert(t('assets.errorTitle'), t('assets.errorLoadBalances'));
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

  // Seed the hero from the cached total so it never flashes $0 while loading.
  useEffect(() => {
    let cancelled = false;
    setCachedTotal(null);
    if (address) getAccountBalance(address).then(v => { if (!cancelled && v != null) setCachedTotal(v); });
    return () => { cancelled = true; };
  }, [address]);

  const totalUsd = tokens.reduce((sum, t) => sum + tokenUsdValue(t), 0);

  // Partial detection + cache fallback (mirrors HomeScreen): never show a
  // confidently-wrong smaller total when a chain failed or a token is unpriced.
  const hasUnpriced = tokens.some(t => tokenBalanceDouble(t) > 0 && t.priceUsd == null);
  const hasLiveData = tokens.length > 0;
  const balancePartial = failedChainIds.length > 0 || (hasLiveData && hasUnpriced);
  const displayTotal =
    !hasLiveData && cachedTotal != null ? cachedTotal
    : balancePartial && cachedTotal != null ? Math.max(totalUsd, cachedTotal)
    : totalUsd;

  const [showAccountSwitcher, setShowAccountSwitcher] = useState(false);
  const [cachedBalances, setCachedBalances] = useState<Map<string, number>>(new Map());

  const openAccountSwitcher = useCallback(async () => {
    // Update active account's cache with live data before opening
    if (address) setAccountBalance(address, displayTotal);
    const addrs = state.accounts.map(a => a.address);
    const balances = await getAccountBalances(addrs);
    // Also set the live balance for current account in case cache was stale
    if (address) balances.set(address, displayTotal);
    setCachedBalances(balances);
    setShowAccountSwitcher(true);
  }, [address, displayTotal, state.accounts]);
  const [tokenSearch, setTokenSearch] = useState('');

  const { copied, copy } = useCopyFeedback();
  const copyAddress = () => {
    if (!address) return;
    copy(address);
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
      {/* Close + account + scan */}
      <Animated.View entering={fadeIn(0, 400)}>
        <View style={styles.headerTopRow}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.closeBtn}>
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Pressable
            style={styles.accountInfo}
            onPress={state.accounts.length > 1 ? openAccountSwitcher : copyAddress}
          >
            <Text style={styles.accountName} numberOfLines={1}>{accountName}</Text>
            <Text style={styles.accountAddr}>{shortAddr(address)}</Text>
          </Pressable>
          <Pressable onPress={() => setShowScanner(true)} hitSlop={8} style={styles.scanBtn}>
            <ScanLine size={20} color={color.fg.base} strokeWidth={2} />
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
          <AmountText
            value={(debugBalance ?? displayTotal) * dc.rate}
            symbol={dc.symbol}
            size={36}
            minScale={0.5}
            showDecimals={shouldShowDecimals((debugBalance ?? displayTotal) * dc.rate, dc.code)}
            style={[styles.balanceInt, { textAlign: 'center' }]}
            tailStyle={styles.balanceDec}
            containerStyle={styles.balanceBox}
          />
          {balancePartial && (
            <View style={styles.balanceStaleRow}>
              <AlertTriangle size={12} color={color.warning.base} strokeWidth={2.5} />
              <Text style={styles.balanceStaleText}>{t('home.balanceStale')}</Text>
            </View>
          )}
        </Animated.View>
      </Pressable>

      {/* Action buttons */}
      <Animated.View style={styles.actionRow} entering={fadeInDown(200, 400)}>
        <ActionButton label={t('assets.actionSend')} icon={ArrowUp} onPress={() => router.push('/send')} accent />
        <ActionButton label={t('assets.actionReceive')} icon={ArrowDown} onPress={() => router.push('/receive')} />
        <ActionButton label={t('assets.actionHistory')} icon={Clock} onPress={() => router.push('/history')} />
      </Animated.View>

      {/* Token list header */}
      <View style={styles.tokenListHeader}>
        <View style={styles.tokenListTitleRow}>
          <Text style={styles.tokenListTitle}>{t('assets.sectionTitle')}</Text>
          {hiddenCount > 0 && (
            <Pressable onPress={() => setShowZeroBalance(!showZeroBalance)} hitSlop={8}>
              <Text style={styles.hiddenCount}>
                {showZeroBalance ? t('assets.hideZero') : t('assets.hiddenCount', { count: hiddenCount })}
              </Text>
            </Pressable>
          )}
        </View>
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
            <Text style={styles.addTokenText}>{t('assets.addToken')}</Text>
          </Pressable>
        </View>
      </View>

      {/* Search bar — always visible */}
      <View style={styles.searchBar}>
        <Search size={14} color={color.fg.subtle} strokeWidth={2} />
        <TextInput
          style={styles.searchInput}
          placeholder={t('assets.searchPlaceholder')}
          placeholderTextColor={color.fg.subtle}
          value={tokenSearch}
          onChangeText={setTokenSearch}
          autoCapitalize="none"
          autoCorrect={false}
        />
      </View>

      {/* RPC failure banner + fix flow (shared with HomeScreen) */}
      <RpcTroubleBanner
        chainIds={failedChainIds}
        onResolved={(chainId) => {
          setFailedChainIds(prev => prev.filter(id => id !== chainId));
          loadTokens(true, true);
        }}
      />
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
          <Text style={styles.emptyTitle}>{t('assets.emptyTitle')}</Text>
          <Text style={styles.emptySubtext}>
            {t('assets.emptySubtext')}
          </Text>
        </VelaCard>
      </Pressable>
    );
  };

  const hiddenCount = allTokens.length - tokens.length;
  const displayTokens = showZeroBalance ? allTokens : tokens;
  const filteredTokens = tokenSearch
    ? displayTokens.filter(t => {
        const q = tokenSearch.toLowerCase();
        return t.symbol.toLowerCase().includes(q) ||
          t.name.toLowerCase().includes(q) ||
          t.network.toLowerCase().includes(q) ||
          chainName(tokenChainId(t)).toLowerCase().includes(q);
      })
    : displayTokens;

  return (
    <ScreenContainer>
      <VelaRefresh refreshing={refreshing} onRefresh={onRefresh}>
        {(scrollProps) => (
          <Animated.FlatList
            {...scrollProps}
            data={filteredTokens}
            keyExtractor={(item: APIToken) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
            ListHeaderComponent={renderHeader()}
            ListEmptyComponent={renderEmpty()}
            renderItem={({ item, index }: { item: APIToken; index: number }) => (
              <TokenRow
                symbol={item.symbol}
                chainLabel={chainName(tokenChainId(item))}
                logoUrls={tokenLogoURLs(item)}
                balance={formatTokenAmount(tokenBalanceDouble(item), { compact: true })}
                usdValue={tokenUsdValue(item) > 0 ? dc.fmt(tokenUsdValue(item)) : undefined}
                onPress={() => navigateToToken(item)}
                index={index}
              />
            )}
            initialNumToRender={10}
            windowSize={5}
            maxToRenderPerBatch={8}
            contentContainerStyle={styles.listContent}
            showsVerticalScrollIndicator={false}
          />
        )}
      </VelaRefresh>

      {/* Account Switcher */}
      <AppModal visible={showAccountSwitcher} onClose={() => setShowAccountSwitcher(false)}>
        <View style={styles.switcherContainer}>
          <View style={styles.switcherHeader}>
            <View>
              <Text style={styles.switcherTitle}>{t('assets.switcherTitle')}</Text>
              {cachedBalances.size > 0 && (
                <Text style={styles.switcherTotal}>
                  {t('assets.switcherTotal', { amount: dc.fmt([...cachedBalances.values()].reduce((s, v) => s + v, 0)) })}
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
                    {bal != null && <Text style={styles.switcherBal}>{dc.fmt(bal)}</Text>}
                    {isActive && <Check size={18} color={color.accent.base} />}
                  </View>
                </Pressable>
              );
            })}
            <View style={styles.switcherEndLine} />
          </ScrollView>
        </View>
      </AppModal>

      {showScanner && (
        <QRScanner
          visible={showScanner}
          onScan={(data) => {
            if (isAddress(data)) {
              setShowScanner(false);
              router.push(`/send?prefilledRecipient=${data}`);
            } else if (isWalletPairURI(data)) {
              setShowScanner(false);
              connectToWalletPair(data);
              router.push('/connect');
            } else {
              const bridgeSession = parseRemoteInjectURL(data);
              if (bridgeSession) {
                setShowScanner(false);
                connectToBridge(bridgeSession);
                router.push('/connect');
              } else {
                showAlert(t('assets.invalidQrTitle'), t('assets.invalidQrMessage'));
              }
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
  header: {
    paddingTop: space.xl,
    marginBottom: space.sm,
  },

  // Header top row (account chip + scan)
  headerTopRow: {
    alignItems: 'center',
    paddingHorizontal: space.lg,
  },
  scanBtn: {
    position: 'absolute',
    right: space.lg,
    padding: space.sm,
  },
  closeBtn: {
    position: 'absolute',
    left: space.lg,
    padding: space.sm,
  },

  // Account info (centered, two-line)
  accountInfo: {
    alignItems: 'center',
    gap: 2,
  },
  accountName: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
    maxWidth: '70%',
  },
  accountAddr: {
    fontSize: text.sm,
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
  balanceBox: {
    width: '100%',
  },
  balanceStaleRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.xs,
    marginTop: space.sm,
    paddingHorizontal: space.xl,
  },
  balanceStaleText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.warning.base,
    textAlign: 'center',
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
    borderWidth: 1,
    borderColor: color.border.base,
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
  tokenListTitleRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
  },
  tokenListTitle: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
  },
  hiddenCount: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
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
    fontSize: 16, // ≥16px prevents iOS Safari auto-zoom on focus
    ...inter.regular,
    color: color.fg.base,
    paddingVertical: space.xs,
    outlineStyle: 'none',
  } as any,

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
}));
