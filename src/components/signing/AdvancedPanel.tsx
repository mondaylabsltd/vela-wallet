/**
 * 技术细节 (Technical details) — the expert layer. Additive, collapsed by default:
 * it NEVER removes the plain-language default above it, it only *adds* the truth.
 * One grey rounded card holds every technical row (the mock): truncated addresses
 * with a quiet copy, the decoded function signature, and the raw calldata. This is
 * the "A 叠加式" expert view — an expert sees more, and the safety framing (summary
 * + warnings) is still all there above it.
 */
import React, { useCallback, useEffect, useMemo, useState } from 'react';
import { View, Text, Pressable, ScrollView } from 'react-native';
import { useTranslation } from 'react-i18next';
import * as Clipboard from 'expo-clipboard';
import { color } from '@/constants/theme';
import { isAddress } from '@/models/types';
import { type ClearSignResult } from '@/services/clear-signing';
import { lookupSelector } from '@/services/selector-registry';
import { ChevronDown, Copy, Check } from 'lucide-react-native';
import { styles, localizeLabel } from './signing-core';

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

export function AdvancedPanel({ method, params, clearSign }: {
  method: string;
  params: any[];
  clearSign: ClearSignResult | null;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  const tx = method === 'eth_sendTransaction' ? params?.[0] ?? {} : null;
  const data: string | undefined = tx?.data && tx.data !== '0x' ? tx.data : undefined;
  const selector = data ? data.slice(0, 10) : undefined;
  const functionSig = useFunctionSig(selector);

  // Every address involved, labelled + truncated to one line — the full 0x the calm
  // rows tuck away here.
  const addresses = useMemo(() => {
    const seen = new Set<string>();
    const out: { label: string; address: string }[] = [];
    const push = (label: string, a?: string) => {
      if (a && isAddress(a) && !seen.has(a.toLowerCase())) { seen.add(a.toLowerCase()); out.push({ label, address: a }); }
    };
    if (clearSign) {
      for (const f of clearSign.fields) if (f.address) push(localizeLabel(f.label), f.address);
      push(t('componentsUi.signing.interactingLabel'), clearSign.contractAddress);
    }
    if (method === 'eth_sendTransaction') {
      // tx.to with calldata is the CONTRACT you're calling (matches the main view's
      // "interacting with"), not a "recipient" — only a plain value transfer is a "to".
      const hasCallData = params?.[0]?.data && params[0].data !== '0x';
      push(hasCallData ? t('componentsUi.signing.interactingLabel') : t('componentsUi.signing.labelTo', { defaultValue: 'To' }), params?.[0]?.to);
    }
    // EIP-5792 batch: each call's target contract.
    if (method === 'wallet_sendCalls') {
      const calls = params?.[0]?.calls;
      if (Array.isArray(calls)) calls.forEach((c: any, i: number) => push(`${t('componentsUi.signing.batchCall')} ${i + 1}`, c?.to));
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

  // Structured rows shown inside the one grey card.
  const rows: { label: string; value: string; copy?: string }[] = [
    ...addresses.map((a) => ({ label: a.label, value: midTrunc(a.address), copy: a.address })),
    ...detailFields.map((f) => ({ label: localizeLabel(f.label), value: f.value })),
    ...(selector ? [{ label: t('componentsUi.signing.techFunction', { defaultValue: 'Function' }), value: functionSig ?? `${selector} · ${t('componentsUi.signing.techUnknownFn', { defaultValue: 'unrecognized' })}` }] : []),
    ...(data ? [{ label: `CALLDATA · ${dataBytes} BYTES`, value: midTrunc(data, 18, 6), copy: data }] : []),
  ];

  if (rows.length === 0 && !raw) return null;

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
          {rows.map((r, i) => (
            <View key={i} style={[styles.drawerRow, i === 0 && styles.drawerRowFirst]}>
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
            <View style={[styles.drawerRow, rows.length === 0 && styles.drawerRowFirst]}>
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
