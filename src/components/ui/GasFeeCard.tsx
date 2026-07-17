/**
 * Reusable gas fee display + fee-asset selector.
 *
 * Shared by SendScreen (native transfers, ERC-20) and SigningRequestModal
 * (dApp contract calls). Shows:
 *   - Collapsed: "Est. Fee ~0.0012 POL ≈ $0.003" (+ a refresh affordance)
 *   - Expanded: fee-asset chips (native + whitelisted stables) on in-band
 *     chains with a DEX. Speed tiers are gone — every estimate runs at 'fast' —
 *     and the technical gas rows are gone with them.
 *
 * The card OWNS the fee-asset option loading (isInBandChain + fetchChainTokens);
 * the selected token itself is controlled by the parent (`gasFeeToken` +
 * `onFeeTokenChange`) so the approve/submit path sends exactly what was quoted.
 * On Tempo no selector renders — the fee is always pathUSD there.
 */

import React, { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { ChevronDown, ChevronUp, RefreshCw } from 'lucide-react-native';
import { VelaCard } from './VelaCard';
import { color, createStyles, inter, radius, space, text } from '@/constants/theme';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { useLocalePrefs, numberSeparators, formatNumber } from '@/services/locale-format';
import {
  estimateTransactionFee,
  refreshGasPrice,
  requoteInBandFee,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';
import { useInBandFeeTokenOptions } from '@/hooks/use-inband-fee-tokens';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/** Token fee amount in the user's number format (decimal mark + grouping). */
function formatFeeAmount(units: number, sep: { group: string; decimal: string; indian?: boolean }): string {
  if (units === 0) return '0';
  if (units < 0.0001) return `< 0${sep.decimal}0001`;
  return formatNumber(units, { maximumFractionDigits: 4 });
}

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------

interface GasFeeCardProps {
  /** Current fee estimate (null while loading). */
  feeEstimate: TransactionFeeEstimate | null;
  /** Whether fee estimation is in progress. */
  estimating: boolean;
  /** Native token symbol (e.g. "POL", "ETH"). */
  nativeSymbol: string;
  /** Native token price in USD (for fiat display). */
  nativeUsdPrice: number;
  /** Safe wallet address (for re-estimation + the in-band probe). */
  safeAddress: string;
  /** Chain ID. */
  chainId: number;
  /** The real tx being signed — passed so fee-asset change/refresh re-estimates
   *  the actual call (dApp tx), not a dummy transfer. Omit for simple transfers. */
  tx?: { to: string; value?: string; data?: string };
  /** An EIP-5792 batch's calls — estimate against the whole MultiSend. */
  batchCalls?: { to: string; value?: string; data?: string }[];
  /** Selected fee asset: null = native, else a whitelisted stablecoin contract.
   *  Controlled by the parent so the submit path uses exactly what was quoted. */
  gasFeeToken?: string | null;
  /** Called when the user picks a fee asset (the card re-estimates itself). */
  onFeeTokenChange?: (token: string | null) => void;
  /** Called when fee estimate is updated (after refresh or fee-asset change). */
  onFeeUpdate: (fee: TransactionFeeEstimate) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function GasFeeCard({
  feeEstimate, estimating, nativeSymbol: sym, nativeUsdPrice,
  safeAddress, chainId, tx, batchCalls, gasFeeToken = null, onFeeTokenChange, onFeeUpdate,
}: GasFeeCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  // Fee-asset options: null = no selector (legacy chain / Tempo / no DEX).

  // Region format: fiat in the chosen display currency (€/¥/…) + the number
  // format's grouping/decimal marks, matching the amounts everywhere else.
  const dc = useDisplayCurrency();
  useLocalePrefs();
  const sep = numberSeparators();

  // Fee-asset options — native + held whitelisted stables — from the SHARED loader used by
  // the Send confirm slide too (one source, can't drift; on-chain balance reads, timing-robust).
  const feeTokenOptions = useInBandFeeTokenOptions(chainId, safeAddress, true);

  // Fee amounts: an in-band stablecoin fee rides in feeAsset (token units, its
  // own decimals — totalWei is 0 then); native keeps the wei-based path.
  const erc20Fee = feeEstimate?.feeAsset?.kind === 'erc20' ? feeEstimate.feeAsset : null;
  const erc20Symbol = erc20Fee
    ? (feeTokenOptions?.find((o) => o.contract?.toLowerCase() === erc20Fee.token.toLowerCase())?.symbol
      ?? `${erc20Fee.token.slice(0, 6)}…`)
    : null;
  const feeUnits = erc20Fee
    ? Number(erc20Fee.amount) / 10 ** erc20Fee.decimals
    : feeEstimate ? Number(feeEstimate.totalWei) / 1e18 : 0;
  // A whitelisted fee stable is USD-pegged: its ≈USD is the same number.
  const feeUsd = erc20Fee ? feeUnits : feeUnits * nativeUsdPrice;
  const feeSym = erc20Fee ? erc20Symbol : sym;
  // Show fiat only when it renders as a meaningful non-zero (formatFiat rounds to
  // 2 dp) — below that the native amount is the honest primary.
  const showFiat = feeUsd >= 0.005;

  const handleRefresh = useCallback(async () => {
    if (refreshing) return;
    setRefreshing(true);
    try {
      await refreshGasPrice(chainId);
      const fee = await estimateTransactionFee(safeAddress, chainId, 'fast', tx, batchCalls, gasFeeToken);
      onFeeUpdate(fee);
    } catch { /* ignore */ }
    setRefreshing(false);
  }, [chainId, safeAddress, tx, batchCalls, gasFeeToken, refreshing, onFeeUpdate]);

  const handleFeeTokenSelect = useCallback(async (contract: string | null) => {
    if ((gasFeeToken?.toLowerCase() ?? null) === (contract?.toLowerCase() ?? null)) return;
    onFeeTokenChange?.(contract);
    setRefreshing(true);
    try {
      // Fast path: an asset switch doesn't change the gas basis — one bundler RPC.
      // HARD 12s ceiling so the spinner always resolves (hung quote → keep previous).
      const run = (async () => {
        const fast = feeEstimate ? await requoteInBandFee(feeEstimate, chainId, safeAddress, contract) : null;
        return fast ?? await estimateTransactionFee(safeAddress, chainId, 'fast', tx, batchCalls, contract);
      })();
      const fee = await Promise.race([run, new Promise<null>((r) => setTimeout(() => r(null), 12_000))]);
      if (fee) onFeeUpdate(fee);
    } catch { /* keep the previous quote — the user can re-tap or refresh */ }
    setRefreshing(false);
  }, [safeAddress, chainId, tx, batchCalls, gasFeeToken, feeEstimate, onFeeTokenChange, onFeeUpdate]);

  const { t } = useTranslation();

  // Estimation finished with no result — a dead-end unless we offer a retry.
  const failed = !estimating && !refreshing && !feeEstimate;
  // Only offer the expand affordance when there is actually a choice to make.
  const selectable = !!feeTokenOptions && feeTokenOptions.length > 1;

  return (
    <>
      {/* Collapsed toggle row — tap to expand the fee-asset picker, or to retry
          when estimation failed */}
      <Pressable
        onPress={failed ? handleRefresh : selectable ? () => setExpanded(!expanded) : undefined}
        style={styles.toggleRow}
      >
        <Text style={styles.toggleLabel}>{t('componentsUi.gas.estFee')}</Text>
        <View style={styles.toggleRight}>
          <View style={styles.toggleValues}>
            {/* Fiat-first: for a novice a fee means "$X". The token amount drops to a
                quiet sub-line. */}
            <Text style={[styles.toggleValue, failed && styles.toggleValueFailed]}>
              {estimating || refreshing
                ? t('componentsUi.gas.estimating')
                : feeEstimate
                  ? (showFiat ? `≈ ${dc.fmt(feeUsd)}` : `~${formatFeeAmount(feeUnits, sep)} ${feeSym}`)
                  : t('componentsUi.gas.estimateFailed')}
            </Text>
            {!estimating && !failed && feeEstimate && showFiat && (
              <Text style={styles.toggleSub}>~{formatFeeAmount(feeUnits, sep)} {feeSym}</Text>
            )}
          </View>
          {failed ? (
            <RefreshCw size={16} color={color.warning.base} strokeWidth={2} />
          ) : (
            <>
              {feeEstimate && !estimating && (
                <Pressable onPress={handleRefresh} hitSlop={8} style={styles.refreshBtn}>
                  {refreshing ? (
                    <ActivityIndicator size={14} color={color.fg.muted} />
                  ) : (
                    <RefreshCw size={14} color={color.fg.muted} strokeWidth={2} />
                  )}
                </Pressable>
              )}
              {selectable && feeEstimate && !estimating ? (
                expanded
                  ? <ChevronUp size={16} color={color.fg.subtle} strokeWidth={2} />
                  : <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2} />
              ) : null}
            </>
          )}
        </View>
      </Pressable>

      {/* Expanded fee-asset picker — native + whitelisted stables. */}
      {expanded && selectable && feeEstimate && (
        <VelaCard style={styles.gasCard}>
          <Text style={styles.feeTokenLabel}>{t('componentsUi.gas.feeToken')}</Text>
          <View style={styles.feeTokenRow}>
            {feeTokenOptions!.map((opt) => {
              const active = (gasFeeToken?.toLowerCase() ?? null) === (opt.contract?.toLowerCase() ?? null);
              return (
                <Pressable
                  key={opt.contract ?? 'native'}
                  style={[styles.feeTokenBtn, active && styles.feeTokenBtnActive]}
                  onPress={() => handleFeeTokenSelect(opt.contract)}
                  accessibilityRole="button"
                  accessibilityState={{ selected: active }}
                >
                  <Text style={[styles.feeTokenBtnText, active && styles.feeTokenBtnTextActive]}>
                    {opt.symbol}
                  </Text>
                </Pressable>
              );
            })}
          </View>
        </VelaCard>
      )}
    </>
  );
}

// ---------------------------------------------------------------------------
// Styles
// ---------------------------------------------------------------------------

const styles = createStyles(() => ({
  toggleRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
    // No horizontal inset — the fee row shares the sheet's left edge with the
    // eyebrow / hero / summary / 技术细节 (they were 4px apart).
    marginBottom: space.sm,
  },
  toggleLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  toggleRight: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
  },
  toggleValues: {
    alignItems: 'flex-end' as const,
  },
  toggleValue: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
  },
  toggleValueFailed: {
    color: color.warning.base,
  },
  toggleSub: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  refreshBtn: {
    padding: space.xs,
  },
  gasCard: {
    padding: space.xl,
    marginBottom: space.lg,
  },
  feeTokenLabel: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
    textTransform: 'uppercase' as const,
    letterSpacing: 0.8,
    marginBottom: space.md,
  },
  // Fee-asset chips (formerly the speed-tier chips) — wraps when a chain
  // whitelists several stables.
  feeTokenRow: {
    flexDirection: 'row',
    flexWrap: 'wrap' as const,
    alignItems: 'center',
    gap: space.sm,
  },
  feeTokenBtn: {
    paddingVertical: space.sm,
    paddingHorizontal: space.lg,
    borderRadius: radius.md,
    alignItems: 'center',
    backgroundColor: color.bg.sunken,
  },
  feeTokenBtnActive: {
    backgroundColor: color.fg.base,
  },
  feeTokenBtnText: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
  },
  feeTokenBtnTextActive: {
    color: color.fg.inverse,
  },
}));
