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
import { type ClearSignResult } from '@/services/clear-signing';
import { tokenLogoURLsByAddress } from '@/models/types';
import { knownContract } from '@/services/local-descriptors';
import { knownTokenSymbol } from '@/services/tokens';
import { TokenLogo } from '@/components/TokenLogo';
import { AlertTriangle } from 'lucide-react-native';
import { styles, riskColors, SigningChainContext } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { ContractBar } from '../ContractBar';
import { WarningBanner } from '../WarningBanner';

export function PermitSignView({ approval, meta, clearSign }: {
  approval: DetectedApproval;
  meta: { symbol: string; decimals: number; verified: boolean } | null;
  clearSign: ClearSignResult | null;
}) {
  const { t } = useTranslation();
  const chainId = React.useContext(SigningChainContext);

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
        : `${formatRawTokenAmount(approval.amountRaw ?? 0n, decimals)} ${symbol}`;

  return (
    <View>
      <IntentHeader intent={verb} color={verbColor} variant={approval.isReducing ? 'eyebrow' : 'hero'} />

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
      />
      {approval.tokenAddress && (
        <ContractBar
          label={t('componentsUi.signingApprove.tokenLabel')}
          name={(meta?.verified ? meta.symbol : undefined) ?? knownTokenSymbol(approval.tokenAddress) ?? knownContract(approval.tokenAddress)?.name}
          address={approval.tokenAddress}
          verified={clearSign?.verified ?? false}
        />
      )}

      {expired && <WarningBanner severity="caution" text={t('componentsUi.signingApprove.expired')} />}

      {/* A bounded permit shows a scaled amount from meta.decimals — if that wasn't
          verified on-chain, flag it (its on-chain-approve sibling already does). */}
      {!approval.isBooleanGrant && !approval.isUnbounded && !meta?.verified && (
        <WarningBanner severity="caution" text={t('componentsUi.signingApprove.decimalsUnverified')} />
      )}

      {dangerous ? (
        <>
          <WarningBanner severity="danger" text={t('componentsUi.signing.unlimitedWarning')} />
          <Text style={styles.permitHint}>
            {t('componentsUi.signingApprove.permitCantCap', {
              defaultValue: "A permit is a signature — its amount can't be capped here. To limit spending, use an on-chain Approve instead.",
            })}
          </Text>
        </>
      ) : !approval.isReducing ? (
        <Text style={styles.permitHint}>
          {t('componentsUi.signingApprove.permitNote', {
            defaultValue: "You're signing a spending permit — the dApp can move up to this amount on your behalf.",
          })}
        </Text>
      ) : null}
    </View>
  );
}
