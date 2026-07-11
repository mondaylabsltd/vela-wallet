/**
 * Clear Signing test screen — preview all signing modal scenarios.
 *
 * Triggers mock signing requests to test the SigningRequestModal UI
 * with different ERC-7730 scenarios.
 */
import React, { useState, useCallback } from 'react';
import { View, Text, ScrollView, Pressable } from 'react-native';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { color, text, inter, space, createStyles } from '@/constants/theme';
import {
  ArrowRightLeft, Send, CheckCircle, Pen, FileText, ShieldAlert,
  Zap, ChevronLeft,
} from 'lucide-react-native';
import { useRouter } from 'expo-router';
import type { BLEIncomingRequest } from '@/models/types';
import { useTranslation } from 'react-i18next';
import { CLEAR_SIGNING_SCENARIOS } from '@/screens/settings/clear-signing-scenarios';
import { AppModal } from '@/components/ui/AppModal';
import { SigningSheet } from '@/components/SigningRequestModal';
import { showAlert } from '@/services/platform';
import { useWallet } from '@/models/wallet-state';

// ---------------------------------------------------------------------------
// Per-scenario icon (presentation only; the request fixtures live in the shared
// clear-signing-scenarios module so the same data can be unit-tested).
// ---------------------------------------------------------------------------

const SCENARIO_ICONS: Record<string, { icon: React.ReactNode; iconBg: string }> = {
  'erc20-transfer': { icon: <Send size={18} color="#E8572A" strokeWidth={2} />, iconBg: '#FFF0EB' },
  'erc20-approve': { icon: <CheckCircle size={18} color="#d4890a" strokeWidth={2} />, iconBg: '#FFF8F0' },
  'eth-transfer': { icon: <Send size={18} color="#E8572A" strokeWidth={2} />, iconBg: '#FFF0EB' },
  'personal-sign': { icon: <Pen size={18} color="#6c5ce7" strokeWidth={2} />, iconBg: '#EEF0FF' },
  'eip712-permit': { icon: <FileText size={18} color="#6c5ce7" strokeWidth={2} />, iconBg: '#EEF0FF' },
  'eip712-unknown': { icon: <FileText size={18} color="#d4890a" strokeWidth={2} />, iconBg: '#FFF8F0' },
  'blind-tx': { icon: <ShieldAlert size={18} color="#d43a2a" strokeWidth={2} />, iconBg: '#FEF2F2' },
  '1inch-swap': { icon: <ArrowRightLeft size={18} color="#E8572A" strokeWidth={2} />, iconBg: '#FFF0EB' },
  'nft-transfer': { icon: <Send size={18} color="#6c5ce7" strokeWidth={2} />, iconBg: '#EEF0FF' },
  'nft-approve-all': { icon: <CheckCircle size={18} color="#d4890a" strokeWidth={2} />, iconBg: '#FFF8F0' },
  'vault-deposit': { icon: <Zap size={18} color="#22a456" strokeWidth={2} />, iconBg: '#EDFAF2' },
  'vault-withdraw': { icon: <Zap size={18} color="#6c5ce7" strokeWidth={2} />, iconBg: '#EEF0FF' },
  'erc20-transferFrom': { icon: <Send size={18} color="#E8572A" strokeWidth={2} />, iconBg: '#FFF0EB' },
  'hex-message': { icon: <Pen size={18} color="#d4890a" strokeWidth={2} />, iconBg: '#FFF8F0' },
  'large-eth-send': { icon: <Send size={18} color="#d43a2a" strokeWidth={2} />, iconBg: '#FEF2F2' },
  'erc20-approve-limited': { icon: <CheckCircle size={18} color="#22a456" strokeWidth={2} />, iconBg: '#EDFAF2' },
  'eth-sign': { icon: <ShieldAlert size={18} color="#d43a2a" strokeWidth={2} />, iconBg: '#FEF2F2' },
  'siwe-phish': { icon: <Pen size={18} color="#d43a2a" strokeWidth={2} />, iconBg: '#FEF2F2' },
  'increase-allowance': { icon: <CheckCircle size={18} color="#d4890a" strokeWidth={2} />, iconBg: '#FFF8F0' },
  'batch-calls': { icon: <Zap size={18} color="#6c5ce7" strokeWidth={2} />, iconBg: '#EEF0FF' },
  'expired-swap': { icon: <ArrowRightLeft size={18} color="#d4890a" strokeWidth={2} />, iconBg: '#FFF8F0' },
};
const DEFAULT_ICON = { icon: <FileText size={18} color={color.fg.muted} strokeWidth={2} />, iconBg: '#F1F1F1' };

const SCENARIOS = CLEAR_SIGNING_SCENARIOS;


// ---------------------------------------------------------------------------
// Main screen
// ---------------------------------------------------------------------------

export default function ClearSigningTestScreen() {
  const { t } = useTranslation();
  const router = useRouter();

  // Tapping a scenario opens a local modal that drives the real <SigningSheet>.
  const [mockRequest, setMockRequest] = useState<BLEIncomingRequest | null>(null);

  const handleScenario = useCallback((scenario: typeof SCENARIOS[number]) => {
    setMockRequest(scenario.request);
  }, []);

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        {/* Header */}
        <View style={styles.header}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.backBtn}>
            <ChevronLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <View>
            <Text style={styles.title}>{t('clearSigning.title')}</Text>
            <Text style={styles.subtitle}>{t('clearSigning.subtitle')}</Text>
          </View>
        </View>

        {/* Scenarios */}
        <VelaCard style={styles.card}>
          {SCENARIOS.map((scenario, i) => {
            const ic = SCENARIO_ICONS[scenario.id] ?? DEFAULT_ICON;
            return (
              <React.Fragment key={scenario.id}>
                <Pressable
                  style={styles.row}
                  onPress={() => handleScenario(scenario)}
                >
                  <View style={[styles.iconWrap, { backgroundColor: ic.iconBg }]}>
                    {ic.icon}
                  </View>
                  <View style={styles.rowInfo}>
                    <Text style={styles.rowTitle}>{t(scenario.labelKey, { defaultValue: scenario.labelKey })}</Text>
                    <Text style={styles.rowSub}>{t(scenario.subtitleKey, { defaultValue: scenario.subtitleKey })}</Text>
                  </View>
                </Pressable>
                {i < SCENARIOS.length - 1 && <View style={styles.divider} />}
              </React.Fragment>
            );
          })}
        </VelaCard>

        <Text style={styles.hint}>{t('clearSigning.hint')}</Text>
      </ScrollView>

      {/* Mock signing modal — renders independently from DAppConnection */}
      {mockRequest && (
        <MockSigningModal
          request={mockRequest}
          onClose={() => setMockRequest(null)}
        />
      )}
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// Mock Signing Modal — drives the REAL <SigningSheet> with mock data.
// One rendering path: production and this harness render the same component
// (no passkey / no transport), so the harness can never drift from production.
// ---------------------------------------------------------------------------

function MockSigningModal({ request, onClose }: {
  request: BLEIncomingRequest;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const { activeAccount } = useWallet();
  return (
    <AppModal visible={true} onClose={onClose}>
      <SigningSheet
        request={request}
        chainId={1}
        account={activeAccount ?? { name: "Wallet" }}
        dappInfo={{ name: "PancakeSwap", url: "https://pancakeswap.finance" }}
        isSigning={false}
        signError={null}
        pendingOpHash={null}
        onApprove={() => { showAlert(t("clearSigning.alertSignedTitle"), t("clearSigning.alertSignedBody")); onClose(); }}
        onReject={onClose}
        onDismiss={onClose}
      />
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  scrollContent: { paddingBottom: space['5xl'] },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    marginTop: space.xl,
    marginBottom: space['2xl'],
  },
  backBtn: {
    width: 36, height: 36, borderRadius: 18,
    backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  title: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base },
  subtitle: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, marginTop: 2 },
  card: { padding: 0, overflow: 'hidden' },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingVertical: space.xl,
    paddingHorizontal: space['2xl'],
  },
  iconWrap: {
    width: 40, height: 40, borderRadius: 12,
    alignItems: 'center', justifyContent: 'center',
  },
  rowInfo: { flex: 1, gap: 2 },
  rowTitle: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  rowSub: { fontSize: text.xs, ...inter.regular, color: color.fg.muted },
  divider: { height: 1, backgroundColor: color.border.base, marginHorizontal: space['2xl'] },
  hint: {
    fontSize: text.sm, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', marginTop: space['2xl'], lineHeight: 18,
    paddingHorizontal: space.xl,
  },
}));
