import React from 'react';
import { View, Text, StyleSheet, Image } from 'react-native';
import { weight } from '@/constants/theme';

interface Props {
  symbol: string;
  logoUrl?: string | null;
  size?: number;
  bgColor?: string;
  textColor?: string;
}

// Generate a deterministic color from a string
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

export function TokenLogo({ symbol, logoUrl, size = 40, bgColor, textColor }: Props) {
  const bg = bgColor ?? stringToBgColor(symbol);
  const fg = textColor ?? stringToColor(symbol);

  if (logoUrl) {
    return (
      <Image
        source={{ uri: logoUrl }}
        style={[styles.image, { width: size, height: size, borderRadius: size / 2 }]}
        defaultSource={undefined}
      />
    );
  }

  return (
    <View style={[styles.fallback, { width: size, height: size, borderRadius: size / 2, backgroundColor: bg }]}>
      <Text style={[styles.label, { color: fg, fontSize: size * 0.42 }]}>
        {symbol.charAt(0).toUpperCase()}
      </Text>
    </View>
  );
}

const styles = StyleSheet.create({
  image: {
    backgroundColor: '#F0F0F0',
  },
  fallback: {
    alignItems: 'center',
    justifyContent: 'center',
  },
  label: {
    fontWeight: weight.bold,
  },
});
