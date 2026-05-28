import '@/global.css';
import { DarkTheme, DefaultTheme, ThemeProvider } from '@react-navigation/native';
import { Stack } from 'expo-router';
import React, { useEffect, useState } from 'react';
import { Appearance, View, ActivityIndicator } from 'react-native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import { WalletProvider } from '@/models/wallet-state';
import { DAppConnectionProvider } from '@/models/dapp-connection';
import { SigningRequestModal } from '@/components/SigningRequestModal';
import { AlertProvider } from '@/components/ui/AppAlert';
import { retryPendingUploads } from '@/services/public-key-upload';
import { hasPendingUploads } from '@/services/storage';
import { loadTextScale, TextScaleProvider } from '@/constants/text-scale';
import { color, rebuildTextScale, rebuildColors, createStyles } from '@/constants/theme';
import { refreshCustomNetworks } from '@/models/network';
import {
  loadColorScheme,
  getColorSchemePreference,
  resolveColorScheme,
  applyColorScheme,
  applyWebThemeColor,
  ColorSchemeProvider,
  useColorSchemePreference,
} from '@/constants/color-scheme';

function AppShell() {
  const { resolved } = useColorSchemePreference();

  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <WalletProvider>
        <DAppConnectionProvider>
          <ThemeProvider value={resolved === 'dark' ? DarkTheme : DefaultTheme}>
            <Stack screenOptions={{ headerShown: false }} key={resolved}>
              <Stack.Screen name="(tabs)" />
              <Stack.Screen name="onboarding" />
              <Stack.Screen name="send" options={{ presentation: 'modal' }} />
              <Stack.Screen name="receive" options={{ presentation: 'modal' }} />
              <Stack.Screen name="token-detail" options={{ presentation: 'modal' }} />
              <Stack.Screen name="add-token" options={{ presentation: 'modal' }} />
              <Stack.Screen name="history" options={{ presentation: 'modal' }} />
              <Stack.Screen name="about" options={{ presentation: 'modal' }} />
            </Stack>
            <SigningRequestModal />
          </ThemeProvider>
        </DAppConnectionProvider>
      </WalletProvider>
    </GestureHandlerRootView>
  );
}

export default function RootLayout() {
  const [ready, setReady] = useState(false);

  useEffect(() => {
    Promise.all([
      loadTextScale().then(() => rebuildTextScale()),
      loadColorScheme().then(() => {
        const pref = getColorSchemePreference();
        // Set native color scheme so useColorScheme() returns correct value
        applyColorScheme(pref);
        const systemScheme = Appearance.getColorScheme();
        const resolved = resolveColorScheme(pref, systemScheme);
        rebuildColors(resolved === 'dark');
        applyWebThemeColor(resolved);
      }),
      refreshCustomNetworks(),
    ]).then(() => setReady(true));

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
      <ColorSchemeProvider>
        <AlertProvider>
          <AppShell />
        </AlertProvider>
      </ColorSchemeProvider>
    </TextScaleProvider>
  );
}

const styles = createStyles(() => ({
  loading: {
    flex: 1,
    justifyContent: 'center',
    alignItems: 'center',
    backgroundColor: color.bg.base,
  },
}));
