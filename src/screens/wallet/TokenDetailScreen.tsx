import React, { useEffect, useState } from 'react';
import { ActivityIndicator, View, Text, Pressable, ScrollView } from 'react-native';
import { useTranslation } from 'react-i18next';
import { useLocalSearchParams } from 'expo-router';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { copyToClipboard, openBrowser } from '@/services/platform';
import { getAllNetworksSync } from '@/models/network';
import { useWallet } from '@/models/wallet-state';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenLogo } from '@/components/TokenLogo';
import { AmountText } from '@/components/ui/AmountText';
import { BarChart } from '@/components/ui/BarChart';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { shortAddr, tokenChainId as networkToChainId } from '@/models/types';
import type { APIToken } from '@/models/types';
import { chainName } from '@/models/network';
import { fetch7DayHistory, type BalancePoint } from '@/services/balance-history';
import { Copy, Check, ArrowLeft, ExternalLink } from 'lucide-react-native';

export default function TokenDetailScreen() {
  const { t } = useTranslation();
  useLocalePrefs(); // re-render when the number format changes
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

  const chainId = networkToChainId({ network } as APIToken);
  const chain = chainName(chainId);

  // Token fiat values follow the selected display currency + number format.
  const dc = useDisplayCurrency();
  const formatUsd = dc.fmt;

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
                <AmountText
                  text={formatTokenAmount(balance, { compact: true })}
                  size={text.xl}
                  minScale={0.7}
                  style={styles.heroAmount}
                  containerStyle={styles.heroAmountBox}
                />
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
            <Text style={styles.chartTitle}>{t('tokenDetail.chartTitle')}</Text>
            {historyLoading ? (
              <View style={styles.chartLoading}>
                <ActivityIndicator size="small" color={color.fg.subtle} />
              </View>
            ) : historyData.length > 0 ? (
              <BarChart data={historyData} symbol={symbol} />
            ) : (
              <Text style={styles.chartEmpty}>{t('tokenDetail.chartEmpty')}</Text>
            )}
          </VelaCard>
        </Animated.View>

        {/* Action buttons */}
        <Animated.View style={styles.buttonRow} entering={fadeInDown(100, 400)}>
          <VelaButton title={t('tokenDetail.send')} onPress={handleSend} style={styles.actionBtn} />
          <VelaButton title={t('tokenDetail.receive')} onPress={handleReceive} variant="secondary" style={styles.actionBtn} />
        </Animated.View>

        {/* Details — contract, decimals, unit price */}
        <Animated.View entering={fadeInDown(200, 400)} style={styles.detailSection}>
          {tokenName !== symbol && (
            <>
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>{t('tokenDetail.labelName')}</Text>
                <Text style={styles.detailValue}>{tokenName}</Text>
              </View>
              <View style={styles.separator} />
            </>
          )}
          {contractAddress && (
            <Pressable onPress={copyContract} style={styles.detailRow}>
              <Text style={styles.detailLabel}>{t('tokenDetail.labelContract')}</Text>
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
                <Text style={styles.detailLabel}>{t('tokenDetail.labelDecimals')}</Text>
                <Text style={styles.detailValue}>{decimals}</Text>
              </View>
            </>
          )}
          {priceUsd > 0 && (
            <>
              {contractAddress && <View style={styles.separator} />}
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>{t('tokenDetail.labelPrice')}</Text>
                <Text style={styles.detailValue}>{t('tokenDetail.priceValue', { symbol, value: formatUsd(priceUsd) })}</Text>
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
            <Text style={styles.detailLabel}>{t('tokenDetail.labelTransactions')}</Text>
            <View style={styles.detailValueRow}>
              <Text style={[styles.detailValue, { color: color.fg.muted }]}>{t('tokenDetail.viewOnExplorer')}</Text>
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
    flexShrink: 1,
    maxWidth: '58%',
  },
  heroAmountBox: {
    alignSelf: 'stretch',
  },
  heroAmount: {
    fontSize: text.xl,
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
    textAlign: 'right',
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
