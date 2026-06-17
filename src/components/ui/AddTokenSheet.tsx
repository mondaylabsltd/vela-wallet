/**
 * AddTokenSheet — the "Add Token" flow presented as a bottom sheet.
 *
 * Wraps {@link AddTokenPanel} in {@link AppModal} so callers (e.g. the Send
 * token picker) can let users import a token without leaving the screen. The
 * full-screen route equivalent is AddTokenScreen.
 */
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { X } from 'lucide-react-native';
import { AppModal } from '@/components/ui/AppModal';
import { AddTokenPanel } from '@/components/ui/AddTokenPanel';
import { color, createStyles, inter, space, text } from '@/constants/theme';
import { useTranslation } from 'react-i18next';

interface Props {
  visible: boolean;
  onClose: () => void;
  /** Fires after a token or network is saved (e.g. to refresh a list). */
  onAdded?: () => void;
}

export function AddTokenSheet({ visible, onClose, onAdded }: Props) {
  const { t } = useTranslation();
  return (
    <AppModal visible={visible} onClose={onClose}>
      <View style={styles.sheet}>
        <View style={styles.head}>
          <View style={styles.headSpacer} />
          <Text style={styles.title}>{t('addToken.navTitle')}</Text>
          <Pressable onPress={onClose} hitSlop={8} style={styles.closeBtn}>
            <X size={18} color={color.fg.base} strokeWidth={2} />
          </Pressable>
        </View>
        <AddTokenPanel onAdded={onAdded} />
      </View>
    </AppModal>
  );
}

const styles = createStyles(() => ({
  sheet: {
    flex: 1,
    backgroundColor: color.bg.base,
    paddingHorizontal: space['2xl'],
  },
  head: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.md,
  },
  headSpacer: { width: 34 },
  title: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  closeBtn: {
    width: 34,
    height: 34,
    borderRadius: 17,
    alignItems: 'center',
    justifyContent: 'center',
  },
}));
