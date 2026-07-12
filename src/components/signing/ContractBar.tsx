/**
 * Counterparty row — the "who / what" of a signing surface. Answers, at a glance,
 * whether you're interacting with a PERSON'S WALLET or a CONTRACT, and whether that
 * contract is verified. Identity drives the visuals:
 *   - wallet (EOA, incl. EIP-7702 delegated) → nimiq identicon + 「钱包」 chip
 *   - contract (spender / router / operator) → neutral glyph + 「合约」 chip, never
 *     an identicon (which reads as a personal identity)
 *   - asset (token / collection)             → no identity chip; it's a thing, not a who
 * The raw 0x address and explorer/copy actions live in the Advanced 技术细节 drawer,
 * not here — the default view stays calm and readable for a first-time user.
 */
import React, { useEffect, useState } from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { shortAddr, isAddress } from '@/models/types';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';
import { resolveRecipientRisk, type RecipientRisk } from '@/services/recipient-risk';
import { color } from '@/constants/theme';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { RecipientTrust } from '@/components/contacts/RecipientTrust';
import { ShieldCheck, ShieldAlert, FileText } from 'lucide-react-native';
import { styles, riskColors, SigningChainContext } from './signing-core';

export function ContractBar({ label, name, address, verified, warning, identity = 'contract', compact, inflow }: {
  label: string;
  name?: string;
  address?: string;
  verified: boolean;
  warning?: boolean;
  /** How to read this counterparty:
   *  'auto'     — a recipient; probe on-chain whether it's a wallet or a contract.
   *  'contract' — a known contract counterparty (spender / operator / router).
   *  'asset'    — a token / collection; no wallet-vs-contract identity chip. */
  identity?: 'auto' | 'contract' | 'asset';
  /** The name is already stated in the summary above — collapse to a single quiet
   *  line (small identicon + "Wallet · first time"), no name repeat, no big block. */
  compact?: boolean;
  /** This counterparty RECEIVES assets (a withdraw/redeem receiver), so the
   *  first-time note reads "first time using this address", not "…sending here". */
  inflow?: boolean;
}) {
  const { t } = useTranslation();
  const isRecipient = identity === 'auto';

  // Resolve an on-chain name (ENS / Basename / SPACE ID) when the descriptor didn't
  // supply one — turns raw hex into a recognizable identity and helps catch address
  // poisoning. Cached in the service; a descriptor-supplied name always wins.
  const [ident, setIdent] = useState<RecipientIdentity | null>(null);
  useEffect(() => {
    setIdent(null);
    if (name || !isAddress(address)) return;
    let cancelled = false;
    resolveRecipientIdentity(address).then((r) => { if (!cancelled) setIdent(r); }).catch(() => {});
    return () => { cancelled = true; };
  }, [address, name]);
  const shownName = name ?? ident?.name;

  const chainId = React.useContext(SigningChainContext);

  // Recipient-risk: "first time" (poisoning defense) + wallet/contract (7702-aware).
  const [risk, setRisk] = useState<RecipientRisk | null>(null);
  useEffect(() => {
    setRisk(null);
    if (!isRecipient || !isAddress(address)) return;
    let cancelled = false;
    resolveRecipientRisk(chainId, address).then((r) => { if (!cancelled) setRisk(r); }).catch(() => {});
    return () => { cancelled = true; };
  }, [isRecipient, address, chainId]);

  // Contract-ness: 'contract' rows are contracts by definition; 'asset' rows aren't
  // a who; 'auto' rows come from the probe (null = still resolving / RPC unreachable).
  const isContract: boolean | null =
    identity === 'contract' ? true
    : identity === 'asset' ? null
    : risk?.isContract ?? null;

  const showFirstTime = !!risk?.firstInteraction;

  // Avatar: a real contract gets a neutral glyph; a wallet (or a still-unknown
  // recipient — the identicon is a useful fingerprint regardless) gets its identicon;
  // an asset gets nothing (its symbol/logo lives in the amount hero).
  const avatar =
    identity === 'asset' ? null
    : isContract === true
      ? (
        <View style={styles.contractGlyph}>
          <FileText size={17} color={color.fg.subtle} strokeWidth={2} />
        </View>
      )
      : isRecipient && address
        ? <ContactAvatar name={shownName ?? ''} address={address} size={36} />
        : null;

  // Identity chips answer "who is this". Wallet / Contract, plus Verified when the
  // descriptor vouches for the contract.
  const idChip =
    identity === 'asset' ? null
    : isContract === true
      ? { box: styles.idChipContract, txt: styles.idChipContractText, label: t('componentsUi.signing.contractTag') }
      : isContract === false
        ? { box: styles.idChipWallet, txt: styles.idChipWalletText, label: t('componentsUi.signing.walletTag', { defaultValue: 'Wallet' }) }
        : null;

  // Compact: the summary already names the recipient and the sim confirms "nothing
  // else leaves", so the whole recipient row is redundant — drop it. The genuinely
  // dangerous case (sending a token to its own contract) still shows its red warning
  // row below (compact only applies when there's no warning).
  if (compact && !warning) return null;

  return (
    <View style={[styles.contractBar, warning && styles.contractBarWarning]}>
      {avatar}
      <View style={styles.contractInfo}>
        {/* A recipient needs no 'RECIPIENT' kicker — the summary above already says
            "sending to vitalik.eth", and the identicon + name carry it. A spender /
            token row keeps its label (which of several counterparties it is). */}
        {identity !== 'auto' && <Text style={styles.contractLabel}>{label}</Text>}
        <View style={styles.contractAddrRow}>
          {shownName ? (
            // Verified descriptor name keeps trust-green; a resolved ENS / plain name
            // stays neutral so color always means "verified". (The name's source lives
            // in the Advanced drawer — a clean row, not an "ENS" tag competing here.)
            <Text style={[styles.contractName, !verified && styles.contractNameNeutral]} numberOfLines={1}>
              {shownName}
            </Text>
          ) : (
            // No name to show — the short address is the primary identity (full 0x
            // lives in the Advanced drawer).
            address ? <Text style={[styles.contractAddr, styles.contractNameNeutral]} numberOfLines={1}>{shortAddr(address)}</Text> : null
          )}
        </View>
        {/* Poisoning signal — a never-before-seen counterparty, in plain words.
            Reworded for an inflow (you're receiving, not sending). */}
        {showFirstTime && (
          <Text style={styles.riskNote}>
            {inflow
              ? t('componentsUi.signing.firstTimeTagNeutral', { defaultValue: 'First time using this address' })
              : t('componentsUi.signing.firstTimeTag')}
          </Text>
        )}
        <RecipientTrust address={address} compact />
      </View>

      {(idChip || verified) && (
        <View style={styles.idChips}>
          {idChip && (
            <View style={[styles.idChip, idChip.box]}>
              <Text style={[styles.idChipText, idChip.txt]}>{idChip.label}</Text>
            </View>
          )}
          {verified && (
            <View style={[styles.idChip, styles.idChipVerified]}>
              <ShieldCheck size={11} color={color.success.base} strokeWidth={2.5} />
              <Text style={[styles.idChipText, styles.idChipVerifiedText]}>{t('componentsUi.signing.verifiedTag', { defaultValue: 'Verified' })}</Text>
            </View>
          )}
        </View>
      )}
      {warning && (
        <ShieldAlert size={14} color={riskColors().danger} strokeWidth={2} />
      )}
    </View>
  );
}
