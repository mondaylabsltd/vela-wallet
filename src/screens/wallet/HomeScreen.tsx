import React, { useState, useCallback, useRef } from 'react';
import { View, Text, FlatList, RefreshControl, Alert, AppState, Pressable } from 'react-native';
import { useRouter } from 'expo-router';
import { useFocusEffect } from '@react-navigation/native';
import * as Clipboard from 'expo-clipboard';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withSpring,
  FadeIn,
  FadeInDown,
} from 'react-native-reanimated';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenRow } from '@/components/ui/TokenRow';
import { color, text, weight, space, radius, shadow, motion, font, createStyles } from '@/constants/theme';
import { useWallet } from '@/models/wallet-state';
import { fetchTokens } from '@/services/wallet-api';
import { loadCustomTokens } from '@/services/storage';
import { tokenUsdValue, tokenBalanceDouble, tokenLogoURL, tokenChainId, formatBalance, shortAddr, type APIToken } from '@/models/types';
import { chainName } from '@/models/network';
import { ArrowUp, ArrowDown, Clock, Copy, Plus, Check } from 'lucide-react-native';

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
  const { activeAccount, state } = useWallet();

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
      result.sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
      const custom = await loadCustomTokens();
      for (const ct of custom) {
        if (!result.find(t => t.tokenAddress?.toLowerCase() === ct.contractAddress.toLowerCase() && tokenChainId(t) === ct.chainId)) {
          result.push({
            network: ct.id.split('_')[0] || 'eth-mainnet',
            chainName: ct.networkName,
            symbol: ct.symbol,
            balance: '0',
            decimals: ct.decimals,
            logo: null,
            name: ct.name,
            tokenAddress: ct.contractAddress,
            priceUsd: null,
            spam: false,
          });
        }
      }
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
      <Animated.View entering={FadeIn.duration(400)}>
        <Pressable style={styles.accountChip} onPress={copyAddress}>
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
              ) : (
                <Copy size={10} color={color.fg.subtle} strokeWidth={2} />
              )}
            </View>
          </View>
        </Pressable>
      </Animated.View>

      {/* Hero balance */}
      <Animated.View style={styles.balanceSection} entering={FadeInDown.delay(100).duration(500)}>
        <Text style={styles.balanceLabel}>Total Balance</Text>
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
      <Animated.View style={styles.actionRow} entering={FadeInDown.delay(200).duration(400)}>
        <ActionButton label="Send" icon={ArrowUp} onPress={() => router.push('/send')} accent />
        <ActionButton label="Receive" icon={ArrowDown} onPress={() => router.push('/receive')} />
        <ActionButton label="History" icon={Clock} onPress={() => router.push('/history')} />
      </Animated.View>

      {/* Token list header */}
      <View style={styles.tokenListHeader}>
        <Text style={styles.tokenListTitle}>Assets</Text>
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
  );

  const renderEmpty = () => {
    if (loading) return null;
    return (
      <VelaCard style={styles.emptyCard}>
        <Text style={styles.emptyTitle}>No tokens yet</Text>
        <Text style={styles.emptySubtext}>
          Receive tokens to your wallet address to get started
        </Text>
      </VelaCard>
    );
  };

  return (
    <ScreenContainer>
      <FlatList
        data={tokens}
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
  accountChip: {
    flexDirection: 'row',
    alignItems: 'center',
    alignSelf: 'center',
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
    fontWeight: weight.bold,
    color: color.accent.base,
  },
  accountTextGroup: {
    gap: 0,
  },
  accountName: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  addrRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
  },
  accountAddr: {
    fontSize: text.xs,
    fontWeight: weight.medium,
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
    fontWeight: weight.medium,
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
    fontWeight: weight.bold,
    fontFamily: font.display,
    color: color.fg.base,
  },
  balanceDec: {
    fontWeight: weight.bold,
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
    fontWeight: weight.medium,
    color: color.fg.base,
  },
  actionLabelAccent: {
    fontWeight: weight.semibold,
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
    fontWeight: weight.bold,
    color: color.fg.base,
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
    fontWeight: weight.semibold,
    color: color.accent.base,
  },

  // Empty
  emptyCard: {
    padding: space['4xl'],
    alignItems: 'center',
    gap: space.md,
  },
  emptyTitle: {
    fontSize: text.xl,
    fontWeight: weight.semibold,
    color: color.fg.muted,
  },
  emptySubtext: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 20,
  },
}));
