/**
 * RPC Providers modal (Settings → Advanced → RPC Providers).
 *
 * One global API key per provider (Alchemy / dRPC / Ankr) unlocks every
 * network that provider serves. When a key is present we probe each supported
 * network's `eth_chainId` and show which networks are reachable and how fast.
 *
 * The keys feed the RPC pool's new `provider` tier (see services/rpc-pool.ts):
 * per-network override > provider keys > Vela built-in > ethereum-data index.
 */

import { ChainLogo } from '@/components/ChainLogo';
import { AppModal } from '@/components/ui/AppModal';
import { VelaCard } from '@/components/ui/VelaCard';
import { color, font, inter, radius, space, text, useStyles } from '@/constants/theme';
import { chainMeta } from '@/models/chains';
import { openURL } from '@/services/platform';
import {
  buildProviderRpcUrl,
  PROVIDER_ORDER,
  PROVIDERS,
  providerChainIds,
  type ProviderId,
  type RpcProviderKeys,
} from '@/services/rpc-providers';
import { invalidateAllPools, probeRpcChainId } from '@/services/rpc-pool';
import { getRpcProviderKeys, loadRpcProviders, saveRpcProviders } from '@/services/storage';
import { ChevronDown, ChevronUp, ExternalLink, Eye, EyeOff, X } from 'lucide-react-native';
import React, { useCallback, useEffect, useState } from 'react';
import { ActivityIndicator, Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import { useTranslation } from 'react-i18next';

const PROBE_TIMEOUT_MS = 6000;
/** Latency thresholds for the per-network badge colour. */
const FAST_MS = 300;
const OK_MS = 800;

type NetResult = { chainId: number; name: string; logoLabel: string; logoColor: string; logoBg: string; ok: boolean; latencyMs: number };
type TestState = { status: 'testing' | 'done'; results: NetResult[] };

export function RpcProvidersModal({ visible, onClose }: { visible: boolean; onClose: () => void }) {
  const { t } = useTranslation();
  const s = useStyles(styleFactory);

  // Draft key per provider — controlled inputs, seeded from the saved cache.
  const [draft, setDraft] = useState<RpcProviderKeys>({});
  const [reveal, setReveal] = useState<Partial<Record<ProviderId, boolean>>>({});
  const [expanded, setExpanded] = useState<Partial<Record<ProviderId, boolean>>>({});
  const [tests, setTests] = useState<Partial<Record<ProviderId, TestState>>>({});

  const runTest = useCallback(async (id: ProviderId, rawKey: string) => {
    const key = rawKey.trim();
    if (!key) {
      setTests(prev => ({ ...prev, [id]: undefined }));
      return;
    }
    const chainIds = providerChainIds(id);
    const base: NetResult[] = chainIds.map(cid => {
      const meta = chainMeta(cid);
      return {
        chainId: cid,
        name: meta?.displayName ?? `Chain ${cid}`,
        logoLabel: meta?.iconLabel ?? '?',
        logoColor: meta?.iconColor ?? color.fg.muted,
        logoBg: meta?.iconBg ?? color.bg.sunken,
        ok: false,
        latencyMs: 0,
      };
    });
    setTests(prev => ({ ...prev, [id]: { status: 'testing', results: base } }));

    const results = await Promise.all(
      base.map(async (r) => {
        const url = buildProviderRpcUrl(id, r.chainId, key);
        if (!url) return { ...r, ok: false, latencyMs: 0 };
        const t0 = Date.now();
        const reported = await probeRpcChainId(url, PROBE_TIMEOUT_MS);
        return { ...r, ok: reported === r.chainId, latencyMs: Date.now() - t0 };
      }),
    );
    setTests(prev => ({ ...prev, [id]: { status: 'done', results } }));
  }, []);

  // Seed drafts and auto-test configured providers whenever the sheet opens.
  useEffect(() => {
    if (!visible) return;
    loadRpcProviders().then(() => {
      const saved = getRpcProviderKeys();
      setDraft({ ...saved });
      setTests({});
      setExpanded({});
      for (const id of PROVIDER_ORDER) {
        if (saved[id]) runTest(id, saved[id]!);
      }
    });
  }, [visible, runTest]);

  // Persist all keys + invalidate pools so the next RPC call picks them up.
  const persist = useCallback(async (next: RpcProviderKeys) => {
    await saveRpcProviders(next);
    invalidateAllPools();
  }, []);

  const onKeyBlur = useCallback((id: ProviderId) => {
    const next = { ...draft, [id]: (draft[id] ?? '').trim() };
    setDraft(next);
    persist(next);
    runTest(id, next[id] ?? '');
  }, [draft, persist, runTest]);

  const onKeyChange = useCallback((id: ProviderId, value: string) => {
    setDraft(prev => ({ ...prev, [id]: value }));
    // Drop stale results so the old latency isn't shown against a new key.
    setTests(prev => ({ ...prev, [id]: undefined }));
  }, []);

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={s.container}>
        <View style={s.header}>
          <Text style={s.title}>{t('settingsModals.rpcProviders.modalTitle', { defaultValue: 'RPC Providers' })}</Text>
          <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>
        <ScrollView style={s.scroll} contentContainerStyle={s.scrollContent} keyboardShouldPersistTaps="handled">
          <Text style={s.description}>
            {t('settingsModals.rpcProviders.description', {
              defaultValue: 'Add an API key to route supported networks through your own provider. Priority: per-network RPC, then your providers, then Vela built-in, then the chain index.',
            })}
          </Text>

          {PROVIDER_ORDER.map((id) => {
            const meta = PROVIDERS[id];
            const value = draft[id] ?? '';
            const hasKey = value.trim().length > 0;
            const test = tests[id];
            const okCount = test?.results.filter(r => r.ok).length ?? 0;
            const isOpen = !!expanded[id];

            return (
              <VelaCard key={id} style={s.card}>
                <View style={s.cardHead}>
                  <Text style={s.providerLabel}>{meta.label}</Text>
                  <ProviderStatus s={s} hasKey={hasKey} test={test} okCount={okCount} />
                </View>

                <View style={s.inputRow}>
                  <TextInput
                    style={s.input}
                    value={value}
                    onChangeText={(v) => onKeyChange(id, v)}
                    onBlur={() => onKeyBlur(id)}
                    placeholder={meta.keyPlaceholder}
                    placeholderTextColor={color.fg.subtle}
                    autoCapitalize="none"
                    autoCorrect={false}
                    secureTextEntry={!reveal[id]}
                  />
                  {hasKey ? (
                    <Pressable onPress={() => setReveal(p => ({ ...p, [id]: !p[id] }))} hitSlop={8} style={s.eyeBtn}>
                      {reveal[id]
                        ? <EyeOff size={18} color={color.fg.muted} strokeWidth={2} />
                        : <Eye size={18} color={color.fg.muted} strokeWidth={2} />}
                    </Pressable>
                  ) : null}
                </View>

                <View style={s.actionsRow}>
                  <Pressable
                    onPress={() => openURL(meta.keyUrl)}
                    hitSlop={8}
                    style={s.getKeyBtn}
                    accessibilityRole="button"
                    accessibilityLabel={t('settingsModals.rpcProviders.getKey', { defaultValue: 'Get a key' })}
                  >
                    <Text style={s.getKeyText}>{t('settingsModals.rpcProviders.getKey', { defaultValue: 'Get a key' })}</Text>
                    <ExternalLink size={13} color={color.fg.muted} strokeWidth={2} />
                  </Pressable>
                  {hasKey ? (
                    <Pressable onPress={() => runTest(id, value)} hitSlop={8} style={s.testBtn} disabled={test?.status === 'testing'}>
                      <Text style={s.testText}>{t('settingsModals.rpcProviders.test', { defaultValue: 'Test' })}</Text>
                    </Pressable>
                  ) : null}
                </View>

                {test?.status === 'done' ? (
                  <>
                    <Pressable style={s.summaryRow} onPress={() => setExpanded(p => ({ ...p, [id]: !p[id] }))}>
                      <Text style={s.summaryText}>
                        {t('settingsModals.rpcProviders.supportsCount', {
                          defaultValue: 'Supports {{count}} of {{total}} networks',
                          count: okCount,
                          total: test.results.length,
                        })}
                      </Text>
                      {isOpen
                        ? <ChevronUp size={16} color={color.fg.subtle} strokeWidth={2} />
                        : <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2} />}
                    </Pressable>
                    {isOpen ? (
                      <View style={s.netList}>
                        {test.results.map((r) => (
                          <View key={r.chainId} style={s.netRow}>
                            <ChainLogo label={r.logoLabel} color={r.logoColor} bgColor={r.logoBg} size={22} />
                            <Text style={s.netName}>{r.name}</Text>
                            <LatencyBadge s={s} ok={r.ok} latencyMs={r.latencyMs} />
                          </View>
                        ))}
                      </View>
                    ) : null}
                  </>
                ) : null}
              </VelaCard>
            );
          })}
        </ScrollView>
      </View>
    </AppModal>
  );
}

function ProviderStatus({ s, hasKey, test, okCount }: {
  s: S; hasKey: boolean; test?: TestState; okCount: number;
}) {
  // useTranslation() inside (vs a `t` prop) keeps the large typed-key union from
  // being re-instantiated at the prop boundary — that trips TS2589 as keys grow.
  const { t } = useTranslation();
  if (!hasKey) {
    return <Text style={s.statusMuted}>{t('settingsModals.rpcProviders.notSet', { defaultValue: 'Not set' })}</Text>;
  }
  if (!test || test.status === 'testing') {
    return <ActivityIndicator size={12} color={color.fg.subtle} />;
  }
  const good = okCount > 0;
  return (
    <View style={[s.statusPill, { backgroundColor: good ? color.success.soft : color.warning.soft }]}>
      <Text style={[s.statusPillText, { color: good ? color.success.base : color.warning.base }]}>
        {good
          ? t('settingsModals.rpcProviders.activeNetworks', { defaultValue: '{{count}} networks', count: okCount })
          : t('settingsModals.rpcProviders.checkKey', { defaultValue: 'Check key' })}
      </Text>
    </View>
  );
}

function LatencyBadge({ s, ok, latencyMs }: {
  s: S; ok: boolean; latencyMs: number;
}) {
  const { t } = useTranslation();
  if (!ok) {
    return (
      <View style={s.latBadge}>
        <View style={[s.latDot, { backgroundColor: color.fg.subtle }]} />
        <Text style={[s.latText, { color: color.fg.subtle }]}>{t('settingsModals.rpcProviders.unavailable', { defaultValue: 'Unavailable' })}</Text>
      </View>
    );
  }
  const c = latencyMs < FAST_MS ? color.success.base : latencyMs < OK_MS ? color.warning.base : color.error.base;
  return (
    <View style={s.latBadge}>
      <View style={[s.latDot, { backgroundColor: c }]} />
      <Text style={[s.latText, { color: c }]}>{latencyMs}ms</Text>
    </View>
  );
}

type S = ReturnType<typeof styleFactory>;

const styleFactory = () => ({
  container: { flex: 1, backgroundColor: color.bg.base },
  header: {
    flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const,
    paddingHorizontal: space.xl, paddingVertical: space.lg,
    borderBottomWidth: 1, borderBottomColor: color.border.base,
  },
  title: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  scroll: { flex: 1 },
  scrollContent: { padding: space.xl, gap: space.lg, paddingBottom: space['5xl'] },
  description: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: text.sm * 1.5, marginBottom: space.xs },

  card: { padding: space.xl, gap: space.lg },
  cardHead: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const },
  providerLabel: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },

  statusMuted: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },
  statusPill: { paddingHorizontal: space.md, paddingVertical: space.xs, borderRadius: radius.full },
  statusPillText: { fontSize: text.xs, ...inter.semibold },

  inputRow: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.sm },
  input: {
    flex: 1, fontSize: text.base, fontFamily: font.mono, color: color.fg.base,
    backgroundColor: color.bg.sunken, borderRadius: radius.md,
    paddingHorizontal: space.lg, paddingVertical: space.md, minHeight: 40,
  },
  eyeBtn: { padding: space.sm },

  actionsRow: { flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const },
  // Quiet external link — accent is reserved for commit actions, not sign-up detours.
  getKeyBtn: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.xs },
  getKeyText: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  testBtn: { paddingHorizontal: space.lg, paddingVertical: space.sm, borderRadius: radius.md, backgroundColor: color.bg.sunken },
  testText: { fontSize: text.sm, ...inter.semibold, color: color.fg.base },

  summaryRow: {
    flexDirection: 'row' as const, alignItems: 'center' as const, justifyContent: 'space-between' as const,
    paddingTop: space.md, borderTopWidth: 1, borderTopColor: color.border.base,
  },
  summaryText: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },

  netList: { gap: space.md, marginTop: space.xs },
  netRow: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.lg },
  netName: { flex: 1, fontSize: text.base, ...inter.medium, color: color.fg.base },

  latBadge: { flexDirection: 'row' as const, alignItems: 'center' as const, gap: space.sm },
  latDot: { width: 7, height: 7, borderRadius: 4 },
  latText: { fontSize: text.sm, ...inter.semibold },
});
