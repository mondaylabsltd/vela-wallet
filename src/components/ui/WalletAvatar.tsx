/**
 * Account avatar — honours the avatar-style preference: the classic accent
 * initial in a soft circle, or a Nimiq identicon seeded by the account's Safe
 * address. One component so Home, the account switcher and Settings previews
 * can never drift apart.
 */
import React from 'react';
import { View, Text, Pressable } from 'react-native';
import { color, inter, createStyles } from '@/constants/theme';
import { Identicon } from '@/components/ui/Identicon';
import { useAvatarStyle } from '@/hooks/use-avatar-style';
import { useIdenticonViewer } from '@/components/ui/IdenticonViewerProvider';
import { isAddress } from '@/models/types';
import { hapticSelection } from '@/services/platform';
import i18n from '@/i18n';

export function WalletAvatar({ name, address, size = 40, letterSize, enlargeable = false }: {
  name: string;
  /** Safe address — the identicon seed. Falls back to the initial without one. */
  address?: string;
  size?: number;
  /** Font size for the initial; defaults to the ~0.34 ratio of the classic 40/44px looks. */
  letterSize?: number;
  /** When the avatar is an identicon, tap it to open the large viewer. */
  enlargeable?: boolean;
}) {
  const style = useAvatarStyle();
  const openViewer = useIdenticonViewer();

  if (style === 'identicon' && address) {
    const idc = <Identicon seed={address} size={size} />;
    if (enlargeable && isAddress(address)) {
      return (
        <Pressable
          // stopPropagation so tapping the avatar enlarges it instead of also
          // firing the row/button it sits inside (e.g. the Home account button).
          // No accessibilityRole="button": these avatars live INSIDE button-role
          // rows, and a <button> nested in a <button> is invalid HTML on web.
          // Rendering as a plain labelled pressable keeps the tap without nesting.
          onPress={(e) => { e.stopPropagation?.(); hapticSelection(); openViewer(name, address); }}
          hitSlop={6}
          accessibilityLabel={i18n.t('componentsUi.identiconViewer.a11yOpen')}
        >
          {idc}
        </Pressable>
      );
    }
    return idc;
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
