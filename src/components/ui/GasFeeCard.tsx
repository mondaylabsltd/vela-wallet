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
 * The card OWNS the fee-asset option loading (the bundler's all-asset quote);
 * the selected token itself is controlled by the parent (`gasFeeToken` +
 * `onFeeTokenChange`) so the approve/submit path sends exactly what was quoted.
 * Tempo uses the same fee-asset UI. Its special transaction envelope stays in the
 * service layer, where it belongs.
 */

import React, { useCallback, useEffect, useRef, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { ChevronDown, ChevronUp, RefreshCw } from 'lucide-react-native';
import { color, createStyles, inter, space, text } from '@/constants/theme';
import { useDisplayCurrency } from '@/hooks/use-display-currency';
import { useLocalePrefs, numberSeparators, formatNumber } from '@/services/locale-format';
import {
  calculateInBandFeeAmount,
  estimateTransactionFee,
  refreshGasPrice,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';
import { isTempoChain, tempoReimbursement } from '@/services/tempo';
import { useInBandFeeTokenOptions, type FeeTokenOption } from '@/hooks/use-inband-fee-tokens';
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
  /** Passkey public key used to construct initCode for an undeployed Safe. */
  publicKeyHex?: string;
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
  /** Fires while the card updates its fee calculation (fee-asset switch / refresh), so the
   *  parent can gate its confirm button — the internal update doesn't touch the
   *  parent's `estimating` flag. */
  onBusyChange?: (busy: boolean) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function GasFeeCard({
  feeEstimate, estimating, nativeSymbol: sym, nativeUsdPrice,
  safeAddress, chainId, publicKeyHex, tx, batchCalls, gasFeeToken = null, onFeeTokenChange, onFeeUpdate, onBusyChange,
}: GasFeeCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);
  // Set the internal update flag AND notify the parent (which gates its confirm button).
  const setBusy = useCallback((b: boolean) => {
    setRefreshing(b);
    onBusyChange?.(b);
  }, [onBusyChange]);
  // Fee-asset options: null = no selector (legacy chain, Tempo, or unavailable quote).

  // Region format: fiat in the chosen display currency (€/¥/…) + the number
  // format's grouping/decimal marks, matching the amounts everywhere else.
  const dc = useDisplayCurrency();
  useLocalePrefs();
  const sep = numberSeparators();

  // Fee-asset options — native + held whitelisted stables — from the shared all-asset quote
  // loader used by the Send confirm slide too, so the two surfaces cannot drift.
  const feeTokenOptions = useInBandFeeTokenOptions(chainId, safeAddress, true);

  // There is no longer a separate per-token request to make after this quote. As soon as it
  // tells us that the Safe has a choice, reveal that choice instead of leaving it hidden behind
  // the compact gas row. Reset only for a different account/chain, so a user can still collapse
  // it themselves while reviewing the same transaction.
  const didRevealOptionsRef = useRef(false);
  useEffect(() => {
    didRevealOptionsRef.current = false;
    setExpanded(false);
  }, [chainId, safeAddress]);
  useEffect(() => {
    if (!didRevealOptionsRef.current && (feeTokenOptions?.length ?? 0) > 1) {
      didRevealOptionsRef.current = true;
      setExpanded(true);
    }
  }, [feeTokenOptions]);

  // The address-only in-band quote carries balances and USD prices for every asset. The shared
  // gas basis derives exact amounts once it is ready; the estimate fallback keeps first render stable.
  const erc20Fee = feeEstimate?.feeAsset?.kind === 'erc20' ? feeEstimate.feeAsset : null;
  // Tempo's default is an ERC-20 fee token, not a synthetic native USD balance. Keep it
  // visibly selected even before the relay publishes additional Tempo fee-token choices.
  const selectedFeeToken = gasFeeToken ?? (isTempoChain(chainId) ? erc20Fee?.token ?? null : null);
  const selectedOption = feeTokenOptions?.find(
    (o) => (o.contract?.toLowerCase() ?? null) === (selectedFeeToken?.toLowerCase() ?? null),
  );
  const nativeOption = feeTokenOptions?.find((o) => o.asset === 'native');
  const feeAmountForOption = useCallback((option: FeeTokenOption): bigint | null => {
    if (!feeEstimate?.inBand) return null;
    if (isTempoChain(chainId)) {
      // Tempo's outer 0x76 is protocol-specific, but its fee is still a normal USD TIP-20
      // amount derived from the same gas basis shown to the user.
      return option.asset === 'erc20'
        ? tempoReimbursement(feeEstimate.totalGas, feeEstimate.networkFeePerGas, option.decimals)
        : null;
    }
    if (!nativeOption) return null;
    return calculateInBandFeeAmount(
      feeEstimate.totalGas,
      feeEstimate.networkFeePerGas,
      option,
      nativeOption,
    );
  }, [chainId, feeEstimate, nativeOption]);
  const erc20Symbol = erc20Fee
    ? (feeTokenOptions?.find((o) => o.contract?.toLowerCase() === erc20Fee.token.toLowerCase())?.symbol
      ?? erc20Fee.symbol
      ?? `${erc20Fee.token.slice(0, 6)}…`)
    : null;
  const selectedFeeAmount = selectedOption ? feeAmountForOption(selectedOption) : null;
  const quoteUnits = selectedOption && selectedFeeAmount !== null
    ? Number(selectedFeeAmount) / 10 ** selectedOption.decimals
    : null;
  const feeUnits = quoteUnits ?? (erc20Fee
    ? Number(erc20Fee.amount) / 10 ** erc20Fee.decimals
    : feeEstimate ? Number(feeEstimate.totalWei) / 1e18 : 0);
  const feeUsd = selectedOption
    ? feeUnits * Number(selectedOption.usdPrice)
    : erc20Fee ? feeUnits : feeUnits * nativeUsdPrice;
  const feeSym = selectedOption?.symbol ?? (erc20Fee ? erc20Symbol : sym);
  // Show fiat only when it renders as a meaningful non-zero (formatFiat rounds to
  // 2 dp) — below that the native amount is the honest primary.
  const showFiat = feeUsd >= 0.005;

  const handleRefresh = useCallback(async () => {
    if (refreshing) return;
    setBusy(true);
    try {
      await refreshGasPrice(chainId);
      const fee = await estimateTransactionFee(
        safeAddress, chainId, 'fast', tx, batchCalls, gasFeeToken, publicKeyHex,
      );
      onFeeUpdate(fee);
    } catch { /* ignore */ }
    setBusy(false);
  }, [chainId, safeAddress, publicKeyHex, tx, batchCalls, gasFeeToken, refreshing, onFeeUpdate, setBusy]);

  const handleFeeTokenSelect = useCallback(async (contract: string | null) => {
    const prev = gasFeeToken ?? null;
    if ((prev?.toLowerCase() ?? null) === (contract?.toLowerCase() ?? null)) return;
    // The selected asset's exact amount is derived from the shared gas basis, so the display can
    // switch immediately while confirm remains gated through this state update.
    onFeeTokenChange?.(contract);
    setBusy(true);
    try {
      // One response already includes every selectable asset's recipient, balance, and USD price.
      // Selecting a chip recalculates locally — no per-token balance, metadata, or price request.
      const option = feeTokenOptions?.find(
        (o) => (o.contract?.toLowerCase() ?? null) === (contract?.toLowerCase() ?? null),
      );
      const amount = option ? feeAmountForOption(option) : null;
      if (feeEstimate?.inBand && option && amount !== null) {
        onFeeUpdate({
          ...feeEstimate,
          totalWei: option.contract === null ? amount : 0n,
          feeRecipient: option.recipient,
          feeAsset: option.contract === null
            ? { kind: 'native' }
            : { kind: 'erc20', token: option.contract, decimals: option.decimals, amount },
        });
      } else {
        // A quote may have expired while this sheet remained open. Fall back to a full estimate;
        // its quote call still returns all assets in one response.
        const fee = await estimateTransactionFee(
          safeAddress, chainId, 'fast', tx, batchCalls, contract, publicKeyHex,
        );
        onFeeUpdate(fee);
      }
    } catch {
      onFeeTokenChange?.(prev); // error → revert; the user can re-tap or refresh
    }
    setBusy(false);
  }, [safeAddress, chainId, publicKeyHex, tx, batchCalls, gasFeeToken, feeEstimate, feeTokenOptions, feeAmountForOption, onFeeTokenChange, onFeeUpdate, setBusy]);

  // Auto-default the fee asset to one the user can actually pay with. The selection starts at
  // native (gasFeeToken=null), but if the native coin can't cover the fee — notably a 0-balance
  // account (which is common now that gas is paid in a held stablecoin) — that row is
  // non-selectable, so leaving it selected is a dead-end. Pre-select the first AFFORDABLE option
  // instead. Runs at most once per (chain, account); a user's later manual pick is never overridden.
  const didAutoDefaultRef = useRef(false);
  useEffect(() => { didAutoDefaultRef.current = false; }, [chainId, safeAddress]);
  useEffect(() => {
    if (didAutoDefaultRef.current) return;
    if (!feeTokenOptions || feeTokenOptions.length === 0) return;
    const affordable = (o: FeeTokenOption): boolean => {
      const amount = feeAmountForOption(o);
      return amount !== null && o.balance >= amount;
    };
    const selKey = selectedFeeToken?.toLowerCase() ?? null;
    const current = feeTokenOptions.find((o) => (o.contract?.toLowerCase() ?? null) === selKey);
    if (current && affordable(current)) { didAutoDefaultRef.current = true; return; } // current is fine
    const pick = feeTokenOptions.find(affordable);
    if (pick) {
      didAutoDefaultRef.current = true;
      handleFeeTokenSelect(pick.contract);
    }
  }, [feeTokenOptions, selectedFeeToken, feeAmountForOption, handleFeeTokenSelect]);

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
            <Text style={styles.toggleLabelSub}>{t('componentsUi.gas.paidWith', { symbol: feeSym })}</Text>
          )}
        </View>
        <View style={styles.toggleRight}>
          <View style={styles.toggleValues}>
            {/* Token-first: the precise amount from the selected quote leads; the quote-supplied
                USD price produces the quiet approximation below. */}
            <Text style={[styles.toggleValue, failed && styles.toggleValueFailed]}>
              {feeEstimate
                ? `~${formatFeeAmount(feeUnits, sep)} ${feeSym}`
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
          selected={selectedFeeToken}
          onSelect={handleFeeTokenSelect}
          feeAmountFor={feeAmountForOption}
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
