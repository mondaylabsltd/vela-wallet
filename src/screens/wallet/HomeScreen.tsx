import React, { useState, useCallback, useRef } from 'react';
import { View, Text, FlatList, TouchableOpacity, StyleSheet, RefreshControl, Alert, AppState } from 'react-native';
import { useRouter } from 'expo-router';
import { useFocusEffect } from '@react-navigation/native';
import * as Clipboard from 'expo-clipboard';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenLogo } from '@/components/TokenLogo';
import { color, text, weight, space, radius, font } from '@/constants/theme';
import { useWallet } from '@/models/wallet-state';
import { fetchTokens } from '@/services/wallet-api';
import { loadCustomTokens } from '@/services/storage';
import { tokenUsdValue, tokenBalanceDouble, tokenLogoURL, tokenChainId, formatBalance, shortAddr, type APIToken } from '@/models/types';
import { chainName } from '@/models/network';
import { ArrowUp, ArrowDown, Menu, Copy } from 'lucide-react-native';

const AUTO_REFRESH_MS = 10 * 60 * 1000;

function formatUsd(value: number): string {
  return '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

/** Integer part including "$" sign, e.g. "$1,234" */
function formatUsdInt(value: number): string {
  const full = formatUsd(value);
  const dot = full.indexOf('.');
  return dot === -1 ? full : full.slice(0, dot);
}

/** Decimal part including dot, e.g. ".00" */
function formatUsdDec(value: number): string {
  const full = formatUsd(value);
  const dot = full.indexOf('.');
  return dot === -1 ? '.00' : full.slice(dot);
}

/** Scale balance font size down as the number gets longer */
function balanceFontSize(usd: number): number {
  const len = formatUsdInt(usd).length; // includes "$" and commas
  if (len <= 7) return 30;   // up to $9,999
  if (len <= 9) return 26;   // up to $999,999
  if (len <= 12) return 22;  // up to $999,999,999
  if (len <= 15) return 18;  // up to $999,999,999,999
  return 15;
}

function TokenRow({ token, onPress }: { token: APIToken; onPress: () => void }) {
  const balance = tokenBalanceDouble(token);
  const usd = tokenUsdValue(token);
  const logo = tokenLogoURL(token);
  const chain = chainName(tokenChainId(token));

  return (
    <TouchableOpacity style={styles.tokenRow} onPress={onPress} activeOpacity={0.7}>
      <TokenLogo symbol={token.symbol} logoUrl={logo} size={32} />
      <View style={styles.tokenInfo}>
        <Text style={styles.tokenName} numberOfLines={1}>{token.symbol}</Text>
        <Text style={styles.tokenChain}>{chain}</Text>
      </View>
      <View style={styles.tokenValues}>
        <Text style={styles.tokenBalance}>{formatBalance(balance)}</Text>
        {usd > 0 && <Text style={styles.tokenUsd}>{formatUsd(usd)}</Text>}
      </View>
    </TouchableOpacity>
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
      // Sort by USD value descending
      result.sort((a, b) => tokenUsdValue(b) - tokenUsdValue(a));
      // Load custom tokens and merge
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
      {/* Account name + address */}
      <View style={styles.accountRow}>
        <Text style={styles.accountName}>{accountName}</Text>
        <TouchableOpacity style={styles.addrRow} onPress={copyAddress} activeOpacity={0.7}>
          <Text style={styles.accountAddr}>{shortAddr(address)}</Text>
          <Copy size={12} color={copied ? color.accent.base : color.fg.subtle} />
          {copied && <Text style={styles.copiedText}>Copied</Text>}
        </TouchableOpacity>
      </View>

      {/* Total balance — dynamic font size based on digit count */}
      <View style={styles.balanceRow}>
        <Text style={[styles.balanceInt, { fontSize: balanceFontSize(totalUsd) }]}>
          {formatUsdInt(totalUsd)}
        </Text>
        <Text style={[styles.balanceDec, { fontSize: balanceFontSize(totalUsd) * 0.62 }]}>
          {formatUsdDec(totalUsd)}
        </Text>
      </View>

      {/* Action buttons */}
      <View style={styles.actionRow}>
        <ActionButton label="Send" icon={ArrowUp} onPress={() => router.push('/send')} />
        <ActionButton label="Receive" icon={ArrowDown} onPress={() => router.push('/receive')} />
        <ActionButton label="History" icon={Menu} onPress={() => router.push('/history')} />
      </View>

      {/* Add Token */}
      <TouchableOpacity onPress={() => router.push('/add-token')} activeOpacity={0.7}>
        <Text style={styles.addTokenLink}>+ Add Token</Text>
      </TouchableOpacity>
    </View>
  );

  const renderEmpty = () => {
    if (loading) return null;
    return (
      <View style={styles.emptyContainer}>
        <Text style={styles.emptyText}>No tokens found</Text>
        <Text style={styles.emptySubtext}>Receive tokens to get started</Text>
      </View>
    );
  };

  return (
    <ScreenContainer>
      <FlatList
        data={tokens}
        keyExtractor={(item) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
        ListHeaderComponent={renderHeader}
        ListEmptyComponent={renderEmpty}
        renderItem={({ item }) => (
          <TokenRow token={item} onPress={() => navigateToToken(item)} />
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

function ActionButton({ label, icon: Icon, onPress }: { label: string; icon: React.ComponentType<any>; onPress: () => void }) {
  return (
    <TouchableOpacity style={styles.actionBtn} onPress={onPress} activeOpacity={0.7}>
      <Icon size={14} color={color.fg.base} strokeWidth={2.5} />
      <Text style={styles.actionLabel}>{label}</Text>
    </TouchableOpacity>
  );
}

const styles = StyleSheet.create({
  listContent: {
    paddingBottom: 100,
  },
  header: {
    paddingTop: space.xl,
    marginBottom: space.md,
  },
  accountRow: {
    alignItems: 'center',
    marginBottom: space.md,
  },
  accountName: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  addrRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginTop: space.xs,
  },
  accountAddr: {
    fontSize: text.xs,
    fontWeight: weight.medium,
    fontFamily: font.mono,
    color: color.fg.subtle,
  },
  copiedText: {
    fontSize: text.xs,
    fontWeight: weight.medium,
    color: color.accent.base,
    marginLeft: space.xs,
  },
  balanceRow: {
    flexDirection: 'row',
    alignItems: 'baseline',
    justifyContent: 'center',
    marginBottom: space['2xl'],
  },
  balanceInt: {
    fontSize: text['4xl'],
    fontWeight: weight.bold,
    color: color.fg.base,
  },
  balanceDec: {
    fontSize: text['2xl'],
    fontWeight: weight.bold,
    color: color.fg.subtle,
  },
  actionRow: {
    flexDirection: 'row',
    justifyContent: 'center',
    gap: space.md,
    marginBottom: space.xl,
  },
  actionBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingHorizontal: space.xl,
    paddingVertical: space.md,
    borderRadius: radius['2xl'],
    borderWidth: 1,
    borderColor: color.border.base,
    backgroundColor: color.bg.raised,
  },
  actionLabel: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  addTokenLink: {
    fontSize: text.sm,
    fontWeight: weight.regular,
    color: color.accent.base,
    textAlign: 'right',
    marginBottom: space.sm,
  },
  tokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: space.lg,
    paddingHorizontal: space.sm,
    gap: space.lg,
  },
  tokenInfo: {
    flex: 1,
    gap: space.xs,
  },
  tokenName: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  tokenChain: {
    fontSize: text.xs,
    fontWeight: weight.regular,
    color: color.fg.subtle,
  },
  tokenValues: {
    alignItems: 'flex-end',
    gap: space.xs,
  },
  tokenBalance: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  tokenUsd: {
    fontSize: text.xs,
    fontWeight: weight.regular,
    color: color.fg.subtle,
  },
  emptyContainer: {
    alignItems: 'center',
    paddingTop: space['5xl'],
    gap: space.md,
  },
  emptyText: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: color.fg.muted,
  },
  emptySubtext: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.subtle,
  },
});
