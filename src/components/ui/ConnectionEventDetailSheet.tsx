/**
 * ConnectionEventDetailSheet — tap a signing record in the Connections panel to
 * see exactly what was authorized. Handles all three dApp event kinds:
 *   - sign_message     → the decoded message text
 *   - sign_typed_data  → the typed-data JSON
 *   - dapp_tx          → value, recipient, calldata, on-chain hash + explorer
 *
 * IA, top to bottom: identity first (which app, what operation), then the signed
 * content (the "what did I authorize"), then the metadata trail (date / status /
 * addresses / hash). Built on AppModal; rows with no data are hidden.
 */
import React, { useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import {
  ArrowLeftRight, Check, Copy, ExternalLink, FileText, Globe, PenLine, X,
} from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { TxStatusBadge } from '@/components/ui/TxStatusBadge';
import { VelaCard } from '@/components/ui/VelaCard';
import { ChainLogo } from '@/components/ChainLogo';
import { chainName, getAllNetworksSync } from '@/models/network';
import type { LocalTransaction } from '@/services/storage';
import { shortAddress } from '@/models/wallet-state';
import { copyToClipboard, openBrowser } from '@/services/platform';
import { formatDateTime, formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  tx: LocalTransaction | null;
  onClose: () => void;
}

type Kind = 'message' | 'typed' | 'tx';

function kindOf(tx: LocalTransaction): Kind {
  if (tx.type === 'sign_message') return 'message';
  if (tx.type === 'sign_typed_data') return 'typed';
  return 'tx';
}

/** Operation title — reuses the canonical operation labels. */
function titleKey(tx: LocalTransaction, kind: Kind): string {
  if (kind === 'message') return 'componentsTx.detail.opSignature';
  if (kind === 'typed') return tx.intent || 'componentsTx.detail.opTypedDataSignature';
  return tx.intent || 'componentsTx.detail.opContractInteraction';
}

/** Native value (hex wei for dApp txs) → "0.5 ETH"; empty string when zero. */
function formatTxValue(value: string | undefined, symbol: string): string {
  try {
    const wei = value && value !== '0x' ? BigInt(value) : 0n;
    if (wei === 0n) return '';
    const eth = Number(wei) / 1e18;
    const s = eth > 0 && eth < 0.0001 ? '< 0.0001' : formatTokenAmount(eth);
    return `${s} ${symbol}`.trim();
  } catch {
    return '';
  }
}

export function ConnectionEventDetailSheet({ visible, tx, onClose }: Props) {
  const { t } = useTranslation();
  useLocalePrefs(); // re-render when number/date/time format changes
  const [copied, setCopied] = useState<string | null>(null);

  const copy = (key: string, value: string) => {
    copyToClipboard(value);
    setCopied(key);
    setTimeout(() => setCopied((c) => (c === key ? null : c)), 1500);
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      {tx && (() => {
        const kind = kindOf(tx);
        const net = getAllNetworksSync().find((n) => n.chainId === tx.chainId);
        const explorer = net?.explorerURL ?? 'https://etherscan.io';
        const title = t(titleKey(tx, kind), { defaultValue: titleKey(tx, kind) });
        const amount = kind === 'tx' ? formatTxValue(tx.value, tx.symbol) : '';
        const HeroIcon = kind === 'message' ? PenLine : kind === 'typed' ? FileText : ArrowLeftRight;
        const contentLabel = kind === 'message'
          ? t('connect.detail.contentMessage')
          : kind === 'typed'
            ? t('connect.detail.contentTypedData')
            : t('connect.detail.contentCallData');
        const offChain = kind !== 'tx';

        return (
          <View style={styles.sheet}>
            <View style={styles.head}>
              <View style={styles.headSpacer} />
              <Text style={styles.headTitle} numberOfLines={1}>{title}</Text>
              <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn}>
                <X size={20} color={color.fg.base} strokeWidth={2} />
              </Pressable>
            </View>

            <ScrollView contentContainerStyle={styles.body} showsVerticalScrollIndicator={false}>
              {/* Identity — which app, what operation */}
              <VelaCard elevated style={styles.hero}>
                <View style={styles.heroIcon}><HeroIcon size={22} color={color.accent.base} strokeWidth={2} /></View>
                <View style={styles.heroInfo}>
                  <Text style={styles.heroTitle} numberOfLines={1}>{title}</Text>
                  <View style={styles.heroDappRow}>
                    <Globe size={13} color={color.fg.muted} strokeWidth={2} />
                    <Text style={styles.heroDapp} numberOfLines={1}>
                      {tx.dappOrigin || t('connect.detail.labelApp')}
                    </Text>
                  </View>
                </View>
                {amount ? <Text style={styles.heroAmount} numberOfLines={1}>{amount}</Text> : null}
              </VelaCard>

              {offChain ? (
                <Text style={styles.offChainNote}>{t('connect.detail.offChainNote')}</Text>
              ) : null}

              {/* Signed content — the "what did I authorize" */}
              <Text style={styles.sectionTitle}>{contentLabel}</Text>
              <VelaCard style={styles.contentCard}>
                {tx.signedContent ? (
                  <>
                    <ScrollView style={styles.contentScroll} nestedScrollEnabled showsVerticalScrollIndicator>
                      <Text style={styles.contentText} selectable>{tx.signedContent}</Text>
                    </ScrollView>
                    <Pressable onPress={() => copy('content', tx.signedContent!)} hitSlop={8} style={styles.copyBtn}>
                      {copied === 'content'
                        ? <Check size={16} color={color.success.base} strokeWidth={2.6} />
                        : <Copy size={16} color={color.fg.subtle} strokeWidth={2} />}
                    </Pressable>
                  </>
                ) : (
                  <Text style={styles.contentMissing}>{t('connect.detail.contentMissing')}</Text>
                )}
              </VelaCard>

              {/* Metadata trail */}
              <Text style={styles.sectionTitle}>{t('componentsTx.detail.sectionTitle')}</Text>
              <VelaCard style={styles.details}>
                <Row label={t('connect.detail.labelApp')} value={tx.dappOrigin || '—'} />
                <Divider />
                <Row label={t('componentsTx.detail.labelDate')} value={formatDateTime(tx.timestamp * 1000)} />
                <Divider />
                <Row label={t('componentsTx.detail.labelStatus')} custom={<TxStatusBadge status={tx.status} />} />
                <Divider />
                <Row label={t('componentsTx.detail.labelOperation')} value={title} />
                <Divider />
                <Row label={t('componentsTx.detail.labelChain')} custom={
                  <View style={styles.chainRow}>
                    {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={18} />}
                    <Text style={styles.chainText}>{chainName(tx.chainId)}</Text>
                  </View>
                } />
                {tx.from ? (<><Divider /><Row label={t('componentsTx.detail.labelFrom')} value={shortAddress(tx.from)} mono onCopy={() => copy('from', tx.from)} copied={copied === 'from'} /></>) : null}
                {kind === 'tx' && tx.to ? (<><Divider /><Row label={t('componentsTx.detail.labelTo')} value={shortAddress(tx.to)} mono onOpen={() => openBrowser(`${explorer}/address/${tx.to}`)} /></>) : null}
                {amount ? (<><Divider /><Row label={t('connect.dapp.detailValue')} value={amount} /></>) : null}
                {tx.txHash ? (<><Divider /><Row label={t('componentsTx.detail.labelHash')} value={shortAddress(tx.txHash)} mono onOpen={() => openBrowser(`${explorer}/tx/${tx.txHash}`)} /></>) : null}
              </VelaCard>
            </ScrollView>
          </View>
        );
      })()}
    </AppModal>
  );
}

function Divider() { return <View style={styles.divider} />; }

function Row({ label, value, custom, mono, onCopy, onOpen, copied }: {
  label: string; value?: string; custom?: React.ReactNode; mono?: boolean;
  onCopy?: () => void; onOpen?: () => void; copied?: boolean;
}) {
  return (
    <View style={styles.row}>
      <Text style={styles.rowLabel}>{label}</Text>
      {custom ?? (
        <Pressable style={styles.rowValueWrap} onPress={onOpen ?? onCopy} disabled={!onOpen && !onCopy} hitSlop={6}>
          <Text style={[styles.rowValue, mono && styles.rowValueMono]} numberOfLines={1}>{value}</Text>
          {onCopy ? (copied ? <Check size={14} color={color.success.base} strokeWidth={2.6} /> : <Copy size={14} color={color.fg.subtle} strokeWidth={2} />) : null}
          {onOpen ? <ExternalLink size={14} color={color.fg.subtle} strokeWidth={2} /> : null}
        </Pressable>
      )}
    </View>
  );
}

const styles = createStyles(() => ({
  sheet: { flex: 1, backgroundColor: color.bg.base },
  head: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space['2xl'], paddingVertical: space.md,
  },
  headSpacer: { width: 34 },
  headTitle: { flex: 1, textAlign: 'center', fontSize: text.xl, ...inter.bold, color: color.fg.base, paddingHorizontal: space.sm },
  closeBtn: { width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center' },
  body: { paddingHorizontal: space['2xl'], paddingBottom: space['4xl'], gap: space.xl },

  // Identity
  hero: { flexDirection: 'row', alignItems: 'center', gap: space.lg, padding: space.xl },
  heroIcon: { width: 44, height: 44, borderRadius: 22, backgroundColor: color.accent.soft, alignItems: 'center', justifyContent: 'center' },
  heroInfo: { flex: 1, gap: 3 },
  heroTitle: { fontSize: text.lg, ...inter.bold, color: color.fg.base },
  heroDappRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  heroDapp: { fontSize: text.sm, ...inter.medium, color: color.fg.muted, flex: 1 },
  heroAmount: { fontSize: text.lg, ...inter.bold, fontFamily: font.display, color: color.fg.base },

  offChainNote: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, lineHeight: 18, marginTop: -space.md },

  // Signed content
  sectionTitle: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  contentCard: { padding: space.lg, gap: space.md },
  contentScroll: { maxHeight: 220 },
  contentText: { fontSize: text.sm, fontFamily: font.mono, color: color.fg.base, lineHeight: 19 },
  contentMissing: { fontSize: text.base, ...inter.regular, color: color.fg.subtle },
  copyBtn: { alignSelf: 'flex-end', width: 32, height: 32, borderRadius: radius.md, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },

  // Details
  details: { paddingHorizontal: space.lg },
  row: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: space.lg, paddingVertical: space.lg },
  rowLabel: { fontSize: text.base, ...inter.regular, color: color.fg.muted },
  rowValueWrap: { flexDirection: 'row', alignItems: 'center', gap: space.sm, flexShrink: 1 },
  rowValue: { fontSize: text.base, ...inter.semibold, color: color.fg.base, flexShrink: 1 },
  rowValueMono: { fontFamily: font.mono },
  divider: { height: 1, backgroundColor: color.border.base },
  statusRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  statusText: { fontSize: text.base, ...inter.bold },
  chainRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  chainText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
}));
