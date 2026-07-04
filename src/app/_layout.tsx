import '@/polyfills'; // MUST be first: installs crypto/btoa/atob/Buffer on Hermes before any dep loads
import '@/global.css';
import { DarkTheme, DefaultTheme, ThemeProvider } from '@react-navigation/native';
import { Stack } from 'expo-router';
import React, { Component, useEffect, useState, type ReactNode } from 'react';
import { Appearance, View, Text, ActivityIndicator, Pressable } from 'react-native';
import { GestureHandlerRootView } from 'react-native-gesture-handler';
import {
  useFonts,
  PlusJakartaSans_400Regular,
  PlusJakartaSans_500Medium,
  PlusJakartaSans_600SemiBold,
  PlusJakartaSans_700Bold,
} from '@expo-google-fonts/plus-jakarta-sans';
import { WalletProvider } from '@/models/wallet-state';
import { DAppConnectionProvider } from '@/models/dapp-connection';
import { SigningRequestModal } from '@/components/SigningRequestModal';
import { AlertProvider } from '@/components/ui/AppAlert';
import { retryPendingUploads } from '@/services/public-key-upload';
import { installFaultConsole } from '@/services/dev/fault-injection';
import { installMetricsConsole } from '@/services/metrics';
import { installParallelConsole, applyParallelSpaceOnBoot } from '@/services/dev/parallel-space';
import { ParallelSpaceBadge } from '@/components/dev/ParallelSpaceBadge';
import { hasPendingUploads, loadLocalePrefs, loadRpcProviders, loadServiceEndpoints } from '@/services/storage';
import { loadAvatarStyle } from '@/services/avatar-style';
import i18n, { loadLanguage } from '@/i18n';
import { LanguageProvider, useLanguagePreference } from '@/i18n/language';
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

// ---------------------------------------------------------------------------
// Error boundary — catches unhandled errors to prevent white screen
// ---------------------------------------------------------------------------

class ErrorBoundary extends Component<{ children: ReactNode }, { error: Error | null }> {
  state = { error: null as Error | null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: React.ErrorInfo) {
    console.error('[ErrorBoundary]', error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <View style={{ flex: 1, justifyContent: 'center', alignItems: 'center', padding: 32, backgroundColor: '#faf9f7' }}>
          <Text style={{ fontSize: 18, fontWeight: '700', color: '#1a1a18', marginBottom: 8 }}>
            {i18n.t('common.somethingWrong')}
          </Text>
          <Text style={{ fontSize: 13, color: '#8a8580', textAlign: 'center', lineHeight: 20, marginBottom: 20 }}>
            {this.state.error.message}
          </Text>
          <Pressable
            onPress={() => this.setState({ error: null })}
            style={{ paddingHorizontal: 24, paddingVertical: 12, backgroundColor: '#E8572A', borderRadius: 12 }}
          >
            <Text style={{ color: '#fff', fontWeight: '700', fontSize: 15 }}>{i18n.t('common.tryAgain')}</Text>
          </Pressable>
        </View>
      );
    }
    return this.props.children;
  }
}

function AppShell() {
  const { resolved } = useColorSchemePreference();
  const { resolved: language } = useLanguagePreference();

  return (
    <GestureHandlerRootView style={{ flex: 1 }}>
      <WalletProvider>
        <DAppConnectionProvider>
          <ThemeProvider value={resolved === 'dark' ? DarkTheme : DefaultTheme}>
            {/* key folds in language so a switch remounts the tree (instant, no restart) */}
            <Stack screenOptions={{ headerShown: false }} key={`${resolved}-${language}`}>
              <Stack.Screen name="(tabs)" />
              <Stack.Screen name="onboarding" />
              <Stack.Screen name="send" options={{ presentation: 'modal' }} />
              <Stack.Screen name="receive" options={{ presentation: 'modal' }} />
              <Stack.Screen name="token-detail" options={{ presentation: 'modal' }} />
              <Stack.Screen name="add-token" options={{ presentation: 'modal' }} />
              <Stack.Screen name="assets" options={{ presentation: 'modal' }} />
              <Stack.Screen name="history" options={{ presentation: 'modal' }} />
              <Stack.Screen name="about" options={{ presentation: 'modal' }} />
              {__DEV__ && <Stack.Screen name="parallel" />}
            </Stack>
            <SigningRequestModal />
            {/* Self-gating (renders null unless parallel mode is armed). Must NOT be
                behind __DEV__: a production build can still enter the parallel space
                via `dev_unlocked`, and the fixture wallet must never be mistakable
                for the real one — its keys are public. */}
            <ParallelSpaceBadge />
          </ThemeProvider>
        </DAppConnectionProvider>
      </WalletProvider>
    </GestureHandlerRootView>
  );
}

export default function RootLayout() {
  const [ready, setReady] = useState(false);
  const [fontsLoaded, fontError] = useFonts({
    PlusJakartaSans_400Regular,
    PlusJakartaSans_500Medium,
    PlusJakartaSans_600SemiBold,
    PlusJakartaSans_700Bold,
  });
  // A wallet must always boot. Font loading can fail or hang indefinitely —
  // e.g. a web host that serves HTML instead of the .ttf files, or a flaky
  // network — and useFonts() never resolves. Release the gate after a short
  // grace period so we fall back to the system font instead of spinning forever.
  const [fontTimedOut, setFontTimedOut] = useState(false);
  useEffect(() => {
    const t = setTimeout(() => setFontTimedOut(true), 3000);
    return () => clearTimeout(t);
  }, []);
  useEffect(() => {
    if (fontError) console.warn('[fonts] load failed, using system fallback:', fontError);
  }, [fontError]);

  useEffect(() => {
    if (__DEV__) { installFaultConsole(); installMetricsConsole(); installParallelConsole(); }
    Promise.all([
      // Parallel space: re-arm the fixed-key signer before the wallet mounts so a
      // reload inside the test environment stays in it (and keeps the badge on).
      // Runs in prod too — a production build can be in parallel mode via
      // `dev_unlocked`, and booting with fixture accounts in storage but no armed
      // signer/badge would present the fixture wallet as the real one. In real
      // space this is a single AsyncStorage read.
      applyParallelSpaceOnBoot(),
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
      // Warm config caches so saved fiat endpoint + format prefs apply at launch.
      loadServiceEndpoints(),
      // Warm provider-key cache so the RPC pool can read it synchronously.
      loadRpcProviders(),
      loadLocalePrefs(),
      // Avatar style must be in the module cache before the first Home render.
      loadAvatarStyle(),
      // Apply the stored UI language before the first render.
      loadLanguage(),
    ])
      .then(() => setReady(true))
      .catch((e) => {
        // A failed init task (storage/config/network) must not strand the
        // user on the splash. Boot with defaults and let screens recover.
        console.warn('[boot] init task failed, continuing with defaults:', e);
        setReady(true);
      });

    hasPendingUploads()
      .then((has) => {
        if (has) {
          retryPendingUploads().catch(() => {});
        }
      })
      .catch(() => {});
  }, []);

  // Fonts are non-blocking: proceed once loaded, failed, or past the grace period.
  const fontsSettled = fontsLoaded || !!fontError || fontTimedOut;
  if (!ready || !fontsSettled) {
    return (
      <View style={styles.loading}>
        <ActivityIndicator size="small" color={color.accent.base} />
      </View>
    );
  }

  return (
    <ErrorBoundary>
      <LanguageProvider>
        <TextScaleProvider>
          <ColorSchemeProvider>
            <AlertProvider>
              <AppShell />
            </AlertProvider>
          </ColorSchemeProvider>
        </TextScaleProvider>
      </LanguageProvider>
    </ErrorBoundary>
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
