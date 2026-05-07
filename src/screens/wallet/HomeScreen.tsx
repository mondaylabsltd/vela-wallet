import React, { useState, useCallback, useRef } from 'react';
import { View, Text, FlatList, TouchableOpacity, StyleSheet, RefreshControl, Alert, AppState } from 'react-native';
import { useRouter } from 'expo-router';
import { useFocusEffect } from '@react-navigation/native';
import * as Clipboard from 'expo-clipboard';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenLogo } from '@/components/TokenLogo';
import { VelaColor, VelaFont, VelaSpacing } from '@/constants/theme';
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

function TokenRow({ token, onPress }: { token: APIToken; onPress: () => void }) {
  const balance = tokenBalanceDouble(token);
  const usd = tokenUsdValue(token);
  const logo = tokenLogoURL(token);
  const chain = chainName(tokenChainId(token));

  return (
    <TouchableOpacity style={styles.tokenRow} onPress={onPress} activeOpacity={0.7}>
      <TokenLogo symbol={token.symbol} logoUrl={logo} size={40} />
      <View style={styles.tokenInfo}>
        <Text style={styles.tokenName} numberOfLines={1}>{token.name || token.symbol}</Text>
        <Text style={styles.tokenChain}>{chain}</Text>
      </View>
      <View style={styles.tokenValues}>
        <Text style={styles.tokenBalance}>{formatBalance(balance)} {token.symbol}</Text>
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
          <Copy size={12} color={copied ? VelaColor.accent : VelaColor.textTertiary} />
          {copied && <Text style={styles.copiedText}>Copied</Text>}
        </TouchableOpacity>
      </View>

      {/* Total balance */}
      <View style={styles.balanceRow}>
        <Text style={styles.balanceInt}>{formatUsdInt(totalUsd)}</Text>
        <Text style={styles.balanceDec}>{formatUsdDec(totalUsd)}</Text>
      </View>

      {/* Action buttons */}
      <View style={styles.actionRow}>
        <ActionButton label="Send" icon={ArrowUp} onPress={() => router.push('/send')} />
        <ActionButton label="Receive" icon={ArrowDown} onPress={() => router.push('/receive')} />
        <ActionButton label="History" icon={Menu} onPress={() => router.push('/history')} />
      </View>
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
            tintColor={VelaColor.accent}
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
      <View style={styles.actionIconBg}>
        <Icon size={20} color="#FFFFFF" strokeWidth={2.5} />
      </View>
      <Text style={styles.actionLabel}>{label}</Text>
    </TouchableOpacity>
  );
}

const styles = StyleSheet.create({
  listContent: {
    paddingBottom: 100,
  },
  header: {
    paddingTop: 20,
    marginBottom: 24,
  },
  accountRow: {
    alignItems: 'center',
    marginBottom: 12,
  },
  accountName: {
    ...VelaFont.title(18),
    color: VelaColor.textPrimary,
  },
  addrRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 5,
    marginTop: 3,
  },
  accountAddr: {
    ...VelaFont.mono(13),
    color: VelaColor.textTertiary,
  },
  copiedText: {
    ...VelaFont.caption(),
    color: VelaColor.accent,
    marginLeft: 2,
  },
  balanceRow: {
    flexDirection: 'row',
    alignItems: 'baseline',
    justifyContent: 'center',
    marginBottom: 28,
  },
  balanceInt: {
    ...VelaFont.heading(38),
    color: VelaColor.textPrimary,
  },
  balanceDec: {
    ...VelaFont.heading(24),
    color: VelaColor.textTertiary,
  },
  actionRow: {
    flexDirection: 'row',
    justifyContent: 'center',
    gap: 36,
    marginBottom: 16,
  },
  actionBtn: {
    alignItems: 'center',
    gap: 8,
  },
  actionIconBg: {
    width: 52,
    height: 52,
    borderRadius: 26,
    backgroundColor: VelaColor.textPrimary,
    alignItems: 'center',
    justifyContent: 'center',
  },
  actionIcon: {
    fontSize: 22,
    color: '#FFFFFF',
    fontWeight: '700',
  },
  actionLabel: {
    ...VelaFont.label(12),
    color: VelaColor.textSecondary,
  },
  tokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: VelaSpacing.itemGap,
    paddingHorizontal: 4,
    gap: 12,
  },
  tokenInfo: {
    flex: 1,
    gap: 2,
  },
  tokenName: {
    ...VelaFont.title(15),
    color: VelaColor.textPrimary,
  },
  tokenChain: {
    ...VelaFont.body(13),
    color: VelaColor.textSecondary,
  },
  tokenValues: {
    alignItems: 'flex-end',
    gap: 2,
  },
  tokenBalance: {
    ...VelaFont.title(15),
    color: VelaColor.textPrimary,
  },
  tokenUsd: {
    ...VelaFont.body(13),
    color: VelaColor.textSecondary,
  },
  emptyContainer: {
    alignItems: 'center',
    paddingTop: 60,
    gap: 8,
  },
  emptyText: {
    ...VelaFont.title(17),
    color: VelaColor.textSecondary,
  },
  emptySubtext: {
    ...VelaFont.body(14),
    color: VelaColor.textTertiary,
  },
});
