/**
 * Reusable gas fee display + tier selector.
 *
 * Shared by SendScreen (native transfers, ERC-20) and SigningRequestModal
 * (dApp contract calls). Shows:
 *   - Collapsed: "Est. Fee ~0.0012 POL ≈ $0.003"
 *   - Expanded: tier buttons (Slow/Standard/Rapid/Fast), gas details
 */

import React, { useCallback, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { ActivityIndicator, Pressable, Text, View } from 'react-native';
import { ChevronDown, ChevronUp, RefreshCw } from 'lucide-react-native';
import { VelaCard } from './VelaCard';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import {
  estimateTransactionFee,
  refreshGasPrice,
  GAS_TIER_MULTIPLIERS,
  type GasTier,
  type TransactionFeeEstimate,
} from '@/services/safe-transaction';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function formatWeiToEth(wei: bigint): string {
  const eth = Number(wei) / 1e18;
  if (eth === 0) return '0';
  if (eth < 0.0001) return '< 0.0001';
  return eth.toFixed(4).replace(/\.?0+$/, '');
}

function formatUsd(v: number): string {
  if (v < 0.01) return `$${v.toFixed(4)}`;
  return `$${v.toFixed(2)}`;
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
  /** Safe wallet address (for re-estimation). */
  safeAddress: string;
  /** Chain ID. */
  chainId: number;
  /** Current gas tier. */
  gasTier: GasTier;
  /** Called when user changes tier. Parent should re-estimate and update feeEstimate. */
  onTierChange: (tier: GasTier) => void;
  /** Called when fee estimate is updated (after refresh or tier change). */
  onFeeUpdate: (fee: TransactionFeeEstimate) => void;
}

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------

export function GasFeeCard({
  feeEstimate, estimating, nativeSymbol: sym, nativeUsdPrice,
  safeAddress, chainId, gasTier, onTierChange, onFeeUpdate,
}: GasFeeCardProps) {
  const [expanded, setExpanded] = useState(false);
  const [refreshing, setRefreshing] = useState(false);

  const feeNative = feeEstimate ? Number(feeEstimate.totalWei) / 1e18 : 0;
  const feeUsd = feeNative * nativeUsdPrice;

  const handleRefresh = useCallback(async () => {
    if (refreshing) return;
    setRefreshing(true);
    try {
      await refreshGasPrice(chainId);
      const fee = await estimateTransactionFee(safeAddress, chainId, gasTier);
      onFeeUpdate(fee);
    } catch { /* ignore */ }
    setRefreshing(false);
  }, [chainId, safeAddress, gasTier, refreshing, onFeeUpdate]);

  const handleTierChange = useCallback(async (tier: GasTier) => {
    onTierChange(tier);
    try {
      const fee = await estimateTransactionFee(safeAddress, chainId, tier);
      onFeeUpdate(fee);
    } catch { /* ignore */ }
  }, [safeAddress, chainId, onTierChange, onFeeUpdate]);

  const { t } = useTranslation();

  return (
    <>
      {/* Collapsed toggle row */}
      <Pressable onPress={() => setExpanded(!expanded)} style={styles.toggleRow}>
        <Text style={styles.toggleLabel}>{t('componentsUi.gas.estFee')}</Text>
        <View style={styles.toggleRight}>
          <View style={styles.toggleValues}>
            <Text style={styles.toggleValue}>
              {estimating ? 'Estimating...' : feeEstimate ? `~${formatWeiToEth(feeEstimate.totalWei)} ${sym}` : '—'}
            </Text>
            {!estimating && feeUsd > 0.001 && (
              <Text style={styles.toggleSub}>≈ {formatUsd(feeUsd)}</Text>
            )}
          </View>
          {feeEstimate && !estimating && (
            expanded
              ? <ChevronUp size={16} color={color.fg.subtle} strokeWidth={2} />
              : <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2} />
          )}
        </View>
      </Pressable>

      {/* Expanded gas details */}
      {expanded && feeEstimate && (() => {
        const bundlerGwei = Number(feeEstimate.bundlerGasPrice) / 1e9;
        const userOpGwei = Number(feeEstimate.maxFeePerGas) / 1e9;

        return (
          <VelaCard style={styles.gasCard}>
            {/* Tier selector */}
            <View style={styles.tierRow}>
              {(['slow', 'standard', 'rapid', 'fast'] as GasTier[]).map((t) => (
                <Pressable
                  key={t}
                  style={[styles.tierBtn, gasTier === t && styles.tierBtnActive]}
                  onPress={() => handleTierChange(t)}
                >
                  <Text style={[styles.tierBtnText, gasTier === t && styles.tierBtnTextActive]}>
                    {GAS_TIER_MULTIPLIERS[t].label}
                  </Text>
                </Pressable>
              ))}
              <Pressable onPress={handleRefresh} hitSlop={8} style={styles.tierRefresh}>
                {refreshing ? (
                  <ActivityIndicator size={14} color={color.fg.muted} />
                ) : (
                  <RefreshCw size={14} color={color.fg.muted} strokeWidth={2} />
                )}
              </Pressable>
            </View>
            <View style={styles.separator} />
            <DetailRow label={t('componentsUi.gas.gasPrice')} value={`${bundlerGwei.toFixed(4)} Gwei`} />
            <View style={styles.separator} />
            <DetailRow label={t('componentsUi.gas.gasPriceUserOp')} value={`${userOpGwei.toFixed(4)} Gwei`} />
            <View style={styles.separator} />
            <DetailRow label={t('componentsUi.gas.gasLimit')} value={feeEstimate.totalGas.toLocaleString()} />
            <View style={styles.separator} />
            <DetailRow label={t('componentsUi.gas.walletDeployed')} value={feeEstimate.deployed ? t('componentsUi.gas.deployedYes') : t('componentsUi.gas.deployedNo')} />
          </VelaCard>
        );
      })()}
    </>
  );
}

function DetailRow({ label, value }: { label: string; value: string }) {
  return (
    <View style={styles.detailRow}>
      <Text style={styles.detailLabel}>{label}</Text>
      <Text style={styles.detailValue}>{value}</Text>
    </View>
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
    paddingHorizontal: space.sm,
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
  toggleSub: {
    fontSize: text.xs,
    ...inter.regular,
    color: color.fg.subtle,
  },
  gasCard: {
    padding: space.xl,
    marginBottom: space.lg,
  },
  tierRow: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingBottom: space.lg,
  },
  tierBtn: {
    flex: 1,
    paddingVertical: space.sm,
    borderRadius: radius.md,
    alignItems: 'center',
    backgroundColor: color.bg.sunken,
  },
  tierBtnActive: {
    backgroundColor: color.fg.base,
  },
  tierBtnText: {
    fontSize: text.xs,
    ...inter.semibold,
    color: color.fg.muted,
  },
  tierBtnTextActive: {
    color: color.fg.inverse,
  },
  tierRefresh: {
    padding: space.sm,
  },
  separator: {
    height: 1,
    backgroundColor: color.border.base,
  },
  detailRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    alignItems: 'center',
    paddingVertical: space.lg,
  },
  detailLabel: {
    fontSize: text.sm,
    ...inter.regular,
    color: color.fg.muted,
  },
  detailValue: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.base,
    fontFamily: font.mono,
  },
}));
