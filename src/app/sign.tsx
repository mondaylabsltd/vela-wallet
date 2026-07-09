// src/app/sign.tsx — inbound target of velawallet://sign?rid=<uuid> from the Safari
// extension. A TRAMPOLINE, not a screen: it hands the rid to the always-mounted root
// <ExtensionSignController> (extension-sign-bus) and immediately returns to WHEREVER
// the user was (router.back), so the ENTIRE sign flow (the SigningRequestModal sheet +
// the result confirmation) renders as OVERLAYS over the CURRENT screen — whatever it
// is — never a standalone page you get stranded on. It shows no UI of its own beyond a
// one-frame app-background flash.
import { useEffect } from 'react';
import { View, StyleSheet } from 'react-native';
import { useLocalSearchParams, useRouter } from 'expo-router';
import { color } from '@/constants/theme';
import { requestExtensionSign } from '@/services/extension-sign-bus';

export default function SignTrampoline(): React.ReactElement {
  const { rid } = useLocalSearchParams<{ rid?: string }>();
  const router = useRouter();

  useEffect(() => {
    if (rid) requestExtensionSign(String(rid));
    // Leave this route so we're on the wallet, not a dead-end screen. The controller
    // (mounted at the root) drives the sign over the wallet from here.
    if (router.canGoBack()) router.back();
    else router.replace('/');
  }, [rid, router]);

  return <View style={styles.bg} />;
}

const styles = StyleSheet.create({
  bg: { flex: 1, backgroundColor: color.bg.base },
});
