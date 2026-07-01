/**
 * `/parallel` — the parallel-space entry.
 *
 * Not a page of its own: it waits for the fixture wallet to load (the layout enters
 * the mode), then drops straight into the REAL production home. Everything from here
 * is the real app, pixel-for-pixel, running in the parallel space. The only visible
 * addition anywhere is the small PARALLEL SPACE badge.
 */
import React from 'react';
import { View, ActivityIndicator, StyleSheet } from 'react-native';
import { Redirect } from 'expo-router';
import { useWallet } from '@/models/wallet-state';
import { FIXTURE_ACCOUNT } from '@/services/dev/passkey-fixture';
import { color } from '@/constants/theme';

export default function ParallelEntry() {
  const { state } = useWallet();
  const ready = state.accounts.some((a) => a.id === FIXTURE_ACCOUNT.id);

  // Once the fixture wallet is live, hand off to the real production app.
  if (ready) return <Redirect href="/(tabs)/wallet" />;

  // Transient boot loader (mirrors the app's own splash) while the mode arms.
  return (
    <View style={styles.loading} testID="parallel-entry">
      <ActivityIndicator size="small" color={color.accent.base} />
    </View>
  );
}

const styles = StyleSheet.create({
  loading: { flex: 1, justifyContent: 'center', alignItems: 'center', backgroundColor: color.bg.base },
});
