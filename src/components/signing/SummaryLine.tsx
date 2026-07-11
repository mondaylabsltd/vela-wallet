/**
 * Plain-language summary — one human sentence that turns the structured hero into
 * something a first-time user reads naturally ("You're letting Uniswap spend up to
 * 500 USDC. Nothing leaves your wallet now."). Sits just under the hero, above the
 * counterparty row. The sentence is the novice's entry point; the zones below are
 * the detail.
 */
import React, { useEffect, useState } from 'react';
import { Text } from 'react-native';
import { isAddress, shortAddr } from '@/models/types';
import { resolveRecipientIdentity } from '@/services/recipient-identity';
import { styles } from './signing-core';

export function SummaryLine({ text, tone = 'neutral' }: {
  text?: string;
  /** 'caution'/'danger' warm the sentence slightly so a risky action's summary
   *  doesn't read as calmly as a benign one. */
  tone?: 'neutral' | 'caution' | 'danger';
}) {
  if (!text) return null;
  return (
    <Text style={[
      styles.summaryLine,
      tone === 'danger' && styles.summaryDanger,
      tone === 'caution' && styles.summaryCaution,
    ]}>
      {text}
    </Text>
  );
}

/**
 * Best human label for a counterparty, for use inside a summary sentence:
 * descriptor name → resolved ENS/Basename → short address. Resolves async (cached);
 * shows the short address until a name arrives so the sentence is never hex-only for
 * long. Returns undefined only when there's no address at all.
 */
export function useResolvedName(address?: string, descriptorName?: string): string | undefined {
  const [name, setName] = useState<string | undefined>(descriptorName);
  useEffect(() => {
    if (descriptorName) { setName(descriptorName); return; }
    if (!isAddress(address)) { setName(undefined); return; }
    setName(shortAddr(address));
    let cancelled = false;
    resolveRecipientIdentity(address!)
      .then((r) => { if (!cancelled && r?.name) setName(r.name); })
      .catch(() => {});
    return () => { cancelled = true; };
  }, [address, descriptorName]);
  return name;
}
