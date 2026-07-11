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
import { shortAddr, tokenLogoURLsByAddress } from '@/models/types';
import { styles } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { EditableApproveCard } from '../EditableApproveCard';
import { ContractBar } from '../ContractBar';
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
  const verbColor = approval.isReducing
    ? color.success.base
    : approval.isUnbounded
      ? color.error.base
      : color.warning.base;

  // Expiry classification (UI-side; the pure resolver injects `now` for tests).
  const deadlineSec = approval.deadline ? Number(approval.deadline) : 0;
  const nowSec = Math.floor(Date.now() / 1000);
  const expired = deadlineSec > 0 && deadlineSec < nowSec;

  const symbol = meta?.symbol ?? '…';
  const decimals = meta?.decimals ?? 18;
  const logoUrls = approval.tokenAddress
    ? tokenLogoURLsByAddress(chainId, approval.tokenAddress)
    : undefined;

  // Plain-language one-liner — what this approval actually lets the spender do.
  const spenderName = clearSign?.contractName ?? knownContract(approval.spender)?.name ?? shortAddr(approval.spender);
  const summary = approval.isReducing
    ? t('componentsUi.signing.summaryRevoke', { spender: spenderName, token: symbol })
    : isNft && approval.isUnbounded
      ? t('componentsUi.signing.summaryApproveNft', { operator: spenderName })
      : approval.isUnbounded
        ? t('componentsUi.signing.summaryApproveUnlimited', { spender: spenderName, token: symbol })
        : t('componentsUi.signing.summaryApprove', { spender: spenderName, amount: `${formatRawTokenAmount(approval.amountRaw ?? 0n, decimals)} ${symbol}` });
  // Neutral by default; only an unbounded grant warms the sentence to red.
  const summaryTone = approval.isUnbounded && !approval.isReducing ? 'danger' : 'neutral';

  return (
    <View>
      {/* Only an UNBOUNDED grant owns the screen with a big verb; a bounded approve
          is routine, so it cedes the headline to the amount + summary (like the mock's
          '最多 500 USDC' hero) with just a quiet kicker. */}
      <IntentHeader intent={verb} color={verbColor} variant={approval.isUnbounded && !approval.isReducing ? 'hero' : 'eyebrow'} />

      <SummaryLine
        text={summary}
        tone={summaryTone}
        emphasize={[spenderName, `${formatRawTokenAmount(approval.amountRaw ?? 0n, decimals)} ${symbol}`, symbol]}
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
        const increment = choice?.type === 'amount' ? choice.amountRaw : (approval.amountRaw ?? 0n);
        const dec = meta?.decimals ?? 18;
        const sym = meta?.symbol ?? '';
        return (
          <View style={styles.allowanceTotalRow}>
            <Text style={styles.allowanceTotalLabel}>{t('componentsUi.signingApprove.resultingTotal')}</Text>
            {currentAllowance !== null ? (
              <Text style={styles.allowanceTotalValue}>
                {`${formatRawTokenAmount(currentAllowance, dec)} + ${formatRawTokenAmount(increment, dec)} = ${formatRawTokenAmount(currentAllowance + increment, dec)} ${sym}`}
              </Text>
            ) : (
              <Text style={styles.allowanceTotalUnknown}>
                {t('componentsUi.signingApprove.resultingTotalUnknown', { amount: `${formatRawTokenAmount(increment, dec)} ${sym}` })}
              </Text>
            )}
          </View>
        );
      })()}

      <ContractBar
        label={isNft ? t('componentsUi.signingApprove.operatorLabel') : t('componentsUi.signingApprove.spenderLabel')}
        // Name the spender/operator from the same known-contract table the permit
        // view uses, so the Universal Router isn't a raw 0x here but named there (F11).
        name={clearSign?.contractName ?? knownContract(approval.spender)?.name}
        address={approval.spender}
        verified={false}
        identity="contract"
      />

      {approval.tokenAddress && (
        <ContractBar
          label={isNft ? t('componentsUi.signingApprove.collectionLabel') : t('componentsUi.signingApprove.tokenLabel')}
          name={clearSign?.contractName ?? (meta?.verified ? meta.symbol : undefined)}
          address={approval.tokenAddress}
          verified={clearSign?.verified ?? false}
          identity="asset"
        />
      )}

      {expired && (
        <WarningBanner severity="caution" text={t('componentsUi.signingApprove.expired')} />
      )}
    </View>
  );
}
