/**
 * Approval View — the editable, never-unlimited spending-cap surface.
 */
import React, { useEffect, useState } from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import {
  type DetectedApproval, type ApprovalChoice,
  formatTokenAmount as formatRawTokenAmount,
} from '@/services/approval-guard';
import { type ClearSignResult } from '@/services/clear-signing';
import { readErc20Allowance } from '@/services/token-reads';
import { knownContract } from '@/services/local-descriptors';
import { useLocalePrefs, numberSeparators } from '@/services/locale-format';
import { shortAddr, tokenLogoURLsByAddress } from '@/models/types';
import { styles } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { EditableApproveCard } from '../EditableApproveCard';
import { WarningBanner } from '../WarningBanner';
import { SummaryLine } from '../SummaryLine';

export function ApprovalView({ approval, meta, choice, onChange, chainId, walletAddress, clearSign, requestId }: {
  approval: DetectedApproval;
  meta: { symbol: string; decimals: number; verified: boolean } | null;
  choice: ApprovalChoice | null;
  onChange: (c: ApprovalChoice | null) => void;
  chainId: number;
  walletAddress?: string;
  clearSign: ClearSignResult | null;
  requestId: string;
}) {
  const { t } = useTranslation();
  const isNft = approval.kind === 'setApprovalForAll';

  // increaseAllowance adds to the EXISTING allowance — showing only the increment
  // is dangerously incomplete. Read the current on-chain allowance so we can show
  // the resulting total (current + increment). On a slow/failed read we still warn
  // the increment ADDS to an existing allowance rather than hiding the row.
  const [currentAllowance, setCurrentAllowance] = useState<bigint | null>(null);
  const [allowanceResolved, setAllowanceResolved] = useState(false);
  useEffect(() => {
    setCurrentAllowance(null);
    setAllowanceResolved(false);
    if (approval.kind !== 'increaseAllowance' || !walletAddress || !approval.tokenAddress) return;
    let cancelled = false;
    readErc20Allowance(chainId, approval.tokenAddress, walletAddress, approval.spender)
      .then((a) => { if (!cancelled) { setCurrentAllowance(a); setAllowanceResolved(true); } })
      .catch(() => { if (!cancelled) setAllowanceResolved(true); });
    return () => { cancelled = true; };
  }, [approval.kind, approval.tokenAddress, approval.spender, walletAddress, chainId]);

  const verb = approval.isReducing
    ? t('componentsUi.signingApprove.verbRevoke')
    : isNft && approval.isUnbounded
      ? t('componentsUi.signingApprove.verbApproveAll')
      : t('componentsUi.signingApprove.verbApprove');
  // Headline hue = meaning: green revoke, red only for a real-danger unbounded
  // grant, ink for a routine bounded approve (amber is reserved for the slider).
  const verbColor = approval.isReducing
    ? color.success.base
    : approval.isUnbounded
      ? color.error.base
      : color.fg.base;

  // Expiry classification (UI-side; the pure resolver injects `now` for tests).
  const deadlineSec = approval.deadline ? Number(approval.deadline) : 0;
  const nowSec = Math.floor(Date.now() / 1000);
  const expired = deadlineSec > 0 && deadlineSec < nowSec;

  useLocalePrefs();
  const sep = numberSeparators();
  const symbol = meta?.symbol ?? '…';
  const decimals = meta?.decimals ?? 18;
  const logoUrls = approval.tokenAddress
    ? tokenLogoURLsByAddress(chainId, approval.tokenAddress)
    : undefined;

  // Amount in the user's number format — computed ONCE so the summary text and the
  // SummaryLine `emphasize` substring stay identical (verbatim match bolds it).
  const approveAmount = `${formatRawTokenAmount(approval.amountRaw ?? 0n, decimals, 6, sep)} ${symbol}`;
  // Plain-language one-liner — what this approval actually lets the spender do.
  const spenderName = clearSign?.contractName ?? knownContract(approval.spender)?.name ?? shortAddr(approval.spender);
  const summary = approval.isReducing
    ? t('componentsUi.signing.summaryRevoke', { spender: spenderName, token: symbol })
    : isNft && approval.isUnbounded
      ? t('componentsUi.signing.summaryApproveNft', { operator: spenderName })
      : approval.isUnbounded
        ? t('componentsUi.signing.summaryApproveUnlimited', { spender: spenderName, token: symbol })
        : t('componentsUi.signing.summaryApprove', { spender: spenderName, amount: approveAmount });
  // Neutral by default; only an unbounded grant warms the sentence to red.
  const summaryTone = approval.isUnbounded && !approval.isReducing ? 'danger' : 'neutral';

  return (
    <View>
      {/* The verb is always a small kicker — the summary + cap card are the
          headline. A dangerous unbounded grant (red) or a safe revoke (green)
          keeps its hue, but not a giant size that fights the summary for focus. */}
      <IntentHeader
        intent={verb}
        color={verbColor}
        variant="eyebrow"
        colorEyebrow={approval.isUnbounded || approval.isReducing}
      />

      <SummaryLine
        text={summary}
        tone={summaryTone}
        emphasize={[spenderName, approveAmount, symbol]}
      />

      <EditableApproveCard
        key={requestId}
        approval={approval}
        symbol={symbol}
        decimals={decimals}
        decimalsVerified={meta?.verified ?? false}
        logoUrls={logoUrls}
        spenderLabel={spenderName}
        choice={choice}
        onChange={onChange}
      />

      {/* increaseAllowance: the chosen value is an INCREMENT — surface the
          resulting total so "increase by 100" can't read as "cap at 100". When the
          current allowance couldn't be read, still say the increment ADDS to it. */}
      {approval.kind === 'increaseAllowance' && allowanceResolved && (() => {
        const dec = meta?.decimals ?? 18;
        const sym = meta?.symbol ?? '';
        // Revoke zeroes the allowance outright — the increment math no longer
        // applies, so the resulting total is simply 0 (not "current + increment").
        if (choice?.type === 'revoke') {
          return (
            <View style={styles.allowanceTotalRow}>
              <Text style={styles.allowanceTotalLabel}>{t('componentsUi.signingApprove.resultingTotal')}</Text>
              <Text style={styles.allowanceTotalValue}>{`0 ${sym}`}</Text>
            </View>
          );
        }
        const increment = choice?.type === 'amount' ? choice.amountRaw : (approval.amountRaw ?? 0n);
        return (
          <View style={styles.allowanceTotalRow}>
            <Text style={styles.allowanceTotalLabel}>{t('componentsUi.signingApprove.resultingTotal')}</Text>
            {currentAllowance !== null ? (
              <Text style={styles.allowanceTotalValue}>
                {`${formatRawTokenAmount(currentAllowance, dec, 6, sep)} + ${formatRawTokenAmount(increment, dec, 6, sep)} = ${formatRawTokenAmount(currentAllowance + increment, dec, 6, sep)} ${sym}`}
              </Text>
            ) : (
              <Text style={styles.allowanceTotalUnknown}>
                {t('componentsUi.signingApprove.resultingTotalUnknown', { amount: `${formatRawTokenAmount(increment, dec, 6, sep)} ${sym}` })}
              </Text>
            )}
          </View>
        );
      })()}

      {/* No standalone spender/operator/collection rows: the spender is already
          named in the summary + the cap card, and every raw address (spender,
          operator, collection contract) lives one tap away under 技术细节. Boxed
          identity rows here would just repeat what's already stated in plain words. */}

      {expired && (
        <WarningBanner severity="caution" text={t('componentsUi.signingApprove.expired')} />
      )}
    </View>
  );
}
