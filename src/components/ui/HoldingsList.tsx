/**
 * HoldingsList — the Assets tab on Home: the tokens the account holds, per
 * chain, with collapsed-by-default search, add-token, and token-detail
 * navigation. Extracted from the retired standalone AssetsScreen so holdings
 * live one visible tap from the hero.
 *
 * Deliberately shows only funded tokens and reads the token set streamed by
 * HomeScreen — no fetching of its own (the old zero-balance superset scan was
 * an uncached full multi-chain fan-out and got cut along with its toggle).
 *
 * The render site keys this component by address, so an account switch
 * remounts it and no state leaks across accounts. Balance privacy comes from
 * the shared store — amounts mask together with the hero.
 */
import { useRouter } from 'expo-router';
import { ArrowDown, Plus, Search } from 'lucide-react-native';
import React, { useState } from 'react';
import { Pressable, Text, TextInput, View, type StyleProp, type ViewStyle } from 'react-native';
import Animated from 'react-native-reanimated';
import { useTranslation } from 'react-i18next';

import { SectionLabel } from '@/components/ui/SectionLabel';
import { TokenRow } from '@/components/ui/TokenRow';
import { VelaRefresh } from '@/components/ui/VelaRefresh';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';
import { useBalancePrivacy } from '@/hooks/use-balance-privacy';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { chainName, tokenBadgeNetwork } from '@/models/network';
import { tokenBalanceDouble, tokenChainId, tokenLogoURLs, tokenUsdValue, type APIToken } from '@/models/types';
import { formatTokenAmount } from '@/services/locale-format';

interface Props {
  /** Live holdings, streamed by HomeScreen — shared, not re-fetched. */
  tokens: APIToken[];
  /** True while the wallet is likely funded but the token scan hasn't painted
      yet — suppresses the empty state instead of flashing it. */
  loading: boolean;
  /** Home's network filter — applies here exactly as on the Activity feed. */
  selectedChainId: number | null;
  /** Hero + nav row, shared with the other tabs. */
  header: React.ReactElement;
  refreshing: boolean;
  onRefresh: () => void;
  /** "Updated Xm ago" caption for the pull-to-refresh indicator. */
  refreshStatus?: string;
  contentContainerStyle: StyleProp<ViewStyle>;
}

export function HoldingsList({
  tokens, loading, selectedChainId, header, refreshing, onRefresh, refreshStatus, contentContainerStyle,
}: Props) {
  const router = useRouter();
  const { t } = useTranslation();
  const dc = useDisplayCurrency();
  const { hidden } = useBalancePrivacy();

  const [searchOpen, setSearchOpen] = useState(false);
  const [tokenSearch, setTokenSearch] = useState('');

  const navigateToToken = (token: APIToken) => {
    router.push({
      pathname: '/token-detail',
      params: {
        symbol: token.symbol,
        name: token.name,
        network: token.network,
        balance: token.balance,
        decimals: String(token.decimals),
        logos: JSON.stringify(tokenLogoURLs(token)),
        tokenAddress: token.tokenAddress ?? '',
        priceUsd: String(token.priceUsd ?? 0),
        chainName: token.chainName,
      },
    });
  };

  const toggleSearch = () => {
    setSearchOpen((open) => {
      if (open) setTokenSearch('');
      return !open;
    });
  };

  const chainFiltered = selectedChainId != null
    ? tokens.filter((tk) => tokenChainId(tk) === selectedChainId)
    : tokens;
  const q = tokenSearch.trim().toLowerCase();
  const filteredTokens = q
    ? chainFiltered.filter((tk) =>
        tk.symbol.toLowerCase().includes(q) ||
        tk.name.toLowerCase().includes(q) ||
        tk.network.toLowerCase().includes(q) ||
        chainName(tokenChainId(tk)).toLowerCase().includes(q))
    : chainFiltered;
  const isFiltering = q.length > 0 || selectedChainId != null;

  const renderListHeader = () => (
    <View>
      {header}
      <View style={styles.tokenListHeader}>
        <SectionLabel style={styles.tokenListTitle}>{t('assets.sectionTitle')}</SectionLabel>
        <View style={styles.tokenListActions}>
          {/* Search is collapsed by default — a plain icon next to Add; the
              field appears only on demand. */}
          <Pressable
            style={styles.searchToggleBtn}
            onPress={toggleSearch}
            hitSlop={8}
            accessibilityRole="button"
            accessibilityLabel={t('assets.searchPlaceholder')}
          >
            <Search size={15} color={searchOpen ? color.accent.base : color.fg.muted} strokeWidth={2.4} />
          </Pressable>
          <Pressable style={styles.addTokenBtn} onPress={() => router.push('/add-token')} hitSlop={8}>
            <Plus size={14} color={color.accent.base} strokeWidth={2.5} />
            <Text style={styles.addTokenText}>{t('assets.addToken')}</Text>
          </Pressable>
        </View>
      </View>
      {searchOpen && (
        <View style={styles.searchBar}>
          <Search size={14} color={color.fg.subtle} strokeWidth={2} />
          <TextInput
            style={styles.searchInput}
            placeholder={t('assets.searchPlaceholder')}
            placeholderTextColor={color.fg.subtle}
            value={tokenSearch}
            onChangeText={setTokenSearch}
            autoCapitalize="none"
            autoCorrect={false}
            autoFocus
          />
        </View>
      )}
    </View>
  );

  const renderEmpty = () => {
    if (loading) return null;
    // A filter/search with no matches is NOT an empty wallet — don't show the
    // receive-onboarding card under a funded hero.
    if (isFiltering) {
      return <Text style={styles.noMatchText}>{t('send.noMatchingTokens')}</Text>;
    }
    return (
      <Pressable style={styles.emptyCard} onPress={() => router.push('/receive')}>
        <View style={styles.emptyIconWrap}>
          <ArrowDown size={22} color={color.accent.base} strokeWidth={2.5} />
        </View>
        <Text style={styles.emptyTitle}>{t('assets.emptyTitle')}</Text>
        <Text style={styles.emptySubtext}>{t('assets.emptySubtext')}</Text>
      </Pressable>
    );
  };

  return (
    <VelaRefresh refreshing={refreshing} onRefresh={onRefresh} statusText={refreshStatus}>
      {(scrollProps) => (
        <Animated.FlatList
          {...scrollProps}
          data={filteredTokens}
          keyExtractor={(item: APIToken) => `${item.network}_${item.tokenAddress ?? 'native'}_${item.symbol}`}
          ListHeaderComponent={renderListHeader()}
          ListEmptyComponent={renderEmpty()}
          renderItem={({ item, index }: { item: APIToken; index: number }) => (
            <TokenRow
              symbol={item.symbol}
              chainLabel={chainName(tokenChainId(item))}
              logoUrls={tokenLogoURLs(item)}
              chain={tokenBadgeNetwork(item)}
              balance={hidden ? '••••' : formatTokenAmount(tokenBalanceDouble(item), { compact: true })}
              usdValue={!hidden && tokenUsdValue(item) > 0 ? dc.fmt(tokenUsdValue(item)) : undefined}
              onPress={() => navigateToToken(item)}
              index={index}
            />
          )}
          ItemSeparatorComponent={() => <View style={styles.sep} />}
          initialNumToRender={10}
          windowSize={5}
          maxToRenderPerBatch={8}
          contentContainerStyle={contentContainerStyle}
          showsVerticalScrollIndicator={false}
        />
      )}
    </VelaRefresh>
  );
}

const styles = createStyles(() => ({
  tokenListHeader: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    marginBottom: space.md,
  },
  // SectionLabel default carries vertical margins for standalone use; zero them
  // so it sits inline in the token-list header row (label ↔ actions).
  tokenListTitle: {
    marginTop: 0,
    marginBottom: 0,
  },
  tokenListActions: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
  },
  searchToggleBtn: {
    width: 32,
    height: 32,
    borderRadius: 16,
    alignItems: 'center',
    justifyContent: 'center',
  },
  addTokenBtn: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
    paddingVertical: space.sm,
    paddingHorizontal: space.md,
  },
  addTokenText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
  },
  // Search — soft bg.sunken input (no border), shown only while search is open.
  searchBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    paddingHorizontal: space.lg,
    paddingVertical: space.md,
    marginBottom: space.lg,
  },
  searchInput: {
    flex: 1,
    fontSize: 16, // ≥16px prevents iOS Safari auto-zoom on focus
    ...inter.regular,
    color: color.fg.base,
    paddingVertical: space.xs,
    outlineStyle: 'none',
  } as any,
  // Hairline divider between de-boxed token rows, inset past the 40px token logo
  // so it aligns under the symbol/chain text (Apple-Wallet style).
  sep: {
    height: 1,
    backgroundColor: color.border.base,
    marginLeft: space.md + 40 + space.lg,
  },
  noMatchText: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    paddingVertical: space['3xl'],
  },
  // Empty — open state (no card): centered icon + copy on the page.
  emptyCard: {
    padding: space['4xl'],
    alignItems: 'center',
    gap: space.md,
  },
  emptyIconWrap: {
    width: 48,
    height: 48,
    borderRadius: 24,
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  emptyTitle: {
    fontSize: text.xl,
    ...inter.semibold,
    color: color.fg.muted,
  },
  emptySubtext: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.subtle,
    textAlign: 'center',
    lineHeight: 20,
  },
}));
