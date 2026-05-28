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
import { View, Text, TextInput, ScrollView, Pressable, Linking, Platform } from 'react-native';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { QRScanner } from '@/components/QRScanner';
import { useDAppConnection } from '@/models/dapp-connection';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { chainName } from '@/models/network';
import { parseRemoteInjectURL } from '@/services/dapp-transport';
import { showAlert } from '@/services/platform';
import { color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import {
  Radio, Unplug, Globe, QrCode, Shield, AlertTriangle,
  ExternalLink, ChevronRight, Zap, Smartphone, Monitor,
  ArrowRight, Link,
} from 'lucide-react-native';

export default function ConnectScreen() {
  const { state, activeAccount } = useWallet();
  const {
    status, errorMessage, session, chainId, dappInfo,
    connectToBridge, disconnectBridge,
  } = useDAppConnection();

  const [showScanner, setShowScanner] = useState(false);
  const [linkInput, setLinkInput] = useState('');

  const handleConnect = useCallback((data: string) => {
    const parsed = parseRemoteInjectURL(data.trim());
    if (!parsed) {
      showAlert('Invalid Link', 'This is not a valid Remote Inject connection link.');
      return;
    }
    connectToBridge(parsed);
  }, [connectToBridge]);

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
          <Text style={styles.emptyText}>Create a wallet first</Text>
        </View>
      </ScreenContainer>
    );
  }

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        <Animated.View entering={fadeIn(0, 300)}>
          <Text style={styles.pageTitle}>Connect</Text>
        </Animated.View>

        {/* ================================================================= */}
        {/* Disconnected — Guide + Scan */}
        {/* ================================================================= */}
        {status === 'disconnected' && (
          <>
            {/* How it works */}
            <Animated.View entering={fadeInDown(50, 300)}>
              <VelaCard style={styles.guideCard}>
                <Text style={styles.guideTitle}>Connect to dApps</Text>
                <Text style={styles.guideSubtitle}>
                  Use your wallet with any dApp in your desktop browser.
                </Text>

                <View style={styles.steps}>
                  <StepRow
                    number={1}
                    icon={<Monitor size={18} color={color.info.base} strokeWidth={2} />}
                    title="Install browser extension"
                    subtitle="Install the Remote Inject extension in Chrome"
                  />
                  <View style={styles.stepConnector} />
                  <StepRow
                    number={2}
                    icon={<QrCode size={18} color={color.info.base} strokeWidth={2} />}
                    title="Scan QR code"
                    subtitle="Open the extension and scan its QR code"
                  />
                  <View style={styles.stepConnector} />
                  <StepRow
                    number={3}
                    icon={<Zap size={18} color={color.info.base} strokeWidth={2} />}
                    title="Use dApps"
                    subtitle="Signing requests will appear on your phone"
                  />
                </View>
              </VelaCard>
            </Animated.View>

            {/* Actions */}
            <Animated.View entering={fadeInDown(150, 300)} style={styles.scanSection}>
              <VelaButton
                title="Scan QR Code"
                onPress={() => setShowScanner(true)}
                variant="accent"
              />

              {/* Divider */}
              <View style={styles.dividerRow}>
                <View style={styles.dividerLine} />
                <Text style={styles.dividerText}>or</Text>
                <View style={styles.dividerLine} />
              </View>

              {/* Paste link input */}
              <View style={styles.linkInputRow}>
                <TextInput
                  style={styles.linkInput}
                  value={linkInput}
                  onChangeText={setLinkInput}
                  placeholder="Paste connection link"
                  placeholderTextColor={color.fg.subtle}
                  autoCapitalize="none"
                  autoCorrect={false}
                  returnKeyType="go"
                  onSubmitEditing={handlePasteConnect}
                />
                <Pressable
                  style={[styles.linkConnectBtn, !linkInput.trim() && styles.linkConnectBtnDisabled]}
                  onPress={handlePasteConnect}
                  disabled={!linkInput.trim()}
                >
                  <ArrowRight size={18} color={!linkInput.trim() ? color.fg.subtle : color.fg.inverse} strokeWidth={2.5} />
                </Pressable>
              </View>

              <Pressable
                style={styles.extensionLink}
                onPress={() => {
                  const url = 'https://remote-inject.awesometools.dev';
                  if (Platform.OS === 'web') window.open(url, '_blank');
                  else Linking.openURL(url);
                }}
              >
                <ExternalLink size={14} color={color.accent.base} strokeWidth={2} />
                <Text style={styles.extensionLinkText}>Get the browser extension</Text>
              </Pressable>
            </Animated.View>
          </>
        )}

        {/* ================================================================= */}
        {/* Connecting */}
        {/* ================================================================= */}
        {status === 'connecting' && (
          <Animated.View entering={fadeIn(0, 300)} style={styles.centered}>
            <Radio size={28} color={color.info.base} />
            <Text style={styles.statusText}>Connecting to bridge...</Text>
          </Animated.View>
        )}

        {/* ================================================================= */}
        {/* Connected */}
        {/* ================================================================= */}
        {status === 'connected' && (
          <Animated.View entering={fadeInDown(50, 300)}>
            {/* Status card */}
            <VelaCard style={styles.connectedCard}>
              <View style={styles.connectedHeader}>
                <View style={styles.connectedDot} />
                <Text style={styles.connectedTitle}>Connected</Text>
              </View>

              {dappInfo ? (
                <View style={styles.infoRow}>
                  <Globe size={14} color={color.fg.muted} strokeWidth={2} />
                  <Text style={styles.infoText} numberOfLines={1}>
                    {dappInfo.name} ({(() => { try { return new URL(dappInfo.url).host; } catch { return dappInfo.url; } })()})
                  </Text>
                </View>
              ) : (
                <View style={styles.infoRow}>
                  <Globe size={14} color={color.fg.muted} strokeWidth={2} />
                  <Text style={styles.infoText} numberOfLines={1}>
                    {session?.serverUrl ?? 'Remote Bridge'}
                  </Text>
                </View>
              )}

              <View style={styles.infoRow}>
                <Smartphone size={14} color={color.fg.muted} strokeWidth={2} />
                <Text style={styles.infoText}>
                  {activeAccount?.name ?? 'Wallet'} ({shortAddress(activeAccount?.address ?? state.address)})
                </Text>
              </View>

              <View style={styles.infoRow}>
                <Link size={14} color={color.fg.muted} strokeWidth={2} />
                <Text style={styles.infoText}>{chainName(chainId)}</Text>
              </View>

              <Text style={styles.connectedHint}>
                Signing requests from dApps will appear automatically.
              </Text>
            </VelaCard>

            {/* Disconnect */}
            <View style={styles.disconnectSection}>
              <VelaButton
                title="Disconnect"
                onPress={disconnectBridge}
                variant="secondary"
              />
            </View>
          </Animated.View>
        )}

        {/* ================================================================= */}
        {/* Error */}
        {/* ================================================================= */}
        {status === 'error' && (
          <Animated.View entering={fadeInDown(0, 300)}>
            <VelaCard style={styles.errorCard}>
              <AlertTriangle size={28} color={color.error.base} />
              <Text style={styles.errorTitle}>Connection Failed</Text>
              <Text style={styles.errorMessage}>{errorMessage ?? 'Unable to connect to the bridge.'}</Text>
              <VelaButton
                title="Scan Again"
                onPress={() => setShowScanner(true)}
                variant="accent"
                style={styles.errorBtn}
              />
              {session && (
                <VelaButton
                  title="Retry"
                  onPress={() => connectToBridge(session)}
                  variant="secondary"
                  style={styles.retryBtn}
                />
              )}
            </VelaCard>
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
  pageTitle: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: color.fg.base,
    marginTop: space.xl,
    marginBottom: space['2xl'],
  },
  centered: {
    alignItems: 'center',
    paddingVertical: space['5xl'],
    gap: space.lg,
  },
  emptyText: { fontSize: text.lg, ...inter.regular, color: color.fg.muted },
  statusText: { fontSize: text.lg, ...inter.semibold, color: color.info.base },

  // Guide card
  guideCard: { padding: space['2xl'], gap: space.lg },
  guideTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  guideSubtitle: { fontSize: text.base, ...inter.regular, color: color.fg.muted, lineHeight: 20 },

  // Steps
  steps: { marginTop: space.lg },
  stepRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
  },
  stepIcon: {
    width: 40, height: 40, borderRadius: 20,
    backgroundColor: color.info.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  stepContent: { flex: 1, gap: 2 },
  stepTitle: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
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
    borderWidth: 1,
    borderColor: color.border.base,
  },
  linkConnectBtn: {
    width: 44,
    height: 44,
    borderRadius: radius.lg,
    backgroundColor: color.accent.base,
    alignItems: 'center',
    justifyContent: 'center',
  },
  linkConnectBtnDisabled: {
    backgroundColor: color.bg.sunken,
  },

  extensionLink: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
  },
  extensionLinkText: {
    fontSize: text.base, ...inter.semibold, color: color.accent.base,
  },

  // Connected card
  connectedCard: { padding: space['2xl'], gap: space.lg },
  connectedHeader: { flexDirection: 'row', alignItems: 'center', gap: space.md },
  connectedDot: {
    width: 10, height: 10, borderRadius: 5,
    backgroundColor: color.success.base,
  },
  connectedTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  infoRow: { flexDirection: 'row', alignItems: 'center', gap: space.md },
  infoText: {
    fontSize: text.sm, fontWeight: '500', fontFamily: font.mono,
    color: color.fg.subtle, flex: 1,
  },
  connectedHint: {
    fontSize: text.base, ...inter.regular, color: color.fg.muted,
    marginTop: space.sm,
  },

  disconnectSection: { marginTop: space['2xl'] },

  // Error card
  errorCard: { padding: space['2xl'], alignItems: 'center', gap: space.md },
  errorTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  errorMessage: {
    fontSize: text.base, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', lineHeight: 20,
  },
  errorBtn: { width: '100%', marginTop: space.md },
  retryBtn: { width: '100%' },
}));
