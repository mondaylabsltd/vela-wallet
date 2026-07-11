/**
 * Message Sign View (personal_sign) — plain message + SIWE domain binding.
 */
import React, { useMemo } from 'react';
import { View, Text } from 'react-native';
import { useTranslation } from 'react-i18next';
import { color } from '@/constants/theme';
import { parseSiwe, checkSiweDomainBinding, siweHost, type SiweBinding } from '@/services/siwe';
import { Pen, ShieldCheck } from 'lucide-react-native';
import { styles, riskColors } from '../signing-core';
import { IntentHeader } from '../IntentHeader';
import { WarningBanner } from '../WarningBanner';

export function MessageSignView({ hexMsg, requestOrigin }: {
  hexMsg: string;
  requestOrigin?: string;
}) {
  const { t } = useTranslation();
  const decoded = decodePersonalMessage(hexMsg);

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
          variant={binding === 'mismatch' ? 'hero' : 'eyebrow'}
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

      <View style={styles.msgBubble}>
        <View style={styles.msgTag}>
          <Pen size={10} color={color.fg.subtle} strokeWidth={2} />
          <Text style={styles.msgTagText}>{t('componentsUi.signing.personalSignTag')}</Text>
        </View>
        <Text style={styles.msgText}>{decoded}</Text>
      </View>
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

export function decodePersonalMessage(hexMsg: string): string {
  try {
    const clean = hexMsg.startsWith('0x') ? hexMsg.slice(2) : hexMsg;
    const bytes = new Uint8Array(clean.match(/.{1,2}/g)!.map(b => parseInt(b, 16)));
    const decoded = new TextDecoder().decode(bytes);
    if (/^[\x20-\x7E\n\r\t]+$/.test(decoded)) return decoded;
    return `0x${clean.slice(0, 64)}${clean.length > 64 ? '...' : ''}`;
  } catch {
    return hexMsg.slice(0, 66) + (hexMsg.length > 66 ? '...' : '');
  }
}
