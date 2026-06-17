import { useEffect, useState } from 'react';
import { Redirect } from 'expo-router';
import AsyncStorage from '@react-native-async-storage/async-storage';

import ClearSigningTestScreen from '@/screens/settings/ClearSigningTestScreen';

/**
 * Clear Signing preview route.
 *
 * Reachable whenever the hidden developer mode is unlocked — the same
 * `dev_unlocked` flag that reveals the Settings row — so it works in production
 * web/native builds too, not just `__DEV__`. It used to be gated on `__DEV__`
 * alone, so after unlocking, the Settings row appeared but opening the page
 * bounced straight to /wallet in any non-dev build.
 */
export default function ClearSigningTestRoute() {
  // `__DEV__` is always allowed; otherwise wait for the async flag read so we
  // don't flash a redirect before we know whether dev mode is unlocked.
  const [access, setAccess] = useState<'checking' | 'allow' | 'deny'>(__DEV__ ? 'allow' : 'checking');

  useEffect(() => {
    if (access !== 'checking') return;
    AsyncStorage.getItem('dev_unlocked')
      .then((v) => setAccess(v === '1' ? 'allow' : 'deny'))
      .catch(() => setAccess('deny'));
  }, [access]);

  if (access === 'checking') return null;
  if (access === 'deny') return <Redirect href="/(tabs)/wallet" />;
  return <ClearSigningTestScreen />;
}
