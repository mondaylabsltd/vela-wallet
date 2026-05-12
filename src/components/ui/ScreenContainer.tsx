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
 * Subscribes to text scale context so that when the user changes text size
 * in Settings, this component re-renders.  Because the parent screen also
 * re-renders (SettingsScreen consumes useTextScale), all children receive
 * fresh JSX and the createStyles Proxy returns updated styles automatically.
 *
 * No key-based remounting — pure re-render, zero flicker.
 */
export function ScreenContainer({ children, style, edges = ['top'] }: Props) {
  // Force re-render on text scale / color scheme change so styles.* Proxy returns fresh values
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
