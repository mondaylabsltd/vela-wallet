/**
 * eth_sign Danger View — opaque-hash blind signing (the classic blind-sign trap).
 */
import React from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { ShieldAlert } from 'lucide-react-native';
import { styles } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { WarningBanner } from '../WarningBanner';

export function EthSignDangerView({ dataHex }: { dataHex: string }) {
  const { t } = useTranslation();
  const hash = typeof dataHex === 'string' ? dataHex : String(dataHex ?? '');
  return (
    <View>
      {/* A small red kicker, not a giant headline — the danger CARD (with the
          explanation + the opaque hash) is the protagonist, consistent with every
          other screen's eyebrow sizing. Red still says "real danger". */}
      <IntentHeader intent={t('componentsUi.signing.ethSignIntent')} color={color.error.base} variant="eyebrow" colorEyebrow />

      <View style={styles.ethSignCard}>
        <View style={styles.ethSignHeader}>
          <ShieldAlert size={16} color={color.error.base} strokeWidth={2} />
          <Text style={styles.ethSignTitle}>{t('componentsUi.signing.ethSignTitle')}</Text>
        </View>
        <Text style={styles.ethSignBody}>{t('componentsUi.signing.ethSignBody')}</Text>
        <Text style={styles.ethSignHash} numberOfLines={2}>{hash}</Text>
      </View>

      <WarningBanner severity="danger" text={t('componentsUi.signing.ethSignWarning')} />
    </View>
  );
}
