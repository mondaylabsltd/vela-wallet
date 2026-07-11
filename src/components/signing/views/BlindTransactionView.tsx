/**
 * Blind Transaction View (no descriptor) — a decoded-less eth_sendTransaction.
 */
import React from 'react';
import { View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { nativeSymbol } from '@/models/network';
import { IntentHeader } from '../IntentHeader';
import { TokenCard, FlowArrow } from '../TokenCard';
import { ContractBar } from '../ContractBar';
import { WarningBanner } from '../WarningBanner';

export function BlindTransactionView({ tx, chainId, simConfident }: {
  tx: any;
  chainId: number;
  /** The tx was simulated and is not expected to revert — the balance-change preview
   *  below shows what actually happens, so the descriptor-absence is a calm note, not
   *  a red alarm. */
  simConfident?: boolean;
}) {
  const { t } = useTranslation();
  const sym = nativeSymbol(chainId);
  const value = formatTxValue(tx.value, chainId);
  const hasData = tx.data && tx.data !== '0x';
  const dataSize = hasData ? Math.floor((tx.data.length - 2) / 2) : 0;
  // A simulated, non-reverting contract call reads as a neutral "contract
  // interaction", not a red "Unknown" — the preview below carries the real meaning.
  const calm = hasData && !!simConfident;

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

      {/* Value card */}
      {value !== `0 ${sym}` && (
        <TokenCard
          field={{ label: t('componentsUi.signing.valueLabel'), value, format: 'amount', role: 'send-amount' }}
          variant={hasData && !calm ? 'danger' : 'send'}
        />
      )}

      {(hasData || value !== `0 ${sym}`) && <FlowArrow danger={hasData && !calm} />}

      {/* Contract / recipient. A plain send goes to a wallet, so run the recipient
          risk check (identicon + first-time / contract note), same as a decoded send. */}
      <ContractBar
        label={hasData ? t('componentsUi.signing.unverifiedLabel') : t('componentsUi.signing.recipientLabel')}
        address={tx.to}
        verified={false}
        warning={hasData && !calm}
        riskCheck={!hasData}
      />

      {/* Descriptor-absence notice. With a confident simulation it's a calm caption
          that points at the preview below; without one it stays a hard blind-sign
          warning (genuinely opaque — no descriptor AND no simulated outcome). */}
      {/* Zone 3 — the descriptor-absence caution. The raw calldata lives in the
          shared Advanced panel below (no duplicate toggle here, which also kept the
          sim ✓ from rendering after it). */}
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
    if (eth < 0.0001) return `< 0.0001 ${sym}`;
    return eth.toFixed(4).replace(/\.?0+$/, '') + ' ' + sym;
  } catch {
    return value ?? '0';
  }
}
