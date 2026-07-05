/**
 * IdenticonViewerSheet — a large, legible view of an address's Nimiq identicon.
 *
 * The avatars in the app are deliberately small (44px on Home, 40px in lists),
 * so the identicon's geometry is hard to read. This is the "see it big" escape
 * hatch: it renders the SAME pattern at full size — the identicon is pure SVG,
 * so there is no upscaling and no blur; a 220px render is the 44px one drawn
 * larger. The point isn't decoration: a Nimiq identicon is a visual fingerprint
 * of the address, so a big, memorable view is a real way to recognise "this is
 * my account" at a glance and to eyeball-match an address.
 *
 * Generic on purpose (name + address) so any avatar — the Home account, an
 * account-switcher row, a contact — can hand off to one viewer. It always draws
 * the identicon regardless of the app's avatar-style preference, because the
 * pattern is a property of the address, not of how the avatar happens to render.
 */
import React from 'react';
import { Pressable, Text, View, useWindowDimensions } from 'react-native';
import { useTranslation } from 'react-i18next';
import { Check, Copy, X } from 'lucide-react-native';

import { AppModal } from '@/components/ui/AppModal';
import { Identicon } from '@/components/ui/Identicon';
import { useCopyFeedback } from '@/hooks/use-copy-feedback';
import { hapticSuccess } from '@/services/platform';
import { color, createStyles, font, inter, radius, space, text } from '@/constants/theme';

interface Props {
  visible: boolean;
  onClose: () => void;
  /** Display name shown under the identicon (account or contact name). */
  name: string;
  /** The identicon seed — also the address shown + copied. */
  address: string;
}

export function IdenticonViewerSheet({ visible, onClose, name, address }: Props) {
  const { t } = useTranslation();
  const { copied, copy } = useCopyFeedback(2000);
  const { width } = useWindowDimensions();

  // Big but bounded — ~56% of the narrow edge, capped so it never overwhelms a
  // large screen or a rotated phone.
  const idcSize = Math.round(Math.min(width * 0.56, 220));

  const onCopy = () => { hapticSuccess(); copy(address); };

  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.sheet}>
        <View style={styles.head}>
          <View style={styles.headSpacer} />
          <Text style={styles.headTitle} numberOfLines={1}>{t('componentsUi.identiconViewer.title')}</Text>
          <Pressable
            onPress={onClose}
            hitSlop={8}
            style={styles.closeBtn}
            accessibilityRole="button"
            accessibilityLabel={t('componentsUi.identiconViewer.close')}
          >
            <X size={20} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>

        <View style={styles.body}>
          <View style={[styles.idcRing, { width: idcSize + 12, height: idcSize + 12, borderRadius: (idcSize + 12) / 2 }]}>
            {!!address && <Identicon seed={address} size={idcSize} />}
          </View>

          {!!name && <Text style={styles.name} numberOfLines={1}>{name}</Text>}

          <Text style={styles.caption}>{t('componentsUi.identiconViewer.caption')}</Text>

          <Pressable
            onPress={onCopy}
            style={({ pressed }) => [styles.copyRow, pressed && styles.copyRowPressed]}
            accessibilityRole="button"
            accessibilityLabel={t('componentsUi.identiconViewer.copyAddress')}
          >
            {copied ? (
              <Check size={15} color={color.success.base} strokeWidth={2.5} />
            ) : (
              <Copy size={15} color={color.fg.subtle} strokeWidth={2} />
            )}
            <Text style={[styles.addr, copied && styles.addrCopied]} numberOfLines={2}>
              {copied ? t('componentsUi.identiconViewer.copied') : address}
            </Text>
          </Pressable>
        </View>
      </View>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  sheet: { backgroundColor: color.bg.base },
  head: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'space-between',
    paddingHorizontal: space['2xl'], paddingVertical: space.md,
  },
  headSpacer: { width: 34 },
  headTitle: { flex: 1, textAlign: 'center', fontSize: text.xl, ...inter.bold, color: color.fg.base, paddingHorizontal: space.sm },
  closeBtn: { width: 34, height: 34, borderRadius: 17, alignItems: 'center', justifyContent: 'center' },

  body: { alignItems: 'center', paddingHorizontal: space['2xl'], paddingTop: space.lg, paddingBottom: space['4xl'] },

  // A hairline ring lifts the identicon off the sheet even when its own
  // background is close to the sheet color.
  idcRing: {
    alignItems: 'center', justifyContent: 'center',
    borderWidth: 1, borderColor: color.border.base,
    backgroundColor: color.bg.raised,
  },

  name: { marginTop: space['2xl'], fontSize: text['2xl'], ...inter.bold, color: color.fg.base, textAlign: 'center' },
  caption: {
    marginTop: space.sm, fontSize: text.sm, ...inter.regular, color: color.fg.muted,
    textAlign: 'center', lineHeight: 19, maxWidth: 320,
  },

  copyRow: {
    flexDirection: 'row', alignItems: 'center', justifyContent: 'center', gap: space.sm,
    marginTop: space['3xl'], paddingVertical: space.md, paddingHorizontal: space.lg,
    borderRadius: radius.lg, backgroundColor: color.bg.sunken, maxWidth: '100%',
  },
  copyRowPressed: { opacity: 0.6 },
  addr: { flexShrink: 1, fontSize: text.base, fontFamily: font.mono, color: color.fg.muted, textAlign: 'center' },
  // inter.semibold's fontFamily replaces the mono face for the "Copied" state.
  addrCopied: { ...inter.semibold, color: color.success.base },
}));
