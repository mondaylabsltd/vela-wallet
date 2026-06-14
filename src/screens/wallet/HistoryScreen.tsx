import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  Pressable,
  ActivityIndicator,
} from 'react-native';
import { useTranslation } from 'react-i18next';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { useFocusEffect } from '@react-navigation/native';
import Animated from 'react-native-reanimated';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaRefresh } from '@/components/ui/VelaRefresh';
import { AmountText } from '@/components/ui/AmountText';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { getAllNetworksSync } from '@/models/network';
import { loadTransactions, type LocalTransaction } from '@/services/storage';
import { openBrowser } from '@/services/platform';
import { formatDate as fmtDateLocale, formatTime as fmtTimeLocale, formatTokenAmount } from '@/services/locale-format';
import {
  ArrowDownLeft, ArrowUpRight, ExternalLink, Check, X,
  FileSignature, FileText, Send, Code,
} from 'lucide-react-native';
import type { TransactionType } from '@/services/storage';

// MARK: - Helpers

function explorerTxUrl(txHash: string, chainId: number): string {
  const network = getAllNetworksSync().find((n) => n.chainId === chainId);
  const base = network?.explorerURL ?? 'https://etherscan.io';
  return `${base}/tx/${txHash}`;
}

function explorerAddressUrl(address: string, chainId: number): string {
  const network = getAllNetworksSync().find((n) => n.chainId === chainId);
  const base = network?.explorerURL ?? 'https://etherscan.io';
  return `${base}/address/${address}`;
}

function formatDate(timestamp: number): string {
  return fmtDateLocale(timestamp * 1000);
}

function formatTime(timestamp: number): string {
  return fmtTimeLocale(timestamp * 1000);
}

interface TransactionGroup {
  title: string;
  data: LocalTransaction[];
}

function groupByDate(txs: LocalTransaction[]): TransactionGroup[] {
  const groups: Record<string, LocalTransaction[]> = {};
  for (const tx of txs) {
    const key = formatDate(tx.timestamp);
    if (!groups[key]) groups[key] = [];
    groups[key].push(tx);
  }
  return Object.entries(groups).map(([title, data]) => ({ title, data }));
}

// MARK: - Component

export default function HistoryScreen() {
  const { t } = useTranslation();
  const router = useSafeRouter();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;

  const [transactions, setTransactions] = useState<LocalTransaction[]>([]);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [selectedChainId, setSelectedChainId] = useState<number | null>(null);

  const loadData = useCallback(async () => {
    try {
      const txs = await loadTransactions();
      // Filter to current account
      const filtered = txs.filter(tx => tx.from.toLowerCase() === address?.toLowerCase());
      setTransactions(filtered);
    } catch {}
    setLoading(false);
  }, [address]);

  useFocusEffect(useCallback(() => {
    setLoading(true);
    loadData();
  }, [loadData]));

  const onRefresh = useCallback(async () => {
    setRefreshing(true);
    // History reads from local storage (near-instant) — hold the branded
    // spinner briefly so it never flickers.
    await Promise.all([loadData(), new Promise((resolve) => setTimeout(resolve, 650))]);
    setRefreshing(false);
  }, [loadData]);

  const filteredTxs = selectedChainId
    ? transactions.filter(tx => tx.chainId === selectedChainId)
    : transactions;

  const handleViewOnExplorer = useCallback(() => {
    if (!address) return;
    const chainId = selectedChainId ?? 1;
    openBrowser(explorerAddressUrl(address, chainId));
  }, [address, selectedChainId]);

  const handleBack = () => router.back();

  // MARK: - Transaction Row

  const renderTransaction = ({ item }: { item: LocalTransaction }) => {
    const network = getAllNetworksSync().find((n) => n.chainId === item.chainId);
    const networkName = network?.displayName ?? `Chain ${item.chainId}`;
    const txType = item.type ?? 'send';
    const { icon, iconBg, label, subtitle, showAmount } = txDisplayInfo(txType, item, networkName);

    return (
      <Pressable
        style={styles.txRow}
        onPress={item.txHash ? () => openBrowser(explorerTxUrl(item.txHash, item.chainId)) : undefined}
      >
        <View style={[styles.txIcon, { backgroundColor: iconBg }]}>
          {icon}
        </View>

        <View style={styles.txInfo}>
          <Text style={styles.txType}>{label}</Text>
          <Text style={styles.txAddress} numberOfLines={1}>{subtitle}</Text>
        </View>

        <View style={styles.txValues}>
          {showAmount ? (
            <AmountText
              text={`-${formatTokenAmount(parseFloat(item.value), { compact: true })}`}
              unit={item.symbol}
              size={text.lg}
              minScale={0.7}
              tailScale={0.7}
              style={styles.txAmount}
              tailStyle={styles.txAmountUnit}
              containerStyle={styles.txAmountBox}
            />
          ) : (
            <Text style={[styles.txTime, { fontSize: text.sm }]}>
              {item.dappOrigin || ''}
            </Text>
          )}
          <Text style={styles.txTime}>
            {item.status === 'failed' ? t('history.statusFailed') : formatTime(item.timestamp)}
          </Text>
        </View>
      </Pressable>
    );
  };

  function txDisplayInfo(txType: TransactionType | undefined, item: LocalTransaction, networkName: string) {
    const sw = 2.5;
    const sz = 18;
    switch (txType) {
      case 'dapp_tx':
        return {
          icon: item.intent
            ? <Code size={sz} color={color.accent.base} strokeWidth={sw} />
            : <ArrowUpRight size={sz} color={color.accent.base} strokeWidth={sw} />,
          iconBg: color.accent.soft,
          label: item.intent || t('history.txLabelDappTx'),
          subtitle: [
            item.dappOrigin,
            item.to ? shortAddress(item.to) : null,
            networkName,
          ].filter(Boolean).join(' · '),
          showAmount: parseFloat(item.value || '0') > 0,
        };
      case 'sign_message':
        return {
          icon: <FileSignature size={sz} color={color.fg.muted} strokeWidth={sw} />,
          iconBg: color.bg.sunken,
          label: t('history.txLabelSignMessage'),
          subtitle: [item.dappOrigin, networkName].filter(Boolean).join(' · '),
          showAmount: false,
        };
      case 'sign_typed_data':
        return {
          icon: <FileText size={sz} color={color.fg.muted} strokeWidth={sw} />,
          iconBg: color.bg.sunken,
          label: item.intent || t('history.txLabelSignTypedData'),
          subtitle: [item.dappOrigin, networkName].filter(Boolean).join(' · '),
          showAmount: false,
        };
      case 'send':
      default:
        return {
          icon: <ArrowUpRight size={sz} color={color.accent.base} strokeWidth={sw} />,
          iconBg: color.accent.soft,
          label: t('history.txLabelSent', { symbol: item.symbol }),
          subtitle: t('history.txSentSubtitle', { toName: item.toName ?? shortAddress(item.to), networkName }),
          showAmount: true,
        };
    }
  }

  const renderSectionHeader = (title: string) => (
    <View style={styles.sectionHeader}>
      <Text style={styles.sectionTitle}>{title}</Text>
    </View>
  );

  // MARK: - Chain Filter

  const renderChainFilter = () => {
    const chainsWithTxs = [...new Set(transactions.map(tx => tx.chainId))];
    if (chainsWithTxs.length <= 1) return null;

    return (
      <View style={styles.chainFilterRow}>
        <Pressable
          style={[styles.chainFilterChip, !selectedChainId && styles.chainFilterChipActive]}
          onPress={() => setSelectedChainId(null)}
        >
          <Text style={[styles.chainFilterText, !selectedChainId && styles.chainFilterTextActive]}>{t('history.filterAll')}</Text>
        </Pressable>
        {chainsWithTxs.map(chainId => {
          const net = getAllNetworksSync().find(n => n.chainId === chainId);
          const isActive = selectedChainId === chainId;
          return (
            <Pressable
              key={chainId}
              style={[styles.chainFilterChip, isActive && styles.chainFilterChipActive]}
              onPress={() => setSelectedChainId(isActive ? null : chainId)}
            >
              <Text style={[styles.chainFilterText, isActive && styles.chainFilterTextActive]}>
                {net?.iconLabel ?? `${chainId}`}
              </Text>
            </Pressable>
          );
        })}
      </View>
    );
  };

  // MARK: - Empty State

  const renderEmpty = () => (
    <View style={styles.emptyContainer}>
      <Animated.View entering={fadeInDown(100, 400)}>
        <VelaCard elevated style={styles.emptyCard}>
          <View style={styles.emptyIconWrap}>
            <ExternalLink size={28} color={color.fg.subtle} />
          </View>
          <Text style={styles.emptyTitle}>{t('history.emptyTitle')}</Text>
          <Text style={styles.emptyBody}>{t('history.emptyBody')}</Text>
          <VelaButton
            title={t('history.viewOnExplorer')}
            onPress={handleViewOnExplorer}
            variant="accent"
            style={styles.explorerBtn}
          />
        </VelaCard>
      </Animated.View>
    </View>
  );

  // MARK: - Render

  const groups = groupByDate(filteredTxs);

  type ListItem =
    | { type: 'header'; title: string; key: string }
    | { type: 'tx'; tx: LocalTransaction; key: string };

  const listData: ListItem[] = [];
  for (const group of groups) {
    listData.push({ type: 'header', title: group.title, key: `h-${group.title}` });
    for (const tx of group.data) {
      listData.push({ type: 'tx', tx, key: tx.id });
    }
  }

  return (
    <ScreenContainer>
      {/* Nav Bar */}
      <View style={styles.navBar}>
        <Pressable onPress={handleBack} hitSlop={8} style={styles.navBtn}>
          <X size={22} color={color.fg.base} strokeWidth={2} />
        </Pressable>
        <Text style={styles.navTitle}>{t('history.navTitle')}</Text>
        <View style={styles.navSpacer} />
      </View>

      {/* Address pill */}
      {address ? (
        <Animated.View style={styles.addressRow} entering={fadeIn(0, 300)}>
          <Text style={styles.addressText}>
            {activeAccount?.name ? `${activeAccount.name} · ` : ''}{shortAddress(address)}
          </Text>
        </Animated.View>
      ) : null}

      {loading ? (
        <View style={styles.loadingContainer}>
          <ActivityIndicator size="large" color={color.accent.base} />
          <Text style={styles.loadingText}>{t('history.loadingText')}</Text>
        </View>
      ) : transactions.length === 0 ? (
        renderEmpty()
      ) : (
        <>
          {renderChainFilter()}
          <VelaRefresh refreshing={refreshing} onRefresh={onRefresh}>
            {(scrollProps) => (
              <Animated.FlatList
                {...scrollProps}
                data={listData}
                keyExtractor={(item: ListItem) => item.key}
                renderItem={({ item }: { item: ListItem }) =>
                  item.type === 'header'
                    ? renderSectionHeader(item.title)
                    : renderTransaction({ item: item.tx })
                }
                initialNumToRender={15}
                windowSize={5}
                maxToRenderPerBatch={10}
                showsVerticalScrollIndicator={false}
                contentContainerStyle={styles.listContent}
                ListEmptyComponent={
                  filteredTxs.length === 0 && transactions.length > 0 ? (
                    <View style={styles.emptyFilterContainer}>
                      <Text style={styles.emptyFilterText}>{t('history.emptyFilter')}</Text>
                    </View>
                  ) : null
                }
              />
            )}
          </VelaRefresh>
        </>
      )}
    </ScreenContainer>
  );
}

// MARK: - Styles

const styles = createStyles(() => ({
  navBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
  },
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  navTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  navSpacer: { minWidth: 60 },

  addressRow: {
    alignSelf: 'center',
    backgroundColor: color.bg.sunken,
    paddingHorizontal: space.xl,
    paddingVertical: space.sm,
    borderRadius: radius.full,
    marginBottom: space.xl,
  },
  addressText: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.muted,
  },
  loadingContainer: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.lg,
  },
  loadingText: {
    fontSize: text.lg,
    ...inter.regular,
    color: color.fg.muted,
  },
  listContent: {
    paddingBottom: space['5xl'],
  },

  // Chain filter
  chainFilterRow: {
    flexDirection: 'row',
    gap: space.md,
    paddingBottom: space.lg,
    flexWrap: 'wrap',
  },
  chainFilterChip: {
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderRadius: radius.full,
    backgroundColor: color.bg.sunken,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  chainFilterChipActive: {
    backgroundColor: color.accent.soft,
    borderColor: color.accent.base,
  },
  chainFilterText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  chainFilterTextActive: {
    color: color.accent.base,
    ...inter.semibold,
  },

  // Section Headers
  sectionHeader: {
    paddingTop: space['2xl'],
    paddingBottom: space.md,
  },
  sectionTitle: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
  },

  // Transaction Row
  txRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: space.lg,
    gap: space.lg,
  },
  txIcon: {
    width: 40,
    height: 40,
    borderRadius: 20,
    alignItems: 'center',
    justifyContent: 'center',
  },
  txInfo: {
    flex: 1,
    gap: 2,
  },
  txType: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
  },
  txAddress: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },
  txValues: {
    alignItems: 'flex-end',
    gap: 2,
    flexShrink: 1,
    maxWidth: '52%',
  },
  txAmountBox: {
    alignSelf: 'stretch',
  },
  txAmount: {
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
    textAlign: 'right',
  },
  txAmountUnit: {
    color: color.fg.muted,
  },
  txTime: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },

  // Empty state
  emptyContainer: {
    flex: 1,
    paddingTop: space.md,
  },
  emptyCard: {
    padding: space['3xl'],
    alignItems: 'center',
    gap: space.lg,
  },
  emptyIconWrap: {
    width: 56,
    height: 56,
    borderRadius: 28,
    backgroundColor: color.bg.sunken,
    alignItems: 'center',
    justifyContent: 'center',
    marginBottom: space.sm,
  },
  emptyTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  emptyBody: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.muted,
    textAlign: 'center',
    lineHeight: 22,
  },
  explorerBtn: {
    marginTop: space.md,
    width: '100%',
  },

  // Empty filter
  emptyFilterContainer: {
    alignItems: 'center',
    paddingTop: space['5xl'],
  },
  emptyFilterText: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.muted,
  },
}));
