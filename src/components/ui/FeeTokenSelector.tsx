/**
 * Shared fee-asset selector — one ROW per fee token (native + whitelisted stables the Safe
 * holds). Each row shows, per the founder's IA, what THIS transaction would cost in that coin
 * ("本次消耗", the emphasis) plus how much of it you hold ("余额"). Used by BOTH the Send confirm
 * slide and the dApp GasFeeCard (via GasFeeCard) so the two can't drift.
 *
 * Presentational only: it never loads options or re-quotes. The parent runs
 * useInBandFeeTokenOptions, owns the selection, and re-prices on `onSelect`. While that re-quote
 * is in flight the parent passes `busy` — the tapped row spins, the rest dim and block taps
 * (which also serialises rapid taps, so no stale-asset race).
 *
 * Per-coin cost is derived CLIENT-SIDE from the current quote's USD value (`feeUsd`): the bundler
 * markup is a uniform 3× across coins, so one quote prices them all — no per-coin bundler RPC.
 * Stables carry the bundler's $0.01 minimum. The exact charge for the SELECTED coin is still the
 * bundler's authoritative quote (signed at submit); the other rows are honest ~estimates.
 *
 * Design: de-boxed (open rows under a hairline, no card), real token logos, and a selected accent
 * check only (the app's picker convention, e.g. CurrencySheet) — no filled tint.
 */

import React, { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { Check } from 'lucide-react-native';
import { TokenLogo } from '@/components/TokenLogo';
import { color, createStyles, font, inter, space, text } from '@/constants/theme';
import { formatTokenAmount, useLocalePrefs } from '@/services/locale-format';
import type { FeeTokenOption } from '@/hooks/use-inband-fee-tokens';

/** The bundler's minimum in-band charge for a stablecoin fee. */
const STABLE_MIN_USD = 0.01;

interface FeeTokenSelectorProps {
  /** Fee-asset options from useInBandFeeTokenOptions (native + held stables). */
  options: FeeTokenOption[];
  /** Current selection: null = native, else a stablecoin contract (case-insensitive). */
  selected: string | null;
  /** The parent re-quotes on select; the selector only signals intent. */
  onSelect: (contract: string | null) => void;
  /** Native-coin USD price — converts the USD fee into the native coin's amount. */
  nativeUsdPrice: number;
  /** The current network fee in USD (from the live quote) — prices every coin's cost. */
  feeUsd: number;
  /** A re-quote is in flight (parent-owned) — rows dim + block taps; the tapped row spins. */
  busy?: boolean;
}

/** null (native) collapses to a stable string key so it never collides with "no selection". */
const keyOf = (contract: string | null) => contract?.toLowerCase() ?? 'native';

/** Fee amount in a coin's own units — capped display so a dust amount reads "< 0.0001". */
function fmtCost(units: number | null): string | null {
  if (units === null || units <= 0) return null;
  if (units < 0.0001) return '< 0.0001';
  return formatTokenAmount(units, { compact: true });
}

export function FeeTokenSelector({ options, selected, onSelect, nativeUsdPrice, feeUsd, busy = false }: FeeTokenSelectorProps) {
  const { t } = useTranslation();
  useLocalePrefs();
  const [pendingKey, setPendingKey] = useState<string | null>(null);

  // Clear the per-row spinner once the parent's re-quote settles.
  useEffect(() => { if (!busy) setPendingKey(null); }, [busy]);

  const selectedKey = keyOf(selected);

  return (
    <View style={styles.container}>
      <Text style={styles.header}>{t('componentsUi.gas.feeToken')}</Text>
      {options.map((opt) => {
        const k = keyOf(opt.contract);
        const active = k === selectedKey;
        const holdings = Number(opt.balance) / 10 ** opt.decimals;
        // Cost in this coin: native = feeUSD ÷ native price; stable = feeUSD (1:1) with the $0.01 floor.
        const costUnits = opt.contract === null
          ? (nativeUsdPrice > 0 ? feeUsd / nativeUsdPrice : null)
          : Math.max(feeUsd, STABLE_MIN_USD);
        const costLabel = fmtCost(costUnits);
        const pending = busy && pendingKey === k;
        return (
          <Pressable
            key={k}
            style={[styles.row, busy && !pending && styles.rowDim]}
            disabled={busy}
            onPress={() => { setPendingKey(k); onSelect(opt.contract); }}
            accessibilityRole="button"
            accessibilityState={{ selected: active, disabled: busy }}
            accessibilityLabel={opt.symbol}
          >
            <TokenLogo symbol={opt.symbol} logoUrls={opt.logoUrls} size={32} />
            <View style={styles.who}>
              <Text style={styles.sym} numberOfLines={1}>{opt.symbol}</Text>
              <Text style={styles.bal} numberOfLines={1}>
                {t('componentsUi.gas.rowBalance')} {formatTokenAmount(holdings, { compact: true })}
              </Text>
            </View>
            {/* Cost of THIS tx in this coin — the emphasis; balance is context on the left. */}
            <View style={styles.cost}>
              <Text style={styles.costAmt} numberOfLines={1}>
                {costLabel ? `~${costLabel} ${opt.symbol}` : '—'}
              </Text>
              <Text style={styles.costLabel} numberOfLines={1}>{t('componentsUi.gas.rowSpend')}</Text>
            </View>
            <View style={styles.trailing}>
              {pending ? (
                <ActivityIndicator size={16} color={color.accent.base} />
              ) : active ? (
                <Check size={18} color={color.accent.base} strokeWidth={2.6} />
              ) : null}
            </View>
          </Pressable>
        );
      })}
    </View>
  );
}

const styles = createStyles(() => ({
  container: {
    borderTopWidth: 1,
    borderTopColor: color.border.base,
    marginBottom: space.lg,
  },
  header: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.8,
    marginTop: space.lg,
    marginBottom: space.xs,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.md,
  },
  rowDim: {
    opacity: 0.4,
  },
  who: {
    flex: 1,
    minWidth: 0,
    gap: 2,
  },
  sym: {
    fontSize: text.base,
    ...inter.semibold,
    color: color.fg.base,
  },
  bal: {
    fontSize: text.xs,
    ...inter.regular,
    fontFamily: font.numeric,
    color: color.fg.subtle,
  },
  cost: {
    alignItems: 'flex-end',
    gap: 2,
  },
  costAmt: {
    fontSize: text.base,
    ...inter.semibold,
    fontFamily: font.numeric,
    color: color.fg.base,
  },
  costLabel: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  trailing: {
    width: 22,
    alignItems: 'center',
    justifyContent: 'center',
  },
}));
