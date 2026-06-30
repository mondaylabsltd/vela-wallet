/**
 * DetailRow — a label ↔ value row with an optional copy / open-in-explorer
 * affordance, plus a 1px Divider. The component AND its styles were byte-identical
 * in TransactionDetailSheet and ConnectionEventDetailSheet; this is the one copy.
 *
 *   <DetailRow label="To" value={shortAddress(to)} mono onOpen={() => openBrowser(url)} />
 *   <DetailRow label="Amount" custom={<AmountText … />} />
 *   <Divider />
 *
 * Pass `onCopy` + `copied` for the copy affordance (Copy ↔ Check), or `onOpen` for
 * the external-link affordance. `custom` replaces the value cell entirely.
 */
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { Check, Copy, ExternalLink } from 'lucide-react-native';
import { color, createStyles, font, inter, space, text } from '@/constants/theme';

export function Divider() {
  return <View style={styles.divider} />;
}

export function DetailRow({ label, value, custom, mono, onCopy, onOpen, copied }: {
  label: string; value?: string; custom?: React.ReactNode; mono?: boolean;
  onCopy?: () => void; onOpen?: () => void; copied?: boolean;
}) {
  return (
    <View style={styles.row}>
      <Text style={styles.rowLabel}>{label}</Text>
      {custom ?? (
        <Pressable style={styles.rowValueWrap} onPress={onOpen ?? onCopy} disabled={!onOpen && !onCopy} hitSlop={6}>
          <Text style={[styles.rowValue, mono && styles.rowValueMono]} numberOfLines={1}>{value}</Text>
          {onCopy ? (copied ? <Check size={14} color={color.success.base} strokeWidth={2.6} /> : <Copy size={14} color={color.fg.subtle} strokeWidth={2} />) : null}
          {onOpen ? <ExternalLink size={14} color={color.fg.subtle} strokeWidth={2} /> : null}
        </Pressable>
      )}
    </View>
  );
}

const styles = createStyles(() => ({
  row: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: space.lg, paddingVertical: space.lg },
  rowLabel: { fontSize: text.base, ...inter.regular, color: color.fg.muted },
  rowValueWrap: { flexDirection: 'row', alignItems: 'center', gap: space.sm, flexShrink: 1 },
  rowValue: { fontSize: text.base, ...inter.semibold, color: color.fg.base, flexShrink: 1 },
  rowValueMono: { fontFamily: font.mono },
  divider: { height: 1, backgroundColor: color.border.base },
}));
