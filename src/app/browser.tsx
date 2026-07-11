// src/app/browser.tsx — the in-app dApp browser (M2–M4).
//
// A NON-modal full-screen route (like sign.tsx) so the root-level global
// <SigningRequestModal> renders ABOVE it. It hosts the native WalletWebView,
// creates a WebViewTransport, and:
//   • intercepts CONNECT/state methods locally (per-origin consent + grant store —
//     the signing brain has no consent gate), and
//   • FORWARDS read-only RPC / chain switch / all signing to the transport, which
//     the transient beginExtensionSign() install feeds into the existing pipeline
//     → SigningRequestModal (clear-signing, asset-sim, gas/funding, passkey, 4337).
//
// Security (ARCHITECTURE.md §5): origin is native-stamped (WalletWebView), signing
// is main-frame-only (WebViewTransport rejects iframes 4100), and on every document
// load we SETTLE in-flight requests with 4900 (never 4001) and tear down a stale
// signing modal owned by this tab — so a page can't swap context under an open
// prompt. Connect/state methods answered locally here are ALSO gated on isMainFrame
// (a cross-origin iframe must not be able to request accounts), signing is blocked
// on insecure public http origins, and the url param is re-validated as http(s).
// NOTE: native views not yet device-compiled.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ActivityIndicator, BackHandler, Image, Platform, Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import { useTranslation } from 'react-i18next';
import { ArrowLeft, ExternalLink, Lock, Plug, RotateCw, TriangleAlert, X } from 'lucide-react-native';

import { useWallet } from '@/models/wallet-state';
import { useDAppConnection } from '@/models/dapp-connection';
import {
  WalletWebView,
  isWalletWebViewSupported,
  type NavigationChangeEvent,
  type ProviderRequestEvent,
  type WalletWebViewHandle,
} from '@/modules/webview';
import { WebViewTransport, type WalletWebViewBridge } from '@/services/webview-transport';
import { coerceBrowserUrl } from '@/services/dapp-transport';
import { recordBrowserVisit } from '@/services/browser-history';
import { buildConnectionRecord } from '@/services/dapp-history';
import { saveTransaction } from '@/services/storage';
import { getGrant, resolveGranted, revokeGrant, setGrant, shouldDropGrant, type DAppGrant } from '@/services/dapp-permissions';
import { decideBrowserRequest } from '@/services/wallet-browser-router';
import { openBrowser, showAlert } from '@/services/platform';
import { color, space, text as textScale, createStyles } from '@/constants/theme';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';

/** One connect prompt, coalescing duplicate requests from the same origin. */
interface ConsentRequest {
  origin: string;
  requests: Array<{ requestId: string; method: string }>;
}

const NAV_SETTLE_ERROR = { code: 4900, message: 'Navigated away — check Vela Activity' } as const;

function hostOf(url: string): string {
  try {
    return new URL(url).host || url;
  } catch {
    return url;
  }
}
function originOf(url: string): string {
  try {
    const u = new URL(url);
    return `${u.protocol}//${u.host}`;
  } catch {
    return '';
  }
}
function shortAddr(a: string): string {
  return `${a.slice(0, 6)}…${a.slice(-4)}`;
}
/** `chainId` number → EIP-1193 hex string (`1` → `"0x1"`). */
function hexChainId(chainId: number): string {
  return '0x' + Math.max(0, Math.floor(chainId)).toString(16);
}

export default function BrowserScreen() {
  const { t } = useTranslation();
  const params = useLocalSearchParams<{ url?: string }>();
  // Re-validate the route param as http(s) — /browser is reachable via the
  // velawallet:// deep link, so never hand an arbitrary scheme (file:/javascript:)
  // straight to the native WebView.
  const initialUrl = coerceBrowserUrl(typeof params.url === 'string' ? params.url : '') ?? '';
  const router = useRouter();
  const { activeAccount, state } = useWallet();
  const { beginExtensionSign, chainId, incomingRequest, isSubmitting, rejectRequest } = useDAppConnection();

  const webRef = useRef<WalletWebViewHandle>(null);
  const originRef = useRef<string>('');

  // The bridge is owned HERE so local (connect/state) responses bypass the
  // transport's pending-id gate; only forwarded requests go through the transport.
  const bridge: WalletWebViewBridge = useMemo(
    () => ({
      respond: (id, result, error) => webRef.current?.respond(id, result, error),
      emitEvent: (event, data) => webRef.current?.emitEvent(event, data),
    }),
    [],
  );
  const transport = useMemo(
    () => new WebViewTransport(bridge, initialUrl ? { name: hostOf(initialUrl), url: initialUrl } : null),
    [bridge, initialUrl],
  );

  const [nav, setNav] = useState<NavigationChangeEvent | null>(null);
  const [consent, setConsent] = useState<ConsentRequest | null>(null);
  const [connectedAddr, setConnectedAddr] = useState<string | null>(null);
  // Reset when the favicon URL changes so a broken icon on page A doesn't hide the
  // (valid) icon on page B.
  const [faviconBroken, setFaviconBroken] = useState(false);
  useEffect(() => { setFaviconBroken(false); }, [nav?.favicon]);

  // consentRef is the SYNCHRONOUS source of truth (updated on the same tick as the
  // state) so back-to-back connect requests and onNavigationChange never race a
  // stale value. setConsentBoth keeps state + ref in lockstep.
  const consentRef = useRef<ConsentRequest | null>(null);
  const setConsentBoth = useCallback((next: ConsentRequest | null) => {
    consentRef.current = next;
    setConsent(next);
  }, []);

  const ready = !state.isLoading && state.hasWallet && !!activeAccount;

  // Install the transport into the transient sign slot (never clobbers a live
  // relay session) so forwarded requests render in the global SigningRequestModal.
  useEffect(() => {
    if (!ready) return;
    beginExtensionSign(transport);
    transport.connect();
    return () => transport.disconnect();
  }, [ready, transport, beginExtensionSign]);

  // Android hardware back: navigate the WEB history first (like every in-app
  // browser); only fall through to closing the route when the page can't go back.
  useEffect(() => {
    if (Platform.OS !== 'android') return;
    const sub = BackHandler.addEventListener('hardwareBackPress', () => {
      if (nav?.canGoBack) {
        webRef.current?.goBack();
        return true;
      }
      return false;
    });
    return () => sub.remove();
  }, [nav?.canGoBack]);

  // Keep the transport's chain current so a forwarded signing request takes the
  // per-request-chain path (F4): the sign sheet / SIWE shows THIS dApp (not a
  // concurrent relay session's), and a browser tx never mutates the global chain.
  useEffect(() => {
    transport.requestChainId = chainId;
  }, [transport, chainId]);

  // Live channel: forward global chain changes (incl. a granted wallet_switchEthereumChain)
  // to the connected page as `chainChanged`. Only when connected and only on an
  // actual change — never emit the address here (no accountsChanged leak).
  const prevChainRef = useRef(chainId);
  useEffect(() => {
    if (prevChainRef.current !== chainId && connectedAddr) {
      bridge.emitEvent('chainChanged', hexChainId(chainId));
    }
    prevChainRef.current = chainId;
  }, [chainId, connectedAddr, bridge]);

  // ALL wallet addresses (not just the active one). A grant is pinned to the address
  // it was made for, so resolveGranted and shouldDropGrant must both judge against the
  // full set — otherwise a grant to a non-active account reads as disconnected while
  // still being kept, re-prompting every request. null while loading → cold-load safe.
  const walletAddresses = useMemo(() => state.accounts?.map((a) => a.address) ?? null, [state.accounts]);

  const refreshGrant = useCallback(
    async (origin: string) => {
      const grant = await getGrant(origin);
      setConnectedAddr(resolveGranted(grant, walletAddresses)[0] ?? null);
    },
    [walletAddresses],
  );

  const onProviderRequest = useCallback(
    async (req: ProviderRequestEvent) => {
      // §5.2 — a cross-origin iframe never sees accounts or triggers connect: it gets
      // a disconnected view (granted = []), so the pure decision rejects/limits it.
      const grant = req.isMainFrame ? await getGrant(req.origin) : null;
      // Physically clean up a grant whose account was DELETED from the wallet.
      if (grant && shouldDropGrant(grant, walletAddresses)) {
        void revokeGrant(req.origin);
      }
      const granted = req.isMainFrame ? resolveGranted(grant, walletAddresses) : [];
      const decision = decideBrowserRequest({
        method: req.method,
        origin: req.origin,
        isMainFrame: req.isMainFrame,
        granted,
        hasActiveAccount: !!activeAccount,
        pendingConsentOrigin: consentRef.current?.origin ?? null,
      });
      const entry = { requestId: req.requestId, method: req.method };

      switch (decision.kind) {
        case 'respond':
          bridge.respond(req.requestId, decision.result, null);
          return;
        case 'reject':
          bridge.respond(req.requestId, undefined, { code: decision.code, message: decision.message });
          return;
        case 'open-consent':
          setConsentBoth({ origin: req.origin, requests: [entry] });
          return;
        case 'merge-consent':
          setConsentBoth({ origin: consentRef.current!.origin, requests: [...consentRef.current!.requests, entry] });
          return;
        case 'forward':
          transport.handleProviderRequest(req.requestId, req.method, req.params, req.origin, req.isMainFrame);
          return;
      }
    },
    [transport, bridge, activeAccount, walletAddresses, setConsentBoth],
  );

  const approveConsent = useCallback(async () => {
    const c = consentRef.current;
    if (!c || !activeAccount) return;
    const grant: DAppGrant = {
      origin: c.origin,
      address: activeAccount.address,
      chainId,
      grantedAt: Date.now(),
    };
    await setGrant(grant);
    // Log a "Connected to <app>" audit row so the in-app browser leaves a session
    // trail (previously it wrote only a silent grant). The consent sheet only opens
    // for a not-yet-granted origin, so this fires once per connection, not on revisit.
    void saveTransaction(
      buildConnectionRecord({ from: activeAccount.address, chainId, dappOrigin: hostOf(c.origin), nowMs: Date.now() }),
    ).catch((e) => console.warn('[Browser] Failed to save connect record:', e));
    // A navigation during the await already rejected+cleared this consent (nav
    // guard) — don't respond or push accountsChanged to a document that has since
    // navigated to a different origin.
    if (consentRef.current !== c) return;
    // Answer every coalesced request with its method-appropriate result.
    for (const r of c.requests) {
      const result =
        r.method === 'wallet_requestPermissions'
          ? [{ parentCapability: 'eth_accounts' }]
          : [activeAccount.address];
      bridge.respond(r.requestId, result, null);
    }
    // Announce connection state to the page (live channel — unlike the extension).
    transport.pushWalletInfo({
      address: activeAccount.address,
      chainId,
      name: activeAccount.name,
      accounts: [{ name: activeAccount.name, address: activeAccount.address }],
    });
    setConnectedAddr(activeAccount.address);
    setConsentBoth(null);
  }, [activeAccount, chainId, transport, bridge, setConsentBoth]);

  const rejectConsent = useCallback(() => {
    const c = consentRef.current;
    if (c) {
      for (const r of c.requests) {
        bridge.respond(r.requestId, undefined, { code: 4001, message: 'User rejected the request' });
      }
    }
    setConsentBoth(null);
  }, [bridge, setConsentBoth]);

  const disconnectOrigin = useCallback(async () => {
    const origin = originRef.current || originOf(initialUrl);
    if (origin) await revokeGrant(origin);
    bridge.emitEvent('accountsChanged', []);
    bridge.emitEvent('disconnect', {});
    setConnectedAddr(null);
  }, [bridge, initialUrl]);

  // Confirm before disconnecting — the chip reads as a status badge, so a bare tap
  // shouldn't silently revoke access (the dApp would then need a fresh consent).
  const confirmDisconnect = useCallback(() => {
    showAlert(t('connect.browser.disconnectTitle'), t('connect.browser.disconnectBody'), [
      { text: t('connect.browser.cancel'), style: 'cancel' },
      { text: t('connect.browser.disconnect'), style: 'destructive', onPress: () => void disconnectOrigin() },
    ]);
  }, [t, disconnectOrigin]);

  const onNavigationChange = useCallback(
    (n: NavigationChangeEvent) => {
      setNav(n);
      // Pass the page's real captured favicon so the signing sheet shows the actual
      // site logo (not a letter). Empty until it resolves → the sheet then derives
      // /favicon.ico from the host, then falls back to a monogram.
      if (n.url) transport.setDAppInfo({ name: n.title || hostOf(n.url), url: n.url, icon: n.favicon || undefined });

      // Record the visit once the page has settled (deduped by origin in the store).
      // Fires on load-finish and again when the favicon resolves, so the entry ends
      // up with the best title + favicon we saw.
      if (n.url && !n.loading && !n.error) {
        void recordBrowserVisit({ url: n.url, title: n.title, favicon: n.favicon }, Date.now());
      }

      // A fresh document load (reload or cross-origin) starts. SPA pushState does
      // NOT set loading, so same-page route changes keep their pending state.
      if (n.loading) {
        // 1. page-side: settle any in-flight promise with 4900 (never 4001).
        transport.settlePending(NAV_SETTLE_ERROR);
        // 2. reject + clear a pending connect sheet — otherwise approving it after
        //    the page navigated would leak accountsChanged into the NEW origin's
        //    document and persist a grant for the OLD origin.
        if (consentRef.current) {
          for (const r of consentRef.current.requests) {
            bridge.respond(r.requestId, undefined, NAV_SETTLE_ERROR);
          }
          setConsentBoth(null);
        }
        // 3. tear down a stale signing modal owned by THIS transport (unless the
        //    tx is already committed — persist-at-submit records the outcome).
        if (
          incomingRequest &&
          (incomingRequest as { __transport?: unknown }).__transport === transport &&
          !isSubmitting
        ) {
          rejectRequest(); // its 4001 is dropped by the transport (id already settled)
        }
        // 4. reset per-origin connection view when the origin actually changes.
        const newOrigin = originOf(n.url);
        if (newOrigin && newOrigin !== originRef.current) {
          originRef.current = newOrigin;
          void refreshGrant(newOrigin);
        }
      }
    },
    [transport, bridge, incomingRequest, isSubmitting, rejectRequest, refreshGrant, setConsentBoth],
  );

  // --- render ---------------------------------------------------------------

  if (!isWalletWebViewSupported) {
    return <Fallback>{t('connect.browser.unsupported')}</Fallback>;
  }
  if (!initialUrl) {
    return <Fallback>{t('connect.browser.noUrl')}</Fallback>;
  }

  const host = hostOf(nav?.url ?? initialUrl);
  const secure = (nav?.url ?? initialUrl).startsWith('https://');
  const loadError = nav?.error ?? '';
  const showFavicon = secure && !!nav?.favicon && !faviconBroken;

  return (
    <SafeAreaView style={styles.root} edges={['top', 'bottom']}>
      <Stack.Screen options={{ headerShown: false }} />

      {/* Top bar: site favicon / security indicator + host/title + connection chip + close */}
      <View style={styles.topBar}>
        <View
          accessibilityRole="image"
          accessibilityLabel={secure ? t('connect.browser.a11ySecure') : t('connect.browser.a11yInsecure')}
        >
          {!secure ? (
            <TriangleAlert size={14} color={color.warning.base} />
          ) : showFavicon ? (
            <Image
              source={{ uri: nav!.favicon }}
              style={styles.favicon}
              onError={() => setFaviconBroken(true)}
            />
          ) : (
            <Lock size={14} color={color.fg.muted} />
          )}
        </View>
        <View style={styles.hostWrap}>
          <Text style={styles.host} numberOfLines={1}>{host}</Text>
          {nav?.title ? <Text style={styles.title} numberOfLines={1}>{nav.title}</Text> : null}
        </View>
        {connectedAddr ? (
          <Pressable
            hitSlop={8}
            onPress={confirmDisconnect}
            style={styles.chip}
            accessibilityRole="button"
            accessibilityLabel={t('connect.browser.a11yDisconnect')}
          >
            <View style={styles.chipDot} />
            <Text style={styles.chipText}>{shortAddr(connectedAddr)}</Text>
          </Pressable>
        ) : null}
        <Pressable
          hitSlop={10}
          onPress={() => router.back()}
          style={styles.iconBtn}
          accessibilityRole="button"
          accessibilityLabel={t('connect.browser.a11yClose')}
        >
          <X size={20} color={color.fg.base} />
        </Pressable>
      </View>

      {nav?.loading ? <View style={styles.loadingBar} /> : null}

      {ready ? (
        <View style={styles.webWrap}>
          <WalletWebView
            ref={webRef}
            uri={initialUrl}
            style={styles.web}
            onProviderRequest={onProviderRequest}
            onNavigationChange={onNavigationChange}
          />
          {loadError ? (
            <View style={styles.errorOverlay}>
              <TriangleAlert size={28} color={color.fg.subtle} />
              <Text style={styles.errorTitle}>{t('connect.browser.loadFailed')}</Text>
              <Text style={styles.errorBody} numberOfLines={2}>{loadError}</Text>
              <VelaButton
                title={t('connect.browser.retry')}
                variant="secondary"
                onPress={() => webRef.current?.reload()}
                style={styles.retryBtn}
              />
            </View>
          ) : null}
        </View>
      ) : state.isLoading ? (
        <View style={styles.center}>
          <ActivityIndicator color={color.accent.base} />
          <Text style={styles.dim}>{t('connect.browser.preparing')}</Text>
        </View>
      ) : (
        // Wallet finished loading but there's none yet (e.g. reached via deep link
        // before onboarding) — don't spin forever pretending to load.
        <View style={styles.center}>
          <Plug size={26} color={color.fg.subtle} strokeWidth={2} />
          <Text style={styles.dim}>{t('connect.list.noWallet')}</Text>
        </View>
      )}

      {/* Bottom bar: back · reload · open in system browser */}
      <View style={styles.bottomBar}>
        <Pressable
          hitSlop={8}
          disabled={!nav?.canGoBack}
          onPress={() => webRef.current?.goBack()}
          style={styles.iconBtn}
          accessibilityRole="button"
          accessibilityLabel={t('connect.browser.a11yBack')}
        >
          <ArrowLeft size={22} color={nav?.canGoBack ? color.fg.base : color.fg.subtle} />
        </Pressable>
        <Pressable
          hitSlop={8}
          onPress={() => webRef.current?.reload()}
          style={styles.iconBtn}
          accessibilityRole="button"
          accessibilityLabel={t('connect.browser.a11yReload')}
        >
          <RotateCw size={20} color={color.fg.muted} />
        </Pressable>
        <View style={styles.flex} />
        <Pressable
          hitSlop={8}
          onPress={() => openBrowser(nav?.url ?? initialUrl)}
          style={styles.iconBtn}
          accessibilityRole="button"
          accessibilityLabel={t('connect.browser.a11yOpenSystem')}
        >
          <ExternalLink size={20} color={color.fg.muted} />
        </Pressable>
      </View>

      {/* Connect-consent sheet — content-height: a full pageSheet towers over the
          page for four rows of content (and on Android used to cover it entirely). */}
      <AppModal visible={!!consent} onClose={rejectConsent} fit>
        <View style={styles.sheet}>
          <Text style={styles.sheetTitle}>
            {t('connect.browser.title', { host: consent ? hostOf(consent.origin) : '' })}
          </Text>
          <Text style={styles.sheetBody}>{t('connect.browser.body')}</Text>
          {activeAccount ? (
            <View style={styles.acctRow}>
              <Text style={styles.acctName}>{activeAccount.name}</Text>
              <Text style={styles.acctAddr}>{shortAddr(activeAccount.address)}</Text>
            </View>
          ) : null}
          <View style={styles.sheetActions}>
            <VelaButton title={t('connect.browser.cancel')} variant="secondary" onPress={rejectConsent} style={styles.sheetBtn} />
            <VelaButton title={t('connect.browser.connect')} variant="accent" onPress={approveConsent} style={styles.sheetBtn} />
          </View>
        </View>
      </AppModal>
    </SafeAreaView>
  );
}

function Fallback({ children }: { children: string }) {
  return (
    <SafeAreaView style={[styles.root, styles.center]}>
      <Stack.Screen options={{ headerShown: false }} />
      <Text style={styles.dim}>{children}</Text>
    </SafeAreaView>
  );
}

const styles = createStyles(() => ({
  root: { flex: 1, backgroundColor: color.bg.base },
  webWrap: { flex: 1 },
  web: { flex: 1 },
  flex: { flex: 1 },
  center: { flex: 1, alignItems: 'center', justifyContent: 'center', gap: space.md },
  dim: { color: color.fg.muted, fontSize: textScale.sm },
  topBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.sm,
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderBottomWidth: StyleSheet.hairlineWidth,
    borderBottomColor: color.border.base,
  },
  favicon: { width: 16, height: 16, borderRadius: 4 },
  hostWrap: { flex: 1 },
  host: { color: color.fg.base, fontSize: textScale.sm, fontWeight: '600' },
  title: { color: color.fg.subtle, fontSize: 11 },
  iconBtn: { padding: space.xs },
  chip: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xs,
    paddingHorizontal: space.sm,
    paddingVertical: 4,
    borderRadius: 999,
    backgroundColor: color.accent.soft,
  },
  chipDot: { width: 7, height: 7, borderRadius: 4, backgroundColor: color.success.base },
  chipText: { color: color.accent.base, fontSize: 12, fontWeight: '600', fontVariant: ['tabular-nums'] },
  loadingBar: { height: 2, backgroundColor: color.accent.base },
  errorOverlay: {
    ...StyleSheet.absoluteFillObject,
    backgroundColor: color.bg.base,
    alignItems: 'center',
    justifyContent: 'center',
    gap: space.sm,
    paddingHorizontal: space.xl,
  },
  errorTitle: { color: color.fg.base, fontSize: textScale.base, fontWeight: '600', marginTop: space.xs },
  errorBody: { color: color.fg.muted, fontSize: textScale.sm, textAlign: 'center' },
  retryBtn: { marginTop: space.md, minWidth: 140 },
  bottomBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xl,
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: color.border.base,
  },
  sheet: { gap: space.md, paddingHorizontal: space.xl, paddingTop: space.sm, paddingBottom: space.lg },
  sheetTitle: { color: color.fg.base, fontSize: textScale.lg, fontWeight: '700' },
  sheetBody: { color: color.fg.muted, fontSize: textScale.sm, lineHeight: 20 },
  acctRow: {
    flexDirection: 'row',
    justifyContent: 'space-between',
    paddingVertical: space.sm,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: color.border.base,
  },
  acctName: { color: color.fg.base, fontWeight: '600', fontSize: textScale.sm },
  acctAddr: { color: color.fg.muted, fontSize: textScale.sm, fontVariant: ['tabular-nums'] },
  sheetActions: { flexDirection: 'row', gap: space.md, marginTop: space.sm },
  sheetBtn: { flex: 1 },
}));
