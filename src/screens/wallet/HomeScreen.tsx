import * as Haptics from 'expo-haptics';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { TokenRow } from '@/components/ui/TokenRow';
import { VelaCard } from '@/components/ui/VelaCard';
import { AppModal } from '@/components/ui/AppModal';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, motion, radius, shadow, space, text } from '@/constants/theme';
import { chainName } from '@/models/network';
import { formatBalance, shortAddr, tokenBalanceDouble, tokenChainId, tokenLogoURL, tokenUsdValue, type APIToken } from '@/models/types';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { fetchTokens } from '@/services/wallet-api';
import { useFocusEffect } from '@react-navigation/native';
import * as Clipboard from 'expo-clipboard';
import { useRouter } from 'expo-router';
import { ArrowDown, ArrowUp, Check, Clock, Copy, Plus, ChevronDown, Search, X } from 'lucide-react-native';
import React, { useCallback, useRef, useState } from 'react';
import { Alert, AppState, FlatList, Pressable, RefreshControl, ScrollView, Text, TextInput, View } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';

const AUTO_REFRESH_MS = 10 * 60 * 1000;

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
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const loadInFlightRef = useRef(false);

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
      const result = await fetchTokens(address, { forceRefresh });
      setTokens(result);
    } catch (err) {
      if (!silent) {
        Alert.alert('Error', 'Failed to load token balances.');
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
      if (AppState.currentState === 'active') {
        loadTokens(true);
      }
    }, AUTO_REFRESH_MS);
    return () => clearInterval(timer);
  }, [loadTokens]));

  const onRefresh = useCallback(() => {
    setRefreshing(true);
    loadTokens(false, true);
  }, [loadTokens]);

  const totalUsd = tokens.reduce((sum, t) => sum + tokenUsdValue(t), 0);

  const [showAccountSwitcher, setShowAccountSwitcher] = useState(false);
  const [tokenSearch, setTokenSearch] = useState('');
  const [showSearch, setShowSearch] = useState(false);

  const [copied, setCopied] = useState(false);
  const copyAddress = async () => {
    if (!address) return;
    await Clipboard.setStringAsync(address);
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
        logo: token.logo ?? '',
        tokenAddress: token.tokenAddress ?? '',
        priceUsd: String(token.priceUsd ?? 0),
        chainName: token.chainName,
      },
    });
  };

  const renderHeader = () => (
    <View style={styles.header}>
      {/* Account chip */}
      <Animated.View entering={fadeIn(0, 400)}>
        <View style={styles.accountChipWrap}>
          <Pressable
            style={styles.accountChip}
            onPress={state.accounts.length > 1 ? () => setShowAccountSwitcher(true) : copyAddress}
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

      {/* Hero balance */}
      <Animated.View style={styles.balanceSection} entering={fadeInDown(100, 500)}>
        {/* <Text style={styles.balanceLabel}>Total Balance</Text> */}
        <View style={styles.balanceRow}>
          <Text style={[styles.balanceInt, { fontSize: balanceFontSize(totalUsd) }]}>
            {formatUsdInt(totalUsd)}
          </Text>
          <Text style={[styles.balanceDec, { fontSize: balanceFontSize(totalUsd) * 0.58 }]}>
            {formatUsdDec(totalUsd)}
          </Text>
        </View>
      </Animated.View>

      {/* Action buttons */}
      <Animated.View style={styles.actionRow} entering={fadeInDown(200, 400)}>
        <ActionButton label="Send" icon={ArrowUp} onPress={() => router.push('/send')} accent />
        <ActionButton label="Receive" icon={ArrowDown} onPress={() => router.push('/receive')} />
        <ActionButton label="History" icon={Clock} onPress={() => router.push('/history')} />
      </Animated.View>

      {/* Token list header */}
      <View style={styles.tokenListHeader}>
        <Text style={styles.tokenListTitle}>Assets</Text>
        <View style={styles.tokenListActions}>
          <Pressable
            style={styles.searchToggleBtn}
            onPress={() => { setShowSearch(!showSearch); if (showSearch) setTokenSearch(''); }}
            hitSlop={8}
          >
            {showSearch ? (
              <X size={14} color={color.fg.muted} strokeWidth={2.5} />
            ) : (
              <Search size={14} color={color.fg.muted} strokeWidth={2.5} />
            )}
          </Pressable>
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

      {/* Search bar */}
      {showSearch && (
        <View style={styles.searchBar}>
          <Search size={14} color={color.fg.subtle} strokeWidth={2} />
          <TextInput
            style={styles.searchInput}
            placeholder="Search tokens..."
            placeholderTextColor={color.fg.subtle}
            value={tokenSearch}
            onChangeText={setTokenSearch}
            autoFocus
            autoCapitalize="none"
            autoCorrect={false}
          />
        </View>
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
        data={filteredTokens}
        keyExtractor={(item) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
        ListHeaderComponent={renderHeader}
        ListEmptyComponent={renderEmpty}
        renderItem={({ item, index }) => (
          <TokenRow
            symbol={item.symbol}
            chainLabel={chainName(tokenChainId(item))}
            logoUrl={tokenLogoURL(item)}
            balance={formatBalance(tokenBalanceDouble(item))}
            usdValue={tokenUsdValue(item) > 0 ? formatUsd(tokenUsdValue(item)) : undefined}
            onPress={() => navigateToToken(item)}
            index={index}
          />
        )}
        refreshControl={
          <RefreshControl
            refreshing={refreshing}
            onRefresh={onRefresh}
            tintColor={color.accent.base}
          />
        }
        contentContainerStyle={styles.listContent}
        showsVerticalScrollIndicator={false}
      />

      {/* Account Switcher */}
      <AppModal visible={showAccountSwitcher} onClose={() => setShowAccountSwitcher(false)}>
        <View style={styles.switcherContainer}>
          <View style={styles.switcherHeader}>
            <Text style={styles.switcherTitle}>Switch Account</Text>
            <Pressable onPress={() => setShowAccountSwitcher(false)} hitSlop={8}>
              <X size={22} color={color.fg.base} strokeWidth={2} />
            </Pressable>
          </View>
          <ScrollView style={styles.switcherScroll}>
            {state.accounts.map((account, index) => {
              const isActive = index === state.activeAccountIndex;
              return (
                <Pressable
                  key={account.id}
                  style={[styles.switcherItem, isActive && styles.switcherItemActive]}
                  onPress={() => {
                    dispatch({ type: 'SWITCH_ACCOUNT', index });
                    Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
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
                  {isActive && <Check size={18} color={color.accent.base} />}
                </Pressable>
              );
            })}
          </ScrollView>
        </View>
      </AppModal>
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
  },
  balanceDec: {
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.subtle,
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
    padding: space['3xl'],
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
