/**
 * CurrencySheet — pick the display currency for the total balance.
 * Searchable (by code or name), single-select, applies + closes on tap.
 * Built on AppModal. Theme-driven.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Check, Search, X } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { getSupportedCurrenciesSync, loadSupportedCurrencies, type Currency } from '@/services/currency';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  selected: string;
  onSelect: (code: string) => void;
  onClose: () => void;
}

export function CurrencySheet({ visible, selected, onSelect, onClose }: Props) {
  const { t } = useTranslation();
  const [query, setQuery] = useState('');
  // Paint instantly from the cached/static list, then refresh from the endpoint
  // so every currency it can price is searchable.
  const [currencies, setCurrencies] = useState<Currency[]>(getSupportedCurrenciesSync);
  const pick = (code: string) => { onSelect(code); setQuery(''); onClose(); };

  useEffect(() => {
    if (!visible) return;
    setCurrencies(getSupportedCurrenciesSync());
    loadSupportedCurrencies().then(setCurrencies).catch(() => {});
  }, [visible]);

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return currencies;
    return currencies.filter((c) => c.code.toLowerCase().includes(q) || c.name.toLowerCase().includes(q));
  }, [query, currencies]);

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.sheet}>
        <Text style={styles.title}>{t('componentsUi.currency.title')}</Text>

        <View style={styles.searchBox}>
          <Search size={18} color={color.fg.subtle} strokeWidth={2.2} />
          <TextInput
            style={styles.searchInput}
            placeholder={t('componentsUi.currency.searchPlaceholder')}
            placeholderTextColor={color.fg.subtle}
            value={query}
            onChangeText={setQuery}
            autoCapitalize="none"
            autoCorrect={false}
            returnKeyType="search"
          />
          {query.length > 0 && (
            <Pressable onPress={() => setQuery('')} hitSlop={8}>
              <X size={18} color={color.fg.subtle} strokeWidth={2.2} />
            </Pressable>
          )}
        </View>

        <ScrollView
          style={styles.list}
          contentContainerStyle={styles.listContent}
          showsVerticalScrollIndicator={false}
          keyboardShouldPersistTaps="handled"
        >
          {filtered.length === 0 ? (
            <Text style={styles.empty}>{t('componentsUi.currency.noMatch', { query: query.trim() })}</Text>
          ) : filtered.map((c) => {
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
  searchBox: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    backgroundColor: color.bg.sunken, borderRadius: radius.full,
    paddingHorizontal: space.lg, paddingVertical: space.md, marginBottom: space.sm,
  },
  searchInput: { flex: 1, fontSize: text.lg, ...inter.medium, color: color.fg.base, padding: 0 },
  list: { marginTop: space.sm },
  listContent: { gap: space.md, paddingBottom: space['3xl'] },
  empty: { fontSize: text.base, ...inter.regular, color: color.fg.subtle, textAlign: 'center', paddingVertical: space['3xl'] },
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
