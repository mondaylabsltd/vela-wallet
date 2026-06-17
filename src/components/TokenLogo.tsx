import React, { useState } from 'react';
import { View, Text, Image } from 'react-native';
import { color, inter, radius, createStyles } from '@/constants/theme';
import { ChainLogo } from '@/components/ChainLogo';
import type { Network } from '@/models/network';

interface Props {
  symbol: string;
  logoUrl?: string | null;
  /** Fallback URLs to try in order when logoUrl fails */
  logoUrls?: string[];
  size?: number;
  bgColor?: string;
  textColor?: string;
  /**
   * When set, overlays the network's logo as a small badge in the bottom-right
   * corner — disambiguates same-symbol tokens across chains (USDT on Arbitrum vs
   * Polygon) and labels which chain a native coin sits on (ETH on Base).
   */
  chain?: Network | null;
}

function stringToColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  const h = Math.abs(hash) % 360;
  return `hsl(${h}, 45%, 55%)`;
}

function stringToBgColor(str: string): string {
  let hash = 0;
  for (let i = 0; i < str.length; i++) {
    hash = str.charCodeAt(i) + ((hash << 5) - hash);
  }
  const h = Math.abs(hash) % 360;
  return `hsl(${h}, 30%, 93%)`;
}

function LetterFallback({ symbol, size, bg, fg }: { symbol: string; size: number; bg: string; fg: string }) {
  return (
    <View style={[styles.fallback, { width: size, height: size, borderRadius: size / 2, backgroundColor: bg }]}>
      <Text style={[styles.label, { color: fg, fontSize: size * 0.42 }]}>
        {symbol.charAt(0).toUpperCase()}
      </Text>
    </View>
  );
}

export function TokenLogo({ symbol, logoUrl, logoUrls, size = 40, bgColor, textColor, chain }: Props) {
  const bg = bgColor ?? stringToBgColor(symbol);
  const fg = textColor ?? stringToColor(symbol);

  // Build ordered candidate list: logoUrl first, then logoUrls extras
  const candidates = React.useMemo(() => {
    const urls: string[] = [];
    if (logoUrl) urls.push(logoUrl);
    if (logoUrls) {
      for (const u of logoUrls) {
        if (!urls.includes(u)) urls.push(u);
      }
    }
    return urls;
  }, [logoUrl, logoUrls]);

  const [urlIndex, setUrlIndex] = useState(0);

  const activeUrl = candidates[urlIndex];

  const logo = activeUrl ? (
    <Image
      source={{ uri: activeUrl }}
      style={[styles.image, { width: size, height: size, borderRadius: size / 2 }]}
      onError={() => setUrlIndex(i => i + 1)}
    />
  ) : (
    <LetterFallback symbol={symbol} size={size} bg={bg} fg={fg} />
  );

  if (!chain) return logo;

  // Badge ~45% of the logo, ringed in the surrounding background so the small
  // network mark reads as separate from the token logo behind it.
  const badgeSize = Math.round(size * 0.45);
  return (
    <View style={{ width: size, height: size }}>
      {logo}
      <View style={styles.badge}>
        <ChainLogo
          label={chain.iconLabel}
          color={chain.iconColor}
          bgColor={chain.iconBg}
          logoURL={chain.logoURL}
          size={badgeSize}
        />
      </View>
    </View>
  );
}

const styles = createStyles(() => ({
  image: {
    backgroundColor: color.bg.sunken,
  },
  badge: {
    position: 'absolute',
    right: -2,
    bottom: -2,
    borderRadius: radius.full,
    borderWidth: 2,
    borderColor: color.bg.base,
  },
  fallback: {
    alignItems: 'center',
    justifyContent: 'center',
  },
  label: {
    ...inter.bold,
  },
}));
