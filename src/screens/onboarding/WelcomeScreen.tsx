import React from 'react';
import { View, Text, StyleSheet } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { VelaColor, VelaFont, VelaSpacing } from '@/constants/theme';
import { VelaButton } from '@/components/ui/VelaButton';

interface Props {
  onCreateWallet: () => void;
  onLogin: () => void;
}

export function WelcomeScreen({ onCreateWallet, onLogin }: Props) {
  return (
    <View style={styles.container}>
      <SafeAreaView style={styles.safeArea} edges={['top', 'bottom']}>
        <View style={styles.logoSection}>
          <Text style={styles.logo}>
            vel<Text style={styles.logoAccent}>a</Text>
          </Text>
          <Text style={styles.tagline}>Smart Account Wallet</Text>
        </View>

        <View style={styles.buttonSection}>
          <VelaButton title="Create Wallet" onPress={onCreateWallet} variant="primary" />
          <View style={styles.buttonGap} />
          <VelaButton title="I already have a wallet" onPress={onLogin} variant="secondary" />
        </View>
      </SafeAreaView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: VelaColor.bg,
  },
  safeArea: {
    flex: 1,
    paddingHorizontal: VelaSpacing.screenH,
  },
  logoSection: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  logo: {
    fontSize: 56,
    fontWeight: '700',
    color: VelaColor.textPrimary,
    letterSpacing: 2,
  },
  logoAccent: {
    color: VelaColor.accent,
  },
  tagline: {
    ...VelaFont.body(17),
    color: VelaColor.textSecondary,
    marginTop: 8,
  },
  buttonSection: {
    paddingBottom: 24,
  },
  buttonGap: {
    height: 12,
  },
});
