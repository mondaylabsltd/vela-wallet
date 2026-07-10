/**
 * WalletWebView — JS wrapper for the wallet-owned native dApp-browser WebView.
 *
 * Native views: iOS WKWebView (modules/vela-wallet-webview/ios), Android
 * android.webkit.WebView (…/android). Both inject the SAME provider bundle
 * (INJECTED_PROVIDER_JS, single-sourced with the Safari extension) at document
 * start and bridge the `vela-1193` envelope to RN.
 *
 * native → page delivery uses an `outbox` PROP (a seq-tracked JSON queue), NOT
 * imperative view commands: under the New Architecture (bridgeless) a legacy view
 * manager is not reachable via `NativeModules.<Manager>`, so command dispatch
 * silently no-ops. Props flow reliably through the interop layer (that is how the
 * provider gets injected), so the outbox is the robust channel. Each respond/emit/
 * nav pushes `{seq, …}`; the native side processes items with seq > lastSeq.
 *
 * Native-only: on web there is no in-process WebView with document-start
 * injection, so the component renders null (`isWalletWebViewSupported` is false).
 * See docs/dapp-browser/ARCHITECTURE.md.
 */
import React, { forwardRef, useCallback, useImperativeHandle, useMemo, useRef, useState } from 'react';
import {
  Platform,
  requireNativeComponent,
  type HostComponent,
  type NativeSyntheticEvent,
  type ViewProps,
} from 'react-native';
import { INJECTED_PROVIDER_JS } from './injected-provider.generated';

const COMPONENT = 'WalletWebView';
export const isWalletWebViewSupported = Platform.OS === 'ios' || Platform.OS === 'android';

/** A provider request bubbled up by the native WebView (origin is native-stamped). */
export interface ProviderRequestEvent {
  requestId: string;
  method: string;
  params: unknown[];
  origin: string;
  isMainFrame: boolean;
}

/** Navigation lifecycle for the URL bar + settle-on-navigation. */
export interface NavigationChangeEvent {
  url: string;
  title: string;
  canGoBack: boolean;
  canGoForward: boolean;
  loading: boolean;
  /** Non-empty when the main-frame load failed (DNS/offline/etc.); '' otherwise. */
  error: string;
  /** Absolute URL of the page's favicon ('' until resolved / on failure). */
  favicon: string;
}

interface NativeProps extends ViewProps {
  sourceURL?: string;
  injectedJavaScript?: string;
  /** seq-tracked JSON queue of native→page deliveries (responses/events/nav). */
  outbox?: string;
  onProviderRequest?: (e: NativeSyntheticEvent<ProviderRequestEvent>) => void;
  onNavigationChange?: (e: NativeSyntheticEvent<NavigationChangeEvent>) => void;
}

const NativeWalletWebView: HostComponent<NativeProps> | null = isWalletWebViewSupported
  ? requireNativeComponent<NativeProps>(COMPONENT)
  : null;

/** Imperative handle — the WebViewTransport drives these. */
export interface WalletWebViewHandle {
  respond(id: string, result: unknown, error: { code: number; message: string } | null): void;
  emitEvent(event: string, data: unknown): void;
  goBack(): void;
  goForward(): void;
  reload(): void;
}

export interface WalletWebViewProps extends ViewProps {
  /** The URL to load. */
  uri: string;
  onProviderRequest?: (req: ProviderRequestEvent) => void;
  onNavigationChange?: (nav: NavigationChangeEvent) => void;
}

type Delivery =
  | { t: 'res'; id: string; result: unknown; error: unknown }
  | { t: 'evt'; event: string; data: unknown }
  | { t: 'nav'; action: 'back' | 'forward' | 'reload' };
type OutboxItem = Delivery & { seq: number };

export const WalletWebView = forwardRef<WalletWebViewHandle, WalletWebViewProps>(
  function WalletWebView({ uri, onProviderRequest, onNavigationChange, ...viewProps }, ref) {
    const seqRef = useRef(0);
    const queueRef = useRef<OutboxItem[]>([]);
    const [outbox, setOutbox] = useState('[]');

    const push = useCallback((item: Delivery) => {
      seqRef.current += 1;
      queueRef.current.push({ seq: seqRef.current, ...item });
      // Bounded tail — native processes each prop update promptly by seq, so a
      // generous window is always enough; this just caps memory. If we ever hit the
      // cap, the trim is surfaced (not silent) — a dropped low-seq delivery would
      // leave a page promise hanging, so it must be visible in dev, not swallowed.
      const CAP = 256;
      if (queueRef.current.length > CAP) {
        if (__DEV__) {
          console.warn(
            `[WalletWebView] outbox exceeded ${CAP} undelivered items — trimming oldest; ` +
              'a page request may hang. This implies native is not draining the prop.',
          );
        }
        queueRef.current = queueRef.current.slice(-CAP);
      }
      setOutbox(JSON.stringify(queueRef.current));
    }, []);

    useImperativeHandle(
      ref,
      () => ({
        respond: (id, result, error) => push({ t: 'res', id, result: result === undefined ? null : result, error: error ?? null }),
        emitEvent: (event, data) => push({ t: 'evt', event, data: data ?? null }),
        goBack: () => push({ t: 'nav', action: 'back' }),
        goForward: () => push({ t: 'nav', action: 'forward' }),
        reload: () => push({ t: 'nav', action: 'reload' }),
      }),
      [push],
    );

    const injected = useMemo(() => INJECTED_PROVIDER_JS, []);

    if (!NativeWalletWebView) return null; // web / unsupported — native-only surface

    return (
      <NativeWalletWebView
        sourceURL={uri}
        injectedJavaScript={injected}
        outbox={outbox}
        onProviderRequest={onProviderRequest ? (e) => onProviderRequest(e.nativeEvent) : undefined}
        onNavigationChange={onNavigationChange ? (e) => onNavigationChange(e.nativeEvent) : undefined}
        {...viewProps}
      />
    );
  },
);
