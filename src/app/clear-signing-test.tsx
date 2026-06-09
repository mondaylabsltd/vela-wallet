import { Redirect } from 'expo-router';

import ClearSigningTestScreen from '@/screens/settings/ClearSigningTestScreen';

function DevOnlyGuard() {
  return <Redirect href="/(tabs)/wallet" />;
}

export default __DEV__ ? ClearSigningTestScreen : DevOnlyGuard;
