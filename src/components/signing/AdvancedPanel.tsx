/**
 * Advanced panel — full untruncated payload + detail-only fields, collapsed by
 * default, for power users who want to verify exactly what's being signed.
 */
import React, { useMemo, useState } from 'react';
import { View, Text, Pressable, ScrollView } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { type ClearSignResult } from '@/services/clear-signing';
import { ChevronDown } from 'lucide-react-native';
import { styles, localizeLabel } from './signing-core';

export function AdvancedPanel({ method, params, clearSign }: {
  method: string;
  params: any[];
  clearSign: ClearSignResult | null;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  // The exact bytes/JSON being signed — untruncated, so a power user can verify.
  const raw = useMemo(() => {
    try {
      if (method === 'eth_sendTransaction') {
        const tx = params?.[0] ?? {};
        return [
          tx.to ? `to: ${tx.to}` : null,
          tx.value && tx.value !== '0x0' ? `value: ${tx.value}` : null,
          tx.data && tx.data !== '0x' ? `data: ${tx.data}` : null,
        ].filter(Boolean).join('\n\n');
      }
      if (method.includes('signTypedData')) {
        const rawData = params?.[1] ?? params?.[0];
        const obj = typeof rawData === 'string' ? JSON.parse(rawData) : rawData;
        return JSON.stringify(obj, null, 2);
      }
      if (method === 'personal_sign') return String(params?.[0] ?? '');
      if (method === 'eth_sign') return String((params?.length > 1 ? params[1] : params?.[0]) ?? '');
      return '';
    } catch { return ''; }
  }, [method, params]);

  const detailFields = clearSign?.fields.filter((f) => f.detail) ?? [];
  if (!raw && detailFields.length === 0) return null;

  return (
    <View>
      <Pressable style={styles.detailsToggle} onPress={() => setOpen((o) => !o)}>
        <Text style={styles.detailsToggleText}>{t('componentsUi.signing.advancedToggle')}</Text>
        <ChevronDown
          size={12} color={color.fg.subtle} strokeWidth={2}
          style={open ? { transform: [{ rotate: '180deg' }] } : undefined}
        />
      </Pressable>
      {open && (
        <View style={styles.advancedBody}>
          {detailFields.map((f, i) => (
            <View key={i} style={styles.genRow}>
              <Text style={styles.genLabel}>{localizeLabel(f.label)}</Text>
              <Text style={styles.genValue} numberOfLines={4}>{f.value}</Text>
            </View>
          ))}
          {!!raw && (
            <ScrollView style={styles.advancedRaw} nestedScrollEnabled>
              <Text style={styles.rawText} selectable>{raw}</Text>
            </ScrollView>
          )}
        </View>
      )}
    </View>
  );
}
