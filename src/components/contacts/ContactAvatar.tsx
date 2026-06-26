/**
 * Contact avatar — a colored initial, deterministically tinted from the address
 * so the same contact always reads the same. Matches the app's avatar style
 * (initials in a soft circle; no blockies). A smart-contract account gets a small
 * badge so "another wallet" reads differently from a person.
 */
import React from 'react';
import { View, Text } from 'react-native';
import { Wallet } from 'lucide-react-native';
import { color, inter, radius, createStyles } from '@/constants/theme';
import type { ContactKind } from '@/services/contacts';

/** Deterministic hue from a string (mirrors TokenLogo's fallback tinting). */
function hue(seed: string): number {
  let h = 0;
  for (let i = 0; i < seed.length; i++) h = seed.charCodeAt(i) + ((h << 5) - h);
  return Math.abs(h) % 360;
}

export function ContactAvatar({ name, address, kind, size = 40 }: {
  name: string;
  address: string;
  kind?: ContactKind;
  size?: number;
}) {
  const seed = address || name;
  const initial = (name.trim()[0] ?? address.slice(2, 3) ?? '?').toUpperCase();
  const h = hue(seed);

  return (
    <View style={{ width: size, height: size }}>
      <View
        style={[
          styles.circle,
          { width: size, height: size, borderRadius: size / 2, backgroundColor: `hsl(${h}, 45%, 92%)` },
        ]}
      >
        <Text style={[styles.initial, { color: `hsl(${h}, 45%, 38%)`, fontSize: size * 0.42 }]}>
          {initial}
        </Text>
      </View>
      {kind === 'account' && (
        <View style={styles.badge}>
          <Wallet size={9} color={color.fg.inverse} strokeWidth={2.5} />
        </View>
      )}
    </View>
  );
}

const styles = createStyles(() => ({
  circle: {
    alignItems: 'center',
    justifyContent: 'center',
  },
  initial: {
    ...inter.bold,
    letterSpacing: -0.5,
  },
  badge: {
    position: 'absolute',
    right: -1,
    bottom: -1,
    width: 16,
    height: 16,
    borderRadius: 8,
    backgroundColor: color.info.base,
    borderWidth: 1.5,
    borderColor: color.bg.raised,
    alignItems: 'center',
    justifyContent: 'center',
  },
}));
