/**
 * Network filter — a single-select chain filter.
 *
 *   NetworkFilterButton: the compact trigger. Shows stacked chain logos + "All"
 *   by default, or the selected chain's logo + name + a clear (✕) control.
 *
 *   NetworkFilterSheet: the "Select Chain" picker (built on AppModal). Single
 *   select only — either "All Networks" or exactly one chain. Tapping a row
 *   applies immediately and closes. Optional search.
 *
 * Reusable and data-agnostic: callers pass the network list and the current
 * selection (chainId | null, where null = All). Theme-driven (light/dark).
 */
import React, { useMemo, useState } from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import { Check, ChevronDown, Globe, Search, X } from 'lucide-react-native';
import { ChainLogo } from '@/components/ChainLogo';
import { AppModal } from '@/components/ui/AppModal';
import type { Network } from '@/models/network';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';
import { useTranslation } from 'react-i18next';

// ---------------------------------------------------------------------------
// Trigger button
// ---------------------------------------------------------------------------

interface NetworkFilterButtonProps {
  networks: Network[];
  /** Currently selected chain, or null for "All". */
  selected: Network | null;
  onPress: () => void;
  onClear: () => void;
}

export function NetworkFilterButton({ networks, selected, onPress, onClear }: NetworkFilterButtonProps) {
  const preview = networks.slice(0, 3);
  return (
    <Pressable style={styles.trigger} onPress={onPress} hitSlop={6}>
      {selected ? (
        <ChainLogo label={selected.iconLabel} color={selected.iconColor} bgColor={selected.iconBg} logoURL={selected.logoURL} size={20} />
      ) : (
        <View style={styles.stack}>
          {preview.map((n, i) => (
            <View key={n.chainId} style={[styles.stackItem, i > 0 && styles.stackOverlap]}>
              <ChainLogo label={n.iconLabel} color={n.iconColor} bgColor={n.iconBg} logoURL={n.logoURL} size={20} />
            </View>
          ))}
        </View>
      )}
      <Text style={styles.triggerLabel} numberOfLines={1}>{selected ? selected.displayName : 'All'}</Text>
      {selected ? (
        <Pressable onPress={onClear} hitSlop={8} style={styles.clearBtn}>
          <X size={12} color={color.fg.muted} strokeWidth={2.6} />
        </Pressable>
      ) : (
        <ChevronDown size={13} color={color.fg.muted} strokeWidth={2.4} />
      )}
    </Pressable>
  );
}

// ---------------------------------------------------------------------------
// Select Chain sheet
// ---------------------------------------------------------------------------

interface NetworkFilterSheetProps {
  visible: boolean;
  networks: Network[];
  selectedChainId: number | null;
  onSelect: (chainId: number | null) => void;
  onClose: () => void;
  /** Optional secondary line per chain (e.g. value or event count). */
  subtitleForChain?: (network: Network) => string | undefined;
}

export function NetworkFilterSheet({
  visible,
  networks,
  selectedChainId,
  onSelect,
  onClose,
  subtitleForChain,
}: NetworkFilterSheetProps) {
  const [searching, setSearching] = useState(false);
  const [query, setQuery] = useState('');

  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return networks;
    return networks.filter((n) => n.displayName.toLowerCase().includes(q) || n.iconLabel.toLowerCase().includes(q));
  }, [networks, query]);

  const pick = (chainId: number | null) => {
    onSelect(chainId);
    onClose();
  };

  const { t } = useTranslation();

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.sheet}>
        <View style={styles.sheetHead}>
          <View style={styles.headSpacer} />
          <Text style={styles.sheetTitle}>{t('componentsUi.networkFilter.selectChain')}</Text>
          <Pressable onPress={() => setSearching((s) => !s)} hitSlop={8} style={styles.searchToggle}>
            {searching ? <X size={18} color={color.fg.base} strokeWidth={2} /> : <Search size={18} color={color.fg.base} strokeWidth={2} />}
          </Pressable>
        </View>

        {searching && (
          <TextInput
            style={styles.search}
            placeholder={t('componentsUi.networkFilter.searchChains')}
            placeholderTextColor={color.fg.subtle}
            value={query}
            onChangeText={setQuery}
            autoFocus
            autoCapitalize="none"
            autoCorrect={false}
          />
        )}

        <ScrollView style={styles.list} contentContainerStyle={styles.listContent} showsVerticalScrollIndicator={false}>
          {/* All Networks */}
          {!query && (
            <Pressable style={[styles.row, selectedChainId === null && styles.rowSelected]} onPress={() => pick(null)}>
              <View style={styles.allIcon}>
                <Globe size={20} color={color.fg.muted} strokeWidth={2} />
              </View>
              <View style={styles.rowInfo}>
                <Text style={styles.rowName}>{t('componentsUi.networkFilter.allNetworks')}</Text>
                <Text style={styles.rowSub}>{t('componentsUi.networkFilter.showEvery')}</Text>
              </View>
              {selectedChainId === null && <Check size={20} color={color.accent.base} strokeWidth={2.6} />}
            </Pressable>
          )}

          {filtered.map((n) => {
            const isSel = selectedChainId === n.chainId;
            const sub = subtitleForChain?.(n);
            return (
              <Pressable key={n.chainId} style={[styles.row, isSel && styles.rowSelected]} onPress={() => pick(n.chainId)}>
                <ChainLogo label={n.iconLabel} color={n.iconColor} bgColor={n.iconBg} logoURL={n.logoURL} size={40} />
                <View style={styles.rowInfo}>
                  <Text style={styles.rowName}>{n.displayName}</Text>
                  {sub ? <Text style={styles.rowSub}>{sub}</Text> : null}
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
  // Trigger
  trigger: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.lg,
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
    maxWidth: 150,
  },
  stack: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  stackItem: {
    borderRadius: radius.full,
    borderWidth: 2,
    borderColor: color.bg.raised,
  },
  stackOverlap: {
    marginLeft: -8,
  },
  triggerLabel: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
    flexShrink: 1,
  },
  clearBtn: {
    width: 20,
    height: 20,
    borderRadius: 10,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
  },

  // Sheet
  sheet: {
    flex: 1,
    backgroundColor: color.bg.base,
    paddingHorizontal: space['2xl'],
  },
  sheetHead: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.md,
  },
  headSpacer: { width: 34 },
  sheetTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  searchToggle: {
    width: 34,
    height: 34,
    borderRadius: 17,
    alignItems: 'center',
    justifyContent: 'center',
  },
  search: {
    fontSize: 16,
    ...inter.regular,
    color: color.fg.base,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.lg,
    paddingHorizontal: space.lg,
    paddingVertical: space.lg,
    marginTop: space.sm,
    outlineStyle: 'none',
  } as any,
  list: {
    marginTop: space.lg,
  },
  listContent: {
    gap: space.md,
    paddingBottom: space['3xl'],
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    backgroundColor: color.bg.raised,
    borderWidth: 1.5,
    borderColor: 'transparent',
    borderRadius: radius.xl,
    padding: space.lg,
  },
  rowSelected: {
    borderColor: color.accent.base,
  },
  allIcon: {
    width: 40,
    height: 40,
    borderRadius: 20,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
  },
  rowInfo: {
    flex: 1,
    gap: 2,
  },
  rowName: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
  },
  rowSub: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },
}));
