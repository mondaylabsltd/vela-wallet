/**
 * Warning banner + generic field row — the shared caution/danger surfaces.
 */
import React from 'react';
import { View, Text } from 'react-native';
import { type ClearSignField } from '@/services/clear-signing';
import { AlertTriangle } from 'lucide-react-native';
import { styles, riskColors, localizeLabel } from './signing-core';

export function WarningBanner({ severity, text: msg }: {
  severity: 'caution' | 'danger';
  text: string;
}) {
  const isDanger = severity === 'danger';
  return (
    <View style={[styles.warnBanner, isDanger ? styles.warnDanger : styles.warnCaution]}>
      <AlertTriangle
        size={14}
        color={isDanger ? riskColors().danger : riskColors().caution}
        strokeWidth={2}
      />
      <Text style={[styles.warnText, { color: isDanger ? riskColors().danger : riskColors().caution }]}>
        {msg}
      </Text>
    </View>
  );
}

export function GenericFieldRow({ field }: { field: ClearSignField }) {
  return (
    <View style={[styles.genRow, field.warning && styles.genRowWarning]}>
      <Text style={styles.genLabel}>{localizeLabel(field.label)}</Text>
      <Text
        style={[
          styles.genValue,
          field.warning && { color: riskColors().danger },
          field.expired && { color: riskColors().caution },
        ]}
        numberOfLines={2}
      >
        {field.value}
      </Text>
    </View>
  );
}
