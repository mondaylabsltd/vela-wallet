/**
 * ReceiveRequestControls — the form for building an EIP-681 payment request.
 *
 * The user picks an asset (which fixes the network + token) and optionally an
 * amount. The asset picker reuses the same TokenSelector as the Send flow, fed
 * with every token — including zero-balance, user-added (custom), and built-in
 * ones (via fetchTokens' includeZeroBalance) — so you can request a token you
 * don't hold yet. Reports the resulting QR value + summary up to ReceiveScreen.
 */
import { AppModal } from '@/components/ui/AppModal';
import { TokenLogo } from '@/components/TokenLogo';
import { TokenSelector } from '@/components/ui/TokenSelector';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';
import { chainName, networkForChainId, tokenBadgeNetwork } from '@/models/network';
import { tokenChainId, tokenLogoURLs, type APIToken } from '@/models/types';
import { buildEIP681 } from '@/services/eip681';
import { hapticLight } from '@/services/platform';
import { clearTokenCache, fetchTokens } from '@/services/wallet-api';
import { ChevronDown } from 'lucide-react-native';
import React, { useEffect, useMemo, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Pressable, Text, TextInput, View } from 'react-native';

interface Props {
  /** The receiving address (the request's beneficiary). */
  recipient: string;
  /** Called whenever the built request changes. */
  onChange: (req: { qrValue: string; summary: string }) => void;
}

/** Default asset shown before anything is picked / loaded: native ETH on Ethereum. */
function defaultAsset(): APIToken {
  return { network: 'eth-mainnet', chainName: 'Ethereum', symbol: 'ETH', balance: '0', decimals: 18, logo: null, name: 'Ethereum', tokenAddress: null, priceUsd: null, spam: false };
}

function sanitizeAmount(text: string, maxDecimals: number): string {
  const cleaned = text.replace(/[^0-9.]/g, '');
  if ((cleaned.match(/\./g) || []).length > 1) return text.slice(0, -1);
  const [i, f] = cleaned.split('.');
  if (f != null && f.length > maxDecimals) return `${i}.${f.slice(0, maxDecimals)}`;
  return cleaned;
}

export function ReceiveRequestControls({ recipient, onChange }: Props) {
  const { t } = useTranslation();

  const [asset, setAsset] = useState<APIToken>(defaultAsset);
  const [amount, setAmount] = useState('');
  const [allTokens, setAllTokens] = useState<APIToken[]>([]);
  const [loadingTokens, setLoadingTokens] = useState(true);
  const [showPicker, setShowPicker] = useState(false);

  const chainId = tokenChainId(asset);
  const networkName = useMemo(
    () => networkForChainId(chainId)?.displayName ?? chainName(chainId),
    [chainId],
  );

  // Load every token (incl. zero-balance / custom / built-in) for the picker.
  const loadTokens = (forceRefresh = false) => {
    if (!recipient) return;
    setLoadingTokens(true);
    if (forceRefresh) clearTokenCache(recipient);
    fetchTokens(recipient, { includeZeroBalance: true })
      .then((list) => setAllTokens(list))
      .catch(() => {})
      .finally(() => setLoadingTokens(false));
  };
  useEffect(() => { loadTokens(); }, [recipient]);

  // Rebuild the URI + summary whenever any input changes.
  useEffect(() => {
    const qrValue = buildEIP681({ recipient, chainId, tokenAddress: asset.tokenAddress, decimals: asset.decimals, amount });
    const hasAmount = !!amount && parseFloat(amount) > 0;
    const summary = hasAmount
      ? t('receive.request.summaryAmount', { amount, symbol: asset.symbol, network: networkName })
      : t('receive.request.summaryOpen', { symbol: asset.symbol, network: networkName });
    onChange({ qrValue, summary });
  }, [recipient, asset, amount, networkName]);

  const pickAsset = (tok: APIToken) => {
    hapticLight();
    setAsset(tok);
    setAmount((a) => sanitizeAmount(a, tok.decimals)); // re-clamp precision
    setShowPicker(false);
  };

  return (
    <View style={styles.wrap}>
      {/* Asset (network + token in one) */}
      <Text style={styles.rowLabel}>{t('receive.request.token')}</Text>
      <Pressable style={styles.selectRow} onPress={() => setShowPicker(true)}>
        <TokenLogo symbol={asset.symbol} logoUrls={tokenLogoURLs(asset)} chain={tokenBadgeNetwork(asset)} size={32} />
        <View style={styles.selectInfo}>
          <Text style={styles.selectValue} numberOfLines={1}>{asset.symbol}</Text>
          <Text style={styles.selectSub} numberOfLines={1}>{networkName}</Text>
        </View>
        <ChevronDown size={18} color={color.fg.muted} strokeWidth={2.2} />
      </Pressable>

      {/* Amount */}
      <Text style={styles.rowLabel}>{t('receive.request.amount')}</Text>
      <View style={styles.amountRow}>
        <TextInput
          style={styles.amountInput}
          value={amount}
          onChangeText={(txt) => setAmount(sanitizeAmount(txt, asset.decimals))}
          placeholder={t('receive.request.amountPlaceholder')}
          placeholderTextColor={color.fg.subtle}
          keyboardType="decimal-pad"
          inputMode="decimal"
        />
        <Text style={styles.amountSymbol}>{asset.symbol}</Text>
      </View>
      <Text style={styles.amountHint}>{t('receive.request.amountHint')}</Text>

      {/* Asset picker — reuses the Send token selector */}
      <AppModal visible={showPicker} onClose={() => setShowPicker(false)}>
        <View style={styles.sheet}>
          <Text style={styles.sheetTitle}>{t('receive.request.selectToken')}</Text>
          <TokenSelector
            tokens={allTokens}
            loading={loadingTokens}
            onSelect={pickAsset}
            onAddChanged={() => loadTokens(true)}
            hideTotals
            defaultCategory="stable"
          />
        </View>
      </AppModal>
    </View>
  );
}

const styles = createStyles(() => ({
  wrap: { gap: space.sm },
  rowLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.subtle,
    marginTop: space.lg,
    marginBottom: space.xs,
  },
  selectRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.lg,
    paddingVertical: space.md,
    paddingHorizontal: space.lg,
  },
  selectInfo: { flex: 1 },
  selectValue: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  selectSub: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },
  amountRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    backgroundColor: color.bg.raised,
    borderWidth: 1,
    borderColor: color.border.base,
    borderRadius: radius.lg,
    paddingHorizontal: space.lg,
  },
  amountInput: {
    flex: 1,
    fontSize: text.lg,
    ...inter.semibold,
    color: color.fg.base,
    paddingVertical: space.md,
    outlineStyle: 'none',
  } as any,
  amountSymbol: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.muted,
  },
  amountHint: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
    marginTop: space.xs,
  },
  sheet: {
    flex: 1,
    backgroundColor: color.bg.base,
    paddingHorizontal: space['2xl'],
    paddingTop: space.md,
  },
  sheetTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
    textAlign: 'center',
    marginBottom: space.lg,
  },
}));
