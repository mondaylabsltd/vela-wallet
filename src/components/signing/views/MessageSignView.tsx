/**
 * Message Sign View (personal_sign) — plain message + SIWE domain binding.
 */
import React, { useMemo } from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { parseSiwe, checkSiweDomainBinding, siweHost, type SiweBinding } from '@/services/siwe';
import { ShieldCheck } from 'lucide-react-native';
import { styles, riskColors } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { WarningBanner } from '../WarningBanner';
// Unicode-aware decoder (issue #82) — a re-export so SigningSheet keeps importing
// `decodePersonalMessage` from this view, but the logic lives in one shared module
// (emoji / CJK / accents decode as text instead of falling back to raw hex).
export { decodePersonalMessage } from '@/services/decode-sign-message';
import { decodePersonalMessage } from '@/services/decode-sign-message';

export function MessageSignView({ hexMsg, requestOrigin }: {
  hexMsg: string;
  requestOrigin?: string;
}) {
  const { t } = useTranslation();
  const decoded = decodePersonalMessage(hexMsg);

  // Non-printable hex isn't a human message — it can be a disguised hash (a transfer
  // or approval hidden behind personal_sign). Legit apps sign readable text; flag the
  // hex case so it never reads as calmly as a login prompt (F9).
  const nonPrintable = useMemo(() => {
    try {
      const clean = hexMsg.startsWith('0x') ? hexMsg.slice(2) : hexMsg;
      if (!/^[0-9a-fA-F]+$/.test(clean) || clean.length < 2) return false;
      const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map((b) => parseInt(b, 16)));
      return !/^[\x20-\x7E\n\r\t]+$/.test(new TextDecoder().decode(bytes));
    } catch { return false; }
  }, [hexMsg]);

  // Sign-In with Ethereum: bind the domain inside the message to the request
  // origin. A mismatch is the canonical phishing pattern.
  const siwe = useMemo(() => parseSiwe(decoded), [decoded]);
  const binding: SiweBinding | null = useMemo(
    () => (siwe ? checkSiweDomainBinding(siwe.domain, requestOrigin) : null),
    [siwe, requestOrigin],
  );

  if (siwe) {
    return (
      <View>
        <IntentHeader
          intent={t('componentsUi.signing.signInIntent')}
          color={binding === 'mismatch' ? color.error.base : color.fg.base}
          variant="eyebrow"
          colorEyebrow={binding === 'mismatch'}
        />

        <View style={styles.genericFields}>
          <View style={styles.genRow}>
            <Text style={styles.contractLabel}>{t('componentsUi.signing.siweDomain')}</Text>
            <Text style={[styles.genValue, binding === 'mismatch' && { color: riskColors().danger }]} numberOfLines={1}>
              {siweHost(siwe.domain) ?? siwe.domain}
            </Text>
          </View>
          {!!siwe.statement && (
            <View style={styles.genRow}>
              <Text style={styles.contractLabel}>{t('componentsUi.signing.siweStatement')}</Text>
              <Text style={styles.genValue} numberOfLines={3}>{siwe.statement}</Text>
            </View>
          )}
        </View>

        {binding === 'mismatch' && (
          <WarningBanner
            severity="danger"
            text={t('componentsUi.signing.siweMismatch', { domain: siwe.domain, origin: hostLabel(requestOrigin) })}
          />
        )}
        {binding === 'ok' && (
          <View style={styles.siweOkRow}>
            <ShieldCheck size={13} color={color.success.base} strokeWidth={2} />
            <Text style={styles.siweOkText}>{t('componentsUi.signing.siweOk', { domain: siwe.domain })}</Text>
          </View>
        )}
      </View>
    );
  }

  return (
    <View>
      {/* context shown in dApp banner */}
      <IntentHeader intent={t('componentsUi.signing.signMessage')} color={color.fg.base} variant="eyebrow" />

      {/* Just the message — the "personal_sign · no gas fee" tag was redundant noise
          (a signature obviously costs no gas). */}
      <View style={styles.msgBubble}>
        <Text style={styles.msgText}>{decoded}</Text>
      </View>

      {nonPrintable && (
        <WarningBanner
          severity="caution"
          text={t('componentsUi.signing.hexMessageWarning', {
            defaultValue: "This isn't readable text — it could be a transaction or approval in disguise. Only sign if you fully trust this site.",
          })}
        />
      )}
    </View>
  );
}

/** Short host label for messages ("app.uniswap.org"). */
function hostLabel(value: string | undefined): string {
  if (!value) return '—';
  try {
    const withScheme = /^[a-z][a-z0-9+.-]*:\/\//i.test(value) ? value : `https://${value}`;
    return new URL(withScheme).host;
  } catch {
    return value;
  }
}

