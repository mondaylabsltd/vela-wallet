import React from 'react';
import { View, type ViewStyle } from 'react-native';
import { color, radius, shadow, createStyles } from '@/constants/theme';

interface Props {
  children: React.ReactNode;
  style?: ViewStyle;
  elevated?: boolean;
}

export function VelaCard({ children, style, elevated }: Props) {
  return (
    <View style={[styles.card, elevated && styles.elevated, style]}>
      {children}
    </View>
  );
}

const styles = createStyles(() => ({
  card: {
    backgroundColor: color.bg.raised,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: color.border.base,
    ...shadow.sm,
  },
  elevated: {
    borderColor: 'transparent',
    ...shadow.md,
  },
}));
