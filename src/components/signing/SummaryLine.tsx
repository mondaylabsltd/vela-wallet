/**
 * Plain-language summary — one human sentence that turns the structured hero into
 * something a first-time user reads naturally ("You're letting Uniswap spend up to
 * 500 USDC. Nothing leaves your wallet now."). Sits just under the hero, above the
 * counterparty row. The sentence is the novice's entry point; the zones below are
 * the detail.
 */
import React, { useEffect, useMemo, useState } from 'react';
import { Text } from 'react-native';
import { isAddress, shortAddr } from '@/models/types';
import { resolveRecipientIdentity } from '@/services/recipient-identity';
import { useWallet } from '@/models/wallet-state';
import { getSavedContact, contactDisplayName } from '@/services/contacts';
import { styles } from './signing-core';

export function SummaryLine({ text, tone = 'neutral', emphasize }: {
  text?: string;
  /** 'caution'/'danger' warm the sentence slightly so a risky action's summary
   *  doesn't read as calmly as a benign one. */
  tone?: 'neutral' | 'caution' | 'danger';
  /** Substrings to render in semibold (amount, counterparty). Matched verbatim in
   *  the already-localized sentence, so it's language-agnostic — no <Trans> markup. */
  emphasize?: (string | undefined)[];
}) {
  if (!text) return null;
  const toned = [
    styles.summaryLine,
    tone === 'danger' && styles.summaryDanger,
    tone === 'caution' && styles.summaryCaution,
  ];
  const subs = (emphasize ?? []).filter((s): s is string => !!s && s.length > 0);
  if (!subs.length) return <Text style={toned}>{text}</Text>;
  return <Text style={toned}>{splitEmphasis(text, subs).map((p, i) =>
    p.bold ? <Text key={i} style={styles.summaryBold}>{p.t}</Text> : p.t,
  )}</Text>;
}

/** Split a sentence into bold/plain runs by matching the emphasize substrings. */
function splitEmphasis(text: string, subs: string[]): { t: string; bold: boolean }[] {
  const escaped = subs.map((s) => s.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
  const re = new RegExp(`(${escaped.join('|')})`, 'g');
  return text.split(re).filter((s) => s !== '').map((s) => ({ t: s, bold: subs.includes(s) }));
}

/**
 * Best human label for a counterparty, for use inside a summary sentence:
 * descriptor name → saved contact → your own account → ENS/Basename → short
 * address. Resolves async (cached); shows the own-account name or short address
 * until a better name arrives so the sentence is never hex-only for long. Returns
 * undefined only when there's no address at all. (Same priority as
 * useAddressIdentity — a send to "Parallel Two" must read as the name, not a 0x.)
 */
export function useResolvedName(address?: string, descriptorName?: string): string | undefined {
  const { state } = useWallet();
  // Own account is a sync, high-trust label — seed it before any round-trip.
  const ownName = useMemo(() => {
    if (descriptorName || !isAddress(address)) return undefined;
    return state.accounts.find((a) => a.address.toLowerCase() === address!.toLowerCase())?.name;
  }, [address, descriptorName, state.accounts]);

  const [name, setName] = useState<string | undefined>(descriptorName ?? ownName);
  useEffect(() => {
    if (descriptorName) { setName(descriptorName); return; }
    if (!isAddress(address)) { setName(undefined); return; }
    setName(ownName ?? shortAddr(address));
    let cancelled = false;
    (async () => {
      // A saved contact is your own label → wins over ENS.
      try {
        const c = await getSavedContact(address!);
        const cn = c ? contactDisplayName(c) : '';
        if (cn) { if (!cancelled) setName(cn); return; }
      } catch { /* fall through */ }
      if (ownName) return; // keep your account name over a reverse-ENS lookup
      try {
        const r = await resolveRecipientIdentity(address!);
        if (!cancelled && r?.name) setName(r.name);
      } catch { /* keep short address */ }
    })();
    return () => { cancelled = true; };
  }, [address, descriptorName, ownName]);
  return name;
}
