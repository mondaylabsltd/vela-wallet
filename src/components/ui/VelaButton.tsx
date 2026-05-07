import React from 'react';
import { TouchableOpacity, Text, StyleSheet, ActivityIndicator, type ViewStyle } from 'react-native';
import { color, text, weight, radius, space } from '@/constants/theme';

interface Props {
  title: string;
  onPress: () => void;
  variant?: 'primary' | 'secondary' | 'accent';
  disabled?: boolean;
  loading?: boolean;
  style?: ViewStyle;
}

export function VelaButton({ title, onPress, variant = 'primary', disabled, loading, style }: Props) {
  const bgColor = variant === 'primary' ? color.fg.base : variant === 'accent' ? color.accent.base : 'transparent';
  const textColor = variant === 'secondary' ? color.fg.base : color.fg.inverse;
  const borderColor = variant === 'secondary' ? color.border.base : 'transparent';

  return (
    <TouchableOpacity
      onPress={onPress}
      disabled={disabled || loading}
      activeOpacity={0.85}
      style={[
        styles.button,
        { backgroundColor: bgColor, borderColor, borderWidth: variant === 'secondary' ? 1.5 : 0 },
        (disabled || loading) && styles.disabled,
        style,
      ]}
    >
      {loading ? (
        <ActivityIndicator color={textColor} />
      ) : (
        <Text style={[styles.text, { color: textColor }]}>{title}</Text>
      )}
    </TouchableOpacity>
  );
}

const styles = StyleSheet.create({
  button: {
    paddingVertical: space.xl,
    borderRadius: radius.xl,
    alignItems: 'center',
    justifyContent: 'center',
  },
  text: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
  },
  disabled: {
    opacity: 0.5,
  },
});
