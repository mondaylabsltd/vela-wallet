import { showAlert } from '@/services/platform';
import React, { useState } from 'react';
import { View, Text, TextInput, Pressable, ScrollView, FlatList } from 'react-native';
import { useSafeRouter } from '@/hooks/use-safe-router';
import Animated from 'react-native-reanimated';
import { fadeInDown } from '@/constants/entering';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { ChainLogo } from '@/components/ChainLogo';
import { color, text, inter, space, radius, font, shadow, createStyles } from '@/constants/theme';
import { getAllNetworksSync } from '@/models/network';
import type { Network } from '@/models/network';
import { saveCustomToken } from '@/services/storage';
import type { CustomToken } from '@/models/types';
import { rpcCall } from '@/services/rpc-adapter';
import { MULTICALL3, encAggregate3, decAggregate3 } from '@/services/abi';
import { Check, ArrowLeft, ChevronDown, Search, X } from 'lucide-react-native';

// Minimal ABI-encoded function selectors for ERC-20 metadata
const NAME_SELECTOR = '0x06fdde03';
const SYMBOL_SELECTOR = '0x95d89b41';
const DECIMALS_SELECTOR = '0x313ce567';

function hexToUtf8(hex: string): string {
  const stripped = hex.startsWith('0x') ? hex.slice(2) : hex;
  if (stripped.length < 128) return '';
  const lengthHex = stripped.slice(64, 128);
  const strLength = parseInt(lengthHex, 16);
  const dataHex = stripped.slice(128, 128 + strLength * 2);
  let result = '';
  for (let i = 0; i < dataHex.length; i += 2) {
    const code = parseInt(dataHex.slice(i, i + 2), 16);
    if (code > 0) result += String.fromCharCode(code);
  }
  return result;
}

function hexToNumber(hex: string): number {
  const stripped = hex.startsWith('0x') ? hex.slice(2) : hex;
  return parseInt(stripped, 16);
}

/**
 * Fetch ERC-20 name, symbol, decimals via a single Multicall3 aggregate3 call.
 * Uses rpcCall which routes through the RPC pool with automatic failover.
 */
async function fetchErc20Meta(
  chainId: number,
  tokenAddress: string,
): Promise<{ name: string; symbol: string; decimals: number } | null> {
  const encoded = encAggregate3([
    { target: tokenAddress, allowFailure: true, callData: '0x' + NAME_SELECTOR.replace('0x', '') },
    { target: tokenAddress, allowFailure: true, callData: '0x' + SYMBOL_SELECTOR.replace('0x', '') },
    { target: tokenAddress, allowFailure: true, callData: '0x' + DECIMALS_SELECTOR.replace('0x', '') },
  ]);

  const response = await rpcCall('eth_call', [{ to: MULTICALL3, data: encoded }, 'latest'], chainId);
  if (response.error || !response.result) return null;

  const results = decAggregate3(response.result);
  if (results.length < 3 || !results[0].success || !results[1].success || !results[2].success) return null;

  const name = hexToUtf8(results[0].data);
  const symbol = hexToUtf8(results[1].data);
  const decimals = hexToNumber(results[2].data);

  if (!name || !symbol) return null;
  return { name, symbol, decimals };
}

// ---------------------------------------------------------------------------
// Network Picker — searchable dropdown
// ---------------------------------------------------------------------------

function NetworkPicker({ selected, onSelect }: { selected: Network; onSelect: (n: Network) => void }) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState('');

  const allNetworks = getAllNetworksSync();
  const filtered = search.trim()
    ? allNetworks.filter(n =>
        n.displayName.toLowerCase().includes(search.toLowerCase()) ||
        n.iconLabel.toLowerCase().includes(search.toLowerCase()) ||
        String(n.chainId).includes(search)
      )
    : allNetworks;

  return (
    <View>
      {/* Selected network button */}
      <Pressable style={styles.pickerButton} onPress={() => setOpen(!open)}>
        <ChainLogo label={selected.iconLabel} color={selected.iconColor} bgColor={selected.iconBg} logoURL={selected.logoURL} size={28} />
        <Text style={styles.pickerButtonText}>{selected.displayName}</Text>
        <Text style={styles.pickerChainId}>Chain {selected.chainId}</Text>
        <ChevronDown size={16} color={color.fg.subtle} style={open ? { transform: [{ rotate: '180deg' }] } : undefined} />
      </Pressable>

      {/* Dropdown */}
      {open && (
        <VelaCard style={styles.pickerDropdown}>
          {/* Search */}
          {allNetworks.length > 5 && (
            <View style={styles.pickerSearchRow}>
              <Search size={14} color={color.fg.subtle} strokeWidth={2} />
              <TextInput
                style={styles.pickerSearchInput}
                placeholder="Search network..."
                placeholderTextColor={color.fg.subtle}
                value={search}
                onChangeText={setSearch}
                autoCapitalize="none"
                autoCorrect={false}
              />
              {search ? (
                <Pressable onPress={() => setSearch('')} hitSlop={8}>
                  <X size={14} color={color.fg.subtle} />
                </Pressable>
              ) : null}
            </View>
          )}

          {/* Network list */}
          <ScrollView style={styles.pickerList} keyboardShouldPersistTaps="handled" nestedScrollEnabled>
            {filtered.map((network, index) => {
              const isSelected = network.chainId === selected.chainId;
              return (
                <Pressable
                  key={network.id}
                  style={[styles.pickerItem, isSelected && styles.pickerItemSelected]}
                  onPress={() => {
                    onSelect(network);
                    setOpen(false);
                    setSearch('');
                  }}
                >
                  <ChainLogo label={network.iconLabel} color={network.iconColor} bgColor={network.iconBg} logoURL={network.logoURL} size={24} />
                  <View style={styles.pickerItemInfo}>
                    <Text style={[styles.pickerItemName, isSelected && styles.pickerItemNameSelected]}>{network.displayName}</Text>
                    <Text style={styles.pickerItemChainId}>Chain {network.chainId}</Text>
                  </View>
                  {isSelected && <Check size={16} color={color.accent.base} strokeWidth={2.5} />}
                </Pressable>
              );
            })}
            {filtered.length === 0 && (
              <Text style={styles.pickerEmpty}>No networks match "{search}"</Text>
            )}
          </ScrollView>
        </VelaCard>
      )}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

export default function AddTokenScreen() {
  const router = useSafeRouter();

  const [contractAddress, setContractAddress] = useState('');
  const [selectedChainId, setSelectedChainId] = useState(1);
  const [loading, setLoading] = useState(false);
  const [tokenMeta, setTokenMeta] = useState<{ name: string; symbol: string; decimals: number } | null>(null);
  const [saving, setSaving] = useState(false);

  const selectedNetwork = getAllNetworksSync().find((n) => n.chainId === selectedChainId) ?? getAllNetworksSync()[0];

  const isValidAddress = /^0x[0-9a-fA-F]{40}$/.test(contractAddress);

  const fetchTokenMetadata = async () => {
    if (!isValidAddress || !selectedNetwork) return;

    setLoading(true);
    setTokenMeta(null);

    try {
      const meta = await fetchErc20Meta(selectedNetwork.chainId, contractAddress);

      if (!meta) {
        showAlert('Not Found', 'Could not find a valid ERC-20 token at this address.');
        return;
      }

      setTokenMeta(meta);
    } catch (err) {
      showAlert('Error', 'Failed to fetch token metadata. Check the address and network.');
    } finally {
      setLoading(false);
    }
  };

  const handleSave = async () => {
    if (!tokenMeta || !selectedNetwork) return;

    setSaving(true);
    try {
      const token: CustomToken = {
        id: `${selectedChainId}_${contractAddress.toLowerCase()}`,
        chainId: selectedChainId,
        contractAddress: contractAddress.toLowerCase(),
        symbol: tokenMeta.symbol,
        name: tokenMeta.name,
        decimals: tokenMeta.decimals,
        networkName: selectedNetwork.displayName,
      };

      await saveCustomToken(token);
      showAlert('Token Added', `${tokenMeta.symbol} has been added to your wallet.`, [
        { text: 'OK', onPress: () => router.back() },
      ]);
    } catch {
      showAlert('Error', 'Failed to save token.');
    } finally {
      setSaving(false);
    }
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        {/* Nav bar */}
        <View style={styles.navBar}>
          <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
            <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
          <Text style={styles.navTitle}>Add Token</Text>
          <View style={styles.navSpacer} />
        </View>

        {/* Network selector */}
        <Text style={styles.fieldLabel}>Network</Text>
        <NetworkPicker
          selected={selectedNetwork}
          onSelect={(n) => { setSelectedChainId(n.chainId); setTokenMeta(null); }}
        />

        {/* Contract address input */}
        <Text style={styles.fieldLabel}>Contract Address</Text>
        <TextInput
          style={styles.input}
          placeholder="0x..."
          placeholderTextColor={color.fg.subtle}
          value={contractAddress}
          onChangeText={(t) => {
            setContractAddress(t);
            setTokenMeta(null);
          }}
          autoCapitalize="none"
          autoCorrect={false}
        />

        {/* Fetch button */}
        <VelaButton
          title="Fetch Token Info"
          onPress={fetchTokenMetadata}
          disabled={!isValidAddress || loading}
          loading={loading}
          variant="secondary"
          style={styles.fetchBtn}
        />

        {/* Token metadata result */}
        {tokenMeta && (
          <Animated.View entering={fadeInDown(0, 300)}>
            <VelaCard elevated style={styles.resultCard}>
              <View style={styles.resultHeader}>
                <Check size={20} color={color.success.base} strokeWidth={2.5} />
                <Text style={styles.resultTitle}>Token Found</Text>
              </View>
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Name</Text>
                <Text style={styles.resultValue}>{tokenMeta.name}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Symbol</Text>
                <Text style={styles.resultValue}>{tokenMeta.symbol}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Decimals</Text>
                <Text style={styles.resultValue}>{tokenMeta.decimals}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Network</Text>
                <Text style={styles.resultValue}>{selectedNetwork?.displayName}</Text>
              </View>

              <VelaButton
                title="Add to Wallet"
                onPress={handleSave}
                variant="accent"
                loading={saving}
                style={styles.saveBtn}
              />
            </VelaCard>
          </Animated.View>
        )}
      </ScrollView>
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  content: {
    paddingBottom: 100,
  },
  navBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
    marginBottom: space.md,
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
  navSpacer: { minWidth: 50 },
  fieldLabel: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase',
    letterSpacing: 0.8,
    marginBottom: space.md,
    marginTop: space['2xl'],
  },

  // Network Picker
  pickerButton: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    paddingHorizontal: space.xl,
    paddingVertical: space.xl,
  },
  pickerButtonText: {
    flex: 1,
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  pickerChainId: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
  },
  pickerDropdown: {
    marginTop: space.md,
    overflow: 'hidden',
  },
  pickerSearchRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingHorizontal: space.xl,
    paddingVertical: space.md,
    borderBottomWidth: 1,
    borderBottomColor: color.border.base,
  },
  pickerSearchInput: {
    flex: 1,
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.base,
    paddingVertical: space.sm,
  },
  pickerList: {
    maxHeight: 280,
  },
  pickerItem: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.lg,
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
  },
  pickerItemSelected: {
    backgroundColor: color.accent.soft,
  },
  pickerItemInfo: {
    flex: 1,
    gap: 1,
  },
  pickerItemName: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.base,
  },
  pickerItemNameSelected: {
    ...inter.semibold,
    color: color.accent.base,
  },
  pickerItemChainId: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  pickerEmpty: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
    textAlign: 'center',
    paddingVertical: space['2xl'],
  },

  // Input
  input: {
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    paddingHorizontal: space.xl,
    paddingVertical: space.xl,
    fontSize: text.base,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  fetchBtn: {
    marginTop: space.xl,
  },
  resultCard: {
    padding: space['2xl'],
    marginTop: space['3xl'],
  },
  resultHeader: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    marginBottom: space.xl,
  },
  resultTitle: {
    fontSize: text.lg,
    ...inter.bold,
    color: color.success.base,
  },
  resultRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
  },
  resultLabel: {
    fontSize: text.base,
    ...inter.regular,
    color: color.fg.muted,
  },
  resultValue: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  separator: {
    height: 1,
    backgroundColor: color.border.base,
  },
  saveBtn: {
    marginTop: space['2xl'],
  },
}));
