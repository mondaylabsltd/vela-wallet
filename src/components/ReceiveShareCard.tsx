/**
 * ReceiveShareCard — the off-screen, branded card that gets screenshotted for
 * the "share / save" image on native. Two variants:
 *   - address: QR of the wallet address + every supported network
 *   - request: QR of the EIP-681 request + the amount/token/network summary
 *
 * The web build draws an equivalent image on a canvas (services/share-card.ts);
 * keep the two visually in sync.
 */
import { ChainLogo } from '@/components/ChainLogo';
import { QRCode } from '@/components/QRCode';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';
import React from 'react';
import { Image, Text, View } from 'react-native';

const LOGO = require('../../assets/images/icon.png');

export interface ShareNetwork {
  label: string;
  name: string;
  color: string;
  bg: string;
  logoURL?: string;
}

export interface ShareCardModel {
  variant: 'address' | 'request';
  name: string;
  /** The string encoded in the QR (address or ethereum: URI). */
  qrValue: string;
  /** Full recipient address. */
  address: string;
  /** Request summary, e.g. "Request 12 ETH · Ethereum". */
  summary?: string;
  /** Supported networks (address variant). */
  networks?: ShareNetwork[];
}

function short(addr: string): string {
  return addr ? `${addr.slice(0, 10)}…${addr.slice(-8)}` : '';
}

export function ReceiveShareCard({ model }: { model: ShareCardModel }) {
  const isRequest = model.variant === 'request';
  return (
    <View style={styles.card}>
      {/* Brand header */}
      <View style={styles.brand}>
        <Image source={LOGO} style={styles.brandLogo} />
        <Text style={styles.brandName}>Vela Wallet</Text>
      </View>

      {/* QR */}
      <View style={styles.qrBox}>
        <QRCode value={model.qrValue || model.address} size={196} />
      </View>

      {isRequest ? (
        <>
          {/* The request leads; the wallet name is a subordinate identity line. */}
          {!!model.summary && <Text style={styles.summaryHero}>{model.summary}</Text>}
          <Text style={styles.nameSub}>{model.name}</Text>
          <Text style={styles.addr}>{short(model.address)}</Text>
        </>
      ) : (
        <>
          <Text style={styles.name}>{model.name}</Text>
          <Text style={styles.addr}>{short(model.address)}</Text>
          {!!model.networks?.length && (
            <View style={styles.netSection}>
              <Text style={styles.netLabel}>{`${model.networks.length} supported networks`}</Text>
              <View style={styles.netGrid}>
                {model.networks.map((n) => (
                  <View key={n.name} style={styles.netChip}>
                    <ChainLogo label={n.label} color={n.color} bgColor={n.bg} logoURL={n.logoURL} size={18} />
                    <Text style={styles.netName} numberOfLines={1}>{n.name}</Text>
                  </View>
                ))}
              </View>
            </View>
          )}
        </>
      )}

      {/* Footer */}
      <Text style={styles.footer}>getvela.app</Text>
    </View>
  );
}

const styles = createStyles(() => ({
  card: {
    width: 360,
    backgroundColor: '#FFFFFF',
    borderRadius: 28,
    paddingHorizontal: 28,
    paddingTop: 24,
    paddingBottom: 22,
    alignItems: 'center',
  },
  brand: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 8,
    marginBottom: 18,
  },
  brandLogo: { width: 24, height: 24, borderRadius: 6 },
  brandName: {
    fontSize: text.base,
    ...inter.bold,
    color: '#16161A',
  },
  qrBox: {
    borderWidth: 1,
    borderColor: '#ECEBE4',
    borderRadius: radius.xl,
    padding: 18,
    backgroundColor: '#FFFFFF',
    marginBottom: 18,
  },
  name: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: '#16161A',
    marginBottom: 6,
  },
  // Request variant: the request is the hero, the wallet name a quiet identity line.
  summaryHero: {
    fontSize: text['2xl'],
    ...inter.bold,
    color: '#16161A',
    marginBottom: 6,
    textAlign: 'center',
  },
  nameSub: {
    fontSize: text.base,
    ...inter.medium,
    color: '#8A8A96',
    marginBottom: 10,
  },
  addr: {
    fontSize: text.sm,
    ...inter.medium,
    fontFamily: font.mono,
    color: '#8A8A96',
    marginBottom: 4,
  },
  netSection: {
    alignSelf: 'stretch',
    marginTop: 16,
  },
  netLabel: {
    fontSize: text.xs,
    ...inter.medium,
    color: '#B0ADA5',
    marginBottom: 10,
    textAlign: 'center',
  },
  netGrid: {
    flexDirection: 'row',
    flexWrap: 'wrap',
    justifyContent: 'space-between',
    rowGap: 8,
  },
  netChip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 7,
    width: '48.5%',
    backgroundColor: '#F5F3EF',
    borderRadius: radius.full,
    paddingHorizontal: 10,
    paddingVertical: 7,
  },
  netName: {
    fontSize: text.xs,
    ...inter.semibold,
    color: '#16161A',
    flexShrink: 1,
  },
  footer: {
    fontSize: text.sm,
    ...inter.semibold,
    color: '#B5B5BE',
    marginTop: 18,
  },
}));
