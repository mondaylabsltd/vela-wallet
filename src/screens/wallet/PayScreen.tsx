/**
 * PayScreen — the public payment-link bridge (route `/pay`).
 *
 * Opened from a shared payment link on a Vela deployment (the hosted web wallet
 * or a self-hosted one). It reads the EIP-681 fields from the query and offers
 * three paths: continue in this Vela wallet (locked Send), pay with any other
 * wallet (EIP-681 *or* plain-address QR), or copy the details to enter by hand.
 */
import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { TokenLogo } from '@/components/TokenLogo';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { VelaButton } from '@/components/ui/VelaButton';
import { VelaCard } from '@/components/ui/VelaCard';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { networkForChainId, networkId, tokenBadgeNetwork } from '@/models/network';
import { isAddress, tokenLogoURLs, type APIToken } from '@/models/types';
import { buildEIP681, toBaseUnits } from '@/services/eip681';
import { copyToClipboard, hapticLight, openURL } from '@/services/platform';
import { Check, Copy } from 'lucide-react-native';
import { useLocalSearchParams } from 'expo-router';
import React, { useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Image, Pressable, ScrollView, Text, View } from 'react-native';

const LOGO = require('../../../assets/images/icon.png');

function short(a: string): string {
  return a ? `${a.slice(0, 10)}…${a.slice(-8)}` : '';
}

export default function PayScreen() {
  const { t } = useTranslation();
  const router = useSafeRouter();
  const p = useLocalSearchParams<{ to?: string; chain?: string; token?: string; amount?: string; sym?: string; dec?: string; net?: string }>();

  const to = (p.to ?? '').trim();
  const chainId = parseInt(p.chain ?? '', 10);
  const token = (p.token ?? '').trim();
  const amount = (p.amount ?? '').trim();
  const symbol = (p.sym ?? '').trim() || 'tokens';
  const decimals = parseInt(p.dec ?? '18', 10) || 18;
  const networkName = (p.net ?? '').trim() || (Number.isFinite(chainId) ? `Chain ${chainId}` : '');
  const valid = isAddress(to) && Number.isFinite(chainId);
  const isNative = !token;

  const network = useMemo(() => (Number.isFinite(chainId) ? networkForChainId(chainId) : null), [chainId]);
  // Synthetic token just for its logo + chain badge.
  const apiToken = useMemo<APIToken>(() => ({
    network: networkId(chainId), chainName: networkName, symbol, balance: '0',
    decimals: isNative ? 18 : decimals, logo: null, name: symbol,
    tokenAddress: token || null, priceUsd: null, spam: false,
  }), [chainId, networkName, symbol, decimals, isNative, token]);
  const badge = useMemo(() => tokenBadgeNetwork(apiToken), [apiToken]);

  const eip681 = buildEIP681({ recipient: to, chainId, tokenAddress: token || null, decimals, amount });
  const amountBase = amount ? toBaseUnits(amount, decimals).toString() : '';

  const [showOther, setShowOther] = useState(false);
  const [qrMode, setQrMode] = useState<'eip681' | 'address'>('eip681');
  const [copiedKey, setCopiedKey] = useState<string | null>(null);
  const copy = async (key: string, value: string) => {
    await copyToClipboard(value);
    hapticLight();
    setCopiedKey(key);
    setTimeout(() => setCopiedKey((k) => (k === key ? null : k)), 1500);
  };

  const openInVela = () => {
    const params: Record<string, string> = { prefilledRecipient: to, prefilledChainId: String(chainId), locked: '1' };
    if (token) params.prefilledTokenAddress = token;
    if (amountBase) params.prefilledAmountBase = amountBase;
    router.push({ pathname: '/send', params });
  };

  const headline = amount ? `${amount} ${symbol}` : symbol;

  if (!valid) {
    return (
      <ScreenContainer>
        <View style={styles.center}>
          <Text style={styles.invalidTitle}>{t('receive.pay.invalidTitle')}</Text>
          <Text style={styles.invalidBody}>{t('receive.pay.invalidBody')}</Text>
        </View>
      </ScreenContainer>
    );
  }

  return (
    <ScreenContainer>
      <ScrollView showsVerticalScrollIndicator={false} contentContainerStyle={styles.content}>
        <View style={styles.brand}>
          <Image source={LOGO} style={styles.logo} />
          <Text style={styles.brandName}>Vela Wallet</Text>
        </View>

        <VelaCard elevated style={styles.card}>
          <TokenLogo symbol={symbol} logoUrls={tokenLogoURLs(apiToken)} chain={badge} size={56} />
          <Text style={styles.eyebrow}>{t('receive.pay.title')}</Text>
          <Text style={styles.headline}>{headline}</Text>
          {/* The token logo already badges the chain; only spell the network out
              when there's no badge (a native coin on its own chain). */}
          {!badge && (
            <View style={styles.networkRow}>
              {network && <ChainLogo label={network.iconLabel} color={network.iconColor} bgColor={network.iconBg} logoURL={network.logoURL} size={16} />}
              <Text style={styles.network}>{networkName}</Text>
            </View>
          )}

          <Pressable style={styles.addrRow} onPress={() => copy('to', to)}>
            <Text style={styles.addr} numberOfLines={1}>{short(to)}</Text>
            {copiedKey === 'to' ? <Check size={16} color={color.success.base} strokeWidth={2.5} /> : <Copy size={16} color={color.fg.muted} strokeWidth={2} />}
          </Pressable>

          <VelaButton title={t('receive.pay.open')} onPress={openInVela} style={styles.openBtn} />
          <Pressable style={styles.otherBtn} onPress={() => setShowOther((s) => !s)}>
            <Text style={styles.otherBtnText}>{t('receive.pay.other')}</Text>
          </Pressable>
        </VelaCard>

        {showOther && (
          <VelaCard style={styles.card}>
            {/* QR mode: full EIP-681 request vs plain address for non-681 wallets */}
            <View style={styles.seg}>
              {(['eip681', 'address'] as const).map((m) => (
                <Pressable key={m} style={[styles.segBtn, qrMode === m && styles.segBtnActive]} onPress={() => setQrMode(m)}>
                  <Text style={[styles.segText, qrMode === m && styles.segTextActive]}>
                    {t(m === 'eip681' ? 'receive.pay.qrEip681' : 'receive.pay.qrAddress')}
                  </Text>
                </Pressable>
              ))}
            </View>

            <View style={styles.qrBox}>
              <QRCode value={qrMode === 'eip681' ? eip681 : to} size={180} />
            </View>
            <Text style={styles.scanHint}>{t(qrMode === 'eip681' ? 'receive.pay.scanHint' : 'receive.pay.scanHintAddress')}</Text>

            {qrMode === 'eip681' && (
              <Pressable style={styles.openWalletBtn} onPress={() => openURL(eip681)}>
                <Text style={styles.openWalletText}>{t('receive.pay.openApp')}</Text>
              </Pressable>
            )}

            <Text style={styles.manualNote}>{t('receive.pay.manualNote')}</Text>
            <DetailRow label={t('receive.pay.recipient')} value={short(to)} onCopy={() => copy('m-to', to)} copied={copiedKey === 'm-to'} />
            <DetailRow
              label={t('receive.pay.network')}
              value={`${networkName} (${chainId})`}
              icon={network ? <ChainLogo label={network.iconLabel} color={network.iconColor} bgColor={network.iconBg} logoURL={network.logoURL} size={18} /> : undefined}
            />
            {isNative
              ? <DetailRow label={t('receive.pay.token')} value={t('receive.pay.native', { symbol })} icon={<TokenLogo symbol={symbol} logoUrls={tokenLogoURLs(apiToken)} size={18} />} />
              : <DetailRow label={t('receive.pay.token')} value={`${symbol} · ${short(token)}`} onCopy={() => copy('m-token', token)} copied={copiedKey === 'm-token'} icon={<TokenLogo symbol={symbol} logoUrls={tokenLogoURLs(apiToken)} size={18} />} />}
            <DetailRow label={t('receive.pay.amount')} value={amount ? `${amount} ${symbol}` : t('receive.pay.anyAmount')} />
          </VelaCard>
        )}
      </ScrollView>
    </ScreenContainer>
  );
}

function DetailRow({ label, value, onCopy, copied, icon }: { label: string; value: string; onCopy?: () => void; copied?: boolean; icon?: React.ReactNode }) {
  return (
    <View style={styles.detailRow}>
      <Text style={styles.detailLabel}>{label}</Text>
      <View style={styles.detailRight}>
        {icon}
        {onCopy ? (
          <Pressable style={styles.detailValueBtn} onPress={onCopy} hitSlop={6}>
            <Text style={styles.detailValue} numberOfLines={1}>{value}</Text>
            {copied ? <Check size={13} color={color.success.base} strokeWidth={2.5} /> : <Copy size={13} color={color.fg.subtle} strokeWidth={2} />}
          </Pressable>
        ) : (
          <Text style={styles.detailValue} numberOfLines={1}>{value}</Text>
        )}
      </View>
    </View>
  );
}

const styles = createStyles(() => ({
  content: { paddingVertical: space['2xl'], gap: space.xl },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', padding: space['3xl'], gap: space.md },
  invalidTitle: { fontSize: text.xl, ...inter.bold, color: color.fg.base, textAlign: 'center' },
  invalidBody: { fontSize: text.base, ...inter.regular, color: color.fg.subtle, textAlign: 'center' },
  brand: { flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm },
  logo: { width: 24, height: 24, borderRadius: 6 },
  brandName: { fontSize: text.base, ...inter.bold, color: color.fg.base },
  card: { padding: space['2xl'], alignItems: 'center' },
  eyebrow: { fontSize: text.xs, ...inter.semibold, color: color.fg.subtle, textTransform: 'uppercase', letterSpacing: 0.5, marginTop: space.lg, marginBottom: space.xs },
  headline: { fontSize: text['3xl'], ...inter.bold, color: color.fg.base, textAlign: 'center' },
  networkRow: { flexDirection: 'row', alignItems: 'center', gap: 6, marginTop: space.xs },
  network: { fontSize: text.base, ...inter.semibold, color: color.accent.base },
  addrRow: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: space.md,
    alignSelf: 'stretch', marginTop: space.xl, paddingVertical: space.md, paddingHorizontal: space.lg,
    borderRadius: radius.lg, backgroundColor: color.bg.sunken, borderWidth: 1, borderColor: color.border.base,
  },
  addr: { flex: 1, fontSize: text.sm, ...inter.medium, fontFamily: font.mono, color: color.fg.base },
  openBtn: { alignSelf: 'stretch', marginTop: space.xl },
  otherBtn: { alignSelf: 'stretch', alignItems: 'center', paddingVertical: space.lg, marginTop: space.sm },
  otherBtnText: { fontSize: text.base, ...inter.semibold, color: color.fg.muted },
  // QR mode segmented toggle
  seg: { flexDirection: 'row', alignSelf: 'stretch', backgroundColor: color.bg.sunken, borderRadius: radius.full, padding: 4, marginBottom: space.lg },
  segBtn: { flex: 1, alignItems: 'center', paddingVertical: space.sm, borderRadius: radius.full },
  segBtnActive: { backgroundColor: color.bg.raised },
  segText: { fontSize: text.sm, ...inter.semibold, color: color.fg.muted },
  segTextActive: { color: color.fg.base },
  qrBox: { borderWidth: 1, borderColor: color.border.base, borderRadius: radius.xl, padding: space.lg, backgroundColor: '#FFFFFF', marginBottom: space.md },
  scanHint: { fontSize: text.sm, ...inter.regular, color: color.fg.subtle, marginBottom: space.lg, textAlign: 'center' },
  openWalletBtn: { alignSelf: 'stretch', alignItems: 'center', paddingVertical: space.lg, borderRadius: radius.lg, borderWidth: 1, borderColor: color.border.strong, backgroundColor: color.bg.raised },
  openWalletText: { fontSize: text.base, ...inter.semibold, color: color.fg.base },
  manualNote: { alignSelf: 'stretch', fontSize: text.sm, ...inter.regular, color: color.fg.subtle, marginTop: space.xl, marginBottom: space.md },
  detailRow: { flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between', gap: space.md, alignSelf: 'stretch', paddingVertical: space.md, borderTopWidth: 1, borderTopColor: color.border.base },
  detailLabel: { fontSize: text.sm, ...inter.medium, color: color.fg.muted },
  detailRight: { flexDirection: 'row', alignItems: 'center', gap: space.sm, flexShrink: 1 },
  detailValueBtn: { flexDirection: 'row', alignItems: 'center', gap: space.sm, flexShrink: 1 },
  detailValue: { fontSize: text.sm, ...inter.semibold, color: color.fg.base, flexShrink: 1 },
}));
