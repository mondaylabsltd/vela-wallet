import { useEffect, useState } from 'react';
import { Redirect } from 'expo-router';
import AsyncStorage from '@react-native-async-storage/async-storage';

import ReceiptHarnessScreen from '@/screens/dev/ReceiptHarnessScreen';

/**
 * Receipt / batch-send harness route.
 *
 * Reachable under __DEV__ or once the hidden developer mode is unlocked
 * (`dev_unlocked`), same gate as /clear-signing-test — so it renders the real
 * receipt + detail-sheet components with mock props WITHOUT passkey/wallet/bundler.
 */
export default function ReceiptHarnessRoute() {
  const [access, setAccess] = useState<'checking' | 'allow' | 'deny'>(__DEV__ ? 'allow' : 'checking');

  useEffect(() => {
    if (access !== 'checking') return;
    AsyncStorage.getItem('dev_unlocked')
      .then((v) => setAccess(v === '1' ? 'allow' : 'deny'))
      .catch(() => setAccess('deny'));
  }, [access]);

  if (access === 'checking') return null;
  if (access === 'deny') return <Redirect href="/(tabs)/wallet" />;
  return <ReceiptHarnessScreen />;
}
