/**
 * Balance number + its loading skeleton — the hero's only actor.
 *
 * `Balance` is the fintech "atomic number" cascade: fit-to-width on one line,
 * drop cents when big, fall back to compact notation ($1.23M) before going
 * illegible. All handled by <AmountText/>; here we just feed it value + prefs.
 *
 * `BalanceSkeleton` is the shimmer placeholder shown before the first balance is
 * known — a bare "0" there reads as a real, wrong value. A light band sweeps
 * across a sunken bar (raised-on-sunken reads as a highlight in BOTH themes),
 * sized to the balance's line box.
 */
import React, { useEffect } from 'react';
import { View } from 'react-native';
import Animated, { Easing, useAnimatedStyle, useSharedValue, withRepeat, withTiming } from 'react-native-reanimated';

import { AmountText } from '@/components/ui/AmountText';
import { shouldShowDecimals } from '@/services/currency';

import { styles, SKELETON_W, SKELETON_BAND_W } from './HomeScreen.styles';

export function Balance({ value, symbol, code }: { value: number; symbol: string; code: string }) {
  return (
    <AmountText
      value={value}
      symbol={symbol}
      size={56}
      symbolScale={0.58}
      minScale={0.55}
      showDecimals={shouldShowDecimals(value, code)}
      style={styles.balanceInt}
      tailStyle={styles.balanceDec}
      containerStyle={styles.balanceFill}
    />
  );
}

export function BalanceSkeleton() {
  const x = useSharedValue(0);
  useEffect(() => {
    x.value = withRepeat(withTiming(1, { duration: 1150, easing: Easing.inOut(Easing.quad) }), -1, false);
  }, [x]);
  const band = useAnimatedStyle(() => ({
    transform: [{ translateX: -SKELETON_BAND_W + x.value * (SKELETON_W + SKELETON_BAND_W) }],
  }));
  return (
    <View style={styles.balanceFill} accessibilityLabel="…" accessibilityRole="progressbar">
      <View style={styles.balanceSkeleton}>
        <Animated.View style={[styles.balanceSkeletonBand, band]} />
      </View>
    </View>
  );
}
