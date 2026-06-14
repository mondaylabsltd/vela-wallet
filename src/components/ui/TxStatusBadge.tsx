/**
 * Transaction status badge (icon + label) shared by the transaction and
 * connection-event detail sheets, so "pending / confirmed / failed" renders
 * identically everywhere a stored transaction's status is shown.
 */
import { color, createStyles, inter, space, text } from '@/constants/theme';
import { CheckCircle2, Clock, XCircle } from 'lucide-react-native';
import React from 'react';
import { useTranslation } from 'react-i18next';
import { Text, View } from 'react-native';

export function TxStatusBadge({ status }: { status: 'pending' | 'confirmed' | 'failed' }) {
  const { t } = useTranslation();
  const cfg = status === 'failed'
    ? { Icon: XCircle, tint: color.error.base, label: t('componentsTx.detail.statusFailed') }
    : status === 'pending'
      ? { Icon: Clock, tint: color.warning.base, label: t('componentsTx.detail.statusPending') }
      : { Icon: CheckCircle2, tint: color.success.base, label: t('componentsTx.detail.statusSucceeded') };
  return (
    <View style={styles.row}>
      <cfg.Icon size={16} color={cfg.tint} strokeWidth={2.4} />
      <Text style={[styles.text, { color: cfg.tint }]}>{cfg.label}</Text>
    </View>
  );
}

const styles = createStyles(() => ({
  row: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  text: { fontSize: text.base, ...inter.semibold },
}));
