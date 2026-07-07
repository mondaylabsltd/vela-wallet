/**
 * Connect screen — manages the remote-inject bridge connection.
 *
 * States:
 *   - Disconnected: Shows guide + scan QR button
 *   - Connecting: Loading indicator
 *   - Connected: Shows status, session info, disconnect button
 *   - Error: Shows error + retry
 */
import React, { useState, useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { View, Text, TextInput, ScrollView, Pressable, Image } from 'react-native';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { SectionLabel } from '@/components/ui/SectionLabel';
import { QRScanner } from '@/components/QRScanner';
import { ConnectionFlowStates } from '@/components/ConnectionFlowStates';
import { useDAppConnection } from '@/models/dapp-connection';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { chainName } from '@/models/network';
import { parseRemoteInjectURL, isHttpUrl } from '@/services/dapp-transport';
import { isWalletPairURI } from '@/services/walletpair-transport';
import { showAlert } from '@/services/platform';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import {
  QrCode, Shield,
  Globe, Zap, Smartphone,
  ArrowRight, Link, Lock, X,
} from 'lucide-react-native';
import { useRouter } from 'expo-router';

export default function ConnectScreen() {
  const { t } = useTranslation();
  const router = useRouter();
  const { state, activeAccount } = useWallet();
  const {
    status, session, chainId, dappInfo, connectionType,
    connectToBridge, connectToWalletPair, disconnectBridge,
  } = useDAppConnection();

  const [showScanner, setShowScanner] = useState(false);
  const [linkInput, setLinkInput] = useState('');

  const handleConnect = useCallback((data: string) => {
    const trimmed = data.trim();

    // Auto-detect: WalletPair URI vs Remote Inject URL
    if (isWalletPairURI(trimmed)) {
      connectToWalletPair(trimmed);
      return;
    }

    const parsed = parseRemoteInjectURL(trimmed);
    if (!parsed) {
      // Not a pairing link but a real web URL → open the in-app dApp browser.
      if (isHttpUrl(trimmed)) {
        router.push({ pathname: '/browser', params: { url: trimmed } });
        return;
      }
      showAlert(t('connect.list.invalidLinkTitle'), t('connect.list.invalidLinkBody'));
      return;
    }
    connectToBridge(parsed);
  }, [connectToBridge, connectToWalletPair, router, t]);

  const handleScan = useCallback((data: string) => {
    setShowScanner(false);
    handleConnect(data);
  }, [handleConnect]);

  const handlePasteConnect = useCallback(() => {
    if (!linkInput.trim()) return;
    handleConnect(linkInput);
    setLinkInput('');
  }, [linkInput, handleConnect]);

  if (!state.hasWallet) {
    return (
      <ScreenContainer>
        <View style={styles.centered}>
          <Shield size={32} color={color.fg.subtle} />
          <Text style={styles.emptyText}>{t('connect.list.noWallet')}</Text>
        </View>
      </ScreenContainer>
    );
  }

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        <Animated.View entering={fadeIn(0, 300)} style={styles.pageHeader}>
          <Text style={styles.pageTitle}>{t('connect.list.pageTitle')}</Text>
          <Pressable onPress={() => router.navigate('/wallet')} hitSlop={8} style={styles.pageClose}>
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </Animated.View>

        {/* ================================================================= */}
        {/* Disconnected — Guide + Scan */}
        {/* ================================================================= */}
        {status === 'disconnected' && (
          <>
            {/* How it works — open section, not a card */}
            <Animated.View entering={fadeInDown(50, 300)}>
              <SectionLabel>{t('connect.list.guideTitle')}</SectionLabel>

              <View style={styles.steps}>
                <StepRow
                  number={1}
                  icon={<QrCode size={18} color={color.accent.base} strokeWidth={2} />}
                  title={t('connect.list.step1Title')}
                  subtitle={t('connect.list.step1Subtitle')}
                />
                <View style={styles.stepConnector} />
                <StepRow
                  number={2}
                  icon={<Lock size={12} color={color.accent.base} strokeWidth={2} />}
                  title={t('connect.list.step2Title')}
                  subtitle={t('connect.list.step2Subtitle')}
                />
                <View style={styles.stepConnector} />
                <StepRow
                  number={3}
                  icon={<Zap size={18} color={color.accent.base} strokeWidth={2} />}
                  title={t('connect.list.step3Title')}
                  subtitle={t('connect.list.step3Subtitle')}
                />
              </View>
            </Animated.View>

            {/* Actions */}
            <Animated.View entering={fadeInDown(150, 300)} style={styles.scanSection}>
              <VelaButton
                title={t('connect.list.scanQR')}
                onPress={() => setShowScanner(true)}
                variant="accent"
              />

              {/* Divider */}
              <View style={styles.dividerRow}>
                <View style={styles.dividerLine} />
                <Text style={styles.dividerText}>{t('connect.list.orDivider')}</Text>
                <View style={styles.dividerLine} />
              </View>

              {/* Paste link input */}
              <View style={styles.linkInputRow}>
                <TextInput
                  style={styles.linkInput}
                  value={linkInput}
                  onChangeText={setLinkInput}
                  placeholder={t('connect.list.pastePlaceholder')}
                  placeholderTextColor={color.fg.subtle}
                  autoCapitalize="none"
                  autoCorrect={false}
                  returnKeyType="go"
                  onSubmitEditing={handlePasteConnect}
                />
                <Pressable
                  style={styles.linkConnectBtn}
                  onPress={handlePasteConnect}
                  disabled={!linkInput.trim()}
                  hitSlop={8}
                >
                  <ArrowRight size={20} color={!linkInput.trim() ? color.fg.subtle : color.accent.base} strokeWidth={2.5} />
                </Pressable>
              </View>
            </Animated.View>
          </>
        )}

        {/* ================================================================= */}
        {/* Connecting (fingerprint / waiting) + Error — shared inline states */}
        {/* ================================================================= */}
        <ConnectionFlowStates onScanAgain={() => setShowScanner(true)} />

        {/* ================================================================= */}
        {/* Connected */}
        {/* ================================================================= */}
        {(status === 'connected' || status === 'reconnecting') && (
          <Animated.View entering={fadeInDown(50, 300)}>
            {/* Status — open section, grouped by a label + hairline rows */}
            <View style={styles.connectedHeader}>
              <View style={[styles.connectedDot, status === 'reconnecting' && styles.reconnectingDot]} />
              <Text style={styles.connectedTitle}>
                {status === 'reconnecting' ? t('connect.list.reconnecting') : t('connect.list.connected')}
              </Text>
              {connectionType === 'walletpair' && (
                <View style={styles.encryptedBadge}>
                  <Lock size={10} color={color.success.base} strokeWidth={2.5} />
                  <Text style={styles.encryptedBadgeText}>E2E</Text>
                </View>
              )}
            </View>

            <View style={styles.infoGroup}>
              {dappInfo ? (
                <View style={styles.infoRow}>
                  {dappInfo.icon ? (
                    <Image source={{ uri: dappInfo.icon }} style={styles.dappIcon} />
                  ) : (
                    <Globe size={14} color={color.fg.muted} strokeWidth={2} />
                  )}
                  <Text style={styles.infoText} numberOfLines={1}>
                    {dappInfo.name}{dappInfo.url ? ` (${(() => { try { return new URL(dappInfo.url).host; } catch { return dappInfo.url; } })()})` : ''}
                  </Text>
                </View>
              ) : (
                <View style={styles.infoRow}>
                  <Globe size={14} color={color.fg.muted} strokeWidth={2} />
                  <Text style={styles.infoText} numberOfLines={1}>
                    {session?.serverUrl ?? t('connect.list.remoteBridge')}
                  </Text>
                </View>
              )}

              <View style={styles.infoSep} />

              <View style={styles.infoRow}>
                <Smartphone size={14} color={color.fg.muted} strokeWidth={2} />
                <Text style={styles.infoText}>
                  {activeAccount?.name ?? t('connect.list.walletFallback')} ({shortAddress(activeAccount?.address ?? state.address)})
                </Text>
              </View>

              <View style={styles.infoSep} />

              <View style={styles.infoRow}>
                <Link size={14} color={color.fg.muted} strokeWidth={2} />
                <Text style={styles.infoText}>{chainName(chainId)}</Text>
              </View>
            </View>

            <Text style={styles.connectedHint}>
              {t('connect.list.signingHint')}
            </Text>

            {/* Disconnect */}
            <View style={styles.disconnectSection}>
              <VelaButton
                title={t('connect.list.disconnect')}
                onPress={disconnectBridge}
                variant="secondary"
              />
            </View>
          </Animated.View>
        )}
      </ScrollView>

      {/* QR Scanner modal */}
      <QRScanner
        visible={showScanner}
        onScan={handleScan}
        onClose={() => setShowScanner(false)}
      />
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// Step row component
// ---------------------------------------------------------------------------

function StepRow({ number, icon, title, subtitle }: {
  number: number;
  icon: React.ReactNode;
  title: string;
  subtitle: string;
}) {
  return (
    <View style={styles.stepRow}>
      <View style={styles.stepIcon}>
        {icon}
      </View>
      <View style={styles.stepContent}>
        <Text style={styles.stepTitle}>{title}</Text>
        <Text style={styles.stepSubtitle}>{subtitle}</Text>
      </View>
    </View>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  scrollContent: { paddingBottom: space['5xl'] },
  pageHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginTop: space.xl,
    marginBottom: space['2xl'],
  },
  pageTitle: {
    fontSize: text['3xl'],
    ...inter.bold,
    color: color.fg.base,
    letterSpacing: -0.5,
  },
  pageClose: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  centered: {
    alignItems: 'center',
    paddingVertical: space['5xl'],
    gap: space.lg,
  },
  emptyText: { fontSize: text.lg, ...inter.regular, color: color.fg.muted },

  // Steps
  steps: { marginTop: space.md },
  stepRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  stepIcon: {
    width: 40, height: 40, borderRadius: 20,
    backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  stepContent: { flex: 1, gap: 2 },
  stepTitle: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  stepSubtitle: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  stepConnector: {
    width: 2, height: 16, marginLeft: 19,
    backgroundColor: color.border.base,
  },

  // Scan section
  scanSection: { marginTop: space['2xl'], gap: space.xl },

  // Or divider
  dividerRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  dividerLine: {
    flex: 1,
    height: 1,
    backgroundColor: color.border.base,
  },
  dividerText: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },

  // Link input
  linkInputRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
  },
  linkInput: {
    flex: 1,
    fontSize: text.sm,
    fontWeight: '500' as const,
    fontFamily: font.mono,
    color: color.fg.base,
    padding: space.lg,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
  },
  // Plain icon button (no bg/border/shadow), ≥44×44 hit target.
  linkConnectBtn: {
    width: 44,
    height: 44,
    alignItems: 'center',
    justifyContent: 'center',
  },

  // Connected — open section (no card)
  connectedHeader: { flexDirection: 'row', alignItems: 'center', gap: space.md, marginBottom: space.lg },
  encryptedBadge: {
    flexDirection: 'row', alignItems: 'center', gap: 3,
    paddingHorizontal: space.sm, paddingVertical: 2,
    backgroundColor: color.success.soft, borderRadius: radius.full,
    marginLeft: 'auto',
  },
  encryptedBadgeText: {
    fontSize: text.xs, ...inter.semibold, color: color.success.base,
  },
  connectedDot: {
    width: 10, height: 10, borderRadius: 5,
    backgroundColor: color.success.base,
  },
  reconnectingDot: {
    backgroundColor: color.warning.base,
    opacity: 0.7,
  },
  connectedTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  infoGroup: {},
  infoRow: { flexDirection: 'row', alignItems: 'center', gap: space.md, paddingVertical: space.lg },
  // Hairline between de-boxed info rows, inset past the leading 14px icon + gap.
  infoSep: { height: 1, backgroundColor: color.border.base, marginLeft: 14 + space.md },
  infoText: {
    fontSize: text.sm, fontWeight: '500', fontFamily: font.mono,
    color: color.fg.subtle, flex: 1,
  },
  connectedHint: {
    fontSize: text.base, ...inter.regular, color: color.fg.muted,
    marginTop: space.lg,
  },

  disconnectSection: { marginTop: space['2xl'] },

  dappIcon: { width: 14, height: 14, borderRadius: 3 },
}));
