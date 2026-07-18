/**
 * TreasuryBootstrapSheet — "帮助启动此网络的中继器" (start this network's relayer).
 *
 * Shown when a network's bundler treasury reports `bootstrapNeeded` (float below its operating
 * floor): sending is impossible until SOMEONE tops up the treasury directly. Instead of a dead
 * error we show the network identity, a scan-to-pay QR + tap-to-copy of the treasury address,
 * which OPERATOR endpoint it belongs to, and a prominent disclaimer — then a "funded, retry".
 *
 * The treasury address comes from whatever bundler endpoint is configured (getActiveBundlerBaseUrl
 * → /v1/treasury/:chainId), so a self-hosted bundler funds ITS OWN treasury — nothing hardcoded.
 * The contribution is NON-REFUNDABLE, goes to the BUNDLER OPERATOR (not Vela), and is the gas that
 * boots this network's relayer — it is NOT payment for the user's current tx (already paid).
 */
import React, { useEffect, useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Check, Copy, X } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';
import { QRCode } from '@/components/QRCode';
import { ChainLogo } from '@/components/ChainLogo';
import { chainName, getAllNetworksSync, nativeSymbol } from '@/models/network';
import { fromBaseUnits } from '@/services/eip681';
import { getActiveBundlerBaseUrl } from '@/services/rpc-pool';
import { copyToClipboard, hapticLight } from '@/services/platform';
import { type TreasuryStatus } from '@/services/bundler-service';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  /** Treasury status (from probeTreasury/fetchTreasuryStatus). null renders nothing. */
  status: TreasuryStatus | null;
  onClose: () => void;
  /** "I've funded — retry the transaction". When omitted a plain Close is shown instead. */
  onRetry?: () => void;
}

export function TreasuryBootstrapSheet({ visible, status, onClose, onRetry }: Props) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  const [endpoint, setEndpoint] = useState<string | null>(null);
  useEffect(() => { if (!visible) setCopied(false); }, [visible]);

  // Which operator's bundler endpoint this treasury belongs to (transparency).
  useEffect(() => {
    if (!status) { setEndpoint(null); return; }
    let cancelled = false;
    getActiveBundlerBaseUrl(status.chainId)
      .then((url) => { if (!cancelled) setEndpoint(url); })
      .catch(() => { if (!cancelled) setEndpoint(null); });
    return () => { cancelled = true; };
  }, [status]);

  if (!status) return null;

  const net = getAllNetworksSync().find((n) => n.chainId === status.chainId);
  // Native treasuries are wei (18 dec); Tempo's pathUSD treasury is 6 dec.
  const decimals = status.asset === 'pathUSD' ? 6 : 18;
  const symbol = status.asset === 'pathUSD' ? 'pathUSD' : nativeSymbol(status.chainId);
  // Suggest 2× the floor, not just back TO it: a floor-exact top-up leaves ~zero working float
  // and the very next send re-opens this sheet — non-refundable money that didn't unblock anything.
  const targetRaw = status.floor * 2n;
  const neededRaw = targetRaw > status.balance ? targetRaw - status.balance : 0n;
  const amountText = fromBaseUnits(neededRaw, decimals);
  // The QR is the bare treasury address — this is funded from a CEX withdrawal or
  // another wallet, and those scanners choke on an EIP-681 `ethereum:…@chainId?value=…`
  // URI (they paste the whole string as the address). The suggested amount is shown
  // as text above, and the tap-to-copy button copies the same bare address.
  const qrValue = status.address;

  const copyAddress = () => {
    hapticLight();
    copyToClipboard(status.address);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.sheet}>
        <View style={styles.head}>
          <View style={styles.headSpacer} />
          <Text style={styles.headTitle} numberOfLines={2}>{t('componentsUi.treasuryBootstrap.title')}</Text>
          <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn} accessibilityRole="button">
            <X size={20} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <ScrollView contentContainerStyle={styles.body} showsVerticalScrollIndicator={false}>
          {/* Network identity — which chain's treasury this is (esp. custom / local nets). */}
          <View style={styles.netRow}>
            {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={26} />}
            <Text style={styles.netName} numberOfLines={1}>{chainName(status.chainId)}</Text>
            <Text style={styles.netId}>· #{status.chainId}</Text>
          </View>

          <Text style={styles.lead}>{t('componentsUi.treasuryBootstrap.lead')}</Text>

          {neededRaw > 0n && (
            <View style={styles.amtBlock}>
              <Text style={styles.amtLbl}>{t('componentsUi.treasuryBootstrap.suggested')}</Text>
              <Text style={styles.amtVal}>~{amountText} {symbol}</Text>
            </View>
          )}

          {/* Scan-to-pay QR of the treasury address. */}
          <View style={styles.qrWrap}>
            <View style={styles.qrCard}><QRCode value={qrValue} size={140} /></View>
          </View>

          {/* Treasury address + the operator endpoint it belongs to. Tap to copy. */}
          <Pressable style={styles.addressCard} onPress={copyAddress} accessibilityRole="button">
            <View style={styles.addressRow}>
              <Text style={styles.addressLabel}>{t('componentsUi.treasuryBootstrap.addressLabel')}</Text>
              {copied
                ? <Check size={14} color={color.success.base} strokeWidth={3} />
                : <Copy size={14} color={color.fg.subtle} strokeWidth={2} />}
            </View>
            <Text style={styles.addressText} selectable>{status.address}</Text>
            {endpoint && <Text style={styles.endpointText} numberOfLines={1}>{endpoint}</Text>}
          </Pressable>

          {/* PROMINENT disclaimer: non-refundable, to the operator, boots the service. */}
          <View style={styles.disclaimerCard}>
            <AlertTriangle size={18} color={color.warning.base} strokeWidth={2} />
            <Text style={styles.disclaimerText}>{t('componentsUi.treasuryBootstrap.disclaimer')}</Text>
          </View>

          <VelaButton
            title={t(copied ? 'componentsUi.treasuryBootstrap.copied' : 'componentsUi.treasuryBootstrap.copyBtn')}
            onPress={copyAddress}
            style={styles.copyBtn}
          />
          {onRetry ? (
            <VelaButton
              title={t('componentsUi.treasuryBootstrap.retryBtn')}
              onPress={onRetry}
              variant="secondary"
              style={styles.closeBottomBtn}
            />
          ) : (
            <VelaButton
              title={t('componentsUi.treasuryBootstrap.closeBtn')}
              onPress={onClose}
              variant="secondary"
              style={styles.closeBottomBtn}
            />
          )}
        </ScrollView>
      </View>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  sheet: { flex: 1, backgroundColor: color.bg.base },
  head: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space['2xl'], paddingVertical: space.md,
  },
  headSpacer: { width: 34 },
  headTitle: { flex: 1, textAlign: 'center', fontSize: text.xl, ...inter.bold, color: color.fg.base, paddingHorizontal: space.sm },
  closeBtn: { width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center' },
  body: { paddingHorizontal: space['2xl'], paddingBottom: space['4xl'] },

  // Network identity chip
  netRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm, marginTop: space.md },
  netName: { fontSize: text.lg, ...inter.bold, color: color.fg.base, flexShrink: 1 },
  netId: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle, fontFamily: font.numeric },

  lead: { fontSize: text.base, ...inter.regular, color: color.fg.muted, lineHeight: 21, textAlign: 'center', marginTop: space.md },

  amtBlock: { alignItems: 'center', marginTop: space.lg },
  amtLbl: { fontSize: text.xs, ...inter.semibold, color: color.fg.subtle },
  amtVal: { fontSize: text['2xl'], ...inter.bold, fontFamily: font.numeric, color: color.fg.base, marginTop: 2 },

  qrWrap: { alignItems: 'center', marginTop: space.lg },
  qrCard: { padding: space.md, backgroundColor: '#FFFFFF', borderRadius: radius.lg, borderWidth: 1, borderColor: color.border.base },

  addressCard: {
    backgroundColor: color.bg.sunken, borderRadius: radius.lg, borderWidth: 1, borderColor: color.border.base,
    padding: space.xl, marginTop: space.lg,
  },
  addressRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', marginBottom: space.sm },
  addressLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  addressText: { fontSize: text.sm, ...inter.medium, fontFamily: font.mono, color: color.fg.base },
  endpointText: { fontSize: text.xs, ...inter.regular, fontFamily: font.mono, color: color.fg.subtle, marginTop: space.sm },

  disclaimerCard: {
    flexDirection: 'row', alignItems: 'flex-start', gap: space.md,
    backgroundColor: color.warning.soft, borderRadius: radius.lg, padding: space.xl, marginTop: space.lg,
  },
  disclaimerText: { flex: 1, fontSize: text.sm, ...inter.semibold, color: color.warning.base, lineHeight: 20 },

  copyBtn: { marginTop: space['2xl'] },
  closeBottomBtn: { marginTop: space.md },
}));
