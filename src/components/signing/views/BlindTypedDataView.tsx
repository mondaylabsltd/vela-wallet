/**
 * Blind Typed Data View (EIP-712, no descriptor).
 */
import React from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { styles } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { ContractBar } from '../ContractBar';
import { WarningBanner } from '../WarningBanner';

export function BlindTypedDataView({ params }: {
  params: any[];
}) {
  const { t } = useTranslation();
  const { primaryType, domain, fields } = parseTypedDataForDisplay(params);

  return (
    <View>
      {/* ZONE 1 — hero: the typed data itself (Type + fields). */}
      <IntentHeader intent={t('componentsUi.signing.signTypedData')} color={color.warning.base} />
      <View style={styles.genericFields}>
        {primaryType && (
          <View style={styles.genRow}>
            {/* fixed label → uppercase kicker; the dynamic struct keys below stay as data */}
            <Text style={styles.contractLabel}>{t('componentsUi.signing.typeLabel')}</Text>
            <Text style={styles.genValue}>{primaryType}</Text>
          </View>
        )}
        {fields.map(([k, v], i) => (
          <View key={i} style={styles.genRow}>
            <Text style={styles.genLabel}>{k}</Text>
            <Text style={styles.genValue} numberOfLines={2}>{v}</Text>
          </View>
        ))}
      </View>

      {/* ZONE 2 — who you're signing for. */}
      {domain && (
        <ContractBar
          label={t('componentsUi.signing.signingFor')}
          name={domain.name}
          address={domain.verifyingContract?.toLowerCase()}
          verified={false}
        />
      )}

      {/* ZONE 3 — undecodable caution. */}
      <WarningBanner
        severity="caution"
        text={t('componentsUi.signing.blindTypedWarning')}
      />
    </View>
  );
}

function parseTypedDataForDisplay(params: any[]): {
  primaryType: string | null;
  domain: any;
  fields: [string, string][];
} {
  try {
    const data = typeof params[1] === 'string' ? JSON.parse(params[1]) : (params[1] ?? params[0]);
    const primaryType = data?.primaryType ?? null;
    const domain = data?.domain;
    const msg = data?.message;
    const fields: [string, string][] = msg
      ? Object.entries(msg).slice(0, 5).map(([k, v]) => [k, (
          v && typeof v === 'object' ? JSON.stringify(v) : String(v)
        ).slice(0, 60)])
      : [];
    return { primaryType, domain, fields };
  } catch {
    return { primaryType: null, domain: null, fields: [] };
  }
}
