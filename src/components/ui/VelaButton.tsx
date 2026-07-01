import React, { useCallback } from 'react';
import { Text, ActivityIndicator, type ViewStyle } from 'react-native';
import Animated, {
  useSharedValue,
  useAnimatedStyle,
  withSpring,
} from 'react-native-reanimated';
import { color, text, inter, radius, space, shadow, motion, createStyles } from '@/constants/theme';

const AnimatedPressable = Animated.createAnimatedComponent(
  require('react-native').Pressable,
);

interface Props {
  title: string;
  onPress: () => void;
  variant?: 'primary' | 'secondary' | 'accent';
  disabled?: boolean;
  loading?: boolean;
  style?: ViewStyle;
  compact?: boolean;
}

export function VelaButton({ title, onPress, variant = 'primary', disabled, loading, style, compact }: Props) {
  const scale = useSharedValue(1);

  const bgColor = variant === 'primary' ? color.fg.base : variant === 'accent' ? color.accent.base : 'transparent';
  const textColor = variant === 'secondary' ? color.fg.base : color.fg.inverse;
  const borderColor = variant === 'secondary' ? color.border.strong : 'transparent';

  const animatedStyle = useAnimatedStyle(() => ({
    transform: [{ scale: scale.value }],
  }));

  const handlePressIn = useCallback(() => {
    scale.value = withSpring(0.97, motion.spring);
  }, [scale]);

  const handlePressOut = useCallback(() => {
    scale.value = withSpring(1, motion.spring);
  }, [scale]);

  return (
    <AnimatedPressable
      onPress={onPress}
      onPressIn={handlePressIn}
      onPressOut={handlePressOut}
      disabled={disabled || loading}
      accessibilityRole="button"
      accessibilityLabel={title}
      accessibilityState={{ disabled: !!(disabled || loading), busy: !!loading }}
      style={[
        styles.button,
        compact && styles.buttonCompact,
        {
          backgroundColor: bgColor,
          borderColor,
          borderWidth: variant === 'secondary' ? 1.5 : 0,
        },
        variant !== 'secondary' && shadow.sm,
        (disabled || loading) && styles.disabled,
        animatedStyle,
        style,
      ]}
    >
      {loading ? (
        <ActivityIndicator color={textColor} />
      ) : (
        <Text style={[styles.text, compact && styles.textCompact, { color: textColor }]}>{title}</Text>
      )}
    </AnimatedPressable>
  );
}

const styles = createStyles(() => ({
  button: {
    paddingVertical: space.xl,
    borderRadius: radius.xl,
    alignItems: 'center',
    justifyContent: 'center',
  },
  buttonCompact: {
    paddingVertical: space.lg,
    paddingHorizontal: space['2xl'],
  },
  text: {
    fontSize: text.lg,
    ...inter.semibold,
  },
  textCompact: {
    fontSize: text.base,
  },
  disabled: {
    opacity: 0.45,
  },
}));
