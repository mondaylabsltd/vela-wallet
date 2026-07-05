/**
 * RPC trouble banner + fix flow.
 *
 * Shown when one or more chains have all their RPC endpoints failing. Surfaces
 * which networks are down, lets the user paste a working RPC URL, and points
 * them at reputable providers where they can get one. Rendered from Home's
 * header so the recovery path is identical everywhere a balance/activity
 * read can silently come up short.
 */
import { ChainLogo } from '@/components/ChainLogo';
import { AppModal } from '@/components/ui/AppModal';
import { fadeInDown } from '@/constants/entering';
import { color, createStyles, inter, radius, shadow, space, text } from '@/constants/theme';
import type { AppLanguage } from '@/i18n';
import { getAllNetworksSync, type Network } from '@/models/network';
import { buildBugReportURL } from '@/services/feedback';
import { openBrowser, showAlert } from '@/services/platform';
import { probeRpcChainId, refreshPool } from '@/services/rpc-pool';
import { getNetworkConfig, saveNetworkConfig } from '@/services/storage';
import { AlertTriangle, ExternalLink, Wifi, X } from 'lucide-react-native';
import React, { useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, Pressable, ScrollView, Text, TextInput, View } from 'react-native';
import Animated from 'react-native-reanimated';

/** Reputable RPC providers with free tiers, plus Chainlist for browsing. */
const RPC_PROVIDERS: { name: string; url: string }[] = [
  { name: 'Alchemy', url: 'https://www.alchemy.com' },
  { name: 'QuickNode', url: 'https://www.quicknode.com' },
  { name: 'dRPC', url: 'https://drpc.org' },
  { name: 'Chainlist', url: 'https://chainlist.org' },
];

export function RpcTroubleBanner({
  chainIds,
  onFix,
}: {
  chainIds: number[];
  /** Open the shared RPC-fix flow for a chain (the modal is owned by the parent,
      so it's a single instance across the banner and the balance-detail sheet). */
  onFix: (chainId: number) => void;
}) {
  const { t } = useTranslation();

  // Enter once, on first mount — a parent re-render (e.g. Home refreshing every
  // account's balance while the switcher is open) must not replay the slide-in.
  const hasEntered = useRef(false);
  useEffect(() => { hasEntered.current = true; }, []);

  const failedNetworks = chainIds
    .map(id => getAllNetworksSync().find(n => n.chainId === id))
    .filter((n): n is Network => !!n);

  if (failedNetworks.length === 0) return null;

  return (
    <Animated.View entering={hasEntered.current ? undefined : fadeInDown(0, 300)} style={styles.banner}>
      <AlertTriangle size={14} color={'#C07A0A'} strokeWidth={2.5} />
      <View style={styles.bannerContent}>
        <Text style={styles.bannerText}>
          {failedNetworks.length === 1
            ? t('assets.rpcUnavailableSingle', { name: failedNetworks[0].displayName })
            : t('assets.rpcUnavailableMultiple', { count: failedNetworks.length })}
        </Text>
        <View style={styles.bannerChips}>
          {failedNetworks.map(net => (
            <Pressable key={net.chainId} style={styles.bannerChip} onPress={() => onFix(net.chainId)}>
              <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />
              <Text style={styles.bannerChipText}>{net.displayName}</Text>
              <Text style={styles.bannerFixLink}>{t('assets.rpcFix')}</Text>
            </Pressable>
          ))}
        </View>
      </View>
    </Animated.View>
  );
}

/**
 * RpcFixModal — the paste-a-working-RPC recovery form, as a standalone controlled
 * modal. Owned by the parent (a single instance drives both the RpcTroubleBanner
 * chips and the balance-detail sheet's "Fix" rows), so closing one surface and
 * opening the fix flow never fights a nested modal. `chainId` = which chain to
 * fix (null = closed).
 */
/**
 * RpcFixForm — just the recovery form body (no modal wrapper). Extracted so it can
 * render EITHER inside its own modal (RpcFixModal, for the banner) OR swapped in as
 * a sub-view of another sheet (the balance-detail sheet). The latter matters on iOS:
 * two sibling AppModals are native pageSheets, and presenting one while dismissing
 * another from the same VC is dropped by UIKit — so a sheet must show the fix form
 * IN PLACE, never by handing off to a second modal. `chainId` is always a real chain.
 */
export function RpcFixForm({
  chainId,
  onClose,
  onResolved,
}: {
  chainId: number;
  onClose: () => void;
  /** Called after the user saves a new RPC for a chain, so the caller can re-fetch. */
  onResolved?: (chainId: number) => void;
}) {
  const { t, i18n } = useTranslation();
  const [fixUrl, setFixUrl] = useState('');
  const [saving, setSaving] = useState(false);

  // Pre-fill the user's *saved* override, not the built-in default. The default
  // is the endpoint that's currently failing, so re-showing it just makes a prior
  // fix look like it didn't stick — and invites re-saving the broken URL. Clear
  // first so a stale prior-chain URL never flashes when switching chains.
  useEffect(() => {
    let active = true;
    setFixUrl('');
    getNetworkConfig(chainId).then(saved => { if (active) setFixUrl(saved?.rpcURL ?? ''); }).catch(() => {});
    return () => { active = false; };
  }, [chainId]);

  const handleSave = async () => {
    if (!fixUrl.trim()) return;
    const url = fixUrl.trim();
    setSaving(true);
    try {
      // Validate before saving — a recovery flow that cheerfully stores a dead or
      // wrong-chain URL (and reports "saved") is worse than no validation at all.
      const reportedChainId = await probeRpcChainId(url);
      if (reportedChainId === null) {
        showAlert(t('assets.errorTitle'), t('assets.rpcFixUnreachable'));
        return;
      }
      if (reportedChainId !== chainId) {
        showAlert(t('assets.errorTitle'), t('assets.rpcFixWrongChain', { expected: chainId, actual: reportedChainId }));
        return;
      }
      // Preserve any explorer/bundler the user already customized in Settings:
      // saveNetworkConfig replaces the whole entry by chainId, so falling back to
      // the built-in defaults here would silently clobber those overrides.
      const saved = await getNetworkConfig(chainId);
      const net = getAllNetworksSync().find(n => n.chainId === chainId);
      await saveNetworkConfig({
        chainId,
        rpcURL: url,
        explorerURL: saved?.explorerURL ?? net?.explorerURL ?? '',
        bundlerURL: saved?.bundlerURL ?? net?.bundlerURL ?? '',
      });
      await refreshPool(chainId);
      onResolved?.(chainId);
      onClose();
    } catch {
      showAlert(t('assets.errorTitle'), t('assets.errorSaveRpc'));
    } finally {
      setSaving(false);
    }
  };

  const net = getAllNetworksSync().find(n => n.chainId === chainId);
  return (
    <View style={styles.fixContainer}>
      <View style={styles.fixHeader}>
        <Text style={styles.fixTitle}>{t('assets.rpcFixTitle')}</Text>
        <Pressable onPress={onClose} hitSlop={8}>
          <X size={22} color={color.fg.base} strokeWidth={2} />
        </Pressable>
      </View>

      <ScrollView contentContainerStyle={styles.fixBody}>
        <View style={styles.fixChainRow}>
          {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={32} />}
          <View>
            <Text style={styles.fixChainName}>{net?.displayName ?? t('assets.chainFallback', { chainId })}</Text>
            <Text style={styles.fixChainSub}>{t('assets.rpcFixChainId', { chainId })}</Text>
          </View>
        </View>

        <View style={styles.fixWarning}>
          <Wifi size={14} color={'#C07A0A'} strokeWidth={2.5} />
          <Text style={styles.fixWarningText}>{t('assets.rpcFixWarning')}</Text>
        </View>

        <Text style={styles.fixLabel}>{t('assets.rpcFixLabel')}</Text>
        <TextInput
          style={styles.fixInput}
          value={fixUrl}
          onChangeText={setFixUrl}
          placeholder="https://rpc.example.com"
          placeholderTextColor={color.fg.subtle}
          autoCapitalize="none"
          autoCorrect={false}
          autoFocus
        />

        <Pressable
          style={[styles.fixBtn, saving && styles.fixBtnDisabled]}
          onPress={handleSave}
          disabled={saving || !fixUrl.trim()}
        >
          {saving
            ? <ActivityIndicator size={16} color={color.fg.inverse} />
            : <Text style={styles.fixBtnText}>{t('assets.rpcFixSaveBtn')}</Text>}
        </Pressable>

        {/* Where to get a reliable RPC */}
        <View style={styles.providers}>
          <Text style={styles.providersTitle}>{t('assets.rpcProvidersTitle')}</Text>
          <Text style={styles.providersHint}>{t('assets.rpcProvidersHint')}</Text>
          <View style={styles.providerChips}>
            {RPC_PROVIDERS.map(p => (
              <Pressable key={p.url} style={styles.providerChip} onPress={() => openBrowser(p.url)}>
                <Text style={styles.providerChipText}>{p.name}</Text>
                <ExternalLink size={12} color={color.fg.subtle} strokeWidth={2} />
              </Pressable>
            ))}
          </View>
        </View>

        {/* Last resort: reach the developer with this exact failure attached */}
        <Pressable
          style={styles.reportRow}
          onPress={() => openBrowser(buildBugReportURL(i18n.language as AppLanguage, {
            extraLines: [
              `- Failing network: ${net?.displayName ?? chainId} (chainId ${chainId})`,
              `- RPC entered: ${fixUrl || net?.rpcURL || 'n/a'}`,
            ],
          }))}
        >
          <Text style={styles.reportText}>{t('assets.rpcReport')}</Text>
          <ExternalLink size={12} color={color.fg.subtle} strokeWidth={2} />
        </Pressable>
      </ScrollView>
    </View>
  );
}

/**
 * RpcFixModal — RpcFixForm in its own AppModal. Used by the banner, where opening
 * the fix flow doesn't dismiss any other modal (so no present-during-dismiss race).
 */
export function RpcFixModal({
  chainId,
  onClose,
  onResolved,
}: {
  chainId: number | null;
  onClose: () => void;
  onResolved?: (chainId: number) => void;
}) {
  return (
    <AppModal visible={chainId !== null} onClose={onClose}>
      {chainId !== null && <RpcFixForm chainId={chainId} onClose={onClose} onResolved={onResolved} />}
    </AppModal>
  );
}

const styles = createStyles(() => ({
  // Banner
  banner: {
    flexDirection: 'row',
    gap: space.md,
    padding: space.lg,
    backgroundColor: color.warning.soft,
    borderRadius: radius.lg,
    borderWidth: 1,
    borderColor: color.warning.border,
    marginBottom: space.lg,
  },
  bannerContent: { flex: 1, gap: space.sm },
  bannerText: { fontSize: text.sm, ...inter.semibold, color: color.warning.base },
  bannerChips: { gap: space.sm },
  bannerChip: { flexDirection: 'row', alignItems: 'center', gap: space.sm, paddingVertical: space.xs },
  bannerChipText: { flex: 1, fontSize: text.sm, ...inter.medium, color: color.fg.base },
  bannerFixLink: { fontSize: text.sm, ...inter.semibold, color: color.accent.base },

  // Fix modal
  fixContainer: { flex: 1, backgroundColor: color.bg.base },
  fixHeader: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space['3xl'], paddingVertical: space.xl,
    borderBottomWidth: 1, borderBottomColor: color.border.base,
  },
  fixTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  fixBody: { padding: space['3xl'], gap: space.xl },
  fixChainRow: { flexDirection: 'row', alignItems: 'center', gap: space.lg },
  fixChainName: { fontSize: text.lg, ...inter.semibold, color: color.fg.base },
  fixChainSub: { fontSize: text.sm, ...inter.medium, color: color.fg.subtle },
  fixWarning: {
    flexDirection: 'row', gap: space.md, padding: space.lg,
    backgroundColor: color.warning.soft, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.warning.border,
  },
  fixWarningText: { flex: 1, fontSize: text.sm, ...inter.regular, color: color.warning.base, lineHeight: 18 },
  fixLabel: { fontSize: text.sm, ...inter.semibold, color: color.fg.base, textTransform: 'uppercase', letterSpacing: 0.5 },
  fixInput: {
    fontSize: text.base, ...inter.regular, color: color.fg.base,
    backgroundColor: color.bg.sunken, borderWidth: 1, borderColor: color.border.base,
    borderRadius: radius.lg, paddingHorizontal: space.lg, paddingVertical: space.lg,
    marginTop: -space.sm,
  },
  fixBtn: {
    alignItems: 'center', justifyContent: 'center',
    backgroundColor: color.accent.base, borderRadius: radius.lg, paddingVertical: space.lg, ...shadow.sm,
  },
  fixBtnDisabled: { opacity: 0.5 },
  fixBtnText: { fontSize: text.base, ...inter.semibold, color: color.fg.inverse },

  // Providers
  providers: { gap: space.sm, paddingTop: space.md, borderTopWidth: 1, borderTopColor: color.border.base },
  providersTitle: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  providersHint: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle, lineHeight: 18 },
  providerChips: { flexDirection: 'row', flexWrap: 'wrap', gap: space.sm, marginTop: space.xs },
  providerChip: {
    flexDirection: 'row', alignItems: 'center', gap: space.xs,
    paddingVertical: space.sm, paddingHorizontal: space.lg,
    backgroundColor: color.bg.sunken, borderRadius: radius.full,
    borderWidth: 1, borderColor: color.border.base,
  },
  providerChipText: { fontSize: text.sm, ...inter.medium, color: color.fg.base },

  reportRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.xs, paddingVertical: space.md },
  reportText: { fontSize: text.sm, ...inter.semibold, color: color.accent.base },
}));
