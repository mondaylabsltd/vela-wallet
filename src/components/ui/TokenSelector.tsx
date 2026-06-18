/**
 * TokenSelector — the reusable "pick a token" list.
 *
 * Search + category chips (stable / gas / other) + single-chain network filter
 * + count/total summary + an "add token" affordance. Extracted from the Send
 * flow so the Receive payment-request builder can reuse the exact same UI and
 * show every token (including zero-balance, custom, and built-in ones).
 */
import { AddTokenSheet } from '@/components/ui/AddTokenSheet';
import { NetworkFilterButton, NetworkFilterSheet } from '@/components/ui/NetworkFilterSheet';
import { TokenRow } from '@/components/ui/TokenRow';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { chainName, getAllNetworksSync, tokenBadgeNetwork } from '@/models/network';
import { isNativeToken, tokenBalanceDouble, tokenChainId, tokenLogoURLs, tokenUsdValue, type APIToken } from '@/models/types';
import { isStable } from '@/services/activity';
import { formatTokenAmount } from '@/services/locale-format';
import { Plus, Search } from 'lucide-react-native';
import React, { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { FlatList, Pressable, ScrollView, Text, TextInput, View } from 'react-native';

/** Quick filter buckets for the token picker. Mutually exclusive + exhaustive. */
export type TokenCategory = 'all' | 'stable' | 'gas' | 'other';

export function tokenMatchesCategory(tok: APIToken, cat: TokenCategory): boolean {
  if (cat === 'all') return true;
  if (cat === 'gas') return isNativeToken(tok); // native coin = the chain's gas token
  if (cat === 'stable') return !isNativeToken(tok) && isStable(tok.symbol);
  return !isNativeToken(tok) && !isStable(tok.symbol); // 'other'
}

interface Props {
  tokens: APIToken[];
  loading?: boolean;
  onSelect: (token: APIToken) => void;
  /** Fired after a custom token is added so the host can refresh its list. */
  onAddChanged?: () => void;
  /** Hide the count + USD total row (e.g. when most balances are zero). */
  hideTotals?: boolean;
  defaultCategory?: TokenCategory;
}

export function TokenSelector({ tokens, loading, onSelect, onAddChanged, hideTotals, defaultCategory = 'stable' }: Props) {
  const { t } = useTranslation();
  const formatUsd = useDisplayCurrency().fmt;
  const networks = getAllNetworksSync();

  const [search, setSearch] = useState('');
  const [category, setCategory] = useState<TokenCategory>(defaultCategory);
  const [chainFilter, setChainFilter] = useState<number | null>(null);
  const [showNetSheet, setShowNetSheet] = useState(false);
  const [showAddToken, setShowAddToken] = useState(false);

  const selectedNetwork = chainFilter != null ? networks.find((n) => n.chainId === chainFilter) ?? null : null;
  const hasActiveFilter = !!search || category !== 'all' || chainFilter != null;
  const q = search.trim().toLowerCase();

  const filtered = tokens
    .filter((tok) => {
      if (chainFilter != null && tokenChainId(tok) !== chainFilter) return false;
      if (!tokenMatchesCategory(tok, category)) return false;
      if (q && !(tok.symbol.toLowerCase().includes(q) || tok.name.toLowerCase().includes(q) || tok.network.toLowerCase().includes(q))) return false;
      return true;
    })
    // Sort by USD value desc, then token amount desc. This keeps zero-value /
    // zero-balance tokens below anything worth more, so an empty token never
    // sits above one the user actually holds.
    .sort((a, b) => {
      const usd = tokenUsdValue(b) - tokenUsdValue(a);
      if (usd !== 0) return usd;
      const amt = tokenBalanceDouble(b) - tokenBalanceDouble(a);
      if (amt !== 0) return amt;
      return a.symbol.localeCompare(b.symbol);
    });
  const filteredTotal = filtered.reduce((s, tok) => s + tokenUsdValue(tok), 0);

  const CATEGORIES: { key: Exclude<TokenCategory, 'all'>; label: string }[] = [
    { key: 'stable', label: t('send.filterStable') },
    { key: 'gas', label: t('send.filterGas') },
    { key: 'other', label: t('send.filterOther') },
  ];

  const addTokenButton = (
    <Pressable style={styles.addTokenRow} onPress={() => setShowAddToken(true)}>
      <Plus size={18} color={color.accent.base} strokeWidth={2.5} />
      <Text style={styles.addTokenText}>{t('send.addTokenBtn')}</Text>
    </Pressable>
  );

  return (
    <View style={styles.container}>
      <View style={styles.searchWrap}>
        <Search size={16} color={color.fg.subtle} strokeWidth={2} />
        <TextInput
          style={styles.searchInput}
          placeholder={t('send.searchPlaceholder')}
          placeholderTextColor={color.fg.subtle}
          value={search}
          onChangeText={setSearch}
          autoCapitalize="none"
          autoCorrect={false}
        />
      </View>

      <View style={styles.filterRow}>
        <ScrollView
          horizontal
          showsHorizontalScrollIndicator={false}
          style={styles.chipScroll}
          contentContainerStyle={styles.chipRow}
          keyboardShouldPersistTaps="handled"
        >
          {CATEGORIES.map((c) => {
            const active = category === c.key;
            return (
              <Pressable key={c.key} onPress={() => setCategory(active ? 'all' : c.key)} style={[styles.chip, active && styles.chipActive]}>
                <Text style={[styles.chipText, active && styles.chipTextActive]} numberOfLines={1}>{c.label}</Text>
              </Pressable>
            );
          })}
        </ScrollView>
        <NetworkFilterButton
          networks={networks}
          selected={selectedNetwork}
          onPress={() => setShowNetSheet(true)}
          onClear={() => setChainFilter(null)}
        />
      </View>

      {!hideTotals && !loading && filtered.length > 0 && (
        <View style={styles.summaryRow}>
          <Text style={styles.summaryCount}>{t('send.tokenCount', { n: filtered.length })}</Text>
          <Text style={styles.summaryTotal}>{formatUsd(filteredTotal)}</Text>
        </View>
      )}

      {loading ? (
        <Text style={styles.loadingText}>{t('send.loadingTokens')}</Text>
      ) : filtered.length === 0 ? (
        <View style={styles.emptyContainer}>
          <Text style={styles.emptyText}>{hasActiveFilter ? t('send.noMatchingTokens') : t('send.noTokensWithBalance')}</Text>
          {addTokenButton}
        </View>
      ) : (
        <FlatList
          data={filtered}
          keyExtractor={(item) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
          renderItem={({ item, index }) => (
            <TokenRow
              symbol={item.symbol}
              chainLabel={chainName(tokenChainId(item))}
              logoUrls={tokenLogoURLs(item)}
              chain={tokenBadgeNetwork(item)}
              contractAddress={item.tokenAddress}
              balance={formatTokenAmount(tokenBalanceDouble(item), { compact: true })}
              usdValue={tokenUsdValue(item) > 0 ? formatUsd(tokenUsdValue(item)) : undefined}
              onPress={() => { onSelect(item); setSearch(''); }}
              index={index}
            />
          )}
          ListFooterComponent={addTokenButton}
          initialNumToRender={10}
          windowSize={5}
          showsVerticalScrollIndicator={false}
          keyboardShouldPersistTaps="handled"
        />
      )}

      <NetworkFilterSheet
        visible={showNetSheet}
        networks={networks}
        selectedChainId={chainFilter}
        onSelect={setChainFilter}
        onClose={() => setShowNetSheet(false)}
        subtitleForChain={(n) => {
          const c = tokens.filter((tk) => tokenChainId(tk) === n.chainId).length;
          return c > 0 ? t('send.tokenCount', { n: c }) : undefined;
        }}
      />
      <AddTokenSheet visible={showAddToken} onClose={() => setShowAddToken(false)} onChanged={onAddChanged} />
    </View>
  );
}

const styles = createStyles(() => ({
  container: { flex: 1 },
  loadingText: {
    fontSize: text.lg,
    ...inter.regular,
    color: color.fg.muted,
    textAlign: 'center',
    marginTop: space['5xl'],
  },
  emptyContainer: {
    alignItems: 'center',
    marginTop: space['5xl'],
  },
  emptyText: {
    fontSize: text.xl,
    ...inter.semibold,
    color: color.fg.muted,
  },
  searchWrap: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    marginBottom: space.md,
  },
  searchInput: {
    flex: 1,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    padding: 0,
    outlineStyle: 'none',
  } as any,
  filterRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    marginBottom: space.xl,
  },
  chipScroll: { flex: 1 },
  chipRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingRight: space.sm,
  },
  chip: {
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderRadius: radius.full,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  chipActive: {
    backgroundColor: color.accent.soft,
    borderColor: color.accent.base,
  },
  chipText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  chipTextActive: { color: color.accent.base },
  summaryRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: space.sm,
    marginBottom: space.md,
  },
  summaryCount: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  summaryTotal: {
    fontSize: text.sm,
    ...inter.semibold,
    fontFamily: font.numeric,
    color: color.fg.base,
  },
  addTokenRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    alignSelf: 'stretch',
    gap: space.sm,
    paddingVertical: space.xl,
    marginTop: space.lg,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
    borderStyle: 'dashed',
    backgroundColor: color.bg.raised,
  },
  addTokenText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.accent.base,
  },
}));
