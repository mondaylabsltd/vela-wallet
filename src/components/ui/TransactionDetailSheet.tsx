/**
 * TransactionDetailSheet — tap an Activity row to see the full transaction.
 *
 * Hero amount + counterparty + a details card (date, status, from, to, operation,
 * chain, hash with explorer link). Built on AppModal; theme-driven. Populated
 * from the stored LocalTransaction; rows with no data are hidden.
 */
import React, { useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { Check, CheckCircle2, ChevronRight, Copy, ExternalLink, X, XCircle } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { VelaCard } from '@/components/ui/VelaCard';
import { ChainLogo } from '@/components/ChainLogo';
import { chainName, getAllNetworksSync } from '@/models/network';
import { formatBalance } from '@/models/types';
import type { LocalTransaction } from '@/services/storage';
import { shortAddress } from '@/models/wallet-state';
import { copyToClipboard, openBrowser } from '@/services/platform';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  tx: LocalTransaction | null;
  /** Resolved name for the counterparty, if known. */
  alias?: string;
  onClose: () => void;
}

function formatDateTime(ts: number): string {
  return new Date(ts * 1000).toLocaleString('en-US', {
    month: 'short', day: 'numeric', year: 'numeric', hour: '2-digit', minute: '2-digit',
  });
}

function operationLabel(tx: LocalTransaction): string {
  switch (tx.type) {
    case 'dapp_tx': return tx.intent || 'Contract interaction';
    case 'sign_message': return 'Signature';
    case 'sign_typed_data': return tx.intent || 'Typed-data signature';
    default: return 'Transfer';
  }
}

export function TransactionDetailSheet({ visible, tx, alias, onClose }: Props) {
  const [copied, setCopied] = useState<string | null>(null);

  const copy = (key: string, value: string) => {
    copyToClipboard(value);
    setCopied(key);
    setTimeout(() => setCopied((c) => (c === key ? null : c)), 1500);
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      {tx && (() => {
        const incoming = (tx.type ?? 'send') === 'receive';
        const net = getAllNetworksSync().find((n) => n.chainId === tx.chainId);
        const explorer = net?.explorerURL ?? 'https://etherscan.io';
        const amt = formatBalance(parseFloat(tx.value || '0'));
        const counterparty = incoming ? tx.from : tx.to;

        return (
          <View style={styles.sheet}>
            <View style={styles.head}>
              <View style={styles.headSpacer} />
              <Text style={styles.headTitle}>{incoming ? 'Received' : 'Sent'}</Text>
              <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn}>
                <X size={20} color={color.fg.base} strokeWidth={2} />
              </Pressable>
            </View>

            <ScrollView contentContainerStyle={styles.body} showsVerticalScrollIndicator={false}>
              {/* Hero amount */}
              <VelaCard elevated style={styles.hero}>
                <View style={styles.tokenWrap}>
                  <View style={styles.token}><Text style={styles.tokenText}>{tx.symbol.slice(0, 3)}</Text></View>
                  {net && (
                    <View style={styles.tokenBadge}>
                      <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={20} />
                    </View>
                  )}
                </View>
                <View style={styles.heroAmounts}>
                  <Text style={[styles.heroAmount, incoming && styles.heroAmountIn]} numberOfLines={1}>
                    {incoming ? '+' : '-'} {amt} {tx.symbol}
                  </Text>
                  <Text style={styles.heroUsd}>≈ {tx.usd ?? '$0.00'}</Text>
                </View>
                {tx.txHash ? (
                  <Pressable hitSlop={8} onPress={() => openBrowser(`${explorer}/tx/${tx.txHash}`)} style={styles.heroChevron}>
                    <ChevronRight size={20} color={color.fg.subtle} strokeWidth={2.4} />
                  </Pressable>
                ) : null}
              </VelaCard>

              {/* Counterparty */}
              {counterparty ? (
                <Pressable onPress={() => copy('cp', counterparty)} style={styles.cpCard}>
                  <Text style={styles.cpLabel}>{incoming ? 'From' : 'To'}</Text>
                  <View style={styles.cpRight}>
                    {alias ? <Text style={styles.cpAlias}>{alias}</Text> : null}
                    <View style={styles.cpAddrRow}>
                      <Text style={styles.cpShort}>{shortAddress(counterparty)}</Text>
                      {copied === 'cp' ? <Check size={15} color={color.success.base} strokeWidth={2.6} /> : <Copy size={15} color={color.fg.subtle} strokeWidth={2} />}
                    </View>
                    <Text style={styles.cpFull}>{counterparty}</Text>
                  </View>
                </Pressable>
              ) : null}

              {/* Details */}
              <Text style={styles.sectionTitle}>Transaction Details</Text>
              <VelaCard style={styles.details}>
                <Row label="Date" value={formatDateTime(tx.timestamp)} />
                <Divider />
                <Row label="Status" custom={
                  <View style={styles.statusRow}>
                    {tx.status === 'failed'
                      ? <XCircle size={16} color={color.error.base} strokeWidth={2.4} />
                      : <CheckCircle2 size={16} color={color.success.base} strokeWidth={2.4} />}
                    <Text style={[styles.statusText, { color: tx.status === 'failed' ? color.error.base : color.success.base }]}>
                      {tx.status === 'failed' ? 'Failed' : 'Succeeded'}
                    </Text>
                  </View>
                } />
                {tx.from ? (<><Divider /><Row label="From" value={shortAddress(tx.from)} mono onCopy={() => copy('from', tx.from)} copied={copied === 'from'} /></>) : null}
                {tx.to ? (<><Divider /><Row label="To" value={shortAddress(tx.to)} mono onOpen={() => openBrowser(`${explorer}/address/${tx.to}`)} /></>) : null}
                <Divider />
                <Row label="Operation" value={operationLabel(tx)} />
                <Divider />
                <Row label="Chain" custom={
                  <View style={styles.chainRow}>
                    {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={18} />}
                    <Text style={styles.chainText}>{chainName(tx.chainId)}</Text>
                  </View>
                } />
                {tx.txHash ? (<><Divider /><Row label="Hash" value={shortAddress(tx.txHash)} mono onOpen={() => openBrowser(`${explorer}/tx/${tx.txHash}`)} /></>) : null}
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
  headTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base },
  closeBtn: { width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center' },
  body: { paddingHorizontal: space['2xl'], paddingBottom: space['4xl'], gap: space.xl },

  // Hero
  hero: { flexDirection: 'row', alignItems: 'center', gap: space.lg, padding: space.xl },
  tokenWrap: { width: 52, height: 52 },
  token: { width: 52, height: 52, borderRadius: 26, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },
  tokenText: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  tokenBadge: { position: 'absolute', right: -2, bottom: -2, borderRadius: radius.full, borderWidth: 2, borderColor: color.bg.raised },
  heroAmounts: { flex: 1, gap: 2 },
  heroAmount: { fontSize: text['2xl'], ...inter.bold, fontFamily: font.display, color: color.fg.base },
  heroAmountIn: { color: color.success.base },
  heroUsd: { fontSize: text.base, ...inter.medium, color: color.fg.muted },
  heroChevron: { width: 36, height: 36, borderRadius: 18, backgroundColor: color.bg.sunken, alignItems: 'center', justifyContent: 'center' },

  // Counterparty
  cpCard: { backgroundColor: color.bg.sunken, borderRadius: radius.xl, padding: space.lg, gap: space.sm },
  cpLabel: { fontSize: text.base, ...inter.medium, color: color.fg.muted },
  cpRight: { gap: 2 },
  cpAlias: { fontSize: text.lg, ...inter.semibold, color: color.accent.base },
  cpAddrRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  cpShort: { fontSize: text.lg, ...inter.bold, color: color.fg.base, fontFamily: font.mono },
  cpFull: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle, fontFamily: font.mono },

  // Details
  sectionTitle: { fontSize: text.base, ...inter.bold, color: color.fg.base },
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
