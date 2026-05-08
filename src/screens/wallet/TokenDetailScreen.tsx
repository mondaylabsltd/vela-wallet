import React from 'react';
import { View, Text, Pressable, ScrollView, Alert } from 'react-native';
import { useRouter, useLocalSearchParams } from 'expo-router';
import * as Clipboard from 'expo-clipboard';
import Animated, { FadeIn, FadeInDown } from 'react-native-reanimated';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { TokenLogo } from '@/components/TokenLogo';
import { color, text, weight, space, radius, shadow, font, createStyles } from '@/constants/theme';
import { formatBalance, shortAddr } from '@/models/types';
import { chainName } from '@/models/network';
import { Copy } from 'lucide-react-native';

export default function TokenDetailScreen() {
  const router = useRouter();
  const params = useLocalSearchParams<{
    symbol: string;
    name: string;
    network: string;
    balance: string;
    decimals: string;
    logo: string;
    tokenAddress: string;
    priceUsd: string;
    chainName: string;
  }>();

  const symbol = params.symbol ?? '';
  const tokenName = params.name ?? symbol;
  const balance = parseFloat(params.balance ?? '0');
  const priceUsd = parseFloat(params.priceUsd ?? '0');
  const usdValue = balance * priceUsd;
  const logoUrl = params.logo || null;
  const contractAddress = params.tokenAddress || null;
  const network = params.network ?? '';
  const decimals = parseInt(params.decimals ?? '18', 10);
  const isNative = !contractAddress;

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

  const copyContract = async () => {
    if (!contractAddress) return;
    await Clipboard.setStringAsync(contractAddress);
    Alert.alert('Copied', 'Contract address copied to clipboard.');
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
          <Pressable onPress={() => router.back()} hitSlop={8}>
            <Text style={styles.backBtn}>Back</Text>
          </Pressable>
          <Text style={styles.navTitle}>{symbol}</Text>
          <View style={styles.navSpacer} />
        </View>

        {/* Token header */}
        <Animated.View style={styles.tokenHeader} entering={FadeIn.duration(400)}>
          <TokenLogo symbol={symbol} logoUrl={logoUrl} size={72} />
          <Text style={styles.tokenName}>{tokenName}</Text>
          <View style={styles.chainBadge}>
            <Text style={styles.chainBadgeText}>{chain}</Text>
          </View>
        </Animated.View>

        {/* Balance card */}
        <Animated.View entering={FadeInDown.delay(100).duration(400)}>
          <VelaCard elevated style={styles.balanceCard}>
            <Text style={styles.balanceLabel}>Balance</Text>
            <Text style={styles.balanceValue} adjustsFontSizeToFit numberOfLines={1}>
              {formatBalance(balance)} {symbol}
            </Text>
            {usdValue > 0 && (
              <Text style={styles.usdValue}>{formatUsd(usdValue)}</Text>
            )}
            {priceUsd > 0 && (
              <Text style={styles.priceLabel}>
                1 {symbol} = {formatUsd(priceUsd)}
              </Text>
            )}
          </VelaCard>
        </Animated.View>

        {/* Action buttons */}
        <Animated.View style={styles.buttonRow} entering={FadeInDown.delay(200).duration(400)}>
          <VelaButton title="Send" onPress={handleSend} style={styles.actionBtn} />
          <VelaButton title="Receive" onPress={handleReceive} variant="secondary" style={styles.actionBtn} />
        </Animated.View>

        {/* Token info */}
        <Animated.View entering={FadeInDown.delay(300).duration(400)}>
          <VelaCard style={styles.infoCard}>
            <InfoRow label="Type" value={isNative ? 'Native' : 'ERC-20'} />
            <View style={styles.separator} />
            <InfoRow label="Network" value={chain} />
            <View style={styles.separator} />
            <InfoRow label="Decimals" value={String(decimals)} />
            {contractAddress && (
              <>
                <View style={styles.separator} />
                <Pressable onPress={copyContract} style={styles.contractRow}>
                  <Text style={styles.infoLabel}>Contract</Text>
                  <View style={styles.infoValueRow}>
                    <Text style={styles.infoValue}>{shortAddr(contractAddress)}</Text>
                    <Copy size={12} color={color.accent.base} />
                  </View>
                </Pressable>
              </>
            )}
          </VelaCard>
        </Animated.View>
      </ScrollView>
    </ScreenContainer>
  );
}

function InfoRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.infoRow}>
      <Text style={styles.infoLabel}>{label}</Text>
      <Text style={styles.infoValue}>{value}</Text>
    </View>
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
  backBtn: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: color.accent.base,
    minWidth: 50,
  },
  navTitle: {
    fontSize: text.xl,
    fontWeight: weight.bold,
    color: color.fg.base,
  },
  navSpacer: { minWidth: 50 },

  // Token header
  tokenHeader: {
    alignItems: 'center',
    paddingVertical: space['3xl'],
    gap: space.md,
  },
  tokenName: {
    fontSize: text['2xl'],
    fontWeight: weight.bold,
    color: color.fg.base,
  },
  chainBadge: {
    backgroundColor: color.bg.sunken,
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderRadius: radius.full,
  },
  chainBadgeText: {
    fontSize: text.sm,
    fontWeight: weight.medium,
    color: color.fg.muted,
  },

  // Balance card
  balanceCard: {
    padding: space['3xl'],
    alignItems: 'center',
    gap: space.md,
    marginBottom: space['2xl'],
  },
  balanceLabel: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 1,
  },
  balanceValue: {
    fontSize: text['4xl'],
    fontWeight: weight.bold,
    fontFamily: font.display,
    color: color.fg.base,
  },
  usdValue: {
    fontSize: text.xl,
    fontWeight: weight.semibold,
    color: color.fg.muted,
  },
  priceLabel: {
    fontSize: text.sm,
    fontWeight: weight.regular,
    color: color.fg.subtle,
    marginTop: space.xs,
  },

  // Buttons
  buttonRow: {
    flexDirection: 'row',
    gap: space.lg,
    marginBottom: space['3xl'],
  },
  actionBtn: {
    flex: 1,
  },

  // Info card
  infoCard: {
    padding: space['2xl'],
  },
  infoRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
  },
  contractRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
  },
  infoLabel: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.muted,
  },
  infoValueRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
  },
  infoValue: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  separator: {
    height: 1,
    backgroundColor: color.border.base,
  },
}));
