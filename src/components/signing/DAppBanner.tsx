/**
 * dApp banner — the "who's asking" header at the top of every signing sheet.
 */
import React, { useEffect, useState } from 'react';
import { View, Text, Image } from 'react-native';
import { shortAddr } from '@/models/types';
import { chainName, DEFAULT_NETWORKS } from '@/models/network';
import { ChainLogo } from '@/components/ChainLogo';
import { ContactAvatar } from '@/components/contacts/ContactAvatar';
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

export function DAppBanner({ name, domain, icon, chainId, accountName, accountAddress }: {
  name: string;
  domain?: string;
  icon?: string;
  chainId: number;
  accountName?: string;
  accountAddress?: string;
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
      {/* Row 1: dApp identity ←→ chain */}
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

      {/* Row 2: FROM — the signing wallet, with its own nimiq identicon so the
          "from" account is as recognizable as the "to". */}
      {accountName && (
        <View style={styles.dappAccountRow}>
          {accountAddress ? <ContactAvatar name={accountName} address={accountAddress} size={18} /> : null}
          <Text style={styles.dappAccountLine} numberOfLines={1}>
            {accountName}{accountAddress ? `  ·  ${shortAddr(accountAddress)}` : ''}
          </Text>
        </View>
      )}
    </View>
  );
}
