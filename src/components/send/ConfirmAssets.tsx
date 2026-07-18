import React, { useState } from 'react';
import { View, Text, Pressable } from 'react-native';
import Animated from 'react-native-reanimated';
import { ChevronDown, ChevronUp } from 'lucide-react-native';
import { TokenLogo } from '@/components/TokenLogo';
import { fadeInDown } from '@/constants/entering';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import type { Network } from '@/models/network';

export interface ConfirmAssetRow {
  key: string;
  symbol: string;
  logoUrls?: string[];
  /** Network for the bottom-right badge on the token logo (null when redundant). */
  chain: Network | null;
  /** Network name (single) or "Network · gas reserved" (sweep detail rows). */
  networkText: string;
  /** Per-token amount — shown on expanded sweep rows only (single carries it on the From/To rows). */
  amountText?: string;
  usdText?: string;
}

/**
 * The "what you're sending" block on the Send confirm screen. Placed BELOW the
 * recipient(s) so every mode shares one flow — sender → ↓ → recipient → assets:
 *
 * - ONE token  → a single quiet identity pill (symbol · network). The amount is
 *   already on the From/To rows, so it isn't repeated here.
 * - N tokens (sweep) → a collapsed pill showing the overlapping token logos (each
 *   keeps its bottom-right network badge) + count + fiat total; tap to expand into
 *   one detail row per token (logo · symbol · network · amount · ≈fiat).
 */
export function ConfirmAssets({ rows, countLabel, totalLabel }: {
  rows: ConfirmAssetRow[];
  /** e.g. "3 个代币" — collapsed multi-asset summary. */
  countLabel?: string;
  /** e.g. "≈ $248.37" — collapsed multi-asset fiat total. */
  totalLabel?: string;
}) {
  const [open, setOpen] = useState(false);
  if (rows.length === 0) return null;

  // Single asset — shown directly, no collapse (the amount lives on the From/To rows).
  if (rows.length === 1) {
    const r = rows[0];
    return (
      <View style={styles.chip}>
        <TokenLogo symbol={r.symbol} logoUrls={r.logoUrls} chain={r.chain} size={20} />
        <Text style={styles.chipText}>{r.symbol} · {r.networkText}</Text>
      </View>
    );
  }

  // Multiple assets — a collapsed cluster that expands to a per-token list.
  const CLUSTER_MAX = 4;
  const cluster = rows.slice(0, CLUSTER_MAX);
  const extra = rows.length - cluster.length;
  return (
    <View>
      <Pressable
        style={styles.chip}
        onPress={() => setOpen(o => !o)}
        hitSlop={8}
        accessibilityRole="button"
      >
        <View style={styles.cluster}>
          {cluster.map((r, i) => (
            <View key={r.key} style={[styles.clusterItem, i > 0 && styles.clusterOverlap]}>
              <TokenLogo symbol={r.symbol} logoUrls={r.logoUrls} chain={r.chain} size={22} />
            </View>
          ))}
          {extra > 0 && (
            <View style={[styles.clusterItem, styles.clusterOverlap, styles.clusterMore]}>
              <Text style={styles.clusterMoreText}>+{extra}</Text>
            </View>
          )}
        </View>
        <Text style={styles.chipText} numberOfLines={1}>
          {countLabel}{totalLabel ? ` · ${totalLabel}` : ''}
        </Text>
        {open
          ? <ChevronUp size={16} color={color.fg.subtle} strokeWidth={2} />
          : <ChevronDown size={16} color={color.fg.subtle} strokeWidth={2} />}
      </Pressable>

      {open && (
        <Animated.View entering={fadeInDown(0, 200)} style={styles.list}>
          {rows.map(r => (
            <View key={r.key} style={styles.row}>
              <TokenLogo symbol={r.symbol} logoUrls={r.logoUrls} chain={r.chain} size={36} />
              <View style={styles.rowIdentity}>
                <Text style={styles.rowSymbol}>{r.symbol}</Text>
                <Text style={styles.rowChain}>{r.networkText}</Text>
              </View>
              {r.amountText ? (
                <View style={styles.rowValues}>
                  <Text style={styles.rowAmount}>{r.amountText}</Text>
                  {r.usdText ? <Text style={styles.rowSub}>{r.usdText}</Text> : null}
                </View>
              ) : null}
            </View>
          ))}
        </Animated.View>
      )}
    </View>
  );
}

const CLUSTER_LOGO = 22;

const styles = createStyles(() => ({
  // Same quiet pill as the old single-asset chip — a de-boxed sunken row that
  // sits left-aligned below the recipient.
  chip: {
    flexDirection: 'row',
    alignItems: 'center',
    alignSelf: 'flex-start',
    gap: space.sm,
    backgroundColor: color.bg.sunken,
    borderRadius: radius.full,
    paddingVertical: space.sm,
    paddingLeft: space.sm,
    paddingRight: space.lg,
    marginTop: space.lg,
  },
  chipText: {
    fontSize: text.sm,
    ...inter.semibold,
    color: color.fg.muted,
  },
  cluster: {
    flexDirection: 'row',
    alignItems: 'center',
  },
  // A ring in the pill's own colour so overlapping logos read as separate discs.
  clusterItem: {
    borderRadius: radius.full,
    borderWidth: 2,
    borderColor: color.bg.sunken,
  },
  clusterOverlap: {
    marginLeft: -8,
  },
  clusterMore: {
    width: CLUSTER_LOGO + 4,
    height: CLUSTER_LOGO + 4,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: color.bg.raised,
  },
  clusterMoreText: {
    fontSize: text.xs,
    ...inter.bold,
    color: color.fg.muted,
  },
  list: {
    marginTop: space.sm,
  },
  // Open detail row (de-boxed, matches the enter-details / sweep list style).
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.md,
    paddingVertical: space.md,
  },
  rowIdentity: {
    flex: 1,
    minWidth: 0,
    gap: 1,
  },
  rowSymbol: {
    fontSize: text.base,
    ...inter.bold,
    color: color.fg.base,
  },
  rowChain: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },
  rowValues: {
    alignItems: 'flex-end',
    gap: 1,
    flexShrink: 0,
  },
  rowAmount: {
    fontSize: text.base,
    ...inter.bold,
    fontFamily: font.numeric,
    color: color.fg.base,
  },
  rowSub: {
    fontSize: text.xs,
    ...inter.medium,
    color: color.fg.subtle,
  },
}));
