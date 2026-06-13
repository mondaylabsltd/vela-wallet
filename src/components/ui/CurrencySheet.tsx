/**
 * CurrencySheet — pick the display currency for the total balance.
 * Single-select, applies + closes on tap. Built on AppModal. Theme-driven.
 */
import React from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { Check } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { CURRENCIES } from '@/services/currency';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  selected: string;
  onSelect: (code: string) => void;
  onClose: () => void;
}

export function CurrencySheet({ visible, selected, onSelect, onClose }: Props) {
  const pick = (code: string) => { onSelect(code); onClose(); };
  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.sheet}>
        <Text style={styles.title}>Display currency</Text>
        <ScrollView style={styles.list} contentContainerStyle={styles.listContent} showsVerticalScrollIndicator={false}>
          {CURRENCIES.map((c) => {
            const isSel = c.code === selected;
            return (
              <Pressable key={c.code} style={[styles.row, isSel && styles.rowSel]} onPress={() => pick(c.code)}>
                <View style={styles.sym}><Text style={styles.symText}>{c.symbol}</Text></View>
                <View style={styles.info}>
                  <Text style={styles.code}>{c.code}</Text>
                  <Text style={styles.name}>{c.name}</Text>
                </View>
                {isSel && <Check size={20} color={color.accent.base} strokeWidth={2.6} />}
              </Pressable>
            );
          })}
        </ScrollView>
      </View>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  sheet: { flex: 1, backgroundColor: color.bg.base, paddingHorizontal: space['2xl'] },
  title: { fontSize: text.xl, ...inter.bold, color: color.fg.base, paddingVertical: space.md, textAlign: 'center' },
  list: { marginTop: space.sm },
  listContent: { gap: space.md, paddingBottom: space['3xl'] },
  row: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    backgroundColor: color.bg.raised, borderRadius: radius.xl,
    borderWidth: 1.5, borderColor: 'transparent', padding: space.lg,
  },
  rowSel: { borderColor: color.accent.base },
  sym: {
    width: 40, height: 40, borderRadius: 20, backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  symText: { fontSize: text.lg, ...inter.bold, color: color.fg.base },
  info: { flex: 1, gap: 2 },
  code: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  name: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
}));
