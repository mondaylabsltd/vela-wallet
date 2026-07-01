/**
 * Parallel-space route group (`/parallel/*`).
 *
 * Entering this prefix switches the app into the parallel-space test environment:
 * it installs the fixed-key passkey signer and loads the fixture wallet into the live
 * wallet context. From there every screen is the REAL production screen, pixel-for-
 * pixel — the ONLY difference anywhere is that signing uses the fixture passkey. The
 * real wallet cache is backed up on entry and restored on exit
 * (see `services/dev/parallel-space.ts`). The app-wide PARALLEL SPACE badge marks the
 * mode. Dev-only: allowed in `__DEV__`, or once `dev_unlocked` is set.
 */
import { useEffect, useRef, useState } from 'react';
import { Stack, Redirect } from 'expo-router';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useWallet } from '@/models/wallet-state';
import { enterParallelSpace, fixtureAccounts } from '@/services/dev/parallel-space';

export default function ParallelLayout() {
  const { dispatch } = useWallet();
  const [access, setAccess] = useState<'checking' | 'allow' | 'deny'>(__DEV__ ? 'allow' : 'checking');
  const entered = useRef(false);

  useEffect(() => {
    if (access !== 'checking') return;
    AsyncStorage.getItem('dev_unlocked')
      .then((v) => setAccess(v === '1' ? 'allow' : 'deny'))
      .catch(() => setAccess('deny'));
  }, [access]);

  // Enter the parallel space and load the fixture wallet into the live context. Runs
  // exactly once for the whole `/parallel/*` group; deliberately NOT cancelled on
  // unmount so a redirect out of `/parallel` (into the real app) still completes it.
  useEffect(() => {
    if (access !== 'allow' || entered.current) return;
    entered.current = true;
    enterParallelSpace().then(() =>
      dispatch({ type: 'SET_WALLET', accounts: fixtureAccounts(), activeIndex: 0 }),
    );
  }, [access, dispatch]);

  if (access === 'checking') return null;
  if (access === 'deny') return <Redirect href="/(tabs)/wallet" />;

  return <Stack screenOptions={{ headerShown: false }} />;
}
