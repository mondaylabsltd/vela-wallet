import React, { useState, useEffect, useRef, useCallback } from 'react';
import { View, Text, TouchableOpacity, StyleSheet, Alert, ScrollView, Share, AppState } from 'react-native';
import { useRouter } from 'expo-router';
import * as Clipboard from 'expo-clipboard';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaColor, VelaFont, VelaRadius, VelaSpacing } from '@/constants/theme';
import { useWallet } from '@/models/wallet-state';
import { DEFAULT_NETWORKS } from '@/models/network';
import { QRCode } from '@/components/QRCode';
import { fetchTokens } from '@/services/wallet-api';
import { tokenUsdValue } from '@/models/types';
import * as Haptics from 'expo-haptics';

const DEPOSIT_CHECK_MS = 5 * 60 * 1000;
const MAX_DEPOSIT_CHECKS = 3;

export default function ReceiveScreen() {
  const router = useRouter();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;
  const accountName = activeAccount?.name ?? 'Wallet';

  const [isListening, setIsListening] = useState(false);
  const [depositDetected, setDepositDetected] = useState(false);
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
          // Reset after 5 seconds
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
    Alert.alert('Copied', 'Address copied to clipboard.');
  };

  const shareAddress = async () => {
    if (!address) return;
    try {
      await Share.share({
        message: address,
        title: `${accountName} Address`,
      });
    } catch {
      // User cancelled share
    }
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        {/* Header */}
        <View style={styles.header}>
          <TouchableOpacity onPress={() => router.back()} activeOpacity={0.7}>
            <Text style={styles.backBtn}>Back</Text>
          </TouchableOpacity>
          <Text style={styles.title}>Receive</Text>
          <View style={{ width: 50 }} />
        </View>

        {/* Address display */}
        <VelaCard style={styles.addressCard}>
          <Text style={styles.addressLabel}>Your Wallet Address</Text>

          {/* QR Code */}
          <View style={styles.qrContainer}>
            {address ? (
              <QRCode value={address} size={200} />
            ) : (
              <View style={styles.qrPlaceholder}>
                <Text style={styles.qrIcon}>QR</Text>
                <Text style={styles.qrHint}>QR Code</Text>
              </View>
            )}
          </View>

          {/* Full address */}
          <TouchableOpacity onPress={copyAddress} activeOpacity={0.7} style={styles.addressBox}>
            <Text style={styles.addressText} selectable>{address}</Text>
          </TouchableOpacity>

          <Text style={styles.tapHint}>Tap address to copy</Text>
        </VelaCard>

        {/* Action buttons */}
        <View style={styles.buttonRow}>
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
        </View>

        {/* Listening indicator */}
        {isListening && !depositDetected && (
          <View style={styles.listeningRow}>
            <View style={styles.listeningDot} />
            <Text style={styles.listeningText}>Listening for deposits...</Text>
          </View>
        )}

        {/* Deposit detected */}
        {depositDetected && (
          <View style={styles.depositAlert}>
            <Text style={styles.depositText}>Deposit received!</Text>
          </View>
        )}

        {/* Supported networks */}
        <Text style={styles.sectionTitle}>Supported Networks</Text>
        <Text style={styles.sectionSubtitle}>
          This address is the same across all EVM networks
        </Text>

        <VelaCard style={styles.networksCard}>
          {DEFAULT_NETWORKS.map((network, index) => (
            <View key={network.id}>
              {index > 0 && <View style={styles.separator} />}
              <View style={styles.networkRow}>
                <View style={[styles.networkIcon, { backgroundColor: network.iconBg }]}>
                  <Text style={[styles.networkIconText, { color: network.iconColor }]}>
                    {network.iconLabel}
                  </Text>
                </View>
                <View style={styles.networkInfo}>
                  <Text style={styles.networkName}>{network.displayName}</Text>
                  {network.isL2 && <Text style={styles.networkBadge}>L2</Text>}
                </View>
              </View>
            </View>
          ))}
        </VelaCard>

        <Text style={styles.warning}>
          All supported networks share the same wallet address. Make sure the sender uses the correct network.
        </Text>
      </ScrollView>
    </ScreenContainer>
  );
}

const styles = StyleSheet.create({
  content: {
    paddingBottom: 100,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: 12,
    marginBottom: 8,
  },
  backBtn: {
    ...VelaFont.title(16),
    color: VelaColor.accent,
    width: 50,
  },
  title: {
    ...VelaFont.title(17),
    color: VelaColor.textPrimary,
  },
  addressCard: {
    padding: VelaSpacing.cardPadding,
    alignItems: 'center',
    marginBottom: 20,
  },
  addressLabel: {
    ...VelaFont.label(13),
    color: VelaColor.textSecondary,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
    marginBottom: 20,
  },
  qrContainer: {
    marginBottom: 20,
  },
  qrPlaceholder: {
    width: 180,
    height: 180,
    borderRadius: VelaRadius.card,
    backgroundColor: VelaColor.bgWarm,
    borderWidth: 1,
    borderColor: VelaColor.border,
    alignItems: 'center',
    justifyContent: 'center',
    gap: 8,
  },
  qrIcon: {
    fontSize: 64,
    color: VelaColor.textTertiary,
  },
  qrHint: {
    ...VelaFont.body(13),
    color: VelaColor.textTertiary,
  },
  addressBox: {
    backgroundColor: VelaColor.bgWarm,
    borderRadius: VelaRadius.cardSmall,
    padding: 14,
    width: '100%',
  },
  addressText: {
    ...VelaFont.mono(13),
    color: VelaColor.textPrimary,
    textAlign: 'center',
    lineHeight: 20,
  },
  tapHint: {
    ...VelaFont.body(12),
    color: VelaColor.textTertiary,
    marginTop: 8,
  },
  buttonRow: {
    flexDirection: 'row',
    gap: 12,
    marginBottom: 32,
  },
  actionButton: {
    flex: 1,
  },
  sectionTitle: {
    ...VelaFont.title(17),
    color: VelaColor.textPrimary,
    marginBottom: 4,
  },
  sectionSubtitle: {
    ...VelaFont.body(13),
    color: VelaColor.textSecondary,
    marginBottom: 14,
  },
  networksCard: {
    padding: VelaSpacing.cardPadding,
  },
  networkRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 12,
    paddingVertical: 10,
  },
  networkIcon: {
    width: 36,
    height: 36,
    borderRadius: 18,
    alignItems: 'center',
    justifyContent: 'center',
  },
  networkIconText: {
    ...VelaFont.label(11),
  },
  networkInfo: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
  },
  networkName: {
    ...VelaFont.title(15),
    color: VelaColor.textPrimary,
  },
  networkBadge: {
    ...VelaFont.caption(),
    color: VelaColor.blue,
    backgroundColor: VelaColor.blueSoft,
    paddingHorizontal: 6,
    paddingVertical: 2,
    borderRadius: 4,
    overflow: 'hidden',
  },
  separator: {
    height: 1,
    backgroundColor: VelaColor.border,
  },
  listeningRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: 8, marginVertical: 12 },
  listeningDot: { width: 8, height: 8, borderRadius: 4, backgroundColor: VelaColor.green },
  listeningText: { ...VelaFont.body(14), color: VelaColor.green },
  depositAlert: { backgroundColor: VelaColor.greenSoft, padding: 16, borderRadius: VelaRadius.cardSmall, marginVertical: 12, alignItems: 'center' },
  depositText: { ...VelaFont.title(16), color: VelaColor.green },
  warning: { ...VelaFont.body(13), color: VelaColor.textTertiary, textAlign: 'center', marginTop: 16, lineHeight: 18 },
});
