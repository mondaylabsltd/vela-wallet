/**
 * Account avatar — honours the avatar-style preference: the classic accent
 * initial in a soft circle, or a Nimiq identicon seeded by the account's Safe
 * address. One component so Home, the account switcher and Settings previews
 * can never drift apart.
 */
import React from 'react';
import { View, Text } from 'react-native';
import { color, inter, createStyles } from '@/constants/theme';
import { Identicon } from '@/components/ui/Identicon';
import { useAvatarStyle } from '@/hooks/use-avatar-style';

export function WalletAvatar({ name, address, size = 40, letterSize }: {
  name: string;
  /** Safe address — the identicon seed. Falls back to the initial without one. */
  address?: string;
  size?: number;
  /** Font size for the initial; defaults to the ~0.34 ratio of the classic 40/44px looks. */
  letterSize?: number;
}) {
  const style = useAvatarStyle();

  if (style === 'identicon' && address) {
    return <Identicon seed={address} size={size} />;
  }

  return (
    <View style={[styles.circle, { width: size, height: size, borderRadius: size / 2 }]}>
      <Text style={[styles.letter, { fontSize: letterSize ?? Math.round(size * 0.34) }]}>
        {(name[0] ?? 'V').toUpperCase()}
      </Text>
    </View>
  );
}

const styles = createStyles(() => ({
  circle: {
    backgroundColor: color.accent.soft,
    alignItems: 'center',
    justifyContent: 'center',
  },
  letter: { ...inter.bold, color: color.accent.base },
}));
