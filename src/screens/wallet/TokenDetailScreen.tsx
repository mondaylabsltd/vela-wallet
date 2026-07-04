import React from 'react';
import { View, Text, Pressable, ScrollView } from 'react-native';
import { useTranslation } from 'react-i18next';
import { useLocalSearchParams } from 'expo-router';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { openBrowser } from '@/services/platform';
import { useCopyFeedback } from '@/hooks/use-copy-feedback';
import { useWallet } from '@/models/wallet-state';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { TokenLogo } from '@/components/TokenLogo';
import { AmountText } from '@/components/ui/AmountText';
import { Divider } from '@/components/ui/DetailRow';
import { color, text, inter, space, font, createStyles } from '@/constants/theme';
import { formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { shortAddr, tokenChainId as networkToChainId } from '@/models/types';
import type { APIToken } from '@/models/types';
import { badgeNetworkFor, chainName, explorerTokenURL, explorerAddressURL } from '@/models/network';
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
  const { copied, copy } = useCopyFeedback();
  const copyContract = () => {
    if (!contractAddress) return;
    copy(contractAddress);
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
        {/* Nav bar — plain icon button, no card surface */}
        <View style={styles.navBar}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.navTitle}>{symbol}</Text>
          <View style={styles.navSpacer} />
        </View>

        {/* Hero — open on the page (no card): token identity + big balance with a
            subordinated fiat value below. */}
        <Animated.View entering={fadeIn(0, 400)} style={styles.hero}>
          <View style={styles.heroIdentityRow}>
            <TokenLogo symbol={symbol} logoUrls={logoUrls} chain={badgeNetworkFor(symbol, chainId, !contractAddress)} size={44} />
            <View style={styles.heroIdentity}>
              <Text style={styles.heroSymbol}>{symbol}</Text>
              <Text style={styles.heroChain}>{chain}</Text>
            </View>
          </View>
          <AmountText
            text={formatTokenAmount(balance, { compact: true })}
            unit={symbol}
            size={text['4xl']}
            minScale={0.6}
            style={styles.heroAmount}
            tailStyle={styles.heroAmountUnit}
            containerStyle={styles.heroAmountBox}
          />
          {usdValue > 0 && (
            // Secondary conversion line — plain text, NOT AmountText's fit-to-width
            // cascade: on this short "≈ $x" line the cascade kept re-measuring and
            // shrank it (the flicker + tiny-text bug). The token amount above is
            // the hero; this just annotates it.
            <Text style={styles.heroUsd} numberOfLines={1}>{`≈ ${formatUsd(usdValue)}`}</Text>
          )}
        </Animated.View>

        {/* Action buttons — VelaButton CTAs, kept */}
        <Animated.View style={styles.buttonRow} entering={fadeInDown(100, 400)}>
          <VelaButton title={t('tokenDetail.send')} onPress={handleSend} style={styles.actionBtn} />
          <VelaButton title={t('tokenDetail.receive')} onPress={handleReceive} variant="secondary" style={styles.actionBtn} />
        </Animated.View>

        {/* Details — open rows on the page, separated by hairline dividers under a
            SectionLabel heading (no per-row / whole-section card). */}
        <Animated.View entering={fadeInDown(200, 400)} style={styles.detailSection}>
          {tokenName !== symbol && (
            <>
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>{t('tokenDetail.labelName')}</Text>
                <Text style={styles.detailValue}>{tokenName}</Text>
              </View>
              <Divider />
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
              <Divider />
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>{t('tokenDetail.labelDecimals')}</Text>
                <Text style={styles.detailValue}>{decimals}</Text>
              </View>
            </>
          )}
          {priceUsd > 0 && (
            <>
              {contractAddress && <Divider />}
              <View style={styles.detailRow}>
                <Text style={styles.detailLabel}>{t('tokenDetail.labelPrice')}</Text>
                <Text style={styles.detailValue}>{t('tokenDetail.priceValue', { symbol, value: formatUsd(priceUsd) })}</Text>
              </View>
            </>
          )}
          <Divider />
          <Pressable
            style={styles.detailRow}
            onPress={() => {
              const url = contractAddress
                ? explorerTokenURL(chainId, contractAddress, walletAddress)
                : explorerAddressURL(chainId, walletAddress);
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

  // Hero — OPEN on the page (no card): token identity, then a big balance with a
  // subordinated fiat value below. Grouped by space, not by a box.
  hero: {
    paddingTop: space.lg,
    marginBottom: space['2xl'],
  },
  heroIdentityRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    marginBottom: space.xl,
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
  heroAmountBox: {
    alignSelf: 'stretch',
  },
  heroAmount: {
    fontSize: text['4xl'],
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.base,
    letterSpacing: -0.8,
  },
  heroAmountUnit: {
    ...inter.bold,
    fontFamily: font.display,
    color: color.fg.subtle,
  },
  heroUsd: {
    marginTop: space.sm,
    fontSize: text['2xl'],
    ...inter.semibold,
    fontFamily: font.display,
    color: color.fg.muted,
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
}));
