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
      {/* ZONE 1 — the fields are the hero, so the action is just a small kicker.
          Neutral grey: the caution lives in the banner below. */}
      <IntentHeader intent={t('componentsUi.signing.signTypedData')} color={color.fg.base} variant="eyebrow" />
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
            {/* One line each — long hex/addresses are mid-truncated so a raw salt
                or maker doesn't wrap into a two-line hex wall. The full, exact
                payload is one tap away under 技术细节. */}
            <Text style={styles.genValue} numberOfLines={1}>{v}</Text>
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
          identity="contract"
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

/**
 * Render one raw typed-data value on a single line. A long hex blob (address /
 * salt / bytes) is mid-truncated (0x1234…5678) so it never wraps into a two-line
 * hex wall; everything else is stringified and capped. We deliberately DON'T
 * reinterpret numbers (no decimals/timestamp guessing) — this descriptor is
 * unknown, so an honest raw value beats a confident wrong one. The full exact
 * payload is available verbatim under 技术细节.
 */
function formatBlindValue(v: unknown): string {
  if (v && typeof v === 'object') return JSON.stringify(v).slice(0, 60);
  const s = String(v);
  if (/^0x[0-9a-fA-F]{21,}$/.test(s)) return `${s.slice(0, 10)}…${s.slice(-8)}`;
  return s.slice(0, 60);
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
      ? Object.entries(msg).slice(0, 5).map(([k, v]) => [k, formatBlindValue(v)])
      : [];
    return { primaryType, domain, fields };
  } catch {
    return { primaryType: null, domain: null, fields: [] };
  }
}
