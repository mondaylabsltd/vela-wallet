import { AddTokenPanel } from '@/components/ui/AddTokenPanel';
import { ScreenContainer } from '@/components/ui/ScreenContainer';
import { color, createStyles, inter, space, text } from '@/constants/theme';
import { useSafeRouter } from '@/hooks/use-safe-router';
import { ArrowLeft } from 'lucide-react-native';
import React from 'react';
import { Pressable, Text, View } from 'react-native';
import { useTranslation } from 'react-i18next';

/**
 * Add Token / Add Network — full-screen host for {@link AddTokenPanel}.
 * The same panel is also presented as a bottom sheet (AddTokenSheet); this
 * route just wraps it with a back-navigation title bar.
 */
export default function AddTokenScreen() {
  const router = useSafeRouter();
  const { t } = useTranslation();

  return (
    <ScreenContainer>
      <View style={styles.navBar}>
        <Pressable onPress={() => router.back()} hitSlop={8} style={styles.navBtn}>
          <ArrowLeft size={22} color={color.fg.base} strokeWidth={2} />
        </Pressable>
        <Text style={styles.navTitle}>{t('addToken.navTitle')}</Text>
        <View style={styles.navSpacer} />
      </View>
      <AddTokenPanel />
    </ScreenContainer>
  );
}

const styles = createStyles(() => ({
  navBar: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingVertical: space.lg,
    marginBottom: space.md,
  },
  navBtn: {
    width: 40,
    height: 40,
    alignItems: 'center',
    justifyContent: 'center',
  },
  navTitle: {
    fontSize: text.xl,
    ...inter.bold,
    color: color.fg.base,
  },
  navSpacer: { minWidth: 50 },
}));
