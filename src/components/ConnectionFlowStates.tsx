/**
 * Shared pairing lifecycle states — fingerprint verification, "waiting for the
 * dApp", and connection errors. Rendered inline by both the dedicated Connect
 * screen and the Home → Connections panel so a pairing never yanks the user to
 * a different screen.
 *
 * The `disconnected` and `connected` states are owned by each host (their
 * layouts differ); this component renders nothing for those.
 */
import React from 'react';
import { useTranslation } from 'react-i18next';
import { View, Text, Image } from 'react-native';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { VelaButton } from '@/components/ui/VelaButton';
import { useDAppConnection } from '@/models/dapp-connection';
import { scaleFont, color, text, inter, space, radius, font, createStyles } from '@/constants/theme';
import { Radio, Globe, AlertTriangle, Lock, Fingerprint } from 'lucide-react-native';

export function ConnectionFlowStates({ onScanAgain }: { onScanAgain: () => void }) {
  const { t } = useTranslation();
  const {
    status, errorMessage, session, dappInfo, pendingFingerprint,
    confirmFingerprint, cancelFingerprint, connectToBridge, disconnectBridge,
  } = useDAppConnection();

  // ----- Connecting — fingerprint verification (WalletPair) -----
  if (status === 'connecting' && pendingFingerprint) {
    return (
      <Animated.View entering={fadeInDown(50, 300)}>
        <View style={styles.fingerprintCard}>
          <View style={styles.fingerprintHeader}>
            <Fingerprint size={28} color={color.accent.base} strokeWidth={2} />
            <Text style={styles.fingerprintTitle}>{t('connect.list.verifyTitle')}</Text>
          </View>

          <Text style={styles.fingerprintHint}>{t('connect.list.verifyHint')}</Text>

          <View style={styles.fingerprintCodeRow}>
            {pendingFingerprint.split('').map((digit, i) => (
              <View key={i} style={styles.fingerprintDigitBox}>
                <Text style={styles.fingerprintDigit}>{digit}</Text>
              </View>
            ))}
          </View>

          {dappInfo && (
            <View style={styles.fingerprintDapp}>
              {dappInfo.icon ? (
                <Image source={{ uri: dappInfo.icon }} style={styles.dappIcon} />
              ) : (
                <Globe size={14} color={color.fg.muted} strokeWidth={2} />
              )}
              <Text style={styles.fingerprintDappText} numberOfLines={1}>{dappInfo.name}</Text>
            </View>
          )}

          <View style={styles.fingerprintBadge}>
            <Lock size={12} color={color.success.base} strokeWidth={2.5} />
            <Text style={styles.fingerprintBadgeText}>{t('connect.list.encryptedBadge')}</Text>
          </View>

          <View style={styles.fingerprintActions}>
            <VelaButton
              title={t('connect.list.confirm')}
              onPress={confirmFingerprint}
              variant="accent"
              style={styles.fingerprintBtn}
            />
            <VelaButton
              title={t('connect.list.cancel')}
              onPress={cancelFingerprint}
              variant="secondary"
              style={styles.fingerprintBtn}
            />
          </View>
        </View>
      </Animated.View>
    );
  }

  // ----- Connecting — waiting for the dApp to accept (remote bridge) -----
  if (status === 'connecting') {
    return (
      <Animated.View entering={fadeIn(0, 300)} style={styles.centered}>
        <View style={styles.waitingIconWrap}>
          <Radio size={32} color={color.accent.base} />
        </View>
        <Text style={styles.statusText}>{t('connect.list.waitingStatus')}</Text>
        <Text style={styles.statusHint}>{t('connect.list.waitingHint')}</Text>
        <VelaButton
          title={t('connect.list.cancel')}
          onPress={disconnectBridge}
          variant="secondary"
          compact
          style={styles.cancelBtn}
        />
      </Animated.View>
    );
  }

  // ----- Error -----
  if (status === 'error') {
    return (
      <Animated.View entering={fadeInDown(0, 300)} style={styles.errorCard}>
        <View style={styles.errorIconWrap}>
          <AlertTriangle size={28} color={color.error.base} />
        </View>
        <Text style={styles.errorTitle}>{t('connect.list.connFailed')}</Text>
        <Text style={styles.errorMessage}>{errorMessage ?? t('connect.list.connError')}</Text>
        <VelaButton
          title={t('connect.list.scanAgain')}
          onPress={onScanAgain}
          variant="accent"
          style={styles.errorBtn}
        />
        {session && (
          <VelaButton
            title={t('connect.list.retry')}
            onPress={() => connectToBridge(session)}
            variant="secondary"
            style={styles.retryBtn}
          />
        )}
      </Animated.View>
    );
  }

  return null;
}

const styles = createStyles(() => ({
  centered: { alignItems: 'center', paddingVertical: space['5xl'], gap: space.lg },
  statusText: { fontSize: text.lg, ...inter.semibold, color: color.accent.base },
  statusHint: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center' },

  dappIcon: { width: 14, height: 14, borderRadius: 3 },

  // Fingerprint verification — a deliberate security GATE, so it stays a surface,
  // but a LIGHT one: soft sunken fill + hairline, no card shadow/raised bg.
  fingerprintCard: {
    padding: space['2xl'], gap: space.lg, alignItems: 'center',
    backgroundColor: color.bg.sunken,
    borderRadius: radius.xl,
    borderWidth: 1, borderColor: color.border.base,
  },
  fingerprintHeader: { flexDirection: 'row', alignItems: 'center', gap: space.md },
  fingerprintTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  fingerprintHint: {
    fontSize: text.base, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', lineHeight: 20,
  },
  fingerprintCodeRow: { flexDirection: 'row', gap: space.lg, marginVertical: space.xl },
  fingerprintDigitBox: {
    width: 52, height: 64, borderRadius: radius.lg,
    backgroundColor: color.bg.raised,
    borderWidth: 1, borderColor: color.border.strong,
    alignItems: 'center', justifyContent: 'center',
  },
  fingerprintDigit: {
    fontSize: scaleFont(28), fontWeight: '700' as const, fontFamily: font.mono, color: color.fg.base,
  },
  fingerprintDapp: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  fingerprintDappText: {
    fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.subtle,
  },
  fingerprintBadge: {
    flexDirection: 'row', alignItems: 'center', gap: 4,
    paddingHorizontal: space.md, paddingVertical: 4,
    backgroundColor: color.success.soft, borderRadius: radius.full,
  },
  fingerprintBadgeText: { fontSize: text.xs, ...inter.semibold, color: color.success.base },
  fingerprintActions: { width: '100%', gap: space.md, marginTop: space.md },
  fingerprintBtn: { width: '100%' },

  // Waiting
  waitingIconWrap: {
    width: 64, height: 64, borderRadius: 32,
    backgroundColor: color.accent.base + '12',
    alignItems: 'center', justifyContent: 'center',
  },
  cancelBtn: { marginTop: space.lg },

  // Error — open, typographic state (no card), matching the waiting state.
  errorCard: { paddingVertical: space['4xl'], alignItems: 'center', gap: space.md },
  errorIconWrap: {
    width: 64, height: 64, borderRadius: 32,
    backgroundColor: color.error.soft,
    alignItems: 'center', justifyContent: 'center', marginBottom: space.sm,
  },
  errorTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  errorMessage: {
    fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center', lineHeight: 20,
  },
  errorBtn: { width: '100%', marginTop: space.md },
  retryBtn: { width: '100%' },
}));
