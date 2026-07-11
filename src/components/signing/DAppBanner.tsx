/**
 * dApp banner — the "who's asking" header at the top of every signing sheet, plus
 * the FROM-account row (SigningAccountRow) which lives at the BOTTOM (below the fee),
 * near the confirm action, not crowding the banner.
 */
import React, { useEffect, useState } from 'react';
import { View, Text, Image, Pressable } from 'react-native';
import { useTranslation } from 'react-i18next';
import { chainName, DEFAULT_NETWORKS } from '@/models/network';
import { color } from '@/constants/theme';
import { ChainLogo } from '@/components/ChainLogo';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
import { ChevronDown } from 'lucide-react-native';
import { styles } from './signing-core';

/**
 * The site's OWN favicon, derived from its host — no third-party favicon service,
 * so signing a tx never leaks the dApp you're on to Google/DuckDuckGo/etc. Returns
 * undefined for non-registrable hosts (the test harness `clear-signing-test`,
 * `localhost`, bare IPs) so the banner falls back to a letter monogram.
 */
function faviconForHost(domain?: string): string | undefined {
  if (!domain) return undefined;
  const host = domain.replace(/^[a-z]+:\/\//i, '').split('/')[0].split(':')[0].trim();
  if (!host || !host.includes('.') || /^\d+(\.\d+){3}$/.test(host)) return undefined;
  return `https://${host}/favicon.ico`;
}

export function DAppBanner({ name, domain, icon, chainId }: {
  name: string;
  domain?: string;
  icon?: string;
  chainId: number;
}) {
  const net = DEFAULT_NETWORKS.find(n => n.chainId === chainId);

  // Prefer an explicit icon (e.g. the in-app browser's captured favicon); otherwise
  // derive the site's own /favicon.ico. Fall back to a letter monogram if the image
  // fails to load (404 / not an image). Reset the failure flag when the target changes.
  const logoUri = icon ?? faviconForHost(domain);
  const [logoFailed, setLogoFailed] = useState(false);
  useEffect(() => { setLogoFailed(false); }, [logoUri]);
  const showLogo = !!logoUri && !logoFailed;

  return (
    <View style={styles.dappBanner}>
      {/* dApp identity ←→ chain (the FROM account now lives at the bottom). */}
      <View style={styles.dappRow1}>
        {showLogo ? (
          <Image
            source={{ uri: logoUri }}
            style={styles.dappLogo}
            onError={() => setLogoFailed(true)}
          />
        ) : (
          <View style={[styles.dappLogo, styles.dappLogoFallback]}>
            <Text style={styles.dappLogoText}>{(name[0] ?? '?').toUpperCase()}</Text>
          </View>
        )}
        <View style={styles.dappInfo}>
          <Text style={styles.dappName} numberOfLines={1}>{name}</Text>
          {domain && <Text style={styles.dappDomain} numberOfLines={1}>{domain}</Text>}
        </View>
        <View style={styles.dappChainRow}>
          {net && <ChainLogo label={net.iconLabel} color={net.iconColor} bgColor={net.iconBg} logoURL={net.logoURL} size={16} />}
          <Text style={styles.dappChainName}>{chainName(chainId)}</Text>
        </View>
      </View>
    </View>
  );
}

/**
 * The wallet you're signing FROM — a quiet row at the bottom of the sheet (below the
 * fee), near the confirm action. Collapsed to identicon + name; tap reveals the 0x.
 */
export function SigningAccountRow({ accountName, accountAddress }: {
  accountName?: string;
  accountAddress?: string;
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  if (!accountName) return null;
  return (
    <>
      <Pressable style={styles.signAccountRow} onPress={() => setOpen((o) => !o)} hitSlop={6}>
        <Text style={styles.signAccountLabel}>{t('componentsUi.signing.signingAccount')}</Text>
        <View style={styles.signAccountRight}>
          {accountAddress ? <ContactAvatar name={accountName} address={accountAddress} size={18} /> : null}
          <Text style={styles.signAccountName} numberOfLines={1}>{accountName}</Text>
          {accountAddress ? (
            <ChevronDown
              size={13} color={color.fg.subtle} strokeWidth={2}
              style={open ? { transform: [{ rotate: '180deg' }] } : undefined}
            />
          ) : null}
        </View>
      </Pressable>
      {open && accountAddress && (
        <Text style={styles.signAccountAddr} selectable>{accountAddress}</Text>
      )}
    </>
  );
}
