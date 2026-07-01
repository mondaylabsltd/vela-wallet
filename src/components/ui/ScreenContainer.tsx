import React from 'react';
import { KeyboardAvoidingView, Platform, View, type ViewStyle } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { color, space, createStyles } from '@/constants/theme';
import { useTextScale } from '@/constants/text-scale';

interface Props {
  children: React.ReactNode;
  style?: ViewStyle;
  edges?: ('top' | 'bottom' | 'left' | 'right')[];
}

/**
 * Standard screen wrapper with safe area, consistent padding, and keyboard avoidance.
 *
 * Dark mode is handled by Stack key={resolved} in _layout.tsx which remounts
 * the entire navigation tree — no need to subscribe to color scheme here.
 */
export function ScreenContainer({ children, style, edges = ['top'] }: Props) {
  // Re-render on text scale change so createStyles Proxy returns fresh values
  useTextScale();

  return (
    <View style={[styles.container, style]}>
      <SafeAreaView style={styles.safeArea} edges={edges}>
        <KeyboardAvoidingView
          style={styles.keyboardAvoiding}
          // iOS needs 'padding' to lift inputs. On Android 'height' mis-measures
          // under edge-to-edge and can leave bottom inputs (Send amount, wallet
          // name) hidden behind the keyboard — let native adjustResize handle it.
          behavior={Platform.OS === 'ios' ? 'padding' : undefined}
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
