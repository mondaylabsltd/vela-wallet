/**
 * Clear Sign View — the descriptor-found, human-readable signing surface.
 */
import React from 'react';
import { View } from 'react-native';
import { useTranslation } from 'react-i18next';
import { type ClearSignResult } from '@/services/clear-signing';
import { styles, intentColor, localizeIntent } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { TokenCard, FlowArrow } from '../TokenCard';
import { ContractBar } from '../ContractBar';
import { WarningBanner, GenericFieldRow } from '../WarningBanner';
import { SummaryLine, useResolvedName } from '../SummaryLine';

export function ClearSignView({ cs, simConfident, walletAddress }: {
  cs: ClearSignResult;
  /** The tx was simulated and is not expected to revert — a best-effort (4byte)
   *  decode then reads as a calm "here's the gist" note instead of a "carefully
   *  check every detail" nag, because the preview below proves the real effect. */
  simConfident?: boolean;
  /** The signing wallet — lets us call out a swap whose output goes somewhere OTHER
   *  than your own address (a real "where did my tokens go" risk). */
  walletAddress?: string;
}) {
  const { t } = useTranslation();
  const rc = intentColor(cs.risk);

  // Separate fields by role
  const sendAmounts = cs.fields.filter(f => f.role === 'send-amount');
  const receiveAmounts = cs.fields.filter(f => f.role === 'receive-amount');
  const recipients = cs.fields.filter(f => f.role === 'recipient');
  const spenders = cs.fields.filter(f => f.role === 'spender');
  // Detail-flagged generics (best-effort raw params) live under the Advanced
  // panel, not the headline body — only body-level generics render here.
  const generic = cs.fields.filter(f => f.role === 'generic' && !f.detail);

  // Determine if this is a swap-like layout (send → receive)
  const isSwapLayout = sendAmounts.length > 0 && receiveAmounts.length > 0;
  const hasRecipient = recipients.length > 0 || spenders.length > 0;

  // Sending a token TO its own contract address burns it irreversibly — a classic
  // costly fat-finger. Flag when a recipient equals the contract being called.
  const sendingToTokenContract =
    !!cs.contractAddress &&
    recipients.some(f => f.address && f.address === cs.contractAddress);

  // Unified 5-zone order: Zone 1 (intent + ONE hero) → Zone 2 (counterparty) →
  // Zone 3 (all warnings, danger→caution) → detail. Warnings never sit above the
  // hero; instead the hero itself carries risk weight so an alert below the fold is
  // never the sole signal (safety invariants A3/A5): a descriptor-unlimited grant
  // tints the amount danger, and an unverified/best-effort/partial decode without a
  // confident sim tints it caution.
  const heroDanger = cs.fields.some(f => f.warning);
  const heroCaution = (cs.bestEffort || cs.partial || cs.fields.some(f => f.unverified)) && !simConfident;
  const sendVariant: 'send' | 'caution' | 'danger' =
    heroDanger ? 'danger' : (heroCaution || cs.risk === 'caution') ? 'caution' : 'send';

  // Plain-language one-liner under the hero — the novice's first read. Names the
  // recipient (descriptor name → ENS → short address, resolved async & cached).
  const recipient = recipients[0];
  const toName = useResolvedName(recipient?.address, recipient?.address ? undefined : recipient?.value);
  // A swap normally lands in your own wallet; if the output recipient is someone
  // ELSE, say so — that's the security-relevant fact ("where did my tokens go").
  const swapToOther = isSwapLayout && recipient?.address && !!toName
    && (!walletAddress || recipient.address.toLowerCase() !== walletAddress.toLowerCase());
  const summary = isSwapLayout
    ? (swapToOther
        ? t('componentsUi.signing.summarySwapTo', { pay: sendAmounts[0]?.value, receive: receiveAmounts[0]?.value, to: toName })
        : t('componentsUi.signing.summarySwap', { pay: sendAmounts[0]?.value, receive: receiveAmounts[0]?.value }))
    : (sendAmounts.length > 0 && recipients.length > 0 && toName)
      ? t('componentsUi.signing.summarySend', { amount: sendAmounts[0].value, to: toName })
      : undefined;
  // Restraint: the summary stays neutral ink; only genuine danger warms it. The hero
  // and Zone-3 warnings carry the risk color, so the sentence doesn't double up.
  const summaryTone = sendVariant === 'danger' ? 'danger' : 'neutral';

  // When there's an asset amount, IT is the hero — the verb is a mere eyebrow (even
  // when caution/danger: the amount card carries the risk colour). Only an amount-less
  // action keeps a big verb.
  const hasAmount = sendAmounts.length > 0 || receiveAmounts.length > 0;

  return (
    <View>
      {/* ZONE 1 — the action + the ONE hero. Benign → eyebrow; risk → big hero. */}
      <IntentHeader
        intent={localizeIntent(cs.intent)}
        color={rc}
        variant={hasAmount || cs.risk === 'normal' ? 'eyebrow' : 'hero'}
      />
      {isSwapLayout ? (
        <>
          {sendAmounts.map((f, i) => (
            <TokenCard key={`s${i}`} field={f} variant="send" hero heroLabel />
          ))}
          <FlowArrow />
          {receiveAmounts.map((f, i) => (
            <TokenCard key={`r${i}`} field={f} variant="receive" hero heroLabel />
          ))}
          <SummaryLine text={summary} tone={summaryTone} emphasize={[sendAmounts[0]?.value, receiveAmounts[0]?.value, swapToOther ? toName : undefined]} />
        </>
      ) : sendAmounts.length > 0 ? (
        <>
          {sendAmounts.map((f, i) => (
            <TokenCard key={`s${i}`} field={f} variant={sendVariant} hideSign hero />
          ))}
          <SummaryLine text={summary} tone={summaryTone} emphasize={[sendAmounts[0]?.value, toName]} />
        </>
      ) : receiveAmounts.length > 0 ? (
        // Pure inflow (a vault withdraw / redeem) — the arriving amount IS the hero.
        // Without this it fell through to nothing (F7 made it a receive-amount).
        <>
          {receiveAmounts.map((f, i) => (
            // Always the receive direction (green "+"), even when unverified — it's an
            // INFLOW; the "couldn't verify" caveat is the warning below, not a "−".
            <TokenCard key={`r${i}`} field={f} variant="receive" hero />
          ))}
          <SummaryLine
            text={t('componentsUi.signing.summaryReceive', { amount: receiveAmounts[0].value, defaultValue: "You'll receive {{amount}}." })}
            tone={summaryTone}
            emphasize={[receiveAmounts[0]?.value]}
          />
        </>
      ) : null}

      {/* ZONE 2 — to whom / what: spender, recipient, or the interacting contract. */}
      {spenders.map((f, i) => (
        <ContractBar
          key={`sp${i}`}
          label={t('componentsUi.signing.spenderLabel')}
          name={f.value}
          address={cs.contractAddress}
          verified={cs.verified}
          identity="contract"
        />
      ))}
      {recipients.map((f, i) => {
        // A swap that lands back in your OWN wallet needs no recipient row at all.
        const isSelf = !!f.address && !!walletAddress && f.address.toLowerCase() === walletAddress.toLowerCase();
        if (isSwapLayout && isSelf) return null;
        return (
          <ContractBar
            key={`re${i}`}
            label={t('componentsUi.signing.recipientLabel')}
            name={f.address ? undefined : f.value}
            address={f.address}
            verified={false}
            identity="auto"
            // The name is already in the summary → collapse to one quiet line.
            compact={!!summary}
            // Sending a token to its own contract burns it — turn the recipient row
            // itself red so Zone 2 contradicts the benign read, not just a Zone-3 banner.
            warning={sendingToTokenContract && !!f.address && f.address === cs.contractAddress}
          />
        );
      })}
      {!hasRecipient && cs.contractAddress && (
        <ContractBar
          label={t('componentsUi.signing.interactingLabel')}
          name={cs.contractName ? `${cs.contractName}${cs.owner ? ` · ${cs.owner}` : ''}` : undefined}
          address={cs.contractAddress}
          verified={cs.verified}
          identity="contract"
        />
      )}

      {/* ZONE 3 — every active warning, danger first then caution. Stacked, never
          "most-severe only": a permit can be expired AND unverified at once. */}
      {sendingToTokenContract && (
        <WarningBanner severity="danger" text={t('componentsUi.signing.tokenToContractWarning')} />
      )}
      {heroDanger && (
        <WarningBanner severity="danger" text={t('componentsUi.signing.unlimitedWarning')} />
      )}
      {cs.fields.some(f => f.expired) && (
        <WarningBanner severity="caution" text={t('componentsUi.signing.expiredWarning')} />
      )}
      {cs.bestEffort && (
        <WarningBanner
          severity="caution"
          text={simConfident
            ? t('componentsUi.signing.bestEffortSimulated', { defaultValue: 'Decoded from the function signature (not a verified descriptor). The preview below shows the actual effect.' })
            : t('componentsUi.signing.bestEffortWarning')}
        />
      )}
      {cs.partial && (
        <WarningBanner severity="caution" text={t('componentsUi.signing.partialWarning')} />
      )}
      {!cs.partial && cs.fields.some(f => f.unverified) && (
        <WarningBanner severity="caution" text={t('componentsUi.signing.unverifiedWarning')} />
      )}

      {/* Detail — raw decoded params, last so they never split the zones above. */}
      {generic.length > 0 && (
        <View style={styles.genericFields}>
          {generic.map((f, i) => (
            <GenericFieldRow key={i} field={f} />
          ))}
        </View>
      )}
    </View>
  );
}
