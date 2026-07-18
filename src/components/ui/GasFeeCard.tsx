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

import React, { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { ChevronDown, ChevronUp, RefreshCw } from 'lucide-react-native';
import { color, createStyles, inter, space, text } from '@/constants/theme';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { useLocalePrefs, numberSeparators, formatNumber } from '@/services/locale-format';
import {
  estimateTransactionFee,
  refreshGasPrice,
  requoteInBandFee,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';
import { useInBandFeeTokenOptions } from '@/hooks/use-inband-fee-tokens';
import { FeeTokenSelector } from './FeeTokenSelector';

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
  /** Fires while the card re-quotes internally (fee-asset switch / refresh), so the
   *  parent can gate its confirm button — the internal re-quote doesn't touch the
   *  parent's `estimating` flag. */
  onBusyChange?: (busy: boolean) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function GasFeeCard({
  feeEstimate, estimating, nativeSymbol: sym, nativeUsdPrice,
  safeAddress, chainId, tx, batchCalls, gasFeeToken = null, onFeeTokenChange, onFeeUpdate, onBusyChange,
}: GasFeeCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  // Set the internal re-quote flag AND notify the parent (which gates its confirm button).
  const setBusy = useCallback((b: boolean) => {
    setRefreshing(b);
    onBusyChange?.(b);
  }, [onBusyChange]);
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

  // Drive the collapsed display from the SELECTION (gasFeeToken), not the quote's own asset —
  // so switching coins updates INSTANTLY instead of waiting on the bundler re-quote (which still
  // runs in the background to set the exact signed amount). The USD cost is ~coin-invariant
  // (uniform bundler markup), converted to the chosen coin; stables carry the $0.01 floor.
  const selNative = (gasFeeToken ?? null) === null;
  const selSym = selNative
    ? sym
    : (feeTokenOptions?.find((o) => o.contract?.toLowerCase() === gasFeeToken!.toLowerCase())?.symbol ?? feeSym);
  const selUnits = selNative
    ? (nativeUsdPrice > 0 ? feeUsd / nativeUsdPrice : feeUnits)
    : Math.max(feeUsd, 0.01);

  const handleRefresh = useCallback(async () => {
    if (refreshing) return;
    setBusy(true);
    try {
      await refreshGasPrice(chainId);
      const fee = await estimateTransactionFee(safeAddress, chainId, 'fast', tx, batchCalls, gasFeeToken);
      onFeeUpdate(fee);
    } catch { /* ignore */ }
    setBusy(false);
  }, [chainId, safeAddress, tx, batchCalls, gasFeeToken, refreshing, onFeeUpdate, setBusy]);

  const handleFeeTokenSelect = useCallback(async (contract: string | null) => {
    const prev = gasFeeToken ?? null;
    if ((prev?.toLowerCase() ?? null) === (contract?.toLowerCase() ?? null)) return;
    // Optimistic: the display switches to the new coin instantly (its cost is client-derived).
    // Confirm stays disabled (setBusy) until the authoritative quote lands, so the SIGNED amount
    // is never the previous coin's; on failure we revert the selection to keep display == signed.
    onFeeTokenChange?.(contract);
    setBusy(true);
    try {
      // Fast path: an asset switch doesn't change the gas basis — one bundler RPC.
      // HARD 12s ceiling so the flag always resolves (hung quote → revert to previous).
      const run = (async () => {
        const fast = feeEstimate ? await requoteInBandFee(feeEstimate, chainId, safeAddress, contract) : null;
        return fast ?? await estimateTransactionFee(safeAddress, chainId, 'fast', tx, batchCalls, contract);
      })();
      const fee = await Promise.race([run, new Promise<null>((r) => setTimeout(() => r(null), 12_000))]);
      if (fee) onFeeUpdate(fee);
      else onFeeTokenChange?.(prev); // re-quote failed → revert so the shown coin matches the quote
    } catch {
      onFeeTokenChange?.(prev); // error → revert; the user can re-tap or refresh
    }
    setBusy(false);
  }, [safeAddress, chainId, tx, batchCalls, gasFeeToken, feeEstimate, onFeeTokenChange, onFeeUpdate, setBusy]);

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
        <View style={styles.toggleLabelCol}>
          <Text style={styles.toggleLabel}>{t('componentsUi.gas.estFee')}</Text>
          {selectable && feeEstimate && (
            <Text style={styles.toggleLabelSub}>{t('componentsUi.gas.paidWith', { symbol: selSym })}</Text>
          )}
        </View>
        <View style={styles.toggleRight}>
          <View style={styles.toggleValues}>
            {/* Token-first: the PRECISE amount charged in the fee coin leads; the ≈fiat
                is the derived approximation on the quiet sub-line below. Once a quote exists we
                keep showing the (selection-derived) amount even while a re-quote is in flight —
                switching coins never blanks to "Estimating…". */}
            <Text style={[styles.toggleValue, failed && styles.toggleValueFailed]}>
              {feeEstimate
                ? `~${formatFeeAmount(selUnits, sep)} ${selSym}`
                : (estimating || refreshing)
                  ? t('componentsUi.gas.estimating')
                  : t('componentsUi.gas.estimateFailed')}
            </Text>
            {!failed && feeEstimate && showFiat && (
              <Text style={styles.toggleSub}>≈ {dc.fmt(feeUsd)}</Text>
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

      {/* Expanded fee-asset picker — one row per asset (native + held stables),
          each with its balance + ≈fiat. Shared with the Send confirm slide. */}
      {expanded && selectable && feeEstimate && (
        <FeeTokenSelector
          options={feeTokenOptions!}
          selected={gasFeeToken}
          onSelect={handleFeeTokenSelect}
          nativeUsdPrice={nativeUsdPrice}
          feeUsd={feeUsd}
          busy={refreshing}
        />
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
  toggleLabelCol: {
    gap: 2,
  },
  toggleLabel: {
    fontSize: text.sm,
    ...inter.medium,
    color: color.fg.muted,
  },
  toggleLabelSub: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
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
}));
