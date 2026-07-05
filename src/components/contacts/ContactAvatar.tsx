/**
 * Contact avatar — a colored initial, deterministically tinted from the address
 * so the same contact always reads the same. To keep a long list calm, the tint
 * is drawn from a small curated palette of muted, harmonious tones (not the full
 * hue wheel) — enough color to tell people apart, without the rainbow. Tones are
 * mode-aware. A smart-contract account gets a small badge so "another wallet"
 * reads differently from a person.
 */
import React from 'react';
import { View, Text, Pressable } from 'react-native';
import { Wallet } from 'lucide-react-native';
import { color, inter, isDarkMode, createStyles } from '@/constants/theme';
import { Identicon } from '@/components/ui/Identicon';
import { useAvatarStyle } from '@/hooks/use-avatar-style';
import { useIdenticonViewer } from '@/components/ui/IdenticonViewerProvider';
import { hapticSelection } from '@/services/platform';
import { isAddress } from '@/models/types';
import i18n from '@/i18n';
import type { ContactKind } from '@/services/contacts';

/**
 * Curated hues (degrees) — a deliberately restrained spread that sits with the
 * warm Vela palette: terracotta, slate, sage, dusty rose, ochre, muted teal,
 * soft violet, dusty cyan. Harmonious as a set, so a screen full of avatars
 * reads as one family rather than confetti.
 */
const HUES = [18, 210, 150, 340, 42, 268, 122, 190];

/** Deterministic index into {@link HUES} from a string seed. */
function pickHue(seed: string): number {
  let h = 0;
  for (let i = 0; i < seed.length; i++) h = seed.charCodeAt(i) + ((h << 5) - h);
  return HUES[Math.abs(h) % HUES.length];
}

export function ContactAvatar({ name, address, kind, size = 40, enlargeable = false }: {
  name: string;
  address: string;
  kind?: ContactKind;
  size?: number;
  /** When the avatar is an identicon, tap it to open the large viewer. */
  enlargeable?: boolean;
}) {
  const avatarStyle = useAvatarStyle();
  const openViewer = useIdenticonViewer();
  const seed = address || name;
  const initial = (name.trim()[0] ?? address.slice(2, 3) ?? '?').toUpperCase();
  const h = pickHue(seed);

  // Identicons need a real address to be meaningful — the add-contact form
  // feeds this the live TextInput value on every keystroke, so hold the tinted
  // initial until the input is an actual address (not a partial or arbitrary
  // string, which would hash to a different identicon per keystroke).
  const showIdenticon = avatarStyle === 'identicon' && isAddress(address);

  // Low saturation + mode-aware lightness keeps the tint quiet in both themes.
  const dark = isDarkMode();
  const bg = dark ? `hsl(${h}, 24%, 22%)` : `hsl(${h}, 32%, 91%)`;
  const fg = dark ? `hsl(${h}, 38%, 74%)` : `hsl(${h}, 40%, 36%)`;

  return (
    <View style={{ width: size, height: size }}>
      {showIdenticon ? (
        enlargeable ? (
          <Pressable
            // stopPropagation so tapping the avatar enlarges it instead of also
            // triggering the row it sits inside (open contact / pick recipient).
            // No accessibilityRole="button": these avatars live INSIDE button-role
            // rows, and a <button> nested in a <button> is invalid HTML on web.
            onPress={(e) => { e.stopPropagation?.(); hapticSelection(); openViewer(name, address); }}
            hitSlop={6}
            accessibilityLabel={i18n.t('componentsUi.identiconViewer.a11yOpen')}
          >
            <Identicon seed={address} size={size} />
          </Pressable>
        ) : (
          <Identicon seed={address} size={size} />
        )
      ) : (
        <View
          style={[
            styles.circle,
            { width: size, height: size, borderRadius: size / 2, backgroundColor: bg },
          ]}
        >
          <Text style={[styles.initial, { color: fg, fontSize: size * 0.42 }]}>
            {initial}
          </Text>
        </View>
      )}
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
