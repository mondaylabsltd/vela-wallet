import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { chainName, getAllNetworksSync } from '@/models/network';
import { formatBalance, tokenBalanceDouble, tokenChainId, tokenId, type APIToken } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import { copyToClipboard, hapticLight, hapticSuccess, isAppActive } from '@/services/platform';
import { fetchTokens } from '@/services/wallet-api';
import { ArrowLeft, Check, Copy, ShieldAlert } from 'lucide-react-native';
import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pressable, ScrollView, Text, View } from 'react-native';
import Animated from 'react-native-reanimated';

// Aggressive polling: 3s for first 1 min, then 60s for next 4 min, then stop
const FAST_INTERVAL_MS = 3_000;
const SLOW_INTERVAL_MS = 60_000;
const FAST_PHASE_MS = 1 * 60_000;
const TOTAL_LISTEN_MS = 5 * 60_000;

export default function ReceiveScreen() {
  const { t } = useTranslation();
  const router = useSafeRouter();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';
  const networks = getAllNetworksSync();

  const [depositDetected, setDepositDetected] = useState(false);
  interface DepositEntry { time: string; items: { symbol: string; amount: string; network: string; usd: string | null }[] }
  const [deposits, setDeposits] = useState<DepositEntry[]>([]);
  const previousTokens = useRef<APIToken[] | null>(null);
  const [copied, setCopied] = useState(false);
  const [warningDismissed, setWarningDismissed] = useState(false);

  // Deposit detection polling — quietly watches for incoming transfers while
  // this screen is open and surfaces them as they land (no persistent status).
  useEffect(() => {
    if (!address) return;
    previousTokens.current = null;
    const startTime = Date.now();
    let timerId: ReturnType<typeof setTimeout>;

    const checkDeposit = async () => {
      if (!isAppActive()) return;
      try {
        const tokens = await fetchTokens(address, { forceRefresh: true });

        if (previousTokens.current !== null) {
          // Guard: a smaller token set than baseline means a chain likely
          // failed — skip comparison to avoid false positives.
          if (tokens.length < previousTokens.current.length) {
            scheduleNext();
            return;
          }

          // Diff: find tokens whose balance increased vs baseline.
          const prevMap = new Map(previousTokens.current.map(tk => [tokenId(tk), tokenBalanceDouble(tk)]));
          const changes: DepositEntry['items'] = [];
          for (const tk of tokens) {
            const prevBal = prevMap.get(tokenId(tk)) ?? 0;
            const curBal = tokenBalanceDouble(tk);
            if (curBal > prevBal) {
              const diff = curBal - prevBal;
              changes.push({
                symbol: tk.symbol,
                amount: formatBalance(diff),
                network: chainName(tokenChainId(tk)),
                usd: tk.priceUsd ? `$${(diff * tk.priceUsd).toFixed(2)}` : null,
              });
            }
          }

          if (changes.length > 0) {
            const time = new Date().toLocaleTimeString('en-US', { hour12: false });
            setDepositDetected(true);
            setDeposits(prev => [{ time, items: changes }, ...prev]);
            hapticSuccess();
            previousTokens.current = tokens;
          }
        } else {
          // First fetch — record initial baseline.
          previousTokens.current = tokens;
        }
      } catch {}

      scheduleNext();
    };

    const scheduleNext = () => {
      const elapsed = Date.now() - startTime;
      if (elapsed >= TOTAL_LISTEN_MS) return;
      const interval = elapsed < FAST_PHASE_MS ? FAST_INTERVAL_MS : SLOW_INTERVAL_MS;
      timerId = setTimeout(checkDeposit, interval);
    };

    checkDeposit();
    return () => { clearTimeout(timerId); };
  }, [address]);

  const copyAddress = useCallback(async () => {
    if (!address) return;
    await copyToClipboard(address);
    hapticLight();
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }, [address]);

  const truncatedAddress = address
    ? `${address.slice(0, 8)}...${address.slice(-6)}`
    : '';

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.title}>{t('receive.title')}</Text>
          <View style={styles.headerSpacer} />
        </View>

        {/* QR Card */}
        <Animated.View entering={fadeInDown(100, 400)}>
          <View style={styles.qrCardWrap}>
            <VelaCard elevated style={styles.qrCard}>
              {/* QR */}
              <View style={styles.qrBorder}>
                {address ? (
                  <QRCode value={address} size={200} />
                ) : (
                  <View style={styles.qrPlaceholder}>
                    <Text style={styles.qrPlaceholderText}>{t('receive.noAddress')}</Text>
                  </View>
                )}
              </View>

              {/* Identity — below the QR */}
              <Text style={styles.walletName}>{accountName}</Text>

              {/* Big, easy-to-tap copy button */}
              <Pressable
                onPress={warningDismissed ? copyAddress : undefined}
                style={[styles.copyBtn, copied && styles.copyBtnCopied]}
              >
                <Text style={[styles.copyAddr, copied && styles.copyAddrCopied]} numberOfLines={1}>{truncatedAddress}</Text>
                {copied ? (
                  <Check size={18} color={color.success.base} strokeWidth={2.5} />
                ) : (
                  <Copy size={18} color={color.fg.muted} strokeWidth={2} />
                )}
              </Pressable>

              {/* Deposit detected — surfaced as it lands */}
              {depositDetected && deposits.length > 0 && (
                <Animated.View style={styles.depositBox} entering={fadeIn(0, 300)}>
                  {deposits.map((entry, i) => (
                    <View key={i} style={[styles.depositEntry, i > 0 && styles.depositEntryBorder]}>
                      <View style={styles.depositHeader}>
                        <View style={styles.depositDot} />
                        <Text style={styles.depositTime}>{entry.time}</Text>
                      </View>
                      {entry.items.map((item, j) => (
                        <View key={j} style={styles.depositRow}>
                          <Text style={styles.depositAmount}>+{item.amount} {item.symbol}</Text>
                          <Text style={styles.depositMeta}>{item.network}{item.usd ? `  ${item.usd}` : ''}</Text>
                        </View>
                      ))}
                    </View>
                  ))}
                </Animated.View>
              )}
            </VelaCard>

            {/* Warning overlay */}
            {!warningDismissed && (
              <View style={styles.warningOverlay}>
                <View style={styles.warningContent}>
                  <View style={styles.warningIconWrap}>
                    <ShieldAlert size={28} color={color.accent.base} strokeWidth={2} />
                  </View>
                  <Text style={styles.warningTitle}>{t('receive.warningTitle')}</Text>
                  <Text style={styles.warningText}>
                    {t('receive.warningBody')}
                  </Text>
                  <Pressable style={styles.warningBtn} onPress={() => setWarningDismissed(true)}>
                    <Text style={styles.warningBtnText}>{t('receive.warningConfirm')}</Text>
                  </Pressable>
                </View>
              </View>
            )}
          </View>
        </Animated.View>

        {/* Networks */}
        <Animated.View entering={fadeInDown(200, 400)}>
          <Text style={styles.sectionLabel}>{t('receive.networksLabel', { count: networks.length })}</Text>

          <View style={styles.networkGrid}>
            {networks.map((network) => (
              <View key={network.id} style={styles.networkChip}>
                <ChainLogo
                  label={network.iconLabel}
                  color={network.iconColor}
                  bgColor={network.iconBg}
                  logoURL={network.logoURL}
                  size={22}
                />
                <Text style={styles.networkChipName} numberOfLines={1}>{network.displayName}</Text>
              </View>
            ))}
          </View>
        </Animated.View>
      </ScrollView>
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  content: {
    paddingBottom: 100,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
    marginBottom: space.md,
  },
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  title: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  headerSpacer: { minWidth: 50 },

  // QR Card wrapper (for overlay positioning)
  qrCardWrap: {
    position: 'relative',
    marginBottom: space.xl,
  },

  // Warning overlay
  warningOverlay: {
    position: 'absolute',
    top: 0,
    left: 0,
    right: 0,
    bottom: 0,
    backgroundColor: color.bg.raised,
    borderRadius: radius['2xl'],
    zIndex: 10,
    alignItems: 'center',
    justifyContent: 'center',
    padding: space['3xl'],
  },
  warningContent: {
    alignItems: 'center',
    gap: space.lg,
  },
  warningIconWrap: {
    width: 56,
    height: 56,
    borderRadius: 28,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  warningTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'center',
  },
  warningText: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 22,
  },
  warningBtn: {
    backgroundColor: color.accent.base,
    borderRadius: radius.lg,
    paddingVertical: space.lg,
    paddingHorizontal: space['4xl'],
    marginTop: space.lg,
    alignSelf: 'stretch',
    alignItems: 'center',
  },
  warningBtnText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.inverse,
  },

  // QR Card
  qrCard: {
    padding: space['3xl'],
    paddingTop: space['3xl'],
    paddingBottom: space['2xl'],
    alignItems: 'center',
  },
  qrBorder: {
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.xl,
    padding: space['2xl'],
    marginBottom: space.xl,
    backgroundColor: "#FFFFFF"
  },
  qrPlaceholder: {
    width: 200,
    height: 200,
    borderRadius: radius.xl,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
  },
  qrPlaceholderText: {
    fontSize: text.base,
    color: color.fg.subtle,
  },
  walletName: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: color.fg.base,
    marginBottom: space.lg,
  },

  // Copy button — full-width, large tap target
  copyBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    gap: space.md,
    alignSelf: 'stretch',
    paddingVertical: space.lg,
    paddingHorizontal: space.xl,
    borderRadius: radius.lg,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  copyBtnCopied: {
    backgroundColor: color.success.soft,
    borderColor: color.success.base,
  },
  copyAddr: {
    flex: 1,
    fontSize: text.base,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
  },
  copyAddrCopied: {
    color: color.success.base,
  },

  // Deposit detection
  depositBox: {
    backgroundColor: color.success.soft,
    borderRadius: radius.lg,
    marginTop: space.lg,
    width: '100%',
    overflow: 'hidden',
  },
  depositEntry: {
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
  },
  depositEntryBorder: {
    borderTopWidth: 1,
    borderTopColor: color.success.base + '20',
  },
  depositHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    marginBottom: space.sm,
  },
  depositDot: {
    width: 6,
    height: 6,
    borderRadius: 3,
    backgroundColor: color.success.base,
  },
  depositTime: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.success.base,
    opacity: 0.7,
  },
  depositRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'baseline',
    paddingLeft: 14,
    marginTop: 2,
  },
  depositAmount: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.success.base,
  },
  depositMeta: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.success.base,
    opacity: 0.7,
  },

  // Networks — compact chip grid
  sectionLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
    marginBottom: space.lg,
  },
  networkGrid: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    justifyContent: 'space-between',
    rowGap: space.md,
    marginBottom: space['4xl'],
  },
  networkChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    width: '48.5%',
  },
  networkChipName: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    flexShrink: 1,
  },
}));
