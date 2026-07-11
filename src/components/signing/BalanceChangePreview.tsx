/**
 * Balance-change preview — the single render path for a transaction simulation
 * summary. Shows, from one `AssetSimResult`:
 *   - a loud "expected to fail" card (you'd still pay gas), or
 *   - the wallet's net asset changes (+ received in green, − sent in neutral ink), or
 *   - a quiet "expected to succeed / no assets leave your wallet" reassurance.
 *
 * Used by both the dApp signing sheet and Send's confirm step so the simulation
 * reads identically wherever a transaction is about to leave the wallet.
 *
 * Safety: an *unverified* token (on-chain decimals couldn't be confirmed) is
 * shown with its direction and a clear "unverified" tag but NO scaled amount —
 * a wrong decimal scale here would misstate value by orders of magnitude.
 */
import React from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { AlertTriangle, ShieldCheck, ArrowDownLeft, ArrowUpRight } from 'lucide-react-native';
import { TokenLogo } from '@/components/TokenLogo';
import { shortAddr, tokenLogoURLsByAddress } from '@/models/types';
import { nativeSymbol, nativeCoinLogoURL } from '@/models/network';
import { formatTokenAmount } from '@/services/sim-assets';
import type { AssetChange, AssetSimResult } from '@/services/tx-simulation';
import { scaleFont, color, text, inter, space, radius, createStyles } from '@/constants/theme';

export function BalanceChangePreview({ result, chainId, selfTransfer, heroFlowCount = 0 }: {
  result: AssetSimResult | null;
  chainId: number;
  /** Recipient == sender. A self-send nets to zero, so say so honestly instead
      of the generic "no assets leave your wallet" (which reads as nonsense when
      you're clearly sending a token to yourself). */
  selfTransfer?: boolean;
  /** How many asset movements the decoded HERO already shows (send + receive
      amount fields). When the simulation moves no MORE than that, it's pure
      corroboration → collapse to a quiet ✓ instead of repeating the amounts.
      If the sim reveals extra/unexpected movement it still expands in full. */
  heroFlowCount?: number;
}) {
  const { t } = useTranslation();
  if (!result) return null;

  // Expected to revert — the highest-signal state. Loud, reason when available.
  if (!result.ok) {
    return (
      <View style={styles.failCard}>
        <AlertTriangle size={16} color={color.error.base} strokeWidth={2} />
        <Text style={styles.failText}>
          {result.revertReason
            ? t('componentsUi.signing.simWillFailReason', { reason: result.revertReason })
            : t('componentsUi.signing.simWillFail')}
        </Text>
      </View>
    );
  }

  const changes = result.changes;
  const hasChanges = !!changes && changes.length > 0;

  // The sim says success, but a native outflow exceeds the real balance — it'll
  // fail on-chain. Loud, and it replaces the green reassurance.
  const underfunded = result.underfundedNative ? (
    <View style={styles.failCard}>
      <AlertTriangle size={16} color={color.error.base} strokeWidth={2} />
      <Text style={styles.failText}>
        {t('componentsUi.signing.balanceUnderfundedNative', { symbol: nativeSymbol(chainId) })}
      </Text>
    </View>
  ) : null;

  // Ran successfully but nothing moved (e.g. an approval) — reassure plainly,
  // unless it's underfunded (then the banner is the whole message).
  if (!hasChanges) {
    if (underfunded) return underfunded;
    const msg = selfTransfer // sending to your own address — net zero, but be explicit
      ? t('componentsUi.signing.balanceSelfTransfer')
      : changes // [] = engine ran and confirmed no movement; null = degraded
      ? t('componentsUi.signing.balanceNoAssetsMove')
      : t('componentsUi.signing.simWillSucceed');
    return (
      <View style={styles.okRow}>
        <ShieldCheck size={13} color={color.success.base} strokeWidth={2} />
        <Text style={styles.okText}>{msg}</Text>
      </View>
    );
  }

  // The hero already showed the decoded flow and the sim moved no more than it —
  // pure corroboration. Collapse to a quiet ✓ instead of repeating the amounts
  // (the flagship de-duplication). Extra/unexpected movement (changes beyond what
  // the hero showed) is NOT corroboration → fall through to the full list.
  if (!underfunded && heroFlowCount > 0 && changes!.length <= heroFlowCount) {
    return (
      <View style={styles.okRow}>
        <ShieldCheck size={13} color={color.success.base} strokeWidth={2} />
        <Text style={styles.okText}>{t('componentsUi.signing.balanceMatchesHero')}</Text>
      </View>
    );
  }

  // Sort received-first for a consistent, scannable layout.
  const ordered = [...changes].sort((a, b) => Number(b.delta > 0n) - Number(a.delta > 0n));

  return (
    <>
      {underfunded}
      <View style={styles.card}>
        <Text style={styles.title}>{t('componentsUi.signing.balanceChangesTitle')}</Text>
        {ordered.map((c, i) => (
          <ChangeRow key={`${c.kind}:${c.token ?? 'native'}:${i}`} change={c} chainId={chainId} />
        ))}
      </View>
    </>
  );
}

function ChangeRow({ change, chainId }: { change: AssetChange; chainId: number }) {
  const { t } = useTranslation();
  const received = change.delta > 0n;
  const tint = received ? color.success.base : color.fg.base;
  // Native coins get the real coin logo (ETH on Base → Ethereum's), ERC-20s the
  // token logo by (chainId, address) — checksummed first, lowercase fallback,
  // same source the rest of the app uses; a bare letter avatar is the last resort.
  const nativeLogo = change.kind === 'native' ? nativeCoinLogoURL(chainId) : undefined;
  const tokenLogos = change.kind !== 'native' && change.token
    ? tokenLogoURLsByAddress(chainId, change.token)
    : undefined;

  return (
    <View style={styles.row}>
      <TokenLogo symbol={change.symbol ?? '?'} logoUrl={nativeLogo} logoUrls={tokenLogos} size={28} />
      <View style={styles.rowInfo}>
        {change.unverified ? (
          // Decimals unknown → never imply a precise amount. Direction + caveat only.
          <>
            <View style={styles.dirLine}>
              {received
                ? <ArrowDownLeft size={13} color={color.warning.base} strokeWidth={2.5} />
                : <ArrowUpRight size={13} color={color.warning.base} strokeWidth={2.5} />}
              <Text style={styles.unverifiedTag}>{t('componentsUi.signing.balanceUnverifiedToken')}</Text>
            </View>
            <Text style={styles.rowSub}>{change.token ? shortAddr(change.token) : ''}</Text>
          </>
        ) : (
          <Text style={[styles.amount, { color: tint }]} numberOfLines={1}>
            {received ? '+' : '−'}{formatTokenAmount(change.delta, change.decimals ?? 18)}
            {change.symbol ? ` ${change.symbol}` : ''}
          </Text>
        )}
      </View>
    </View>
  );
}

const styles = createStyles(() => ({
  // De-containered (Wise): the balance-change list sits on an open block split
  // from the content above by a hairline, not a gray card.
  card: {
    paddingVertical: space.lg,
    borderTopWidth: 1,
    borderTopColor: color.border.base,
    marginTop: space.md,
    gap: space.sm,
  },
  title: {
    fontSize: scaleFont(10), ...inter.semibold, color: color.fg.subtle,
    textTransform: 'uppercase', letterSpacing: 0.3, marginBottom: space.xs,
  },
  row: {
    flexDirection: 'row', alignItems: 'center', gap: space.lg,
    paddingVertical: space.sm,
  },
  rowInfo: { flex: 1, gap: 1 },
  amount: { fontSize: text.base, ...inter.semibold, letterSpacing: -0.3 },
  dirLine: { flexDirection: 'row', alignItems: 'center', gap: space.xs },
  unverifiedTag: { fontSize: text.sm, ...inter.semibold, color: color.warning.base },
  rowSub: {
    fontSize: text.xs, fontWeight: '500' as const, color: color.fg.muted,
  },

  // ===== Expected-to-fail card =====
  failCard: {
    flexDirection: 'row', alignItems: 'center', gap: space.md,
    paddingVertical: space.lg, paddingHorizontal: space.xl,
    backgroundColor: color.error.soft, borderWidth: 1, borderColor: color.error.base,
    borderRadius: radius.xl, marginVertical: space.md,
  },
  failText: { fontSize: text.sm, ...inter.semibold, color: color.error.base, flex: 1, lineHeight: 18 },

  // ===== Quiet success row =====
  okRow: {
    flexDirection: 'row', alignItems: 'center', gap: space.sm,
    paddingVertical: space.sm, paddingHorizontal: space.sm, marginBottom: space.xs,
  },
  okText: { fontSize: text.xs, ...inter.medium, color: color.success.base },
}));
