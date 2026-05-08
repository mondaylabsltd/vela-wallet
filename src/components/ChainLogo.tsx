import React, { useState } from 'react';
import { View, Text, Image } from 'react-native';
import { weight, createStyles } from '@/constants/theme';

interface Props {
  label: string;
  color: string;
  bgColor: string;
  /** Remote logo URL (e.g. from ethereum-data API) */
  logoURL?: string;
  size?: number;
}

export function ChainLogo({ label, color, bgColor, logoURL, size = 32 }: Props) {
  const [failed, setFailed] = useState(false);

  if (logoURL && !failed) {
    return (
      <Image
        source={{ uri: logoURL }}
        style={[styles.image, { width: size, height: size, borderRadius: size / 2 }]}
        onError={() => setFailed(true)}
      />
    );
  }

  return (
    <View style={[styles.container, { width: size, height: size, borderRadius: size / 2, backgroundColor: bgColor }]}>
      <Text style={[styles.label, { color, fontSize: size * 0.3 }]}>{label}</Text>
    </View>
  );
}

const styles = createStyles(() => ({
  container: {
    alignItems: 'center',
    justifyContent: 'center',
  },
  image: {
    backgroundColor: 'transparent',
  },
  label: {
    fontWeight: weight.bold,
  },
}));
