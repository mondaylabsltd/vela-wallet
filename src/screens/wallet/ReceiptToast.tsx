/**
 * Receipt toast — strong "money in" cue, slides in from the top (cross-platform).
 */
import React, { useEffect } from 'react';
import { useTranslation } from 'react-i18next';
import { Text, View } from 'react-native';
import Animated, { Easing, useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';

import { styles } from './HomeScreen.styles';

export function ReceiptToast({ amount, token, top }: { amount: string; token: string; top: number }) {
  const { t } = useTranslation();
  const v = useSharedValue(0);
  useEffect(() => {
    v.value = withTiming(1, { duration: 320, easing: Easing.out(Easing.quad) });
  }, [v]);
  const style = useAnimatedStyle(() => ({
    opacity: v.value,
    transform: [{ translateY: (1 - v.value) * -24 }],
  }));
  return (
    <Animated.View pointerEvents="none" style={[styles.toast, { top }, style]}>
      <View style={styles.toastDot} />
      <Text style={styles.toastText}>{t('home.toastReceived', { amount, token })}</Text>
    </Animated.View>
  );
}
