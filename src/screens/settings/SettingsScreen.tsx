import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { AppModal } from '@/components/ui/AppModal';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { useColorSchemePreference, type ColorSchemePreference } from '@/constants/color-scheme';
import { fadeIn, fadeInDown } from '@/constants/entering';
import { TEXT_SCALE_LEVELS, useTextScale } from '@/constants/text-scale';
import { color, font, inter, radius, shadow, space, text, useStyles } from '@/constants/theme';
import type { Network } from '@/models/network';
import { DEFAULT_NETWORKS, getAllNetworks, getAllNetworksSync, refreshCustomNetworks } from '@/models/network';
import type { CompatibilityResult, CustomNetwork, NetworkConfig, ServiceEndpoints, LocalePrefs } from '@/models/types';
import { DEFAULT_SERVICE_ENDPOINTS, isNativeToken, tokenChainId } from '@/models/types';
import { numberFormatOptions, dateFormatOptions, timeFormatOptions, type FormatOption } from '@/services/locale-format';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { shortAddress, useWallet } from '@/models/wallet-state';
import { getAccountBalances } from '@/services/balance-cache';
import { clearBundlerCache } from '@/services/bundler-service';
import { fetchWithTimeout, NET_TIMEOUTS } from '@/services/net';
import { fetchChainInfo, searchChains, type ChainSearchResult } from '@/services/chain-registry';
import { checkNetworkCompatibility } from '@/services/network-checker';
import { copyToClipboard, hapticLight, hapticSuccess, openURL, showAlert } from '@/services/platform';
import { getBuiltinBundlerUrl, invalidateAllPools, poolRpcCall, refreshPool } from '@/services/rpc-pool';
import { isTempoChain, TEMPO_DEFAULT_FEE_TOKEN } from '@/services/tempo';
import { getBundlerServiceURL, getLocalePrefs, hasPendingUploads, loadCustomNetworks, loadLocalePrefs, loadNetworkConfigs, loadServiceEndpoints, removeCustomNetwork, saveCustomNetwork, saveLocalePrefs, saveNetworkConfig, saveServiceEndpoints } from '@/services/storage';
import { fetchTokens } from '@/services/wallet-api';
import { RpcProvidersModal } from './RpcProvidersModal';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useRouter } from 'expo-router';
import { AlertTriangle, BookUser, Calendar, Check, CheckCircle2, ChevronDown, ChevronRight, Clock, Copy, ExternalLink, Hash, Info as InfoIcon, Key, Languages, LogOut as LogOutIcon, MessageSquare, Monitor, Moon, Globe as NetworkIcon, Plus, RefreshCw, Server, Sun, Trash2, User as UserIcon, X, XCircle, Zap } from 'lucide-react-native';
import { ContactsManager } from '@/components/contacts/ContactsManager';
import { BugReportModal } from '@/components/ui/BugReportModal';
import { AutoGrowTextInput } from '@/components/ui/AutoGrowTextInput';
import { useTranslation } from 'react-i18next';
import { useLanguagePreference } from '@/i18n/language';
import { LANGUAGE_NATIVE_NAMES, SUPPORTED_LANGUAGES, type AppLanguage, type LanguagePreference } from '@/i18n';
import React, { useCallback, useEffect, useState } from 'react';
import {
    ActivityIndicator,
    Pressable,
    ScrollView,
    Text,
    TextInput,
    View,
} from 'react-native';
import { Gesture, GestureDetector } from 'react-native-gesture-handler';
import Animated, {
    runOnJS,
    useAnimatedStyle,
    useSharedValue,
    withSpring,
} from 'react-native-reanimated';

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
      // Explorer: it's a website, not a JSON API — it sends no CORS headers (and
      // usually sits behind Cloudflare), so on web a normal fetch is blocked and
      // every explorer falsely reads "offline". Use no-cors: the opaque response
      // can't be inspected, but the request still goes out, so "resolved without
      // throwing" == host reachable. That's the only honest liveness signal we can
      // get for a cross-origin site, and it matches the bundler check above.
      await fetch(url, { method: 'GET', mode: 'no-cors', signal: controller.signal, redirect: 'follow' });
      clearTimeout(timeout);
      return { status: 'ok', latencyMs: Date.now() - start };
    }
  } catch {
    clearTimeout(timeout);
    return { status: 'error' };
  }
}

function HealthBadge({ health }: { health: EndpointHealth }) {
  const { t } = useTranslation();
  if (health.status === 'checking') {
    return <ActivityIndicator size={10} color={color.fg.subtle} style={{ marginLeft: 6 }} />;
  }
  const dotColor = health.status === 'ok' ? color.success.base : color.accent.base;
  const label = health.status === 'ok'
    ? `${health.latencyMs}ms`
    : t('settingsModals.health.offline');
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
  const { t } = useTranslation();
  const [expanded, setExpanded] = useState(false);
  const [rpcURL, setRpcURL] = useState(savedConfig?.rpcURL ?? network.rpcURL);
  const [explorerURL, setExplorerURL] = useState(savedConfig?.explorerURL ?? network.explorerURL);
  const [healths, setHealths] = useState<[EndpointHealth, EndpointHealth]>([
    { status: 'checking' }, { status: 'checking' },
  ]);

  // Re-seed the inputs whenever the saved config arrives or changes. The parent
  // loads savedConfig asynchronously, so on the first render it can still be
  // undefined — and the fallback (the built-in default) now differs from a saved
  // URL only by its query string. Without this sync a saved
  // "…publicnode.com/?apikey=X" rendered as the bare default, so the user's key
  // appeared to vanish on every reload (localStorage still held it).
  useEffect(() => {
    setRpcURL(savedConfig?.rpcURL ?? network.rpcURL);
    setExplorerURL(savedConfig?.explorerURL ?? network.explorerURL);
  }, [savedConfig?.rpcURL, savedConfig?.explorerURL, network.rpcURL, network.explorerURL]);

  // The bundler isn't editable per-network: the one configured in Service
  // Endpoints applies to every chain (the pool appends `/<chainId>`). Preserve
  // whatever was already saved so we never clobber a custom network's bundler.
  const handleSave = useCallback(() => {
    onSave({
      chainId: network.chainId,
      rpcURL,
      explorerURL,
      bundlerURL: savedConfig?.bundlerURL ?? network.bundlerURL,
    });
  }, [network.chainId, network.bundlerURL, rpcURL, explorerURL, savedConfig?.bundlerURL, onSave]);

  // Run health checks when expanded
  useEffect(() => {
    if (!expanded) return;
    setHealths([{ status: 'checking' }, { status: 'checking' }]);
    const fields: [string, 'rpc' | 'explorer'][] = [[rpcURL, 'rpc'], [explorerURL, 'explorer']];
    fields.forEach(([url, type], i) => {
      checkEndpointHealth(url, type).then(h => {
        setHealths(prev => { const next = [...prev] as typeof prev; next[i] = h; return next; });
      });
    });
  }, [expanded, rpcURL, explorerURL]);

  return (
    <VelaCard style={s.networkCard}>
      <Pressable style={s.networkHeader} onPress={() => setExpanded(!expanded)}>
        <ChainLogo label={network.iconLabel} color={network.iconColor} bgColor={network.iconBg} logoURL={network.logoURL} size={36} />
        <View style={s.networkHeaderText}>
          <Text style={s.networkName}>{network.displayName}</Text>
          <Text style={s.networkChainId}>{t('settingsModals.network.chainId', { chainId: network.chainId })}</Text>
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
          {([
            ['settingsModals.network.fieldRpcUrl', rpcURL, setRpcURL],
            ['settingsModals.network.fieldExplorer', explorerURL, setExplorerURL],
          ] as const).map(([labelKey, val, setter], i) => {
            const label = t(labelKey);
            return (
              <View key={labelKey} style={s.configField}>
                <View style={s.configLabelRow}>
                  <Text style={s.configLabel}>{label}</Text>
                  <HealthBadge health={healths[i]} />
                </View>
                <TextInput style={s.configInput} value={val} onChangeText={setter} onBlur={handleSave}
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

function AccountSwitcherModal({ s, visible, onClose }: { s: S; visible: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const { state, dispatch } = useWallet();
  const router = useRouter();
  const dc = useDisplayCurrency();
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
            <Text style={s.modalTitle}>{t('settingsModals.account.modalTitle')}</Text>
            {cachedBalances.size > 0 && (
              <Text style={s.accountTotalLabel}>{t('settingsModals.account.total', { amount: dc.fmt(allTotal) })}</Text>
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
                onPress={() => { dispatch({ type: 'SWITCH_ACCOUNT', index }); hapticSuccess(); onClose(); }}>
                <View style={s.accountAvatar}>
                  <Text style={s.accountAvatarText}>{(account.name[0] ?? 'V').toUpperCase()}</Text>
                </View>
                <View style={s.accountInfo}>
                  <Text style={s.accountNameModal}>{account.name}</Text>
                  <Text style={s.accountAddress}>{shortAddress(account.address)}</Text>
                </View>
                <View style={s.accountRight}>
                  {bal != null && <Text style={s.accountBal}>{dc.fmt(bal)}</Text>}
                  {isActive && <Check size={18} color={color.accent.base} />}
                </View>
              </Pressable>
            );
          })}
          <View style={s.accountActions}>
            <VelaButton title={t('settingsModals.account.createNew')} onPress={() => { onClose(); router.push('/onboarding'); }} />
            <VelaButton title={t('settingsModals.account.signInExisting')} variant="secondary" onPress={() => { onClose(); router.push('/onboarding'); }} />
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
  const { t } = useTranslation();
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
    showAlert(t('settingsModals.network.removeTitle'), t('settingsModals.network.removeBody'), [
      { text: t('settingsModals.network.removeCancel'), style: 'cancel' },
      { text: t('settingsModals.network.removeConfirm'), style: 'destructive', onPress: async () => {
        await removeCustomNetwork(id);
        await refreshCustomNetworks();
        setAllNetworks(await getAllNetworks());
        setCustomIds(prev => { const next = new Set(prev); next.delete(id); return next; });
      }},
    ]);
  }, [t]);

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <Text style={s.modalTitle}>{t('settingsModals.network.modalTitle')}</Text>
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
  url: string, type: 'data' | 'passkey' | 'bundler' | 'fiat',
): Promise<ServiceHealth> {
  if (!url) return { status: 'unreachable', detail: 'Empty URL' };

  // 1. HTTPS check (allow http for localhost / 127.0.0.1 during development)
  const isLocalhost = /^http:\/\/(localhost|127\.0\.0\.1)(:\d+)?(\/|$)/.test(url);
  if (!url.startsWith('https://') && !isLocalhost) {
    return { status: 'not_https', detail: 'HTTPS required' };
  }

  // Fiat-rate provider: third-party (no /api/health) — GET the URL itself and
  // validate it returns a USD-based `{ rates: {...} }` map.
  if (type === 'fiat') {
    const start = Date.now();
    const controller = new AbortController();
    const timeout = setTimeout(() => controller.abort(), 10000);
    try {
      const res = await fetch(url.trim().replace(/[\r\n]/g, ''), { method: 'GET', signal: controller.signal });
      clearTimeout(timeout);
      const latencyMs = Date.now() - start;
      if (!res.ok) return { status: 'unreachable', latencyMs, detail: `HTTP ${res.status}` };
      const data = await res.json();
      // Accept Frankfurter v2's array shape or an object `{rates:{…}}` (open.er-api / v1).
      const n = Array.isArray(data)
        ? data.length
        : (data?.rates && typeof data.rates === 'object' ? Object.keys(data.rates).length : 0);
      if (!n) return { status: 'invalid_response', latencyMs, detail: 'No rates returned' };
      return { status: 'ok', latencyMs, detail: `${n} currencies` };
    } catch {
      clearTimeout(timeout);
      return { status: 'unreachable', detail: 'Connection failed' };
    }
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
  const { t } = useTranslation();
  if (health.status === 'checking') {
    return <ActivityIndicator size={10} color={color.fg.subtle} style={{ marginLeft: 6 }} />;
  }
  const cfg: Record<string, { dot: string; label: string }> = {
    ok: { dot: color.success.base, label: `${health.latencyMs ?? 0}ms` },
    not_https: { dot: color.accent.base, label: t('settingsModals.health.httpsRequired') },
    unreachable: { dot: color.accent.base, label: t('settingsModals.health.offline') },
    invalid_response: { dot: color.warning.base, label: health.detail ?? t('settingsModals.health.invalid') },
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
  const { t } = useTranslation();
  const [endpoints, setEndpoints] = useState<ServiceEndpoints>({ ...DEFAULT_SERVICE_ENDPOINTS });
  const [healths, setHealths] = useState<Record<string, ServiceHealth>>({});
  const [refreshCount, setRefreshCount] = useState(0);

  useEffect(() => { if (visible) loadServiceEndpoints().then(setEndpoints); }, [visible]);

  // Health checks on open and manual refresh
  useEffect(() => {
    if (!visible) return;
    const keys = ['ethereumDataURL', 'passkeyIndexURL', 'bundlerServiceURL', 'fiatRatesURL'] as const;
    const types = ['data', 'passkey', 'bundler', 'fiat'] as const;
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
    invalidateAllPools();
    setRefreshCount(c => c + 1);
  }, [endpoints]);

  const fields: { key: keyof ServiceEndpoints; labelKey: string; hintKey: string; healthType: 'data' | 'passkey' | 'bundler' | 'fiat' }[] = [
    { key: 'ethereumDataURL', labelKey: 'settingsModals.endpoints.chainDataLabel', hintKey: 'settingsModals.endpoints.chainDataHint', healthType: 'data' },
    { key: 'passkeyIndexURL', labelKey: 'settingsModals.endpoints.passkeyLabel', hintKey: 'settingsModals.endpoints.passkeyHint', healthType: 'passkey' },
    { key: 'bundlerServiceURL', labelKey: 'settingsModals.endpoints.bundlerLabel', hintKey: 'settingsModals.endpoints.bundlerHint', healthType: 'bundler' },
    { key: 'fiatRatesURL', labelKey: 'settingsModals.endpoints.fiatLabel', hintKey: 'settingsModals.endpoints.fiatHint', healthType: 'fiat' },
  ];

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <Text style={s.modalTitle}>{t('settingsModals.endpoints.modalTitle')}</Text>
          <View style={s.modalHeaderRight}>
            <Pressable onPress={() => openURL('https://github.com/atshelchin/vela-wallet#self-deploy-service-endpoints')} hitSlop={8} style={s.refreshBtn}>
              <ExternalLink size={18} color={color.fg.muted} strokeWidth={2} />
            </Pressable>
            <Pressable onPress={() => setRefreshCount(c => c + 1)} hitSlop={8} style={s.refreshBtn}>
              <RefreshCw size={18} color={color.fg.muted} strokeWidth={2} />
            </Pressable>
            <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
          </View>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.epScrollContent} keyboardShouldPersistTaps="handled">
          <Text style={s.epDescription}>{t('settingsModals.endpoints.description')}</Text>
          {fields.map(({ key, labelKey, hintKey }) => (
            <VelaCard key={key} style={s.epCard}>
              <View style={s.epCardHeader}>
                <View style={s.epCardHeaderLeft}>
                  <Text style={s.epCardLabel}>{t(labelKey, { defaultValue: labelKey })}</Text>
                  <Text style={s.epCardHint}>{t(hintKey, { defaultValue: hintKey })}</Text>
                </View>
                <ServiceHealthBadge health={healths[key] ?? { status: 'checking' }} />
              </View>
              <View style={s.epCardDivider} />
              <AutoGrowTextInput
                style={s.endpointInput}
                minHeight={56}
                value={endpoints[key]}
                onChangeText={(v) => setEndpoints({ ...endpoints, [key]: v })}
                onBlur={() => handleSave(key, endpoints[key])}
                autoCapitalize="none"
                autoCorrect={false}
                placeholder={DEFAULT_SERVICE_ENDPOINTS[key]}
                placeholderTextColor={color.fg.subtle}
              />
            </VelaCard>
          ))}
          <Pressable style={s.resetEndpointsBtn} onPress={() => { setEndpoints({ ...DEFAULT_SERVICE_ENDPOINTS }); saveServiceEndpoints({ ...DEFAULT_SERVICE_ENDPOINTS }); setRefreshCount(c => c + 1); }}>
            <Text style={s.resetEndpointsText}>{t('settingsModals.endpoints.resetToDefaults')}</Text>
          </Pressable>
        </ScrollView>
      </View>
    </AppModal>
  );
}


// ---------------------------------------------------------------------------
// Format Picker Modal (number / date / time) — pick by live example
// ---------------------------------------------------------------------------

function FormatPickerModal<K extends string>({ s, visible, title, subtitle, options, selected, onSelect, onClose }: {
  s: S; visible: boolean; title: string; subtitle: string;
  options: FormatOption<K>[]; selected: K; onSelect: (k: K) => void; onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <Text style={s.modalTitle}>{title}</Text>
          <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.epScrollContent}>
          <Text style={s.epDescription}>{subtitle}</Text>
          {options.map((o) => {
            const sel = o.key === selected;
            return (
              <Pressable key={o.key} style={[s.fmtRow, sel && s.fmtRowSel]} onPress={() => { onSelect(o.key); onClose(); }}>
                <View style={s.fmtRowInfo}>
                  <Text style={s.fmtExample}>{o.example}</Text>
                  {o.noteKey ? <Text style={s.fmtNote}>{t(`settings.formatNote.${o.noteKey}`)}</Text> : null}
                </View>
                {sel && <Check size={20} color={color.accent.base} strokeWidth={2.6} />}
              </Pressable>
            );
          })}
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
  const { t } = useTranslation();
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
        bundlerURL: `${getBundlerServiceURL()}/${chainInfo.chainId}`,
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
          <Text style={s.modalTitle}>{t('settingsModals.addNetwork.modalTitle')}</Text>
          <Pressable onPress={() => { reset(); onClose(); }} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.modalScrollContent} keyboardShouldPersistTaps="handled">
          <Text style={s.endpointDescription}>{t('settingsModals.addNetwork.description')}</Text>

          {/* Search input */}
          <View style={s.searchField}>
            <TextInput
              style={s.configInput}
              value={query}
              onChangeText={handleQueryChange}
              placeholder={t('settingsModals.addNetwork.searchPlaceholder')}
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
                      {t('settingsModals.addNetwork.chainMeta', { chainId: item.chainId, symbol: item.nativeCurrencySymbol })}
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
              <Text style={s.loadingText}>{t('settingsModals.addNetwork.searching')}</Text>
            </View>
          )}

          {loading && (
            <View style={s.loadingRow}>
              <ActivityIndicator size="small" color={color.accent.base} />
              <Text style={s.loadingText}>{t('settingsModals.addNetwork.checkingCompatibility')}</Text>
            </View>
          )}

          {error ? <Text style={s.addNetError}>{error}</Text> : null}

          {/* Chain info result */}
          {chainInfo && (
            <VelaCard style={s.addNetResult}>
              <Text style={s.addNetResultName}>{chainInfo.name}</Text>
              <Text style={s.addNetResultDetail}>{t('settingsModals.addNetwork.chainIdLabel', { chainId: chainInfo.chainId })}</Text>
              <Text style={s.addNetResultDetail}>{t('settingsModals.addNetwork.nativeLabel', { symbol: chainInfo.nativeCurrency.symbol })}</Text>
              {chainInfo.isTestnet && <Text style={s.addNetTestnet}>{t('settingsModals.addNetwork.testnet')}</Text>}
            </VelaCard>
          )}

          {/* Custom RPC input */}
          {chainInfo && (
            <VelaCard style={s.addNetCompat}>
              <Text style={s.addNetCompatTitle}>{t('settingsModals.addNetwork.customRpcTitle')}</Text>
              <TextInput
                style={s.configInput}
                value={customRpc}
                onChangeText={setCustomRpc}
                placeholder={t('settingsModals.addNetwork.customRpcPlaceholder')}
                placeholderTextColor={color.fg.subtle}
                autoCapitalize="none"
                autoCorrect={false}
              />
              {customRpc.trim() !== '' && (
                <VelaButton
                  title={t('settingsModals.addNetwork.recheckWithRpc')}
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
                <Text style={s.addNetCompatText}>{t('settingsModals.addNetwork.bestRpc', { latencyMs: compatResult.bestRpcLatency })}</Text>
              </View>
              <Text style={s.addNetCompatDetail} numberOfLines={1}>{compatResult.bestRpcUrl}</Text>
            </VelaCard>
          )}

          {/* Per-contract status + P256 */}
          {compatResult && !compatResult.rpcFailed && (
            <VelaCard style={s.addNetCompat}>
              <Text style={s.addNetCompatTitle}>{t('settingsModals.addNetwork.compatibilityCheck')}</Text>

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
                <Text style={s.addNetCompatText}>{t('settingsModals.addNetwork.unableToVerify')}</Text>
              </View>
              <Text style={s.addNetCompatError}>{compatResult.error}</Text>
              <VelaButton
                title={t('settingsModals.addNetwork.retry')}
                onPress={() => selectedChainId && handleSelect(selectedChainId)}
                variant="secondary"
                style={{ marginTop: space.md }}
              />
            </VelaCard>
          )}

          {compatResult?.compatible && (
            <VelaButton title={t('settingsModals.addNetwork.addNetworkBtn')} onPress={handleAdd} variant="accent" loading={saving} style={s.checkBtn} />
          )}
          {compatResult && !compatResult.compatible && !compatResult.rpcFailed && (
            <View>
              <Text style={s.addNetHint}>{t('settingsModals.addNetwork.incompatibleHint')}</Text>
              <VelaButton
                title={t('settingsModals.addNetwork.openChainSetupTool')}
                onPress={() => openURL(VELA_CHAIN_SETUP_URL)}
                variant="accent"
                style={s.checkBtn}
              />
              <VelaButton
                title={t('settingsModals.addNetwork.recheck')}
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

const THEME_OPTIONS: { key: ColorSchemePreference; labelKey: 'settings.appearance.themeLight' | 'settings.appearance.themeDark' | 'settings.appearance.themeAuto'; Icon: React.ComponentType<{ size: number; color: string; strokeWidth?: number }> }[] = [
  { key: 'light', labelKey: 'settings.appearance.themeLight', Icon: Sun },
  { key: 'dark', labelKey: 'settings.appearance.themeDark', Icon: Moon },
  { key: 'auto', labelKey: 'settings.appearance.themeAuto', Icon: Monitor },
];

function ThemePicker({ s, current, onChange }: {
  s: S; current: ColorSchemePreference; onChange: (pref: ColorSchemePreference) => void;
}) {
  const { t } = useTranslation();
  return (
    <View style={s.themePickerContainer}>
      {THEME_OPTIONS.map(({ key, labelKey, Icon }) => {
        const active = current === key;
        return (
          <Pressable
            key={key}
            style={[s.themeOption, active && s.themeOptionActive]}
            onPress={() => {
              if (key !== current) {
                hapticLight();
                onChange(key);
              }
            }}
          >
            <Icon size={18} color={active ? color.accent.base : color.fg.subtle} strokeWidth={2} />
            <Text style={[s.themeOptionLabel, active && s.themeOptionLabelActive]}>{t(labelKey)}</Text>
          </Pressable>
        );
      })}
    </View>
  );
}

// ---------------------------------------------------------------------------
// Language Picker — Follow System / English / 简体中文 (instant, no restart)
// ---------------------------------------------------------------------------

const VELA_REPO_URL = 'https://github.com/mondaylabsltd/vela-wallet';

/** Shipped app version — surfaced in About and attached to bug reports. */
const APP_VERSION = '1.0.0';

/**
 * Deep-link to the prefilled "translation fix" issue form, scoped to the
 * language the user is actually reading. The `language` query param matches a
 * dropdown option in .github/ISSUE_TEMPLATE/translation.yml verbatim, so the
 * form opens with that language already selected.
 */
function translationIssueURL(lang: AppLanguage): string {
  const option = `${LANGUAGE_NATIVE_NAMES[lang]} (${lang})`;
  const query = [
    'template=translation.yml',
    `title=${encodeURIComponent(`[i18n] ${option}: `)}`,
    `language=${encodeURIComponent(option)}`,
  ].join('&');
  return `${VELA_REPO_URL}/issues/new?${query}`;
}

function LanguagePickerModal({ s, visible, preference, systemLanguage, onSelect, onClose }: {
  s: S; visible: boolean; preference: LanguagePreference; systemLanguage: AppLanguage;
  onSelect: (pref: LanguagePreference) => void; onClose: () => void;
}) {
  const { t } = useTranslation();
  const options: { key: LanguagePreference; label: string; note?: string }[] = [
    // "Follow System" first; its note shows which concrete language the device resolves to.
    { key: 'auto', label: t('language.followSystem'), note: LANGUAGE_NATIVE_NAMES[systemLanguage] },
    // Each language is listed by its endonym (shown in its own script).
    ...SUPPORTED_LANGUAGES.map((code) => ({ key: code, label: LANGUAGE_NATIVE_NAMES[code] })),
  ];
  // The language the user is actually reading — what a translation report is about.
  const effectiveLang: AppLanguage = preference === 'auto' ? systemLanguage : preference;
  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.modalContainer}>
        <View style={s.modalHeader}>
          <Text style={s.modalTitle}>{t('language.pickerTitle')}</Text>
          <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>
        <ScrollView style={s.modalScroll} contentContainerStyle={s.epScrollContent}>
          <Text style={s.epDescription}>{t('language.pickerSubtitle')}</Text>
          {options.map((o) => {
            const sel = o.key === preference;
            return (
              <Pressable key={o.key} style={[s.fmtRow, sel && s.fmtRowSel]} onPress={() => { onSelect(o.key); onClose(); }}>
                <View style={s.fmtRowInfo}>
                  <Text style={s.fmtExample}>{o.label}</Text>
                  {o.note ? <Text style={s.fmtNote}>{o.note}</Text> : null}
                </View>
                {sel && <Check size={20} color={color.accent.base} strokeWidth={2.6} />}
              </Pressable>
            );
          })}
          <Pressable style={s.langContribute} onPress={() => openURL(translationIssueURL(effectiveLang))}>
            <Text style={s.langContributeNote}>{t('language.contributeNote')}</Text>
            <View style={s.langContributeCtaRow}>
              <Text style={s.langContributeCta}>{t('language.contributeCta')}</Text>
              <ExternalLink size={14} color={color.accent.base} strokeWidth={2.4} />
            </View>
          </Pressable>
        </ScrollView>
      </View>
    </AppModal>
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
    hapticLight();
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
// Treasury (Developer Options)
// ---------------------------------------------------------------------------

type TreasuryBalance = { chainId: number; name: string; explorerURL: string; balance: string; wei: bigint; recommended: string; recommendedWei: bigint; usd: number | null; loading: boolean };

function TreasuryModal({ visible, onClose }: { visible: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const [address, setAddress] = useState<string | null>(null);
  const [balances, setBalances] = useState<TreasuryBalance[]>([]);
  const [copied, setCopied] = useState(false);
  const [loading, setLoading] = useState(true);
  const [refreshKey, setRefreshKey] = useState(0);

  const { activeAccount } = useWallet();

  const loadBalances = useCallback(async (addr: string) => {
    const networks = getAllNetworksSync();
    const initial: TreasuryBalance[] = networks.map(n => ({
      chainId: n.chainId, name: n.displayName, explorerURL: n.explorerURL,
      balance: '...', wei: 0n, recommended: '...', recommendedWei: 0n, usd: null, loading: true,
    }));
    setBalances(initial);

    // Get native token prices from the existing token cache
    const nativePrices = new Map<number, number>();
    if (activeAccount?.address) {
      try {
        const tokens = await fetchTokens(activeAccount.address);
        for (const t of tokens) {
          if (isNativeToken(t) && t.priceUsd) {
            nativePrices.set(tokenChainId(t), t.priceUsd);
          }
        }
      } catch { /* prices are optional */ }
    }

    for (const net of networks) {
      fetchTreasuryBalance(addr, net.chainId).then(result => {
        const price = nativePrices.get(net.chainId);
        // Tempo returns a direct USD (pathUSD) value; native chains derive it from price.
        const usd = result.usd ?? (price ? (Number(result.wei) / 1e18) * price : null);
        setBalances(prev => prev.map(b =>
          b.chainId === net.chainId ? { ...b, balance: result.formatted, wei: result.wei, recommended: result.recommendedFormatted, recommendedWei: result.recommendedWei, usd, loading: false } : b
        ));
      });
    }
  }, [activeAccount?.address]);

  useEffect(() => {
    if (!visible) return;
    let cancelled = false;
    (async () => {
      setLoading(true);
      try {
        // Ensure user-configured endpoints are loaded before reading bundler URL
        const endpoints = await loadServiceEndpoints();
        const baseUrl = endpoints.bundlerServiceURL || getBuiltinBundlerUrl();
        const res = await fetchWithTimeout(
          `${baseUrl}/v1/treasury`,
          { headers: { 'Accept': 'application/json' } },
          { timeoutMs: NET_TIMEOUTS.bundlerRest },
        );
        if (!res.ok) throw new Error('Failed to fetch treasury');
        const data = await res.json();
        if (cancelled) return;
        setAddress(data.address);
        setLoading(false);
        loadBalances(data.address);
      } catch (err) {
        console.warn('[Treasury] Failed:', err);
        if (!cancelled) setLoading(false);
      }
    })();
    return () => { cancelled = true; };
  }, [visible, loadBalances]);

  // Refresh balances when refreshKey changes (but not on initial mount)
  useEffect(() => {
    if (refreshKey > 0 && address) loadBalances(address);
  }, [refreshKey, address, loadBalances]);

  const handleCopy = async () => {
    if (!address) return;
    await copyToClipboard(address);
    setCopied(true);
    hapticLight();
    setTimeout(() => setCopied(false), 2000);
  };

  const handleRefresh = () => {
    hapticLight();
    setRefreshKey(k => k + 1);
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <ScrollView style={{ flex: 1, backgroundColor: color.bg.base }} contentContainerStyle={{ padding: space['2xl'], paddingTop: space.xl }}>
        {/* Header with refresh */}
        <View style={{ flexDirection: 'row', justifyContent: 'center', alignItems: 'center', marginBottom: space.lg }}>
          <Text style={{ fontSize: text.xl, ...inter.bold, color: color.fg.base }}>{t('settingsModals.treasury.modalTitle')}</Text>
          {address && (
            <Pressable onPress={handleRefresh} hitSlop={8} style={{ position: 'absolute', right: 0 }}>
              <RefreshCw size={18} color={color.fg.subtle} strokeWidth={2} />
            </Pressable>
          )}
        </View>

        {loading && !address ? (
          <ActivityIndicator color={color.accent.base} style={{ marginTop: space['2xl'] }} />
        ) : address ? (
          <>
            {/* QR Code */}
            <View style={{ alignItems: 'center', padding: space.lg, backgroundColor: '#FFFFFF', borderRadius: radius.xl, alignSelf: 'center', marginBottom: space.lg, ...shadow.sm }}>
              <QRCode value={address} size={120} />
            </View>

            {/* Address + Copy */}
            <Pressable
              onPress={handleCopy}
              style={{ backgroundColor: color.bg.sunken, borderRadius: radius.lg, padding: space.lg, marginBottom: space.lg, borderWidth: 1, borderColor: color.border.base }}
            >
              <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: space.xs }}>
                <Text style={{ fontSize: text.xs, ...inter.semibold, color: color.fg.muted, textTransform: 'uppercase', letterSpacing: 0.5 }}>
                  {t('settingsModals.treasury.addressLabel')}
                </Text>
                {copied
                  ? <Check size={14} color={color.accent.base} strokeWidth={3} />
                  : <Copy size={14} color={color.fg.subtle} strokeWidth={2} />}
              </View>
              <Text style={{ fontSize: text.xs, ...inter.medium, fontFamily: font.mono, color: color.fg.base }} selectable>
                {address}
              </Text>
            </Pressable>

            {/* Total USD */}
            {(() => {
              const totalUsd = balances.reduce((sum, b) => sum + (b.usd ?? 0), 0);
              const anyLoading = balances.some(b => b.loading);
              return (
                <View style={{ flexDirection: 'row', justifyContent: 'space-between', alignItems: 'center', marginBottom: space.lg }}>
                  <Text style={{ fontSize: text.sm, ...inter.semibold, color: color.fg.muted, textTransform: 'uppercase', letterSpacing: 0.5 }}>
                    {t('settingsModals.treasury.balancesLabel')}
                  </Text>
                  {!anyLoading && totalUsd > 0 && (
                    <Text style={{ fontSize: text.sm, ...inter.bold, color: color.fg.base }}>
                      ${totalUsd < 0.01 ? totalUsd.toFixed(4) : totalUsd.toFixed(2)}
                    </Text>
                  )}
                </View>
              );
            })()}
            <VelaCard style={{ padding: 0 }}>
              {balances.map((b, i) => {
                const needsFunding = !b.loading && (b.wei < b.recommendedWei || (b.wei === 0n && b.recommendedWei === 0n));
                const explorerLink = `${b.explorerURL}/address/${address}`;
                return (
                  <View key={b.chainId}>
                    <Pressable
                      style={{ flexDirection: 'row', alignItems: 'center', paddingHorizontal: space.lg, paddingVertical: space.md }}
                      onPress={() => openURL(explorerLink)}
                    >
                      <View style={{ flex: 1 }}>
                        <View style={{ flexDirection: 'row', alignItems: 'center', gap: space.sm }}>
                          <Text style={{ fontSize: text.sm, ...inter.medium, color: color.fg.base }}>{b.name}</Text>
                          <ExternalLink size={10} color={color.fg.subtle} strokeWidth={2} />
                        </View>
                        {!b.loading && needsFunding && b.recommendedWei > 0n && (
                          <Text style={{ fontSize: text.xs, ...inter.regular, color: color.fg.muted, marginTop: 2 }}>
                            {t('settingsModals.treasury.minBalance', { amount: b.recommended })}
                          </Text>
                        )}
                      </View>
                      {b.loading ? (
                        <ActivityIndicator size="small" color={color.fg.subtle} />
                      ) : (
                        <View style={{ alignItems: 'flex-end', gap: 2 }}>
                          <View style={{ flexDirection: 'row', alignItems: 'center', gap: space.sm }}>
                            {needsFunding && (
                              <AlertTriangle size={12} color={color.warning.base} strokeWidth={2.5} />
                            )}
                            <Text style={{
                              fontSize: text.sm, ...inter.semibold, fontFamily: font.mono,
                              color: needsFunding ? color.warning.base : color.fg.base,
                            }}>
                              {b.balance}
                            </Text>
                          </View>
                          {b.usd != null && b.usd > 0 && (
                            <Text style={{ fontSize: text.xs, ...inter.regular, color: color.fg.muted }}>
                              ${b.usd < 0.01 ? b.usd.toFixed(4) : b.usd.toFixed(2)}
                            </Text>
                          )}
                        </View>
                      )}
                    </Pressable>
                    {i < balances.length - 1 && <View style={{ height: 1, backgroundColor: color.border.base, marginLeft: space.lg }} />}
                  </View>
                );
              })}
            </VelaCard>
          </>
        ) : (
          <Text style={{ fontSize: text.sm, color: color.fg.muted, textAlign: 'center' }}>
            {t('settingsModals.treasury.unreachable')}
          </Text>
        )}

        <Pressable style={{ alignItems: 'center', paddingVertical: space.xl }} onPress={onClose}>
          <Text style={{ fontSize: text.base, ...inter.medium, color: color.fg.subtle }}>{t('settingsModals.treasury.close')}</Text>
        </Pressable>
      </ScrollView>
    </AppModal>
  );
}

/** Fetch treasury balance and recommended minimum using the RPC pool. */
async function fetchTreasuryBalance(address: string, chainId: number): Promise<{
  formatted: string; wei: bigint; recommendedFormatted: string; recommendedWei: bigint; usd?: number | null;
}> {
  try {
    // Tempo has no native coin — eth_getBalance returns a meaningless sentinel.
    // Show the treasury's pathUSD (fee-token) balance instead. pathUSD is the same
    // address (0x20c0…0000, 6 decimals) on Tempo mainnet and testnet.
    if (isTempoChain(chainId)) {
      const data = '0x70a08231000000000000000000000000' + address.toLowerCase().replace(/^0x/, '');
      const balRes = await poolRpcCall('eth_call', [{ to: TEMPO_DEFAULT_FEE_TOKEN, data }, 'latest'], chainId);
      const wei = BigInt((balRes.result as string) ?? '0x0'); // pathUSD, 6 decimals
      const recommendedWei = 5_000_000n; // ~5 pathUSD float to seed gas fronting
      return {
        formatted: formatPathUsd(wei),
        wei,
        recommendedFormatted: formatPathUsd(recommendedWei),
        recommendedWei,
        usd: Number(wei) / 1e6, // pathUSD ≈ $1
      };
    }

    const [balRes, gasPriceRes] = await Promise.all([
      poolRpcCall('eth_getBalance', [address, 'latest'], chainId),
      poolRpcCall('eth_gasPrice', [], chainId),
    ]);
    const wei = BigInt((balRes.result as string) ?? '0x0');
    const gasPrice = BigInt((gasPriceRes.result as string) ?? '0x0');
    // Recommended: gasPrice × 10M gas — enough to sponsor ~15-20 new users
    const recommendedWei = gasPrice * 10_000_000n;
    return {
      formatted: formatEth(wei),
      wei,
      recommendedFormatted: formatEth(recommendedWei),
      recommendedWei,
    };
  } catch {
    return { formatted: 'error', wei: 0n, recommendedFormatted: '?', recommendedWei: 0n };
  }
}

/** Format a 6-decimal pathUSD balance for the treasury view. */
function formatPathUsd(units: bigint): string {
  const v = Number(units) / 1e6;
  if (v === 0) return '0';
  if (v < 0.01) return v.toFixed(4);
  return v.toFixed(2);
}

function formatEth(wei: bigint): string {
  const eth = Number(wei) / 1e18;
  if (eth === 0) return '0';
  if (eth < 0.000001) return '< 0.000001';
  if (eth < 0.001) return eth.toFixed(6);
  if (eth < 1) return eth.toFixed(4);
  return eth.toFixed(3);
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

export default function SettingsScreen() {
  const styles = useStyles(styleFactory);
  const { state, dispatch, activeAccount } = useWallet();
  const router = useRouter();
  const [showAccountSwitcher, setShowAccountSwitcher] = useState(false);
  const [showContacts, setShowContacts] = useState(false);
  const [showBugReport, setShowBugReport] = useState(false);
  const [showNetworkEditor, setShowNetworkEditor] = useState(false);
  const [showEndpointEditor, setShowEndpointEditor] = useState(false);
  const [showAddNetwork, setShowAddNetwork] = useState(false);
  const [showRpcProviders, setShowRpcProviders] = useState(false);
  const [localePrefs, setLocalePrefs] = useState<LocalePrefs>(getLocalePrefs);
  const [fmtPicker, setFmtPicker] = useState<null | 'number' | 'date' | 'time'>(null);
  useEffect(() => { loadLocalePrefs().then(setLocalePrefs); }, []);
  const applyLocale = async (patch: Partial<LocalePrefs>) => {
    const next = { ...localePrefs, ...patch };
    setLocalePrefs(next);
    await saveLocalePrefs(next);
  };
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [showDevOptions, setShowDevOptions] = useState(false);
  const [showTreasury, setShowTreasury] = useState(false);
  const [devUnlocked, setDevUnlocked] = useState(false);
  useEffect(() => { AsyncStorage.getItem('dev_unlocked').then(v => { if (v === '1') setDevUnlocked(true); }); }, []);
  const [showSignOut, setShowSignOut] = useState(false);
  const [pendingSync, setPendingSync] = useState(false);
  const [signingOut, setSigningOut] = useState(false);
  const { levelIndex: currentScaleIndex, setIndex: setScaleIndex } = useTextScale();
  const { preference: colorPref, setPreference: setColorPref } = useColorSchemePreference();
  const { t } = useTranslation();
  const { preference: langPref, resolved: langResolved, systemLanguage, setPreference: setLangPref } = useLanguagePreference();
  const [showLanguagePicker, setShowLanguagePicker] = useState(false);

  const accountName = activeAccount?.name ?? 'No Wallet';
  const address = activeAccount?.address ?? state.address;

  const handleOpenSignOut = async () => {
    const pending = await hasPendingUploads();
    setPendingSync(pending);
    setShowSignOut(true);
  };

  const handleSignOut = () => {
    setSigningOut(true);
    dispatch({ type: 'LOGOUT' });
    router.replace('/');
  };

  const languageLabel = LANGUAGE_NATIVE_NAMES[langResolved];
  const languageSubtitle = langPref === 'auto' ? `${languageLabel} · ${t('common.system')}` : languageLabel;

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.scrollContent}>
        <Animated.View entering={fadeIn(0, 300)} style={styles.screenHeader}>
          <Text style={styles.screenTitle}>{t('settings.title')}</Text>
          <Pressable onPress={() => router.navigate('/wallet')} hitSlop={8} style={styles.screenClose}>
            <X size={22} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </Animated.View>

        {/* Account */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(50, 300)}>
          <Text style={styles.sectionTitle}>{t('settings.sections.account')}</Text>
          <VelaCard>
            <SettingsRow s={styles} icon={{ bg: color.accent.soft, fg: color.accent.base, Icon: UserIcon }}
              title={accountName} subtitle={address ? shortAddress(address) : t('settings.account.switch')}
              showDivider={false} onPress={() => setShowAccountSwitcher(true)} />
          </VelaCard>
        </Animated.View>

        {/* Contacts */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(60, 300)}>
          <VelaCard>
            <SettingsRow s={styles} icon={{ bg: color.info.soft, fg: color.info.base, Icon: BookUser }}
              title={t('contacts.title')} subtitle={t('contacts.manageSubtitle')}
              showDivider={false} onPress={() => setShowContacts(true)} />
          </VelaCard>
        </Animated.View>

        {/* Feedback */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(75, 300)}>
          <VelaCard>
            <SettingsRow s={styles} icon={{ bg: color.accent.soft, fg: color.accent.base, Icon: MessageSquare }}
              title={t('settings.feedback.title')} subtitle={t('settings.feedback.subtitle')}
              showDivider={false} onPress={() => setShowBugReport(true)}
              right={<ExternalLink size={16} color={color.fg.subtle} />} />
          </VelaCard>
        </Animated.View>

        {/* Appearance */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(100, 300)}>
          <Text style={styles.sectionTitle}>{t('settings.sections.appearance')}</Text>
          <VelaCard>
            <SettingsRow s={styles} icon={{ bg: color.info.soft, fg: color.info.base, Icon: Languages }}
              title={t('language.title')} subtitle={languageSubtitle}
              showDivider onPress={() => setShowLanguagePicker(true)} />
            <TextScaleSlider
              s={styles}
              currentIndex={currentScaleIndex}
              onChangeIndex={setScaleIndex}
            />
            <View style={styles.settingsRowDividerFull} />
            <ThemePicker s={styles} current={colorPref} onChange={setColorPref} />
          </VelaCard>
        </Animated.View>

        {/* Localization */}
        {(() => {
          const numOpts = numberFormatOptions();
          const dateOpts = dateFormatOptions();
          const timeOpts = timeFormatOptions();
          const subtitleFor = <K extends string>(key: K, opts: FormatOption<K>[]) => {
            const ex = opts.find((o) => o.key === key)?.example ?? '';
            return key === 'auto' ? t('settings.localization.autoExample', { example: ex }) : ex;
          };
          return (
            <Animated.View style={styles.sectionContainer} entering={fadeInDown(135, 300)}>
              <Text style={styles.sectionTitle}>{t('settings.sections.localization')}</Text>
              <VelaCard>
                <SettingsRow s={styles} icon={{ bg: color.info.soft, fg: color.info.base, Icon: Hash }}
                  title={t('settings.localization.numberTitle')} subtitle={subtitleFor(localePrefs.numberFormat, numOpts)}
                  showDivider onPress={() => setFmtPicker('number')} />
                <SettingsRow s={styles} icon={{ bg: color.info.soft, fg: color.info.base, Icon: Calendar }}
                  title={t('settings.localization.dateTitle')} subtitle={subtitleFor(localePrefs.dateFormat, dateOpts)}
                  showDivider onPress={() => setFmtPicker('date')} />
                <SettingsRow s={styles} icon={{ bg: color.info.soft, fg: color.info.base, Icon: Clock }}
                  title={t('settings.localization.timeTitle')} subtitle={subtitleFor(localePrefs.timeFormat, timeOpts)}
                  showDivider={false} onPress={() => setFmtPicker('time')} />
              </VelaCard>
              <FormatPickerModal s={styles} visible={fmtPicker === 'number'} title={t('settings.localization.numberTitle')}
                subtitle={t('settings.localization.numberSubtitle')}
                options={numOpts} selected={localePrefs.numberFormat}
                onSelect={(k) => applyLocale({ numberFormat: k })} onClose={() => setFmtPicker(null)} />
              <FormatPickerModal s={styles} visible={fmtPicker === 'date'} title={t('settings.localization.dateTitle')}
                subtitle={t('settings.localization.dateSubtitle')}
                options={dateOpts} selected={localePrefs.dateFormat}
                onSelect={(k) => applyLocale({ dateFormat: k })} onClose={() => setFmtPicker(null)} />
              <FormatPickerModal s={styles} visible={fmtPicker === 'time'} title={t('settings.localization.timeTitle')}
                subtitle={t('settings.localization.timeSubtitle')}
                options={timeOpts} selected={localePrefs.timeFormat}
                onSelect={(k) => applyLocale({ timeFormat: k })} onClose={() => setFmtPicker(null)} />
            </Animated.View>
          );
        })()}

        {/* Advanced */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(150, 300)}>
          <Pressable style={styles.advancedHeader} onPress={() => setShowAdvanced(!showAdvanced)}>
            <Text style={styles.sectionTitle}>{t('settings.sections.advanced')}</Text>
            <ChevronDown size={14} color={color.fg.subtle} style={showAdvanced ? { transform: [{ rotate: '180deg' }] } : undefined} />
          </Pressable>
          {showAdvanced && (
            <VelaCard>
              <SettingsRow s={styles} icon={{ bg: color.info.soft, fg: color.info.base, Icon: NetworkIcon }}
                title={t('settings.advanced.networksTitle')} subtitle={t('settings.advanced.networksSubtitle')}
                showDivider={true} onPress={() => setShowNetworkEditor(true)} />
              <SettingsRow s={styles} icon={{ bg: color.accent.soft, fg: color.accent.base, Icon: Zap }}
                title={t('settings.advanced.rpcProvidersTitle', { defaultValue: 'RPC Providers' })}
                subtitle={t('settings.advanced.rpcProvidersSubtitle', { defaultValue: 'Alchemy, dRPC, Ankr keys' })}
                showDivider={true} onPress={() => setShowRpcProviders(true)} />
              <SettingsRow s={styles} icon={{ bg: color.success.soft, fg: color.success.base, Icon: Plus }}
                title={t('settings.advanced.addNetworkTitle')} subtitle={t('settings.advanced.addNetworkSubtitle')}
                showDivider={true} onPress={() => setShowAddNetwork(true)} />
              <SettingsRow s={styles} icon={{ bg: color.success.soft, fg: color.success.base, Icon: Server }}
                title={t('settings.advanced.endpointsTitle')} subtitle={t('settings.advanced.endpointsSubtitle')}
                showDivider={false} onPress={() => setShowEndpointEditor(true)} />
            </VelaCard>
          )}
        </Animated.View>

        {/* Developer Options (hidden until 6-tap unlock on ADVANCED) */}
        {devUnlocked && (
          <Animated.View style={styles.sectionContainer} entering={fadeInDown(175, 300)}>
            <Pressable style={styles.advancedHeader} onPress={() => setShowDevOptions(!showDevOptions)}>
              <Text style={styles.sectionTitle}>{t('settings.sections.developer')}</Text>
              <ChevronDown size={14} color={color.fg.subtle} style={showDevOptions ? { transform: [{ rotate: '180deg' }] } : undefined} />
            </Pressable>
            {showDevOptions && (
              <VelaCard>
                <SettingsRow s={styles} icon={{ bg: color.warning.soft, fg: color.warning.base, Icon: Key }}
                  title={t('settings.developer.treasuryTitle')} subtitle={t('settings.developer.treasurySubtitle')}
                  showDivider={true} onPress={() => setShowTreasury(true)} />
                <SettingsRow s={styles} icon={{ bg: color.accent.soft, fg: color.accent.base, Icon: Key }}
                  title={t('settings.developer.clearSigningTitle')} subtitle={t('settings.developer.clearSigningSubtitle')}
                  showDivider={false} onPress={() => router.push('/clear-signing-test')} />
              </VelaCard>
            )}
          </Animated.View>
        )}

        {/* About & Sign Out */}
        <Animated.View style={styles.sectionContainer} entering={fadeInDown(200, 300)}>
          <VelaCard>
            <SettingsRow s={styles} icon={{ bg: color.bg.sunken, fg: color.fg.muted, Icon: InfoIcon }}
              title={t('settings.about.title')} subtitle={t('settings.about.subtitle', { version: APP_VERSION })} showDivider={false} onPress={() => router.push('/about')} />
          </VelaCard>
        </Animated.View>

        <Animated.View entering={fadeInDown(225, 300)}>
          <Pressable style={styles.logoutButton} onPress={handleOpenSignOut}>
            <LogOutIcon size={16} color={color.accent.base} />
            <Text style={styles.logoutText}>{t('settings.signOut.button')}</Text>
          </Pressable>
        </Animated.View>
      </ScrollView>

      <LanguagePickerModal s={styles} visible={showLanguagePicker} preference={langPref}
        systemLanguage={systemLanguage} onSelect={setLangPref} onClose={() => setShowLanguagePicker(false)} />
      <AccountSwitcherModal s={styles} visible={showAccountSwitcher} onClose={() => setShowAccountSwitcher(false)} />
      <ContactsManager visible={showContacts} onClose={() => setShowContacts(false)} />
      <BugReportModal visible={showBugReport} language={langResolved} onClose={() => setShowBugReport(false)} />
      <NetworkEditorModal s={styles} visible={showNetworkEditor} onClose={() => setShowNetworkEditor(false)} />
      <EndpointEditorModal s={styles} visible={showEndpointEditor} onClose={() => setShowEndpointEditor(false)} />
      <AddNetworkModal s={styles} visible={showAddNetwork} onClose={() => setShowAddNetwork(false)} onAdded={() => {}} />
      <RpcProvidersModal visible={showRpcProviders} onClose={() => setShowRpcProviders(false)} />
      <TreasuryModal visible={showTreasury} onClose={() => setShowTreasury(false)} />

      {/* Sign Out Confirmation */}
      <AppModal visible={showSignOut} onClose={() => setShowSignOut(false)}>
        <View style={styles.signOutModal}>
          <View style={styles.signOutIconWrap}>
            <LogOutIcon size={24} color={color.accent.base} strokeWidth={2} />
          </View>
          <Text style={styles.signOutTitle}>{t('settings.signOut.title')}</Text>
          <Text style={styles.signOutDesc}>
            {t('settings.signOut.desc')}
          </Text>

          {pendingSync && (
            <View style={styles.signOutWarning}>
              <AlertTriangle size={16} color={color.warning.base} strokeWidth={2} />
              <Text style={styles.signOutWarningText}>
                {t('settings.signOut.warning')}
              </Text>
            </View>
          )}

          <VelaButton
            title={pendingSync ? t('settings.signOut.anyway') : t('settings.signOut.button')}
            onPress={handleSignOut}
            variant="accent"
            loading={signingOut}
            style={styles.signOutBtn}
          />
          <Pressable style={styles.signOutCancel} onPress={() => setShowSignOut(false)}>
            <Text style={styles.signOutCancelText}>{t('settings.signOut.cancel')}</Text>
          </Pressable>
        </View>
      </AppModal>
    </ScreenContainer>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styleFactory = () => ({
  scrollContent: { paddingTop: space.md, paddingBottom: space['5xl'] },
  screenHeader: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const, marginBottom: space['3xl'] },
  screenTitle: { fontSize: text['3xl'], ...inter.bold, color: color.fg.base, letterSpacing: -0.5 },
  screenClose: { width: 40, height: 40, alignItems: 'center' as const, justifyContent: 'center' as const },
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

  // Format picker (number / date / time)
  fmtRow: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.lg, paddingVertical: space.lg, paddingHorizontal: space.xl, marginBottom: space.md, backgroundColor: color.bg.raised, borderRadius: radius.xl, borderWidth: 1.5, borderColor: 'transparent' as const, ...shadow.sm },
  fmtRowSel: { borderColor: color.accent.base },
  fmtRowInfo: { flex: 1, gap: 2 },
  fmtExample: { fontSize: text.lg, ...inter.semibold, color: color.fg.base, fontFamily: font.mono },
  fmtNote: { fontSize: text.sm, ...inter.regular, color: color.fg.muted },

  // Language picker — "help us translate" footer below the language list
  langContribute: { marginTop: space.lg, paddingHorizontal: space.sm, paddingVertical: space.md, gap: space.sm },
  langContributeNote: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 20 },
  langContributeCtaRow: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.xs },
  langContributeCta: { fontSize: text.sm, ...inter.medium, color: color.accent.base },

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

  // Sign Out Modal
  signOutModal: { padding: space['3xl'], paddingTop: space['2xl'], alignItems: 'center' as const },
  signOutIconWrap: { width: 56, height: 56, borderRadius: 28, backgroundColor: color.accent.soft, alignItems: 'center' as const, justifyContent: 'center' as const, marginBottom: space.xl },
  signOutTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base, marginBottom: space.md },
  signOutDesc: { fontSize: text.base, ...inter.regular, color: color.fg.muted, textAlign: 'center' as const, lineHeight: 22, marginBottom: space.xl },
  signOutWarning: { flexDirection: 'row' as const, alignItems: 'flex-start' as const, gap: space.md, backgroundColor: color.warning.soft, borderRadius: radius.lg, padding: space.xl, marginBottom: space.xl, width: '100%' as const },
  signOutWarningText: { flex: 1, fontSize: text.sm, ...inter.medium, color: color.warning.base, lineHeight: 20 },
  signOutBtn: { width: '100%' as const, marginBottom: space.lg },
  signOutCancel: { paddingVertical: space.lg },
  signOutCancelText: { fontSize: text.base, ...inter.semibold, color: color.fg.muted },
});
