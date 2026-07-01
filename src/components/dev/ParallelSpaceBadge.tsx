/**
 * PARALLEL SPACE badge.
 *
 * A persistent, unmissable marker shown app-wide whenever the parallel-space test
 * environment is active (fixed passkey, everything else real). It exists so the test
 * space and the real space can never be confused. Dev-only — the root layout renders
 * it behind `__DEV__`, and it returns null unless the parallel mode is on.
 *
 * Tapping it opens the parallel-space hub (`/parallel`).
 */
import React, { useEffect, useState } from 'react';
import { Pressable, Text, View, StyleSheet } from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';
import { useRouter } from 'expo-router';
import { FlaskConical } from 'lucide-react-native';

// Read the flag straight off globalThis (the single source of truth set by
// installMockPasskey → setActive). Reading the global — rather than a module export —
// keeps the badge correct even if Metro bundles the parallel-space service twice.
const isActive = () => !!(globalThis as any).__VELA_PARALLEL__;

export function ParallelSpaceBadge() {
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const [active, setActive] = useState(isActive);

  // Poll the global so the badge flips on the moment the mode arms, without coupling
  // to the service's listener registry (a 400ms tick is imperceptible for a dev badge).
  useEffect(() => {
    if (active) return;
    const id = setInterval(() => { if (isActive()) setActive(true); }, 400);
    return () => clearInterval(id);
  }, [active]);

  if (!active) return null;

  return (
    // box-none: the wrapper never eats touches; only the pill itself is pressable.
    <View style={[styles.wrap, { top: insets.top + 6 }]} pointerEvents="box-none">
      <Pressable
        testID="parallel-space-badge"
        accessibilityLabel="Parallel space active — test environment"
        onPress={() => router.navigate('/parallel')}
        style={styles.pill}
        hitSlop={8}
      >
        <FlaskConical size={12} color="#fff" strokeWidth={2.5} />
        <Text style={styles.text}>PARALLEL SPACE</Text>
        <Text style={styles.sub}>mock passkey · test</Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  wrap: {
    position: 'absolute',
    left: 0,
    right: 0,
    alignItems: 'center',
    zIndex: 9999,
    elevation: 9999,
  },
  pill: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 6,
    paddingHorizontal: 12,
    paddingVertical: 5,
    borderRadius: 999,
    backgroundColor: '#7c3aed', // violet — deliberately unlike any brand colour
    shadowColor: '#000',
    shadowOpacity: 0.25,
    shadowRadius: 8,
    shadowOffset: { width: 0, height: 2 },
  },
  text: {
    color: '#fff',
    fontSize: 11,
    fontWeight: '800',
    letterSpacing: 0.6,
  },
  sub: {
    color: 'rgba(255,255,255,0.8)',
    fontSize: 10,
    fontWeight: '600',
  },
});
