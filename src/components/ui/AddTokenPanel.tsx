/**
 * AddTokenPanel — the reusable body of the "Add Token" flow (no screen chrome).
 *
 * Two tabs: import an ERC-20 by contract address (auto-detected across every
 * known network via Multicall3), or add a whole custom network. Rendered both
 * as a full screen (AddTokenScreen) and inside a bottom sheet (AddTokenSheet),
 * so it owns the form + logic but NOT the title bar — the host supplies that.
 *
 * `onAdded` fires after a token or network is successfully saved, letting hosts
 * refresh their lists.
 */
import { QRScanner } from '@/components/QRScanner';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';
import { DEFAULT_NETWORKS, getAllNetworksSync, refreshCustomNetworks } from '@/models/network';
import type { CompatibilityResult, CustomNetwork } from '@/models/types';
import { MULTICALL3, SEL, decAggregate3, decString, decU8, encAggregate3 } from '@/services/abi';
import { fetchChainInfo, searchChains, type ChainSearchResult } from '@/services/chain-registry';
import { checkNetworkCompatibility } from '@/services/network-checker';
import { hapticSuccess, openBrowser, showAlert } from '@/services/platform';
import { rpcCall } from '@/services/rpc-adapter';
import { loadCustomNetworks, loadCustomTokens, saveCustomNetwork, saveCustomToken, getBundlerServiceURL } from '@/services/storage';
import { Check, Globe, ScanLine, Search, X } from 'lucide-react-native';
import React, { useState } from 'react';
import { Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';
import { useTranslation } from 'react-i18next';

/**
 * Fetch ERC-20 name, symbol, decimals via a single Multicall3 aggregate3 call.
 * Uses rpcCall which routes through the RPC pool with automatic failover.
 * Decoding goes through the shared `abi` helpers (`decString` handles both
 * standard string and legacy bytes32 symbols, with proper UTF-8).
 */
async function fetchErc20Meta(
  chainId: number,
  tokenAddress: string,
): Promise<{ name: string; symbol: string; decimals: number } | null> {
  const encoded = encAggregate3([
    { target: tokenAddress, allowFailure: true, callData: '0x' + SEL.name },
    { target: tokenAddress, allowFailure: true, callData: '0x' + SEL.symbol },
    { target: tokenAddress, allowFailure: true, callData: '0x' + SEL.decimals },
  ]);

  const response = await rpcCall('eth_call', [{ to: MULTICALL3, data: encoded }, 'latest'], chainId);
  if (response.error || !response.result) return null;

  const results = decAggregate3(response.result);
  if (results.length < 3 || !results[0].success || !results[1].success || !results[2].success) return null;

  const name = decString(results[0].data);
  const symbol = decString(results[1].data);
  const decimals = decU8(results[2].data);

  if (!name || !symbol) return null;
  return { name, symbol, decimals };
}

type Tab = 'erc20' | 'network';

export function AddTokenPanel({ onAdded }: { onAdded?: () => void }) {
  const { t } = useTranslation();
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
        setNetError(t('addToken.errorAlreadyAdded'));
        setNetLoading(false);
        return;
      }
      const info = await fetchChainInfo(chainId);
      if (!info) { setNetError(t('addToken.errorChainNotFound')); setNetLoading(false); return; }
      setNetChainInfo(info);
      const compat = await checkNetworkCompatibility(info.rpcUrls, chainId);
      setNetCompat(compat);
      if (!compat.compatible) {
        setNetError(compat.error ?? t('addToken.errorNotCompatible'));
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
        bundlerURL: `${getBundlerServiceURL()}/${netChainInfo.chainId}`,
        nativeSymbol: netChainInfo.nativeCurrency?.symbol ?? 'ETH',
        addedAt: new Date().toISOString(),
      };
      await saveCustomNetwork(network);
      await refreshCustomNetworks();
      hapticSuccess();
      setNetError(null);
      setNetChainInfo({ ...netChainInfo, _added: true });
      onAdded?.();
    } catch {
      showAlert(t('addToken.errorTitle'), t('addToken.errorAddNetwork'));
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
      showAlert(t('addToken.notFoundTitle'), t('addToken.notFoundMessage'));
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
      onAdded?.();
    } catch {
      showAlert(t('addToken.errorTitle'), t('addToken.errorSaveToken'));
    } finally {
      setSaving(false);
    }
  };

  return (
    <>
      <ScrollView style={styles.scroll} showsVerticalScrollIndicator={false} contentContainerStyle={styles.content} keyboardShouldPersistTaps="handled">
        {/* Tab switcher */}
        <View style={styles.tabRow}>
          <Pressable style={[styles.tab, tab === 'erc20' && styles.tabActive]} onPress={() => setTab('erc20')}>
            <Text style={[styles.tabText, tab === 'erc20' && styles.tabTextActive]}>{t('addToken.tabErc20')}</Text>
          </Pressable>
          <Pressable style={[styles.tab, tab === 'network' && styles.tabActive]} onPress={() => setTab('network')}>
            <Globe size={14} color={tab === 'network' ? color.accent.base : color.fg.subtle} strokeWidth={2} />
            <Text style={[styles.tabText, tab === 'network' && styles.tabTextActive]}>{t('addToken.tabNative')}</Text>
          </Pressable>
        </View>

        {tab === 'network' ? (
          <>
            <Text style={styles.fieldLabel}>{t('addToken.netSearchLabel')}</Text>
            <TextInput
              style={styles.input}
              placeholder={t('addToken.netSearchPlaceholder')}
              placeholderTextColor={color.fg.subtle}
              value={netQuery}
              onChangeText={handleNetSearch}
              autoCapitalize="none"
              autoCorrect={false}
            />

            {netSearching && <Text style={styles.searchHint}>{t('addToken.netSearching')}</Text>}

            {netSuggestions.length > 0 && (
              <VelaCard style={styles.suggestionsCard}>
                {netSuggestions.map((s, i) => (
                  <React.Fragment key={s.chainId}>
                    {i > 0 && <View style={styles.separator} />}
                    <Pressable style={styles.suggestionRow} onPress={() => { setNetQuery(s.name); handleNetSelect(s.chainId); }}>
                      <Text style={styles.suggestionName}>{s.name}</Text>
                      <Text style={styles.suggestionChainId}>{t('addToken.chainId', { chainId: s.chainId })}</Text>
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
                    <Text style={styles.resultLabel}>{t('addToken.labelName')}</Text>
                    <Text style={styles.resultValue}>{netChainInfo.name}</Text>
                  </View>
                  <View style={styles.separator} />
                  <View style={styles.resultRow}>
                    <Text style={styles.resultLabel}>{t('addToken.labelChainId')}</Text>
                    <Text style={styles.resultValue}>{netChainInfo.chainId}</Text>
                  </View>
                  <View style={styles.separator} />
                  <View style={styles.resultRow}>
                    <Text style={styles.resultLabel}>{t('addToken.labelNativeToken')}</Text>
                    <Text style={styles.resultValue}>{netChainInfo.nativeCurrency?.symbol}</Text>
                  </View>
                  <View style={styles.separator} />
                  <View style={styles.resultRow}>
                    <Text style={styles.resultLabel}>{t('addToken.labelDecimals')}</Text>
                    <Text style={styles.resultValue}>{netChainInfo.nativeCurrency?.decimals}</Text>
                  </View>
                  {netChainInfo.explorerUrl ? (
                    <>
                      <View style={styles.separator} />
                      <View style={styles.resultRow}>
                        <Text style={styles.resultLabel}>{t('addToken.labelExplorer')}</Text>
                        <Pressable onPress={() => openBrowser(netChainInfo.explorerUrl)}>
                          <Text style={[styles.resultValue, { color: color.accent.base }]}>{t('addToken.labelExplorerLink')}</Text>
                        </Pressable>
                      </View>
                    </>
                  ) : null}
                  {/* Editable RPC URL */}
                  <View style={styles.separator} />
                  <Text style={[styles.fieldLabel, { marginTop: space.lg }]}>{t('addToken.labelRpcUrl')}</Text>
                  <TextInput
                    style={styles.input}
                    value={netChainInfo.rpcUrl}
                    onChangeText={(val) => setNetChainInfo({ ...netChainInfo, rpcUrl: val, rpcUrls: [val] })}
                    autoCapitalize="none"
                    autoCorrect={false}
                    placeholder="https://..."
                    placeholderTextColor={color.fg.subtle}
                  />
                </VelaCard>
              </Animated.View>
            )}

            {netLoading && <Text style={styles.searchHint}>{t('addToken.checkingCompat')}</Text>}
            {netError && netCompat && !netCompat.compatible && (
              <Animated.View entering={fadeInDown(0, 300)}>
                <VelaCard elevated style={styles.compatCard}>
                  <Text style={styles.compatTitle}>{t('addToken.compatTitle')}</Text>

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
                      <Text style={styles.compatActionText}>{t('addToken.deployContracts')}</Text>
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
                    <Text style={styles.resultTitle}>{t('addToken.compatible')}</Text>
                  </View>
                  {netChainInfo._added ? (
                    <View style={styles.addedRow}>
                      <Check size={16} color={color.success.base} strokeWidth={2.5} />
                      <Text style={styles.addedText}>{t('addToken.networkAdded')}</Text>
                    </View>
                  ) : (
                    <VelaButton
                      title={t('addToken.addNetworkBtn')}
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
        <Text style={styles.fieldLabel}>{t('addToken.tokenAddressLabel')}</Text>
        <View style={styles.inputRow}>
          <TextInput
            style={styles.inputWithIcon}
            placeholder="0x..."
            placeholderTextColor={color.fg.subtle}
            value={contractAddress}
            onChangeText={(val) => {
              setContractAddress(val);
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
          title={loading ? t('addToken.searchingNetworks') : t('addToken.searchTokenBtn')}
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
                <Text style={styles.resultLabel}>{t('addToken.labelName')}</Text>
                <Text style={styles.resultValue}>{token.name}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>{t('addToken.labelSymbol')}</Text>
                <Text style={styles.resultValue}>{token.symbol}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>{t('addToken.labelDecimals')}</Text>
                <Text style={styles.resultValue}>{token.decimals}</Text>
              </View>
              <View style={styles.separator} />
              <View style={styles.resultRow}>
                <Text style={styles.resultLabel}>{t('addToken.labelNetwork')}</Text>
                <Text style={styles.resultValue}>{token.networkName}</Text>
              </View>

              {addedTokenIds.has(`${token.chainId}_${contractAddress.toLowerCase()}`) ? (
                <View style={styles.addedRow}>
                  <Check size={16} color={color.success.base} strokeWidth={2.5} />
                  <Text style={styles.addedText}>{t('addToken.tokenAdded')}</Text>
                </View>
              ) : (
                <VelaButton
                  title={t('addToken.addToWalletBtn')}
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
    </>
  );
}

const styles = createStyles(() => ({
  scroll: {
    flex: 1,
  },
  content: {
    paddingBottom: 100,
  },

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
