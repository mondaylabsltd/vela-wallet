import React, { useState, useEffect, useCallback } from 'react';
import {
  View,
  Text,
  ScrollView,
  Pressable,
  Alert,
  TextInput,
  ActivityIndicator,
  Linking,
} from 'react-native';
import { AppModal } from '@/components/ui/AppModal';
import { useRouter } from 'expo-router';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaCard } from '@/components/ui/VelaCard';
import { VelaButton } from '@/components/ui/VelaButton';
import { ChainLogo } from '@/components/ChainLogo';
import { color, text, inter, space, radius, font, shadow, useStyles } from '@/constants/theme';
import { TEXT_SCALE_LEVELS, useTextScale } from '@/constants/text-scale';
import { useColorSchemePreference, type ColorSchemePreference } from '@/constants/color-scheme';
import { useWallet, shortAddress } from '@/models/wallet-state';
import { DEFAULT_NETWORKS, getAllNetworks, refreshCustomNetworks } from '@/models/network';
import type { Network } from '@/models/network';
import { saveNetworkConfig, loadNetworkConfigs, clearAll, loadServiceEndpoints, saveServiceEndpoints, saveCustomNetwork, loadCustomNetworks, removeCustomNetwork } from '@/services/storage';
import { checkNetworkCompatibility } from '@/services/network-checker';
import { refreshPool } from '@/services/rpc-pool';
import { clearBundlerCache } from '@/services/bundler-service';
import { fetchChainInfo, searchChains, type ChainSearchResult } from '@/services/chain-registry';
import { User as UserIcon, Globe as NetworkIcon, Info as InfoIcon, LogOut as LogOutIcon, Check, ChevronRight, ChevronDown, X, Server, Plus, Trash2, RefreshCw, CheckCircle2, XCircle, AlertTriangle, ExternalLink, Sun, Moon, Monitor } from 'lucide-react-native';
import type { NetworkConfig, ServiceEndpoints, CustomNetwork, CompatibilityResult } from '@/models/types';
import { DEFAULT_SERVICE_ENDPOINTS } from '@/models/types';
import { getAccountBalances } from '@/services/balance-cache';
import { toHex } from '@/services/hex';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withSpring,
  runOnJS,
} from 'react-native-reanimated';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import * as Haptics from 'expo-haptics';
import { fadeIn, fadeInDown } from '@/constants/entering';

// All styles in one factory → useStyles recomputes everything on text scale change
type S = ReturnType<typeof styleFactory>;

type IconConfig = { bg: string; fg: string; Icon: React.ComponentType<{ size: number; color: string }> };

function SettingsRow({ s, icon, title, subtitle, showDivider = true, onPress, right }: {
  s: S; icon: IconConfig; title: string; subtitle?: string; showDivider?: boolean; onPress?: () => void; right?: React.ReactNode;
}) {
  return (
    <Pressable style={s.settingsRow} onPress={onPress} disabled={!onPress}>
      <View style={[s.settingsIcon, { backgroundColor: icon.bg }]}>
        <icon.Icon size={16} color={icon.fg} />
      </View>
      <View style={s.settingsRowContent}>
        <Text style={s.settingsRowTitle}>{title}</Text>
        {subtitle ? <Text style={s.settingsRowSubtitle}>{subtitle}</Text> : null}
      </View>
      {right ?? (onPress ? <ChevronRight size={16} color={color.fg.subtle} /> : null)}
      {showDivider ? <View style={s.settingsRowDivider} /> : null}
    </Pressable>
  );
}

type EndpointHealth = { status: 'checking' | 'ok' | 'error'; latencyMs?: number };

async function checkEndpointHealth(url: string, type: 'rpc' | 'explorer' | 'bundler'): Promise<EndpointHealth> {
  if (!url) return { status: 'error' };
  const start = Date.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10000);
  try {
    if (type === 'rpc') {
      if (url.startsWith('wss://') || url.startsWith('ws://')) {
        // WebSocket RPC: open connection, send eth_chainId, wait for response
        return await new Promise<EndpointHealth>((resolve) => {
          const ws = new WebSocket(url);
          const done = (result: EndpointHealth) => { try { ws.close(); } catch {} clearTimeout(timeout); resolve(result); };
          ws.onopen = () => { ws.send(JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] })); };
          ws.onmessage = (e) => { try { const d = JSON.parse(e.data); if (d.result) done({ status: 'ok', latencyMs: Date.now() - start }); else done({ status: 'error' }); } catch { done({ status: 'error' }); } };
          ws.onerror = () => done({ status: 'error' });
          controller.signal.addEventListener('abort', () => done({ status: 'error' }));
        });
      }
      // HTTPS RPC: send eth_chainId, check for valid JSON-RPC response
      const res = await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      if (!res.ok) return { status: 'error' };
      const json = await res.json();
      if (json.result) return { status: 'ok', latencyMs: Date.now() - start };
      return { status: 'error' };
    } else if (type === 'bundler') {
      // Bundler: may require API key, just check if server responds (even 401/403 means reachable)
      await fetch(url, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'eth_chainId', params: [] }),
        signal: controller.signal,
      });
      clearTimeout(timeout);
      // Any HTTP response means the server is reachable
      return { status: 'ok', latencyMs: Date.now() - start };
    } else {
      // Explorer: GET the URL, follow redirects, accept any 2xx/3xx
      const res = await fetch(url, { method: 'GET', signal: controller.signal, redirect: 'follow' });
      clearTimeout(timeout);
      return { status: res.ok ? 'ok' : 'error', latencyMs: Date.now() - start };
    }
  } catch {
    clearTimeout(timeout);
    return { status: 'error' };
  }
}

function HealthBadge({ health }: { health: EndpointHealth }) {
  if (health.status === 'checking') {
    return <ActivityIndicator size={10} color={color.fg.subtle} style={{ marginLeft: 6 }} />;
  }
  const dotColor = health.status === 'ok' ? color.success.base : color.accent.base;
  const label = health.status === 'ok'
    ? `${health.latencyMs}ms`
    : 'Offline';
  return (
    <View style={healthStyles.badge}>
      <View style={[healthStyles.dot, { backgroundColor: dotColor }]} />
      <Text style={[healthStyles.text, { color: health.status === 'ok' ? color.success.base : color.accent.base }]}>
        {label}
      </Text>
    </View>
  );
}

const healthStyles = {
  badge: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: 4, marginLeft: 8 },
  dot: { width: 6, height: 6, borderRadius: 3 },
  text: { fontSize: 11, fontWeight: '500' as const },
};

function NetworkConfigCard({ s, network, savedConfig, onSave, onDelete }: {
  s: S; network: Network; savedConfig?: NetworkConfig;
  onSave: (config: NetworkConfig) => void; onDelete?: () => void;
}) {
  const [expanded, setExpanded] = useState(false);
  const [rpcURL, setRpcURL] = useState(savedConfig?.rpcURL ?? network.rpcURL);
  const [explorerURL, setExplorerURL] = useState(savedConfig?.explorerURL ?? network.explorerURL);
  const [bundlerURL, setBundlerURL] = useState(savedConfig?.bundlerURL ?? network.bundlerURL);
  const [healths, setHealths] = useState<[EndpointHealth, EndpointHealth, EndpointHealth]>([
    { status: 'checking' }, { status: 'checking' }, { status: 'checking' },
  ]);

  const handleSave = useCallback(() => {
    onSave({ chainId: network.chainId, rpcURL, explorerURL, bundlerURL });
  }, [network.chainId, rpcURL, explorerURL, bundlerURL, onSave]);

  // Run health checks when expanded
  useEffect(() => {
    if (!expanded) return;
    setHealths([{ status: 'checking' }, { status: 'checking' }, { status: 'checking' }]);
    const urls = [rpcURL, explorerURL, bundlerURL];
    const types: ('rpc' | 'explorer' | 'bundler')[] = ['rpc', 'explorer', 'bundler'];
    urls.forEach((url, i) => {
      checkEndpointHealth(url, types[i]).then(h => {
        setHealths(prev => { const next = [...prev] as typeof prev; next[i] = h; return next; });
      });
    });
  }, [expanded, rpcURL, explorerURL, bundlerURL]);

  return (
    <VelaCard style={s.networkCard}>
      <Pressable style={s.networkHeader} onPress={() => setExpanded(!expanded)}>
        <ChainLogo label={network.iconLabel} color={network.iconColor} bgColor={network.iconBg} logoURL={network.logoURL} size={36} />
        <View style={s.networkHeaderText}>
          <Text style={s.networkName}>{network.displayName}</Text>
          <Text style={s.networkChainId}>Chain {network.chainId}</Text>
        </View>
        {onDelete && (
          <Pressable onPress={onDelete} hitSlop={8} style={s.deleteNetBtn}>
            <Trash2 size={14} color={color.fg.subtle} />
          </Pressable>
        )}
        <ChevronRight size={16} color={color.fg.subtle} style={expanded ? { transform: [{ rotate: '90deg' }] } : undefined} />
      </Pressable>
      {expanded && (
        <View style={s.networkFields}>
          <View style={s.dividerFull} />
          {(['RPC URL', 'EXPLORER', 'BUNDLER'] as const).map((label, i) => {
            const vals = [rpcURL, explorerURL, bundlerURL];
            const setters = [setRpcURL, setExplorerURL, setBundlerURL];
            return (
              <View key={label} style={s.configField}>
                <View style={s.configLabelRow}>
                  <Text style={s.configLabel}>{label}</Text>
                  <HealthBadge health={healths[i]} />
                </View>
                <TextInput style={s.configInput} value={vals[i]} onChangeText={setters[i]} onBlur={handleSave}
                  autoCapitalize="none" autoCorrect={false} placeholder={label} placeholderTextColor={color.fg.subtle} />
              </View>
            );
          })}
        </View>
      )}
    </VelaCard>
  );
}

// ---------------------------------------------------------------------------
// Account Switcher Modal
// ---------------------------------------------------------------------------

function formatUsd(value: number): string {
  return '$' + value.toLocaleString('en-US', { minimumFractionDigits: 2, maximumFractionDigits: 2 });
}

function AccountSwitcherModal({ s, visible, onClose }: { s: S; visible: boolean; onClose: () => void }) {
  const { state, dispatch } = useWallet();
  const router = useRouter();
  const [cachedBalances, setCachedBalances] = useState<Map<string, number>>(new Map());

  useEffect(() => {
    if (!visible) return;
    getAccountBalances(state.accounts.map(a => a.address)).then(setCachedBalances);
  }, [visible, state.accounts]);

  const allTotal = [...cachedBalances.values()].reduce((s, v) => s + v, 0);

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <View>
            <Text style={s.modalTitle}>Accounts</Text>
            {cachedBalances.size > 0 && (
              <Text style={s.accountTotalLabel}>Total {formatUsd(allTotal)}</Text>
            )}
          </View>
          <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.modalScrollContent}>
          {state.accounts
            .map((account, index) => ({ account, index }))
            .sort((a, b) => {
              const balA = cachedBalances.get(a.account.address) ?? -1;
              const balB = cachedBalances.get(b.account.address) ?? -1;
              if (balB !== balA) return balB - balA;
              return a.account.name.localeCompare(b.account.name);
            })
            .map(({ account, index }) => {
            const isActive = index === state.activeAccountIndex;
            const bal = cachedBalances.get(account.address);
            return (
              <Pressable key={account.id} style={[s.accountItem, isActive && s.accountItemActive]}
                onPress={() => { dispatch({ type: 'SWITCH_ACCOUNT', index }); Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success); onClose(); }}>
                <View style={s.accountAvatar}>
                  <Text style={s.accountAvatarText}>{(account.name[0] ?? 'V').toUpperCase()}</Text>
                </View>
                <View style={s.accountInfo}>
                  <Text style={s.accountNameModal}>{account.name}</Text>
                  <Text style={s.accountAddress}>{shortAddress(account.address)}</Text>
                </View>
                <View style={s.accountRight}>
                  {bal != null && <Text style={s.accountBal}>{formatUsd(bal)}</Text>}
                  {isActive && <Check size={18} color={color.accent.base} />}
                </View>
              </Pressable>
            );
          })}
          <View style={s.accountActions}>
            <VelaButton title="Create New Account" onPress={() => { onClose(); router.push('/onboarding'); }} />
            <VelaButton title="Sign In with Existing" variant="secondary" onPress={() => { onClose(); router.push('/onboarding'); }} />
          </View>
        </ScrollView>
      </View>
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Network Editor Modal (with custom networks)
// ---------------------------------------------------------------------------

function NetworkEditorModal({ s, visible, onClose }: { s: S; visible: boolean; onClose: () => void }) {
  const [savedConfigs, setSavedConfigs] = useState<NetworkConfig[]>([]);
  const [allNetworks, setAllNetworks] = useState<Network[]>(DEFAULT_NETWORKS);
  const [customIds, setCustomIds] = useState<Set<string>>(new Set());

  useEffect(() => {
    if (!visible) return;
    loadNetworkConfigs().then(setSavedConfigs);
    getAllNetworks().then(setAllNetworks);
    loadCustomNetworks().then(cn => setCustomIds(new Set(cn.map(c => c.id))));
  }, [visible]);

  const handleSave = useCallback(async (config: NetworkConfig) => {
    await saveNetworkConfig(config);
    setSavedConfigs(await loadNetworkConfigs());
    // Flush caches so new endpoints take effect immediately
    refreshPool(config.chainId);
    clearBundlerCache(config.chainId);
  }, []);

  const handleDelete = useCallback(async (id: string) => {
    Alert.alert('Remove Network', 'Remove this custom network?', [
      { text: 'Cancel', style: 'cancel' },
      { text: 'Remove', style: 'destructive', onPress: async () => {
        await removeCustomNetwork(id);
        await refreshCustomNetworks();
        setAllNetworks(await getAllNetworks());
        setCustomIds(prev => { const next = new Set(prev); next.delete(id); return next; });
      }},
    ]);
  }, []);

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <Text style={s.modalTitle}>Networks</Text>
          <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.networkScrollContent} keyboardShouldPersistTaps="handled">
          {allNetworks.map((network) => (
            <NetworkConfigCard key={network.id} s={s} network={network}
              savedConfig={savedConfigs.find((c) => c.chainId === network.chainId)}
              onSave={handleSave}
              onDelete={customIds.has(network.id) ? () => handleDelete(network.id) : undefined} />
          ))}
        </ScrollView>
      </View>
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Endpoint Editor Modal
// ---------------------------------------------------------------------------

type ServiceHealth = {
  status: 'checking' | 'ok' | 'not_https' | 'unreachable' | 'invalid_response';
  latencyMs?: number;
  detail?: string;
};

const SERVICE_IDENTITY: Record<string, string> = {
  data: 'ethereum-data',
  passkey: 'webauthn-p256-publickey-index',
  bundler: 'vela-bundler',
};

async function checkServiceEndpointHealth(
  url: string, type: 'data' | 'passkey' | 'bundler',
): Promise<ServiceHealth> {
  if (!url) return { status: 'unreachable', detail: 'Empty URL' };

  // 1. HTTPS check
  if (!url.startsWith('https://')) {
    return { status: 'not_https', detail: 'HTTPS required' };
  }

  // 2. Connectivity + 3. Response validation via /api/health
  const base = url.trim().replace(/[\r\n]/g, '').replace(/\/$/, '');
  const expected = SERVICE_IDENTITY[type];
  const start = Date.now();
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 10000);
  try {
    console.log(`[HealthCheck] ${type} → GET ${base}/api/health?_t=${start}`);
    const res = await fetch(
      `${base}/api/health?_t=${start}`,
      { method: 'GET', signal: controller.signal },
    );
    clearTimeout(timeout);
    const latencyMs = Date.now() - start;
    console.log(`[HealthCheck] ${type} → HTTP ${res.status}, ${latencyMs}ms`);
    if (!res.ok) return { status: 'unreachable', latencyMs, detail: `HTTP ${res.status}` };
    const text = await res.text();
    console.log(`[HealthCheck] ${type} → body: ${text}`);
    const json = JSON.parse(text);
    if (json.service !== expected || json.status !== 'ok') {
      console.log(`[HealthCheck] ${type} → INVALID: expected service="${expected}", got service="${json.service}" status="${json.status}"`);
      return { status: 'invalid_response', latencyMs, detail: `Not a valid ${expected} service` };
    }
    return { status: 'ok', latencyMs };
  } catch (e: any) {
    clearTimeout(timeout);
    console.log(`[HealthCheck] ${type} → CATCH: ${e?.message ?? e}`);
    return { status: 'unreachable', detail: 'Connection failed' };
  }
}

function ServiceHealthBadge({ health }: { health: ServiceHealth }) {
  if (health.status === 'checking') {
    return <ActivityIndicator size={10} color={color.fg.subtle} style={{ marginLeft: 6 }} />;
  }
  const cfg: Record<string, { dot: string; label: string }> = {
    ok: { dot: color.success.base, label: `${health.latencyMs ?? 0}ms` },
    not_https: { dot: color.accent.base, label: 'HTTPS required' },
    unreachable: { dot: color.accent.base, label: 'Offline' },
    invalid_response: { dot: color.warning.base, label: health.detail ?? 'Invalid' },
  };
  const { dot, label } = cfg[health.status] ?? cfg.unreachable;
  return (
    <View style={healthStyles.badge}>
      <View style={[healthStyles.dot, { backgroundColor: dot }]} />
      <Text style={[healthStyles.text, { color: dot }]}>{label}</Text>
    </View>
  );
}

function EndpointEditorModal({ s, visible, onClose }: { s: S; visible: boolean; onClose: () => void }) {
  const [endpoints, setEndpoints] = useState<ServiceEndpoints>({ ...DEFAULT_SERVICE_ENDPOINTS });
  const [healths, setHealths] = useState<Record<string, ServiceHealth>>({});
  const [refreshCount, setRefreshCount] = useState(0);

  useEffect(() => { if (visible) loadServiceEndpoints().then(setEndpoints); }, [visible]);

  // Health checks on open and manual refresh
  useEffect(() => {
    if (!visible) return;
    const keys = ['ethereumDataURL', 'passkeyIndexURL', 'bundlerServiceURL'] as const;
    const types = ['data', 'passkey', 'bundler'] as const;
    setHealths(Object.fromEntries(keys.map(k => [k, { status: 'checking' as const }])));
    keys.forEach((key, i) => {
      checkServiceEndpointHealth(endpoints[key], types[i]).then(h => {
        setHealths(prev => ({ ...prev, [key]: h }));
      });
    });
  }, [visible, refreshCount]);

  const handleSave = useCallback(async (field: keyof ServiceEndpoints, value: string) => {
    const clean = value.trim().replace(/[\r\n]/g, '');
    const updated = { ...endpoints, [field]: clean };
    setEndpoints(updated);
    await saveServiceEndpoints(updated);
    setRefreshCount(c => c + 1);
  }, [endpoints]);

  const fields: { key: keyof ServiceEndpoints; label: string; hint: string; healthType: 'data' | 'passkey' | 'bundler' }[] = [
    { key: 'ethereumDataURL', label: 'CHAIN DATA INDEX', hint: 'Provides network info, token data, and chain logos', healthType: 'data' },
    { key: 'passkeyIndexURL', label: 'PASSKEY INDEX', hint: 'Stores public keys for cross-device recovery', healthType: 'passkey' },
    { key: 'bundlerServiceURL', label: 'BUNDLER SERVICE', hint: 'Vela Bundler compatible endpoint required', healthType: 'bundler' },
  ];

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <Text style={s.modalTitle}>Service Endpoints</Text>
          <View style={s.modalHeaderRight}>
            <Pressable onPress={() => Linking.openURL('https://github.com/atshelchin/vela-wallet-mobile#self-deploy-service-endpoints')} hitSlop={8} style={s.refreshBtn}>
              <ExternalLink size={18} color={color.fg.muted} strokeWidth={2} />
            </Pressable>
            <Pressable onPress={() => setRefreshCount(c => c + 1)} hitSlop={8} style={s.refreshBtn}>
              <RefreshCw size={18} color={color.fg.muted} strokeWidth={2} />
            </Pressable>
            <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
          </View>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.epScrollContent} keyboardShouldPersistTaps="handled">
          <Text style={s.epDescription}>
            These services power your wallet.{'\n'}You can deploy your own instances for full self-custody.
          </Text>
          {fields.map(({ key, label, hint }) => (
            <VelaCard key={key} style={s.epCard}>
              <View style={s.epCardHeader}>
                <View style={s.epCardHeaderLeft}>
                  <Text style={s.epCardLabel}>{label}</Text>
                  <Text style={s.epCardHint}>{hint}</Text>
                </View>
                <ServiceHealthBadge health={healths[key] ?? { status: 'checking' }} />
              </View>
              <View style={s.epCardDivider} />
              <TextInput
                style={s.endpointInput}
                value={endpoints[key]}
                onChangeText={(v) => setEndpoints({ ...endpoints, [key]: v })}
                onBlur={() => handleSave(key, endpoints[key])}
                autoCapitalize="none"
                autoCorrect={false}
                multiline
                scrollEnabled={false}
                placeholder={DEFAULT_SERVICE_ENDPOINTS[key]}
                placeholderTextColor={color.fg.subtle}
              />
            </VelaCard>
          ))}
          <Pressable style={s.resetEndpointsBtn} onPress={() => { setEndpoints({ ...DEFAULT_SERVICE_ENDPOINTS }); saveServiceEndpoints({ ...DEFAULT_SERVICE_ENDPOINTS }); setRefreshCount(c => c + 1); }}>
            <Text style={s.resetEndpointsText}>Reset to Defaults</Text>
          </Pressable>
        </ScrollView>
      </View>
    </AppModal>
  );
}


// ---------------------------------------------------------------------------
// Add Network Modal
// ---------------------------------------------------------------------------

const VELA_CHAIN_SETUP_URL = 'https://biubiu.tools/apps/vela-wallet-chain-setup';

function AddNetworkModal({ s, visible, onClose, onAdded }: { s: S; visible: boolean; onClose: () => void; onAdded: () => void }) {
  const [query, setQuery] = useState('');
  const [suggestions, setSuggestions] = useState<ChainSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [selectedChainId, setSelectedChainId] = useState<number | null>(null);
  const [loading, setLoading] = useState(false);
  const [chainInfo, setChainInfo] = useState<Awaited<ReturnType<typeof fetchChainInfo>> | null>(null);
  const [compatResult, setCompatResult] = useState<CompatibilityResult | null>(null);
  const [error, setError] = useState('');
  const [saving, setSaving] = useState(false);
  const [customRpc, setCustomRpc] = useState('');
  const searchTimer = React.useRef<ReturnType<typeof setTimeout> | null>(null);

  const reset = () => {
    setQuery(''); setSuggestions([]); setSelectedChainId(null);
    setChainInfo(null); setCompatResult(null); setError(''); setCustomRpc('');
  };

  // Debounced search
  const handleQueryChange = (text: string) => {
    setQuery(text);
    setSelectedChainId(null);
    setChainInfo(null);
    setCompatResult(null);
    setError('');

    if (searchTimer.current) clearTimeout(searchTimer.current);
    if (!text.trim()) { setSuggestions([]); return; }

    searchTimer.current = setTimeout(async () => {
      setSearching(true);
      try {
        const results = await searchChains(text);
        setSuggestions(results);
      } catch {} finally { setSearching(false); }
    }, 300);
  };

  // Select a chain from suggestions
  const handleSelect = async (chainId: number, keepCustomRpc = false) => {
    setSelectedChainId(chainId);
    setSuggestions([]);
    setLoading(true);
    setError('');
    setChainInfo(null);
    setCompatResult(null);
    if (!keepCustomRpc) setCustomRpc('');

    // Check if already exists
    const existing = DEFAULT_NETWORKS.find(n => n.chainId === chainId);
    const custom = await loadCustomNetworks();
    if (existing || custom.find(n => n.chainId === chainId)) {
      setError(`This network is already added`);
      setLoading(false);
      return;
    }

    try {
      const info = await fetchChainInfo(chainId);
      if (!info) { setError(`Chain ${chainId} not found`); setLoading(false); return; }
      setChainInfo(info);
      setQuery(info.name);

      const rpcs = [
        ...(customRpc.trim() ? [customRpc.trim()] : []),
        ...(info.rpcUrls.length > 0 ? info.rpcUrls : info.rpcUrl ? [info.rpcUrl] : []),
      ];
      if (rpcs.length > 0) {
        const compat = await checkNetworkCompatibility(rpcs, chainId);
        setCompatResult(compat);
      } else {
        setError('No RPC endpoint available for this network');
      }
    } catch (e: any) {
      setError(e.message ?? 'Check failed');
    } finally { setLoading(false); }
  };

  const handleAdd = async () => {
    if (!chainInfo || !compatResult?.compatible) return;
    setSaving(true);
    try {
      const network: CustomNetwork = {
        id: `custom-${chainInfo.chainId}`,
        displayName: chainInfo.name,
        chainId: chainInfo.chainId,
        iconLabel: chainInfo.nativeCurrency.symbol.slice(0, 4),
        iconColor: '#888888',
        iconBg: '#F0F0F0',
        logoURL: chainInfo.logoURL,
        isL2: false,
        rpcURL: compatResult.bestRpcUrl ?? chainInfo.rpcUrl, // Use the fastest RPC
        explorerURL: chainInfo.explorerUrl,
        bundlerURL: `https://bundler.getvela.app/${chainInfo.chainId}`,
        nativeSymbol: chainInfo.nativeCurrency.symbol,
        addedAt: new Date().toISOString(),
      };
      await saveCustomNetwork(network);
      await refreshCustomNetworks();
      onAdded();
      reset();
      onClose();
    } catch (e: any) { setError(e.message ?? 'Failed to save'); }
    finally { setSaving(false); }
  };

  return (
    <AppModal visible={visible} onClose={() => { reset(); onClose(); }}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <Text style={s.modalTitle}>Add Network</Text>
          <Pressable onPress={() => { reset(); onClose(); }} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.modalScrollContent} keyboardShouldPersistTaps="handled">
          <Text style={s.endpointDescription}>
            Search by network name, token symbol, or Chain ID.
          </Text>

          {/* Search input */}
          <View style={s.searchField}>
            <TextInput
              style={s.configInput}
              value={query}
              onChangeText={handleQueryChange}
              placeholder="e.g. Gnosis, ACE, 648..."
              placeholderTextColor={color.fg.subtle}
              autoFocus
              autoCapitalize="none"
              autoCorrect={false}
            />
          </View>

          {/* Suggestions dropdown */}
          {suggestions.length > 0 && !selectedChainId && (
            <VelaCard style={s.suggestionsCard}>
              {suggestions.map((item, idx) => (
                <Pressable
                  key={item.chainId}
                  style={[s.suggestionRow, idx < suggestions.length - 1 && s.suggestionRowBorder]}
                  onPress={() => handleSelect(item.chainId)}
                >
                  <View style={s.suggestionInfo}>
                    <Text style={s.suggestionName}>{item.name}</Text>
                    <Text style={s.suggestionMeta}>
                      Chain {item.chainId} · {item.nativeCurrencySymbol}
                    </Text>
                  </View>
                  <ChevronRight size={14} color={color.fg.subtle} />
                </Pressable>
              ))}
            </VelaCard>
          )}

          {searching && (
            <View style={s.loadingRow}>
              <ActivityIndicator size="small" color={color.accent.base} />
              <Text style={s.loadingText}>Searching...</Text>
            </View>
          )}

          {loading && (
            <View style={s.loadingRow}>
              <ActivityIndicator size="small" color={color.accent.base} />
              <Text style={s.loadingText}>Checking compatibility...</Text>
            </View>
          )}

          {error ? <Text style={s.addNetError}>{error}</Text> : null}

          {/* Chain info result */}
          {chainInfo && (
            <VelaCard style={s.addNetResult}>
              <Text style={s.addNetResultName}>{chainInfo.name}</Text>
              <Text style={s.addNetResultDetail}>Chain ID: {chainInfo.chainId}</Text>
              <Text style={s.addNetResultDetail}>Native: {chainInfo.nativeCurrency.symbol}</Text>
              {chainInfo.isTestnet && <Text style={s.addNetTestnet}>Testnet</Text>}
            </VelaCard>
          )}

          {/* Custom RPC input */}
          {chainInfo && (
            <VelaCard style={s.addNetCompat}>
              <Text style={s.addNetCompatTitle}>Custom RPC (optional)</Text>
              <TextInput
                style={s.configInput}
                value={customRpc}
                onChangeText={setCustomRpc}
                placeholder="https://your-rpc-url..."
                placeholderTextColor={color.fg.subtle}
                autoCapitalize="none"
                autoCorrect={false}
              />
              {customRpc.trim() !== '' && (
                <VelaButton
                  title="Re-check with this RPC"
                  onPress={() => selectedChainId && handleSelect(selectedChainId, true)}
                  variant="secondary"
                  style={{ marginTop: space.sm }}
                />
              )}
            </VelaCard>
          )}

          {/* Best RPC info */}
          {compatResult?.bestRpcUrl && (
            <VelaCard style={s.addNetCompat}>
              <View style={s.addNetCompatRow}>
                <CheckCircle2 size={16} color={color.success.base} strokeWidth={2} />
                <Text style={s.addNetCompatText}>Best RPC: {compatResult.bestRpcLatency}ms</Text>
              </View>
              <Text style={s.addNetCompatDetail} numberOfLines={1}>{compatResult.bestRpcUrl}</Text>
            </VelaCard>
          )}

          {/* Per-contract status + P256 */}
          {compatResult && !compatResult.rpcFailed && (
            <VelaCard style={s.addNetCompat}>
              <Text style={s.addNetCompatTitle}>Compatibility Check</Text>

              {/* P256 precompile — shown first */}
              <View style={s.contractRow}>
                <View style={s.contractStatusRow}>
                  {compatResult.p256Available
                    ? <CheckCircle2 size={14} color={color.success.base} strokeWidth={2} />
                    : <XCircle size={14} color={color.accent.base} strokeWidth={2} />}
                  <Text style={[s.addNetCompatText, !compatResult.p256Available && s.addNetCompatMissing]}>P256 Precompile (RIP-7212)</Text>
                </View>
              </View>

              {/* Contracts */}
              {compatResult.contracts.map((c) => (
                <View key={c.address} style={s.contractRow}>
                  <View style={s.contractStatusRow}>
                    {c.deployed
                      ? <CheckCircle2 size={14} color={color.success.base} strokeWidth={2} />
                      : <XCircle size={14} color={color.accent.base} strokeWidth={2} />}
                    <Text style={[s.addNetCompatText, !c.deployed && s.addNetCompatMissing]}>{c.name}</Text>
                  </View>
                </View>
              ))}
            </VelaCard>
          )}

          {/* RPC failure — inconclusive, allow retry */}
          {compatResult?.rpcFailed && (
            <VelaCard style={s.addNetCompat}>
              <View style={s.addNetCompatRow}>
                <AlertTriangle size={16} color={color.warning.base} strokeWidth={2} />
                <Text style={s.addNetCompatText}>Unable to verify — RPC request failed</Text>
              </View>
              <Text style={s.addNetCompatError}>{compatResult.error}</Text>
              <VelaButton
                title="Retry"
                onPress={() => selectedChainId && handleSelect(selectedChainId)}
                variant="secondary"
                style={{ marginTop: space.md }}
              />
            </VelaCard>
          )}

          {compatResult?.compatible && (
            <VelaButton title="Add Network" onPress={handleAdd} variant="accent" loading={saving} style={s.checkBtn} />
          )}
          {compatResult && !compatResult.compatible && !compatResult.rpcFailed && (
            <View>
              <Text style={s.addNetHint}>
                Some required contracts are not yet deployed on this chain.{'\n'}
                Use the Vela Wallet Chain Setup tool to deploy them, then come back and re-check.
              </Text>
              <VelaButton
                title="Open Chain Setup Tool"
                onPress={() => Linking.openURL(VELA_CHAIN_SETUP_URL)}
                variant="accent"
                style={s.checkBtn}
              />
              <VelaButton
                title="Re-check"
                onPress={() => selectedChainId && handleSelect(selectedChainId, true)}
                variant="secondary"
                style={s.checkBtn}
              />
            </View>
          )}
        </ScrollView>
      </View>
    </AppModal>
  );
}

// ---------------------------------------------------------------------------
// Theme Picker — segmented control for auto / light / dark
// ---------------------------------------------------------------------------

const THEME_OPTIONS: { key: ColorSchemePreference; label: string; Icon: React.ComponentType<{ size: number; color: string; strokeWidth?: number }> }[] = [
  { key: 'light', label: 'Light', Icon: Sun },
  { key: 'dark', label: 'Dark', Icon: Moon },
  { key: 'auto', label: 'Auto', Icon: Monitor },
];

function ThemePicker({ s, current, onChange }: {
  s: S; current: ColorSchemePreference; onChange: (pref: ColorSchemePreference) => void;
}) {
  return (
    <View style={s.themePickerContainer}>
      {THEME_OPTIONS.map(({ key, label, Icon }) => {
        const active = current === key;
        return (
          <Pressable
            key={key}
            style={[s.themeOption, active && s.themeOptionActive]}
            onPress={() => {
              if (key !== current) {
                Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
                onChange(key);
              }
            }}
          >
            <Icon size={18} color={active ? color.accent.base : color.fg.subtle} strokeWidth={2} />
            <Text style={[s.themeOptionLabel, active && s.themeOptionLabelActive]}>{label}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Text Scale Slider — drag to adjust, snaps to levels, haptic on each snap
// ---------------------------------------------------------------------------

const SLIDER_STEP_COUNT = TEXT_SCALE_LEVELS.length - 1;

function TextScaleSlider({ s, currentIndex, onChangeIndex }: {
  s: S; currentIndex: number; onChangeIndex: (index: number) => void;
}) {
  const THUMB_SIZE = 28;

  const trackWidth = useSharedValue(0);
  const thumbX = useSharedValue(0);
  const startX = useSharedValue(0);
  const lastSnappedIndex = useSharedValue(currentIndex);
  const isDragging = useSharedValue(false);

  // Sync thumb position when currentIndex changes externally
  useEffect(() => {
    if (!isDragging.value && trackWidth.value > 0) {
      thumbX.value = withSpring(
        (currentIndex / SLIDER_STEP_COUNT) * trackWidth.value,
        { damping: 20, stiffness: 200 },
      );
      lastSnappedIndex.value = currentIndex;
    }
  }, [currentIndex, isDragging, thumbX, trackWidth, lastSnappedIndex]);

  const snapAndApply = useCallback((index: number) => {
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
    onChangeIndex(index);
  }, [onChangeIndex]);

  const pan = Gesture.Pan()
    .onStart(() => {
      isDragging.value = true;
      startX.value = thumbX.value;
    })
    .onUpdate((e) => {
      const w = trackWidth.value;
      if (w <= 0) return;
      const raw = Math.max(0, Math.min(w, startX.value + e.translationX));
      thumbX.value = raw;
      const nearestIndex = Math.round((raw / w) * SLIDER_STEP_COUNT);
      if (nearestIndex !== lastSnappedIndex.value) {
        lastSnappedIndex.value = nearestIndex;
        runOnJS(snapAndApply)(nearestIndex);
      }
    })
    .onEnd(() => {
      isDragging.value = false;
      const w = trackWidth.value;
      if (w <= 0) return;
      thumbX.value = withSpring(
        (lastSnappedIndex.value / SLIDER_STEP_COUNT) * w,
        { damping: 20, stiffness: 200 },
      );
    });

  const thumbStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: thumbX.value - THUMB_SIZE / 2 }],
  }));

  const fillStyle = useAnimatedStyle(() => ({
    width: thumbX.value,
  }));

  return (
    <View style={s.sliderContainer}>
      <Text style={s.sliderLabelSmall}>A</Text>
      <View
        style={s.sliderTrackOuter}
        onLayout={(e) => {
          const w = e.nativeEvent.layout.width;
          trackWidth.value = w;
          thumbX.value = (currentIndex / SLIDER_STEP_COUNT) * w;
        }}
      >
        <View style={s.sliderTrack} />
        <Animated.View style={[s.sliderFill, fillStyle]} />
        <View style={s.sliderTicks}>
          {TEXT_SCALE_LEVELS.map((_, i) => (
            <View key={i} style={[s.sliderTickDot, i <= currentIndex && s.sliderTickDotActive]} />
          ))}
        </View>
        <GestureDetector gesture={pan}>
          <Animated.View style={[s.sliderThumb, thumbStyle]} hitSlop={12} />
        </GestureDetector>
      </View>
      <Text style={s.sliderLabelLarge}>A</Text>
    </View>
  );
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

export default function SettingsScreen() {
  const styles = useStyles(styleFactory);
  const { state, dispatch, activeAccount } = useWallet();
  const router = useRouter();
  const [showAccountSwitcher, setShowAccountSwitcher] = useState(false);
  const [showNetworkEditor, setShowNetworkEditor] = useState(false);
  const [showEndpointEditor, setShowEndpointEditor] = useState(false);
  const [showAddNetwork, setShowAddNetwork] = useState(false);
  const [showAdvanced, setShowAdvanced] = useState(false);
  const { levelIndex: currentScaleIndex, setIndex: setScaleIndex } = useTextScale();
  const { preference: colorPref, setPreference: setColorPref } = useColorSchemePreference();

  const accountName = activeAccount?.name ?? 'No Wallet';
  const address = activeAccount?.address ?? state.address;

  const handleLogout = () => {
    Alert.alert('Logout', 'Are you sure you want to logout? This will clear all local data.', [
      { text: 'Cancel', style: 'cancel' },
      { text: 'Logout', style: 'destructive', onPress: async () => { await clearAll(); dispatch({ type: 'LOGOUT' }); router.replace('/'); } },
    ]);
  };

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        <Animated.View entering={fadeIn(0, 300)}>
          <Text style={styles.screenTitle}>Settings</Text>
        </Animated.View>

        {/* Account */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(50, 300)}>
          <Text style={styles.sectionTitle}>ACCOUNT</Text>
          <VelaCard>
            <SettingsRow s={styles} icon={{ bg: color.accent.soft, fg: color.accent.base, Icon: UserIcon }}
              title={accountName} subtitle={address ? shortAddress(address) : 'Switch account'}
              showDivider={false} onPress={() => setShowAccountSwitcher(true)} />
          </VelaCard>
        </Animated.View>

        {/* General */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(100, 300)}>
          <Text style={styles.sectionTitle}>GENERAL</Text>
          <VelaCard>
            <TextScaleSlider
              s={styles}
              currentIndex={currentScaleIndex}
              onChangeIndex={setScaleIndex}
            />
            <View style={styles.settingsRowDividerFull} />
            <ThemePicker s={styles} current={colorPref} onChange={setColorPref} />
            <View style={styles.settingsRowDividerFull} />
            <SettingsRow s={styles} icon={{ bg: color.bg.sunken, fg: color.fg.muted, Icon: InfoIcon }}
              title="About" subtitle="Vela Wallet v1.0.0" showDivider={false} onPress={() => router.push('/about')} />
          </VelaCard>
        </Animated.View>

        {/* Advanced */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(150, 300)}>
          <Pressable style={styles.advancedHeader} onPress={() => setShowAdvanced(!showAdvanced)}>
            <Text style={styles.sectionTitle}>ADVANCED</Text>
            <ChevronDown size={14} color={color.fg.subtle} style={showAdvanced ? { transform: [{ rotate: '180deg' }] } : undefined} />
          </Pressable>
          {showAdvanced && (
            <VelaCard>
              <SettingsRow s={styles} icon={{ bg: color.info.soft, fg: color.info.base, Icon: NetworkIcon }}
                title="Networks" subtitle="RPC, Explorer & Bundler URLs"
                showDivider={true} onPress={() => setShowNetworkEditor(true)} />
              <SettingsRow s={styles} icon={{ bg: color.success.soft, fg: color.success.base, Icon: Plus }}
                title="Add Network" subtitle="Add custom EVM network"
                showDivider={true} onPress={() => setShowAddNetwork(true)} />
              <SettingsRow s={styles} icon={{ bg: color.success.soft, fg: color.success.base, Icon: Server }}
                title="Service Endpoints" subtitle="Chain data, identity index, Bundler"
                showDivider={false} onPress={() => setShowEndpointEditor(true)} />
            </VelaCard>
          )}
        </Animated.View>

        {/* Logout */}
        <Animated.View entering={fadeInDown(200, 300)}>
          <Pressable style={styles.logoutButton} onPress={handleLogout}>
            <LogOutIcon size={16} color={color.accent.base} />
            <Text style={styles.logoutText}>Logout</Text>
          </Pressable>
        </Animated.View>
      </ScrollView>

      <AccountSwitcherModal s={styles} visible={showAccountSwitcher} onClose={() => setShowAccountSwitcher(false)} />
      <NetworkEditorModal s={styles} visible={showNetworkEditor} onClose={() => setShowNetworkEditor(false)} />
      <EndpointEditorModal s={styles} visible={showEndpointEditor} onClose={() => setShowEndpointEditor(false)} />
      <AddNetworkModal s={styles} visible={showAddNetwork} onClose={() => setShowAddNetwork(false)} onAdded={() => {}} />
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styleFactory = () => ({
  scrollContent: { paddingTop: space.md, paddingBottom: space['5xl'] },
  screenTitle: { fontSize: text['2xl'], ...inter.bold, color: color.fg.base, marginBottom: space['3xl'] },
  sectionContainer: { marginBottom: space['2xl'] },
  sectionTitle: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle, letterSpacing: 1.2, textTransform: 'uppercase' as const, marginBottom: space.md, paddingHorizontal: space.sm },
  advancedHeader: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const, paddingRight: space.md, marginBottom: space.md },

  // Settings Row
  settingsRow: { flexDirection: 'row' as const, alignItems: 'center' as const, paddingHorizontal: space.xl, paddingVertical: space.xl, position: 'relative' as const },
  settingsIcon: { width: 34, height: 34, borderRadius: 10, alignItems: 'center' as const, justifyContent: 'center' as const },
  settingsRowContent: { flex: 1, marginLeft: space.lg, gap: 2 },
  settingsRowTitle: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  settingsRowSubtitle: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },
  settingsRowDivider: { position: 'absolute' as const, bottom: 0, left: 66, right: 0, height: 1, backgroundColor: color.border.base },
  settingsRowDividerFull: { height: 1, backgroundColor: color.border.base, marginHorizontal: space.xl },

  // Logout
  logoutButton: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'center' as const, paddingVertical: space.xl, backgroundColor: color.bg.raised, borderRadius: radius.xl, borderWidth: 1, borderColor: color.border.base, gap: space.md, ...shadow.sm },
  logoutText: { fontSize: text.lg, ...inter.semibold, color: color.accent.base },

  // Text Scale
  // Theme Picker
  themePickerContainer: { flexDirection: 'row' as const, paddingVertical: space.xl, paddingHorizontal: space.xl, gap: space.md },
  themeOption: { flex: 1, flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'center' as const, gap: space.sm, paddingVertical: space.lg, borderRadius: radius.lg, backgroundColor: color.bg.sunken },
  themeOptionActive: { backgroundColor: color.accent.soft, borderWidth: 1.5, borderColor: color.accent.base },
  themeOptionLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },
  themeOptionLabelActive: { color: color.accent.base, ...inter.semibold },

  // Slider
  sliderContainer: { flexDirection: 'row' as const, alignItems: 'center' as const, paddingVertical: space['2xl'], paddingHorizontal: space.xl, gap: space.lg },
  sliderLabelSmall: { fontSize: text.sm, ...inter.semibold, color: color.fg.subtle },
  sliderLabelLarge: { fontSize: text.xl, ...inter.semibold, color: color.fg.subtle },
  sliderTrackOuter: { flex: 1, height: 36, justifyContent: 'center' as const },
  sliderTrack: { position: 'absolute' as const, left: 0, right: 0, height: 4, borderRadius: 2, backgroundColor: color.border.base },
  sliderFill: { position: 'absolute' as const, left: 0, height: 4, borderRadius: 2, backgroundColor: color.accent.base },
  sliderTicks: { position: 'absolute' as const, left: 0, right: 0, flexDirection: 'row' as const, justifyContent: 'space-between' as const, alignItems: 'center' as const },
  sliderTickDot: { width: 8, height: 8, borderRadius: 4, backgroundColor: color.border.strong },
  sliderTickDotActive: { backgroundColor: color.accent.base },
  sliderThumb: { position: 'absolute' as const, top: 4, width: 28, height: 28, borderRadius: 14, backgroundColor: color.bg.raised, borderWidth: 2, borderColor: color.accent.base, ...shadow.md },

  // Modal shared
  modalContainer: { flex: 1, backgroundColor: color.bg.base },
  modalHeader: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const, paddingHorizontal: space['3xl'], paddingVertical: space.xl, borderBottomWidth: 1, borderBottomColor: color.border.base },
  modalHeaderRight: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.lg },
  modalTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  modalScroll: { flex: 1 },
  modalScrollContent: { padding: space['3xl'], paddingBottom: space['5xl'] },

  // Account Switcher
  accountItem: { flexDirection: 'row' as const, alignItems: 'center' as const, padding: space.xl, backgroundColor: color.bg.raised, borderRadius: radius.xl, borderWidth: 1, borderColor: color.border.base, marginBottom: space.lg, gap: space.lg, ...shadow.sm },
  accountItemActive: { borderColor: color.accent.base, borderWidth: 1.5 },
  accountAvatar: { width: 40, height: 40, borderRadius: 20, backgroundColor: color.accent.soft, alignItems: 'center' as const, justifyContent: 'center' as const },
  accountAvatarText: { fontSize: text.lg, ...inter.semibold, color: color.accent.base },
  accountInfo: { flex: 1, gap: 2 },
  accountNameModal: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  accountAddress: { fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.subtle },
  accountTotalLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle, marginTop: 2 },
  accountRight: { marginLeft: 'auto' as const, alignItems: 'flex-end' as const, gap: 4 },
  accountBal: { fontSize: text.sm, ...inter.bold, color: color.fg.base },
  accountActions: { marginTop: space.xl, gap: space.lg },

  // Network Editor
  networkScrollContent: { padding: space.xl, paddingBottom: space['5xl'], gap: space.lg },
  networkCard: { overflow: 'hidden' as const },
  networkHeader: { flexDirection: 'row' as const, alignItems: 'center' as const, padding: space.xl, gap: space.lg },
  networkHeaderText: { flex: 1, gap: 2 },
  networkName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  networkChainId: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },
  networkFields: { paddingHorizontal: space.xl, paddingBottom: space.xl, gap: space.lg },
  dividerFull: { height: 1, backgroundColor: color.border.base, marginHorizontal: -space.xl, marginBottom: space.sm },
  deleteNetBtn: { padding: space.sm, marginRight: space.sm },
  configField: { gap: space.sm, marginBottom: space.lg },
  configLabelRow: { flexDirection: 'row' as const, alignItems: 'center' as const },
  configLabel: { fontSize: text.xs, ...inter.semibold, color: color.fg.subtle, letterSpacing: 1, textTransform: 'uppercase' as const },
  configInput: { fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.base, padding: space.lg, backgroundColor: color.bg.sunken, borderRadius: radius.lg, borderWidth: 1, borderColor: color.border.base },

  // Endpoint Editor
  epScrollContent: { padding: space.xl, paddingBottom: space['5xl'] },
  epDescription: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 20, marginBottom: space.xl, paddingHorizontal: space.sm },
  endpointDescription: { fontSize: text.base, ...inter.regular, color: color.fg.muted, lineHeight: 22, marginBottom: space['2xl'] },
  epCard: { marginBottom: space.lg, padding: space.xl },
  epCardHeader: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const, marginBottom: space.md },
  epCardHeaderLeft: { flex: 1, gap: 2 },
  epCardLabel: { fontSize: text.sm, ...inter.bold, color: color.fg.base, letterSpacing: 0.5, textTransform: 'uppercase' as const },
  epCardHint: { fontSize: text.xs, ...inter.regular, color: color.fg.subtle },
  epCardDivider: { height: 1, backgroundColor: color.border.base, marginBottom: space.lg },
  endpointInput: { fontSize: text.sm, fontWeight: '500' as const, fontFamily: font.mono, color: color.fg.base, padding: space.lg, backgroundColor: color.bg.sunken, borderRadius: radius.lg, borderWidth: 1, borderColor: color.border.base, minHeight: 56, textAlignVertical: 'top' as const },
  resetEndpointsBtn: { alignItems: 'center' as const, paddingVertical: space.xl, marginTop: space.sm },
  resetEndpointsText: { fontSize: text.base, ...inter.semibold, color: color.accent.base },
  refreshBtn: { padding: space.sm },
  loadingRow: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'center' as const, gap: space.md, paddingVertical: space['3xl'] },
  loadingText: { fontSize: text.base, ...inter.regular, color: color.fg.muted },

  // Add Network — search
  searchField: { marginBottom: space.md },
  suggestionsCard: { marginBottom: space.lg, paddingVertical: space.sm },
  suggestionRow: { flexDirection: 'row' as const, alignItems: 'center' as const, paddingVertical: space.lg, paddingHorizontal: space.xl },
  suggestionRowBorder: { borderBottomWidth: 1, borderBottomColor: color.border.base },
  suggestionInfo: { flex: 1, gap: 2 },
  suggestionName: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  suggestionMeta: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },

  // Add Network — results
  checkBtn: { marginTop: space.lg, marginBottom: space.lg },
  addNetError: { fontSize: text.sm, ...inter.medium, color: color.accent.base, marginTop: space.md },
  addNetResult: { padding: space['2xl'], gap: space.sm, marginBottom: space.lg },
  addNetResultName: { fontSize: text.lg, ...inter.bold, color: color.fg.base },
  addNetResultDetail: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },
  addNetTestnet: { fontSize: text.xs, ...inter.semibold, color: color.warning.base, backgroundColor: color.warning.soft, paddingHorizontal: space.md, paddingVertical: 2, borderRadius: radius.sm, alignSelf: 'flex-start' as const },
  addNetCompat: { padding: space.xl, gap: space.md, marginBottom: space.lg },
  addNetCompatTitle: { fontSize: text.sm, ...inter.bold, color: color.fg.base, marginBottom: space.sm },
  contractRow: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const, paddingVertical: 4 },
  contractStatusRow: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.md, flex: 1 },
  deployBtn: { backgroundColor: color.accent.soft, paddingHorizontal: space.lg, paddingVertical: space.sm, borderRadius: radius.md },
  deployBtnText: { fontSize: text.xs, ...inter.semibold, color: color.accent.base },
  addNetCompatRow: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.md, paddingVertical: 3 },
  addNetCompatText: { fontSize: text.sm, ...inter.medium, color: color.fg.base },
  addNetCompatMissing: { color: color.accent.base },
  addNetCompatDetail: { fontSize: text.xs, ...inter.regular, color: color.fg.subtle, marginTop: 2, marginLeft: 30 },
  addNetCompatError: { fontSize: text.sm, ...inter.regular, color: color.accent.base, marginTop: space.sm },
  addNetHint: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, textAlign: 'center' as const, lineHeight: 20 },

});
