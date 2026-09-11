/**
 * Sign-in controller — WEB, driven by the portable Rust state machine.
 *
 * Owns no rules: resolution order (local → index → on-device recovery), the
 * recovery offer, the background index heal and the reachability probe all live
 * in `rust/crates/vela-core/src/app/login.rs`.
 */

import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { useRouter } from 'expo-router';
import { useTranslation } from 'react-i18next';

import { useWallet } from '@/models/wallet-state';
import { createLoginSession, type LoginSession } from '@/services/onboarding-core/session';
import type { Account } from '@/services/onboarding-core/generated/Account';
import type { CompletionMode } from '@/services/onboarding-core/generated/CompletionMode';
import type { LoginView } from '@/services/onboarding-core/generated/LoginView';
import type { StoredAccount } from '@/models/types';

import type {
  OnboardingLoginController,
  OnboardingLoginControllerOptions,
} from './onboarding-controller-types';

const INITIAL_VIEW: LoginView = { busy: false, endpoint_unreachable: false, transport_failed: false };

function toStored(account: Account): StoredAccount {
  return {
    id: account.id,
    name: account.name,
    address: account.address,
    publicKeyHex: account.public_key_hex,
    createdAt: account.created_at_iso,
    keys: (account.keys ?? []).map((key) => ({
      credentialId: key.credential_id,
      publicKeyHex: key.public_key_hex,
      name: key.name,
    })),
  };
}

export function useOnboardingLogin(
  options: OnboardingLoginControllerOptions = {},
): OnboardingLoginController {
  const { t } = useTranslation();
  const router = useRouter();
  const { dispatch } = useWallet();
  const [view, setView] = useState<LoginView>(INITIAL_VIEW);
  const session = useRef<LoginSession | null>(null);

  const latest = useRef({ t, dispatch, router, onComplete: options.onComplete });
  latest.current = { t, dispatch, router, onComplete: options.onComplete };

  const complete = useCallback((mode: CompletionMode) => {
    const { dispatch: dispatchWallet, router: currentRouter, onComplete } = latest.current;
    if (mode.type === 'set_wallet') {
      dispatchWallet({
        type: 'SET_WALLET',
        accounts: mode.accounts.map(toStored),
        activeIndex: mode.active_index,
      });
    } else {
      dispatchWallet({ type: 'ADD_ACCOUNT', account: toStored(mode.account) });
    }
    // Embedded onboarding (the dApp popup) resumes the request that opened it
    // rather than navigating away.
    if (onComplete) onComplete();
    else currentRouter.replace('/(tabs)/wallet');
  }, []);

  useEffect(() => {
    const loop = createLoginSession({
      onView: setView,
      deps: {
        t: (key, opts) => latest.current.t(key, opts) as string,
        complete,
      },
      onError: (error) => console.error('[onboarding-login] core fault:', error),
    });
    session.current = loop;
    // `start` also begins the index reachability probe.
    loop.start({ type: 'start' });

    return () => {
      loop.dispose();
      session.current = null;
    };
  }, [complete]);

  return useMemo<OnboardingLoginController>(
    () => ({
      busy: view.busy,
      endpointUnreachable: view.endpoint_unreachable,
      signIn: () => session.current?.dispatch({ type: 'sign_in', method: 'platform' }),
    }),
    [view],
  );
}
