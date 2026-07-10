/**
 * <AccountFileWriter/> — headless. Keeps the Safari extension's account cache
 * (vela.ext.account.json) in sync with the active account so the extension can
 * answer connect/read/state in-Safari with zero app hop.
 *
 * Mounted inside WalletProvider (src/app/_layout.tsx, next to <SigningRequestModal/>).
 * Renders nothing. No-op off iOS (the sync service guards via isSupportedSync).
 *
 * Writes on (a) any account change and (b) every foreground — §12.1.6: a user
 * who installed the extension while already logged in would otherwise have an
 * empty cache until their next in-app account switch.
 *
 * IMPORTANT lifecycle guards:
 *  - Never treat the initial LOADING window as "logged out". WalletProvider boots
 *    with { isLoading:true, hasWallet:false } and only later restores accounts
 *    from AsyncStorage; clearing on loading would delete a logged-in user's cache
 *    (and a slow/failed restore would delete it permanently). We only clear once
 *    loading has resolved AND there is genuinely no wallet.
 *  - chainId is a STABLE default (not the volatile dApp-bridge chainId). The
 *    wallet has no global "current network"; each connected dApp picks/switches
 *    its own chain in the extension (per-origin). So this writer never depends on
 *    useDAppConnection().
 */
import { useEffect, useRef } from 'react';
import { AppState } from 'react-native';
import * as Linking from 'expo-linking';
import { useWallet } from '@/models/wallet-state';
import { useColorSchemePreference } from '@/constants/color-scheme';
import { useLanguagePreference } from '@/i18n/language';
import {
  writeAccountCache,
  clearAccountCache,
  markUniversalLinkVerified,
  DEFAULT_EXT_CHAIN_ID,
} from '@/services/app-group-account-sync';
import { requestExtensionSign } from '@/services/extension-sign-bus';

// The UL attestation probe rid (getvela.app/sign?rid=ul-selftest) — not a real sign.
const UL_SELFTEST_RID = 'ul-selftest';

// A getvela.app /sign Universal Link resolving to the app PROVES the applinks
// association is live on this device for THE EXACT PATH the extension launches
// (`/sign`) — the one signal that lets it safely switch the sign hand-off from the
// velawallet:// scheme to the UL. Scoped to /sign (not any getvela.app path) so
// attestation proves precisely what the launch relies on; anchored to the exact
// apex host so evil-getvela.app.com / getvela.app.evil.com / a path containing the
// string can't spoof it.
const GETVELA_SIGN_UL = /^https:\/\/getvela\.app\/sign(?:[/?#]|$)/i;

export function AccountFileWriter(): null {
  const { state, activeAccount } = useWallet();
  // The app's theme preference + resolved language ride the cache so the extension
  // UI matches the app exactly. Re-writing on change keeps them fresh even when the
  // user flips theme/language without touching accounts (this writer isn't inside
  // the Stack that remounts on those changes).
  const { preference: theme } = useColorSchemePreference();
  const { resolved: locale } = useLanguagePreference();

  const isLoading = state.isLoading;
  const hasWallet = state.hasWallet;
  const address = activeAccount?.address ?? '';
  const name = activeAccount?.name ?? '';
  const accounts = state.accounts;

  // Latest snapshot for the (registered-once) foreground handler.
  const latest = useRef({ isLoading, hasWallet, address, name, accounts, theme, locale });
  latest.current = { isLoading, hasWallet, address, name, accounts, theme, locale };

  function sync(d: typeof latest.current) {
    if (d.isLoading) return; // still restoring — neither write nor clear
    if (!d.hasWallet || !d.address) {
      void clearAccountCache(); // genuinely logged out → empty-state in the extension
      return;
    }
    void writeAccountCache({
      address: d.address,
      name: d.name,
      accounts: d.accounts.map((a) => ({ name: a.name, address: a.address })),
      chainId: DEFAULT_EXT_CHAIN_ID,
      theme: d.theme,
      locale: d.locale,
    });
  }

  // (a) write whenever loading resolves, the account set / active account changes,
  //     OR the theme/language preference changes (so the cache stays app-matched).
  useEffect(() => {
    sync(latest.current);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isLoading, hasWallet, address, name, accounts, theme, locale]);

  // (b) write on every foreground.
  useEffect(() => {
    const subscription = AppState.addEventListener('change', (s) => {
      if (s === 'active') sync(latest.current);
    });
    return () => subscription.remove();
  }, []);

  // (c) Universal-Link sign + attestation: if the app was opened (cold or warm) via a
  // https://getvela.app/sign?rid UL, (1) drive the extension sign for that rid, and
  // (2) mark UL verified. CRITICAL: expo-router maps only the velawallet:// scheme to
  // the /sign route — the getvela.app DOMAIN is NOT a router prefix, so a UL launch
  // would otherwise open the app to HOME and never show the sign sheet. This handler
  // is the ONLY thing that routes a UL sign into the flow (via the same bus the
  // /sign trampoline uses for the scheme path). ul-selftest is the attestation probe,
  // not a real sign — skip driving a sign for it.
  useEffect(() => {
    let mounted = true;
    const onUrl = async (url: string | null) => {
      if (!url || !GETVELA_SIGN_UL.test(url)) return;
      let rid: string | null = null;
      try { rid = new URL(url).searchParams.get('rid'); } catch { /* no rid */ }
      if (rid && rid !== UL_SELFTEST_RID) requestExtensionSign(rid); // buffered if the controller isn't up yet
      await markUniversalLinkVerified(); // fresh timestamp — also refreshes the TTL
      if (mounted) sync(latest.current); // re-write; writeAccountCache re-reads the flag
    };
    Linking.getInitialURL().then(onUrl).catch(() => {});
    const sub = Linking.addEventListener('url', ({ url }) => onUrl(url));
    return () => {
      mounted = false;
      sub.remove();
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return null;
}
