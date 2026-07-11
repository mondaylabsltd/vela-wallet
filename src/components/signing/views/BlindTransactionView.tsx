/**
 * Blind Transaction View (no descriptor) — a decoded-less eth_sendTransaction.
 */
import React from 'react';
import { View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { nativeSymbol } from '@/models/network';
import { useLocalePrefs, numberSeparators, formatNumber } from '@/services/locale-format';
import { IntentHeader } from '../IntentHeader';
import { TokenCard } from '../TokenCard';
import { ContractBar } from '../ContractBar';
import { WarningBanner } from '../WarningBanner';
import { SummaryLine, useResolvedName } from '../SummaryLine';

export function BlindTransactionView({ tx, chainId, simConfident, nativeUsdPrice }: {
  tx: any;
  chainId: number;
  /** The tx was simulated and is not expected to revert — the balance-change preview
   *  below shows what actually happens, so the descriptor-absence is a calm note, not
   *  a red alarm. */
  simConfident?: boolean;
  /** USD price of the native coin, for the ≈ $ under a plain-send hero (F3). */
  nativeUsdPrice?: number;
}) {
  const { t } = useTranslation();
  useLocalePrefs();
  const sym = nativeSymbol(chainId);
  const value = formatTxValue(tx.value, chainId);
  const hasData = tx.data && tx.data !== '0x';
  const dataSize = hasData ? Math.floor((tx.data.length - 2) / 2) : 0;
  // A simulated, non-reverting contract call reads as a neutral "contract
  // interaction", not a red "Unknown" — the preview below carries the real meaning.
  const calm = hasData && !!simConfident;
  const hasValue = value !== `0 ${sym}`;

  // Native USD magnitude for the hero (F3) — TokenCard formats it in the user's
  // display currency (€/¥/etc.), like every ERC-20.
  const usdValue = nativeUsd(tx.value, nativeUsdPrice);

  // A plain native send (no calldata) is a wallet-to-wallet transfer — resolve the
  // recipient and give it the same plain-language one-liner as the descriptor path.
  const toName = useResolvedName(hasData ? undefined : tx.to);
  const summary = !hasData && hasValue && toName
    ? t('componentsUi.signing.summarySend', { amount: value, to: toName })
    : undefined;

  return (
    <View>
      {/* context shown in dApp banner. A plain native send is a benign value
          transfer, so it cedes the headline to the amount (eyebrow) exactly like
          the descriptor path; a contract call / blind keeps the big hero. */}
      <IntentHeader
        intent={!hasData
          ? t('componentsUi.signing.intentSend')
          : calm
            ? t('componentsUi.signing.intentContractCall', { defaultValue: 'Contract interaction' })
            : t('componentsUi.signing.intentUnknown')}
        color={hasData && !calm ? color.error.base : color.fg.base}
        variant={hasData && !calm ? 'hero' : 'eyebrow'}
      />

      {/* Value card — a plain send drops the −/+ (the eyebrow + summary carry
          direction); a blind call with a value keeps it. */}
      {hasValue && (
        <TokenCard
          field={{ label: t('componentsUi.signing.valueLabel'), value, format: 'amount', role: 'send-amount', usdValue }}
          variant={hasData && !calm ? 'danger' : 'send'}
          hideSign={!hasData}
          hero={!hasData}
        />
      )}

      {!hasData && <SummaryLine text={summary} emphasize={[value, toName]} />}

      {/* Contract / recipient. A plain send goes to a wallet, so run the recipient
          risk check (identicon + first-time / contract note), same as a decoded send. */}
      <ContractBar
        label={hasData ? t('componentsUi.signing.unverifiedLabel') : t('componentsUi.signing.recipientLabel')}
        address={tx.to}
        verified={false}
        warning={hasData && !calm}
        // A plain send goes to a wallet-or-contract recipient (probe it); a call with
        // data is by definition a contract.
        identity={hasData ? 'contract' : 'auto'}
        // Plain send names the recipient in its summary → compact.
        compact={!hasData && !!summary}
      />

      {/* Descriptor-absence notice. With a confident simulation it's a calm caption
          that points at the preview below; without one it stays a hard blind-sign
          warning (genuinely opaque — no descriptor AND no simulated outcome). */}
      {hasData && (
        <WarningBanner
          severity={calm ? 'caution' : 'danger'}
          text={calm
            ? t('componentsUi.signing.blindButSimulated', { defaultValue: "Vela couldn't read this contract's details, but the preview below shows exactly what this transaction does." })
            : t('componentsUi.signing.blindDecodeWarning', { bytes: dataSize })}
        />
      )}
    </View>
  );
}

function formatTxValue(value: string | undefined, cid: number): string {
  const sym = nativeSymbol(cid);
  if (!value || value === '0x0' || value === '0x') return `0 ${sym}`;
  try {
    const clean = value.startsWith('0x') ? value.slice(2) : value;
    const wei = BigInt('0x' + clean);
    const eth = Number(wei) / 1e18;
    if (eth === 0) return `0 ${sym}`;
    if (eth < 0.0001) return `< 0${numberSeparators().decimal}0001 ${sym}`;
    return `${formatNumber(eth, { maximumFractionDigits: 4 })} ${sym}`;
  } catch {
    return value ?? '0';
  }
}

/** USD magnitude for a native value (TokenCard formats it into the display
 *  currency), or undefined when price/value is unavailable. */
function nativeUsd(value: string | undefined, price?: number): number | undefined {
  if (!price || price <= 0 || !value || value === '0x0' || value === '0x') return undefined;
  try {
    const wei = BigInt(value.startsWith('0x') ? value : `0x${value}`);
    const n = (Number(wei) / 1e18) * price;
    return n > 0 ? n : undefined;
  } catch {
    return undefined;
  }
}
