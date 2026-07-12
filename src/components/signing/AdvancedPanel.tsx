/**
 * 技术细节 (Technical details) — the expert layer. Additive, collapsed by default:
 * it NEVER removes the plain-language default above it, it only *adds* the truth.
 * One grey rounded card holds every technical row (the mock): truncated addresses
 * with a quiet copy, the decoded function signature, and the raw calldata. This is
 * the "A 叠加式" expert view — an expert sees more, and the safety framing (summary
 * + warnings) is still all there above it.
 */
import React, { useCallback, useContext, useEffect, useMemo, useState } from 'react';
import { View, Text, Pressable, ScrollView } from 'react-native';
import { useTranslation } from 'react-i18next';
import * as Clipboard from 'expo-clipboard';
import { color } from '@/constants/theme';
import { isAddress, tokenLogoURLsByAddress } from '@/models/types';
import { type ClearSignResult } from '@/services/clear-signing';
import { knownToken } from '@/services/tokens';
import { lookupSelector } from '@/services/selector-registry';
import { TokenLogo } from '@/components/TokenLogo';
import { explorerBaseURL, explorerAddressURL } from '@/models/network';
import { openURL } from '@/services/platform';
import { useLocalePrefs, numberSeparators } from '@/services/locale-format';
import type { AssetSimResult } from '@/services/tx-simulation';
import { ChevronDown, Copy, Check, FileText, ExternalLink } from 'lucide-react-native';
import { Identicon } from '@/components/ui/Identicon';
import { styles, localizeLabel, SigningChainContext } from './signing-core';
import { useAddressIdentity, type AddrKind } from './use-address-identity';
import { summariseSimResult } from './BalanceChangePreview';

// Instant signatures for the common selectors, so the 函数 row fills without a
// round-trip; anything else is resolved async via the shared selector registry.
const KNOWN_SELECTORS: Record<string, string> = {
  '0xa9059cbb': 'transfer(address,uint256)',
  '0x095ea7b3': 'approve(address,uint256)',
  '0x23b872dd': 'transferFrom(address,address,uint256)',
  '0xa22cb465': 'setApprovalForAll(address,bool)',
  '0x40c10f19': 'mint(address,uint256)',
  '0x39509351': 'increaseAllowance(address,uint256)',
  '0x42842e0e': 'safeTransferFrom(address,address,uint256)',
  '0xb88d4fde': 'safeTransferFrom(address,address,uint256,bytes)',
  '0x2e1a7d4d': 'withdraw(uint256)',
  '0xd0e30db0': 'deposit()',
  '0x6e553f65': 'deposit(uint256,address)',
  '0xb460af94': 'withdraw(uint256,address,address)',
  '0xba087652': 'redeem(uint256,address,address)',
};

/** Mid-ellipsis so an address stays on one line (0xd8dA6BF269…4d37aA96045). */
const midTrunc = (a: string, head = 12, tail = 8) =>
  a.length > head + tail + 2 ? `${a.slice(0, head)}…${a.slice(-tail)}` : a;

function useFunctionSig(selector?: string): string | undefined {
  const [sig, setSig] = useState<string | undefined>(selector ? KNOWN_SELECTORS[selector] : undefined);
  useEffect(() => {
    if (!selector) { setSig(undefined); return; }
    const known = KNOWN_SELECTORS[selector];
    if (known) { setSig(known); return; }
    setSig(undefined);
    let cancelled = false;
    lookupSelector(selector).then((sigs) => { if (!cancelled && sigs[0]) setSig(sigs[0]); }).catch(() => {});
    return () => { cancelled = true; };
  }, [selector]);
  return sig;
}

function CopyBtn({ value }: { value: string }) {
  const [copied, setCopied] = useState(false);
  const copy = useCallback(async () => {
    await Clipboard.setStringAsync(value);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [value]);
  return (
    <Pressable onPress={copy} hitSlop={10} style={styles.drawerCopy}>
      {copied
        ? <Check size={14} color={color.success.base} strokeWidth={2.5} />
        : <Copy size={14} color={color.fg.muted} strokeWidth={2} />}
    </Pressable>
  );
}

export function AdvancedPanel({ method, params, clearSign, simResult = null, heroFlows = [] }: {
  method: string;
  params: any[];
  clearSign: ClearSignResult | null;
  /** Simulation result, rendered as a factual "模拟结果" row (net balance changes)
      instead of a green promise in the main view. */
  simResult?: AssetSimResult | null;
  heroFlows?: { token?: string; dir: 'out' | 'in' }[];
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const chainId = useContext(SigningChainContext);
  useLocalePrefs();
  const sep = numberSeparators();

  const tx = method === 'eth_sendTransaction' ? params?.[0] ?? {} : null;
  const data: string | undefined = tx?.data && tx.data !== '0x' ? tx.data : undefined;
  const selector = data ? data.slice(0, 10) : undefined;
  const functionSig = useFunctionSig(selector);

  // Every address involved, tagged wallet vs contract so each row can show the
  // right identity (identicon + ENS/contact for a wallet, glyph + name for a
  // contract) — the full 0x stays visible either way.
  const addresses = useMemo(() => {
    const seen = new Set<string>();
    const out: { label: string; address: string; kind: AddrKind; seedName?: string; isToken?: boolean }[] = [];
    // isToken → render the real token logo (name-sized) instead of a contract glyph.
    const push = (label: string, a: string | undefined, kind: AddrKind, seedName?: string, isToken?: boolean) => {
      if (a && isAddress(a) && !seen.has(a.toLowerCase())) {
        seen.add(a.toLowerCase());
        out.push({ label, address: a, kind, seedName, isToken: isToken ?? (kind === 'contract' && !!knownToken(a)) });
      }
    };
    if (clearSign) {
      // Token contracts referenced by amount fields (transfer token, swap in/out) —
      // pushed FIRST so a token op's contract shows as its token (logo + address),
      // not a bare "interacting with". Deduped against the interacting contract.
      for (const f of clearSign.fields) {
        if ((f.role === 'send-amount' || f.role === 'receive-amount') && f.tokenAddress) {
          push(t('componentsUi.signing.labelToken', { defaultValue: 'Token' }), f.tokenAddress, 'contract', undefined, true);
        }
      }
      // A spender is a contract (router/protocol); recipients/owners/generic are
      // treated as wallets (identicon + name), the safer, more useful default.
      for (const f of clearSign.fields) if (f.address) push(localizeLabel(f.label), f.address, f.role === 'spender' ? 'contract' : 'wallet');
      push(t('componentsUi.signing.interactingLabel'), clearSign.contractAddress, 'contract', clearSign.contractName);
    }
    if (method === 'eth_sendTransaction') {
      // tx.to with calldata is the CONTRACT you're calling (matches the main view's
      // "interacting with"), not a "recipient" — only a plain value transfer is a "to".
      const hasCallData = params?.[0]?.data && params[0].data !== '0x';
      push(
        hasCallData ? t('componentsUi.signing.interactingLabel') : t('componentsUi.signing.labelTo', { defaultValue: 'To' }),
        params?.[0]?.to,
        hasCallData ? 'contract' : 'wallet',
      );
    }
    // EIP-5792 batch: each call's target contract.
    if (method === 'wallet_sendCalls') {
      const calls = params?.[0]?.calls;
      if (Array.isArray(calls)) calls.forEach((c: any, i: number) => push(`${t('componentsUi.signing.batchCall')} ${i + 1}`, c?.to, 'contract'));
    }
    return out;
  }, [clearSign, method, params, t]);

  // Raw payload for the non-tx cases (typed data JSON / message).
  const raw = useMemo(() => {
    try {
      if (method.includes('signTypedData')) {
        const rawData = params?.[1] ?? params?.[0];
        return JSON.stringify(typeof rawData === 'string' ? JSON.parse(rawData) : rawData, null, 2);
      }
      if (method === 'personal_sign') return String(params?.[0] ?? '');
      if (method === 'eth_sign') return String((params?.length > 1 ? params[1] : params?.[0]) ?? '');
      if (method === 'wallet_sendCalls') return JSON.stringify(params?.[0]?.calls ?? params?.[0] ?? [], null, 2);
      return '';
    } catch { return ''; }
  }, [method, params]);

  const detailFields = clearSign?.fields.filter((f) => f.detail) ?? [];
  const dataBytes = data ? Math.floor((data.length - 2) / 2) : 0;

  // Factual simulation result — "−1,000 USDC · 无其他变动" — the calm, non-
  // promissory replacement for the green "nothing leaves your wallet" line.
  const simSummary = useMemo(() => {
    const s = summariseSimResult(simResult, heroFlows, sep);
    if (!s) return null;
    if (s.noChange) return t('componentsUi.signing.simResultNoChange', { defaultValue: 'No asset changes' });
    const base = s.parts.join(' · ');
    return s.corroborated
      ? `${base} · ${t('componentsUi.signing.simResultNoOther', { defaultValue: 'no other changes' })}`
      : base;
  }, [simResult, heroFlows, sep, t]);

  // Non-address rows (decoded params, function signature, raw calldata) — the
  // address rows render above these with their resolved identity.
  const otherRows: { label: string; value: string; copy?: string }[] = [
    ...detailFields.map((f) => ({ label: localizeLabel(f.label), value: f.value })),
    ...(selector ? [{ label: t('componentsUi.signing.techFunction', { defaultValue: 'Function' }), value: functionSig ?? `${selector} · ${t('componentsUi.signing.techUnknownFn', { defaultValue: 'unrecognized' })}` }] : []),
    ...(data ? [{ label: `CALLDATA · ${dataBytes} BYTES`, value: midTrunc(data, 18, 6), copy: data }] : []),
  ];

  if (!simSummary && addresses.length === 0 && otherRows.length === 0 && !raw) return null;

  return (
    <View>
      <Pressable style={styles.detailsToggle} onPress={() => setOpen((o) => !o)}>
        <Text style={styles.detailsToggleText}>{t('componentsUi.signing.advancedToggle')}</Text>
        <ChevronDown
          size={16} color={color.fg.subtle} strokeWidth={2}
          style={open ? { transform: [{ rotate: '180deg' }] } : undefined}
        />
      </Pressable>
      {open && (
        <View style={styles.drawerCard}>
          {/* Simulation result — the factual net balance change, replacing the
              green "nothing leaves your wallet" promise in the main view. */}
          {!!simSummary && (
            <View style={[styles.drawerRow, styles.drawerRowFirst]}>
              <View style={{ flex: 1 }}>
                <Text style={styles.drawerLabel}>{t('componentsUi.signing.simResultLabel', { defaultValue: 'Simulation result' })}</Text>
                <Text style={styles.drawerValue} numberOfLines={2}>{simSummary}</Text>
              </View>
            </View>
          )}
          {/* Identity-enriched address rows — a resolved name + identicon/glyph
              + explorer link, with the raw hex kept as ground truth. */}
          {addresses.map((a, i) => (
            <AddressRow key={`a${i}`} entry={a} chainId={chainId} first={!simSummary && i === 0} />
          ))}
          {otherRows.map((r, i) => (
            <View key={`o${i}`} style={[styles.drawerRow, !simSummary && addresses.length === 0 && i === 0 && styles.drawerRowFirst]}>
              <View style={{ flex: 1 }}>
                <Text style={styles.drawerLabel}>{r.label}</Text>
                <Text style={styles.drawerValue} numberOfLines={1}>{r.value}</Text>
              </View>
              {r.copy && <CopyBtn value={r.copy} />}
            </View>
          ))}
          {/* Signature (typed data / message): the FULL, exact payload being signed —
              the whole point of a 712 review — as a complete, scrollable, copyable
              block, not one lone address. */}
          {!!raw && (
            <View style={[styles.drawerRow, !simSummary && addresses.length === 0 && otherRows.length === 0 && styles.drawerRowFirst]}>
              <View style={{ flex: 1 }}>
                <Text style={styles.drawerLabel}>{method.includes('signTypedData') || method === 'wallet_sendCalls' ? 'JSON' : t('componentsUi.signing.signMessage')}</Text>
                <ScrollView style={styles.drawerRaw} nestedScrollEnabled>
                  <Text style={styles.drawerValue} selectable>{raw}</Text>
                </ScrollView>
                <CopyBtn value={raw} />
              </View>
            </View>
          )}
        </View>
      )}
    </View>
  );
}

/**
 * One address row in the technical panel: role label, resolved identity
 * (identicon + ENS/contact/account name for a wallet; glyph + token/contract
 * name for a contract), the raw hex kept underneath as ground truth, plus copy
 * and block-explorer actions. The name is ADDITIVE — the exact bytes are always
 * shown, so a spoofed label can't hide the address.
 */
function AddressRow({ entry, chainId, first }: {
  entry: { label: string; address: string; kind: AddrKind; seedName?: string; isToken?: boolean };
  chainId: number;
  first: boolean;
}) {
  const { name } = useAddressIdentity(entry.address, chainId, entry.kind, entry.seedName);
  const trunc = midTrunc(entry.address);
  const explorer = explorerBaseURL(chainId) ? explorerAddressURL(chainId, entry.address) : null;

  // A token → its real logo at name size; a wallet → its identicon fingerprint;
  // any other contract → a neutral glyph.
  const avatar = entry.isToken
    ? <TokenLogo symbol={name ?? '?'} logoUrls={tokenLogoURLsByAddress(chainId, entry.address)} size={18} />
    : entry.kind === 'wallet'
      ? <Identicon seed={entry.address} size={18} />
      : <View style={styles.drawerContractGlyph}><FileText size={11} color={color.fg.muted} strokeWidth={2} /></View>;

  return (
    <View style={[styles.drawerRow, first && styles.drawerRowFirst]}>
      <View style={{ flex: 1 }}>
        <Text style={styles.drawerLabel}>{entry.label}</Text>
        <View style={styles.drawerIdentityRow}>
          {avatar}
          {name
            ? <Text style={styles.drawerName} numberOfLines={1}>{name}</Text>
            : <Text style={styles.drawerValue} numberOfLines={1}>{trunc}</Text>}
        </View>
        {/* When a name resolved, the exact address still shows — right below it. */}
        {!!name && <Text style={styles.drawerAddrSub} numberOfLines={1}>{trunc}</Text>}
      </View>
      {explorer && (
        <Pressable onPress={() => openURL(explorer)} hitSlop={10} style={styles.drawerCopy}>
          <ExternalLink size={14} color={color.fg.muted} strokeWidth={2} />
        </Pressable>
      )}
      <CopyBtn value={entry.address} />
    </View>
  );
}
