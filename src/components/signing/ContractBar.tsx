/**
 * Contract / recipient bar — the "to whom / what" row of a signing surface,
 * with identity resolution, recipient-risk signals, copy + explorer actions.
 */
import React, { useCallback, useEffect, useState } from 'react';
import { View, Text, Pressable } from 'react-native';
import { useTranslation } from 'react-i18next';
import * as Clipboard from 'expo-clipboard';
import { shortAddr, isAddress } from '@/models/types';
import { explorerBaseURL } from '@/models/network';
import { openBrowser } from '@/services/platform';
import { resolveRecipientIdentity, type RecipientIdentity } from '@/services/recipient-identity';
import { resolveRecipientRisk, type RecipientRisk } from '@/services/recipient-risk';
import { color } from '@/constants/theme';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { RecipientTrust } from '@/components/contacts/RecipientTrust';
import { Copy, Check, ShieldCheck, ShieldAlert, ExternalLink } from 'lucide-react-native';
import { styles, riskColors, SigningChainContext } from './signing-core';

export function ContractBar({ label, name, address, verified, warning, riskCheck }: {
  label: string;
  name?: string;
  address?: string;
  verified: boolean;
  warning?: boolean;
  /** Resolve recipient-risk signals (first-interaction + contract/EOA). */
  riskCheck?: boolean;
}) {
  const { t } = useTranslation();
  const [copied, setCopied] = useState(false);
  // Resolve an on-chain name (ENS / Basename / SPACE ID) when the descriptor
  // didn't supply one — turns a raw hex address into a recognizable identity and
  // helps catch address-poisoning. Cached in the service; descriptor name wins.
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

  // Recipient-risk: "first time" (address-poisoning defense) + contract/EOA.
  const [risk, setRisk] = useState<RecipientRisk | null>(null);
  useEffect(() => {
    setRisk(null);
    if (!riskCheck || !isAddress(address)) return;
    let cancelled = false;
    resolveRecipientRisk(chainId, address).then((r) => { if (!cancelled) setRisk(r); }).catch(() => {});
    return () => { cancelled = true; };
  }, [riskCheck, address, chainId]);

  const explorerBase = explorerBaseURL(chainId);
  const isFullAddr = isAddress(address);
  const explorerUrl = explorerBase && isFullAddr ? `${explorerBase}/address/${address}` : undefined;

  const handleCopy = useCallback(async () => {
    if (!address) return;
    await Clipboard.setStringAsync(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [address]);

  // A first-ever interaction is the real address-poisoning signal; being a contract
  // only matters as a caveat ON a first-time send (a known/repeat recipient that's a
  // contract — e.g. a swap router — is expected, so don't badge it). This keeps the
  // row to at most ONE subtle risk note instead of two competing chips.
  const showFirstTime = !!risk?.firstInteraction;
  const showContractCaveat = risk?.isContract === true && showFirstTime;

  return (
    <View style={[styles.contractBar, warning && styles.contractBarWarning]}>
      {/* Identicon only for a wallet recipient (riskCheck rows) — a token has its
          own logo and a contract (spender/operator/collection) isn't a personal
          identity, so a nimiq identicon there is noise. */}
      {riskCheck && address ? <ContactAvatar name={shownName ?? ''} address={address} size={36} /> : null}
      <View style={styles.contractInfo}>
        <Text style={styles.contractLabel}>{label}</Text>
        <View style={styles.contractAddrRow}>
          {/* Verified descriptor name keeps the trust-green; a resolved ENS / raw
              address stays neutral so color always means "verified". */}
          {shownName && (
            <Text style={[styles.contractName, !verified && styles.contractNameNeutral]} numberOfLines={1}>
              {shownName}
            </Text>
          )}
          {!name && ident && <Text style={styles.sourceTag}>{ident.source}</Text>}
          {address && (
            <Text style={styles.contractAddr}>{shortAddr(address)}</Text>
          )}
        </View>
        {/* One restrained safety line, not two chips — first-time is the poisoning
            signal, "· contract" a caveat only when it's also a first-time send. */}
        {showFirstTime && (
          <Text style={styles.riskNote}>
            {t('componentsUi.signing.firstTimeTag')}
            {showContractCaveat ? ` · ${t('componentsUi.signing.contractTag')}` : ''}
          </Text>
        )}
        <RecipientTrust address={address} compact />
      </View>
      {address && (
        <Pressable onPress={handleCopy} hitSlop={8} style={[styles.copyBtn, copied && styles.copyBtnDone]}>
          {copied
            ? <Check size={12} color={color.success.base} strokeWidth={2.5} />
            : <Copy size={12} color={color.fg.muted} strokeWidth={2} />
          }
        </Pressable>
      )}
      {/* Jump out to the block explorer to audit the contract / address. */}
      {explorerUrl && (
        <Pressable onPress={() => openBrowser(explorerUrl)} hitSlop={8} style={styles.copyBtn}>
          <ExternalLink size={12} color={color.fg.muted} strokeWidth={2} />
        </Pressable>
      )}
      {verified && (
        <View style={styles.verifiedBadge}>
          <ShieldCheck size={12} color={color.success.base} strokeWidth={2} />
        </View>
      )}
      {warning && (
        <ShieldAlert size={14} color={riskColors().danger} strokeWidth={2} />
      )}
    </View>
  );
}
