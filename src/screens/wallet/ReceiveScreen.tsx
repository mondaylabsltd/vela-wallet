import React, { useState, useEffect, useRef } from 'react';
import { View, Text, Pressable, Alert, ScrollView, Share, AppState } from 'react-native';
import { useRouter } from 'expo-router';
import * as Clipboard from 'expo-clipboard';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withRepeat,
  withSequence,
  withTiming,
  FadeIn,
  FadeInDown,
} from 'react-native-reanimated';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { ChainLogo } from '@/components/ChainLogo';
import { color, text, weight, space, radius, font, shadow, motion, createStyles } from '@/constants/theme';
import { useWallet } from '@/models/wallet-state';
import { DEFAULT_NETWORKS } from '@/models/network';
import { QRCode } from '@/components/QRCode';
import { fetchTokens } from '@/services/wallet-api';
import { tokenUsdValue } from '@/models/types';
import * as Haptics from 'expo-haptics';
import { Copy, Share2, Check } from 'lucide-react-native';

const DEPOSIT_CHECK_MS = 5 * 60 * 1000;
const MAX_DEPOSIT_CHECKS = 3;

function PulsingDot() {
  const opacity = useSharedValue(1);

  useEffect(() => {
    opacity.value = withRepeat(
      withSequence(
        withTiming(0.3, { duration: 800 }),
        withTiming(1, { duration: 800 }),
      ),
      -1,
      false,
    );
  }, [opacity]);

  const animatedStyle = useAnimatedStyle(() => ({
    opacity: opacity.value,
  }));

  return (
    <Animated.View style={[styles.listeningDot, animatedStyle]} />
  );
}

export default function ReceiveScreen() {
  const router = useRouter();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const [isListening, setIsListening] = useState(false);
  const [depositDetected, setDepositDetected] = useState(false);
  const [copied, setCopied] = useState(false);
  const previousBalance = useRef<number | null>(null);
  const checkCount = useRef(0);

  // Deposit detection polling
  useEffect(() => {
    if (!address) return;
    setIsListening(true);
    previousBalance.current = null;
    checkCount.current = 0;

    const checkDeposit = async () => {
      if (AppState.currentState !== 'active') return;
      try {
        const tokens = await fetchTokens(address);
        const total = tokens.reduce((sum, t) => sum + tokenUsdValue(t), 0);

        if (previousBalance.current !== null && total > previousBalance.current) {
          setDepositDetected(true);
          Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
          setTimeout(() => setDepositDetected(false), 5000);
        }
        previousBalance.current = total;
      } catch {}
    };

    checkDeposit();
    const timer = setInterval(() => {
      checkCount.current += 1;
      if (checkCount.current > MAX_DEPOSIT_CHECKS) {
        clearInterval(timer);
        setIsListening(false);
        return;
      }
      checkDeposit();
    }, DEPOSIT_CHECK_MS);
    return () => { clearInterval(timer); setIsListening(false); };
  }, [address]);

  const copyAddress = async () => {
    if (!address) return;
    await Clipboard.setStringAsync(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  };

  const shareAddress = async () => {
    if (!address) return;
    try {
      await Share.share({
        message: address,
        title: `${accountName} Address`,
      });
    } catch {}
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8}>
            <Text style={styles.backBtn}>Back</Text>
          </Pressable>
          <Text style={styles.title}>Receive</Text>
          <View style={styles.headerSpacer} />
        </View>

        {/* QR Card */}
        <Animated.View entering={FadeInDown.delay(100).duration(400)}>
          <VelaCard elevated style={styles.qrCard}>
            <View style={styles.qrContainer}>
              {address ? (
                <View style={styles.qrFrame}>
                  <QRCode value={address} size={200} />
                </View>
              ) : (
                <View style={styles.qrPlaceholder}>
                  <Text style={styles.qrPlaceholderText}>No address</Text>
                </View>
              )}
            </View>

            {/* Full address */}
            <Pressable onPress={copyAddress} style={styles.addressBox}>
              <Text style={styles.addressText} selectable>{address}</Text>
            </Pressable>

            {/* Status indicator */}
            {isListening && !depositDetected && (
              <Animated.View style={styles.listeningRow} entering={FadeIn.duration(300)}>
                <PulsingDot />
                <Text style={styles.listeningText}>Listening for deposits</Text>
              </Animated.View>
            )}

            {depositDetected && (
              <Animated.View style={styles.depositAlert} entering={FadeIn.duration(300)}>
                <Check size={16} color={color.success.base} strokeWidth={3} />
                <Text style={styles.depositText}>Deposit received!</Text>
              </Animated.View>
            )}
          </VelaCard>
        </Animated.View>

        {/* Action buttons */}
        <Animated.View style={styles.buttonRow} entering={FadeInDown.delay(200).duration(400)}>
          <VelaButton
            title="Copy Address"
            onPress={copyAddress}
            style={styles.actionButton}
          />
          <VelaButton
            title="Share"
            onPress={shareAddress}
            variant="secondary"
            style={styles.actionButton}
          />
        </Animated.View>

        {/* Supported networks */}
        <Animated.View entering={FadeInDown.delay(300).duration(400)}>
          <Text style={styles.sectionTitle}>Supported Networks</Text>
          <Text style={styles.sectionSubtitle}>
            Same address across all EVM networks
          </Text>

          <VelaCard style={styles.networksCard}>
            {DEFAULT_NETWORKS.map((network, index) => (
              <View key={network.id}>
                {index > 0 && <View style={styles.separator} />}
                <View style={styles.networkRow}>
                  <ChainLogo
                    label={network.iconLabel}
                    color={network.iconColor}
                    bgColor={network.iconBg}
                    logoURL={network.logoURL}
                    size={32}
                  />
                  <Text style={styles.networkName}>{network.displayName}</Text>
                  {network.isL2 && (
                    <View style={styles.networkBadge}>
                      <Text style={styles.networkBadgeText}>L2</Text>
                    </View>
                  )}
                </View>
              </View>
            ))}
          </VelaCard>
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
  backBtn: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: color.accent.base,
    minWidth: 50,
  },
  title: {
    fontSize: text.xl,
    fontWeight: weight.bold,
    color: color.fg.base,
  },
  headerSpacer: { minWidth: 50 },

  // QR Card
  qrCard: {
    padding: space['3xl'],
    alignItems: 'center',
    marginBottom: space['2xl'],
  },
  qrContainer: {
    marginBottom: space['2xl'],
  },
  qrFrame: {
    padding: space.xl,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    ...shadow.sm,
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
  addressBox: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: space.xl,
    width: '100%',
    marginBottom: space.lg,
  },
  addressText: {
    fontSize: text.sm,
    fontWeight: weight.medium,
    fontFamily: font.mono,
    color: color.fg.base,
    textAlign: 'center',
    lineHeight: 20,
  },

  // Listening
  listeningRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.sm,
  },
  listeningDot: {
    width: 8,
    height: 8,
    borderRadius: 4,
    backgroundColor: color.success.base,
  },
  listeningText: {
    fontSize: text.sm,
    fontWeight: weight.medium,
    color: color.success.base,
  },

  // Deposit
  depositAlert: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.success.soft,
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
    borderRadius: radius.lg,
    width: '100%',
  },
  depositText: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.success.base,
  },

  // Buttons
  buttonRow: {
    flexDirection: 'row',
    gap: space.lg,
    marginBottom: space['4xl'],
  },
  actionButton: {
    flex: 1,
  },

  // Networks
  sectionTitle: {
    fontSize: text.lg,
    fontWeight: weight.bold,
    color: color.fg.base,
    marginBottom: space.sm,
  },
  sectionSubtitle: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.muted,
    marginBottom: space.xl,
  },
  networksCard: {
    paddingVertical: space.md,
  },
  networkRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.lg,
    paddingHorizontal: space['2xl'],
  },
  networkName: {
    fontSize: text.base,
    fontWeight: weight.semibold,
    color: color.fg.base,
    flex: 1,
  },
  networkBadge: {
    backgroundColor: color.info.soft,
    paddingHorizontal: space.md,
    paddingVertical: 2,
    borderRadius: radius.sm,
  },
  networkBadgeText: {
    fontSize: text.xs,
    fontWeight: weight.semibold,
    color: color.info.base,
  },
  separator: {
    height: 1,
    backgroundColor: color.border.base,
    marginHorizontal: space['2xl'],
  },
}));
