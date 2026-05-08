import React, { useState, useCallback } from 'react';
import {
  View,
  Text,
  FlatList,
  Pressable,
  ActivityIndicator,
  RefreshControl,
} from 'react-native';
import { useRouter } from 'expo-router';
import Animated, { FadeIn, FadeInDown } from 'react-native-reanimated';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { color, text, weight, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { DEFAULT_NETWORKS } from '@/models/network';
import * as WebBrowser from 'expo-web-browser';
import { ArrowDownLeft, ArrowUpRight, ExternalLink, Check } from 'lucide-react-native';

// MARK: - Types

interface Transaction {
  hash: string;
  from: string;
  to: string;
  value: string;
  timestamp: number;
  status: 'success' | 'failed' | 'pending';
  chainId: number;
  isIncoming: boolean;
}

interface TransactionGroup {
  title: string;
  data: Transaction[];
}

// MARK: - Helpers

function explorerUrlForAddress(address: string, chainId: number): string {
  const network = DEFAULT_NETWORKS.find((n) => n.chainId === chainId);
  const base = network?.explorerURL ?? 'https://etherscan.io';
  return `${base}/address/${address}`;
}

function formatDate(timestamp: number): string {
  const d = new Date(timestamp * 1000);
  return d.toLocaleDateString('en-US', {
    month: 'short',
    day: 'numeric',
    year: 'numeric',
  });
}

function formatTime(timestamp: number): string {
  const d = new Date(timestamp * 1000);
  return d.toLocaleTimeString('en-US', {
    hour: '2-digit',
    minute: '2-digit',
  });
}

function formatEthValue(weiHex: string): string {
  try {
    const wei = BigInt(weiHex);
    const eth = Number(wei) / 1e18;
    if (eth === 0) return '0';
    if (eth < 0.0001) return '< 0.0001';
    return eth.toFixed(4).replace(/\.?0+$/, '');
  } catch {
    return '0';
  }
}

function groupByDate(txs: Transaction[]): TransactionGroup[] {
  const groups: Record<string, Transaction[]> = {};
  for (const tx of txs) {
    const key = formatDate(tx.timestamp);
    if (!groups[key]) groups[key] = [];
    groups[key].push(tx);
  }
  return Object.entries(groups).map(([title, data]) => ({ title, data }));
}

// MARK: - Component

export default function HistoryScreen() {
  const router = useRouter();
  const { activeAccount, state } = useWallet();
  const address = activeAccount?.address ?? state.address;

  const [transactions] = useState<Transaction[]>([]);
  const [loading] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  const [selectedChainId, setSelectedChainId] = useState(1);

  const selectedNetwork = DEFAULT_NETWORKS.find((n) => n.chainId === selectedChainId);

  const onRefresh = useCallback(() => {
    setRefreshing(true);
    setTimeout(() => setRefreshing(false), 800);
  }, []);

  const handleViewOnExplorer = useCallback(() => {
    if (!address) return;
    const url = explorerUrlForAddress(address, selectedChainId);
    WebBrowser.openBrowserAsync(url);
  }, [address, selectedChainId]);

  const handleBack = () => router.back();

  // MARK: - Transaction Row

  const renderTransaction = ({ item }: { item: Transaction }) => {
    const IconComponent = item.isIncoming ? ArrowDownLeft : ArrowUpRight;
    const iconBg = item.isIncoming ? color.success.soft : color.accent.soft;
    const iconColor = item.isIncoming ? color.success.base : color.accent.base;
    const counterparty = item.isIncoming ? item.from : item.to;
    const sign = item.isIncoming ? '+' : '-';
    const amountColor = item.isIncoming ? color.success.base : color.fg.base;
    const nativeSym =
      DEFAULT_NETWORKS.find((n) => n.chainId === item.chainId)?.iconLabel ?? 'ETH';
    const statusColor =
      item.status === 'failed'
        ? color.accent.base
        : item.status === 'pending'
          ? color.fg.subtle
          : color.fg.muted;

    return (
      <Pressable
        style={styles.txRow}
        onPress={() => {
          const network = DEFAULT_NETWORKS.find((n) => n.chainId === item.chainId);
          const base = network?.explorerURL ?? 'https://etherscan.io';
          WebBrowser.openBrowserAsync(`${base}/tx/${item.hash}`);
        }}
      >
        <View style={[styles.txIcon, { backgroundColor: iconBg }]}>
          <IconComponent size={18} color={iconColor} strokeWidth={2.5} />
        </View>

        <View style={styles.txInfo}>
          <Text style={styles.txType}>
            {item.isIncoming ? 'Received' : 'Sent'}
          </Text>
          <Text style={styles.txAddress}>
            {item.isIncoming ? 'From ' : 'To '}
            {shortAddress(counterparty)}
          </Text>
        </View>

        <View style={styles.txValues}>
          <Text style={[styles.txAmount, { color: amountColor }]}>
            {sign}{formatEthValue(item.value)} {nativeSym}
          </Text>
          <Text style={[styles.txTime, { color: statusColor }]}>
            {item.status === 'pending'
              ? 'Pending'
              : item.status === 'failed'
                ? 'Failed'
                : formatTime(item.timestamp)}
          </Text>
        </View>
      </Pressable>
    );
  };

  const renderSectionHeader = (title: string) => (
    <View style={styles.sectionHeader}>
      <Text style={styles.sectionTitle}>{title}</Text>
    </View>
  );

  // MARK: - Empty State

  const renderEmpty = () => (
    <View style={styles.emptyContainer}>
      <Animated.View entering={FadeInDown.delay(100).duration(400)}>
        <VelaCard elevated style={styles.emptyCard}>
          <View style={styles.emptyIconWrap}>
            <ExternalLink size={28} color={color.fg.subtle} />
          </View>
          <Text style={styles.emptyTitle}>No History Yet</Text>
          <Text style={styles.emptyBody}>
            View your full transaction history on the block explorer.
          </Text>
          <VelaButton
            title={`View on ${selectedNetwork?.displayName ?? 'Explorer'}`}
            onPress={handleViewOnExplorer}
            variant="accent"
            style={styles.explorerBtn}
          />
        </VelaCard>
      </Animated.View>

      {/* Network Selector */}
      <Animated.View entering={FadeInDown.delay(200).duration(400)}>
        <Text style={styles.networkLabel}>SELECT NETWORK</Text>
        <VelaCard style={styles.networkCard}>
          {DEFAULT_NETWORKS.map((net) => {
            const isSelected = net.chainId === selectedChainId;
            return (
              <Pressable
                key={net.id}
                style={[styles.networkRow, isSelected && styles.networkRowSelected]}
                onPress={() => setSelectedChainId(net.chainId)}
              >
                <View style={[styles.networkDot, { backgroundColor: net.iconColor }]} />
                <Text
                  style={[
                    styles.networkName,
                    isSelected && styles.networkNameSelected,
                  ]}
                >
                  {net.displayName}
                </Text>
                {isSelected && <Check size={16} color={color.accent.base} strokeWidth={2.5} />}
              </Pressable>
            );
          })}
        </VelaCard>
      </Animated.View>
    </View>
  );

  // MARK: - Render

  const groups = groupByDate(transactions);

  type ListItem =
    | { type: 'header'; title: string; key: string }
    | { type: 'tx'; tx: Transaction; key: string };

  const listData: ListItem[] = [];
  for (const group of groups) {
    listData.push({ type: 'header', title: group.title, key: `h-${group.title}` });
    for (const tx of group.data) {
      listData.push({ type: 'tx', tx, key: tx.hash });
    }
  }

  return (
    <ScreenContainer>
      {/* Nav Bar */}
      <View style={styles.navBar}>
        <Pressable onPress={handleBack} hitSlop={8}>
          <Text style={styles.navBack}>Close</Text>
        </Pressable>
        <Text style={styles.navTitle}>History</Text>
        <View style={styles.navSpacer} />
      </View>

      {/* Address pill */}
      {address ? (
        <Animated.View style={styles.addressRow} entering={FadeIn.duration(300)}>
          <Text style={styles.addressText}>{shortAddress(address)}</Text>
        </Animated.View>
      ) : null}

      {loading ? (
        <View style={styles.loadingContainer}>
          <ActivityIndicator size="large" color={color.accent.base} />
          <Text style={styles.loadingText}>Loading transactions...</Text>
        </View>
      ) : transactions.length === 0 ? (
        renderEmpty()
      ) : (
        <FlatList
          data={listData}
          keyExtractor={(item) => item.key}
          renderItem={({ item }) =>
            item.type === 'header'
              ? renderSectionHeader(item.title)
              : renderTransaction({ item: item.tx })
          }
          refreshControl={
            <RefreshControl
              refreshing={refreshing}
              onRefresh={onRefresh}
              tintColor={color.accent.base}
            />
          }
          showsVerticalScrollIndicator={false}
          contentContainerStyle={styles.listContent}
        />
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
  navBack: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: color.accent.base,
    minWidth: 60,
  },
  navTitle: {
    fontSize: text.xl,
    fontWeight: weight.bold,
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
    fontWeight: weight.medium,
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
    fontWeight: weight.regular,
    color: color.fg.muted,
  },
  listContent: {
    paddingBottom: space['5xl'],
  },

  // Section Headers
  sectionHeader: {
    paddingTop: space['2xl'],
    paddingBottom: space.md,
  },
  sectionTitle: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
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
    fontWeight: weight.semibold,
    color: color.fg.base,
  },
  txAddress: {
    fontSize: text.sm,
    fontWeight: weight.regular,
    color: color.fg.muted,
  },
  txValues: {
    alignItems: 'flex-end',
    gap: 2,
  },
  txAmount: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
  },
  txTime: {
    fontSize: text.sm,
    fontWeight: weight.regular,
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
    fontWeight: weight.bold,
    color: color.fg.base,
  },
  emptyBody: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.muted,
    textAlign: 'center',
    lineHeight: 22,
  },
  explorerBtn: {
    marginTop: space.md,
    width: '100%',
  },

  // Network selector
  networkLabel: {
    fontSize: text.sm,
    fontWeight: weight.semibold,
    color: color.fg.subtle,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
    marginTop: space['3xl'],
    marginBottom: space.lg,
  },
  networkCard: {
    paddingVertical: space.sm,
  },
  networkRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingVertical: space.lg,
    paddingHorizontal: space['2xl'],
    gap: space.lg,
  },
  networkRowSelected: {
    backgroundColor: color.bg.sunken,
  },
  networkDot: {
    width: 10,
    height: 10,
    borderRadius: 5,
  },
  networkName: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: color.fg.base,
    flex: 1,
  },
  networkNameSelected: {
    fontWeight: weight.semibold,
  },
}));
