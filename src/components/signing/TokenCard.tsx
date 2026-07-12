/**
 * Token card / flow arrow — the asset-amount hero of a signing surface.
 */
import { TokenLogo } from '@/components/TokenLogo';
import { color } from '@/constants/theme';
import { nativeCoinLogoURL, nativeSymbol } from '@/models/network';
import { tokenLogoURLsByAddress } from '@/models/types';
import { type ClearSignField } from '@/services/clear-signing';
import { knownTokenSymbol } from '@/services/tokens';
import { AlertTriangle, ArrowDown } from 'lucide-react-native';
import React from 'react';
import { Text, View } from 'react-native';
import { useLocalePrefs } from '@/services/locale-format';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { localizeLabel, riskColors, SigningChainContext, styles } from './signing-core';

export function TokenCard({ field, variant, hideSign, hero ,heroLabel}: {
  field: ClearSignField;
  variant: 'send' | 'receive' | 'caution' | 'danger';
  /** Drop the leading −/+ on a lone amount (a plain send hero) — the eyebrow and the
   *  plain-language summary already say "sending", so a bare "1,000 USDC" reads
   *  friendlier than a "−1,000" that a novice can misread as an error. A swap keeps
   *  its signs, since ± is what distinguishes pay from receive. */
  hideSign?: boolean;
  /** The amount hero: render logo-less and left-aligned (number in ink, ticker muted)
   *  so the sheet keeps one clean left edge, like the mock. */
  hero?: boolean;
  /** Keep the label + fiat sub-line under a hero amount (a swap's 支付 / 最少收到，which
   *  a plain send doesn't need — its summary already says "sending"). */
  heroLabel?: boolean;
}) {
  // Re-render on number-format preset change; format fiat in the user's currency.
  useLocalePrefs();
  const dc = useDisplayCurrency();
  // Wise-style de-container: benign amounts sit on an OPEN row (no card), letting
  // the number breathe; only caution/danger get a tinted card, so a filled card
  // always means "pay attention".
  const tinted = variant === 'caution' || variant === 'danger';
  const tintBg = variant === 'caution'
    ? { backgroundColor: color.warning.soft }
    : { backgroundColor: color.error.soft };

  const chainId = React.useContext(SigningChainContext);
  // A `amount`-format field with no token address is the chain's native coin
  // (e.g. a plain ETH send) — show the real coin symbol + logo, not a "?".
  const isNative = !field.tokenAddress && field.format === 'amount';
  const symbol = field.tokenAddress
    ? guessTokenSymbol(field.tokenAddress)
    : isNative ? nativeSymbol(chainId) : undefined;
  // Per-chain logo (checksummed + lowercase fallback) — not a mainnet-only guess.
  const logoUrls = field.tokenAddress
    ? tokenLogoURLsByAddress(chainId, field.tokenAddress)
    : isNative ? [nativeCoinLogoURL(chainId)] : undefined;

  // Directional framing (MetaMask/Rainbow "estimated changes" convention, shared
  // with BalanceChangePreview): "+" green for what arrives, "−" neutral ink for
  // what leaves. The signed amount is the hero of a benign transfer.
  const incoming = variant === 'receive';
  const sign = incoming ? '+' : '−';
  const amountTint = incoming ? color.success.base : color.fg.base;

  // Logo-less hero: one clean left edge, number in ink + ticker muted (the mock).
  if (hero && !tinted) {
    const val = hideSign ? field.value : `${sign}${field.value}`;
    const cut = val.lastIndexOf(' ');
    const num = cut > 0 ? val.slice(0, cut) : val;
    const ticker = cut > 0 ? val.slice(cut + 1) : '';
    return (
      <View style={styles.heroRow}>
        <View style={{ flex: 1 }}>
          <View style={styles.heroAmountLine}>
            <Text style={[styles.heroAmount, { color: amountTint, flexShrink: 1 }]} numberOfLines={1} adjustsFontSizeToFit minimumFontScale={0.5}>
              {num}{ticker ? <Text style={styles.heroTicker}> {ticker}</Text> : null}
            </Text>
            {/* Token logo beside the ticker — the asset's face, ~ticker-sized. */}
            {(logoUrls || isNative) && <TokenLogo symbol={symbol ?? '?'} logoUrls={logoUrls} size={26} />}
          </View>
          {heroLabel && (
            <View style={styles.tokenSubRow}>
              <Text style={styles.tokenLabel}>{localizeLabel(field.label)}</Text>
              {field.usdValue != null && <Text style={styles.tokenUsd}>≈ {dc.fmt(field.usdValue)}</Text>}
            </View>
          )}
        </View>
        {field.warning && <AlertTriangle size={16} color={riskColors().danger} strokeWidth={2} />}
      </View>
    );
  }

  return (
    <View style={[tinted ? styles.tokenCard : styles.tokenRow, tinted && tintBg]}>
      <TokenLogo
        symbol={symbol ?? '?'}
        logoUrls={logoUrls}
        size={44}
      />
      <View style={styles.tokenInfo}>
        <Text
          style={[styles.tokenAmount, { color: amountTint }]}
          numberOfLines={1}
          adjustsFontSizeToFit
          minimumFontScale={0.6}
        >
          {hideSign ? field.value : `${sign}${field.value}`}
        </Text>
        <View style={styles.tokenSubRow}>
          <Text style={styles.tokenLabel}>{localizeLabel(field.label)}</Text>
          {field.usdValue != null && <Text style={styles.tokenUsd}>≈ {dc.fmt(field.usdValue)}</Text>}
        </View>
      </View>
      {field.warning && (
        <View style={styles.tokenWarning}>
          <AlertTriangle size={14} color={riskColors().danger} strokeWidth={2} />
        </View>
      )}
    </View>
  );
}

/** Guess token symbol from the shared known-token table, with an address fallback. */
function guessTokenSymbol(addr: string): string {
  return knownTokenSymbol(addr) ?? addr.slice(2, 6).toUpperCase();
}

export function FlowArrow({ danger }: { danger?: boolean }) {
  return (
    <View style={styles.flowArrow}>
      <View style={[styles.flowCircle, danger && styles.flowCircleDanger]}>
        <ArrowDown
          size={14}
          color={danger ? riskColors().danger : color.fg.subtle}
          strokeWidth={2.5}
        />
      </View>
    </View>
  );
}
