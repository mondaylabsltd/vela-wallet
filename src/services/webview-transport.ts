// WebViewTransport — the 4th DAppTransport, backing the in-app dApp browser.
//
// Unlike RemoteInject (SSE), WalletPair (WS), and ExtensionBridge (one-shot App
// Group file), this transport wraps a LIVE, in-process channel to a native
// WalletWebView: provider requests arrive on the WebView's message handler and
// responses/events go straight back into the page. Because the channel is live
// and same-process, three whole classes of Safari-only complexity disappear
// (cold-launch race, UL attestation, account-snapshot staleness).
//
// It plugs into the existing pipeline exactly like the extension: install it via
// the transient pattern (never `wireTransport`) so every request carries
// `__transport`/`__chainId`/`__dapp`, then `handleIncoming` → `SigningRequestModal`
// renders the real signing UI for free (clear-signing, asset-sim, gas/funding,
// passkey, ERC-4337, persist-at-submit).
//
// See docs/dapp-browser/ARCHITECTURE.md.
import type { DAppTransport, DAppTransportEvents, WalletInfo, DAppInfo } from './dapp-transport';

/**
 * The imperative side of the native WalletWebView, as seen by the transport.
 * The browser screen wires these to the native view's `respond` / `emitProviderEvent`
 * commands. The transport passes plain values — the injected `vela-1193` shim owns
 * the envelope, so this layer never needs to know CHANNEL (no cross-language drift).
 */
export interface WalletWebViewBridge {
  /** Resolve/reject the page's pending EIP-1193 promise for `id`. */
  respond(id: string, result: unknown, error: { code: number; message: string } | null): void;
  /** Push an EIP-1193 event (`accountsChanged` / `chainChanged` / …) into the page. */
  emitEvent(event: string, data: unknown): void;
}

type Listeners = { [K in keyof DAppTransportEvents]: Set<DAppTransportEvents[K]> };

export class WebViewTransport implements DAppTransport {
  readonly name = 'In-App Browser';

  private _connected = false;
  private _dapp: DAppInfo | null;
  private readonly bridge: WalletWebViewBridge;
  /**
   * The chain the in-app browser is currently on. Read by `beginExtensionSign` so a
   * forwarded signing request takes handleIncoming's PER-REQUEST-chain path (F4):
   *   - the sign sheet / SIWE anti-phishing shows THIS dApp (`__dapp`), not a
   *     concurrent relay session's global dappInfo, and
   *   - a browser tx never mutates the GLOBAL chainId shared with a relay session.
   * The browser screen keeps this in sync with the wallet's active chain.
   */
  requestChainId?: number;
  /** Requests emitted upstream but not yet responded to — bounds memory + makes
   *  sendResponse idempotent (exactly one response per request id). */
  private readonly pending = new Set<string>();
  private readonly listeners: Listeners = {
    connected: new Set(),
    disconnected: new Set(),
    reconnecting: new Set(),
    request: new Set(),
    error: new Set(),
  };

  constructor(bridge: WalletWebViewBridge, dapp?: DAppInfo | null) {
    this.bridge = bridge;
    this._dapp = dapp ?? null;
  }

  get connected(): boolean {
    return this._connected;
  }

  async connect(): Promise<void> {
    this._connected = true;
    this.emit('connected', this.name);
  }

  disconnect(): void {
    if (!this._connected) return;
    this._connected = false;
    // Any promise still open when the browser closes settles as unknown-pending
    // (4900) — never a false 4001, which a dApp treats as safe-to-retry.
    this.settlePending({ code: 4900, message: 'Browser closed — check Vela Activity' });
    this.emit('disconnected');
  }

  /**
   * Settle every in-flight page promise with a terminal error. Called on
   * navigation away / reload / close (settle-on-navigation, ARCHITECTURE.md §5.3).
   * MUST use a non-4001 code (4900 unknown-pending) — a dApp retries an explicit
   * 4001 "user rejected", which would double-spend a request that may have landed.
   */
  settlePending(error: { code: number; message: string }): void {
    for (const id of this.pending) {
      try {
        this.bridge.respond(id, undefined, error);
      } catch {
        /* view gone */
      }
    }
    this.pending.clear();
  }

  /**
   * Feed a provider request bubbled up by the native WebView.
   *
   * `origin` MUST be the native-observed committed origin (iOS
   * `frameInfo.securityOrigin`, Android `sourceOrigin`) — NEVER a value the page
   * put in the message body. `isMainFrame` gates signing upstream (a cross-origin
   * iframe must not be able to request accounts or signatures).
   */
  handleProviderRequest(
    id: string,
    method: string,
    params: unknown[],
    origin: string,
    isMainFrame: boolean,
  ): void {
    if (!this._connected) return;
    // Security: iframe provider traffic never reaches the signing pipeline.
    if (!isMainFrame) {
      this.bridge.respond(id, undefined, { code: 4100, message: 'Unauthorized frame' });
      return;
    }
    this.pending.add(id);
    this.emit('request', id, method, Array.isArray(params) ? params : [], origin);
  }

  sendResponse(id: string, result?: unknown, error?: { code: number; message: string }): void {
    // Idempotent: only the first response for a known id reaches the page.
    if (!this.pending.delete(id)) return;
    try {
      this.bridge.respond(id, error ? undefined : result, error ?? null);
    } catch {
      /* view already torn down; drop */
    }
  }

  pushWalletInfo(info: WalletInfo): void {
    // A live channel exists (unlike the Safari extension, which no-ops this).
    try {
      this.bridge.emitEvent('accountsChanged', info.address ? [info.address] : []);
      this.bridge.emitEvent('chainChanged', toHexChainId(info.chainId));
    } catch {
      /* view already torn down; drop */
    }
  }

  async fetchDAppInfo(): Promise<DAppInfo | null> {
    return this._dapp;
  }

  /** The browser screen updates identity as the tab navigates. */
  setDAppInfo(info: DAppInfo | null): void {
    this._dapp = info;
  }

  on<K extends keyof DAppTransportEvents>(event: K, listener: DAppTransportEvents[K]): () => void {
    this.listeners[event].add(listener);
    return () => {
      this.listeners[event].delete(listener);
    };
  }

  private emit<K extends keyof DAppTransportEvents>(
    event: K,
    ...args: Parameters<DAppTransportEvents[K]>
  ): void {
    for (const listener of this.listeners[event]) {
      try {
        (listener as (...a: unknown[]) => void)(...args);
      } catch {
        /* a throwing listener must not break the transport */
      }
    }
  }
}

/** `chainId` number → EIP-1193 hex string (`1` → `"0x1"`). */
function toHexChainId(chainId: number): string {
  return '0x' + Math.max(0, Math.floor(chainId)).toString(16);
}
