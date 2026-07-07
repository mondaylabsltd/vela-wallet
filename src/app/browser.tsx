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
// prompt. NOTE: native views not yet device-compiled.
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import { ActivityIndicator, Pressable, StyleSheet, Text, View } from 'react-native';
import { SafeAreaView } from 'react-native-safe-area-context';
import { Stack, useLocalSearchParams, useRouter } from 'expo-router';
import { ArrowLeft, ExternalLink, RotateCw, X } from 'lucide-react-native';

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
import { getGrant, resolveGranted, revokeGrant, setGrant, type DAppGrant } from '@/services/dapp-permissions';
import { classifyBrowserRequest } from '@/services/wallet-browser-router';
import { openBrowser } from '@/services/platform';
import { color, space, text as textScale } from '@/constants/theme';
import { AppModal } from '@/components/ui/AppModal';
import { VelaButton } from '@/components/ui/VelaButton';

interface ConsentRequest {
  requestId: string;
  method: string;
  origin: string;
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

export default function BrowserScreen() {
  const params = useLocalSearchParams<{ url?: string }>();
  const initialUrl = typeof params.url === 'string' ? params.url : '';
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

  const ready = !state.isLoading && state.hasWallet && !!activeAccount;

  // Install the transport into the transient sign slot (never clobbers a live
  // relay session) so forwarded requests render in the global SigningRequestModal.
  useEffect(() => {
    if (!ready) return;
    beginExtensionSign(transport);
    transport.connect();
    return () => transport.disconnect();
  }, [ready, transport, beginExtensionSign]);

  const refreshGrant = useCallback(
    async (origin: string) => {
      const grant = await getGrant(origin);
      const addresses = activeAccount ? [activeAccount.address] : [];
      setConnectedAddr(resolveGranted(grant, addresses)[0] ?? null);
    },
    [activeAccount],
  );

  const onProviderRequest = useCallback(
    async (req: ProviderRequestEvent) => {
      const grant = await getGrant(req.origin);
      const addresses = activeAccount ? [activeAccount.address] : [];
      const granted = resolveGranted(grant, addresses);
      const action = classifyBrowserRequest(req.method, granted);

      if (action.kind === 'respond') {
        bridge.respond(req.requestId, action.result, null);
        return;
      }
      if (action.kind === 'consent') {
        if (!activeAccount) {
          bridge.respond(req.requestId, undefined, { code: 4001, message: 'No account available' });
          return;
        }
        setConsent({ requestId: req.requestId, method: req.method, origin: req.origin });
        return;
      }
      // forward: read-only RPC / chain switch / signing → existing pipeline
      transport.handleProviderRequest(req.requestId, req.method, req.params, req.origin, req.isMainFrame);
    },
    [transport, bridge, activeAccount],
  );

  const approveConsent = useCallback(async () => {
    const c = consent;
    if (!c || !activeAccount) return;
    const grant: DAppGrant = {
      origin: c.origin,
      address: activeAccount.address,
      chainId,
      grantedAt: Date.now(),
    };
    await setGrant(grant);
    const result =
      c.method === 'wallet_requestPermissions'
        ? [{ parentCapability: 'eth_accounts' }]
        : [activeAccount.address];
    bridge.respond(c.requestId, result, null);
    // Announce connection state to the page (live channel — unlike the extension).
    transport.pushWalletInfo({
      address: activeAccount.address,
      chainId,
      name: activeAccount.name,
      accounts: [{ name: activeAccount.name, address: activeAccount.address }],
    });
    setConnectedAddr(activeAccount.address);
    setConsent(null);
  }, [consent, activeAccount, chainId, transport, bridge]);

  const rejectConsent = useCallback(() => {
    if (consent) bridge.respond(consent.requestId, undefined, { code: 4001, message: 'User rejected the request' });
    setConsent(null);
  }, [consent, bridge]);

  const disconnectOrigin = useCallback(async () => {
    const origin = originRef.current || originOf(initialUrl);
    if (origin) await revokeGrant(origin);
    bridge.emitEvent('accountsChanged', []);
    bridge.emitEvent('disconnect', {});
    setConnectedAddr(null);
  }, [bridge, initialUrl]);

  const onNavigationChange = useCallback(
    (n: NavigationChangeEvent) => {
      setNav(n);
      if (n.url) transport.setDAppInfo({ name: n.title || hostOf(n.url), url: n.url });

      // A fresh document load (reload or cross-origin) starts. SPA pushState does
      // NOT fire this, so same-page route changes keep their pending state.
      if (n.loading) {
        // 1. page-side: settle any in-flight promise with 4900 (never 4001).
        transport.settlePending(NAV_SETTLE_ERROR);
        // 2. tear down a stale signing modal owned by THIS transport (unless the
        //    tx is already committed — persist-at-submit records the outcome).
        if (
          incomingRequest &&
          (incomingRequest as { __transport?: unknown }).__transport === transport &&
          !isSubmitting
        ) {
          rejectRequest(); // its 4001 is dropped by the transport (id already settled)
        }
        // 3. reset per-origin connection view when the origin actually changes.
        const newOrigin = originOf(n.url);
        if (newOrigin && newOrigin !== originRef.current) {
          originRef.current = newOrigin;
          void refreshGrant(newOrigin);
        }
      }
    },
    [transport, incomingRequest, isSubmitting, rejectRequest, refreshGrant],
  );

  // --- render ---------------------------------------------------------------

  if (!isWalletWebViewSupported) {
    return <Fallback>The in-app dApp browser is only available on iOS and Android.</Fallback>;
  }
  if (!initialUrl) {
    return <Fallback>No URL to open.</Fallback>;
  }

  const host = hostOf(nav?.url ?? initialUrl);
  const secure = (nav?.url ?? initialUrl).startsWith('https://');

  return (
    <SafeAreaView style={styles.root} edges={['top', 'bottom']}>
      <Stack.Screen options={{ headerShown: false }} />

      {/* Top bar: security lock + host/title + connection chip + close */}
      <View style={styles.topBar}>
        <Text style={styles.lock}>{secure ? '🔒' : '⚠︎'}</Text>
        <View style={styles.hostWrap}>
          <Text style={styles.host} numberOfLines={1}>{host}</Text>
          {nav?.title ? <Text style={styles.title} numberOfLines={1}>{nav.title}</Text> : null}
        </View>
        {connectedAddr ? (
          <Pressable hitSlop={8} onPress={disconnectOrigin} style={styles.chip}>
            <View style={styles.chipDot} />
            <Text style={styles.chipText}>{shortAddr(connectedAddr)}</Text>
          </Pressable>
        ) : null}
        <Pressable hitSlop={10} onPress={() => router.back()} style={styles.iconBtn}>
          <X size={20} color={color.fg.base} />
        </Pressable>
      </View>

      {nav?.loading ? <View style={styles.loadingBar} /> : null}

      {ready ? (
        <WalletWebView
          ref={webRef}
          uri={initialUrl}
          style={styles.web}
          onProviderRequest={onProviderRequest}
          onNavigationChange={onNavigationChange}
        />
      ) : (
        <View style={styles.center}>
          <ActivityIndicator color={color.accent.base} />
          <Text style={styles.dim}>Preparing wallet…</Text>
        </View>
      )}

      {/* Bottom bar: back · reload · open in system browser */}
      <View style={styles.bottomBar}>
        <Pressable hitSlop={8} disabled={!nav?.canGoBack} onPress={() => webRef.current?.goBack()} style={styles.iconBtn}>
          <ArrowLeft size={22} color={nav?.canGoBack ? color.fg.base : color.fg.subtle} />
        </Pressable>
        <Pressable hitSlop={8} onPress={() => webRef.current?.reload()} style={styles.iconBtn}>
          <RotateCw size={20} color={color.fg.muted} />
        </Pressable>
        <View style={styles.flex} />
        <Pressable hitSlop={8} onPress={() => openBrowser(nav?.url ?? initialUrl)} style={styles.iconBtn}>
          <ExternalLink size={20} color={color.fg.muted} />
        </Pressable>
      </View>

      {/* Connect-consent sheet */}
      <AppModal visible={!!consent} onClose={rejectConsent}>
        <View style={styles.sheet}>
          <Text style={styles.sheetTitle}>Connect to {consent ? hostOf(consent.origin) : ''}</Text>
          <Text style={styles.sheetBody}>
            This site wants to see your address and ask you to sign. It can’t move funds without your
            approval.
          </Text>
          {activeAccount ? (
            <View style={styles.acctRow}>
              <Text style={styles.acctName}>{activeAccount.name}</Text>
              <Text style={styles.acctAddr}>{shortAddr(activeAccount.address)}</Text>
            </View>
          ) : null}
          <View style={styles.sheetActions}>
            <VelaButton title="Cancel" variant="secondary" onPress={rejectConsent} style={styles.sheetBtn} />
            <VelaButton title="Connect" variant="accent" onPress={approveConsent} style={styles.sheetBtn} />
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

const styles = StyleSheet.create({
  root: { flex: 1, backgroundColor: color.bg.base },
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
  lock: { fontSize: 13 },
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
  bottomBar: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: space.xl,
    paddingHorizontal: space.lg,
    paddingVertical: space.sm,
    borderTopWidth: StyleSheet.hairlineWidth,
    borderTopColor: color.border.base,
  },
  sheet: { gap: space.md, paddingHorizontal: space.xl, paddingTop: space.sm },
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
});
