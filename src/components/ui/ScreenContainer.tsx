import React from 'react';
import { KeyboardAvoidingView, Platform, View, type ViewStyle } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { color, space, createStyles } from '@/constants/theme';
import { useTextScale } from '@/constants/text-scale';
import { useColorSchemePreference } from '@/constants/color-scheme';

interface Props {
  children: React.ReactNode;
  style?: ViewStyle;
  edges?: ('top' | 'bottom' | 'left' | 'right')[];
}

/**
 * Standard screen wrapper with safe area, consistent padding, and keyboard avoidance.
 *
 * Subscribes to color scheme and text scale contexts so that createStyles
 * Proxy returns fresh values on re-render. The actual re-render trigger
 * comes from Appearance.setColorScheme() which fires useColorScheme()
 * throughout the entire app — including React Navigation internals.
 */
export function ScreenContainer({ children, style, edges = ['top'] }: Props) {
  useTextScale();
  useColorSchemePreference();

  return (
    <View style={[styles.container, style]}>
      <SafeAreaView style={styles.safeArea} edges={edges}>
        <KeyboardAvoidingView
          style={styles.keyboardAvoiding}
          behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
        >
          {children}
        </KeyboardAvoidingView>
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
  keyboardAvoiding: {
    flex: 1,
  },
}));
