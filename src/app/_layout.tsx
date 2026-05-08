import '@/global.css';
import { DarkTheme, DefaultTheme, ThemeProvider } from '@react-navigation/native';
import { Stack } from 'expo-router';
import React, { useEffect, useState } from 'react';
import { useColorScheme, View, ActivityIndicator, StyleSheet } from 'react-native';
import { WalletProvider } from '@/models/wallet-state';
import { retryPendingUploads } from '@/services/public-key-upload';
import { hasPendingUploads } from '@/services/storage';
import { loadTextScale, TextScaleProvider } from '@/constants/text-scale';
import { color, rebuildTextScale } from '@/constants/theme';

function AppShell() {
  const colorScheme = useColorScheme();

  return (
    <WalletProvider>
      <ThemeProvider value={colorScheme === 'dark' ? DarkTheme : DefaultTheme}>
        <Stack screenOptions={{ headerShown: false }}>
          <Stack.Screen name="(tabs)" />
          <Stack.Screen name="onboarding" />
          <Stack.Screen name="send" options={{ presentation: 'modal' }} />
          <Stack.Screen name="receive" options={{ presentation: 'modal' }} />
          <Stack.Screen name="token-detail" options={{ presentation: 'modal' }} />
          <Stack.Screen name="add-token" options={{ presentation: 'modal' }} />
          <Stack.Screen name="history" options={{ presentation: 'modal' }} />
        </Stack>
      </ThemeProvider>
    </WalletProvider>
  );
}

export default function RootLayout() {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    loadTextScale().then(() => {
      rebuildTextScale();
      setReady(true);
    });

    hasPendingUploads().then((has) => {
      if (has) {
        retryPendingUploads().catch(() => {});
      }
    });
  }, []);

  if (!ready) {
    return (
      <View style={styles.loading}>
        <ActivityIndicator size="small" color={color.accent.base} />
      </View>
    );
  }

  return (
    <TextScaleProvider>
      <AppShell />
    </TextScaleProvider>
  );
}

const styles = StyleSheet.create({
  loading: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    backgroundColor: color.bg.base,
  },
});
