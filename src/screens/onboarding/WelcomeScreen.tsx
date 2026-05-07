import React from 'react';
import { View, Text, StyleSheet, TouchableOpacity, ActivityIndicator } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { color, text, weight, space, radius } from '@/constants/theme';

interface Props {
  onCreateWallet: () => void;
  onLogin: () => void;
  loginLoading?: boolean;
}

export function WelcomeScreen({ onCreateWallet, onLogin, loginLoading }: Props) {
  return (
    <View style={styles.container}>
      <SafeAreaView style={styles.safeArea} edges={['top', 'bottom']}>
        <View style={styles.logoSection}>
          <Text style={styles.logo}>
            vel<Text style={styles.logoAccent}>a</Text>
          </Text>
          <Text style={styles.tagline}>
            Your keys, your coins.{'\n'}Simple as a tap.
          </Text>
        </View>

        <View style={styles.buttonSection}>
          <TouchableOpacity style={styles.primaryBtn} onPress={onCreateWallet} activeOpacity={0.85}>
            <Text style={styles.primaryBtnText}>Create Wallet</Text>
          </TouchableOpacity>
          <View style={styles.buttonGap} />
          <TouchableOpacity
            style={styles.secondaryBtn}
            onPress={onLogin}
            activeOpacity={0.85}
            disabled={loginLoading}
          >
            {loginLoading ? (
              <ActivityIndicator color={color.fg.subtle} />
            ) : (
              <Text style={styles.secondaryBtnText}>I already have a wallet</Text>
            )}
          </TouchableOpacity>
        </View>
      </SafeAreaView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    backgroundColor: '#1A1A18',
  },
  safeArea: {
    flex: 1,
    paddingHorizontal: space['3xl'],
  },
  logoSection: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
  },
  logo: {
    fontSize: 40,
    fontWeight: weight.bold,
    color: '#FFFFFF',
    letterSpacing: 2,
  },
  logoAccent: {
    color: color.accent.base,
  },
  tagline: {
    fontSize: text.base,
    fontWeight: weight.regular,
    color: '#999',
    marginTop: space.lg,
    textAlign: 'center',
    lineHeight: 20,
  },
  buttonSection: {
    paddingBottom: space['3xl'],
  },
  primaryBtn: {
    paddingVertical: space.xl,
    borderRadius: radius.xl,
    backgroundColor: color.accent.base,
    alignItems: 'center',
    justifyContent: 'center',
  },
  primaryBtnText: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: '#FFFFFF',
  },
  secondaryBtn: {
    paddingVertical: space.xl,
    borderRadius: radius.xl,
    borderWidth: 1,
    borderColor: '#333',
    alignItems: 'center',
    justifyContent: 'center',
  },
  secondaryBtnText: {
    fontSize: text.lg,
    fontWeight: weight.semibold,
    color: '#999',
  },
  buttonGap: {
    height: space.lg,
  },
});
