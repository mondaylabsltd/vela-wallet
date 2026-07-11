/**
 * 技术细节 (Technical details) — the expert layer. Additive, collapsed by default:
 * it NEVER removes the plain-language default above it, it only *adds* the full 0x
 * addresses (with copy + explorer), the function selector, and the exact bytes/JSON
 * being signed. This is the "A 叠加式" expert view — an expert sees more truth, and
 * the safety framing (summary + warnings) is still all there.
 */
import React, { useCallback, useMemo, useState } from 'react';
import { View, Text, Pressable, ScrollView } from 'react-native';
import { useTranslation } from 'react-i18next';
import * as Clipboard from 'expo-clipboard';
import { color } from '@/constants/theme';
import { isAddress } from '@/models/types';
import { explorerBaseURL } from '@/models/network';
import { openBrowser } from '@/services/platform';
import { type ClearSignResult } from '@/services/clear-signing';
import { ChevronDown, Copy, Check, ExternalLink } from 'lucide-react-native';
import { styles, localizeLabel, SigningChainContext } from './signing-core';

/** One full address with copy + block-explorer — the actions the calm rows dropped. */
function AddressRow({ label, address }: { label: string; address: string }) {
  const chainId = React.useContext(SigningChainContext);
  const [copied, setCopied] = useState(false);
  const explorerBase = explorerBaseURL(chainId);
  const explorerUrl = explorerBase ? `${explorerBase}/address/${address}` : undefined;
  const copy = useCallback(async () => {
    await Clipboard.setStringAsync(address);
    setCopied(true);
    setTimeout(() => setCopied(false), 1500);
  }, [address]);
  return (
    <View style={styles.advAddrRow}>
      <View style={{ flex: 1 }}>
        <Text style={styles.advAddrLabel}>{label}</Text>
        <Text style={styles.advAddrValue} selectable>{address}</Text>
      </View>
      <Pressable onPress={copy} hitSlop={8} style={[styles.copyBtn, copied && styles.copyBtnDone]}>
        {copied
          ? <Check size={12} color={color.success.base} strokeWidth={2.5} />
          : <Copy size={12} color={color.fg.muted} strokeWidth={2} />}
      </Pressable>
      {explorerUrl && (
        <Pressable onPress={() => openBrowser(explorerUrl)} hitSlop={8} style={styles.copyBtn}>
          <ExternalLink size={12} color={color.fg.muted} strokeWidth={2} />
        </Pressable>
      )}
    </View>
  );
}

export function AdvancedPanel({ method, params, clearSign }: {
  method: string;
  params: any[];
  clearSign: ClearSignResult | null;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);

  // Every address involved, labeled — the full 0x the calm rows tuck away here.
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
    if (method === 'eth_sendTransaction') push(t('componentsUi.signing.labelTo', { defaultValue: 'To' }), params?.[0]?.to);
    return out;
  }, [clearSign, method, params, t]);

  // The exact bytes/JSON being signed — untruncated, so a power user can verify.
  const raw = useMemo(() => {
    try {
      if (method === 'eth_sendTransaction') {
        const tx = params?.[0] ?? {};
        const data: string | undefined = tx.data && tx.data !== '0x' ? tx.data : undefined;
        return [
          data ? `selector: ${data.slice(0, 10)}` : null,
          tx.value && tx.value !== '0x0' ? `value: ${tx.value}` : null,
          data ? `data: ${data}` : null,
        ].filter(Boolean).join('\n\n');
      }
      if (method.includes('signTypedData')) {
        const rawData = params?.[1] ?? params?.[0];
        const obj = typeof rawData === 'string' ? JSON.parse(rawData) : rawData;
        return JSON.stringify(obj, null, 2);
      }
      if (method === 'personal_sign') return String(params?.[0] ?? '');
      if (method === 'eth_sign') return String((params?.length > 1 ? params[1] : params?.[0]) ?? '');
      return '';
    } catch { return ''; }
  }, [method, params]);

  const detailFields = clearSign?.fields.filter((f) => f.detail) ?? [];
  if (!raw && detailFields.length === 0 && addresses.length === 0) return null;

  return (
    <View>
      <Pressable style={styles.detailsToggle} onPress={() => setOpen((o) => !o)}>
        <Text style={styles.detailsToggleText}>{t('componentsUi.signing.advancedToggle')}</Text>
        <ChevronDown
          size={12} color={color.fg.subtle} strokeWidth={2}
          style={open ? { transform: [{ rotate: '180deg' }] } : undefined}
        />
      </Pressable>
      {open && (
        <View style={styles.advancedBody}>
          {addresses.map((a, i) => (
            <AddressRow key={`a${i}`} label={a.label} address={a.address} />
          ))}
          {detailFields.map((f, i) => (
            <View key={`d${i}`} style={styles.genRow}>
              <Text style={styles.genLabel}>{localizeLabel(f.label)}</Text>
              <Text style={styles.genValue} numberOfLines={4}>{f.value}</Text>
            </View>
          ))}
          {!!raw && (
            <ScrollView style={styles.advancedRaw} nestedScrollEnabled>
              <Text style={styles.rawText} selectable>{raw}</Text>
            </ScrollView>
          )}
        </View>
      )}
    </View>
  );
}
