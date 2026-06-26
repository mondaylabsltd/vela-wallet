/**
 * Press-and-hold confirm button for danger-level signing actions.
 *
 * The founder mandate: an unlimited/grant-all approval or an opaque eth_sign must
 * NOT be one tap away from a normal transfer. Holding adds deliberate friction
 * (and a haptic) proportional to the risk, so a fat-finger can't authorize a
 * drainer. A normal action keeps the one-tap <VelaButton>.
 */
import React, { useCallback, useEffect, useRef } from 'react';
import { Text, View, ActivityIndicator, type ViewStyle } from 'react-native';
import Animated, {
  useSharedValue, useAnimatedStyle, withTiming, runOnJS, cancelAnimation,
} from 'react-native-reanimated';
import * as Haptics from 'expo-haptics';
import { color, text, inter, radius, space, createStyles } from '@/constants/theme';

const AnimatedPressable = Animated.createAnimatedComponent(
  require('react-native').Pressable,
);

const HOLD_MS = 800;

interface Props {
  title: string;
  hint: string;
  onConfirm: () => void;
  disabled?: boolean;
  loading?: boolean;
  style?: ViewStyle;
}

export function HoldToConfirmButton({ title, hint, onConfirm, disabled, loading, style }: Props) {
  const progress = useSharedValue(0);
  const fired = useRef(false);

  const fire = useCallback(() => {
    if (fired.current) return;
    fired.current = true;
    // Re-check at completion: the action can become disabled mid-hold (e.g. a gas
    // estimate fails). Never fire onConfirm for a now-disabled action.
    if (disabled || loading) return;
    Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success).catch(() => {});
    onConfirm();
  }, [onConfirm, disabled, loading]);

  // If the action is disabled/loading mid-hold, abort the in-flight fill.
  useEffect(() => {
    if (disabled || loading) {
      cancelAnimation(progress);
      progress.value = 0;
    }
  }, [disabled, loading, progress]);

  const start = useCallback(() => {
    if (disabled || loading) return;
    fired.current = false;
    Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium).catch(() => {});
    progress.value = withTiming(1, { duration: HOLD_MS }, (finished) => {
      if (finished) runOnJS(fire)();
    });
  }, [disabled, loading, fire, progress]);

  const end = useCallback(() => {
    cancelAnimation(progress);
    progress.value = withTiming(0, { duration: 160 });
  }, [progress]);

  const fillStyle = useAnimatedStyle(() => ({ width: `${progress.value * 100}%` }));

  return (
    <AnimatedPressable
      onPressIn={start}
      onPressOut={end}
      disabled={disabled || loading}
      style={[styles.btn, (disabled || loading) && styles.disabled, style]}
    >
      {/* Sweeping fill that completes the action. */}
      <Animated.View style={[styles.fill, fillStyle]} pointerEvents="none" />
      {loading ? (
        <ActivityIndicator color={color.fg.inverse} />
      ) : (
        <View style={styles.labels} pointerEvents="none">
          <Text style={styles.title}>{title}</Text>
          <Text style={styles.hint}>{hint}</Text>
        </View>
      )}
    </AnimatedPressable>
  );
}

const styles = createStyles(() => ({
  btn: {
    paddingVertical: space.lg,
    borderRadius: radius.xl,
    alignItems: 'center',
    justifyContent: 'center',
    backgroundColor: color.error.base,
    overflow: 'hidden',
  },
  fill: {
    position: 'absolute',
    left: 0, top: 0, bottom: 0,
    backgroundColor: 'rgba(0,0,0,0.22)',
  },
  labels: { alignItems: 'center', gap: 1 },
  title: { fontSize: text.lg, ...inter.semibold, color: color.fg.inverse },
  hint: { fontSize: text.xs, ...inter.medium, color: color.fg.inverse, opacity: 0.85 },
  disabled: { opacity: 0.45 },
}));
