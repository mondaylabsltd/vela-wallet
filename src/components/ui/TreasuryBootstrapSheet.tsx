/**
 * TreasuryBootstrapSheet — "帮助启动此网络的中继器" (help start this network's
 * relayer).
 *
 * Shown by the Send flow when a network's relayer treasury reports
 * `bootstrapNeeded` (float below its operating floor): sending is impossible
 * until SOMEONE funds the treasury directly, so instead of a dead-end error
 * toast we surface the treasury address and the shortfall, and let the user
 * choose to contribute.
 *
 * The disclaimer is deliberately prominent: the contribution is NON-REFUNDABLE
 * and is NOT gas credit — the contributor's future transactions still pay gas
 * normally. It only bootstraps this network's relayer float.
 *
 * Structure mirrors AddTokenSheet/BalanceDetailSheet (AppModal + header + X),
 * with BundlerFundingModal's tap-to-copy address card.
 */
import React, { useEffect, useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, Check, Copy, X } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';
import { nativeSymbol } from '@/models/network';
import { fromBaseUnits } from '@/services/eip681';
import { copyToClipboard, hapticLight } from '@/services/platform';
import { type TreasuryStatus } from '@/services/bundler-service';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  /** Treasury status (from fetchTreasuryStatus). null renders nothing. */
  status: TreasuryStatus | null;
  onClose: () => void;
}

export function TreasuryBootstrapSheet({ visible, status, onClose }: Props) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  // A fresh open starts un-copied (the sheet may be reused across networks).
  useEffect(() => { if (!visible) setCopied(false); }, [visible]);

  if (!status) return null;

  // Native treasuries are wei (18 dec); Tempo's pathUSD treasury is 6 dec.
  const decimals = status.asset === 'pathUSD' ? 6 : 18;
  const symbol = status.asset === 'pathUSD' ? 'pathUSD' : nativeSymbol(status.chainId);
  // Suggest 2× the floor, not just back TO the floor: the relayer refuses to spend
  // below its floor, so a floor-exact contribution leaves ~zero working float and
  // the very next send would re-open this sheet — non-refundable money that didn't
  // actually unblock anything.
  const targetRaw = status.floor * 2n;
  const neededRaw = targetRaw > status.balance ? targetRaw - status.balance : 0n;
  const amountText = fromBaseUnits(neededRaw, decimals);

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
          <Text style={styles.headTitle} numberOfLines={2}>
            {t('componentsUi.treasuryBootstrap.title')}
          </Text>
          <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn} accessibilityRole="button">
            <X size={20} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <ScrollView contentContainerStyle={styles.body} showsVerticalScrollIndicator={false}>
          <Text style={styles.lead}>{t('componentsUi.treasuryBootstrap.lead')}</Text>

          {/* Shortfall — how much the treasury needs to get back over its floor. */}
          {neededRaw > 0n && (
            <Text style={styles.amountHint}>
              {t('componentsUi.treasuryBootstrap.amountHint', { amount: amountText, symbol })}
            </Text>
          )}

          {/* Treasury address — monospace, tap-to-copy (the user is holding the
              device; they can't scan their own screen). */}
          <Pressable style={styles.addressCard} onPress={copyAddress} accessibilityRole="button">
            <View style={styles.addressRow}>
              <Text style={styles.addressLabel}>{t('componentsUi.treasuryBootstrap.addressLabel')}</Text>
              {copied ? (
                <Check size={14} color={color.accent.base} strokeWidth={3} />
              ) : (
                <Copy size={14} color={color.fg.subtle} strokeWidth={2} />
              )}
            </View>
            <Text style={styles.addressText} selectable>{status.address}</Text>
          </Pressable>

          {/* PROMINENT disclaimer: non-refundable, not gas credit. */}
          <View style={styles.disclaimerCard}>
            <AlertTriangle size={18} color={color.warning.base} strokeWidth={2} />
            <Text style={styles.disclaimerText}>{t('componentsUi.treasuryBootstrap.disclaimer')}</Text>
          </View>

          <VelaButton
            title={t(copied ? 'componentsUi.treasuryBootstrap.copied' : 'componentsUi.treasuryBootstrap.copyBtn')}
            onPress={copyAddress}
            style={styles.copyBtn}
          />
          <VelaButton
            title={t('componentsUi.treasuryBootstrap.closeBtn')}
            onPress={onClose}
            variant="secondary"
            style={styles.closeBottomBtn}
          />
        </ScrollView>
      </View>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  sheet: { flex: 1, backgroundColor: color.bg.base },
  head: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space['2xl'],
    paddingVertical: space.md,
  },
  headSpacer: { width: 34 },
  headTitle: {
    flex: 1,
    textAlign: 'center',
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    paddingHorizontal: space.sm,
  },
  closeBtn: { width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center' },
  body: { paddingHorizontal: space['2xl'], paddingBottom: space['4xl'] },

  lead: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.muted,
    lineHeight: 22,
    marginTop: space.lg,
  },
  amountHint: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
    marginTop: space.lg,
  },

  // Address card — tap-to-copy (mirrors BundlerFundingModal's address card).
  addressCard: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    padding: space.xl,
    marginTop: space.xl,
  },
  addressRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    marginBottom: space.sm,
  },
  addressLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  addressText: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
  },

  // Disclaimer — the one thing the user MUST internalize before contributing.
  disclaimerCard: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: space.md,
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
    padding: space.xl,
    marginTop: space.xl,
  },
  disclaimerText: {
    flex: 1,
    fontSize: text.sm,
    ...inter.semibold,
    color: color.warning.base,
    lineHeight: 20,
  },

  copyBtn: { marginTop: space['2xl'] },
  closeBottomBtn: { marginTop: space.md },
}));
