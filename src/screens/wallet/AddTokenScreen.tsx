import { ChainLogo } from '@/components/ChainLogo';
import { QRScanner } from '@/components/QRScanner';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import type { Network } from '@/models/network';
import { DEFAULT_NETWORKS, getAllNetworksSync, refreshCustomNetworks } from '@/models/network';
import type { CompatibilityResult, CustomNetwork, CustomToken } from '@/models/types';
import { MULTICALL3, decAggregate3, encAggregate3 } from '@/services/abi';
import { fetchChainInfo, searchChains, type ChainSearchResult } from '@/services/chain-registry';
import { checkNetworkCompatibility } from '@/services/network-checker';
import { hapticSuccess, openBrowser, showAlert } from '@/services/platform';
import { rpcCall } from '@/services/rpc-adapter';
import { loadCustomNetworks, loadCustomTokens, saveCustomNetwork, saveCustomToken } from '@/services/storage';
import { ArrowLeft, Check, ChevronDown, Globe, ScanLine, Search, X } from 'lucide-react-native';
import React, { useState } from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

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

type Tab = 'erc20' | 'network';

export default function AddTokenScreen() {
  const router = useSafeRouter();
  const [tab, setTab] = useState<Tab>('erc20');

  // ERC-20 state
  const [contractAddress, setContractAddress] = useState('');
  const [loading, setLoading] = useState(false);
  const [foundTokens, setFoundTokens] = useState<{ chainId: number; networkName: string; name: string; symbol: string; decimals: number }[]>([]);
  const [saving, setSaving] = useState(false);
  const [showScanner, setShowScanner] = useState(false);
  const [addedTokenIds, setAddedTokenIds] = useState<Set<string>>(new Set());

  // Network state
  const [netQuery, setNetQuery] = useState('');
  const [netSuggestions, setNetSuggestions] = useState<ChainSearchResult[]>([]);
  const [netSearching, setNetSearching] = useState(false);
  const [netChainInfo, setNetChainInfo] = useState<any>(null);
  const [netCompat, setNetCompat] = useState<CompatibilityResult | null>(null);
  const [netLoading, setNetLoading] = useState(false);
  const [netSaving, setNetSaving] = useState(false);
  const [netError, setNetError] = useState<string | null>(null);

  // --- Network tab logic ---
  const handleNetSearch = async (q: string) => {
    setNetQuery(q);
    setNetChainInfo(null);
    setNetCompat(null);
    setNetError(null);
    if (q.trim().length < 2) { setNetSuggestions([]); return; }
    setNetSearching(true);
    try {
      const results = await searchChains(q.trim());
      setNetSuggestions(results.slice(0, 8));
    } catch { setNetSuggestions([]); }
    setNetSearching(false);
  };

  const handleNetSelect = async (chainId: number) => {
    setNetSuggestions([]);
    setNetLoading(true);
    setNetError(null);
    try {
      const existing = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
      const custom = await loadCustomNetworks();
      if (existing || custom.find(n => n.chainId === chainId)) {
        setNetError('This network is already added');
        setNetLoading(false);
        return;
      }
      const info = await fetchChainInfo(chainId);
      if (!info) { setNetError('Chain info not found'); setNetLoading(false); return; }
      setNetChainInfo(info);
      const compat = await checkNetworkCompatibility(info.rpcUrls, chainId);
      setNetCompat(compat);
      if (!compat.compatible) {
        setNetError(compat.error ?? 'Not compatible with Vela Wallet');
      }
    } catch (err) {
      setNetError(err instanceof Error ? err.message : 'Failed to fetch chain info');
    }
    setNetLoading(false);
  };

  const handleNetAdd = async () => {
    if (!netChainInfo || !netCompat?.compatible) return;
    setNetSaving(true);
    try {
      const network: CustomNetwork = {
        id: `custom-${netChainInfo.chainId}`,
        displayName: netChainInfo.name,
        chainId: netChainInfo.chainId,
        iconLabel: (netChainInfo.nativeCurrency?.symbol ?? 'ETH').slice(0, 4),
        iconColor: '#888888',
        iconBg: '#F0F0F0',
        logoURL: netChainInfo.logoURL ?? '',
        isL2: false,
        rpcURL: netCompat.bestRpcUrl ?? netChainInfo.rpcUrl ?? '',
        explorerURL: netChainInfo.explorerUrl ?? '',
        bundlerURL: `https://bundler.getvela.app/${netChainInfo.chainId}`,
        nativeSymbol: netChainInfo.nativeCurrency?.symbol ?? 'ETH',
        addedAt: new Date().toISOString(),
      };
      await saveCustomNetwork(network);
      await refreshCustomNetworks();
      hapticSuccess();
      setNetError(null);
      setNetChainInfo({ ...netChainInfo, _added: true });
    } catch {
      showAlert('Error', 'Failed to add network.');
    }
    setNetSaving(false);
  };

  const isValidAddress = /^0x[0-9a-fA-F]{40}$/.test(contractAddress);

  const fetchTokenMetadata = async () => {
    if (!isValidAddress) return;

    setLoading(true);
    setFoundTokens([]);

    // Query all networks in parallel
    const allNetworks = getAllNetworksSync();
    const results = await Promise.allSettled(
      allNetworks.map(async (network) => {
        const meta = await fetchErc20Meta(network.chainId, contractAddress);
        if (!meta) return null;
        return { chainId: network.chainId, networkName: network.displayName, ...meta };
      }),
    );

    const found: typeof foundTokens = [];
    for (const r of results) {
      if (r.status === 'fulfilled' && r.value) found.push(r.value);
    }

    if (found.length === 0) {
      showAlert('Not Found', 'Could not find this token on any network.');
    }
    setFoundTokens(found);
    setLoading(false);
  };

  const handleSave = async (token: typeof foundTokens[0]) => {
    const tokenId = `${token.chainId}_${contractAddress.toLowerCase()}`;

    // Check if already added
    if (addedTokenIds.has(tokenId)) return;
    const existing = await loadCustomTokens();
    if (existing.some(t => t.id === tokenId)) {
      setAddedTokenIds(prev => new Set(prev).add(tokenId));
      return;
    }

    setSaving(true);
    try {
      await saveCustomToken({
        id: tokenId,
        chainId: token.chainId,
        contractAddress: contractAddress.toLowerCase(),
        symbol: token.symbol,
        name: token.name,
        decimals: token.decimals,
        networkName: token.networkName,
      });
      hapticSuccess();
      setAddedTokenIds(prev => new Set(prev).add(tokenId));
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

        {/* Tab switcher */}
        <View style={styles.tabRow}>
          <Pressable style={[styles.tab, tab === 'erc20' && styles.tabActive]} onPress={() => setTab('erc20')}>
            <Text style={[styles.tabText, tab === 'erc20' && styles.tabTextActive]}>ERC-20 Token</Text>
          </Pressable>
          <Pressable style={[styles.tab, tab === 'network' && styles.tabActive]} onPress={() => setTab('network')}>
            <Globe size={14} color={tab === 'network' ? color.accent.base : color.fg.subtle} strokeWidth={2} />
            <Text style={[styles.tabText, tab === 'network' && styles.tabTextActive]}>Native Token</Text>
          </Pressable>
        </View>

        {tab === 'network' ? (
          <>
            <Text style={styles.fieldLabel}>Search Network</Text>
            <TextInput
              style={styles.input}
              placeholder="Name or chain ID (e.g. Avalanche, 43114)"
              placeholderTextColor={color.fg.subtle}
              value={netQuery}
              onChangeText={handleNetSearch}
              autoCapitalize="none"
              autoCorrect={false}
            />

            {netSearching && <Text style={styles.searchHint}>Searching...</Text>}

            {netSuggestions.length > 0 && (
              <VelaCard style={styles.suggestionsCard}>
                {netSuggestions.map((s, i) => (
                  <React.Fragment key={s.chainId}>
                    {i > 0 && <View style={styles.separator} />}
                    <Pressable style={styles.suggestionRow} onPress={() => { setNetQuery(s.name); handleNetSelect(s.chainId); }}>
                      <Text style={styles.suggestionName}>{s.name}</Text>
                      <Text style={styles.suggestionChainId}>Chain {s.chainId}</Text>
                    </Pressable>
                  </React.Fragment>
                ))}
              </VelaCard>
            )}

            {/* Chain info card — shown as soon as chain data is fetched */}
            {netChainInfo && !netChainInfo._added && (
              <Animated.View entering={fadeInDown(0, 300)}>
                <VelaCard style={styles.resultCard}>
                  <View style={styles.resultRow}>
                    <Text style={styles.resultLabel}>Name</Text>
                    <Text style={styles.resultValue}>{netChainInfo.name}</Text>
                  </View>
                  <View style={styles.separator} />
                  <View style={styles.resultRow}>
                    <Text style={styles.resultLabel}>Chain ID</Text>
                    <Text style={styles.resultValue}>{netChainInfo.chainId}</Text>
                  </View>
                  <View style={styles.separator} />
                  <View style={styles.resultRow}>
                    <Text style={styles.resultLabel}>Native Token</Text>
                    <Text style={styles.resultValue}>{netChainInfo.nativeCurrency?.symbol}</Text>
                  </View>
                  <View style={styles.separator} />
                  <View style={styles.resultRow}>
                    <Text style={styles.resultLabel}>Decimals</Text>
                    <Text style={styles.resultValue}>{netChainInfo.nativeCurrency?.decimals}</Text>
                  </View>
                  {netChainInfo.explorerUrl ? (
                    <>
                      <View style={styles.separator} />
                      <View style={styles.resultRow}>
                        <Text style={styles.resultLabel}>Explorer</Text>
                        <Pressable onPress={() => openBrowser(netChainInfo.explorerUrl)}>
                          <Text style={[styles.resultValue, { color: color.accent.base }]}>View ↗</Text>
                        </Pressable>
                      </View>
                    </>
                  ) : null}
                  {/* Editable RPC URL */}
                  <View style={styles.separator} />
                  <Text style={[styles.fieldLabel, { marginTop: space.lg }]}>RPC URL</Text>
                  <TextInput
                    style={styles.input}
                    value={netChainInfo.rpcUrl}
                    onChangeText={(t) => setNetChainInfo({ ...netChainInfo, rpcUrl: t, rpcUrls: [t] })}
                    autoCapitalize="none"
                    autoCorrect={false}
                    placeholder="https://..."
                    placeholderTextColor={color.fg.subtle}
                  />
                </VelaCard>
              </Animated.View>
            )}

            {netLoading && <Text style={styles.searchHint}>Checking compatibility...</Text>}
            {netError && netCompat && !netCompat.compatible && (
              <Animated.View entering={fadeInDown(0, 300)}>
                <VelaCard elevated style={styles.compatCard}>
                  <Text style={styles.compatTitle}>Compatibility Check</Text>

                  {/* Contract checklist */}
                  {netCompat.contracts.map((c) => (
                    <View key={c.address} style={styles.compatRow}>
                      {c.deployed ? (
                        <Check size={14} color={color.success.base} strokeWidth={2.5} />
                      ) : (
                        <X size={14} color={color.fg.subtle} strokeWidth={2} />
                      )}
                      <Text style={[styles.compatName, c.deployed && styles.compatNameOk]}>
                        {c.name}
                      </Text>
                    </View>
                  ))}

                  {/* P256 status */}
                  <View style={styles.compatRow}>
                    {netCompat.p256Available ? (
                      <Check size={14} color={color.success.base} strokeWidth={2.5} />
                    ) : (
                      <X size={14} color={color.fg.subtle} strokeWidth={2} />
                    )}
                    <Text style={[styles.compatName, netCompat.p256Available && styles.compatNameOk]}>
                      P256 Precompile (RIP-7212)
                    </Text>
                  </View>

                  {/* Deploy link */}
                  {netChainInfo && (
                    <Pressable
                      style={styles.compatAction}
                      onPress={() => openBrowser(`https://biubiu.tools/apps/vela-wallet-chain-setup?chainId=${netChainInfo.chainId}`)}
                    >
                      <Text style={styles.compatActionText}>Deploy missing contracts ↗</Text>
                    </Pressable>
                  )}
                </VelaCard>
              </Animated.View>
            )}
            {netError && !netCompat && <Text style={styles.errorText}>{netError}</Text>}

            {netChainInfo && netCompat?.compatible && (
              <Animated.View entering={fadeInDown(0, 300)}>
                <VelaCard elevated style={styles.resultCard}>
                  <View style={styles.resultHeader}>
                    <Check size={20} color={color.success.base} strokeWidth={2.5} />
                    <Text style={styles.resultTitle}>Compatible</Text>
                  </View>
                  {netChainInfo._added ? (
                    <View style={styles.addedRow}>
                      <Check size={16} color={color.success.base} strokeWidth={2.5} />
                      <Text style={styles.addedText}>Network Added</Text>
                    </View>
                  ) : (
                    <VelaButton
                      title="Add Network"
                      onPress={handleNetAdd}
                      variant="accent"
                      loading={netSaving}
                      style={styles.saveBtn}
                    />
                  )}
                </VelaCard>
              </Animated.View>
            )}
          </>
        ) : (
          <>
        {/* ERC-20 tab content — just contract address, auto-detect networks */}
        <Text style={styles.fieldLabel}>Token Address</Text>
        <View style={styles.inputRow}>
          <TextInput
            style={styles.inputWithIcon}
            placeholder="0x..."
            placeholderTextColor={color.fg.subtle}
            value={contractAddress}
            onChangeText={(t) => {
              setContractAddress(t);
              setFoundTokens([]);
            }}
            autoCapitalize="none"
            autoCorrect={false}
          />
          <Pressable onPress={() => setShowScanner(true)} hitSlop={6} style={styles.scanBtn}>
            <ScanLine size={20} color={color.fg.subtle} strokeWidth={2} />
          </Pressable>
        </View>

        {/* Fetch button */}
        <VelaButton
          title={loading ? 'Searching all networks...' : 'Search Token'}
          onPress={fetchTokenMetadata}
          disabled={!isValidAddress || loading}
          loading={loading}
          variant="secondary"
          style={styles.fetchBtn}
        />

        {/* Results — one card per network where the token was found */}
        {foundTokens.map((token) => (
          <Animated.View key={token.chainId} entering={fadeInDown(0, 300)}>
            <VelaCard style={styles.resultCard}>
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Name</Text>
                <Text style={styles.resultValue}>{token.name}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Symbol</Text>
                <Text style={styles.resultValue}>{token.symbol}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Decimals</Text>
                <Text style={styles.resultValue}>{token.decimals}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>Network</Text>
                <Text style={styles.resultValue}>{token.networkName}</Text>
              </View>

              {addedTokenIds.has(`${token.chainId}_${contractAddress.toLowerCase()}`) ? (
                <View style={styles.addedRow}>
                  <Check size={16} color={color.success.base} strokeWidth={2.5} />
                  <Text style={styles.addedText}>Added</Text>
                </View>
              ) : (
                <VelaButton
                  title="Add to Wallet"
                  onPress={() => handleSave(token)}
                  variant="accent"
                  loading={saving}
                  style={styles.saveBtn}
                />
              )}
            </VelaCard>
          </Animated.View>
        ))}
          </>
        )}
      </ScrollView>

      {showScanner && (
        <QRScanner
          visible={showScanner}
          onScan={(data) => {
            setShowScanner(false);
            // Extract 0x address from QR data (may include ethereum: prefix or extra params)
            const match = data.match(/0x[0-9a-fA-F]{40}/);
            if (match) {
              setContractAddress(match[0]);
              setFoundTokens([]);
            }
          }}
          onClose={() => setShowScanner(false)}
        />
      )}
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

  // Tabs
  tabRow: {
    flexDirection: 'row',
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    padding: 3,
    marginBottom: space.xl,
  },
  tab: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingVertical: space.md,
    borderRadius: radius.md,
  },
  tabActive: {
    backgroundColor: color.bg.raised,
    ...shadow.sm,
  },
  tabText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.subtle,
  },
  tabTextActive: {
    color: color.accent.base,
  },

  // Network search
  searchHint: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.md,
  },
  errorText: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.error.base,
    marginTop: space.md,
  },
  compatCard: {
    padding: space.xl,
    marginTop: space.xl,
  },
  compatTitle: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.8,
    marginBottom: space.lg,
  },
  compatRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.sm,
  },
  compatName: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
  },
  compatNameOk: {
    color: color.fg.base,
  },
  compatAction: {
    marginTop: space.xl,
    paddingVertical: space.lg,
    backgroundColor: color.accent.soft,
    borderRadius: radius.lg,
    alignItems: 'center',
  },
  compatActionText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.accent.base,
  },
  suggestionsCard: {
    marginTop: space.md,
    overflow: 'hidden' as const,
  },
  suggestionRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingHorizontal: space.xl,
    paddingVertical: space.lg,
  },
  suggestionName: {
    fontSize: text.base,
    ...inter.medium,
    color: color.fg.base,
  },
  suggestionChainId: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.subtle,
  },

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
  inputRow: {
    flexDirection: 'row',
    alignItems: 'center',
    backgroundColor: color.bg.sunken,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.border.base,
  },
  inputWithIcon: {
    flex: 1,
    paddingHorizontal: space.xl,
    paddingVertical: space.xl,
    fontSize: text.base,
    ...inter.medium,
    fontFamily: font.mono,
    color: color.fg.base,
  },
  scanBtn: {
    paddingHorizontal: space.lg,
    paddingVertical: space.lg,
  },
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
  addedRow: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    marginTop: space['2xl'],
    paddingVertical: space.lg,
  },
  addedText: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.success.base,
  },
}));
