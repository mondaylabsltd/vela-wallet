/**
 * TransactionDetailSheet — tap an Activity row to see the full transaction.
 *
 * Hero amount + counterparty + a details card (date, status, from, to, operation,
 * chain, hash with explorer link). Built on AppModal; theme-driven. Populated
 * from the stored LocalTransaction; rows with no data are hidden.
 */
import React, { useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Check, ChevronRight, Copy, X } from 'lucide-react-native';
import { AmountText } from '@/components/ui/AmountText';
import { AppModal } from '@/components/ui/AppModal';
import { DetailRow as Row, Divider } from '@/components/ui/DetailRow';
import { TxStatusBadge } from '@/components/ui/TxStatusBadge';
import { VelaCard } from '@/components/ui/VelaCard';
import { ChainLogo } from '@/components/ChainLogo';
import { chainName, getAllNetworksSync, explorerTxURL, explorerAddressURL } from '@/models/network';
import type { LocalTransaction } from '@/services/storage';
import { shortAddress } from '@/models/wallet-state';
import { copyToClipboard, openBrowser } from '@/services/platform';
import { formatFiat, type Currency } from '@/services/currency';
import { txUsdValue } from '@/services/activity';
import { formatDateTime, formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  tx: LocalTransaction | null;
  /** Resolved name for the counterparty, if known. */
  alias?: string;
  /** USD → display-currency rate and the chosen currency, for the fiat estimate. */
  rate: number;
  currency: Currency;
  onClose: () => void;
}

/** Short token glyph for the hero circle — full symbol up to 4 chars (avoids "USDT" → "USD"). */
function tokenGlyph(symbol: string): string {
  return symbol.length <= 4 ? symbol : symbol.slice(0, 4);
}

/** Returns a translation key (or raw intent string) for the operation label. */
function operationLabelKey(tx: LocalTransaction): string {
  switch (tx.type) {
    case 'dapp_tx': return tx.intent || 'componentsTx.detail.opContractInteraction';
    case 'sign_message': return 'componentsTx.detail.opSignature';
    case 'sign_typed_data': return tx.intent || 'componentsTx.detail.opTypedDataSignature';
    default: return 'componentsTx.detail.opTransfer';
  }
}

export function TransactionDetailSheet({ visible, tx, alias, rate, currency, onClose }: Props) {
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
        const incoming = (tx.type ?? 'send') === 'receive';
        const net = getAllNetworksSync().find((n) => n.chainId === tx.chainId);
        const amt = formatTokenAmount(parseFloat(tx.value || '0'), { compact: true }); // match the feed
        const counterparty = incoming ? tx.from : tx.to;
        const usdVal = txUsdValue(tx);
        const fiat = usdVal > 0 ? formatFiat(usdVal * rate, currency.code, currency.symbol) : null;

        return (
          <View style={styles.sheet}>
            <View style={styles.head}>
              <View style={styles.headSpacer} />
              <Text style={styles.headTitle}>{incoming ? t('componentsTx.detail.received') : t('componentsTx.detail.sent')}</Text>
              <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn}>
                <X size={20} color={color.fg.base} strokeWidth={2} />
              </Pressable>
            </View>

            <ScrollView contentContainerStyle={styles.body} showsVerticalScrollIndicator={false}>
              {/* Hero amount */}
              <VelaCard elevated style={styles.hero}>
                <View style={styles.tokenWrap}>
                  <View style={styles.token}><Text style={styles.tokenText} numberOfLines={1}>{tokenGlyph(tx.symbol)}</Text></View>
                  {net && (
                    <View style={styles.tokenBadge}>
                      <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={20} />
                    </View>
                  )}
                </View>
                <View style={styles.heroAmounts}>
                  <AmountText
                    text={`${incoming ? '+' : '-'} ${amt}`}
                    unit={tx.symbol}
                    size={text['2xl']}
                    minScale={0.55}
                    tailScale={0.62}
                    style={[styles.heroAmount, incoming && styles.heroAmountIn]}
                    tailStyle={styles.heroUnit}
                  />
                  {fiat ? <Text style={styles.heroUsd}>≈ {fiat}</Text> : null}
                </View>
                {tx.txHash ? (
                  <Pressable hitSlop={8} onPress={() => openBrowser(explorerTxURL(tx.chainId, tx.txHash))} style={styles.heroChevron}>
                    <ChevronRight size={20} color={color.fg.subtle} strokeWidth={2.4} />
                  </Pressable>
                ) : null}
              </VelaCard>

              {/* Counterparty (the "who" — shown once here, not repeated in Details) */}
              {counterparty ? (
                <Pressable onPress={() => copy('cp', counterparty)} style={styles.cpCard}>
                  <Text style={styles.cpLabel}>{incoming ? t('componentsTx.detail.from') : t('componentsTx.detail.to')}</Text>
                  <View style={styles.cpRight}>
                    {alias ? <Text style={styles.cpAlias} numberOfLines={1}>{alias}</Text> : null}
                    <View style={styles.cpAddrRow}>
                      <Text style={styles.cpShort}>{shortAddress(counterparty)}</Text>
                      {copied === 'cp' ? <Check size={15} color={color.success.base} strokeWidth={2.6} /> : <Copy size={15} color={color.fg.subtle} strokeWidth={2} />}
                    </View>
                  </View>
                </Pressable>
              ) : null}

              {/* Details */}
              <Text style={styles.sectionTitle}>{t('componentsTx.detail.sectionTitle')}</Text>
              <VelaCard style={styles.details}>
                <Row label={t('componentsTx.detail.labelDate')} value={formatDateTime(tx.timestamp * 1000)} />
                <Divider />
                <Row label={t('componentsTx.detail.labelStatus')} custom={<TxStatusBadge status={tx.status} />} />
                {tx.from ? (<><Divider /><Row label={t('componentsTx.detail.labelFrom')} value={shortAddress(tx.from)} mono onCopy={() => copy('from', tx.from)} copied={copied === 'from'} /></>) : null}
                {tx.to ? (<><Divider /><Row label={t('componentsTx.detail.labelTo')} value={shortAddress(tx.to)} mono onOpen={() => openBrowser(explorerAddressURL(tx.chainId, tx.to))} /></>) : null}
                <Divider />
                <Row label={t('componentsTx.detail.labelOperation')} value={t(operationLabelKey(tx), { defaultValue: operationLabelKey(tx) })} />
                <Divider />
                <Row label={t('componentsTx.detail.labelChain')} custom={
                  <View style={styles.chainRow}>
                    {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={18} />}
                    <Text style={styles.chainText}>{chainName(tx.chainId)}</Text>
                  </View>
                } />
                {tx.txHash ? (<><Divider /><Row label={t('componentsTx.detail.labelHash')} value={shortAddress(tx.txHash)} mono onOpen={() => openBrowser(explorerTxURL(tx.chainId, tx.txHash))} /></>) : null}
              </VelaCard>
            </ScrollView>
          </View>
        );
      })()}
    </AppModal>
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
  heroUnit: { color: color.fg.muted }, // ticker subordinated next to the amount
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
  statusRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  statusText: { fontSize: text.base, ...inter.bold },
  chainRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  chainText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
}));
