import type { StoredAccount } from '@/models/types';
import { useWallet } from '@/models/wallet-state';
import * as Passkey from '@/modules/passkey';
import { PasskeyError, PasskeyErrorCode } from '@/modules/passkey';
import { fromHex } from '@/services/hex';
import * as PublicKeyIndex from '@/services/public-key-index';
import { computeAddress } from '@/services/safe-address';
import { loadAccounts, saveAccount } from '@/services/storage';
import { useRouter, useLocalSearchParams } from 'expo-router';
import { showAlert } from '@/services/platform';
import { useTranslation } from 'react-i18next';
import React, { useState, useEffect, useRef } from 'react';
import { CreateWalletScreen } from './CreateWalletScreen';
import { WelcomeScreen, OnboardingSettingsModal } from './WelcomeScreen';
import { loadServiceEndpoints } from '@/services/storage';
import { DEFAULT_SERVICE_ENDPOINTS } from '@/models/types';

type Step = 'welcome' | 'create';

/** Check if a Passkey Index URL is reachable by hitting /api/health. */
async function isPasskeyIndexReachable(url: string): Promise<boolean> {
  const base = url.trim().replace(/\/$/, '');
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 8000);
  try {
    const res = await fetch(`${base}/api/health?_t=${Date.now()}`, {
      method: 'GET',
      signal: controller.signal,
    });
    clearTimeout(timeout);
    if (!res.ok) return false;
    const json = await res.json();
    return json.service === 'webauthn-p256-publickey-index' && json.status === 'ok';
  } catch {
    clearTimeout(timeout);
    return false;
  }
}

export default function OnboardingScreen() {
  const { t } = useTranslation();
  // Deep-link: /onboarding?mode=create jumps straight to the create form.
  // Any other value (or none, incl. ?mode=signin) stays on the welcome screen,
  // which has the "I already have a wallet" sign-in button.
  const { mode } = useLocalSearchParams<{ mode?: string }>();
  const [step, setStep] = useState<Step>(mode === 'create' ? 'create' : 'welcome');
  const [loginLoading, setLoginLoading] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [endpointUnreachable, setEndpointUnreachable] = useState(false);
  const healthCheckDone = useRef(false);
  const router = useRouter();
  const { dispatch } = useWallet();

  // Auto-detect Passkey Index reachability — retry 3 times before flagging
  useEffect(() => {
    if (healthCheckDone.current) return;
    healthCheckDone.current = true;

    (async () => {
      const endpoints = await loadServiceEndpoints();
      const url = endpoints.passkeyIndexURL || DEFAULT_SERVICE_ENDPOINTS.passkeyIndexURL;

      let failures = 0;
      for (let i = 0; i < 3; i++) {
        const ok = await isPasskeyIndexReachable(url);
        if (ok) return; // reachable — all good
        failures++;
        if (i < 2) await new Promise(r => setTimeout(r, 2000)); // wait 2s between retries
      }

      if (failures >= 3) {
        setEndpointUnreachable(true);
        setShowSettings(true);
      }
    })();
  }, []);

  async function handleLogin() {
    if (loginLoading) return;
    try {
      setLoginLoading(true);
      __DEV__ && console.log('[Login] Starting login...');

      const supported = await Passkey.isSupported();
      __DEV__ && console.log('[Login] Passkey supported:', supported);
      if (!supported) {
        showAlert(t('onboarding.login.alertNotSupportedTitle'), t('onboarding.login.alertNotSupportedBody'));
        return;
      }

      // 1. Authenticate with existing passkey
      __DEV__ && console.log('[Login] Calling authenticate()...');
      const assertion = await Passkey.authenticate();
      __DEV__ && console.log('[Login] credentialId:', assertion.credentialId);

      // 2. Verify passkey compatibility with Safe contracts
      const { verifySafeWebAuthn } = await import('@/services/webauthn-verify');
      const compat = verifySafeWebAuthn(assertion);
      __DEV__ && console.log('[Login] Safe compat:', compat.ok, compat.reason ?? '');
      if (!compat.ok) {
        showAlert(
          t('onboarding.login.alertIncompatibleTitle'),
          t('onboarding.login.alertIncompatibleBody'),
        );
        return;
      }

      // 3. Try local AsyncStorage first
      const localAccounts = await loadAccounts();
      __DEV__ && console.log('[Login] Local accounts:', localAccounts.length, localAccounts.map(a => ({ id: a.id.slice(0, 12), name: a.name })));
      const local = localAccounts.find(a => a.id === assertion.credentialId);

      if (local) {
        __DEV__ && console.log('[Login] Found locally:', local.name, local.address);
        dispatch({
          type: 'SET_WALLET',
          accounts: localAccounts,
          activeIndex: localAccounts.indexOf(local),
        });
        router.replace('/(tabs)/wallet');
        return;
      }
      __DEV__ && console.log('[Login] Not found locally');

      // 4. Try public key index server
      const rpId = Passkey.getRelyingPartyId();
      __DEV__ && console.log('[Login] Querying server: rpId=', rpId, 'credentialId=', assertion.credentialId);
      const record = await PublicKeyIndex.queryRecord(
        rpId,
        assertion.credentialId,
      );
      __DEV__ && console.log('[Login] Server returned:', record.name, 'publicKey:', record.publicKey.slice(0, 16) + '...');

      const address = computeAddress(record.publicKey);
      const userName = record.name || decodeUserNameFromAssertion(assertion.userIdHex);

      const account: StoredAccount = {
        id: assertion.credentialId,
        name: userName,
        address,
        publicKeyHex: record.publicKey,
        createdAt: new Date().toISOString(),
      };
      await saveAccount(account);
      dispatch({ type: 'ADD_ACCOUNT', account });
      router.replace('/(tabs)/wallet');

    } catch (error) {
      if (error instanceof PasskeyError && error.code === PasskeyErrorCode.CANCELLED) {
        // User cancelled — do nothing
      } else {
        const msg = error instanceof Error ? error.message : String(error);
        if (msg.includes('404')) {
          showAlert(
            t('onboarding.login.alertNotFoundTitle'),
            t('onboarding.login.alertNotFoundBody'),
          );
        } else if (msg.includes('Network request failed') || msg.includes('fetch') || msg.includes('Connection failed')) {
          setEndpointUnreachable(true);
          setShowSettings(true);
        } else {
          showAlert(
            t('onboarding.login.alertSignInFailedTitle'),
            t('onboarding.login.alertSignInFailedBody', { message: msg }),
          );
        }
      }
    } finally {
      setLoginLoading(false);
    }
  }

  const openSettings = () => setShowSettings(true);

  if (step === 'create') {
    return (
      <>
        <CreateWalletScreen
          onBack={() => setStep('welcome')}
          onCreated={() => router.replace('/(tabs)/wallet')}
          onOpenSettings={openSettings}
        />
        <OnboardingSettingsModal visible={showSettings} onClose={() => setShowSettings(false)} unreachable={endpointUnreachable} />
      </>
    );
  }

  return (
    <>
      <WelcomeScreen
        onCreateWallet={() => setStep('create')}
        onLogin={handleLogin}
        loginLoading={loginLoading}
        onOpenSettings={openSettings}
        autoShowSettings={endpointUnreachable}
      />
      <OnboardingSettingsModal visible={showSettings} onClose={() => setShowSettings(false)} unreachable={endpointUnreachable} />
    </>
  );
}

/** Decode username from the assertion's userIdHex field. */
function decodeUserNameFromAssertion(userIdHex?: string): string {
  if (!userIdHex) return 'Wallet';
  try {
    const bytes = fromHex(userIdHex);
    const str = String.fromCharCode(...bytes);
    return Passkey.decodeUserName(str) || 'Wallet';
  } catch {
    return 'Wallet';
  }
}
