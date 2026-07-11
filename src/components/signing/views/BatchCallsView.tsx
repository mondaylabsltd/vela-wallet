/**
 * Batch View (EIP-5792 wallet_sendCalls) — per-call breakdown with an editable
 * spending cap on every approval leg.
 */
import React from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { type DetectedApproval, type ApprovalChoice } from '@/services/approval-guard';
import { type ClearSignResult } from '@/services/clear-signing';
import { shortAddr, tokenLogoURLsByAddress } from '@/models/types';
import { ShieldAlert } from 'lucide-react-native';
import { styles, riskColors, SigningChainContext } from '../signing-core';
import { EditableApproveCard } from '../EditableApproveCard';
import { IntentHeader } from '../IntentHeader';
import { WarningBanner } from '../WarningBanner';

/** One resolved leg of an EIP-5792 batch (wallet_sendCalls). */
export interface BatchItem {
  to: string;
  clearSign: ClearSignResult | null;
  approval: DetectedApproval | null;
}

/** First meaningful amount/recipient line for a batch leg. */
function batchSummary(it: BatchItem): string | undefined {
  const f = it.clearSign?.fields.find(
    (x) => x.role === 'send-amount' || x.role === 'receive-amount' || x.format === 'tokenAmount' || x.format === 'amount',
  );
  return f?.value;
}

/**
 * Does this batch approval leg still need a deliberate decision before the bundle
 * can be confirmed? (Unbounded amount not yet capped/revoked, or a grant-all with
 * no choice.) Finite amounts are pre-accepted — editing them is optional.
 */
export function legNeedsChoice(ap: DetectedApproval | null, choice: ApprovalChoice | null | undefined): boolean {
  if (!ap || !ap.editable || ap.isReducing) return false;
  if (ap.isBooleanGrant) return !choice;
  if (ap.isUnbounded) return !(choice && (choice.type === 'amount' || choice.type === 'revoke'));
  return false;
}

/** After the user's choice, does this leg still grant broad/unbounded access? */
function legGrantsBroad(ap: DetectedApproval | null, choice: ApprovalChoice | null | undefined): boolean {
  if (!ap || ap.isReducing) return false;
  if (ap.isBooleanGrant) return choice?.type === 'grant' || !choice;
  if (ap.isUnbounded) return !(choice && (choice.type === 'amount' || choice.type === 'revoke'));
  return false;
}

/** Is this leg an editable, amount/grant-bearing approval that gets the inline cap editor? */
function legIsEditableApproval(ap: DetectedApproval | null): boolean {
  return !!ap && ap.editable && !ap.isReducing;
}

export function BatchCallsView({ items, choices, onChoiceChange, metaByToken, editable, requestId }: {
  items: BatchItem[];
  choices: Record<number, ApprovalChoice | null>;
  onChoiceChange: (index: number, choice: ApprovalChoice | null) => void;
  metaByToken: Map<string, { symbol: string; decimals: number; verified: boolean }>;
  editable: boolean;
  /** Remounts each leg's editor when the request changes (no stale cap state). */
  requestId: string;
}) {
  const { t } = useTranslation();
  const chainId = React.useContext(SigningChainContext);
  // Banner reflects the EFFECTIVE state: only still-uncapped grants are flagged.
  const anyUncapped = editable
    ? items.some((it, i) => legGrantsBroad(it.approval, choices[i]))
    : items.some((it) => it.approval?.isUnbounded && !it.approval.isReducing && !it.approval.isBooleanGrant);
  // A leg that sends a token to its OWN contract burns it — the same fat-finger the
  // single-send path flags, easy to miss buried in a batch (F13).
  const anyToOwnToken = items.some((it) => {
    const to = it.to?.toLowerCase();
    return !!to && (it.clearSign?.fields.some((f) => f.role === 'recipient' && f.address?.toLowerCase() === to) ?? false);
  });

  return (
    <View>
      <IntentHeader intent={t('componentsUi.signing.batchIntent')} color={color.fg.base} variant="eyebrow" />
      <Text style={styles.batchSub}>{t('componentsUi.signing.batchSubtitle', { count: items.length })}</Text>

      {items.map((it, i) => {
        const ap = it.approval;
        const title = it.clearSign?.intent
          ?? (ap ? t('componentsUi.signingApprove.verbApprove') : t('componentsUi.signing.batchCall'));

        // Editable approval leg → inline spending-cap editor (same control single
        // approvals use), so an unlimited approve can be capped here, not only rejected.
        if (editable && legIsEditableApproval(ap) && ap) {
          const meta = ap.tokenAddress ? metaByToken.get(ap.tokenAddress.toLowerCase()) : undefined;
          const logoUrls = ap.tokenAddress ? tokenLogoURLsByAddress(chainId, ap.tokenAddress) : undefined;
          return (
            <View key={`${requestId}-${i}`} style={styles.batchEditLeg}>
              <View style={styles.batchEditHead}>
                <View style={styles.batchNum}>
                  <Text style={styles.batchNumText}>{i + 1}</Text>
                </View>
                <Text style={styles.batchEditTitle} numberOfLines={1}>{title}</Text>
              </View>
              <EditableApproveCard
                approval={ap}
                symbol={meta?.symbol ?? '…'}
                decimals={meta?.decimals ?? 18}
                decimalsVerified={meta?.verified ?? false}
                logoUrls={logoUrls}
                spenderLabel={it.clearSign?.contractName ?? shortAddr(ap.spender)}
                choice={choices[i] ?? null}
                onChange={(c) => onChoiceChange(i, c)}
              />
            </View>
          );
        }

        // Non-approval / reducing / read-only leg → compact summary row.
        const danger = legGrantsBroad(ap, choices[i]);
        const summary = batchSummary(it);
        return (
          <View key={i} style={[styles.batchRow, danger && styles.batchRowDanger]}>
            <View style={styles.batchNum}>
              <Text style={styles.batchNumText}>{i + 1}</Text>
            </View>
            <View style={styles.batchInfo}>
              <Text style={styles.batchTitle} numberOfLines={1}>{title}</Text>
              {!!summary && <Text style={styles.batchDetail} numberOfLines={1}>{summary}</Text>}
              <Text style={styles.batchAddr} numberOfLines={1}>{it.to ? shortAddr(it.to) : '—'}</Text>
            </View>
            {danger && <ShieldAlert size={14} color={riskColors().danger} strokeWidth={2} />}
          </View>
        );
      })}

      {anyToOwnToken && (
        <WarningBanner severity="danger" text={t('componentsUi.signing.tokenToContractWarning')} />
      )}
      {anyUncapped && (
        <WarningBanner severity="danger" text={t('componentsUi.signing.unlimitedWarning')} />
      )}
    </View>
  );
}
