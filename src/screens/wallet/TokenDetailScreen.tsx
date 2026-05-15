import React, { useEffect, useState } from 'react';
import { ActivityIndicator, View, Text, Pressable, ScrollView } from 'react-native';
import { useLocalSearchParams } from 'expo-router';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { copyToClipboard, openBrowser } from '@/services/platform';
import { getAllNetworksSync } from '@/models/network';
import { useWallet } from '@/models/wallet-state';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenLogo } from '@/components/TokenLogo';
import { BarChart } from '@/components/ui/BarChart';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { formatBalance, shortAddr } from '@/models/types';
import { chainName } from '@/models/network';
import { fetch7DayHistory, type BalancePoint } from '@/services/balance-history';
import { Copy, Check, ArrowLeft, ExternalLink } from 'lucide-react-native';

export default function TokenDetailScreen() {
  const router = useSafeRouter();
  const params = useLocalSearchParams<{
    symbol: string;
    name: string;
    network: string;
    balance: string;
    decimals: string;
    logos: string;
    tokenAddress: string;
    priceUsd: string;
    chainName: string;
  }>();

  const symbol = params.symbol ?? '';
  const tokenName = params.name ?? symbol;
  const balance = parseFloat(params.balance ?? '0');
  const priceUsd = parseFloat(params.priceUsd ?? '0');
  const usdValue = balance * priceUsd;
  const logoUrls: string[] = (() => { try { return JSON.parse(params.logos ?? '[]'); } catch { return []; } })();
  const contractAddress = params.tokenAddress || null;
  const network = params.network ?? '';
  const decimals = parseInt(params.decimals ?? '18', 10);

  const chainIdMap: Record<string, number> = {
    'eth-mainnet': 1,
    'arb-mainnet': 42161,
    'base-mainnet': 8453,
    'opt-mainnet': 10,
    'matic-mainnet': 137,
    'bnb-mainnet': 56,
    'avax-mainnet': 43114,
  };
  const chainId = chainIdMap[network] ?? 1;
  const chain = chainName(chainId);

  const formatUsd = (value: number) =>
    '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });

  const { activeAccount, state } = useWallet();
  const walletAddress = activeAccount?.address ?? state.address;
  const [copied, setCopied] = useState(false);
  const [historyData, setHistoryData] = useState<BalancePoint[]>([]);
  const [historyLoading, setHistoryLoading] = useState(true);

  useEffect(() => {
    if (!walletAddress) return;
    setHistoryLoading(true);
    fetch7DayHistory({
      address: walletAddress,
      chainId,
      tokenAddress: contractAddress,
      decimals,
      currentBalance: balance,
    })
      .then(setHistoryData)
      .catch(() => {})
      .finally(() => setHistoryLoading(false));
  }, [walletAddress, chainId, contractAddress, decimals, balance]);
  const copyContract = async () => {
    if (!contractAddress) return;
    await copyToClipboard(contractAddress);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  const handleSend = () => {
    router.push({
      pathname: '/send',
      params: { preselectedSymbol: symbol, preselectedNetwork: network },
    });
  };

  const handleReceive = () => {
    router.push('/receive');
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Nav bar */}
        <View style={styles.navBar}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.navTitle}>{symbol}</Text>
          <View style={styles.navSpacer} />
        </View>

        {/* Hero card — logo, name, network, balance, USD */}
        <Animated.View entering={fadeIn(0, 400)}>
          <VelaCard elevated style={styles.heroCard}>
            <View style={styles.heroRow}>
              <TokenLogo symbol={symbol} logoUrls={logoUrls} size={44} />
              <View style={styles.heroIdentity}>
                <Text style={styles.heroSymbol}>{symbol}</Text>
                <Text style={styles.heroChain}>{chain}</Text>
              </View>
              <View style={styles.heroBalance}>
                <Text style={styles.heroAmount} adjustsFontSizeToFit numberOfLines={1}>
                  {formatBalance(balance)}
                </Text>
                {usdValue > 0 && (
                  <Text style={styles.heroUsd} adjustsFontSizeToFit numberOfLines={1}>
                    {formatUsd(usdValue)}
                  </Text>
                )}
              </View>
            </View>
          </VelaCard>
        </Animated.View>

        {/* 7-day balance chart */}
        <Animated.View entering={fadeInDown(50, 400)}>
          <VelaCard style={styles.chartCard}>
            <Text style={styles.chartTitle}>7-Day Balance</Text>
            {historyLoading ? (
              <View style={styles.chartLoading}>
                <ActivityIndicator size="small" color={color.fg.subtle} />
              </View>
            ) : historyData.length > 1 ? (
              <BarChart data={historyData} symbol={symbol} />
            ) : (
              <Text style={styles.chartEmpty}>No historical data available</Text>
            )}
          </VelaCard>
        </Animated.View>

        {/* Action buttons */}
        <Animated.View style={styles.buttonRow} entering={fadeInDown(100, 400)}>
          <VelaButton title="Send" onPress={handleSend} style={styles.actionBtn} />
          <VelaButton title="Receive" onPress={handleReceive} variant="secondary" style={styles.actionBtn} />
        </Animated.View>

        {/* Details — contract, decimals, unit price */}
        <Animated.View entering={fadeInDown(200, 400)} style={styles.detailSection}>
          {tokenName !== symbol && (
            <>
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>Name</Text>
                <Text style={styles.detailValue}>{tokenName}</Text>
              </View>
              <View style={styles.separator} />
            </>
          )}
          {contractAddress && (
            <Pressable onPress={copyContract} style={styles.detailRow}>
              <Text style={styles.detailLabel}>Contract</Text>
              <View style={styles.detailValueRow}>
                <Text style={styles.detailValue}>{shortAddr(contractAddress)}</Text>
                {copied ? (
                  <Check size={12} color={color.success.base} strokeWidth={3} />
                ) : (
                  <Copy size={12} color={color.fg.subtle} />
                )}
              </View>
            </Pressable>
          )}
          {contractAddress && (
            <>
              <View style={styles.separator} />
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>Decimals</Text>
                <Text style={styles.detailValue}>{decimals}</Text>
              </View>
            </>
          )}
          {priceUsd > 0 && (
            <>
              {contractAddress && <View style={styles.separator} />}
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>Price</Text>
                <Text style={styles.detailValue}>1 {symbol} = {formatUsd(priceUsd)}</Text>
              </View>
            </>
          )}
          <View style={styles.separator} />
          <Pressable
            style={styles.detailRow}
            onPress={() => {
              const net = getAllNetworksSync().find(n => n.chainId === chainId);
              const base = net?.explorerURL ?? 'https://etherscan.io';
              const url = contractAddress
                ? `${base}/token/${contractAddress}?a=${walletAddress}`
                : `${base}/address/${walletAddress}`;
              openBrowser(url);
            }}
          >
            <Text style={styles.detailLabel}>Transactions</Text>
            <View style={styles.detailValueRow}>
              <Text style={[styles.detailValue, { color: color.fg.muted }]}>View on Explorer</Text>
              <ExternalLink size={12} color={color.fg.muted} strokeWidth={2} />
            </View>
          </Pressable>
        </Animated.View>
      </ScrollView>
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  content: {
    paddingBottom: 100,
  },
  navBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
  },
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  navTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  navSpacer: { minWidth: 50 },

  // Hero card
  heroCard: {
    padding: space['2xl'],
    marginBottom: space['2xl'],
  },
  heroRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  heroIdentity: {
    flex: 1,
    gap: 2,
  },
  heroSymbol: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.fg.base,
  },
  heroChain: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
  },
  heroBalance: {
    alignItems: 'flex-end',
    gap: 2,
  },
  heroAmount: {
    fontSize: text.xl,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
  },
  heroUsd: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },

  // Buttons
  chartCard: {
    padding: space.xl,
    marginBottom: space.lg,
  },
  chartTitle: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.5,
  },
  chartLoading: {
    height: 120,
    justifyContent: 'center',
    alignItems: 'center',
  },
  chartEmpty: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center' as const,
    paddingVertical: space['3xl'],
  },

  buttonRow: {
    flexDirection: 'row',
    gap: space.lg,
    marginBottom: space['3xl'],
  },
  actionBtn: {
    flex: 1,
  },

  // Detail section
  detailSection: {
    paddingHorizontal: space.lg,
  },
  detailRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
  },
  detailLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
  },
  detailValueRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
  },
  detailValue: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.base,
  },
  separator: {
    height: 1,
    backgroundColor: color.border.base,
  },
}));
