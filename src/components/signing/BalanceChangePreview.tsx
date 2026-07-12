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
import { useLocalePrefs, numberSeparators } from '@/services/locale-format';
import type { AssetChange, AssetSimResult } from '@/services/tx-simulation';
import { scaleFont, color, text, inter, space, radius, createStyles } from '@/constants/theme';

/**
 * Factual, non-promissory summary of a simulation for the 技术细节 panel —
 * "−1,000 USDC · 无其他变动". Returns null for a degraded/reverting/underfunded
 * sim (those are surfaced loudly by the component, not as a calm detail row).
 * `noChange` = the sim ran and confirmed nothing moved; `corroborated` = every
 * simulated change is exactly a declared hero flow (so "no other changes" holds).
 */
export function summariseSimResult(
  result: AssetSimResult | null,
  heroFlows: { token?: string; dir: 'out' | 'in' }[],
  sep: { group: string; decimal: string; indian?: boolean },
): { parts: string[]; corroborated: boolean; noChange: boolean } | null {
  if (!result || !result.ok || result.underfundedNative) return null;
  const changes = result.changes;
  if (!changes) return null; // degraded — nothing to report
  if (changes.length === 0) return { parts: [], corroborated: false, noChange: true };
  const parts = changes.map((c) =>
    c.unverified
      ? `${c.delta > 0n ? '+' : '−'}? ${c.symbol ?? '?'}`
      : `${c.delta > 0n ? '+' : '−'}${formatTokenAmount(c.delta, c.decimals ?? 18, 6, sep)}${c.symbol ? ` ${c.symbol}` : ''}`,
  );
  const corroborated =
    heroFlows.length > 0 &&
    !changes.some((c) => c.unverified) &&
    changes.every((c) =>
      heroFlows.some((h) =>
        h.token === (c.token?.toLowerCase() ?? undefined) && (h.dir === 'out' ? c.delta < 0n : c.delta > 0n),
      ),
    );
  return { parts, corroborated, noChange: false };
}

export function BalanceChangePreview({ result, chainId, selfTransfer, heroFlows = [], hideReassurance = false }: {
  result: AssetSimResult | null;
  chainId: number;
  /** Suppress the quiet green "nothing else leaves your wallet" reassurance — the
      signing sheet moves that (as a neutral factual "模拟结果" row) into 技术细节,
      to avoid a prominent promise that reads as a guarantee. LOUD states (revert,
      underfunded, unexpected changes) are unaffected. Default false keeps the
      reassurance for Send's confirm step and the connection-event detail sheet. */
  hideReassurance?: boolean;
  /** Recipient == sender. A self-send nets to zero, so say so honestly instead
      of the generic "no assets leave your wallet" (which reads as nonsense when
      you're clearly sending a token to yourself). */
  selfTransfer?: boolean;
  /** The asset movements the decoded HERO already shows, each as {token, dir}
      (token lowercased; undefined = native coin). The sim collapses to a quiet ✓
      ONLY when EVERY simulated change is corroborated by a same-token, same-
      direction hero flow AND none is `unverified` — so an undeclared outflow, a
      swapped-token identity mismatch, or an unverified-decimals caution can never
      hide behind the checkmark. Any unmatched movement expands the full list.
      Approvals / permits / batches pass [] (they never corroborate a balance move). */
  heroFlows?: { token?: string; dir: 'out' | 'in' }[];
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
  // unless it's underfunded (then the banner is the whole message). When the
  // signing sheet asks to hide the reassurance, this becomes a factual row in
  // 技术细节 instead of a green promise here.
  if (!hasChanges) {
    if (underfunded) return underfunded;
    if (hideReassurance) return null;
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

  // Collapse to a quiet ✓ ONLY when the simulation is pure corroboration of the
  // decoded hero: every change maps to a same-token, same-direction hero flow and
  // none is unverified. A count budget would let an undeclared outflow, a swapped
  // output token, or an unverified-decimals caution hide behind the ✓ — this checks
  // identity + direction per change instead, so any unmatched movement expands below.
  const corroborated =
    heroFlows.length > 0 &&
    !changes!.some((c) => c.unverified) &&
    changes!.every((c) =>
      heroFlows.some((h) =>
        h.token === (c.token?.toLowerCase() ?? undefined) &&
        (h.dir === 'out' ? c.delta < 0n : c.delta > 0n),
      ),
    );
  if (!underfunded && corroborated) {
    if (hideReassurance) return null; // shown as a factual "模拟结果" row in 技术细节
    return (
      <View style={styles.okRow}>
        <ShieldCheck size={13} color={color.success.base} strokeWidth={2} />
        {/* Outflows only — the received side is spoofable, so we never imply it
            was corroborated (safety review). */}
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
  useLocalePrefs();
  const sep = numberSeparators();
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
            {received ? '+' : '−'}{formatTokenAmount(change.delta, change.decimals ?? 18, 6, sep)}
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
