/**
 * Activity re-scan sheet — the "I'm missing a payment" recovery path.
 *
 * Lets the user re-query event logs directly for a recent window (10m / 1h / 6h)
 * instead of waiting on the incremental monitor. Reports how many new receipts
 * were found and which chains couldn't be reached; the caller surfaces the
 * fix-RPC / explorer fallback for any failed chains via the home banner.
 */
import { AppModal } from '@/components/ui/AppModal';
import { color, createStyles, inter, radius, shadow, space, text } from '@/constants/theme';
import { chainName, getAllNetworksSync } from '@/models/network';
import { rescanRecentTransfers, type RescanOutcome } from '@/services/activity';
import { openBrowser } from '@/services/platform';
import { Check, ChevronRight, Clock, ExternalLink, Info, RefreshCw, X } from 'lucide-react-native';
import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, Pressable, ScrollView, Text, View } from 'react-native';

const WINDOWS: { minutes: number; key: string }[] = [
  { minutes: 10, key: 'home.rescanWindow10m' },
  { minutes: 60, key: 'home.rescanWindow1h' },
  { minutes: 360, key: 'home.rescanWindow6h' },
];

type Phase = 'choose' | 'scanning' | 'done';

export function ActivityRescanSheet({
  visible,
  address,
  onClose,
  onResult,
}: {
  visible: boolean;
  address: string;
  onClose: () => void;
  /** Reports the outcome so the caller can reload activity + show fix-RPC for failed chains. */
  onResult: (outcome: RescanOutcome) => void;
}) {
  const { t } = useTranslation();
  const [phase, setPhase] = useState<Phase>('choose');
  const [windowLabel, setWindowLabel] = useState('');
  const [result, setResult] = useState<RescanOutcome | null>(null);

  // Reset to the chooser each time the sheet opens.
  useEffect(() => {
    if (visible) { setPhase('choose'); setResult(null); setWindowLabel(''); }
  }, [visible]);

  const run = async (minutes: number, label: string) => {
    setWindowLabel(label);
    setPhase('scanning');
    const r = await rescanRecentTransfers(address, minutes).catch(
      () => ({ found: 0, okChains: [], failedChains: [] } as RescanOutcome),
    );
    setResult(r);
    setPhase('done');
    onResult(r);
  };

  const failedNames = (result?.failedChains ?? []).map(id => chainName(id)).join(', ');

  // Explorer fallback targets: the scanned chains, unreachable ones first. Shown
  // only when nothing turned up (likely a plain native send the log scan can't
  // see, or the wrong chain) or a chain couldn't be reached — a clean hit stays
  // uncluttered. Each row deep-links to the chain's own explorer address page.
  const explorerChains = result && (result.found === 0 || result.failedChains.length > 0)
    ? [...result.failedChains, ...result.okChains]
    : [];

  const openExplorer = (chainId: number) => {
    const base = getAllNetworksSync().find(n => n.chainId === chainId)?.explorerURL ?? 'https://etherscan.io';
    openBrowser(`${base}/address/${address}`);
  };
  const explorerHost = (chainId: number) => {
    const base = getAllNetworksSync().find(n => n.chainId === chainId)?.explorerURL ?? 'https://etherscan.io';
    return base.replace(/^https?:\/\//, '').replace(/\/$/, '');
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.container}>
        <View style={styles.header}>
          <View style={styles.headerIcon}><RefreshCw size={18} color={color.accent.base} strokeWidth={2.4} /></View>
          <Text style={styles.title}>{t('home.rescanTitle')}</Text>
          <Pressable onPress={onClose} hitSlop={8}><X size={22} color={color.fg.base} strokeWidth={2} /></Pressable>
        </View>

        <View style={styles.body}>
          {phase === 'choose' && (
            <>
              <Text style={styles.subtitle}>{t('home.rescanSubtitle')}</Text>
              {WINDOWS.map(w => (
                <Pressable key={w.minutes} style={styles.windowRow} onPress={() => run(w.minutes, t(w.key as any))}>
                  <Clock size={18} color={color.fg.muted} strokeWidth={2.2} />
                  <Text style={styles.windowText}>{t(w.key as any)}</Text>
                  <ChevronRight size={18} color={color.fg.subtle} strokeWidth={2.4} />
                </Pressable>
              ))}
            </>
          )}

          {phase === 'scanning' && (
            <View style={styles.center}>
              <ActivityIndicator size="large" color={color.accent.base} />
              <Text style={styles.scanningText}>{t('home.rescanScanning', { window: windowLabel })}</Text>
            </View>
          )}

          {phase === 'done' && result && (
            <>
              <View style={styles.center}>
                <View style={[styles.resultIcon, result.found > 0 && styles.resultIconOk]}>
                  {result.found > 0
                    ? <Check size={26} color={color.success.base} strokeWidth={2.6} />
                    : <RefreshCw size={24} color={color.fg.muted} strokeWidth={2.2} />}
                </View>
                <Text style={styles.resultText}>
                  {result.found > 0
                    ? t('home.rescanFound', { count: result.found })
                    : t('home.rescanNone', { window: windowLabel })}
                </Text>
              </View>

              {result.failedChains.length > 0 && (
                <View style={styles.failedNote}>
                  <Text style={styles.failedText}>{t('home.rescanFailed', { names: failedNames })}</Text>
                </View>
              )}

              {/* Honest note: native (gas-coin) receipts can't be detected on
                  chains without EIP-7708 — reassure the balance is still right
                  and point to the explorer rather than silently showing nothing. */}
              <View style={styles.nativeNote}>
                <Info size={14} color={color.fg.subtle} strokeWidth={2.2} />
                <Text style={styles.nativeNoteText}>{t('home.rescanNativeNote')}</Text>
              </View>

              {/* Actionable fallback: jump straight to each chain's own block
                  explorer (authoritative, shows plain native sends the scan can't). */}
              {explorerChains.length > 0 && (
                <View style={styles.explorerSection}>
                  <Text style={styles.explorerTitle}>{t('home.rescanExplorerTitle')}</Text>
                  <ScrollView style={styles.explorerList} contentContainerStyle={styles.explorerListContent} showsVerticalScrollIndicator={false}>
                    {explorerChains.map(id => (
                      <Pressable key={id} style={styles.explorerRow} onPress={() => openExplorer(id)}>
                        <Text style={styles.explorerChain} numberOfLines={1}>{chainName(id)}</Text>
                        <Text style={styles.explorerHost} numberOfLines={1}>{explorerHost(id)}</Text>
                        <ExternalLink size={16} color={color.fg.subtle} strokeWidth={2.2} />
                      </Pressable>
                    ))}
                  </ScrollView>
                </View>
              )}

              <View style={styles.actions}>
                <Pressable style={styles.secondaryBtn} onPress={() => setPhase('choose')}>
                  <Text style={styles.secondaryBtnText}>{t('home.rescanAgain')}</Text>
                </Pressable>
                <Pressable style={styles.primaryBtn} onPress={onClose}>
                  <Text style={styles.primaryBtnText}>{t('home.rescanClose')}</Text>
                </Pressable>
              </View>
            </>
          )}
        </View>
      </View>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  container: { flex: 1, backgroundColor: color.bg.base },
  header: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    paddingHorizontal: space['3xl'], paddingVertical: space.xl,
    borderBottomWidth: 1, borderBottomColor: color.border.base,
  },
  headerIcon: {
    width: 34, height: 34, borderRadius: 11, backgroundColor: color.accent.soft,
    alignItems: 'center', justifyContent: 'center',
  },
  title: { flex: 1, fontSize: text.xl, ...inter.bold, color: color.fg.base },
  body: { padding: space['3xl'], gap: space.lg },
  subtitle: { fontSize: text.base, ...inter.regular, color: color.fg.muted, lineHeight: 20 },

  windowRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    padding: space.xl, backgroundColor: color.bg.raised,
    borderRadius: radius.xl, borderWidth: 1, borderColor: color.border.base, ...shadow.sm,
  },
  windowText: { flex: 1, fontSize: text.lg, ...inter.semibold, color: color.fg.base },

  center: { alignItems: 'center', gap: space.lg, paddingVertical: space['3xl'] },
  scanningText: { fontSize: text.base, ...inter.medium, color: color.fg.muted },

  resultIcon: {
    width: 56, height: 56, borderRadius: 28, backgroundColor: color.bg.sunken,
    alignItems: 'center', justifyContent: 'center',
  },
  resultIconOk: { backgroundColor: color.success.soft },
  resultText: { fontSize: text.lg, ...inter.semibold, color: color.fg.base, textAlign: 'center', paddingHorizontal: space.xl },

  failedNote: {
    padding: space.lg, backgroundColor: color.warning.soft,
    borderRadius: radius.lg, borderWidth: 1, borderColor: color.warning.border,
  },
  failedText: { fontSize: text.sm, ...inter.medium, color: color.warning.base, lineHeight: 18 },

  nativeNote: {
    flexDirection: 'row', gap: space.sm, padding: space.lg,
    backgroundColor: color.bg.sunken, borderRadius: radius.lg,
  },
  nativeNoteText: { flex: 1, fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 18 },

  explorerSection: { gap: space.sm },
  explorerTitle: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  explorerList: { maxHeight: 220 },
  explorerListContent: { gap: space.sm },
  explorerRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.bg.raised, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base,
  },
  explorerChain: { flex: 1, fontSize: text.base, ...inter.semibold, color: color.fg.base },
  explorerHost: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle },

  actions: { flexDirection: 'row', gap: space.md, marginTop: space.sm },
  secondaryBtn: {
    flex: 1, alignItems: 'center', paddingVertical: space.lg, borderRadius: radius.lg,
    borderWidth: 1, borderColor: color.border.base, backgroundColor: color.bg.raised,
  },
  secondaryBtnText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  primaryBtn: {
    flex: 1, alignItems: 'center', paddingVertical: space.lg, borderRadius: radius.lg,
    backgroundColor: color.accent.base, ...shadow.sm,
  },
  primaryBtnText: { fontSize: text.base, ...inter.semibold, color: color.fg.inverse },
}));
