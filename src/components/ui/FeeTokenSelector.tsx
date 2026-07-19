/**
 * Shared fee-asset selector — one ROW per fee token (native + whitelisted stables the Safe
 * holds). Each row shows, per the founder's IA, what THIS transaction would cost in that coin
 * ("本次消耗", the emphasis) plus how much of it you hold ("余额"). Used by BOTH the Send confirm
 * slide and the dApp GasFeeCard (via GasFeeCard) so the two can't drift.
 *
 * Presentational only: it never loads options or re-quotes. The parent runs
 * useInBandFeeTokenOptions, owns the selection, and applies the selected quote on `onSelect`.
 * While that update is in flight the parent passes `busy` — the tapped row spins, the rest dim and block taps
 * (which also serialises rapid taps, so no stale-asset race).
 *
 * The bundler's address-only quote supplies each row's balance and USD price in one response.
 * The parent applies the shared gas × gas-price formula to derive each row's exact amount.
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

interface FeeTokenSelectorProps {
  /** Fee-asset options from useInBandFeeTokenOptions (native + held stables). */
  options: FeeTokenOption[];
  /** Current selection: null = native, else a stablecoin contract (case-insensitive). */
  selected: string | null;
  /** The parent applies the selected quote; the selector only signals intent. */
  onSelect: (contract: string | null) => void;
  /** Exact current fee in an option's base units, calculated from the shared gas basis. */
  feeAmountFor: (option: FeeTokenOption) => bigint | null;
  /** A parent-owned selection update is in flight — rows dim + block taps; the tapped row spins. */
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

export function FeeTokenSelector({ options, selected, onSelect, feeAmountFor, busy = false }: FeeTokenSelectorProps) {
  const { t } = useTranslation();
  useLocalePrefs();
  const [pendingKey, setPendingKey] = useState<string | null>(null);

  // Clear the per-row spinner once the parent's selection update settles.
  useEffect(() => { if (!busy) setPendingKey(null); }, [busy]);

  const selectedKey = keyOf(selected);

  return (
    <View style={styles.container}>
      <Text style={styles.header}>{t('componentsUi.gas.feeToken')}</Text>
      {options.map((opt) => {
        const k = keyOf(opt.contract);
        const active = k === selectedKey;
        const holdings = Number(opt.balance) / 10 ** opt.decimals;
        const feeAmount = feeAmountFor(opt);
        const costUnits = feeAmount === null ? null : Number(feeAmount) / 10 ** opt.decimals;
        const costLabel = fmtCost(costUnits);
        // A coin that can't cover the fee (notably the native coin at 0 balance) is SHOWN for
        // context but not selectable — paying gas in it would only produce a doomed op. It's
        // insufficient when held ≤ 0, or held < the fee it would cost.
        const insufficient = feeAmount === null || opt.balance < feeAmount;
        const pending = busy && pendingKey === k;
        return (
          <Pressable
            key={k}
            style={[styles.row, (insufficient || (busy && !pending)) && styles.rowDim]}
            disabled={busy || insufficient}
            onPress={() => { if (insufficient) return; setPendingKey(k); onSelect(opt.contract); }}
            accessibilityRole="button"
            accessibilityState={{ selected: active, disabled: busy || insufficient }}
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
