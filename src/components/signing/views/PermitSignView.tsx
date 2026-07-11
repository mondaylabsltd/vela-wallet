/**
 * Permit Sign View — off-chain spending permit (Permit2 / ERC-2612 / DAI).
 *
 * Off-chain permit signatures are redeemed by the dApp, which submits its OWN
 * permit struct on-chain — so the wallet can't cap the amount (rewriting it only
 * desyncs the signature and reverts the dApp's tx). We therefore show the real
 * risk and sign VERBATIM under a deliberate hold, rather than the cap editor.
 */
import React from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { type DetectedApproval, formatTokenAmount as formatRawTokenAmount } from '@/services/approval-guard';
import { useLocalePrefs, numberSeparators } from '@/services/locale-format';
import { type ClearSignResult } from '@/services/clear-signing';
import { shortAddr, tokenLogoURLsByAddress } from '@/models/types';
import { knownContract } from '@/services/local-descriptors';
import { TokenLogo } from '@/components/TokenLogo';
import { AlertTriangle } from 'lucide-react-native';
import { styles, riskColors, SigningChainContext } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { ContractBar } from '../ContractBar';
import { WarningBanner } from '../WarningBanner';
import { SummaryLine } from '../SummaryLine';

export function PermitSignView({ approval, meta, clearSign }: {
  approval: DetectedApproval;
  meta: { symbol: string; decimals: number; verified: boolean } | null;
  clearSign: ClearSignResult | null;
}) {
  const { t } = useTranslation();
  const chainId = React.useContext(SigningChainContext);
  useLocalePrefs();
  const sep = numberSeparators();

  const symbol = meta?.symbol ?? '…';
  const decimals = meta?.decimals ?? 18;
  const logoUrls = approval.tokenAddress ? tokenLogoURLsByAddress(chainId, approval.tokenAddress) : undefined;
  const dangerous = approval.isUnbounded && !approval.isReducing;

  const deadlineSec = approval.deadline ? Number(approval.deadline) : 0;
  const expired = deadlineSec > 0 && deadlineSec < Math.floor(Date.now() / 1000);

  const verb = approval.isReducing
    ? t('componentsUi.signingApprove.verbRevoke')
    : t('componentsUi.signingApprove.verbApprove');
  const verbColor = approval.isReducing
    ? color.success.base
    : approval.isUnbounded ? color.error.base : color.warning.base;

  // What the dApp's permit will be authorized to spend.
  const amountText = approval.isBooleanGrant
    ? (approval.isUnbounded
        ? t('componentsUi.signingApprove.fullBalance')
        : t('componentsUi.signingApprove.revokeValue'))
    : approval.kind === 'permit2-batch'
      ? t('componentsUi.signingApprove.multiplePermits', { defaultValue: 'Multiple tokens' })
      : approval.isUnbounded
        ? t('componentsUi.signingApprove.unlimitedValue', { defaultValue: 'Unlimited' })
        : `${formatRawTokenAmount(approval.amountRaw ?? 0n, decimals, 6, sep)} ${symbol}`;

  // Plain-language one-liner — a permit is a signature the dApp redeems later.
  const spenderName = knownContract(approval.spender)?.name ?? shortAddr(approval.spender);
  const summary = approval.isReducing
    ? t('componentsUi.signing.summaryRevoke', { spender: spenderName, token: symbol })
    : dangerous
      ? t('componentsUi.signing.summaryPermitUnlimited', { spender: spenderName, token: symbol })
      : t('componentsUi.signing.summaryPermit', { spender: spenderName, amount: amountText });
  const summaryTone = dangerous ? 'danger' : 'neutral';

  return (
    <View>
      <IntentHeader intent={verb} color={verbColor} variant={approval.isUnbounded && !approval.isReducing ? 'hero' : 'eyebrow'} />

      <SummaryLine text={summary} tone={summaryTone} emphasize={[spenderName, amountText, symbol]} />

      <View style={[styles.tokenCard, dangerous && { backgroundColor: color.error.soft }]}>
        <TokenLogo symbol={approval.tokenAddress ? symbol : '?'} logoUrls={logoUrls} size={40} />
        <View style={styles.tokenInfo}>
          <Text style={styles.tokenAmount} numberOfLines={1}>{amountText}</Text>
          <Text style={styles.tokenLabel}>
            {t('componentsUi.signingApprove.permitTag', { defaultValue: 'Spending permit (signature)' })}
          </Text>
        </View>
        {dangerous && <AlertTriangle size={14} color={riskColors().danger} strokeWidth={2} />}
      </View>

      {/* Resolve each row from its OWN address — never the EIP-712 domain name
          (clearSign.contractName), which for a Permit2 is "Permit2" and for an
          ERC-2612 permit is the token name, i.e. wrong on the spender row. */}
      <ContractBar
        label={t('componentsUi.signingApprove.spenderLabel')}
        name={knownContract(approval.spender)?.name}
        address={approval.spender}
        verified={false}
        identity="contract"
      />
      {/* Token row dropped — the symbol is already in the amount card + the summary. */}

      {expired && <WarningBanner severity="caution" text={t('componentsUi.signingApprove.expired')} />}

      {/* A bounded permit shows a scaled amount from meta.decimals — if that wasn't
          verified on-chain, flag it (its on-chain-approve sibling already does). */}
      {!approval.isBooleanGrant && !approval.isUnbounded && !meta?.verified && (
        <WarningBanner severity="caution" text={t('componentsUi.signingApprove.decimalsUnverified')} />
      )}

      {/* Only the actionable advice remains — the plain "it's a permit / spends up to
          X on your behalf" is already the summary line under the hero (no duplicate). */}
      {dangerous && (
        <>
          <WarningBanner severity="danger" text={t('componentsUi.signing.unlimitedWarning')} />
          <Text style={styles.permitHint}>
            {t('componentsUi.signingApprove.permitCantCap', {
              defaultValue: "A permit is a signature — its amount can't be capped here. To limit spending, use an on-chain Approve instead.",
            })}
          </Text>
        </>
      )}
    </View>
  );
}
