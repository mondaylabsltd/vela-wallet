/**
 * TransactionDetailSheet — tap an Activity row to see the full transaction.
 *
 * Hero amount + counterparty + a details card (date, status, from, to, operation,
 * chain, hash with explorer link). Built on AppModal; theme-driven. Populated
 * from the stored LocalTransaction; rows with no data are hidden.
 */
import React, { useEffect, useState } from 'react';
import { Pressable, ScrollView, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Check, ChevronRight, Copy, X } from 'lucide-react-native';
import { AmountText } from '@/components/ui/AmountText';
import { AppModal } from '@/components/ui/AppModal';
import { DetailRow as Row, Divider } from '@/components/ui/DetailRow';
import { TxStatusBadge } from '@/components/ui/TxStatusBadge';
import { VelaCard } from '@/components/ui/VelaCard';
import { ChainLogo } from '@/components/ChainLogo';
import { TokenLogo } from '@/components/TokenLogo';
import { chainName, getAllNetworksSync, explorerTxURL, explorerAddressURL } from '@/models/network';
import { updateTransaction, type LocalTransaction } from '@/services/storage';
import { shortAddress } from '@/models/wallet-state';
import { copyToClipboard, openBrowser } from '@/services/platform';
import { formatFiat, type Currency } from '@/services/currency';
import { txUsdValue, type ActivityBatch } from '@/services/activity';
import { pollUserOpReceipt } from '@/services/tx-reconciler';
import { formatDateTime, formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import { color, createStyles, font, inter, radius, shadow, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  tx: LocalTransaction | null;
  /** Set instead of `tx` for a batch send (split / multiSelect) — renders a breakdown. */
  batch?: ActivityBatch | null;
  /** Resolved name for the counterparty, if known. */
  alias?: string;
  /** USD → display-currency rate and the chosen currency, for the fiat estimate. */
  rate: number;
  currency: Currency;
  onClose: () => void;
  /** Called when a still-pending tx resolves (confirmed/failed) so the feed refreshes. */
  onResolved?: () => void;
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

export function TransactionDetailSheet({ visible, tx, batch, alias, rate, currency, onClose, onResolved }: Props) {
  const { t } = useTranslation();
  useLocalePrefs(); // re-render when number/date/time format changes
  const [copied, setCopied] = useState<string | null>(null);

  // A transaction the user opens may still be 'pending'. Rather than show a stale
  // badge until the next Home reconcile sweep, poll the bundler while the sheet is
  // open and converge the displayed status live (and persist it so the feed agrees).
  const [liveStatus, setLiveStatus] = useState<LocalTransaction['status'] | null>(null);
  const [liveTxHash, setLiveTxHash] = useState<string | null>(null);
  const targetKey = batch ? `b:${batch.userOpHash}` : tx ? `t:${tx.id}` : '';

  useEffect(() => {
    setLiveStatus(null);
    setLiveTxHash(null);
    if (!visible) return;
    const userOpHash = batch?.userOpHash || tx?.userOpHash;
    const chainId = batch?.chainId ?? tx?.chainId;
    const baseStatus = batch?.status ?? tx?.status;
    const ids = batch?.ids ?? (tx ? [tx.id] : []);
    if (!userOpHash || !chainId || baseStatus !== 'pending') return;

    let cancelled = false;
    let timer: ReturnType<typeof setTimeout>;
    let attempts = 0;
    const tick = async () => {
      if (cancelled) return;
      attempts++;
      const r = await pollUserOpReceipt(userOpHash, chainId);
      if (cancelled) return;
      if (r && (r.confirmed || r.failed)) {
        const next: LocalTransaction['status'] = r.failed ? 'failed' : 'confirmed';
        setLiveStatus(next);
        if (r.txHash) setLiveTxHash(r.txHash);
        await Promise.all(ids.map((id) => updateTransaction(id, { status: next, ...(r.txHash ? { txHash: r.txHash } : {}) }).catch(() => {})));
        onResolved?.();
        return; // final — stop polling
      }
      if (attempts < 40) timer = setTimeout(tick, 4000);
    };
    timer = setTimeout(tick, 1200);
    return () => { cancelled = true; clearTimeout(timer); };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [visible, targetKey]);

  const effStatus: LocalTransaction['status'] = liveStatus ?? batch?.status ?? tx?.status ?? 'pending';
  const effTxHash = liveTxHash ?? batch?.txHash ?? tx?.txHash ?? '';

  const copy = (key: string, value: string) => {
    copyToClipboard(value);
    setCopied(key);
    setTimeout(() => setCopied((c) => (c === key ? null : c)), 1500);
  };

  return (
    <AppModal visible={visible} onClose={onClose}>
      {batch ? (() => {
        const net = getAllNetworksSync().find((n) => n.chainId === batch.chainId);
        const isSplit = batch.kind === 'split';
        const splitTotal = isSplit
          ? formatTokenAmount(batch.transfers.reduce((s, x) => s + (parseFloat(x.value) || 0), 0), { compact: true })
          : '';
        const fiat = batch.totalUsd > 0 ? formatFiat(batch.totalUsd * rate, currency.code, currency.symbol) : null;

        return (
          <View style={styles.sheet}>
            <View style={styles.head}>
              <View style={styles.headSpacer} />
              <Text style={styles.headTitle}>{t('componentsTx.detail.sent')}</Text>
              <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn}>
                <X size={20} color={color.fg.base} strokeWidth={2} />
              </Pressable>
            </View>

            <ScrollView contentContainerStyle={styles.body} showsVerticalScrollIndicator={false}>
              {/* Hero: split = one-token total; multiSelect = asset count + fiat total */}
              <VelaCard elevated style={styles.hero}>
                {isSplit ? (
                  <TokenLogo symbol={batch.symbol ?? ''} logoUrls={batch.logoUrls} chain={net ?? null} size={52} />
                ) : (
                  <View style={styles.tokenWrap}>
                    <View style={styles.token}>
                      <Text style={styles.tokenText} numberOfLines={1}>{String(batch.count)}</Text>
                    </View>
                    {net && (
                      <View style={styles.tokenBadge}>
                        <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={20} />
                      </View>
                    )}
                  </View>
                )}
                <View style={styles.heroAmounts}>
                  {isSplit ? (
                    <AmountText
                      text={`- ${splitTotal}`}
                      unit={batch.symbol ?? ''}
                      size={text['2xl']}
                      minScale={0.55}
                      tailScale={0.62}
                      style={styles.heroAmount}
                      tailStyle={styles.heroUnit}
                    />
                  ) : (
                    <Text style={styles.heroAmount} numberOfLines={1}>{t('componentsTx.receipt.assetsCount', { n: batch.count })}</Text>
                  )}
                  {fiat ? <Text style={styles.heroUsd}>≈ {fiat}</Text> : null}
                </View>
                {effTxHash ? (
                  <Pressable hitSlop={8} onPress={() => openBrowser(explorerTxURL(batch.chainId, effTxHash))} style={styles.heroChevron}>
                    <ChevronRight size={20} color={color.fg.subtle} strokeWidth={2.4} />
                  </Pressable>
                ) : null}
              </VelaCard>

              {/* Counterparty: split → recipient count; multiSelect → the one recipient */}
              {isSplit ? (
                <View style={styles.cpCard}>
                  <Text style={styles.cpLabel}>{t('componentsTx.detail.to')}</Text>
                  <View style={styles.cpRight}>
                    <Text style={styles.cpShort}>{t('componentsTx.receipt.recipientsCount', { n: batch.count })}</Text>
                  </View>
                </View>
              ) : batch.to ? (
                <Pressable onPress={() => copy('cp', batch.to!)} style={styles.cpCard}>
                  <Text style={styles.cpLabel}>{t('componentsTx.detail.to')}</Text>
                  <View style={styles.cpRight}>
                    {batch.toName ? <Text style={styles.cpAlias} numberOfLines={1}>{batch.toName}</Text> : null}
                    <View style={styles.cpAddrRow}>
                      <Text style={styles.cpShort}>{shortAddress(batch.to)}</Text>
                      {copied === 'cp' ? <Check size={15} color={color.success.base} strokeWidth={2.6} /> : <Copy size={15} color={color.fg.subtle} strokeWidth={2} />}
                    </View>
                  </View>
                </Pressable>
              ) : null}

              {/* Per-line breakdown — split rows are recipients, multiSelect rows are tokens */}
              <Text style={styles.sectionTitle}>{t('componentsTx.detail.breakdownTitle')}</Text>
              <VelaCard style={styles.details}>
                {batch.transfers.map((it, i) => {
                  const amt = formatTokenAmount(parseFloat(it.value || '0'), { compact: true });
                  const lineFiat = it.usdValue > 0 ? formatFiat(it.usdValue * rate, currency.code, currency.symbol) : null;
                  return (
                    <React.Fragment key={i}>
                      {i > 0 ? <Divider /> : null}
                      <View style={styles.brRow}>
                        {isSplit ? (
                          <View style={styles.brLeft}>
                            <Text style={styles.brPrimary} numberOfLines={1}>{it.toName || shortAddress(it.to)}</Text>
                            <Text style={styles.brSub} numberOfLines={1}>{shortAddress(it.to)}</Text>
                          </View>
                        ) : (
                          <View style={styles.brTokenLeft}>
                            <TokenLogo symbol={it.symbol} logoUrls={it.logoUrls} size={28} />
                            <Text style={styles.brPrimary} numberOfLines={1}>{it.symbol}</Text>
                          </View>
                        )}
                        <View style={styles.brRight}>
                          <Text style={styles.brAmount} numberOfLines={1}>- {amt} {it.symbol}</Text>
                          {lineFiat ? <Text style={styles.brSub}>{lineFiat}</Text> : null}
                        </View>
                      </View>
                    </React.Fragment>
                  );
                })}
              </VelaCard>

              {/* Details */}
              <Text style={styles.sectionTitle}>{t('componentsTx.detail.sectionTitle')}</Text>
              <VelaCard style={styles.details}>
                <Row label={t('componentsTx.detail.labelDate')} value={formatDateTime(batch.timestamp * 1000)} />
                <Divider />
                <Row label={t('componentsTx.detail.labelStatus')} custom={<TxStatusBadge status={effStatus} />} />
                {batch.from ? (<><Divider /><Row label={t('componentsTx.detail.labelFrom')} value={shortAddress(batch.from)} mono onCopy={() => copy('from', batch.from)} copied={copied === 'from'} /></>) : null}
                {!isSplit && batch.to ? (<><Divider /><Row label={t('componentsTx.detail.labelTo')} value={shortAddress(batch.to)} mono onOpen={() => openBrowser(explorerAddressURL(batch.chainId, batch.to!))} /></>) : null}
                <Divider />
                <Row label={t('componentsTx.detail.labelOperation')} value={t('componentsTx.detail.opTransfer')} />
                <Divider />
                <Row label={t('componentsTx.detail.labelChain')} custom={
                  <View style={styles.chainRow}>
                    {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={18} />}
                    <Text style={styles.chainText}>{chainName(batch.chainId)}</Text>
                  </View>
                } />
                {effTxHash ? (<><Divider /><Row label={t('componentsTx.detail.labelHash')} value={shortAddress(effTxHash)} mono onOpen={() => openBrowser(explorerTxURL(batch.chainId, effTxHash))} /></>) : null}
              </VelaCard>
            </ScrollView>
          </View>
        );
      })() : tx && (() => {
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
                <TokenLogo symbol={tx.symbol} logoUrls={tx.logoUrls} chain={net ?? null} size={52} />
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
                {effTxHash ? (
                  <Pressable hitSlop={8} onPress={() => openBrowser(explorerTxURL(tx.chainId, effTxHash))} style={styles.heroChevron}>
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
                <Row label={t('componentsTx.detail.labelStatus')} custom={<TxStatusBadge status={effStatus} />} />
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
                {effTxHash ? (<><Divider /><Row label={t('componentsTx.detail.labelHash')} value={shortAddress(effTxHash)} mono onOpen={() => openBrowser(explorerTxURL(tx.chainId, effTxHash))} /></>) : null}
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

  // Batch breakdown rows
  brRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: space.md, paddingVertical: space.md },
  brLeft: { flex: 1, minWidth: 0, gap: 2 },
  brTokenLeft: { flex: 1, minWidth: 0, flexDirection: 'row', alignItems: 'center', gap: space.md },
  brRight: { alignItems: 'flex-end', gap: 2 },
  brPrimary: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  brSub: { fontSize: text.sm, ...inter.regular, color: color.fg.muted, fontFamily: font.mono },
  brAmount: { fontSize: text.base, ...inter.bold, color: color.fg.base },

  // Details
  sectionTitle: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  details: { paddingHorizontal: space.lg },
  statusRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  statusText: { fontSize: text.base, ...inter.bold },
  chainRow: { flexDirection: 'row', alignItems: 'center', gap: space.sm },
  chainText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
}));
