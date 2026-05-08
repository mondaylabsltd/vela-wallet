import React from 'react';
import { View, type ViewStyle } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { color, space, createStyles } from '@/constants/theme';
import { useTextScale } from '@/constants/text-scale';

interface Props {
  children: React.ReactNode;
  style?: ViewStyle;
  edges?: ('top' | 'bottom' | 'left' | 'right')[];
}

/**
 * Standard screen wrapper with safe area and consistent padding.
 *
 * Subscribes to text scale context so that when the user changes text size,
 * the screen content re-mounts with fresh styles from the createStyles proxy.
 * The outer container stays painted (no flash) — only children re-mount.
 */
export function ScreenContainer({ children, style, edges = ['top'] }: Props) {
  const { version } = useTextScale();

  return (
    <View style={[styles.container, style]}>
      <SafeAreaView style={styles.safeArea} edges={edges} key={version}>
        {children}
      </SafeAreaView>
    </View>
  );
}

const styles = createStyles(() => ({
  container: {
    flex: 1,
    backgroundColor: color.bg.base,
  },
  safeArea: {
    flex: 1,
    paddingHorizontal: space['3xl'],
  },
}));
